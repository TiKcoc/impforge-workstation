//! AI Provider Implementations
//!
//! Unified interface for all supported AI providers

use super::{AiError, ChatMessage, ChatRequest, ChatResponse, Provider, get_api_key};
use serde_json::json;

/// Execute chat completion with any provider
pub async fn chat_completion(request: ChatRequest) -> Result<ChatResponse, AiError> {
    match request.provider {
        Provider::OpenAI => openai_chat(request).await,
        Provider::Anthropic => anthropic_chat(request).await,
        Provider::Mistral => mistral_chat(request).await,
        Provider::Groq => groq_chat(request).await,
        Provider::OpenRouter => openrouter_chat(request).await,
        Provider::Ollama => ollama_chat(request).await,
        Provider::Google => google_chat(request).await,
        Provider::Cohere => cohere_chat(request).await,
        Provider::XAI => xai_chat(request).await,
        Provider::DeepSeek => deepseek_chat(request).await,
        Provider::Together => together_chat(request).await,
        Provider::Fireworks => fireworks_chat(request).await,
        Provider::LlamaCpp | Provider::Candle => {
            Err(AiError::ProviderUnavailable("Use inference module for local models".to_string()))
        }
        Provider::FastEmbed => {
            Err(AiError::ProviderUnavailable("FastEmbed is for embeddings only".to_string()))
        }
    }
}

// ============================================================================
// OpenAI
// ============================================================================

async fn openai_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::OpenAI)
        .ok_or_else(|| AiError::MissingApiKey("OpenAI".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::OpenAI.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::OpenAI, &model).await
}

// ============================================================================
// Anthropic
// ============================================================================

async fn anthropic_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Anthropic)
        .ok_or_else(|| AiError::MissingApiKey("Anthropic".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Anthropic.default_model().to_string());

    // Convert messages to Anthropic format (separate system from messages)
    let (system, messages): (Vec<_>, Vec<_>) = request.messages
        .into_iter()
        .partition(|m| m.role == "system");

    let system_prompt = system.first().map(|m| m.content.clone());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": request.max_tokens.unwrap_or(4096),
    });

    if let Some(sys) = system_prompt {
        body["system"] = json!(sys);
    }

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AiError::InvalidResponse(e.to_string()))?;

    let content = json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(ChatResponse {
        provider: Provider::Anthropic,
        model,
        content,
        tokens_used: json["usage"]["output_tokens"].as_u64().map(|t| t as u32),
        finish_reason: json["stop_reason"].as_str().map(|s| s.to_string()),
    })
}

// ============================================================================
// Mistral
// ============================================================================

async fn mistral_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Mistral)
        .ok_or_else(|| AiError::MissingApiKey("Mistral".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Mistral.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.mistral.ai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::Mistral, &model).await
}

// ============================================================================
// Groq
// ============================================================================

async fn groq_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Groq)
        .ok_or_else(|| AiError::MissingApiKey("Groq".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Groq.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::Groq, &model).await
}

// ============================================================================
// OpenRouter
// ============================================================================

async fn openrouter_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::OpenRouter)
        .ok_or_else(|| AiError::MissingApiKey("OpenRouter".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::OpenRouter.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("HTTP-Referer", "https://impforge.dev")
        .header("X-Title", "ImpForge AI Workstation")
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::OpenRouter, &model).await
}

// ============================================================================
// Ollama (Local)
// ============================================================================

async fn ollama_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let model = request.model.unwrap_or_else(|| Provider::Ollama.default_model().to_string());
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post(format!("{}/api/chat", ollama_url))
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7),
            }
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AiError::InvalidResponse(e.to_string()))?;

    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(ChatResponse {
        provider: Provider::Ollama,
        model,
        content,
        tokens_used: json["eval_count"].as_u64().map(|t| t as u32),
        finish_reason: Some("stop".to_string()),
    })
}

// ============================================================================
// Google (Gemini)
// ============================================================================

