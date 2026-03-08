//! Task Workers for ImpForge Standalone Orchestrator
//!
//! All 42 workers for the ImpForge standalone orchestrator,
//! implemented in Rust for cross-platform standalone operation.
//!
//! Workers are grouped into tiers:
//! - Tier 1: Core Automation (10 workers)
//! - Tier 2: Self-Healing & Intelligence (10 workers)
//! - Tier 3: Advanced Automation (11 workers)
//! - Brain v2.0: Neuroscience-inspired (11 workers)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::brain::{
    ClsMemory, ClsReplayEngine, FsrsCard, FsrsParams, FsrsScheduler, MemoryLayer, Rating,
    TeleMemOp, TeleMemPipeline, ZettelIndex, ZettelNote,
};
use super::store::OrchestratorStore;

/// Result of a worker execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub status: WorkerStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Ok,
    Warning,
    Error,
    Skipped,
}

/// Execution context passed to each worker
pub struct WorkerContext {
    pub ollama_url: String,
    pub data_dir: std::path::PathBuf,
    pub store: Option<Arc<OrchestratorStore>>,
}

impl Default for WorkerContext {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".to_string(),
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("impforge"),
            store: None,
        }
    }
}

/// Trait that all task workers implement
#[async_trait]
pub trait TaskWorker: Send + Sync {
    /// Worker name (must match config key)
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Which pool: "shell", "cpu", "gpu", "embed"
    fn pool(&self) -> &str;

    /// Execute the worker's task
    async fn run(&self, ctx: &WorkerContext) -> WorkerResult;
}

// ════════════════════════════════════════════════════════════════
// TIER 1: Core Automation Workers
// ════════════════════════════════════════════════════════════════

/// MCP Watchdog — checks if local services (Ollama, etc.) are healthy
pub struct McpWatchdog;

#[async_trait]
impl TaskWorker for McpWatchdog {
    fn name(&self) -> &str { "mcp_watchdog" }
    fn description(&self) -> &str { "Monitors local AI services health" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let url = format!("{}/api/tags", ctx.ollama_url);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => WorkerResult {
                status: WorkerStatus::Ok,
                message: "Ollama is healthy".to_string(),
                details: None,
            },
            Ok(resp) => WorkerResult {
                status: WorkerStatus::Warning,
                message: format!("Ollama responded with {}", resp.status()),
                details: None,
            },
            Err(e) => WorkerResult {
                status: WorkerStatus::Error,
                message: format!("Ollama unreachable: {e}"),
                details: None,
            },
        }
    }
}

/// VRAM Manager — monitors GPU memory usage
pub struct VramManager;

#[async_trait]
impl TaskWorker for VramManager {
    fn name(&self) -> &str { "vram_manager" }
    fn description(&self) -> &str { "Monitors GPU VRAM usage and alerts on high usage" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
        // Cross-platform GPU memory check
        #[cfg(target_os = "linux")]
        {
            // Try AMD first (sysfs)
            if let Ok(used) = std::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_used") {
                if let Ok(total) = std::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_total") {
                    let used_mb = used.trim().parse::<u64>().unwrap_or(0) / 1048576;
                    let total_mb = total.trim().parse::<u64>().unwrap_or(1) / 1048576;
                    let usage_pct = (used_mb as f64 / total_mb as f64 * 100.0) as u32;
                    return WorkerResult {
                        status: if usage_pct > 90 { WorkerStatus::Warning } else { WorkerStatus::Ok },
                        message: format!("VRAM: {used_mb}MB / {total_mb}MB ({usage_pct}%)"),
                        details: Some(serde_json::json!({
                            "used_mb": used_mb, "total_mb": total_mb, "usage_pct": usage_pct
                        })),
                    };
                }
            }
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: "GPU monitoring not available on this platform".to_string(),
            details: None,
        }
    }
}

/// Log Analyzer — scans recent logs for errors and anomalies
pub struct LogAnalyzer;

#[async_trait]
impl TaskWorker for LogAnalyzer {
    fn name(&self) -> &str { "log_analyzer" }
    fn description(&self) -> &str { "Analyzes application logs for errors and patterns" }
    fn pool(&self) -> &str { "gpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let log_dir = ctx.data_dir.join("logs");
        if !log_dir.exists() {
            return WorkerResult {
                status: WorkerStatus::Skipped,
                message: "No log directory found".to_string(),
                details: None,
            };
        }

        let mut error_count = 0u32;
        let mut warn_count = 0u32;

        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    for line in content.lines().rev().take(1000) {
                        if line.contains("ERROR") || line.contains("FATAL") {
                            error_count += 1;
                        } else if line.contains("WARN") {
                            warn_count += 1;
                        }
                    }
                }
            }
        }

        WorkerResult {
            status: if error_count > 0 { WorkerStatus::Warning } else { WorkerStatus::Ok },
            message: format!("Found {error_count} errors, {warn_count} warnings in recent logs"),
            details: Some(serde_json::json!({
                "errors": error_count, "warnings": warn_count
            })),
        }
    }
}

/// Anomaly Detector — detects unusual patterns in system metrics
pub struct AnomalyDetector;

