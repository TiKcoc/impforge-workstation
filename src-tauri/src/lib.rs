// SPDX-License-Identifier: Apache-2.0
//! ImpForge — AI Workstation Builder
//!
//! Main library entry point for the Tauri application.

pub mod error;
pub mod traits;
pub mod serialization;
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

// Standalone Orchestrator — ImpForge's own AI brain (no external dependencies)
mod orchestrator;
mod neuralswarm;

// Ollama Local Inference — streaming chat, model listing, health checks
mod ollama;

// Built-in Web Scraper (MIT-licensed, no external API required)
mod web_scraper;

// Social Media Hub — content creation, scheduling, AI generation
mod social;

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

// ForgeMemory — Custom AI Memory Engine (SQLite + HNSW + BM25 + MemGPT + KG)
mod forge_memory;

// News Feed — AI/Dev News Aggregator (RSS/Atom feeds, offline-capable)
mod news_feed;

// ForgeSunshine — Moonlight Remote Access Manager (Sunshine streaming server)
mod sunshine;

// App Launcher — Self-extending application registry (Universal app launcher)
mod app_launcher;

// ForgeWriter — Document editor & AI writing assistant (Phase 3: Office tools)
mod forge_writer;

// Freelancer Hub — Gig management, CRM-lite, proposals, invoices, time tracking
mod freelancer;

// ForgeSheets — KI-native Spreadsheet Engine (Excel replacement, Clean Room)
mod forge_sheets;

// Auto-Publisher — Universal cross-platform automation (CDP-based, user-approved)
mod auto_publisher;

// ForgePDF — PDF viewer, text extraction, AI analysis & conversion
mod forge_pdf;

// ForgeCanvas — 3-Panel AI Document Workspace (drag sources, generate output, inspect context)
mod forge_canvas;

// Universal File Processor — format detection, preview, conversion, routing
mod file_processor;

// ForgeSlides — Markdown-based Presentation Creator & AI Generator
mod forge_slides;

// ForgeMail — AI-Powered Email Client (compose, categorize, manage)
mod forge_mail;

// ForgeTeam — P2P Team Collaboration & ImpBook Shared Knowledge Workspace
mod forge_team;

// ForgeCalendar — AI-Powered Calendar with ICS/iCal Import (Google, Outlook, Apple)
mod forge_calendar;

// Auto-Import — CDP-powered data import from external services (Google, Outlook, etc.)
mod auto_import;

// ForgeNotes — Personal Knowledge Base with Wiki-Links & Knowledge Graph
mod forge_notes;

use tauri::Manager;
use serde::{Deserialize, Serialize};

use forge_memory::engine::ForgeMemoryEngine;
use forge_memory::watch::ForgeWatcher;

/// Message for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedMessage {
    pub content: String,
    pub model_id: Option<String>,
    pub conversation_id: Option<String>,
}

/// Build router config with auto-detected Ollama availability
fn build_router_config() -> router::RouterConfig {
    let mut config = router::RouterConfig::new()
        .with_openrouter_key(std::env::var("OPENROUTER_API_KEY").unwrap_or_default());

    // Auto-detect local Ollama (default VRAM estimate based on common GPUs)
    if std::process::Command::new("ollama").arg("--version").output().is_ok() {
        config = config.with_ollama(8.0);
    }

    config
}

/// Route a message through the intelligent router
#[tauri::command]
async fn route_message(message: RoutedMessage) -> Result<String, String> {
    log::info!("Routing message: {:?}", message.content.chars().take(50).collect::<String>());

    let config = build_router_config();

    match router::route_and_execute(&message.content, None, &config).await {
        Ok(response) => Ok(response),
        Err(e) => Err(e.to_string()),
    }
}

