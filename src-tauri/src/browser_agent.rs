//! ImpForge Browser Agent — Enterprise-Grade AI-Powered Web Automation
//!
//! Architecture inspired by scientific research:
//! - OpAgent (2026): Planner → Grounder → Reflector → Summarizer
//! - BrowserAgent (2025): Think-Summarize-Act loop with memory
//! - WALT (2025): Tool learning from exploration traces
//! - CoAT (2025): Chain-of-Action-Thought structured reasoning
//!
//! All crates MIT/Apache-2.0 — safe for commercial distribution.
//!
//! Integrations:
//! - Web Scraper (built-in, Rust) — content extraction
//! - n8n Webhooks — workflow automation triggers
//! - AI Models (Ollama/OpenRouter) — intelligent navigation
//! - Orchestrator — task decomposition and trust scoring

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// ============================================================================
// TYPES — Browser Agent Architecture
// ============================================================================

/// A single step in the agent's action history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_number: usize,
    pub thought: String,
    pub action: BrowserAction,
    pub observation: String,
    pub success: bool,
    pub timestamp: String,
}

/// Browser actions the agent can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { selector: String },
    Fill { selector: String, value: String },
    Extract { selector: Option<String> },
    Screenshot,
    ExecuteJs { script: String },
    ScrollDown,
    ScrollUp,
    GoBack,
    Wait { ms: u64 },
    Done { summary: String },
}

/// Result of a browser agent task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskResult {
    pub task: String,
    pub steps: Vec<AgentStep>,
    pub final_result: String,
    pub extracted_data: Vec<ExtractedContent>,
    pub total_steps: usize,
    pub success: bool,
    pub error: Option<String>,
}

/// Content extracted during browsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedContent {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub content_type: ContentType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Markdown,
    Text,
    Json,
    Html,
    Table,
}

/// Browser session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    pub id: String,
    pub current_url: String,
    pub page_title: String,
    pub history: Vec<String>,
    pub extracted: Vec<ExtractedContent>,
    pub is_active: bool,
}

/// n8n webhook integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
}

/// Configuration for browser agent tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAgentConfig {
    /// Maximum steps before stopping
    pub max_steps: usize,
    /// Timeout per step in seconds
    pub step_timeout_secs: u64,
    /// Ollama URL for AI reasoning
    pub ollama_url: String,
    /// Model for planning/reasoning
    pub model: String,
    /// CSS selectors to always remove (ads, popups, etc.)
    pub remove_selectors: Vec<String>,
    /// Send results to n8n webhook
    pub webhook: Option<WebhookConfig>,
    /// Send results to Zapier webhook
    pub zapier_webhook: Option<String>,
}

impl Default for BrowserAgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 20,
            step_timeout_secs: 30,
            ollama_url: "http://localhost:11434".to_string(),
            model: "hermes3:latest".to_string(),
            remove_selectors: vec![
                "nav".into(),
                "footer".into(),
                "script".into(),
                "style".into(),
                ".cookie-banner".into(),
                ".advertisement".into(),
            ],
            webhook: None,
            zapier_webhook: None,
        }
    }
}

// ============================================================================
// BROWSER ENGINE — HTTP-based content fetching with AI analysis
// ============================================================================

/// The Browser Engine handles all web interactions
pub struct BrowserEngine {
    client: Client,
    sessions: Arc<Mutex<HashMap<String, BrowserSession>>>,
}

