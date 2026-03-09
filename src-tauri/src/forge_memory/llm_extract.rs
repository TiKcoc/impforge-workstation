// Public API — consumed via forge_memory::commands Tauri layer
#![allow(dead_code)]
//! LLM-powered extraction for ForgeMemory Auto-Learn v2
//!
//! Uses local Ollama to extract structured facts, entities, and summaries
//! from conversation turns. Complements the NLP pipeline for deeper analysis.
//!
//! This is the OPTIONAL second extraction mode — customers can toggle it
//! independently from the NLP pipeline. Requires Ollama running locally.
//!
//! Recommended models (small, fast):
//!   - qwen2.5-coder:1.5b (~300ms on GPU)
//!   - hermes-3:1b (~500ms on CPU)
//!
//! References:
//!   - Mem0 (arXiv:2504.19413) — LLM-based memory extraction
//!   - Memoria (arXiv:2512.12686) — structured memory for agents

use serde::{Deserialize, Serialize};

// ── Result types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub facts: Vec<ExtractedFact>,
    #[serde(default)]
    pub entities: Vec<ExtractedLlmEntity>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub sentiment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: f64,
    #[serde(default)]
    pub category: String,
}

fn default_importance() -> f64 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedLlmEntity {
    pub name: String,
    #[serde(rename = "type", default)]
    pub entity_type: String,
    #[serde(default)]
    pub relations: Vec<String>,
}

// ── Extraction prompt ───────────────────────────────────────────

const EXTRACTION_PROMPT: &str = r#"Extract structured information from this conversation turn.
Return ONLY valid JSON with this exact schema:
{
  "facts": [{"content": "...", "importance": 0.0-1.0, "category": "preference|decision|fact|technical|correction"}],
  "entities": [{"name": "...", "type": "person|tool|language|framework|concept", "relations": ["uses X", "depends on Y"]}],
  "summary": "one-line summary",
  "sentiment": "positive|negative|neutral|mixed"
}

Rules:
- Only extract genuinely important information
- Skip greetings, filler, and small talk
- importance: 0.9+ for explicit instructions, 0.7 for preferences, 0.5 for facts
- If nothing important, return empty arrays and empty strings

User: {user_message}
AI: {ai_response}

JSON:"#;

/// Build the extraction prompt for a conversation turn.
pub fn build_extraction_prompt(user_message: &str, ai_response: &str) -> String {
    let truncated_response = &ai_response[..ai_response.len().min(2000)];
    EXTRACTION_PROMPT
        .replace("{user_message}", user_message)
        .replace("{ai_response}", truncated_response)
}

/// Parse the LLM's JSON response into structured extraction result.
pub fn parse_extraction_response(response: &str) -> Result<ExtractionResult, String> {
    let json_str = extract_json_block(response);
    serde_json::from_str::<ExtractionResult>(&json_str)
        .map_err(|e| format!("Failed to parse LLM extraction: {e}"))
}

/// Call local Ollama for extraction (async).
///
/// Sends the conversation turn to Ollama with a structured extraction prompt.
/// Uses low temperature (0.1) for deterministic output and limits to 512 tokens.
/// Timeout: 10 seconds (returns error if Ollama is slow or unavailable).
pub async fn extract_via_ollama(
    user_message: &str,
    ai_response: &str,
    model: &str,
    ollama_url: &str,
) -> Result<ExtractionResult, String> {
    let prompt = build_extraction_prompt(user_message, ai_response);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .post(format!("{}/api/generate", ollama_url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1,
                "num_predict": 512,
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned status {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let text = body["response"].as_str().unwrap_or("");
    parse_extraction_response(text)
}

/// Check if Ollama is available and the model is loaded.
pub async fn check_ollama_health(ollama_url: &str, model: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(format!("{}/api/tags", ollama_url))
        .send()
        .await
        .map_err(|e| format!("Ollama health check failed: {e}"))?;

    if !response.status().is_success() {
        return Ok(false);
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama tags: {e}"))?;

    let models = body["models"].as_array();
    let has_model = models
        .map(|m| {
            m.iter()
                .any(|v| v["name"].as_str().unwrap_or("").starts_with(model))
        })
        .unwrap_or(false);

    Ok(has_model)
}

/// Extract a JSON block from text that might contain surrounding prose.
fn extract_json_block(text: &str) -> String {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return text[start..=end].to_string();
            }
        }
    }
    text.to_string()
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_extraction_prompt() {
        let prompt = build_extraction_prompt("I prefer Rust", "Great choice!");
        assert!(prompt.contains("I prefer Rust"));
        assert!(prompt.contains("Great choice!"));
        assert!(prompt.contains("facts"));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("JSON:"));
    }

    #[test]
    fn test_parse_extraction_response_valid() {
        let json = r#"{"facts":[{"content":"User prefers Rust","importance":0.8,"category":"preference"}],"entities":[],"summary":"Rust preference","sentiment":"positive"}"#;
        let result = parse_extraction_response(json);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        let extracted = result.unwrap();
        assert_eq!(extracted.facts.len(), 1);
        assert_eq!(extracted.facts[0].category, "preference");
        assert_eq!(extracted.facts[0].importance, 0.8);
        assert_eq!(extracted.summary, "Rust preference");
        assert_eq!(extracted.sentiment, "positive");
    }

    #[test]
    fn test_parse_extraction_response_invalid() {
        let result = parse_extraction_response("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_extraction_response_empty() {
        let json = r#"{"facts":[],"entities":[],"summary":"","sentiment":"neutral"}"#;
        let result = parse_extraction_response(json);
        assert!(result.is_ok());
        let extracted = result.unwrap();
        assert!(extracted.facts.is_empty());
        assert!(extracted.entities.is_empty());
        assert_eq!(extracted.sentiment, "neutral");
    }

    #[test]
    fn test_parse_extraction_with_surrounding_text() {
        let response = r#"Here is the JSON:
{"facts":[{"content":"Port is 8080","importance":0.6,"category":"technical"}],"entities":[],"summary":"Port config","sentiment":"neutral"}
That's the extraction."#;
        let result = parse_extraction_response(response);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        assert_eq!(result.unwrap().facts[0].content, "Port is 8080");
    }

    #[test]
    fn test_parse_extraction_with_entities() {
        let json = r#"{"facts":[],"entities":[{"name":"Rust","type":"language","relations":["used by backend"]}],"summary":"","sentiment":"neutral"}"#;
        let result = parse_extraction_response(json);
        assert!(result.is_ok());
        let extracted = result.unwrap();
        assert_eq!(extracted.entities.len(), 1);
        assert_eq!(extracted.entities[0].name, "Rust");
        assert_eq!(extracted.entities[0].entity_type, "language");
    }

    #[test]
    fn test_extract_json_block() {
        assert_eq!(
            extract_json_block(r#"blah {"a":1} blah"#),
            r#"{"a":1}"#
        );
        assert_eq!(extract_json_block("no json"), "no json");
        assert_eq!(
            extract_json_block(r#"{"nested":{"a":1}}"#),
            r#"{"nested":{"a":1}}"#
        );
    }

    #[test]
    fn test_build_prompt_truncates_long_response() {
        let long_response = "x".repeat(5000);
        let prompt = build_extraction_prompt("test", &long_response);
        // Response should be truncated to 2000 chars in the prompt
        assert!(prompt.len() < 5000);
    }
}
