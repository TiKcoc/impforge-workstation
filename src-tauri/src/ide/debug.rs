// SPDX-License-Identifier: Apache-2.0
//! DAP (Debug Adapter Protocol) Backend for ImpForge CodeForge IDE
//!
//! Manages debug adapter processes (codelldb, debugpy, node --inspect, etc.),
//! communicates via DAP JSON messages over stdin/stdout, tracks breakpoints,
//! call stack, variables, and watch expressions, and exposes Tauri IPC
//! commands to the Svelte 5 frontend.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐   Tauri IPC   ┌──────────────┐  DAP/stdio  ┌──────────────────┐
//! │  Svelte 5   │ ────────────> │ DebugManager │ ──────────> │ codelldb         │
//! │  Frontend   │ <──────────── │   (Rust)     │ <────────── │ debugpy          │
//! └─────────────┘               └──────────────┘             │ node --inspect   │
//!                                                             └──────────────────┘
//! ```
//!
//! ## Protocol
//!
//! DAP uses Content-Length framed JSON messages (identical framing to LSP).
//! Messages are typed as "request", "response", or "event".
//!
//! ## Supported Adapters
//!
//! - **codelldb**: Rust, C, C++ (LLDB-based, from VS Code extension)
//! - **debugpy**: Python (pip install debugpy)
//! - **node --inspect**: JavaScript, TypeScript (built-in Node.js debugger)
//!
//! ## Key DAP Flow
//!
//! 1. `initialize` → adapter reports capabilities
//! 2. `launch` → start the debuggee program
//! 3. `setBreakpoints` → register breakpoints per source file
//! 4. `configurationDone` → signal adapter that configuration is complete
//! 5. Handle `stopped` events → breakpoint hit, step complete, exception
//! 6. `threads` / `stackTrace` / `scopes` / `variables` → inspect state
//! 7. `continue` / `next` / `stepIn` / `stepOut` → control execution
//! 8. Handle `terminated` / `exited` events → session cleanup

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Types exposed to the frontend (Tauri IPC serializable)
// ---------------------------------------------------------------------------

/// Current state of a debug session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugState {
    /// One of: "idle", "running", "stopped", "terminated"
    pub status: String,
    /// Reason the debuggee stopped (e.g. "breakpoint", "step", "exception")
    pub stopped_reason: Option<String>,
    /// Thread that triggered the stop event
    pub stopped_thread_id: Option<u64>,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            stopped_reason: None,
            stopped_thread_id: None,
        }
    }
}

/// A breakpoint with verification status from the debug adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: u32,
    pub file_path: String,
    pub line: u32,
    /// Whether the adapter confirmed this breakpoint is valid
    pub verified: bool,
    /// Optional conditional expression (break only when true)
    pub condition: Option<String>,
}

/// A thread in the debuggee process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapThread {
    pub id: u64,
    pub name: String,
}

/// A single frame in the call stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub file_path: Option<String>,
    pub line: u32,
    pub column: u32,
}

/// A variable or expression value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub var_type: Option<String>,
    /// Reference for fetching child variables (0 = no children / leaf)
    pub children_ref: u64,
}

/// Summary of a debug session for the status command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSessionInfo {
    pub id: String,
    pub language: String,
    pub program: String,
    pub state: DebugState,
    pub breakpoint_count: usize,
    pub adapter_pid: Option<u32>,
    /// The debug adapter identifier (e.g. "codelldb", "debugpy", "js-debug", "delve")
    pub adapter_id: String,
    /// Number of threads currently tracked in the session
    pub thread_count: usize,
    /// Number of stack frames currently tracked
    pub stack_frame_count: usize,
    /// Number of variables currently tracked
    pub variable_count: usize,
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// A running DAP debug session backed by an adapter child process.
struct DebugSession {
    id: String,
    language: String,
    program: String,
    /// The debug adapter identifier (e.g. "codelldb", "debugpy")
    adapter_id: String,
    adapter_process: Child,
    writer: BufWriter<ChildStdin>,
    request_seq: AtomicU64,
    state: DebugState,
    breakpoints: Vec<Breakpoint>,
    threads: Vec<DapThread>,
    stack_frames: Vec<StackFrame>,
    variables: Vec<Variable>,
    /// Adapter capabilities reported during initialize
    capabilities: serde_json::Value,
    /// Pending request futures: seq -> oneshot sender for the response body
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    /// Shared state updated by the background DAP reader task
    shared_state: Arc<SharedSessionState>,
}

/// Manages all active debug sessions.
pub struct DebugManager {
    sessions: Mutex<HashMap<String, DebugSession>>,
}

impl DebugManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Debug adapter discovery
// ---------------------------------------------------------------------------

/// Configuration for spawning a debug adapter process.
#[derive(Debug)]
struct AdapterConfig {
    /// The binary to execute
    binary: String,
    /// Arguments to pass
    args: Vec<String>,
    /// Human-readable adapter name
    adapter_id: String,
}

/// Map a language identifier to the debug adapter binary and arguments.
///
/// Returns the binary path, spawn arguments, and an adapter identifier string.
fn adapter_for_language(language: &str) -> Result<AdapterConfig, String> {
    match language.to_lowercase().as_str() {
        "rust" | "c" | "cpp" | "c++" | "cc" | "cxx" => {
            // codelldb — LLDB-based adapter from the VS Code extension
            // Users install it via: VS Code extension pack, or download the
            // standalone binary from https://github.com/vadimcn/codelldb/releases
            Ok(AdapterConfig {
                binary: "codelldb".to_string(),
                args: vec!["--port".to_string(), "0".to_string()],
                adapter_id: "codelldb".to_string(),
            })
        }
        "python" | "py" => {
            // debugpy — Microsoft's Python debug adapter
            // pip install debugpy
            Ok(AdapterConfig {
                binary: "python3".to_string(),
                args: vec![
                    "-m".to_string(),
                    "debugpy.adapter".to_string(),
                ],
                adapter_id: "debugpy".to_string(),
            })
        }
        "javascript" | "typescript" | "js" | "ts" => {
            // For JS/TS we use the Node.js built-in inspector protocol.
            // The debug adapter is js-debug (vscode-js-debug) which speaks DAP
            // over stdio. Fallback: use node with --inspect-brk and connect.
            Ok(AdapterConfig {
                binary: "js-debug-adapter".to_string(),
                args: vec![],
                adapter_id: "js-debug".to_string(),
            })
        }
        "go" | "golang" => {
            // dlv dap — Delve debugger in DAP mode
            Ok(AdapterConfig {
                binary: "dlv".to_string(),
                args: vec!["dap".to_string()],
                adapter_id: "delve".to_string(),
            })
        }
        _ => Err(format!(
            "Unsupported language '{}' for debugging. \
             Supported: rust, c, cpp, python, javascript, typescript, go",
            language
        )),
    }
}