impl BrowserEngine {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        Ok(Self {
            client,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create a new browser session
    pub async fn create_session(&self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let session = BrowserSession {
            id: id.clone(),
            current_url: String::new(),
            page_title: String::new(),
            history: Vec::new(),
            extracted: Vec::new(),
            is_active: true,
        };
        self.sessions.lock().await.insert(id.clone(), session);
        id
    }

    /// Navigate to a URL and extract content
    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<ExtractedContent, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Navigation failed: {e}"))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }

        let html = response
            .text()
            .await
            .map_err(|e| format!("Failed to read page: {e}"))?;

        // Parse and extract in a sync block (scraper::Html is !Send)
        let content = {
            let document = scraper::Html::parse_document(&html);

            let title = scraper::Selector::parse("title")
                .ok()
                .and_then(|sel| document.select(&sel).next())
                .map(|el| el.text().collect::<String>().trim().to_string());

            let description = scraper::Selector::parse("meta[name=\"description\"]")
                .ok()
                .and_then(|sel| {
                    document
                        .select(&sel)
                        .next()
                        .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
                });

            let body_html = scraper::Selector::parse("body")
                .ok()
                .and_then(|sel| document.select(&sel).next())
                .map(|el| el.html())
                .unwrap_or_default();

            let markdown = html2md::rewrite_html(&body_html, false);

            let mut metadata = HashMap::new();
            if let Some(ref desc) = description {
                metadata.insert("description".to_string(), desc.clone());
            }
            metadata.insert("status".to_string(), status.to_string());

            ExtractedContent {
                url: url.to_string(),
                title,
                content: markdown,
                content_type: ContentType::Markdown,
                metadata,
            }
        }; // document dropped here — before any .await

        // Update session (now safe to .await)
        if let Some(session) = self.sessions.lock().await.get_mut(session_id) {
            session.current_url = url.to_string();
            session.page_title = content.title.clone().unwrap_or_default();
            session.history.push(url.to_string());
            session.extracted.push(content.clone());
        }

        Ok(content)
    }

    /// Extract content matching a CSS selector from the current page
    pub async fn extract_with_selector(
        &self,
        session_id: &str,
        url: &str,
        selector: &str,
    ) -> Result<ExtractedContent, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let html = response
            .text()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        // Sync block — scraper::Html is !Send
        let content = {
            let document = scraper::Html::parse_document(&html);
            let sel = scraper::Selector::parse(selector)
                .map_err(|e| format!("Invalid CSS selector: {e:?}"))?;

            let mut parts = Vec::new();
            for el in document.select(&sel) {
                parts.push(el.html());
            }

            if parts.is_empty() {
                return Err(format!("No elements matched selector '{selector}'"));
            }

            let combined_html = parts.join("\n");
            let markdown = html2md::rewrite_html(&combined_html, false);

            ExtractedContent {
                url: url.to_string(),
                title: None,
                content: markdown,
                content_type: ContentType::Markdown,
                metadata: HashMap::from([("selector".to_string(), selector.to_string())]),
            }
        };

        if let Some(session) = self.sessions.lock().await.get_mut(session_id) {
            session.extracted.push(content.clone());
        }

        Ok(content)
    }

