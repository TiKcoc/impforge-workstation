// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Activity Log / Timeline — Tracks all user actions across ImpForge modules.
//!
//! Provides a chronological record of everything the user does:
//! document creation, workflow runs, emails sent, agent tasks, etc.
//!
//! Data is persisted to `~/.impforge/activity_log.json` — fully offline,
//! no telemetry, no network.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single entry in the activity timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: String,
    /// Machine-readable action key (e.g. "created_document", "ran_workflow").
    pub action: String,
    /// Module that generated the activity (e.g. "writer", "workflows", "mail").
    pub module: String,
    /// Human-readable title for display.
    pub title: String,
    /// Optional additional detail (e.g. document name, model used).
    pub detail: Option<String>,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

/// In-memory store protected by a Mutex.
pub struct ActivityStore {
    entries: Vec<ActivityEntry>,
    dirty: bool,
}

impl ActivityStore {
    pub fn new() -> Self {
        let entries = load_from_disk();
        Self {
            entries,
            dirty: false,
        }
    }

    fn persist(&mut self) {
        if self.dirty {
            save_to_disk(&self.entries);
            self.dirty = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

fn storage_path() -> PathBuf {
    data_dir().join("activity_log.json")
}

fn load_from_disk() -> Vec<ActivityEntry> {
    let path = storage_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_to_disk(entries: &[ActivityEntry]) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let json = serde_json::to_string_pretty(entries).unwrap_or_default();
    let _ = std::fs::write(storage_path(), json);
}

fn generate_id() -> String {
    format!("act_{}", uuid::Uuid::new_v4().as_simple())
}

/// Parse an ISO 8601 date string and return just the date portion (YYYY-MM-DD).
fn date_of(timestamp: &str) -> Option<String> {
    // Accept both full RFC 3339 and date-only formats
    timestamp.get(..10).map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Get the activity log, limited to the most recent `limit` entries.
#[tauri::command]
pub fn activity_log(
    limit: u32,
    store: tauri::State<'_, Mutex<ActivityStore>>,
) -> AppResult<Vec<ActivityEntry>> {
    let guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let max = limit as usize;
    Ok(guard.entries.iter().take(max).cloned().collect())
}

/// Get today's activity entries only.
#[tauri::command]
pub fn activity_log_today(
    store: tauri::State<'_, Mutex<ActivityStore>>,
) -> AppResult<Vec<ActivityEntry>> {
    let guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let results: Vec<ActivityEntry> = guard
        .entries
        .iter()
        .filter(|e| date_of(&e.timestamp).as_deref() == Some(&today))
        .cloned()
        .collect();

    Ok(results)
}

/// Track a new activity event. Called by any module when something notable happens.
#[tauri::command]
pub fn activity_track(
    action: String,
    module: String,
    title: String,
    detail: Option<String>,
    store: tauri::State<'_, Mutex<ActivityStore>>,
) -> AppResult<()> {
    let entry = ActivityEntry {
        id: generate_id(),
        action,
        module,
        title,
        detail,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    // Prepend (newest first)
    guard.entries.insert(0, entry);

    // Cap at 500 entries
    guard.entries.truncate(500);
    guard.dirty = true;
    guard.persist();

    Ok(())
}

/// Get a summary: how many actions per module today.
#[tauri::command]
pub fn activity_summary(
    store: tauri::State<'_, Mutex<ActivityStore>>,
) -> AppResult<Vec<(String, u32)>> {
    let guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for entry in &guard.entries {
        if date_of(&entry.timestamp).as_deref() == Some(&today) {
            *counts.entry(entry.module.clone()).or_insert(0) += 1;
        }
    }

    let mut result: Vec<(String, u32)> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(result)
}

/// Clear the entire activity log.
#[tauri::command]
pub fn activity_clear(
    store: tauri::State<'_, Mutex<ActivityStore>>,
) -> AppResult<()> {
    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    guard.entries.clear();
    guard.dirty = true;
    guard.persist();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_of_rfc3339() {
        assert_eq!(
            date_of("2026-03-18T12:34:56Z"),
            Some("2026-03-18".to_string())
        );
    }

    #[test]
    fn test_date_of_short() {
        assert_eq!(date_of("2026-03-18"), Some("2026-03-18".to_string()));
    }

    #[test]
    fn test_date_of_too_short() {
        assert_eq!(date_of("2026"), None);
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.starts_with("act_"));
        assert!(id.len() > 8);
    }

    #[test]
    fn test_activity_entry_serialization() {
        let entry = ActivityEntry {
            id: "act_001".to_string(),
            action: "created_document".to_string(),
            module: "writer".to_string(),
            title: "Created Q1 Report".to_string(),
            detail: Some("Quarterly financial summary".to_string()),
            timestamp: "2026-03-18T10:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("created_document"));
        assert!(json.contains("writer"));

        let parsed: ActivityEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.action, "created_document");
        assert_eq!(parsed.module, "writer");
    }

    #[test]
    fn test_activity_store_new() {
        let store = ActivityStore::new();
        assert!(!store.dirty);
    }
}