// ---------------------------------------------------------------------------
// DAP message encoding / decoding helpers
// ---------------------------------------------------------------------------

/// Encode a DAP message with Content-Length header framing.
///
/// Format:
/// ```text
/// Content-Length: <byte count>\r\n
/// \r\n
/// <JSON body>
/// ```
fn encode_dap_message(body: &serde_json::Value) -> Vec<u8> {
    let body_str = serde_json::to_string(body).unwrap_or_default();
    let header = format!("Content-Length: {}\r\n\r\n", body_str.len());
    let mut msg = header.into_bytes();
    msg.extend_from_slice(body_str.as_bytes());
    msg
}

/// Build a DAP request message.
fn build_dap_request(
    seq: u64,
    command: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "seq": seq,
        "type": "request",
        "command": command,
        "arguments": arguments,
    })
}

// ---------------------------------------------------------------------------
// Stdout reader task — runs in background per debug session
// ---------------------------------------------------------------------------

/// Shared state that the reader task updates when events arrive.
struct SharedSessionState {
    state: Mutex<DebugState>,
    threads: Mutex<Vec<DapThread>>,
    stack_frames: Mutex<Vec<StackFrame>>,
    variables: Mutex<Vec<Variable>>,
    breakpoints: Mutex<Vec<Breakpoint>>,
}

/// Spawn a background task that reads DAP messages from the adapter's stdout,
/// dispatches responses to pending request futures, and processes events
/// (stopped, terminated, thread, output, etc.).
fn spawn_dap_reader(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    shared: Arc<SharedSessionState>,
    session_id: String,
    app_handle: Option<AppHandle>,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut header_buf = String::new();

        loop {
            // --- Read Content-Length header ---
            header_buf.clear();
            let mut content_length: usize = 0;

            loop {
                header_buf.clear();
                match reader.read_line(&mut header_buf).await {
                    Ok(0) => return, // EOF — adapter exited
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[DAP:{}] Failed to read header: {}", session_id, e);
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
                // Ignore Content-Type and other headers
            }

            if content_length == 0 {
                continue;
            }

            // --- Read exactly content_length bytes of JSON body ---
            let mut body_buf = vec![0u8; content_length];
            if let Err(e) = reader.read_exact(&mut body_buf).await {
                log::error!("[DAP:{}] Failed to read body: {}", session_id, e);
                return;
            }

            let body: serde_json::Value = match serde_json::from_slice(&body_buf) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("[DAP:{}] Invalid JSON from adapter: {}", session_id, e);
                    continue;
                }
            };

            let msg_type = body
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            match msg_type {
                "response" => {
                    // Dispatch to the pending oneshot channel by request_seq
                    if let Some(req_seq) = body.get("request_seq").and_then(|v| v.as_u64()) {
                        let mut pending_guard = pending.lock().await;
                        if let Some(sender) = pending_guard.remove(&req_seq) {
                            let _ = sender.send(body);
                        }
                    }
                }
                "event" => {
                    let event_name = body
                        .get("event")
                        .and_then(|e| e.as_str())
                        .unwrap_or("");
                    let event_body = body
                        .get("body")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    handle_dap_event(
                        event_name,
                        &event_body,
                        &shared,
                        &session_id,
                        &app_handle,
                    )
                    .await;
                }
                _ => {
                    log::trace!(
                        "[DAP:{}] Unknown message type: {}",
                        session_id,
                        msg_type
                    );
                }
            }
        }
    });
}

