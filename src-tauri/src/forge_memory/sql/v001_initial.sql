-- ForgeMemory v001 — Initial 25-Table Schema
--
-- Groups:
--   A. Core Memory & Knowledge (6 tables)
--   B. Task & Event System (4 tables)
--   C. Code Intelligence (6 tables)
--   D. Search & AI Infrastructure (9 tables)
--
-- References:
--   - MemGPT Tiered Memory (Packer et al. 2023)
--   - FSRS-5 Spaced Repetition (Jarrett Ye 2022-2024)
--   - BM25 (Robertson & Zaragoza 2009)
--   - HNSW (Malkov & Yashunin 2018)

-- ============================================================================
-- GROUP A: Core Memory & Knowledge (6 tables)
-- ============================================================================

-- MemGPT-inspired tiered memory (Packer et al. 2023)
CREATE TABLE IF NOT EXISTS memories (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    scope       TEXT NOT NULL CHECK (scope IN ('core','recall','archival')),
    category    TEXT NOT NULL DEFAULT 'general',
    key         TEXT,
    content     TEXT NOT NULL,
    importance  REAL NOT NULL DEFAULT 0.5 CHECK (importance BETWEEN 0.0 AND 1.0),
    embedding   BLOB,
    stability   REAL NOT NULL DEFAULT 1.0,
    difficulty  REAL NOT NULL DEFAULT 0.3,
    reps        INTEGER NOT NULL DEFAULT 0,
    lapses      INTEGER NOT NULL DEFAULT 0,
    next_review TEXT,
    source      TEXT,
    tags        TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    accessed_at TEXT,
    expires_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);
CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance DESC);
CREATE INDEX IF NOT EXISTS idx_memories_next_review ON memories(next_review);

-- Knowledge tier system
CREATE TABLE IF NOT EXISTS knowledge_items (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    tier        TEXT NOT NULL DEFAULT 'unverified'
                CHECK (tier IN ('golden','verified','medium','unverified')),
    category    TEXT NOT NULL DEFAULT 'general',
    importance  INTEGER NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    embedding   BLOB,
    source      TEXT,
    tags        TEXT,
    metadata    TEXT,
    access_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    valid_at    TEXT,
    invalid_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_knowledge_tier ON knowledge_items(tier);
CREATE INDEX IF NOT EXISTS idx_knowledge_category ON knowledge_items(category);
CREATE INDEX IF NOT EXISTS idx_knowledge_importance ON knowledge_items(importance DESC);

-- Knowledge graph nodes
CREATE TABLE IF NOT EXISTS kg_nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN (
                    'file','symbol','concept','session','user',
                    'pattern','crate','module','function','type')),
    label       TEXT NOT NULL,
    properties  TEXT,
    embedding   BLOB,
    confidence  REAL NOT NULL DEFAULT 1.0 CHECK (confidence BETWEEN 0.0 AND 1.0),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_kind ON kg_nodes(kind);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_label ON kg_nodes(label);

-- Knowledge graph edges
CREATE TABLE IF NOT EXISTS kg_edges (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    source_id   TEXT NOT NULL REFERENCES kg_nodes(id) ON DELETE CASCADE,
    target_id   TEXT NOT NULL REFERENCES kg_nodes(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN (
                    'contains','references','co_changes','depends_on',
                    'derives','similar','edits','owns','imports','calls')),
    weight      REAL NOT NULL DEFAULT 1.0,
    properties  TEXT,
    confidence  REAL NOT NULL DEFAULT 1.0,
    valid_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    invalid_at  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_kg_edges_source ON kg_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_kg_edges_target ON kg_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_kg_edges_kind ON kg_edges(kind);

-- Sessions
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    ended_at    TEXT,
    summary     TEXT,
    files_touched TEXT,
    metadata    TEXT
);

-- Session summaries
CREATE TABLE IF NOT EXISTS session_summaries (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    session_id  TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    summary     TEXT NOT NULL,
    key_decisions TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP B: Task & Event System (4 tables)
-- ============================================================================

-- Task orchestration
CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','in_progress','completed','blocked','cancelled')),
    priority    TEXT NOT NULL DEFAULT 'medium'
                CHECK (priority IN ('critical','high','medium','low','batch')),
    assignee    TEXT,
    tags        TEXT,
    metadata    TEXT,
    embedding   BLOB,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);

-- Immutable event ledger (Event Sourcing)
CREATE TABLE IF NOT EXISTS task_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL CHECK (event_type IN (
                    'created','status_changed','assigned','commented',
                    'priority_changed','completed','cancelled','reopened')),
    old_value   TEXT,
    new_value   TEXT,
    actor       TEXT NOT NULL DEFAULT 'system',
    metadata    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type ON task_events(event_type);

-- Task dependencies
CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on  TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on)
);

-- Audit log
CREATE TABLE IF NOT EXISTS audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    action      TEXT NOT NULL CHECK (action IN ('INSERT','UPDATE','DELETE')),
    old_data    TEXT,
    new_data    TEXT,
    actor       TEXT NOT NULL DEFAULT 'system',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_audit_table ON audit_log(table_name, created_at DESC);

-- ============================================================================
-- GROUP C: Code Intelligence (6 tables)
-- ============================================================================

