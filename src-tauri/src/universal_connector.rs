// SPDX-License-Identifier: Apache-2.0
//! Universal Connector — Zero-config auto-discovery for all available services.
//!
//! Scientific basis: arXiv:2506.01056 (MCP-Zero: Active Tool Discovery for
//! Autonomous LLM Agents).
//!
//! When ImpForge starts it scans the local network for services that can be
//! integrated (Ollama, PostgreSQL, Redis, Docker, MCP servers, n8n, Git, etc.)
//! and exposes them through a unified dashboard.
//!
//! Design constraints:
//! - Port scanning uses 50 ms TCP connect timeout + parallel tokio::spawn
//! - Never blocks the UI — every scan is async
//! - No .unwrap() in production code
//! - All state persisted to `~/.impforge/connector/`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredService {
    pub id: String,
    pub name: String,
    pub service_type: ServiceType,
    pub url: String,
    pub port: u16,
    pub status: ServiceStatus,
    pub capabilities: Vec<String>,
    pub auto_connected: bool,
    pub last_check: String,
    pub response_time_ms: Option<u64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    Ollama,
    PostgreSql,
    Redis,
    Docker,
    Git,
    McpServer,
    ClaudeCode,
    N8n,
    HttpService,
    WebSocket,
    CustomApi,
}

impl ServiceType {
    #[allow(dead_code)]
    fn label(&self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::PostgreSql => "PostgreSQL",
            Self::Redis => "Redis",
            Self::Docker => "Docker",
            Self::Git => "Git",
            Self::McpServer => "MCP Server",
            Self::ClaudeCode => "Claude Code",
            Self::N8n => "n8n",
            Self::HttpService => "HTTP Service",
            Self::WebSocket => "WebSocket",
            Self::CustomApi => "Custom API",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Online,
    Offline,
    Checking,
    Error,
    AuthRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub auto_scan_on_start: bool,
    pub scan_interval_seconds: u32,
    pub custom_endpoints: Vec<CustomEndpoint>,
    pub notification_on_change: bool,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            auto_scan_on_start: true,
            scan_interval_seconds: 0,
            custom_endpoints: Vec::new(),
            notification_on_change: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEndpoint {
    pub name: String,
    pub url: String,
    pub health_path: Option<String>,
    pub expected_status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub services: Vec<DiscoveredService>,
    pub scan_duration_ms: u64,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// TCP connect timeout per port (must be fast to not block).
const PORT_TIMEOUT_MS: u64 = 50;

/// HTTP health-check timeout.
const HTTP_TIMEOUT: Duration = Duration::from_millis(2000);

/// Maximum number of scan results to keep in history.
const MAX_HISTORY: usize = 10;

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

fn connector_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".impforge")
        .join("connector")
}

fn config_path() -> PathBuf {
    connector_dir().join("config.json")
}

fn last_scan_path() -> PathBuf {
    connector_dir().join("last_scan.json")
}

fn history_path() -> PathBuf {
    connector_dir().join("scan_history.json")
}

fn ensure_dir() {
    let dir = connector_dir();
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
}

fn load_config() -> ConnectorConfig {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config_to_disk(config: &ConnectorConfig) {
    ensure_dir();
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(config_path(), json);
    }
}

fn save_scan_result(result: &ScanResult) {
    ensure_dir();
    if let Ok(json) = serde_json::to_string_pretty(result) {
        let _ = std::fs::write(last_scan_path(), json);
    }

    // Append to history (keep last MAX_HISTORY entries)
    let mut history = load_history();
    history.push(result.clone());
    if history.len() > MAX_HISTORY {
        history.drain(0..history.len() - MAX_HISTORY);
    }
    if let Ok(json) = serde_json::to_string_pretty(&history) {
        let _ = std::fs::write(history_path(), json);
    }
}

fn load_last_scan() -> Option<ScanResult> {
    std::fs::read_to_string(last_scan_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn load_history() -> Vec<ScanResult> {
    std::fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Low-level scanning
// ---------------------------------------------------------------------------

/// Try a raw TCP connect with a short timeout.
async fn scan_port(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{host}:{port}");
    tokio::time::timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr))
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
}

/// Measure round-trip time of a TCP connect.
async fn measure_port(host: &str, port: u16) -> Option<u64> {
    let addr = format!("{host}:{port}");
    let start = Instant::now();
    let ok = tokio::time::timeout(Duration::from_millis(PORT_TIMEOUT_MS), TcpStream::connect(&addr))
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false);
    if ok {
        Some(start.elapsed().as_millis() as u64)
    } else {
        None
    }
}

/// HTTP GET with timeout, returns (status, body).
async fn http_get(url: &str) -> Option<(u16, String)> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let resp = client.get(url).send().await.ok()?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    Some((status, body))
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn make_id(service_type: &ServiceType, port: u16) -> String {
    format!("{}-{}", serde_json::to_string(service_type).unwrap_or_default().trim_matches('"'), port)
}

// ---------------------------------------------------------------------------
// Service-specific detectors
// ---------------------------------------------------------------------------

async fn detect_ollama() -> Option<DiscoveredService> {
    let port: u16 = 11434;
    let start = Instant::now();

    let (status, body) = http_get("http://localhost:11434/api/tags").await?;
    let rtt = start.elapsed().as_millis() as u64;

    if status != 200 {
        return None;
    }

    // Parse model names from the JSON response
    let mut models: Vec<String> = Vec::new();
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(arr) = parsed.get("models").and_then(|m| m.as_array()) {
            for m in arr {
                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                    models.push(name.to_string());
                }
            }
        }
    }

    let mut caps = vec!["chat".into(), "embeddings".into(), "models".into(), "generate".into()];
    if !models.is_empty() {
        caps.push(format!("{} models loaded", models.len()));
    }

    Some(DiscoveredService {
        id: make_id(&ServiceType::Ollama, port),
        name: "Ollama".into(),
        service_type: ServiceType::Ollama,
        url: "http://localhost:11434".into(),
        port,
        status: ServiceStatus::Online,
        capabilities: caps,
        auto_connected: true,
        last_check: now_iso(),
        response_time_ms: Some(rtt),
        metadata: serde_json::json!({ "models": models }),
    })
}

