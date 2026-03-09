# ForgeMemory — Custom AI Memory Engine for ImpForge IDE

**Date**: 2026-03-09
**Status**: Approved
**Approach**: C — SQLite Core + Custom Rust AI Layer

## Executive Summary

ForgeMemory is ImpForge's proprietary AI memory engine — a standalone, embedded,
zero-config knowledge system that gives the IDE persistent memory across sessions.
It combines battle-tested SQLite storage with custom Rust-native AI layers for
vector search (HNSW), full-text ranking (BM25), and hybrid retrieval (RRF fusion).

Architecture derived from ork-station's 94-table PostgreSQL schema, distilled to
~25 tables optimized for a standalone desktop IDE.

## Scientific Foundations

| Concept | Paper | Application |
|---------|-------|-------------|
| HNSW Vector Index | Malkov & Yashunin 2018 (arXiv:1603.09320) | Approximate k-NN for semantic search |
| BM25 Ranking | Robertson & Zaragoza 2009 | Full-text relevance scoring |
| Reciprocal Rank Fusion | Cormack et al. 2009 | Combining BM25 + vector results |
| MemGPT Tiered Memory | Packer et al. 2023 (arXiv:2310.08560) | Core/Recall/Archival memory tiers |
| FSRS-5 Spaced Repetition | Jarrett Ye 2022-2024 | Memory consolidation scheduling |
| Event Sourcing | Fowler 2005 | Immutable task/audit ledger |
| Knowledge Tiers | ork-station pattern | golden/verified/medium/unverified |
| Co-Change Coupling | Ball et al. 1997 | File relationship detection |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    ForgeMemory Engine (Rust)                     │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐│
│  │ Tiered Memory│  │ Knowledge    │  │ Hybrid Search Pipeline ││
│  │ (MemGPT)     │  │ Graph Engine │  │ (BM25 + Vector + RRF) ││
│  │              │  │              │  │                        ││
│  │ Core: ≤100   │  │ Nodes: typed │  │  ┌─────┐  ┌────────┐ ││
│  │ Recall: ∞    │  │ Edges: typed │  │  │BM25 │  │HNSW Vec│ ││
│  │ Archival: ∞  │  │ Traversal    │  │  │Index│  │ Index  │ ││
│  └──────┬───────┘  │ Temporal     │  │  └──┬──┘  └───┬────┘ ││
│         │          └──────┬───────┘  │     └────┬────┘      ││
│         │                 │          │     RRF Fusion        ││
│         └────────┬────────┘          └──────────┼───────────┘│
│                  ▼                              │             │
│  ┌──────────────────────────────────────────────▼───────────┐│
│  │              SQLite Storage Layer (WAL Mode)              ││
│  │  25 tables | 15+ indexes | Event Sourcing | ACID         ││
│  │  ~/.impforge/forge_memory.db (single portable file)      ││
│  └──────────────────────────────────────────────────────────┘│
│                                                               │
│  ┌──────────────────────────────────────────────────────────┐│
│  │           Embedding Providers (fastembed + Ollama)        ││
│  │  Default: all-MiniLM-L6-v2 (384-dim, ONNX, offline)     ││
│  │  Optional: Ollama nomic-embed-text (768-dim, GPU)        ││
│  └──────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

## SQLite Schema (25 Tables)

### Group A: Core Memory & Knowledge

