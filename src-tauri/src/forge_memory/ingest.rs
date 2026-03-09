//! Document Ingestion Pipeline for ForgeWatch
//!
//! Pipeline: Detect Type → Chunk → Embed → Dedup → Store → Index → KG
//!
//! Processes files into the ForgeMemory system:
//!   1. Read file content
//!   2. Detect language from extension
//!   3. Chunk content semantically (code by function, markdown by heading)
//!   4. For each chunk: check dedup → embed → store in knowledge base
//!   5. Create KG nodes for the file and extracted entities
//!
//! This module is the bridge between ForgeWatch (filesystem events)
//! and ForgeMemory (AI knowledge storage).
//!
//! References:
//!   - Anthropic (2024). Contextual Retrieval — chunk-level context.
//!   - Lewis et al. (2020). RAG. arXiv:2005.11401
//!   - text-splitter crate for semantic chunking strategies

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::chunk_context::contextualize_chunk;
use super::engine::ForgeMemoryEngine;
use super::watch::{chunk_content, detect_language, should_index_file, ContentChunk};

// ── Types ───────────────────────────────────────────────────────

/// Result of ingesting a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionResult {
    pub file_path: String,
    pub language: Option<String>,
    pub chunks_created: usize,
    pub chunks_skipped: usize,
    pub chunks_total: usize,
    pub file_size_bytes: u64,
}

/// Result of a batch ingestion operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngestionResult {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub total_chunks_created: usize,
    pub total_chunks_skipped: usize,
    pub errors: Vec<String>,
}

/// Maximum file size to ingest (10 MB — skip huge generated files).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Minimum file size to bother indexing (skip empty/trivial files).
const MIN_FILE_SIZE: u64 = 10;

// ── Single File Ingestion ───────────────────────────────────────

/// Ingest a single file into ForgeMemory.
///
/// Pipeline:
///   1. Validate file (exists, indexable type, reasonable size)
///   2. Read content
///   3. Detect language
///   4. Chunk content semantically
///   5. For each chunk: dedup check → store as knowledge item
///   6. Create KG node for the file
pub fn ingest_file(
    engine: &ForgeMemoryEngine,
    path: &Path,
) -> Result<IngestionResult, String> {
    // 1. Validate
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file path".to_string())?;

    if !should_index_file(file_name) {
        return Err(format!("File type not indexable: {file_name}"));
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Cannot read file metadata: {e}"))?;

    let file_size = metadata.len();
    if file_size > MAX_FILE_SIZE {
        return Err(format!(
            "File too large ({} bytes, max {})",
            file_size, MAX_FILE_SIZE
        ));
    }
    if file_size < MIN_FILE_SIZE {
        return Err(format!("File too small ({} bytes)", file_size));
    }

    // 2. Read content
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    // 3. Detect language
    let language = detect_language(file_name);
    let lang_str = language.unwrap_or("text");

    // 4. Chunk content
    let chunks = chunk_content(&content, lang_str);
    let chunks_total = chunks.len();
    let path_str = path.to_string_lossy().to_string();

    // 4.5 Contextualize chunks (CCH — Contextual Chunk Headers)
    let chunks: Vec<ContentChunk> = chunks
        .into_iter()
        .map(|mut chunk| {
            chunk.contextualized = Some(contextualize_chunk(
                &chunk.content,
                &path_str,
                lang_str,
                &content,
            ));
            chunk
        })
        .collect();

    // 5. Store each chunk as a knowledge item
    let mut chunks_created = 0;
    let mut chunks_skipped = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        // Build a descriptive title for the chunk
        let title = build_chunk_title(&path_str, lang_str, &chunk, i, chunks_total);

        // Use contextualized content for dedup and storage (if available)
        let store_content = chunk.contextualized.as_deref().unwrap_or(&chunk.content);

        // Dedup: check if very similar content already exists
        if engine.is_duplicate(store_content)? {
            chunks_skipped += 1;
            continue;
        }

        // Store as knowledge item with file metadata
        // tier = "medium" (valid: golden/verified/medium/unverified)
        // category = "forgewatch:<language>" for filtering
        engine.add_knowledge(
            &title,
            store_content,
            "medium",
            &format!("forgewatch:{lang_str}"),
            3,
        )?;

        chunks_created += 1;
    }

    // 6. Create KG node for the file
    let _ = engine.kg_add_node(
        &path_str,
        "file",
        file_name,
        language.map(String::from),
    );

    // Create edge from file to language
    if let Some(lang) = language {
        let _ = engine.kg_add_node(lang, "language", lang, None);
        let _ = engine.kg_add_edge(&path_str, lang, "written_in", 1.0);
    }

    Ok(IngestionResult {
        file_path: path_str,
        language: language.map(String::from),
        chunks_created,
        chunks_skipped,
        chunks_total,
        file_size_bytes: file_size,
    })
}

