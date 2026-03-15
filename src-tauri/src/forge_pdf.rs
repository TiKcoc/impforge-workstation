// SPDX-License-Identifier: Apache-2.0
//! ForgePDF -- PDF Viewer, Editor & Converter
//!
//! Provides PDF document management: import, metadata extraction, text extraction,
//! AI-powered summarization/Q&A, and conversion to text/markdown formats.
//!
//! PDFs are stored as copies in `~/.impforge/pdfs/` with metadata sidecars.
//! Text extraction uses a lightweight byte-level parser that finds text streams
//! inside the PDF binary format. For richer extraction, Ollama can be used.
//!
//! This module is part of ImpForge Phase 3 (Office/Productivity tools).

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds imported PDF files.
const PDFS_DIR: &str = "pdfs";

/// Extension for the metadata sidecar files.
const META_EXT: &str = "pdf.meta.json";

/// Max characters for text preview stored in metadata.
const PREVIEW_MAX_CHARS: usize = 500;

/// Ollama HTTP timeout for AI requests (generous for large PDFs).
const AI_TIMEOUT_SECS: u64 = 180;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Full PDF document representation returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfDocument {
    pub id: String,
    pub title: String,
    pub path: String,
    pub file_size: u64,
    pub page_count: u32,
    pub text_preview: String,
    pub created_at: String,
    pub imported_at: String,
}

/// Lightweight listing entry (no text preview).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMeta {
    pub id: String,
    pub title: String,
    pub file_size: u64,
    pub page_count: u32,
    pub imported_at: String,
}

/// Persisted sidecar metadata (JSON next to the PDF copy).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaFile {
    id: String,
    title: String,
    original_path: String,
    file_size: u64,
    page_count: u32,
    text_preview: String,
    created_at: String,
    imported_at: String,
}

/// AI operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfAiResult {
    pub document_id: String,
    pub action: String,
    pub result: String,
    pub model: String,
}

/// Conversion result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfConvertResult {
    pub document_id: String,
    pub output_path: String,
    pub format: String,
    pub char_count: u32,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the PDFs directory, creating it if necessary.
fn pdfs_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(PDFS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create PDFs directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Build the metadata sidecar path for a given document.
fn meta_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{META_EXT}"))
}

/// Build the PDF file path for a given document.
fn pdf_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.pdf"))
}

/// Read and parse a metadata sidecar file.
fn read_meta(path: &Path) -> Result<MetaFile, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "META_READ_FAILED",
            format!("Cannot read PDF metadata: {e}"),
        )
    })?;
    serde_json::from_str::<MetaFile>(&data).map_err(|e| {
        ImpForgeError::internal(
            "META_PARSE_FAILED",
            format!("Corrupt PDF metadata: {e}"),
        )
    })
}

/// Persist a metadata sidecar file.
fn write_meta(path: &Path, meta: &MetaFile) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| {
        ImpForgeError::internal("META_SERIALIZE", format!("Cannot serialize metadata: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "META_WRITE_FAILED",
            format!("Cannot write PDF metadata: {e}"),
        )
    })
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Estimate page count from raw PDF bytes.
///
/// Scans for `/Type /Page` markers (excluding `/Type /Pages` which is the
/// page-tree root). This is a heuristic but works well for most PDFs.
fn estimate_page_count(bytes: &[u8]) -> u32 {
    let haystack = bytes;
    let needle = b"/Type /Page";
    let pages_needle = b"/Type /Pages";
    let mut count: u32 = 0;
    let mut pos = 0;

    while pos + needle.len() <= haystack.len() {
        if haystack[pos..].starts_with(needle) {
            // Make sure this is NOT "/Type /Pages"
            let is_pages = pos + pages_needle.len() <= haystack.len()
                && haystack[pos..].starts_with(pages_needle);
            if !is_pages {
                count += 1;
            }
            pos += needle.len();
        } else {
            pos += 1;
        }
    }

    // Fallback: at least 1 page if the file is a valid PDF
    if count == 0 && haystack.len() > 4 && haystack.starts_with(b"%PDF") {
        count = 1;
    }

    count
}