```sql
-- MemGPT-inspired tiered memory (Packer et al. 2023)
CREATE TABLE memories (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    scope       TEXT NOT NULL CHECK (scope IN ('core','recall','archival')),
    category    TEXT NOT NULL DEFAULT 'general',
    key         TEXT,
    content     TEXT NOT NULL,
    importance  REAL NOT NULL DEFAULT 0.5 CHECK (importance BETWEEN 0.0 AND 1.0),
    embedding   BLOB,  -- f32 array, 384-dim or 768-dim
    -- FSRS-5 fields (Jarrett Ye 2022-2024)
    stability   REAL NOT NULL DEFAULT 1.0,
    difficulty  REAL NOT NULL DEFAULT 0.3,
    reps        INTEGER NOT NULL DEFAULT 0,
    lapses      INTEGER NOT NULL DEFAULT 0,
    next_review TEXT,  -- ISO-8601 datetime
    -- Metadata
    source      TEXT,  -- file_path, chat, user, agent
    tags        TEXT,  -- JSON array
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    accessed_at TEXT,
    expires_at  TEXT   -- NULL = never expires
);
CREATE INDEX idx_memories_scope ON memories(scope);
CREATE INDEX idx_memories_category ON memories(category);
CREATE INDEX idx_memories_importance ON memories(importance DESC);
CREATE INDEX idx_memories_next_review ON memories(next_review);

-- Knowledge tier system (ork-station pattern)
CREATE TABLE knowledge_items (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    tier        TEXT NOT NULL DEFAULT 'unverified'
                CHECK (tier IN ('golden','verified','medium','unverified')),
    category    TEXT NOT NULL DEFAULT 'general',
    importance  INTEGER NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    embedding   BLOB,
    source      TEXT,
    tags        TEXT,  -- JSON array
    metadata    TEXT,  -- JSON object
    access_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    valid_at    TEXT,  -- temporal validity start
    invalid_at  TEXT   -- temporal validity end (NULL = still valid)
);
CREATE INDEX idx_knowledge_tier ON knowledge_items(tier);
CREATE INDEX idx_knowledge_category ON knowledge_items(category);
CREATE INDEX idx_knowledge_importance ON knowledge_items(importance DESC);

-- Knowledge graph nodes
CREATE TABLE kg_nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN (
                    'file','symbol','concept','session','user',
                    'pattern','crate','module','function','type')),
    label       TEXT NOT NULL,
    properties  TEXT,  -- JSON object
    embedding   BLOB,
    confidence  REAL NOT NULL DEFAULT 1.0 CHECK (confidence BETWEEN 0.0 AND 1.0),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_kg_nodes_kind ON kg_nodes(kind);
CREATE INDEX idx_kg_nodes_label ON kg_nodes(label);

-- Knowledge graph edges (temporal validity from ork-station)
CREATE TABLE kg_edges (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    source_id   TEXT NOT NULL REFERENCES kg_nodes(id) ON DELETE CASCADE,
    target_id   TEXT NOT NULL REFERENCES kg_nodes(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN (
                    'contains','references','co_changes','depends_on',
                    'derives','similar','edits','owns','imports','calls')),
    weight      REAL NOT NULL DEFAULT 1.0,
    properties  TEXT,  -- JSON object
    confidence  REAL NOT NULL DEFAULT 1.0,
    valid_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    invalid_at  TEXT,  -- NULL = still valid
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_kg_edges_source ON kg_edges(source_id);
CREATE INDEX idx_kg_edges_target ON kg_edges(target_id);
CREATE INDEX idx_kg_edges_kind ON kg_edges(kind);
CREATE UNIQUE INDEX idx_kg_edges_unique ON kg_edges(source_id, target_id, kind)
    WHERE invalid_at IS NULL;

-- IDE sessions
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    ended_at    TEXT,
    summary     TEXT,
    files_touched TEXT,  -- JSON array of file paths
    metadata    TEXT     -- JSON object
);

-- Compressed context summaries
CREATE TABLE session_summaries (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    session_id  TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    summary     TEXT NOT NULL,
    key_decisions TEXT,  -- JSON array
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
```

### Group B: Task & Event System (Event Sourcing)

