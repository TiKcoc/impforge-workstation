// SPDX-License-Identifier: Apache-2.0
//! ForgeBrowser -- Arc-style Spaces, Layout Presets, and AI Browser Features
//!
//! Provides:
//! - **Spaces**: Arc-style container isolation (Work / Personal / Research)
//!   with per-space tab grouping and automatic domain assignment.
//! - **Tabs**: Pinned and unpinned browser tabs scoped to a space.
//! - **Layout Presets**: Developer, Full Stack, Researcher, Writer, Designer,
//!   Manager, AI Workspace, Zen -- each defines a panel arrangement.
//! - **AI Features**: Page summarization, translation, web clipping, and
//!   reader mode powered by Ollama local inference.
//!
//! All state is persisted to `~/.impforge/browser_spaces.json` so the user
//! never loses their workspace across restarts.

use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ============================================================================
// TYPES
// ============================================================================

/// A browser space groups tabs under a single context (like Arc spaces).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSpace {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub tabs: Vec<BrowserTab>,
    /// Domains auto-assigned to this space (e.g. "github.com" -> Work).
    pub auto_assign_domains: Vec<String>,
}

/// A single browser tab inside a space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub space_id: String,
    pub pinned: bool,
    pub is_active: bool,
    pub favicon: Option<String>,
    pub last_visited: String,
}

/// One of the built-in layout presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub layout: LayoutConfig,
}

/// Describes which panels are open and where.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    pub panels: Vec<PanelConfig>,
}

/// A single panel inside a layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    /// Module identifier: "ide", "browser", "chat", "terminal", "notes", etc.
    pub module: String,
    /// Position within the layout.
    pub position: String,
    /// Percentage of available space this panel occupies.
    pub size_percent: u32,
    /// Whether this panel is visible by default.
    pub visible: bool,
}

/// Result from an AI browser operation (summarize, translate, clip, reader).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiBrowserResult {
    pub url: String,
    pub operation: String,
    pub content: String,
    pub model_used: String,
    pub timestamp: String,
}

// ============================================================================
// PERSISTENCE
// ============================================================================

/// In-memory state for all spaces, persisted to disk on mutation.
struct SpaceStore {
    spaces: Vec<BrowserSpace>,
    active_preset: String,
}

impl SpaceStore {
    fn new() -> Self {
        Self {
            spaces: default_spaces(),
            active_preset: "developer".into(),
        }
    }
}

static SPACE_STORE: std::sync::LazyLock<Mutex<SpaceStore>> =
    std::sync::LazyLock::new(|| {
        let store = load_from_disk().unwrap_or_else(|_| SpaceStore::new());
        Mutex::new(store)
    });

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

fn spaces_path() -> PathBuf {
    data_dir().join("browser_spaces.json")
}

fn save_to_disk(spaces: &[BrowserSpace]) -> Result<(), ImpForgeError> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        ImpForgeError::filesystem("BROWSER_SAVE", format!("Cannot create data dir: {e}"))
    })?;
    let json = serde_json::to_string_pretty(spaces).map_err(ImpForgeError::from)?;
    std::fs::write(spaces_path(), json).map_err(|e| {
        ImpForgeError::filesystem("BROWSER_SAVE", format!("Cannot write spaces: {e}"))
    })?;
    Ok(())
}

fn load_from_disk() -> Result<SpaceStore, ImpForgeError> {
    let path = spaces_path();
    if !path.exists() {
        return Ok(SpaceStore::new());
    }
    let data = std::fs::read_to_string(&path).map_err(ImpForgeError::from)?;
    let spaces: Vec<BrowserSpace> =
        serde_json::from_str(&data).map_err(ImpForgeError::from)?;
    Ok(SpaceStore {
        spaces,
        active_preset: "developer".into(),
    })
}

// ============================================================================
// DEFAULT DATA
// ============================================================================

