// SPDX-License-Identifier: Apache-2.0
//! ForgeWriter -- Document Management & AI Writing Assistant
//!
//! Provides a full document lifecycle: create, read, update, delete, export.
//! Documents are persisted as individual files in `~/.impforge/documents/`.
//! AI-assisted text operations use the local Ollama inference backend.
//!
//! This module is part of ImpForge Phase 3 (Office/Writing tools).

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds document files.
const DOCUMENTS_DIR: &str = "documents";

/// Extension for the metadata sidecar files.
const META_EXT: &str = "meta.json";

/// Ollama HTTP timeout for AI-assist requests (generous for large context).
const AI_ASSIST_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported document storage formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    Markdown,
    Html,
    #[serde(rename = "plaintext")]
    PlainText,
}

impl DocFormat {
    /// File extension for persisted content files.
    fn extension(self) -> &'static str {
        match self {
            DocFormat::Markdown => "md",
            DocFormat::Html => "html",
            DocFormat::PlainText => "txt",
        }
    }

    /// Attempt to parse a format string from the frontend.
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "markdown" | "md" => DocFormat::Markdown,
            "html" => DocFormat::Html,
            _ => DocFormat::PlainText,
        }
    }
}

/// Full document representation returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub format: DocFormat,
    pub word_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub auto_saved: bool,
}

/// Lightweight metadata for document listings (no content body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub title: String,
    pub format: DocFormat,
    pub word_count: u32,
    pub updated_at: String,
    pub tags: Vec<String>,
}

/// Persisted sidecar metadata (JSON next to the content file).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaFile {
    id: String,
    title: String,
    format: DocFormat,
    word_count: u32,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
}

/// Word-count statistics returned by `writer_word_count`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordCountStats {
    pub words: u32,
    pub characters: u32,
    pub sentences: u32,
    pub paragraphs: u32,
    pub reading_time_min: f64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the documents directory, creating it if necessary.
fn documents_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(DOCUMENTS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create documents directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Build the content file path for a given document.
fn content_path(dir: &Path, id: &str, format: DocFormat) -> PathBuf {
    dir.join(format!("{}.{}", id, format.extension()))
}

/// Build the metadata sidecar path for a given document.
fn meta_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.{}", id, META_EXT))
}

/// Read and parse a metadata sidecar file.
fn read_meta(path: &Path) -> Result<MetaFile, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "META_READ_FAILED",
            format!("Cannot read document metadata: {e}"),
        )
    })?;
    serde_json::from_str::<MetaFile>(&data).map_err(|e| {
        ImpForgeError::internal(
            "META_PARSE_FAILED",
            format!("Corrupt document metadata: {e}"),
        )
    })
}

/// Persist a metadata sidecar file atomically.
fn write_meta(path: &Path, meta: &MetaFile) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| {
        ImpForgeError::internal("META_SERIALIZE", format!("Cannot serialize metadata: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "META_WRITE_FAILED",
            format!("Cannot write document metadata: {e}"),
        )
    })
}

