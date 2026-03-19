//! ForgeAI -- Cursor-like AI coding assistant features for CodeForge IDE.
//!
//! Provides high-level AI actions that operate on code selections or entire files:
//!
//! - **ForgeComplete**: Inline code completions with configurable delay and multiline support.
//! - **ForgeEdit**: Instruction-driven code edits ("Make this function async") with unified diff.
//! - **ForgeExplain**: Natural-language explanation of code snippets.
//! - **ForgeFix**: Error-aware code repair given a compiler/lint error message.
//! - **ForgeTest**: Unit test generation for a given code snippet.
//!
//! All features route through Ollama (offline-first) with an optional OpenRouter cloud fallback.
//! The Ollama URL and model names are configurable via environment variables so the user can
//! swap models without rebuilding.
//!
//! References:
//! - Cursor AI Feature Set (2024): inline edit, explain, fix, test generation
//! - JetBrains AI Assistant (2024): context-aware refactoring, test scaffolding

use crate::error::{AppResult, ImpForgeError};
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Configuration for the ForgeAI subsystem.
/// Stored in the user's `~/.impforge/settings.json` and loaded at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeAiConfig {
    /// Enable inline code completions (ghost text).
    pub complete_enabled: bool,
    /// Debounce delay before triggering completion (milliseconds).
    pub complete_delay_ms: u32,
    /// Allow multi-line completions (vs single-line only).
    pub complete_multiline: bool,
    /// Enable instruction-driven code edits.
    pub edit_enabled: bool,
    /// Enable the autonomous agent mode (file-level refactoring).
    pub agent_enabled: bool,
    /// Let the agent auto-run tool calls without confirmation.
    pub agent_auto_run: bool,
    /// Default Ollama model name for AI features (e.g. "qwen2.5-coder:7b").
    pub default_model: String,
}

impl Default for ForgeAiConfig {
    fn default() -> Self {
        Self {
            complete_enabled: true,
            complete_delay_ms: 300,
            complete_multiline: true,
            edit_enabled: true,
            agent_enabled: true,
            agent_auto_run: false,
            default_model: "qwen2.5-coder:7b".to_string(),
        }
    }
}

// ============================================================================
// TYPES
// ============================================================================

/// A single completion suggestion returned to the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    /// The text to insert.
    pub text: String,
    /// Line range where this completion should be inserted (start_line, end_line).
    pub insert_range: (u32, u32),
    /// Model confidence score (0.0 -- 1.0).
    pub confidence: f32,
}

/// Result of an AI-driven code edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    /// The modified code after applying the instruction.
    pub new_code: String,
    /// Unified diff between old and new code.
    pub diff: String,
    /// Natural-language explanation of what was changed and why.
    pub explanation: String,
}

// ============================================================================
// HELPERS
// ============================================================================

/// Resolve the Ollama base URL from the environment (default: http://localhost:11434).
fn ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("IMPFORGE_OLLAMA_URL"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
}

/// Resolve the model to use for chat-style AI features.
fn resolve_model() -> String {
    std::env::var("IMPFORGE_AI_MODEL")
        .or_else(|_| std::env::var("IMPFORGE_COMPLETION_MODEL"))
        .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string())
}

/// Send a chat-style prompt to Ollama and return the assistant response text.
///
/// Uses the `/api/chat` endpoint with `stream: false` so we get a single JSON
/// response. Returns an `ImpForgeError` on network or parse failure.
async fn ollama_chat(
    system: &str,
    user: &str,
    model: &str,
    max_tokens: u32,
) -> AppResult<String> {
    let base = ollama_url();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            ImpForgeError::service("HTTP_CLIENT", format!("Failed to create HTTP client: {e}"))
        })?;

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user",   "content": user },
        ],
        "stream": false,
        "options": {
            "temperature": 0.2,
            "top_p": 0.9,
            "num_predict": max_tokens,
        }
    });

    let response = client
        .post(format!("{base}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("OLLAMA_UNREACHABLE", format!("Cannot reach Ollama: {e}"))
                .with_suggestion("Start Ollama with: ollama serve")
        })?;

    if !response.status().is_success() {
        return Err(ImpForgeError::service(
            "OLLAMA_ERROR",
            format!("Ollama returned HTTP {}", response.status()),
        ));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE", format!("Failed to parse Ollama response: {e}"))
    })?;

    let text = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(text)
}

