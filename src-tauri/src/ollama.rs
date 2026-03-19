// SPDX-License-Identifier: Apache-2.0
//! Ollama Local Inference Module
//!
//! Provides streaming chat, model listing, and health checks for Ollama.
//! Ollama serves as ImpForge's primary offline inference backend.
//!
//! API Reference: https://github.com/ollama/ollama/blob/main/docs/api.md
//! Response format: NDJSON (one JSON object per line, NOT SSE)

use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures_util::StreamExt;
use tauri::ipc::Channel;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::chat::ChatEvent;
use crate::error::{AppResult, ImpForgeError};
use crate::forge_memory::context;
use crate::forge_memory::engine::ForgeMemoryEngine;

// ── Rate Limiter ─────────────────────────────────────────────────────────
// Prevents request flooding on direct Ollama command calls (max 10 req/sec).

static LAST_OLLAMA_REQUEST_MS: AtomicU64 = AtomicU64::new(0);
const MIN_OLLAMA_INTERVAL_MS: u64 = 100; // 10 requests per second max

fn check_ollama_rate_limit() -> Result<(), ImpForgeError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last = LAST_OLLAMA_REQUEST_MS.load(Ordering::Relaxed);
    if last > 0 && now.saturating_sub(last) < MIN_OLLAMA_INTERVAL_MS {
        return Err(ImpForgeError::validation(
            "RATE_LIMITED",
            "Rate limit: please wait before sending another request",
        ));
    }
    LAST_OLLAMA_REQUEST_MS.store(now, Ordering::Relaxed);
    Ok(())
}

/// Default Ollama base URL
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Timeout for health checks (seconds)
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 3;

/// Timeout for streaming chat requests (seconds) — generous for large models
const CHAT_TIMEOUT_SECS: u64 = 300;

/// Model info returned by Ollama /api/tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub modified_at: String,
}

/// Ollama /api/tags response
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelEntry>,
}

/// Individual model entry from /api/tags
#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    name: String,
    model: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: String,
    #[serde(default)]
    modified_at: String,
}

/// Ollama streaming chat response (one line of NDJSON)
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are part of the Ollama API schema; deserialized but not all read
struct OllamaChatChunk {
    #[serde(default)]
    message: Option<OllamaChatMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Message object within an Ollama chat response
#[derive(Debug, Deserialize)]
struct OllamaChatMessage {
    #[serde(default)]
    content: String,
}

/// Resolve the Ollama base URL from the environment or settings store.
/// Falls back to `http://localhost:11434` when nothing is configured.
fn resolve_ollama_url(ollama_url: Option<&str>) -> String {
    if let Some(url) = ollama_url {
        if !url.is_empty() {
            return url.trim_end_matches('/').to_string();
        }
    }
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Build an HTTP client with appropriate timeouts for the given use-case.
fn health_client() -> Result<Client, ImpForgeError> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
        .build()
        .map_err(|e| ImpForgeError::internal("HTTP_CLIENT_ERROR", format!("Failed to build HTTP client: {e}")))
}

fn chat_client() -> Result<Client, ImpForgeError> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(CHAT_TIMEOUT_SECS))
        .build()
        .map_err(|e| ImpForgeError::internal("HTTP_CLIENT_ERROR", format!("Failed to build HTTP client: {e}")))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Check Ollama availability and return status + model list.
///
/// Returns JSON: `{ "available": bool, "models": [...], "url": "..." }`
#[tauri::command]
pub async fn cmd_ollama_status(
    ollama_url: Option<String>,
) -> AppResult<serde_json::Value> {
    let url = resolve_ollama_url(ollama_url.as_deref());
    let client = health_client()?;

    let resp = match client.get(format!("{url}/api/tags")).send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(serde_json::json!({
                "available": false,
                "models": [],
                "url": url,
                "error": "Connection refused — is Ollama running?"
            }));
        }
    };

    if !resp.status().is_success() {
        return Ok(serde_json::json!({
            "available": false,
            "models": [],
            "url": url,
            "error": format!("Ollama returned HTTP {}", resp.status())
        }));
    }

    let body: OllamaTagsResponse = resp.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse Ollama response: {e}"))
            .with_suggestion("Check that Ollama is running a compatible version (0.1.24+).")
    })?;

    let model_names: Vec<&str> = body.models.iter().map(|m| m.name.as_str()).collect();

    Ok(serde_json::json!({
        "available": true,
        "models": model_names,
        "url": url,
        "model_count": body.models.len()
    }))
}