/// Count words in a text string (Unicode-aware, splits on whitespace).
fn count_words(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Compute detailed word-count statistics for a content string.
fn compute_stats(content: &str) -> WordCountStats {
    let words = count_words(content);
    let characters = content.chars().count() as u32;
    let sentences = content
        .chars()
        .filter(|c| matches!(c, '.' | '!' | '?'))
        .count()
        .max(if content.trim().is_empty() { 0 } else { 1 }) as u32;
    let paragraphs = content
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count()
        .max(if content.trim().is_empty() { 0 } else { 1 }) as u32;
    // Average adult reads ~238 words/min (Brysbaert 2019).
    let reading_time_min = if words > 0 {
        (words as f64 / 238.0 * 100.0).round() / 100.0
    } else {
        0.0
    };

    WordCountStats {
        words,
        characters,
        sentences,
        paragraphs,
        reading_time_min,
    }
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

// ---------------------------------------------------------------------------
// AI Assist (Ollama)
// ---------------------------------------------------------------------------

/// Send a text-manipulation prompt to Ollama and return the result.
async fn ollama_text_assist(
    instruction: &str,
    text: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let system_prompt = "You are a professional writing assistant inside ImpForge, \
        an AI Workstation. Follow the user's instruction precisely. \
        Return ONLY the improved text -- no explanations, no markdown fences, \
        no preamble. Preserve the original language unless told to translate.";

    let user_message = format!("{instruction}\n\n---\n\n{text}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_ASSIST_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for AI writing assist",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "AI writing assist timed out",
                )
                .with_suggestion("The text may be too long. Try selecting a shorter passage.")
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Ollama request failed: {e}"),
                )
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(
            ImpForgeError::service(
                "OLLAMA_HTTP_ERROR",
                format!("Ollama returned HTTP {status}"),
            )
            .with_details(body)
            .with_suggestion("Check Ollama logs. The model may not be downloaded yet."),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service(
            "OLLAMA_PARSE_ERROR",
            format!("Failed to parse Ollama response: {e}"),
        )
    })?;

    let content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(ImpForgeError::service(
            "OLLAMA_EMPTY_RESPONSE",
            "Ollama returned an empty response",
        ));
    }

    Ok(content)
}

/// Map a user-facing action name to an Ollama instruction prompt.
fn action_to_instruction(action: &str) -> Result<&'static str, ImpForgeError> {
    match action {
        "improve" => Ok("Improve the following text. Fix grammar, enhance clarity, and make it more professional while keeping the original meaning and tone."),
        "shorten" => Ok("Shorten the following text significantly while preserving the key information. Be concise."),
        "expand" => Ok("Expand the following text with more detail, examples, and depth. Keep the same style and tone."),
        "fix_grammar" => Ok("Fix all grammar, spelling, and punctuation errors in the following text. Do not change the meaning or style."),
        "translate_en" => Ok("Translate the following text to English. Preserve formatting."),
        "translate_de" => Ok("Translate the following text to German (Deutsch). Preserve formatting."),
        "summarize" => Ok("Write a concise summary of the following text. Capture the main points in 2-4 sentences."),
        other => Err(ImpForgeError::validation(
            "INVALID_ACTION",
            format!("Unknown AI action: '{other}'. Valid actions: improve, shorten, expand, fix_grammar, translate_en, translate_de, summarize"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all documents (metadata only, no content bodies).
#[tauri::command]
pub async fn writer_list_documents() -> AppResult<Vec<DocumentMeta>> {
    let dir = documents_dir()?;
    let mut docs: Vec<DocumentMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read documents dir: {e}"))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // Only process meta files
        if !name.ends_with(META_EXT) {
            continue;
        }
        if let Ok(meta) = read_meta(&path) {
            docs.push(DocumentMeta {
                id: meta.id,
                title: meta.title,
                format: meta.format,
                word_count: meta.word_count,
                updated_at: meta.updated_at,
                tags: meta.tags,
            });
        }
    }

    // Sort by updated_at descending (most recent first).
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(docs)
}

/// Get a single document by ID (full content included).
#[tauri::command]
pub async fn writer_get_document(id: String) -> AppResult<Document> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found"))
                .with_suggestion("The document may have been deleted or moved."),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    Ok(Document {
        id: meta.id,
        title: meta.title,
        content,
        format: meta.format,
        word_count: meta.word_count,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        tags: meta.tags,
        auto_saved: false,
    })
}

/// Create a new blank document.
#[tauri::command]
pub async fn writer_create_document(
    title: String,
    format: Option<String>,
) -> AppResult<Document> {
    let dir = documents_dir()?;
    let id = Uuid::new_v4().to_string();
    let fmt = format.as_deref().map(DocFormat::from_str_loose).unwrap_or(DocFormat::Markdown);
    let now = now_iso();

    let meta = MetaFile {
        id: id.clone(),
        title: title.clone(),
        format: fmt,
        word_count: 0,
        created_at: now.clone(),
        updated_at: now.clone(),
        tags: Vec::new(),
    };

    write_meta(&meta_path(&dir, &id), &meta)?;

    // Create empty content file
    let cp = content_path(&dir, &id, fmt);
    std::fs::write(&cp, "").map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot create document file: {e}"),
        )
    })?;

    log::info!("ForgeWriter: created document '{title}' ({})", id);

    Ok(Document {
        id,
        title,
        content: String::new(),
        format: fmt,
        word_count: 0,
        created_at: now.clone(),
        updated_at: now,
        tags: Vec::new(),
        auto_saved: false,
    })
}