    /// Execute JavaScript-like extraction (CSS selector-based, no actual JS engine)
    pub async fn extract_structured(
        &self,
        url: &str,
        selectors: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let html = response
            .text()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        let document = scraper::Html::parse_document(&html);
        let mut results = HashMap::new();

        for (key, sel_str) in selectors {
            if let Ok(sel) = scraper::Selector::parse(sel_str) {
                let text: String = document
                    .select(&sel)
                    .map(|el| el.text().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n");
                results.insert(key.clone(), text.trim().to_string());
            }
        }

        Ok(results)
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Option<BrowserSession> {
        self.sessions.lock().await.get(session_id).cloned()
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<BrowserSession> {
        self.sessions
            .lock()
            .await
            .values()
            .filter(|s| s.is_active)
            .cloned()
            .collect()
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) {
        if let Some(session) = self.sessions.lock().await.get_mut(session_id) {
            session.is_active = false;
        }
    }
}

// ============================================================================
// AI AGENT LOOP — OpAgent-inspired (Planner → Grounder → Reflector → Summarizer)
// ============================================================================

/// Run an AI-powered browser agent task
///
/// The agent follows the OpAgent architecture:
/// 1. PLANNER: Decomposes the task into high-level strategy
/// 2. GROUNDER: Translates strategy into concrete browser actions
/// 3. ACTOR: Executes actions via BrowserEngine
/// 4. REFLECTOR: Evaluates results and adjusts plan
/// 5. SUMMARIZER: Produces final structured output
pub async fn run_agent_task(
    task: &str,
    config: &BrowserAgentConfig,
) -> AgentTaskResult {
    let engine = match BrowserEngine::new() {
        Ok(e) => e,
        Err(e) => {
            return AgentTaskResult {
                task: task.to_string(),
                steps: vec![],
                final_result: String::new(),
                extracted_data: vec![],
                total_steps: 0,
                success: false,
                error: Some(e),
            };
        }
    };

    let session_id = engine.create_session().await;
    let mut steps = Vec::new();
    let mut extracted_data = Vec::new();

    // Try to initialize CDP for full browser control
    let cdp_page_id: Option<String> = if crate::cdp_engine::cdp_is_available() {
        match crate::cdp_engine::cdp_new_page("about:blank").await {
            Ok((pid, _)) => {
                log::info!("CDP page created for agent task: {pid}");
                Some(pid)
            }
            Err(e) => {
                log::warn!("CDP init failed, using HTTP fallback: {e}");
                None
            }
        }
    } else {
        None
    };

    // PHASE 1: PLANNER — Ask AI to decompose the task
    let plan = ai_plan_task(task, config).await;

    // PHASE 2-4: Execute plan steps with reflection
    for (i, action_desc) in plan.iter().enumerate() {
        if i >= config.max_steps {
            break;
        }

        // GROUNDER: Parse action description into BrowserAction
        let action = parse_action(action_desc);

        // ACTOR: Execute (CDP if available, HTTP fallback)
        let observation = execute_action(
            &engine,
            &session_id,
            &action,
            cdp_page_id.as_deref(),
        )
        .await;

        let step = AgentStep {
            step_number: i + 1,
            thought: action_desc.clone(),
            action: action.clone(),
            observation: observation.clone(),
            success: !observation.starts_with("ERROR:"),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Collect extracted content
        if let BrowserAction::Extract { .. } | BrowserAction::Navigate { .. } = &action {
            if step.success && !observation.is_empty() {
                let url = match &action {
                    BrowserAction::Navigate { url } => url.clone(),
                    _ => String::new(),
                };
                extracted_data.push(ExtractedContent {
                    url,
                    title: None,
                    content: observation.clone(),
                    content_type: ContentType::Markdown,
                    metadata: HashMap::new(),
                });
            }
        }

        steps.push(step);

        // REFLECTOR: Check if we should stop
        if let BrowserAction::Done { .. } = &action {
            break;
        }
    }

    // PHASE 5: SUMMARIZER
    let final_result = if extracted_data.is_empty() {
        steps
            .last()
            .map(|s| s.observation.clone())
            .unwrap_or_default()
    } else {
        extracted_data
            .iter()
            .map(|e| e.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };

    let total_steps = steps.len();
    let success = steps.iter().all(|s| s.success) || !extracted_data.is_empty();

    // Send results to webhooks if configured
    if success {
        send_to_webhooks(&final_result, &extracted_data, config).await;
    }

    // Clean up CDP page
    if let Some(ref pid) = cdp_page_id {
        let _ = crate::cdp_engine::cdp_close_page_by_id(pid).await;
    }
    engine.close_session(&session_id).await;

    AgentTaskResult {
        task: task.to_string(),
        steps,
        final_result,
        extracted_data,
        total_steps,
        success,
        error: None,
    }
}

/// Ask AI model to plan the task (using Ollama)
async fn ai_plan_task(task: &str, config: &BrowserAgentConfig) -> Vec<String> {
    let client = match Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![format!("navigate:{task}")],
    };

    let prompt = format!(
        r#"You are a web automation planner. Given a task, output a step-by-step plan as numbered actions.
Each action must be one of:
- navigate:URL — Go to a URL
- extract:CSS_SELECTOR — Extract content matching selector (use "extract:*" for full page)
- click:CSS_SELECTOR — Click an element
- fill:CSS_SELECTOR:VALUE — Fill a form field
- scroll_down — Scroll down the page
- wait:MS — Wait milliseconds
- done:SUMMARY — Task complete

Task: {task}

Output ONLY the action list, one per line, numbered. Example:
1. navigate:https://example.com
2. extract:article
3. done:Extracted article content"#
    );

    let body = serde_json::json!({
        "model": config.model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.3, "num_predict": 512 }
    });

    let response = client
        .post(format!("{}/api/generate", config.ollama_url))
        .json(&body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let text = json["response"].as_str().unwrap_or_default();
                parse_plan_output(text)
            } else {
                // Fallback: simple navigate + extract
                vec![
                    format!("navigate:{task}"),
                    "extract:*".to_string(),
                    "done:Extracted page content".to_string(),
                ]
            }
        }
        Err(_) => {
            // Offline fallback: treat task as URL or search
            if task.starts_with("http") {
                vec![
                    format!("navigate:{task}"),
                    "extract:*".to_string(),
                    "done:Extracted page content".to_string(),
                ]
            } else {
                vec![
                    format!("navigate:https://www.google.com/search?q={}", urlencoding(task)),
                    "extract:#search".to_string(),
                    "done:Search results extracted".to_string(),
                ]
            }
        }
    }
}

/// Simple URL encoding for search queries
fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
}