async fn detect_postgresql(port: u16) -> Option<DiscoveredService> {
    let rtt = measure_port("localhost", port).await?;

    Some(DiscoveredService {
        id: make_id(&ServiceType::PostgreSql, port),
        name: format!("PostgreSQL (:{port})"),
        service_type: ServiceType::PostgreSql,
        url: format!("localhost:{port}"),
        port,
        status: ServiceStatus::Online,
        capabilities: vec!["sql".into(), "transactions".into(), "pgvector".into()],
        auto_connected: false,
        last_check: now_iso(),
        response_time_ms: Some(rtt),
        metadata: serde_json::json!({ "note": "TCP port open — credentials required to connect" }),
    })
}

async fn detect_redis() -> Option<DiscoveredService> {
    let port: u16 = 6379;
    let rtt = measure_port("localhost", port).await?;

    Some(DiscoveredService {
        id: make_id(&ServiceType::Redis, port),
        name: "Redis".into(),
        service_type: ServiceType::Redis,
        url: "localhost:6379".into(),
        port,
        status: ServiceStatus::Online,
        capabilities: vec!["cache".into(), "pub_sub".into(), "streams".into(), "vectors".into()],
        auto_connected: false,
        last_check: now_iso(),
        response_time_ms: Some(rtt),
        metadata: serde_json::json!({ "note": "TCP port open — auth may be required" }),
    })
}

async fn detect_docker() -> Option<DiscoveredService> {
    // Linux: check for socket
    let socket_exists = std::path::Path::new("/var/run/docker.sock").exists();

    // Fallback: TCP API
    let tcp_up = if !socket_exists {
        scan_port("localhost", 2375, PORT_TIMEOUT_MS).await
    } else {
        false
    };

    if !socket_exists && !tcp_up {
        return None;
    }

    let url = if socket_exists {
        "unix:///var/run/docker.sock".to_string()
    } else {
        "http://localhost:2375".to_string()
    };

    // Try to get Docker version info via HTTP if TCP is available
    let mut metadata = serde_json::json!({ "socket": socket_exists, "tcp": tcp_up });
    if tcp_up {
        if let Some((_status, body)) = http_get("http://localhost:2375/version").await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                metadata = v;
            }
        }
    }

    Some(DiscoveredService {
        id: make_id(&ServiceType::Docker, 2375),
        name: "Docker".into(),
        service_type: ServiceType::Docker,
        url,
        port: 2375,
        status: ServiceStatus::Online,
        capabilities: vec!["containers".into(), "images".into(), "volumes".into(), "networks".into()],
        auto_connected: socket_exists,
        last_check: now_iso(),
        response_time_ms: Some(0),
        metadata,
    })
}

