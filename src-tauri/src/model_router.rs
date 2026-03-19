// SPDX-License-Identifier: Apache-2.0
//! Mini-Model Router -- Smart tiered inference routing
//!
//! Uses the smallest possible model for each task, escalating only when
//! confidence is low. Based on arXiv:2510.03847 (Small Language Models for
//! Agentic Systems Survey).
//!
//! Tier hierarchy:
//!   1 (Nano)   -> qwen3:0.6b     -- classify, extract, yes/no
//!   2 (Small)  -> qwen3:1.7b     -- summarize, translate, simple code
//!   3 (Medium) -> qwen2.5-coder  -- code generation, analysis
//!   4 (Large)  -> qwen3:8b       -- complex reasoning, creative
//!   5 (Cloud)  -> OpenRouter      -- unlimited context, expert tasks

use std::sync::Mutex;
use std::time::Instant;

use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{AppResult, ImpForgeError};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// A single model tier in the routing hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTier {
    pub name: String,
    pub model_id: String,
    pub tier_level: u8,
    pub max_context: u32,
    pub capabilities: Vec<String>,
    pub cost_per_token: f64,
    pub avg_latency_ms: u64,
    pub available: bool,
}

/// Aggregate statistics for the router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterStats {
    pub total_requests: u64,
    pub local_handled: u64,
    pub cloud_fallbacks: u64,
    pub avg_confidence: f32,
    pub cost_saved_usd: f64,
    pub escalations: u64,
}

impl Default for RouterStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            local_handled: 0,
            cloud_fallbacks: 0,
            avg_confidence: 0.0,
            cost_saved_usd: 0.0,
            escalations: 0,
        }
    }
}

/// The result of a task complexity classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    /// Classification, yes/no answers -> 0.6B model
    Trivial,
    /// Summarise, extract, short answers -> 1.7B model
    Simple,
    /// Code generation, translation -> 3-8B model
    Medium,
    /// Multi-step reasoning, debate -> 8B+ or cloud
    Complex,
    /// Research, deep analysis, 100K+ context -> cloud
    Expert,
}

impl TaskComplexity {
    /// Human-readable label for the UI.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trivial => "Trivial",
            Self::Simple => "Simple",
            Self::Medium => "Medium",
            Self::Complex => "Complex",
            Self::Expert => "Expert",
        }
    }

    /// Minimum tier level that can handle this complexity.
    pub fn min_tier(&self) -> u8 {
        match self {
            Self::Trivial => 1,
            Self::Simple => 2,
            Self::Medium => 3,
            Self::Complex => 4,
            Self::Expert => 5,
        }
    }
}

/// Response from a routed inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterResponse {
    pub text: String,
    pub model_used: String,
    pub tier_level: u8,
    pub confidence: f32,
    pub escalated: bool,
    pub latency_ms: u64,
    pub tokens: u32,
    pub complexity: TaskComplexity,
}

/// Ollama `/api/tags` entry for model auto-detection.
#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    name: String,
    #[serde(default)]
    size: u64,
}

/// Ollama `/api/tags` response envelope.
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelEntry>,
}

