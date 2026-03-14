// SPDX-License-Identifier: Apache-2.0
//! Chat Streaming Module
//!
//! Unified streaming chat that routes to either:
//!   1. **Ollama** (local) — when model starts with "ollama:", OR no API key and Ollama is running
//!   2. **OpenRouter** (cloud) — when an API key is present
//!
//! Both paths stream tokens via the Tauri `Channel<ChatEvent>` mechanism.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use reqwest::Client;
use futures_util::StreamExt;

use crate::error::{AppResult, ImpForgeError};
use crate::forge_memory::context;
use crate::forge_memory::engine::ForgeMemoryEngine;
use crate::ollama;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum ChatEvent {
    Started { model: String, task_type: String },
    Delta { content: String },
    Finished { total_tokens: u32 },
    Error { message: String },
    Routing {
        task_type: String,
        selected_model: String,
        reason: String,
        classification_ms: f64,
    },
}

/// Determine whether a model ID refers to an Ollama model.
fn is_ollama_model(model: &str) -> bool {
    // Explicit prefix
    if model.starts_with("ollama:") {
        return true;
    }
    // Ollama model names are short (e.g. "llama3.2:latest", "qwen2.5-coder:7b")
    // and never contain a "/" (OpenRouter uses "provider/model" format).
    !model.contains('/')
}

#[tauri::command]
pub async fn chat_stream(
    engine: State<'_, ForgeMemoryEngine>,
    message: String,
    model_id: Option<String>,
    system_prompt: Option<String>,
    openrouter_key: Option<String>,
    ollama_url: Option<String>,
    conversation_id: Option<String>,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    let classify_start = std::time::Instant::now();
    let task_type = crate::router::classify_fast(&message);
    let classification_ms = classify_start.elapsed().as_secs_f64() * 1000.0;
    let task_type_str = format!("{:?}", task_type);

    let key = openrouter_key.unwrap_or_default();
    let has_api_key = !key.is_empty();

    // ── Model selection logic ──
    // Priority:
    //   1. Explicit model_id from user
    //   2. If no API key → try Ollama with a sensible default
    //   3. If API key present → use OpenRouter cloud models
    let model = if let Some(ref id) = model_id {
        id.clone()
    } else if !has_api_key {
        // No API key — default to a common Ollama model
        "llama3.2:latest".to_string()
    } else {
        // API key present — use cloud models based on task type
        match task_type {
            crate::router::TaskType::CodeGeneration
            | crate::router::TaskType::DockerfileGen => {
                "mistralai/devstral-small:free".to_string()
            }
            crate::router::TaskType::MultiStepReasoning => {
                "qwen/qwen3-30b-a3b:free".to_string()
            }
            _ => "meta-llama/llama-4-scout:free".to_string(),
        }
    };

    // ── Decide backend: Ollama vs OpenRouter ──
    let use_ollama = if is_ollama_model(&model) {
        // Model looks like an Ollama model — use Ollama
        true
    } else if !has_api_key {
        // No API key and model looks like cloud — try Ollama as fallback
        true
    } else {
        // Has API key and model is a cloud model — use OpenRouter
        false
    };

    // ── ForgeMemory: Enrich system prompt with memory context ──
    let base_system = system_prompt.unwrap_or_else(||
        "You are a helpful AI assistant in ImpForge, an AI Workstation Builder.".to_string()
    );
    let enriched_system = match context::build_context(&engine, &message, 5) {
        Ok(ctx) if !ctx.system_supplement.is_empty() => {
            format!("{}\n\n{}", base_system, ctx.system_supplement)
        }
        _ => base_system,
    };

    let mut messages = Vec::new();
    messages.push(serde_json::json!({"role": "system", "content": enriched_system}));
    messages.push(serde_json::json!({"role": "user", "content": message}));

    if use_ollama {
        // ── Ollama path ──
        let model_display = model.strip_prefix("ollama:").unwrap_or(&model);

        let _ = on_event.send(ChatEvent::Routing {
            task_type: task_type_str.clone(),
            selected_model: model_display.to_string(),
            reason: format!(
                "Local Ollama inference — classified as {} in {:.1}ms",
                task_type.description(),
                classification_ms
            ),
            classification_ms,
        });

        on_event.send(ChatEvent::Started {
            model: model_display.to_string(),
            task_type: task_type_str,
        }).map_err(|e| e.to_string())?;

        // Check Ollama availability first
        if !ollama::is_ollama_available(ollama_url.as_deref()).await {
            let err_msg = if has_api_key {
                "Ollama is not running. Falling back to OpenRouter would require a cloud model ID. \
                 Start Ollama with: ollama serve"
            } else {
                "No API key configured and Ollama is not running. \
                 Either add an OpenRouter API key in Settings, or start Ollama: ollama serve"
            };
            on_event.send(ChatEvent::Error {
                message: err_msg.to_string(),
            }).map_err(|e| e.to_string())?;
            return Ok(());
        }

        match ollama::ollama_chat_stream(
            model_display,
            &messages,
            ollama_url.as_deref(),
            &on_event,
        )
        .await
        {
            Ok((total_tokens, full_response)) => {
                // ForgeMemory auto-learn (fire-and-forget)
                let _ = context::auto_learn(
                    &engine,
                    &message,
                    &full_response,
                    conversation_id.as_deref(),
                );
                on_event
                    .send(ChatEvent::Finished { total_tokens })
                    .map_err(|e| e.to_string())?;
            }
            Err(_) => {
                // Error already sent via on_event inside ollama_chat_stream
            }
        }
    } else {
        // ── OpenRouter path (existing logic) ──
        let _ = on_event.send(ChatEvent::Routing {
            task_type: task_type_str.clone(),
            selected_model: model.clone(),
            reason: format!(
                "Classified as {} in {:.1}ms",
                task_type.description(),
                classification_ms
            ),
            classification_ms,
        });

        on_event.send(ChatEvent::Started {
            model: model.clone(),
            task_type: task_type_str,
        }).map_err(|e| e.to_string())?;

        let client = Client::new();

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", key))
            .header(
                "HTTP-Referer",
                "https://github.com/AiImpDevelopment/impforge-workstation",
            )
            .header("X-Title", "ImpForge AI Workstation")
            .json(&serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": true,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            on_event
                .send(ChatEvent::Error {
                    message: format!("OpenRouter API error {}: {}", status, body),
                })
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut total_tokens: u32 = 0;
        let mut buffer = String::new();
        let mut full_response = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.starts_with("data: [DONE]") {
                    break;
                }
                if line.starts_with("data: ") {
                    let json_str = &line[6..];
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                            if !content.is_empty() {
                                total_tokens += 1;
                                full_response.push_str(content);
                                let _ = on_event.send(ChatEvent::Delta {
                                    content: content.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // ForgeMemory auto-learn (fire-and-forget)
        let _ = context::auto_learn(
            &engine,
            &message,
            &full_response,
            conversation_id.as_deref(),
        );

        on_event
            .send(ChatEvent::Finished { total_tokens })
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
