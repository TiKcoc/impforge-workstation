// SPDX-License-Identifier: Apache-2.0
//! ForgeCanvas -- 3-Panel AI Document Workspace
//!
//! A unified workspace where users drag source files in (left panel),
//! see AI-generated output documents (center panel), and inspect
//! source links, formulas, and calculation breakdowns (right panel).
//! Context-aware chat at the bottom uses selected source chunks.
//!
//! Projects are persisted as JSON files in `~/.impforge/canvas/`.
//! AI generation uses the local Ollama inference backend.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds canvas project files.
const CANVAS_DIR: &str = "canvas";

/// Ollama HTTP timeout for AI generation requests (generous for large context).
const AI_GENERATE_TIMEOUT_SECS: u64 = 180;

/// Maximum number of source chunks that can be sent to AI in one request.
const MAX_CHUNKS_PER_REQUEST: usize = 50;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The type of document being generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    BusinessReport,
    QuarterlyReport,
    RestaurantMenu,
    ActionCard,
    BusinessPlan,
    MasterPlan,
    Summary,
    Presentation,
    Letter,
    Invoice,
    Custom,
}

impl OutputType {
    /// Parse from a user-facing string (case-insensitive, lenient).
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "business_report" => Self::BusinessReport,
            "quarterly_report" => Self::QuarterlyReport,
            "restaurant_menu" => Self::RestaurantMenu,
            "action_card" | "promo_card" => Self::ActionCard,
            "business_plan" => Self::BusinessPlan,
            "master_plan" => Self::MasterPlan,
            "summary" => Self::Summary,
            "presentation" => Self::Presentation,
            "letter" | "cover_letter" => Self::Letter,
            "invoice" => Self::Invoice,
            _ => Self::Custom,
        }
    }
}

/// A message in the canvas chat history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub referenced_chunks: Vec<String>,
    pub timestamp: String,
}

/// A source file that has been imported into the canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    pub id: String,
    pub file_path: Option<String>,
    pub file_name: String,
    pub content: String,
    pub chunks: Vec<SourceChunk>,
    pub file_type: String,
}

/// A digestible piece of a source file, with line references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChunk {
    pub id: String,
    pub text: String,
    pub line_start: u32,
    pub line_end: u32,
    pub source_id: String,
    pub used_in_output: bool,
    pub relevance_score: f32,
}

/// Bidirectional link between an output section and source chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSourceLink {
    pub output_section_idx: u32,
    pub chunk_ids: Vec<String>,
    pub confidence: f32,
}

/// A full canvas project (serialized to disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasProject {
    pub id: String,
    pub name: String,
    pub sources: Vec<SourceEntry>,
    pub output_content: String,
    pub output_type: OutputType,
    pub template: Option<String>,
    pub background: Option<String>,
    pub source_links: Vec<OutputSourceLink>,
    pub chat_history: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight metadata for project listings (no content/sources).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasProjectMeta {
    pub id: String,
    pub name: String,
    pub output_type: OutputType,
    pub source_count: u32,
    pub template: Option<String>,
    pub updated_at: String,
}

/// Template definition for the template gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub sections: Vec<String>,
    pub has_background: bool,
    pub description: String,
}

/// Response from the AI chat command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    pub referenced_chunks: Vec<String>,
    pub updated_output: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the canvas directory, creating it if necessary.
fn canvas_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(CANVAS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create canvas directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Build the project file path.
fn project_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Read and parse a project file.
fn read_project(path: &Path) -> Result<CanvasProject, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "PROJECT_READ_FAILED",
            format!("Cannot read canvas project: {e}"),
        )
    })?;
    serde_json::from_str::<CanvasProject>(&data).map_err(|e| {
        ImpForgeError::internal(
            "PROJECT_PARSE_FAILED",
            format!("Corrupt canvas project file: {e}"),
        )
    })
}

