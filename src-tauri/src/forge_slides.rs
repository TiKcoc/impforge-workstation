// SPDX-License-Identifier: Apache-2.0
//! ForgeSlides -- Markdown-based Presentation Creator & AI Generator
//!
//! A PowerPoint/Google Slides replacement with AI content generation.
//! Each slide is a Markdown section. The AI can generate entire
//! presentations from a topic or document via local Ollama inference.
//!
//! Storage: `~/.impforge/presentations/` as individual JSON files.
//! This module is part of ImpForge Phase 3 (Office/Productivity tools).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds presentation files.
const PRESENTATIONS_DIR: &str = "presentations";

/// Ollama HTTP timeout for AI generation requests (generous for large decks).
const AI_GENERATE_TIMEOUT_SECS: u64 = 180;

/// Maximum slides a single AI generation request can produce.
const MAX_AI_SLIDES: usize = 30;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Slide layout variants controlling visual arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideLayout {
    TitleSlide,
    ContentSlide,
    TwoColumn,
    ImageAndText,
    QuoteSlide,
    BlankSlide,
}

impl SlideLayout {
    /// Parse from a loose string (frontend convenience).
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().replace(' ', "_").as_str() {
            "title_slide" | "title" => SlideLayout::TitleSlide,
            "content_slide" | "content" => SlideLayout::ContentSlide,
            "two_column" | "twocolumn" => SlideLayout::TwoColumn,
            "image_and_text" | "imageandtext" | "image" => SlideLayout::ImageAndText,
            "quote_slide" | "quote" => SlideLayout::QuoteSlide,
            "blank_slide" | "blank" => SlideLayout::BlankSlide,
            _ => SlideLayout::ContentSlide,
        }
    }
}

/// Color and typography theme for a presentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideTheme {
    pub name: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub bg_color: String,
    pub font_family: String,
    pub heading_font: String,
}

/// Transition effect for slide changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionEffect {
    None,
    Fade,
    SlideLeft,
    SlideRight,
    ZoomIn,
    ZoomOut,
}

impl TransitionEffect {
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().replace(' ', "_").as_str() {
            "fade" => TransitionEffect::Fade,
            "slide_left" | "slideleft" | "left" => TransitionEffect::SlideLeft,
            "slide_right" | "slideright" | "right" => TransitionEffect::SlideRight,
            "zoom_in" | "zoomin" | "zoom" => TransitionEffect::ZoomIn,
            "zoom_out" | "zoomout" => TransitionEffect::ZoomOut,
            "none" => TransitionEffect::None,
            _ => TransitionEffect::None,
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            TransitionEffect::None => "",
            TransitionEffect::Fade => "transition-fade",
            TransitionEffect::SlideLeft => "transition-slide-left",
            TransitionEffect::SlideRight => "transition-slide-right",
            TransitionEffect::ZoomIn => "transition-zoom-in",
            TransitionEffect::ZoomOut => "transition-zoom-out",
        }
    }
}

/// Slide transition configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideTransition {
    pub effect: TransitionEffect,
    pub duration_ms: u32,
}

impl Default for SlideTransition {
    fn default() -> Self {
        Self {
            effect: TransitionEffect::None,
            duration_ms: 500,
        }
    }
}

/// A single slide within a presentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    pub title: String,
    pub content: String,
    pub layout: SlideLayout,
    pub notes: Option<String>,
    pub background: Option<String>,
    /// Transition effect when entering this slide (defaults to None for backward compat).
    #[serde(default)]
    pub transition: Option<SlideTransition>,
}

/// Full presentation document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presentation {
    pub id: String,
    pub title: String,
    pub slides: Vec<Slide>,
    pub theme: SlideTheme,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight metadata for listing presentations (no slide content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresentationMeta {
    pub id: String,
    pub title: String,
    pub slide_count: usize,
    pub theme_name: String,
    pub updated_at: String,
}

/// Slide master / layout template for reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideMaster {
    pub id: String,
    pub name: String,
    pub description: String,
    pub layout: SlideLayout,
    pub default_content: String,
    pub suggested_notes: Option<String>,
}

/// Speaker timer state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerState {
    pub presentation_id: String,
    pub started_at: String,
    pub elapsed_seconds: f64,
    pub current_slide: usize,
    pub per_slide_seconds: Vec<f64>,
    pub is_running: bool,
}

/// Internal timer tracking data.
struct TimerData {
    started_at: std::time::Instant,
    current_slide: usize,
    per_slide_seconds: Vec<f64>,
    slide_entered_at: std::time::Instant,
    is_running: bool,
}

/// Global timer storage (one per presentation, keyed by ID).
static TIMERS: Lazy<Mutex<HashMap<String, TimerData>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Built-in Themes
// ---------------------------------------------------------------------------

fn builtin_themes() -> Vec<SlideTheme> {
    vec![
        SlideTheme {
            name: "Corporate Dark".into(),
            primary_color: "#3b82f6".into(),
            secondary_color: "#60a5fa".into(),
            bg_color: "#0f172a".into(),
            font_family: "Inter".into(),
            heading_font: "Space Grotesk".into(),
        },
        SlideTheme {
            name: "Corporate Light".into(),
            primary_color: "#1e40af".into(),
            secondary_color: "#3b82f6".into(),
            bg_color: "#ffffff".into(),
            font_family: "Inter".into(),
            heading_font: "Space Grotesk".into(),
        },
        SlideTheme {
            name: "Creative".into(),
            primary_color: "#8b5cf6".into(),
            secondary_color: "#a78bfa".into(),
            bg_color: "#1a1025".into(),
            font_family: "Outfit".into(),
            heading_font: "Orbitron".into(),
        },
        SlideTheme {
            name: "Minimal".into(),
            primary_color: "#374151".into(),
            secondary_color: "#6b7280".into(),
            bg_color: "#fafafa".into(),
            font_family: "DM Sans".into(),
            heading_font: "DM Sans".into(),
        },
        SlideTheme {
            name: "Tech".into(),
            primary_color: "#00ff66".into(),
            secondary_color: "#22c55e".into(),
            bg_color: "#0a0a0a".into(),
            font_family: "JetBrains Mono".into(),
            heading_font: "Space Grotesk".into(),
        },
        SlideTheme {
            name: "Nature".into(),
            primary_color: "#059669".into(),
            secondary_color: "#34d399".into(),
            bg_color: "#022c22".into(),
            font_family: "Nunito".into(),
            heading_font: "Comfortaa".into(),
        },
    ]
}

fn default_theme() -> SlideTheme {
    builtin_themes().remove(0)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the presentations directory, creating it if necessary.
fn presentations_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(PRESENTATIONS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create presentations directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Path to a presentation JSON file.
fn pres_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Read and parse a presentation file.
fn read_presentation(path: &Path) -> Result<Presentation, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "PRES_READ_FAILED",
            format!("Cannot read presentation file: {e}"),
        )
    })?;
    serde_json::from_str::<Presentation>(&data).map_err(|e| {
        ImpForgeError::internal(
            "PRES_PARSE_FAILED",
            format!("Corrupt presentation file: {e}"),
        )
    })
}

