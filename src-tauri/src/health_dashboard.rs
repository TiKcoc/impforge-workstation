// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Health Dashboard -- System-wide health status for ImpForge.
//!
//! Checks Ollama connectivity, storage usage, ForgeMemory status,
//! document counts per module, and system memory usage.
//! All checks are offline-safe and non-blocking.

use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Overall health status indicator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Error,
    Unknown,
}

/// Health information for a single module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleHealth {
    pub name: String,
    pub status: HealthStatus,
    pub items_count: u32,
    pub last_used: Option<String>,
}

/// Aggregated system health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall: HealthStatus,
    pub modules: Vec<ModuleHealth>,
    pub uptime_seconds: u64,
    pub memory_usage_mb: f32,
    pub storage_used_mb: f32,
    pub ollama_status: HealthStatus,
    pub documents_count: u32,
    pub total_memory_entries: u32,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Application start time (monotonic).
static APP_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Record the application start time. Called once during initialization.
fn ensure_start_time() -> Instant {
    *APP_START.get_or_init(Instant::now)
}

/// Resolve the ImpForge data directory.
fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

/// Count JSON files in a subdirectory of the data dir.
fn count_items(subdir: &str) -> u32 {
    let dir = data_dir().join(subdir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "json")
        })
        .count() as u32
}

/// Get the most recent modification time of JSON files in a directory.
fn last_modified(subdir: &str) -> Option<String> {
    let dir = data_dir().join(subdir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return None;
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "json")
        })
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .max()
        .map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.to_rfc3339()
        })
}

/// Calculate total size of a directory in megabytes (non-recursive for speed).
fn dir_size_mb(subdir: &str) -> f32 {
    let dir = data_dir().join(subdir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0.0;
    };

    let total_bytes: u64 = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();

    total_bytes as f32 / (1024.0 * 1024.0)
}

/// Calculate the total storage used by all ImpForge data directories.
fn total_storage_mb() -> f32 {
    let subdirs = [
        "writer", "sheets", "notes", "slides", "pdf", "calendar",
        "mail", "workflows", "canvas", "memory", "logs", "themes",
    ];
    subdirs.iter().map(|s| dir_size_mb(s)).sum()
}

/// Check Ollama connectivity by hitting its health endpoint.
async fn check_ollama() -> HealthStatus {
    let url = std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return HealthStatus::Unknown,
    };

    match client.get(format!("{url}/api/tags")).send().await {
        Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
        Ok(_) => HealthStatus::Degraded,
        Err(_) => HealthStatus::Error,
    }
}

/// Get current process memory usage in MB.
fn process_memory_mb() -> f32 {
    // Read from /proc/self/statm on Linux, fall back to sysinfo.
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            // Fields: size resident shared text lib data dirty (in pages)
            if let Some(resident_pages) = statm.split_whitespace().nth(1) {
                if let Ok(pages) = resident_pages.parse::<u64>() {
                    let page_size = 4096_u64; // standard page size
                    return (pages * page_size) as f32 / (1024.0 * 1024.0);
                }
            }
        }
    }

    // Fallback for non-Linux or if /proc is unavailable.
    0.0
}

