//! ImpForge Standalone Orchestrator — Main Module
//!
//! Rust-native AI orchestrator for the ImpForge commercial product.
//! 100% standalone — no systemd, no PostgreSQL, no Redis.
//!
//! Architecture:
//!   - tokio background tasks for scheduling
//!   - SQLite (rusqlite bundled) for persistence
//!   - In-memory event bus for real-time updates
//!   - Hebbian/STDP trust scoring per worker
//!   - MAPE-K self-healing health loop
//!   - Brain v2.0 (FSRS, CLS, Zettelkasten, TeleMem)
//!
//! This module is completely independent of ORK-Station infrastructure.

pub mod brain;
pub mod events;
pub mod health;
pub mod store;
pub mod trust;
pub mod workers;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use self::events::{EventBus, OrchestratorEvent};
use self::health::MapeKLoop;
use self::store::OrchestratorStore;
use self::trust::HebbianTrustManager;
use self::workers::{TaskWorker, WorkerContext, WorkerStatus};

/// Task schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSchedule {
    pub name: String,
    pub interval_secs: u64,
    pub pool: String,
    pub enabled: bool,
    pub trigger: Option<String>,
}

/// Orchestrator status (exposed to frontend via Tauri commands)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    pub running: bool,
    pub task_count: usize,
    pub tasks_ok: usize,
    pub tasks_fail: usize,
    pub uptime_seconds: u64,
    pub avg_trust: f64,
}

/// Task status for frontend display
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

/// Full snapshot for efficient single-call UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorSnapshot {
    pub status: OrchestratorStatus,
    pub tasks: Vec<TaskStatus>,
    pub services: Vec<health::ServiceState>,
    pub recent_events: Vec<events::OrchestratorEvent>,
    pub trust_scores: Vec<trust::WorkerTrust>,
}