/// Extract text from PDF binary data using a lightweight byte-level parser.
///
/// This scans for text streams (between `BT` and `ET` markers) and extracts
/// parenthesized string operands from `Tj` and `TJ` operators. It handles
/// basic PDF text encoding (PDFDocEncoding / Latin-1 subset).
///
/// For complex PDFs (CID fonts, ToUnicode CMaps, encrypted), this may return
/// partial or empty text. In that case, the AI summarizer can describe the
/// document via its visual structure.
fn extract_text_from_pdf(bytes: &[u8]) -> String {
    let mut text = String::new();

    // Quick scan: find parenthesized strings after text operators.
    // PDF text objects are between BT...ET. Inside, Tj and TJ show strings.
    let content = String::from_utf8_lossy(bytes);

    // Strategy: find all parenthesized strings (X) in text-showing contexts.
    // We look for patterns like "(text) Tj" or "[(text)] TJ".
    let mut chars = content.char_indices().peekable();
    let mut in_paren = false;
    let mut paren_depth = 0;
    let mut current_string = String::new();

    while let Some((_i, ch)) = chars.next() {
        if in_paren {
            match ch {
                '\\' => {
                    // Escaped character — consume the next char
                    if let Some((_j, esc)) = chars.next() {
                        match esc {
                            'n' => current_string.push('\n'),
                            'r' => current_string.push('\r'),
                            't' => current_string.push('\t'),
                            '(' => current_string.push('('),
                            ')' => current_string.push(')'),
                            '\\' => current_string.push('\\'),
                            _ => {
                                // Octal or unknown escape — skip gracefully
                                current_string.push(esc);
                            }
                        }
                    }
                }
                '(' => {
                    paren_depth += 1;
                    current_string.push(ch);
                }
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                        current_string.push(ch);
                    } else {
                        // End of string
                        in_paren = false;
                        if !current_string.trim().is_empty() {
                            if !text.is_empty()
                                && !text.ends_with(' ')
                                && !text.ends_with('\n')
                            {
                                text.push(' ');
                            }
                            text.push_str(current_string.trim());
                        }
                        current_string.clear();
                    }
                }
                _ => {
                    // Filter control characters but keep printable text
                    if ch >= ' ' || ch == '\n' || ch == '\r' || ch == '\t' {
                        current_string.push(ch);
                    }
                }
            }
        } else if ch == '(' {
            in_paren = true;
            paren_depth = 0;
            current_string.clear();
        }
    }

    // Clean up: collapse excessive whitespace, remove binary garbage
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace() || c.is_alphanumeric())
        .collect();

    // Collapse runs of whitespace
    let mut result = String::with_capacity(cleaned.len());
    let mut prev_space = false;
    for ch in cleaned.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }

    result.trim().to_string()
}

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Send a prompt to Ollama and return the response text.
async fn ollama_request(
    system_prompt: &str,
    user_message: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_TIMEOUT_SECS))
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
                    "Cannot connect to Ollama for PDF AI analysis",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "PDF AI analysis timed out",
                )
                .with_suggestion("The document may be too large. Try a shorter section.")
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

/// Sanitize a filename: keep only safe characters.
fn sanitize_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();

    if safe.is_empty() {
        "untitled".to_string()
    } else {
        safe
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all imported PDFs (metadata only).
#[tauri::command]
pub async fn pdf_list() -> AppResult<Vec<PdfMeta>> {
    let dir = pdfs_dir()?;
    let mut docs: Vec<PdfMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read PDFs dir: {e}"))
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
        if !name.ends_with(META_EXT) {
            continue;
        }
        if let Ok(meta) = read_meta(&path) {
            docs.push(PdfMeta {
                id: meta.id,
                title: meta.title,
                file_size: meta.file_size,
                page_count: meta.page_count,
                imported_at: meta.imported_at,
            });
        }
    }

    // Most recently imported first.
    docs.sort_by(|a, b| b.imported_at.cmp(&a.imported_at));

    Ok(docs)
}

