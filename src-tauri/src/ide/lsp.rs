// SPDX-License-Identifier: Apache-2.0
//! LSP (Language Server Protocol) Backend for ImpForge CodeForge IDE
//!
//! Manages multiple LSP server processes (rust-analyzer, pyright, etc.),
//! communicates via JSON-RPC over stdin/stdout, tracks diagnostics per file,
//! and exposes Tauri IPC commands to the Svelte 5 frontend.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐   Tauri IPC   ┌────────────┐  JSON-RPC/stdio  ┌──────────────────┐
//! │  Svelte 5   │ ────────────> │ LspManager │ ───────────────> │ rust-analyzer    │
//! │  Frontend   │ <──────────── │  (Rust)    │ <─────────────── │ pyright          │
//! └─────────────┘               └────────────┘                  │ tsserver, etc.   │
//!                                                                └──────────────────┘
//! ```
//!
//! ## Capabilities
//!
//! - Multi-language: Rust, Python, TypeScript, JavaScript, Go, C/C++, Svelte
//! - Diagnostics tracking with frontend-ready serialization
//! - Hover, completion, go-to-definition requests
//! - File open/change notifications
//! - Graceful shutdown with process cleanup

use lsp_types::{
    ClientCapabilities, CompletionOptions, DiagnosticSeverity, GotoCapability,
    HoverClientCapabilities, InitializeParams, InitializedParams, MarkupKind,
    NumberOrString, Position, PublishDiagnosticsParams, TextDocumentClientCapabilities,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkspaceFolder,
    notification::Notification,
    request::Request,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Types exposed to the frontend
// ---------------------------------------------------------------------------

/// A single diagnostic message for a file, serializable for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub end_line: u32,
    pub end_character: u32,
    /// One of: "error", "warning", "info", "hint"
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

/// Status of a single running LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerStatus {
    pub language: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub server_binary: String,
    pub workspace_path: String,
}

/// Hover result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspHoverResult {
    pub contents: String,
    pub range: Option<LspRange>,
}

/// A text range (line/character pairs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// A single completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCompletionItem {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub documentation: Option<String>,
}

/// A location result (for go-to-definition).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspLocation {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// A running LSP server instance.
struct LspServer {
    language: String,
    server_binary: String,
    workspace_path: String,
    process: Child,
    writer: BufWriter<ChildStdin>,
    request_id: AtomicU64,
    /// Pending request futures: id -> oneshot sender
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
}

/// Manages all running LSP server instances.
pub struct LspManager {
    servers: Mutex<HashMap<String, LspServer>>,
    diagnostics: Arc<Mutex<HashMap<String, Vec<LspDiagnostic>>>>,
    /// Document versions tracked for didChange incremental versioning
    doc_versions: Arc<Mutex<HashMap<String, i32>>>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
            diagnostics: Arc::new(Mutex::new(HashMap::new())),
            doc_versions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// LSP server binary discovery
// ---------------------------------------------------------------------------

/// Map a language identifier to the expected LSP server binary name.
fn lsp_binary_for_language(language: &str) -> Result<&'static str, String> {
    match language.to_lowercase().as_str() {
        "rust" => Ok("rust-analyzer"),
        "python" => Ok("pyright-langserver"),
        "typescript" | "javascript" | "ts" | "js" => Ok("typescript-language-server"),
        "go" | "golang" => Ok("gopls"),
        "c" | "cpp" | "c++" | "cc" | "cxx" => Ok("clangd"),
        "svelte" => Ok("svelteserver"),
        _ => Err(format!(
            "Unsupported language '{}'. Supported: rust, python, typescript, javascript, go, c, cpp, svelte",
            language
        )),
    }
}