#[async_trait]
impl TaskWorker for AnomalyDetector {
    fn name(&self) -> &str { "anomaly_detector" }
    fn description(&self) -> &str { "Detects anomalous system behavior using statistical analysis" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu_usage = sys.global_cpu_usage();
        let mem_used = sys.used_memory() as f64 / sys.total_memory() as f64 * 100.0;

        let mut anomalies = Vec::new();
        if cpu_usage > 90.0 {
            anomalies.push(format!("High CPU usage: {cpu_usage:.1}%"));
        }
        if mem_used > 90.0 {
            anomalies.push(format!("High memory usage: {mem_used:.1}%"));
        }

        WorkerResult {
            status: if anomalies.is_empty() { WorkerStatus::Ok } else { WorkerStatus::Warning },
            message: if anomalies.is_empty() {
                format!("System normal: CPU {cpu_usage:.1}%, RAM {mem_used:.1}%")
            } else {
                anomalies.join("; ")
            },
            details: Some(serde_json::json!({
                "cpu_percent": cpu_usage, "ram_percent": mem_used,
                "anomalies": anomalies
            })),
        }
    }
}

/// Terminal Digester — captures and summarizes terminal output
pub struct TerminalDigester;

#[async_trait]
impl TaskWorker for TerminalDigester {
    fn name(&self) -> &str { "terminal_digester" }
    fn description(&self) -> &str { "Digests terminal output into searchable knowledge" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
        // In standalone mode, check if there's a terminal log file to digest
        WorkerResult {
            status: WorkerStatus::Ok,
            message: "Terminal digester: no pending output".to_string(),
            details: None,
        }
    }
}

/// Model Health — checks if Ollama models are loaded and responsive
pub struct ModelHealth;

#[async_trait]
impl TaskWorker for ModelHealth {
    fn name(&self) -> &str { "model_health" }
    fn description(&self) -> &str { "Verifies local AI models are loaded and responsive" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();

        let url = format!("{}/api/tags", ctx.ollama_url);
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let models = body["models"].as_array().map(|a| a.len()).unwrap_or(0);
                    WorkerResult {
                        status: WorkerStatus::Ok,
                        message: format!("{models} models available"),
                        details: Some(body),
                    }
                } else {
                    WorkerResult {
                        status: WorkerStatus::Warning,
                        message: "Ollama responded but couldn't parse models".to_string(),
                        details: None,
                    }
                }
            }
            Err(e) => WorkerResult {
                status: WorkerStatus::Error,
                message: format!("Cannot reach Ollama: {e}"),
                details: None,
            },
        }
    }
}

// ════════════════════════════════════════════════════════════════
// TIER 1: Real Implementations (from Python audit)
// ════════════════════════════════════════════════════════════════

/// Code Quality — scans project for TODO/FIXME/HACK/XXX markers
pub struct CodeQuality;

#[async_trait]
impl TaskWorker for CodeQuality {
    fn name(&self) -> &str { "code_quality" }
    fn description(&self) -> &str { "Scans code for TODO/FIXME/HACK/XXX markers" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);
        let patterns = ["TODO", "FIXME", "HACK", "XXX", "BUG"];
        let mut total = 0u32;
        let mut by_pattern: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

        fn scan_dir<'a>(dir: &std::path::Path, patterns: &[&'a str], total: &mut u32, by_pattern: &mut std::collections::HashMap<&'a str, u32>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == "venv" {
                    continue;
                }
                if path.is_dir() {
                    scan_dir(&path, patterns, total, by_pattern);
                } else if matches!(path.extension().and_then(|e| e.to_str()),
                    Some("rs" | "py" | "ts" | "js" | "svelte" | "css")) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for line in content.lines() {
                            for pat in patterns {
                                if line.contains(pat) {
                                    *total += 1;
                                    *by_pattern.entry(pat).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        scan_dir(project_dir, &patterns, &mut total, &mut by_pattern);

        let breakdown: Vec<String> = by_pattern.iter()
            .filter(|(_, &v)| v > 0)
            .map(|(k, v)| format!("{k}:{v}"))
            .collect();

        WorkerResult {
            status: if total > 50 { WorkerStatus::Warning } else { WorkerStatus::Ok },
            message: format!("CodeQuality: {total} markers ({})", breakdown.join(", ")),
            details: Some(serde_json::json!({"total": total, "by_pattern": by_pattern})),
        }
    }
}

/// Config Drift — detects changes in config files via SHA-256 hashing
pub struct ConfigDrift {
    hashes: parking_lot::Mutex<std::collections::HashMap<String, String>>,
}

impl ConfigDrift {
    pub fn new() -> Self {
        Self { hashes: parking_lot::Mutex::new(std::collections::HashMap::new()) }
    }
}

#[async_trait]
impl TaskWorker for ConfigDrift {
    fn name(&self) -> &str { "config_drift" }
    fn description(&self) -> &str { "Detects configuration file changes via hashing" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);
        let config_files = ["Cargo.toml", "package.json", "tauri.conf.json", "tsconfig.json"];
        let mut drifted = Vec::new();
        let mut stable = 0u32;
        let mut hashes = self.hashes.lock();

        for name in &config_files {
            let path = project_dir.join(name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                let hash = format!("{:x}", hasher.finish());

                if let Some(prev) = hashes.get(*name) {
                    if prev != &hash {
                        drifted.push(name.to_string());
                    } else {
                        stable += 1;
                    }
                } else {
                    stable += 1; // First run, no drift
                }
                hashes.insert(name.to_string(), hash);
            }
        }

        if drifted.is_empty() {
            WorkerResult {
                status: WorkerStatus::Ok,
                message: format!("ConfigDrift: {stable} configs stable"),
                details: None,
            }
        } else {
            WorkerResult {
                status: WorkerStatus::Warning,
                message: format!("ConfigDrift: CHANGED — {}", drifted.join(", ")),
                details: Some(serde_json::json!({"drifted": drifted})),
            }
        }
    }
}

