-- ForgeMemory v002 — Enterprise Expansion (55 new tables)
--
-- Migrated from ork-station PostgreSQL (155 tables across 5 schemas).
-- All ork-station/ImpUI/ImpOS/AiImp/systemd/Redis/PG-specific references removed.
-- Every table is standalone SQLite-compatible for any customer's IDE.
--
-- Groups:
--   E. Agent Management & Security (8 tables)
--   F. Rules Engine & Priority System (6 tables)
--   G. Document & Knowledge Management (6 tables)
--   H. RLM / Recursive Context Engine (7 tables)
--   I. Memory Lifecycle & Provenance (4 tables)
--   J. Task Intelligence (7 tables)
--   K. Taxonomy & Auto-Labeling (5 tables)
--   L. Context Orchestration (6 tables)
--   M. System Health & Observability (6 tables)
--
-- Conventions (same as v001):
--   UUID  -> TEXT (hex(randomblob(16)))
--   JSONB -> TEXT (JSON strings)
--   vector(N) -> BLOB (f32 arrays)
--   tsvector  -> handled by BM25 tables in v001
--   SERIAL/BIGSERIAL -> INTEGER PRIMARY KEY AUTOINCREMENT
--   TIMESTAMP WITH TIME ZONE -> TEXT (ISO 8601)
--   arrays -> TEXT (JSON arrays)
--   enums  -> TEXT with CHECK constraints
--   interval -> TEXT (ISO 8601 duration, e.g. 'PT1H30M')

-- ============================================================================
-- GROUP E: Agent Management & Security (8 tables)
-- ============================================================================

-- Agent registry: tracks all AI agents / workers available in the IDE
-- Source: workspace.impbook_agent_registry (renamed, generic)
CREATE TABLE IF NOT EXISTS agents (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL UNIQUE,
    agent_type  TEXT NOT NULL,  -- free-form: 'coder','reviewer','orchestrator','monitor', etc.
    version     TEXT,
    description TEXT,
    host        TEXT,
    port        INTEGER,
    endpoint    TEXT,
    status      TEXT NOT NULL DEFAULT 'inactive'
                CHECK (status IN ('active','inactive','degraded','error','deregistered')),
    capabilities TEXT,  -- JSON: list of capability strings
    health      TEXT,   -- JSON: {healthy: bool, last_check: str, details: {...}}
    metrics     TEXT,   -- JSON: {tasks_completed: N, avg_latency_ms: N, ...}
    tags        TEXT,   -- JSON array
    metadata    TEXT,   -- JSON: arbitrary agent-specific config
    load        REAL NOT NULL DEFAULT 0.0,
    registered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    deregistered_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_agents_type ON agents(agent_type);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

-- Agent configurations with versioning
-- Source: workspace.agent_configs
CREATE TABLE IF NOT EXISTS agent_configs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    environment TEXT NOT NULL DEFAULT 'default'
                CHECK (environment IN ('default','development','production','testing')),
    config_data TEXT NOT NULL,  -- JSON: full config object
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','draft','archived','rollback')),
    version     INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    tags        TEXT,       -- JSON array
    checksum    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_by  TEXT NOT NULL DEFAULT 'system'
);
CREATE INDEX IF NOT EXISTS idx_agent_configs_agent ON agent_configs(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_configs_env ON agent_configs(environment);
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_configs_version ON agent_configs(agent_id, environment, version);

-- Agent config history (immutable changelog)
-- Source: workspace.agent_config_history
CREATE TABLE IF NOT EXISTS agent_config_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL,
    environment TEXT NOT NULL DEFAULT 'default',
    version     INTEGER NOT NULL,
    config_data TEXT NOT NULL,  -- JSON
    change_type TEXT NOT NULL CHECK (change_type IN (
                    'create','update','rollback','archive','activate')),
    change_message TEXT,
    checksum    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_by  TEXT NOT NULL DEFAULT 'system'
);
CREATE INDEX IF NOT EXISTS idx_agent_config_history_agent ON agent_config_history(agent_id);

-- Agent memory state: tracks what context each agent currently holds
-- Source: workspace.agent_memory_state
CREATE TABLE IF NOT EXISTS agent_memory_state (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    state_type  TEXT NOT NULL CHECK (state_type IN (
                    'context_window','working_memory','long_term','scratchpad')),
    state_data  TEXT NOT NULL,  -- JSON: the actual state
    context_window_tokens INTEGER NOT NULL DEFAULT 0,
    max_tokens  INTEGER NOT NULL DEFAULT 4096,
    last_sync   TEXT,
    version     INTEGER NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memory_agent_type ON agent_memory_state(agent_id, state_type);

-- Agent metrics over time
-- Source: workspace.agent_metrics
CREATE TABLE IF NOT EXISTS agent_metrics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL,
    agent_name  TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    response_time_ms    REAL,
    response_time_p50   REAL,
    response_time_p95   REAL,
    response_time_p99   REAL,
    response_time_count INTEGER,
    cpu_percent         REAL,
    memory_mb           REAL,
    tasks_completed     INTEGER NOT NULL DEFAULT 0,
    tasks_failed        INTEGER NOT NULL DEFAULT 0,
    tasks_per_minute    REAL,
    error_rate          REAL,
    queue_depth         INTEGER NOT NULL DEFAULT 0,
    active_tasks        INTEGER NOT NULL DEFAULT 0,
    is_healthy          INTEGER NOT NULL DEFAULT 1,
    heartbeat_latency_ms REAL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    total_restarts      INTEGER NOT NULL DEFAULT 0,
    extended_stats      TEXT   -- JSON: model-specific metrics
);
CREATE INDEX IF NOT EXISTS idx_agent_metrics_agent ON agent_metrics(agent_id, timestamp DESC);