```sql
-- Task orchestration (ork-station event sourcing pattern)
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','in_progress','completed','blocked','cancelled')),
    priority    TEXT NOT NULL DEFAULT 'medium'
                CHECK (priority IN ('critical','high','medium','low','batch')),
    assignee    TEXT,
    tags        TEXT,      -- JSON array
    metadata    TEXT,      -- JSON object
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);

-- Immutable event ledger (Event Sourcing — Fowler 2005)
CREATE TABLE task_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL CHECK (event_type IN (
                    'created','status_changed','assigned','commented',
                    'priority_changed','completed','cancelled','reopened')),
    old_value   TEXT,
    new_value   TEXT,
    actor       TEXT NOT NULL DEFAULT 'system',
    metadata    TEXT,  -- JSON object
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_task_events_task ON task_events(task_id);
CREATE INDEX idx_task_events_type ON task_events(event_type);

-- Task dependencies
CREATE TABLE task_dependencies (
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on  TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on)
);

-- Audit log (append-only)
CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    action      TEXT NOT NULL CHECK (action IN ('INSERT','UPDATE','DELETE')),
    old_data    TEXT,  -- JSON
    new_data    TEXT,  -- JSON
    actor       TEXT NOT NULL DEFAULT 'system',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_audit_table ON audit_log(table_name, created_at DESC);
```

### Group C: Code Intelligence

```sql
-- Indexed code chunks with embeddings
CREATE TABLE code_chunks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    file_path   TEXT NOT NULL,
    language    TEXT NOT NULL,
    chunk_type  TEXT NOT NULL CHECK (chunk_type IN (
                    'function','class','struct','enum','trait','impl',
                    'interface','type','module','import','comment','block')),
    name        TEXT,       -- symbol name
    content     TEXT NOT NULL,
    signature   TEXT,       -- type signature for cross-file context
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    embedding   BLOB,
    parent_id   TEXT REFERENCES code_chunks(id) ON DELETE SET NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_chunks_file ON code_chunks(file_path);
CREATE INDEX idx_chunks_name ON code_chunks(name);
CREATE INDEX idx_chunks_type ON code_chunks(chunk_type);
CREATE INDEX idx_chunks_lang ON code_chunks(language);

-- File metadata for incremental indexing
CREATE TABLE file_index (
    file_path   TEXT PRIMARY KEY,
    language    TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    mtime       TEXT NOT NULL,    -- ISO-8601 file modification time
    hash        TEXT NOT NULL,    -- SHA-256 content hash
    line_count  INTEGER NOT NULL,
    symbol_count INTEGER NOT NULL DEFAULT 0,
    last_indexed TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_file_mtime ON file_index(mtime DESC);

-- AI completion analytics
CREATE TABLE completion_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path   TEXT NOT NULL,
    language    TEXT NOT NULL,
    model_used  TEXT NOT NULL,
    complexity  TEXT NOT NULL CHECK (complexity IN ('simple','medium','complex')),
    latency_ms  INTEGER NOT NULL,
    from_cache  INTEGER NOT NULL DEFAULT 0,  -- boolean
    accepted    INTEGER,  -- NULL=unknown, 1=accepted, 0=dismissed
    prefix_hash TEXT NOT NULL,
    completion  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_completion_model ON completion_history(model_used);
CREATE INDEX idx_completion_time ON completion_history(created_at DESC);

-- Persistent chat history
CREATE TABLE chat_conversations (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT,
    model_id    TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE chat_messages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('user','assistant','system')),
    content     TEXT NOT NULL,
    model_id    TEXT,
    tokens_used INTEGER,
    latency_ms  INTEGER,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_chat_conv ON chat_messages(conversation_id);

-- Evaluation results (Agent-as-a-Judge)
CREATE TABLE evaluation_results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_output TEXT NOT NULL,
    grader_score REAL,
    critic_feedback TEXT,
    defender_response TEXT,
    meta_verdict TEXT,
    final_score REAL NOT NULL,
    dimensions  TEXT,  -- JSON: {correctness, completeness, safety, style, reasoning}
    model_used  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
```

### Group D: Search & AI Infrastructure