/// List all models available in Ollama with full metadata.
#[tauri::command]
pub async fn cmd_ollama_models(
    ollama_url: Option<String>,
) -> AppResult<Vec<OllamaModel>> {
    let url = resolve_ollama_url(ollama_url.as_deref());
    let client = health_client()?;

    let resp = client
        .get(format!("{url}/api/tags"))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service("OLLAMA_UNREACHABLE", "Cannot connect to Ollama")
                    .with_details(e.to_string())
                    .with_suggestion("Start Ollama with: ollama serve")
            } else {
                ImpForgeError::from(e)
            }
        })?;

    if !resp.status().is_success() {
        return Err(
            ImpForgeError::service(
                "OLLAMA_HTTP_ERROR",
                format!("Ollama returned HTTP {}", resp.status()),
            )
            .with_suggestion("Check Ollama logs for details. Restart with: ollama serve"),
        );
    }

    let body: OllamaTagsResponse = resp.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse Ollama models: {e}"))
    })?;

    Ok(body
        .models
        .into_iter()
        .map(|m| OllamaModel {
            name: m.name,
            model: m.model,
            size: m.size,
            digest: m.digest,
            modified_at: m.modified_at,
        })
        .collect())
}

/// Stream a chat completion from Ollama via the existing ChatEvent channel.
///
/// This is the core function called by `chat_stream` when routing to Ollama.
/// It handles NDJSON streaming and emits Delta/Finished/Error events.
///
/// Returns `Result<(total_tokens, full_response), ImpForgeError>`.
pub async fn ollama_chat_stream(
    model: &str,
    messages: &[serde_json::Value],
    ollama_url: Option<&str>,
    on_event: &Channel<ChatEvent>,
) -> Result<(u32, String), ImpForgeError> {
    let url = resolve_ollama_url(ollama_url);
    let client = chat_client()?;

    // Strip "ollama:" prefix if present
    let model_name = model.strip_prefix("ollama:").unwrap_or(model);

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": model_name,
            "messages": messages,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|e| {
            let err = if e.is_connect() {
                ImpForgeError::service("OLLAMA_UNREACHABLE", "Cannot connect to Ollama")
                    .with_details(e.to_string())
                    .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "Ollama request timed out")
                    .with_details(e.to_string())
                    .with_suggestion("The model may be loading. Wait a moment and try again.")
            } else {
                ImpForgeError::service("OLLAMA_REQUEST_FAILED", format!("Ollama request failed: {e}"))
            };
            let _ = on_event.send(ChatEvent::Error { message: err.message.clone() });
            err
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "(response body unreadable)".to_string());

        // Parse Ollama error for friendlier messages
        let err = if status.as_u16() == 404 {
            ImpForgeError::model("OLLAMA_MODEL_NOT_FOUND", format!("Model '{}' not found in Ollama", model_name))
                .with_details(body)
                .with_suggestion(format!("Download it first with: ollama pull {}", model_name))
        } else {
            ImpForgeError::service("OLLAMA_HTTP_ERROR", format!("Ollama error {}", status))
                .with_details(body)
                .with_suggestion("Check Ollama logs for details.")
        };

        let _ = on_event.send(ChatEvent::Error {
            message: err.message.clone(),
        });
        return Err(err);
    }

    // Stream NDJSON response
    let mut stream = response.bytes_stream();
    let mut total_tokens: u32 = 0;
    let mut buffer = String::new();
    let mut full_response = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| {
            let err = ImpForgeError::service("OLLAMA_STREAM_ERROR", format!("Stream interrupted: {e}"))
                .with_suggestion("The connection to Ollama was lost. Check if Ollama is still running.");
            let _ = on_event.send(ChatEvent::Error { message: err.message.clone() });
            err
        })?;

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Ollama sends NDJSON — one JSON object per line
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<OllamaChatChunk>(&line) {
                Ok(parsed) => {
                    if let Some(msg) = &parsed.message {
                        if !msg.content.is_empty() {
                            total_tokens += 1;
                            full_response.push_str(&msg.content);
                            let _ = on_event.send(ChatEvent::Delta {
                                content: msg.content.clone(),
                            });
                        }
                    }

                    // Use eval_count from the final message if available
                    if parsed.done {
                        if let Some(eval_count) = parsed.eval_count {
                            total_tokens = eval_count;
                        }
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse Ollama chunk: {e} — line: {line}");
                }
            }
        }
    }

    Ok((total_tokens, full_response))
}

