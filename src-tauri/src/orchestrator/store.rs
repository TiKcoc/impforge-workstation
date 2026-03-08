//! SQLite persistence layer for ImpForge Standalone Orchestrator
//!
//! Uses rusqlite with bundled SQLite — no external database dependency.
//! WAL mode for concurrent reads, auto-migrations on startup.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use parking_lot::Mutex;

/// Trust score record persisted in SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRecord {
    pub worker_name: String,
    pub score: f64,
    pub successes: u64,
    pub failures: u64,
    pub last_updated: DateTime<Utc>,
}

/// Task execution log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLog {
    pub id: i64,
    pub worker_name: String,
    pub status: String,
    pub duration_ms: u64,
    pub result_summary: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Memory entry (Brain v2.0 — FSRS-scored)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub key: String,
    pub content: String,
    pub importance: f64,
    pub stability: f64,
    pub difficulty: f64,
    pub retrievability: f64,
    pub last_review: DateTime<Utc>,
    pub next_review: DateTime<Utc>,
    pub reps: u32,
    pub lapses: u32,
    pub created_at: DateTime<Utc>,
}

/// Event log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub event_type: String,
    pub payload: String,
    pub created_at: DateTime<Utc>,
}

/// Health check record for MAPE-K
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRecord {
    pub service_name: String,
    pub status: String,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub restart_count: u32,
}

/// Orchestrator persistent store
pub struct OrchestratorStore {
    conn: Mutex<Connection>,
}

