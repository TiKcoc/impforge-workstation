//! GGUF Model Loading and Inference
//!
//! Uses llama-cpp-2 for GGUF model inference

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{InferenceError, InferenceResult, LocalModelConfig};

/// GGUF model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufConfig {
    /// Path to the GGUF model file
    pub model_path: PathBuf,
    /// Number of GPU layers to offload (-1 = all, 0 = CPU)
    pub n_gpu_layers: i32,
    /// Context size (default: 4096)
    pub n_ctx: u32,
    /// Number of threads (default: auto)
    pub n_threads: Option<u32>,
    /// Enable Flash Attention
    pub flash_attn: bool,
    /// Temperature for sampling
    pub temperature: f32,
    /// Top-p sampling
    pub top_p: f32,
    /// Repetition penalty
    pub repeat_penalty: f32,
}

impl Default for GgufConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            n_gpu_layers: -1,
            n_ctx: 4096,
            n_threads: None,
            flash_attn: true,
            temperature: 0.7,
            top_p: 0.95,
            repeat_penalty: 1.1,
        }
    }
}

/// GGUF Model wrapper
pub struct GgufModel {
    config: GgufConfig,
    // Note: llama-cpp-2 model handle would be stored here
    // For now, we use a placeholder since llama-cpp-2 requires
    // llama.cpp to be built with the correct backend
}

impl GgufModel {
    /// Load a GGUF model
    pub fn load(config: GgufConfig) -> Result<Self, InferenceError> {
        if !config.model_path.exists() {
            return Err(InferenceError::ModelNotFound(
                config.model_path.to_string_lossy().to_string()
            ));
        }

        // Verify GGUF format
        let extension = config.model_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if extension != "gguf" {
            return Err(InferenceError::InvalidFormat(
                format!("Expected .gguf file, got .{}", extension)
            ));
        }

        log::info!("Loading GGUF model: {:?}", config.model_path);
        log::info!("GPU layers: {}, Context: {}", config.n_gpu_layers, config.n_ctx);

        // Validate GPU availability when GPU layers requested
        if config.n_gpu_layers != 0 && !super::check_gpu_available() {
            log::warn!("GPU layers requested but no GPU available — falling back to CPU");
            if config.n_gpu_layers > 0 {
                return Err(InferenceError::GpuUnavailable);
            }
        }

        // TODO: Initialize llama-cpp-2 model
        // This requires llama.cpp to be built with ROCm/CUDA support
        // For now, return a placeholder

        Ok(Self { config })
    }

    /// Generate text completion
    pub async fn generate(
        &self,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<InferenceResult, InferenceError> {
        log::info!("Generating with GGUF model, max_tokens: {}", max_tokens);

        // TODO: Implement actual inference with llama-cpp-2
        // For now, return an error indicating inference is not yet wired
        if prompt.is_empty() {
            return Err(InferenceError::InferenceFailed("Empty prompt".to_string()));
        }

        Ok(InferenceResult {
            text: format!("[GGUF placeholder] Response to: {}", &prompt[..prompt.len().min(50)]),
            tokens_generated: 0,
            tokens_per_second: 0.0,
            model_id: self.config.model_path.to_string_lossy().to_string(),
        })
    }

    /// Get model path
    pub fn model_path(&self) -> &PathBuf {
        &self.config.model_path
    }

    /// Get configuration
    pub fn config(&self) -> &GgufConfig {
        &self.config
    }
}

/// Convert LocalModelConfig to GgufConfig
impl From<LocalModelConfig> for GgufConfig {
    fn from(config: LocalModelConfig) -> Self {
        Self {
            model_path: PathBuf::from(&config.model_id),
            n_gpu_layers: config.n_gpu_layers,
            n_ctx: config.context_size,
            flash_attn: config.flash_attention,
            ..Default::default()
        }
    }
}

/// Tauri command: Load a GGUF model
#[tauri::command]
pub async fn cmd_load_gguf(
    model_path: String,
    n_gpu_layers: Option<i32>,
    n_ctx: Option<u32>,
) -> Result<String, String> {
    let config = GgufConfig {
        model_path: PathBuf::from(&model_path),
        n_gpu_layers: n_gpu_layers.unwrap_or(-1),
        n_ctx: n_ctx.unwrap_or(4096),
        ..Default::default()
    };

    let model = GgufModel::load(config)
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "Model loaded: {} (ctx={})",
        model.model_path().display(),
        model.config().n_ctx,
    ))
}

/// Tauri command: Generate with GGUF model
#[tauri::command]
pub async fn cmd_generate_gguf(
    model_path: String,
    prompt: String,
    max_tokens: Option<u32>,
) -> Result<InferenceResult, String> {
    let config = GgufConfig {
        model_path: PathBuf::from(&model_path),
        ..Default::default()
    };

    let model = GgufModel::load(config)
        .map_err(|e| e.to_string())?;

    model.generate(&prompt, max_tokens.unwrap_or(256))
        .await
        .map_err(|e| e.to_string())
}
