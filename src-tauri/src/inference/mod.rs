//! ImpForge Local Inference Module
//!
//! Provides local LLM inference using:
//! - llama.cpp for GGUF models
//! - Candle for safetensors/HuggingFace models
//! - HuggingFace Hub for model downloads
//!
//! Feature-gated optional upgrades:
//! - `local-inference`: Enhanced Candle engine (GPU detection, embedding)
//! - `rig-router`: Multi-provider LLM routing via Rig framework
//! - `fsrs-brain`: FSRS-5 spaced repetition scheduler

pub mod hub;
pub mod gguf;

#[cfg(feature = "local-inference")]
pub mod candle_engine;

#[cfg(feature = "rig-router")]
pub mod rig_router;

#[cfg(feature = "fsrs-brain")]
pub mod fsrs_scheduler;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Local inference errors
#[derive(Error, Debug)]
pub enum InferenceError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Invalid model format: {0}")]
    InvalidFormat(String),

    #[error("GPU not available")]
    GpuUnavailable,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Model quantization types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationType {
    /// No quantization (FP16/FP32)
    None,
    /// 4-bit quantization (Q4_0, Q4_1, Q4_K_M, etc.)
    Q4,
    /// 5-bit quantization (Q5_0, Q5_1, Q5_K_M, etc.)
    Q5,
    /// 8-bit quantization (Q8_0)
    Q8,
    /// QLoRA (4-bit with adapters)
    QLoRA,
}

/// Local model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelConfig {
    /// HuggingFace model ID or local path
    pub model_id: String,
    /// Specific file to load (e.g., "model-q4_k_m.gguf")
    pub filename: Option<String>,
    /// Quantization type
    pub quantization: QuantizationType,
    /// Number of GPU layers (-1 = all, 0 = CPU only)
    pub n_gpu_layers: i32,
    /// Context size
    pub context_size: u32,
    /// Use Flash Attention
    pub flash_attention: bool,
}

impl Default for LocalModelConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            filename: None,
            quantization: QuantizationType::Q4,
            n_gpu_layers: -1,
            context_size: 4096,
            flash_attention: true,
        }
    }
}

/// Inference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub text: String,
    pub tokens_generated: u32,
    pub tokens_per_second: f32,
    pub model_id: String,
}

/// Get GPU status and recommended layers for a model size
#[tauri::command]
pub fn cmd_gpu_info(model_params_b: Option<f32>) -> Result<serde_json::Value, String> {
    let gpu = check_gpu_available();
    let layers = model_params_b
        .map(|params| recommended_gpu_layers(if gpu { 16.0 } else { 0.0 }, params));

    Ok(serde_json::json!({
        "gpu_available": gpu,
        "recommended_layers": layers,
        "backend": if std::path::Path::new("/opt/rocm").exists() {
            "rocm"
        } else if std::path::Path::new("/usr/local/cuda").exists() {
            "cuda"
        } else {
            "cpu"
        },
    }))
}

/// Check if GPU/ROCm is available
pub fn check_gpu_available() -> bool {
    // Check for AMD ROCm
    if std::env::var("HSA_OVERRIDE_GFX_VERSION").is_ok() {
        return true;
    }

    // Check for ROCm path
    if std::path::Path::new("/opt/rocm").exists() {
        return true;
    }

    // Check for CUDA
    if std::path::Path::new("/usr/local/cuda").exists() {
        return true;
    }

    false
}

/// Get recommended GPU layers based on VRAM
pub fn recommended_gpu_layers(vram_gb: f32, model_params_b: f32) -> i32 {
    // Rough estimate: ~0.5GB per billion parameters for Q4
    let estimated_model_gb = model_params_b * 0.5;

    if vram_gb >= estimated_model_gb * 1.2 {
        -1 // All layers on GPU
    } else if vram_gb >= estimated_model_gb * 0.5 {
        // Partial offload
        ((vram_gb / estimated_model_gb) * 32.0) as i32
    } else {
        0 // CPU only
    }
}