/// Persist a project file atomically.
fn write_project(path: &Path, project: &CanvasProject) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(project).map_err(|e| {
        ImpForgeError::internal("PROJECT_SERIALIZE", format!("Cannot serialize project: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "PROJECT_WRITE_FAILED",
            format!("Cannot write canvas project: {e}"),
        )
    })
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

/// Detect file type from extension.
fn detect_file_type(file_name: &str) -> String {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => "markdown".to_string(),
        "txt" | "text" => "text".to_string(),
        "csv" => "csv".to_string(),
        "json" => "json".to_string(),
        "html" | "htm" => "html".to_string(),
        "xml" => "xml".to_string(),
        "pdf" => "pdf".to_string(),
        "docx" => "docx".to_string(),
        "xlsx" | "xls" => "spreadsheet".to_string(),
        "eml" => "email".to_string(),
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" => "code".to_string(),
        "yaml" | "yml" | "toml" => "config".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Split text content into chunks with line references.
/// Uses paragraph-based splitting for natural boundaries.
fn chunk_content(source_id: &str, content: &str) -> Vec<SourceChunk> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut chunk_start_line: u32 = 1;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as u32;

    for (idx, line) in lines.iter().enumerate() {
        let current_line = idx as u32 + 1;

        // Start a new chunk on empty lines (paragraph boundary)
        // but only if we have accumulated meaningful content
        if line.trim().is_empty() && current_chunk.trim().len() > 20 {
            let chunk_text = current_chunk.trim().to_string();
            if !chunk_text.is_empty() {
                chunks.push(SourceChunk {
                    id: Uuid::new_v4().to_string(),
                    text: chunk_text,
                    line_start: chunk_start_line,
                    line_end: current_line.saturating_sub(1).max(chunk_start_line),
                    source_id: source_id.to_string(),
                    used_in_output: false,
                    relevance_score: 0.0,
                });
            }
            current_chunk.clear();
            chunk_start_line = current_line + 1;
            continue;
        }

        if !current_chunk.is_empty() {
            current_chunk.push('\n');
        }
        current_chunk.push_str(line);

        // Also split at ~500 chars to keep chunks digestible
        if current_chunk.len() > 500 {
            let chunk_text = current_chunk.trim().to_string();
            if !chunk_text.is_empty() {
                chunks.push(SourceChunk {
                    id: Uuid::new_v4().to_string(),
                    text: chunk_text,
                    line_start: chunk_start_line,
                    line_end: current_line,
                    source_id: source_id.to_string(),
                    used_in_output: false,
                    relevance_score: 0.0,
                });
            }
            current_chunk.clear();
            chunk_start_line = current_line + 1;
        }
    }

    // Flush remaining content
    let chunk_text = current_chunk.trim().to_string();
    if !chunk_text.is_empty() {
        chunks.push(SourceChunk {
            id: Uuid::new_v4().to_string(),
            text: chunk_text,
            line_start: chunk_start_line,
            line_end: total_lines.max(chunk_start_line),
            source_id: source_id.to_string(),
            used_in_output: false,
            relevance_score: 0.0,
        });
    }

    // If content was too short to split, ensure at least one chunk
    if chunks.is_empty() && !content.trim().is_empty() {
        chunks.push(SourceChunk {
            id: Uuid::new_v4().to_string(),
            text: content.trim().to_string(),
            line_start: 1,
            line_end: total_lines.max(1),
            source_id: source_id.to_string(),
            used_in_output: false,
            relevance_score: 0.0,
        });
    }

    chunks
}

/// Extract text content from a file path (reads UTF-8 text files).
/// Binary formats (PDF, DOCX, XLSX) return a placeholder — future phases
/// will add proper extraction via dedicated crates.
fn extract_text(file_path: &Path) -> Result<String, ImpForgeError> {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let file_type = detect_file_type(file_name);

    match file_type.as_str() {
        "pdf" | "docx" | "spreadsheet" => {
            // For binary formats, return a note that extraction is limited
            // In production, lopdf / calamine / docx-rs would handle these
            Ok(format!(
                "[Binary file: {file_name}]\n\
                 File type: {file_type}\n\
                 Note: Full extraction for {file_type} files will be available in a future update.\n\
                 For now, please convert to .txt, .csv, or .md for best results."
            ))
        }
        _ => {
            // Try to read as UTF-8 text
            std::fs::read_to_string(file_path).map_err(|e| {
                ImpForgeError::filesystem(
                    "FILE_READ_FAILED",
                    format!("Cannot read file '{}': {e}", file_path.display()),
                )
                .with_suggestion("Ensure the file exists and is readable text (UTF-8).")
            })
        }
    }
}

// ---------------------------------------------------------------------------
// AI Generation (Ollama)
// ---------------------------------------------------------------------------

/// Build the system prompt for document generation based on output type.
fn build_system_prompt(output_type: &OutputType, template: &Option<String>) -> String {
    let base = "You are a professional document generation AI inside ForgeCanvas, \
        part of the ImpForge AI Workstation. Your task is to generate well-structured, \
        professional documents from the provided source data.\n\n\
        RULES:\n\
        - Generate clean Markdown output\n\
        - Use headings, bullet points, and tables where appropriate\n\
        - Reference source data accurately — never invent numbers or facts\n\
        - Maintain professional tone and formatting\n\
        - For financial data, show calculations clearly\n\
        - Wrap each section in a comment like <!-- section:N chunk:ID1,ID2 --> \
          to enable source linking\n";

    let type_instruction = match output_type {
        OutputType::BusinessReport => "Generate a professional Business Report with Executive Summary, Key Findings, Financial Overview, Analysis, and Recommendations.",
        OutputType::QuarterlyReport => "Generate a Quarterly Report with Overview, Revenue Analysis, Expense Breakdown, Key Metrics, and Outlook.",
        OutputType::RestaurantMenu => "Generate an elegant Restaurant Menu with categorized items, descriptions, and prices. Use appetizing language.",
        OutputType::ActionCard => "Generate a compelling Action/Promo Card with a strong headline, clear offer, supporting details, and call-to-action.",
        OutputType::BusinessPlan => "Generate a Business Plan with Vision, Market Analysis, Product/Service Description, Financial Projections, and Team.",
        OutputType::MasterPlan => "Generate a comprehensive Master Plan with Objectives, Timeline, Resources, Milestones, and Risk Assessment.",
        OutputType::Summary => "Generate a concise executive summary capturing all key points from the source material.",
        OutputType::Presentation => "Generate a Presentation Outline with Title Slide, Problem Statement, Solution, Key Points, Demo Notes, and Call to Action.",
        OutputType::Letter => "Generate a professional letter with proper header, salutation, body paragraphs, and closing.",
        OutputType::Invoice => "Generate a professional Invoice with From/To details, itemized list, subtotals, tax, and total amount.",
        OutputType::Custom => "Generate a well-structured document based on the user's instructions.",
    };

    let mut prompt = format!("{base}\nDOCUMENT TYPE: {type_instruction}\n");

    if let Some(tmpl) = template {
        prompt.push_str(&format!("\nTEMPLATE: Use the '{tmpl}' template structure.\n"));
    }

    prompt
}

/// Send a generation request to Ollama with source chunks as context.
async fn ollama_generate(
    instruction: &str,
    chunks: &[&SourceChunk],
    output_type: &OutputType,
    template: &Option<String>,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let system_prompt = build_system_prompt(output_type, template);

    // Build context from selected chunks
    let mut context = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        context.push_str(&format!(
            "--- SOURCE CHUNK {} (ID: {}, Lines {}-{}) ---\n{}\n\n",
            i + 1,
            chunk.id,
            chunk.line_start,
            chunk.line_end,
            chunk.text
        ));
    }

    let user_message = format!(
        "INSTRUCTION: {instruction}\n\n\
         SOURCE DATA:\n{context}\n\
         Generate the document now. Use <!-- section:N chunk:ID --> comments to link output sections to source chunks."
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_GENERATE_TIMEOUT_SECS))
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
                    "Cannot connect to Ollama for document generation",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "Document generation timed out — the source data may be too large",
                )
                .with_suggestion("Try selecting fewer source chunks or a smaller model.")
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
            "Ollama returned an empty response for document generation",
        ));
    }

    Ok(content)
}

