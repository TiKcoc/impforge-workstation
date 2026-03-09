//! AI Code Completion — Fill-in-the-Middle (FIM) for inline ghost text
//!
//! Provides Copilot-style inline code completions using:
//! - **Ollama** (local, offline-first) — uses FIM tokens for Qwen2.5-Coder, CodeLlama, etc.
//! - **OpenRouter** (cloud fallback) — uses chat completion with FIM-style prompt
//!
//! Called from the frontend via monacopilot's `requestHandler` callback through Tauri IPC.
//! No HTTP server needed — the Tauri command IS the completion endpoint.

use serde::{Deserialize, Serialize};

/// Request sent from the frontend (monacopilot completionMetadata mapped to this struct)
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionRequest {
    /// Path of the file being edited
    pub file_path: String,
    /// Programming language (e.g. "rust", "typescript", "python")
    pub language: String,
    /// Code text before the cursor (trimmed to last N chars on frontend)
    pub prefix: String,
    /// Code text after the cursor (trimmed to first N chars on frontend)
    pub suffix: String,
    /// Cursor line number (1-based)
    pub line: u32,
    /// Cursor column (1-based)
    pub column: u32,
}

/// Response returned to the frontend
#[derive(Debug, Clone, Serialize)]
pub struct CompletionResponse {
    /// The completion text to insert as ghost text
    pub completion: String,
}

/// Tauri command — AI inline code completion via Ollama FIM or OpenRouter fallback
///
/// Flow:
/// 1. Try Ollama locally (offline-first, zero latency for the user)
/// 2. If Ollama fails, try OpenRouter (if API key is set)
/// 3. Return empty completion on total failure (never error the UI)
#[tauri::command]
pub async fn ai_complete(request: CompletionRequest) -> Result<CompletionResponse, String> {
    // Skip trivially empty requests
    if request.prefix.trim().is_empty() && request.suffix.trim().is_empty() {
        return Ok(CompletionResponse {
            completion: String::new(),
        });
    }

    // 1. Try Ollama (offline-first)
    match ollama_fim_complete(&request).await {
        Ok(text) if !text.is_empty() => {
            return Ok(CompletionResponse { completion: text });
        }
        Ok(_) => { /* empty response, try fallback */ }
        Err(e) => {
            log::debug!("Ollama FIM completion failed: {}", e);
        }
    }

    // 2. Try OpenRouter fallback (if API key available)
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            match openrouter_fim_complete(&request, &key).await {
                Ok(text) if !text.is_empty() => {
                    return Ok(CompletionResponse { completion: text });
                }
                Ok(_) => {}
                Err(e) => {
                    log::debug!("OpenRouter FIM completion failed: {}", e);
                }
            }
        }
    }

    // 3. No completion available — return empty (UI shows nothing, no error)
    Ok(CompletionResponse {
        completion: String::new(),
    })
}

/// Complete code using Ollama's /api/generate endpoint with FIM (Fill-in-the-Middle) tokens.
///
/// Uses the Qwen2.5-Coder FIM format:
///   <|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>
///
/// Also works with CodeLlama, StarCoder, DeepSeek-Coder, and other FIM-capable models.
async fn ollama_fim_complete(request: &CompletionRequest) -> Result<String, String> {
    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Use the configured completion model, or fall back to qwen2.5-coder
    let model = std::env::var("IMPFORGE_COMPLETION_MODEL")
        .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());

    // Build FIM prompt using Qwen2.5-Coder / CodeLlama / StarCoder standard tokens
    let prompt = format!(
        "<|fim_prefix|>{}<|fim_suffix|>{}<|fim_middle|>",
        request.prefix, request.suffix
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(format!("{}/api/generate", ollama_url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.2,
                "top_p": 0.9,
                "num_predict": 128,
                "stop": ["\n\n", "<|fim_pad|>", "<|endoftext|>", "<|fim_prefix|>", "<|fim_suffix|>", "<|fim_middle|>"]
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama connection failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned status {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let text = body["response"].as_str().unwrap_or("").to_string();

    // Clean up: remove any trailing FIM tokens that leaked through
    let cleaned = text
        .trim_end_matches("<|fim_pad|>")
        .trim_end_matches("<|endoftext|>")
        .trim_end_matches("<|fim_prefix|>")
        .trim_end_matches("<|fim_suffix|>")
        .trim_end_matches("<|fim_middle|>")
        .trim_end()
        .to_string();

    Ok(cleaned)
}

/// Complete code using OpenRouter's chat API with a FIM-style system prompt.
///
/// Uses a code-specialized free model (Devstral or Llama) with an instruction
/// that mimics FIM behavior through the chat interface.
async fn openrouter_fim_complete(request: &CompletionRequest, api_key: &str) -> Result<String, String> {
    // Use a free code model on OpenRouter
    let model = std::env::var("IMPFORGE_OPENROUTER_COMPLETION_MODEL")
        .unwrap_or_else(|_| "mistralai/devstral-small:free".to_string());

    let system_prompt = format!(
        "You are a code completion engine. You ONLY output the code that should be inserted at the cursor position. \
         No explanations, no markdown, no comments — just the raw code to insert.\n\
         Language: {}\n\
         File: {}",
        request.language,
        request.file_path.rsplit('/').next().unwrap_or(&request.file_path)
    );

    let user_prompt = format!(
        "Complete the code at the cursor position marked by <CURSOR>. Output ONLY the completion text.\n\n\
         {}<CURSOR>{}",
        // Use limited context to keep the prompt small
        &request.prefix[request.prefix.len().saturating_sub(1500)..],
        &request.suffix[..request.suffix.len().min(500)]
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("HTTP-Referer", "https://impforge.dev")
        .header("X-Title", "ImpForge CodeForge IDE")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.2,
            "max_tokens": 128,
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| format!("OpenRouter connection failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("OpenRouter returned status {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;

    let text = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Strip any markdown code fences the model may have wrapped around
    let cleaned = text
        .trim()
        .strip_prefix("```")
        .and_then(|s| {
            // Remove optional language tag on first line
            let s = s.trim_start();
            if let Some(newline_pos) = s.find('\n') {
                let first_line = &s[..newline_pos];
                if first_line.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '+') {
                    Some(&s[newline_pos + 1..])
                } else {
                    Some(s)
                }
            } else {
                Some(s)
            }
        })
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(&text)
        .trim()
        .to_string();

    Ok(cleaned)
}