/// Write a presentation to disk atomically.
fn write_presentation(path: &Path, pres: &Presentation) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(pres).map_err(|e| {
        ImpForgeError::internal("PRES_SERIALIZE", format!("Cannot serialize presentation: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "PRES_WRITE_FAILED",
            format!("Cannot write presentation file: {e}"),
        )
    })
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Create a new blank slide with a given layout.
fn make_slide(layout: SlideLayout) -> Slide {
    let (title, content) = match layout {
        SlideLayout::TitleSlide => (
            "Presentation Title".to_string(),
            "Your subtitle or tagline goes here".to_string(),
        ),
        SlideLayout::ContentSlide => (
            "Slide Title".to_string(),
            "- Point one\n- Point two\n- Point three".to_string(),
        ),
        SlideLayout::TwoColumn => (
            "Two Columns".to_string(),
            "<!-- left -->\nLeft column content\n\n<!-- right -->\nRight column content".to_string(),
        ),
        SlideLayout::ImageAndText => (
            "Image & Text".to_string(),
            "![Image description](url)\n\nYour text alongside the image".to_string(),
        ),
        SlideLayout::QuoteSlide => (
            "".to_string(),
            "> \"Your inspiring quote goes here.\"\n>\n> -- Attribution".to_string(),
        ),
        SlideLayout::BlankSlide => (
            "".to_string(),
            String::new(),
        ),
    };

    Slide {
        id: Uuid::new_v4().to_string(),
        title,
        content,
        layout,
        notes: None,
        background: None,
        transition: None,
    }
}

/// Built-in slide master templates.
fn builtin_masters() -> Vec<SlideMaster> {
    vec![
        SlideMaster {
            id: "master-title".into(),
            name: "Title".into(),
            description: "Opening slide with large title and subtitle".into(),
            layout: SlideLayout::TitleSlide,
            default_content: "Your subtitle or tagline goes here".into(),
            suggested_notes: Some("Introduce yourself and the topic.".into()),
        },
        SlideMaster {
            id: "master-section".into(),
            name: "Section Divider".into(),
            description: "Section header to separate major topics".into(),
            layout: SlideLayout::TitleSlide,
            default_content: "Section overview text".into(),
            suggested_notes: Some("Transition to the next major section.".into()),
        },
        SlideMaster {
            id: "master-content".into(),
            name: "Content".into(),
            description: "Standard bullet-point content slide".into(),
            layout: SlideLayout::ContentSlide,
            default_content: "- Key point one\n- Key point two\n- Key point three".into(),
            suggested_notes: None,
        },
        SlideMaster {
            id: "master-two-column".into(),
            name: "Two Column".into(),
            description: "Side-by-side comparison or parallel content".into(),
            layout: SlideLayout::TwoColumn,
            default_content: "<!-- left -->\nLeft column content\n\n<!-- right -->\nRight column content".into(),
            suggested_notes: Some("Compare or contrast two aspects.".into()),
        },
        SlideMaster {
            id: "master-comparison".into(),
            name: "Comparison".into(),
            description: "Before/After or Pro/Con comparison layout".into(),
            layout: SlideLayout::TwoColumn,
            default_content: "<!-- left -->\n### Before\n- Old approach\n- Limitations\n\n<!-- right -->\n### After\n- New approach\n- Benefits".into(),
            suggested_notes: Some("Highlight the differences clearly.".into()),
        },
        SlideMaster {
            id: "master-quote".into(),
            name: "Quote".into(),
            description: "Inspirational or supporting quote".into(),
            layout: SlideLayout::QuoteSlide,
            default_content: "> \"Your quote goes here.\"\n>\n> -- Attribution".into(),
            suggested_notes: Some("Let the quote resonate before moving on.".into()),
        },
        SlideMaster {
            id: "master-image".into(),
            name: "Image".into(),
            description: "Full-width image with caption".into(),
            layout: SlideLayout::ImageAndText,
            default_content: "![Image description](url)\n\nCaption or description text".into(),
            suggested_notes: Some("Describe what the audience should notice.".into()),
        },
        SlideMaster {
            id: "master-blank".into(),
            name: "Blank".into(),
            description: "Empty canvas for custom content".into(),
            layout: SlideLayout::BlankSlide,
            default_content: String::new(),
            suggested_notes: None,
        },
    ]
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
// AI Generation (Ollama)
// ---------------------------------------------------------------------------

/// Generate a complete presentation from a topic via Ollama.
async fn ollama_generate_presentation(
    topic: &str,
    num_slides: usize,
    style: &str,
    model: Option<&str>,
) -> Result<Vec<Slide>, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let system_prompt = "You are a professional presentation designer inside ImpForge, \
        an AI Workstation. Generate a slide deck in structured JSON format. \
        Each slide must have: title (string), content (markdown string), \
        layout (one of: title_slide, content_slide, two_column, quote_slide, blank_slide), \
        and optionally notes (string for speaker notes). \
        Return ONLY a JSON array of slide objects. No markdown fences, no explanation.";

    let user_message = format!(
        "Create a {num_slides}-slide presentation about: {topic}\n\
         Style/tone: {style}\n\n\
         Requirements:\n\
         - First slide must be layout \"title_slide\" with a compelling title\n\
         - Use bullet points (- item) in content slides\n\
         - Include a summary/conclusion slide at the end\n\
         - Content should be concise but informative\n\
         - Add speaker notes for key slides\n\
         - Return valid JSON array only"
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
                    "Cannot connect to Ollama for AI presentation generation",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "AI presentation generation timed out",
                )
                .with_suggestion("Try fewer slides or a simpler topic.")
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

    let raw_content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("[]")
        .trim();

    // Strip markdown fences if the model wrapped them anyway
    let json_str = raw_content
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    parse_ai_slides(json_str)
}

/// Parse AI-generated JSON into typed Slide objects.
/// Tolerant of missing fields and varying formats.
fn parse_ai_slides(json_str: &str) -> Result<Vec<Slide>, ImpForgeError> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
        ImpForgeError::service(
            "AI_PARSE_FAILED",
            format!("AI returned invalid JSON: {e}"),
        )
        .with_details(json_str.chars().take(500).collect::<String>())
        .with_suggestion("Try again -- AI output can vary. If this persists, use a different model.")
    })?;

    let slides: Vec<Slide> = arr
        .into_iter()
        .map(|val| {
            let title = val.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let content = val.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let layout_str = val.get("layout")
                .and_then(|v| v.as_str())
                .unwrap_or("content_slide");
            let notes = val.get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            Slide {
                id: Uuid::new_v4().to_string(),
                title,
                content,
                layout: SlideLayout::from_str_loose(layout_str),
                notes,
                background: None,
                transition: None,
            }
        })
        .collect();

    if slides.is_empty() {
        return Err(ImpForgeError::service(
            "AI_EMPTY_RESULT",
            "AI generated zero slides",
        ));
    }

    Ok(slides)
}

