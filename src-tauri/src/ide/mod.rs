//! IDE Module - Filesystem & AI Agent Tool System
//!
//! Provides Claude-Code-like capabilities where local AI models
//! can interact with the filesystem: read, write, search, and execute.

pub mod ai_complete;
pub mod billing;
pub mod collab;
pub mod db_client;
pub mod debug;
pub mod git;
pub mod http_client;
pub mod lsp;
pub mod pty;
pub mod indexer;
pub mod shadow;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// File entry for directory listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub extension: Option<String>,
}

/// Search result from grep-like operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line: u32,
    pub content: String,
}

/// Tool call from AI agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "args")]
pub enum AgentTool {
    #[serde(rename = "read_file")]
    ReadFile { path: String },
    #[serde(rename = "write_file")]
    WriteFile { path: String, content: String },
    #[serde(rename = "list_dir")]
    ListDir { path: String },
    #[serde(rename = "search")]
    Search { pattern: String, path: String },
    #[serde(rename = "execute")]
    Execute { command: String, cwd: Option<String> },
}

/// Tool result returned to the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Validate path is within allowed scope (home directory)
fn validate_path(path: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let resolved = if path.starts_with('~') {
        home.join(&path[2..])
    } else {
        PathBuf::from(path)
    };

    let canonical = resolved.canonicalize().unwrap_or(resolved.clone());

    if !canonical.starts_with(&home) && !canonical.starts_with("/tmp") {
        return Err(format!(
            "Access denied: path '{}' is outside allowed scope",
            path
        ));
    }

    Ok(canonical)
}

/// Read a directory listing
#[tauri::command]
pub async fn ide_read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let dir_path = validate_path(&path)?;

    let mut entries = Vec::new();
    let mut read_dir = fs::read_dir(&dir_path)
        .await
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("Failed to read entry: {}", e))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        let (is_dir, size) = match entry.metadata().await {
            Ok(m) => (m.is_dir(), m.len()),
            Err(_) => (false, 0),
        };

        entries.push(FileEntry {
            extension: Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_string()),
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir,
            size,
        });
    }

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Read a file's content
#[tauri::command]
pub async fn ide_read_file(path: String) -> Result<String, String> {
    let file_path = validate_path(&path)?;

    // Size check: don't read files > 5MB
    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|e| format!("File not found: {}", e))?;

    if metadata.len() > 5 * 1024 * 1024 {
        return Err("File too large (>5MB). Use a streaming approach.".to_string());
    }

    fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

/// Write content to a file
#[tauri::command]
pub async fn ide_write_file(path: String, content: String) -> Result<(), String> {
    let file_path = validate_path(&path)?;

    // Create parent directories if needed
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    fs::write(&file_path, &content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))
}

/// Search for text patterns in files (grep-like)
#[tauri::command]
pub async fn ide_search_files(
    pattern: String,
    search_path: String,
    max_results: Option<u32>,
) -> Result<Vec<SearchMatch>, String> {
    let dir_path = validate_path(&search_path)?;
    let max = max_results.unwrap_or(50) as usize;

    let mut matches = Vec::new();
    search_recursive(&dir_path, &pattern, &mut matches, max).await;

    Ok(matches)
}