/// Ollama `/api/generate` response (non-streaming).
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are part of the Ollama API schema; deserialized but not all read
struct OllamaGenerateResponse {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    eval_count: Option<u32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Global state (thread-safe, lazy-initialised)
// ─────────────────────────────────────────────────────────────────────────────

static ROUTER_STATE: Lazy<Mutex<RouterState>> = Lazy::new(|| {
    Mutex::new(RouterState {
        tiers: default_tiers(),
        stats: RouterStats::default(),
        confidence_sum: 0.0,
    })
});

struct RouterState {
    tiers: Vec<ModelTier>,
    stats: RouterStats,
    /// Running sum for incremental average calculation.
    confidence_sum: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Default tier definitions
// ─────────────────────────────────────────────────────────────────────────────

fn default_tiers() -> Vec<ModelTier> {
    vec![
        ModelTier {
            name: "Nano".into(),
            model_id: "qwen3:0.6b".into(),
            tier_level: 1,
            max_context: 4096,
            capabilities: vec![
                "classify".into(),
                "extract".into(),
                "yes_no".into(),
            ],
            cost_per_token: 0.0,
            avg_latency_ms: 50,
            available: false,
        },
        ModelTier {
            name: "Small".into(),
            model_id: "qwen3:1.7b".into(),
            tier_level: 2,
            max_context: 8192,
            capabilities: vec![
                "summarize".into(),
                "translate".into(),
                "simple_code".into(),
            ],
            cost_per_token: 0.0,
            avg_latency_ms: 100,
            available: false,
        },
        ModelTier {
            name: "Medium".into(),
            model_id: "qwen2.5-coder:7b".into(),
            tier_level: 3,
            max_context: 32768,
            capabilities: vec![
                "code".into(),
                "analyze".into(),
                "reason".into(),
            ],
            cost_per_token: 0.0,
            avg_latency_ms: 300,
            available: false,
        },
        ModelTier {
            name: "Large".into(),
            model_id: "qwen3:8b".into(),
            tier_level: 4,
            max_context: 32768,
            capabilities: vec![
                "complex_reason".into(),
                "creative".into(),
                "research".into(),
            ],
            cost_per_token: 0.0,
            avg_latency_ms: 500,
            available: false,
        },
        ModelTier {
            name: "Cloud".into(),
            model_id: "meta-llama/llama-4-scout:free".into(),
            tier_level: 5,
            max_context: 131072,
            capabilities: vec!["everything".into()],
            cost_per_token: 0.0,
            avg_latency_ms: 1000,
            available: true,
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Task classification (heuristic, no LLM required -- <1ms)
// ─────────────────────────────────────────────────────────────────────────────

/// Keyword sets used for fast heuristic classification.
const TRIVIAL_KEYWORDS: &[&str] = &[
    "yes or no", "true or false", "is it", "does it", "classify",
    "label", "categorize", "which one", "pick one", "select",
];

const SIMPLE_KEYWORDS: &[&str] = &[
    "summarize", "summary", "tldr", "tl;dr", "translate",
    "rephrase", "rewrite", "shorten", "simplify", "list",
    "extract", "bullet points", "key points",
];

const MEDIUM_KEYWORDS: &[&str] = &[
    "code", "function", "implement", "write a program", "script",
    "debug", "fix this", "refactor", "sql", "query", "api",
    "class", "struct", "regex", "parse", "convert",
];

const COMPLEX_KEYWORDS: &[&str] = &[
    "explain", "analyze", "compare", "contrast", "evaluate",
    "pros and cons", "trade-off", "design", "architect",
    "step by step", "reasoning", "think through", "debate",
];

const EXPERT_KEYWORDS: &[&str] = &[
    "research", "literature review", "systematic", "comprehensive",
    "in-depth analysis", "white paper", "dissertation", "thesis",
    "peer review", "meta-analysis",
];

/// Classify a prompt into a task complexity tier using fast heuristics.
///
/// Order of evaluation (most specific first):
/// 1. Very short prompts (<15 words) with question marks -> Trivial
/// 2. Keyword matching against known patterns
/// 3. Word count and structural complexity
/// 4. Fallback to Medium (safe default)
fn classify_prompt(prompt: &str) -> TaskComplexity {
    let lower = prompt.to_lowercase();
    let word_count = prompt.split_whitespace().count();

    // Very short prompts are almost always trivial
    if word_count < 10 && (lower.contains('?') || lower.ends_with('?')) {
        // Unless they contain complex keywords
        if !has_keyword(&lower, COMPLEX_KEYWORDS) && !has_keyword(&lower, EXPERT_KEYWORDS) {
            return TaskComplexity::Trivial;
        }
    }

    // Expert-level tasks (highest priority keywords)
    if has_keyword(&lower, EXPERT_KEYWORDS) || word_count > 500 {
        return TaskComplexity::Expert;
    }

    // Complex tasks (check BEFORE medium -- higher priority)
    if has_keyword(&lower, COMPLEX_KEYWORDS) && word_count > 15 {
        return TaskComplexity::Complex;
    }

    // Medium tasks (code-related)
    if has_keyword(&lower, MEDIUM_KEYWORDS) {
        return TaskComplexity::Medium;
    }

    // Simple tasks
    if has_keyword(&lower, SIMPLE_KEYWORDS) {
        return TaskComplexity::Simple;
    }

    // Trivial short prompts
    if has_keyword(&lower, TRIVIAL_KEYWORDS) || word_count < 15 {
        return TaskComplexity::Trivial;
    }

    // Default: Medium (safe middle ground)
    TaskComplexity::Medium
}

/// Check whether any keyword appears in the text.
fn has_keyword(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

// ─────────────────────────────────────────────────────────────────────────────
// Ollama interaction helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the Ollama base URL from environment or default.
fn ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".into())
        .trim_end_matches('/')
        .to_string()
}

/// Build an HTTP client with a generous timeout for generation.
fn make_client(timeout_secs: u64) -> Result<Client, ImpForgeError> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| {
            ImpForgeError::internal(
                "HTTP_CLIENT_ERROR",
                format!("Failed to build HTTP client: {e}"),
            )
        })
}

/// Call Ollama's `/api/generate` endpoint (non-streaming).
async fn ollama_generate(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    max_tokens: Option<u32>,
) -> Result<(String, u32), ImpForgeError> {
    let url = format!("{}/api/generate", ollama_url());
    let client = make_client(300)?;

    let mut body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
    });