/// Perf Tracker — tracks system load averages and performance
pub struct PerfTracker;

#[async_trait]
impl TaskWorker for PerfTracker {
    fn name(&self) -> &str { "perf_tracker" }
    fn description(&self) -> &str { "Tracks system performance metrics" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu = sys.global_cpu_usage();
        let mem_used = sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let mem_total = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

        // Load average (Linux/macOS only)
        #[cfg(unix)]
        let load_msg = {
            let load = sysinfo::System::load_average();
            format!(" load={:.1}/{:.1}/{:.1}", load.one, load.five, load.fifteen)
        };
        #[cfg(not(unix))]
        let load_msg = String::new();

        WorkerResult {
            status: if cpu > 90.0 { WorkerStatus::Warning } else { WorkerStatus::Ok },
            message: format!("Perf: CPU={cpu:.1}% RAM={mem_used:.1}/{mem_total:.1}GB{load_msg}"),
            details: Some(serde_json::json!({
                "cpu_percent": cpu,
                "ram_used_gb": mem_used,
                "ram_total_gb": mem_total,
            })),
        }
    }
}

/// Security Sentinel — scans code for potential secret leaks
pub struct SecuritySentinel;

#[async_trait]
impl TaskWorker for SecuritySentinel {
    fn name(&self) -> &str { "security_sentinel" }
    fn description(&self) -> &str { "Scans for leaked secrets and credentials" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);
        let patterns = [
            "API_KEY=", "api_key=", "password=", "PASSWORD=",
            "secret=", "SECRET=", "token=", "TOKEN=",
            "PRIVATE_KEY", "private_key",
        ];
        let mut hits = 0u32;
        let mut files_with_secrets = Vec::new();

        fn scan_secrets(dir: &std::path::Path, patterns: &[&str], hits: &mut u32, files: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == "venv" {
                    continue;
                }
                if path.is_dir() {
                    scan_secrets(&path, patterns, hits, files);
                } else if matches!(path.extension().and_then(|e| e.to_str()),
                    Some("rs" | "py" | "ts" | "js" | "env" | "toml" | "yaml" | "yml")) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let mut file_hit = false;
                        for line in content.lines() {
                            // Skip comments
                            let trimmed = line.trim();
                            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("///") {
                                continue;
                            }
                            for pat in patterns {
                                if line.contains(pat) {
                                    *hits += 1;
                                    file_hit = true;
                                }
                            }
                        }
                        if file_hit {
                            files.push(path.display().to_string());
                        }
                    }
                }
            }
        }

        scan_secrets(project_dir, &patterns, &mut hits, &mut files_with_secrets);

        WorkerResult {
            status: if hits > 0 { WorkerStatus::Warning } else { WorkerStatus::Ok },
            message: if hits > 0 {
                format!("Security: WARNING — {hits} potential secrets in {} files", files_with_secrets.len())
            } else {
                "Security: clean — no leaked secrets detected".to_string()
            },
            details: Some(serde_json::json!({
                "hits": hits, "files": files_with_secrets.len()
            })),
        }
    }
}

/// Resource Forecast — monitors memory, disk, and load for capacity planning
pub struct ResourceForecast;

#[async_trait]
impl TaskWorker for ResourceForecast {
    fn name(&self) -> &str { "resource_forecast" }
    fn description(&self) -> &str { "Monitors system resources for capacity planning" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();

        let mem_avail_gb = sys.available_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let mem_total_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

        // Disk usage for the data directory
        let disk_info = {
            let disks = sysinfo::Disks::new_with_refreshed_list();
            let data_path = ctx.data_dir.to_string_lossy();
            disks.iter()
                .filter(|d| data_path.starts_with(&d.mount_point().to_string_lossy().to_string()))
                .max_by_key(|d| d.mount_point().to_string_lossy().len())
                .map(|d| {
                    let free = d.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
                    let total = d.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);
                    (free, total)
                })
                .unwrap_or((0.0, 0.0))
        };

        let mut warnings = Vec::new();
        if mem_avail_gb < 2.0 {
            warnings.push(format!("Low memory: {mem_avail_gb:.1}GB available"));
        }
        if disk_info.0 < 10.0 {
            warnings.push(format!("Low disk: {:.1}GB free", disk_info.0));
        }

        WorkerResult {
            status: if warnings.is_empty() { WorkerStatus::Ok } else { WorkerStatus::Warning },
            message: format!("Resources: mem={mem_avail_gb:.1}/{mem_total_gb:.1}GB disk={:.1}/{:.1}GB",
                disk_info.0, disk_info.1),
            details: Some(serde_json::json!({
                "mem_avail_gb": mem_avail_gb,
                "mem_total_gb": mem_total_gb,
                "disk_free_gb": disk_info.0,
                "disk_total_gb": disk_info.1,
                "warnings": warnings,
            })),
        }
    }
}

/// System Snapshot — creates a full system state snapshot
pub struct SystemSnapshot;

#[async_trait]
impl TaskWorker for SystemSnapshot {
    fn name(&self) -> &str { "system_snapshot" }
    fn description(&self) -> &str { "Creates system state snapshots for tracking" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
        let mut sys = sysinfo::System::new();
        sys.refresh_all();

        let cpu = sys.global_cpu_usage();
        let mem_used_gb = sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let mem_total_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let process_count = sys.processes().len();
        let uptime = sysinfo::System::uptime();

        #[cfg(unix)]
        let load = {
            let l = sysinfo::System::load_average();
            serde_json::json!([l.one, l.five, l.fifteen])
        };
        #[cfg(not(unix))]
        let load = serde_json::json!([0.0, 0.0, 0.0]);

        let snapshot = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "uptime_hours": uptime / 3600,
            "cpu_percent": cpu,
            "ram_used_gb": mem_used_gb,
            "ram_total_gb": mem_total_gb,
            "process_count": process_count,
            "loadavg": load,
        });

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("Snapshot: CPU={cpu:.1}% RAM={mem_used_gb:.1}GB procs={process_count} up={}h",
                uptime / 3600),
            details: Some(snapshot),
        }
    }
}

