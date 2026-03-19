// ImpForge LLM Targets
// Defines all available LLM backends and routing logic

use super::{classifier::TaskType, RouterError};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Available LLM targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmTarget {
    /// OpenRouter API (cloud)
    OpenRouter { model: String },

    /// Local Ollama instance
    Ollama { model: String },

    /// Local ONNX model (T5-small for summarization)
    LocalOnnx { model_path: String },

    /// Local Stable Diffusion for image generation
    LocalDiffusion,
}

impl LlmTarget {
    /// Human-readable name for UI
    pub fn display_name(&self) -> String {
        match self {
            Self::OpenRouter { model } => format!("OpenRouter: {}", model),
            Self::Ollama { model } => format!("Ollama: {}", model),
            Self::LocalOnnx { model_path } => format!("Local ONNX: {}", model_path),
            Self::LocalDiffusion => "Local Stable Diffusion".to_string(),
        }
    }

    /// Execute with streaming, emitting tokens via Tauri events
    pub async fn execute_streaming(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        app: &tauri::AppHandle,
        conversation_id: &str,
    ) -> Result<String, RouterError> {
        match self {
            Self::OpenRouter { model } => {
                stream_openrouter(model, system_prompt, user_prompt, app, conversation_id).await
            }
            Self::Ollama { model } => {
                stream_ollama(model, system_prompt, user_prompt, app, conversation_id).await
            }
            _ => {
                // Non-streamable targets: execute normally and emit final result
                let result = self.execute(system_prompt, user_prompt).await?;
                let _ = app.emit("chat-stream", serde_json::json!({
                    "conversation_id": conversation_id,
                    "delta": &result,
                    "done": true,
                    "full_content": &result,
                }));
                Ok(result)
            }
        }
    }

    /// Whether this target is free (no API cost)
    pub fn is_free(&self) -> bool {
        match self {
            Self::OpenRouter { model } => model.ends_with(":free"),
            Self::Ollama { .. } => true,
            Self::LocalOnnx { .. } => true,
            Self::LocalDiffusion => true,
        }
    }

    /// Execute a request to this target
    pub async fn execute(&self, system_prompt: &str, user_prompt: &str) -> Result<String, RouterError> {
        match self {
            Self::OpenRouter { model } => {
                execute_openrouter(model, system_prompt, user_prompt).await
            }
            Self::Ollama { model } => {
                execute_ollama(model, system_prompt, user_prompt).await
            }
            Self::LocalOnnx { model_path } => {
                Err(RouterError::ModelUnavailable { model: format!(
                    "Local ONNX ({model_path}). Use Ollama or OpenRouter instead."
                )})
            }
            Self::LocalDiffusion => {
                Err(RouterError::ModelUnavailable { model:
                    "Local diffusion. Use an external image generation API instead.".to_string()
                })
            }
        }
    }
}

/// Router configuration
#[derive(Debug, Clone, Default)]
pub struct RouterConfig {
    /// OpenRouter API key
    pub openrouter_key: Option<String>,

    /// Whether Ollama is available locally
    pub ollama_available: bool,

    /// Ollama VRAM in GB (for model selection)
    pub ollama_vram_gb: Option<f32>,

    /// Prefer free models (default: true)
    pub prefer_free_models: bool,
}

impl RouterConfig {
    pub fn new() -> Self {
        Self {
            openrouter_key: None,
            ollama_available: false,
            ollama_vram_gb: None,
            prefer_free_models: true,
        }
    }

    pub fn with_openrouter_key(mut self, key: String) -> Self {
        self.openrouter_key = Some(key);
        self
    }

    pub fn with_ollama(mut self, vram_gb: f32) -> Self {
        self.ollama_available = true;
        self.ollama_vram_gb = Some(vram_gb);
        self
    }
}

/// Select the optimal LLM target based on task type and configuration
pub fn select_target(task_type: TaskType, config: &RouterConfig) -> LlmTarget {
    // If prefer_free_models is set and Ollama is available, route locally when possible
    if config.prefer_free_models && config.ollama_available {
        if matches!(task_type, TaskType::SimpleClassification | TaskType::ReadmeSummary) {
            return LlmTarget::Ollama {
                model: "qwen2.5-coder:7b".to_string(),
            };
        }
    }

    match task_type {
        // CODE: Devstral Small (FREE, specialized for code)
        TaskType::CodeGeneration | TaskType::DockerfileGen => {
            if config.openrouter_key.is_some() {
                LlmTarget::OpenRouter {
                    model: "mistralai/devstral-small:free".to_string(),
                }
            } else if config.ollama_available {
                LlmTarget::Ollama {
                    model: "qwen2.5-coder:7b".to_string(),
                }
            } else {
                // Fallback to OpenRouter even without key (will fail gracefully)
                LlmTarget::OpenRouter {
                    model: "mistralai/devstral-small:free".to_string(),
                }
            }
        }

        // CODE EXPLANATION: Same as code generation
        TaskType::CodeExplanation => {
            LlmTarget::OpenRouter {
                model: "mistralai/devstral-small:free".to_string(),
            }
        }

        // N8N WORKFLOWS: Devstral Small (best free option for automation)
        TaskType::N8nWorkflowGen => {
            LlmTarget::OpenRouter {
                model: "mistralai/devstral-small:free".to_string(),
            }
        }

        // README: Local T5-ONNX (no API call)
        TaskType::ReadmeSummary => {
            LlmTarget::LocalOnnx {
                model_path: "models/t5-small-summarizer.onnx".to_string(),
            }
        }

        // GENERAL CHAT: Llama 4 Scout (FREE, 10M context!)
        TaskType::GeneralChat | TaskType::TechQuestion => {
            LlmTarget::OpenRouter {
                model: "meta-llama/llama-4-scout:free".to_string(),
            }
        }

        // RESEARCH: Llama 4 Scout (128k context for long papers)
        TaskType::ResearchDigest => {
            LlmTarget::OpenRouter {
                model: "meta-llama/llama-4-scout:free".to_string(),
            }
        }

        // MULTI-STEP: Qwen3-30B-A3B (FREE, reasoning-optimized)
        TaskType::MultiStepReasoning => {
            LlmTarget::OpenRouter {
                model: "qwen/qwen3-30b-a3b:free".to_string(),
            }
        }

        // WEB AUTOMATION: Devstral for precise instructions
        TaskType::WebAutomation => {
            LlmTarget::OpenRouter {
                model: "mistralai/devstral-small:free".to_string(),
            }
        }

        // IMAGE: Local SD if VRAM >= 8GB, else FLUX free
        TaskType::ImageGeneration => {
            if config.ollama_vram_gb.unwrap_or(0.0) >= 8.0 {
                LlmTarget::LocalDiffusion
            } else {
                LlmTarget::OpenRouter {
                    model: "black-forest-labs/FLUX-1-schnell:free".to_string(),
                }
            }
        }

        // SIMPLE: Local ONNX classifier
        TaskType::SimpleClassification => {
            LlmTarget::LocalOnnx {
                model_path: "models/minilm-classifier.onnx".to_string(),
            }
        }
    }
}