    if let Some(sys) = system {
        body["system"] = serde_json::Value::String(sys.to_string());
    }

    if let Some(max) = max_tokens {
        body["options"] = serde_json::json!({ "num_predict": max });
    }

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service("OLLAMA_UNREACHABLE", "Cannot connect to Ollama")
                    .with_details(e.to_string())
                    .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "Ollama request timed out")
                    .with_suggestion("The model may still be loading. Try again in a moment.")
            } else {
                ImpForgeError::service("OLLAMA_REQUEST_FAILED", e.to_string())
            }
        })?;

    if !resp.status().is_success() {
        return Err(ImpForgeError::service(
            "OLLAMA_HTTP_ERROR",
            format!("Ollama returned HTTP {}", resp.status()),
        )
        .with_suggestion("Check that the model is pulled: ollama pull <model>"));
    }

    let gen: OllamaGenerateResponse = resp.json().await.map_err(|e| {
        ImpForgeError::service(
            "OLLAMA_PARSE_ERROR",
            format!("Failed to parse Ollama response: {e}"),
        )
    })?;

    let tokens = gen.eval_count.unwrap_or(0);
    Ok((gen.response, tokens))
}

/// Call OpenRouter's chat completions API (cloud fallback).
async fn openrouter_generate(
    model: &str,
    prompt: &str,
    system: Option<&str>,
    max_tokens: Option<u32>,
) -> Result<(String, u32), ImpForgeError> {
    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err(
            ImpForgeError::config("MISSING_API_KEY", "No OpenRouter API key configured")
                .with_suggestion(
                    "Add your OpenRouter API key in Settings > API Keys to enable cloud fallback.",
                ),
        );
    }

    let client = make_client(120)?;

    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({"role": "user", "content": prompt}));

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
    });
    if let Some(max) = max_tokens {
        body["max_tokens"] = serde_json::Value::Number(max.into());
    }

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("HTTP-Referer", "https://impforge.dev")
        .header("X-Title", "ImpForge AI Workstation")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("OPENROUTER_REQUEST_FAILED", e.to_string())
                .with_suggestion("Check your internet connection and API key.")
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ImpForgeError::service(
            "OPENROUTER_HTTP_ERROR",
            format!("OpenRouter returned HTTP {status}"),
        )
        .with_details(text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        ImpForgeError::service(
            "OPENROUTER_PARSE_ERROR",
            format!("Failed to parse response: {e}"),
        )
    })?;

    let text = json["choices"]
        .get(0)
        .and_then(|c| c["message"]["content"].as_str())
        .unwrap_or("")
        .to_string();

    let tokens = json["usage"]["completion_tokens"]
        .as_u64()
        .unwrap_or(0) as u32;

    Ok((text, tokens))
}

// ─────────────────────────────────────────────────────────────────────────────
// Core routing logic
// ─────────────────────────────────────────────────────────────────────────────

/// Select the best available tier for a given complexity level.
/// Walks up from the minimum tier to find the first available one.
fn select_tier(complexity: &TaskComplexity, tiers: &[ModelTier]) -> Option<ModelTier> {
    let min = complexity.min_tier();

    // First try: exact or higher tier that is available
    let mut sorted: Vec<&ModelTier> = tiers.iter().filter(|t| t.available).collect();
    sorted.sort_by_key(|t| t.tier_level);

    // Find the first available tier at or above the minimum
    if let Some(tier) = sorted.iter().find(|t| t.tier_level >= min) {
        return Some((*tier).clone());
    }

    // Fallback: use any available tier (even if below ideal)
    sorted.first().map(|t| (*t).clone())
}