/// The main ImpForge Orchestrator
pub struct ImpForgeOrchestrator {
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<DateTime<Utc>>>>,
    store: Arc<OrchestratorStore>,
    trust_manager: Arc<RwLock<HebbianTrustManager>>,
    health_loop: Arc<RwLock<MapeKLoop>>,
    event_bus: Arc<EventBus>,
    workers: Arc<HashMap<String, Box<dyn TaskWorker>>>,
    schedules: Arc<Vec<TaskSchedule>>,
    worker_context: Arc<WorkerContext>,
    last_run: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    task_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl ImpForgeOrchestrator {
    /// Create a new orchestrator with default configuration
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        let db_path = data_dir.join("impforge_orchestrator.db");
        let store = OrchestratorStore::open(&db_path)
            .map_err(|e| format!("Failed to open database: {e}"))?;
        let store_arc = Arc::new(store);

        let mut health_loop = MapeKLoop::new();
        health_loop.register_defaults();

        let all_workers = workers::create_all_workers();
        let mut worker_map: HashMap<String, Box<dyn TaskWorker>> = HashMap::new();
        for w in all_workers {
            worker_map.insert(w.name().to_string(), w);
        }

        // Build schedules from worker definitions (matching Python config)
        let schedules = build_default_schedules();

        // Load trust scores from DB
        let mut trust_manager = HebbianTrustManager::new();
        if let Ok(records) = store_arc.get_all_trust() {
            let trust_data: Vec<_> = records.iter()
                .map(|r| (r.worker_name.clone(), r.score, r.successes, r.failures))
                .collect();
            trust_manager.load_from_records(trust_data);
        }

        Ok(Self {
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            store: Arc::clone(&store_arc),
            trust_manager: Arc::new(RwLock::new(trust_manager)),
            health_loop: Arc::new(RwLock::new(health_loop)),
            event_bus: Arc::new(EventBus::new()),
            workers: Arc::new(worker_map),
            schedules: Arc::new(schedules),
            worker_context: Arc::new(WorkerContext {
                ollama_url: std::env::var("OLLAMA_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string()),
                data_dir,
                store: Some(Arc::clone(&store_arc)),
            }),
            last_run: Arc::new(RwLock::new(HashMap::new())),
            task_handles: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Start the orchestrator (spawns background tasks)
    pub async fn start(&self) -> Result<(), String> {
        {
            let is_running = *self.running.read().await;
            if is_running {
                return Err("Orchestrator is already running".to_string());
            }
        }

        *self.running.write().await = true;
        *self.started_at.write().await = Some(Utc::now());

        log::info!("ImpForge Orchestrator starting with {} workers", self.workers.len());

        // Spawn the main scheduler loop
        let running = self.running.clone();
        let workers = self.workers.clone();
        let schedules = self.schedules.clone();
        let ctx = self.worker_context.clone();
        let trust = self.trust_manager.clone();
        let store = self.store.clone();
        let bus = self.event_bus.clone();
        let last_run = self.last_run.clone();

        let scheduler_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

            loop {
                interval.tick().await;

                if !*running.read().await {
                    break;
                }

                let now = Utc::now();
                let mut runs = last_run.write().await;

                for schedule in schedules.iter() {
                    if !schedule.enabled || schedule.interval_secs == 0 {
                        continue;
                    }

                    // Check if enough time has passed
                    let should_run = match runs.get(&schedule.name) {
                        Some(last) => {
                            (now - *last).num_seconds() as u64 >= schedule.interval_secs
                        }
                        None => true,
                    };

                    if !should_run {
                        continue;
                    }

                    // Trust gate: skip workers with very low trust
                    let worker_trust = trust.read().await.get_trust(&schedule.name);
                    if worker_trust < 0.15 {
                        log::debug!("Skipping {} (trust {:.2} < 0.15)", schedule.name, worker_trust);
                        continue;
                    }

                    // Execute the worker
                    if let Some(worker) = workers.get(&schedule.name) {
                        let start = std::time::Instant::now();
                        let result = worker.run(&ctx).await;
                        let duration_ms = start.elapsed().as_millis() as u64;

                        // Update trust based on result
                        match result.status {
                            WorkerStatus::Ok => {
                                trust.write().await.record_success(&schedule.name, duration_ms);
                            }
                            WorkerStatus::Error => {
                                trust.write().await.record_failure(&schedule.name);
                            }
                            _ => {}
                        }

                        // Log to SQLite
                        let status_str = match result.status {
                            WorkerStatus::Ok => "ok",
                            WorkerStatus::Warning => "warning",
                            WorkerStatus::Error => "error",
                            WorkerStatus::Skipped => "skipped",
                        };
                        let _ = store.log_task(
                            &schedule.name,
                            status_str,
                            duration_ms,
                            Some(&result.message),
                            None,
                        );

                        // Publish event
                        bus.publish(OrchestratorEvent::task_completed(
                            &schedule.name,
                            result.status == WorkerStatus::Ok,
                            duration_ms,
                        ));

                        // Persist trust to SQLite
                        let wt = trust.read().await.get_worker_trust(&schedule.name);
                        let _ = store.set_trust(&schedule.name, wt.score, wt.successes, wt.failures);

                        runs.insert(schedule.name.clone(), now);
                    }
                }
            }

            log::info!("ImpForge Orchestrator scheduler stopped");
        });

        // Spawn the MAPE-K health loop
        let running_h = self.running.clone();
        let health = self.health_loop.clone();
        let bus_h = self.event_bus.clone();

        let health_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if !*running_h.read().await {
                    break;
                }

                let mut hl = health.write().await;
                hl.run_cycle().await;

                // Publish health events for services that are down
                for state in hl.get_summary() {
                    if state.status == health::ServiceStatus::Offline {
                        bus_h.publish(OrchestratorEvent::service_down(&state.service.name));
                    }
                }
            }

            log::info!("MAPE-K health loop stopped");
        });

        let mut handles = self.task_handles.write().await;
        handles.push(scheduler_handle);
        handles.push(health_handle);