-- Agent blackboard: shared message board for inter-agent communication
-- Source: workspace.agent_blackboard
CREATE TABLE IF NOT EXISTS agent_blackboard (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL,
    topic       TEXT NOT NULL,
    content     TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'text'
                CHECK (content_type IN ('text','json','code','error','result')),
    embedding   BLOB,
    read_by     TEXT,  -- JSON array of agent IDs
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_blackboard_topic ON agent_blackboard(topic);
CREATE INDEX IF NOT EXISTS idx_blackboard_agent ON agent_blackboard(agent_id);

-- Security profiles per agent: what each agent is allowed to do
-- Source: workspace.agent_security_profiles
CREATE TABLE IF NOT EXISTS agent_security_profiles (
    agent_id            TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    agent_name          TEXT NOT NULL,
    security_level      INTEGER NOT NULL DEFAULT 1 CHECK (security_level BETWEEN 0 AND 5),
    default_capabilities TEXT,  -- JSON: list of capability type strings
    max_capabilities    INTEGER NOT NULL DEFAULT 10,
    max_capability_duration TEXT,  -- ISO 8601 duration
    allowed_base_paths  TEXT,  -- JSON array of path prefixes
    denied_paths        TEXT,  -- JSON array of denied path prefixes
    allowed_domains     TEXT,  -- JSON array of allowed network domains
    blocked_domains     TEXT,  -- JSON array of blocked domains
    allowed_tools       TEXT,  -- JSON array of tool/server names
    can_delegate        INTEGER NOT NULL DEFAULT 0,
    can_escalate        INTEGER NOT NULL DEFAULT 0,
    requires_approval   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Capability grants and requests
-- Source: workspace.capabilities + workspace.capability_requests (merged)
CREATE TABLE IF NOT EXISTS capabilities (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    capability_type TEXT NOT NULL,
    scope           TEXT NOT NULL DEFAULT 'session'
                    CHECK (scope IN ('session','task','global','temporary')),
    constraints     TEXT,  -- JSON: {max_calls: N, allowed_paths: [...], ...}
    agent_id        TEXT NOT NULL,
    granted_by      TEXT NOT NULL DEFAULT 'system',
    granted_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at      TEXT,
    revoked_at      TEXT,
    revoked_by      TEXT,
    revoke_reason   TEXT,
    task_id         TEXT,
    session_id      TEXT,
    reason          TEXT,
    security_level  INTEGER NOT NULL DEFAULT 1,
    delegation_allowed INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_capabilities_agent ON capabilities(agent_id);
CREATE INDEX IF NOT EXISTS idx_capabilities_type ON capabilities(capability_type);


-- ============================================================================
-- GROUP F: Rules Engine & Priority System (6 tables)
-- ============================================================================

-- Rule definitions: configurable rules that govern IDE behavior
-- Source: rules.rule_definitions
CREATE TABLE IF NOT EXISTS rule_definitions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id     TEXT NOT NULL UNIQUE,
    class       TEXT NOT NULL DEFAULT '0'
                CHECK (class IN ('000','00','0','1','2','3')),
    priority    INTEGER NOT NULL DEFAULT 50,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    scope       TEXT,  -- JSON array: ['global','project','agent','session']
    mandatory   INTEGER NOT NULL DEFAULT 1,
    category    TEXT NOT NULL DEFAULT 'general',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_by  TEXT NOT NULL DEFAULT 'system',
    version     INTEGER NOT NULL DEFAULT 1,
    deprecated  INTEGER NOT NULL DEFAULT 0,
    embedding   BLOB,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    modification_log TEXT  -- JSON: [{date, by, reason}, ...]
);
CREATE INDEX IF NOT EXISTS idx_rules_class ON rule_definitions(class);
CREATE INDEX IF NOT EXISTS idx_rules_category ON rule_definitions(category);
CREATE INDEX IF NOT EXISTS idx_rules_priority ON rule_definitions(priority DESC);

-- Rule version history (immutable)
-- Source: rules.rule_history
CREATE TABLE IF NOT EXISTS rule_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id     TEXT NOT NULL,
    version     INTEGER NOT NULL,
    content     TEXT NOT NULL,
    metadata    TEXT,  -- JSON
    changed_by  TEXT NOT NULL DEFAULT 'system',
    changed_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    change_reason TEXT
);
CREATE INDEX IF NOT EXISTS idx_rule_history_rule ON rule_history(rule_id, version DESC);

-- Context-to-rule mappings: which rules apply in which contexts
-- Source: rules.context_mappings
CREATE TABLE IF NOT EXISTS context_rule_mappings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    context_type    TEXT NOT NULL,  -- e.g. 'coding', 'debugging', 'review', 'deploy'
    rule_class      TEXT NOT NULL,
    load_priority   INTEGER NOT NULL DEFAULT 50,
    active          INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_context_mappings_type ON context_rule_mappings(context_type);

-- Priority rules: high-priority rules injected into every agent context
-- Source: workspace.priority_rules
CREATE TABLE IF NOT EXISTS priority_rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id     TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    priority    INTEGER NOT NULL DEFAULT 50,
    scope       TEXT NOT NULL DEFAULT 'global'
                CHECK (scope IN ('global','project','agent','session')),
    project_filter TEXT,  -- optional: only apply to specific project
    mandatory   INTEGER NOT NULL DEFAULT 1,
    token_count INTEGER,
    embedding   BLOB,
    rule_class  TEXT DEFAULT '0',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_priority_rules_scope ON priority_rules(scope);
CREATE INDEX IF NOT EXISTS idx_priority_rules_priority ON priority_rules(priority DESC);

-- Rules book: simplified key-value rule store for quick access
-- Source: workspace.rules_book
CREATE TABLE IF NOT EXISTS rules_book (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id     TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general',
    priority    INTEGER NOT NULL DEFAULT 50,
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rules_book_category ON rules_book(category);

-- Config schemas: JSON schemas defining valid config structures per agent type
-- Source: workspace.config_schemas
CREATE TABLE IF NOT EXISTS config_schemas (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_type      TEXT NOT NULL,
    schema_json     TEXT NOT NULL,  -- JSON Schema
    schema_version  INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_config_schemas_type ON config_schemas(agent_type, schema_version);


-- ============================================================================
-- GROUP G: Document & Knowledge Management (6 tables)
-- ============================================================================

-- Documents: ingested docs, guides, research papers
-- Source: knowledge.documents
CREATE TABLE IF NOT EXISTS documents (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    doc_id      TEXT NOT NULL UNIQUE,
    project     TEXT,
    category    TEXT NOT NULL DEFAULT 'general',
    title       TEXT NOT NULL,
    file_path   TEXT,
    word_count  INTEGER,
    full_text   TEXT,
    summary     TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    version     INTEGER NOT NULL DEFAULT 1,
    deprecated  INTEGER NOT NULL DEFAULT 0,
    tags        TEXT,   -- JSON array
    source      TEXT,
    author      TEXT,
    embedding   BLOB
);
CREATE INDEX IF NOT EXISTS idx_documents_project ON documents(project);
CREATE INDEX IF NOT EXISTS idx_documents_category ON documents(category);

-- Document chunks: split documents for RAG retrieval
-- Source: knowledge.doc_chunks
CREATE TABLE IF NOT EXISTS doc_chunks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    doc_id      TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    chunk_text  TEXT NOT NULL,
    token_count INTEGER,
    embedding   BLOB,
    section_title TEXT,
    importance_score REAL NOT NULL DEFAULT 0.5
);
CREATE INDEX IF NOT EXISTS idx_doc_chunks_doc ON doc_chunks(doc_id);

-- Document relationships: how docs relate to each other
-- Source: knowledge.doc_relationships
CREATE TABLE IF NOT EXISTS doc_relationships (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_doc_id TEXT NOT NULL,
    target_doc_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL CHECK (relationship_type IN (
                    'supersedes','references','extends','contradicts',
                    'summarizes','depends_on','related'))
);
CREATE INDEX IF NOT EXISTS idx_doc_rel_source ON doc_relationships(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_doc_rel_target ON doc_relationships(target_doc_id);

-- Important info: curated high-value knowledge items with lifecycle
-- Source: workspace.important_info (simplified)
CREATE TABLE IF NOT EXISTS important_info (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    info_id         TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    title           TEXT NOT NULL,
    content         TEXT NOT NULL,
    info_type       TEXT NOT NULL DEFAULT 'note'
                    CHECK (info_type IN ('note','decision','finding','rule','reference','warning')),
    category        TEXT NOT NULL DEFAULT 'general',
    importance      INTEGER NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    is_current      INTEGER NOT NULL DEFAULT 1,
    superseded_by   TEXT,
    related_task_ids TEXT,  -- JSON array
    related_rule_ids TEXT,  -- JSON array
    created_by      TEXT NOT NULL DEFAULT 'system',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    valid_from      TEXT,
    valid_until     TEXT,
    tags            TEXT,  -- JSON array
    embedding       BLOB,
    metadata        TEXT,  -- JSON
    content_hash    TEXT,
    -- Memory lifecycle fields
    scope           TEXT DEFAULT 'global'
                    CHECK (scope IN ('global','project','session','agent')),
    scope_id        TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0,
    last_accessed   TEXT,
    memory_strength REAL NOT NULL DEFAULT 1.0,
    decay_rate      REAL NOT NULL DEFAULT 0.01,
    -- Knowledge tier
    knowledge_tier  TEXT DEFAULT 'unverified'
                    CHECK (knowledge_tier IN ('golden','verified','medium','unverified')),
    is_golden       INTEGER NOT NULL DEFAULT 0,
    reviewed_at     TEXT,
    reviewed_by     TEXT,
    -- FSRS spaced repetition
    fsrs_difficulty REAL,
    fsrs_stability  REAL,
    fsrs_retrievability REAL,
    last_reviewed_at TEXT,
    review_count    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_important_info_type ON important_info(info_type);
CREATE INDEX IF NOT EXISTS idx_important_info_category ON important_info(category);
CREATE INDEX IF NOT EXISTS idx_important_info_importance ON important_info(importance DESC);
CREATE INDEX IF NOT EXISTS idx_important_info_tier ON important_info(knowledge_tier);
CREATE INDEX IF NOT EXISTS idx_important_info_current ON important_info(is_current) WHERE is_current = 1;

-- Knowledge routing: directs queries to the right tables/strategies
-- Source: workspace.knowledge_routing
CREATE TABLE IF NOT EXISTS knowledge_routing (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    route_id    TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    query_domain TEXT NOT NULL,
    target_tables TEXT NOT NULL,  -- JSON array of table names
    boost_keywords TEXT,          -- JSON array
    retrieval_strategy TEXT NOT NULL DEFAULT 'hybrid'
                    CHECK (retrieval_strategy IN ('hybrid','semantic','bm25','exact','kg')),
    confidence_threshold REAL NOT NULL DEFAULT 0.5,
    max_results INTEGER NOT NULL DEFAULT 10,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_knowledge_routing_domain ON knowledge_routing(query_domain);

-- Knowledge chunks: hierarchical content chunks for RAG
-- Source: workspace.knowledge_chunks
CREATE TABLE IF NOT EXISTS knowledge_chunks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_id        TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    parent_chunk_id TEXT,
    hierarchy_level INTEGER NOT NULL DEFAULT 0,
    title           TEXT,
    content         TEXT NOT NULL,
    summary         TEXT,
    category        TEXT,
    subcategory     TEXT,
    keywords        TEXT,  -- JSON array
    importance      INTEGER NOT NULL DEFAULT 3,
    token_count     INTEGER,
    source_id       TEXT,
    source_type     TEXT,
    embedding       BLOB,
    embedding_model TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    is_deduplicated INTEGER NOT NULL DEFAULT 0,
    quality_score   REAL,
    compression_ratio REAL,
    original_length INTEGER,
    is_compressed   INTEGER NOT NULL DEFAULT 0,
    semantic_hash   TEXT,
    access_count    INTEGER NOT NULL DEFAULT 0,
    last_accessed   TEXT,
    version         INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_parent ON knowledge_chunks(parent_chunk_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_category ON knowledge_chunks(category);
CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_source ON knowledge_chunks(source_type, source_id);


-- ============================================================================
-- GROUP H: RLM / Recursive Context Engine (7 tables)
-- ============================================================================

-- RLM sessions: tracks recursive language model sessions
-- Source: workspace.rlm_sessions
CREATE TABLE IF NOT EXISTS rlm_sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL UNIQUE,
    root_query      TEXT NOT NULL,
    context_hash    TEXT,
    context_length  INTEGER,
    max_depth       INTEGER NOT NULL DEFAULT 5,
    max_iterations  INTEGER NOT NULL DEFAULT 10,
    status          TEXT NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active','completed','failed','timeout','cancelled')),
    started_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at    TEXT,
    final_answer    TEXT,
    total_tokens_used INTEGER NOT NULL DEFAULT 0,
    root_model      TEXT,
    sub_model       TEXT,
    effective_context INTEGER,
    cost_estimate   REAL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_sessions_status ON rlm_sessions(status);

-- RLM variables: external variables stored outside the prompt
-- Source: workspace.rlm_variables
CREATE TABLE IF NOT EXISTS rlm_variables (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    var_id          TEXT NOT NULL UNIQUE,
    name            TEXT,
    content_type    TEXT NOT NULL DEFAULT 'text'
                    CHECK (content_type IN ('text','code','json','markdown','log','document','structured')),
    total_tokens    INTEGER NOT NULL DEFAULT 0,
    chunk_count     INTEGER NOT NULL DEFAULT 0,
    chunks          TEXT,  -- JSON: [{chunk_id, start_line, end_line, token_count}, ...]
    metadata        TEXT,  -- JSON
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at      TEXT,
    source          TEXT,
    owner           TEXT
);
CREATE INDEX IF NOT EXISTS idx_rlm_variables_source ON rlm_variables(source);

-- RLM chunks: the actual chunked content for RLM processing
-- Source: workspace.rlm_chunks
CREATE TABLE IF NOT EXISTS rlm_chunks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path   TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content     TEXT NOT NULL,
    embedding   BLOB,
    token_count INTEGER NOT NULL DEFAULT 0,
    priority    INTEGER NOT NULL DEFAULT 5,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_chunks_file ON rlm_chunks(file_path);

-- RLM call tree: tracks the recursive call hierarchy
-- Source: workspace.rlm_calls + claude_rlm.rlm_call_tree (merged)
CREATE TABLE IF NOT EXISTS rlm_calls (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL,
    call_id         TEXT NOT NULL,
    depth           INTEGER NOT NULL DEFAULT 0,
    iteration       INTEGER NOT NULL DEFAULT 0,
    parent_call_id  TEXT,
    prompt_preview  TEXT,
    code_executed   TEXT,
    output_preview  TEXT,
    tokens_used     INTEGER NOT NULL DEFAULT 0,
    duration_ms     REAL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','running','completed','failed')),
    is_final        INTEGER NOT NULL DEFAULT 0,
    model_used      TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_calls_session ON rlm_calls(session_id);
CREATE INDEX IF NOT EXISTS idx_rlm_calls_parent ON rlm_calls(parent_call_id);

-- RLM query log: analytics on RLM queries
-- Source: workspace.rlm_query_log
CREATE TABLE IF NOT EXISTS rlm_query_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    query_id        TEXT NOT NULL,
    query_text      TEXT NOT NULL,
    variable_ids    TEXT,  -- JSON array
    chunks_processed INTEGER NOT NULL DEFAULT 0,
    result_summary  TEXT,
    latency_ms      REAL,
    worker_model    TEXT,
    aggregator_model TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_query_log_time ON rlm_query_log(created_at DESC);

-- RLM pending: queue of files waiting for RLM processing
-- Source: workspace.rlm_pending
CREATE TABLE IF NOT EXISTS rlm_pending (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path       TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    content_length  INTEGER NOT NULL,
    queued_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    processed_at    TEXT,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued','processing','completed','failed','skipped'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_pending_status ON rlm_pending(status);

-- RLM REPL state: state of recursive read-eval-print loop iterations
-- Source: workspace.rlm_repl_state
CREATE TABLE IF NOT EXISTS rlm_repl_state (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL,
    iteration   INTEGER NOT NULL,
    variables   TEXT,  -- JSON: current variable bindings
    stdout      TEXT,
    stderr      TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_rlm_repl_session ON rlm_repl_state(session_id);


-- ============================================================================
-- GROUP I: Memory Lifecycle & Provenance (4 tables)
-- ============================================================================

-- Memory lifecycle events: tracks birth, access, decay, archival of memories
-- Source: workspace.memory_lifecycle
CREATE TABLE IF NOT EXISTS memory_lifecycle (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    memory_id   TEXT NOT NULL,       -- FK to memories.id (v001)
    memory_table TEXT NOT NULL DEFAULT 'memories',
    event_type  TEXT NOT NULL CHECK (event_type IN (
                    'created','accessed','updated','reinforced','decayed',
                    'promoted','demoted','archived','deleted','merged','split')),
    event_data  TEXT,  -- JSON: details about the event
    triggered_by TEXT NOT NULL DEFAULT 'system',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_memory ON memory_lifecycle(memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_lifecycle_type ON memory_lifecycle(event_type);

-- Memory operations: conflict resolution and merge tracking
-- Source: workspace.memory_operations
CREATE TABLE IF NOT EXISTS memory_operations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id       TEXT NOT NULL,
    operation       TEXT NOT NULL CHECK (operation IN (
                        'merge','split','deduplicate','update','conflict','resolve')),
    old_content     TEXT,
    new_content     TEXT,
    similarity_score REAL,
    conflict_reason TEXT,
    resolved_by     TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_operations_memory ON memory_operations(memory_id);

-- Memory provenance: full audit trail of how memory changed over time
-- Source: workspace.memory_provenance
CREATE TABLE IF NOT EXISTS memory_provenance (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id           TEXT NOT NULL,
    memory_table        TEXT NOT NULL DEFAULT 'memories',
    operation           TEXT NOT NULL CHECK (operation IN (
                            'insert','update','delete','promote','demote','reinforce')),
    field_changed       TEXT,
    old_value           TEXT,
    new_value           TEXT,
    event_time          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    transaction_time    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    triggered_by        TEXT NOT NULL DEFAULT 'system',
    trigger_source      TEXT,
    reason              TEXT,
    confidence          REAL,
    related_memory_id   TEXT,
    relationship_type   TEXT CHECK (relationship_type IN (
                            'supersedes','derives_from','conflicts_with',
                            'corroborates','extends',NULL)),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_provenance_memory ON memory_provenance(memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_provenance_time ON memory_provenance(event_time DESC);

-- Similarity cache: precomputed similarity scores between items
-- Source: workspace.similarity_cache
CREATE TABLE IF NOT EXISTS similarity_cache (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id       TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    source_table    TEXT NOT NULL DEFAULT 'memories',
    target_table    TEXT NOT NULL DEFAULT 'memories',
    similarity_score REAL NOT NULL,
    comparison_type TEXT NOT NULL DEFAULT 'cosine'
                    CHECK (comparison_type IN ('cosine','jaccard','bm25','hybrid')),
    is_duplicate    INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_similarity_source ON similarity_cache(source_id, source_table);
CREATE INDEX IF NOT EXISTS idx_similarity_target ON similarity_cache(target_id, target_table);
CREATE UNIQUE INDEX IF NOT EXISTS idx_similarity_pair ON similarity_cache(source_id, target_id, source_table, target_table, comparison_type);


-- ============================================================================
-- GROUP J: Task Intelligence (7 tables)
-- ============================================================================

-- Task detection patterns: regex/keyword patterns for auto-extracting tasks from text
-- Source: workspace.task_detection_patterns
CREATE TABLE IF NOT EXISTS task_detection_patterns (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern             TEXT NOT NULL,
    pattern_type        TEXT NOT NULL DEFAULT 'regex'
                        CHECK (pattern_type IN ('regex','keyword','nlp','combined')),
    priority_mapping    TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority_mapping IN ('critical','high','medium','low','batch')),
    extract_title_group INTEGER,  -- regex capture group for title
    is_active           INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Task extraction log: what was found and from where
-- Source: workspace.task_extraction_log
CREATE TABLE IF NOT EXISTS task_extraction_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type     TEXT NOT NULL,   -- 'comment', 'commit', 'chat', 'file', 'manual'
    source_id       TEXT,
    content_hash    TEXT,
    tasks_found     INTEGER NOT NULL DEFAULT 0,
    patterns_matched TEXT,  -- JSON: {pattern_id: count, ...}
    extracted_title TEXT,
    pattern_used    TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_task_extraction_source ON task_extraction_log(source_type, created_at DESC);

-- Task completion rules: auto-complete conditions
-- Source: workspace.task_completion_rules
CREATE TABLE IF NOT EXISTS task_completion_rules (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id             TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    pattern             TEXT NOT NULL,
    completion_condition TEXT NOT NULL,
    auto_complete       INTEGER NOT NULL DEFAULT 0,
    priority            INTEGER NOT NULL DEFAULT 50,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    is_active           INTEGER NOT NULL DEFAULT 1
);

-- Task clusters: group related tasks together
-- Source: workspace.task_clusters
CREATE TABLE IF NOT EXISTS task_clusters (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id          TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    cluster_name        TEXT NOT NULL,
    cluster_category    TEXT,
    cluster_description TEXT,
    task_ids            TEXT NOT NULL,  -- JSON array of task IDs
    priority_score      REAL NOT NULL DEFAULT 0.5,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_task_clusters_category ON task_clusters(cluster_category);

-- Task archive: completed/cancelled tasks moved here to keep main table fast
-- Source: workspace.tasks_archive
CREATE TABLE IF NOT EXISTS tasks_archive (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    description     TEXT,
    task_type       TEXT,
    priority        TEXT,
    status          TEXT NOT NULL,
    parent_task_id  TEXT,
    assignee        TEXT,
    created_by      TEXT NOT NULL DEFAULT 'system',
    created_at      TEXT NOT NULL,
    completed_at    TEXT,
    archived_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    started_at      TEXT,
    due_date        TEXT,
    progress_percentage INTEGER,
    actual_effort_hours REAL,
    tags            TEXT,   -- JSON array
    embedding       BLOB,
    metadata        TEXT,   -- JSON
    source          TEXT,
    archive_reason  TEXT CHECK (archive_reason IN (
                        'completed','cancelled','superseded','expired','manual')),
    summary         TEXT,
    outcome         TEXT CHECK (outcome IN ('success','partial','failure','abandoned',NULL))
);
CREATE INDEX IF NOT EXISTS idx_tasks_archive_status ON tasks_archive(status);
CREATE INDEX IF NOT EXISTS idx_tasks_archive_archived ON tasks_archive(archived_at DESC);

-- Trust scores: Hebbian-inspired trust for agents/workers
-- Source: workspace.trust_scores
CREATE TABLE IF NOT EXISTS trust_scores (
    task_name       TEXT PRIMARY KEY,
    score           REAL NOT NULL DEFAULT 0.5,
    successes       INTEGER NOT NULL DEFAULT 0,
    failures        INTEGER NOT NULL DEFAULT 0,
    last_updated    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    co_activations  TEXT,  -- JSON: {agent_id: activation_count, ...}
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Future plans: roadmap items and planned features
-- Source: workspace.future_plans
CREATE TABLE IF NOT EXISTS future_plans (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id             TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    title               TEXT NOT NULL,
    description         TEXT,
    category            TEXT,
    priority            TEXT DEFAULT 'medium'
                        CHECK (priority IN ('critical','high','medium','low','wishlist')),
    estimated_effort_days INTEGER,
    dependencies        TEXT,  -- JSON array of plan_ids
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    target_milestone    TEXT,
    tags                TEXT,  -- JSON array
    embedding           BLOB
);
CREATE INDEX IF NOT EXISTS idx_future_plans_priority ON future_plans(priority);
CREATE INDEX IF NOT EXISTS idx_future_plans_category ON future_plans(category);


-- ============================================================================
-- GROUP K: Taxonomy & Auto-Labeling (5 tables)
-- ============================================================================

-- Label taxonomy: hierarchical classification schemes
-- Source: workspace.label_taxonomy
CREATE TABLE IF NOT EXISTS label_taxonomy (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name        TEXT NOT NULL,
    description TEXT,
    domain      TEXT NOT NULL DEFAULT 'general',
    version     INTEGER NOT NULL DEFAULT 1,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_taxonomy_domain ON label_taxonomy(domain);

-- Taxonomy tree nodes
-- Source: workspace.taxonomy_nodes
CREATE TABLE IF NOT EXISTS taxonomy_nodes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    taxonomy_id     TEXT NOT NULL REFERENCES label_taxonomy(id) ON DELETE CASCADE,
    node_name       TEXT NOT NULL,
    node_type       TEXT NOT NULL DEFAULT 'category'
                    CHECK (node_type IN ('root','category','subcategory','leaf','tag')),
    level           INTEGER NOT NULL DEFAULT 0,
    parent_node_id  INTEGER REFERENCES taxonomy_nodes(id) ON DELETE SET NULL,
    description     TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_taxonomy_nodes_parent ON taxonomy_nodes(parent_node_id);
CREATE INDEX IF NOT EXISTS idx_taxonomy_nodes_taxonomy ON taxonomy_nodes(taxonomy_id);

-- Auto-labels: machine-generated labels for content
-- Source: workspace.auto_labels
CREATE TABLE IF NOT EXISTS auto_labels (
    id                      TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    content_id              TEXT NOT NULL,
    content_path            TEXT,
    content_hash            TEXT,
    taxonomy_id             TEXT REFERENCES label_taxonomy(id),
    domain                  TEXT,
    category                TEXT,
    attributes              TEXT,  -- JSON array
    primary_label           TEXT,
    secondary_labels        TEXT,  -- JSON array
    overall_confidence      REAL NOT NULL DEFAULT 0.0,
    entropy_score           REAL,
    lf_agreement_score      REAL,
    confident_learning_score REAL,
    generation_method       TEXT,
    worker_model            TEXT,
    lf_votes                TEXT,  -- JSON: {function_id: vote, ...}
    rlm_variable_id         TEXT,
    chunks_processed        INTEGER,
    status                  TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','labeled','reviewed','rejected','expired')),
    human_label             TEXT,
    human_label_date        TEXT,
    human_feedback          TEXT,
    embedding               BLOB,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_auto_labels_content ON auto_labels(content_id);
CREATE INDEX IF NOT EXISTS idx_auto_labels_status ON auto_labels(status);
CREATE INDEX IF NOT EXISTS idx_auto_labels_domain ON auto_labels(domain);

-- Labeling functions: weak supervision labeling functions (Snorkel-style)
-- Source: workspace.labeling_functions
CREATE TABLE IF NOT EXISTS labeling_functions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    taxonomy_id     TEXT REFERENCES label_taxonomy(id),
    function_name   TEXT NOT NULL,
    function_type   TEXT NOT NULL CHECK (function_type IN (
                        'keyword','regex','model','heuristic','embedding','composite')),
    description     TEXT,
    pattern         TEXT,
    output_label    TEXT NOT NULL,
    confidence_weight REAL NOT NULL DEFAULT 1.0,
    is_active       INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_labeling_functions_taxonomy ON labeling_functions(taxonomy_id);

-- Active learning feedback: human corrections that improve auto-labeling
-- Source: workspace.active_learning_feedback
CREATE TABLE IF NOT EXISTS active_learning_feedback (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    auto_label_id       TEXT NOT NULL REFERENCES auto_labels(id),
    auto_label_value    TEXT,
    human_label_value   TEXT NOT NULL,
    was_correct         INTEGER NOT NULL,
    confidence_before   REAL,
    confidence_after    REAL,
    feedback_date       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    lf_votes_snapshot   TEXT,  -- JSON
    entropy_score       REAL,
    agreement_rate      REAL
);
CREATE INDEX IF NOT EXISTS idx_active_learning_label ON active_learning_feedback(auto_label_id);


-- ============================================================================
-- GROUP L: Context Orchestration (6 tables)
-- ============================================================================

-- Context packages: precomputed context bundles for model consumption
-- Source: workspace.context_packages
CREATE TABLE IF NOT EXISTS context_packages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    target_model    TEXT NOT NULL,
    token_budget    INTEGER NOT NULL,
    token_used      INTEGER NOT NULL DEFAULT 0,
    rules_count     INTEGER NOT NULL DEFAULT 0,
    tasks_count     INTEGER NOT NULL DEFAULT 0,
    memories_count  INTEGER NOT NULL DEFAULT 0,
    kg_entities     INTEGER NOT NULL DEFAULT 0,
    sources         TEXT,  -- JSON array of source identifiers
    generation_ms   REAL,
    package_data    TEXT NOT NULL,  -- JSON: the actual context package
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_context_packages_model ON context_packages(target_model);
CREATE INDEX IF NOT EXISTS idx_context_packages_expires ON context_packages(expires_at);

-- Context table of contents: structured navigation for large context
-- Source: workspace.context_toc
CREATE TABLE IF NOT EXISTS context_toc (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    tier                TEXT NOT NULL DEFAULT '0',
    section_id          TEXT NOT NULL,
    parent_id           TEXT,
    title               TEXT NOT NULL,
    icon                TEXT,
    description         TEXT,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    is_collapsible      INTEGER NOT NULL DEFAULT 1,
    is_collapsed_default INTEGER NOT NULL DEFAULT 0,
    item_count          INTEGER NOT NULL DEFAULT 0,
    metadata            TEXT,  -- JSON
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    token_budget        INTEGER,
    auto_include        INTEGER NOT NULL DEFAULT 0,
    priority            INTEGER NOT NULL DEFAULT 50,
    keywords            TEXT,  -- JSON array
    last_accessed       TEXT
);
CREATE INDEX IF NOT EXISTS idx_context_toc_tier ON context_toc(tier);
CREATE INDEX IF NOT EXISTS idx_context_toc_parent ON context_toc(parent_id);

-- Context chains: linked session histories for deep context
-- Source: workspace.context_chains
CREATE TABLE IF NOT EXISTS context_chains (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    chain_id        TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    project_path    TEXT,
    session_ids     TEXT NOT NULL,  -- JSON array of session IDs
    chain_summary   TEXT,
    chain_embedding BLOB,
    total_sessions  INTEGER NOT NULL DEFAULT 0,
    total_pending_tasks INTEGER NOT NULL DEFAULT 0,
    importance_score REAL NOT NULL DEFAULT 0.5,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_context_chains_project ON context_chains(project_path);

-- Epoch summaries: compressed multi-session summaries
-- Source: workspace.epoch_summaries
CREATE TABLE IF NOT EXISTS epoch_summaries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    epoch_id            TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    project_path        TEXT,
    start_timestamp     TEXT NOT NULL,
    end_timestamp       TEXT NOT NULL,
    session_count       INTEGER NOT NULL,
    session_ids         TEXT,  -- JSON array
    epoch_summary       TEXT NOT NULL,
    key_decisions       TEXT,  -- JSON array
    key_achievements    TEXT,  -- JSON array
    unresolved_issues   TEXT,  -- JSON array
    epoch_embedding     BLOB,
    compression_ratio   REAL,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_epoch_summaries_project ON epoch_summaries(project_path);
CREATE INDEX IF NOT EXISTS idx_epoch_summaries_time ON epoch_summaries(end_timestamp DESC);

-- Token budgets: per-model token allocation config
-- Source: workspace.token_budgets
CREATE TABLE IF NOT EXISTS token_budgets (
    model_id            TEXT PRIMARY KEY,
    max_tokens          INTEGER NOT NULL,
    warning_threshold   REAL NOT NULL DEFAULT 0.8,
    critical_threshold  REAL NOT NULL DEFAULT 0.95,
    reserve_tokens      INTEGER NOT NULL DEFAULT 1000
);

-- Model contexts: extended model configuration for context injection
-- Source: workspace.model_contexts
CREATE TABLE IF NOT EXISTS model_contexts (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id                TEXT NOT NULL UNIQUE,
    display_name            TEXT,
    total_context_tokens    INTEGER NOT NULL,
    reserved_for_response   INTEGER NOT NULL DEFAULT 4096,
    max_injection_tokens    INTEGER NOT NULL,
    temperature             REAL NOT NULL DEFAULT 0.7,
    capabilities            TEXT,  -- JSON: {vision: bool, tools: bool, streaming: bool, ...}
    priority_rules_required INTEGER NOT NULL DEFAULT 0,
    metadata                TEXT,  -- JSON
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);


-- ============================================================================
-- GROUP M: System Health & Observability (6 tables)
-- ============================================================================

-- Component registry: IDE components, plugins, services
-- Source: system_inventory.components (generic)
CREATE TABLE IF NOT EXISTS components (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    display_name    TEXT,
    category        TEXT NOT NULL CHECK (category IN (
                        'core','plugin','service','model','database','tool','extension','agent')),
    description     TEXT,
    file_path       TEXT,
    port            INTEGER,
    language        TEXT,
    framework       TEXT,
    health          TEXT NOT NULL DEFAULT 'unknown'
                    CHECK (health IN ('healthy','degraded','unhealthy','unknown','starting','stopped')),
    version         TEXT,
    depends_on      TEXT,  -- JSON array
    tags            TEXT,  -- JSON array
    config_location TEXT,
    documentation_url TEXT,
    token_budget    INTEGER,
    vram_usage_mb   INTEGER,
    cpu_cores       REAL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Component version history
-- Source: system_inventory.component_versions
CREATE TABLE IF NOT EXISTS component_versions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    component_name  TEXT NOT NULL,
    version         TEXT NOT NULL,
    release_date    TEXT,
    changelog       TEXT,
    breaking_changes INTEGER NOT NULL DEFAULT 0,
    migration_notes TEXT,
    deprecated      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_component_versions_name ON component_versions(component_name, version);

-- Component dependencies: who depends on whom
-- Source: system_inventory.dependencies
CREATE TABLE IF NOT EXISTS component_dependencies (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    source_component    TEXT NOT NULL,
    target_component    TEXT NOT NULL,
    dependency_type     TEXT NOT NULL DEFAULT 'runtime'
                        CHECK (dependency_type IN ('runtime','build','optional','dev','peer')),
    version_constraint  TEXT,
    notes               TEXT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_comp_deps_source ON component_dependencies(source_component);
CREATE INDEX IF NOT EXISTS idx_comp_deps_target ON component_dependencies(target_component);

-- Health history: rolling health check log
-- Source: system_inventory.health_history
CREATE TABLE IF NOT EXISTS health_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    component_name  TEXT NOT NULL,
    health          TEXT NOT NULL,
    latency_ms      INTEGER,
    error_message   TEXT,
    metrics         TEXT,  -- JSON: {cpu: N, mem: N, ...}
    checked_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_health_history_component ON health_history(component_name, checked_at DESC);

-- Incidents: tracked issues / outages
-- Source: system_inventory.incidents
CREATE TABLE IF NOT EXISTS incidents (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    component_name      TEXT NOT NULL,
    severity            TEXT NOT NULL DEFAULT 'medium'
                        CHECK (severity IN ('critical','high','medium','low','info')),
    title               TEXT NOT NULL,
    description         TEXT,
    root_cause          TEXT,
    resolution          TEXT,
    status              TEXT NOT NULL DEFAULT 'open'
                        CHECK (status IN ('open','investigating','mitigated','resolved','closed')),
    started_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at         TEXT,
    affected_components TEXT,  -- JSON array
    tags                TEXT   -- JSON array
);
CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents(status);
CREATE INDEX IF NOT EXISTS idx_incidents_severity ON incidents(severity);
CREATE INDEX IF NOT EXISTS idx_incidents_component ON incidents(component_name);

-- Performance metrics: time-series metrics for any component
-- Source: system_inventory.performance_metrics
CREATE TABLE IF NOT EXISTS performance_metrics (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    component_name  TEXT NOT NULL,
    metric_name     TEXT NOT NULL,
    metric_value    REAL NOT NULL,
    unit            TEXT,
    tags            TEXT,  -- JSON: {env: "prod", ...}
    recorded_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_perf_metrics_component ON performance_metrics(component_name, metric_name, recorded_at DESC);


-- ============================================================================
-- Schema version tracking
-- ============================================================================
INSERT OR IGNORE INTO system_config (key, value, category, description)
VALUES ('schema_version', '2', 'database', 'ForgeMemory schema version');