/// Strip markdown code fences from model output (```lang ... ```).
fn strip_fences(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(after) = trimmed.strip_prefix("```") {
        // Skip optional language tag on the first line
        let content = match after.find('\n') {
            Some(nl) => {
                let first_line = &after[..nl];
                if first_line
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '+' || c == '#')
                {
                    &after[nl + 1..]
                } else {
                    after
                }
            }
            None => after,
        };
        content
            .strip_suffix("```")
            .unwrap_or(content)
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build a minimal unified diff between two code strings.
///
/// This produces a human-readable diff without pulling in a full diff crate.
/// Lines prefixed with `-` are removed, `+` are added, ` ` are context.
fn simple_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut diff = String::from("--- original\n+++ modified\n");
    let max_len = old_lines.len().max(new_lines.len());

    for i in 0..max_len {
        match (old_lines.get(i), new_lines.get(i)) {
            (Some(o), Some(n)) if *o == *n => {
                diff.push_str(&format!(" {o}\n"));
            }
            (Some(o), Some(n)) => {
                diff.push_str(&format!("-{o}\n"));
                diff.push_str(&format!("+{n}\n"));
            }
            (Some(o), None) => {
                diff.push_str(&format!("-{o}\n"));
            }
            (None, Some(n)) => {
                diff.push_str(&format!("+{n}\n"));
            }
            (None, None) => {}
        }
    }

    diff
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Generate inline code completions for the editor.
///
/// Sends the surrounding code context (prefix text up to cursor, language, position)
/// to Ollama's FIM endpoint and returns a ranked list of completion suggestions.
#[tauri::command]
pub async fn forge_ai_complete(
    code: String,
    language: String,
    cursor_line: u32,
    cursor_col: u32,
    max_tokens: Option<u32>,
) -> AppResult<Vec<Completion>> {
    let start = Instant::now();
    let model = resolve_model();
    let tokens = max_tokens.unwrap_or(128);

    let system = format!(
        "You are a code completion engine for {language}. \
         Output ONLY the code to insert at the cursor. \
         No explanations, no markdown fences, no comments. \
         Just raw code."
    );

    // Trim context to a reasonable window around the cursor
    let lines: Vec<&str> = code.lines().collect();
    let ctx_start = cursor_line.saturating_sub(30) as usize;
    let ctx_end = (cursor_line as usize + 10).min(lines.len());
    let context_window = lines
        .get(ctx_start..ctx_end)
        .map(|s| s.join("\n"))
        .unwrap_or_default();

    let user = format!(
        "Complete the code at line {cursor_line}, column {cursor_col}.\n\n{context_window}"
    );

    let text = ollama_chat(&system, &user, &model, tokens).await?;
    let cleaned = strip_fences(&text);

    if cleaned.is_empty() {
        return Ok(vec![]);
    }

    let line_count = cleaned.lines().count().max(1) as u32;
    let latency_ms = start.elapsed().as_millis() as f32;
    // Heuristic confidence: shorter latency and non-trivial output => higher confidence
    let confidence = (1.0 - (latency_ms / 10_000.0).min(0.8)).max(0.1);

    Ok(vec![Completion {
        text: cleaned,
        insert_range: (cursor_line, cursor_line + line_count - 1),
        confidence,
    }])
}

/// Apply a natural-language instruction to edit code.
///
/// Example instructions:
/// - "Make this function async"
/// - "Add error handling with Result"
/// - "Convert to iterator pattern"
///
/// Returns the modified code, a unified diff, and an explanation.
#[tauri::command]
pub async fn forge_ai_edit(
    code: String,
    instruction: String,
    language: String,
    selection: Option<(u32, u32)>,
) -> AppResult<EditResult> {
    let model = resolve_model();

    let selection_hint = match selection {
        Some((start, end)) => format!(" Focus on lines {start} to {end}."),
        None => String::new(),
    };

    let system = format!(
        "You are a code editor for {language}. \
         Apply the user's instruction to the code. \
         Return ONLY the complete modified code. \
         No explanations, no markdown fences."
    );

    let user = format!(
        "Instruction: {instruction}{selection_hint}\n\nCode:\n{code}"
    );

    let new_raw = ollama_chat(&system, &user, &model, 2048).await?;
    let new_code = strip_fences(&new_raw);

    if new_code.is_empty() {
        return Err(ImpForgeError::model(
            "EMPTY_EDIT",
            "AI returned an empty edit result",
        ));
    }

    // Generate diff
    let diff = simple_diff(&code, &new_code);

    // Generate a brief explanation
    let explain_system = format!(
        "You are a code reviewer for {language}. \
         Explain in 1-2 sentences what changed and why. Be concise."
    );
    let explain_user = format!(
        "Original instruction: {instruction}\n\nDiff:\n{diff}"
    );
    let explanation = ollama_chat(&explain_system, &explain_user, &model, 200)
        .await
        .unwrap_or_else(|_| "Code was modified according to the instruction.".to_string());

    Ok(EditResult {
        new_code,
        diff,
        explanation,
    })
}

/// Explain what a code snippet does in natural language.
///
/// Returns a concise explanation suitable for display in a hover tooltip or
/// side panel. Covers purpose, algorithm, and notable patterns.
#[tauri::command]
pub async fn forge_ai_explain(
    code: String,
    language: String,
) -> AppResult<String> {
    let model = resolve_model();

    let system = format!(
        "You are a {language} expert. Explain the following code clearly and concisely. \
         Cover: what it does, how it works, and any important patterns or edge cases. \
         Keep it under 200 words."
    );

    let explanation = ollama_chat(&system, &code, &model, 512).await?;

    if explanation.trim().is_empty() {
        return Err(ImpForgeError::model(
            "EMPTY_EXPLAIN",
            "AI returned an empty explanation",
        ));
    }

    Ok(explanation)
}

/// Fix a code error given the code and the error message.
///
/// The AI analyzes the error, identifies the root cause, and returns
/// the corrected code with a diff and explanation of the fix.
#[tauri::command]
pub async fn forge_ai_fix(
    code: String,
    error_message: String,
    language: String,
) -> AppResult<EditResult> {
    let model = resolve_model();

    let system = format!(
        "You are a {language} debugger. Fix the error in the code below. \
         Return ONLY the complete fixed code. \
         No explanations, no markdown fences."
    );

    let user = format!(
        "Error message:\n{error_message}\n\nCode with error:\n{code}"
    );

    let new_raw = ollama_chat(&system, &user, &model, 2048).await?;
    let new_code = strip_fences(&new_raw);

    if new_code.is_empty() {
        return Err(ImpForgeError::model(
            "EMPTY_FIX",
            "AI returned an empty fix result",
        ));
    }

    let diff = simple_diff(&code, &new_code);

    let explain_system = format!(
        "You are a {language} debugger. \
         Explain in 1-2 sentences what the bug was and how it was fixed. Be concise."
    );
    let explain_user = format!(
        "Error: {error_message}\n\nDiff:\n{diff}"
    );
    let explanation = ollama_chat(&explain_system, &explain_user, &model, 200)
        .await
        .unwrap_or_else(|_| "The error was fixed.".to_string());

    Ok(EditResult {
        new_code,
        diff,
        explanation,
    })
}

/// Generate unit tests for a code snippet.
///
/// The AI produces idiomatic test code for the given language, covering
/// happy paths, edge cases, and error conditions where applicable.
#[tauri::command]
pub async fn forge_ai_test(
    code: String,
    language: String,
) -> AppResult<String> {
    let model = resolve_model();

    let test_framework = match language.as_str() {
        "rust" => "Use #[test] and assert! / assert_eq! macros.",
        "typescript" | "javascript" => "Use describe/it/expect (vitest or jest).",
        "python" => "Use pytest with def test_ functions.",
        "go" => "Use testing.T with t.Run subtests.",
        "java" | "kotlin" => "Use JUnit 5 with @Test annotations.",
        "csharp" => "Use xUnit with [Fact] and [Theory] attributes.",
        _ => "Use the standard test framework for this language.",
    };

    let system = format!(
        "You are a {language} test engineer. Generate comprehensive unit tests for \
         the code below. {test_framework} \
         Cover: happy path, edge cases, and error conditions. \
         Return ONLY the test code. No explanations."
    );

    let tests = ollama_chat(&system, &code, &model, 2048).await?;
    let cleaned = strip_fences(&tests);

    if cleaned.trim().is_empty() {
        return Err(ImpForgeError::model(
            "EMPTY_TESTS",
            "AI returned empty test output",
        ));
    }

    Ok(cleaned)
}

/// Get the current ForgeAI configuration.
///
/// Reads from `~/.impforge/forge_ai.json`. Returns defaults if the file
/// does not exist yet.
#[tauri::command]
pub fn forge_ai_get_config() -> AppResult<ForgeAiConfig> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".impforge")
        .join("forge_ai.json");

    if config_path.exists() {
        let data = std::fs::read_to_string(&config_path).map_err(|e| {
            ImpForgeError::filesystem(
                "CONFIG_READ",
                format!("Failed to read ForgeAI config: {e}"),
            )
        })?;
        let config: ForgeAiConfig = serde_json::from_str(&data).map_err(|e| {
            ImpForgeError::config("CONFIG_PARSE", format!("Invalid ForgeAI config JSON: {e}"))
        })?;
        Ok(config)
    } else {
        Ok(ForgeAiConfig::default())
    }
}