/// Perform a quick Ollama health check (non-blocking, swallows errors).
/// Returns `true` if Ollama is reachable within the timeout.
pub async fn is_ollama_available(ollama_url: Option<&str>) -> bool {
    let url = resolve_ollama_url(ollama_url);
    let client = match health_client() {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get(format!("{url}/api/tags"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Stream chat via Ollama as a full Tauri command (standalone, not via chat.rs).
/// This provides a direct Ollama streaming path for advanced use-cases.
#[tauri::command]
pub async fn cmd_ollama_chat(
    engine: tauri::State<'_, ForgeMemoryEngine>,
    model: String,
    message: String,
    system_prompt: Option<String>,
    ollama_url: Option<String>,
    conversation_id: Option<String>,
    on_event: Channel<ChatEvent>,
) -> AppResult<()> {
    // Input validation & rate limiting
    check_ollama_rate_limit()?;
    if message.trim().is_empty() {
        return Err(ImpForgeError::validation("EMPTY_MESSAGE", "Message cannot be empty").into());
    }
    if message.len() > 200_000 {
        return Err(ImpForgeError::validation(
            "MESSAGE_TOO_LONG",
            "Message exceeds maximum length of 200,000 characters",
        ).into());
    }

    let model_name = model.strip_prefix("ollama:").unwrap_or(&model);

    // Send routing event
    let _ = on_event.send(ChatEvent::Routing {
        task_type: "OllamaLocal".to_string(),
        selected_model: model_name.to_string(),
        reason: "Direct Ollama local inference".to_string(),
        classification_ms: 0.0,
    });

    on_event
        .send(ChatEvent::Started {
            model: model_name.to_string(),
            task_type: "OllamaLocal".to_string(),
        })
        .map_err(|e| ImpForgeError::internal("EVENT_SEND_FAILED", e.to_string()))?;

    // Build messages with optional memory enrichment
    let base_system = system_prompt.unwrap_or_else(|| {
        "You are a helpful AI assistant in ImpForge, an AI Workstation Builder.".to_string()
    });
    let enriched_system = match context::build_context(&engine, &message, 5) {
        Ok(ctx) if !ctx.system_supplement.is_empty() => {
            format!("{}\n\n{}", base_system, ctx.system_supplement)
        }
        _ => base_system,
    };

    let messages = vec![
        serde_json::json!({"role": "system", "content": enriched_system}),
        serde_json::json!({"role": "user", "content": message}),
    ];

    let (total_tokens, full_response) = ollama_chat_stream(
        model_name,
        &messages,
        ollama_url.as_deref(),
        &on_event,
    )
    .await?;

    // ForgeMemory auto-learn (fire-and-forget — must never block chat)
    let _ = context::auto_learn(&engine, &message, &full_response, conversation_id.as_deref());

    on_event
        .send(ChatEvent::Finished { total_tokens })
        .map_err(|e| ImpForgeError::internal("EVENT_SEND_FAILED", e.to_string()))?;

    Ok(())
}
