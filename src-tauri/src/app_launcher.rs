// SPDX-License-Identifier: Apache-2.0
//! App Launcher — Self-Extending Application Registry
//!
//! A universal app launcher that lets users add external programs, websites,
//! and API services as launchable modules. Entries persist in a JSON store
//! via `tauri-plugin-store` and native processes are tracked in memory so
//! `app_health` can report whether they are still running.
//!
//! ## Supported app types
//!
//! | Type            | Launch behaviour                                  |
//! |-----------------|---------------------------------------------------|
//! | NativeProcess   | Spawns via `std::process::Command`, tracked by PID|
//! | WebView         | Returns URL; frontend opens an embedded webview   |
//! | WebService      | Health-checks the endpoint, returns status         |
//! | McpServer       | Spawns a process with MCP transport metadata       |
//!
//! ## Desktop file discovery (Linux)
//!
//! `app_discover_installed` scans `~/.local/share/applications` and
//! `/usr/share/applications` for `.desktop` files and extracts `Name=`,
//! `Exec=`, and `Icon=` fields so the user can quickly import system apps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ════════════════════════════════════════════════════════════════════════════

/// Store file name — lives inside the Tauri app-data directory.
const STORE_FILE: &str = ".impforge-apps.json";

/// Store key under which the `Vec<AppEntry>` is persisted.
const STORE_KEY: &str = "apps";

// ════════════════════════════════════════════════════════════════════════════
// DATA MODEL
// ════════════════════════════════════════════════════════════════════════════

/// A registered application / service in the launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub app_type: AppType,
    pub icon: Option<String>,
    pub category: String,
    pub pinned: bool,
    pub launch_config: LaunchConfig,
    pub usage_count: u64,
    pub last_used: Option<String>,
    pub created_at: String,
}

/// Discriminated union for the four supported app types.
/// `#[serde(tag = "type")]` produces `{ "type": "NativeProcess", ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppType {
    NativeProcess {
        executable: String,
        args: Vec<String>,
        cwd: Option<String>,
    },
    WebView {
        url: String,
        inject_css: Option<String>,
    },
    WebService {
        api_url: String,
        health_endpoint: Option<String>,
    },
    McpServer {
        command: String,
        args: Vec<String>,
        transport: String,
    },
}

/// Per-entry launch configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub auto_start: bool,
    pub show_in_sidebar: bool,
    pub monitoring_enabled: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            auto_start: false,
            show_in_sidebar: true,
            monitoring_enabled: true,
        }
    }
}

/// A discovered system application (from .desktop files).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredApp {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub desktop_file: String,
    pub categories: Option<String>,
}

// ════════════════════════════════════════════════════════════════════════════
// PROCESS TRACKER
// ════════════════════════════════════════════════════════════════════════════

/// Wrapper around `std::process::Child` that is safe to share across threads.
///
/// `Child` itself is `Send` but not `Sync`; wrapping it in `Mutex` satisfies
/// the `Sync` requirement for the `Lazy<Mutex<HashMap<_, _>>>` static.
struct TrackedChild {
    inner: std::process::Child,
}

// SAFETY: `Child` is safe to send between threads (it is just a PID handle).
// We guard all access behind `Mutex` so only one thread touches it at a time.
unsafe impl Send for TrackedChild {}

impl TrackedChild {
    fn new(child: std::process::Child) -> Self {
        Self { inner: child }
    }

    fn id(&self) -> u32 {
        self.inner.id()
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.inner.try_wait()
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.inner.kill()
    }
}

/// Global map of launched native processes, keyed by app entry ID.
/// We store `TrackedChild` handles so we can query `try_wait()` for liveness.
static RUNNING_PROCESSES: Lazy<Mutex<HashMap<String, TrackedChild>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ════════════════════════════════════════════════════════════════════════════
// STORE HELPERS
// ════════════════════════════════════════════════════════════════════════════