/// Return the LSP language identifier string expected by the protocol.
fn lsp_language_id(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "rust" => "rust",
        "python" => "python",
        "typescript" | "ts" => "typescript",
        "javascript" | "js" => "javascript",
        "go" | "golang" => "go",
        "c" => "c",
        "cpp" | "c++" | "cc" | "cxx" => "cpp",
        "svelte" => "svelte",
        _ => "plaintext",
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

/// Encode a JSON-RPC message with Content-Length header.
fn encode_message(body: &serde_json::Value) -> Vec<u8> {
    let body_str = serde_json::to_string(body).unwrap_or_default();
    let header = format!("Content-Length: {}\r\n\r\n", body_str.len());
    let mut msg = header.into_bytes();
    msg.extend_from_slice(body_str.as_bytes());
    msg
}

/// Build a JSON-RPC request.
fn build_request(id: u64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

/// Build a JSON-RPC notification (no id, no response expected).
fn build_notification(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

/// Convert a file path string to an LSP URI.
fn path_to_uri(file_path: &str) -> Result<Uri, String> {
    let abs = if file_path.starts_with('/') {
        file_path.to_string()
    } else {
        std::env::current_dir()
            .map(|d| d.join(file_path).to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string())
    };
    let uri_str = format!("file://{}", abs);
    uri_str.parse::<Uri>().map_err(|e| format!("Invalid URI: {}", e))
}

/// Convert an LSP URI back to a file path string.
fn uri_to_path(uri: &Uri) -> String {
    let s = uri.as_str();
    if let Some(path) = s.strip_prefix("file://") {
        path.to_string()
    } else {
        s.to_string()
    }
}

/// Convert `DiagnosticSeverity` to a human-readable string.
fn severity_to_string(sev: Option<DiagnosticSeverity>) -> String {
    match sev {
        Some(DiagnosticSeverity::ERROR) => "error".to_string(),
        Some(DiagnosticSeverity::WARNING) => "warning".to_string(),
        Some(DiagnosticSeverity::INFORMATION) => "info".to_string(),
        Some(DiagnosticSeverity::HINT) => "hint".to_string(),
        _ => "info".to_string(),
    }
}

/// Convert a CompletionItemKind number to a readable string.
fn completion_kind_to_string(kind: Option<lsp_types::CompletionItemKind>) -> Option<String> {
    kind.map(|k| {
        match k {
            lsp_types::CompletionItemKind::TEXT => "text",
            lsp_types::CompletionItemKind::METHOD => "method",
            lsp_types::CompletionItemKind::FUNCTION => "function",
            lsp_types::CompletionItemKind::CONSTRUCTOR => "constructor",
            lsp_types::CompletionItemKind::FIELD => "field",
            lsp_types::CompletionItemKind::VARIABLE => "variable",
            lsp_types::CompletionItemKind::CLASS => "class",
            lsp_types::CompletionItemKind::INTERFACE => "interface",
            lsp_types::CompletionItemKind::MODULE => "module",
            lsp_types::CompletionItemKind::PROPERTY => "property",
            lsp_types::CompletionItemKind::UNIT => "unit",
            lsp_types::CompletionItemKind::VALUE => "value",
            lsp_types::CompletionItemKind::ENUM => "enum",
            lsp_types::CompletionItemKind::KEYWORD => "keyword",
            lsp_types::CompletionItemKind::SNIPPET => "snippet",
            lsp_types::CompletionItemKind::COLOR => "color",
            lsp_types::CompletionItemKind::FILE => "file",
            lsp_types::CompletionItemKind::REFERENCE => "reference",
            lsp_types::CompletionItemKind::FOLDER => "folder",
            lsp_types::CompletionItemKind::ENUM_MEMBER => "enum_member",
            lsp_types::CompletionItemKind::CONSTANT => "constant",
            lsp_types::CompletionItemKind::STRUCT => "struct",
            lsp_types::CompletionItemKind::EVENT => "event",
            lsp_types::CompletionItemKind::OPERATOR => "operator",
            lsp_types::CompletionItemKind::TYPE_PARAMETER => "type_parameter",
            _ => "unknown",
        }
        .to_string()
    })
}

/// Extract documentation text from a CompletionItem.
fn extract_documentation(doc: &Option<lsp_types::Documentation>) -> Option<String> {
    match doc {
        Some(lsp_types::Documentation::String(s)) => Some(s.clone()),
        Some(lsp_types::Documentation::MarkupContent(mc)) => Some(mc.value.clone()),
        None => None,
    }
}

// ---------------------------------------------------------------------------
// Stdout reader task — runs in background per LSP server
// ---------------------------------------------------------------------------

/// Spawn a background task that reads JSON-RPC messages from the LSP server's
/// stdout, dispatches responses to pending request futures, and processes
/// notifications (diagnostics, etc.).
fn spawn_reader_task(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    diagnostics: Arc<Mutex<HashMap<String, Vec<LspDiagnostic>>>>,
    language: String,
    app_handle: Option<AppHandle>,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut header_buf = String::new();

        loop {
            // Read headers until empty line
            header_buf.clear();
            let mut content_length: usize = 0;

            loop {
                header_buf.clear();
                match reader.read_line(&mut header_buf).await {
                    Ok(0) => return, // EOF — server exited
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[LSP:{}] Failed to read header: {}", language, e);
                        return;
                    }
                }

                let trimmed = header_buf.trim();
                if trimmed.is_empty() {
                    break; // End of headers
                }

                if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
                    if let Ok(len) = len_str.parse::<usize>() {
                        content_length = len;
                    }
                }
            }

            if content_length == 0 {
                continue;
            }

            // Read exactly content_length bytes
            let mut body_buf = vec![0u8; content_length];
            if let Err(e) = reader.read_exact(&mut body_buf).await {
                log::error!("[LSP:{}] Failed to read body: {}", language, e);
                return;
            }

            let body: serde_json::Value = match serde_json::from_slice(&body_buf) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("[LSP:{}] Invalid JSON from server: {}", language, e);
                    continue;
                }
            };

            // Response (has "id") — dispatch to pending request
            if let Some(id) = body.get("id").and_then(|v| v.as_u64()) {
                let mut pending_guard = pending.lock().await;
                if let Some(sender) = pending_guard.remove(&id) {
                    let _ = sender.send(body);
                }
                continue;
            }

            // Notification (has "method" but no "id")
            if let Some(method) = body.get("method").and_then(|v| v.as_str()) {
                match method {
                    "textDocument/publishDiagnostics" => {
                        if let Some(params) = body.get("params") {
                            handle_diagnostics_notification(
                                params,
                                &diagnostics,
                                &app_handle,
                            )
                            .await;
                        }
                    }
                    "window/logMessage" | "window/showMessage" => {
                        if let Some(msg) = body
                            .get("params")
                            .and_then(|p| p.get("message"))
                            .and_then(|m| m.as_str())
                        {
                            log::debug!("[LSP:{}] {}", language, msg);
                        }
                    }
                    _ => {
                        log::trace!("[LSP:{}] Unhandled notification: {}", language, method);
                    }
                }
            }
        }
    });
}