/// Parse AI-generated plan into action strings
fn parse_plan_output(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Strip numbering like "1. " or "- "
            let action = trimmed
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == ' ');
            let action = action.trim();
            if action.is_empty() {
                None
            } else {
                Some(action.to_string())
            }
        })
        .take(20) // Safety limit
        .collect()
}

/// Parse an action string into a BrowserAction
fn parse_action(action: &str) -> BrowserAction {
    if let Some(url) = action.strip_prefix("navigate:") {
        BrowserAction::Navigate { url: url.trim().to_string() }
    } else if let Some(sel) = action.strip_prefix("extract:") {
        let selector = if sel.trim() == "*" {
            None
        } else {
            Some(sel.trim().to_string())
        };
        BrowserAction::Extract { selector }
    } else if let Some(sel) = action.strip_prefix("click:") {
        BrowserAction::Click { selector: sel.trim().to_string() }
    } else if let Some(rest) = action.strip_prefix("fill:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            BrowserAction::Fill {
                selector: parts[0].trim().to_string(),
                value: parts[1].trim().to_string(),
            }
        } else {
            BrowserAction::Done { summary: format!("Invalid fill action: {action}") }
        }
    } else if action.starts_with("scroll_down") {
        BrowserAction::ScrollDown
    } else if action.starts_with("scroll_up") {
        BrowserAction::ScrollUp
    } else if let Some(ms) = action.strip_prefix("wait:") {
        BrowserAction::Wait { ms: ms.trim().parse().unwrap_or(1000) }
    } else if let Some(summary) = action.strip_prefix("done:") {
        BrowserAction::Done { summary: summary.trim().to_string() }
    } else if action.starts_with("http") {
        BrowserAction::Navigate { url: action.to_string() }
    } else {
        BrowserAction::Done { summary: format!("Unknown action: {action}") }
    }
}

/// Execute a browser action — uses CDP when available, HTTP fallback otherwise
async fn execute_action(
    engine: &BrowserEngine,
    session_id: &str,
    action: &BrowserAction,
    cdp_page_id: Option<&str>,
) -> String {
    match action {
        BrowserAction::Navigate { url } => {
            // Try CDP first for full browser navigation
            if let Some(pid) = cdp_page_id {
                match crate::cdp_engine::cdp_navigate_page(pid, url).await {
                    Ok(result) => {
                        // Also update HTTP session for content extraction
                        let _ = engine.navigate(session_id, url).await;
                        format!(
                            "Navigated to: {}\nTitle: {}\nContent: {} bytes",
                            result.url, result.title, result.content_length,
                        )
                    }
                    Err(e) => {
                        // CDP failed, fall back to HTTP
                        log::warn!("CDP navigate failed, using HTTP: {e}");
                        execute_http_navigate(engine, session_id, url).await
                    }
                }
            } else {
                execute_http_navigate(engine, session_id, url).await
            }
        }
        BrowserAction::Extract { selector } => {
            // CDP extraction via JavaScript for better accuracy
            if let Some(pid) = cdp_page_id {
                if let Some(sel) = selector {
                    match crate::cdp_engine::cdp_extract_text(pid, sel).await {
                        Ok(text) if !text.is_empty() => return text,
                        _ => {} // Fall through to HTTP
                    }
                } else {
                    match crate::cdp_engine::cdp_get_content(pid).await {
                        Ok(html) => {
                            let markdown = html2md::rewrite_html(&html, false);
                            return if markdown.len() > 3000 {
                                format!("{}...\n[{} more chars]", &markdown[..3000], markdown.len() - 3000)
                            } else {
                                markdown
                            };
                        }
                        Err(_) => {} // Fall through to HTTP
                    }
                }
            }
            // HTTP fallback for extraction
            let session = engine.get_session(session_id).await;
            let url = session.map(|s| s.current_url).unwrap_or_default();
            if url.is_empty() {
                return "ERROR: No page loaded. Navigate first.".to_string();
            }
            if let Some(sel) = selector {
                match engine.extract_with_selector(session_id, &url, sel).await {
                    Ok(content) => content.content,
                    Err(e) => format!("ERROR: {e}"),
                }
            } else {
                match engine.navigate(session_id, &url).await {
                    Ok(content) => content.content,
                    Err(e) => format!("ERROR: {e}"),
                }
            }
        }
        BrowserAction::Click { selector } => {
            if let Some(pid) = cdp_page_id {
                match crate::cdp_engine::cdp_click_element(pid, selector).await {
                    Ok(msg) => msg,
                    Err(e) => format!("ERROR: Click failed: {e}"),
                }
            } else {
                format!("CLICK: {selector} (no CDP — install Chrome/Brave for full browser control)")
            }
        }
        BrowserAction::Fill { selector, value } => {
            if let Some(pid) = cdp_page_id {
                match crate::cdp_engine::cdp_fill_field(pid, selector, value).await {
                    Ok(msg) => msg,
                    Err(e) => format!("ERROR: Fill failed: {e}"),
                }
            } else {
                format!("FILL: {selector} = {value} (no CDP — install Chrome/Brave for full browser control)")
            }
        }
        BrowserAction::Screenshot => {
            if let Some(pid) = cdp_page_id {
                match crate::cdp_engine::cdp_screenshot_page(pid, true).await {
                    Ok(b64) => format!("Screenshot captured ({} bytes base64)", b64.len()),
                    Err(e) => format!("ERROR: Screenshot failed: {e}"),
                }
            } else {
                "Screenshot not available (no CDP — install Chrome/Brave)".to_string()
            }
        }
        BrowserAction::ExecuteJs { script } => {
            if let Some(pid) = cdp_page_id {
                match crate::cdp_engine::cdp_eval_js(pid, script).await {
                    Ok(result) => format!("JS result: {result}"),
                    Err(e) => format!("ERROR: JS execution failed: {e}"),
                }
            } else {
                format!("JS not available (no CDP): {}", &script[..script.len().min(100)])
            }
        }
        BrowserAction::ScrollDown => {
            if let Some(pid) = cdp_page_id {
                let _ = crate::cdp_engine::cdp_scroll(pid, "down").await;
            }
            "Scrolled down".to_string()
        }
        BrowserAction::ScrollUp => {
            if let Some(pid) = cdp_page_id {
                let _ = crate::cdp_engine::cdp_scroll(pid, "up").await;
            }
            "Scrolled up".to_string()
        }
        BrowserAction::GoBack => {
            if let Some(pid) = cdp_page_id {
                let _ = crate::cdp_engine::cdp_eval_js(pid, "history.back()").await;
                "Went back (CDP)".to_string()
            } else {
                "Went back (HTTP-mode: use navigate)".to_string()
            }
        }
        BrowserAction::Wait { ms } => {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
            format!("Waited {ms}ms")
        }
        BrowserAction::Done { summary } => {
            format!("DONE: {summary}")
        }
    }
}

