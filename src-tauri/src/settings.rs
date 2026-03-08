//! ImpForge Settings Module
//!
//! Provides Tauri commands for reading/writing application settings.
//! Uses tauri-plugin-store for JSON-backed persistence.

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_store::StoreExt;

/// All configurable settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpForgeSettings {
    pub openrouter_api_key: Option<String>,
    pub ollama_url: String,
    pub default_provider: String,
    pub font_size: u32,
    pub theme: String,
    pub services: ServiceUrls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceUrls {
    pub n8n: String,
    pub langflow: String,
    pub openwebui: String,
    pub grafana: String,
    pub searxng: String,
    pub comfyui: String,
}

impl Default for ImpForgeSettings {
    fn default() -> Self {
        Self {
            openrouter_api_key: None,
            ollama_url: "http://localhost:11434".to_string(),
            default_provider: "auto".to_string(),
            font_size: 14,
            theme: "opera-gx-dark".to_string(),
            services: ServiceUrls::default(),
        }
    }
}

impl Default for ServiceUrls {
    fn default() -> Self {
        Self {
            n8n: "http://localhost:5678".to_string(),
            langflow: "http://localhost:7860".to_string(),
            openwebui: "http://localhost:3000".to_string(),
            grafana: "http://localhost:4000".to_string(),
            searxng: "http://localhost:8888".to_string(),
            comfyui: "http://localhost:8188".to_string(),
        }
    }
}

/// Get all settings
#[tauri::command]
pub fn cmd_get_settings(app: tauri::AppHandle) -> Result<ImpForgeSettings, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;

    let settings = ImpForgeSettings {
        openrouter_api_key: store
            .get("openrouter_api_key")
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        ollama_url: store
            .get("ollama_url")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "http://localhost:11434".to_string()),
        default_provider: store
            .get("default_provider")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "auto".to_string()),
        font_size: store
            .get("font_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(14) as u32,
        theme: store
            .get("theme")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "opera-gx-dark".to_string()),
        services: ServiceUrls {
            n8n: store
                .get("service_n8n")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:5678".to_string()),
            langflow: store
                .get("service_langflow")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:7860".to_string()),
            openwebui: store
                .get("service_openwebui")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:3000".to_string()),
            grafana: store
                .get("service_grafana")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:4000".to_string()),
            searxng: store
                .get("service_searxng")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:8888".to_string()),
            comfyui: store
                .get("service_comfyui")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "http://localhost:8188".to_string()),
        },
    };

    // Also set the OpenRouter key as env var so the router can use it
    if let Some(ref key) = settings.openrouter_api_key {
        if !key.is_empty() {
            std::env::set_var("OPENROUTER_API_KEY", key);
        }
    }

    Ok(settings)
}

/// Save a single setting
#[tauri::command]
pub fn cmd_set_setting(app: tauri::AppHandle, key: String, value: String) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set(&key, serde_json::Value::String(value.clone()));
    store.save().map_err(|e| e.to_string())?;

    // If it's the OpenRouter key, also set env var immediately
    if key == "openrouter_api_key" && !value.is_empty() {
        std::env::set_var("OPENROUTER_API_KEY", &value);
    }

    Ok(())
}

/// Test connection to Ollama
#[tauri::command]
pub async fn cmd_test_ollama(url: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(format!("{}/api/tags", url))
        .send()
        .await
        .map_err(|_| "Connection failed".to_string())?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let model_count = body
            .get("models")
            .and_then(|m| m.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        Ok(format!("Connected — {} models available", model_count))
    } else {
        Err(format!("Server returned {}", resp.status()))
    }
}

/// Validate OpenRouter API key
#[tauri::command]
pub async fn cmd_validate_openrouter_key(key: String) -> Result<String, String> {
    if key.is_empty() {
        return Err("No key provided".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .map_err(|_| "Connection to OpenRouter failed".to_string())?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let credits = body
            .pointer("/data/limit_remaining")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        Ok(format!("Valid key — ${:.2} credits remaining", credits))
    } else {
        Err("Invalid API key".to_string())
    }
}

/// Get application data paths (uses Manager trait for cross-platform resolution)
#[tauri::command]
pub fn cmd_get_app_paths(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "data": data_dir.to_string_lossy(),
        "config": config_dir.to_string_lossy(),
        "cache": cache_dir.to_string_lossy(),
        "logs": log_dir.to_string_lossy(),
    }))
}
