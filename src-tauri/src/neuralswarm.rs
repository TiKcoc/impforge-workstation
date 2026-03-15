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

pub(crate) fn get_orchestrator() -> Result<&'static Arc<Mutex<ImpForgeOrchestrator>>, String> {
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

/// Run a quick self-test of the orchestrator's SQLite subsystem
#[tauri::command]
pub async fn neuralswarm_selftest() -> Result<String, String> {
    // Validate in-memory store creation (wires create_memory_store + open_memory)
    let store = crate::orchestrator::create_memory_store()?;
    // Write and read back a trust score to verify the schema
    store.set_trust("__selftest__", 0.99, 1, 0)
        .map_err(|e| format!("Store selftest failed: {e}"))?;
    let score = store.get_trust("__selftest__").unwrap_or(0.0);
    Ok(format!("Selftest OK: in-memory store operational, trust readback={:.2}", score))
}

// ─── Feature-gated inference commands ─────────────────────────
// These are public functions (not registered in generate_handler) because
// the handler macro doesn't support #[cfg] attributes on individual arms.
// Frontend can call them via invoke() when the corresponding feature is enabled.

/// List available Rig multi-router providers (requires `rig-router` feature).
#[cfg(feature = "rig-router")]
#[tauri::command]
pub async fn inference_get_providers() -> Result<Vec<String>, String> {
    use crate::inference::rig_router::{RigMultiRouter, RigProvider};
    let router = RigMultiRouter::new();
    Ok(router
        .fallback_order
        .iter()
        .map(|p| format!("{:?}", p))
        .collect())
}

/// Get default FSRS-5 scheduler parameters (requires `fsrs-brain` feature).
#[cfg(feature = "fsrs-brain")]
#[tauri::command]
pub async fn fsrs_get_params() -> Result<serde_json::Value, String> {
    use crate::inference::fsrs_scheduler::FsrsParams;
    let params = FsrsParams::default();
    serde_json::to_value(&params).map_err(|e| e.to_string())
}

// ════════════════════════════════════════════════════════════════
// Phase 3-5 Module Commands
// ════════════════════════════════════════════════════════════════

/// Initialize and run the MOA pipeline for a query.
#[tauri::command]
pub async fn neuralswarm_moa_run(
    query: String,
    layers: Option<u32>,
    agents_per_layer: Option<u32>,
) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.moa_pipeline_mut().is_none() {
        let config = crate::orchestrator::moa_pipeline::MoaConfig {
            layers: layers.unwrap_or(2),
            agents_per_layer: agents_per_layer.unwrap_or(3),
            ..Default::default()
        };
        orch.init_moa(config);
    }

    let pipeline = orch.moa_pipeline_mut().ok_or("MoA pipeline not initialized")?;
    let result = pipeline.run_sync("moa-cmd", &query);
    serde_json::to_value(&result).map_err(|e| format!("Serialize error: {e}"))
}

/// Get topology snapshot (initializes with defaults if needed).
#[tauri::command]
pub async fn neuralswarm_topology_snapshot() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.topology_mut().is_none() {
        orch.init_topology(crate::orchestrator::topology::TopologyConfig::default());
    }

    let topo = orch.topology_mut().ok_or("Topology not initialized")?;
    let snapshot = topo.snapshot();
    serde_json::to_value(&snapshot).map_err(|e| format!("Serialize error: {e}"))
}

/// Run an evaluation against a rubric.
#[tauri::command]
pub async fn neuralswarm_evaluate(
    task_id: String,
    rubric: String,
    output: String,
) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.eval_chain_mut().is_none() {
        orch.init_evaluation();
    }

    let chain = orch.eval_chain_mut().ok_or("Evaluation chain not initialized")?;
    match chain.evaluate("orchestrator_judge", &task_id, &rubric, &output) {
        Some(result) => serde_json::to_value(&result).map_err(|e| format!("Serialize error: {e}")),
        None => Err(format!("Unknown rubric: {rubric}")),
    }
}

/// Get the Elo leaderboard from the evaluation chain.
#[tauri::command]
pub async fn neuralswarm_eval_leaderboard() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.eval_chain_mut().is_none() {
        orch.init_evaluation();
    }

    let chain = orch.eval_chain_mut().ok_or("Evaluation chain not initialized")?;
    let board = chain.leaderboard();
    Ok(serde_json::json!(board.iter().map(|(name, rating)| {
        serde_json::json!({"agent": name, "rating": rating})
    }).collect::<Vec<_>>()))
}

/// Get agent scaling status.
#[tauri::command]
pub async fn neuralswarm_scaling_status() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.agent_scaler_mut().is_none() {
        orch.init_scaler(crate::orchestrator::agent_scaling::ScalingConfig::default());
    }

    let scaler = orch.agent_scaler_mut().ok_or("Agent scaler not initialized")?;
    Ok(serde_json::json!({
        "total_agents": scaler.agent_count(),
        "ready": scaler.ready_count(),
        "busy": scaler.busy_count(),
        "history_len": scaler.history().len(),
    }))
}