/// Load the full app list from the JSON store.
fn load_entries(app: &tauri::AppHandle) -> AppResult<Vec<AppEntry>> {
    let store = app.store(STORE_FILE).map_err(|e| {
        ImpForgeError::config("STORE_OPEN", format!("Cannot open app store: {e}"))
            .with_suggestion("The app data directory may be read-only. Check file permissions.")
    })?;

    let entries: Vec<AppEntry> = store
        .get(STORE_KEY)
        .map(|val| serde_json::from_value(val.clone()).unwrap_or_default())
        .unwrap_or_default();

    Ok(entries)
}

/// Persist the full app list back to the JSON store.
fn save_entries(app: &tauri::AppHandle, entries: &[AppEntry]) -> AppResult<()> {
    let store = app.store(STORE_FILE).map_err(|e| {
        ImpForgeError::config("STORE_OPEN", format!("Cannot open app store: {e}"))
    })?;

    let value = serde_json::to_value(entries).map_err(|e| {
        ImpForgeError::internal("SERIALIZE", format!("Failed to serialize app entries: {e}"))
    })?;

    store.set(STORE_KEY, value);
    store.save().map_err(|e| {
        ImpForgeError::filesystem("STORE_SAVE", format!("Failed to write app store: {e}"))
            .with_suggestion("Check disk space and file permissions for the ImpForge data directory.")
    })?;

    Ok(())
}

/// Find a single entry by ID, returning its index and a clone.
fn find_entry(entries: &[AppEntry], id: &str) -> AppResult<(usize, AppEntry)> {
    entries
        .iter()
        .enumerate()
        .find(|(_, e)| e.id == id)
        .map(|(i, e)| (i, e.clone()))
        .ok_or_else(|| {
            ImpForgeError::validation("APP_NOT_FOUND", format!("No app with id '{id}'"))
                .with_suggestion("The app may have been removed. Refresh the launcher list.")
        })
}

// ════════════════════════════════════════════════════════════════════════════
// TAURI COMMANDS
// ════════════════════════════════════════════════════════════════════════════

/// List all registered applications.
#[tauri::command]
pub async fn app_list(app: tauri::AppHandle) -> AppResult<Vec<AppEntry>> {
    load_entries(&app)
}

/// Register a new application entry.
#[tauri::command]
pub async fn app_add(
    app: tauri::AppHandle,
    name: String,
    app_type: AppType,
    category: String,
    icon: Option<String>,
) -> AppResult<AppEntry> {
    if name.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_NAME",
            "App name must not be empty",
        ));
    }

    let entry = AppEntry {
        id: Uuid::new_v4().to_string(),
        name,
        app_type,
        icon,
        category,
        pinned: false,
        launch_config: LaunchConfig::default(),
        usage_count: 0,
        last_used: None,
        created_at: Utc::now().to_rfc3339(),
    };

    let mut entries = load_entries(&app)?;
    entries.push(entry.clone());
    save_entries(&app, &entries)?;

    log::info!("App registered: {} ({})", entry.name, entry.id);
    Ok(entry)
}

/// Remove an application entry and kill its process if running.
#[tauri::command]
pub async fn app_remove(app: tauri::AppHandle, id: String) -> AppResult<()> {
    let mut entries = load_entries(&app)?;
    let original_len = entries.len();
    entries.retain(|e| e.id != id);

    if entries.len() == original_len {
        return Err(ImpForgeError::validation(
            "APP_NOT_FOUND",
            format!("No app with id '{id}'"),
        ));
    }

    save_entries(&app, &entries)?;

    // Kill the process if it is tracked
    if let Ok(mut procs) = RUNNING_PROCESSES.lock() {
        if let Some(mut child) = procs.remove(&id) {
            let _ = child.kill();
            log::info!("Killed tracked process for removed app {id}");
        }
    }

    log::info!("App removed: {id}");
    Ok(())
}