/// Recursive file search
#[async_recursion::async_recursion]
async fn search_recursive(
    dir: &Path,
    pattern: &str,
    matches: &mut Vec<SearchMatch>,
    max: usize,
) {
    if matches.len() >= max {
        return;
    }

    let mut entries = match fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        if matches.len() >= max {
            break;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dirs and common non-text dirs
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "__pycache__"
            || name == ".git"
        {
            continue;
        }

        if path.is_dir() {
            search_recursive(&path, pattern, matches, max).await;
        } else if is_text_file(&name) {
            if let Ok(content) = fs::read_to_string(&path).await {
                let pattern_lower = pattern.to_lowercase();
                for (i, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&pattern_lower) {
                        matches.push(SearchMatch {
                            file: path.to_string_lossy().to_string(),
                            line: (i + 1) as u32,
                            content: line.trim().to_string(),
                        });
                        if matches.len() >= max {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Check if a file is likely a text file based on extension
fn is_text_file(name: &str) -> bool {
    let text_extensions = [
        "rs", "py", "ts", "tsx", "js", "jsx", "svelte", "html", "css", "scss",
        "json", "toml", "yaml", "yml", "md", "txt", "sh", "bash", "zsh",
        "sql", "xml", "csv", "env", "conf", "cfg", "ini", "lock",
        "c", "cpp", "h", "hpp", "cs", "java", "go", "rb", "php",
        "vue", "astro", "dockerfile", "makefile",
    ];

    if let Some(ext) = Path::new(name).extension() {
        text_extensions
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e))
    } else {
        // Files without extension: Makefile, Dockerfile, etc.
        let lower = name.to_lowercase();
        lower == "makefile" || lower == "dockerfile" || lower == "readme"
    }
}

/// Execute a shell command (sandboxed)
#[tauri::command]
pub async fn ide_execute_command(
    command: String,
    cwd: Option<String>,
) -> Result<ToolResult, String> {
    // Security: only allow safe commands
    let cmd_name = command.split_whitespace().next().unwrap_or("");
    let allowed = [
        "ls", "cat", "head", "tail", "wc", "find", "grep", "rg",
        "git", "cargo", "pnpm", "npm", "node", "python3", "python",
        "rustc", "rustfmt", "clippy-driver", "echo", "pwd", "which",
        "tree", "file", "stat", "diff", "patch",
    ];

    if !allowed.iter().any(|a| *a == cmd_name) {
        return Ok(ToolResult {
            tool: "execute".to_string(),
            success: false,
            output: String::new(),
            error: Some(format!(
                "Command '{}' is not in the allowed list. Allowed: {:?}",
                cmd_name, allowed
            )),
        });
    }

    let working_dir = if let Some(ref dir) = cwd {
        validate_path(dir)?
    } else {
        dirs::home_dir().ok_or("Cannot determine home directory")?
    };

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&working_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Truncate output to 50KB
    let truncated = if stdout.len() > 50_000 {
        format!("{}...\n[truncated, {} bytes total]", &stdout[..50_000], stdout.len())
    } else {
        stdout
    };

    Ok(ToolResult {
        tool: "execute".to_string(),
        success: output.status.success(),
        output: truncated,
        error: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
    })
}

/// Execute an AI agent tool call and return result
#[tauri::command]
pub async fn ide_agent_tool_call(tool: AgentTool) -> Result<ToolResult, String> {
    match tool {
        AgentTool::ReadFile { path } => {
            match ide_read_file(path.clone()).await {
                Ok(content) => Ok(ToolResult {
                    tool: "read_file".to_string(),
                    success: true,
                    output: content,
                    error: None,
                }),
                Err(e) => Ok(ToolResult {
                    tool: "read_file".to_string(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                }),
            }
        }
        AgentTool::WriteFile { path, content } => {
            match ide_write_file(path.clone(), content).await {
                Ok(()) => Ok(ToolResult {
                    tool: "write_file".to_string(),
                    success: true,
                    output: format!("Successfully wrote to {}", path),
                    error: None,
                }),
                Err(e) => Ok(ToolResult {
                    tool: "write_file".to_string(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                }),
            }
        }
        AgentTool::ListDir { path } => {
            match ide_read_dir(path).await {
                Ok(entries) => Ok(ToolResult {
                    tool: "list_dir".to_string(),
                    success: true,
                    output: serde_json::to_string_pretty(&entries).unwrap_or_default(),
                    error: None,
                }),
                Err(e) => Ok(ToolResult {
                    tool: "list_dir".to_string(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                }),
            }
        }
        AgentTool::Search { pattern, path } => {
            match ide_search_files(pattern, path, Some(50)).await {
                Ok(results) => Ok(ToolResult {
                    tool: "search".to_string(),
                    success: true,
                    output: serde_json::to_string_pretty(&results).unwrap_or_default(),
                    error: None,
                }),
                Err(e) => Ok(ToolResult {
                    tool: "search".to_string(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                }),
            }
        }
        AgentTool::Execute { command, cwd } => {
            ide_execute_command(command, cwd).await
        }
    }
}