/// Parse source links from AI-generated output (<!-- section:N chunk:ID1,ID2 -->).
fn parse_source_links(output: &str) -> Vec<OutputSourceLink> {
    let mut links = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("<!-- section:") {
            continue;
        }
        // Parse: <!-- section:N chunk:ID1,ID2 -->
        let inner = trimmed
            .trim_start_matches("<!--")
            .trim_end_matches("-->")
            .trim();

        let mut section_idx: Option<u32> = None;
        let mut chunk_ids: Vec<String> = Vec::new();

        for part in inner.split_whitespace() {
            if let Some(s) = part.strip_prefix("section:") {
                section_idx = s.parse().ok();
            } else if let Some(c) = part.strip_prefix("chunk:") {
                chunk_ids = c.split(',').map(|id| id.trim().to_string()).collect();
            }
        }

        if let Some(idx) = section_idx {
            links.push(OutputSourceLink {
                output_section_idx: idx,
                chunk_ids,
                confidence: 0.85,
            });
        }
    }
    links
}

// ---------------------------------------------------------------------------
// Built-in Templates
// ---------------------------------------------------------------------------

/// Return the list of built-in templates.
fn builtin_templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo {
            id: "business-report".to_string(),
            name: "Business Report".to_string(),
            sections: vec![
                "Executive Summary".into(),
                "Financials".into(),
                "Analysis".into(),
                "Recommendations".into(),
            ],
            has_background: false,
            description: "Professional business report with executive summary and financial analysis.".into(),
        },
        TemplateInfo {
            id: "quarterly-report".to_string(),
            name: "Quarterly Report".to_string(),
            sections: vec![
                "Overview".into(),
                "Revenue".into(),
                "Expenses".into(),
                "Outlook".into(),
            ],
            has_background: false,
            description: "Quarterly financial report with revenue breakdown and forecast.".into(),
        },
        TemplateInfo {
            id: "restaurant-menu".to_string(),
            name: "Restaurant Menu".to_string(),
            sections: vec![
                "Appetizers".into(),
                "Main Courses".into(),
                "Desserts".into(),
                "Drinks".into(),
            ],
            has_background: true,
            description: "Elegant restaurant menu with categorized items and descriptions.".into(),
        },
        TemplateInfo {
            id: "action-card".to_string(),
            name: "Action / Promo Card".to_string(),
            sections: vec![
                "Headline".into(),
                "Offer".into(),
                "Details".into(),
                "CTA".into(),
            ],
            has_background: true,
            description: "Promotional action card with compelling headline and call-to-action.".into(),
        },
        TemplateInfo {
            id: "business-plan".to_string(),
            name: "Business Plan".to_string(),
            sections: vec![
                "Vision".into(),
                "Market".into(),
                "Product".into(),
                "Financials".into(),
                "Team".into(),
            ],
            has_background: false,
            description: "Comprehensive business plan for investors and stakeholders.".into(),
        },
        TemplateInfo {
            id: "cover-letter".to_string(),
            name: "Cover Letter".to_string(),
            sections: vec![
                "Header".into(),
                "Opening".into(),
                "Body".into(),
                "Closing".into(),
            ],
            has_background: false,
            description: "Professional cover letter with proper structure and tone.".into(),
        },
        TemplateInfo {
            id: "invoice".to_string(),
            name: "Invoice".to_string(),
            sections: vec![
                "From".into(),
                "To".into(),
                "Items".into(),
                "Total".into(),
                "Payment".into(),
            ],
            has_background: false,
            description: "Professional invoice with itemized charges and payment details.".into(),
        },
        TemplateInfo {
            id: "presentation".to_string(),
            name: "Presentation Outline".to_string(),
            sections: vec![
                "Title".into(),
                "Problem".into(),
                "Solution".into(),
                "Demo".into(),
                "Call to Action".into(),
            ],
            has_background: false,
            description: "Presentation outline following the problem-solution-demo pattern.".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Create a new canvas project.
#[tauri::command]
pub async fn canvas_create(name: String, output_type: String) -> AppResult<CanvasProject> {
    let dir = canvas_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let otype = OutputType::from_str_loose(&output_type);

    let project = CanvasProject {
        id: id.clone(),
        name: name.clone(),
        sources: Vec::new(),
        output_content: String::new(),
        output_type: otype,
        template: None,
        background: None,
        source_links: Vec::new(),
        chat_history: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };

    write_project(&project_path(&dir, &id), &project)?;
    log::info!("ForgeCanvas: created project '{}' ({})", name, id);
    Ok(project)
}

/// List all canvas projects (metadata only).
#[tauri::command]
pub async fn canvas_list() -> AppResult<Vec<CanvasProjectMeta>> {
    let dir = canvas_dir()?;
    let mut projects: Vec<CanvasProjectMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read canvas dir: {e}"))
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
        if !name.ends_with(".json") {
            continue;
        }
        if let Ok(project) = read_project(&path) {
            projects.push(CanvasProjectMeta {
                id: project.id,
                name: project.name,
                output_type: project.output_type,
                source_count: project.sources.len() as u32,
                template: project.template,
                updated_at: project.updated_at,
            });
        }
    }

    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(projects)
}