/// Import a PDF file: copy it to ~/.impforge/pdfs/, extract metadata.
#[tauri::command]
pub async fn pdf_import(file_path: String) -> AppResult<PdfDocument> {
    let source = Path::new(&file_path);

    if !source.exists() {
        return Err(
            ImpForgeError::filesystem(
                "FILE_NOT_FOUND",
                format!("PDF file not found: {file_path}"),
            )
            .with_suggestion("Check the file path and try again."),
        );
    }

    // Validate it looks like a PDF
    let header = std::fs::read(source)
        .map_err(|e| {
            ImpForgeError::filesystem(
                "FILE_READ_FAILED",
                format!("Cannot read PDF file: {e}"),
            )
        })?;

    if header.len() < 5 || !header.starts_with(b"%PDF") {
        return Err(
            ImpForgeError::validation(
                "NOT_A_PDF",
                "The selected file does not appear to be a valid PDF",
            )
            .with_suggestion("Only PDF files can be imported. Check the file format."),
        );
    }

    let dir = pdfs_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    // Derive title from filename (without extension)
    let title = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled PDF")
        .to_string();

    let file_size = header.len() as u64;
    let page_count = estimate_page_count(&header);

    // Extract text preview
    let full_text = extract_text_from_pdf(&header);
    let text_preview = if full_text.len() > PREVIEW_MAX_CHARS {
        format!("{}...", &full_text[..PREVIEW_MAX_CHARS])
    } else {
        full_text
    };

    // Copy PDF to storage
    let dest = pdf_path(&dir, &id);
    std::fs::copy(source, &dest).map_err(|e| {
        ImpForgeError::filesystem(
            "COPY_FAILED",
            format!("Cannot copy PDF to storage: {e}"),
        )
    })?;

    // Write metadata sidecar
    let meta = MetaFile {
        id: id.clone(),
        title: title.clone(),
        original_path: file_path.clone(),
        file_size,
        page_count,
        text_preview: text_preview.clone(),
        created_at: now.clone(),
        imported_at: now.clone(),
    };
    write_meta(&meta_path(&dir, &id), &meta)?;

    log::info!("ForgePDF: imported '{}' ({} bytes, ~{} pages)", title, file_size, page_count);

    Ok(PdfDocument {
        id,
        title,
        path: dest.to_string_lossy().to_string(),
        file_size,
        page_count,
        text_preview,
        created_at: now.clone(),
        imported_at: now,
    })
}

/// Get detailed info for a specific PDF document.
#[tauri::command]
pub async fn pdf_get_info(id: String) -> AppResult<PdfDocument> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found"))
                .with_suggestion("The document may have been deleted."),
        );
    }

    let meta = read_meta(&mp)?;
    let pp = pdf_path(&dir, &id);

    Ok(PdfDocument {
        id: meta.id,
        title: meta.title,
        path: pp.to_string_lossy().to_string(),
        file_size: meta.file_size,
        page_count: meta.page_count,
        text_preview: meta.text_preview,
        created_at: meta.created_at,
        imported_at: meta.imported_at,
    })
}

/// Extract full text content from a PDF.
#[tauri::command]
pub async fn pdf_get_text(id: String) -> AppResult<String> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let pp = pdf_path(&dir, &id);
    let bytes = std::fs::read(&pp).map_err(|e| {
        ImpForgeError::filesystem(
            "PDF_READ_FAILED",
            format!("Cannot read PDF file: {e}"),
        )
    })?;

    let text = extract_text_from_pdf(&bytes);

    if text.is_empty() {
        Ok("(No extractable text found. This PDF may contain only images or use complex fonts. Try using AI Summarize for analysis.)".to_string())
    } else {
        Ok(text)
    }
}

/// Delete a PDF document and its metadata sidecar.
#[tauri::command]
pub async fn pdf_delete(id: String) -> AppResult<()> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let pp = pdf_path(&dir, &id);

    // Remove both files (ignore error on PDF -- it may already be gone).
    let _ = std::fs::remove_file(&pp);
    std::fs::remove_file(&mp).map_err(|e| {
        ImpForgeError::filesystem(
            "DELETE_FAILED",
            format!("Cannot delete PDF metadata: {e}"),
        )
    })?;

    log::info!("ForgePDF: deleted document '{}'", id);

    Ok(())
}

/// AI-powered summarization of a PDF via Ollama.
#[tauri::command]
pub async fn pdf_ai_summarize(id: String, model: Option<String>) -> AppResult<PdfAiResult> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let pp = pdf_path(&dir, &id);
    let bytes = std::fs::read(&pp).map_err(|e| {
        ImpForgeError::filesystem("PDF_READ_FAILED", format!("Cannot read PDF: {e}"))
    })?;

    let text = extract_text_from_pdf(&bytes);
    let model_name = model.as_deref().unwrap_or("dolphin3:8b");

    let content_for_ai = if text.is_empty() {
        format!(
            "This is a PDF document titled '{}'. It has approximately {} pages and is {} bytes. \
             No text could be extracted (it may be image-based). \
             Please describe what this document likely contains based on the title.",
            meta.title, meta.page_count, meta.file_size
        )
    } else {
        // Truncate to a reasonable size for the AI context window
        let max_chars = 12_000;
        if text.len() > max_chars {
            format!("{}\n\n[... truncated, {} more characters ...]", &text[..max_chars], text.len() - max_chars)
        } else {
            text
        }
    };

    let system_prompt = "You are a document analysis assistant inside ImpForge, an AI Workstation. \
        Provide a clear, structured summary of the given PDF content. \
        Include: main topic, key points (bulleted), and a brief conclusion. \
        If the document appears technical, note the domain. Keep the summary concise (200-400 words).";

    let user_msg = format!(
        "Summarize this PDF document titled '{}':\n\n{}",
        meta.title, content_for_ai
    );

    log::info!("ForgePDF: AI summarize '{}' with model '{}'", meta.title, model_name);

    let result = ollama_request(system_prompt, &user_msg, Some(model_name)).await?;

    Ok(PdfAiResult {
        document_id: id,
        action: "summarize".to_string(),
        result,
        model: model_name.to_string(),
    })
}

