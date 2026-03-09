//! Codebase Indexer — Semantic code chunking for search
//!
//! Parses source files into semantic chunks (functions, classes, structs)
//! using simple heuristic splitting. Later upgradeable to full Tree-sitter AST.
//! Provides text-based search as initial implementation.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub node_type: String,
    pub name: String,
    pub language: String,
    pub content: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub indexed_files: u32,
    pub total_chunks: u32,
    pub last_indexed: Option<String>,
    pub indexing: bool,
    pub workspace: String,
}

pub struct CodebaseIndexer {
    status: Mutex<IndexStatus>,
    chunks: Mutex<Vec<CodeChunk>>,
}

impl CodebaseIndexer {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(IndexStatus {
                indexed_files: 0,
                total_chunks: 0,
                last_indexed: None,
                indexing: false,
                workspace: String::new(),
            }),
            chunks: Mutex::new(Vec::new()),
        }
    }
}

/// Detect language from file extension
fn detect_language(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?;
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "svelte" => Some("svelte"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" => Some("cpp"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        _ => None,
    }
}

/// Extract semantic chunks from source code using heuristic boundary detection
fn extract_chunks(content: &str, file_path: &str, language: &str) -> Vec<CodeChunk> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return chunks;
    }

    let mut current_start = 0;
    let mut current_name = file_path.split('/').last().unwrap_or("unknown").to_string();
    let mut current_type = "module".to_string();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        let boundary = match language {
            "rust" => {
                if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
                    Some(("function", extract_name(trimmed, "fn ")))
                } else if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                    Some(("struct", extract_name(trimmed, "struct ")))
                } else if trimmed.starts_with("impl ") {
                    Some(("impl", extract_name(trimmed, "impl ")))
                } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
                    Some(("enum", extract_name(trimmed, "enum ")))
                } else if trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") {
                    Some(("trait", extract_name(trimmed, "trait ")))
                } else {
                    None
                }
            }
            "python" => {
                if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                    Some(("function", extract_name(trimmed, "def ")))
                } else if trimmed.starts_with("class ") {
                    Some(("class", extract_name(trimmed, "class ")))
                } else {
                    None
                }
            }
            "typescript" | "javascript" | "svelte" => {
                if trimmed.starts_with("function ") || trimmed.starts_with("export function ") {
                    Some(("function", extract_name(trimmed, "function ")))
                } else if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                    Some(("class", extract_name(trimmed, "class ")))
                } else if trimmed.starts_with("export default ") {
                    Some(("export", "default".to_string()))
                } else if trimmed.contains("const ") && trimmed.contains(" = ") && (trimmed.contains("=>") || trimmed.contains("function")) {
                    Some(("function", extract_const_name(trimmed)))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some((node_type, name)) = boundary {
            // Save previous chunk if substantial
            if i > current_start + 2 {
                let chunk_content: String = lines[current_start..i].join("\n");
                chunks.push(CodeChunk {
                    file_path: file_path.to_string(),
                    start_line: current_start as u32 + 1,
                    end_line: i as u32,
                    node_type: current_type.clone(),
                    name: current_name.clone(),
                    language: language.to_string(),
                    content: chunk_content,
                    score: None,
                });
            }
            current_start = i;
            current_name = name;
            current_type = node_type.to_string();
        }
    }

    // Final chunk
    if current_start < lines.len() {
        let chunk_content: String = lines[current_start..].join("\n");
        chunks.push(CodeChunk {
            file_path: file_path.to_string(),
            start_line: current_start as u32 + 1,
            end_line: lines.len() as u32,
            node_type: current_type,
            name: current_name,
            language: language.to_string(),
            content: chunk_content,
            score: None,
        });
    }

    chunks
}

fn extract_name(line: &str, keyword: &str) -> String {
    let after = if let Some(pos) = line.find(keyword) {
        &line[pos + keyword.len()..]
    } else {
        line
    };
    // Remove "pub " prefix if present
    let after = after.strip_prefix("pub ").unwrap_or(after);
    after
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

fn extract_const_name(line: &str) -> String {
    let trimmed = line.trim();
    let after_const = trimmed
        .strip_prefix("export ")
        .unwrap_or(trimmed)
        .strip_prefix("const ")
        .unwrap_or(trimmed);
    after_const
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

/// Index an entire codebase directory
#[tauri::command]
pub async fn index_codebase(app: AppHandle, workspace_path: String) -> Result<IndexStatus, String> {
    let indexer = app.state::<CodebaseIndexer>();

    {
        let mut status = indexer.status.lock().await;
        if status.indexing {
            return Err("Indexing already in progress".to_string());
        }
        status.indexing = true;
        status.workspace = workspace_path.clone();
    }

    let mut all_chunks = Vec::new();
    let mut file_count = 0u32;

    // Walk the directory
    for entry in walkdir::WalkDir::new(&workspace_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let path_str = path.to_string_lossy().to_string();

        // Skip hidden, node_modules, target, .git
        if path_str.contains("/.")
            || path_str.contains("/node_modules/")
            || path_str.contains("/target/")
            || path_str.contains("/__pycache__/")
        {
            continue;
        }

        if let Some(language) = detect_language(&path_str) {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                // Skip very large files (>500KB)
                if content.len() > 500_000 {
                    continue;
                }
                let chunks = extract_chunks(&content, &path_str, language);
                all_chunks.extend(chunks);
                file_count += 1;
            }
        }
    }

    let chunk_count = all_chunks.len() as u32;

    // Store chunks
    {
        let mut stored = indexer.chunks.lock().await;
        *stored = all_chunks;
    }

    // Update status
    let status = {
        let mut s = indexer.status.lock().await;
        s.indexed_files = file_count;
        s.total_chunks = chunk_count;
        s.last_indexed = Some(chrono::Utc::now().to_rfc3339());
        s.indexing = false;
        s.clone()
    };

    Ok(status)
}

/// Search indexed codebase with text matching (semantic search placeholder)
#[tauri::command]
pub async fn search_codebase(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<CodeChunk>, String> {
    let indexer = app.state::<CodebaseIndexer>();
    let chunks = indexer.chunks.lock().await;
    let limit = limit.unwrap_or(10);
    let query_lower = query.to_lowercase();

    let mut results: Vec<CodeChunk> = chunks
        .iter()
        .filter_map(|chunk| {
            let content_lower = chunk.content.to_lowercase();
            let name_lower = chunk.name.to_lowercase();

            // Simple relevance scoring
            let mut score = 0.0f32;
            if name_lower.contains(&query_lower) {
                score += 2.0;
            }
            if content_lower.contains(&query_lower) {
                score += 1.0;
                // Bonus for exact word match
                let count = content_lower.matches(&query_lower).count();
                score += (count as f32).min(5.0) * 0.2;
            }

            if score > 0.0 {
                let mut result = chunk.clone();
                result.score = Some(score);
                Some(result)
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .unwrap_or(0.0)
            .partial_cmp(&a.score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    Ok(results)
}

/// Get current index status
#[tauri::command]
pub async fn index_status(app: AppHandle) -> Result<IndexStatus, String> {
    let indexer = app.state::<CodebaseIndexer>();
    let status = indexer.status.lock().await;
    Ok(status.clone())
}
