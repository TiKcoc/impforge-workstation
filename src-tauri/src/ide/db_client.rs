//! Database Client — Multi-dialect SQL execution engine
//!
//! Supports SQLite (bundled via rusqlite) with connection pooling,
//! query history, and result serialization. Enterprise-grade features:
//! - Schema introspection (tables, columns, indexes, views)
//! - Query execution with timing metrics
//! - Connection management (open/close/test)
//! - Query history with search
//! - Export results as JSON/CSV

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

/// Active database connections keyed by connection ID
pub struct DbConnectionPool {
    connections: Mutex<HashMap<String, DbConn>>,
    history: Mutex<Vec<QueryHistoryEntry>>,
}

struct DbConn {
    conn: Connection,
    path: String,
    name: String,
}

impl DbConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            history: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub dialect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count: i64,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub col_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
    pub elapsed_ms: f64,
    pub affected_rows: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub query: String,
    pub connection_id: String,
    pub timestamp: String,
    pub elapsed_ms: f64,
    pub row_count: usize,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaOverview {
    pub tables: Vec<TableInfo>,
    pub views: Vec<String>,
    pub indexes: Vec<IndexInfo>,
}

/// Open a SQLite database connection
#[tauri::command]
pub fn db_connect(
    pool: State<'_, DbConnectionPool>,
    name: String,
    path: String,
) -> Result<ConnectionInfo, String> {
    let db_path = PathBuf::from(&path);

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    let conn = Connection::open_with_flags(&db_path, flags)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("Failed to set pragmas: {}", e))?;

    let id = format!("conn_{}", uuid_simple());

    let info = ConnectionInfo {
        id: id.clone(),
        name: name.clone(),
        path: path.clone(),
        dialect: "sqlite".to_string(),
    };

    let mut conns = pool.connections.lock().map_err(|e| e.to_string())?;
    conns.insert(
        id,
        DbConn {
            conn,
            path,
            name,
        },
    );

    Ok(info)
}

/// Disconnect a database
#[tauri::command]
pub fn db_disconnect(
    pool: State<'_, DbConnectionPool>,
    connection_id: String,
) -> Result<(), String> {
    let mut conns = pool.connections.lock().map_err(|e| e.to_string())?;
    conns.remove(&connection_id);
    Ok(())
}

/// List active connections
#[tauri::command]
pub fn db_list_connections(
    pool: State<'_, DbConnectionPool>,
) -> Result<Vec<ConnectionInfo>, String> {
    let conns = pool.connections.lock().map_err(|e| e.to_string())?;
    Ok(conns
        .iter()
        .map(|(id, c)| ConnectionInfo {
            id: id.clone(),
            name: c.name.clone(),
            path: c.path.clone(),
            dialect: "sqlite".to_string(),
        })
        .collect())
}