async fn google_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Google)
        .ok_or_else(|| AiError::MissingApiKey("Google".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Google.default_model().to_string());

    // Convert to Gemini format
    let contents: Vec<_> = request.messages.iter().map(|m| {
        json!({
            "role": if m.role == "assistant" { "model" } else { "user" },
            "parts": [{"text": m.content}]
        })
    }).collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        ))
        .header("Content-Type", "application/json")
        .json(&json!({
            "contents": contents,
            "generationConfig": {
                "temperature": request.temperature.unwrap_or(0.7),
                "maxOutputTokens": request.max_tokens.unwrap_or(4096),
            }
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AiError::InvalidResponse(e.to_string()))?;

    let content = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(ChatResponse {
        provider: Provider::Google,
        model,
        content,
        tokens_used: json["usageMetadata"]["totalTokenCount"].as_u64().map(|t| t as u32),
        finish_reason: json["candidates"][0]["finishReason"].as_str().map(|s| s.to_string()),
    })
}

// ============================================================================
// Cohere
// ============================================================================

async fn cohere_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Cohere)
        .ok_or_else(|| AiError::MissingApiKey("Cohere".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Cohere.default_model().to_string());

    // Get last user message
    let message = request.messages.last()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Convert history
    let chat_history: Vec<_> = request.messages[..request.messages.len().saturating_sub(1)]
        .iter()
        .map(|m| json!({
            "role": if m.role == "user" { "USER" } else { "CHATBOT" },
            "message": m.content
        }))
        .collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.cohere.ai/v1/chat")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "message": message,
            "chat_history": chat_history,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AiError::InvalidResponse(e.to_string()))?;

    Ok(ChatResponse {
        provider: Provider::Cohere,
        model,
        content: json["text"].as_str().unwrap_or("").to_string(),
        tokens_used: json["meta"]["tokens"]["output_tokens"].as_u64().map(|t| t as u32),
        finish_reason: json["finish_reason"].as_str().map(|s| s.to_string()),
    })
}

// ============================================================================
// xAI (Grok)
// ============================================================================

async fn xai_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::XAI)
        .ok_or_else(|| AiError::MissingApiKey("xAI".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::XAI.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.x.ai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::XAI, &model).await
}

// ============================================================================
// DeepSeek
// ============================================================================

async fn deepseek_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::DeepSeek)
        .ok_or_else(|| AiError::MissingApiKey("DeepSeek".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::DeepSeek.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.deepseek.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::DeepSeek, &model).await
}

// ============================================================================
// Together AI
// ============================================================================

async fn together_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Together)
        .ok_or_else(|| AiError::MissingApiKey("Together".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Together.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.together.xyz/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::Together, &model).await
}

// ============================================================================
// Fireworks AI
// ============================================================================

async fn fireworks_chat(request: ChatRequest) -> Result<ChatResponse, AiError> {
    let api_key = get_api_key(Provider::Fireworks)
        .ok_or_else(|| AiError::MissingApiKey("Fireworks".to_string()))?;

    let model = request.model.unwrap_or_else(|| Provider::Fireworks.default_model().to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .post("https://api.fireworks.ai/inference/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        }))
        .send()
        .await
        .map_err(|e| AiError::RequestFailed(e.to_string()))?;

    parse_openai_response(response, Provider::Fireworks, &model).await
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn parse_openai_response(
    response: reqwest::Response,
    provider: Provider,
    model: &str,
) -> Result<ChatResponse, AiError> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.as_u16() == 429 {
            return Err(AiError::RateLimited(format!("{}: {}", provider as i32, body)));
        }

        return Err(AiError::RequestFailed(format!("Status {}: {}", status, body)));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AiError::InvalidResponse(e.to_string()))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(ChatResponse {
        provider,
        model: model.to_string(),
        content,
        tokens_used: json["usage"]["total_tokens"].as_u64().map(|t| t as u32),
        finish_reason: json["choices"][0]["finish_reason"].as_str().map(|s| s.to_string()),
    })
}
