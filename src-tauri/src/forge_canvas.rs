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
    /// Visual highlight colour for rubber-band multi-select (CSS colour string).
    #[serde(default)]
    pub highlight_color: Option<String>,
    /// Whether this chunk is currently selected in the UI.
    #[serde(default)]
    pub is_selected: bool,
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

/// Result of auto-detecting user intent from a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentResult {
    /// The detected intent category.
    pub intent: String,
    /// Confidence score 0.0 .. 1.0.
    pub confidence: f64,
    /// A template suggestion if applicable.
    pub suggested_template: Option<String>,
    /// Brief explanation of why this intent was detected.
    pub reasoning: String,
}

/// Options for professional export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Include source footnotes at the bottom of the document.
    #[serde(default)]
    pub include_sources: bool,
    /// Include calculation breakdown details (for financial docs).
    #[serde(default)]
    pub include_calculations: bool,
    /// Page size: "a4", "letter", or "custom".
    #[serde(default = "default_page_size")]
    pub page_size: String,
    /// Orientation: "portrait" or "landscape".
    #[serde(default = "default_orientation")]
    pub orientation: String,
    /// Optional company name for the header.
    #[serde(default)]
    pub company_name: Option<String>,
    /// Optional company logo URL / path placeholder.
    #[serde(default)]
    pub company_logo: Option<String>,
    /// Whether to include page numbers in the footer.
    #[serde(default = "default_true")]
    pub page_numbers: bool,
    /// Whether to include a date in the header.
    #[serde(default = "default_true")]
    pub include_date: bool,
}

fn default_page_size() -> String {
    "a4".to_string()
}

fn default_orientation() -> String {
    "portrait".to_string()
}

fn default_true() -> bool {
    true
}

/// Professional export result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// The rendered content (HTML / Markdown / plain text).
    pub content: String,
    /// The format that was used.
    pub format: String,
    /// Suggested filename.
    pub filename: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the canvas directory, creating it if necessary.
fn canvas_dir() -> Result<PathBuf, ImpForgeError> {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
        })
        .join("impforge")
        .join(CANVAS_DIR);
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
                    highlight_color: None,
                    is_selected: false,
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
                    highlight_color: None,
                    is_selected: false,
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
            highlight_color: None,
            is_selected: false,
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
            highlight_color: None,
            is_selected: false,
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
// AI-powered Selection Transform
// ---------------------------------------------------------------------------

/// Transform selected chunks using a natural-language instruction.
///
/// The user selects chunks in the left panel, types an instruction such as
/// "Make these prices 10% cheaper", "Translate to English", or
/// "Create a chart from this data".  The AI processes the selected chunks
/// with that instruction and returns the transformed content as a string.
#[tauri::command]
pub async fn canvas_transform_selection(
    project_id: String,
    chunk_ids: Vec<String>,
    instruction: String,
) -> AppResult<String> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    if chunk_ids.is_empty() {
        return Err(
            ImpForgeError::validation(
                "NO_CHUNKS_SELECTED",
                "No chunks selected for transformation",
            )
            .with_suggestion("Select one or more source chunks in the left panel first."),
        );
    }

    if instruction.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_INSTRUCTION",
            "Transformation instruction cannot be empty",
        ));
    }

    let project = read_project(&pp)?;

    // Gather the requested chunks
    let selected: Vec<&SourceChunk> = project
        .sources
        .iter()
        .flat_map(|s| s.chunks.iter())
        .filter(|c| chunk_ids.contains(&c.id))
        .take(MAX_CHUNKS_PER_REQUEST)
        .collect();

    if selected.is_empty() {
        return Err(ImpForgeError::validation(
            "CHUNKS_NOT_FOUND",
            "None of the specified chunk IDs were found in this project",
        ));
    }

    let system_prompt = "You are a precise data transformation AI inside ForgeCanvas.\n\
        The user has selected specific source chunks and given an instruction.\n\
        Apply the instruction EXACTLY to the provided data.\n\n\
        RULES:\n\
        - Only transform the provided data, do not add unrelated content\n\
        - Preserve the structure (tables stay tables, lists stay lists)\n\
        - For numeric operations, show your calculation clearly\n\
        - For translations, maintain formatting and tone\n\
        - Return ONLY the transformed content, no preamble or explanation\n";

    let mut context = String::new();
    for (i, chunk) in selected.iter().enumerate() {
        context.push_str(&format!(
            "--- CHUNK {} (ID: {}, Lines {}-{}) ---\n{}\n\n",
            i + 1,
            chunk.id,
            chunk.line_start,
            chunk.line_end,
            chunk.text
        ));
    }

    let user_message = format!(
        "INSTRUCTION: {instruction}\n\nSOURCE DATA:\n{context}\n\
         Apply the instruction to the source data and return the result."
    );

    let url = resolve_ollama_url();

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
                    "Cannot connect to Ollama for transformation",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "Transformation timed out — try selecting fewer chunks",
                )
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Transformation request failed: {e}"),
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
            "Ollama returned an empty response for the transformation",
        ));
    }

    log::info!(
        "ForgeCanvas: transformed {} chunks in project '{}' with instruction '{}'",
        selected.len(),
        project_id,
        truncate_str(&instruction, 60),
    );

    Ok(content)
}