/// Process a textDocument/publishDiagnostics notification and update storage.
async fn handle_diagnostics_notification(
    params: &serde_json::Value,
    diagnostics: &Arc<Mutex<HashMap<String, Vec<LspDiagnostic>>>>,
    app_handle: &Option<AppHandle>,
) {
    let parsed: Result<PublishDiagnosticsParams, _> = serde_json::from_value(params.clone());
    let publish = match parsed {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[LSP] Failed to parse diagnostics: {}", e);
            return;
        }
    };

    let file_path = uri_to_path(&publish.uri);
    let diags: Vec<LspDiagnostic> = publish
        .diagnostics
        .iter()
        .map(|d| LspDiagnostic {
            file_path: file_path.clone(),
            line: d.range.start.line,
            character: d.range.start.character,
            end_line: d.range.end.line,
            end_character: d.range.end.character,
            severity: severity_to_string(d.severity),
            message: d.message.clone(),
            source: d.source.clone(),
            code: d.code.as_ref().map(|c| match c {
                NumberOrString::Number(n) => n.to_string(),
                NumberOrString::String(s) => s.clone(),
            }),
        })
        .collect();

    let diag_count = diags.len();

    // Update storage
    let mut store = diagnostics.lock().await;
    if diags.is_empty() {
        store.remove(&file_path);
    } else {
        store.insert(file_path.clone(), diags);
    }

    log::debug!(
        "[LSP] Diagnostics updated for {}: {} items",
        file_path,
        diag_count
    );

    // Emit event to frontend so it can reactively update
    if let Some(handle) = app_handle {
        let _ = handle.emit("lsp-diagnostics-updated", &file_path);
    }
}

// ---------------------------------------------------------------------------
// LspServer methods
// ---------------------------------------------------------------------------

impl LspServer {
    /// Send a JSON-RPC request and wait for the response (with timeout).
    async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let msg = build_request(id, method, params);
        let encoded = encode_message(&msg);

