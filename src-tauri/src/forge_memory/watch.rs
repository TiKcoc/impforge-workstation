//! ForgeWatch — Intelligent Filesystem Watcher Engine
//!
//! Monitors user-configured directories for file changes and feeds content
//! into the ForgeMemory ingestion pipeline. Designed for AI workstations
//! where developers want their local files to be searchable and contextual.
//!
//! Architecture:
//!   - `notify` v8.2 (Rust-native inotify/FSEvents/ReadDirectoryChanges)
//!   - Two-tier debouncing: OS-level 2s (notify-debouncer-full) + app-level batch
//!   - Language-aware semantic chunking (code by function, markdown by heading)
//!   - Auto-discovery of git repos and documentation directories
//!   - Configurable file type filters and skip patterns
//!
//! References:
//!   - notify crate: https://docs.rs/notify/8 (cross-platform fs events)
//!   - Semantic Chunking for RAG: Anthropic (2024) Contextual Retrieval
//!   - Text Splitting: Langchain RecursiveCharacterTextSplitter patterns
//!   - HNSW indexing: Malkov & Yashunin (2018) for vector search after ingest

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use text_splitter::CodeSplitter;
use tokio::sync::mpsc;

use super::tree_sitter_langs::get_tree_sitter_language;

// ── Types ───────────────────────────────────────────────────────

/// A discovered path worth watching (git repo, docs folder, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPath {
    pub path: String,
    pub reason: String,
    pub project_type: Option<String>,
    pub estimated_files: usize,
}

/// A user-configured watched path with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedPath {
    pub path: String,
    pub label: Option<String>,
    pub recursive: bool,
    pub enabled: bool,
    pub scan_mode: ScanMode,
}

/// How often to scan for changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScanMode {
    Realtime,
    Hourly,
    Manual,
}

impl Default for ScanMode {
    fn default() -> Self {
        ScanMode::Realtime
    }
}

/// Status of the ForgeWatch engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatus {
    pub running: bool,
    pub watched_paths: usize,
    pub total_files_indexed: usize,
    pub events_processed: u64,
    pub last_event_at: Option<String>,
}

/// A content chunk extracted from a file for indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: String, // "function", "struct", "heading", "section", "block"
    pub symbol_name: Option<String>,
    /// Contextualized content with CCH header prepended.
    /// Set by the ingestion pipeline after chunking.
    pub contextualized: Option<String>,
}

/// Result of a file change event processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeResult {
    pub path: String,
    pub action: String,
    pub chunks_updated: usize,
}

// ── File Type Filters ───────────────────────────────────────────

/// Indexable file extensions (code, docs, config — NOT binaries/images).
const INDEXABLE_EXTENSIONS: &[&str] = &[
    // Code
    "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "kt", "swift",
    "c", "cpp", "h", "hpp", "cs", "rb", "php", "lua", "zig", "nim",
    "ex", "exs", "clj", "scala", "r", "jl", "dart", "v", "odin",
    // Shell
    "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd",
    // Web
    "html", "css", "scss", "sass", "less", "svelte", "vue", "astro",
    // Config
    "toml", "yaml", "yml", "json", "xml", "ini", "cfg", "conf",
    "env", "editorconfig", "prettierrc",
    // Docs
    "md", "mdx", "txt", "rst", "adoc", "org", "tex",
    // Data
    "sql", "graphql", "proto", "avsc",
    // Build
    "makefile", "cmake", "gradle", "dockerfile",
    // Other
    "gitignore", "dockerignore",
];

/// Files to always skip (lock files, generated, etc.)
const SKIP_FILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
    "go.sum",
    ".DS_Store",
    "thumbs.db",
];

/// Directories to always skip.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    ".next",
    ".nuxt",
    "dist",
    "build",
    ".cache",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".cargo",
    ".rustup",
    "vendor",
    "bower_components",
    ".svelte-kit",
    ".turbo",
    "coverage",
    ".idea",
    ".vscode",
    ".settings",
    "bin",
    "obj",
    ".gradle",
    "out",
];