/// Open a canvas project by ID (full data).
#[tauri::command]
pub async fn canvas_open(id: String) -> AppResult<CanvasProject> {
    let dir = canvas_dir()?;
    let path = project_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PROJECT_NOT_FOUND", format!("Canvas project '{id}' not found"))
                .with_suggestion("The project may have been deleted."),
        );
    }

    read_project(&path)
}

/// Save a canvas project (full update).
#[tauri::command]
pub async fn canvas_save(project: CanvasProject) -> AppResult<()> {
    let dir = canvas_dir()?;
    let path = project_path(&dir, &project.id);

    let mut updated = project;
    updated.updated_at = now_iso();

    write_project(&path, &updated)?;
    Ok(())
}

/// Delete a canvas project.
#[tauri::command]
pub async fn canvas_delete(id: String) -> AppResult<()> {
    let dir = canvas_dir()?;
    let path = project_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PROJECT_NOT_FOUND", format!("Canvas project '{id}' not found")),
        );
    }

    std::fs::remove_file(&path).map_err(|e| {
        ImpForgeError::filesystem(
            "DELETE_FAILED",
            format!("Cannot delete canvas project: {e}"),
        )
    })?;

    log::info!("ForgeCanvas: deleted project '{}'", id);
    Ok(())
}

/// Add a source file to a canvas project.
/// Reads the file, extracts text, and chunks it into digestible pieces.
#[tauri::command]
pub async fn canvas_add_source(project_id: String, file_path: String) -> AppResult<SourceEntry> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    let mut project = read_project(&pp)?;
    let path = PathBuf::from(&file_path);

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let content = extract_text(&path)?;
    let source_id = Uuid::new_v4().to_string();
    let file_type = detect_file_type(&file_name);
    let chunks = chunk_content(&source_id, &content);

    let source = SourceEntry {
        id: source_id,
        file_path: Some(file_path),
        file_name: file_name.clone(),
        content,
        chunks,
        file_type,
    };

    project.sources.push(source.clone());
    project.updated_at = now_iso();
    write_project(&pp, &project)?;

    log::info!(
        "ForgeCanvas: added source '{}' ({} chunks) to project '{}'",
        file_name,
        source.chunks.len(),
        project_id
    );

    Ok(source)
}