/// Process a DAP event and update shared session state.
async fn handle_dap_event(
    event: &str,
    body: &serde_json::Value,
    shared: &Arc<SharedSessionState>,
    session_id: &str,
    app_handle: &Option<AppHandle>,
) {
    match event {
        "stopped" => {
            let reason = body
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown")
                .to_string();
            let thread_id = body
                .get("threadId")
                .and_then(|t| t.as_u64());

            log::info!(
                "[DAP:{}] Stopped: reason={}, thread={:?}",
                session_id,
                reason,
                thread_id
            );

            {
                let mut state = shared.state.lock().await;
                state.status = "stopped".to_string();
                state.stopped_reason = Some(reason.clone());
                state.stopped_thread_id = thread_id;
            }

            // Clear stale stack frames and variables from the previous stop —
            // the frontend should re-fetch them for the new stop location.
            {
                let mut frames = shared.stack_frames.lock().await;
                frames.clear();
            }
            {
                let mut vars = shared.variables.lock().await;
                vars.clear();
            }

            // Include the cached thread count so the frontend knows how many
            // threads are active without a separate roundtrip.
            let thread_count = {
                let threads = shared.threads.lock().await;
                threads.len()
            };

            if let Some(handle) = app_handle {
                let _ = handle.emit("debug-stopped", serde_json::json!({
                    "session_id": session_id,
                    "reason": reason,
                    "thread_id": thread_id,
                    "cached_thread_count": thread_count,
                }));
            }
        }
        "continued" => {
            {
                let mut state = shared.state.lock().await;
                state.status = "running".to_string();
                state.stopped_reason = None;
                state.stopped_thread_id = None;
            }

            if let Some(handle) = app_handle {
                let _ = handle.emit("debug-continued", serde_json::json!({
                    "session_id": session_id,
                }));
            }
        }
        "terminated" => {
            log::info!("[DAP:{}] Terminated", session_id);

            {
                let mut state = shared.state.lock().await;
                state.status = "terminated".to_string();
                state.stopped_reason = None;
                state.stopped_thread_id = None;
            }

            if let Some(handle) = app_handle {
                let _ = handle.emit("debug-terminated", serde_json::json!({
                    "session_id": session_id,
                }));
            }
        }
        "exited" => {
            let exit_code = body
                .get("exitCode")
                .and_then(|c| c.as_i64())
                .unwrap_or(-1);

            log::info!(
                "[DAP:{}] Exited with code {}",
                session_id,
                exit_code
            );

            {
                let mut state = shared.state.lock().await;
                state.status = "terminated".to_string();
            }

            if let Some(handle) = app_handle {
                let _ = handle.emit("debug-exited", serde_json::json!({
                    "session_id": session_id,
                    "exit_code": exit_code,
                }));
            }
        }
        "thread" => {
            let thread_id = body
                .get("threadId")
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            let reason = body
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");

            log::debug!(
                "[DAP:{}] Thread event: id={}, reason={}",
                session_id,
                thread_id,
                reason
            );

            // Maintain the cached thread list so it's available without
            // extra DAP roundtrips when the frontend queries debug status.
            {
                let mut threads = shared.threads.lock().await;
                match reason {
                    "started" => {
                        // Add the new thread if not already tracked
                        if !threads.iter().any(|t| t.id == thread_id) {
                            threads.push(DapThread {
                                id: thread_id,
                                name: format!("Thread {}", thread_id),
                            });
                        }
                    }
                    "exited" => {
                        threads.retain(|t| t.id != thread_id);
                        // Also clear stale stack frames / variables for the exited thread
                        let mut frames = shared.stack_frames.lock().await;
                        frames.clear();
                        let mut vars = shared.variables.lock().await;
                        vars.clear();
                    }
                    _ => {}
                }
            }
        }
        "output" => {
            let category = body
                .get("category")
                .and_then(|c| c.as_str())
                .unwrap_or("console");
            let output = body
                .get("output")
                .and_then(|o| o.as_str())
                .unwrap_or("");

            if !output.is_empty() {
                log::debug!("[DAP:{}:{}] {}", session_id, category, output.trim_end());

                if let Some(handle) = app_handle {
                    let _ = handle.emit("debug-output", serde_json::json!({
                        "session_id": session_id,
                        "category": category,
                        "output": output,
                    }));
                }
            }
        }
        "breakpoint" => {
            // Breakpoint status changed (verified, moved, etc.)
            if let Some(bp) = body.get("breakpoint") {
                let bp_id = bp.get("id").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
                let verified = bp.get("verified").and_then(|v| v.as_bool()).unwrap_or(false);
                let new_line = bp.get("line").and_then(|l| l.as_u64());

                let mut bps = shared.breakpoints.lock().await;
                if let Some(existing) = bps.iter_mut().find(|b| b.id == bp_id) {
                    existing.verified = verified;
                    if let Some(line) = new_line {
                        existing.line = line as u32;
                    }
                }

                log::debug!(
                    "[DAP:{}] Breakpoint changed: id={}, verified={}",
                    session_id,
                    bp_id,
                    verified
                );
            }
        }
        "initialized" => {
            // The adapter is ready to accept configuration requests
            // (setBreakpoints, configurationDone, etc.)
            log::info!("[DAP:{}] Adapter initialized, ready for configuration", session_id);

            if let Some(handle) = app_handle {
                let _ = handle.emit("debug-initialized", serde_json::json!({
                    "session_id": session_id,
                }));
            }
        }
        "loadedSource" | "module" | "process" | "capabilities" | "memory"
        | "progressStart" | "progressUpdate" | "progressEnd" => {
            log::trace!("[DAP:{}] Event '{}' (ignored)", session_id, event);
        }
        _ => {
            log::trace!("[DAP:{}] Unhandled event: {}", session_id, event);
        }
    }
}

// ---------------------------------------------------------------------------
// DebugSession methods
// ---------------------------------------------------------------------------

impl DebugSession {
    /// Send a DAP request and wait for the response (with timeout).
    async fn send_request(
        &mut self,
        command: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let seq = self.request_seq.fetch_add(1, Ordering::SeqCst);
        let msg = build_dap_request(seq, command, arguments);
        let encoded = encode_dap_message(&msg);

        // Register a oneshot channel for the response
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(seq, tx);
        }