/// Check if a file should be indexed based on its name/extension.
pub fn should_index_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();

    // Skip known lock/generated files
    if SKIP_FILES.iter().any(|s| lower == *s) {
        return false;
    }

    // Check extension
    if let Some(ext) = Path::new(&lower).extension().and_then(|e| e.to_str()) {
        return INDEXABLE_EXTENSIONS.contains(&ext);
    }

    // Extensionless files that are indexable
    let extensionless_indexable = [
        "makefile", "dockerfile", "rakefile", "gemfile",
        "procfile", "vagrantfile", "justfile",
    ];
    extensionless_indexable.iter().any(|s| lower == *s)
}

/// Check if a directory should be skipped entirely.
pub fn should_skip_directory(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

// ── Language Detection ──────────────────────────────────────────

/// Detect programming language from file extension.
pub fn detect_language(filename: &str) -> Option<&'static str> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())?
        .to_lowercase();

    match ext.as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" | "cxx" => Some("cpp"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "lua" => Some("lua"),
        "zig" => Some("zig"),
        "ex" | "exs" => Some("elixir"),
        "clj" => Some("clojure"),
        "scala" => Some("scala"),
        "r" => Some("r"),
        "jl" => Some("julia"),
        "dart" => Some("dart"),
        "sh" | "bash" | "zsh" => Some("shell"),
        "html" => Some("html"),
        "css" | "scss" | "sass" | "less" => Some("css"),
        "svelte" => Some("svelte"),
        "vue" => Some("vue"),
        "md" | "mdx" => Some("markdown"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "xml" => Some("xml"),
        "sql" => Some("sql"),
        "txt" | "rst" | "adoc" | "org" => Some("text"),
        "tex" => Some("latex"),
        "proto" => Some("protobuf"),
        "graphql" => Some("graphql"),
        _ => None,
    }
}

// ── Semantic Chunking ───────────────────────────────────────────

/// Maximum chunk size in characters (~500 tokens at 4 chars/token).
const MAX_CHUNK_CHARS: usize = 2000;
/// Minimum meaningful chunk size.
const MIN_CHUNK_CHARS: usize = 50;

/// Chunk file content using AST-aware splitting via tree-sitter.
///
/// Uses text-splitter's CodeSplitter with tree-sitter grammars for semantic
/// code chunking. Falls back to sliding window for unknown languages.
///
/// Scientific basis: cAST (arXiv:2506.15655) — AST-aware chunking achieves
/// +4.3 Recall@5, +6.7 Pass@1 vs fixed-size splitting (EMNLP 2025).
pub fn chunk_content(text: &str, language: &str) -> Vec<ContentChunk> {
    if text.len() < MIN_CHUNK_CHARS {
        return vec![ContentChunk {
            content: text.to_string(),
            start_line: 1,
            end_line: text.lines().count().max(1),
            chunk_type: "file".to_string(),
            symbol_name: None,
            contextualized: None,
        }];
    }

    // Try AST-aware chunking with tree-sitter grammar
    if let Some(ts_lang) = get_tree_sitter_language(language) {
        if let Ok(splitter) = CodeSplitter::new(ts_lang, MAX_CHUNK_CHARS) {
            let raw_chunks: Vec<&str> = splitter.chunks(text).collect();
            if !raw_chunks.is_empty() {
                let result: Vec<ContentChunk> = raw_chunks
                    .into_iter()
                    .filter(|c| c.len() >= MIN_CHUNK_CHARS)
                    .map(|chunk_text| {
                        let (start, end) = find_line_range(text, chunk_text);
                        ContentChunk {
                            content: chunk_text.to_string(),
                            start_line: start,
                            end_line: end,
                            chunk_type: "ast".to_string(),
                            symbol_name: extract_first_symbol(chunk_text),
                            contextualized: None,
                        }
                    })
                    .collect();
                if !result.is_empty() {
                    return result;
                }
            }
        }
    }

    // Fallback: sliding window for unknown/unsupported languages
    chunk_sliding_window(text)
}

/// Find the line range of a chunk within the full text.
///
/// Returns (start_line, end_line) as 1-indexed line numbers.
fn find_line_range(full_text: &str, chunk: &str) -> (usize, usize) {
    // Find byte offset of chunk in full text
    let chunk_ptr = chunk.as_ptr() as usize;
    let text_ptr = full_text.as_ptr() as usize;

    // If the chunk is a slice of the original text, we can compute offset directly
    if chunk_ptr >= text_ptr && chunk_ptr < text_ptr + full_text.len() {
        let byte_offset = chunk_ptr - text_ptr;
        let start_line = full_text[..byte_offset].matches('\n').count() + 1;
        let end_line = start_line + chunk.matches('\n').count();
        return (start_line, end_line);
    }

    // Fallback: search for the chunk text (less efficient but always works)
    if let Some(pos) = full_text.find(chunk) {
        let start_line = full_text[..pos].matches('\n').count() + 1;
        let end_line = start_line + chunk.matches('\n').count();
        return (start_line, end_line);
    }

    (1, 1)
}