/// Update fields on an existing app entry.
///
/// Accepts a partial JSON object — only present keys are merged.
/// Supported keys: `name`, `icon`, `category`, `pinned`, `launch_config`, `app_type`.
#[tauri::command]
pub async fn app_update(
    app: tauri::AppHandle,
    id: String,
    updates: serde_json::Value,
) -> AppResult<AppEntry> {
    let mut entries = load_entries(&app)?;
    let (idx, _) = find_entry(&entries, &id)?;

    let entry = &mut entries[idx];

    if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
        if name.trim().is_empty() {
            return Err(ImpForgeError::validation(
                "EMPTY_NAME",
                "App name must not be empty",
            ));
        }
        entry.name = name.to_string();
    }

    if let Some(icon) = updates.get("icon") {
        entry.icon = icon.as_str().map(|s| s.to_string());
    }

    if let Some(category) = updates.get("category").and_then(|v| v.as_str()) {
        entry.category = category.to_string();
    }

    if let Some(pinned) = updates.get("pinned").and_then(|v| v.as_bool()) {
        entry.pinned = pinned;
    }

    if let Some(lc) = updates.get("launch_config") {
        if let Ok(config) = serde_json::from_value::<LaunchConfig>(lc.clone()) {
            entry.launch_config = config;
        }
    }

    if let Some(at) = updates.get("app_type") {
        if let Ok(app_type) = serde_json::from_value::<AppType>(at.clone()) {
            entry.app_type = app_type;
        }
    }

    let updated = entry.clone();
    save_entries(&app, &entries)?;

    log::info!("App updated: {} ({})", updated.name, updated.id);
    Ok(updated)
}

/// Launch an application.
///
/// - **NativeProcess / McpServer**: spawns via `std::process::Command` and
///   tracks the `Child` handle for later health queries.
/// - **WebView**: returns the URL so the frontend can open an embedded view.
/// - **WebService**: performs a health check and returns status JSON.
#[tauri::command]
pub async fn app_launch(app: tauri::AppHandle, id: String) -> AppResult<String> {
    let mut entries = load_entries(&app)?;
    let (idx, entry) = find_entry(&entries, &id)?;

    let result = match &entry.app_type {
        AppType::NativeProcess {
            executable,
            args,
            cwd,
        } => launch_native(&id, executable, args, cwd.as_deref())?,

        AppType::McpServer {
            command,
            args,
            transport,
        } => {
            let status = launch_native(&id, command, args, None)?;
            format!("{status} (transport: {transport})")
        }

        AppType::WebView { url, .. } => {
            format!("webview:{url}")
        }

        AppType::WebService {
            api_url,
            health_endpoint,
        } => {
            let check_url = health_endpoint
                .as_deref()
                .unwrap_or(api_url.as_str());
            check_web_health(check_url).await?
        }
    };

    // Update usage statistics
    entries[idx].usage_count += 1;
    entries[idx].last_used = Some(Utc::now().to_rfc3339());
    save_entries(&app, &entries)?;

    Ok(result)
}

/// Toggle the pinned state of an app entry.
#[tauri::command]
pub async fn app_pin(app: tauri::AppHandle, id: String, pinned: bool) -> AppResult<()> {
    let mut entries = load_entries(&app)?;
    let (idx, _) = find_entry(&entries, &id)?;

    entries[idx].pinned = pinned;
    save_entries(&app, &entries)?;

    log::info!("App {} pinned={pinned}", entries[idx].name);
    Ok(())
}