/// Estimate the confidence of a response based on simple heuristics.
/// Real systems would use perplexity or self-evaluation; this is a fast
/// approximation suitable for routing decisions.
fn estimate_confidence(response: &str, tier_level: u8, complexity: &TaskComplexity) -> f32 {
    let min_tier = complexity.min_tier();
    let mut confidence: f32 = 0.85;

    // Tier match bonus/penalty
    if tier_level >= min_tier + 1 {
        confidence += 0.05; // Overqualified model -> high confidence
    } else if tier_level < min_tier {
        confidence -= 0.15; // Under-qualified -> lower confidence
    }

    // Response length heuristics
    let word_count = response.split_whitespace().count();
    if word_count < 5 {
        confidence -= 0.2; // Very short responses are suspicious
    } else if word_count > 50 {
        confidence += 0.03; // Detailed responses are usually better
    }

    // Error/uncertainty markers
    let lower = response.to_lowercase();
    if lower.contains("i don't know")
        || lower.contains("i'm not sure")
        || lower.contains("i cannot")
        || lower.contains("as an ai")
    {
        confidence -= 0.15;
    }

    confidence.clamp(0.1, 1.0)
}

/// Estimated cost per token for a cloud model (for savings tracking).
const CLOUD_COST_PER_TOKEN: f64 = 0.000001; // $1 per million tokens (conservative)

// ─────────────────────────────────────────────────────────────────────────────
// Model auto-detection
// ─────────────────────────────────────────────────────────────────────────────

/// Known model name patterns and their tier assignments.
const TIER_PATTERNS: &[(&str, u8)] = &[
    // Tier 1 -- Nano (< 1B params)
    ("qwen3:0.6b", 1),
    ("qwen3:0.5b", 1),
    ("tinyllama", 1),
    ("phi-1", 1),
    // Tier 2 -- Small (1-3B params)
    ("qwen3:1.7b", 2),
    ("qwen3:1.8b", 2),
    ("phi-4-mini", 2),
    ("phi4-mini", 2),
    ("gemma:2b", 2),
    ("gemma2:2b", 2),
    ("stablelm-zephyr", 2),
    // Tier 3 -- Medium (3-8B params)
    ("qwen2.5-coder", 3),
    ("codellama", 3),
    ("deepseek-coder", 3),
    ("codegemma", 3),
    ("phi-4", 3),
    ("phi4", 3),
    ("mistral:7b", 3),
    ("gemma:7b", 3),
    ("gemma2:9b", 3),
    // Tier 4 -- Large (8B+ params)
    ("qwen3:8b", 4),
    ("qwen3:14b", 4),
    ("qwen3:32b", 4),
    ("llama3", 4),
    ("llama3.2", 4),
    ("llama3.1", 4),
    ("mixtral", 4),
    ("dolphin3", 4),
    ("hermes-3", 4),
    ("command-r", 4),
];

/// Determine which tier a model name belongs to.
fn detect_tier_for_model(name: &str) -> u8 {
    let lower = name.to_lowercase();
    for (pattern, tier) in TIER_PATTERNS {
        if lower.contains(pattern) {
            return *tier;
        }
    }
    // Default to tier 3 for unknown models (safe middle ground)
    3
}

/// Capabilities list based on tier level.
fn capabilities_for_tier(tier: u8) -> Vec<String> {
    match tier {
        1 => vec!["classify".into(), "extract".into(), "yes_no".into()],
        2 => vec!["summarize".into(), "translate".into(), "simple_code".into()],
        3 => vec!["code".into(), "analyze".into(), "reason".into()],
        4 => vec![
            "complex_reason".into(),
            "creative".into(),
            "research".into(),
        ],
        _ => vec!["everything".into()],
    }
}

/// Context window estimate based on tier level.
fn context_for_tier(tier: u8) -> u32 {
    match tier {
        1 => 4096,
        2 => 8192,
        3 => 32768,
        4 => 32768,
        _ => 131072,
    }
}