fn default_spaces() -> Vec<BrowserSpace> {
    vec![
        BrowserSpace {
            id: "space-work".into(),
            name: "Work".into(),
            color: "#00ff66".into(),
            icon: "Briefcase".into(),
            tabs: Vec::new(),
            auto_assign_domains: vec![
                "github.com".into(),
                "gitlab.com".into(),
                "stackoverflow.com".into(),
                "jira.atlassian.net".into(),
            ],
        },
        BrowserSpace {
            id: "space-personal".into(),
            name: "Personal".into(),
            color: "#7c3aed".into(),
            icon: "User".into(),
            tabs: Vec::new(),
            auto_assign_domains: vec![
                "youtube.com".into(),
                "reddit.com".into(),
                "twitter.com".into(),
            ],
        },
        BrowserSpace {
            id: "space-research".into(),
            name: "Research".into(),
            color: "#06b6d4".into(),
            icon: "BookOpen".into(),
            tabs: Vec::new(),
            auto_assign_domains: vec![
                "arxiv.org".into(),
                "scholar.google.com".into(),
                "huggingface.co".into(),
                "docs.rs".into(),
            ],
        },
    ]
}

/// Returns the 8 built-in layout presets.
fn built_in_presets() -> Vec<LayoutPreset> {
    vec![
        LayoutPreset {
            id: "developer".into(),
            name: "Developer".into(),
            description: "Code editor with terminal and browser side-by-side".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "ide".into(), position: "left".into(), size_percent: 40, visible: true },
                    PanelConfig { module: "terminal".into(), position: "bottom-left".into(), size_percent: 20, visible: true },
                    PanelConfig { module: "browser".into(), position: "right".into(), size_percent: 40, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "fullstack".into(),
            name: "Full Stack".into(),
            description: "IDE, browser, terminal, and chat in a quad layout".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "ide".into(), position: "top-left".into(), size_percent: 30, visible: true },
                    PanelConfig { module: "browser".into(), position: "top-right".into(), size_percent: 30, visible: true },
                    PanelConfig { module: "terminal".into(), position: "bottom-left".into(), size_percent: 20, visible: true },
                    PanelConfig { module: "chat".into(), position: "bottom-right".into(), size_percent: 20, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "researcher".into(),
            name: "Researcher".into(),
            description: "Browser-centric layout with notes and AI chat".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "browser".into(), position: "center".into(), size_percent: 55, visible: true },
                    PanelConfig { module: "notes".into(), position: "right".into(), size_percent: 25, visible: true },
                    PanelConfig { module: "chat".into(), position: "bottom-right".into(), size_percent: 20, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "writer".into(),
            name: "Writer".into(),
            description: "Distraction-free writing with reference browser".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "writer".into(), position: "center".into(), size_percent: 70, visible: true },
                    PanelConfig { module: "browser".into(), position: "right".into(), size_percent: 30, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "designer".into(),
            name: "Designer".into(),
            description: "Visual workspace with browser for inspiration".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "browser".into(), position: "left".into(), size_percent: 45, visible: true },
                    PanelConfig { module: "notes".into(), position: "right".into(), size_percent: 35, visible: true },
                    PanelConfig { module: "chat".into(), position: "bottom-right".into(), size_percent: 20, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "manager".into(),
            name: "Manager".into(),
            description: "Dashboard overview with calendar and mail".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "browser".into(), position: "left".into(), size_percent: 40, visible: true },
                    PanelConfig { module: "calendar".into(), position: "top-right".into(), size_percent: 35, visible: true },
                    PanelConfig { module: "mail".into(), position: "bottom-right".into(), size_percent: 25, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "ai-workspace".into(),
            name: "AI Workspace".into(),
            description: "Chat, models, and browser for AI experimentation".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "chat".into(), position: "left".into(), size_percent: 40, visible: true },
                    PanelConfig { module: "browser".into(), position: "top-right".into(), size_percent: 35, visible: true },
                    PanelConfig { module: "terminal".into(), position: "bottom-right".into(), size_percent: 25, visible: true },
                ],
            },
        },
        LayoutPreset {
            id: "zen".into(),
            name: "Zen".into(),
            description: "Single focused panel, nothing else".into(),
            layout: LayoutConfig {
                panels: vec![
                    PanelConfig { module: "browser".into(), position: "center".into(), size_percent: 100, visible: true },
                ],
            },
        },
    ]
}