/// Check whether an app is currently alive.
///
/// - **NativeProcess / McpServer**: checks if the tracked `Child` is still running.
/// - **WebView**: always reports `{ "status": "webview" }`.
/// - **WebService**: performs an HTTP health check.
#[tauri::command]
pub async fn app_health(app: tauri::AppHandle, id: String) -> AppResult<serde_json::Value> {
    let entries = load_entries(&app)?;
    let (_, entry) = find_entry(&entries, &id)?;

    match &entry.app_type {
        AppType::NativeProcess { .. } | AppType::McpServer { .. } => {
            let (running, pid) = check_process_alive(&id);
            Ok(serde_json::json!({
                "status": if running { "running" } else { "stopped" },
                "pid": pid,
                "type": "process",
            }))
        }
        AppType::WebView { url, .. } => Ok(serde_json::json!({
            "status": "webview",
            "url": url,
            "type": "webview",
        })),
        AppType::WebService {
            api_url,
            health_endpoint,
        } => {
            let check_url = health_endpoint.as_deref().unwrap_or(api_url.as_str());
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .map_err(|e| {
                    ImpForgeError::service("HTTP_CLIENT", format!("Cannot build HTTP client: {e}"))
                })?;

            match client.get(check_url).send().await {
                Ok(resp) => Ok(serde_json::json!({
                    "status": if resp.status().is_success() { "healthy" } else { "unhealthy" },
                    "http_status": resp.status().as_u16(),
                    "type": "web_service",
                    "url": check_url,
                })),
                Err(e) => Ok(serde_json::json!({
                    "status": "unreachable",
                    "error": e.to_string(),
                    "type": "web_service",
                    "url": check_url,
                })),
            }
        }
    }
}

/// Discover installed applications by scanning `.desktop` files on Linux.
///
/// On non-Linux platforms this returns an empty list (cross-platform safe).
#[tauri::command]
pub async fn app_discover_installed() -> AppResult<Vec<DiscoveredApp>> {
    #[cfg(target_os = "linux")]
    {
        discover_desktop_files()
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(Vec::new())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INTERNAL HELPERS
// ════════════════════════════════════════════════════════════════════════════

/// Spawn a native process and track it in `RUNNING_PROCESSES`.
fn launch_native(
    id: &str,
    executable: &str,
    args: &[String],
    cwd: Option<&str>,
) -> AppResult<String> {
    let resolved = resolve_executable(executable);

    let mut cmd = std::process::Command::new(&resolved);
    cmd.args(args);

    if let Some(dir) = cwd {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            cmd.current_dir(p);
        } else {
            log::warn!("Working directory does not exist, using default: {dir}");
        }
    }

    // Detach stdin/stdout/stderr so the child does not block the Tauri event loop.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd.spawn().map_err(|e| {
        ImpForgeError::service(
            "SPAWN_FAILED",
            format!("Failed to start '{resolved}': {e}"),
        )
        .with_suggestion(
            "Check that the executable exists and is in your PATH. \
             On Linux you may need to install the package first.",
        )
    })?;

    let pid = child.id();

    if let Ok(mut procs) = RUNNING_PROCESSES.lock() {
        procs.insert(id.to_string(), TrackedChild::new(child));
    }

    log::info!("Launched '{resolved}' (pid {pid}) for app {id}");
    Ok(format!("started:pid={pid}"))
}

/// Try to resolve a bare command name to its full path via `which`.
/// Falls back to the original string if resolution fails (e.g. Windows, or
/// the binary is a full path already).
fn resolve_executable(name: &str) -> String {
    // If it looks like an absolute or relative path, use as-is
    if name.contains('/') || name.contains('\\') {
        return name.to_string();
    }

    // Try `which` on Unix
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("which")
            .arg(name)
            .output()
        {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    let trimmed = path.trim().to_string();
                    if !trimmed.is_empty() {
                        return trimmed;
                    }
                }
            }
        }
    }

    name.to_string()
}

/// Check whether a tracked native process is still alive.
/// Returns `(is_running, optional_pid)`.
fn check_process_alive(id: &str) -> (bool, Option<u32>) {
    let mut procs = match RUNNING_PROCESSES.lock() {
        Ok(p) => p,
        Err(_) => return (false, None),
    };

    if let Some(child) = procs.get_mut(id) {
        let pid = child.id();
        match child.try_wait() {
            // Process has exited — clean up
            Ok(Some(_status)) => {
                procs.remove(id);
                (false, Some(pid))
            }
            // Still running
            Ok(None) => (true, Some(pid)),
            // Error querying — assume dead
            Err(_) => {
                procs.remove(id);
                (false, Some(pid))
            }
        }
    } else {
        (false, None)
    }
}