        // Register pending response channel
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        // Write to stdin
        self.writer
            .write_all(&encoded)
            .await
            .map_err(|e| format!("Failed to write to LSP stdin: {}", e))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("Failed to flush LSP stdin: {}", e))?;

        // Wait for response with 30-second timeout
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                // Check for JSON-RPC error
                if let Some(err) = response.get("error") {
                    let err_msg = err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown LSP error");
                    Err(format!("LSP error: {}", err_msg))
                } else {
                    Ok(response
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null))
                }
            }
            Ok(Err(_)) => Err("LSP response channel dropped (server may have crashed)".to_string()),
            Err(_) => {
                // Clean up the pending entry on timeout
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err(format!("LSP request '{}' timed out after 30s", method))
            }
        }
    }

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        let msg = build_notification(method, params);
        let encoded = encode_message(&msg);

        self.writer
            .write_all(&encoded)
            .await
            .map_err(|e| format!("Failed to write notification: {}", e))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("Failed to flush notification: {}", e))?;

        Ok(())
    }

    /// Send the LSP `initialize` request with workspace capabilities.
    async fn initialize(&mut self) -> Result<serde_json::Value, String> {
        let workspace_uri = path_to_uri(&self.workspace_path)?;

        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": workspace_uri.to_string(),
            "rootPath": self.workspace_path,
            "capabilities": {
                "textDocument": {
                    "hover": {
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": true,
                            "documentationFormat": ["markdown", "plaintext"],
                            "resolveSupport": {
                                "properties": ["documentation", "detail"]
                            }
                        },
                        "contextSupport": true
                    },
                    "definition": {
                        "dynamicRegistration": false
                    },
                    "publishDiagnostics": {
                        "relatedInformation": true,
                        "tagSupport": {
                            "valueSet": [1, 2]
                        }
                    },
                    "synchronization": {
                        "didSave": true,
                        "willSave": false,
                        "willSaveWaitUntil": false,
                        "dynamicRegistration": false
                    }
                },
                "workspace": {
                    "workspaceFolders": true,
                    "configuration": true
                }
            },
            "workspaceFolders": [{
                "uri": workspace_uri.to_string(),
                "name": std::path::Path::new(&self.workspace_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".to_string()),
            }]
        });

        let result = self.send_request("initialize", init_params).await?;

        // Send `initialized` notification (required by LSP spec)
        self.send_notification("initialized", serde_json::json!({}))
            .await?;

        log::info!(
            "[LSP:{}] Server initialized for {}",
            self.language,
            self.workspace_path
        );

        Ok(result)
    }

    /// Send a graceful shutdown request followed by an exit notification.
    async fn shutdown_graceful(&mut self) {
        // Send shutdown request (server should respond)
        let _ = self
            .send_request("shutdown", serde_json::Value::Null)
            .await;

        // Send exit notification
        let _ = self
            .send_notification("exit", serde_json::Value::Null)
            .await;

        // Give it a moment, then force-kill if still alive
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = self.process.kill().await;
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Start an LSP server for the given language and workspace.
#[tauri::command]
pub async fn lsp_start(
    app: AppHandle,
    language: String,
    workspace_path: String,
) -> Result<LspServerStatus, String> {
    let manager = app.state::<LspManager>();

    let lang_key = language.to_lowercase();
    let binary = lsp_binary_for_language(&lang_key)?;

    // Check if already running
    {
        let servers = manager.servers.lock().await;
        if servers.contains_key(&lang_key) {
            return Err(format!(
                "LSP server for '{}' is already running. Stop it first.",
                lang_key
            ));
        }
    }

    // Validate workspace path exists
    if !std::path::Path::new(&workspace_path).is_dir() {
        return Err(format!(
            "Workspace path does not exist: {}",
            workspace_path
        ));
    }

    // Verify the LSP binary is available on PATH
    let binary_check = tokio::process::Command::new("which")
        .arg(binary)
        .output()
        .await;

    match &binary_check {
        Ok(output) if !output.status.success() => {
            return Err(format!(
                "LSP server binary '{}' not found. Install it first.\n\
                 Hint: cargo install {} / pip install {} / npm install -g {}",
                binary, binary, binary, binary
            ));
        }
        Err(e) => {
            return Err(format!("Failed to check for '{}': {}", binary, e));
        }
        _ => {}
    }

    // Build the command — some servers need extra args
    let mut cmd = Command::new(binary);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(&workspace_path);

    // Server-specific arguments
    match lang_key.as_str() {
        "typescript" | "javascript" | "ts" | "js" => {
            cmd.arg("--stdio");
        }
        "python" => {
            cmd.arg("--stdio");
        }
        "svelte" => {
            cmd.arg("--stdio");
        }
        _ => {
            // rust-analyzer, gopls, clangd default to stdio
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {}", binary, e))?;

    let pid = child.id();

    let stdin = child
        .stdin
        .take()
        .ok_or("Failed to capture LSP server stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture LSP server stdout")?;

    // Log stderr in background (don't block)
    if let Some(stderr) = child.stderr.take() {
        let lang_for_log = lang_key.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            log::debug!("[LSP:{}:stderr] {}", lang_for_log, trimmed);
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Spawn the stdout reader task
    spawn_reader_task(
        stdout,
        Arc::clone(&pending),
        Arc::clone(&manager.diagnostics),
        lang_key.clone(),
        Some(app.clone()),
    );

    let mut server = LspServer {
        language: lang_key.clone(),
        server_binary: binary.to_string(),
        workspace_path: workspace_path.clone(),
        process: child,
        writer: BufWriter::new(stdin),
        request_id: AtomicU64::new(1),
        pending,
    };

    // Send initialize + initialized
    server.initialize().await?;

    let status = LspServerStatus {
        language: lang_key.clone(),
        running: true,
        pid,
        server_binary: binary.to_string(),
        workspace_path,
    };

    // Store the server
    {
        let mut servers = manager.servers.lock().await;
        servers.insert(lang_key, server);
    }

    log::info!("[LSP] Started '{}' (pid: {:?})", binary, pid);
    Ok(status)
}

/// Stop an LSP server for the given language.
#[tauri::command]
pub async fn lsp_stop(app: AppHandle, language: String) -> Result<String, String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();

    let mut server = {
        let mut servers = manager.servers.lock().await;
        servers
            .remove(&lang_key)
            .ok_or(format!("No LSP server running for '{}'", lang_key))?
    };

    server.shutdown_graceful().await;

    // Clear diagnostics associated with this language's workspace
    // (We keep per-file diagnostics, so we cannot easily know which files
    // belong to which server. Clear all for now — a more granular approach
    // would track file->language mappings.)
    log::info!("[LSP] Stopped server for '{}'", lang_key);
    Ok(format!("LSP server for '{}' stopped", lang_key))
}

/// Get diagnostics for a specific file.
#[tauri::command]
pub async fn lsp_diagnostics(
    app: AppHandle,
    file_path: String,
) -> Result<Vec<LspDiagnostic>, String> {
    let manager = app.state::<LspManager>();
    let store = manager.diagnostics.lock().await;
    Ok(store.get(&file_path).cloned().unwrap_or_default())
}

/// Get hover information at a specific position.
#[tauri::command]
pub async fn lsp_hover(
    app: AppHandle,
    file_path: String,
    line: u32,
    character: u32,
    language: String,
) -> Result<Option<LspHoverResult>, String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();

    let mut servers = manager.servers.lock().await;
    let server = servers
        .get_mut(&lang_key)
        .ok_or(format!("No LSP server running for '{}'", lang_key))?;

    let uri = path_to_uri(&file_path)?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri.to_string() },
        "position": { "line": line, "character": character }
    });

    let result = server.send_request("textDocument/hover", params).await?;

    if result.is_null() {
        return Ok(None);
    }

    // Parse the hover response
    let contents = if let Some(contents) = result.get("contents") {
        if let Some(s) = contents.as_str() {
            s.to_string()
        } else if let Some(value) = contents.get("value").and_then(|v| v.as_str()) {
            value.to_string()
        } else if let Some(arr) = contents.as_array() {
            // MarkedString array
            arr.iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| item.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            serde_json::to_string_pretty(contents).unwrap_or_default()
        }
    } else {
        return Ok(None);
    };

    let range = result.get("range").and_then(|r| {
        Some(LspRange {
            start_line: r.get("start")?.get("line")?.as_u64()? as u32,
            start_character: r.get("start")?.get("character")?.as_u64()? as u32,
            end_line: r.get("end")?.get("line")?.as_u64()? as u32,
            end_character: r.get("end")?.get("character")?.as_u64()? as u32,
        })
    });

    Ok(Some(LspHoverResult { contents, range }))
}

