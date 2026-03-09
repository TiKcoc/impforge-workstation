# ForgeMemory v3 — Retrieval Quality Upgrade Design

**Date**: 2026-03-09
**Status**: Approved
**Author**: Claude Opus 4.6 + User

## Goal

Upgrade ForgeMemory's ingestion pipeline from ~42% to 70%+ Recall@5 through 4 compounding, scientifically-grounded improvements: AST-aware chunking, notify v8 upgrade, two-tier debouncing, and contextualized chunks.

## Architecture

Four pillars compound multiplicatively:

1. **AST-aware chunking** via `text-splitter` CodeSplitter + 34 tree-sitter grammars (replaces hand-rolled brace-counting)
2. **notify v7→v8.2** upgrade with `notify-debouncer-full` for OS-level event coalescing
3. **Two-tier debouncing** (OS 2s + app 30s batch) replacing single-tier 500ms
4. **Contextualized Chunk Headers (CCH)** — programmatic prepending of file path, scope chain, imports, signatures

```
ForgeWatch v2 (notify 8.2)
  ├── OS Debounce (2s, notify-debouncer-full)
  └── App Batch (30s window, dedup + coalesce)
         │
         ▼
AST Chunking Engine
  ├── text-splitter CodeSplitter + 34 tree-sitter grammars
  └── Fallback: improved sliding window for unknown languages
         │
         ▼
Chunk Contextualization (CCH)
  ├── Code: file_path + module + scope_chain + signature + imports
  ├── Markdown: file_path + section_hierarchy + tags
  └── Config: file_path + section + format
         │
         ▼
Existing Pipeline: Dedup → Embed → Store → KG
```

## Scientific Foundations

| Paper/Source | Finding | Impact |
|---|---|---|
| cAST (arXiv:2506.15655, EMNLP 2025) | AST-aware chunking: +4.3 Recall@5, +6.7 Pass@1 vs fixed-size | Pillar 1 |
| Anthropic Contextual Retrieval (2024) | Chunk contextualization: -49% retrieval failure rate (with BM25) | Pillar 4 |
| CCH (NirDiamant RAG Techniques) | Programmatic headers: +27.9% average retrieval score, no LLM needed | Pillar 4 |
| Supermemory code-chunk | AST chunking: 70.1% vs 49.0% Recall@5 (vs chonkie-code) | Pillar 1 |
| notify-rs ecosystem | v8.2.0 stable, official debouncer-full for OS-level coalescing | Pillars 2-3 |

## Pillar 1: AST-Aware Chunking

**Replace**: ~250 lines of hand-rolled `chunk_curly_brace_language()`, `chunk_indentation_language()`, `chunk_markdown()` in `watch.rs`

**With**: `text-splitter` v0.29 `CodeSplitter` (already in Cargo.toml, just needs `code` feature flag)

**34 tree-sitter grammars** (full language coverage):
- **Systems**: Rust, C, C++, Go, Zig
- **JVM**: Java, Kotlin, Scala
- **Apple**: Swift, Dart
- **Scripting**: Python, Ruby, PHP, Lua, Elixir, R, Julia
- **Web**: TypeScript, JavaScript, HTML, CSS, SCSS, Svelte, Vue
- **Shell**: Bash
- **Config**: TOML, YAML, JSON, XML
- **Data**: SQL, GraphQL, Protobuf
- **Docs**: Markdown
- **Enterprise**: C#

**API** (replaces 250 lines with ~30):
```rust
let splitter = CodeSplitter::new(tree_sitter_rust::LANGUAGE, MAX_CHUNK_CHARS)?;
let chunks = splitter.chunks(source_code);
```

## Pillar 2: notify v7 → v8.2

**Breaking changes** (all already satisfied):
- MSRV 1.77 — our `rust-version = "1.77.2"` passes
- `crossbeam` feature → `crossbeam-channel` — we don't use it
- `instant` → `web-time` — transparent

**New dependency**: `notify-debouncer-full` for OS-level event coalescing.

## Pillar 3: Two-Tier Debouncing

| Tier | Window | Purpose |
|------|--------|---------|
| OS (notify-debouncer-full) | 2 seconds | Coalesce IDE save events (VS Code: 3-5 events per save) |
| App batch | 30 seconds | Accumulate saves into single ingestion batch |

Replaces current single-tier 500ms manual debounce.

## Pillar 4: Contextualized Chunk Headers (CCH)

**Programmatic, no LLM needed.** Extract metadata via tree-sitter AST traversal:

| Field | Source | Cost |
|-------|--------|------|
| File path (last 3 segments) | Filesystem | Free |
| Module/namespace path | AST parent traversal | ~1ms |
| Scope chain | AST ancestor walk | ~1ms |
| Function/method signature | AST node text | ~1ms |
| Import list (top 10) | AST import nodes | ~1ms |
| Document section heading | Markdown AST | Free |

**Output format** (prepended to each chunk before embedding + BM25):
```
# src/orchestrator/brain.rs
# Module: orchestrator::brain
# Scope: BrainV2 > process_memory
# Defines: pub async fn process_memory(&self, input: MemoryInput) -> Result<MemoryOutput>
# Uses: tokio, serde, HebbianTrust

[actual code chunk]
```

Both BM25 and vector embeddings index the contextualized text. BM25 gains domain keywords from headers; embeddings capture semantic scope.

## Files Changed

| File | Action | Lines ~Changed |
|------|--------|---------------|
| `Cargo.toml` | Modify: upgrade notify, add tree-sitter grammars, add `code` feature | +40 |
| `watch.rs` | Major refactor: CodeSplitter + notify v8 + two-tier debounce | -250, +150 |
| `ingest.rs` | Modify: contextualize chunks before storing | +60 |
| `watch/context.rs` | New: CCH contextualization engine | +200 |

**Net**: Delete ~250 lines of hand-rolled chunking, add ~450 lines of production-quality code.

## Testing Strategy

~70 new tests:
- AST chunking per language (34)
- Chunking quality (5)
- Contextualization correctness (10)
- notify v8 integration (3)
- Two-tier debounce (4)
- Ingestion pipeline E2E (5)
- Regression (10)

## Decision: Full Language Coverage

User chose Option A (full coverage) — all 34 tree-sitter grammars. This maximizes retrieval quality for all supported languages at the cost of increased binary size and compile time.