/// Extract the first symbol name from a chunk of code.
///
/// Looks for common definition patterns (fn, def, class, struct, etc.)
/// in the chunk and returns the first symbol found.
fn extract_first_symbol(chunk: &str) -> Option<String> {
    for line in chunk.lines() {
        if let Some(name) = extract_symbol_name(line) {
            return Some(name);
        }
    }
    None
}

/// Fallback: sliding window chunking with paragraph/sentence boundaries.
fn chunk_sliding_window(text: &str) -> Vec<ContentChunk> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return chunks;
    }

    let mut current_start = 0;
    let mut current_len = 0;
    let mut chunk_start_line = 0;

    for (i, line) in lines.iter().enumerate() {
        current_len += line.len() + 1; // +1 for newline

        if current_len >= MAX_CHUNK_CHARS {
            let content: String = lines[chunk_start_line..=i].join("\n");
            if content.len() >= MIN_CHUNK_CHARS {
                chunks.push(ContentChunk {
                    content,
                    start_line: chunk_start_line + 1,
                    end_line: i + 1,
                    chunk_type: "block".to_string(),
                    symbol_name: None,
                    contextualized: None,
                });
            }
            chunk_start_line = i + 1;
            current_len = 0;
        }
    }

    // Flush remaining
    if chunk_start_line < lines.len() {
        let content: String = lines[chunk_start_line..].join("\n");
        if content.len() >= MIN_CHUNK_CHARS {
            chunks.push(ContentChunk {
                content,
                start_line: chunk_start_line + 1,
                end_line: lines.len(),
                chunk_type: "block".to_string(),
                symbol_name: None,
                contextualized: None,
            });
        }
    }

    chunks
}

/// Extract a symbol name from a definition line.
fn extract_symbol_name(line: &str) -> Option<String> {
    let trimmed = line.trim();

    // Try common patterns: "fn name(", "def name(", "class Name", "struct Name"
    // Order matters: longer prefixes first (e.g., "pub async fn " before "fn ")
    for keyword in ["pub async fn ", "async fn ", "pub fn ", "fn ",
                    "async def ", "def ", "class ", "pub struct ", "struct ",
                    "pub enum ", "enum ", "pub trait ", "trait ",
                    "impl", "func ", "pub type ", "type "] {
        if let Some(rest) = trimmed.strip_prefix(keyword) {
            // For "impl", skip whitespace and possible generic angle brackets
            let rest = rest.trim_start();
            // Take alphanumeric + underscore chars (stop at generic '<', '(' or whitespace)
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

// ── Auto-Discovery ──────────────────────────────────────────────

/// Auto-discover interesting directories to watch.
///
/// Scans the given root path (typically $HOME) for:
///   - Git repositories (.git directory)
///   - Rust projects (Cargo.toml)
///   - Node.js projects (package.json)
///   - Python projects (pyproject.toml, setup.py)
///   - Documentation directories (docs/, wiki/)
///
/// Stops at depth 4 to avoid deep scanning. Skips SKIP_DIRS.
pub fn auto_discover(root: &Path) -> Vec<DiscoveredPath> {
    let mut discoveries = Vec::new();
    let mut visited = HashSet::new();

    discover_recursive(root, &mut discoveries, &mut visited, 0, 4);
    discoveries
}

fn discover_recursive(
    dir: &Path,
    results: &mut Vec<DiscoveredPath>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    if !dir.is_dir() {
        return;
    }
    let canonical = match dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return,
    };
    if !visited.insert(canonical.clone()) {
        return;
    }

    // Check for project markers
    let has_git = dir.join(".git").exists();
    let has_cargo = dir.join("Cargo.toml").exists();
    let has_package_json = dir.join("package.json").exists();
    let has_pyproject = dir.join("pyproject.toml").exists() || dir.join("setup.py").exists();
    let has_go_mod = dir.join("go.mod").exists();

    if has_git || has_cargo || has_package_json || has_pyproject || has_go_mod {
        let mut reasons = Vec::new();
        let mut project_type = None;

        if has_git {
            reasons.push("git repository");
        }
        if has_cargo {
            reasons.push("Rust project");
            project_type = Some("rust");
        }
        if has_package_json {
            reasons.push("Node.js project");
            project_type = project_type.or(Some("nodejs"));
        }
        if has_pyproject {
            reasons.push("Python project");
            project_type = project_type.or(Some("python"));
        }
        if has_go_mod {
            reasons.push("Go project");
            project_type = project_type.or(Some("go"));
        }

        // Estimate file count (fast — just count immediate children)
        let estimated = std::fs::read_dir(dir)
            .map(|entries| entries.count())
            .unwrap_or(0);

        results.push(DiscoveredPath {
            path: canonical.to_string_lossy().to_string(),
            reason: reasons.join(", "),
            project_type: project_type.map(String::from),
            estimated_files: estimated,
        });
    }

    // Recurse into subdirectories (skip known heavy ones)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !should_skip_directory(&name_str) && !name_str.starts_with('.') {
                    discover_recursive(&path, results, visited, depth + 1, max_depth);
                }
            }
        }
    }
}