-- Code chunks with embeddings
CREATE TABLE IF NOT EXISTS code_chunks (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    file_path   TEXT NOT NULL,
    language    TEXT NOT NULL,
    chunk_type  TEXT NOT NULL CHECK (chunk_type IN (
                    'function','class','struct','enum','trait','impl',
                    'interface','type','module','import','comment','block')),
    name        TEXT,
    content     TEXT NOT NULL,
    signature   TEXT,
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    embedding   BLOB,
    parent_id   TEXT REFERENCES code_chunks(id) ON DELETE SET NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_chunks_file ON code_chunks(file_path);
CREATE INDEX IF NOT EXISTS idx_chunks_name ON code_chunks(name);
CREATE INDEX IF NOT EXISTS idx_chunks_type ON code_chunks(chunk_type);
CREATE INDEX IF NOT EXISTS idx_chunks_lang ON code_chunks(language);

-- File metadata for incremental indexing
CREATE TABLE IF NOT EXISTS file_index (
    file_path   TEXT PRIMARY KEY,
    language    TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    mtime       TEXT NOT NULL,
    hash        TEXT NOT NULL,
    line_count  INTEGER NOT NULL,
    symbol_count INTEGER NOT NULL DEFAULT 0,
    last_indexed TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_file_mtime ON file_index(mtime DESC);

-- AI completion analytics
CREATE TABLE IF NOT EXISTS completion_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path   TEXT NOT NULL,
    language    TEXT NOT NULL,
    model_used  TEXT NOT NULL,
    complexity  TEXT NOT NULL CHECK (complexity IN ('simple','medium','complex')),
    latency_ms  INTEGER NOT NULL,
    from_cache  INTEGER NOT NULL DEFAULT 0,
    accepted    INTEGER,
    prefix_hash TEXT NOT NULL,
    completion  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_completion_model ON completion_history(model_used);
CREATE INDEX IF NOT EXISTS idx_completion_time ON completion_history(created_at DESC);

-- Chat conversations
CREATE TABLE IF NOT EXISTS chat_conversations (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title       TEXT,
    model_id    TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Chat messages
CREATE TABLE IF NOT EXISTS chat_messages (
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
CREATE INDEX IF NOT EXISTS idx_chat_conv ON chat_messages(conversation_id);

-- Evaluation results
CREATE TABLE IF NOT EXISTS evaluation_results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_output TEXT NOT NULL,
    grader_score REAL,
    critic_feedback TEXT,
    defender_response TEXT,
    meta_verdict TEXT,
    final_score REAL NOT NULL,
    dimensions  TEXT,
    model_used  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ============================================================================
-- GROUP D: Search & AI Infrastructure (9 tables)
-- ============================================================================

-- BM25 inverted index terms
CREATE TABLE IF NOT EXISTS bm25_terms (
    term        TEXT NOT NULL,
    doc_id      TEXT NOT NULL,
    doc_table   TEXT NOT NULL,
    tf          INTEGER NOT NULL,
    positions   TEXT,
    PRIMARY KEY (term, doc_id, doc_table)
);
CREATE INDEX IF NOT EXISTS idx_bm25_term ON bm25_terms(term);
CREATE INDEX IF NOT EXISTS idx_bm25_doc ON bm25_terms(doc_id, doc_table);

-- BM25 document statistics
CREATE TABLE IF NOT EXISTS bm25_doc_stats (
    doc_id      TEXT NOT NULL,
    doc_table   TEXT NOT NULL,
    doc_length  INTEGER NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (doc_id, doc_table)
);

-- BM25 corpus statistics
CREATE TABLE IF NOT EXISTS bm25_corpus_stats (
    doc_table   TEXT PRIMARY KEY,
    total_docs  INTEGER NOT NULL DEFAULT 0,
    avg_doc_length REAL NOT NULL DEFAULT 0.0,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Embedding cache
CREATE TABLE IF NOT EXISTS embeddings_cache (
    content_hash TEXT PRIMARY KEY,
    model_id    TEXT NOT NULL,
    dimensions  INTEGER NOT NULL,
    embedding   BLOB NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Domain keyword boosting
CREATE TABLE IF NOT EXISTS keywords (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword     TEXT NOT NULL UNIQUE,
    category    TEXT NOT NULL DEFAULT 'general',
    boost       REAL NOT NULL DEFAULT 1.0,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- System configuration
CREATE TABLE IF NOT EXISTS system_config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general',
    description TEXT,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Context cache
CREATE TABLE IF NOT EXISTS context_cache (
    cache_key   TEXT PRIMARY KEY,
    content     TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_context_expires ON context_cache(expires_at);

-- Model configurations
CREATE TABLE IF NOT EXISTS model_configs (
    model_id    TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    provider    TEXT NOT NULL,
    token_budget INTEGER NOT NULL DEFAULT 4096,
    embedding_dim INTEGER,
    supports_fim INTEGER NOT NULL DEFAULT 0,
    config      TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Search history
CREATE TABLE IF NOT EXISTS search_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    query       TEXT NOT NULL,
    result_count INTEGER NOT NULL DEFAULT 0,
    search_type TEXT NOT NULL DEFAULT 'hybrid',
    latency_ms  INTEGER,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_search_time ON search_history(created_at DESC);