/// AI-powered Q&A about a PDF document via Ollama.
#[tauri::command]
pub async fn pdf_ai_ask(
    id: String,
    question: String,
    model: Option<String>,
) -> AppResult<PdfAiResult> {
    if question.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_QUESTION",
            "Please provide a question about the document",
        ));
    }

    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let pp = pdf_path(&dir, &id);
    let bytes = std::fs::read(&pp).map_err(|e| {
        ImpForgeError::filesystem("PDF_READ_FAILED", format!("Cannot read PDF: {e}"))
    })?;

    let text = extract_text_from_pdf(&bytes);
    let model_name = model.as_deref().unwrap_or("dolphin3:8b");

    let context = if text.is_empty() {
        format!(
            "Document: '{}' ({} pages, {} bytes). No extractable text available.",
            meta.title, meta.page_count, meta.file_size
        )
    } else {
        let max_chars = 12_000;
        if text.len() > max_chars {
            format!("{}\n\n[... content truncated ...]", &text[..max_chars])
        } else {
            text
        }
    };

    let system_prompt = "You are a document Q&A assistant inside ImpForge, an AI Workstation. \
        Answer questions about the provided PDF content accurately and concisely. \
        If the answer is not in the document, say so clearly. \
        Cite relevant sections when possible.";

    let user_msg = format!(
        "Document: '{}'\n\nContent:\n{}\n\nQuestion: {}",
        meta.title, context, question
    );

    log::info!("ForgePDF: AI ask '{}' about '{}'", question.chars().take(50).collect::<String>(), meta.title);

    let result = ollama_request(system_prompt, &user_msg, Some(model_name)).await?;

    Ok(PdfAiResult {
        document_id: id,
        action: "ask".to_string(),
        result,
        model: model_name.to_string(),
    })
}

/// Convert PDF text content to a .txt file. Returns the output file path.
#[tauri::command]
pub async fn pdf_convert_to_text(id: String) -> AppResult<PdfConvertResult> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let pp = pdf_path(&dir, &id);
    let bytes = std::fs::read(&pp).map_err(|e| {
        ImpForgeError::filesystem("PDF_READ_FAILED", format!("Cannot read PDF: {e}"))
    })?;

    let text = extract_text_from_pdf(&bytes);

    if text.is_empty() {
        return Err(
            ImpForgeError::validation(
                "NO_TEXT_CONTENT",
                "No extractable text found in this PDF",
            )
            .with_suggestion("This PDF may contain only images. Try using AI Summarize instead."),
        );
    }

    // Write to ~/Documents (or fallback to pdfs dir)
    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "EXPORT_DIR_FAILED",
                format!("Cannot create export directory: {e}"),
            )
        })?;
    }

    let safe_title = sanitize_filename(&meta.title);
    let output_path = export_dir.join(format!("{safe_title}.txt"));

    std::fs::write(&output_path, &text).map_err(|e| {
        ImpForgeError::filesystem(
            "EXPORT_WRITE_FAILED",
            format!("Cannot write text file: {e}"),
        )
    })?;

    log::info!("ForgePDF: converted '{}' to text at {}", meta.title, output_path.display());

    Ok(PdfConvertResult {
        document_id: id,
        output_path: output_path.to_string_lossy().to_string(),
        format: "txt".to_string(),
        char_count: text.len() as u32,
    })
}