// ── ForgeWatcher (Runtime) ──────────────────────────────────────

/// The runtime filesystem watcher.
///
/// Wraps `notify::RecommendedWatcher` with debounced event processing.
/// File events are sent to an async channel for processing by the
/// ingestion pipeline without blocking the watcher thread.
pub struct ForgeWatcher {
    watched_paths: Arc<RwLock<Vec<WatchedPath>>>,
    events_processed: Arc<std::sync::atomic::AtomicU64>,
    running: Arc<std::sync::atomic::AtomicBool>,
    /// Channel sender for file events (receiver is in the ingestion task)
    event_tx: Option<mpsc::Sender<WatchEvent>>,
}

/// Internal watch event (debounced, filtered).
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: WatchEventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Removed,
}

impl ForgeWatcher {
    /// Create a new ForgeWatcher (not yet started).
    pub fn new() -> Self {
        Self {
            watched_paths: Arc::new(RwLock::new(Vec::new())),
            events_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_tx: None,
        }
    }

    /// Add a path to watch.
    pub fn add_path(&self, path: &str, label: Option<&str>, scan_mode: ScanMode) {
        let mut paths = self.watched_paths.write();
        // Avoid duplicates
        if paths.iter().any(|p| p.path == path) {
            return;
        }
        paths.push(WatchedPath {
            path: path.to_string(),
            label: label.map(String::from),
            recursive: true,
            enabled: true,
            scan_mode,
        });
    }

    /// Remove a watched path.
    pub fn remove_path(&self, path: &str) -> bool {
        let mut paths = self.watched_paths.write();
        let before = paths.len();
        paths.retain(|p| p.path != path);
        paths.len() < before
    }

    /// List all watched paths.
    pub fn list_paths(&self) -> Vec<WatchedPath> {
        self.watched_paths.read().clone()
    }

    /// Get current status.
    pub fn status(&self) -> WatchStatus {
        WatchStatus {
            running: self.running.load(std::sync::atomic::Ordering::Relaxed),
            watched_paths: self.watched_paths.read().len(),
            total_files_indexed: 0, // Updated by ingestion pipeline
            events_processed: self.events_processed.load(std::sync::atomic::Ordering::Relaxed),
            last_event_at: None,
        }
    }