// ============================================================================
// HELPERS
// ============================================================================

/// Find which space a domain should be auto-assigned to.
fn find_space_for_domain(spaces: &[BrowserSpace], url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_string();
    for space in spaces {
        for domain in &space.auto_assign_domains {
            if host.ends_with(domain) {
                return Some(space.id.clone());
            }
        }
    }
    None
}

/// Fetch page content via HTTP for AI processing.
async fn fetch_page_content(url: &str) -> Result<String, ImpForgeError> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("ImpForge/0.6 (AI Workstation Builder)")
        .build()
        .map_err(ImpForgeError::from)?;

    let response = client.get(url).send().await.map_err(ImpForgeError::from)?;
    if !response.status().is_success() {
        return Err(ImpForgeError::browser(
            "FETCH_FAILED",
            format!("HTTP {} for {}", response.status(), url),
        ));
    }

    let html = response.text().await.map_err(ImpForgeError::from)?;

    // Strip HTML tags for a rough text extraction (light-weight, no heavy dep)
    let text = strip_html_tags(&html);
    // Limit to ~8000 chars to avoid blowing up the LLM context
    let truncated = if text.len() > 8000 {
        format!("{}...\n\n[Content truncated at 8000 chars]", &text[..8000])
    } else {
        text
    };
    Ok(truncated)
}

/// Minimal HTML tag stripper without pulling in a full parser.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut inside_tag = false;
    let mut inside_script = false;
    let mut inside_style = false;
    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !inside_tag && check_prefix(&lower_chars, i, "<script") {
            inside_script = true;
            inside_tag = true;
        } else if !inside_tag && check_prefix(&lower_chars, i, "<style") {
            inside_style = true;
            inside_tag = true;
        } else if inside_script && check_prefix(&lower_chars, i, "</script") {
            inside_script = false;
            inside_tag = true;
        } else if inside_style && check_prefix(&lower_chars, i, "</style") {
            inside_style = false;
            inside_tag = true;
        } else if chars[i] == '<' {
            inside_tag = true;
        } else if chars[i] == '>' {
            inside_tag = false;
            i += 1;
            continue;
        }

        if !inside_tag && !inside_script && !inside_style {
            result.push(chars[i]);
        }
        i += 1;
    }

    // Collapse whitespace runs
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_ws = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                collapsed.push(if ch == '\n' { '\n' } else { ' ' });
            }
            prev_ws = true;
        } else {
            collapsed.push(ch);
            prev_ws = false;
        }
    }
    collapsed.trim().to_string()
}

fn check_prefix(chars: &[char], start: usize, prefix: &str) -> bool {
    let prefix_chars: Vec<char> = prefix.chars().collect();
    if start + prefix_chars.len() > chars.len() {
        return false;
    }
    chars[start..start + prefix_chars.len()] == prefix_chars[..]
}

/// Send a prompt to the local Ollama instance and return the response text.
async fn ollama_generate(prompt: &str, model: &str) -> Result<String, ImpForgeError> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(ImpForgeError::from)?;

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.3, "num_predict": 2048 }
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("OLLAMA_UNREACHABLE", "Cannot connect to Ollama")
                .with_details(e.to_string())
                .with_suggestion("Start Ollama with: ollama serve")
        })?;

    if !resp.status().is_success() {
        return Err(ImpForgeError::service(
            "OLLAMA_ERROR",
            format!("Ollama returned HTTP {}", resp.status()),
        ));
    }

    #[derive(Deserialize)]
    struct OllamaResponse {
        response: String,
    }

    let data: OllamaResponse = resp.json().await.map_err(ImpForgeError::from)?;
    Ok(data.response.trim().to_string())
}

// ============================================================================
// TAURI COMMANDS -- SPACES
// ============================================================================

#[tauri::command]
pub async fn browser_list_spaces() -> AppResult<Vec<BrowserSpace>> {
    let store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    Ok(store.spaces.clone())
}