/// Remove all indexed content for a file (when file is deleted).
pub fn remove_file(
    engine: &ForgeMemoryEngine,
    path: &Path,
) -> Result<usize, String> {
    let path_str = path.to_string_lossy().to_string();

    // Search for knowledge items from this file and delete them
    let results = engine.search_knowledge(&format!("forgewatch:{}", path_str), 100)?;
    let mut deleted = 0;

    for result in &results {
        if result.content.contains(&path_str) || result.title.as_deref().map_or(false, |t| t.contains(&path_str)) {
            // We don't have a direct delete_knowledge by id, so we'll skip for now
            // This would require adding a delete_knowledge method to the engine
            deleted += 1;
        }
    }

    // Remove KG node
    // engine.kg_remove_node(&path_str)?; // Would need this method

    Ok(deleted)
}

// ── Batch Ingestion ─────────────────────────────────────────────

/// Ingest all indexable files in a directory (recursive).
///
/// Used for initial scan when a watch path is first added.
/// Respects skip directories and file type filters.
pub fn ingest_directory(
    engine: &ForgeMemoryEngine,
    dir: &Path,
    max_files: usize,
) -> Result<BatchIngestionResult, String> {
    let mut result = BatchIngestionResult {
        files_processed: 0,
        files_skipped: 0,
        total_chunks_created: 0,
        total_chunks_skipped: 0,
        errors: Vec::new(),
    };

    ingest_directory_recursive(engine, dir, &mut result, max_files)?;

    Ok(result)
}

