#![allow(dead_code)]
//! Schema migration system for ForgeMemory
//!
//! Tracks schema versions and auto-applies migrations on startup.
//! Each migration is a separate SQL file loaded via include_str!().

use rusqlite::Connection;

pub const CURRENT_VERSION: i64 = 3;

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );"
    )?;

    let current: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .unwrap_or(0);

    if current < 1 {
        migrate_v1(conn)?;
    }
    if current < 2 {
        migrate_v2(conn)?;
    }
    if current < 3 {
        migrate_v3(conn)?;
    }

    Ok(())
}

fn migrate_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(include_str!("sql/v001_initial.sql"))?;
    conn.execute(
        "INSERT INTO schema_version (version, description) VALUES (1, 'Initial ForgeMemory schema — 25 core tables')",
        [],
    )?;
    Ok(())
}

fn migrate_v2(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(include_str!("sql/v002_enterprise_expansion.sql"))?;
    conn.execute(
        "INSERT INTO schema_version (version, description) VALUES (2, 'Enterprise expansion — 55 tables (agents, rules, docs, RLM, taxonomy, health)')",
        [],
    )?;
    Ok(())
}

fn migrate_v3(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(include_str!("sql/v003_domain_knowledge.sql"))?;
    conn.execute(
        "INSERT INTO schema_version (version, description) VALUES (3, 'Domain knowledge — 90 tables (dev, finance, research, user, web sources, AI, collab)')",
        [],
    )?;
    Ok(())
}