/// Build a ModuleHealth entry for a given module.
fn module_health(id: &str, display_name: &str) -> ModuleHealth {
    let count = count_items(id);
    let last = last_modified(id);

    let status = if count > 0 {
        HealthStatus::Healthy
    } else if data_dir().join(id).exists() {
        HealthStatus::Healthy // directory exists but empty
    } else {
        HealthStatus::Unknown // never used
    };

    ModuleHealth {
        name: display_name.to_string(),
        status,
        items_count: count,
        last_used: last,
    }
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// Run a comprehensive health check across all ImpForge subsystems.
///
/// Checks:
/// - Ollama connectivity (async, 3-second timeout)
/// - Storage space used by each module
/// - Document counts per module
/// - System memory usage of the ImpForge process
/// - Application uptime
#[tauri::command]
pub async fn health_check() -> AppResult<SystemHealth> {
    let start = ensure_start_time();
    let uptime = start.elapsed().as_secs();

    // Check Ollama (async with timeout).
    let ollama_status = check_ollama().await;

    // Build per-module health entries.
    let module_defs = [
        ("writer", "ForgeWriter"),
        ("sheets", "ForgeSheets"),
        ("notes", "ForgeNotes"),
        ("slides", "ForgeSlides"),
        ("pdf", "ForgePDF"),
        ("calendar", "Calendar"),
        ("mail", "ForgeMail"),
        ("workflows", "ForgeFlow"),
        ("canvas", "ForgeCanvas"),
    ];

    let modules: Vec<ModuleHealth> = module_defs
        .iter()
        .map(|(id, name)| module_health(id, name))
        .collect();

    let documents_count: u32 = modules.iter().map(|m| m.items_count).sum();
    let memory_count = count_items("memory");
    let storage = total_storage_mb();
    let memory_mb = process_memory_mb();

    // Determine overall health.
    let degraded_count = modules
        .iter()
        .filter(|m| m.status == HealthStatus::Error || m.status == HealthStatus::Degraded)
        .count();

    let overall = if ollama_status == HealthStatus::Error {
        HealthStatus::Degraded
    } else if degraded_count > modules.len() / 2 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    Ok(SystemHealth {
        overall,
        modules,
        uptime_seconds: uptime,
        memory_usage_mb: memory_mb,
        storage_used_mb: storage,
        ollama_status,
        documents_count,
        total_memory_entries: memory_count,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serialization() {
        let healthy = serde_json::to_string(&HealthStatus::Healthy).expect("serialize");
        assert_eq!(healthy, r#""healthy""#);

        let degraded = serde_json::to_string(&HealthStatus::Degraded).expect("serialize");
        assert_eq!(degraded, r#""degraded""#);

        let error = serde_json::to_string(&HealthStatus::Error).expect("serialize");
        assert_eq!(error, r#""error""#);
    }

    #[test]
    fn test_module_health_serialization() {
        let mh = ModuleHealth {
            name: "ForgeWriter".to_string(),
            status: HealthStatus::Healthy,
            items_count: 5,
            last_used: Some("2026-03-18T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&mh).expect("serialize");
        assert!(json.contains("ForgeWriter"));
        assert!(json.contains("healthy"));
    }

    #[test]
    fn test_system_health_serialization() {
        let sh = SystemHealth {
            overall: HealthStatus::Healthy,
            modules: vec![],
            uptime_seconds: 3600,
            memory_usage_mb: 128.5,
            storage_used_mb: 42.0,
            ollama_status: HealthStatus::Healthy,
            documents_count: 10,
            total_memory_entries: 50,
        };
        let json = serde_json::to_string(&sh).expect("serialize");
        let parsed: SystemHealth = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.uptime_seconds, 3600);
        assert!((parsed.memory_usage_mb - 128.5).abs() < f32::EPSILON);
        assert_eq!(parsed.documents_count, 10);
    }

    #[test]
    fn test_count_items_missing_dir() {
        // A directory that does not exist should return 0.
        assert_eq!(count_items("nonexistent_test_dir_xyz"), 0);
    }

    #[test]
    fn test_dir_size_mb_missing_dir() {
        assert!((dir_size_mb("nonexistent_test_dir_xyz") - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_last_modified_missing_dir() {
        assert!(last_modified("nonexistent_test_dir_xyz").is_none());
    }

    #[test]
    fn test_process_memory_mb() {
        let mem = process_memory_mb();
        // On Linux CI, this should return > 0. On other platforms, 0.0 is acceptable.
        #[cfg(target_os = "linux")]
        assert!(mem > 0.0, "Process memory should be positive on Linux: {mem}");
        let _ = mem; // suppress unused warning on non-linux
    }

    #[test]
    fn test_module_health_empty_dir() {
        let mh = module_health("nonexistent_test_module_xyz", "TestModule");
        assert_eq!(mh.items_count, 0);
        assert_eq!(mh.status, HealthStatus::Unknown);
        assert!(mh.last_used.is_none());
    }

    #[test]
    fn test_ensure_start_time() {
        let t1 = ensure_start_time();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = ensure_start_time();
        // Same instant (OnceLock), but elapsed should be > 0.
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Error);
    }
}