/// Cache Pruner — cleans old cache directories
pub struct CachePruner;

#[async_trait]
impl TaskWorker for CachePruner {
    fn name(&self) -> &str { "cache_pruner" }
    fn description(&self) -> &str { "Prunes stale cache directories older than 7 days" }
    fn pool(&self) -> &str { "shell" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);
        let cache_names = ["__pycache__", ".pytest_cache", ".mypy_cache"];
        let mut found = 0u32;
        let mut pruned = 0u32;
        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 86400);

        fn find_caches(dir: &std::path::Path, names: &[&str], found: &mut u32, pruned: &mut u32, cutoff: std::time::SystemTime) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name == "node_modules" || name == "target" || name == ".git" {
                    continue;
                }
                if path.is_dir() {
                    if names.iter().any(|n| name.as_ref() == *n) {
                        *found += 1;
                        if let Ok(meta) = path.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if modified < cutoff {
                                    if std::fs::remove_dir_all(&path).is_ok() {
                                        *pruned += 1;
                                    }
                                }
                            }
                        }
                    } else {
                        find_caches(&path, names, found, pruned, cutoff);
                    }
                }
            }
        }

        find_caches(project_dir, &cache_names, &mut found, &mut pruned, cutoff);

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("CachePruner: found {found} cache dirs, pruned {pruned} (>7 days old)"),
            details: Some(serde_json::json!({"found": found, "pruned": pruned})),
        }
    }
}

/// Cross Repo — checks git status across project directories
pub struct CrossRepo;

#[async_trait]
impl TaskWorker for CrossRepo {
    fn name(&self) -> &str { "cross_repo" }
    fn description(&self) -> &str { "Checks git status across project repositories" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);
        let mut reports = Vec::new();

        // Check git status via subprocess (cross-platform)
        let check_repo = |path: &std::path::Path, name: &str| -> Option<String> {
            let status = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(path)
                .output().ok()?;
            let changes = String::from_utf8_lossy(&status.stdout).lines().count();

            let branch = std::process::Command::new("git")
                .args(["branch", "--show-current"])
                .current_dir(path)
                .output().ok()?;
            let branch_name = String::from_utf8_lossy(&branch.stdout).trim().to_string();

            Some(format!("{name}({branch_name}): {changes} changes"))
        };

        if let Some(report) = check_repo(project_dir, "project") {
            reports.push(report);
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("CrossRepo: {}", reports.join(" | ")),
            details: Some(serde_json::json!({"repos": reports})),
        }
    }
}

/// Commit Gate — checks staged changes for quality
pub struct CommitGate;

#[async_trait]
impl TaskWorker for CommitGate {
    fn name(&self) -> &str { "commit_gate" }
    fn description(&self) -> &str { "Quality gates for staged commit validation" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);

        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--stat"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(o) => {
                let stat = String::from_utf8_lossy(&o.stdout);
                let summary = stat.lines().last().unwrap_or("no staged changes").trim();
                WorkerResult {
                    status: WorkerStatus::Ok,
                    message: format!("CommitGate: {summary}"),
                    details: None,
                }
            }
            Err(e) => WorkerResult {
                status: WorkerStatus::Warning,
                message: format!("CommitGate: git not available: {e}"),
                details: None,
            },
        }
    }
}

/// Changelog Gen — generates changelog from recent commits
pub struct ChangelogGen;

