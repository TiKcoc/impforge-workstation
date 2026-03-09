//! SQLite persistence layer for ForgeMemory
//!
//! Pattern: Same as orchestrator/store.rs -- WAL mode, bundled SQLite,
//! parking_lot::Mutex for concurrent access, auto-migrations on startup.
//!
//! 25 tables across 4 groups:
//!   A. Core Memory & Knowledge (6)
//!   B. Task & Event System (4)
//!   C. Code Intelligence (6)
//!   D. Search & AI Infrastructure (9)

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
        assert_eq!(stats.schema_version, 1);
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
}