        // Write to adapter stdin
        self.writer
            .write_all(&encoded)
            .await
            .map_err(|e| format!("Failed to write to DAP adapter stdin: {}", e))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("Failed to flush DAP adapter stdin: {}", e))?;

        // Wait for response with 30-second timeout
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                // Check DAP success flag
                let success = response
                    .get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);

                if !success {
                    let err_msg = response
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown DAP error");
                    let err_details = response
                        .get("body")
                        .and_then(|b| b.get("error"))
                        .and_then(|e| e.get("format"))
                        .and_then(|f| f.as_str());

                    return Err(format!(
                        "DAP error ({}): {}{}",
                        command,
                        err_msg,
                        err_details
                            .map(|d| format!(" - {}", d))
                            .unwrap_or_default()
                    ));
                }

                Ok(response
                    .get("body")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null))
            }
            Ok(Err(_)) => {
                Err("DAP response channel dropped (adapter may have crashed)".to_string())
            }
            Err(_) => {
                // Clean up stale pending entry
                let mut pending = self.pending.lock().await;
                pending.remove(&seq);
                Err(format!(
                    "DAP request '{}' timed out after 30s",
                    command
                ))
            }
        }
    }

    /// Send the DAP `initialize` request with client capabilities.
    async fn initialize(&mut self) -> Result<serde_json::Value, String> {
        let args = serde_json::json!({
            "clientID": "impforge",
            "clientName": "ImpForge CodeForge IDE",
            "adapterID": self.adapter_id,
            "pathFormat": "path",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "supportsVariableType": true,
            "supportsVariablePaging": false,
            "supportsRunInTerminalRequest": false,
            "supportsMemoryReferences": false,
            "supportsProgressReporting": false,
            "supportsInvalidatedEvent": false,
            "locale": "en-US",
        });

        let caps = self.send_request("initialize", args).await?;
        self.capabilities = caps.clone();

        log::info!(
            "[DAP:{}] Adapter '{}' initialized (capabilities received)",
            self.id,
            self.adapter_id
        );

        Ok(caps)
    }

    /// Send the DAP `launch` request to start debugging a program.
    async fn launch(
        &mut self,
        program: &str,
        args: &[String],
        cwd: &str,
    ) -> Result<(), String> {
        let launch_args = match self.language.as_str() {
            "rust" | "c" | "cpp" | "c++" | "cc" | "cxx" => {
                serde_json::json!({
                    "program": program,
                    "args": args,
                    "cwd": cwd,
                    "stopOnEntry": false,
                    "type": "lldb",
                })
            }
            "python" | "py" => {
                serde_json::json!({
                    "program": program,
                    "args": args,
                    "cwd": cwd,
                    "stopOnEntry": false,
                    "type": "python",
                    "console": "integratedTerminal",
                    "justMyCode": true,
                })
            }
            "javascript" | "typescript" | "js" | "ts" => {
                serde_json::json!({
                    "program": program,
                    "args": args,
                    "cwd": cwd,
                    "stopOnEntry": false,
                    "type": "pwa-node",
                    "console": "integratedTerminal",
                })
            }
            "go" | "golang" => {
                serde_json::json!({
                    "program": program,
                    "args": args,
                    "cwd": cwd,
                    "stopOnEntry": false,
                    "mode": "debug",
                })
            }
            _ => {
                serde_json::json!({
                    "program": program,
                    "args": args,
                    "cwd": cwd,
                    "stopOnEntry": false,
                })
            }
        };

        self.send_request("launch", launch_args).await?;
        self.state.status = "running".to_string();

        log::info!(
            "[DAP:{}] Launched program: {} in {}",
            self.id,
            program,
            cwd
        );

        Ok(())
    }

    /// Send `configurationDone` to signal the adapter that all initial
    /// configuration (breakpoints, exception filters, etc.) is complete.
    async fn configuration_done(&mut self) -> Result<(), String> {
        self.send_request("configurationDone", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Send a graceful `disconnect` request, then force-kill if needed.
    async fn disconnect(&mut self) {
        let _ = self
            .send_request(
                "disconnect",
                serde_json::json!({
                    "restart": false,
                    "terminateDebuggee": true,
                }),
            )
            .await;

        // Give the adapter a moment to shut down cleanly
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = self.adapter_process.kill().await;

        self.state.status = "terminated".to_string();
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Launch a debug session for the given language and program.
///
/// Returns a unique session ID that must be passed to all subsequent
/// debug commands for this session.
#[tauri::command]
pub async fn debug_launch(
    app: AppHandle,
    language: String,
    program: String,
    args: Vec<String>,
    cwd: String,
) -> Result<String, String> {
    let manager = app.state::<DebugManager>();
    let lang_key = language.to_lowercase();

    // Validate the program file exists
    if !std::path::Path::new(&program).exists() {
        return Err(format!(
            "Program not found: {}. Build it first.",
            program
        ));
    }

    // Validate working directory
    if !std::path::Path::new(&cwd).is_dir() {
        return Err(format!("Working directory does not exist: {}", cwd));
    }

    // Resolve the adapter binary
    let adapter_config = adapter_for_language(&lang_key)?;

    // Verify the adapter binary is available
    let binary_check = Command::new("which")
        .arg(&adapter_config.binary)
        .output()
        .await;

    match &binary_check {
        Ok(output) if !output.status.success() => {
            return Err(format!(
                "Debug adapter '{}' (binary: '{}') not found on PATH.\n\
                 Install instructions:\n\
                 - Rust/C/C++: Install codelldb from https://github.com/vadimcn/codelldb/releases\n\
                 - Python: pip install debugpy\n\
                 - JS/TS: npm install -g js-debug-adapter\n\
                 - Go: go install github.com/go-delve/delve/cmd/dlv@latest",
                adapter_config.adapter_id, adapter_config.binary
            ));
        }
        Err(e) => {
            return Err(format!(
                "Failed to check for adapter '{}' (binary: '{}'): {}",
                adapter_config.adapter_id, adapter_config.binary, e
            ));
        }
        _ => {}
    }

    // Generate a unique session ID
    let session_id = format!(
        "dbg-{}-{}",
        lang_key,
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0000")
    );

    // Spawn the adapter process
    let mut cmd = Command::new(&adapter_config.binary);
    cmd.args(&adapter_config.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(&cwd);

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to spawn debug adapter '{}' (binary: '{}'): {}",
            adapter_config.adapter_id, adapter_config.binary, e
        )
    })?;

    let pid = child.id();

    let stdin = child
        .stdin
        .take()
        .ok_or("Failed to capture debug adapter stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture debug adapter stdout")?;

    // Log stderr in background
    if let Some(stderr) = child.stderr.take() {
        let sid = session_id.clone();
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
                            log::debug!("[DAP:{}:stderr] {}", sid, trimmed);
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let shared = Arc::new(SharedSessionState {
        state: Mutex::new(DebugState::default()),
        threads: Mutex::new(Vec::new()),
        stack_frames: Mutex::new(Vec::new()),
        variables: Mutex::new(Vec::new()),
        breakpoints: Mutex::new(Vec::new()),
    });

    // Spawn the stdout reader task
    spawn_dap_reader(
        stdout,
        Arc::clone(&pending),
        Arc::clone(&shared),
        session_id.clone(),
        Some(app.clone()),
    );

    let mut session = DebugSession {
        id: session_id.clone(),
        language: lang_key.clone(),
        program: program.clone(),
        adapter_id: adapter_config.adapter_id.clone(),
        adapter_process: child,
        writer: BufWriter::new(stdin),
        request_seq: AtomicU64::new(1),
        state: DebugState::default(),
        breakpoints: Vec::new(),
        threads: Vec::new(),
        stack_frames: Vec::new(),
        variables: Vec::new(),
        capabilities: serde_json::Value::Null,
        pending,
        shared_state: Arc::clone(&shared),
    };

    // DAP handshake: initialize -> launch -> configurationDone
    session.initialize().await?;
    session.launch(&program, &args, &cwd).await?;
    session.configuration_done().await?;

    log::info!(
        "[DAP] Session '{}' started: adapter='{}' binary='{}' (pid: {:?})",
        session_id,
        adapter_config.adapter_id,
        adapter_config.binary,
        pid
    );

    // Store the session
    {
        let mut sessions = manager.sessions.lock().await;
        sessions.insert(session_id.clone(), session);
    }

    Ok(session_id)
}

/// Set breakpoints for a source file within an active debug session.
///
/// This replaces all breakpoints for the given file — any previously set
/// breakpoints in that file that are not in the `lines` list will be removed.
#[tauri::command]
pub async fn debug_set_breakpoints(
    app: AppHandle,
    session_id: String,
    file_path: String,
    lines: Vec<u32>,
) -> Result<Vec<Breakpoint>, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    let source_breakpoints: Vec<serde_json::Value> = lines
        .iter()
        .map(|&line| serde_json::json!({ "line": line }))
        .collect();

    let params = serde_json::json!({
        "source": {
            "path": file_path,
        },
        "breakpoints": source_breakpoints,
    });

    let result = session
        .send_request("setBreakpoints", params)
        .await?;

    // Parse the response breakpoints
    let response_bps = result
        .get("breakpoints")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();

    let mut breakpoints = Vec::new();
    for (i, bp) in response_bps.iter().enumerate() {
        let id = bp.get("id").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
        let verified = bp
            .get("verified")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let actual_line = bp
            .get("line")
            .and_then(|l| l.as_u64())
            .unwrap_or(lines.get(i).copied().unwrap_or(0) as u64) as u32;

        breakpoints.push(Breakpoint {
            id,
            file_path: file_path.clone(),
            line: actual_line,
            verified,
            condition: None,
        });
    }

    // Remove old breakpoints for this file and add new ones
    session
        .breakpoints
        .retain(|b| b.file_path != file_path);
    session.breakpoints.extend(breakpoints.clone());

    log::debug!(
        "[DAP:{}] Set {} breakpoints in {}",
        session_id,
        breakpoints.len(),
        file_path
    );

    Ok(breakpoints)
}

/// Continue execution of a stopped thread.
#[tauri::command]
pub async fn debug_continue(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    session
        .send_request(
            "continue",
            serde_json::json!({ "threadId": thread_id }),
        )
        .await?;

    session.state.status = "running".to_string();
    session.state.stopped_reason = None;
    session.state.stopped_thread_id = None;

    Ok(())
}

/// Step over the current line (next statement, staying in same frame).
#[tauri::command]
pub async fn debug_step_over(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    session
        .send_request("next", serde_json::json!({ "threadId": thread_id }))
        .await?;

    session.state.status = "running".to_string();

    Ok(())
}

/// Step into the current function call.
#[tauri::command]
pub async fn debug_step_in(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    session
        .send_request("stepIn", serde_json::json!({ "threadId": thread_id }))
        .await?;

    session.state.status = "running".to_string();

    Ok(())
}

/// Step out of the current function (return to caller).
#[tauri::command]
pub async fn debug_step_out(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    session
        .send_request("stepOut", serde_json::json!({ "threadId": thread_id }))
        .await?;

    session.state.status = "running".to_string();

    Ok(())
}

/// Pause a running thread.
#[tauri::command]
pub async fn debug_pause(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    session
        .send_request("pause", serde_json::json!({ "threadId": thread_id }))
        .await?;

    // The actual state change will come via a "stopped" event from the adapter

    Ok(())
}

/// Stop (terminate) a debug session and clean up resources.
#[tauri::command]
pub async fn debug_stop(
    app: AppHandle,
    session_id: String,
) -> Result<(), String> {
    let manager = app.state::<DebugManager>();

    let mut session = {
        let mut sessions = manager.sessions.lock().await;
        sessions
            .remove(&session_id)
            .ok_or(format!("No debug session with id '{}'", session_id))?
    };

    session.disconnect().await;

    log::info!("[DAP] Session '{}' stopped and cleaned up", session_id);

    Ok(())
}

/// Get the list of threads in the debuggee process.
#[tauri::command]
pub async fn debug_get_threads(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<DapThread>, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    let result = session
        .send_request("threads", serde_json::json!({}))
        .await?;

    let raw_threads = result
        .get("threads")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let threads: Vec<DapThread> = raw_threads
        .iter()
        .map(|t| DapThread {
            id: t.get("id").and_then(|i| i.as_u64()).unwrap_or(0),
            name: t
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unnamed")
                .to_string(),
        })
        .collect();

    session.threads = threads.clone();

    // Sync the authoritative thread list into the shared state so the
    // background reader task and status queries have up-to-date data.
    {
        let mut shared_threads = session.shared_state.threads.lock().await;
        *shared_threads = threads.clone();
    }

    Ok(threads)
}

/// Get the call stack (stack trace) for a specific thread.
#[tauri::command]
pub async fn debug_get_stack_trace(
    app: AppHandle,
    session_id: String,
    thread_id: u64,
) -> Result<Vec<StackFrame>, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    let result = session
        .send_request(
            "stackTrace",
            serde_json::json!({
                "threadId": thread_id,
                "startFrame": 0,
                "levels": 100,
            }),
        )
        .await?;

    let raw_frames = result
        .get("stackFrames")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    let frames: Vec<StackFrame> = raw_frames
        .iter()
        .map(|f| {
            let file_path = f
                .get("source")
                .and_then(|s| s.get("path"))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());

            StackFrame {
                id: f.get("id").and_then(|i| i.as_u64()).unwrap_or(0),
                name: f
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("<unknown>")
                    .to_string(),
                file_path,
                line: f
                    .get("line")
                    .and_then(|l| l.as_u64())
                    .unwrap_or(0) as u32,
                column: f
                    .get("column")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as u32,
            }
        })
        .collect();

    session.stack_frames = frames.clone();

    // Keep the shared state in sync so status queries reflect current data.
    {
        let mut shared_frames = session.shared_state.stack_frames.lock().await;
        *shared_frames = frames.clone();
    }

    Ok(frames)
}