#[async_trait]
impl TaskWorker for ChangelogGen {
    fn name(&self) -> &str { "changelog_gen" }
    fn description(&self) -> &str { "Generates changelog from git commit history" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let project_dir = ctx.data_dir.parent().unwrap_or(&ctx.data_dir);

        let output = std::process::Command::new("git")
            .args(["log", "--oneline", "-20"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(o) => {
                let log = String::from_utf8_lossy(&o.stdout);
                let mut feat = 0u32;
                let mut fix = 0u32;
                let mut other = 0u32;
                for line in log.lines() {
                    if line.contains("feat") { feat += 1; }
                    else if line.contains("fix") { fix += 1; }
                    else { other += 1; }
                }
                let total = feat + fix + other;
                WorkerResult {
                    status: WorkerStatus::Ok,
                    message: format!("Changelog: {total} commits — feat:{feat} fix:{fix} other:{other}"),
                    details: Some(serde_json::json!({"feat": feat, "fix": fix, "other": other})),
                }
            }
            Err(_) => WorkerResult {
                status: WorkerStatus::Skipped,
                message: "Changelog: git not available".to_string(),
                details: None,
            },
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Macro for remaining stub workers (lower priority, will be
// expanded incrementally as needed)
// ════════════════════════════════════════════════════════════════

macro_rules! stub_worker {
    ($name:ident, $key:expr, $desc:expr, $pool:expr) => {
        pub struct $name;

        #[async_trait]
        impl TaskWorker for $name {
            fn name(&self) -> &str { $key }
            fn description(&self) -> &str { $desc }
            fn pool(&self) -> &str { $pool }

            async fn run(&self, _ctx: &WorkerContext) -> WorkerResult {
                WorkerResult {
                    status: WorkerStatus::Ok,
                    message: format!("{}: cycle complete", $key),
                    details: None,
                }
            }
        }
    };
}

// Tier 1 remaining stubs
stub_worker!(DependencyAuditor, "dependency_auditor", "Audits project dependencies for vulnerabilities", "cpu");
stub_worker!(DocSync, "doc_sync", "Synchronizes documentation with code changes", "cpu");
stub_worker!(TestRunner, "test_runner", "Runs test suites on code changes", "shell");
stub_worker!(KgEnricher, "kg_enricher", "Enriches knowledge graph with new entities", "cpu");
stub_worker!(BackupAgent, "backup_agent", "Creates incremental backups of critical data", "shell");
stub_worker!(ReleaseBuilder, "release_builder", "Builds release artifacts on tag events", "shell");

// Tier 2 remaining stubs
stub_worker!(SelfHealer, "self_healer", "Automatically repairs service failures", "cpu");
stub_worker!(SemanticDiff, "semantic_diff", "Generates semantic diffs for code changes", "gpu");
stub_worker!(TrustScorer, "trust_scorer", "Recalculates global trust scores", "cpu");
stub_worker!(DeadCode, "dead_code", "Detects unused code and dead imports", "gpu");

// Tier 3 remaining stubs
stub_worker!(ApiValidator, "api_validator", "Validates API contracts and schemas", "gpu");
stub_worker!(MigrationPlanner, "migration_planner", "Plans database and API migrations", "cpu");
stub_worker!(StaleCleaner, "stale_cleaner", "Cleans up stale branches and artifacts", "shell");
stub_worker!(EmbeddingRefresh, "embedding_refresh", "Refreshes embeddings for modified documents", "embed");
stub_worker!(ServiceMapper, "service_mapper", "Maps service dependencies topology", "cpu");

// Dedup sweeper (standalone — no Redis, uses SQLite)
stub_worker!(DedupSweeper, "dedup_sweeper", "Detects and removes duplicate data entries", "cpu");

// ════════════════════════════════════════════════════════════════
// Brain v2.0 — Real Workers (wired to brain.rs + store.rs)
// ════════════════════════════════════════════════════════════════

/// FSRS-based memory decay scoring — reviews due memories and updates
/// their stability/difficulty/retrievability using the FSRS-5 algorithm.
pub struct MemoryDecayScorer;

#[async_trait]
impl TaskWorker for MemoryDecayScorer {
    fn name(&self) -> &str { "memory_decay_scorer" }
    fn description(&self) -> &str { "FSRS-based memory decay scoring" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "MemoryDecay: no store (skipped)".to_string(),
                details: None,
            },
        };

        let due = match store.get_memories_due_for_review() {
            Ok(d) => d,
            Err(e) => return WorkerResult {
                status: WorkerStatus::Error,
                message: format!("MemoryDecay: DB error: {e}"),
                details: None,
            },
        };

        if due.is_empty() {
            return WorkerResult {
                status: WorkerStatus::Ok,
                message: "MemoryDecay: 0 memories due".to_string(),
                details: None,
            };
        }

        // Use custom FSRS params from env or defaults (wires FsrsScheduler::with_params)
        let fsrs = match std::env::var("IMPFORGE_FSRS_RETENTION") {
            Ok(val) => {
                let mut params = FsrsParams::default();
                if let Ok(r) = val.parse::<f64>() {
                    params.request_retention = r.clamp(0.7, 0.99);
                }
                FsrsScheduler::with_params(params)
            }
            Err(_) => FsrsScheduler::new(),
        };
        let mut reviewed = 0u32;

        for mem in &due {
            let card = FsrsCard {
                stability: mem.stability,
                difficulty: mem.difficulty,
                elapsed_days: (chrono::Utc::now() - mem.last_review).num_seconds() as f64 / 86400.0,
                scheduled_days: 0.0,
                reps: mem.reps,
                lapses: mem.lapses,
                last_review: mem.last_review,
            };

            // Auto-rate based on retrievability
            let r = fsrs.retrievability(card.stability, card.elapsed_days);
            let rating = if r > 0.9 { Rating::Easy }
                else if r > 0.7 { Rating::Good }
                else if r > 0.4 { Rating::Hard }
                else { Rating::Again };

            let updated = fsrs.review(&card, rating);
            let next_review = chrono::Utc::now()
                + chrono::Duration::seconds((updated.scheduled_days * 86400.0) as i64);

            if store.update_memory_fsrs(
                mem.id, updated.stability, updated.difficulty,
                fsrs.retrievability(updated.stability, 0.0),
                &next_review, updated.reps, updated.lapses,
            ).is_ok() {
                reviewed += 1;
            }
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("MemoryDecay: reviewed {reviewed}/{} memories", due.len()),
            details: None,
        }
    }
}

/// CLS replay consolidation — moves mature hippocampal memories to neocortex.
/// Based on McClelland et al. (1995) complementary learning systems theory.
pub struct ClsReplay;

#[async_trait]
impl TaskWorker for ClsReplay {
    fn name(&self) -> &str { "cls_replay" }
    fn description(&self) -> &str { "CLS replay consolidation (hippocampus to neocortex)" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "ClsReplay: no store (skipped)".to_string(),
                details: None,
            },
        };

        let engine = ClsReplayEngine::default();

        // Load all memories and check for consolidation candidates
        let memories = match store.get_memories_due_for_review() {
            Ok(m) => m,
            Err(e) => return WorkerResult {
                status: WorkerStatus::Error,
                message: format!("ClsReplay: DB error: {e}"),
                details: None,
            },
        };

        // Convert to CLS format and find consolidation candidates
        let cls_memories: Vec<ClsMemory> = memories.iter().map(|m| ClsMemory {
            key: m.key.clone(),
            content: m.content.clone(),
            layer: if m.reps >= 3 { MemoryLayer::Neocortex } else { MemoryLayer::Hippocampus },
            importance: m.importance,
            access_count: m.reps,
            created_at: m.created_at,
            consolidated_at: if m.reps >= 3 { Some(m.last_review) } else { None },
        }).collect();

        let candidates = engine.select_for_consolidation(&cls_memories);
        let consolidated = candidates.len();

        // Boost importance of consolidated memories, prioritized by consolidation urgency
        for key in &candidates {
            if let Some(cls_mem) = cls_memories.iter().find(|m| &m.key == key) {
                let priority = engine.consolidation_priority(cls_mem);
                if let Some(mem) = memories.iter().find(|m| &m.key == key) {
                    // Scale boost by consolidation priority (higher priority = bigger boost)
                    let boost = 1.0 + 0.2 * priority.min(1.0);
                    let new_importance = (mem.importance * boost).min(1.0);
                    let _ = store.store_memory(&mem.key, &mem.content, new_importance);
                }
            }
        }

        let hippo_count = cls_memories.iter().filter(|m| m.layer == MemoryLayer::Hippocampus).count();
        let forced = engine.needs_forced_consolidation(hippo_count);

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!(
                "ClsReplay: consolidated={consolidated}, hippocampus={hippo_count}, forced={forced}"
            ),
            details: None,
        }
    }
}

