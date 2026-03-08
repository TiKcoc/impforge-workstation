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

// Browser Agent — AI-powered web automation (OpAgent-inspired, MIT/Apache-2.0)
mod browser_agent;

// CDP Browser Engine — Chrome DevTools Protocol (chromiumoxide, MIT/Apache-2.0)
mod cdp_engine;

// Browser Data Import — auto-detect & import bookmarks, history from installed browsers
mod browser_import;

// CDP Network Monitor — HTTP waterfall via Chrome DevTools Protocol
mod cdp_network;

// CDP DevTools — Console capture, Performance metrics, Cookie management
mod cdp_devtools;

// Theme Engine — Customer-facing UI customization (ElvUI/BenikUI-inspired)
mod theme_engine;

// Widget Registry — Modular dashboard components for layout manager
mod widget_registry;

// Style Engine — BenikUI-inspired deep sub-component customization
// Every widget decomposes into independently styleable sub-elements
mod style_engine;

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
            settings::cmd_get_app_paths,
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
            // Browser Agent commands (AI-powered web automation + n8n/Zapier webhooks)
            browser_agent::browser_agent_run,
            browser_agent::browser_agent_quick_extract,
            browser_agent::browser_agent_structured_extract,
            browser_agent::browser_agent_send_webhook,
            // CDP Engine commands (full browser automation via Chrome DevTools Protocol)
            cdp_engine::cdp_detect_browsers,
            cdp_engine::cdp_open_page,
            cdp_engine::cdp_navigate,
            cdp_engine::cdp_click,
            cdp_engine::cdp_fill,
            cdp_engine::cdp_execute_js,
            cdp_engine::cdp_extract,
            cdp_engine::cdp_screenshot,
            cdp_engine::cdp_get_page_content,
            cdp_engine::cdp_page_scroll,
            cdp_engine::cdp_pages,
            cdp_engine::cdp_close_page,
            // Browser Data Import commands (auto-detect profiles, import bookmarks/history)
            browser_import::browser_detect_profiles,
            browser_import::browser_import_bookmarks,
            browser_import::browser_import_history,
            browser_import::browser_import_all,
            // CDP Element Picker (visual CSS selector picker)
            cdp_engine::cdp_get_elements,
            cdp_engine::cdp_highlight_element,
            // CDP Network Monitor (HTTP waterfall)
            cdp_network::cdp_network_entries,
            cdp_network::cdp_network_enable,
            cdp_network::cdp_network_clear,
            // CDP DevTools (Console, Performance, Cookies)
            cdp_devtools::cdp_console_entries,
            cdp_devtools::cdp_console_clear,
            cdp_devtools::cdp_console_enable,
            cdp_devtools::cdp_console_flush,
            cdp_devtools::cdp_perf_metrics,
            cdp_devtools::cdp_get_cookies,
            cdp_devtools::cdp_delete_cookie,
            // Theme Engine (customer UI customization + WCAG validation)
            theme_engine::theme_list,
            theme_engine::theme_get_active,
            theme_engine::theme_set_active,
            theme_engine::theme_save,
            theme_engine::theme_delete,
            theme_engine::theme_export,
            theme_engine::theme_import,
            theme_engine::theme_validate_contrast,
            theme_engine::theme_suggest_fixes,
            theme_engine::theme_validate_all,
            theme_engine::layout_save,
            theme_engine::layout_get,
            theme_engine::layout_delete,
            // Widget Registry (modular dashboard components)
            widget_registry::widget_list,
            widget_registry::widget_get,
            widget_registry::widget_categories,
            widget_registry::widget_config_schema,
            // Style Engine (BenikUI-inspired deep sub-component customization)
            style_engine::style_get_widget,
            style_engine::style_save_widget,
            style_engine::style_reset_widget,
            style_engine::style_list_defaults,
            style_engine::style_list_fonts,
            style_engine::style_save_graph,
            style_engine::style_get_graph,
            style_engine::style_list_profiles,
            style_engine::style_create_profile,
            style_engine::style_delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}