// --- API Execution Functions ---

async fn execute_openrouter(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Get API key from environment (will be passed from Tauri store later)
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| RouterError::MissingApiKey {
            provider: "OpenRouter".to_string()
        })?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://impforge.dev")
        .header("X-Title", "ImpForge AI Workstation Builder")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.as_u16() == 429 {
            return Err(RouterError::RateLimitExceeded {
                model: model.to_string()
            });
        }

        return Err(RouterError::InvalidResponse(format!(
            "Status {}: {}", status, body
        )));
    }

    let json: serde_json::Value = response.json().await?;

    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| RouterError::InvalidResponse("No content in response".to_string()))
}

async fn execute_ollama(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "stream": false
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(RouterError::ModelUnavailable {
            model: model.to_string()
        });
    }

    let json: serde_json::Value = response.json().await?;

    json["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| RouterError::InvalidResponse("No content in Ollama response".to_string()))
}

// --- Streaming Execution Functions ---

async fn stream_openrouter(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app: &tauri::AppHandle,
    conversation_id: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| RouterError::MissingApiKey { provider: "OpenRouter".to_string() })?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://impforge.dev")
        .header("X-Title", "ImpForge AI Workstation Builder")
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 429 {
            return Err(RouterError::RateLimitExceeded { model: model.to_string() });
        }
        return Err(RouterError::InvalidResponse(format!("Status {}: {}", status, body)));
    }

    let mut full_content = String::new();
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(RouterError::RequestFailed)?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    break;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                        full_content.push_str(delta);
                        let _ = app.emit("chat-stream", serde_json::json!({
                            "conversation_id": conversation_id,
                            "delta": delta,
                            "done": false,
                        }));
                    }
                }
            }
        }
    }

    let _ = app.emit("chat-stream", serde_json::json!({
        "conversation_id": conversation_id,
        "delta": "",
        "done": true,
        "full_content": &full_content,
    }));

    Ok(full_content)
}

async fn stream_ollama(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app: &tauri::AppHandle,
    conversation_id: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(RouterError::ModelUnavailable { model: model.to_string() });
    }

    let mut full_content = String::new();
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(RouterError::RequestFailed)?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(content) = json["message"]["content"].as_str() {
                    full_content.push_str(content);
                    let done = json["done"].as_bool().unwrap_or(false);
                    let _ = app.emit("chat-stream", serde_json::json!({
                        "conversation_id": conversation_id,
                        "delta": content,
                        "done": done,
                    }));
                }
            }
        }
    }

    let _ = app.emit("chat-stream", serde_json::json!({
        "conversation_id": conversation_id,
        "delta": "",
        "done": true,
        "full_content": &full_content,
    }));

    Ok(full_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_selection_code() {
        let config = RouterConfig::new().with_openrouter_key("test".to_string());
        let target = select_target(TaskType::CodeGeneration, &config);

        assert!(matches!(target, LlmTarget::OpenRouter { model } if model.contains("devstral")));
    }

    #[test]
    fn test_target_selection_chat() {
        let config = RouterConfig::new().with_openrouter_key("test".to_string());
        let target = select_target(TaskType::GeneralChat, &config);

        assert!(matches!(target, LlmTarget::OpenRouter { model } if model.contains("llama-4")));
    }

    #[test]
    fn test_local_routing_with_ollama() {
        let config = RouterConfig::new()
            .with_openrouter_key("test".to_string())
            .with_ollama(16.0);
        assert!(config.ollama_available);
        assert_eq!(config.ollama_vram_gb, Some(16.0));

        // SimpleClassification routes locally when prefer_free_models + ollama
        let target = select_target(TaskType::SimpleClassification, &config);
        assert!(target.is_free());
    }

    #[test]
    fn test_all_free_targets() {
        let config = RouterConfig::new().with_openrouter_key("test".to_string());

        let task_types = vec![
            TaskType::CodeGeneration,
            TaskType::CodeExplanation,
            TaskType::DockerfileGen,
            TaskType::N8nWorkflowGen,
            TaskType::GeneralChat,
            TaskType::TechQuestion,
            TaskType::ResearchDigest,
            TaskType::MultiStepReasoning,
            TaskType::WebAutomation,
        ];

        for task_type in task_types {
            let target = select_target(task_type.clone(), &config);
            assert!(target.is_free(), "Task {:?} should route to free model", task_type);
        }
    }
}