async fn detect_git() -> Option<DiscoveredService> {
    // Check if `git` binary is available and current dir or home has a repo
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .await
        .ok()?;

    let inside = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if inside != "true" {
        return None;
    }

    // Get current branch
    let branch_output = tokio::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .await
        .ok();
    let branch = branch_output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // Get remote URL
    let remote_output = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .await
        .ok();
    let remote = remote_output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    Some(DiscoveredService {
        id: make_id(&ServiceType::Git, 0),
        name: "Git Repository".into(),
        service_type: ServiceType::Git,
        url: if remote.is_empty() { "(local)".into() } else { remote.clone() },
        port: 0,
        status: ServiceStatus::Online,
        capabilities: vec!["version_control".into(), "branches".into(), "commits".into()],
        auto_connected: true,
        last_check: now_iso(),
        response_time_ms: Some(0),
        metadata: serde_json::json!({ "branch": branch, "remote": remote }),
    })
}

async fn detect_n8n() -> Option<DiscoveredService> {
    let port: u16 = 5678;
    let start = Instant::now();

    // n8n exposes a health endpoint or at minimum responds on its web port
    if let Some((status, _body)) = http_get("http://localhost:5678/healthz").await {
        let rtt = start.elapsed().as_millis() as u64;
        if status == 200 || status == 401 {
            return Some(DiscoveredService {
                id: make_id(&ServiceType::N8n, port),
                name: "n8n Workflow Automation".into(),
                service_type: ServiceType::N8n,
                url: "http://localhost:5678".into(),
                port,
                status: if status == 401 { ServiceStatus::AuthRequired } else { ServiceStatus::Online },
                capabilities: vec!["workflows".into(), "webhooks".into(), "integrations".into()],
                auto_connected: status == 200,
                last_check: now_iso(),
                response_time_ms: Some(rtt),
                metadata: serde_json::json!({}),
            });
        }
    }

    // Fallback: just check the port
    if let Some(rtt) = measure_port("localhost", port).await {
        return Some(DiscoveredService {
            id: make_id(&ServiceType::N8n, port),
            name: "n8n Workflow Automation".into(),
            service_type: ServiceType::N8n,
            url: "http://localhost:5678".into(),
            port,
            status: ServiceStatus::Online,
            capabilities: vec!["workflows".into(), "webhooks".into()],
            auto_connected: false,
            last_check: now_iso(),
            response_time_ms: Some(rtt),
            metadata: serde_json::json!({}),
        });
    }

    None
}

async fn detect_claude_code() -> Option<DiscoveredService> {
    // Check if `claude` process is running via a cross-platform approach
    let check = if cfg!(target_os = "windows") {
        tokio::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq claude.exe", "/NH"])
            .output()
            .await
    } else {
        tokio::process::Command::new("pgrep")
            .arg("-x")
            .arg("claude")
            .output()
            .await
    };

    let running = match check {
        Ok(output) => output.status.success(),
        Err(_) => false,
    };

    if !running {
        return None;
    }

    Some(DiscoveredService {
        id: make_id(&ServiceType::ClaudeCode, 0),
        name: "Claude Code".into(),
        service_type: ServiceType::ClaudeCode,
        url: "(local process)".into(),
        port: 0,
        status: ServiceStatus::Online,
        capabilities: vec!["code_generation".into(), "refactoring".into(), "debugging".into()],
        auto_connected: false,
        last_check: now_iso(),
        response_time_ms: Some(0),
        metadata: serde_json::json!({}),
    })
}

/// Scan MCP server ports (8001-8015).  Try GET /health or GET / on each.
async fn detect_mcp_servers() -> Vec<DiscoveredService> {
    let known_names: HashMap<u16, &str> = HashMap::from([
        (8001, "HarmonyLoop"),
        (8002, "Semantic"),
        (8003, "Blender"),
        (8004, "Offline Coding"),
        (8005, "Creative Apps"),
        (8006, "Unity"),
        (8010, "Unlimited Context"),
        (8015, "RLM HTTP"),
    ]);

    let mut handles = Vec::new();

    for port in 8001..=8015 {
        let name_hint = known_names.get(&port).map(|s| s.to_string());
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let open = scan_port("localhost", port, PORT_TIMEOUT_MS).await;
            if !open {
                return None;
            }
            let rtt = start.elapsed().as_millis() as u64;

            // Try health endpoint
            let mut caps = vec!["mcp".into(), "tools".into()];
            let health_url = format!("http://localhost:{port}/health");
            if let Some((status, _body)) = http_get(&health_url).await {
                if status == 200 {
                    caps.push("health_check".into());
                }
            }

            let label = name_hint.unwrap_or_else(|| format!("MCP :{port}"));

            Some(DiscoveredService {
                id: make_id(&ServiceType::McpServer, port),
                name: format!("MCP: {label}"),
                service_type: ServiceType::McpServer,
                url: format!("http://localhost:{port}"),
                port,
                status: ServiceStatus::Online,
                capabilities: caps,
                auto_connected: false,
                last_check: now_iso(),
                response_time_ms: Some(rtt),
                metadata: serde_json::json!({ "label": label }),
            })
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        if let Ok(Some(svc)) = h.await {
            results.push(svc);
        }
    }
    results
}

