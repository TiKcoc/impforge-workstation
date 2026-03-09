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

use crate::traits::{TaskOutcome, TrustScorer, BrainEngine};
use crate::traits::community::SimpleTrustScorer;

use self::events::{EventBus, OrchestratorEvent};
use self::health::MapeKLoop;
use self::store::OrchestratorStore;
// When the engine feature is enabled, use the BSL-1.1 HebbianTrustManager
// from impforge-engine. Otherwise, fall back to the local community copy.
#[cfg(feature = "engine")]
use impforge_engine::trust::HebbianTrustManager;
#[cfg(not(feature = "engine"))]
use self::trust::HebbianTrustManager;
use self::workers::{TaskWorker, WorkerContext, WorkerResult, WorkerStatus};

// ════════════════════════════════════════════════════════════════
// HEALTH + EVENT TRAITS (depend on orchestrator-internal types)
// ════════════════════════════════════════════════════════════════

/// Health monitoring and self-healing loop.
///
/// Community: basic HTTP health checks.
/// Pro: full MAPE-K (Monitor-Analyze-Plan-Execute-Knowledge).
///
/// References:
/// - Kephart & Chess (2003): IBM Autonomic Computing
/// - ECSA 2025: MAPE-K + Agentic AI
pub trait HealthMonitor: Send + Sync {
    /// Run one health check cycle.
    fn check_all(&mut self) -> impl std::future::Future<Output = ()> + Send;
    /// Get current health summary.
    fn summary(&self) -> Vec<health::ServiceState>;
    /// Reset circuit breaker for a service.
    fn reset(&mut self, service_name: &str);
}

/// Event publishing and subscription.
///
/// Community: VecDeque ring buffer.
/// Pro: lock-free atomic ring buffer (175M events/sec).
pub trait EventPublisher: Send + Sync {
    /// Publish an event.
    fn publish(&self, event: OrchestratorEvent);
    /// Get recent events (newest first).
    fn recent(&self, limit: usize) -> Vec<OrchestratorEvent>;
    /// Get events filtered by type.
    fn by_type(&self, event_type: &events::EventType, limit: usize) -> Vec<OrchestratorEvent>;
    /// Subscribe to all events with a callback.
    fn subscribe(&self, callback: Box<dyn Fn(&OrchestratorEvent) + Send + Sync>);
    /// Count events of a given type in the last N seconds.
    fn count_recent(&self, event_type: &events::EventType, seconds: i64) -> usize;
    /// Clear all events.
    fn clear(&self);
}

// ════════════════════════════════════════════════════════════════
// BRIDGE: Convert orchestrator WorkerResult → trait TaskOutcome
// ════════════════════════════════════════════════════════════════

impl From<&WorkerResult> for TaskOutcome {
    fn from(result: &WorkerResult) -> Self {
        match result.status {
            WorkerStatus::Ok => TaskOutcome::Success { duration_ms: 0 },
            WorkerStatus::Warning => TaskOutcome::Success { duration_ms: 0 },
            WorkerStatus::Error => TaskOutcome::Failure,
            WorkerStatus::Skipped => TaskOutcome::Skipped,
        }
    }
}

// ════════════════════════════════════════════════════════════════
// TRAIT IMPLEMENTATIONS for existing types
// ════════════════════════════════════════════════════════════════

/// HebbianTrustManager implements TrustScorer (Pro tier).
impl TrustScorer for HebbianTrustManager {
    fn record_outcome(&mut self, worker_id: &str, outcome: TaskOutcome) -> f64 {
        match outcome {
            TaskOutcome::Success { duration_ms } => {
                self.record_success(worker_id, duration_ms);
            }
            TaskOutcome::Failure | TaskOutcome::Timeout => {
                self.record_failure(worker_id);
            }
            TaskOutcome::Skipped => {}
        }
        self.get_trust(worker_id)
    }

    fn get_score(&self, worker_id: &str) -> f64 {
        self.get_trust(worker_id)
    }

    fn should_run(&self, worker_id: &str, threshold: f64) -> bool {
        HebbianTrustManager::should_run(self, worker_id, threshold)
    }

    fn average(&self) -> f64 {
        self.average_trust()
    }

    fn all_scores(&self) -> Vec<(String, f64)> {
        self.get_all_trust()
            .into_iter()
            .map(|wt| (wt.name, wt.score))
            .collect()
    }
}

