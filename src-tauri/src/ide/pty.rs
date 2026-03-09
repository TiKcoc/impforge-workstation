//! PTY management for real terminal support
//!
//! Uses portable-pty to spawn shell processes with full PTY support.
//! Each terminal tab gets its own PTY session with a unique ID.
//! Communication with the frontend happens via Tauri events (bidirectional).

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

static NEXT_PTY_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PtyOutput {
    pub id: u32,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PtyInfo {
    pub id: u32,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
}

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send>,
    shell: String,
    cols: u16,
    rows: u16,
}

pub struct PtyManager {
    sessions: Mutex<HashMap<u32, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    shell: Option<String>,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<PtyInfo, String> {
    let pty_system = native_pty_system();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let shell_path = shell.clone().unwrap_or_else(|| {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    });

    let mut cmd = CommandBuilder::new(&shell_path);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    if let Some(dir) = &cwd {
        cmd.cwd(dir);
    } else if let Some(home) = dirs::home_dir() {
        cmd.cwd(home);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let id = NEXT_PTY_ID.fetch_add(1, Ordering::SeqCst);

    // Clone reader and writer from the master PTY before storing it
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    let app_clone = app.clone();
    let event_name = format!("pty-output-{}", id);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit(&event_name, PtyOutput { id, data });
                }
                Err(_) => break,
            }
        }
        // PTY closed — notify frontend
        let _ = app_clone.emit(&format!("pty-exit-{}", id), id);
    });

    let info = PtyInfo {
        id,
        shell: shell_path.clone(),
        cols,
        rows,
    };

    let manager = app.state::<PtyManager>();
    manager.sessions.lock().await.insert(
        id,
        PtySession {
            master: pair.master,
            writer,
            _child: child,
            shell: shell_path,
            cols,
            rows,
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn pty_write(app: AppHandle, id: u32, data: String) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("PTY session {} not found", id))?;

    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write to PTY: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn pty_resize(app: AppHandle, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("PTY session {} not found", id))?;

    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to resize PTY: {}", e))?;

    session.cols = cols;
    session.rows = rows;

    Ok(())
}

#[tauri::command]
pub async fn pty_kill(app: AppHandle, id: u32) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;

    if sessions.remove(&id).is_some() {
        Ok(())
    } else {
        Err(format!("PTY session {} not found", id))
    }
}

#[tauri::command]
pub async fn pty_list(app: AppHandle) -> Result<Vec<PtyInfo>, String> {
    let manager = app.state::<PtyManager>();
    let sessions = manager.sessions.lock().await;

    Ok(sessions
        .iter()
        .map(|(id, s)| PtyInfo {
            id: *id,
            shell: s.shell.clone(),
            cols: s.cols,
            rows: s.rows,
        })
        .collect())
}