    /// Start the filesystem watcher (spawns a background thread).
    ///
    /// Returns a receiver for watch events that should be consumed by
    /// the ingestion pipeline.
    pub fn start(&mut self) -> Result<mpsc::Receiver<WatchEvent>, String> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Watcher already running".to_string());
        }

        let (tx, rx) = mpsc::channel(1000);
        self.event_tx = Some(tx.clone());

        let paths = self.watched_paths.clone();
        let events_counter = self.events_processed.clone();
        let running = self.running.clone();

        running.store(true, std::sync::atomic::Ordering::Relaxed);

        // Spawn the watcher thread with notify-debouncer-full (OS-level 2s coalescing)
        std::thread::spawn(move || {
            let tx_clone = tx.clone();
            let events_counter_clone = events_counter.clone();

            // Create debouncer with 2-second OS-level event coalescing
            // This replaces the manual 500ms debounce with proper OS-level dedup
            // (notify-debouncer-full coalesces IDE save storms: VS Code emits 3-5 events per save)
            let mut debouncer = match new_debouncer(
                std::time::Duration::from_secs(2),
                None, // No tick rate override — use default
                move |result: DebounceEventResult| {
                    match result {
                        Ok(events) => {
                            for event in events {
                                for path in &event.paths {
                                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                        if should_index_file(name) {
                                            let kind = if path.exists() {
                                                WatchEventKind::Modified
                                            } else {
                                                WatchEventKind::Removed
                                            };
                                            let _ = tx_clone.blocking_send(WatchEvent {
                                                path: path.clone(),
                                                kind,
                                            });
                                            events_counter_clone.fetch_add(
                                                1,
                                                std::sync::atomic::Ordering::Relaxed,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(errors) => {
                            for e in errors {
                                log::warn!("Watch error: {e}");
                            }
                        }
                    }
                },
            ) {
                Ok(d) => d,
                Err(e) => {
                    log::error!("Failed to create debounced watcher: {e}");
                    running.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            // Register all enabled paths
            let watched = paths.read().clone();
            for wp in &watched {
                if wp.enabled && wp.scan_mode == ScanMode::Realtime {
                    let mode = if wp.recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    };
                    if let Err(e) = debouncer.watch(Path::new(&wp.path), mode) {
                        log::warn!("Failed to watch {}: {e}", wp.path);
                    }
                }
            }

            // Keep thread alive while watcher is running
            // The debouncer handles all event processing via its callback
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            // Debouncer is dropped here, which stops the underlying watcher
        });

        Ok(rx)
    }

    /// Stop the filesystem watcher.
    pub fn stop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        self.event_tx = None;
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── File filter tests ──────────────────────────────

    #[test]
    fn test_should_index_code_files() {
        assert!(should_index_file("main.rs"));
        assert!(should_index_file("app.py"));
        assert!(should_index_file("index.ts"));
        assert!(should_index_file("Component.tsx"));
        assert!(should_index_file("main.go"));
        assert!(should_index_file("App.java"));
        assert!(should_index_file("program.cs"));
        assert!(should_index_file("lib.zig"));
    }

    #[test]
    fn test_should_index_doc_files() {
        assert!(should_index_file("README.md"));
        assert!(should_index_file("guide.txt"));
        assert!(should_index_file("config.toml"));
        assert!(should_index_file("docker-compose.yml"));
    }

    #[test]
    fn test_should_not_index_binaries() {
        assert!(!should_index_file("image.png"));
        assert!(!should_index_file("photo.jpg"));
        assert!(!should_index_file("binary.exe"));
        assert!(!should_index_file("archive.zip"));
        assert!(!should_index_file("video.mp4"));
        assert!(!should_index_file("font.woff2"));
    }

    #[test]
    fn test_should_not_index_lock_files() {
        assert!(!should_index_file("package-lock.json"));
        assert!(!should_index_file("Cargo.lock"));
        assert!(!should_index_file("yarn.lock"));
        assert!(!should_index_file("pnpm-lock.yaml"));
    }

    #[test]
    fn test_should_index_extensionless() {
        assert!(should_index_file("Makefile"));
        assert!(should_index_file("Dockerfile"));
        assert!(should_index_file("Justfile"));
    }

    // ── Directory filter tests ─────────────────────────

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(should_skip_directory("target"));
        assert!(should_skip_directory("__pycache__"));
        assert!(should_skip_directory(".venv"));
        assert!(should_skip_directory("dist"));
        assert!(should_skip_directory("coverage"));
    }

    #[test]
    fn test_should_not_skip_source_dirs() {
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("docs"));
        assert!(!should_skip_directory("lib"));
        assert!(!should_skip_directory("tests"));
        assert!(!should_skip_directory("examples"));
    }

    // ── Language detection tests ───────────────────────

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust"));
        assert_eq!(detect_language("app.py"), Some("python"));
        assert_eq!(detect_language("index.ts"), Some("typescript"));
        assert_eq!(detect_language("app.tsx"), Some("typescript"));
        assert_eq!(detect_language("main.go"), Some("go"));
        assert_eq!(detect_language("App.java"), Some("java"));
        assert_eq!(detect_language("Program.cs"), Some("csharp"));
        assert_eq!(detect_language("README.md"), Some("markdown"));
        assert_eq!(detect_language("config.toml"), Some("toml"));
        assert_eq!(detect_language("photo.jpg"), None);
    }

    // ── Chunking tests ────────────────────────────────

    #[test]
    fn test_chunk_rust_code() {
        let code = r#"use std::io;

fn hello() {
    println!("hello");
}

fn world() {
    println!("world");
}

struct Point {
    x: f64,
    y: f64,
}
"#;
        let chunks = chunk_content(code, "rust");
        assert!(chunks.len() >= 1, "Expected at least 1 chunk, got {}", chunks.len());
        // Verify chunks contain the functions
        let all_content: String = chunks.iter().map(|c| c.content.clone()).collect();
        assert!(all_content.contains("fn hello"));
        assert!(all_content.contains("fn world"));
    }

    #[test]
    fn test_chunk_python_code() {
        let code = "import os\nimport sys\n\ndef greet(name):\n    \"\"\"Greet someone by name with a friendly message.\"\"\"\n    message = f\"Hello {name}, welcome to the system!\"\n    print(message)\n    return message\n\nclass Person:\n    \"\"\"A person with a name and greeting capability.\"\"\"\n    def __init__(self, name):\n        self.name = name\n        self.greetings = []\n\n    def say_hello(self):\n        \"\"\"Say hello and record the greeting.\"\"\"\n        greeting = f\"Hi, I'm {self.name}\"\n        self.greetings.append(greeting)\n        print(greeting)\n        return greeting\n\ndef farewell(name):\n    \"\"\"Say farewell to someone with a friendly message.\"\"\"\n    message = f\"Goodbye {name}, see you next time!\"\n    print(message)\n    return message\n";
        let chunks = chunk_content(code, "python");
        assert!(chunks.len() >= 1, "Expected at least 1 chunk, got {}", chunks.len());
        let all_content: String = chunks.iter().map(|c| c.content.clone()).collect::<Vec<_>>().join("\n");
        assert!(all_content.contains("def greet"), "Missing 'def greet' in chunks");
        assert!(all_content.contains("class Person"), "Missing 'class Person' in chunks");
    }

    #[test]
    fn test_chunk_markdown() {
        let md = "# Title\n\nThis is the introduction paragraph with enough content to pass the minimum chunk size threshold. It provides important context about the document.\n\n## Section 1\n\nContent for section one is here with detailed explanations about the first topic. This needs to be long enough to exceed minimum chunk size.\n\n## Section 2\n\nContent for section two is here with additional information about the second topic. This section also needs sufficient length.";
        let chunks = chunk_content(md, "markdown");
        // CodeSplitter may produce fewer chunks than heading-based splitting
        // when the total text fits within MAX_CHUNK_CHARS
        assert!(chunks.len() >= 1, "Expected >=1 markdown chunks, got {}", chunks.len());
        assert!(chunks[0].chunk_type == "ast", "Expected 'ast' chunk type for markdown, got '{}'", chunks[0].chunk_type);
    }

    #[test]
    fn test_chunk_small_file() {
        let small = "hello world";
        let chunks = chunk_content(small, "text");
        // Small files should not be chunked (below MIN_CHUNK_CHARS)
        assert!(chunks.len() <= 1);
    }

    #[test]
    fn test_chunk_sliding_window() {
        // Generate a long text
        let text = (0..100).map(|i| format!("Line {} of the document with some content.", i)).collect::<Vec<_>>().join("\n");
        let chunks = chunk_content(&text, "text");
        assert!(chunks.len() >= 1);
        // Verify no chunk exceeds max size significantly
        for chunk in &chunks {
            assert!(chunk.content.len() <= MAX_CHUNK_CHARS + 200, "Chunk too large: {} chars", chunk.content.len());
        }
    }

    // ── Symbol extraction tests ───────────────────────

    #[test]
    fn test_extract_symbol_name() {
        assert_eq!(extract_symbol_name("fn hello()"), Some("hello".to_string()));
        assert_eq!(extract_symbol_name("pub fn process_data(x: i32)"), Some("process_data".to_string()));
        assert_eq!(extract_symbol_name("struct Point {"), Some("Point".to_string()));
        assert_eq!(extract_symbol_name("class MyClass:"), Some("MyClass".to_string()));
        assert_eq!(extract_symbol_name("def greet(name):"), Some("greet".to_string()));
        assert_eq!(extract_symbol_name("impl Foo {"), Some("Foo".to_string()));
    }

    // ── Auto-discovery tests ──────────────────────────

    #[test]
    fn test_auto_discover_git_repo() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("myproject");
        fs::create_dir_all(proj.join(".git")).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(proj.join("main.rs"), "fn main() {}").unwrap();

        let discoveries = auto_discover(tmp.path());
        assert!(!discoveries.is_empty(), "Expected at least one discovery");
        assert!(discoveries[0].reason.contains("git"));
        assert!(discoveries[0].reason.contains("Rust"));
        assert_eq!(discoveries[0].project_type.as_deref(), Some("rust"));
    }

    #[test]
    fn test_auto_discover_node_project() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("webapp");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("package.json"), "{}").unwrap();

        let discoveries = auto_discover(tmp.path());
        assert!(!discoveries.is_empty());
        assert!(discoveries[0].reason.contains("Node.js"));
    }

    #[test]
    fn test_auto_discover_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let discoveries = auto_discover(tmp.path());
        assert!(discoveries.is_empty());
    }

    #[test]
    fn test_auto_discover_python_project() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("pyapp");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();

        let discoveries = auto_discover(tmp.path());
        assert!(!discoveries.is_empty());
        assert!(discoveries[0].reason.contains("Python"));
    }

    // ── Watcher lifecycle tests ───────────────────────

    #[test]
    fn test_watcher_add_remove_paths() {
        let watcher = ForgeWatcher::new();
        watcher.add_path("/tmp/test", Some("Test"), ScanMode::Realtime);
        assert_eq!(watcher.list_paths().len(), 1);

        // No duplicates
        watcher.add_path("/tmp/test", Some("Test2"), ScanMode::Realtime);
        assert_eq!(watcher.list_paths().len(), 1);

        // Remove
        assert!(watcher.remove_path("/tmp/test"));
        assert_eq!(watcher.list_paths().len(), 0);

        // Remove non-existent
        assert!(!watcher.remove_path("/tmp/nonexistent"));
    }

    #[test]
    fn test_watcher_status_initial() {
        let watcher = ForgeWatcher::new();
        let status = watcher.status();
        assert!(!status.running);
        assert_eq!(status.watched_paths, 0);
        assert_eq!(status.events_processed, 0);
    }

    // ── AST-aware chunking tests ─────────────────────

    #[test]
    fn test_chunk_ast_rust() {
        let code = r#"use std::io;

/// A greeting function.
fn hello(name: &str) {
    println!("Hello, {}!", name);
}

/// A farewell function.
fn goodbye(name: &str) {
    println!("Goodbye, {}!", name);
}

struct Config {
    name: String,
    value: i32,
}

impl Config {
    fn new(name: &str, value: i32) -> Self {
        Self { name: name.to_string(), value }
    }
}
"#;
        let chunks = chunk_content(code, "rust");
        assert!(!chunks.is_empty(), "AST chunking should produce chunks");
        // All chunks should be AST type (not "block" from sliding window)
        for chunk in &chunks {
            assert_eq!(chunk.chunk_type, "ast", "Expected AST chunks, got: {}", chunk.chunk_type);
        }
    }

    #[test]
    fn test_chunk_ast_python() {
        let code = r#"import os
import sys

def process_data(input_path):
    """Process data from the given input path and return results."""
    data = open(input_path).read()
    result = data.upper()
    return result

class DataProcessor:
    """Handles data processing operations with configuration."""
    def __init__(self, config):
        self.config = config
        self.results = []

    def run(self):
        """Execute the data processing pipeline."""
        for item in self.config.items:
            result = self.process_item(item)
            self.results.append(result)
        return self.results
"#;
        let chunks = chunk_content(code, "python");
        assert!(!chunks.is_empty(), "AST chunking should produce Python chunks");
        for chunk in &chunks {
            assert_eq!(chunk.chunk_type, "ast");
        }
    }

    #[test]
    fn test_chunk_fallback_unknown() {
        // Unknown language should fall back to sliding window
        let text = (0..50).map(|i| format!("Line {} with enough content to make chunks.", i))
            .collect::<Vec<_>>().join("\n");
        let chunks = chunk_content(&text, "unknown_lang");
        assert!(!chunks.is_empty(), "Fallback should produce chunks");
        for chunk in &chunks {
            assert_eq!(chunk.chunk_type, "block", "Fallback should produce 'block' chunks");
        }
    }
}