/// Get variables for a given scope or variable reference.
///
/// To get top-level scopes, first call `debug_get_stack_trace` to get frame IDs,
/// then use the `scopes` request internally. The `scope_ref` parameter corresponds
/// to a `variablesReference` from a scope or a parent variable.
#[tauri::command]
pub async fn debug_get_variables(
    app: AppHandle,
    session_id: String,
    scope_ref: u64,
) -> Result<Vec<Variable>, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    // If scope_ref matches a known stack frame ID, auto-resolve scopes first
    // and fetch variables from all scopes in that frame. This saves the frontend
    // from having to call debug_get_scopes separately.
    let is_frame_id = session.stack_frames.iter().any(|f| f.id == scope_ref);

    if is_frame_id {
        // Resolve scopes for this frame, then fetch variables from each scope
        let scopes = get_scopes_for_frame(session, scope_ref).await?;

        let mut all_variables: Vec<Variable> = Vec::new();
        for scope in &scopes {
            let var_ref = scope
                .get("variablesReference")
                .and_then(|r| r.as_u64())
                .unwrap_or(0);
            if var_ref == 0 {
                continue;
            }

            let scope_name = scope
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("Unknown");

            log::debug!(
                "[DAP:{}] Fetching variables for scope '{}' (ref={})",
                session.id,
                scope_name,
                var_ref
            );

            let scope_result = session
                .send_request(
                    "variables",
                    serde_json::json!({ "variablesReference": var_ref }),
                )
                .await?;

            let scope_vars = scope_result
                .get("variables")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for v in &scope_vars {
                all_variables.push(Variable {
                    name: v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    value: v.get("value").and_then(|val| val.as_str()).unwrap_or("").to_string(),
                    var_type: v.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()),
                    children_ref: v.get("variablesReference").and_then(|r| r.as_u64()).unwrap_or(0),
                });
            }
        }

        session.variables = all_variables.clone();
        {
            let mut shared_vars = session.shared_state.variables.lock().await;
            *shared_vars = all_variables.clone();
        }

        return Ok(all_variables);
    }

    // Direct variablesReference lookup (for expanding child variables
    // or when the frontend already resolved scopes)
    let result = session
        .send_request(
            "variables",
            serde_json::json!({
                "variablesReference": scope_ref,
            }),
        )
        .await?;

    let raw_vars = result
        .get("variables")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let variables: Vec<Variable> = raw_vars
        .iter()
        .map(|v| Variable {
            name: v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string(),
            value: v
                .get("value")
                .and_then(|val| val.as_str())
                .unwrap_or("")
                .to_string(),
            var_type: v
                .get("type")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
            children_ref: v
                .get("variablesReference")
                .and_then(|r| r.as_u64())
                .unwrap_or(0),
        })
        .collect();

    session.variables = variables.clone();

    // Keep the shared state in sync so status queries reflect current data.
    {
        let mut shared_vars = session.shared_state.variables.lock().await;
        *shared_vars = variables.clone();
    }

    Ok(variables)
}