/// Remove a source from a canvas project.
#[tauri::command]
pub async fn canvas_remove_source(project_id: String, source_id: String) -> AppResult<()> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    let mut project = read_project(&pp)?;
    let before_len = project.sources.len();
    project.sources.retain(|s| s.id != source_id);

    if project.sources.len() == before_len {
        return Err(ImpForgeError::validation(
            "SOURCE_NOT_FOUND",
            format!("Source '{source_id}' not found in project"),
        ));
    }

    // Also remove related source links
    project.source_links.retain(|link| {
        !link.chunk_ids.iter().any(|cid| {
            // Check if this chunk belonged to the removed source
            cid.is_empty()
        })
    });

    project.updated_at = now_iso();
    write_project(&pp, &project)?;

    log::info!(
        "ForgeCanvas: removed source '{}' from project '{}'",
        source_id,
        project_id
    );

    Ok(())
}

/// Generate document output from selected source chunks using AI.
#[tauri::command]
pub async fn canvas_generate(
    project_id: String,
    instruction: String,
    selected_chunks: Vec<String>,
) -> AppResult<String> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    let mut project = read_project(&pp)?;

    // Collect the selected chunks (or all chunks if none selected)
    let all_chunks: Vec<&SourceChunk> = project
        .sources
        .iter()
        .flat_map(|s| s.chunks.iter())
        .collect();

    let chunks_to_use: Vec<&SourceChunk> = if selected_chunks.is_empty() {
        // Use all chunks (up to MAX)
        all_chunks.into_iter().take(MAX_CHUNKS_PER_REQUEST).collect()
    } else {
        all_chunks
            .into_iter()
            .filter(|c| selected_chunks.contains(&c.id))
            .take(MAX_CHUNKS_PER_REQUEST)
            .collect()
    };

    if chunks_to_use.is_empty() {
        return Err(
            ImpForgeError::validation(
                "NO_CHUNKS",
                "No source chunks available for generation",
            )
            .with_suggestion("Add source files to the project first, then select chunks to use."),
        );
    }

    let output = ollama_generate(
        &instruction,
        &chunks_to_use,
        &project.output_type,
        &project.template,
        None,
    )
    .await?;

    let num_chunks_used = chunks_to_use.len();

    // Parse source links from the generated output
    let links = parse_source_links(&output);

    // Mark used chunks
    let used_chunk_ids: Vec<String> = links
        .iter()
        .flat_map(|l| l.chunk_ids.clone())
        .collect();

    for source in &mut project.sources {
        for chunk in &mut source.chunks {
            chunk.used_in_output = used_chunk_ids.contains(&chunk.id);
        }
    }

    project.output_content = output.clone();
    project.source_links = links;
    project.updated_at = now_iso();
    write_project(&pp, &project)?;

    log::info!(
        "ForgeCanvas: generated document for project '{}' ({} chunks used)",
        project_id,
        num_chunks_used
    );

    Ok(output)
}