/// Get completion items at a specific position.
#[tauri::command]
pub async fn lsp_completions(
    app: AppHandle,
    file_path: String,
    line: u32,
    character: u32,
    language: String,
) -> Result<Vec<LspCompletionItem>, String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();

    let mut servers = manager.servers.lock().await;
    let server = servers
        .get_mut(&lang_key)
        .ok_or(format!("No LSP server running for '{}'", lang_key))?;

    let uri = path_to_uri(&file_path)?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri.to_string() },
        "position": { "line": line, "character": character }
    });

    let result = server
        .send_request("textDocument/completion", params)
        .await?;

    if result.is_null() {
        return Ok(Vec::new());
    }

    // Completion result can be a CompletionList or a plain array
    let items_value = if let Some(items) = result.get("items") {
        items.clone()
    } else if result.is_array() {
        result
    } else {
        return Ok(Vec::new());
    };

    let raw_items: Vec<serde_json::Value> = match serde_json::from_value(items_value) {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let completions: Vec<LspCompletionItem> = raw_items
        .into_iter()
        .take(100) // Limit to 100 items to avoid flooding the frontend
        .map(|item| {
            let kind_num = item.get("kind").and_then(|k| k.as_u64());
            let kind_val = kind_num.and_then(|n| {
                let kind = match n {
                    1 => lsp_types::CompletionItemKind::TEXT,
                    2 => lsp_types::CompletionItemKind::METHOD,
                    3 => lsp_types::CompletionItemKind::FUNCTION,
                    4 => lsp_types::CompletionItemKind::CONSTRUCTOR,
                    5 => lsp_types::CompletionItemKind::FIELD,
                    6 => lsp_types::CompletionItemKind::VARIABLE,
                    7 => lsp_types::CompletionItemKind::CLASS,
                    8 => lsp_types::CompletionItemKind::INTERFACE,
                    9 => lsp_types::CompletionItemKind::MODULE,
                    10 => lsp_types::CompletionItemKind::PROPERTY,
                    13 => lsp_types::CompletionItemKind::ENUM,
                    14 => lsp_types::CompletionItemKind::KEYWORD,
                    15 => lsp_types::CompletionItemKind::SNIPPET,
                    21 => lsp_types::CompletionItemKind::CONSTANT,
                    22 => lsp_types::CompletionItemKind::STRUCT,
                    23 => lsp_types::CompletionItemKind::EVENT,
                    25 => lsp_types::CompletionItemKind::TYPE_PARAMETER,
                    _ => lsp_types::CompletionItemKind::TEXT,
                };
                Some(kind)
            });

            LspCompletionItem {
                label: item
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or("")
                    .to_string(),
                kind: completion_kind_to_string(kind_val),
                detail: item
                    .get("detail")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string()),
                insert_text: item
                    .get("insertText")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()),
                sort_text: item
                    .get("sortText")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()),
                documentation: item.get("documentation").and_then(|d| {
                    if let Some(s) = d.as_str() {
                        Some(s.to_string())
                    } else {
                        d.get("value").and_then(|v| v.as_str()).map(|s| s.to_string())
                    }
                }),
            }
        })
        .collect();

    Ok(completions)
}