```sql
-- BM25 inverted index (Robertson & Zaragoza 2009)
CREATE TABLE bm25_terms (
    term        TEXT NOT NULL,
    doc_id      TEXT NOT NULL,
    doc_table   TEXT NOT NULL,  -- which table: memories, knowledge_items, code_chunks, etc.
    tf          INTEGER NOT NULL,  -- term frequency in document
    positions   TEXT,  -- JSON array of positions for highlighting
    PRIMARY KEY (term, doc_id, doc_table)
);
CREATE INDEX idx_bm25_term ON bm25_terms(term);
CREATE INDEX idx_bm25_doc ON bm25_terms(doc_id, doc_table);

-- BM25 document statistics
CREATE TABLE bm25_doc_stats (
    doc_id      TEXT NOT NULL,
    doc_table   TEXT NOT NULL,
    doc_length  INTEGER NOT NULL,  -- total terms in document
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (doc_id, doc_table)
);

-- BM25 corpus statistics
CREATE TABLE bm25_corpus_stats (
    doc_table   TEXT PRIMARY KEY,
    total_docs  INTEGER NOT NULL DEFAULT 0,
    avg_doc_length REAL NOT NULL DEFAULT 0.0,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Embedding cache (avoid recomputation)
CREATE TABLE embeddings_cache (
    content_hash TEXT PRIMARY KEY,  -- SHA-256 of input text
    model_id    TEXT NOT NULL,
    dimensions  INTEGER NOT NULL,
    embedding   BLOB NOT NULL,     -- f32 array
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Domain keyword boosting (ork-station pattern)
CREATE TABLE keywords (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword     TEXT NOT NULL UNIQUE,
    category    TEXT NOT NULL DEFAULT 'general',
    boost       REAL NOT NULL DEFAULT 1.0,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- System configuration (key-value)
CREATE TABLE system_config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general',
    description TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Precomputed context cache
CREATE TABLE context_cache (
    cache_key   TEXT PRIMARY KEY,
    content     TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_context_expires ON context_cache(expires_at);

-- Model configurations
CREATE TABLE model_configs (
    model_id    TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    provider    TEXT NOT NULL,  -- ollama, openrouter, local
    token_budget INTEGER NOT NULL DEFAULT 4096,
    embedding_dim INTEGER,
    supports_fim INTEGER NOT NULL DEFAULT 0,  -- boolean
    config      TEXT,  -- JSON: temperature, top_p, etc.
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Search history for user patterns
CREATE TABLE search_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    query       TEXT NOT NULL,
    result_count INTEGER NOT NULL DEFAULT 0,
    search_type TEXT NOT NULL DEFAULT 'hybrid',  -- hybrid, bm25, vector, code
    latency_ms  INTEGER,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_search_time ON search_history(created_at DESC);
```

### Schema Migrations

```sql
-- Version tracking for auto-migration
CREATE TABLE schema_version (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO schema_version (version, description) VALUES (1, 'Initial ForgeMemory schema');
```

## Custom Rust AI Layer

### HNSW Vector Index (Malkov & Yashunin 2018)

Custom implementation in `vector.rs`:
- **Layers**: L0 = all nodes (flat), higher layers = exponentially fewer nodes
- **Distance**: Cosine similarity (1 - dot(a,b) / (|a|*|b|))
- **Parameters**: M=16 (connections per layer), ef_construction=200, ef_search=50
- **Persistence**: Node data in SQLite, graph structure in-memory (rebuilt on startup)
- **Insert**: O(log n) amortized
- **Search**: O(log n) approximate k-NN

### BM25 Scoring Engine (Robertson & Zaragoza 2009)

Custom implementation in `bm25.rs`:
- **Formula**: score(D,Q) = SUM[ IDF(qi) * (f(qi,D) * (k1+1)) / (f(qi,D) + k1*(1-b+b*|D|/avgdl)) ]
- **Parameters**: k1=1.5, b=0.75 (tunable)
- **Tokenizer**: Unicode-aware, language-specific (code-aware: splits camelCase, snake_case)
- **Stemmer**: rust-stemmers crate (Snowball algorithm)
- **Inverted Index**: Stored in bm25_terms table
- **IDF**: Precomputed in bm25_corpus_stats

### Hybrid Search Pipeline (RRF — Cormack et al. 2009)