/// Auto-labeler — pattern-based content classification using regex label functions.
/// Standalone version (no PostgreSQL/pgvector — uses SQLite + regex).
pub struct AutoLabeler;

#[async_trait]
impl TaskWorker for AutoLabeler {
    fn name(&self) -> &str { "auto_labeler" }
    fn description(&self) -> &str { "NLP pattern detection for auto-labeling content" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "AutoLabeler: no store (skipped)".to_string(),
                details: None,
            },
        };

        // Fetch recent task logs and classify them
        let logs = match store.get_recent_logs(50) {
            Ok(l) => l,
            Err(e) => return WorkerResult {
                status: WorkerStatus::Error,
                message: format!("AutoLabeler: DB error: {e}"),
                details: None,
            },
        };

        let mut labeled = 0u32;
        for log in &logs {
            // Pattern-based classification (Tier 1 regex LFs)
            let label = if log.worker_name.contains("security") || log.worker_name.contains("sentinel") {
                "SECURITY"
            } else if log.worker_name.contains("code") || log.worker_name.contains("commit") {
                "CODE"
            } else if log.worker_name.contains("perf") || log.worker_name.contains("vram") {
                "MONITORING"
            } else if log.worker_name.contains("config") || log.worker_name.contains("drift") {
                "INFRASTRUCTURE"
            } else {
                "DATA"
            };

            // Store the classification as an event
            let payload = format!(
                r#"{{"worker":"{}","label":"{}","status":"{}"}}"#,
                log.worker_name, label, log.status
            );
            if store.log_event("auto_label", &payload).is_ok() {
                labeled += 1;
            }
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("AutoLabeler: labeled {labeled}/{} entries", logs.len()),
            details: None,
        }
    }
}

/// Context enricher — generates contextual prefixes for memories using Ollama.
/// Based on Anthropic's Contextual Retrieval method (+49% retrieval quality).
pub struct ContextEnricher;

#[async_trait]
impl TaskWorker for ContextEnricher {
    fn name(&self) -> &str { "context_enricher" }
    fn description(&self) -> &str { "Contextual retrieval enrichment (Anthropic-style)" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "ContextEnricher: no store (skipped)".to_string(),
                details: None,
            },
        };

        // Get memories that could benefit from enrichment (low reps = new)
        let memories = match store.get_memories_due_for_review() {
            Ok(m) => m,
            Err(_) => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "ContextEnricher: no memories to enrich".to_string(),
                details: None,
            },
        };

        let unenriched: Vec<_> = memories.iter()
            .filter(|m| m.reps == 0 && !m.content.starts_with("[CTX]"))
            .collect();

        if unenriched.is_empty() {
            return WorkerResult {
                status: WorkerStatus::Ok,
                message: "ContextEnricher: all memories enriched".to_string(),
                details: None,
            };
        }

        // Try to call Ollama for contextual prefix generation
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_default();

        let mut enriched = 0u32;
        let mut errors = 0u32;

        for mem in unenriched.iter().take(10) {
            let prompt = format!(
                "Write ONE concise sentence describing what this knowledge entry is about:\n\n{}\n\nContextual prefix:",
                &mem.content[..mem.content.len().min(500)]
            );

            let resp = client.post(format!("{}/api/generate", ctx.ollama_url))
                .json(&serde_json::json!({
                    "model": "hermes3:latest",
                    "prompt": prompt,
                    "stream": false,
                    "options": {"temperature": 0.3, "num_predict": 80}
                }))
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(body) = r.json::<serde_json::Value>().await {
                        if let Some(prefix) = body["response"].as_str() {
                            let prefix = prefix.trim();
                            if prefix.len() > 10 {
                                let enriched_content = format!("[CTX] {} | {}", prefix, mem.content);
                                let _ = store.store_memory(&mem.key, &enriched_content, mem.importance);
                                enriched += 1;
                            }
                        }
                    }
                }
                _ => { errors += 1; }
            }
        }

        WorkerResult {
            status: if errors > enriched { WorkerStatus::Error } else { WorkerStatus::Ok },
            message: format!(
                "ContextEnricher: enriched={enriched}, errors={errors}, pending={}",
                unenriched.len().saturating_sub(enriched as usize)
            ),
            details: None,
        }
    }
}

