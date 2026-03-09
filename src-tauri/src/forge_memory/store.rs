//! SQLite persistence layer for ForgeMemory
//!
//! Pattern: Same as orchestrator/store.rs -- WAL mode, bundled SQLite,
//! parking_lot::Mutex for concurrent access, auto-migrations on startup.
//!
//! 80 tables across 13 groups (v001 + v002):
//!   A. Core Memory & Knowledge (6)
//!   B. Task & Event System (4)
//!   C. Code Intelligence (6)
//!   D. Search & AI Infrastructure (9)
//!   E. Agent Management & Security (8)
//!   F. Rules Engine & Priority System (6)
//!   G. Document & Knowledge Management (6)
//!   H. RLM / Recursive Context Engine (7)
//!   I. Memory Lifecycle & Provenance (4)
//!   J. Task Intelligence (7)
//!   K. Taxonomy & Auto-Labeling (5)
//!   L. Context Orchestration (6)
//!   M. System Health & Observability (6)

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::migration;

// ── Stats ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMemoryStats {
    pub total_memories: u64,
    pub total_knowledge: u64,
    pub total_kg_nodes: u64,
    pub total_kg_edges: u64,
    pub total_code_chunks: u64,
    pub total_files_indexed: u64,
    pub total_chat_messages: u64,
    pub total_embeddings_cached: u64,
    pub total_agents: u64,
    pub total_rules: u64,
    pub total_documents: u64,
    pub total_components: u64,
    pub total_incidents: u64,
    pub schema_version: i64,
    pub db_size_bytes: u64,
}

// ── Memory Record ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub scope: String,
    pub category: String,
    pub key: Option<String>,
    pub content: String,
    pub importance: f64,
    pub created_at: String,
}

// ── Knowledge Record ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRecord {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tier: String,
    pub category: String,
    pub importance: i32,
    pub created_at: String,
}

// ── KG Node ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgNodeRecord {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub properties: Option<String>,
    pub confidence: f64,
}

// ── KG Edge ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgEdgeRecord {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub weight: f64,
    pub confidence: f64,
}

// ── Chat Message ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub created_at: String,
}

// ── Agent Record ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub status: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub registered_at: String,
}

// ── Rule Record ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRecord {
    pub id: i64,
    pub rule_id: String,
    pub class: String,
    pub priority: i32,
    pub title: String,
    pub content: String,
    pub mandatory: bool,
}

// ── Document Record ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: i64,
    pub doc_id: String,
    pub project: String,
    pub category: String,
    pub title: String,
    pub summary: Option<String>,
    pub version: i32,
}

// ── Component Record ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentRecord {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub health: String,
    pub version: Option<String>,
}

// ── Incident Record ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: i64,
    pub component_name: Option<String>,
    pub severity: Option<String>,
    pub title: String,
    pub status: String,
    pub started_at: String,
}

// ── Trust Score Record ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScoreRecord {
    pub task_name: String,
    pub score: f64,
    pub successes: i32,
    pub failures: i32,
}

// ── Store ──────────────────────────────────────────────────────

pub struct ForgeMemoryStore {
    conn: Mutex<Connection>,
}