/// Scan a list of common HTTP service ports for anything responding.
async fn detect_generic_http() -> Vec<DiscoveredService> {
    let ports: Vec<u16> = vec![
        3000, 3001, 4000, 4200, 5000, 5173, 8000, 8080, 8443, 9000, 9090, 9100,
    ];

    let mut handles = Vec::new();
    for port in ports {
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let open = scan_port("localhost", port, PORT_TIMEOUT_MS).await;
            if !open {
                return None;
            }
            let rtt = start.elapsed().as_millis() as u64;

            // Try to determine what it is
            let url = format!("http://localhost:{port}");
            let mut name = format!("HTTP Service :{port}");
            let mut caps: Vec<String> = vec!["http".into()];

            if let Some((status, body)) = http_get(&url).await {
                if status < 500 {
                    caps.push(format!("status_{status}"));
                }
                // Heuristic identification from response body
                let body_lower = body.to_lowercase();
                if body_lower.contains("grafana") {
                    name = format!("Grafana (:{port})");
                    caps.push("monitoring".into());
                } else if body_lower.contains("prometheus") {
                    name = format!("Prometheus (:{port})");
                    caps.push("metrics".into());
                } else if body_lower.contains("comfyui") || body_lower.contains("comfy") {
                    name = format!("ComfyUI (:{port})");
                    caps.push("image_generation".into());
                } else if body_lower.contains("jupyter") {
                    name = format!("Jupyter (:{port})");
                    caps.push("notebooks".into());
                } else if body_lower.contains("vscode") || body_lower.contains("code-server") {
                    name = format!("Code Server (:{port})");
                    caps.push("ide".into());
                }
            }

            Some(DiscoveredService {
                id: make_id(&ServiceType::HttpService, port),
                name,
                service_type: ServiceType::HttpService,
                url,
                port,
                status: ServiceStatus::Online,
                capabilities: caps,
                auto_connected: false,
                last_check: now_iso(),
                response_time_ms: Some(rtt),
                metadata: serde_json::json!({}),
            })
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        if let Ok(Some(svc)) = h.await {
            results.push(svc);
        }
    }
    results
}

/// Scan a single custom endpoint.
async fn check_custom_endpoint(ep: &CustomEndpoint) -> DiscoveredService {
    let full_url = if let Some(ref health) = ep.health_path {
        let base = ep.url.trim_end_matches('/');
        format!("{base}{health}")
    } else {
        ep.url.clone()
    };

    let start = Instant::now();
    let (status_val, rtt) = match http_get(&full_url).await {
        Some((code, _body)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let st = if code == ep.expected_status {
                ServiceStatus::Online
            } else if code == 401 || code == 403 {
                ServiceStatus::AuthRequired
            } else {
                ServiceStatus::Error
            };
            (st, Some(elapsed))
        }
        None => (ServiceStatus::Offline, None),
    };

    DiscoveredService {
        id: format!("custom-{}", ep.name.to_lowercase().replace(' ', "-")),
        name: ep.name.clone(),
        service_type: ServiceType::CustomApi,
        url: ep.url.clone(),
        port: 0,
        status: status_val,
        capabilities: vec!["custom".into()],
        auto_connected: false,
        last_check: now_iso(),
        response_time_ms: rtt,
        metadata: serde_json::json!({
            "health_path": ep.health_path,
            "expected_status": ep.expected_status,
        }),
    }
}

// ---------------------------------------------------------------------------
// Full scan orchestrator
// ---------------------------------------------------------------------------