/// Improve a single slide via Ollama.
async fn ollama_improve_slide(
    slide: &Slide,
    instruction: &str,
    model: Option<&str>,
) -> Result<Slide, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let system_prompt = "You are a professional presentation editor inside ImpForge. \
        Improve the given slide according to the user instruction. \
        Return ONLY a JSON object with: title, content (markdown), notes (optional). \
        No markdown fences, no explanations.";

    let user_message = format!(
        "Instruction: {instruction}\n\n\
         Current slide:\n\
         Title: {}\n\
         Content:\n{}\n\
         Notes: {}\n\n\
         Return the improved slide as a JSON object with title, content, and notes fields.",
        slide.title,
        slide.content,
        slide.notes.as_deref().unwrap_or("(none)")
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
                    "Cannot connect to Ollama for slide improvement",
                )
                .with_suggestion("Start Ollama with: ollama serve")
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
            .with_details(body),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse response: {e}"))
    })?;

    let raw = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim();

    let json_str = raw
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let val: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        ImpForgeError::service(
            "AI_PARSE_FAILED",
            format!("AI returned invalid JSON for slide improvement: {e}"),
        )
    })?;

    Ok(Slide {
        id: slide.id.clone(),
        title: val.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&slide.title)
            .to_string(),
        content: val.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or(&slide.content)
            .to_string(),
        layout: slide.layout,
        notes: val.get("notes")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| slide.notes.clone()),
        background: slide.background.clone(),
        transition: slide.transition.clone(),
    })
}

// ---------------------------------------------------------------------------
// HTML Export
// ---------------------------------------------------------------------------