/// Go to definition at a specific position.
#[tauri::command]
pub async fn lsp_definition(
    app: AppHandle,
    file_path: String,
    line: u32,
    character: u32,
    language: String,
) -> Result<Vec<LspLocation>, String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();

    let mut servers = manager.servers.lock().await;
    let server = servers
        .get_mut(&lang_key)
        .ok_or(format!("No LSP server running for '{}'", lang_key))?;

    let uri = path_to_uri(&file_path)?;

    let params = serde_json::json!({
        "textDocument": { "uri": uri.to_string() },
        "position": { "line": line, "character": character }
    });

    let result = server
        .send_request("textDocument/definition", params)
        .await?;

    if result.is_null() {
        return Ok(Vec::new());
    }

    // Definition result can be Location, Location[], or LocationLink[]
    let locations = if result.is_array() {
        result
            .as_array()
            .map(|arr| arr.to_vec())
            .unwrap_or_default()
    } else if result.is_object() {
        vec![result]
    } else {
        return Ok(Vec::new());
    };

    let mut results = Vec::new();
    for loc in locations {
        // Handle both Location and LocationLink
        let (uri_val, range_val) = if let Some(target_uri) = loc.get("targetUri") {
            // LocationLink
            (target_uri, loc.get("targetRange").or_else(|| loc.get("targetSelectionRange")))
        } else {
            // Location
            (loc.get("uri").unwrap_or(&serde_json::Value::Null), loc.get("range"))
        };

        if let (Some(uri_str), Some(range)) = (uri_val.as_str(), range_val) {
            let parsed_uri = match uri_str.parse::<Uri>() {
                Ok(u) => u,
                Err(_) => continue,
            };
            let path = uri_to_path(&parsed_uri);

            if let (Some(start), Some(end)) = (range.get("start"), range.get("end")) {
                results.push(LspLocation {
                    file_path: path,
                    line: start.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    character: start
                        .get("character")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    end_line: end.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    end_character: end
                        .get("character")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                });
            }
        }
    }

    Ok(results)
}

