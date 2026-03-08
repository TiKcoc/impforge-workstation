//! NeuralSwarm — ImpForge-native AI Orchestrator (STANDALONE)
//!
//! Tauri command interface to the ImpForge standalone orchestrator.
//! This bridges the orchestrator module to the Svelte frontend.
//!
//! 100% standalone — no systemd, no PostgreSQL, no Redis.
//! Works on any customer PC: Linux, Windows, macOS.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::orchestrator::ImpForgeOrchestrator;

/// Global orchestrator instance
static ORCHESTRATOR: OnceCell<Arc<Mutex<ImpForgeOrchestrator>>> = OnceCell::new();

fn get_orchestrator() -> Result<&'static Arc<Mutex<ImpForgeOrchestrator>>, String> {
    ORCHESTRATOR.get().ok_or_else(|| {
        "Orchestrator not initialized. Call neuralswarm_action('start') first.".to_string()
    })
}

fn init_orchestrator() -> Result<&'static Arc<Mutex<ImpForgeOrchestrator>>, String> {
    ORCHESTRATOR.get_or_try_init(|| {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("impforge");

        std::fs::create_dir_all(&data_dir)
            .map_err(|e| format!("Failed to create data dir: {e}"))?;

        // Use community (simple trust) or pro (Hebbian) based on env toggle
        let orch = if std::env::var("IMPFORGE_COMMUNITY_MODE").is_ok() {
            ImpForgeOrchestrator::new_community(data_dir)
        } else {
            ImpForgeOrchestrator::new(data_dir)
        }
        .map_err(|e| format!("Failed to create orchestrator: {e}"))?;

        Ok(Arc::new(Mutex::new(orch)))
    })
}

/// Orchestrator status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    pub running: bool,
    pub task_count: usize,
    pub tasks_ok: usize,
    pub tasks_fail: usize,
    pub uptime_seconds: u64,
    pub avg_trust: f64,
}

/// Task status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub name: String,
    pub description: String,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub trust: f64,
    pub last_run: Option<String>,
    pub pool: String,
    pub enabled: bool,
}

/// Service health for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: String,
    pub endpoint: Option<String>,
    pub response_time_ms: Option<u64>,
}

/// Full snapshot for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralSwarmSnapshot {
    pub status: OrchestratorStatus,
    pub tasks: Vec<TaskStatus>,
    pub services: Vec<ServiceHealth>,
}

/// Get orchestrator status
#[tauri::command]
pub async fn neuralswarm_status() -> Result<OrchestratorStatus, String> {
    match get_orchestrator() {
        Ok(orch) => {
            let orch = orch.lock().await;
            let status = orch.status().await;
            Ok(OrchestratorStatus {
                running: status.running,
                task_count: status.task_count,
                tasks_ok: status.tasks_ok,
                tasks_fail: status.tasks_fail,
                uptime_seconds: status.uptime_seconds,
                avg_trust: status.avg_trust,
            })
        }
        Err(_) => Ok(OrchestratorStatus {
            running: false,
            task_count: 42,
            tasks_ok: 0,
            tasks_fail: 0,
            uptime_seconds: 0,
            avg_trust: 0.5,
        }),
    }
}

/// Get task statuses
#[tauri::command]
pub async fn neuralswarm_tasks() -> Result<Vec<TaskStatus>, String> {
    match get_orchestrator() {
        Ok(orch) => {
            let orch = orch.lock().await;
            let tasks = orch.task_statuses().await;
            Ok(tasks
                .into_iter()
                .map(|t| TaskStatus {
                    name: t.name,
                    description: t.description,
                    status: t.status,
                    duration_ms: t.duration_ms,
                    trust: t.trust,
                    last_run: t.last_run,
                    pool: t.pool,
                    enabled: t.enabled,
                })
                .collect())
        }
        Err(_) => Ok(vec![]),
    }
}