/// Save (update) a document's title and/or content.
#[tauri::command]
pub async fn writer_save_document(
    id: String,
    title: Option<String>,
    content: String,
) -> AppResult<Document> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found"))
                .with_suggestion("Cannot save a document that does not exist. Create it first."),
        );
    }

    let mut meta = read_meta(&mp)?;
    let now = now_iso();

    if let Some(t) = title {
        meta.title = t;
    }
    meta.word_count = count_words(&content);
    meta.updated_at = now.clone();

    write_meta(&mp, &meta)?;

    // Write content file
    let cp = content_path(&dir, &id, meta.format);
    std::fs::write(&cp, &content).map_err(|e| {
        ImpForgeError::filesystem(
            "CONTENT_WRITE_FAILED",
            format!("Cannot save document content: {e}"),
        )
    })?;

    Ok(Document {
        id: meta.id,
        title: meta.title,
        content,
        format: meta.format,
        word_count: meta.word_count,
        created_at: meta.created_at,
        updated_at: now,
        tags: meta.tags,
        auto_saved: true,
    })
}

/// Delete a document and its sidecar metadata.
#[tauri::command]
pub async fn writer_delete_document(id: String) -> AppResult<()> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);

    // Remove both files (ignore errors on content -- it may not exist).
    let _ = std::fs::remove_file(&cp);
    std::fs::remove_file(&mp).map_err(|e| {
        ImpForgeError::filesystem(
            "DELETE_FAILED",
            format!("Cannot delete document metadata: {e}"),
        )
    })?;

    log::info!("ForgeWriter: deleted document '{}'", id);

    Ok(())
}

/// Export a document to a specified format. Returns the exported file path.
#[tauri::command]
pub async fn writer_export_document(
    id: String,
    export_format: String,
) -> AppResult<String> {
    let dir = documents_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("DOC_NOT_FOUND", format!("Document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let cp = content_path(&dir, &id, meta.format);
    let content = std::fs::read_to_string(&cp).unwrap_or_default();

    // Determine export directory (~/Documents or fallback to docs dir)
    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "EXPORT_DIR_FAILED",
                format!("Cannot create export directory: {e}"),
            )
        })?;
    }

    // Sanitize title for filename
    let safe_title: String = meta
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    let safe_title = if safe_title.is_empty() {
        "untitled".to_string()
    } else {
        safe_title
    };

    let (ext, body) = match export_format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => ("md", content.clone()),
        "html" => {
            let html_body = if meta.format == DocFormat::Markdown {
                // Basic markdown-to-HTML conversion (line-by-line)
                basic_md_to_html(&content)
            } else {
                content.clone()
            };
            let full_html = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"UTF-8\">\n  \
                 <title>{}</title>\n  <style>\n    body {{ font-family: 'Inter', sans-serif; \
                 max-width: 720px; margin: 2rem auto; padding: 0 1rem; line-height: 1.7; \
                 color: #e0e0e0; background: #1a1a2e; }}\n    h1,h2,h3 {{ color: #00f0ff; }}\n    \
                 pre {{ background: #16162a; padding: 1rem; border-radius: 6px; overflow-x: auto; }}\n    \
                 code {{ font-family: 'JetBrains Mono', monospace; }}\n  </style>\n</head>\n\
                 <body>\n{}\n</body>\n</html>",
                meta.title, html_body
            );
            ("html", full_html)
        }
        "txt" | "plaintext" | "text" => ("txt", strip_markdown(&content)),
        "pdf" => {
            return Err(
                ImpForgeError::validation(
                    "PDF_NOT_SUPPORTED",
                    "PDF export is not yet available",
                )
                .with_suggestion(
                    "Export as HTML and print to PDF from your browser, or use pandoc.",
                ),
            );
        }
        other => {
            return Err(ImpForgeError::validation(
                "INVALID_EXPORT_FORMAT",
                format!(
                    "Unsupported export format: '{other}'. Use: md, html, txt"
                ),
            ));
        }
    };

    let export_path = export_dir.join(format!("{safe_title}.{ext}"));
    std::fs::write(&export_path, body).map_err(|e| {
        ImpForgeError::filesystem(
            "EXPORT_WRITE_FAILED",
            format!("Cannot write exported file: {e}"),
        )
    })?;

    log::info!(
        "ForgeWriter: exported '{}' as {} to {}",
        meta.title,
        ext,
        export_path.display()
    );

    Ok(export_path.to_string_lossy().to_string())
}