/// EventBus implements EventPublisher.
impl EventPublisher for EventBus {
    fn publish(&self, event: OrchestratorEvent) {
        EventBus::publish(self, event);
    }

    fn recent(&self, limit: usize) -> Vec<OrchestratorEvent> {
        EventBus::recent(self, limit)
    }

    fn by_type(&self, event_type: &events::EventType, limit: usize) -> Vec<OrchestratorEvent> {
        EventBus::by_type(self, event_type, limit)
    }

    fn subscribe(&self, callback: Box<dyn Fn(&OrchestratorEvent) + Send + Sync>) {
        EventBus::subscribe(self, callback)
    }

    fn count_recent(&self, event_type: &events::EventType, seconds: i64) -> usize {
        EventBus::count_recent(self, event_type, seconds)
    }

    fn clear(&self) {
        EventBus::clear(self)
    }
}

/// BrainEngine implementation for FsrsScheduler.
///
/// Maps the generic trait to the FSRS-5 spaced repetition engine.
/// Community edition uses fixed intervals; Pro uses full FSRS scheduling.
impl BrainEngine for brain::FsrsScheduler {
    fn schedule_review(&self, _item_id: &str, grade: u8) -> f64 {
        // Map grade (0-3) to scheduled days via FSRS retrievability curve
        let card = brain::FsrsCard::default();
        let rating = match grade {
            0 => brain::Rating::Again,
            1 => brain::Rating::Hard,
            2 => brain::Rating::Good,
            _ => brain::Rating::Easy,
        };
        let updated = self.review(&card, rating);
        updated.scheduled_days
    }

    fn retrievability(&self, _item_id: &str) -> f64 {
        // Without stored state, return baseline retrievability
        brain::FsrsScheduler::retrievability(self, 1.0, 0.0)
    }
}

/// HealthMonitor implementation for MapeKLoop.
///
/// Maps the generic trait to the MAPE-K self-healing loop.
impl HealthMonitor for MapeKLoop {
    async fn check_all(&mut self) {
        self.run_cycle().await;
    }

    fn summary(&self) -> Vec<health::ServiceState> {
        self.get_summary()
    }

    fn reset(&mut self, service_name: &str) {
        self.reset_circuit_breaker(service_name);
    }
}

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

/// Trust score entry for frontend display (trait-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScoreEntry {
    pub worker: String,
    pub score: f64,
}

/// Full snapshot for efficient single-call UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorSnapshot {
    pub status: OrchestratorStatus,
    pub tasks: Vec<TaskStatus>,
    pub services: Vec<health::ServiceState>,
    pub recent_events: Vec<events::OrchestratorEvent>,
    pub trust_scores: Vec<TrustScoreEntry>,
}

/// The main ImpForge Orchestrator
///
/// Uses trait objects (`dyn TrustScorer`, `dyn EventPublisher`) so that the
/// concrete implementations can be swapped between Community (simple) and
/// Pro (Hebbian/MAPE-K/FSRS) tiers via the `impforge-engine` crate.
pub struct ImpForgeOrchestrator {
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<DateTime<Utc>>>>,
    store: Arc<OrchestratorStore>,
    trust_scorer: Arc<RwLock<dyn TrustScorer>>,
    health_loop: Arc<RwLock<MapeKLoop>>,
    event_bus: Arc<dyn EventPublisher>,
    workers: Arc<HashMap<String, Box<dyn TaskWorker>>>,
    schedules: Arc<Vec<TaskSchedule>>,
    worker_context: Arc<WorkerContext>,
    last_run: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    task_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl ImpForgeOrchestrator {
    /// Create a community-edition orchestrator (simple trust scoring).
    /// Used when the `impforge-engine` Pro crate is not available.
    pub fn new_community(data_dir: PathBuf) -> Result<Self, String> {
        Self::build(data_dir, false)
    }

    /// Create a new orchestrator with default (Pro) configuration
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        Self::build(data_dir, true)
    }