#[tauri::command]
pub async fn browser_create_space(
    name: String,
    color: String,
    icon: String,
) -> AppResult<BrowserSpace> {
    if name.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_NAME",
            "Space name cannot be empty",
        ));
    }
    let space = BrowserSpace {
        id: format!("space-{}", Uuid::new_v4()),
        name: name.trim().to_string(),
        color,
        icon,
        tabs: Vec::new(),
        auto_assign_domains: Vec::new(),
    };

    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    store.spaces.push(space.clone());
    save_to_disk(&store.spaces)?;
    Ok(space)
}

#[tauri::command]
pub async fn browser_delete_space(id: String) -> AppResult<()> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    let before = store.spaces.len();
    store.spaces.retain(|s| s.id != id);
    if store.spaces.len() == before {
        return Err(ImpForgeError::validation(
            "SPACE_NOT_FOUND",
            format!("No space with id '{id}'"),
        ));
    }
    save_to_disk(&store.spaces)?;
    Ok(())
}

#[tauri::command]
pub async fn browser_update_space_domains(
    id: String,
    domains: Vec<String>,
) -> AppResult<BrowserSpace> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    let space = store.spaces.iter_mut().find(|s| s.id == id).ok_or_else(|| {
        ImpForgeError::validation("SPACE_NOT_FOUND", format!("No space with id '{id}'"))
    })?;
    space.auto_assign_domains = domains;
    let result = space.clone();
    save_to_disk(&store.spaces)?;
    Ok(result)
}

// ============================================================================
// TAURI COMMANDS -- TABS
// ============================================================================

#[tauri::command]
pub async fn browser_open_tab(url: String, space_id: Option<String>) -> AppResult<BrowserTab> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;

    // Resolve space: explicit > auto-assign > first space
    let resolved_space_id = space_id
        .or_else(|| find_space_for_domain(&store.spaces, &url))
        .unwrap_or_else(|| {
            store
                .spaces
                .first()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "space-work".to_string())
        });

    let tab = BrowserTab {
        id: format!("tab-{}", Uuid::new_v4()),
        url: url.clone(),
        title: url.clone(),
        space_id: resolved_space_id.clone(),
        pinned: false,
        is_active: true,
        favicon: None,
        last_visited: Utc::now().to_rfc3339(),
    };

    // Deactivate other tabs in the same space, then add
    if let Some(space) = store.spaces.iter_mut().find(|s| s.id == resolved_space_id) {
        for t in &mut space.tabs {
            t.is_active = false;
        }
        space.tabs.push(tab.clone());
    } else {
        return Err(ImpForgeError::validation(
            "SPACE_NOT_FOUND",
            format!("No space with id '{resolved_space_id}'"),
        ));
    }

    save_to_disk(&store.spaces)?;
    Ok(tab)
}

#[tauri::command]
pub async fn browser_close_tab(tab_id: String) -> AppResult<()> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    let mut found = false;
    for space in &mut store.spaces {
        let before = space.tabs.len();
        space.tabs.retain(|t| t.id != tab_id);
        if space.tabs.len() != before {
            found = true;
            // Activate the last tab if we closed the active one
            if !space.tabs.iter().any(|t| t.is_active) {
                if let Some(last) = space.tabs.last_mut() {
                    last.is_active = true;
                }
            }
            break;
        }
    }
    if !found {
        return Err(ImpForgeError::validation(
            "TAB_NOT_FOUND",
            format!("No tab with id '{tab_id}'"),
        ));
    }
    save_to_disk(&store.spaces)?;
    Ok(())
}

#[tauri::command]
pub async fn browser_list_tabs(space_id: Option<String>) -> AppResult<Vec<BrowserTab>> {
    let store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    match space_id {
        Some(sid) => {
            let space = store.spaces.iter().find(|s| s.id == sid).ok_or_else(|| {
                ImpForgeError::validation("SPACE_NOT_FOUND", format!("No space '{sid}'"))
            })?;
            Ok(space.tabs.clone())
        }
        None => {
            let all: Vec<BrowserTab> =
                store.spaces.iter().flat_map(|s| s.tabs.clone()).collect();
            Ok(all)
        }
    }
}