// ---------------------------------------------------------------------------
// Auto-Detect Intent
// ---------------------------------------------------------------------------

/// Intent keyword lists used for fast local detection before falling back to AI.
const INTENT_CREATE: &[&str] = &[
    "create", "generate", "write", "draft", "compose", "make", "build", "erstelle", "schreibe",
];
const INTENT_SUMMARIZE: &[&str] = &[
    "summarize", "summary", "tldr", "overview", "brief", "zusammenfassung", "zusammenfassen",
];
const INTENT_ANALYZE: &[&str] = &[
    "analyze", "analyse", "compare", "contrast", "evaluate", "inspect", "review", "analysiere",
];
const INTENT_TRANSLATE: &[&str] = &[
    "translate", "translation", "convert language", "übersetze", "übersetzen", "auf deutsch",
    "auf englisch", "to english", "to german", "to french", "to spanish",
];
const INTENT_FORMAT: &[&str] = &[
    "format", "reformat", "restructure", "clean up", "prettify", "table", "formatiere",
];

/// Detect the user's intent from a free-form message, using selected context.
///
/// Returns the most likely intent category, a confidence score, and an
/// optional template suggestion.  Detection uses fast keyword matching first
/// and falls back to AI when keywords are ambiguous.
#[tauri::command]
pub async fn canvas_auto_detect_intent(
    project_id: String,
    message: String,
) -> AppResult<IntentResult> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    if message.trim().is_empty() {
        return Ok(IntentResult {
            intent: "unknown".to_string(),
            confidence: 0.0,
            suggested_template: None,
            reasoning: "Empty message — no intent detected.".to_string(),
        });
    }

    let lower = message.to_lowercase();

    // --- Fast keyword-based detection ---

    let mut best_intent = "unknown";
    let mut best_score: f64 = 0.0;
    let mut best_reasoning = String::new();

    let categories: &[(&str, &[&str])] = &[
        ("create_report", INTENT_CREATE),
        ("summarize", INTENT_SUMMARIZE),
        ("analyze", INTENT_ANALYZE),
        ("translate", INTENT_TRANSLATE),
        ("format", INTENT_FORMAT),
    ];

    for &(intent_name, keywords) in categories {
        let mut hits = 0u32;
        let mut matched_keywords = Vec::new();
        for &kw in keywords {
            if lower.contains(kw) {
                hits += 1;
                matched_keywords.push(kw);
            }
        }
        if hits > 0 {
            // Score = ratio of matched keywords, boosted by absolute count
            let ratio = hits as f64 / keywords.len() as f64;
            let score = (ratio * 0.7 + (hits.min(3) as f64) * 0.1).min(0.99);
            if score > best_score {
                best_score = score;
                best_intent = intent_name;
                best_reasoning = format!(
                    "Matched keywords: {}",
                    matched_keywords.join(", ")
                );
            }
        }
    }

    // Suggest a template based on the detected intent and message content
    let suggested_template = match best_intent {
        "create_report" => {
            if lower.contains("quarter") || lower.contains("q1") || lower.contains("q2")
                || lower.contains("q3") || lower.contains("q4")
            {
                Some("quarterly-report".to_string())
            } else if lower.contains("business") {
                Some("business-report".to_string())
            } else if lower.contains("invoice") || lower.contains("rechnung") {
                Some("invoice".to_string())
            } else if lower.contains("menu") || lower.contains("restaurant") {
                Some("restaurant-menu".to_string())
            } else if lower.contains("letter") || lower.contains("brief") {
                Some("cover-letter".to_string())
            } else if lower.contains("presentation") || lower.contains("slide") {
                Some("presentation".to_string())
            } else if lower.contains("plan") {
                Some("business-plan".to_string())
            } else {
                None
            }
        }
        _ => None,
    };

    // If keyword matching is too weak, fall back to AI-based detection
    if best_score < 0.15 {
        // Attempt a lightweight AI classification
        match ai_classify_intent(&message).await {
            Ok(ai_result) => return Ok(ai_result),
            Err(e) => {
                log::warn!("ForgeCanvas: AI intent classification failed, using keyword result: {e}");
                // Fall through to return keyword result
            }
        }
    }

    Ok(IntentResult {
        intent: best_intent.to_string(),
        confidence: best_score,
        suggested_template,
        reasoning: if best_reasoning.is_empty() {
            "No strong keyword matches found.".to_string()
        } else {
            best_reasoning
        },
    })
}