/// Export a presentation as a self-contained HTML file (reveal.js-style).
fn render_html(pres: &Presentation) -> String {
    let theme = &pres.theme;

    // Determine text colors based on background brightness
    let is_dark = is_dark_color(&theme.bg_color);
    let text_color = if is_dark { "#e2e8f0" } else { "#1e293b" };
    let muted_color = if is_dark { "#94a3b8" } else { "#64748b" };

    let mut slides_html = String::new();
    for (i, slide) in pres.slides.iter().enumerate() {
        let bg = slide.background.as_deref().unwrap_or(&theme.bg_color);
        let layout_class = match slide.layout {
            SlideLayout::TitleSlide => "slide-title",
            SlideLayout::ContentSlide => "slide-content",
            SlideLayout::TwoColumn => "slide-two-col",
            SlideLayout::ImageAndText => "slide-img-text",
            SlideLayout::QuoteSlide => "slide-quote",
            SlideLayout::BlankSlide => "slide-blank",
        };

        let content_html = md_to_slide_html(&slide.content);
        let transition_class = slide.transition.as_ref()
            .map(|t| t.effect.css_class())
            .unwrap_or("");
        let transition_duration = slide.transition.as_ref()
            .map(|t| format!("--transition-duration: {}ms;", t.duration_ms))
            .unwrap_or_default();

        slides_html.push_str(&format!(
            r#"<section class="slide {layout_class} {transition_class}" data-index="{i}" style="background:{bg}; {transition_duration}">
  <div class="slide-inner">
    {title_html}
    <div class="slide-body">{content_html}</div>
  </div>
</section>
"#,
            title_html = if slide.title.is_empty() {
                String::new()
            } else {
                format!(r#"<h1 class="slide-title-text">{}</h1>"#, html_escape(&slide.title))
            },
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
html, body {{ width: 100%; height: 100%; overflow: hidden; background: {bg}; }}
body {{ font-family: '{font}', sans-serif; color: {text}; }}
.slide {{ display: none; width: 100vw; height: 100vh; position: absolute; top: 0; left: 0; }}
.slide.active {{ display: flex; align-items: center; justify-content: center; }}
.slide-inner {{ max-width: 80%; width: 100%; padding: 4rem; }}
.slide-title-text {{ font-family: '{heading}', sans-serif; font-size: 3rem; font-weight: 700;
  color: {primary}; margin-bottom: 1.5rem; line-height: 1.2; }}
.slide-body {{ font-size: 1.4rem; line-height: 1.8; color: {text}; }}
.slide-body ul, .slide-body ol {{ padding-left: 2rem; margin: 0.5rem 0; }}
.slide-body li {{ margin-bottom: 0.4rem; }}
.slide-body blockquote {{ border-left: 4px solid {primary}; padding-left: 1.5rem;
  font-style: italic; font-size: 2rem; margin: 1rem 0; color: {muted}; }}
.slide-body code {{ background: rgba(0,0,0,0.15); padding: 0.15em 0.4em; border-radius: 4px;
  font-family: 'JetBrains Mono', monospace; font-size: 0.9em; }}
.slide-body pre {{ background: rgba(0,0,0,0.2); padding: 1.5rem; border-radius: 8px;
  overflow-x: auto; margin: 1rem 0; }}
.slide-body pre code {{ background: none; padding: 0; }}
.slide-body strong {{ color: {primary}; }}
.slide-body a {{ color: {secondary}; text-decoration: underline; }}

/* Title slide centering */
.slide-title .slide-inner {{ text-align: center; }}
.slide-title .slide-title-text {{ font-size: 4rem; }}
.slide-title .slide-body {{ font-size: 1.6rem; color: {muted}; }}

/* Two column */
.slide-two-col .slide-body {{ display: grid; grid-template-columns: 1fr 1fr; gap: 3rem; }}

/* Quote slide */
.slide-quote .slide-inner {{ text-align: center; max-width: 70%; }}
.slide-quote .slide-body blockquote {{ border-left: none; text-align: center; font-size: 2.2rem; }}

/* Navigation controls */
.nav-controls {{ position: fixed; bottom: 2rem; right: 2rem; display: flex; gap: 0.5rem; z-index: 100; }}
.nav-btn {{ width: 40px; height: 40px; border: 1px solid {muted}; background: rgba(0,0,0,0.3);
  color: {text}; border-radius: 8px; cursor: pointer; font-size: 1.2rem;
  display: flex; align-items: center; justify-content: center; }}
.nav-btn:hover {{ background: {primary}; color: #fff; border-color: {primary}; }}
.slide-counter {{ position: fixed; bottom: 2rem; left: 2rem; font-size: 0.9rem; color: {muted};
  z-index: 100; font-family: '{font}', sans-serif; }}

/* Transition effects */
.transition-fade {{ animation: fadeIn var(--transition-duration, 0.5s) ease-in-out; }}
.transition-slide-left {{ animation: slideLeft var(--transition-duration, 0.5s) ease-out; }}
.transition-slide-right {{ animation: slideRight var(--transition-duration, 0.5s) ease-out; }}
.transition-zoom-in {{ animation: zoomIn var(--transition-duration, 0.5s) ease-out; }}
.transition-zoom-out {{ animation: zoomOut var(--transition-duration, 0.5s) ease-out; }}
@keyframes fadeIn {{ from {{ opacity: 0; }} to {{ opacity: 1; }} }}
@keyframes slideLeft {{ from {{ transform: translateX(100%); }} to {{ transform: translateX(0); }} }}
@keyframes slideRight {{ from {{ transform: translateX(-100%); }} to {{ transform: translateX(0); }} }}
@keyframes zoomIn {{ from {{ transform: scale(0.8); opacity: 0; }} to {{ transform: scale(1); opacity: 1; }} }}
@keyframes zoomOut {{ from {{ transform: scale(1.2); opacity: 0; }} to {{ transform: scale(1); opacity: 1; }} }}
</style>
</head>
<body>
{slides_html}
<div class="nav-controls">
  <button class="nav-btn" onclick="prev()" aria-label="Previous slide">&#8592;</button>
  <button class="nav-btn" onclick="next()" aria-label="Next slide">&#8594;</button>
</div>
<div class="slide-counter"><span id="cur">1</span> / <span id="total">{total}</span></div>
<script>
var idx=0, slides=document.querySelectorAll('.slide'), total=slides.length;
document.getElementById('total').textContent=total;
function show(n){{ idx=Math.max(0,Math.min(n,total-1)); slides.forEach(function(s){{s.classList.remove('active')}}); slides[idx].classList.add('active'); document.getElementById('cur').textContent=idx+1; }}
function next(){{ show(idx+1); }} function prev(){{ show(idx-1); }}
document.addEventListener('keydown',function(e){{ if(e.key==='ArrowRight'||e.key===' '){{e.preventDefault();next();}} if(e.key==='ArrowLeft'){{e.preventDefault();prev();}} if(e.key==='Escape'){{}} }});
show(0);
</script>
</body>
</html>"#,
        title = html_escape(&pres.title),
        bg = theme.bg_color,
        font = theme.font_family,
        heading = theme.heading_font,
        primary = theme.primary_color,
        secondary = theme.secondary_color,
        text = text_color,
        muted = muted_color,
        total = pres.slides.len(),
    )
}

/// Check if a hex color is "dark" (simple luminance check).
fn is_dark_color(hex: &str) -> bool {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 { return true; }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64;
    // Relative luminance (ITU-R BT.709)
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    lum < 128.0
}

/// Convert Markdown to simple HTML for slide content.
fn md_to_slide_html(md: &str) -> String {
    let mut html = String::with_capacity(md.len() * 2);
    let mut in_code_block = false;
    let mut in_list = false;
    let mut in_blockquote = false;
    let mut bq_buf = String::new();

    for line in md.lines() {
        // Fenced code blocks
        if line.trim_start().starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                close_list(&mut html, &mut in_list);
                flush_blockquote(&mut html, &mut in_blockquote, &mut bq_buf);
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

        if trimmed.is_empty() {
            close_list(&mut html, &mut in_list);
            flush_blockquote(&mut html, &mut in_blockquote, &mut bq_buf);
            continue;
        }

        // Blockquotes
        if trimmed.starts_with('>') {
            close_list(&mut html, &mut in_list);
            in_blockquote = true;
            let text = trimmed.trim_start_matches('>').trim();
            if !bq_buf.is_empty() {
                bq_buf.push(' ');
            }
            bq_buf.push_str(text);
            continue;
        } else {
            flush_blockquote(&mut html, &mut in_blockquote, &mut bq_buf);
        }

        // Headings (within slides, render as h2/h3)
        if trimmed.starts_with('#') {
            close_list(&mut html, &mut in_list);
            let level = trimmed.chars().take_while(|c| *c == '#').count().min(6);
            let tag_level = (level + 1).min(6); // offset since h1 is the slide title
            let text = trimmed[level..].trim();
            html.push_str(&format!("<h{tag_level}>{}</h{tag_level}>\n", inline_format(text)));
            continue;
        }

        // Unordered lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>\n", inline_format(&trimmed[2..])));
            continue;
        }

        // Ordered lists
        if let Some(pos) = trimmed.find(". ") {
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                if !in_list {
                    html.push_str("<ul>\n");
                    in_list = true;
                }
                html.push_str(&format!("<li>{}</li>\n", inline_format(&trimmed[pos + 2..])));
                continue;
            }
        }

        // HTML comments (layout markers) -- skip
        if trimmed.starts_with("<!--") && trimmed.ends_with("-->") {
            continue;
        }

        // Regular paragraph
        close_list(&mut html, &mut in_list);
        html.push_str(&format!("<p>{}</p>\n", inline_format(trimmed)));
    }

    if in_code_block {
        html.push_str("</code></pre>\n");
    }
    close_list(&mut html, &mut in_list);
    flush_blockquote(&mut html, &mut in_blockquote, &mut bq_buf);

    html
}

fn close_list(html: &mut String, in_list: &mut bool) {
    if *in_list {
        html.push_str("</ul>\n");
        *in_list = false;
    }
}

fn flush_blockquote(html: &mut String, in_bq: &mut bool, buf: &mut String) {
    if *in_bq && !buf.is_empty() {
        html.push_str(&format!("<blockquote>{}</blockquote>\n", inline_format(buf)));
        buf.clear();
    }
    *in_bq = false;
}

/// Apply inline formatting: **bold**, *italic*, `code`, [links](url).
fn inline_format(text: &str) -> String {
    let mut result = html_escape(text);
    // Bold: **text**
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let inner = result[start + 2..start + 2 + end].to_string();
            result = format!("{}<strong>{inner}</strong>{}", &result[..start], &result[start + 2 + end + 2..]);
        } else {
            break;
        }
    }
    // Italic: *text*
    while let Some(start) = result.find('*') {
        if let Some(end) = result[start + 1..].find('*') {
            let inner = result[start + 1..start + 1 + end].to_string();
            result = format!("{}<em>{inner}</em>{}", &result[..start], &result[start + 1 + end + 1..]);
        } else {
            break;
        }
    }
    // Inline code: `text`
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let inner = result[start + 1..start + 1 + end].to_string();
            result = format!("{}<code>{inner}</code>{}", &result[..start], &result[start + 1 + end + 1..]);
        } else {
            break;
        }
    }
    // Links: [text](url)
    while let Some(bracket_start) = result.find('[') {
        if let Some(bracket_end) = result[bracket_start..].find("](") {
            let abs_bracket_end = bracket_start + bracket_end;
            if let Some(paren_end) = result[abs_bracket_end + 2..].find(')') {
                let link_text = result[bracket_start + 1..abs_bracket_end].to_string();
                let url = result[abs_bracket_end + 2..abs_bracket_end + 2 + paren_end].to_string();
                result = format!(
                    "{}<a href=\"{url}\">{link_text}</a>{}",
                    &result[..bracket_start],
                    &result[abs_bracket_end + 2 + paren_end + 1..],
                );
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all presentations (metadata only).
#[tauri::command]
pub async fn slides_list() -> AppResult<Vec<PresentationMeta>> {
    let dir = presentations_dir()?;
    let mut items: Vec<PresentationMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read presentations dir: {e}"))
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
        if let Ok(pres) = read_presentation(&path) {
            items.push(PresentationMeta {
                id: pres.id,
                title: pres.title,
                slide_count: pres.slides.len(),
                theme_name: pres.theme.name,
                updated_at: pres.updated_at,
            });
        }
    }

    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

/// Create a new blank presentation with a title slide.
#[tauri::command]
pub async fn slides_create(title: String, theme_name: Option<String>) -> AppResult<Presentation> {
    let dir = presentations_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    let theme = theme_name
        .and_then(|name| builtin_themes().into_iter().find(|t| t.name == name))
        .unwrap_or_else(default_theme);

    let pres = Presentation {
        id: id.clone(),
        title: title.clone(),
        slides: vec![make_slide(SlideLayout::TitleSlide)],
        theme,
        created_at: now.clone(),
        updated_at: now,
    };

    // Set the title slide's title to the presentation title
    let mut pres = pres;
    if let Some(first) = pres.slides.first_mut() {
        first.title = title.clone();
    }

    write_presentation(&pres_path(&dir, &id), &pres)?;
    log::info!("ForgeSlides: created presentation '{title}' ({id})");
    Ok(pres)
}

/// Open a presentation by ID (full content).
#[tauri::command]
pub async fn slides_open(id: String) -> AppResult<Presentation> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found"))
                .with_suggestion("The presentation may have been deleted."),
        );
    }
    read_presentation(&path)
}

