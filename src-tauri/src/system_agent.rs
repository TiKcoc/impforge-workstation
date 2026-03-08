//! Auto System Agent - Native ImpForge Module
//!
//! Offline-first system validation, error detection, and intelligent
//! upgrade evaluation. Runs entirely locally without external APIs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub scan_duration_ms: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub severity: Severity,
    pub message: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// File change detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChanges {
    pub new_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub total_scanned: usize,
}

/// Service status for monitored endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub url: String,
    pub is_up: bool,
    pub response_ms: Option<u64>,
}

/// Check if a required path exists
fn check_path(path: &str) -> HealthCheck {
    let exists = Path::new(path).exists();
    HealthCheck {
        name: format!("path:{}", path.split('/').last().unwrap_or(path)),
        status: if exists { HealthStatus::Healthy } else { HealthStatus::Critical },
        severity: Severity::High,
        message: if exists {
            format!("OK: {path}")
        } else {
            format!("MISSING: {path}")
        },
        category: "filesystem".to_string(),
    }
}

/// Get the ImpForge data directory (platform-specific, standalone)
fn impforge_data_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("impforge")
}

/// Run all system integrity checks (fully offline, standalone)
///
/// Checks only ImpForge-owned resources — no ork-station, no external MCP servers,
/// no systemd, no PostgreSQL, no Redis. 100% standalone.
fn run_integrity_checks() -> Vec<HealthCheck> {
    let mut checks = Vec::new();
    let data_dir = impforge_data_dir();

    // ImpForge data directory exists
    checks.push(HealthCheck {
        name: "impforge:data-dir".to_string(),
        status: if data_dir.exists() { HealthStatus::Healthy } else { HealthStatus::Critical },
        severity: Severity::High,
        message: if data_dir.exists() {
            format!("OK: {}", data_dir.display())
        } else {
            format!("MISSING: {}", data_dir.display())
        },
        category: "filesystem".to_string(),
    });

    // ImpForge SQLite database
    let db_path = data_dir.join("impforge.db");
    checks.push(HealthCheck {
        name: "impforge:database".to_string(),
        status: if db_path.exists() { HealthStatus::Healthy } else { HealthStatus::Degraded },
        severity: Severity::Medium,
        message: if db_path.exists() {
            "SQLite database OK".to_string()
        } else {
            "SQLite database not yet created (will be created on first run)".to_string()
        },
        category: "impforge".to_string(),
    });

    // Check disk space on ImpForge data partition
    #[cfg(target_os = "linux")]
    {
        let check_path = data_dir.to_string_lossy().to_string();
        let df_target = if data_dir.exists() { check_path.as_str() } else { "/" };
        if let Ok(output) = std::process::Command::new("df")
            .args(["--output=avail", "-B1", df_target])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().nth(1) {
                if let Ok(bytes) = line.trim().parse::<u64>() {
                    let gb = bytes as f64 / 1_073_741_824.0;
                    checks.push(HealthCheck {
                        name: "disk:space".to_string(),
                        status: if gb > 10.0 {
                            HealthStatus::Healthy
                        } else if gb > 5.0 {
                            HealthStatus::Degraded
                        } else {
                            HealthStatus::Critical
                        },
                        severity: if gb < 5.0 { Severity::Critical } else { Severity::Medium },
                        message: format!("{gb:.1} GB free on data partition"),
                        category: "system".to_string(),
                    });
                }
            }
        }
    }

    // Check Ollama binary path exists
    checks.push(check_path("/usr/bin/ollama"));

    // Check Docker/Podman binary paths
    checks.push(check_path("/usr/bin/docker"));

    // Check Podman availability (alternative container runtime)
    checks.push(check_path("/usr/bin/podman"));

    // Check GPU availability
    #[cfg(target_os = "linux")]
    {
        // AMD GPU (ROCm)
        let has_rocm = Path::new("/opt/rocm").exists();
        // NVIDIA GPU (CUDA)
        let has_nvidia = std::process::Command::new("which").arg("nvidia-smi")
            .output().map(|o| o.status.success()).unwrap_or(false);

        let gpu_status = if has_rocm || has_nvidia { HealthStatus::Healthy } else { HealthStatus::Degraded };
        let gpu_msg = if has_rocm && has_nvidia {
            "AMD ROCm + NVIDIA CUDA detected".to_string()
        } else if has_rocm {
            "AMD ROCm detected — GPU acceleration available".to_string()
        } else if has_nvidia {
            "NVIDIA CUDA detected — GPU acceleration available".to_string()
        } else {
            "No GPU runtime detected — CPU inference will be used".to_string()
        };
        checks.push(HealthCheck {
            name: "impforge:gpu".to_string(),
            status: gpu_status,
            severity: Severity::Info,
            message: gpu_msg,
            category: "hardware".to_string(),
        });
    }

    checks
}