/// Save the ForgeAI configuration to disk.
#[tauri::command]
pub fn forge_ai_save_config(config: ForgeAiConfig) -> AppResult<()> {
    let config_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".impforge");

    std::fs::create_dir_all(&config_dir).map_err(|e| {
        ImpForgeError::filesystem("CONFIG_DIR", format!("Failed to create config dir: {e}"))
    })?;

    let json = serde_json::to_string_pretty(&config).map_err(|e| {
        ImpForgeError::internal("CONFIG_SERIALIZE", format!("Failed to serialize config: {e}"))
    })?;

    std::fs::write(config_dir.join("forge_ai.json"), json).map_err(|e| {
        ImpForgeError::filesystem("CONFIG_WRITE", format!("Failed to write config: {e}"))
    })?;

    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = ForgeAiConfig::default();
        assert!(cfg.complete_enabled);
        assert_eq!(cfg.complete_delay_ms, 300);
        assert!(cfg.complete_multiline);
        assert!(cfg.edit_enabled);
        assert!(cfg.agent_enabled);
        assert!(!cfg.agent_auto_run);
        assert_eq!(cfg.default_model, "qwen2.5-coder:7b");
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = ForgeAiConfig {
            complete_enabled: false,
            complete_delay_ms: 500,
            complete_multiline: false,
            edit_enabled: true,
            agent_enabled: false,
            agent_auto_run: true,
            default_model: "codellama:13b".to_string(),
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let restored: ForgeAiConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(!restored.complete_enabled);
        assert_eq!(restored.complete_delay_ms, 500);
        assert_eq!(restored.default_model, "codellama:13b");
    }

    #[test]
    fn test_strip_fences_rust() {
        let input = "```rust\nfn main() {\n    println!(\"hi\");\n}\n```";
        let output = strip_fences(input);
        assert_eq!(output, "fn main() {\n    println!(\"hi\");\n}");
    }

    #[test]
    fn test_strip_fences_plain() {
        let input = "let x = 42;";
        assert_eq!(strip_fences(input), "let x = 42;");
    }

    #[test]
    fn test_strip_fences_no_lang_tag() {
        let input = "```\nsome code\n```";
        // Empty lang tag => entire "some code" is the content
        let output = strip_fences(input);
        assert_eq!(output, "some code");
    }

    #[test]
    fn test_simple_diff_identical() {
        let diff = simple_diff("line1\nline2", "line1\nline2");
        assert!(diff.contains(" line1"));
        assert!(diff.contains(" line2"));
        // Skip header lines and verify all content lines are context (space prefix)
        let content_lines: Vec<&str> = diff
            .lines()
            .filter(|l| !l.starts_with("---") && !l.starts_with("+++") && !l.starts_with("@@") && !l.is_empty())
            .collect();
        assert!(!content_lines.is_empty());
        assert!(content_lines.iter().all(|l| l.starts_with(' ')));
    }

    #[test]
    fn test_simple_diff_modification() {
        let diff = simple_diff("let x = 1;", "let x = 2;");
        assert!(diff.contains("-let x = 1;"));
        assert!(diff.contains("+let x = 2;"));
    }

    #[test]
    fn test_simple_diff_addition() {
        let diff = simple_diff("line1", "line1\nline2");
        assert!(diff.contains(" line1"));
        assert!(diff.contains("+line2"));
    }

    #[test]
    fn test_simple_diff_deletion() {
        let diff = simple_diff("line1\nline2", "line1");
        assert!(diff.contains(" line1"));
        assert!(diff.contains("-line2"));
    }

    #[test]
    fn test_completion_struct() {
        let c = Completion {
            text: "println!(\"hello\");".to_string(),
            insert_range: (5, 5),
            confidence: 0.85,
        };
        let json = serde_json::to_string(&c).expect("serialize");
        assert!(json.contains("println!"));
        assert!(json.contains("0.85"));
    }

    #[test]
    fn test_edit_result_struct() {
        let er = EditResult {
            new_code: "async fn foo() {}".to_string(),
            diff: "+async fn foo() {}".to_string(),
            explanation: "Made function async".to_string(),
        };
        let json = serde_json::to_string(&er).expect("serialize");
        assert!(json.contains("async fn foo"));
    }

    #[test]
    fn test_resolve_model_default() {
        // Without env vars set, should return the default
        // (Can't unset env vars in a test safely, so just verify the function doesn't panic)
        let model = resolve_model();
        assert!(!model.is_empty());
    }

    #[test]
    fn test_ollama_url_default() {
        let url = ollama_url();
        // Should be non-empty and a valid URL-ish string
        assert!(url.starts_with("http"));
    }
}