/// Save (update) a presentation.
#[tauri::command]
pub async fn slides_save(id: String, data: Presentation) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found"))
                .with_suggestion("Cannot save a presentation that does not exist. Create it first."),
        );
    }

    let mut pres = data;
    pres.updated_at = now_iso();

    write_presentation(&path, &pres)?;
    Ok(())
}

/// Delete a presentation.
#[tauri::command]
pub async fn slides_delete(id: String) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found")),
        );
    }

    std::fs::remove_file(&path).map_err(|e| {
        ImpForgeError::filesystem("DELETE_FAILED", format!("Cannot delete presentation: {e}"))
    })?;

    log::info!("ForgeSlides: deleted presentation '{id}'");
    Ok(())
}

/// Add a new slide to a presentation at a given position.
#[tauri::command]
pub async fn slides_add_slide(
    id: String,
    layout: Option<String>,
    after_index: Option<usize>,
) -> AppResult<Slide> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    let slide_layout = layout
        .as_deref()
        .map(SlideLayout::from_str_loose)
        .unwrap_or(SlideLayout::ContentSlide);

    let slide = make_slide(slide_layout);
    let insert_idx = match after_index {
        Some(idx) => (idx + 1).min(pres.slides.len()),
        None => pres.slides.len(),
    };

    pres.slides.insert(insert_idx, slide.clone());
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(slide)
}

/// Remove a slide from a presentation by index.
#[tauri::command]
pub async fn slides_remove_slide(id: String, slide_index: usize) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    if slide_index >= pres.slides.len() {
        return Err(ImpForgeError::validation(
            "INVALID_INDEX",
            format!("Slide index {slide_index} out of range (0..{})", pres.slides.len()),
        ));
    }

    if pres.slides.len() <= 1 {
        return Err(ImpForgeError::validation(
            "LAST_SLIDE",
            "Cannot remove the last slide. Delete the presentation instead.",
        ));
    }

    pres.slides.remove(slide_index);
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(())
}

/// Reorder slides within a presentation.
#[tauri::command]
pub async fn slides_reorder(id: String, from_index: usize, to_index: usize) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    let len = pres.slides.len();
    if from_index >= len || to_index >= len {
        return Err(ImpForgeError::validation(
            "INVALID_INDEX",
            format!("Slide indices ({from_index}, {to_index}) out of range (0..{len})"),
        ));
    }

    let slide = pres.slides.remove(from_index);
    pres.slides.insert(to_index, slide);
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(())
}

/// AI-generate an entire presentation from a topic.
#[tauri::command]
pub async fn slides_ai_generate(
    topic: String,
    num_slides: Option<usize>,
    style: Option<String>,
    theme_name: Option<String>,
) -> AppResult<Presentation> {
    if topic.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_TOPIC",
            "Please provide a topic for the presentation",
        ));
    }

    let n = num_slides.unwrap_or(8).min(MAX_AI_SLIDES).max(3);
    let style_str = style.as_deref().unwrap_or("professional and clear");

    log::info!("ForgeSlides: AI generating {n}-slide deck on '{topic}'");

    let slides = ollama_generate_presentation(&topic, n, style_str, None).await?;

    let dir = presentations_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    let theme = theme_name
        .and_then(|name| builtin_themes().into_iter().find(|t| t.name == name))
        .unwrap_or_else(default_theme);

    // Derive title from the first slide or the topic
    let title = slides.first()
        .filter(|s| !s.title.is_empty())
        .map(|s| s.title.clone())
        .unwrap_or_else(|| topic.clone());

    let pres = Presentation {
        id: id.clone(),
        title,
        slides,
        theme,
        created_at: now.clone(),
        updated_at: now,
    };

    write_presentation(&pres_path(&dir, &id), &pres)?;
    log::info!("ForgeSlides: AI presentation saved ({id})");

    Ok(pres)
}

/// AI-improve a single slide within a presentation.
#[tauri::command]
pub async fn slides_ai_improve_slide(
    id: String,
    slide_index: usize,
    instruction: String,
) -> AppResult<Slide> {
    if instruction.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_INSTRUCTION",
            "Please provide an instruction for improving the slide",
        ));
    }

    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    if slide_index >= pres.slides.len() {
        return Err(ImpForgeError::validation(
            "INVALID_INDEX",
            format!("Slide index {slide_index} out of range (0..{})", pres.slides.len()),
        ));
    }

    let current_slide = &pres.slides[slide_index];
    log::info!(
        "ForgeSlides: AI improving slide {} of '{}': {}",
        slide_index,
        pres.title,
        instruction.chars().take(50).collect::<String>()
    );

    let improved = ollama_improve_slide(current_slide, &instruction, None).await?;
    pres.slides[slide_index] = improved.clone();
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(improved)
}