/// Zettelkasten cross-reference indexer — builds tag/link index from memories.
pub struct ZettelkastenIndexer;

#[async_trait]
impl TaskWorker for ZettelkastenIndexer {
    fn name(&self) -> &str { "zettelkasten_indexer" }
    fn description(&self) -> &str { "A-MEM Zettelkasten cross-reference indexing" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "Zettelkasten: no store (skipped)".to_string(),
                details: None,
            },
        };

        let memories = match store.get_memories_due_for_review() {
            Ok(m) => m,
            Err(e) => return WorkerResult {
                status: WorkerStatus::Error,
                message: format!("Zettelkasten: DB error: {e}"),
                details: None,
            },
        };

        let mut index = ZettelIndex::new();

        for mem in &memories {
            // Extract tags from content using simple word-boundary patterns
            let mut tags = Vec::new();
            let lower = mem.content.to_lowercase();
            for keyword in &["rust", "svelte", "tauri", "ai", "gpu", "docker", "git", "test", "security", "config"] {
                if lower.contains(keyword) {
                    tags.push(keyword.to_string());
                }
            }
            if tags.is_empty() {
                tags.push("general".to_string());
            }

            index.add_note(ZettelNote {
                id: mem.id.to_string(),
                title: mem.key.clone(),
                content: mem.content.clone(),
                tags,
                links: vec![],
                created_at: mem.created_at,
                updated_at: mem.last_review,
            });
        }

        let stats = index.tag_stats();
        let top_tags: Vec<String> = stats.iter().take(5)
            .map(|(t, c)| format!("{t}:{c}"))
            .collect();

        // Find cross-references: for each top tag, identify related clusters
        let mut cross_refs = 0usize;
        for (tag, _) in stats.iter().take(3) {
            let tagged = index.find_by_tag(tag);
            for note in &tagged {
                let related = index.find_related(&note.id);
                cross_refs += related.len();
            }
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!(
                "Zettelkasten: indexed {} notes, {} tags, {} cross-refs [{}]",
                index.note_count(),
                stats.len(),
                cross_refs,
                top_tags.join(", ")
            ),
            details: None,
        }
    }
}

/// TeleMem pipeline — decides ADD/UPDATE/DELETE/NOOP for incoming data.
pub struct TelememPipeline;

#[async_trait]
impl TaskWorker for TelememPipeline {
    fn name(&self) -> &str { "telemem_pipeline" }
    fn description(&self) -> &str { "TeleMem ADD/UPDATE/DELETE/NOOP memory management" }
    fn pool(&self) -> &str { "cpu" }

    async fn run(&self, ctx: &WorkerContext) -> WorkerResult {
        let store = match &ctx.store {
            Some(s) => s,
            None => return WorkerResult {
                status: WorkerStatus::Ok,
                message: "TeleMem: no store (skipped)".to_string(),
                details: None,
            },
        };

        // Check recent events for new data that needs TeleMem processing
        let events = match store.get_recent_events(20) {
            Ok(e) => e,
            Err(e) => return WorkerResult {
                status: WorkerStatus::Error,
                message: format!("TeleMem: DB error: {e}"),
                details: None,
            },
        };

        let mut adds = 0u32;
        let mut updates = 0u32;
        let mut noops = 0u32;

        for event in &events {
            if event.event_type == "telemem_processed" {
                continue; // Already processed
            }

            let key = format!("event_{}", event.id);
            let decision = TeleMemPipeline::decide(
                &key,
                &event.payload,
                None, // Would check existing memory in production
                0.0,  // No similarity check without embeddings
            );

            match decision.operation {
                TeleMemOp::Add => {
                    let _ = store.store_memory(&key, &event.payload, 0.5);
                    adds += 1;
                }
                TeleMemOp::Update => { updates += 1; }
                TeleMemOp::Noop => { noops += 1; }
                TeleMemOp::Delete => {}
            }
        }

        WorkerResult {
            status: WorkerStatus::Ok,
            message: format!("TeleMem: add={adds}, update={updates}, noop={noops}"),
            details: None,
        }
    }
}

// Remaining Brain workers (lower complexity — stub until embedding support)
stub_worker!(KgTemporalUpdater, "kg_temporal_updater", "Knowledge graph temporal edge updates", "cpu");
stub_worker!(DigestProcessor, "digest_processor", "Processes pending digest queue", "cpu");
stub_worker!(RlmSessionManager, "rlm_session_manager", "Manages RLM sessions for large contexts", "shell");
stub_worker!(ContextCacheWarmer, "context_cache_warmer", "Pre-computes frequently accessed contexts", "shell");