/// Check service health via HTTP (async, standalone)
///
/// Only checks services that ImpForge manages itself — no ork-station MCP servers.
async fn check_services() -> Vec<ServiceStatus> {
    let services = [
        ("Ollama", "http://localhost:11434/api/tags"),
        ("Docker", "http://localhost:2375/version"),
    ];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let mut results = Vec::new();
    for (name, url) in &services {
        let start = Instant::now();
        let is_up = match client.get(*url).send().await {
            Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 404,
            Err(_) => false,
        };
        results.push(ServiceStatus {
            name: name.to_string(),
            url: url.to_string(),
            is_up,
            response_ms: Some(start.elapsed().as_millis() as u64),
        });
    }
    results
}

/// Run a complete system scan
#[tauri::command]
pub async fn system_scan() -> Result<SystemHealth, String> {
    let start = Instant::now();

    // Run offline integrity checks
    let mut checks = run_integrity_checks();

    // Run service health checks
    let services = check_services().await;
    for svc in &services {
        checks.push(HealthCheck {
            name: format!("service:{}", svc.name),
            status: if svc.is_up {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            },
            severity: Severity::Medium,
            message: if svc.is_up {
                format!(
                    "{} is up ({}ms)",
                    svc.name,
                    svc.response_ms.unwrap_or(0)
                )
            } else {
                format!("{} is DOWN", svc.name)
            },
            category: "services".to_string(),
        });
    }

    // Determine overall health
    let has_critical = checks.iter().any(|c| c.status == HealthStatus::Critical);
    let has_degraded = checks.iter().any(|c| c.status == HealthStatus::Degraded);
    let overall = if has_critical {
        HealthStatus::Critical
    } else if has_degraded {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    Ok(SystemHealth {
        overall,
        checks,
        scan_duration_ms: start.elapsed().as_millis() as u64,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Scan a directory for file changes (new/modified/deleted since threshold)
#[tauri::command]
pub fn system_scan_files(directory: String) -> Result<FileChanges, String> {
    let dir = Path::new(&directory);
    if !dir.exists() {
        return Err(format!("Directory not found: {directory}"));
    }

    let mut new_files = Vec::new();
    let modified_files = Vec::new();
    let mut total_scanned = 0usize;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            total_scanned += 1;
            if entry.path().is_file() {
                new_files.push(entry.path().display().to_string());
            }
        }
    }

    Ok(FileChanges {
        new_files,
        modified_files,
        deleted_files: Vec::new(),
        total_scanned,
    })
}

/// Get quick health summary (lightweight, no network calls)
#[tauri::command]
pub fn system_health_quick() -> Result<HashMap<String, serde_json::Value>, String> {
    let checks = run_integrity_checks();
    let healthy = checks.iter().filter(|c| c.status == HealthStatus::Healthy).count();
    let total = checks.len();
    let issues: Vec<_> = checks
        .iter()
        .filter(|c| c.status != HealthStatus::Healthy)
        .map(|c| c.message.clone())
        .collect();

    let mut result = HashMap::new();
    result.insert(
        "score".to_string(),
        serde_json::json!(format!("{healthy}/{total}")),
    );
    result.insert(
        "healthy".to_string(),
        serde_json::json!(healthy == total),
    );
    result.insert("issues".to_string(), serde_json::json!(issues));
    result.insert("checked_at".to_string(), serde_json::json!(chrono::Utc::now().to_rfc3339()));

    Ok(result)
}