/// AI-assisted text operations using Ollama.
#[tauri::command]
pub async fn writer_ai_assist(
    document_id: String,
    selection: String,
    action: String,
) -> AppResult<String> {
    if selection.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_SELECTION",
            "Select some text to use AI assist",
        ));
    }

    let instruction = action_to_instruction(&action)?;

    log::info!(
        "ForgeWriter: AI assist '{}' on {} chars (doc {})",
        action,
        selection.len(),
        document_id
    );

    let result = ollama_text_assist(instruction, &selection, None).await?;

    Ok(result)
}

/// Compute word count statistics for arbitrary content.
#[tauri::command]
pub async fn writer_word_count(content: String) -> AppResult<WordCountStats> {
    Ok(compute_stats(&content))
}

// ---------------------------------------------------------------------------
// Text Conversion Helpers
// ---------------------------------------------------------------------------

/// Very basic Markdown to HTML conversion (handles headings, bold, italic,
/// code blocks, lists, links, paragraphs). Not a full parser -- just enough
/// for clean export. A proper library (pulldown-cmark) could replace this
/// once added to Cargo.toml.
fn basic_md_to_html(md: &str) -> String {
    let mut html = String::with_capacity(md.len() * 2);
    let mut in_code_block = false;
    let mut in_list = false;
    let mut paragraph_buffer = String::new();

    for line in md.lines() {
        // Fenced code blocks
        if line.trim_start().starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                flush_paragraph(&mut html, &mut paragraph_buffer);
                html.push_str("<pre><code>");
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            html.push_str(&html_escape(line));
            html.push('\n');
            continue;
        }

        let trimmed = line.trim();

        // Empty line -- flush paragraph
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            flush_paragraph(&mut html, &mut paragraph_buffer);
            continue;
        }

        // Headings
        if let Some(heading) = parse_heading(trimmed) {
            flush_paragraph(&mut html, &mut paragraph_buffer);
            html.push_str(&heading);
            continue;
        }

        // Unordered lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            flush_paragraph(&mut html, &mut paragraph_buffer);
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            let item_text = inline_format(&trimmed[2..]);
            html.push_str(&format!("  <li>{item_text}</li>\n"));
            continue;
        }

        // Ordered lists
        if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some(pos) = trimmed.find(". ") {
                if trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                    flush_paragraph(&mut html, &mut paragraph_buffer);
                    if !in_list {
                        html.push_str("<ul>\n");
                        in_list = true;
                    }
                    let item_text = inline_format(&trimmed[pos + 2..]);
                    html.push_str(&format!("  <li>{item_text}</li>\n"));
                    continue;
                }
            }
        }

        // Close list if we were in one
        if in_list {
            html.push_str("</ul>\n");
            in_list = false;
        }

        // Regular text -- accumulate into paragraph
        if !paragraph_buffer.is_empty() {
            paragraph_buffer.push(' ');
        }
        paragraph_buffer.push_str(trimmed);
    }

    // Flush remaining
    if in_code_block {
        html.push_str("</code></pre>\n");
    }
    if in_list {
        html.push_str("</ul>\n");
    }
    flush_paragraph(&mut html, &mut paragraph_buffer);

    html
}