/// Execute a SQL query and return results
#[tauri::command]
pub fn db_execute_query(
    pool: State<'_, DbConnectionPool>,
    connection_id: String,
    query: String,
) -> Result<QueryResult, String> {
    let conns = pool.connections.lock().map_err(|e| e.to_string())?;
    let db = conns
        .get(&connection_id)
        .ok_or("Connection not found")?;

    let start = Instant::now();
    let trimmed = query.trim().to_uppercase();

    // Determine if this is a SELECT-like query (returns rows) or a mutation
    let is_select = trimmed.starts_with("SELECT")
        || trimmed.starts_with("PRAGMA")
        || trimmed.starts_with("EXPLAIN")
        || trimmed.starts_with("WITH");

    let result = if is_select {
        let mut stmt = db
            .conn
            .prepare(&query)
            .map_err(|e| format!("Prepare error: {}", e))?;

        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let col_count = columns.len();

        let rows: Vec<Vec<serde_json::Value>> = stmt
            .query_map([], |row| {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val = sqlite_value_to_json(row, i);
                    values.push(val);
                }
                Ok(values)
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        let row_count = rows.len();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        QueryResult {
            columns,
            rows,
            row_count,
            elapsed_ms: elapsed,
            affected_rows: None,
        }
    } else {
        let affected = db
            .conn
            .execute_batch(&query)
            .map(|_| db.conn.changes())
            .map_err(|e| format!("Execute error: {}", e))?;

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        QueryResult {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            elapsed_ms: elapsed,
            affected_rows: Some(affected as usize),
        }
    };

    // Record in history (fire-and-forget, ignore lock failure)
    if let Ok(mut hist) = pool.history.lock() {
        hist.push(QueryHistoryEntry {
            query: query.clone(),
            connection_id: connection_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            elapsed_ms: result.elapsed_ms,
            row_count: result.row_count,
            success: true,
        });
        // Keep last 500 entries
        let len = hist.len();
        if len > 500 {
            hist.drain(0..len - 500);
        }
    }

    Ok(result)
}

/// Get database schema overview (tables, views, indexes)
#[tauri::command]
pub fn db_schema(
    pool: State<'_, DbConnectionPool>,
    connection_id: String,
) -> Result<SchemaOverview, String> {
    let conns = pool.connections.lock().map_err(|e| e.to_string())?;
    let db = conns
        .get(&connection_id)
        .ok_or("Connection not found")?;

    // Tables
    let mut tables = Vec::new();
    {
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|e| e.to_string())?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        for tname in table_names {
            let row_count: i64 = db
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{}\"", tname),
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let mut col_stmt = db
                .conn
                .prepare(&format!("PRAGMA table_info(\"{}\")", tname))
                .map_err(|e| e.to_string())?;

            let columns: Vec<ColumnInfo> = col_stmt
                .query_map([], |row| {
                    Ok(ColumnInfo {
                        name: row.get(1)?,
                        col_type: row.get::<_, String>(2).unwrap_or_default(),
                        nullable: row.get::<_, i32>(3).unwrap_or(1) == 0,
                        primary_key: row.get::<_, i32>(5).unwrap_or(0) != 0,
                        default_value: row.get(4).ok(),
                    })
                })
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            tables.push(TableInfo {
                name: tname,
                row_count,
                columns,
            });
        }
    }

    // Views
    let views: Vec<String> = {
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name")
            .map_err(|e| e.to_string())?;
        let result: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    // Indexes
    let indexes: Vec<IndexInfo> = {
        let mut stmt = db
            .conn
            .prepare("SELECT name, tbl_name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|e| e.to_string())?;

        let raw: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let mut idxs = Vec::new();
        for (idx_name, tbl_name) in raw {
            let mut idx_stmt = db
                .conn
                .prepare(&format!("PRAGMA index_info(\"{}\")", idx_name))
                .map_err(|e| e.to_string())?;
            let cols: Vec<String> = idx_stmt
                .query_map([], |row| row.get(2))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            // Check uniqueness
            let unique: bool = db
                .conn
                .query_row(
                    &format!(
                        "SELECT \"unique\" FROM pragma_index_list(\"{}\") WHERE name=?1",
                        tbl_name
                    ),
                    [&idx_name],
                    |r| r.get(0),
                )
                .unwrap_or(false);

            idxs.push(IndexInfo {
                name: idx_name,
                table_name: tbl_name,
                columns: cols,
                unique,
            });
        }
        idxs
    };

    Ok(SchemaOverview {
        tables,
        views,
        indexes,
    })
}

/// Get query history
#[tauri::command]
pub fn db_query_history(
    pool: State<'_, DbConnectionPool>,
    limit: Option<usize>,
) -> Result<Vec<QueryHistoryEntry>, String> {
    let hist = pool.history.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(50);
    let start = if hist.len() > limit {
        hist.len() - limit
    } else {
        0
    };
    Ok(hist[start..].to_vec())
}

/// Export query results as CSV string
#[tauri::command]
pub fn db_export_csv(
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
) -> Result<String, String> {
    let mut csv = String::new();

    // Header
    csv.push_str(&columns.join(","));
    csv.push('\n');

    // Rows
    for row in &rows {
        let line: Vec<String> = row
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => format!("\"{}\"", s.replace('"', "\"\"")),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            })
            .collect();
        csv.push_str(&line.join(","));
        csv.push('\n');
    }

    Ok(csv)
}

// --- Helpers ---

fn sqlite_value_to_json(row: &rusqlite::Row, idx: usize) -> serde_json::Value {
    // Try types in order: integer, real, text, blob, null
    if let Ok(v) = row.get::<_, i64>(idx) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.get::<_, f64>(idx) {
        return serde_json::json!(v);
    }
    if let Ok(v) = row.get::<_, String>(idx) {
        return serde_json::Value::String(v);
    }
    if let Ok(v) = row.get::<_, Vec<u8>>(idx) {
        return serde_json::Value::String(format!("<blob {} bytes>", v.len()));
    }
    serde_json::Value::Null
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", t)
}
