# ForgeMemory v3 — Retrieval Quality Upgrade Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade ForgeMemory's ingestion pipeline from ~42% to 70%+ Recall@5 through AST-aware chunking, notify v8, two-tier debouncing, and contextualized chunk headers.

**Architecture:** Replace hand-rolled regex-style chunking with `text-splitter` CodeSplitter backed by 34 tree-sitter grammars. Upgrade filesystem watcher from notify v7 to v8.2 with `notify-debouncer-full` for OS-level event coalescing. Add Contextual Chunk Headers (CCH) — programmatic metadata prepended to every chunk before embedding and BM25 indexing.

**Tech Stack:** Rust, text-splitter 0.29, tree-sitter 0.26, notify 8.2, notify-debouncer-full, rusqlite, Tauri 2.10

**Design Doc:** `docs/plans/2026-03-09-forgememory-v3-retrieval-upgrade-design.md`

---

## Phase 1: Dependencies & Language Registry

### Task 1: Update Cargo.toml

**Files:** Modify `src-tauri/Cargo.toml`

1. Update text-splitter: add `"code"` to features list
2. Change `notify = "7"` to `notify = "8.2"`, add `notify-debouncer-full = "0.4"`
3. Add 35 tree-sitter crates (tree-sitter core + 34 language grammars)
4. Run `cargo check` — verify compilation succeeds
5. Commit

### Task 2: Create tree-sitter language registry

**Files:** Create `src-tauri/src/forge_memory/tree_sitter_langs.rs`, Modify `mod.rs`

1. Create `get_tree_sitter_language(lang: &str) -> Option<Language>` matching all 34 languages
2. Add `pub mod tree_sitter_langs;` to mod.rs
3. Write 6 tests (rust, python, typescript, all-34, unknown, parser-validation)
4. Run tests — all 6 PASS
5. Commit

---

## Phase 2: AST-Aware Chunking

### Task 3: Replace chunk_content() with CodeSplitter

**Files:** Modify `src-tauri/src/forge_memory/watch.rs`

1. Add imports: `text_splitter::CodeSplitter`, `tree_sitter_langs::get_tree_sitter_language`
2. Rewrite `chunk_content()`: try CodeSplitter with tree-sitter grammar, fallback to sliding window
3. Add `find_line_range()` helper for chunk-to-line mapping
4. Delete old functions: `chunk_curly_brace_language`, `chunk_indentation_language`, `chunk_markdown`, `chunk_config`, `split_oversized_chunks` (~250 lines removed)
5. Add 3 new tests (AST rust, AST python, fallback unknown)
6. Run all watch tests — PASS
7. Commit

---

## Phase 3: Contextualized Chunk Headers

### Task 4: Create CCH contextualization module

**Files:** Create `src-tauri/src/forge_memory/context.rs`, Modify `mod.rs`

1. Implement `ChunkContext` struct, `shorten_file_path()`, `extract_imports()` (12 language patterns), `build_context_header()`, `contextualize_chunk()`, `extract_module_path()`
2. Add `pub mod context;` to mod.rs
3. Write 10+ tests (path shortening x3, imports x4, header x2, contextualize x2)
4. Run tests — all PASS
5. Commit

### Task 5: Integrate CCH into ingestion pipeline

**Files:** Modify `watch.rs` (ContentChunk), Modify `ingest.rs`

1. Add `contextualized: Option<String>` field to `ContentChunk`, update all constructors
2. In `ingest_file()`: after chunking, map through `contextualize_chunk()` to set `chunk.contextualized`
3. Use contextualized text for dedup check and storage
4. Run all forge_memory tests — PASS
5. Commit

---

## Phase 4: notify v8 + Two-Tier Debouncing

### Task 6: Upgrade ForgeWatcher to notify v8.2 + debouncer

**Files:** Modify `src-tauri/src/forge_memory/watch.rs` (ForgeWatcher)

1. Add import: `notify_debouncer_full::{new_debouncer, DebounceEventResult}`
2. Rewrite `start()`: replace manual 500ms debounce with `new_debouncer(Duration::from_secs(2), ...)`, register paths via `debouncer.watch()`
3. Run watcher tests — PASS
4. Commit

---

## Phase 5: Final Integration

### Task 7: Integration tests

**Files:** Modify `src-tauri/src/forge_memory/ingest.rs`

1. Add `test_ingest_rust_with_contextualization` — verify CCH headers in stored content
2. Add `test_ingest_typescript_with_imports` — verify TS AST + import extraction
3. Add `test_ingest_directory_with_mixed_languages` — multi-language project
4. Run full test suite — all PASS
5. Commit

### Task 8: Final verification and cleanup

1. Run full `cargo test` — all PASS
2. Fix any unused import warnings from `cargo check`
3. Commit cleanup
4. Done

---

## Summary

| Task | Description | ~Lines |
|------|-------------|--------|
| 1 | Cargo.toml dependencies | +45 |
| 2 | Tree-sitter language registry | +120 |
| 3 | AST-aware CodeSplitter | -250, +80 |
| 4 | CCH contextualization module | +250 |
| 5 | CCH pipeline integration | +30 |
| 6 | notify v8 + debouncer | -40, +60 |
| 7 | Integration tests | +80 |
| 8 | Cleanup | ~0 |

**Net**: +375 lines, ~22 new tests, massive retrieval quality upgrade.