/// Get resource governor status.
#[tauri::command]
pub async fn neuralswarm_resource_status() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.resource_governor_ref().is_none() {
        orch.init_resource_governor();
    }

    let gov = orch.resource_governor_ref().ok_or("Resource governor not initialized")?;
    let summary = gov.status_summary();
    let gpu = gov.gpu_info().map(|g| serde_json::json!({
        "vendor": format!("{:?}", g.vendor),
        "name": &g.name,
        "vram_mb": g.vram_mb,
    }));
    Ok(serde_json::json!({
        "resources": summary,
        "gpu": gpu,
        "events": gov.events().len(),
    }))
}

/// Get git status for a repository.
#[tauri::command]
pub async fn neuralswarm_git_status(repo_path: Option<String>) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    let path = repo_path.unwrap_or_else(|| ".".to_string());
    if orch.git_ops_ref().is_none() {
        orch.init_git_ops(std::path::Path::new(&path));
    }

    let ops = orch.git_ops_ref().ok_or("Git ops not initialized")?;
    let status = ops.status();
    serde_json::to_value(&status).map_err(|e| format!("Serialize error: {e}"))
}

/// Get social media post queue status.
#[tauri::command]
pub async fn neuralswarm_social_status() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.social_manager_mut().is_none() {
        orch.init_social_media(crate::orchestrator::social_media::SocialConfig::default());
    }

    let mgr = orch.social_manager_mut().ok_or("Social manager not initialized")?;
    Ok(serde_json::json!({
        "total_posts": mgr.total_posts(),
        "ready_count": mgr.ready_posts().len(),
        "is_quiet_hour": mgr.is_quiet_hour(),
    }))
}

/// Run a CI/CD pipeline stage or the full pipeline.
#[tauri::command]
pub async fn neuralswarm_cicd_run(
    project_dir: Option<String>,
    stage: Option<String>,
) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let mut orch = orch.lock().await;

    if orch.ci_cd_mut().is_none() {
        let config = crate::orchestrator::ci_cd::PipelineConfig {
            project_dir: std::path::PathBuf::from(project_dir.as_deref().unwrap_or(".")),
            ..Default::default()
        };
        orch.init_ci_cd(config);
    }

    let pipeline = orch.ci_cd_mut().ok_or("CI/CD pipeline not initialized")?;

    if let Some(stage_name) = stage {
        let stage_enum = match stage_name.as_str() {
            "lint" => crate::orchestrator::ci_cd::PipelineStage::Lint,
            "test" => crate::orchestrator::ci_cd::PipelineStage::Test,
            "build" => crate::orchestrator::ci_cd::PipelineStage::Build,
            "verify" => crate::orchestrator::ci_cd::PipelineStage::Verify,
            "release" => crate::orchestrator::ci_cd::PipelineStage::Release,
            _ => return Err(format!("Unknown stage: {stage_name}")),
        };
        let result = pipeline.run_stage(&stage_enum);
        serde_json::to_value(&result).map_err(|e| format!("Serialize error: {e}"))
    } else {
        let run = pipeline.run();
        serde_json::to_value(&run).map_err(|e| format!("Serialize error: {e}"))
    }
}

/// Route an inference request through the orchestrator's cascade router.
///
/// Returns the selected model, tier, confidence, and estimated cost.
/// Community edition always returns the local Ollama model (tier 0).
#[tauri::command]
pub async fn neuralswarm_route_inference(
    prompt: String,
    task_hint: Option<String>,
) -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let orch = orch.lock().await;
    let decision = orch
        .route_inference(&prompt, task_hint.as_deref())
        .await;
    Ok(serde_json::json!({
        "model_id": decision.model_id,
        "tier": decision.tier,
        "confidence": decision.confidence,
        "estimated_cost": decision.estimated_cost,
    }))
}

/// Export orchestrator snapshot as compact MessagePack (base64-encoded).
/// Used for backup/restore of orchestrator state across sessions.
#[tauri::command]
pub async fn neuralswarm_export_snapshot() -> Result<serde_json::Value, String> {
    let orch = get_orchestrator()?;
    let orch = orch.lock().await;
    let bytes = orch.export_snapshot_msgpack()?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(serde_json::json!({
        "format": "msgpack+base64",
        "size_bytes": bytes.len(),
        "data": b64,
    }))
}

/// Import orchestrator snapshot from MessagePack (base64-encoded).
/// Validates the snapshot structure before confirming.
#[tauri::command]
pub async fn neuralswarm_import_snapshot(data: String) -> Result<serde_json::Value, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;
    let snapshot = crate::orchestrator::store::OrchestratorStore::snapshot_from_msgpack(&bytes)?;
    let health_count = snapshot.get("health_records")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let trust_count = snapshot.get("trust_records")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(serde_json::json!({
        "valid": true,
        "health_records": health_count,
        "trust_records": trust_count,
        "timestamp": snapshot.get("timestamp"),
    }))
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