/// Context-aware chat — selected chunks are included as context.
#[tauri::command]
pub async fn canvas_chat(
    project_id: String,
    message: String,
    selected_chunks: Vec<String>,
) -> AppResult<ChatResponse> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    let mut project = read_project(&pp)?;

    // Collect chunks for context
    let all_chunks: Vec<&SourceChunk> = project
        .sources
        .iter()
        .flat_map(|s| s.chunks.iter())
        .collect();

    let context_chunks: Vec<&SourceChunk> = if selected_chunks.is_empty() {
        Vec::new()
    } else {
        all_chunks
            .into_iter()
            .filter(|c| selected_chunks.contains(&c.id))
            .collect()
    };

    // Build context for the chat
    let mut context = String::new();
    if !context_chunks.is_empty() {
        context.push_str("SELECTED SOURCE CONTEXT:\n");
        for chunk in &context_chunks {
            context.push_str(&format!(
                "--- Chunk {} (Lines {}-{}) ---\n{}\n\n",
                chunk.id, chunk.line_start, chunk.line_end, chunk.text
            ));
        }
    }

    if !project.output_content.is_empty() {
        context.push_str("CURRENT DOCUMENT:\n");
        context.push_str(&project.output_content);
        context.push_str("\n\n");
    }

    let url = resolve_ollama_url();
    let system_prompt = "You are a document assistant in ForgeCanvas. Help the user refine, \
        analyze, or modify their document. When the user references selected source chunks, \
        use them as context. If your response should update the document, wrap the new document \
        content in <document>...</document> tags. Otherwise, respond conversationally.";

    let user_message = format!("{context}USER: {message}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_GENERATE_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": "dolphin3:8b",
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
                    "Cannot connect to Ollama for chat",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Chat request failed: {e}"),
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
            .with_details(body),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse response: {e}"))
    })?;

    let ai_response = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    // Check if AI wants to update the document
    let updated_output = if let (Some(start), Some(end)) = (
        ai_response.find("<document>"),
        ai_response.rfind("</document>"),
    ) {
        let doc_content = ai_response[start + 10..end].trim().to_string();
        project.output_content = doc_content.clone();
        Some(doc_content)
    } else {
        None
    };

    // Strip <document> tags from the chat message
    let clean_message = if ai_response.contains("<document>") {
        let before_doc = ai_response
            .split("<document>")
            .next()
            .unwrap_or("")
            .trim();
        let after_doc = ai_response
            .rsplit("</document>")
            .next()
            .unwrap_or("")
            .trim();
        format!("{before_doc}\n{after_doc}").trim().to_string()
    } else {
        ai_response.clone()
    };

    let display_message = if clean_message.is_empty() {
        "Document updated.".to_string()
    } else {
        clean_message
    };

    // Save chat message
    let user_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: message,
        referenced_chunks: selected_chunks.clone(),
        timestamp: now_iso(),
    };
    let ai_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        content: display_message.clone(),
        referenced_chunks: selected_chunks.clone(),
        timestamp: now_iso(),
    };

    project.chat_history.push(user_msg);
    project.chat_history.push(ai_msg);
    project.updated_at = now_iso();
    write_project(&pp, &project)?;

    Ok(ChatResponse {
        message: display_message,
        referenced_chunks: selected_chunks,
        updated_output,
    })
}