Custom implementation in `search.rs`:
- **RRF Formula**: score(d) = SUM[ 1/(k + rank_i(d)) ] with k=60
- **Pipeline**: Query → [BM25 top-50] + [HNSW top-50] → RRF merge → top-K
- **Temporal boost**: recent_score = base_score * (1 + 0.1 * recency_factor)
- **Keyword boost**: Check keywords table for domain-specific boosting
- **Deduplication**: Same doc from both sources merged, scores combined

### Embedding Providers (fastembed + Ollama)

Implementation in `embeddings.rs`:
- **Default**: fastembed-rs with all-MiniLM-L6-v2 (384-dim, ONNX, offline)
- **Optional**: Ollama nomic-embed-text (768-dim, requires Ollama running)
- **Caching**: embeddings_cache table (SHA-256 content hash → vector)
- **Batch**: Process up to 32 texts at once for throughput
- **Fallback**: If fastembed fails, try Ollama; if both fail, skip embedding

### MemGPT Tiered Memory (Packer et al. 2023)

Implementation in `memory.rs`:
- **Core Memory**: ≤100 items, always loaded, highest importance
- **Recall Memory**: Unlimited, searchable via hybrid search
- **Archival Memory**: Compressed, long-term, FSRS-scheduled reviews
- **Promotion**: recall→core when importance > 0.8 and access_count > 5
- **Demotion**: core→recall when not accessed for 7 days
- **Consolidation**: FSRS-5 scheduling (already in brain.rs, reused)

## Cargo Dependencies (New)

```toml
fastembed = "4"              # ONNX embeddings (MIT)
rust-stemmers = "1.2"        # BM25 stemming (MIT)
unicode-segmentation = "1.11" # Tokenization (MIT/Apache-2.0)
sha2 = "0.10"                # Content hashing (MIT/Apache-2.0)
```

## File Structure

```
src-tauri/src/forge_memory/
├── mod.rs           # Public API + Tauri commands (~200 LoC)
├── store.rs         # SQLite schema + migrations + CRUD (~600 LoC)
├── vector.rs        # Custom HNSW vector index (~400 LoC)
├── bm25.rs          # Custom BM25 scoring engine (~350 LoC)
├── search.rs        # Hybrid search pipeline + RRF (~300 LoC)
├── memory.rs        # MemGPT tiered memory (~350 LoC)
├── graph.rs         # Knowledge graph engine (~400 LoC)
├── embeddings.rs    # fastembed + Ollama providers (~250 LoC)
├── indexer.rs       # Code intelligence indexer (~300 LoC)
└── migration.rs     # Schema versioning (~100 LoC)
```

**Estimated total**: ~3,250 LoC Rust

## Tauri Command API

```
forge_memory_store, forge_memory_recall, forge_memory_forget,
forge_memory_stats, forge_memory_compact, forge_memory_export, forge_memory_import,
forge_kg_add_node, forge_kg_add_edge, forge_kg_traverse, forge_kg_analytics,
forge_search, forge_search_code, forge_search_similar,
forge_index_file, forge_index_workspace, forge_get_symbols,
forge_chat_save, forge_chat_history, forge_eval_save, forge_eval_history
```

## Sources

- [HNSW Paper (Malkov & Yashunin 2018)](https://arxiv.org/abs/1603.09320)
- [BM25 (Robertson & Zaragoza 2009)](https://en.wikipedia.org/wiki/Okapi_BM25)
- [MemGPT (Packer et al. 2023)](https://arxiv.org/abs/2310.08560)
- [FSRS-5 (Jarrett Ye 2022-2024)](https://github.com/open-spaced-repetition/fsrs-rs)
- [RRF (Cormack et al. 2009)](https://opensearch.org/blog/introducing-reciprocal-rank-fusion-hybrid-search/)
- [fastembed-rs](https://github.com/Anush008/fastembed-rs)
- [sqlite-vec](https://github.com/asg017/sqlite-vec)
- [rust-stemmers](https://crates.io/crates/rust-stemmers)