#[tauri::command]
pub async fn browser_pin_tab(tab_id: String, pinned: bool) -> AppResult<()> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    for space in &mut store.spaces {
        if let Some(tab) = space.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.pinned = pinned;
            save_to_disk(&store.spaces)?;
            return Ok(());
        }
    }
    Err(ImpForgeError::validation(
        "TAB_NOT_FOUND",
        format!("No tab with id '{tab_id}'"),
    ))
}

#[tauri::command]
pub async fn browser_activate_tab(tab_id: String) -> AppResult<()> {
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    for space in &mut store.spaces {
        if space.tabs.iter().any(|t| t.id == tab_id) {
            for t in &mut space.tabs {
                t.is_active = t.id == tab_id;
            }
            save_to_disk(&store.spaces)?;
            return Ok(());
        }
    }
    Err(ImpForgeError::validation(
        "TAB_NOT_FOUND",
        format!("No tab with id '{tab_id}'"),
    ))
}

// ============================================================================
// TAURI COMMANDS -- AI FEATURES
// ============================================================================

const DEFAULT_AI_MODEL: &str = "hermes3:latest";

#[tauri::command]
pub async fn browser_ai_summarize(url: String) -> AppResult<AiBrowserResult> {
    let content = fetch_page_content(&url).await?;
    let prompt = format!(
        "Summarize the following web page content in 3-5 concise bullet points. \
         Focus on the key information and main takeaways.\n\n\
         Content:\n{content}"
    );
    let summary = ollama_generate(&prompt, DEFAULT_AI_MODEL).await?;
    Ok(AiBrowserResult {
        url,
        operation: "summarize".into(),
        content: summary,
        model_used: DEFAULT_AI_MODEL.into(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn browser_ai_translate(
    url: String,
    target_language: String,
) -> AppResult<AiBrowserResult> {
    let content = fetch_page_content(&url).await?;
    let prompt = format!(
        "Translate the following web page content into {target_language}. \
         Preserve the original meaning and structure. Only output the translation, \
         no commentary.\n\n\
         Content:\n{content}"
    );
    let translated = ollama_generate(&prompt, DEFAULT_AI_MODEL).await?;
    Ok(AiBrowserResult {
        url,
        operation: format!("translate-{target_language}"),
        content: translated,
        model_used: DEFAULT_AI_MODEL.into(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn browser_ai_web_clip(url: String, format: String) -> AppResult<AiBrowserResult> {
    let content = fetch_page_content(&url).await?;
    let format_instruction = match format.as_str() {
        "markdown" => "Convert the content into clean, well-structured Markdown.",
        "summary" => "Create a concise summary with key facts as bullet points.",
        "outline" => "Extract the document structure as a hierarchical outline.",
        _ => "Convert the content into clean, well-structured Markdown.",
    };
    let prompt = format!(
        "{format_instruction} Strip all ads, navigation, and boilerplate. \
         Only keep the main article content.\n\n\
         Web page content:\n{content}"
    );
    let clipped = ollama_generate(&prompt, DEFAULT_AI_MODEL).await?;
    Ok(AiBrowserResult {
        url,
        operation: format!("clip-{format}"),
        content: clipped,
        model_used: DEFAULT_AI_MODEL.into(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn browser_ai_reader_mode(url: String) -> AppResult<AiBrowserResult> {
    let content = fetch_page_content(&url).await?;
    let prompt = format!(
        "Reformat the following web page into a clean, readable article. \
         Remove all ads, navigation menus, footers, and sidebars. \
         Keep only the main content. Format as clean Markdown with proper \
         headings, paragraphs, and lists. Preserve all important information.\n\n\
         Raw content:\n{content}"
    );
    let reader = ollama_generate(&prompt, DEFAULT_AI_MODEL).await?;
    Ok(AiBrowserResult {
        url,
        operation: "reader-mode".into(),
        content: reader,
        model_used: DEFAULT_AI_MODEL.into(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

// ============================================================================
// TAURI COMMANDS -- LAYOUT PRESETS
// ============================================================================

#[tauri::command]
pub async fn browser_get_presets() -> AppResult<Vec<LayoutPreset>> {
    Ok(built_in_presets())
}

#[tauri::command]
pub async fn browser_apply_preset(preset_id: String) -> AppResult<LayoutPreset> {
    let presets = built_in_presets();
    let preset = presets.iter().find(|p| p.id == preset_id).ok_or_else(|| {
        ImpForgeError::validation(
            "PRESET_NOT_FOUND",
            format!("No layout preset with id '{preset_id}'"),
        )
    })?;
    let mut store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    store.active_preset = preset_id;
    Ok(preset.clone())
}

#[tauri::command]
pub async fn browser_active_preset() -> AppResult<String> {
    let store = SPACE_STORE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_FAILED", format!("Space store lock failed: {e}"))
    })?;
    Ok(store.active_preset.clone())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_spaces_count() {
        let spaces = default_spaces();
        assert_eq!(spaces.len(), 3);
        assert_eq!(spaces[0].name, "Work");
        assert_eq!(spaces[1].name, "Personal");
        assert_eq!(spaces[2].name, "Research");
    }

    #[test]
    fn test_built_in_presets_count() {
        let presets = built_in_presets();
        assert_eq!(presets.len(), 8);
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Developer"));
        assert!(names.contains(&"Zen"));
        assert!(names.contains(&"AI Workspace"));
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<html><head><title>Test</title></head><body><h1>Hello</h1><p>World</p></body></html>";
        let text = strip_html_tags(html);
        assert!(text.contains("Test"));
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<h1>"));
    }

    #[test]
    fn test_strip_html_removes_script_and_style() {
        let html = "<p>Keep</p><script>var x = 1;</script><style>.hidden{}</style><p>Also keep</p>";
        let text = strip_html_tags(html);
        assert!(text.contains("Keep"));
        assert!(text.contains("Also keep"));
        assert!(!text.contains("var x"));
        assert!(!text.contains(".hidden"));
    }

    #[test]
    fn test_find_space_for_domain() {
        let spaces = default_spaces();
        assert_eq!(
            find_space_for_domain(&spaces, "https://github.com/user/repo"),
            Some("space-work".to_string())
        );
        assert_eq!(
            find_space_for_domain(&spaces, "https://arxiv.org/abs/1234"),
            Some("space-research".to_string())
        );
        assert_eq!(
            find_space_for_domain(&spaces, "https://youtube.com/watch"),
            Some("space-personal".to_string())
        );
        assert_eq!(
            find_space_for_domain(&spaces, "https://random-site.example.com"),
            None
        );
    }

    #[test]
    fn test_preset_panels_have_valid_sizes() {
        for preset in built_in_presets() {
            let total: u32 = preset.layout.panels.iter().map(|p| p.size_percent).sum();
            assert!(
                total == 100,
                "Preset '{}' panels sum to {} instead of 100",
                preset.name,
                total
            );
        }
    }

    #[test]
    fn test_check_prefix() {
        let chars: Vec<char> = "hello world".chars().collect();
        assert!(check_prefix(&chars, 0, "hello"));
        assert!(!check_prefix(&chars, 0, "world"));
        assert!(check_prefix(&chars, 6, "world"));
    }

    #[test]
    fn test_browser_space_serialization() {
        let space = BrowserSpace {
            id: "test-id".into(),
            name: "Test".into(),
            color: "#ff0000".into(),
            icon: "Star".into(),
            tabs: vec![BrowserTab {
                id: "tab-1".into(),
                url: "https://example.com".into(),
                title: "Example".into(),
                space_id: "test-id".into(),
                pinned: true,
                is_active: false,
                favicon: None,
                last_visited: "2026-01-01T00:00:00Z".into(),
            }],
            auto_assign_domains: vec!["example.com".into()],
        };
        let json = serde_json::to_string(&space).expect("serialization should work");
        let back: BrowserSpace = serde_json::from_str(&json).expect("deserialization should work");
        assert_eq!(back.name, "Test");
        assert_eq!(back.tabs.len(), 1);
        assert!(back.tabs[0].pinned);
    }
}