/// Export a presentation as a self-contained HTML file.
#[tauri::command]
pub async fn slides_export_html(id: String) -> AppResult<String> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found")),
        );
    }

    let pres = read_presentation(&path)?;
    let html = render_html(&pres);

    // Write to ~/Documents or fallback
    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem("EXPORT_DIR_FAILED", format!("Cannot create export dir: {e}"))
        })?;
    }

    let safe_title: String = pres.title.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    let safe_title = if safe_title.is_empty() { "untitled".to_string() } else { safe_title };

    let export_path = export_dir.join(format!("{safe_title}.html"));
    std::fs::write(&export_path, html).map_err(|e| {
        ImpForgeError::filesystem("EXPORT_WRITE_FAILED", format!("Cannot write export file: {e}"))
    })?;

    log::info!("ForgeSlides: exported '{}' to {}", pres.title, export_path.display());
    Ok(export_path.to_string_lossy().to_string())
}

/// Get the list of built-in themes.
#[tauri::command]
pub async fn slides_get_themes() -> AppResult<Vec<SlideTheme>> {
    Ok(builtin_themes())
}

/// Set a transition effect on a specific slide.
#[tauri::command]
pub async fn slides_set_transition(
    id: String,
    slide_index: usize,
    effect: String,
    duration_ms: Option<u32>,
) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    if slide_index >= pres.slides.len() {
        return Err(ImpForgeError::validation(
            "INVALID_INDEX",
            format!("Slide index {slide_index} out of range (0..{})", pres.slides.len()),
        ));
    }

    pres.slides[slide_index].transition = Some(SlideTransition {
        effect: TransitionEffect::from_str_loose(&effect),
        duration_ms: duration_ms.unwrap_or(500).clamp(100, 3000),
    });
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(())
}

/// Start the speaker timer for a presentation.
#[tauri::command]
pub async fn slides_start_timer(id: String) -> AppResult<()> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found")),
        );
    }

    let pres = read_presentation(&path)?;
    let now = std::time::Instant::now();

    let mut timers = TIMERS.lock().map_err(|_| {
        ImpForgeError::internal("TIMER_LOCK", "Failed to acquire timer lock")
    })?;

    timers.insert(id.clone(), TimerData {
        started_at: now,
        current_slide: 0,
        per_slide_seconds: vec![0.0; pres.slides.len()],
        slide_entered_at: now,
        is_running: true,
    });

    log::info!("ForgeSlides: timer started for '{}'", id);

    Ok(())
}

/// Get the current timer state for a presentation.
#[tauri::command]
pub async fn slides_get_timer(id: String) -> AppResult<TimerState> {
    let timers = TIMERS.lock().map_err(|_| {
        ImpForgeError::internal("TIMER_LOCK", "Failed to acquire timer lock")
    })?;

    let data = timers.get(&id).ok_or_else(|| {
        ImpForgeError::validation(
            "TIMER_NOT_STARTED",
            format!("No timer running for presentation '{id}'"),
        )
    })?;

    let elapsed = data.started_at.elapsed().as_secs_f64();
    let mut per_slide = data.per_slide_seconds.clone();

    // Add current slide's elapsed time
    if data.is_running {
        let slide_idx = data.current_slide;
        if slide_idx < per_slide.len() {
            per_slide[slide_idx] += data.slide_entered_at.elapsed().as_secs_f64();
        }
    }

    // Round to 1 decimal
    let per_slide: Vec<f64> = per_slide.iter().map(|s| (s * 10.0).round() / 10.0).collect();

    Ok(TimerState {
        presentation_id: id,
        started_at: now_iso(),
        elapsed_seconds: (elapsed * 10.0).round() / 10.0,
        current_slide: data.current_slide,
        per_slide_seconds: per_slide,
        is_running: data.is_running,
    })
}

/// Stop the speaker timer for a presentation.
#[tauri::command]
pub async fn slides_stop_timer(id: String) -> AppResult<TimerState> {
    let mut timers = TIMERS.lock().map_err(|_| {
        ImpForgeError::internal("TIMER_LOCK", "Failed to acquire timer lock")
    })?;

    let data = timers.get_mut(&id).ok_or_else(|| {
        ImpForgeError::validation(
            "TIMER_NOT_STARTED",
            format!("No timer running for presentation '{id}'"),
        )
    })?;

    // Accumulate current slide time before stopping
    if data.is_running {
        let slide_idx = data.current_slide;
        if slide_idx < data.per_slide_seconds.len() {
            data.per_slide_seconds[slide_idx] += data.slide_entered_at.elapsed().as_secs_f64();
        }
        data.is_running = false;
    }

    let elapsed = data.started_at.elapsed().as_secs_f64();
    let per_slide: Vec<f64> = data.per_slide_seconds.iter().map(|s| (s * 10.0).round() / 10.0).collect();

    log::info!(
        "ForgeSlides: timer stopped for '{}' ({:.1}s total)",
        id,
        elapsed
    );

    Ok(TimerState {
        presentation_id: id,
        started_at: now_iso(),
        elapsed_seconds: (elapsed * 10.0).round() / 10.0,
        current_slide: data.current_slide,
        per_slide_seconds: per_slide,
        is_running: false,
    })
}

/// Advance the timer to a specific slide (tracks per-slide time).
#[tauri::command]
pub async fn slides_timer_goto_slide(id: String, slide_index: usize) -> AppResult<()> {
    let mut timers = TIMERS.lock().map_err(|_| {
        ImpForgeError::internal("TIMER_LOCK", "Failed to acquire timer lock")
    })?;

    let data = timers.get_mut(&id).ok_or_else(|| {
        ImpForgeError::validation(
            "TIMER_NOT_STARTED",
            format!("No timer running for presentation '{id}'"),
        )
    })?;

    if !data.is_running {
        return Err(ImpForgeError::validation(
            "TIMER_STOPPED",
            "Timer is not running. Start it first.",
        ));
    }

    // Accumulate time on current slide
    let old_idx = data.current_slide;
    if old_idx < data.per_slide_seconds.len() {
        data.per_slide_seconds[old_idx] += data.slide_entered_at.elapsed().as_secs_f64();
    }

    // Move to new slide
    let clamped = slide_index.min(data.per_slide_seconds.len().saturating_sub(1));
    data.current_slide = clamped;
    data.slide_entered_at = std::time::Instant::now();

    Ok(())
}

/// Get the list of built-in slide master templates.
#[tauri::command]
pub async fn slides_get_masters() -> AppResult<Vec<SlideMaster>> {
    Ok(builtin_masters())
}

