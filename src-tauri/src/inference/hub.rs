//! HuggingFace Hub Integration
//!
//! Download and manage models from HuggingFace Hub

use hf_hub::api::tokio::Api;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use super::InferenceError;

/// Model information from HuggingFace Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub local_path: Option<PathBuf>,
    pub downloaded: bool,
}

/// Download a model from HuggingFace Hub
///
/// # Arguments
/// * `repo_id` - HuggingFace repository ID (e.g., "TheBloke/Llama-2-7B-GGUF")
/// * `filename` - Specific file to download (e.g., "llama-2-7b.Q4_K_M.gguf")
///
/// # Returns
/// Path to the downloaded model file
pub async fn download_model(repo_id: &str, filename: &str) -> Result<PathBuf, InferenceError> {
    log::info!("Downloading model: {}/{}", repo_id, filename);

    let api = Api::new()
        .map_err(|e| InferenceError::DownloadFailed(e.to_string()))?;

    let repo = api.model(repo_id.to_string());

    let path = repo
        .get(filename)
        .await
        .map_err(|e| InferenceError::DownloadFailed(e.to_string()))?;

    log::info!("Model downloaded to: {:?}", path);
    Ok(path)
}

/// List all cached models in the HuggingFace cache directory
pub async fn list_cached_models() -> Result<Vec<ModelInfo>, InferenceError> {
    let cache_dir = get_hf_cache_dir()?;

    if !cache_dir.exists() {
        return Ok(vec![]);
    }

    let mut models = Vec::new();

    // Scan the hub/models directory
    let models_dir = cache_dir.join("hub");
    if models_dir.exists() {
        let mut entries = fs::read_dir(&models_dir)
            .await
            .map_err(InferenceError::IoError)?;

        while let Some(entry) = entries.next_entry().await.map_err(InferenceError::IoError)? {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Parse model ID from directory name (models--org--repo format)
                if dir_name.starts_with("models--") {
                    let model_id = dir_name
                        .strip_prefix("models--")
                        .unwrap_or("")
                        .replace("--", "/");

                    // Find GGUF or safetensors files
                    let snapshots_dir = path.join("snapshots");
                    if snapshots_dir.exists() {
                        if let Ok(mut snapshot_entries) = fs::read_dir(&snapshots_dir).await {
                            while let Ok(Some(snapshot)) = snapshot_entries.next_entry().await {
                                let snapshot_path = snapshot.path();
                                if let Ok(mut files) = fs::read_dir(&snapshot_path).await {
                                    while let Ok(Some(file)) = files.next_entry().await {
                                        let file_path = file.path();
                                        let file_name = file_path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("")
                                            .to_string();

                                        if file_name.ends_with(".gguf") || file_name.ends_with(".safetensors") {
                                            let metadata = fs::metadata(&file_path).await.ok();
                                            let size = metadata.map(|m| m.len()).unwrap_or(0);

                                            models.push(ModelInfo {
                                                id: model_id.clone(),
                                                filename: file_name,
                                                size_bytes: size,
                                                local_path: Some(file_path),
                                                downloaded: true,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(models)
}

/// Get HuggingFace cache directory
fn get_hf_cache_dir() -> Result<PathBuf, InferenceError> {
    // Check HF_HOME first
    if let Ok(hf_home) = std::env::var("HF_HOME") {
        return Ok(PathBuf::from(hf_home));
    }

    // Check XDG_CACHE_HOME
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg_cache).join("huggingface"));
    }

    // Default to ~/.cache/huggingface
    let home = std::env::var("HOME")
        .map_err(|_| InferenceError::IoError(
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set")
        ))?;

    Ok(PathBuf::from(home).join(".cache").join("huggingface"))
}

/// Popular GGUF models for quick access
pub const POPULAR_GGUF_MODELS: &[(&str, &str, &str)] = &[
    // (repo_id, filename, description)
    ("TheBloke/Llama-2-7B-GGUF", "llama-2-7b.Q4_K_M.gguf", "Llama 2 7B Q4"),
    ("TheBloke/Mistral-7B-Instruct-v0.2-GGUF", "mistral-7b-instruct-v0.2.Q4_K_M.gguf", "Mistral 7B Instruct Q4"),
    ("TheBloke/CodeLlama-7B-Instruct-GGUF", "codellama-7b-instruct.Q4_K_M.gguf", "CodeLlama 7B Q4"),
    ("Qwen/Qwen2.5-Coder-7B-Instruct-GGUF", "qwen2.5-coder-7b-instruct-q4_k_m.gguf", "Qwen2.5 Coder 7B Q4"),
    ("bartowski/dolphin-2.9.4-llama3.1-8b-GGUF", "dolphin-2.9.4-llama3.1-8b-Q4_K_M.gguf", "Dolphin 3 8B Q4"),
];

/// Tauri command: Download a model
#[tauri::command]
pub async fn cmd_download_model(repo_id: String, filename: String) -> Result<String, String> {
    let path = download_model(&repo_id, &filename)
        .await
        .map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

/// Tauri command: List cached models
#[tauri::command]
pub async fn cmd_list_models() -> Result<Vec<ModelInfo>, String> {
    list_cached_models()
        .await
        .map_err(|e| e.to_string())
}

/// Tauri command: Get popular models
#[tauri::command]
pub fn cmd_popular_models() -> Vec<serde_json::Value> {
    POPULAR_GGUF_MODELS
        .iter()
        .map(|(repo, file, desc)| {
            serde_json::json!({
                "repo_id": repo,
                "filename": file,
                "description": desc
            })
        })
        .collect()
}
