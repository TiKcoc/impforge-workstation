//! NEXUS - AI Orchestration Hub
//!
//! Main library entry point for the Tauri application.

mod router;
mod agents;
mod docker;
mod github;
mod ide;
mod inference;
mod settings;
// Quick monitoring (sysfs-based, always available)
mod monitoring_quick;
// Full monitoring module disabled until sysinfo 0.38 API migration
// mod monitoring;
mod evaluation;
mod chat;
mod browser;
mod system_agent;

// Standalone Orchestrator — Nexus's own AI brain (no external dependencies)
mod orchestrator;
mod neuralswarm;

// Built-in Web Scraper (MIT-licensed, no external API required)
mod web_scraper;

// use tauri::Manager; // Reserved for future app handle operations
use serde::{Deserialize, Serialize};

/// Message for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedMessage {
    pub content: String,
    pub model_id: Option<String>,
    pub conversation_id: Option<String>,
}

/// Route a message through the intelligent router
#[tauri::command]
async fn route_message(message: RoutedMessage) -> Result<String, String> {
    log::info!("Routing message: {:?}", message.content.chars().take(50).collect::<String>());

    let config = router::RouterConfig::new()
        .with_openrouter_key(std::env::var("OPENROUTER_API_KEY").unwrap_or_default());

    match router::route_and_execute(&message.content, None, &config).await {
        Ok(response) => Ok(response),
        Err(e) => Err(e.to_string()),
    }
}

/// Get routing decision preview (task type and target model)
#[tauri::command]
fn get_routing_preview(prompt: String) -> (String, String) {
    let config = router::RouterConfig::new();
    let (task_type, target) = router::get_routing_decision(&prompt, &config);
    (task_type.description().to_string(), target.display_name())
}

/// Route a message with streaming response (tokens emitted via "chat-stream" event)
#[tauri::command]
async fn route_message_stream(
    app: tauri::AppHandle,
    message: RoutedMessage,
) -> Result<String, String> {
    log::info!("Streaming message: {:?}", message.content.chars().take(50).collect::<String>());

    let config = router::RouterConfig::new()
        .with_openrouter_key(std::env::var("OPENROUTER_API_KEY").unwrap_or_default());

    let task_type = router::classify_fast(&message.content);
    let target = router::targets::select_target(task_type, &config);
    let conv_id = message.conversation_id.unwrap_or_else(|| "default".to_string());

    target
        .execute_streaming(
            "You are a helpful AI assistant in NEXUS, an AI Workstation Builder.",
            &message.content,
            &app,
            &conv_id,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Get list of available models
#[tauri::command]
async fn get_available_models() -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![
        serde_json::json!({
            "id": "dolphin3:8b",
            "name": "Dolphin 3 8B",
            "provider": "Ollama",
            "free": true
        }),
        serde_json::json!({
            "id": "qwen2.5-coder:7b",
            "name": "Qwen2.5 Coder 7B",
            "provider": "Ollama",
            "free": true
        }),
        serde_json::json!({
            "id": "mistralai/devstral-small:free",
            "name": "Devstral Small (Code)",
            "provider": "OpenRouter",
            "free": true
        }),
        serde_json::json!({
            "id": "meta-llama/llama-4-scout:free",
            "name": "Llama 4 Scout",
            "provider": "OpenRouter",
            "free": true
        }),
    ])
}

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Initialize logging in debug mode
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            // Load settings on startup to set env vars (e.g. OPENROUTER_API_KEY)
            let _ = settings::cmd_get_settings(app.handle().clone());

            log::info!("NEXUS initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            route_message,
            route_message_stream,
            get_routing_preview,
            get_available_models,
            // Docker commands
            docker::list_containers,
            docker::container_action,
            docker::docker_info,
            // GitHub commands
            github::get_repos,
            github::get_issues,
            github::get_pull_requests,
            github::get_user,
            // Agent commands
            agents::list_agents,
            agents::get_agent,
            agents::create_agent,
            agents::update_agent,
            agents::delete_agent,
            agents::get_agent_by_role,
            // Inference commands (HuggingFace, llama.cpp, GGUF)
            inference::hub::cmd_download_model,
            inference::hub::cmd_list_models,
            inference::hub::cmd_popular_models,
            inference::gguf::cmd_load_gguf,
            inference::gguf::cmd_generate_gguf,
            // IDE commands (filesystem, search, agent tools)
            ide::ide_read_dir,
            ide::ide_read_file,
            ide::ide_write_file,
            ide::ide_search_files,
            ide::ide_execute_command,
            ide::ide_agent_tool_call,
            // Monitoring commands (lightweight sysfs-based, status bar + health checks)
            monitoring_quick::cmd_get_quick_stats,
            monitoring_quick::cmd_check_service_health,
            // Settings commands
            settings::cmd_get_settings,
            settings::cmd_set_setting,
            settings::cmd_test_ollama,
            settings::cmd_validate_openrouter_key,
            // Evaluation chain commands (Agent-as-a-Judge)
            evaluation::eval_agent_output,
            evaluation::eval_quick,
            evaluation::eval_history,
            evaluation::eval_get_config,
            // Chat streaming (Tauri Channel-based)
            chat::chat_stream,
            // Internal browser commands
            browser::open_internal_browser,
            browser::close_internal_browser,
            // System Agent commands (offline-first health checks)
            system_agent::system_scan,
            system_agent::system_health_quick,
            // NeuralSwarm Orchestrator commands
            neuralswarm::neuralswarm_status,
            neuralswarm::neuralswarm_tasks,
            neuralswarm::neuralswarm_logs,
            neuralswarm::neuralswarm_action,
            neuralswarm::neuralswarm_snapshot,
            // Web Scraper commands (built-in + optional Firecrawl Cloud)
            web_scraper::web_scrape,
            web_scraper::web_scrape_batch,
            web_scraper::web_extract_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}