impl OrchestratorStore {
    /// Open or create the SQLite database at the given path
    pub fn open(db_path: &PathBuf) -> SqlResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path)?;

        // Production WAL PRAGMAs (Tauri 2 best practice — research 2025)
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", "5000")?;
        conn.pragma_update(None, "cache_size", "-16000")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "mmap_size", "268435456")?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.run_migrations()?;
        Ok(store)
    }

    /// Optimize query planner statistics on graceful shutdown
    pub fn optimize(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize;");
    }

    /// Open an in-memory database (for testing)
    pub fn open_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS trust_scores (
                worker_name TEXT PRIMARY KEY,
                score REAL NOT NULL DEFAULT 0.5,
                successes INTEGER NOT NULL DEFAULT 0,
                failures INTEGER NOT NULL DEFAULT 0,
                last_updated TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS task_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                worker_name TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                result_summary TEXT,
                error TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 0.5,
                stability REAL NOT NULL DEFAULT 1.0,
                difficulty REAL NOT NULL DEFAULT 0.3,
                retrievability REAL NOT NULL DEFAULT 1.0,
                last_review TEXT NOT NULL,
                next_review TEXT NOT NULL,
                reps INTEGER NOT NULL DEFAULT 0,
                lapses INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS health_checks (
                service_name TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'unknown',
                last_check TEXT NOT NULL,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                restart_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_task_logs_worker ON task_logs(worker_name);
            CREATE INDEX IF NOT EXISTS idx_task_logs_created ON task_logs(created_at);
            CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
            CREATE INDEX IF NOT EXISTS idx_memories_next_review ON memories(next_review);
            "
        )?;
        Ok(())
    }

    // ─── Trust Scores ───────────────────────────────────────────

    pub fn get_trust(&self, worker: &str) -> SqlResult<f64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT score FROM trust_scores WHERE worker_name = ?1",
            params![worker],
            |row| row.get(0),
        )
        .or(Ok(0.5)) // Default trust
    }

    pub fn set_trust(&self, worker: &str, score: f64, successes: u64, failures: u64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO trust_scores (worker_name, score, successes, failures, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(worker_name) DO UPDATE SET
                score = ?2, successes = ?3, failures = ?4, last_updated = ?5",
            params![worker, score, successes, failures, now],
        )?;
        Ok(())
    }

    pub fn get_all_trust(&self) -> SqlResult<Vec<TrustRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT worker_name, score, successes, failures, last_updated FROM trust_scores ORDER BY score DESC"
        )?;
        let records = stmt.query_map([], |row| {
            let ts: String = row.get(4)?;
            Ok(TrustRecord {
                worker_name: row.get(0)?,
                score: row.get(1)?,
                successes: row.get(2)?,
                failures: row.get(3)?,
                last_updated: DateTime::parse_from_rfc3339(&ts)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(records)
    }

    // ─── Task Logs ──────────────────────────────────────────────

    pub fn log_task(&self, worker: &str, status: &str, duration_ms: u64, summary: Option<&str>, error: Option<&str>) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_logs (worker_name, status, duration_ms, result_summary, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![worker, status, duration_ms, summary, error, now],
        )?;
        Ok(())
    }

    pub fn get_recent_logs(&self, limit: u32) -> SqlResult<Vec<TaskLog>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, worker_name, status, duration_ms, result_summary, error, created_at
             FROM task_logs ORDER BY created_at DESC LIMIT ?1"
        )?;
        let logs = stmt.query_map(params![limit], |row| {
            let ts: String = row.get(6)?;
            Ok(TaskLog {
                id: row.get(0)?,
                worker_name: row.get(1)?,
                status: row.get(2)?,
                duration_ms: row.get(3)?,
                result_summary: row.get(4)?,
                error: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&ts)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(logs)
    }

    pub fn get_worker_stats(&self, worker: &str) -> SqlResult<(u64, u64)> {
        let conn = self.conn.lock();
        let ok: u64 = conn.query_row(
            "SELECT COUNT(*) FROM task_logs WHERE worker_name = ?1 AND status = 'ok'",
            params![worker], |row| row.get(0),
        ).unwrap_or(0);
        let fail: u64 = conn.query_row(
            "SELECT COUNT(*) FROM task_logs WHERE worker_name = ?1 AND status = 'error'",
            params![worker], |row| row.get(0),
        ).unwrap_or(0);
        Ok((ok, fail))
    }

    // ─── Memories (Brain v2.0 FSRS) ────────────────────────────

    pub fn store_memory(&self, key: &str, content: &str, importance: f64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO memories (key, content, importance, stability, difficulty, retrievability, last_review, next_review, reps, lapses, created_at)
             VALUES (?1, ?2, ?3, 1.0, 0.3, 1.0, ?4, ?4, 0, 0, ?4)
             ON CONFLICT(key) DO UPDATE SET content = ?2, importance = ?3, last_review = ?4",
            params![key, content, importance, now],
        )?;
        Ok(())
    }

    pub fn get_memories_due_for_review(&self) -> SqlResult<Vec<MemoryEntry>> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, key, content, importance, stability, difficulty, retrievability,
                    last_review, next_review, reps, lapses, created_at
             FROM memories WHERE next_review <= ?1 ORDER BY importance DESC"
        )?;
        let entries = stmt.query_map(params![now], |row| {
            let lr: String = row.get(7)?;
            let nr: String = row.get(8)?;
            let ca: String = row.get(11)?;
            Ok(MemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                importance: row.get(3)?,
                stability: row.get(4)?,
                difficulty: row.get(5)?,
                retrievability: row.get(6)?,
                last_review: DateTime::parse_from_rfc3339(&lr).unwrap_or_default().with_timezone(&Utc),
                next_review: DateTime::parse_from_rfc3339(&nr).unwrap_or_default().with_timezone(&Utc),
                reps: row.get(9)?,
                lapses: row.get(10)?,
                created_at: DateTime::parse_from_rfc3339(&ca).unwrap_or_default().with_timezone(&Utc),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn update_memory_fsrs(&self, id: i64, stability: f64, difficulty: f64, retrievability: f64, next_review: &DateTime<Utc>, reps: u32, lapses: u32) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        let nr = next_review.to_rfc3339();
        conn.execute(
            "UPDATE memories SET stability = ?1, difficulty = ?2, retrievability = ?3,
                    next_review = ?4, last_review = ?5, reps = ?6, lapses = ?7
             WHERE id = ?8",
            params![stability, difficulty, retrievability, nr, now, reps, lapses, id],
        )?;
        Ok(())
    }

    // ─── Events ─────────────────────────────────────────────────

    pub fn log_event(&self, event_type: &str, payload: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO events (event_type, payload, created_at) VALUES (?1, ?2, ?3)",
            params![event_type, payload, now],
        )?;
        Ok(())
    }

    pub fn get_recent_events(&self, limit: u32) -> SqlResult<Vec<EventRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, event_type, payload, created_at FROM events ORDER BY created_at DESC LIMIT ?1"
        )?;
        let events = stmt.query_map(params![limit], |row| {
            let ts: String = row.get(3)?;
            Ok(EventRecord {
                id: row.get(0)?,
                event_type: row.get(1)?,
                payload: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&ts).unwrap_or_default().with_timezone(&Utc),
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(events)
    }

    // ─── Health Checks (MAPE-K) ─────────────────────────────────

    pub fn upsert_health(&self, service: &str, status: &str, failures: u32, restarts: u32) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO health_checks (service_name, status, last_check, consecutive_failures, restart_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(service_name) DO UPDATE SET
                status = ?2, last_check = ?3, consecutive_failures = ?4, restart_count = ?5",
            params![service, status, now, failures, restarts],
        )?;
        Ok(())
    }

    pub fn get_all_health(&self) -> SqlResult<Vec<HealthRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT service_name, status, last_check, consecutive_failures, restart_count FROM health_checks"
        )?;
        let records = stmt.query_map([], |row| {
            let ts: String = row.get(2)?;
            Ok(HealthRecord {
                service_name: row.get(0)?,
                status: row.get(1)?,
                last_check: DateTime::parse_from_rfc3339(&ts).unwrap_or_default().with_timezone(&Utc),
                consecutive_failures: row.get(3)?,
                restart_count: row.get(4)?,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(records)
    }

    // ─── Cleanup ────────────────────────────────────────────────

    pub fn cleanup_old_logs(&self, days: u32) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let cutoff = (Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        let deleted = conn.execute(
            "DELETE FROM task_logs WHERE created_at < ?1",
            params![cutoff],
        )?;
        conn.execute(
            "DELETE FROM events WHERE created_at < ?1",
            params![cutoff],
        )?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_trust() {
        let store = OrchestratorStore::open_memory().unwrap();
        store.set_trust("test_worker", 0.8, 10, 2).unwrap();
        let score = store.get_trust("test_worker").unwrap();
        assert!((score - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_store_task_log() {
        let store = OrchestratorStore::open_memory().unwrap();
        store.log_task("worker1", "ok", 150, Some("done"), None).unwrap();
        let logs = store.get_recent_logs(10).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].worker_name, "worker1");
    }

    #[test]
    fn test_store_memory_fsrs() {
        let store = OrchestratorStore::open_memory().unwrap();
        store.store_memory("key1", "some content", 0.8).unwrap();
        let due = store.get_memories_due_for_review().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].key, "key1");
    }

    #[test]
    fn test_store_events() {
        let store = OrchestratorStore::open_memory().unwrap();
        store.log_event("task_completed", r#"{"worker":"w1"}"#).unwrap();
        let events = store.get_recent_events(10).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_store_health() {
        let store = OrchestratorStore::open_memory().unwrap();
        store.upsert_health("ollama", "online", 0, 0).unwrap();
        let all = store.get_all_health().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, "online");
    }
}