        // Log startup event
        self.event_bus.publish(OrchestratorEvent {
            event_type: events::EventType::OrchestratorAction,
            source: "orchestrator".to_string(),
            payload: serde_json::json!({"action": "started", "workers": self.workers.len()}),
            timestamp: Utc::now(),
        });

        log::info!("ImpForge Orchestrator started successfully");
        Ok(())
    }

    /// Stop the orchestrator gracefully
    pub async fn stop(&self) -> Result<(), String> {
        *self.running.write().await = false;

        // Abort task handles
        let mut handles = self.task_handles.write().await;
        for handle in handles.drain(..) {
            handle.abort();
        }

        self.event_bus.publish(OrchestratorEvent {
            event_type: events::EventType::OrchestratorAction,
            source: "orchestrator".to_string(),
            payload: serde_json::json!({"action": "stopped"}),
            timestamp: Utc::now(),
        });

        log::info!("ImpForge Orchestrator stopped");
        Ok(())
    }

    /// Get orchestrator status
    pub async fn status(&self) -> OrchestratorStatus {
        let running = *self.running.read().await;
        let started = *self.started_at.read().await;

        let uptime = match started {
            Some(s) if running => (Utc::now() - s).num_seconds() as u64,
            _ => 0,
        };

        let (ok, fail) = self.count_results().await;

        OrchestratorStatus {
            running,
            task_count: self.workers.len(),
            tasks_ok: ok,
            tasks_fail: fail,
            uptime_seconds: uptime,
            avg_trust: self.trust_manager.read().await.average_trust(),
        }
    }

    /// Get all task statuses
    pub async fn task_statuses(&self) -> Vec<TaskStatus> {
        let trust = self.trust_manager.read().await;
        let last_run = self.last_run.read().await;

        self.schedules
            .iter()
            .map(|s| {
                let worker_trust = trust.get_trust(&s.name);
                let description = self.workers
                    .get(&s.name)
                    .map(|w| w.description().to_string())
                    .unwrap_or_default();

                TaskStatus {
                    name: s.name.clone(),
                    description,
                    status: if *self.running.blocking_read() && s.enabled {
                        "active".to_string()
                    } else {
                        "idle".to_string()
                    },
                    duration_ms: None,
                    trust: worker_trust,
                    last_run: last_run.get(&s.name).map(|t| t.to_rfc3339()),
                    pool: s.pool.clone(),
                    enabled: s.enabled,
                }
            })
            .collect()
    }

    /// Get a full snapshot for the frontend
    pub async fn snapshot(&self) -> OrchestratorSnapshot {
        let status = self.status().await;
        let tasks = self.task_statuses().await;
        let services = self.health_loop.read().await.get_summary();
        let recent_events = self.event_bus.recent(50);
        let trust_scores = self.trust_manager.read().await.get_all_trust();

        OrchestratorSnapshot {
            status,
            tasks,
            services,
            recent_events,
            trust_scores,
        }
    }

    /// Get recent logs from the store
    pub async fn recent_logs(&self, limit: u32) -> Vec<store::TaskLog> {
        self.store.get_recent_logs(limit).unwrap_or_default()
    }

    async fn count_results(&self) -> (usize, usize) {
        let events = self.event_bus.by_type(&events::EventType::TaskCompleted, 10000);
        let ok = events.iter().filter(|e| e.payload["success"] == true).count();
        let fail = events.iter().filter(|e| e.payload["success"] == false).count();
        (ok, fail)
    }
}