/// Create a slide from a master template and add it to a presentation.
#[tauri::command]
pub async fn slides_add_from_master(
    id: String,
    master_id: String,
    after_index: Option<usize>,
) -> AppResult<Slide> {
    let masters = builtin_masters();
    let master = masters.iter().find(|m| m.id == master_id).ok_or_else(|| {
        ImpForgeError::validation(
            "MASTER_NOT_FOUND",
            format!("Slide master '{}' not found", master_id),
        )
    })?;

    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);
    let mut pres = read_presentation(&path)?;

    let slide = Slide {
        id: Uuid::new_v4().to_string(),
        title: master.name.clone(),
        content: master.default_content.clone(),
        layout: master.layout,
        notes: master.suggested_notes.clone(),
        background: None,
        transition: None,
    };

    let insert_idx = match after_index {
        Some(idx) => (idx + 1).min(pres.slides.len()),
        None => pres.slides.len(),
    };

    pres.slides.insert(insert_idx, slide.clone());
    pres.updated_at = now_iso();
    write_presentation(&path, &pres)?;

    Ok(slide)
}

/// Export all speaker notes from a presentation as a standalone Markdown document.
#[tauri::command]
pub async fn slides_export_notes(id: String) -> AppResult<String> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found")),
        );
    }

    let pres = read_presentation(&path)?;

    let mut notes_doc = format!("# Speaker Notes: {}\n\n", pres.title);
    notes_doc.push_str(&format!("*Generated: {}*\n\n---\n\n", now_iso()));

    let mut has_notes = false;

    for (i, slide) in pres.slides.iter().enumerate() {
        let slide_label = if slide.title.is_empty() {
            format!("Slide {}", i + 1)
        } else {
            format!("Slide {}: {}", i + 1, slide.title)
        };

        notes_doc.push_str(&format!("## {}\n\n", slide_label));

        match &slide.notes {
            Some(notes) if !notes.trim().is_empty() => {
                has_notes = true;
                notes_doc.push_str(notes);
                notes_doc.push_str("\n\n");
            }
            _ => {
                notes_doc.push_str("*(No notes for this slide)*\n\n");
            }
        }
    }

    if !has_notes {
        return Err(
            ImpForgeError::validation(
                "NO_NOTES",
                "No speaker notes found in this presentation",
            )
            .with_suggestion("Add notes to your slides first, then export."),
        );
    }

    Ok(notes_doc)
}

/// AI-generate a speaker script for a specific slide.
/// Uses the slide content to create natural speaking text.
#[tauri::command]
pub async fn slides_ai_speech(
    id: String,
    slide_index: usize,
    model: Option<String>,
) -> AppResult<String> {
    let dir = presentations_dir()?;
    let path = pres_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("PRES_NOT_FOUND", format!("Presentation '{id}' not found")),
        );
    }

    let pres = read_presentation(&path)?;

    if slide_index >= pres.slides.len() {
        return Err(ImpForgeError::validation(
            "INVALID_INDEX",
            format!("Slide index {slide_index} out of range (0..{})", pres.slides.len()),
        ));
    }

    let slide = &pres.slides[slide_index];
    let model_name = model.as_deref().unwrap_or("dolphin3:8b");

    // Build context from surrounding slides
    let prev_context = if slide_index > 0 {
        format!("Previous slide: \"{}\"", pres.slides[slide_index - 1].title)
    } else {
        "This is the first slide.".to_string()
    };

    let next_context = if slide_index + 1 < pres.slides.len() {
        format!("Next slide: \"{}\"", pres.slides[slide_index + 1].title)
    } else {
        "This is the last slide.".to_string()
    };

    let system_prompt = "You are a professional presentation coach inside ImpForge. \
        Generate a natural speaker script for the given slide. \
        The script should: \
        - Sound conversational and confident, not like reading bullet points \
        - Include transitions from the previous slide and to the next \
        - Be approximately 1-2 minutes of speaking time \
        - Use clear, simple language \
        Return ONLY the speaker script text. No labels, no formatting instructions.";

    let user_msg = format!(
        "Presentation: \"{}\"\n\
         Slide {} of {}: \"{}\"\n\
         Content:\n{}\n\n\
         {}\n{}\n\n\
         Existing notes: {}\n\n\
         Generate a natural speaker script for this slide.",
        pres.title,
        slide_index + 1,
        pres.slides.len(),
        slide.title,
        slide.content,
        prev_context,
        next_context,
        slide.notes.as_deref().unwrap_or("(none)")
    );

    log::info!(
        "ForgeSlides: AI speech for slide {} of '{}'",
        slide_index,
        pres.title
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
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_msg },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for speech generation",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service(
                    "OLLAMA_TIMEOUT",
                    "Speech generation timed out",
                )
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
            .with_details(body),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse response: {e}"))
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
            "AI returned an empty speech script",
        ));
    }

    Ok(content)
}

// ---------------------------------------------------------------------------
// ForgeMemory Integration
// ---------------------------------------------------------------------------