/// Convert PDF text content to a .md (Markdown) file. Returns the output file path.
#[tauri::command]
pub async fn pdf_convert_to_markdown(id: String) -> AppResult<PdfConvertResult> {
    let dir = pdfs_dir()?;
    let mp = meta_path(&dir, &id);

    if !mp.exists() {
        return Err(
            ImpForgeError::filesystem("PDF_NOT_FOUND", format!("PDF document '{id}' not found")),
        );
    }

    let meta = read_meta(&mp)?;
    let pp = pdf_path(&dir, &id);
    let bytes = std::fs::read(&pp).map_err(|e| {
        ImpForgeError::filesystem("PDF_READ_FAILED", format!("Cannot read PDF: {e}"))
    })?;

    let text = extract_text_from_pdf(&bytes);

    if text.is_empty() {
        return Err(
            ImpForgeError::validation(
                "NO_TEXT_CONTENT",
                "No extractable text found in this PDF",
            )
            .with_suggestion("This PDF may contain only images. Try using AI Summarize instead."),
        );
    }

    // Build a simple Markdown document from the extracted text
    let mut markdown = format!("# {}\n\n", meta.title);
    markdown.push_str(&format!(
        "> Converted from PDF | {} pages | {} bytes | {}\n\n",
        meta.page_count, meta.file_size, meta.imported_at
    ));
    markdown.push_str("---\n\n");

    // Split text into paragraphs (double-newline or long whitespace gaps)
    for paragraph in text.split("  ").filter(|p| !p.trim().is_empty()) {
        let trimmed = paragraph.trim();
        if !trimmed.is_empty() {
            markdown.push_str(trimmed);
            markdown.push_str("\n\n");
        }
    }

    // Write to ~/Documents (or fallback to pdfs dir)
    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "EXPORT_DIR_FAILED",
                format!("Cannot create export directory: {e}"),
            )
        })?;
    }

    let safe_title = sanitize_filename(&meta.title);
    let output_path = export_dir.join(format!("{safe_title}.md"));

    std::fs::write(&output_path, &markdown).map_err(|e| {
        ImpForgeError::filesystem(
            "EXPORT_WRITE_FAILED",
            format!("Cannot write markdown file: {e}"),
        )
    })?;

    log::info!("ForgePDF: converted '{}' to markdown at {}", meta.title, output_path.display());

    Ok(PdfConvertResult {
        document_id: id,
        output_path: output_path.to_string_lossy().to_string(),
        format: "md".to_string(),
        char_count: markdown.len() as u32,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_page_count_empty() {
        assert_eq!(estimate_page_count(b""), 0);
    }

    #[test]
    fn test_estimate_page_count_valid_pdf_header() {
        // Minimal PDF-like content with no page markers should still yield 1
        assert_eq!(estimate_page_count(b"%PDF-1.4 some content"), 1);
    }

    #[test]
    fn test_estimate_page_count_with_pages() {
        let content = b"%PDF-1.4 /Type /Page /Type /Page /Type /Pages";
        // Two /Type /Page and one /Type /Pages (which should not be counted)
        assert_eq!(estimate_page_count(content), 2);
    }

    #[test]
    fn test_estimate_page_count_only_pages_tree() {
        let content = b"%PDF-1.4 /Type /Pages";
        // /Type /Pages is the tree node, not a page — fallback to 1
        assert_eq!(estimate_page_count(content), 1);
    }

    #[test]
    fn test_extract_text_basic() {
        let pdf_like = b"(Hello World) Tj";
        let text = extract_text_from_pdf(pdf_like);
        assert!(text.contains("Hello World"));
    }

    #[test]
    fn test_extract_text_escaped_parens() {
        let pdf_like = b"(Hello \\(World\\)) Tj";
        let text = extract_text_from_pdf(pdf_like);
        assert!(text.contains("Hello (World)"));
    }

    #[test]
    fn test_extract_text_empty() {
        let text = extract_text_from_pdf(b"%PDF-1.4 no text objects here");
        assert!(text.is_empty());
    }

    #[test]
    fn test_extract_text_multiple_strings() {
        let pdf_like = b"BT (First) Tj (Second) Tj ET";
        let text = extract_text_from_pdf(pdf_like);
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("My Document"), "My Document");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("file<>|name"), "file___name");
    }

    #[test]
    fn test_sanitize_filename_empty() {
        assert_eq!(sanitize_filename(""), "untitled");
    }

    #[test]
    fn test_now_iso_format() {
        let ts = now_iso();
        // Should start with a year
        assert!(ts.starts_with("20"));
        // Should contain 'T' separator
        assert!(ts.contains('T'));
    }

    #[test]
    fn test_meta_serialization_roundtrip() {
        let meta = MetaFile {
            id: "test-123".to_string(),
            title: "Test PDF".to_string(),
            original_path: "/tmp/test.pdf".to_string(),
            file_size: 1024,
            page_count: 5,
            text_preview: "Hello world".to_string(),
            created_at: now_iso(),
            imported_at: now_iso(),
        };

        let json = serde_json::to_string(&meta).expect("serialize");
        let parsed: MetaFile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.id, "test-123");
        assert_eq!(parsed.title, "Test PDF");
        assert_eq!(parsed.page_count, 5);
    }
}