/// Build default task schedules matching the Python NeuralSwarm config
fn build_default_schedules() -> Vec<TaskSchedule> {
    vec![
        // Tier 1: Core Automation
        TaskSchedule { name: "mcp_watchdog".into(), interval_secs: 60, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "vram_manager".into(), interval_secs: 30, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "log_analyzer".into(), interval_secs: 900, pool: "gpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "anomaly_detector".into(), interval_secs: 900, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "terminal_digester".into(), interval_secs: 60, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "model_health".into(), interval_secs: 300, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "dependency_auditor".into(), interval_secs: 21600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "doc_sync".into(), interval_secs: 0, pool: "cpu".into(), enabled: true, trigger: Some("file:changed(.md)".into()) },
        TaskSchedule { name: "test_runner".into(), interval_secs: 0, pool: "shell".into(), enabled: true, trigger: Some("file:changed(.rs,.ts)".into()) },
        TaskSchedule { name: "kg_enricher".into(), interval_secs: 1800, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "backup_agent".into(), interval_secs: 43200, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "code_quality".into(), interval_secs: 7200, pool: "gpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "release_builder".into(), interval_secs: 0, pool: "shell".into(), enabled: true, trigger: Some("tag:release".into()) },
        // Tier 2: Self-Healing & Intelligence
        TaskSchedule { name: "self_healer".into(), interval_secs: 0, pool: "cpu".into(), enabled: true, trigger: Some("service:down".into()) },
        TaskSchedule { name: "semantic_diff".into(), interval_secs: 0, pool: "gpu".into(), enabled: true, trigger: Some("file:changed".into()) },
        TaskSchedule { name: "config_drift".into(), interval_secs: 3600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "perf_tracker".into(), interval_secs: 1800, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "security_sentinel".into(), interval_secs: 0, pool: "gpu".into(), enabled: true, trigger: Some("file:changed".into()) },
        TaskSchedule { name: "trust_scorer".into(), interval_secs: 21600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "dead_code".into(), interval_secs: 43200, pool: "gpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "cross_repo".into(), interval_secs: 7200, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "cache_pruner".into(), interval_secs: 21600, pool: "shell".into(), enabled: true, trigger: None },
        // Tier 3: Advanced Automation
        TaskSchedule { name: "changelog_gen".into(), interval_secs: 0, pool: "cpu".into(), enabled: true, trigger: Some("tag:release".into()) },
        TaskSchedule { name: "api_validator".into(), interval_secs: 0, pool: "gpu".into(), enabled: true, trigger: Some("file:changed(.rs,.ts)".into()) },
        TaskSchedule { name: "resource_forecast".into(), interval_secs: 3600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "migration_planner".into(), interval_secs: 86400, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "stale_cleaner".into(), interval_secs: 43200, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "embedding_refresh".into(), interval_secs: 0, pool: "embed".into(), enabled: true, trigger: Some("file:changed(.md,.py,.rs)".into()) },
        TaskSchedule { name: "service_mapper".into(), interval_secs: 86400, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "commit_gate".into(), interval_secs: 0, pool: "cpu".into(), enabled: true, trigger: Some("commit:ready".into()) },
        TaskSchedule { name: "system_snapshot".into(), interval_secs: 21600, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "dedup_sweeper".into(), interval_secs: 3600, pool: "cpu".into(), enabled: true, trigger: None },
        // Brain v2.0
        TaskSchedule { name: "memory_decay_scorer".into(), interval_secs: 1800, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "cls_replay".into(), interval_secs: 1800, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "auto_labeler".into(), interval_secs: 3600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "context_enricher".into(), interval_secs: 0, pool: "cpu".into(), enabled: true, trigger: Some("file:changed".into()) },
        TaskSchedule { name: "kg_temporal_updater".into(), interval_secs: 7200, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "digest_processor".into(), interval_secs: 900, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "rlm_session_manager".into(), interval_secs: 300, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "context_cache_warmer".into(), interval_secs: 1800, pool: "shell".into(), enabled: true, trigger: None },
        TaskSchedule { name: "zettelkasten_indexer".into(), interval_secs: 3600, pool: "cpu".into(), enabled: true, trigger: None },
        TaskSchedule { name: "telemem_pipeline".into(), interval_secs: 1800, pool: "cpu".into(), enabled: true, trigger: None },
    ]
}