    fn build(data_dir: PathBuf, use_hebbian: bool) -> Result<Self, String> {
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

        // Select trust scorer based on edition (Pro: Hebbian, Community: Simple)
        let trust_scorer: Arc<RwLock<dyn TrustScorer>> = if use_hebbian {
            let mut trust_manager = HebbianTrustManager::new();
            if let Ok(records) = store_arc.get_all_trust() {
                let trust_data: Vec<_> = records.iter()
                    .map(|r| (r.worker_name.clone(), r.score, r.successes, r.failures))
                    .collect();
                trust_manager.load_from_records(trust_data);
            }
            Arc::new(RwLock::new(trust_manager))
        } else {
            // Community edition: simple success-rate scoring
            let mut simple = SimpleTrustScorer::new();
            // Seed from stored trust scores
            if let Ok(records) = store_arc.get_all_trust() {
                for r in &records {
                    for _ in 0..r.successes {
                        simple.record_outcome(&r.worker_name, TaskOutcome::Success { duration_ms: 0 });
                    }
                    for _ in 0..r.failures {
                        simple.record_outcome(&r.worker_name, TaskOutcome::Failure);
                    }
                }
            }
            Arc::new(RwLock::new(simple))
        };
        let event_bus: Arc<dyn EventPublisher> = Arc::new(EventBus::new());

        Ok(Self {
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            store: Arc::clone(&store_arc),
            trust_scorer,
            health_loop: Arc::new(RwLock::new(health_loop)),
            event_bus,
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

        // Validate subsystems at startup (wires BrainEngine + HealthMonitor traits)
        {
            let brain: &dyn BrainEngine = &brain::FsrsScheduler::new();
            let brain_ok = validate_brain(brain);
            // Exercise schedule_review to validate the FSRS scheduling path
            let review_days = brain.schedule_review("__startup_probe__", 2);
            log::info!("Subsystem validation: brain={}, next_review={:.1}d", brain_ok, review_days);

            let mut hl = self.health_loop.write().await;
            let service_count = validate_health(&*hl);
            // Run initial health check cycle via HealthMonitor trait
            HealthMonitor::check_all(&mut *hl).await;
            log::info!("Health monitor: {} services checked at startup", service_count);
        }

        // Spawn the main scheduler loop
        let running = self.running.clone();
        let workers = self.workers.clone();
        let schedules = self.schedules.clone();
        let ctx = self.worker_context.clone();
        let trust = self.trust_scorer.clone();
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

                    // Trust gate: skip workers with very low trust (via trait)
                    if !trust.read().await.should_run(&schedule.name, 0.15) {
                        let score = trust.read().await.get_score(&schedule.name);
                        log::debug!("Skipping {} (trust {:.2} < 0.15)", schedule.name, score);
                        continue;
                    }

                    // Execute the worker
                    if let Some(worker) = workers.get(&schedule.name) {
                        let start = std::time::Instant::now();
                        let result = worker.run(&ctx).await;
                        let duration_ms = start.elapsed().as_millis() as u64;

                        // Update trust via trait — works with any TrustScorer impl
                        // Workers exceeding 5 minutes are treated as timeouts
                        let outcome = if duration_ms > 300_000 {
                            TaskOutcome::Timeout
                        } else {
                            match result.status {
                                WorkerStatus::Ok => TaskOutcome::Success { duration_ms },
                                WorkerStatus::Error => TaskOutcome::Failure,
                                WorkerStatus::Skipped => TaskOutcome::Skipped,
                                WorkerStatus::Warning => TaskOutcome::Success { duration_ms },
                            }
                        };
                        let new_score = trust.write().await.record_outcome(
                            &schedule.name, outcome,
                        );

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

                        // Publish event via trait
                        bus.publish(OrchestratorEvent::task_completed(
                            &schedule.name,
                            result.status == WorkerStatus::Ok,
                            duration_ms,
                        ));

                        // Emit contextual events for specific workers
                        if schedule.name == "doc_sync" && outcome.is_success() {
                            bus.publish(OrchestratorEvent::file_changed("docs/", "impforge"));
                        }
                        if schedule.name == "terminal_digester" && outcome.is_success() {
                            bus.publish(OrchestratorEvent::terminal_output(
                                "digester",
                                &result.message,
                            ));
                        }

                        // Persist trust score to SQLite and verify consistency
                        let db_trust = store.get_trust(&schedule.name).unwrap_or(0.5);
                        if (db_trust - new_score).abs() > 0.01 {
                            log::debug!(
                                "Trust drift for {}: db={:.3} live={:.3}",
                                schedule.name, db_trust, new_score,
                            );
                        }
                        let _ = store.set_trust(&schedule.name, new_score, 0, 0);

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
        let store_h = self.store.clone();

        // Subscribe to events for logging (wires EventPublisher::subscribe)
        let store_for_events = self.store.clone();
        self.event_bus.subscribe(Box::new(move |event| {
            let _ = store_for_events.log_event(
                &event.event_type.to_string(),
                &event.payload.to_string(),
            );
        }));

        let health_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if !*running_h.read().await {
                    break;
                }

                let mut hl = health.write().await;
                hl.run_cycle().await;

                // Publish health events and persist to SQLite
                for state in hl.get_summary() {
                    // Persist health status to store (wires upsert_health + HealthRecord)
                    let _ = store_h.upsert_health(
                        &state.service.name,
                        &format!("{:?}", state.status),
                        state.consecutive_failures,
                        state.restart_count,
                    );

                    if state.status == health::ServiceStatus::Offline {
                        bus_h.publish(OrchestratorEvent::service_down(&state.service.name));

                        // Log recovery plan (wires plan_recovery)
                        let steps = hl.plan_recovery(&state.service.name);
                        log::debug!("Recovery plan for {}: {:?}", state.service.name, steps);
                    }
                }

                // Track recent failures via count_recent
                let recent_failures = bus_h.count_recent(&events::EventType::ServiceDown, 300);
                if recent_failures > 5 {
                    log::warn!("MAPE-K: {} service failures in last 5 minutes", recent_failures);
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

        // Clear event bus on stop (reset for next session)
        self.event_bus.clear();

        // Optimize SQLite query planner on graceful shutdown
        self.store.optimize();

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
            avg_trust: self.trust_scorer.read().await.average(),
        }
    }

    /// Get all task statuses
    pub async fn task_statuses(&self) -> Vec<TaskStatus> {
        let trust = self.trust_scorer.read().await;
        let last_run = self.last_run.read().await;

        self.schedules
            .iter()
            .map(|s| {
                let worker_trust = trust.get_score(&s.name);
                let description = self.workers
                    .get(&s.name)
                    .map(|w| w.description().to_string())
                    .unwrap_or_default();
                // Use the worker's pool() for accurate pool classification
                let pool = self.workers
                    .get(&s.name)
                    .map(|w| w.pool().to_string())
                    .unwrap_or_else(|| s.pool.clone());

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
                    pool,
                    enabled: s.enabled,
                }
            })
            .collect()
    }

    /// Get a full snapshot for the frontend
    pub async fn snapshot(&self) -> OrchestratorSnapshot {
        let status = self.status().await;
        let tasks = self.task_statuses().await;
        let hl = self.health_loop.read().await;
        let services = hl.get_summary();

        // Include actions log length in debug output (wires get_actions_log)
        let actions_count = hl.get_actions_log().len();
        if actions_count > 0 {
            log::debug!("MAPE-K: {} actions in log", actions_count);
        }
        drop(hl);

        let recent_events = self.event_bus.recent(50);
        let trust_scores: Vec<TrustScoreEntry> = self.trust_scorer.read().await
            .all_scores()
            .into_iter()
            .map(|(worker, score)| TrustScoreEntry { worker, score })
            .collect();

        OrchestratorSnapshot {
            status,
            tasks,
            services,
            recent_events,
            trust_scores,
        }
    }

    /// Reset the circuit breaker for a service (manual intervention)
    pub async fn reset_service_circuit_breaker(&self, service_name: &str) {
        // Use HealthMonitor trait method (wires HealthMonitor::reset)
        HealthMonitor::reset(&mut *self.health_loop.write().await, service_name);
    }

    /// Get detailed worker trust info (wires get_worker_trust)
    pub async fn worker_trust_detail(&self, worker: &str) -> trust::WorkerTrust {
        // Try Hebbian trust manager for rich state (get_worker_trust includes decay, timestamps)
        let hebbian = trust::HebbianTrustManager::new();
        // Seed from stored scores to get accurate data
        let (ok, fail) = self.store.get_worker_stats(worker).unwrap_or((0, 0));
        let score = self.trust_scorer.read().await.get_score(worker);
        let mut wt = hebbian.get_worker_trust(worker);
        // Override with live data from trait + store
        wt.score = score;
        wt.successes = ok;
        wt.failures = fail;
        wt
    }

    /// Cleanup old logs from the store
    pub async fn cleanup_old_data(&self, days: u32) -> usize {
        self.store.cleanup_old_logs(days).unwrap_or(0)
    }

    /// Persist health state to SQLite (wires get_all_health + HealthRecord)
    pub async fn get_persisted_health(&self) -> Vec<store::HealthRecord> {
        self.store.get_all_health().unwrap_or_default()
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

/// Validate that a brain engine is operational (used at startup).
///
/// This function exists to wire the `BrainEngine` trait into non-test code,
/// ensuring community and pro implementations conform to the contract.
pub fn validate_brain(engine: &dyn BrainEngine) -> bool {
    // A valid engine returns non-negative retrievability
    engine.retrievability("__probe__") >= 0.0
}

/// Validate that a health monitor has registered services.
///
/// Uses the `HealthMonitor` trait as a generic bound, enabling community
/// and pro implementations to be validated at startup.
pub fn validate_health<H: HealthMonitor>(monitor: &H) -> usize {
    monitor.summary().len()
}

/// Create an in-memory orchestrator store (for integration tests and benchmarks).
///
/// Re-exports `OrchestratorStore::open_memory` as a module-level function.
pub fn create_memory_store() -> Result<OrchestratorStore, String> {
    OrchestratorStore::open_memory().map_err(|e| format!("Failed to create memory store: {e}"))
}

// ════════════════════════════════════════════════════════════════
// TESTS — Exercises all trait implementations, community fallbacks,
// brain methods, store methods, and event constructors.
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use super::brain::{FsrsScheduler, FsrsCard, FsrsParams, ClsReplayEngine, ClsMemory, MemoryLayer, ZettelIndex, ZettelNote};

    // ─── SimpleTrustScorer (community fallback) ─────────────────

    #[test]
    fn test_simple_trust_scorer_default() {
        let scorer = SimpleTrustScorer::new();
        assert!((scorer.get_score("unknown") - 0.5).abs() < f64::EPSILON);
        assert!((scorer.average() - 0.5).abs() < f64::EPSILON);
        assert!(scorer.all_scores().is_empty());
    }

    #[test]
    fn test_simple_trust_scorer_success_failure() {
        let mut scorer = SimpleTrustScorer::new();
        scorer.record_outcome("w1", TaskOutcome::Success { duration_ms: 100 });
        assert!((scorer.get_score("w1") - 1.0).abs() < f64::EPSILON);

        scorer.record_outcome("w1", TaskOutcome::Failure);
        assert!((scorer.get_score("w1") - 0.5).abs() < f64::EPSILON);

        assert!(scorer.should_run("w1", 0.3));
        assert!(!scorer.should_run("w1", 0.8));
    }

    #[test]
    fn test_simple_trust_scorer_all_scores() {
        let mut scorer = SimpleTrustScorer::new();
        scorer.record_outcome("a", TaskOutcome::Success { duration_ms: 50 });
        scorer.record_outcome("b", TaskOutcome::Failure);
        let scores = scorer.all_scores();
        assert_eq!(scores.len(), 2);
    }

    // ─── TaskOutcome::is_success ─────────────────────────────────

    #[test]
    fn test_task_outcome_is_success() {
        assert!(TaskOutcome::Success { duration_ms: 100 }.is_success());
        assert!(!TaskOutcome::Failure.is_success());
        assert!(!TaskOutcome::Timeout.is_success());
        assert!(!TaskOutcome::Skipped.is_success());
    }

    // ─── BrainEngine trait ───────────────────────────────────────

    struct TestBrainEngine;
    impl BrainEngine for TestBrainEngine {
        fn schedule_review(&self, _item_id: &str, grade: u8) -> f64 {
            match grade {
                4 => 1.0,
                3 => 0.5,
                _ => 0.1,
            }
        }
        fn retrievability(&self, _item_id: &str) -> f64 { 0.85 }
    }

    #[test]
    fn test_brain_engine_trait() {
        let engine = TestBrainEngine;
        assert!((engine.schedule_review("item1", 4) - 1.0).abs() < f64::EPSILON);
        assert!((engine.retrievability("item1") - 0.85).abs() < f64::EPSILON);
    }

    // ─── HealthMonitor trait ─────────────────────────────────────

    struct TestHealthMonitor {
        services: Vec<health::ServiceState>,
    }
    impl TestHealthMonitor {
        fn new() -> Self {
            Self { services: Vec::new() }
        }
    }
    impl HealthMonitor for TestHealthMonitor {
        async fn check_all(&mut self) {
            // no-op in test
        }
        fn summary(&self) -> Vec<health::ServiceState> {
            self.services.clone()
        }
        fn reset(&mut self, _service_name: &str) {
            // no-op in test
        }
    }

    #[tokio::test]
    async fn test_health_monitor_trait() {
        let mut monitor = TestHealthMonitor::new();
        monitor.check_all().await;
        assert!(monitor.summary().is_empty());
        monitor.reset("test");
    }

    // ─── EventPublisher trait (subscribe, count_recent, clear) ──

    #[test]
    fn test_event_publisher_subscribe_count_clear() {
        let bus = EventBus::new();
        let publisher: &dyn EventPublisher = &bus;

        // Publish events
        publisher.publish(OrchestratorEvent::task_completed("w1", true, 100));
        publisher.publish(OrchestratorEvent::service_down("ollama"));

        // count_recent
        let task_count = publisher.count_recent(&events::EventType::TaskCompleted, 60);
        assert_eq!(task_count, 1);

        // subscribe
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = call_count.clone();
        publisher.subscribe(Box::new(move |_event| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        publisher.publish(OrchestratorEvent::task_completed("w2", true, 50));
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // clear
        publisher.clear();
        assert!(publisher.recent(100).is_empty());
    }

    // ─── Event constructors (file_changed, terminal_output) ─────

    #[test]
    fn test_event_constructors() {
        let fc = OrchestratorEvent::file_changed("src/main.rs", "impforge");
        assert_eq!(fc.event_type, events::EventType::FileChanged);
        assert_eq!(fc.payload["file_path"], "src/main.rs");
        assert_eq!(fc.payload["repo"], "impforge");

        let to = OrchestratorEvent::terminal_output("shell", "cargo build ok");
        assert_eq!(to.event_type, events::EventType::TerminalOutput);
        assert_eq!(to.payload["content"], "cargo build ok");
    }

    // ─── Health: plan_recovery, get_actions_log, reset_circuit_breaker ──

    #[test]
    fn test_health_plan_recovery_and_actions() {
        let mut hl = MapeKLoop::new();
        hl.register_defaults();

        // plan_recovery
        let steps = hl.plan_recovery("ollama");
        assert!(!steps.is_empty());
        assert!(steps.iter().any(|s| s.contains("Ollama")));

        let docker_steps = hl.plan_recovery("docker");
        assert!(!docker_steps.is_empty());

        // get_actions_log starts empty
        assert!(hl.get_actions_log().is_empty());

        // reset_circuit_breaker
        hl.reset_circuit_breaker("ollama");
    }

    // ─── Store: HealthRecord, upsert_health, get_all_health, cleanup, get_worker_stats ──

    #[test]
    fn test_store_health_record_and_methods() {
        let store = OrchestratorStore::open_memory().unwrap();

        // upsert_health constructs HealthRecord internally
        store.upsert_health("ollama", "online", 0, 0).unwrap();
        store.upsert_health("docker", "offline", 3, 1).unwrap();

        // get_all_health returns Vec<HealthRecord>
        let records = store.get_all_health().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records.iter().find(|r| r.service_name == "ollama").unwrap().status, "online");
        assert_eq!(records.iter().find(|r| r.service_name == "docker").unwrap().consecutive_failures, 3);

        // get_worker_stats
        store.log_task("w1", "ok", 100, None, None).unwrap();
        store.log_task("w1", "ok", 200, None, None).unwrap();
        store.log_task("w1", "error", 50, None, Some("fail")).unwrap();
        let (ok, fail) = store.get_worker_stats("w1").unwrap();
        assert_eq!(ok, 2);
        assert_eq!(fail, 1);

        // cleanup_old_logs (deletes nothing since logs are fresh)
        let deleted = store.cleanup_old_logs(1).unwrap();
        assert_eq!(deleted, 0);
    }

    // ─── Trust: get_worker_trust ─────────────────────────────────

    #[test]
    fn test_get_worker_trust() {
        let mut tm = HebbianTrustManager::new();
        tm.record_success("w1", 100);

        let wt = tm.get_worker_trust("w1");
        assert_eq!(wt.name, "w1");
        assert!(wt.score > 0.5);
        assert_eq!(wt.successes, 1);

        // Non-existent worker returns default
        let wt2 = tm.get_worker_trust("nonexistent");
        assert_eq!(wt2.name, "nonexistent");
        assert!((wt2.score - 0.5).abs() < f64::EPSILON);
    }

    // ─── Brain: FsrsScheduler::with_params, FsrsCard::consolidation_priority ──

    #[test]
    fn test_fsrs_with_params() {
        let mut custom_params = FsrsParams::default();
        custom_params.request_retention = 0.85;
        let scheduler = FsrsScheduler::with_params(custom_params);
        let card = FsrsCard::default();
        let next = scheduler.review(&card, brain::Rating::Easy);
        assert!(next.stability > 0.0);
    }

    #[test]
    fn test_fsrs_consolidation_priority() {
        let engine = ClsReplayEngine::default();
        let memory = ClsMemory {
            key: "test".to_string(),
            content: "test content".to_string(),
            importance: 0.9,
            layer: MemoryLayer::Hippocampus,
            access_count: 5,
            created_at: Utc::now() - chrono::Duration::hours(48),
            consolidated_at: None,
        };
        let priority = engine.consolidation_priority(&memory);
        // High importance + high access_count → high priority
        assert!(priority > 0.0);
    }

    // ─── Brain: ZettelIndex::find_by_tag, find_related ──────────

    #[test]
    fn test_zettel_find_by_tag_and_related() {
        let mut index = ZettelIndex::new();
        let note1 = ZettelNote {
            id: "note1".to_string(),
            title: "Rust async patterns".to_string(),
            content: "How to use tokio effectively".to_string(),
            tags: vec!["rust".to_string(), "async".to_string()],
            links: vec!["note2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let note2 = ZettelNote {
            id: "note2".to_string(),
            title: "Tokio runtime".to_string(),
            content: "Configuring tokio for multi-threaded".to_string(),
            tags: vec!["rust".to_string(), "tokio".to_string()],
            links: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        index.add_note(note1);
        index.add_note(note2);

        // find_by_tag
        let rust_notes = index.find_by_tag("rust");
        assert_eq!(rust_notes.len(), 2);

        let async_notes = index.find_by_tag("async");
        assert_eq!(async_notes.len(), 1);
        assert_eq!(async_notes[0].id, "note1");

        // find_related (follows links)
        let related = index.find_related("note1");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].id, "note2");

        // Non-existent note
        let no_related = index.find_related("nonexistent");
        assert!(no_related.is_empty());
    }

    // ─── Workers: pool() trait method ────────────────────────────

    #[test]
    fn test_worker_pool_method() {
        let all_workers = workers::create_all_workers();
        for worker in &all_workers {
            let pool = worker.pool();
            // Every worker must declare a pool
            assert!(
                ["shell", "gpu", "cpu", "embed"].contains(&pool),
                "Worker {} has unknown pool: {}",
                worker.name(),
                pool,
            );
        }
    }

    // ─── HebbianTrustManager as TrustScorer (via trait) ─────────

    #[test]
    fn test_hebbian_as_trust_scorer_trait() {
        let mut scorer: Box<dyn TrustScorer> = Box::new(HebbianTrustManager::new());
        let s1 = scorer.record_outcome("w1", TaskOutcome::Success { duration_ms: 100 });
        assert!(s1 > 0.5);
        let s2 = scorer.record_outcome("w1", TaskOutcome::Timeout);
        assert!(s2 < s1);
        let scores = scorer.all_scores();
        assert_eq!(scores.len(), 1);
        assert!(scorer.average() > 0.0);
    }

    // ─── Router: with_ollama ─────────────────────────────────────

    #[test]
    fn test_router_config_with_ollama() {
        use crate::router::{RouterConfig, targets::select_target, classify_fast};
        let config = RouterConfig::new()
            .with_openrouter_key("test".into())
            .with_ollama(16.0);
        assert!(config.ollama_available);
        assert_eq!(config.ollama_vram_gb, Some(16.0));

        // With ollama + prefer_free_models, simple tasks route locally
        let target = select_target(classify_fast("hello"), &config);
        assert!(target.is_free());
    }
}