impl ForgeMemoryStore {
    /// Open or create the SQLite database at the given path.
    pub fn open(db_path: &PathBuf) -> SqlResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path)?;
        Self::configure_and_migrate(conn)
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn)
    }

    fn configure_and_migrate(conn: Connection) -> SqlResult<Self> {
        // Production WAL PRAGMAs (Tauri 2 best practice)
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", "5000")?;
        conn.pragma_update(None, "cache_size", "-16000")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "mmap_size", "268435456")?;
        migration::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Optimize query planner statistics on graceful shutdown.
    pub fn optimize(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize;");
    }

    // ── Stats ──────────────────────────────────────────────

    pub fn stats(&self) -> SqlResult<ForgeMemoryStats> {
        let conn = self.conn.lock();
        let count = |table: &str| -> u64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(0)
        };
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(ForgeMemoryStats {
            total_memories: count("memories"),
            total_knowledge: count("knowledge_items"),
            total_kg_nodes: count("kg_nodes"),
            total_kg_edges: count("kg_edges"),
            total_code_chunks: count("code_chunks"),
            total_files_indexed: count("file_index"),
            total_chat_messages: count("chat_messages"),
            total_embeddings_cached: count("embeddings_cache"),
            total_agents: count("agents"),
            total_rules: count("rule_definitions"),
            total_documents: count("documents"),
            total_components: count("components"),
            total_incidents: count("incidents"),
            schema_version: version,
            db_size_bytes: 0,
        })
    }

    // ── Memory CRUD ────────────────────────────────────────

    pub fn insert_memory(
        &self,
        scope: &str,
        category: &str,
        key: Option<&str>,
        content: &str,
        importance: f64,
        embedding: Option<&[u8]>,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO memories (id, scope, category, key, content, importance, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, scope, category, key, content, importance, embedding],
        )?;
        Ok(id)
    }

    pub fn get_memories_by_scope(&self, scope: &str, limit: u32) -> SqlResult<Vec<MemoryRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, scope, category, key, content, importance, created_at
             FROM memories WHERE scope = ?1
             ORDER BY importance DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![scope, limit], |row| {
            Ok(MemoryRecord {
                id: row.get(0)?,
                scope: row.get(1)?,
                category: row.get(2)?,
                key: row.get(3)?,
                content: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_memory(&self, id: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn update_memory_scope(&self, id: &str, new_scope: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE memories SET scope = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![new_scope, id],
        )?;
        Ok(affected > 0)
    }

    // ── Knowledge CRUD ─────────────────────────────────────

    pub fn insert_knowledge(
        &self,
        title: &str,
        content: &str,
        tier: &str,
        category: &str,
        importance: i32,
        embedding: Option<&[u8]>,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO knowledge_items (id, title, content, tier, category, importance, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, title, content, tier, category, importance, embedding],
        )?;
        Ok(id)
    }

    pub fn get_knowledge_by_tier(
        &self,
        tier: &str,
        limit: u32,
    ) -> SqlResult<Vec<KnowledgeRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, content, tier, category, importance, created_at
             FROM knowledge_items WHERE tier = ?1
             ORDER BY importance DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![tier, limit], |row| {
            Ok(KnowledgeRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tier: row.get(3)?,
                category: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    // ── KG CRUD ────────────────────────────────────────────

    pub fn insert_kg_node(
        &self,
        id: &str,
        kind: &str,
        label: &str,
        properties: Option<&str>,
        embedding: Option<&[u8]>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO kg_nodes (id, kind, label, properties, embedding,
             updated_at) VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![id, kind, label, properties, embedding],
        )?;
        Ok(())
    }

    pub fn insert_kg_edge(
        &self,
        source_id: &str,
        target_id: &str,
        kind: &str,
        weight: f64,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO kg_edges (id, source_id, target_id, kind, weight)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, source_id, target_id, kind, weight],
        )?;
        Ok(id)
    }

    pub fn get_kg_neighbors(&self, node_id: &str) -> SqlResult<Vec<KgNodeRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT n.id, n.kind, n.label, n.properties, n.confidence
             FROM kg_nodes n
             INNER JOIN kg_edges e ON (e.target_id = n.id AND e.source_id = ?1)
                                   OR (e.source_id = n.id AND e.target_id = ?1)
             WHERE e.invalid_at IS NULL",
        )?;
        let rows = stmt.query_map(params![node_id], |row| {
            Ok(KgNodeRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                label: row.get(2)?,
                properties: row.get(3)?,
                confidence: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    // ── Task CRUD (Event Sourcing) ─────────────────────────

    pub fn insert_task(
        &self,
        title: &str,
        description: Option<&str>,
        priority: &str,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks (id, title, description, priority)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, title, description, priority],
        )?;
        conn.execute(
            "INSERT INTO task_events (task_id, event_type, new_value, actor)
             VALUES (?1, 'created', ?2, 'system')",
            params![id, title],
        )?;
        Ok(id)
    }

    pub fn update_task_status(&self, task_id: &str, new_status: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let old_status: String = conn
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let affected = conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![new_status, task_id],
        )?;
        if affected > 0 {
            conn.execute(
                "INSERT INTO task_events (task_id, event_type, old_value, new_value, actor)
                 VALUES (?1, 'status_changed', ?2, ?3, 'system')",
                params![task_id, old_status, new_status],
            )?;
        }
        Ok(affected > 0)
    }

    // ── Chat CRUD ──────────────────────────────────────────

    pub fn create_conversation(
        &self,
        title: Option<&str>,
        model_id: Option<&str>,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chat_conversations (id, title, model_id) VALUES (?1, ?2, ?3)",
            params![id, title, model_id],
        )?;
        Ok(id)
    }

    pub fn insert_chat_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        model_id: Option<&str>,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO chat_messages (conversation_id, role, content, model_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![conversation_id, role, content, model_id],
        )?;
        let row_id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE chat_conversations SET message_count = message_count + 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            params![conversation_id],
        )?;
        Ok(row_id)
    }

    pub fn get_chat_messages(
        &self,
        conversation_id: &str,
    ) -> SqlResult<Vec<ChatMessageRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, model_id, created_at
             FROM chat_messages WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(ChatMessageRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                model_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    // ── Embedding Cache ────────────────────────────────────

    pub fn cache_embedding(
        &self,
        content_hash: &str,
        model_id: &str,
        dimensions: u32,
        embedding: &[u8],
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO embeddings_cache (content_hash, model_id, dimensions, embedding)
             VALUES (?1, ?2, ?3, ?4)",
            params![content_hash, model_id, dimensions, embedding],
        )?;
        Ok(())
    }

    pub fn get_cached_embedding(&self, content_hash: &str) -> SqlResult<Option<Vec<u8>>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT embedding FROM embeddings_cache WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )
        .optional()
    }

    // ── System Config ──────────────────────────────────────

    pub fn set_config(&self, key: &str, value: &str, category: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value, category, updated_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![key, value, category],
        )?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> SqlResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM system_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
    }

    // ── Agent CRUD (Group E) ───────────────────────────────

    pub fn insert_agent(
        &self,
        name: &str,
        agent_type: &str,
        description: Option<&str>,
        version: Option<&str>,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO agents (id, name, agent_type, description, version, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active')",
            params![id, name, agent_type, description, version.unwrap_or("1.0.0")],
        )?;
        Ok(id)
    }

    pub fn get_agents(&self, status: Option<&str>) -> SqlResult<Vec<AgentRecord>> {
        let conn = self.conn.lock();
        let (sql, p): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
            Some(s) => (
                "SELECT id, name, agent_type, status, version, description, registered_at
                 FROM agents WHERE status = ?1 ORDER BY name".into(),
                vec![Box::new(s.to_string())],
            ),
            None => (
                "SELECT id, name, agent_type, status, version, description, registered_at
                 FROM agents ORDER BY name".into(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(AgentRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                agent_type: row.get(2)?,
                status: row.get(3)?,
                version: row.get(4)?,
                description: row.get(5)?,
                registered_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_agent_status(&self, agent_id: &str, status: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE agents SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![status, agent_id],
        )?;
        Ok(affected > 0)
    }

    // ── Rule CRUD (Group F) ──────────────────────────────

    pub fn insert_rule(
        &self,
        rule_id: &str,
        class: &str,
        priority: i32,
        title: &str,
        content: &str,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO rule_definitions (rule_id, class, priority, title, content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![rule_id, class, priority, title, content],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_rules_by_class(&self, class: &str) -> SqlResult<Vec<RuleRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, rule_id, class, priority, title, content, mandatory
             FROM rule_definitions WHERE class = ?1 AND deprecated = 0
             ORDER BY priority ASC",
        )?;
        let rows = stmt.query_map(params![class], |row| {
            Ok(RuleRecord {
                id: row.get(0)?,
                rule_id: row.get(1)?,
                class: row.get(2)?,
                priority: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                mandatory: row.get::<_, i32>(6)? != 0,
            })
        })?;
        rows.collect()
    }

    // ── Document CRUD (Group G) ──────────────────────────

    pub fn insert_document(
        &self,
        project: &str,
        category: &str,
        title: &str,
        full_text: Option<&str>,
        summary: Option<&str>,
        embedding: Option<&[u8]>,
    ) -> SqlResult<String> {
        let conn = self.conn.lock();
        let doc_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (doc_id, project, category, title, full_text, summary, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![doc_id, project, category, title, full_text, summary, embedding],
        )?;
        Ok(doc_id)
    }

    pub fn get_documents_by_project(&self, project: &str) -> SqlResult<Vec<DocumentRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, doc_id, project, category, title, summary, version
             FROM documents WHERE project = ?1 AND deprecated = 0
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![project], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                doc_id: row.get(1)?,
                project: row.get(2)?,
                category: row.get(3)?,
                title: row.get(4)?,
                summary: row.get(5)?,
                version: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    // ── Component CRUD (Group M) ─────────────────────────

    pub fn insert_component(
        &self,
        name: &str,
        category: &str,
        description: Option<&str>,
        version: Option<&str>,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO components (name, category, description, version)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, category, description, version],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_components(&self, category: Option<&str>) -> SqlResult<Vec<ComponentRecord>> {
        let conn = self.conn.lock();
        let (sql, p): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match category {
            Some(c) => (
                "SELECT id, name, category, health, version FROM components WHERE category = ?1 ORDER BY name".into(),
                vec![Box::new(c.to_string())],
            ),
            None => (
                "SELECT id, name, category, health, version FROM components ORDER BY name".into(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(ComponentRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                category: row.get(2)?,
                health: row.get(3)?,
                version: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_component_health(&self, name: &str, health: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE components SET health = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE name = ?2",
            params![health, name],
        )?;
        // Record in health_history
        if affected > 0 {
            conn.execute(
                "INSERT INTO health_history (component_name, health) VALUES (?1, ?2)",
                params![name, health],
            )?;
        }
        Ok(affected > 0)
    }

    // ── Trust Scores (Group J — Hebbian, Bi & Poo 1998) ──

    pub fn upsert_trust_score(
        &self,
        task_name: &str,
        score: f64,
        success: bool,
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        let (succ_inc, fail_inc) = if success { (1, 0) } else { (0, 1) };
        conn.execute(
            "INSERT INTO trust_scores (task_name, score, successes, failures)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(task_name) DO UPDATE SET
                score = ?2,
                successes = successes + ?3,
                failures = failures + ?4,
                last_updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            params![task_name, score, succ_inc, fail_inc],
        )?;
        Ok(())
    }

    pub fn get_trust_score(&self, task_name: &str) -> SqlResult<Option<TrustScoreRecord>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT task_name, score, successes, failures FROM trust_scores WHERE task_name = ?1",
            params![task_name],
            |row| {
                Ok(TrustScoreRecord {
                    task_name: row.get(0)?,
                    score: row.get(1)?,
                    successes: row.get(2)?,
                    failures: row.get(3)?,
                })
            },
        )
        .optional()
    }

    // ── Incidents (Group M — MAPE-K, IBM 2003) ──────────

    pub fn insert_incident(
        &self,
        title: &str,
        severity: &str,
        component_name: Option<&str>,
        description: Option<&str>,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO incidents (title, severity, component_name, description)
             VALUES (?1, ?2, ?3, ?4)",
            params![title, severity, component_name, description],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_open_incidents(&self) -> SqlResult<Vec<IncidentRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, component_name, severity, title, status, started_at
             FROM incidents WHERE status NOT IN ('resolved','closed')
             ORDER BY CASE severity
                WHEN 'critical' THEN 0 WHEN 'high' THEN 1
                WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(IncidentRecord {
                id: row.get(0)?,
                component_name: row.get(1)?,
                severity: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                started_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn resolve_incident(&self, id: i64, resolution: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE incidents SET status = 'resolved', resolution = ?1,
             resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![resolution, id],
        )?;
        Ok(affected > 0)
    }

    // ── Memory Lifecycle (Group I) ───────────────────────

    pub fn log_memory_event(
        &self,
        memory_id: &str,
        event_type: &str,
        event_data: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO memory_lifecycle (memory_id, event_type, event_data)
             VALUES (?1, ?2, ?3)",
            params![memory_id, event_type, event_data.unwrap_or("{}")],
        )?;
        Ok(())
    }

    // ── Performance Metrics (Group M) ────────────────────

    pub fn record_metric(
        &self,
        component: &str,
        metric_name: &str,
        value: f64,
        unit: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO performance_metrics (component_name, metric_name, metric_value, unit)
             VALUES (?1, ?2, ?3, ?4)",
            params![component, metric_name, value, unit],
        )?;
        Ok(())
    }

    // ── Vacuum & Maintenance ───────────────────────────────

    pub fn vacuum(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    pub fn cleanup_expired_cache(&self) -> SqlResult<u64> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "DELETE FROM context_cache WHERE expires_at < strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            [],
        )?;
        Ok(affected as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> ForgeMemoryStore {
        ForgeMemoryStore::open_memory().unwrap()
    }

    #[test]
    fn test_open_memory_db() {
        let store = test_store();
        let stats = store.stats().unwrap();
        assert_eq!(stats.total_memories, 0);
        assert_eq!(stats.total_knowledge, 0);
        assert_eq!(stats.total_kg_nodes, 0);
        assert_eq!(stats.schema_version, 2);
    }

    #[test]
    fn test_memory_crud() {
        let store = test_store();
        let id = store
            .insert_memory("core", "general", Some("project"), "ImpForge", 0.9, None)
            .unwrap();
        let mems = store.get_memories_by_scope("core", 10).unwrap();
        assert_eq!(mems.len(), 1);
        assert_eq!(mems[0].content, "ImpForge");
        assert!(store.delete_memory(&id).unwrap());
        assert_eq!(store.get_memories_by_scope("core", 10).unwrap().len(), 0);
    }

    #[test]
    fn test_memory_scope_update() {
        let store = test_store();
        let id = store
            .insert_memory("recall", "general", None, "test", 0.5, None)
            .unwrap();
        assert!(store.update_memory_scope(&id, "core").unwrap());
        let core = store.get_memories_by_scope("core", 10).unwrap();
        assert_eq!(core.len(), 1);
        assert_eq!(
            store.get_memories_by_scope("recall", 10).unwrap().len(),
            0
        );
    }

    #[test]
    fn test_knowledge_crud() {
        let store = test_store();
        store
            .insert_knowledge("HNSW", "Vector index algorithm", "golden", "algorithms", 5, None)
            .unwrap();
        store
            .insert_knowledge("BM25", "Text ranking", "verified", "algorithms", 4, None)
            .unwrap();
        let golden = store.get_knowledge_by_tier("golden", 10).unwrap();
        assert_eq!(golden.len(), 1);
        assert_eq!(golden[0].title, "HNSW");
    }

    #[test]
    fn test_kg_node_edge() {
        let store = test_store();
        store
            .insert_kg_node("file:main.rs", "file", "main.rs", None, None)
            .unwrap();
        store
            .insert_kg_node("fn:main", "function", "main", None, None)
            .unwrap();
        store
            .insert_kg_edge("file:main.rs", "fn:main", "contains", 1.0)
            .unwrap();
        let neighbors = store.get_kg_neighbors("file:main.rs").unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].id, "fn:main");
    }

    #[test]
    fn test_task_event_sourcing() {
        let store = test_store();
        let id = store
            .insert_task("Build HNSW", Some("Vector index"), "high")
            .unwrap();
        assert!(store.update_task_status(&id, "in_progress").unwrap());
        assert!(store.update_task_status(&id, "completed").unwrap());
        let stats = store.stats().unwrap();
        assert_eq!(stats.total_memories, 0); // tasks don't count as memories
    }

    #[test]
    fn test_chat_persistence() {
        let store = test_store();
        let conv = store
            .create_conversation(Some("Test chat"), Some("dolphin3:8b"))
            .unwrap();
        store
            .insert_chat_message(&conv, "user", "Hello!", None)
            .unwrap();
        store
            .insert_chat_message(&conv, "assistant", "Hi there!", Some("dolphin3:8b"))
            .unwrap();
        let msgs = store.get_chat_messages(&conv).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].role, "assistant");
    }

    #[test]
    fn test_config_and_cache() {
        let store = test_store();
        store.set_config("theme", "dark", "ui").unwrap();
        assert_eq!(store.get_config("theme").unwrap(), Some("dark".into()));
        assert_eq!(store.get_config("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_embedding_cache() {
        let store = test_store();
        let hash = "abc123";
        let embedding = vec![0u8; 384 * 4]; // 384 f32s as bytes
        store
            .cache_embedding(hash, "minilm", 384, &embedding)
            .unwrap();
        let cached = store.get_cached_embedding(hash).unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 384 * 4);
    }

    // ── v002 Enterprise Expansion Tests ─────────────────

    #[test]
    fn test_schema_v2_applied() {
        let store = test_store();
        let stats = store.stats().unwrap();
        assert_eq!(stats.schema_version, 2);
        assert_eq!(stats.total_agents, 0);
        assert_eq!(stats.total_rules, 0);
        assert_eq!(stats.total_documents, 0);
        assert_eq!(stats.total_components, 0);
        assert_eq!(stats.total_incidents, 0);
    }

    #[test]
    fn test_agent_crud() {
        let store = test_store();
        let id = store
            .insert_agent("code-assistant", "coder", Some("AI code helper"), Some("2.0.0"))
            .unwrap();
        assert!(!id.is_empty());

        let agents = store.get_agents(Some("active")).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "code-assistant");
        assert_eq!(agents[0].agent_type, "coder");

        assert!(store.update_agent_status(&id, "inactive").unwrap());
        let active = store.get_agents(Some("active")).unwrap();
        assert_eq!(active.len(), 0);

        let all = store.get_agents(None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, "inactive");
    }

    #[test]
    fn test_rule_definitions() {
        let store = test_store();
        store
            .insert_rule("RULE-001", "000", 1, "Always test", "All code must have tests")
            .unwrap();
        store
            .insert_rule("RULE-002", "000", 2, "No secrets", "Never commit credentials")
            .unwrap();
        store
            .insert_rule("RULE-003", "0", 10, "Code style", "Follow project conventions")
            .unwrap();

        let absolute = store.get_rules_by_class("000").unwrap();
        assert_eq!(absolute.len(), 2);
        assert_eq!(absolute[0].title, "Always test"); // priority 1 first
        assert!(absolute[0].mandatory);

        let standard = store.get_rules_by_class("0").unwrap();
        assert_eq!(standard.len(), 1);
    }

    #[test]
    fn test_document_management() {
        let store = test_store();
        let doc_id = store
            .insert_document("my-project", "architecture", "System Design", Some("Full text..."), Some("Summary"), None)
            .unwrap();
        assert!(!doc_id.is_empty());

        let docs = store.get_documents_by_project("my-project").unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "System Design");
        assert_eq!(docs[0].category, "architecture");

        let empty = store.get_documents_by_project("nonexistent").unwrap();
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_component_health() {
        let store = test_store();
        store
            .insert_component("ollama", "service", Some("Local LLM server"), Some("0.17"))
            .unwrap();
        store
            .insert_component("fastembed", "model", Some("Embedding engine"), Some("5.12"))
            .unwrap();

        let all = store.get_components(None).unwrap();
        assert_eq!(all.len(), 2);

        let services = store.get_components(Some("service")).unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "ollama");
        assert_eq!(services[0].health, "unknown");

        assert!(store.update_component_health("ollama", "healthy").unwrap());
        let updated = store.get_components(Some("service")).unwrap();
        assert_eq!(updated[0].health, "healthy");
    }

    #[test]
    fn test_trust_scores_hebbian() {
        let store = test_store();
        store.upsert_trust_score("code_review", 0.8, true).unwrap();
        let trust = store.get_trust_score("code_review").unwrap().unwrap();
        assert_eq!(trust.score, 0.8);
        assert_eq!(trust.successes, 1);
        assert_eq!(trust.failures, 0);

        store.upsert_trust_score("code_review", 0.75, false).unwrap();
        let trust = store.get_trust_score("code_review").unwrap().unwrap();
        assert_eq!(trust.score, 0.75);
        assert_eq!(trust.successes, 1);
        assert_eq!(trust.failures, 1);

        assert!(store.get_trust_score("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_incidents_mape_k() {
        let store = test_store();
        store
            .insert_component("search-engine", "service", None, None)
            .unwrap();

        let inc_id = store
            .insert_incident("Search latency spike", "high", Some("search-engine"), Some("P95 > 500ms"))
            .unwrap();

        let open = store.get_open_incidents().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].title, "Search latency spike");
        assert_eq!(open[0].severity, Some("high".into()));

        assert!(store.resolve_incident(inc_id, "Increased cache size").unwrap());
        let open_after = store.get_open_incidents().unwrap();
        assert_eq!(open_after.len(), 0);
    }

    #[test]
    fn test_memory_lifecycle() {
        let store = test_store();
        let mem_id = store
            .insert_memory("recall", "general", None, "test data", 0.5, None)
            .unwrap();
        store.log_memory_event(&mem_id, "created", None).unwrap();
        store.log_memory_event(&mem_id, "accessed", Some("{\"source\":\"search\"}")).unwrap();
        store.log_memory_event(&mem_id, "promoted", Some("{\"from\":\"recall\",\"to\":\"core\"}")).unwrap();
        // No panic = events logged correctly
    }

    #[test]
    fn test_performance_metrics() {
        let store = test_store();
        store.record_metric("hnsw-index", "search_latency_ms", 12.5, Some("ms")).unwrap();
        store.record_metric("hnsw-index", "search_latency_ms", 8.3, Some("ms")).unwrap();
        store.record_metric("bm25-engine", "index_size", 50000.0, Some("terms")).unwrap();
        // No panic = metrics recorded correctly
    }

    #[test]
    fn test_80_table_creation() {
        // Verify all 80 tables (25 v001 + 55 v002) exist
        let store = test_store();
        let conn = store.conn.lock();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                 AND name NOT LIKE 'sqlite_%' AND name != 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 80, "Expected 80 tables, found {count}");
    }
}