async fn run_full_scan() -> ScanResult {
    let start = Instant::now();
    let mut services: Vec<DiscoveredService> = Vec::new();

    // Launch all detectors in parallel
    let (ollama, pg5432, pg5433, redis, docker, git, n8n, claude, mcps, generic) = tokio::join!(
        detect_ollama(),
        detect_postgresql(5432),
        detect_postgresql(5433),
        detect_redis(),
        detect_docker(),
        detect_git(),
        detect_n8n(),
        detect_claude_code(),
        detect_mcp_servers(),
        detect_generic_http(),
    );

    if let Some(s) = ollama { services.push(s); }
    if let Some(s) = pg5432 { services.push(s); }
    if let Some(s) = pg5433 { services.push(s); }
    if let Some(s) = redis { services.push(s); }
    if let Some(s) = docker { services.push(s); }
    if let Some(s) = git { services.push(s); }
    if let Some(s) = n8n { services.push(s); }
    if let Some(s) = claude { services.push(s); }
    services.extend(mcps);

    // Filter generic HTTP to avoid duplicating already-detected ports
    let known_ports: Vec<u16> = services.iter().map(|s| s.port).collect();
    for svc in generic {
        if !known_ports.contains(&svc.port) {
            services.push(svc);
        }
    }

    // Scan custom endpoints from config
    let config = load_config();
    for ep in &config.custom_endpoints {
        services.push(check_custom_endpoint(ep).await);
    }

    let duration = start.elapsed().as_millis() as u64;

    let result = ScanResult {
        services,
        scan_duration_ms: duration,
        timestamp: now_iso(),
    };

    save_scan_result(&result);
    result
}

// ---------------------------------------------------------------------------
// Tauri commands (10)
// ---------------------------------------------------------------------------

/// Scan ALL known ports and services, return discovered list.
#[tauri::command]
pub async fn connector_scan() -> Result<ScanResult, String> {
    Ok(run_full_scan().await)
}

/// Return cached list of last discovered services.
#[tauri::command]
pub async fn connector_get_services() -> Result<Vec<DiscoveredService>, String> {
    match load_last_scan() {
        Some(scan) => Ok(scan.services),
        None => Ok(Vec::new()),
    }
}

/// Check a single service health by its id.
#[tauri::command]
pub async fn connector_check_service(id: String) -> Result<DiscoveredService, String> {
    // First check if it is a custom endpoint
    let config = load_config();
    for ep in &config.custom_endpoints {
        let ep_id = format!("custom-{}", ep.name.to_lowercase().replace(' ', "-"));
        if ep_id == id {
            return Ok(check_custom_endpoint(ep).await);
        }
    }

    // Otherwise re-detect based on the id prefix
    let parts: Vec<&str> = id.rsplitn(2, '-').collect();
    let port: u16 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);

    if id.starts_with("ollama") {
        if let Some(s) = detect_ollama().await {
            return Ok(s);
        }
    } else if id.starts_with("postgre") {
        if let Some(s) = detect_postgresql(port).await {
            return Ok(s);
        }
    } else if id.starts_with("redis") {
        if let Some(s) = detect_redis().await {
            return Ok(s);
        }
    } else if id.starts_with("docker") {
        if let Some(s) = detect_docker().await {
            return Ok(s);
        }
    } else if id.starts_with("git") {
        if let Some(s) = detect_git().await {
            return Ok(s);
        }
    } else if id.starts_with("n8n") || id.starts_with("n_8_n") {
        if let Some(s) = detect_n8n().await {
            return Ok(s);
        }
    } else if id.starts_with("claude") {
        if let Some(s) = detect_claude_code().await {
            return Ok(s);
        }
    } else if id.starts_with("mcp") {
        let mcps = detect_mcp_servers().await;
        for s in mcps {
            if s.id == id {
                return Ok(s);
            }
        }
    }

    Err(format!("Service '{id}' not found or is offline"))
}

/// Add a custom service endpoint and test it immediately.
#[tauri::command]
pub async fn connector_add_custom(
    name: String,
    url: String,
    health_path: Option<String>,
) -> Result<DiscoveredService, String> {
    let ep = CustomEndpoint {
        name: name.clone(),
        url: url.clone(),
        health_path,
        expected_status: 200,
    };

    let mut config = load_config();

    // Prevent duplicates
    let new_id = format!("custom-{}", name.to_lowercase().replace(' ', "-"));
    config.custom_endpoints.retain(|e| {
        format!("custom-{}", e.name.to_lowercase().replace(' ', "-")) != new_id
    });

    config.custom_endpoints.push(ep.clone());
    save_config_to_disk(&config);

    Ok(check_custom_endpoint(&ep).await)
}