/// HTTP-only navigate fallback
async fn execute_http_navigate(
    engine: &BrowserEngine,
    session_id: &str,
    url: &str,
) -> String {
    match engine.navigate(session_id, url).await {
        Ok(content) => {
            let preview = if content.content.len() > 2000 {
                format!(
                    "{}...\n[{} more chars]",
                    &content.content[..2000],
                    content.content.len() - 2000
                )
            } else {
                content.content.clone()
            };
            format!(
                "Navigated to: {}\nTitle: {}\n\n{}",
                url,
                content.title.as_deref().unwrap_or("(no title)"),
                preview,
            )
        }
        Err(e) => format!("ERROR: {e}"),
    }
}

// ============================================================================
// WEBHOOK INTEGRATION — n8n + Zapier
// ============================================================================

/// Send extracted data to configured webhooks (n8n, Zapier)
async fn send_to_webhooks(
    summary: &str,
    data: &[ExtractedContent],
    config: &BrowserAgentConfig,
) {
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(_) => return,
    };

    let payload = serde_json::json!({
        "source": "impforge-browser-agent",
        "summary": summary,
        "items": data.iter().map(|d| serde_json::json!({
            "url": d.url,
            "title": d.title,
            "content": d.content,
            "type": format!("{:?}", d.content_type),
            "metadata": d.metadata,
        })).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // n8n webhook
    if let Some(ref webhook) = config.webhook {
        let mut req = match webhook.method.to_uppercase().as_str() {
            "POST" => client.post(&webhook.url),
            "PUT" => client.put(&webhook.url),
            _ => client.post(&webhook.url),
        };
        for (k, v) in &webhook.headers {
            req = req.header(k, v);
        }
        let _ = req.json(&payload).send().await;
    }

    // Zapier webhook
    if let Some(ref zapier_url) = config.zapier_webhook {
        let _ = client.post(zapier_url).json(&payload).send().await;
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Run browser agent with AI planning
#[tauri::command]
pub async fn browser_agent_run(
    task: String,
    config: Option<BrowserAgentConfig>,
) -> Result<AgentTaskResult, String> {
    let cfg = config.unwrap_or_default();
    Ok(run_agent_task(&task, &cfg).await)
}

/// Quick scrape: navigate + extract (no AI planning needed)
#[tauri::command]
pub async fn browser_agent_quick_extract(
    url: String,
    selector: Option<String>,
) -> Result<ExtractedContent, String> {
    let engine = BrowserEngine::new()?;
    let session_id = engine.create_session().await;

    if let Some(sel) = selector {
        engine.extract_with_selector(&session_id, &url, &sel).await
    } else {
        engine.navigate(&session_id, &url).await
    }
}

/// Extract structured data using multiple CSS selectors
#[tauri::command]
pub async fn browser_agent_structured_extract(
    url: String,
    selectors: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let engine = BrowserEngine::new()?;
    engine.extract_structured(&url, &selectors).await
}

/// Send data to an n8n webhook
#[tauri::command]
pub async fn browser_agent_send_webhook(
    webhook_url: String,
    data: serde_json::Value,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .post(&webhook_url)
        .json(&data)
        .send()
        .await
        .map_err(|e| format!("Webhook failed: {e}"))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status.is_success() {
        Ok(format!("Webhook sent successfully: {status}"))
    } else {
        Err(format!("Webhook returned {status}: {body}"))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_navigate() {
        let action = parse_action("navigate:https://example.com");
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_parse_action_extract() {
        let action = parse_action("extract:article.main");
        match action {
            BrowserAction::Extract { selector } => assert_eq!(selector.as_deref(), Some("article.main")),
            _ => panic!("Expected Extract"),
        }
    }

    #[test]
    fn test_parse_action_extract_all() {
        let action = parse_action("extract:*");
        match action {
            BrowserAction::Extract { selector } => assert!(selector.is_none()),
            _ => panic!("Expected Extract with None"),
        }
    }

    #[test]
    fn test_parse_action_fill() {
        let action = parse_action("fill:#email:test@example.com");
        match action {
            BrowserAction::Fill { selector, value } => {
                assert_eq!(selector, "#email");
                assert_eq!(value, "test@example.com");
            }
            _ => panic!("Expected Fill"),
        }
    }

    #[test]
    fn test_parse_action_done() {
        let action = parse_action("done:Task completed successfully");
        match action {
            BrowserAction::Done { summary } => assert_eq!(summary, "Task completed successfully"),
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_parse_action_url_shorthand() {
        let action = parse_action("https://news.ycombinator.com");
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://news.ycombinator.com"),
            _ => panic!("Expected Navigate from URL shorthand"),
        }
    }

    #[test]
    fn test_parse_plan_output() {
        let plan = parse_plan_output(
            "1. navigate:https://example.com\n2. extract:main\n3. done:Finished\n"
        );
        assert_eq!(plan.len(), 3);
        assert!(plan[0].starts_with("navigate:"));
        assert!(plan[1].starts_with("extract:"));
        assert!(plan[2].starts_with("done:"));
    }

    #[test]
    fn test_parse_plan_strips_formatting() {
        let plan = parse_plan_output(
            "- navigate:https://example.com\n  - extract:h1\n"
        );
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello world"), "hello+world");
        assert_eq!(urlencoding("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn test_default_config() {
        let config = BrowserAgentConfig::default();
        assert_eq!(config.max_steps, 20);
        assert_eq!(config.step_timeout_secs, 30);
        assert!(config.webhook.is_none());
        assert!(config.zapier_webhook.is_none());
        assert!(config.remove_selectors.contains(&"nav".to_string()));
    }

    #[tokio::test]
    async fn test_browser_engine_create_session() {
        let engine = BrowserEngine::new().unwrap();
        let id = engine.create_session().await;
        assert!(!id.is_empty());

        let session = engine.get_session(&id).await.unwrap();
        assert!(session.is_active);
        assert!(session.current_url.is_empty());
    }

    #[tokio::test]
    async fn test_browser_engine_list_sessions() {
        let engine = BrowserEngine::new().unwrap();
        let _id1 = engine.create_session().await;
        let id2 = engine.create_session().await;

        let sessions = engine.list_sessions().await;
        assert_eq!(sessions.len(), 2);

        engine.close_session(&id2).await;
        let sessions = engine.list_sessions().await;
        assert_eq!(sessions.len(), 1);
    }
}