/// Evaluate an expression in the context of a stopped frame.
///
/// If `frame_id` is `None`, the expression is evaluated in the global scope.
#[tauri::command]
pub async fn debug_evaluate(
    app: AppHandle,
    session_id: String,
    expression: String,
    frame_id: Option<u64>,
) -> Result<String, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    let mut eval_args = serde_json::json!({
        "expression": expression,
        "context": "repl",
    });

    if let Some(fid) = frame_id {
        eval_args
            .as_object_mut()
            .ok_or_else(|| "Failed to construct eval arguments object".to_string())?
            .insert("frameId".to_string(), serde_json::json!(fid));
    }

    let result = session
        .send_request("evaluate", eval_args)
        .await?;

    let value = result
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    Ok(value)
}

/// Get the scopes for a specific stack frame.
///
/// Each scope has a `variables_reference` that can be passed to
/// `debug_get_variables` to retrieve the variables in that scope.
///
/// This is an internal helper used by `debug_get_variables` to resolve
/// frame IDs into scope variable references. It can also be called
/// directly for advanced use cases.
async fn get_scopes_for_frame(
    session: &mut DebugSession,
    frame_id: u64,
) -> Result<Vec<serde_json::Value>, String> {
    let result = session
        .send_request(
            "scopes",
            serde_json::json!({ "frameId": frame_id }),
        )
        .await?;

    let scopes = result
        .get("scopes")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    log::debug!(
        "[DAP:{}] Retrieved {} scopes for frame {}",
        session.id,
        scopes.len(),
        frame_id
    );

    Ok(scopes)
}

/// Get the scopes for a specific stack frame (Tauri command wrapper).
///
/// Each scope has a `variables_reference` that can be passed to
/// `debug_get_variables` to retrieve the variables in that scope.
#[tauri::command]
pub async fn debug_get_scopes(
    app: AppHandle,
    session_id: String,
    frame_id: u64,
) -> Result<Vec<serde_json::Value>, String> {
    let manager = app.state::<DebugManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(format!("No debug session with id '{}'", session_id))?;

    get_scopes_for_frame(session, frame_id).await
}