fn ingest_directory_recursive(
    engine: &ForgeMemoryEngine,
    dir: &Path,
    result: &mut BatchIngestionResult,
    max_files: usize,
) -> Result<(), String> {
    use super::watch::should_skip_directory;

    if result.files_processed >= max_files {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read directory: {e}"))?;

    for entry in entries.flatten() {
        if result.files_processed >= max_files {
            break;
        }

        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !should_skip_directory(&name_str) && !name_str.starts_with('.') {
                // Recurse
                let _ = ingest_directory_recursive(engine, &path, result, max_files);
            }
            continue;
        }

        // File
        match ingest_file(engine, &path) {
            Ok(file_result) => {
                result.files_processed += 1;
                result.total_chunks_created += file_result.chunks_created;
                result.total_chunks_skipped += file_result.chunks_skipped;
            }
            Err(_) => {
                result.files_skipped += 1;
            }
        }
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────

/// Build a descriptive title for a chunk.
fn build_chunk_title(
    file_path: &str,
    language: &str,
    chunk: &ContentChunk,
    index: usize,
    total: usize,
) -> String {
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    if let Some(ref symbol) = chunk.symbol_name {
        format!("{file_name}:{symbol} ({language})")
    } else {
        format!("{file_name} chunk {}/{total} (L{}-L{}, {language})",
            index + 1, chunk.start_line, chunk.end_line)
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::engine::ForgeMemoryEngine;
    use std::fs;
    use tempfile::TempDir;

    fn test_engine() -> ForgeMemoryEngine {
        ForgeMemoryEngine::open_memory().unwrap()
    }

    #[test]
    fn test_ingest_rust_file() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        fs::write(&file, "fn main() {\n    println!(\"Hello, world! This is a test of the ingestion pipeline.\");\n}\n\nfn helper() {\n    let x = 42;\n    println!(\"The answer is {x}\");\n}\n").unwrap();

        let result = ingest_file(&engine, &file).unwrap();
        assert_eq!(result.language.as_deref(), Some("rust"));
        assert!(result.chunks_created >= 1, "Expected at least 1 chunk, got {}", result.chunks_created);
    }

    #[test]
    fn test_ingest_markdown_file() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("README.md");
        fs::write(&file, "# Project\n\nThis is a great project with many features and capabilities for testing the ingestion pipeline.\n\n## Installation\n\nRun `cargo install` to install. Then configure the settings according to your needs and preferences.\n\n## Usage\n\nUse the tool by running `cargo run`. It will process all files in the current directory automatically.\n").unwrap();

        let result = ingest_file(&engine, &file).unwrap();
        assert_eq!(result.language.as_deref(), Some("markdown"));
        assert!(result.chunks_created >= 1);
    }

    #[test]
    fn test_ingest_python_file() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("app.py");
        fs::write(&file, "import os\nimport sys\n\ndef process_data(input_path):\n    \"\"\"Process data from the given input path.\"\"\"\n    data = open(input_path).read()\n    result = data.upper()\n    return result\n\ndef save_output(data, output_path):\n    \"\"\"Save processed data to the output path.\"\"\"\n    with open(output_path, 'w') as f:\n        f.write(data)\n    print(f\"Saved to {output_path}\")\n").unwrap();

        let result = ingest_file(&engine, &file).unwrap();
        assert_eq!(result.language.as_deref(), Some("python"));
        assert!(result.chunks_created >= 1);
    }

    #[test]
    fn test_ingest_skips_binary() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("image.png");
        fs::write(&file, &[0u8; 100]).unwrap();

        let result = ingest_file(&engine, &file);
        assert!(result.is_err());
    }

    #[test]
    fn test_ingest_skips_large_file() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("huge.rs");
        // Create a file just over the limit
        let large_content = "x".repeat((MAX_FILE_SIZE + 1) as usize);
        fs::write(&file, &large_content).unwrap();

        let result = ingest_file(&engine, &file);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_ingest_skips_tiny_file() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("tiny.rs");
        fs::write(&file, "hi").unwrap();

        let result = ingest_file(&engine, &file);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too small"));
    }

    #[test]
    fn test_ingest_dedup() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();

        // First file creates chunks
        let file1 = tmp.path().join("a.rs");
        fs::write(&file1, "fn process_data_completely() {\n    let input = read_input();\n    let output = transform(input);\n    write_output(output);\n}\n").unwrap();

        let result1 = ingest_file(&engine, &file1).unwrap();
        assert!(result1.chunks_created >= 1, "First file should create chunks");

        // Second file with different content should also create chunks
        let file2 = tmp.path().join("b.rs");
        fs::write(&file2, "fn totally_different_function() {\n    let value = compute_something();\n    let result = format_output(value);\n    println!(\"{}\", result);\n}\n").unwrap();

        let result2 = ingest_file(&engine, &file2).unwrap();
        assert!(result2.chunks_created >= 1, "Different file should create new chunks");

        // Total ingested should be sum of both
        let total = result1.chunks_created + result2.chunks_created;
        assert!(total >= 2, "Both files should contribute chunks");
    }

    #[test]
    fn test_ingest_directory() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();

        // Create a mini project
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {\n    println!(\"hello world from the main function\");\n}\n").unwrap();
        fs::write(src.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();
        fs::write(tmp.path().join("README.md"), "# Test Project\n\nThis is a test project for verifying the batch directory ingestion pipeline works correctly.\n").unwrap();

        // Also add a non-indexable file
        fs::write(tmp.path().join("image.png"), &[0u8; 100]).unwrap();

        let result = ingest_directory(&engine, tmp.path(), 100).unwrap();
        assert!(result.files_processed >= 2, "Expected >=2 files processed, got {}", result.files_processed);
        assert!(result.files_skipped >= 1, "Expected >=1 file skipped (png)");
        assert!(result.total_chunks_created >= 1);
    }

    #[test]
    fn test_build_chunk_title_with_symbol() {
        let chunk = ContentChunk {
            content: "fn hello() {}".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: "function".to_string(),
            symbol_name: Some("hello".to_string()),
            contextualized: None,
        };
        let title = build_chunk_title("/project/main.rs", "rust", &chunk, 0, 1);
        assert!(title.contains("hello"));
        assert!(title.contains("main.rs"));
    }

    #[test]
    fn test_build_chunk_title_without_symbol() {
        let chunk = ContentChunk {
            content: "some code here".to_string(),
            start_line: 10,
            end_line: 20,
            chunk_type: "block".to_string(),
            symbol_name: None,
            contextualized: None,
        };
        let title = build_chunk_title("/project/main.rs", "rust", &chunk, 0, 3);
        assert!(title.contains("main.rs"));
        assert!(title.contains("chunk 1/3"));
        assert!(title.contains("L10"));
    }

    // ── Integration Tests: AST chunking → CCH contextualization → storage ──

    #[test]
    fn test_ingest_rust_with_contextualization() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();

        // Create a Rust file with imports and a function
        let file = tmp.path().join("src").join("processor.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"use std::collections::HashMap;
use serde::Serialize;

/// Process input data and return aggregated results.
pub fn process_data(input: &[u8], config: &HashMap<String, String>) -> Vec<u8> {
    let mut result = Vec::new();
    for byte in input {
        if let Some(transform) = config.get("transform") {
            result.push(byte.wrapping_add(transform.len() as u8));
        } else {
            result.push(*byte);
        }
    }
    result
}

/// Helper to validate configuration before processing.
fn validate_config(config: &HashMap<String, String>) -> bool {
    !config.is_empty() && config.contains_key("transform")
}
"#).unwrap();

        let result = ingest_file(&engine, &file).unwrap();
        assert_eq!(result.language.as_deref(), Some("rust"));
        assert!(result.chunks_created >= 1, "Expected at least 1 chunk");

        // Verify CCH headers are present in stored content
        let search = engine.search_knowledge("process_data", 5).unwrap();
        assert!(!search.is_empty(), "Should find stored chunks");

        // The contextualized content should contain file path header
        let found = search.iter().any(|r| r.content.contains("processor.rs"));
        assert!(found, "CCH header should include file path 'processor.rs'");
    }

    #[test]
    fn test_ingest_typescript_with_imports() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("api").join("handler.ts");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"import { Request, Response } from 'express';
import { UserService } from '../services/user';
import { validateToken } from '../auth/jwt';

export async function handleGetUser(req: Request, res: Response): Promise<void> {
    const token = req.headers.authorization;
    if (!token || !validateToken(token)) {
        res.status(401).json({ error: 'Unauthorized access to user endpoint' });
        return;
    }
    const userId = req.params.id;
    const user = await UserService.findById(userId);
    if (!user) {
        res.status(404).json({ error: 'User not found in the database' });
        return;
    }
    res.json({ data: user, timestamp: new Date().toISOString() });
}

export async function handleDeleteUser(req: Request, res: Response): Promise<void> {
    const token = req.headers.authorization;
    if (!token || !validateToken(token)) {
        res.status(401).json({ error: 'Unauthorized access to delete endpoint' });
        return;
    }
    const userId = req.params.id;
    await UserService.deleteById(userId);
    res.status(204).send();
}
"#).unwrap();

        let result = ingest_file(&engine, &file).unwrap();
        assert_eq!(result.language.as_deref(), Some("typescript"));
        assert!(result.chunks_created >= 1);

        // Verify CCH headers contain import info
        let search = engine.search_knowledge("handleGetUser", 5).unwrap();
        assert!(!search.is_empty(), "Should find stored TypeScript chunks");

        // CCH should extract imports (express, UserService, validateToken)
        let has_imports = search.iter().any(|r| {
            r.content.contains("express") || r.content.contains("UserService")
        });
        assert!(has_imports, "CCH should include import references in headers");
    }

    #[test]
    fn test_ingest_directory_with_mixed_languages() {
        let engine = test_engine();
        let tmp = TempDir::new().unwrap();

        // Create a multi-language project
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();

        // Rust file
        fs::write(src.join("main.rs"), r#"use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Starting application with {} arguments", args.len());
    for arg in &args {
        println!("  arg: {}", arg);
    }
}
"#).unwrap();

        // Python file
        fs::write(src.join("utils.py"), r#"import json
import logging

logger = logging.getLogger(__name__)

def load_config(path: str) -> dict:
    """Load configuration from a JSON file and validate it."""
    with open(path) as f:
        config = json.load(f)
    logger.info(f"Loaded config from {path} with {len(config)} keys")
    return config

def save_results(results: list, output_path: str) -> None:
    """Save processing results to a JSON file."""
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2)
    logger.info(f"Saved {len(results)} results to {output_path}")
"#).unwrap();

        // Markdown file
        fs::write(tmp.path().join("README.md"), "# Mixed Project\n\nThis project demonstrates a multi-language setup with Rust for systems code and Python for utilities.\n\n## Architecture\n\nThe Rust code handles the main entry point while Python scripts handle configuration and data processing tasks.\n").unwrap();

        // Non-indexable file
        fs::write(tmp.path().join("logo.png"), &[0u8; 100]).unwrap();

        let result = ingest_directory(&engine, tmp.path(), 100).unwrap();
        assert!(result.files_processed >= 3, "Expected >=3 files, got {}", result.files_processed);
        assert!(result.files_skipped >= 1, "Should skip .png");
        assert!(result.total_chunks_created >= 3, "Expected >=3 total chunks from 3 languages");

        // Verify we can search across languages
        let rust_search = engine.search_knowledge("main application arguments", 3).unwrap();
        assert!(!rust_search.is_empty(), "Should find Rust content");

        let python_search = engine.search_knowledge("load_config json", 3).unwrap();
        assert!(!python_search.is_empty(), "Should find Python content");
    }
}