/// Latency estimate based on tier level.
fn latency_for_tier(tier: u8) -> u64 {
    match tier {
        1 => 50,
        2 => 100,
        3 => 300,
        4 => 500,
        _ => 1000,
    }
}

/// Tier display name from tier level.
fn name_for_tier(tier: u8) -> &'static str {
    match tier {
        1 => "Nano",
        2 => "Small",
        3 => "Medium",
        4 => "Large",
        _ => "Cloud",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri commands
// ─────────────────────────────────────────────────────────────────────────────

/// Classify a prompt into a task complexity tier (heuristic, <1ms).
#[tauri::command]
pub async fn router_classify_task(prompt: String) -> AppResult<serde_json::Value> {
    let complexity = classify_prompt(&prompt);
    let word_count = prompt.split_whitespace().count();

    Ok(serde_json::json!({
        "complexity": complexity,
        "label": complexity.label(),
        "min_tier": complexity.min_tier(),
        "word_count": word_count,
    }))
}

/// Return the current model tier configuration.
#[tauri::command]
pub async fn router_get_tiers() -> AppResult<Vec<ModelTier>> {
    let state = ROUTER_STATE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
    })?;
    Ok(state.tiers.clone())
}

/// Smart inference routing: classify -> pick smallest model -> escalate if needed.
#[tauri::command]
pub async fn router_infer(
    prompt: String,
    system: Option<String>,
    max_tokens: Option<u32>,
) -> AppResult<RouterResponse> {
    let complexity = classify_prompt(&prompt);

    // Snapshot the tiers (release lock before async work)
    let tiers = {
        let state = ROUTER_STATE.lock().map_err(|e| {
            ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
        })?;
        state.tiers.clone()
    };

    let selected = select_tier(&complexity, &tiers).ok_or_else(|| {
        ImpForgeError::model("NO_MODEL_AVAILABLE", "No model tier is available for inference")
            .with_suggestion(
                "Install at least one Ollama model (ollama pull qwen3:0.6b) or configure an OpenRouter API key.",
            )
    })?;

    let start = Instant::now();
    let is_cloud = selected.tier_level >= 5;

    let (text, tokens) = if is_cloud {
        openrouter_generate(
            &selected.model_id,
            &prompt,
            system.as_deref(),
            max_tokens,
        )
        .await?
    } else {
        ollama_generate(
            &selected.model_id,
            &prompt,
            system.as_deref(),
            max_tokens,
        )
        .await?
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    let confidence = estimate_confidence(&text, selected.tier_level, &complexity);
    let escalated = selected.tier_level > complexity.min_tier();

    // Update global stats
    {
        let mut state = ROUTER_STATE.lock().map_err(|e| {
            ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
        })?;
        state.stats.total_requests += 1;
        if is_cloud {
            state.stats.cloud_fallbacks += 1;
        } else {
            state.stats.local_handled += 1;
            // Estimate savings vs always using cloud
            state.stats.cost_saved_usd += tokens as f64 * CLOUD_COST_PER_TOKEN;
        }
        if escalated {
            state.stats.escalations += 1;
        }
        state.confidence_sum += confidence as f64;
        state.stats.avg_confidence = if state.stats.total_requests > 0 {
            (state.confidence_sum / state.stats.total_requests as f64) as f32
        } else {
            0.0
        };
    }

    Ok(RouterResponse {
        text,
        model_used: selected.model_id,
        tier_level: selected.tier_level,
        confidence,
        escalated,
        latency_ms,
        tokens,
        complexity,
    })
}

/// Return aggregated routing statistics.
#[tauri::command]
pub async fn router_get_stats() -> AppResult<RouterStats> {
    let state = ROUTER_STATE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
    })?;
    Ok(state.stats.clone())
}

/// Update the tier configuration (user customisation).
#[tauri::command]
pub async fn router_configure_tiers(tiers: Vec<ModelTier>) -> AppResult<()> {
    if tiers.is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_TIERS",
            "At least one model tier must be configured",
        ));
    }

    // Validate tier levels are in 1..=5 range
    for tier in &tiers {
        if tier.tier_level == 0 || tier.tier_level > 5 {
            return Err(ImpForgeError::validation(
                "INVALID_TIER_LEVEL",
                format!(
                    "Tier level must be 1-5, got {} for '{}'",
                    tier.tier_level, tier.name
                ),
            ));
        }
    }

    let mut state = ROUTER_STATE.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
    })?;
    state.tiers = tiers;
    Ok(())
}