/// Notify the LSP server that a file was opened.
#[tauri::command]
pub async fn lsp_did_open(
    app: AppHandle,
    file_path: String,
    content: String,
    language: String,
) -> Result<(), String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();
    let lang_id = lsp_language_id(&lang_key);

    // Set initial version
    {
        let mut versions = manager.doc_versions.lock().await;
        versions.insert(file_path.clone(), 1);
    }

    let mut servers = manager.servers.lock().await;
    let server = servers
        .get_mut(&lang_key)
        .ok_or(format!("No LSP server running for '{}'", lang_key))?;

    let uri = path_to_uri(&file_path)?;

    let params = serde_json::json!({
        "textDocument": {
            "uri": uri.to_string(),
            "languageId": lang_id,
            "version": 1,
            "text": content,
        }
    });

    server
        .send_notification("textDocument/didOpen", params)
        .await?;

    log::debug!("[LSP:{}] didOpen: {}", lang_key, file_path);
    Ok(())
}

/// Notify the LSP server that a file's content changed (full sync).
#[tauri::command]
pub async fn lsp_did_change(
    app: AppHandle,
    file_path: String,
    content: String,
    language: String,
) -> Result<(), String> {
    let manager = app.state::<LspManager>();
    let lang_key = language.to_lowercase();

    // Increment version
    let version = {
        let mut versions = manager.doc_versions.lock().await;
        let ver = versions.entry(file_path.clone()).or_insert(0);
        *ver += 1;
        *ver
    };

    let mut servers = manager.servers.lock().await;
    let server = servers
        .get_mut(&lang_key)
        .ok_or(format!("No LSP server running for '{}'", lang_key))?;

    let uri = path_to_uri(&file_path)?;

    // Full document sync (TextDocumentSyncKind::Full = 1)
    let params = serde_json::json!({
        "textDocument": {
            "uri": uri.to_string(),
            "version": version,
        },
        "contentChanges": [{
            "text": content,
        }]
    });

    server
        .send_notification("textDocument/didChange", params)
        .await?;

    Ok(())
}