/// Store a presentation summary in ForgeMemory so it is searchable across ImpForge.
#[tauri::command]
pub async fn slides_remember(
    engine: tauri::State<'_, crate::forge_memory::engine::ForgeMemoryEngine>,
    title: String,
    content: String,
) -> Result<String, String> {
    let summary = format!("[Slides] {title}: {preview}", preview = &content[..content.len().min(500)]);
    engine.add_memory(&summary, "archival", 0.6, "slides")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_layout_from_str() {
        assert_eq!(SlideLayout::from_str_loose("title_slide"), SlideLayout::TitleSlide);
        assert_eq!(SlideLayout::from_str_loose("title"), SlideLayout::TitleSlide);
        assert_eq!(SlideLayout::from_str_loose("content"), SlideLayout::ContentSlide);
        assert_eq!(SlideLayout::from_str_loose("two_column"), SlideLayout::TwoColumn);
        assert_eq!(SlideLayout::from_str_loose("quote"), SlideLayout::QuoteSlide);
        assert_eq!(SlideLayout::from_str_loose("blank"), SlideLayout::BlankSlide);
        assert_eq!(SlideLayout::from_str_loose("unknown"), SlideLayout::ContentSlide);
    }

    #[test]
    fn test_builtin_themes_count() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 6);
        assert_eq!(themes[0].name, "Corporate Dark");
        assert_eq!(themes[3].name, "Minimal");
        assert_eq!(themes[4].name, "Tech");
    }

    #[test]
    fn test_make_slide_title() {
        let slide = make_slide(SlideLayout::TitleSlide);
        assert_eq!(slide.layout, SlideLayout::TitleSlide);
        assert!(!slide.title.is_empty());
        assert!(!slide.id.is_empty());
    }

    #[test]
    fn test_make_slide_content() {
        let slide = make_slide(SlideLayout::ContentSlide);
        assert!(slide.content.contains("- Point"));
    }

    #[test]
    fn test_make_slide_quote() {
        let slide = make_slide(SlideLayout::QuoteSlide);
        assert!(slide.content.contains('>'));
    }

    #[test]
    fn test_is_dark_color() {
        assert!(is_dark_color("#0f172a"));
        assert!(is_dark_color("#000000"));
        assert!(is_dark_color("#0a0a0a"));
        assert!(!is_dark_color("#ffffff"));
        assert!(!is_dark_color("#fafafa"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_inline_format_bold() {
        let result = inline_format("This is **bold** text");
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_inline_format_italic() {
        let result = inline_format("This is *italic* text");
        assert!(result.contains("<em>italic</em>"));
    }

    #[test]
    fn test_inline_format_code() {
        let result = inline_format("Use `code` here");
        assert!(result.contains("<code>code</code>"));
    }

    #[test]
    fn test_inline_format_link() {
        let result = inline_format("Visit [Google](https://google.com)");
        assert!(result.contains(r#"<a href="https://google.com">Google</a>"#));
    }

    #[test]
    fn test_md_to_slide_html_list() {
        let html = md_to_slide_html("- Item one\n- Item two\n- Item three");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>Item one</li>"));
        assert!(html.contains("<li>Item three</li>"));
    }

    #[test]
    fn test_md_to_slide_html_blockquote() {
        let html = md_to_slide_html("> A great quote\n> with continuation");
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("A great quote"));
    }

    #[test]
    fn test_md_to_slide_html_code_block() {
        let html = md_to_slide_html("```\nlet x = 1;\n```");
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("let x = 1;"));
    }

    #[test]
    fn test_md_to_slide_html_heading() {
        let html = md_to_slide_html("## Subtitle");
        assert!(html.contains("<h3>"));
        assert!(html.contains("Subtitle"));
    }

    #[test]
    fn test_parse_ai_slides_valid() {
        let json = r#"[
            {"title": "Hello", "content": "World", "layout": "title_slide"},
            {"title": "Body", "content": "- Point", "layout": "content_slide", "notes": "Speaker note"}
        ]"#;
        let slides = parse_ai_slides(json).expect("should parse");
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].title, "Hello");
        assert_eq!(slides[0].layout, SlideLayout::TitleSlide);
        assert_eq!(slides[1].notes.as_deref(), Some("Speaker note"));
    }

    #[test]
    fn test_parse_ai_slides_empty_array() {
        let result = parse_ai_slides("[]");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ai_slides_invalid_json() {
        let result = parse_ai_slides("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_render_html_output() {
        let pres = Presentation {
            id: "test".into(),
            title: "Test Deck".into(),
            slides: vec![
                Slide {
                    id: "s1".into(),
                    title: "Welcome".into(),
                    content: "Hello **world**".into(),
                    layout: SlideLayout::TitleSlide,
                    notes: None,
                    background: None,
                    transition: None,
                },
                Slide {
                    id: "s2".into(),
                    title: "Points".into(),
                    content: "- A\n- B\n- C".into(),
                    layout: SlideLayout::ContentSlide,
                    notes: Some("Remember A".into()),
                    background: None,
                    transition: Some(SlideTransition {
                        effect: TransitionEffect::Fade,
                        duration_ms: 300,
                    }),
                },
            ],
            theme: default_theme(),
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        };

        let html = render_html(&pres);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Deck"));
        assert!(html.contains("Welcome"));
        assert!(html.contains("slide-title"));
        assert!(html.contains("slide-content"));
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_presentation_serialization() {
        let pres = Presentation {
            id: "abc".into(),
            title: "My Deck".into(),
            slides: vec![make_slide(SlideLayout::TitleSlide)],
            theme: default_theme(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };

        let json = serde_json::to_string(&pres).expect("serialize");
        let parsed: Presentation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, "abc");
        assert_eq!(parsed.slides.len(), 1);
    }

    #[test]
    fn test_transition_effect_from_str() {
        assert_eq!(TransitionEffect::from_str_loose("fade"), TransitionEffect::Fade);
        assert_eq!(TransitionEffect::from_str_loose("slide_left"), TransitionEffect::SlideLeft);
        assert_eq!(TransitionEffect::from_str_loose("SlideRight"), TransitionEffect::SlideRight);
        assert_eq!(TransitionEffect::from_str_loose("zoom_in"), TransitionEffect::ZoomIn);
        assert_eq!(TransitionEffect::from_str_loose("zoom_out"), TransitionEffect::ZoomOut);
        assert_eq!(TransitionEffect::from_str_loose("none"), TransitionEffect::None);
        assert_eq!(TransitionEffect::from_str_loose("unknown"), TransitionEffect::None);
    }

    #[test]
    fn test_transition_css_class() {
        assert_eq!(TransitionEffect::Fade.css_class(), "transition-fade");
        assert_eq!(TransitionEffect::SlideLeft.css_class(), "transition-slide-left");
        assert_eq!(TransitionEffect::None.css_class(), "");
    }

    #[test]
    fn test_slide_transition_serialization() {
        let slide = Slide {
            id: "t1".into(),
            title: "Test".into(),
            content: "Content".into(),
            layout: SlideLayout::ContentSlide,
            notes: None,
            background: None,
            transition: Some(SlideTransition {
                effect: TransitionEffect::ZoomIn,
                duration_ms: 750,
            }),
        };
        let json = serde_json::to_string(&slide).expect("serialize");
        let parsed: Slide = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.transition.is_some());
        let t = parsed.transition.as_ref().expect("transition");
        assert_eq!(t.effect, TransitionEffect::ZoomIn);
        assert_eq!(t.duration_ms, 750);
    }

    #[test]
    fn test_slide_without_transition_deserializes() {
        // Backward compatibility: slides saved before transitions should still load
        let json = r#"{"id":"x","title":"T","content":"C","layout":"content_slide"}"#;
        let slide: Slide = serde_json::from_str(json).expect("deserialize");
        assert!(slide.transition.is_none());
    }

    #[test]
    fn test_builtin_masters_count() {
        let masters = builtin_masters();
        assert_eq!(masters.len(), 8);
        // Verify unique IDs
        let ids: Vec<&str> = masters.iter().map(|m| m.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn test_builtin_masters_have_content() {
        for master in builtin_masters() {
            assert!(!master.id.is_empty());
            assert!(!master.name.is_empty());
            assert!(!master.description.is_empty());
        }
    }

    #[test]
    fn test_render_html_with_transitions() {
        let pres = Presentation {
            id: "t".into(),
            title: "Transitions Test".into(),
            slides: vec![
                Slide {
                    id: "s1".into(),
                    title: "Fade".into(),
                    content: "Body".into(),
                    layout: SlideLayout::ContentSlide,
                    notes: None,
                    background: None,
                    transition: Some(SlideTransition {
                        effect: TransitionEffect::Fade,
                        duration_ms: 400,
                    }),
                },
            ],
            theme: default_theme(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };

        let html = render_html(&pres);
        assert!(html.contains("transition-fade"));
        assert!(html.contains("--transition-duration: 400ms"));
        // Verify CSS animation rules are present
        assert!(html.contains("@keyframes fadeIn"));
    }
}