/// Get recent orchestrator logs
#[tauri::command]
pub async fn neuralswarm_logs(lines: Option<u32>) -> Result<String, String> {
    match get_orchestrator() {
        Ok(orch) => {
            let orch = orch.lock().await;
            let logs = orch.recent_logs(lines.unwrap_or(50)).await;
            let output = logs
                .iter()
                .map(|l| {
                    format!(
                        "[{}] {} — {} ({}ms)",
                        l.created_at.format("%H:%M:%S"),
                        l.worker_name,
                        l.result_summary.as_deref().unwrap_or(&l.status),
                        l.duration_ms
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(if output.is_empty() {
                "No logs yet. Start the orchestrator to begin collecting data.".to_string()
            } else {
                output
            })
        }
        Err(_) => Ok(
            "ImpForge Orchestrator not yet started. Click 'Start' to begin.".to_string(),
        ),
    }
}

/// Control the orchestrator
#[tauri::command]
pub async fn neuralswarm_action(action: String) -> Result<String, String> {
    match action.as_str() {
        "start" => {
            let orch = init_orchestrator()?;
            let orch = orch.lock().await;
            orch.start().await?;
            Ok("ImpForge Orchestrator started with 42 workers".to_string())
        }
        "stop" => {
            let orch = get_orchestrator()?;
            let orch = orch.lock().await;
            orch.stop().await?;
            Ok("ImpForge Orchestrator stopped".to_string())
        }
        "restart" => {
            if let Ok(orch) = get_orchestrator() {
                let orch = orch.lock().await;
                orch.stop().await.ok();
                orch.start().await?;
                Ok("ImpForge Orchestrator restarted".to_string())
            } else {
                let orch = init_orchestrator()?;
                let orch = orch.lock().await;
                orch.start().await?;
                Ok("ImpForge Orchestrator started (fresh)".to_string())
            }
        }
        _ => Err(format!("Invalid action: {action}. Use start/stop/restart")),
    }
}

/// Full snapshot (single call for UI efficiency)
#[tauri::command]
pub async fn neuralswarm_snapshot() -> Result<NeuralSwarmSnapshot, String> {
    let status = neuralswarm_status().await?;
    let tasks = neuralswarm_tasks().await?;

    // Map health states from MAPE-K loop
    let services = match get_orchestrator() {
        Ok(orch) => {
            let orch = orch.lock().await;
            let snapshot = orch.snapshot().await;
            snapshot
                .services
                .into_iter()
                .map(|s| ServiceHealth {
                    name: s.service.name,
                    status: format!("{:?}", s.status).to_lowercase(),
                    endpoint: s.service.health_url,
                    response_time_ms: s.response_time_ms,
                })
                .collect()
        }
        Err(_) => {
            // Fallback: quick Ollama check
            let ollama_ok = check_http_health("http://localhost:11434/api/tags").await;
            vec![ServiceHealth {
                name: "Ollama".to_string(),
                status: if ollama_ok {
                    "online".to_string()
                } else {
                    "offline".to_string()
                },
                endpoint: Some("http://localhost:11434".to_string()),
                response_time_ms: None,
            }]
        }
    };

    Ok(NeuralSwarmSnapshot {
        status,
        tasks,
        services,
    })
}

/// Reset circuit breaker for a service (manual intervention)
#[tauri::command]
pub async fn neuralswarm_reset_circuit_breaker(service_name: String) -> Result<String, String> {
    let orch = get_orchestrator()?;
    let orch = orch.lock().await;
    orch.reset_service_circuit_breaker(&service_name).await;
    Ok(format!("Circuit breaker reset for {service_name}"))
}

/// Get detailed trust info for a specific worker
#[tauri::command]
pub async fn neuralswarm_worker_trust(worker: String) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let orch = orch.lock().await;
    let wt = orch.worker_trust_detail(&worker).await;
    Ok(serde_json::json!({
        "name": wt.name,
        "score": wt.score,
        "successes": wt.successes,
        "failures": wt.failures,
    }))
}

/// Cleanup old orchestrator data
#[tauri::command]
pub async fn neuralswarm_cleanup(days: Option<u32>) -> Result<String, String> {
    let orch = get_orchestrator()?;
    let orch = orch.lock().await;
    let deleted = orch.cleanup_old_data(days.unwrap_or(30)).await;
    // Also optimize the SQLite query planner
    let health = orch.get_persisted_health().await;
    Ok(format!("Cleaned up {} old records, {} health entries persisted", deleted, health.len()))
}

async fn check_http_health(url: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    matches!(client.get(url).send().await, Ok(resp) if resp.status().is_success())
}