/// Get status of all running LSP servers.
#[tauri::command]
pub async fn lsp_status(app: AppHandle) -> Result<Vec<LspServerStatus>, String> {
    let manager = app.state::<LspManager>();
    let servers = manager.servers.lock().await;

    let statuses: Vec<LspServerStatus> = servers
        .values()
        .map(|server| LspServerStatus {
            language: server.language.clone(),
            running: true, // If it's in the map, it was started
            pid: server.process.id(),
            server_binary: server.server_binary.clone(),
            workspace_path: server.workspace_path.clone(),
        })
        .collect();

    Ok(statuses)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_binary_for_language() {
        assert_eq!(lsp_binary_for_language("rust").unwrap(), "rust-analyzer");
        assert_eq!(
            lsp_binary_for_language("python").unwrap(),
            "pyright-langserver"
        );
        assert_eq!(
            lsp_binary_for_language("typescript").unwrap(),
            "typescript-language-server"
        );
        assert_eq!(
            lsp_binary_for_language("javascript").unwrap(),
            "typescript-language-server"
        );
        assert_eq!(lsp_binary_for_language("go").unwrap(), "gopls");
        assert_eq!(lsp_binary_for_language("c").unwrap(), "clangd");
        assert_eq!(lsp_binary_for_language("cpp").unwrap(), "clangd");
        assert_eq!(lsp_binary_for_language("svelte").unwrap(), "svelteserver");
        assert!(lsp_binary_for_language("brainfuck").is_err());
    }

    #[test]
    fn test_lsp_language_id() {
        assert_eq!(lsp_language_id("rust"), "rust");
        assert_eq!(lsp_language_id("python"), "python");
        assert_eq!(lsp_language_id("ts"), "typescript");
        assert_eq!(lsp_language_id("js"), "javascript");
        assert_eq!(lsp_language_id("cpp"), "cpp");
        assert_eq!(lsp_language_id("unknown"), "plaintext");
    }

    #[test]
    fn test_encode_message() {
        let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let encoded = encode_message(&body);
        let encoded_str = String::from_utf8(encoded).unwrap();
        assert!(encoded_str.starts_with("Content-Length: "));
        assert!(encoded_str.contains("\r\n\r\n"));
        assert!(encoded_str.contains("\"jsonrpc\""));
    }

    #[test]
    fn test_build_request() {
        let req = build_request(42, "textDocument/hover", serde_json::json!({}));
        assert_eq!(req["id"], 42);
        assert_eq!(req["method"], "textDocument/hover");
        assert_eq!(req["jsonrpc"], "2.0");
    }

    #[test]
    fn test_build_notification() {
        let notif = build_notification("initialized", serde_json::json!({}));
        assert!(notif.get("id").is_none());
        assert_eq!(notif["method"], "initialized");
    }

    #[test]
    fn test_severity_to_string() {
        assert_eq!(severity_to_string(Some(DiagnosticSeverity::ERROR)), "error");
        assert_eq!(
            severity_to_string(Some(DiagnosticSeverity::WARNING)),
            "warning"
        );
        assert_eq!(
            severity_to_string(Some(DiagnosticSeverity::INFORMATION)),
            "info"
        );
        assert_eq!(severity_to_string(Some(DiagnosticSeverity::HINT)), "hint");
        assert_eq!(severity_to_string(None), "info");
    }

    #[test]
    fn test_path_to_uri() {
        let uri = path_to_uri("/home/user/project/src/main.rs").unwrap();
        assert!(uri.to_string().starts_with("file:///"));
        assert!(uri.to_string().contains("main.rs"));
    }

    #[test]
    fn test_uri_to_path() {
        let uri: Uri = "file:///home/user/project/src/main.rs".parse().unwrap();
        let path = uri_to_path(&uri);
        assert_eq!(path, "/home/user/project/src/main.rs");
    }

    #[test]
    fn test_lsp_diagnostic_serialize() {
        let diag = LspDiagnostic {
            file_path: "/test/main.rs".to_string(),
            line: 10,
            character: 5,
            end_line: 10,
            end_character: 20,
            severity: "error".to_string(),
            message: "mismatched types".to_string(),
            source: Some("rust-analyzer".to_string()),
            code: Some("E0308".to_string()),
        };

        let json = serde_json::to_string(&diag).unwrap();
        assert!(json.contains("mismatched types"));
        assert!(json.contains("E0308"));

        let deserialized: LspDiagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.line, 10);
        assert_eq!(deserialized.severity, "error");
    }

    #[test]
    fn test_completion_kind_to_string() {
        assert_eq!(
            completion_kind_to_string(Some(lsp_types::CompletionItemKind::FUNCTION)),
            Some("function".to_string())
        );
        assert_eq!(
            completion_kind_to_string(Some(lsp_types::CompletionItemKind::STRUCT)),
            Some("struct".to_string())
        );
        assert_eq!(completion_kind_to_string(None), None);
    }

    #[test]
    fn test_lsp_manager_new() {
        let manager = LspManager::new();
        // Just verify it constructs without panic
        assert!(true);
    }
}
