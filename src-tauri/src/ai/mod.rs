//! NEXUS AI Provider System
//!
//! Unified interface for 20+ AI providers including:
//! - Cloud: OpenAI, Anthropic, Mistral, Groq, Google, Cohere, xAI
//! - Local: Ollama, llama.cpp, Candle
//! - Embeddings: FastEmbed, OpenAI, Cohere
//! - Vector DBs: Qdrant, LanceDB

pub mod providers;
pub mod embeddings;
pub mod routing;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AI Provider errors
#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider not available: {0}")]
    ProviderUnavailable(String),

    #[error("API key missing for: {0}")]
    MissingApiKey(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Embedding failed: {0}")]
    EmbeddingFailed(String),
}

/// Supported AI providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    // Cloud Providers
    OpenAI,
    Anthropic,
    Mistral,
    Groq,
    Google,
    Cohere,
    XAI,
    DeepSeek,
    Together,
    Fireworks,
    OpenRouter,

    // Local Providers
    Ollama,
    LlamaCpp,
    Candle,

    // Embedding Providers
    FastEmbed,
}

impl Provider {
    /// Check if provider requires API key
    pub fn requires_api_key(&self) -> bool {
        matches!(
            self,
            Provider::OpenAI
                | Provider::Anthropic
                | Provider::Mistral
                | Provider::Groq
                | Provider::Google
                | Provider::Cohere
                | Provider::XAI
                | Provider::DeepSeek
                | Provider::Together
                | Provider::Fireworks
                | Provider::OpenRouter
        )
    }

    /// Check if provider is local (no internet needed)
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            Provider::Ollama | Provider::LlamaCpp | Provider::Candle | Provider::FastEmbed
        )
    }

    /// Get environment variable name for API key
    pub fn api_key_env_var(&self) -> Option<&'static str> {
        match self {
            Provider::OpenAI => Some("OPENAI_API_KEY"),
            Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
            Provider::Mistral => Some("MISTRAL_API_KEY"),
            Provider::Groq => Some("GROQ_API_KEY"),
            Provider::Google => Some("GOOGLE_API_KEY"),
            Provider::Cohere => Some("COHERE_API_KEY"),
            Provider::XAI => Some("XAI_API_KEY"),
            Provider::DeepSeek => Some("DEEPSEEK_API_KEY"),
            Provider::Together => Some("TOGETHER_API_KEY"),
            Provider::Fireworks => Some("FIREWORKS_API_KEY"),
            Provider::OpenRouter => Some("OPENROUTER_API_KEY"),
            _ => None,
        }
    }

    /// Get default model for provider
    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::OpenAI => "gpt-4o-mini",
            Provider::Anthropic => "claude-3-5-sonnet-20241022",
            Provider::Mistral => "mistral-small-latest",
            Provider::Groq => "llama-3.3-70b-versatile",
            Provider::Google => "gemini-1.5-flash",
            Provider::Cohere => "command-r-plus",
            Provider::XAI => "grok-2",
            Provider::DeepSeek => "deepseek-chat",
            Provider::Together => "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            Provider::Fireworks => "accounts/fireworks/models/llama-v3p3-70b-instruct",
            Provider::OpenRouter => "meta-llama/llama-4-scout:free",
            Provider::Ollama => "llama3.2:latest",
            Provider::LlamaCpp => "default",
            Provider::Candle => "microsoft/phi-3-mini-4k-instruct",
            Provider::FastEmbed => "BAAI/bge-small-en-v1.5",
        }
    }

    /// Check if provider supports streaming
    pub fn supports_streaming(&self) -> bool {
        !matches!(self, Provider::FastEmbed)
    }

    /// Check if provider supports function calling
    pub fn supports_function_calling(&self) -> bool {
        matches!(
            self,
            Provider::OpenAI
                | Provider::Anthropic
                | Provider::Mistral
                | Provider::Groq
                | Provider::Google
                | Provider::Cohere
                | Provider::Together
        )
    }
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Chat completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub provider: Provider,
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub provider: Provider,
    pub model: String,
    pub content: String,
    pub tokens_used: Option<u32>,
    pub finish_reason: Option<String>,
}

/// Embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub provider: Provider,
    pub model: Option<String>,
    pub texts: Vec<String>,
}

/// Embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub provider: Provider,
    pub model: String,
    pub embeddings: Vec<Vec<f32>>,
    pub dimensions: usize,
}

/// Get API key for provider from environment
pub fn get_api_key(provider: Provider) -> Option<String> {
    provider
        .api_key_env_var()
        .and_then(|var| std::env::var(var).ok())
}

/// Check if provider is available (has API key or is local)
pub fn is_provider_available(provider: Provider) -> bool {
    if provider.is_local() {
        true
    } else {
        get_api_key(provider).is_some()
    }
}

/// List all available providers
pub fn list_available_providers() -> Vec<Provider> {
    vec![
        Provider::OpenAI,
        Provider::Anthropic,
        Provider::Mistral,
        Provider::Groq,
        Provider::Google,
        Provider::Cohere,
        Provider::XAI,
        Provider::DeepSeek,
        Provider::Together,
        Provider::Fireworks,
        Provider::OpenRouter,
        Provider::Ollama,
        Provider::LlamaCpp,
        Provider::Candle,
        Provider::FastEmbed,
    ]
    .into_iter()
    .filter(|p| is_provider_available(*p))
    .collect()
}