/// Get status of all active debug sessions.
#[tauri::command]
pub async fn debug_status(
    app: AppHandle,
) -> Result<Vec<DebugSessionInfo>, String> {
    let manager = app.state::<DebugManager>();
    let sessions = manager.sessions.lock().await;

    let mut infos: Vec<DebugSessionInfo> = Vec::new();

    for session in sessions.values() {
        // Read cached counts from the shared state (updated by the
        // background DAP reader task and by the query commands).
        let thread_count = {
            let threads = session.shared_state.threads.lock().await;
            threads.len()
        };
        let stack_frame_count = {
            let frames = session.shared_state.stack_frames.lock().await;
            frames.len()
        };
        let variable_count = {
            let vars = session.shared_state.variables.lock().await;
            vars.len()
        };

        infos.push(DebugSessionInfo {
            id: session.id.clone(),
            language: session.language.clone(),
            program: session.program.clone(),
            state: session.state.clone(),
            breakpoint_count: session.breakpoints.len(),
            adapter_pid: session.adapter_process.id(),
            adapter_id: session.adapter_id.clone(),
            thread_count,
            stack_frame_count,
            variable_count,
        });
    }

    Ok(infos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_for_language() {
        let rust = adapter_for_language("rust").unwrap();
        assert_eq!(rust.binary, "codelldb");
        assert_eq!(rust.adapter_id, "codelldb");

        let cpp = adapter_for_language("cpp").unwrap();
        assert_eq!(cpp.binary, "codelldb");

        let c = adapter_for_language("c").unwrap();
        assert_eq!(c.binary, "codelldb");

        let python = adapter_for_language("python").unwrap();
        assert_eq!(python.binary, "python3");
        assert_eq!(python.adapter_id, "debugpy");

        let js = adapter_for_language("javascript").unwrap();
        assert_eq!(js.binary, "js-debug-adapter");
        assert_eq!(js.adapter_id, "js-debug");

        let ts = adapter_for_language("typescript").unwrap();
        assert_eq!(ts.binary, "js-debug-adapter");

        let go = adapter_for_language("go").unwrap();
        assert_eq!(go.binary, "dlv");
        assert_eq!(go.adapter_id, "delve");

        assert!(adapter_for_language("brainfuck").is_err());
        assert!(adapter_for_language("cobol").is_err());
    }

    #[test]
    fn test_adapter_for_language_case_insensitive() {
        assert!(adapter_for_language("Rust").is_ok());
        assert!(adapter_for_language("PYTHON").is_ok());
        assert!(adapter_for_language("JavaScript").is_ok());
        assert!(adapter_for_language("Go").is_ok());
    }

    #[test]
    fn test_adapter_aliases() {
        // C++ aliases
        assert!(adapter_for_language("c++").is_ok());
        assert!(adapter_for_language("cc").is_ok());
        assert!(adapter_for_language("cxx").is_ok());

        // Python alias
        assert!(adapter_for_language("py").is_ok());

        // JS/TS aliases
        assert!(adapter_for_language("js").is_ok());
        assert!(adapter_for_language("ts").is_ok());

        // Go alias
        assert!(adapter_for_language("golang").is_ok());
    }

    #[test]
    fn test_encode_dap_message() {
        let body = serde_json::json!({
            "seq": 1,
            "type": "request",
            "command": "initialize"
        });
        let encoded = encode_dap_message(&body);
        let encoded_str = String::from_utf8(encoded).unwrap();

        assert!(encoded_str.starts_with("Content-Length: "));
        assert!(encoded_str.contains("\r\n\r\n"));
        assert!(encoded_str.contains("\"seq\""));
        assert!(encoded_str.contains("\"request\""));
        assert!(encoded_str.contains("\"initialize\""));
    }

    #[test]
    fn test_encode_dap_message_content_length_accuracy() {
        let body = serde_json::json!({"seq": 1, "type": "request", "command": "test"});
        let encoded = encode_dap_message(&body);
        let encoded_str = String::from_utf8(encoded).unwrap();

        // Extract Content-Length value
        let cl_line = encoded_str
            .lines()
            .find(|l| l.starts_with("Content-Length: "))
            .unwrap();
        let declared_len: usize = cl_line
            .strip_prefix("Content-Length: ")
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        // The body starts after the double CRLF
        let body_start = encoded_str.find("\r\n\r\n").unwrap() + 4;
        let actual_body = &encoded_str[body_start..];

        assert_eq!(declared_len, actual_body.len());
    }

    #[test]
    fn test_build_dap_request() {
        let req = build_dap_request(
            42,
            "initialize",
            serde_json::json!({"clientID": "impforge"}),
        );

        assert_eq!(req["seq"], 42);
        assert_eq!(req["type"], "request");
        assert_eq!(req["command"], "initialize");
        assert_eq!(req["arguments"]["clientID"], "impforge");
    }

    #[test]
    fn test_build_dap_request_empty_args() {
        let req = build_dap_request(1, "configurationDone", serde_json::json!({}));

        assert_eq!(req["seq"], 1);
        assert_eq!(req["command"], "configurationDone");
        assert!(req["arguments"].is_object());
    }

    #[test]
    fn test_debug_state_default() {
        let state = DebugState::default();
        assert_eq!(state.status, "idle");
        assert!(state.stopped_reason.is_none());
        assert!(state.stopped_thread_id.is_none());
    }

    #[test]
    fn test_debug_state_serialization() {
        let state = DebugState {
            status: "stopped".to_string(),
            stopped_reason: Some("breakpoint".to_string()),
            stopped_thread_id: Some(1),
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"stopped\""));
        assert!(json.contains("\"breakpoint\""));

        let deserialized: DebugState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "stopped");
        assert_eq!(deserialized.stopped_reason.unwrap(), "breakpoint");
        assert_eq!(deserialized.stopped_thread_id.unwrap(), 1);
    }

    #[test]
    fn test_breakpoint_serialization() {
        let bp = Breakpoint {
            id: 1,
            file_path: "/home/user/project/src/main.rs".to_string(),
            line: 42,
            verified: true,
            condition: Some("x > 10".to_string()),
        };

        let json = serde_json::to_string(&bp).unwrap();
        assert!(json.contains("main.rs"));
        assert!(json.contains("42"));
        assert!(json.contains("\"verified\":true"));
        assert!(json.contains("x > 10"));

        let deserialized: Breakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.line, 42);
        assert!(deserialized.verified);
        assert_eq!(deserialized.condition.unwrap(), "x > 10");
    }

    #[test]
    fn test_breakpoint_without_condition() {
        let bp = Breakpoint {
            id: 5,
            file_path: "/test/lib.py".to_string(),
            line: 10,
            verified: false,
            condition: None,
        };

        let json = serde_json::to_string(&bp).unwrap();
        let deserialized: Breakpoint = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.verified);
        assert!(deserialized.condition.is_none());
    }

    #[test]
    fn test_dap_thread_serialization() {
        let thread = DapThread {
            id: 1,
            name: "main".to_string(),
        };

        let json = serde_json::to_string(&thread).unwrap();
        assert!(json.contains("\"main\""));

        let deserialized: DapThread = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.name, "main");
    }

    #[test]
    fn test_stack_frame_serialization() {
        let frame = StackFrame {
            id: 100,
            name: "main::handle_request".to_string(),
            file_path: Some("/src/handler.rs".to_string()),
            line: 55,
            column: 8,
        };

        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("handle_request"));
        assert!(json.contains("handler.rs"));

        let deserialized: StackFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 100);
        assert_eq!(deserialized.line, 55);
        assert_eq!(deserialized.column, 8);
    }

    #[test]
    fn test_stack_frame_no_source() {
        let frame = StackFrame {
            id: 200,
            name: "<unknown>".to_string(),
            file_path: None,
            line: 0,
            column: 0,
        };

        let json = serde_json::to_string(&frame).unwrap();
        let deserialized: StackFrame = serde_json::from_str(&json).unwrap();
        assert!(deserialized.file_path.is_none());
    }

    #[test]
    fn test_variable_serialization() {
        let var = Variable {
            name: "counter".to_string(),
            value: "42".to_string(),
            var_type: Some("i32".to_string()),
            children_ref: 0,
        };

        let json = serde_json::to_string(&var).unwrap();
        assert!(json.contains("\"counter\""));
        assert!(json.contains("\"42\""));
        assert!(json.contains("\"i32\""));

        let deserialized: Variable = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "counter");
        assert_eq!(deserialized.value, "42");
        assert_eq!(deserialized.children_ref, 0);
    }

    #[test]
    fn test_variable_with_children() {
        let var = Variable {
            name: "my_struct".to_string(),
            value: "MyStruct { ... }".to_string(),
            var_type: Some("MyStruct".to_string()),
            children_ref: 1001, // Non-zero = expandable
        };

        let deserialized: Variable =
            serde_json::from_str(&serde_json::to_string(&var).unwrap()).unwrap();
        assert_eq!(deserialized.children_ref, 1001);
    }

    #[test]
    fn test_debug_session_info_serialization() {
        let info = DebugSessionInfo {
            id: "dbg-rust-abcd".to_string(),
            language: "rust".to_string(),
            program: "/home/user/project/target/debug/myapp".to_string(),
            state: DebugState {
                status: "stopped".to_string(),
                stopped_reason: Some("breakpoint".to_string()),
                stopped_thread_id: Some(1),
            },
            breakpoint_count: 3,
            adapter_pid: Some(12345),
            adapter_id: "codelldb".to_string(),
            thread_count: 2,
            stack_frame_count: 5,
            variable_count: 12,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("dbg-rust-abcd"));
        assert!(json.contains("12345"));
        assert!(json.contains("codelldb"));
        assert!(json.contains("\"thread_count\":2"));
        assert!(json.contains("\"stack_frame_count\":5"));
        assert!(json.contains("\"variable_count\":12"));

        let deserialized: DebugSessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.breakpoint_count, 3);
        assert_eq!(deserialized.adapter_pid, Some(12345));
        assert_eq!(deserialized.adapter_id, "codelldb");
        assert_eq!(deserialized.thread_count, 2);
        assert_eq!(deserialized.stack_frame_count, 5);
        assert_eq!(deserialized.variable_count, 12);
    }

    #[test]
    fn test_debug_manager_new() {
        let manager = DebugManager::new();
        // Verify construction does not panic
        let _ = manager;
    }

    #[test]
    fn test_adapter_error_message_format() {
        let err = adapter_for_language("haskell").unwrap_err();
        assert!(err.contains("Unsupported language"));
        assert!(err.contains("haskell"));
        assert!(err.contains("Supported:"));
    }

    #[test]
    fn test_codelldb_uses_port_zero() {
        let config = adapter_for_language("rust").unwrap();
        // codelldb --port 0 lets the OS pick a free port for the DAP server
        assert!(config.args.contains(&"--port".to_string()));
        assert!(config.args.contains(&"0".to_string()));
    }

    #[test]
    fn test_debugpy_uses_module_flag() {
        let config = adapter_for_language("python").unwrap();
        assert!(config.args.contains(&"-m".to_string()));
        assert!(config.args.contains(&"debugpy.adapter".to_string()));
    }

    #[test]
    fn test_delve_uses_dap_subcommand() {
        let config = adapter_for_language("go").unwrap();
        assert!(config.args.contains(&"dap".to_string()));
    }

    #[test]
    fn test_multiple_breakpoint_files() {
        // Verify that breakpoints from different files can coexist
        let bp1 = Breakpoint {
            id: 1,
            file_path: "/src/main.rs".to_string(),
            line: 10,
            verified: true,
            condition: None,
        };
        let bp2 = Breakpoint {
            id: 2,
            file_path: "/src/lib.rs".to_string(),
            line: 20,
            verified: true,
            condition: None,
        };

        let mut all_bps = vec![bp1.clone(), bp2.clone()];

        // Simulate replacing breakpoints for main.rs only
        all_bps.retain(|b| b.file_path != "/src/main.rs");
        let replacement = Breakpoint {
            id: 3,
            file_path: "/src/main.rs".to_string(),
            line: 15,
            verified: true,
            condition: None,
        };
        all_bps.push(replacement);

        assert_eq!(all_bps.len(), 2);
        assert!(all_bps.iter().any(|b| b.file_path == "/src/lib.rs" && b.line == 20));
        assert!(all_bps.iter().any(|b| b.file_path == "/src/main.rs" && b.line == 15));
    }

    #[test]
    fn test_dap_message_roundtrip() {
        // Verify that encoding produces valid JSON that can be re-parsed
        let original = serde_json::json!({
            "seq": 1,
            "type": "request",
            "command": "setBreakpoints",
            "arguments": {
                "source": { "path": "/test/main.rs" },
                "breakpoints": [{ "line": 10 }, { "line": 20 }]
            }
        });

        let encoded = encode_dap_message(&original);
        let encoded_str = String::from_utf8(encoded).unwrap();

        // Find the body after the header
        let body_start = encoded_str.find("\r\n\r\n").unwrap() + 4;
        let body_str = &encoded_str[body_start..];

        let parsed: serde_json::Value = serde_json::from_str(body_str).unwrap();
        assert_eq!(parsed["command"], "setBreakpoints");
        assert_eq!(
            parsed["arguments"]["breakpoints"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}