/// Get the list of built-in templates.
#[tauri::command]
pub async fn canvas_get_templates() -> AppResult<Vec<TemplateInfo>> {
    Ok(builtin_templates())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_type_from_str() {
        assert_eq!(OutputType::from_str_loose("business-report"), OutputType::BusinessReport);
        assert_eq!(OutputType::from_str_loose("QUARTERLY_REPORT"), OutputType::QuarterlyReport);
        assert_eq!(OutputType::from_str_loose("restaurant menu"), OutputType::RestaurantMenu);
        assert_eq!(OutputType::from_str_loose("invoice"), OutputType::Invoice);
        assert_eq!(OutputType::from_str_loose("unknown_thing"), OutputType::Custom);
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(detect_file_type("report.md"), "markdown");
        assert_eq!(detect_file_type("data.csv"), "csv");
        assert_eq!(detect_file_type("code.rs"), "code");
        assert_eq!(detect_file_type("config.yaml"), "config");
        assert_eq!(detect_file_type("document.pdf"), "pdf");
        assert_eq!(detect_file_type("mystery"), "unknown");
    }

    #[test]
    fn test_chunk_content_simple() {
        let content = "Line one\nLine two\nLine three";
        let chunks = chunk_content("src-1", content);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].source_id, "src-1");
        assert_eq!(chunks[0].line_start, 1);
    }

    #[test]
    fn test_chunk_content_paragraphs() {
        let content = "First paragraph with enough text to be meaningful and pass the threshold.\n\n\
                       Second paragraph also with enough content to be a separate chunk.";
        let chunks = chunk_content("src-2", content);
        assert!(chunks.len() >= 1);
    }

    #[test]
    fn test_chunk_content_empty() {
        let chunks = chunk_content("src-3", "");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_content_whitespace_only() {
        let chunks = chunk_content("src-4", "   \n\n   ");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_parse_source_links() {
        let output = "# Report\n<!-- section:1 chunk:abc123,def456 -->\nSome text\n<!-- section:2 chunk:ghi789 -->\nMore text";
        let links = parse_source_links(output);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].output_section_idx, 1);
        assert_eq!(links[0].chunk_ids, vec!["abc123", "def456"]);
        assert_eq!(links[1].output_section_idx, 2);
        assert_eq!(links[1].chunk_ids, vec!["ghi789"]);
    }

    #[test]
    fn test_parse_source_links_none() {
        let output = "Just some normal text\nNo comments here.";
        let links = parse_source_links(output);
        assert!(links.is_empty());
    }

    #[test]
    fn test_builtin_templates() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 8);
        assert_eq!(templates[0].id, "business-report");
        assert!(templates[2].has_background); // restaurant-menu
        assert!(templates[3].has_background); // action-card
        assert!(!templates[0].has_background); // business-report
    }

    #[test]
    fn test_now_iso_format() {
        let ts = now_iso();
        assert!(ts.contains('T'));
        assert!(ts.len() > 20);
    }
}