/// Remove a custom service endpoint by its id.
#[tauri::command]
pub async fn connector_remove_custom(id: String) -> Result<(), String> {
    let mut config = load_config();
    let before = config.custom_endpoints.len();
    config.custom_endpoints.retain(|e| {
        let ep_id = format!("custom-{}", e.name.to_lowercase().replace(' ', "-"));
        ep_id != id
    });
    if config.custom_endpoints.len() == before {
        return Err(format!("Custom endpoint '{id}' not found"));
    }
    save_config_to_disk(&config);
    Ok(())
}

/// Get the connector configuration.
#[tauri::command]
pub async fn connector_get_config() -> Result<ConnectorConfig, String> {
    Ok(load_config())
}

/// Save updated connector configuration.
#[tauri::command]
pub async fn connector_save_config(config: ConnectorConfig) -> Result<(), String> {
    save_config_to_disk(&config);
    Ok(())
}

/// Get capabilities for a specific discovered service.
#[tauri::command]
pub async fn connector_get_capabilities(service_id: String) -> Result<Vec<String>, String> {
    // Try the last scan first
    if let Some(scan) = load_last_scan() {
        for svc in &scan.services {
            if svc.id == service_id {
                return Ok(svc.capabilities.clone());
            }
        }
    }

    // If not found in cache, do a live check
    match connector_check_service(service_id.clone()).await {
        Ok(svc) => Ok(svc.capabilities),
        Err(_) => Err(format!("Service '{service_id}' not found")),
    }
}

/// Attempt to auto-connect and configure a service.
#[tauri::command]
pub async fn connector_auto_connect(service_id: String) -> Result<bool, String> {
    let svc = connector_check_service(service_id).await?;

    // For now auto-connect means verifying the service is online.
    // Future: write connection details into ImpForge settings.
    match svc.status {
        ServiceStatus::Online => Ok(true),
        ServiceStatus::AuthRequired => {
            Err("Service requires authentication — configure credentials in Settings".into())
        }
        _ => Ok(false),
    }
}

/// Return the last N scan results for monitoring.
#[tauri::command]
pub async fn connector_scan_history() -> Result<Vec<ScanResult>, String> {
    Ok(load_history())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_id() {
        assert_eq!(make_id(&ServiceType::Ollama, 11434), "ollama-11434");
        assert_eq!(make_id(&ServiceType::PostgreSql, 5433), "postgre_sql-5433");
    }

    #[test]
    fn test_default_config() {
        let config = ConnectorConfig::default();
        assert!(config.auto_scan_on_start);
        assert_eq!(config.scan_interval_seconds, 0);
        assert!(config.custom_endpoints.is_empty());
    }

    #[test]
    fn test_service_type_label() {
        assert_eq!(ServiceType::Ollama.label(), "Ollama");
        assert_eq!(ServiceType::McpServer.label(), "MCP Server");
        assert_eq!(ServiceType::ClaudeCode.label(), "Claude Code");
    }

    #[test]
    fn test_service_serialization() {
        let svc = DiscoveredService {
            id: "ollama-11434".into(),
            name: "Ollama".into(),
            service_type: ServiceType::Ollama,
            url: "http://localhost:11434".into(),
            port: 11434,
            status: ServiceStatus::Online,
            capabilities: vec!["chat".into()],
            auto_connected: true,
            last_check: "2026-03-18T00:00:00Z".into(),
            response_time_ms: Some(5),
            metadata: serde_json::json!({}),
        };

        let json = serde_json::to_string(&svc).expect("serialize");
        assert!(json.contains("\"ollama\""));
        assert!(json.contains("\"online\""));
    }

    #[test]
    fn test_config_serialization() {
        let config = ConnectorConfig {
            auto_scan_on_start: true,
            scan_interval_seconds: 60,
            custom_endpoints: vec![CustomEndpoint {
                name: "My API".into(),
                url: "http://localhost:9999".into(),
                health_path: Some("/health".into()),
                expected_status: 200,
            }],
            notification_on_change: true,
        };

        let json = serde_json::to_string_pretty(&config).expect("serialize");
        let restored: ConnectorConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.scan_interval_seconds, 60);
        assert_eq!(restored.custom_endpoints.len(), 1);
    }

    #[test]
    fn test_scan_result_serialization() {
        let result = ScanResult {
            services: vec![],
            scan_duration_ms: 42,
            timestamp: "2026-03-18T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("42"));
    }

    #[tokio::test]
    async fn test_scan_port_closed() {
        // Port 1 is virtually always closed
        let open = scan_port("localhost", 1, 20).await;
        assert!(!open);
    }
}