/// Perform an HTTP GET health check with a short timeout.
async fn check_web_health(url: &str) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| {
            ImpForgeError::service("HTTP_CLIENT", format!("Cannot build HTTP client: {e}"))
        })?;

    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            Ok(format!("healthy:status={}", resp.status().as_u16()))
        }
        Ok(resp) => Ok(format!("unhealthy:status={}", resp.status().as_u16())),
        Err(e) => Err(
            ImpForgeError::service("HEALTH_CHECK", format!("Cannot reach {url}: {e}"))
                .with_suggestion("Check that the service is running and the URL is correct."),
        ),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// LINUX DESKTOP FILE DISCOVERY
// ════════════════════════════════════════════════════════════════════════════

/// Scan standard Linux `.desktop` file directories.
#[cfg(target_os = "linux")]
fn discover_desktop_files() -> AppResult<Vec<DiscoveredApp>> {
    let mut apps = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Standard XDG directories
    let mut dirs_to_scan: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    // User-local directory
    if let Some(home) = dirs::home_dir() {
        dirs_to_scan.push(home.join(".local/share/applications"));
    }

    // XDG_DATA_DIRS (Flatpak, Snap, etc.)
    if let Ok(xdg_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg_dirs.split(':') {
            let p = PathBuf::from(dir).join("applications");
            if !dirs_to_scan.contains(&p) {
                dirs_to_scan.push(p);
            }
        }
    }

    for dir in &dirs_to_scan {
        if !dir.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }

            if let Some(discovered) = parse_desktop_file(&path) {
                // Deduplicate by name (user-local overrides system)
                if seen_names.insert(discovered.name.clone()) {
                    apps.push(discovered);
                }
            }
        }
    }

    // Sort alphabetically for a predictable UX
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    log::info!("Discovered {} installed applications", apps.len());
    Ok(apps)
}

/// Parse a single `.desktop` file and extract Name, Exec, Icon, Categories.
///
/// Follows the [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
/// with pragmatic simplifications:
/// - Only reads `[Desktop Entry]` section.
/// - Skips entries with `NoDisplay=true` or `Hidden=true`.
/// - Strips `%f`, `%F`, `%u`, `%U` field codes from `Exec` lines.
#[cfg(target_os = "linux")]
fn parse_desktop_file(path: &Path) -> Option<DiscoveredApp> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut categories: Option<String> = None;
    let mut in_desktop_section = false;
    let mut no_display = false;
    let mut hidden = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Section headers
        if trimmed.starts_with('[') {
            in_desktop_section = trimmed == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_section {
            continue;
        }

        // Skip comments
        if trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                // Use non-localised Name only (Name[xx] would have brackets)
                "Name" => name = Some(value.to_string()),
                "Exec" => {
                    // Strip field codes (%f %F %u %U %d %D %n %N %i %c %k %v %m)
                    let cleaned = value
                        .split_whitespace()
                        .filter(|tok| !(tok.starts_with('%') && tok.len() == 2))
                        .collect::<Vec<_>>()
                        .join(" ");
                    exec = Some(cleaned);
                }
                "Icon" => icon = Some(value.to_string()),
                "Categories" => categories = Some(value.to_string()),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }

    // Skip non-visible entries
    if no_display || hidden {
        return None;
    }

    // Both Name and Exec are mandatory
    let app_name = name?;
    let app_exec = exec?;

    if app_name.is_empty() || app_exec.is_empty() {
        return None;
    }

    Some(DiscoveredApp {
        name: app_name,
        exec: app_exec,
        icon,
        desktop_file: path.to_string_lossy().to_string(),
        categories,
    })
}