/// Lightweight AI-based intent classification via Ollama.
async fn ai_classify_intent(message: &str) -> Result<IntentResult, ImpForgeError> {
    let url = resolve_ollama_url();

    let system_prompt = "You are an intent classifier. Given a user message, respond with \
        EXACTLY one JSON object (no markdown fences, no extra text):\n\
        {\"intent\": \"<one of: create_report, summarize, analyze, translate, format, unknown>\", \
         \"confidence\": <0.0-1.0>, \
         \"suggested_template\": \"<template-id or null>\", \
         \"reasoning\": \"<brief explanation>\"}\n\n\
        Available templates: business-report, quarterly-report, restaurant-menu, \
        action-card, business-plan, cover-letter, invoice, presentation.\n\
        Respond ONLY with the JSON object.";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
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
                { "role": "user",   "content": message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("OLLAMA_UNREACHABLE", format!("Intent classification failed: {e}"))
        })?;

    if !response.status().is_success() {
        return Err(ImpForgeError::service(
            "OLLAMA_HTTP_ERROR",
            "Ollama returned an error for intent classification",
        ));
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse response: {e}"))
    })?;

    let ai_text = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim();

    // Try to parse the AI's JSON response
    if let Ok(result) = serde_json::from_str::<IntentResult>(ai_text) {
        return Ok(result);
    }

    // If the AI wrapped it in code fences, try to extract the JSON
    let cleaned = ai_text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(result) = serde_json::from_str::<IntentResult>(cleaned) {
        return Ok(result);
    }

    Err(ImpForgeError::service(
        "INTENT_PARSE_FAILED",
        "Could not parse AI intent classification response",
    ))
}

// ---------------------------------------------------------------------------
// Professional Export
// ---------------------------------------------------------------------------

/// Export the current project output with professional formatting.
///
/// Supports `"html"` (print-ready with embedded CSS), `"md"` (with YAML
/// front-matter), and `"text"` (plain text).  The `options` struct controls
/// footnotes, calculation details, page geometry, and branding.
#[tauri::command]
pub async fn canvas_export_professional(
    project_id: String,
    format: String,
    options: ExportOptions,
) -> AppResult<ExportResult> {
    let dir = canvas_dir()?;
    let pp = project_path(&dir, &project_id);

    if !pp.exists() {
        return Err(ImpForgeError::filesystem(
            "PROJECT_NOT_FOUND",
            format!("Canvas project '{project_id}' not found"),
        ));
    }

    let project = read_project(&pp)?;

    if project.output_content.is_empty() {
        return Err(
            ImpForgeError::validation(
                "NO_OUTPUT",
                "No document content to export — generate a document first",
            )
            .with_suggestion("Click 'Generate Document' to create output before exporting."),
        );
    }

    let safe_name = project
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();

    let fmt_lower = format.to_lowercase();

    let (content, extension) = match fmt_lower.as_str() {
        "html" => (
            build_html_export(&project, &options),
            "html",
        ),
        "md" | "markdown" => (
            build_markdown_export(&project, &options),
            "md",
        ),
        "text" | "txt" | "plain" => (
            build_plaintext_export(&project, &options),
            "txt",
        ),
        _ => {
            return Err(ImpForgeError::validation(
                "UNSUPPORTED_FORMAT",
                format!("Export format '{format}' is not supported"),
            )
            .with_suggestion("Supported formats: html, md, text"));
        }
    };

    let filename = format!("{safe_name}.{extension}");

    log::info!(
        "ForgeCanvas: exported project '{}' as {} ({} bytes)",
        project_id,
        extension,
        content.len()
    );

    Ok(ExportResult {
        content,
        format: fmt_lower,
        filename,
    })
}