/// Get routing decision preview (task type and target model)
#[tauri::command]
fn get_routing_preview(prompt: String) -> (String, String) {
    let config = build_router_config();
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

    let config = build_router_config();

    let task_type = router::classify_fast(&message.content);
    let target = router::targets::select_target(task_type, &config);
    let conv_id = message.conversation_id.unwrap_or_else(|| "default".to_string());

    target
        .execute_streaming(
            "You are a helpful AI assistant in ImpForge, an AI Workstation Builder.",
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
    // Install panic hook — prevents crashes from killing the Tauri window
    error::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
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

            app.manage(ide::pty::PtyManager::new());
            app.manage(ide::db_client::DbConnectionPool::new());
            app.manage(ide::indexer::CodebaseIndexer::new());
            app.manage(ide::shadow::ShadowManager::new());
            app.manage(ide::lsp::LspManager::new());
            app.manage(ide::debug::DebugManager::new());
            app.manage(ide::collab::CollabManager::new());
            app.manage(ide::billing::LicenseManager::new());

            // ForgeMemory — AI Memory Engine (SQLite + HNSW + BM25 + MemGPT + KG)
            let db_path = app
                .handle()
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("forge_memory.db");
            match ForgeMemoryEngine::new(&db_path) {
                Ok(engine) => {
                    log::info!("ForgeMemory initialized at {}", db_path.display());
                    app.manage(engine);
                }
                Err(e) => {
                    log::error!("ForgeMemory init failed: {e}");
                    // Still start the app — memory features will be unavailable
                }
            }

            // ForgeWatch — Filesystem Watcher Engine
            app.manage(ForgeWatcher::new());

            log::info!("ImpForge initialized");
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
            // Agent commands (CRUD)
            agents::list_agents,
            agents::get_agent,
            agents::create_agent,
            agents::update_agent,
            agents::delete_agent,
            agents::get_agent_by_role,
            // Agent commands (runtime: run, stop, logs, status)
            agents::run_agent,
            agents::stop_agent,
            agents::agent_logs,
            agents::agent_status,
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
            // IDE Codebase Indexer
            ide::indexer::index_codebase,
            ide::indexer::search_codebase,
            ide::indexer::index_status,
            // IDE Git commands
            ide::git::git_status,
            ide::git::git_diff,
            ide::git::git_log,
            ide::git::git_commit,
            ide::git::git_stage,
            ide::git::git_unstage,
            ide::git::git_branches,
            ide::git::git_create_branch,
            ide::git::git_checkout,
            ide::git::git_delete_branch,
            ide::git::git_push,
            ide::git::git_pull,
            ide::git::git_blame,
            // IDE Shadow Workspace (isolated AI code modification with diff review)
            ide::shadow::shadow_create,
            ide::shadow::shadow_write,
            ide::shadow::shadow_read,
            ide::shadow::shadow_diff_all,
            ide::shadow::shadow_diff_file,
            ide::shadow::shadow_apply,
            ide::shadow::shadow_apply_all,
            ide::shadow::shadow_discard,
            ide::shadow::shadow_list,
            // IDE AI Completion (Multi-Model Cascading + Caching + Telemetry)
            ide::ai_complete::ai_complete,
            ide::ai_complete::ai_completion_stats,
            // IDE LSP Backend (multi-language LSP server management)
            ide::lsp::lsp_start,
            ide::lsp::lsp_stop,
            ide::lsp::lsp_diagnostics,
            ide::lsp::lsp_hover,
            ide::lsp::lsp_completions,
            ide::lsp::lsp_definition,
            ide::lsp::lsp_did_open,
            ide::lsp::lsp_did_change,
            ide::lsp::lsp_status,
            // IDE Debug Adapter Protocol (DAP)
            ide::debug::debug_launch,
            ide::debug::debug_set_breakpoints,
            ide::debug::debug_continue,
            ide::debug::debug_step_over,
            ide::debug::debug_step_in,
            ide::debug::debug_step_out,
            ide::debug::debug_pause,
            ide::debug::debug_stop,
            ide::debug::debug_get_threads,
            ide::debug::debug_get_stack_trace,
            ide::debug::debug_get_variables,
            ide::debug::debug_get_scopes,
            ide::debug::debug_evaluate,
            ide::debug::debug_status,
            // IDE Collaboration (CRDT + Presence)
            ide::collab::collab_create_room,
            ide::collab::collab_join_room,
            ide::collab::collab_leave_room,
            ide::collab::collab_send_operation,
            ide::collab::collab_update_cursor,
            ide::collab::collab_get_peers,
            ide::collab::collab_get_rooms,
            ide::collab::collab_status,
            ide::collab::collab_knowledge_graph,
            ide::collab::collab_co_changes,
            // IDE Billing & License Management
            ide::billing::billing_activate_license,
            ide::billing::billing_get_license,
            ide::billing::billing_check_feature,
            ide::billing::billing_get_tier,
            ide::billing::billing_get_usage,
            ide::billing::billing_record_completion,
            ide::billing::billing_deactivate,
            ide::billing::billing_get_pricing,
            ide::billing::billing_team_members,
            // IDE PTY commands (real terminal)
            ide::pty::pty_spawn,
            ide::pty::pty_write,
            ide::pty::pty_resize,
            ide::pty::pty_kill,
            ide::pty::pty_list,
            // IDE Database Client (SQLite, schema introspection, query execution)
            ide::db_client::db_connect,
            ide::db_client::db_disconnect,
            ide::db_client::db_list_connections,
            ide::db_client::db_execute_query,
            ide::db_client::db_schema,
            ide::db_client::db_query_history,
            ide::db_client::db_export_csv,
            // IDE HTTP Client (REST API testing, cURL export, collections)
            ide::http_client::http_send_request,
            ide::http_client::http_to_curl,
            ide::http_client::http_save_collection,
            ide::http_client::http_load_collection,
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
            // Ollama local inference commands
            ollama::cmd_ollama_status,
            ollama::cmd_ollama_models,
            ollama::cmd_ollama_chat,
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
            neuralswarm::neuralswarm_reset_circuit_breaker,
            neuralswarm::neuralswarm_worker_trust,
            neuralswarm::neuralswarm_cleanup,
            neuralswarm::neuralswarm_selftest,
            // Phase 3-5 module commands
            neuralswarm::neuralswarm_moa_run,
            neuralswarm::neuralswarm_topology_snapshot,
            neuralswarm::neuralswarm_evaluate,
            neuralswarm::neuralswarm_eval_leaderboard,
            neuralswarm::neuralswarm_scaling_status,
            neuralswarm::neuralswarm_resource_status,
            neuralswarm::neuralswarm_git_status,
            neuralswarm::neuralswarm_social_status,
            neuralswarm::neuralswarm_cicd_run,
            neuralswarm::neuralswarm_route_inference,
            neuralswarm::neuralswarm_export_snapshot,
            neuralswarm::neuralswarm_import_snapshot,
            // Social Media Hub commands (content creation, scheduling, AI generation)
            social::social_get_platforms,
            social::social_compose_post,
            social::social_ai_generate,
            social::social_get_queue,
            social::social_get_templates,
            social::social_cancel_post,
            social::social_mark_published,
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
            // Style Engine — Theme palette presets
            style_engine::style_get_theme_palette,
            style_engine::style_list_theme_presets,
            // ForgeMemory — AI Memory Engine commands
            forge_memory::commands::forge_memory_add,
            forge_memory::commands::forge_memory_get_core,
            forge_memory::commands::forge_memory_search,
            forge_memory::commands::forge_memory_promote,
            forge_memory::commands::forge_memory_demote,
            forge_memory::commands::forge_memory_review,
            forge_memory::commands::forge_memory_consolidate,
            forge_memory::commands::forge_memory_delete,
            forge_memory::commands::forge_memory_add_knowledge,
            forge_memory::commands::forge_memory_search_knowledge,
            forge_memory::commands::forge_memory_kg_add_node,
            forge_memory::commands::forge_memory_kg_add_edge,
            forge_memory::commands::forge_memory_kg_neighbors,
            forge_memory::commands::forge_memory_kg_traverse,
            forge_memory::commands::forge_memory_kg_communities,
            forge_memory::commands::forge_memory_kg_subgraph,
            forge_memory::commands::forge_memory_kg_most_connected,
            forge_memory::commands::forge_memory_create_conversation,
            forge_memory::commands::forge_memory_save_message,
            forge_memory::commands::forge_memory_get_messages,
            forge_memory::commands::forge_memory_get_context,
            forge_memory::commands::forge_memory_auto_learn,
            forge_memory::commands::forge_memory_stats,
            forge_memory::commands::forge_memory_status,
            forge_memory::commands::forge_memory_persist,
            // ForgeWatch — Filesystem monitoring + ingestion
            forge_memory::commands::forge_watch_discover,
            forge_memory::commands::forge_watch_add_path,
            forge_memory::commands::forge_watch_remove_path,
            forge_memory::commands::forge_watch_list_paths,
            forge_memory::commands::forge_watch_status,
            forge_memory::commands::forge_watch_reindex,
            forge_memory::commands::forge_watch_ingest_file,
            // Universal Input Digest
            forge_memory::commands::forge_digest_text,
            forge_memory::commands::forge_digest_text_configured,
            forge_memory::commands::forge_digest_get_config,
            // ForgeSunshine — Moonlight remote access
            sunshine::sunshine_detect,
            sunshine::sunshine_install_cmd,
            sunshine::sunshine_get_config,
            sunshine::sunshine_save_config,
            sunshine::sunshine_start,
            sunshine::sunshine_stop,
            sunshine::sunshine_status,
            // News Feed commands (RSS/Atom aggregation)
            news_feed::news_fetch,
            news_feed::news_sources,
            // Agent status commands
            agents::get_agent_statuses,
            // System Agent — file scanning
            system_agent::system_scan_files,
            // Inference — GPU info
            inference::cmd_gpu_info,
            // CDP Engine — browser info
            cdp_engine::cdp_browser_info,
            // CDP Network — record requests
            cdp_network::cdp_network_record,
            // Browser Agent — session listing
            browser_agent::browser_agent_list_sessions,
            // App Launcher — self-extending application registry
            app_launcher::app_list,
            app_launcher::app_add,
            app_launcher::app_remove,
            app_launcher::app_update,
            app_launcher::app_launch,
            app_launcher::app_pin,
            app_launcher::app_health,
            app_launcher::app_discover_installed,
            // ForgeWriter — Document editor & AI writing assistant
            forge_writer::writer_list_documents,
            forge_writer::writer_get_document,
            forge_writer::writer_create_document,
            forge_writer::writer_save_document,
            forge_writer::writer_delete_document,
            forge_writer::writer_export_document,
            forge_writer::writer_ai_assist,
            forge_writer::writer_word_count,
            // Freelancer Hub — gig management, CRM, proposals, invoices, time tracking
            freelancer::freelancer_get_profile,
            freelancer::freelancer_save_profile,
            freelancer::freelancer_list_gigs,
            freelancer::freelancer_add_gig,
            freelancer::freelancer_update_gig,
            freelancer::freelancer_delete_gig,
            freelancer::freelancer_list_clients,
            freelancer::freelancer_add_client,
            freelancer::freelancer_update_client,
            freelancer::freelancer_list_proposals,
            freelancer::freelancer_save_proposal,
            freelancer::freelancer_generate_proposal,
            freelancer::freelancer_list_invoices,
            freelancer::freelancer_create_invoice,
            freelancer::freelancer_update_invoice_status,
            freelancer::freelancer_time_entries,
            freelancer::freelancer_start_timer,
            freelancer::freelancer_stop_timer,
            freelancer::freelancer_add_time_entry,
            freelancer::freelancer_earnings_summary,
            // Auto-Publisher — cross-platform automation (CDP-based, user-approved)
            auto_publisher::autopub_get_platforms,
            auto_publisher::autopub_toggle_platform,
            auto_publisher::autopub_add_platform,
            auto_publisher::autopub_remove_platform,
            auto_publisher::autopub_publish,
            auto_publisher::autopub_sync_profile,
            auto_publisher::autopub_execute_action,
            auto_publisher::autopub_get_log,
            auto_publisher::autopub_get_scripts,
            auto_publisher::autopub_update_script,
            // ForgeSheets — KI-native Spreadsheet Engine (Excel replacement)
            forge_sheets::sheets_list,
            forge_sheets::sheets_create,
            forge_sheets::sheets_open,
            forge_sheets::sheets_save,
            forge_sheets::sheets_delete,
            forge_sheets::sheets_import_file,
            forge_sheets::sheets_export,
            forge_sheets::sheets_set_cell,
            forge_sheets::sheets_get_range,
            forge_sheets::sheets_add_sheet,
            forge_sheets::sheets_ai_formula,
            forge_sheets::sheets_ai_analyze,
            forge_sheets::sheets_evaluate_formula,
            // ForgePDF — PDF viewer, AI analysis & conversion
            forge_pdf::pdf_list,
            forge_pdf::pdf_import,
            forge_pdf::pdf_get_info,
            forge_pdf::pdf_get_text,
            forge_pdf::pdf_delete,
            forge_pdf::pdf_ai_summarize,
            forge_pdf::pdf_ai_ask,
            forge_pdf::pdf_convert_to_text,
            forge_pdf::pdf_convert_to_markdown,
            // ForgeCanvas — 3-Panel AI Document Workspace
            forge_canvas::canvas_create,
            forge_canvas::canvas_list,
            forge_canvas::canvas_open,
            forge_canvas::canvas_save,
            forge_canvas::canvas_delete,
            forge_canvas::canvas_add_source,
            forge_canvas::canvas_remove_source,
            forge_canvas::canvas_generate,
            forge_canvas::canvas_chat,
            forge_canvas::canvas_get_templates,
            forge_canvas::canvas_transform_selection,
            forge_canvas::canvas_auto_detect_intent,
            forge_canvas::canvas_export_professional,
            // Universal File Processor — detect, preview, convert, route, AI digest
            file_processor::file_detect,
            file_processor::file_preview,
            file_processor::file_convert,
            file_processor::file_open_in_module,
            file_processor::file_batch_convert,
            file_processor::file_ai_digest,
            file_processor::file_supported_formats,
            file_processor::file_recent,
            // ForgeSlides — Markdown Presentation Creator & AI Generator
            forge_slides::slides_list,
            forge_slides::slides_create,
            forge_slides::slides_open,
            forge_slides::slides_save,
            forge_slides::slides_delete,
            forge_slides::slides_add_slide,
            forge_slides::slides_remove_slide,
            forge_slides::slides_reorder,
            forge_slides::slides_ai_generate,
            forge_slides::slides_ai_improve_slide,
            forge_slides::slides_export_html,
            forge_slides::slides_get_themes,
            // ForgeMail — AI-Powered Email Client
            forge_mail::mail_list_accounts,
            forge_mail::mail_add_account,
            forge_mail::mail_remove_account,
            forge_mail::mail_list_emails,
            forge_mail::mail_get_email,
            forge_mail::mail_mark_read,
            forge_mail::mail_star,
            forge_mail::mail_delete,
            forge_mail::mail_move,
            forge_mail::mail_search,
            forge_mail::mail_ai_compose,
            forge_mail::mail_ai_categorize,
            forge_mail::mail_send_draft,
            forge_mail::mail_webmail_url,
            forge_mail::mail_folder_counts,
            // ForgeTeam — P2P Collaboration & ImpBook Shared Workspace
            forge_team::team_create,
            forge_team::team_list,
            forge_team::team_get,
            forge_team::team_join,
            forge_team::team_leave,
            forge_team::team_invite_code,
            forge_team::team_update_member_status,
            forge_team::team_get_members,
            forge_team::impbook_list_entries,
            forge_team::impbook_create_entry,
            forge_team::impbook_update_entry,
            forge_team::impbook_delete_entry,
            forge_team::impbook_add_comment,
            forge_team::impbook_add_reaction,
            forge_team::impbook_pin_entry,
            forge_team::team_activity_feed,
            forge_team::team_share_agent_result,
            // ForgeCalendar — AI-Powered Calendar with ICS/iCal Import
            forge_calendar::calendar_list,
            forge_calendar::calendar_create,
            forge_calendar::calendar_delete,
            forge_calendar::calendar_import_ics,
            forge_calendar::calendar_list_events,
            forge_calendar::calendar_create_event,
            forge_calendar::calendar_update_event,
            forge_calendar::calendar_delete_event,
            forge_calendar::calendar_get_day,
            forge_calendar::calendar_ai_suggest_time,
            forge_calendar::calendar_ai_daily_briefing,
            forge_calendar::calendar_ai_generate_agenda,
            forge_calendar::calendar_sync_ics,
            // Auto-Import — CDP-powered data import from external services
            auto_import::autoimport_list_sources,
            auto_import::autoimport_add_source,
            auto_import::autoimport_remove_source,
            auto_import::autoimport_toggle,
            auto_import::autoimport_set_target,
            auto_import::autoimport_get_steps,
            auto_import::autoimport_run,
            auto_import::autoimport_complete,
            auto_import::autoimport_history,
            auto_import::autoimport_source_types,
            auto_import::autoimport_target_modules,
            auto_import::autoimport_reset_status,
            // ForgeTeam — Chat, Goals, Learning, Suggestions, Related
            forge_team::team_send_message,
            forge_team::team_get_messages,
            forge_team::team_set_goal,
            forge_team::team_list_goals,
            forge_team::team_update_goal_progress,
            forge_team::impbook_learn_from_feedback,
            forge_team::impbook_suggest_entries,
            forge_team::impbook_related_entries,
            // ForgeNotes — Personal Knowledge Base with Wiki-Links & Knowledge Graph
            forge_notes::notes_list,
            forge_notes::notes_create,
            forge_notes::notes_get,
            forge_notes::notes_save,
            forge_notes::notes_delete,
            forge_notes::notes_search,
            forge_notes::notes_get_backlinks,
            forge_notes::notes_get_tags,
            forge_notes::notes_pin,
            forge_notes::notes_archive,
            forge_notes::notes_ai_generate,
            forge_notes::notes_ai_connect,
            forge_notes::notes_ai_summarize_tag,
            forge_notes::notes_get_graph,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImpForge");
}