/// Auto-detect available Ollama models and build tier assignments.
///
/// Queries Ollama `/api/tags`, matches model names to tier patterns,
/// and merges with the existing configuration (preserving cloud tier).
#[tauri::command]
pub async fn router_detect_models() -> AppResult<Vec<ModelTier>> {
    let url = format!("{}/api/tags", ollama_url());
    let client = make_client(5)?;

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            // Ollama not reachable -- return defaults with nothing available
            return Ok(default_tiers());
        }
    };

    if !resp.status().is_success() {
        return Ok(default_tiers());
    }

    let tags: OllamaTagsResponse = resp.json().await.unwrap_or(OllamaTagsResponse {
        models: Vec::new(),
    });

    // Build a map of tier_level -> best model for that tier
    let mut tier_map: std::collections::HashMap<u8, ModelTier> = std::collections::HashMap::new();

    for entry in &tags.models {
        let tier_level = detect_tier_for_model(&entry.name);
        let tier = ModelTier {
            name: name_for_tier(tier_level).into(),
            model_id: entry.name.clone(),
            tier_level,
            max_context: context_for_tier(tier_level),
            capabilities: capabilities_for_tier(tier_level),
            cost_per_token: 0.0,
            avg_latency_ms: latency_for_tier(tier_level),
            available: true,
        };

        // Prefer larger models within the same tier (more capable)
        let existing_size = tier_map
            .get(&tier_level)
            .map(|_| 0u64) // placeholder
            .unwrap_or(0);
        if entry.size > existing_size || !tier_map.contains_key(&tier_level) {
            tier_map.insert(tier_level, tier);
        }
    }

    // Merge with defaults: keep defaults for missing tiers, mark available
    let mut result = Vec::new();
    for default_tier in default_tiers() {
        if let Some(detected) = tier_map.remove(&default_tier.tier_level) {
            result.push(detected);
        } else {
            result.push(default_tier);
        }
    }

    // Add any extra tiers from Ollama that don't match defaults
    for (_, extra) in tier_map {
        result.push(extra);
    }

    result.sort_by_key(|t| t.tier_level);

    // Save to global state
    {
        let mut state = ROUTER_STATE.lock().map_err(|e| {
            ImpForgeError::internal("LOCK_ERROR", format!("Router state lock poisoned: {e}"))
        })?;
        state.tiers = result.clone();
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_trivial_question() {
        assert_eq!(classify_prompt("Is the sky blue?"), TaskComplexity::Trivial);
        assert_eq!(classify_prompt("yes or no: is Rust fast?"), TaskComplexity::Trivial);
    }

    #[test]
    fn test_classify_simple() {
        assert_eq!(
            classify_prompt("Summarize this article about climate change"),
            TaskComplexity::Simple
        );
        assert_eq!(
            classify_prompt("Translate this sentence to French"),
            TaskComplexity::Simple
        );
    }

    #[test]
    fn test_classify_medium() {
        assert_eq!(
            classify_prompt("Write a function to sort a list in Rust"),
            TaskComplexity::Medium
        );
        assert_eq!(
            classify_prompt("Implement a binary search tree in Python"),
            TaskComplexity::Medium
        );
    }

    #[test]
    fn test_classify_complex() {
        let prompt = "Explain the trade-offs between different database architectures for a high-throughput real-time analytics system. Compare columnar vs row-oriented storage.";
        assert_eq!(classify_prompt(prompt), TaskComplexity::Complex);
    }

    #[test]
    fn test_classify_expert() {
        assert_eq!(
            classify_prompt("Write a comprehensive literature review on transformer architectures"),
            TaskComplexity::Expert
        );
    }

    #[test]
    fn test_classify_short_prompt_no_question() {
        // Short without question mark or keywords -> Trivial
        assert_eq!(classify_prompt("Hello world"), TaskComplexity::Trivial);
    }

    #[test]
    fn test_default_tiers() {
        let tiers = default_tiers();
        assert_eq!(tiers.len(), 5);
        assert_eq!(tiers[0].tier_level, 1);
        assert_eq!(tiers[4].tier_level, 5);
        assert!(tiers[4].available); // Cloud is always available
        assert!(!tiers[0].available); // Local models need detection
    }

    #[test]
    fn test_select_tier_cloud_only() {
        let tiers = default_tiers(); // Only cloud is available
        let selected = select_tier(&TaskComplexity::Trivial, &tiers);
        assert!(selected.is_some());
        assert_eq!(selected.as_ref().map(|t| t.tier_level), Some(5));
    }

    #[test]
    fn test_select_tier_with_local() {
        let mut tiers = default_tiers();
        tiers[0].available = true; // Make Nano available
        let selected = select_tier(&TaskComplexity::Trivial, &tiers);
        assert_eq!(selected.as_ref().map(|t| t.tier_level), Some(1));
    }

    #[test]
    fn test_select_tier_needs_escalation() {
        let mut tiers = default_tiers();
        tiers[2].available = true; // Only Medium available
        // Trivial needs tier 1, but only tier 3 is available
        let selected = select_tier(&TaskComplexity::Trivial, &tiers);
        assert_eq!(selected.as_ref().map(|t| t.tier_level), Some(3));
    }

    #[test]
    fn test_estimate_confidence_good() {
        let conf = estimate_confidence(
            "The answer is 42, which represents the meaning of life.",
            3,
            &TaskComplexity::Medium,
        );
        assert!(conf > 0.8);
    }

    #[test]
    fn test_estimate_confidence_uncertain() {
        let conf = estimate_confidence(
            "I'm not sure about this topic.",
            1,
            &TaskComplexity::Complex,
        );
        assert!(conf < 0.75);
    }

    #[test]
    fn test_estimate_confidence_very_short() {
        let conf = estimate_confidence("ok", 1, &TaskComplexity::Medium);
        assert!(conf < 0.7);
    }

    #[test]
    fn test_detect_tier_for_known_models() {
        assert_eq!(detect_tier_for_model("qwen3:0.6b"), 1);
        assert_eq!(detect_tier_for_model("qwen3:1.7b"), 2);
        assert_eq!(detect_tier_for_model("qwen2.5-coder:7b-instruct"), 3);
        assert_eq!(detect_tier_for_model("qwen3:8b"), 4);
        assert_eq!(detect_tier_for_model("llama3.2:8b"), 4);
        assert_eq!(detect_tier_for_model("dolphin3:8b"), 4);
    }

    #[test]
    fn test_detect_tier_for_unknown_model() {
        assert_eq!(detect_tier_for_model("some-random-model:latest"), 3);
    }

    #[test]
    fn test_has_keyword() {
        assert!(has_keyword("please summarize this", SIMPLE_KEYWORDS));
        assert!(!has_keyword("hello world", SIMPLE_KEYWORDS));
    }

    #[test]
    fn test_complexity_labels() {
        assert_eq!(TaskComplexity::Trivial.label(), "Trivial");
        assert_eq!(TaskComplexity::Expert.label(), "Expert");
    }

    #[test]
    fn test_complexity_min_tiers() {
        assert_eq!(TaskComplexity::Trivial.min_tier(), 1);
        assert_eq!(TaskComplexity::Simple.min_tier(), 2);
        assert_eq!(TaskComplexity::Medium.min_tier(), 3);
        assert_eq!(TaskComplexity::Complex.min_tier(), 4);
        assert_eq!(TaskComplexity::Expert.min_tier(), 5);
    }

    #[test]
    fn test_router_stats_default() {
        let stats = RouterStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.local_handled, 0);
        assert_eq!(stats.cloud_fallbacks, 0);
        assert_eq!(stats.cost_saved_usd, 0.0);
    }

    #[test]
    fn test_capabilities_for_tier() {
        let caps = capabilities_for_tier(1);
        assert!(caps.contains(&"classify".to_string()));
        let caps3 = capabilities_for_tier(3);
        assert!(caps3.contains(&"code".to_string()));
    }

    #[test]
    fn test_no_available_tiers() {
        let tiers: Vec<ModelTier> = default_tiers()
            .into_iter()
            .map(|mut t| {
                t.available = false;
                t
            })
            .collect();
        assert!(select_tier(&TaskComplexity::Trivial, &tiers).is_none());
    }
}