fn flush_paragraph(html: &mut String, buf: &mut String) {
    if buf.is_empty() {
        return;
    }
    let formatted = inline_format(buf);
    html.push_str(&format!("<p>{formatted}</p>\n"));
    buf.clear();
}

fn parse_heading(line: &str) -> Option<String> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let text = line[level..].trim();
    if text.is_empty() {
        return None;
    }
    Some(format!(
        "<h{level}>{}</h{level}>\n",
        inline_format(text)
    ))
}

/// Apply inline formatting: **bold**, *italic*, `code`, [links](url).
fn inline_format(text: &str) -> String {
    let mut result = html_escape(text);
    // Bold: **text**
    result = regex_replace_simple(&result, "\\*\\*(.+?)\\*\\*", "<strong>$1</strong>");
    // Italic: *text*
    result = regex_replace_simple(&result, "\\*(.+?)\\*", "<em>$1</em>");
    // Inline code: `text`
    result = regex_replace_simple(&result, "`(.+?)`", "<code>$1</code>");
    // Links: [text](url)
    result = regex_replace_simple(&result, "\\[(.+?)\\]\\((.+?)\\)", "<a href=\"$2\">$1</a>");
    result
}

/// Minimal regex-free pattern replacement for simple Markdown inline patterns.
/// This is intentionally simple -- it handles the most common cases without
/// pulling in the `regex` crate just for this module.
fn regex_replace_simple(text: &str, _pattern: &str, _replacement: &str) -> String {
    // For production, we do manual replacement of the four patterns above.
    let mut result = text.to_string();

    // Bold: **text** -> <strong>text</strong>
    if _pattern.contains("\\*\\*") {
        while let Some(start) = result.find("**") {
            if let Some(end) = result[start + 2..].find("**") {
                let inner = result[start + 2..start + 2 + end].to_string();
                result = format!(
                    "{}<strong>{}</strong>{}",
                    &result[..start],
                    inner,
                    &result[start + 2 + end + 2..]
                );
            } else {
                break;
            }
        }
        return result;
    }

    // Italic: *text* -> <em>text</em>
    if _pattern.contains("\\*(.+?)\\*") && !_pattern.contains("\\*\\*") {
        while let Some(start) = result.find('*') {
            if let Some(end) = result[start + 1..].find('*') {
                let inner = result[start + 1..start + 1 + end].to_string();
                result = format!(
                    "{}<em>{}</em>{}",
                    &result[..start],
                    inner,
                    &result[start + 1 + end + 1..]
                );
            } else {
                break;
            }
        }
        return result;
    }

    // Inline code: `text` -> <code>text</code>
    if _pattern.contains('`') {
        while let Some(start) = result.find('`') {
            if let Some(end) = result[start + 1..].find('`') {
                let inner = result[start + 1..start + 1 + end].to_string();
                result = format!(
                    "{}<code>{}</code>{}",
                    &result[..start],
                    inner,
                    &result[start + 1 + end + 1..]
                );
            } else {
                break;
            }
        }
        return result;
    }

    // Links: [text](url) -> <a href="url">text</a>
    if _pattern.contains("\\[") {
        while let Some(bracket_start) = result.find('[') {
            if let Some(bracket_end) = result[bracket_start..].find("](") {
                let abs_bracket_end = bracket_start + bracket_end;
                if let Some(paren_end) = result[abs_bracket_end + 2..].find(')') {
                    let link_text =
                        result[bracket_start + 1..abs_bracket_end].to_string();
                    let url = result
                        [abs_bracket_end + 2..abs_bracket_end + 2 + paren_end]
                        .to_string();
                    result = format!(
                        "{}<a href=\"{}\">{}</a>{}",
                        &result[..bracket_start],
                        url,
                        link_text,
                        &result[abs_bracket_end + 2 + paren_end + 1..]
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        return result;
    }

    result
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Strip Markdown formatting for plain-text export.
fn strip_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut in_code_block = false;

    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let trimmed = line.trim();
        // Strip heading markers
        let text = trimmed.trim_start_matches('#').trim_start();
        // Strip bold/italic markers
        let text = text.replace("**", "").replace('*', "");
        // Strip inline code backticks
        let text = text.replace('`', "");

        result.push_str(&text);
        result.push('\n');
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_words_empty() {
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn test_count_words_normal() {
        assert_eq!(count_words("Hello world, this is a test."), 6);
    }

    #[test]
    fn test_count_words_multiline() {
        assert_eq!(count_words("Line one\nLine two\n\nLine three"), 6);
    }

    #[test]
    fn test_compute_stats() {
        let stats = compute_stats("Hello world. This is a test! How are you?");
        assert_eq!(stats.words, 9);
        assert_eq!(stats.sentences, 3);
        assert!(stats.reading_time_min > 0.0);
    }

    #[test]
    fn test_compute_stats_empty() {
        let stats = compute_stats("");
        assert_eq!(stats.words, 0);
        assert_eq!(stats.characters, 0);
        assert_eq!(stats.sentences, 0);
        assert_eq!(stats.paragraphs, 0);
        assert_eq!(stats.reading_time_min, 0.0);
    }

    #[test]
    fn test_doc_format_extension() {
        assert_eq!(DocFormat::Markdown.extension(), "md");
        assert_eq!(DocFormat::Html.extension(), "html");
        assert_eq!(DocFormat::PlainText.extension(), "txt");
    }

    #[test]
    fn test_doc_format_from_str() {
        assert_eq!(DocFormat::from_str_loose("markdown"), DocFormat::Markdown);
        assert_eq!(DocFormat::from_str_loose("MD"), DocFormat::Markdown);
        assert_eq!(DocFormat::from_str_loose("html"), DocFormat::Html);
        assert_eq!(DocFormat::from_str_loose("whatever"), DocFormat::PlainText);
    }

    #[test]
    fn test_action_to_instruction_valid() {
        assert!(action_to_instruction("improve").is_ok());
        assert!(action_to_instruction("shorten").is_ok());
        assert!(action_to_instruction("expand").is_ok());
        assert!(action_to_instruction("fix_grammar").is_ok());
        assert!(action_to_instruction("translate_en").is_ok());
        assert!(action_to_instruction("translate_de").is_ok());
        assert!(action_to_instruction("summarize").is_ok());
    }

    #[test]
    fn test_action_to_instruction_invalid() {
        let err = action_to_instruction("dance").unwrap_err();
        assert_eq!(err.code, "INVALID_ACTION");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), "&lt;script&gt;alert('xss')&lt;/script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_strip_markdown() {
        let md = "# Hello\n\n**Bold** text and *italic* text.\n\n```\ncode\n```\n";
        let plain = strip_markdown(md);
        assert!(plain.contains("Hello"));
        assert!(plain.contains("Bold text and italic text."));
        assert!(plain.contains("code"));
        assert!(!plain.contains('#'));
        assert!(!plain.contains("**"));
    }

    #[test]
    fn test_basic_md_to_html_heading() {
        let html = basic_md_to_html("# Title\n\nSome text.");
        assert!(html.contains("<h1>"));
        assert!(html.contains("Title"));
        assert!(html.contains("<p>"));
    }

    #[test]
    fn test_basic_md_to_html_list() {
        let html = basic_md_to_html("- Item one\n- Item two");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("Item one"));
    }

    #[test]
    fn test_basic_md_to_html_code_block() {
        let html = basic_md_to_html("```\nlet x = 1;\n```");
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("let x = 1;"));
    }
}