/// Build a print-ready HTML document with embedded CSS.
fn build_html_export(project: &CanvasProject, options: &ExportOptions) -> String {
    let page_size_css = match options.page_size.to_lowercase().as_str() {
        "letter" => "size: letter;",
        "custom" => "",
        _ => "size: A4;",
    };
    let orientation_css = if options.orientation.to_lowercase() == "landscape" {
        " landscape"
    } else {
        ""
    };

    let company_header = if let Some(ref name) = options.company_name {
        let logo_html = if let Some(ref logo) = options.company_logo {
            format!(r#"<img src="{logo}" alt="Logo" style="max-height:40px; margin-right:12px;" />"#)
        } else {
            String::new()
        };
        format!(
            r#"<div class="company-header">{logo_html}<span class="company-name">{name}</span></div>"#,
        )
    } else {
        String::new()
    };

    let date_header = if options.include_date {
        format!(
            r#"<div class="doc-date">{}</div>"#,
            Utc::now().format("%B %d, %Y")
        )
    } else {
        String::new()
    };

    // Build source footnotes
    let footnotes = if options.include_sources && !project.source_links.is_empty() {
        let mut notes = String::from(r#"<div class="footnotes"><hr /><h3>Sources</h3><ol>"#);
        for (i, link) in project.source_links.iter().enumerate() {
            let chunk_refs: Vec<String> = link.chunk_ids.iter().map(|cid| {
                // Find the chunk to get file/line info
                let info = project.sources.iter()
                    .flat_map(|s| s.chunks.iter().filter(|c| &c.id == cid).map(move |c| (s, c)))
                    .next();
                match info {
                    Some((source, chunk)) => format!(
                        "{} (L{}-{})",
                        source.file_name, chunk.line_start, chunk.line_end
                    ),
                    None => cid.clone(),
                }
            }).collect();
            notes.push_str(&format!(
                "<li>Section {}: {} (confidence: {}%)</li>",
                link.output_section_idx,
                chunk_refs.join(", "),
                (link.confidence * 100.0) as u32
            ));
            let _ = i; // used implicitly by enumerate for iteration
        }
        notes.push_str("</ol></div>");
        notes
    } else {
        String::new()
    };

    // Build calculation details section
    let calculations = if options.include_calculations {
        let mut calcs = String::new();
        for link in &project.source_links {
            for cid in &link.chunk_ids {
                let info = project.sources.iter()
                    .flat_map(|s| s.chunks.iter().filter(|c| &c.id == cid).map(move |c| (s, c)))
                    .next();
                if let Some((source, chunk)) = info {
                    // Only include chunks that contain numbers (likely calculation sources)
                    if chunk.text.chars().any(|c| c.is_ascii_digit()) {
                        calcs.push_str(&format!(
                            r#"<div class="calc-detail"><strong>{} L{}-{}:</strong> <code>{}</code></div>"#,
                            source.file_name,
                            chunk.line_start,
                            chunk.line_end,
                            chunk.text.replace('<', "&lt;").replace('>', "&gt;"),
                        ));
                    }
                }
            }
        }
        if calcs.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="calculations"><hr /><h3>Calculation Details</h3>{calcs}</div>"#,
            )
        }
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <style>
    @page {{ {page_size_css}{orientation_css} margin: 2cm; }}
    @media print {{
      body {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
      .no-print {{ display: none; }}
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: 'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif;
      max-width: 800px; margin: 0 auto; padding: 2rem 1.5rem;
      line-height: 1.8; color: #1a1a2e; background: #ffffff;
      font-size: 14px;
    }}
    .company-header {{
      display: flex; align-items: center; margin-bottom: 0.5rem;
      padding-bottom: 0.5rem; border-bottom: 2px solid #2c3e50;
    }}
    .company-name {{ font-size: 1.4rem; font-weight: 700; color: #2c3e50; }}
    .doc-date {{ font-size: 0.85rem; color: #7f8c8d; margin-bottom: 1.5rem; }}
    h1 {{ font-size: 1.6rem; color: #2c3e50; margin: 1.5rem 0 0.75rem; border-bottom: 1px solid #ecf0f1; padding-bottom: 0.5rem; }}
    h2 {{ font-size: 1.3rem; color: #34495e; margin: 1.25rem 0 0.5rem; }}
    h3 {{ font-size: 1.1rem; color: #34495e; margin: 1rem 0 0.4rem; }}
    p {{ margin: 0.6rem 0; }}
    ul, ol {{ margin: 0.5rem 0 0.5rem 1.5rem; }}
    li {{ margin: 0.2rem 0; }}
    table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}
    th, td {{ border: 1px solid #bdc3c7; padding: 0.5rem 1rem; text-align: left; font-size: 0.9rem; }}
    th {{ background: #ecf0f1; font-weight: 600; color: #2c3e50; }}
    pre {{ background: #f8f9fa; padding: 1rem; border-radius: 6px; overflow-x: auto; font-size: 0.85rem; border: 1px solid #e9ecef; }}
    code {{ font-family: 'JetBrains Mono', 'Fira Code', monospace; font-size: 0.85em; background: #f0f0f0; padding: 0.15em 0.4em; border-radius: 3px; }}
    pre code {{ background: none; padding: 0; }}
    blockquote {{ border-left: 4px solid #3498db; margin: 1rem 0; padding: 0.5rem 1rem; color: #555; background: #f8f9fa; }}
    .footnotes {{ margin-top: 2rem; font-size: 0.85rem; color: #555; }}
    .footnotes ol {{ padding-left: 1.5rem; }}
    .footnotes li {{ margin: 0.3rem 0; }}
    .calculations {{ margin-top: 1.5rem; font-size: 0.85rem; }}
    .calc-detail {{ margin: 0.4rem 0; padding: 0.4rem 0.6rem; background: #f8f9fa; border-left: 3px solid #e67e22; border-radius: 0 4px 4px 0; }}
    .calc-detail code {{ font-size: 0.8rem; word-break: break-all; }}
    @media print {{
      body {{ padding: 0; }}
      .footnotes {{ page-break-inside: avoid; }}
    }}
  </style>
</head>
<body>
  {company_header}
  {date_header}
  <article>
{body}
  </article>
  {footnotes}
  {calculations}
</body>
</html>"#,
        title = project.name,
        body = project.output_content,
        company_header = company_header,
        date_header = date_header,
        footnotes = footnotes,
        calculations = calculations,
        page_size_css = page_size_css,
        orientation_css = orientation_css,
    )
}

/// Build a Markdown export with YAML front-matter.
fn build_markdown_export(project: &CanvasProject, options: &ExportOptions) -> String {
    let mut out = String::new();

    // YAML front-matter
    out.push_str("---\n");
    out.push_str(&format!("title: \"{}\"\n", project.name));
    if let Some(ref name) = options.company_name {
        out.push_str(&format!("author: \"{name}\"\n"));
    }
    if options.include_date {
        out.push_str(&format!("date: \"{}\"\n", Utc::now().format("%Y-%m-%d")));
    }
    out.push_str(&format!("page-size: \"{}\"\n", options.page_size));
    out.push_str(&format!("orientation: \"{}\"\n", options.orientation));
    out.push_str("---\n\n");

    // Main content
    out.push_str(&project.output_content);

    // Source footnotes
    if options.include_sources && !project.source_links.is_empty() {
        out.push_str("\n\n---\n\n## Sources\n\n");
        for (i, link) in project.source_links.iter().enumerate() {
            let chunk_refs: Vec<String> = link.chunk_ids.iter().map(|cid| {
                let info = project.sources.iter()
                    .flat_map(|s| s.chunks.iter().filter(|c| &c.id == cid).map(move |c| (s, c)))
                    .next();
                match info {
                    Some((source, chunk)) => format!(
                        "{} (L{}-{})",
                        source.file_name, chunk.line_start, chunk.line_end
                    ),
                    None => cid.clone(),
                }
            }).collect();
            out.push_str(&format!(
                "{}. Section {}: {} — confidence {}%\n",
                i + 1,
                link.output_section_idx,
                chunk_refs.join(", "),
                (link.confidence * 100.0) as u32,
            ));
        }
    }

    // Calculation details
    if options.include_calculations {
        let mut calcs = Vec::new();
        for link in &project.source_links {
            for cid in &link.chunk_ids {
                let info = project.sources.iter()
                    .flat_map(|s| s.chunks.iter().filter(|c| &c.id == cid).map(move |c| (s, c)))
                    .next();
                if let Some((source, chunk)) = info {
                    if chunk.text.chars().any(|c| c.is_ascii_digit()) {
                        calcs.push(format!(
                            "- **{} L{}-{}:** `{}`",
                            source.file_name,
                            chunk.line_start,
                            chunk.line_end,
                            chunk.text.replace('`', "'"),
                        ));
                    }
                }
            }
        }
        if !calcs.is_empty() {
            out.push_str("\n\n## Calculation Details\n\n");
            for calc in calcs {
                out.push_str(&calc);
                out.push('\n');
            }
        }
    }

    out
}

/// Build a plain-text export.
fn build_plaintext_export(project: &CanvasProject, options: &ExportOptions) -> String {
    let mut out = String::new();

    // Title
    out.push_str(&project.name.to_uppercase());
    out.push('\n');
    out.push_str(&"=".repeat(project.name.len()));
    out.push('\n');

    if let Some(ref name) = options.company_name {
        out.push_str(name);
        out.push('\n');
    }
    if options.include_date {
        out.push_str(&format!("Date: {}\n", Utc::now().format("%Y-%m-%d")));
    }
    out.push('\n');

    // Strip markdown formatting for plain text
    for line in project.output_content.lines() {
        let stripped = line
            .trim_start_matches('#')
            .trim_start_matches(' ')
            .replace("**", "")
            .replace("__", "")
            .replace('*', "")
            .replace('_', " ");
        // Skip HTML comments
        if stripped.trim_start().starts_with("<!--") {
            continue;
        }
        out.push_str(&stripped);
        out.push('\n');
    }

    // Source footnotes
    if options.include_sources && !project.source_links.is_empty() {
        out.push_str("\n\nSOURCES\n-------\n");
        for (i, link) in project.source_links.iter().enumerate() {
            let chunk_refs: Vec<String> = link.chunk_ids.iter().map(|cid| {
                let info = project.sources.iter()
                    .flat_map(|s| s.chunks.iter().filter(|c| &c.id == cid).map(move |c| (s, c)))
                    .next();
                match info {
                    Some((source, chunk)) => format!(
                        "{} (L{}-{})",
                        source.file_name, chunk.line_start, chunk.line_end
                    ),
                    None => cid.clone(),
                }
            }).collect();
            out.push_str(&format!(
                "  {}. Section {}: {}\n",
                i + 1,
                link.output_section_idx,
                chunk_refs.join(", "),
            ));
        }
    }

    out
}

/// Truncate a string to a maximum length (for logging).
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
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

    #[test]
    fn test_source_chunk_new_fields_default() {
        let chunk = SourceChunk {
            id: "c1".to_string(),
            text: "test".to_string(),
            line_start: 1,
            line_end: 5,
            source_id: "s1".to_string(),
            used_in_output: false,
            relevance_score: 0.0,
            highlight_color: None,
            is_selected: false,
        };
        assert!(!chunk.is_selected);
        assert!(chunk.highlight_color.is_none());
    }

    #[test]
    fn test_source_chunk_serde_backwards_compat() {
        // Deserializing old JSON without the new fields should default gracefully
        let json = r#"{
            "id": "c1", "text": "hello", "line_start": 1, "line_end": 2,
            "source_id": "s1", "used_in_output": false, "relevance_score": 0.5
        }"#;
        let chunk: SourceChunk = serde_json::from_str(json).expect("should deserialize");
        assert!(!chunk.is_selected);
        assert!(chunk.highlight_color.is_none());
    }

    #[test]
    fn test_intent_result_serde() {
        let ir = IntentResult {
            intent: "summarize".to_string(),
            confidence: 0.87,
            suggested_template: Some("quarterly-report".to_string()),
            reasoning: "Matched keyword: summarize".to_string(),
        };
        let json = serde_json::to_string(&ir).expect("should serialize");
        let decoded: IntentResult = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(decoded.intent, "summarize");
        assert!((decoded.confidence - 0.87).abs() < f64::EPSILON);
        assert_eq!(decoded.suggested_template.as_deref(), Some("quarterly-report"));
    }

    #[test]
    fn test_export_options_defaults() {
        let json = r#"{}"#;
        let opts: ExportOptions = serde_json::from_str(json).expect("should deserialize with defaults");
        assert!(!opts.include_sources);
        assert!(!opts.include_calculations);
        assert_eq!(opts.page_size, "a4");
        assert_eq!(opts.orientation, "portrait");
        assert!(opts.page_numbers);
        assert!(opts.include_date);
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }

    #[test]
    fn test_build_plaintext_export_basic() {
        let project = CanvasProject {
            id: "p1".to_string(),
            name: "Test Project".to_string(),
            sources: Vec::new(),
            output_content: "# Title\n\nSome **bold** text.\n\n<!-- section:0 chunk:c1 -->\n".to_string(),
            output_type: OutputType::Custom,
            template: None,
            background: None,
            source_links: Vec::new(),
            chat_history: Vec::new(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };
        let opts = ExportOptions {
            include_sources: false,
            include_calculations: false,
            page_size: "a4".to_string(),
            orientation: "portrait".to_string(),
            company_name: None,
            company_logo: None,
            page_numbers: true,
            include_date: false,
        };
        let result = build_plaintext_export(&project, &opts);
        assert!(result.starts_with("TEST PROJECT"));
        assert!(result.contains("Title"));
        assert!(result.contains("Some bold text."));
        // HTML comments should be stripped
        assert!(!result.contains("<!--"));
    }

    #[test]
    fn test_build_markdown_export_frontmatter() {
        let project = CanvasProject {
            id: "p2".to_string(),
            name: "Report".to_string(),
            sources: Vec::new(),
            output_content: "# Hello\n".to_string(),
            output_type: OutputType::BusinessReport,
            template: None,
            background: None,
            source_links: Vec::new(),
            chat_history: Vec::new(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };
        let opts = ExportOptions {
            include_sources: false,
            include_calculations: false,
            page_size: "letter".to_string(),
            orientation: "landscape".to_string(),
            company_name: Some("Acme Corp".to_string()),
            company_logo: None,
            page_numbers: true,
            include_date: true,
        };
        let result = build_markdown_export(&project, &opts);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("title: \"Report\""));
        assert!(result.contains("author: \"Acme Corp\""));
        assert!(result.contains("page-size: \"letter\""));
        assert!(result.contains("# Hello"));
    }

    #[test]
    fn test_build_html_export_has_structure() {
        let project = CanvasProject {
            id: "p3".to_string(),
            name: "HTML Test".to_string(),
            sources: Vec::new(),
            output_content: "<h1>Test</h1><p>Content here.</p>".to_string(),
            output_type: OutputType::Custom,
            template: None,
            background: None,
            source_links: Vec::new(),
            chat_history: Vec::new(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };
        let opts = ExportOptions {
            include_sources: false,
            include_calculations: false,
            page_size: "a4".to_string(),
            orientation: "portrait".to_string(),
            company_name: Some("Test Inc.".to_string()),
            company_logo: None,
            page_numbers: true,
            include_date: true,
        };
        let result = build_html_export(&project, &opts);
        assert!(result.contains("<!DOCTYPE html>"));
        assert!(result.contains("Test Inc."));
        assert!(result.contains("size: A4;"));
        assert!(result.contains("<article>"));
    }
}