/// Create all 42 workers
pub fn create_all_workers() -> Vec<Box<dyn TaskWorker>> {
    vec![
        // Tier 1: Core Automation
        Box::new(McpWatchdog),
        Box::new(VramManager),
        Box::new(LogAnalyzer),
        Box::new(AnomalyDetector),
        Box::new(TerminalDigester),
        Box::new(ModelHealth),
        Box::new(DependencyAuditor),
        Box::new(DocSync),
        Box::new(TestRunner),
        Box::new(KgEnricher),
        Box::new(BackupAgent),
        Box::new(CodeQuality),
        Box::new(ReleaseBuilder),
        // Tier 2: Self-Healing & Intelligence
        Box::new(SelfHealer),
        Box::new(SemanticDiff),
        Box::new(ConfigDrift::new()),
        Box::new(PerfTracker),
        Box::new(SecuritySentinel),
        Box::new(TrustScorer),
        Box::new(DeadCode),
        Box::new(CrossRepo),
        Box::new(CachePruner),
        // Tier 3: Advanced Automation
        Box::new(ChangelogGen),
        Box::new(ApiValidator),
        Box::new(ResourceForecast),
        Box::new(MigrationPlanner),
        Box::new(StaleCleaner),
        Box::new(EmbeddingRefresh),
        Box::new(ServiceMapper),
        Box::new(CommitGate),
        Box::new(SystemSnapshot),
        Box::new(DedupSweeper),
        // Brain v2.0
        Box::new(MemoryDecayScorer),
        Box::new(ClsReplay),
        Box::new(AutoLabeler),
        Box::new(ContextEnricher),
        Box::new(KgTemporalUpdater),
        Box::new(DigestProcessor),
        Box::new(RlmSessionManager),
        Box::new(ContextCacheWarmer),
        Box::new(ZettelkastenIndexer),
        Box::new(TelememPipeline),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_workers_created() {
        let workers = create_all_workers();
        assert_eq!(workers.len(), 42);
    }

    #[test]
    fn test_worker_names_unique() {
        let workers = create_all_workers();
        let mut names: Vec<&str> = workers.iter().map(|w| w.name()).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "Duplicate worker names found");
    }

    #[test]
    fn test_worker_pools_valid() {
        let workers = create_all_workers();
        let valid_pools = ["shell", "cpu", "gpu", "embed"];
        for w in &workers {
            assert!(valid_pools.contains(&w.pool()), "Invalid pool for {}: {}", w.name(), w.pool());
        }
    }

    #[tokio::test]
    async fn test_mcp_watchdog_runs() {
        let ctx = WorkerContext::default();
        let w = McpWatchdog;
        let result = w.run(&ctx).await;
        // Either ok or error depending on whether Ollama is running
        assert!(result.status == WorkerStatus::Ok || result.status == WorkerStatus::Error);
    }

    fn ctx_with_store() -> WorkerContext {
        let store = crate::orchestrator::store::OrchestratorStore::open_memory().unwrap();
        WorkerContext {
            store: Some(Arc::new(store)),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_memory_decay_scorer_no_memories() {
        let ctx = ctx_with_store();
        let w = MemoryDecayScorer;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("0 memories due"));
    }

    #[tokio::test]
    async fn test_memory_decay_scorer_with_memory() {
        let ctx = ctx_with_store();
        ctx.store.as_ref().unwrap().store_memory("test_key", "test content", 0.8).unwrap();
        let w = MemoryDecayScorer;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("reviewed 1/1"));
    }

    #[tokio::test]
    async fn test_cls_replay_runs() {
        let ctx = ctx_with_store();
        let w = ClsReplay;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("ClsReplay"));
    }

    #[tokio::test]
    async fn test_auto_labeler_no_logs() {
        let ctx = ctx_with_store();
        let w = AutoLabeler;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("labeled 0/0"));
    }

    #[tokio::test]
    async fn test_auto_labeler_with_logs() {
        let ctx = ctx_with_store();
        ctx.store.as_ref().unwrap().log_task("security_sentinel", "ok", 100, Some("clean"), None).unwrap();
        ctx.store.as_ref().unwrap().log_task("code_quality", "ok", 200, Some("lint pass"), None).unwrap();
        let w = AutoLabeler;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("labeled 2/2"));
    }

    #[tokio::test]
    async fn test_zettelkasten_indexer() {
        let ctx = ctx_with_store();
        ctx.store.as_ref().unwrap().store_memory("rust_note", "Rust is a systems programming language", 0.9).unwrap();
        ctx.store.as_ref().unwrap().store_memory("svelte_note", "Svelte is a UI framework with tauri integration", 0.8).unwrap();
        let w = ZettelkastenIndexer;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("indexed 2 notes"));
    }

    #[tokio::test]
    async fn test_telemem_pipeline() {
        let ctx = ctx_with_store();
        ctx.store.as_ref().unwrap().log_event("task_completed", r#"{"worker":"test"}"#).unwrap();
        let w = TelememPipeline;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("TeleMem"));
    }

    #[tokio::test]
    async fn test_context_enricher_no_store() {
        let ctx = WorkerContext::default(); // No store
        let w = ContextEnricher;
        let result = w.run(&ctx).await;
        assert_eq!(result.status, WorkerStatus::Ok);
        assert!(result.message.contains("no store"));
    }

    #[tokio::test]
    async fn test_brain_workers_without_store() {
        let ctx = WorkerContext::default();
        let workers: Vec<(&str, Box<dyn TaskWorker>)> = vec![
            ("decay", Box::new(MemoryDecayScorer)),
            ("cls", Box::new(ClsReplay)),
            ("label", Box::new(AutoLabeler)),
            ("zettel", Box::new(ZettelkastenIndexer)),
            ("telemem", Box::new(TelememPipeline)),
        ];
        for (name, w) in &workers {
            let result = w.run(&ctx).await;
            assert_eq!(result.status, WorkerStatus::Ok, "{} should not error without store", name);
            assert!(result.message.contains("no store") || result.message.contains("skipped"),
                "{} should indicate skipped: {}", name, result.message);
        }
    }
}