// ════════════════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_entry_serialization() {
        let entry = AppEntry {
            id: "test-123".to_string(),
            name: "Firefox".to_string(),
            app_type: AppType::NativeProcess {
                executable: "firefox".to_string(),
                args: vec!["--new-window".to_string()],
                cwd: None,
            },
            icon: Some("firefox".to_string()),
            category: "Browser".to_string(),
            pinned: true,
            launch_config: LaunchConfig::default(),
            usage_count: 5,
            last_used: Some("2026-03-15T12:00:00Z".to_string()),
            created_at: "2026-03-15T10:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("NativeProcess"));
        assert!(json.contains("firefox"));

        let parsed: AppEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.name, "Firefox");
        assert_eq!(parsed.usage_count, 5);
        assert!(parsed.pinned);
    }

    #[test]
    fn test_webview_type_serialization() {
        let entry = AppEntry {
            id: "wv-1".to_string(),
            name: "Grafana".to_string(),
            app_type: AppType::WebView {
                url: "http://localhost:3000".to_string(),
                inject_css: Some("body { font-size: 14px; }".to_string()),
            },
            icon: None,
            category: "Monitoring".to_string(),
            pinned: false,
            launch_config: LaunchConfig::default(),
            usage_count: 0,
            last_used: None,
            created_at: "2026-03-15T10:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("WebView"));
        assert!(json.contains("inject_css"));

        let parsed: AppEntry = serde_json::from_str(&json).expect("deserialize");
        match &parsed.app_type {
            AppType::WebView { url, inject_css } => {
                assert_eq!(url, "http://localhost:3000");
                assert!(inject_css.is_some());
            }
            _ => panic!("Expected WebView"),
        }
    }

    #[test]
    fn test_web_service_type_serialization() {
        let entry = AppEntry {
            id: "ws-1".to_string(),
            name: "Ollama".to_string(),
            app_type: AppType::WebService {
                api_url: "http://localhost:11434".to_string(),
                health_endpoint: Some("http://localhost:11434/api/tags".to_string()),
            },
            icon: Some("brain".to_string()),
            category: "AI".to_string(),
            pinned: true,
            launch_config: LaunchConfig {
                auto_start: true,
                show_in_sidebar: true,
                monitoring_enabled: true,
            },
            usage_count: 42,
            last_used: Some("2026-03-15T14:30:00Z".to_string()),
            created_at: "2026-03-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("WebService"));
        assert!(json.contains("health_endpoint"));

        let parsed: AppEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.usage_count, 42);
    }

    #[test]
    fn test_mcp_server_type_serialization() {
        let entry = AppEntry {
            id: "mcp-1".to_string(),
            name: "Semantic MCP".to_string(),
            app_type: AppType::McpServer {
                command: "python".to_string(),
                args: vec!["-m".to_string(), "ork_semantic".to_string()],
                transport: "stdio".to_string(),
            },
            icon: Some("server".to_string()),
            category: "MCP".to_string(),
            pinned: false,
            launch_config: LaunchConfig::default(),
            usage_count: 0,
            last_used: None,
            created_at: "2026-03-15T10:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("McpServer"));
        assert!(json.contains("stdio"));
    }

    #[test]
    fn test_launch_config_defaults() {
        let config = LaunchConfig::default();
        assert!(!config.auto_start);
        assert!(config.show_in_sidebar);
        assert!(config.monitoring_enabled);
    }

    #[test]
    fn test_resolve_executable_absolute_path() {
        let result = resolve_executable("/usr/bin/firefox");
        assert_eq!(result, "/usr/bin/firefox");
    }

    #[test]
    fn test_resolve_executable_relative_path() {
        let result = resolve_executable("./my-app");
        assert_eq!(result, "./my-app");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_desktop_file_basic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("test.desktop");

        std::fs::write(
            &file_path,
            "[Desktop Entry]\n\
             Name=Test App\n\
             Exec=/usr/bin/test-app --flag %u\n\
             Icon=test-icon\n\
             Categories=Utility;\n\
             Type=Application\n",
        )
        .expect("write");

        let result = parse_desktop_file(&file_path);
        assert!(result.is_some());

        let app = result.expect("parsed");
        assert_eq!(app.name, "Test App");
        // %u field code should be stripped
        assert_eq!(app.exec, "/usr/bin/test-app --flag");
        assert_eq!(app.icon.as_deref(), Some("test-icon"));
        assert_eq!(app.categories.as_deref(), Some("Utility;"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_desktop_file_no_display() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("hidden.desktop");

        std::fs::write(
            &file_path,
            "[Desktop Entry]\n\
             Name=Hidden App\n\
             Exec=/usr/bin/hidden\n\
             NoDisplay=true\n",
        )
        .expect("write");

        let result = parse_desktop_file(&file_path);
        assert!(result.is_none(), "NoDisplay=true entries should be skipped");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_desktop_file_hidden() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("hidden2.desktop");

        std::fs::write(
            &file_path,
            "[Desktop Entry]\n\
             Name=Also Hidden\n\
             Exec=/usr/bin/hidden2\n\
             Hidden=true\n",
        )
        .expect("write");

        let result = parse_desktop_file(&file_path);
        assert!(result.is_none(), "Hidden=true entries should be skipped");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_desktop_file_missing_exec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("noexec.desktop");

        std::fs::write(
            &file_path,
            "[Desktop Entry]\n\
             Name=No Exec App\n\
             Icon=noexec-icon\n",
        )
        .expect("write");

        let result = parse_desktop_file(&file_path);
        assert!(result.is_none(), "Entries without Exec should be skipped");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_desktop_file_only_desktop_section() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("sections.desktop");

        std::fs::write(
            &file_path,
            "[Desktop Entry]\n\
             Name=Real App\n\
             Exec=/usr/bin/real\n\
             \n\
             [Desktop Action New]\n\
             Name=New Window\n\
             Exec=/usr/bin/real --new-window\n",
        )
        .expect("write");

        let result = parse_desktop_file(&file_path);
        assert!(result.is_some());

        let app = result.expect("parsed");
        assert_eq!(app.name, "Real App");
        assert_eq!(app.exec, "/usr/bin/real");
    }

    #[test]
    fn test_discovered_app_serialization() {
        let app = DiscoveredApp {
            name: "Visual Studio Code".to_string(),
            exec: "/usr/bin/code".to_string(),
            icon: Some("vscode".to_string()),
            desktop_file: "/usr/share/applications/code.desktop".to_string(),
            categories: Some("Development;IDE;".to_string()),
        };

        let json = serde_json::to_string(&app).expect("serialize");
        assert!(json.contains("Visual Studio Code"));
        assert!(json.contains("/usr/bin/code"));

        let parsed: DiscoveredApp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.name, "Visual Studio Code");
    }

    #[test]
    fn test_check_process_alive_unknown_id() {
        let (running, pid) = check_process_alive("nonexistent-id-12345");
        assert!(!running);
        assert!(pid.is_none());
    }

    #[test]
    fn test_all_app_types_deserialize() {
        let native = r#"{"type":"NativeProcess","executable":"ls","args":["-la"],"cwd":null}"#;
        let wv = r#"{"type":"WebView","url":"https://example.com","inject_css":null}"#;
        let ws = r#"{"type":"WebService","api_url":"http://localhost:8080","health_endpoint":null}"#;
        let mcp = r#"{"type":"McpServer","command":"node","args":["server.js"],"transport":"stdio"}"#;

        assert!(serde_json::from_str::<AppType>(native).is_ok());
        assert!(serde_json::from_str::<AppType>(wv).is_ok());
        assert!(serde_json::from_str::<AppType>(ws).is_ok());
        assert!(serde_json::from_str::<AppType>(mcp).is_ok());
    }
}
