// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Universal Auto-Publisher -- Cross-platform automation layer.
//!
//! Turns ImpForge into a personal digital assistant that can auto-post,
//! auto-fill, and auto-sync across ALL platforms the user connects.
//!
//! Architecture:
//! - Platforms with APIs get direct integrations
//! - Platforms without APIs get Browser Agent (CDP) automation
//! - All actions are logged for transparency
//! - requires_confirmation defaults to true for write actions
//!
//! Storage: tauri-plugin-store `.impforge-platforms.json`
//! License: Apache-2.0 (commercial distribution safe)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

// ============================================================================
// TYPES -- Platform & Automation Models
// ============================================================================

/// The type of platform being automated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformType {
    SocialMedia,
    Freelancer,
    Professional,
    JobBoard,
    Custom,
}

/// Current connection status of a platform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "message")]
pub enum PlatformStatus {
    Connected,
    Disconnected,
    Syncing,
    Error(String),
}

/// A platform the user has connected for auto-publishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedPlatform {
    pub id: String,
    pub name: String,
    pub platform_type: PlatformType,
    pub url: String,
    pub icon: String,
    pub enabled: bool,
    pub auto_sync_profile: bool,
    pub auto_post: bool,
    pub last_synced: Option<String>,
    pub status: PlatformStatus,
    pub added_at: String,
}

/// The type of automation action to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    PostContent,
    UpdateProfile,
    CreateGig,
    SubmitProposal,
    SendMessage,
    ApplyToJob,
}

/// An automation action to execute on a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAction {
    pub platform_id: String,
    pub action_type: ActionType,
    pub content: serde_json::Value,
    pub requires_confirmation: bool,
}

/// A single step in a CDP automation script.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step_type")]
pub enum CdpStep {
    Navigate { url: String },
    WaitFor { selector: String, timeout_ms: u64 },
    Click { selector: String },
    Fill { selector: String, value: String },
    Screenshot { label: String },
    Wait { ms: u64 },
    ExecuteJs { script: String },
}

/// A queued automation script awaiting user review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedScript {
    pub id: String,
    pub platform_id: String,
    pub platform_name: String,
    pub action_type: ActionType,
    pub steps: Vec<CdpStep>,
    pub content_preview: String,
    pub status: ScriptStatus,
    pub created_at: String,
    pub executed_at: Option<String>,
    pub error: Option<String>,
}

/// Status of a queued automation script.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStatus {
    Pending,
    Approved,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Result of publishing to a single platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub platform_id: String,
    pub platform_name: String,
    pub success: bool,
    pub script_id: Option<String>,
    pub message: String,
}

/// Result of syncing profile to a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub platform_id: String,
    pub platform_name: String,
    pub success: bool,
    pub fields_synced: Vec<String>,
    pub message: String,
}

/// A log entry for automation actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationLogEntry {
    pub id: String,
    pub timestamp: String,
    pub platform_id: String,
    pub platform_name: String,
    pub action_type: String,
    pub status: String,
    pub details: String,
}

// ============================================================================
// PERSISTENT STORE
// ============================================================================

/// Top-level container serialised to `.impforge-platforms.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PlatformData {
    platforms: Vec<ConnectedPlatform>,
    scripts: Vec<QueuedScript>,
    log: Vec<AutomationLogEntry>,
}

static DATA: once_cell::sync::Lazy<Mutex<PlatformData>> =
    once_cell::sync::Lazy::new(|| Mutex::new(PlatformData::default()));

async fn persist(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store(".impforge-platforms.json")
        .map_err(|e| format!("Store error: {e}"))?;
    let data = DATA.lock().map_err(|e| format!("Lock error: {e}"))?;
    store.set(
        "platform_data",
        serde_json::to_value(&*data).map_err(|e| format!("Serialize: {e}"))?,
    );
    store.save().map_err(|e| format!("Save: {e}"))?;
    Ok(())
}

fn ensure_loaded(app: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;
    let already = {
        let d = DATA.lock().unwrap_or_else(|e| e.into_inner());
        !d.platforms.is_empty() || !d.log.is_empty()
    };
    if already {
        return;
    }

    if let Ok(store) = app.store(".impforge-platforms.json") {
        if let Some(val) = store.get("platform_data") {
            if let Ok(loaded) = serde_json::from_value::<PlatformData>(val.clone()) {
                let mut d = DATA.lock().unwrap_or_else(|e| e.into_inner());
                *d = loaded;
            }
        }
    }
}

fn log_action(
    data: &mut PlatformData,
    platform_id: &str,
    platform_name: &str,
    action_type: &str,
    status: &str,
    details: &str,
) {
    data.log.push(AutomationLogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        platform_id: platform_id.to_string(),
        platform_name: platform_name.to_string(),
        action_type: action_type.to_string(),
        status: status.to_string(),
        details: details.to_string(),
    });
    // Keep the log to a reasonable size (last 500 entries)
    if data.log.len() > 500 {
        let drain_count = data.log.len() - 500;
        data.log.drain(..drain_count);
    }
}

// ============================================================================
// PRE-BUILT PLATFORM DEFINITIONS
// ============================================================================

/// Return the list of well-known platforms with default metadata.
fn builtin_platforms() -> Vec<ConnectedPlatform> {
    vec![
        ConnectedPlatform {
            id: "linkedin".into(),
            name: "LinkedIn".into(),
            platform_type: PlatformType::SocialMedia,
            url: "https://www.linkedin.com".into(),
            icon: "linkedin".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "twitter".into(),
            name: "X / Twitter".into(),
            platform_type: PlatformType::SocialMedia,
            url: "https://x.com".into(),
            icon: "twitter".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "github".into(),
            name: "GitHub".into(),
            platform_type: PlatformType::Professional,
            url: "https://github.com".into(),
            icon: "github".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "fiverr".into(),
            name: "Fiverr".into(),
            platform_type: PlatformType::Freelancer,
            url: "https://www.fiverr.com".into(),
            icon: "briefcase".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "upwork".into(),
            name: "Upwork".into(),
            platform_type: PlatformType::Freelancer,
            url: "https://www.upwork.com".into(),
            icon: "briefcase".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "facebook".into(),
            name: "Facebook".into(),
            platform_type: PlatformType::SocialMedia,
            url: "https://www.facebook.com".into(),
            icon: "facebook".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "instagram".into(),
            name: "Instagram".into(),
            platform_type: PlatformType::SocialMedia,
            url: "https://www.instagram.com".into(),
            icon: "instagram".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
        ConnectedPlatform {
            id: "tiktok".into(),
            name: "TikTok".into(),
            platform_type: PlatformType::SocialMedia,
            url: "https://www.tiktok.com".into(),
            icon: "video".into(),
            enabled: false,
            auto_sync_profile: false,
            auto_post: false,
            last_synced: None,
            status: PlatformStatus::Disconnected,
            added_at: Utc::now().to_rfc3339(),
        },
    ]
}

// ============================================================================
// CDP AUTOMATION SCRIPTS -- Per-platform step generators
// ============================================================================

/// Generate CDP steps for posting on LinkedIn.
fn linkedin_post_steps(content: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://www.linkedin.com/feed/".into(),
        },
        CdpStep::WaitFor {
            selector: "div.share-box-feed-entry__trigger".into(),
            timeout_ms: 5000,
        },
        CdpStep::Click {
            selector: "div.share-box-feed-entry__trigger".into(),
        },
        CdpStep::WaitFor {
            selector: "div.ql-editor".into(),
            timeout_ms: 3000,
        },
        CdpStep::Fill {
            selector: "div.ql-editor".into(),
            value: content.to_string(),
        },
        CdpStep::Screenshot {
            label: "linkedin-post-preview".into(),
        },
        CdpStep::Click {
            selector: "button.share-actions__primary-action".into(),
        },
        CdpStep::Wait { ms: 2000 },
    ]
}

/// Generate CDP steps for posting on X/Twitter.
fn twitter_post_steps(content: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://x.com/compose/post".into(),
        },
        CdpStep::WaitFor {
            selector: "div[data-testid='tweetTextarea_0']".into(),
            timeout_ms: 5000,
        },
        CdpStep::Fill {
            selector: "div[data-testid='tweetTextarea_0']".into(),
            value: content.to_string(),
        },
        CdpStep::Screenshot {
            label: "twitter-post-preview".into(),
        },
        CdpStep::Click {
            selector: "button[data-testid='tweetButton']".into(),
        },
        CdpStep::Wait { ms: 2000 },
    ]
}

/// Generate CDP steps for updating a GitHub profile README.
fn github_readme_steps(content: &str) -> Vec<CdpStep> {
    // Uses the GitHub web editor to update the profile README
    vec![
        CdpStep::Navigate {
            url: "https://github.com".into(),
        },
        CdpStep::WaitFor {
            selector: "a[href*='/settings/profile']".into(),
            timeout_ms: 5000,
        },
        CdpStep::Navigate {
            url: "https://github.com/settings/profile".into(),
        },
        CdpStep::WaitFor {
            selector: "input#user_profile_bio".into(),
            timeout_ms: 3000,
        },
        CdpStep::Fill {
            selector: "input#user_profile_bio".into(),
            value: content.to_string(),
        },
        CdpStep::Screenshot {
            label: "github-profile-preview".into(),
        },
    ]
}

/// Generate CDP steps for updating Fiverr profile.
fn fiverr_profile_steps(bio: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://www.fiverr.com/users/settings".into(),
        },
        CdpStep::WaitFor {
            selector: "textarea[name='description']".into(),
            timeout_ms: 5000,
        },
        CdpStep::Fill {
            selector: "textarea[name='description']".into(),
            value: bio.to_string(),
        },
        CdpStep::Screenshot {
            label: "fiverr-profile-preview".into(),
        },
    ]
}

/// Generate CDP steps for updating Upwork profile overview.
fn upwork_profile_steps(overview: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://www.upwork.com/freelancers/settings/profile".into(),
        },
        CdpStep::WaitFor {
            selector: "textarea.profile-overview".into(),
            timeout_ms: 5000,
        },
        CdpStep::Fill {
            selector: "textarea.profile-overview".into(),
            value: overview.to_string(),
        },
        CdpStep::Screenshot {
            label: "upwork-profile-preview".into(),
        },
    ]
}

/// Generate automation steps for a given platform and action.
fn generate_steps(platform_id: &str, action_type: &ActionType, content: &str) -> Vec<CdpStep> {
    match (platform_id, action_type) {
        ("linkedin", ActionType::PostContent) => linkedin_post_steps(content),
        ("twitter", ActionType::PostContent) => twitter_post_steps(content),
        ("github", ActionType::UpdateProfile) => github_readme_steps(content),
        ("fiverr", ActionType::UpdateProfile) => fiverr_profile_steps(content),
        ("upwork", ActionType::UpdateProfile) => upwork_profile_steps(content),
        // Generic fallback: navigate to the platform
        (_, ActionType::PostContent) => {
            vec![
                CdpStep::Navigate {
                    url: format!("Platform '{}' does not have built-in post automation. Use the Browser Agent manually.", platform_id),
                },
            ]
        }
        (_, ActionType::UpdateProfile) => {
            vec![
                CdpStep::Navigate {
                    url: format!("Platform '{}' does not have built-in profile automation. Use the Browser Agent manually.", platform_id),
                },
            ]
        }
        _ => vec![],
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// List all platforms (connected and available).
/// On first load, populates with built-in platform definitions.
#[tauri::command]
pub async fn autopub_get_platforms(app: tauri::AppHandle) -> Result<Vec<ConnectedPlatform>, String> {
    ensure_loaded(&app);

    let needs_seed = {
        let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.platforms.is_empty()
    };

    if needs_seed {
        {
            let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
            d.platforms = builtin_platforms();
        }
        persist(&app).await?;
    }

    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    Ok(d.platforms.clone())
}

/// Toggle a platform field (enabled, auto_sync_profile, auto_post).
#[tauri::command]
pub async fn autopub_toggle_platform(
    app: tauri::AppHandle,
    id: String,
    field: String,
    value: bool,
) -> Result<(), String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let platform = d
            .platforms
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Platform not found: {id}"))?;

        match field.as_str() {
            "enabled" => {
                platform.enabled = value;
                platform.status = if value {
                    PlatformStatus::Connected
                } else {
                    PlatformStatus::Disconnected
                };
            }
            "auto_sync_profile" => platform.auto_sync_profile = value,
            "auto_post" => platform.auto_post = value,
            _ => return Err(format!("Unknown field: {field}")),
        }

        let name = platform.name.clone();
        log_action(&mut d, &id, &name, "toggle", "success", &format!("{field} = {value}"));
    }
    persist(&app).await
}

/// Add a custom platform.
#[tauri::command]
pub async fn autopub_add_platform(
    app: tauri::AppHandle,
    name: String,
    url: String,
    platform_type: PlatformType,
) -> Result<ConnectedPlatform, String> {
    ensure_loaded(&app);

    if name.trim().is_empty() {
        return Err("Platform name cannot be empty".into());
    }

    let platform = ConnectedPlatform {
        id: format!("custom-{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("0000")),
        name: name.clone(),
        platform_type,
        url,
        icon: "globe".into(),
        enabled: false,
        auto_sync_profile: false,
        auto_post: false,
        last_synced: None,
        status: PlatformStatus::Disconnected,
        added_at: Utc::now().to_rfc3339(),
    };

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.platforms.push(platform.clone());
        log_action(&mut d, &platform.id, &name, "add_platform", "success", "Custom platform added");
    }
    persist(&app).await?;
    Ok(platform)
}

/// Remove a platform by ID.
#[tauri::command]
pub async fn autopub_remove_platform(app: tauri::AppHandle, id: String) -> Result<(), String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let name = d
            .platforms
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.clone());
        d.platforms.retain(|p| p.id != id);
        log_action(&mut d, &id, &name, "remove_platform", "success", "Platform removed");
    }
    persist(&app).await
}

/// Publish content to selected platforms.
///
/// For each platform, generates a CDP automation script and queues it
/// for user review. The script is NOT executed automatically -- the user
/// must approve it first (requires_confirmation = true by default).
#[tauri::command]
pub async fn autopub_publish(
    app: tauri::AppHandle,
    content: String,
    platform_ids: Vec<String>,
) -> Result<Vec<PublishResult>, String> {
    ensure_loaded(&app);

    if content.trim().is_empty() {
        return Err("Content cannot be empty".into());
    }
    if platform_ids.is_empty() {
        return Err("Select at least one platform".into());
    }

    let mut results = Vec::new();

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;

        for pid in &platform_ids {
            let platform = match d.platforms.iter().find(|p| p.id == *pid && p.enabled) {
                Some(p) => p.clone(),
                None => {
                    results.push(PublishResult {
                        platform_id: pid.clone(),
                        platform_name: pid.clone(),
                        success: false,
                        script_id: None,
                        message: format!("Platform '{pid}' not found or not enabled"),
                    });
                    continue;
                }
            };

            let steps = generate_steps(pid, &ActionType::PostContent, &content);
            let script_id = Uuid::new_v4().to_string();

            let preview = if content.len() > 100 {
                format!("{}...", &content[..100])
            } else {
                content.clone()
            };

            let script = QueuedScript {
                id: script_id.clone(),
                platform_id: pid.clone(),
                platform_name: platform.name.clone(),
                action_type: ActionType::PostContent,
                steps,
                content_preview: preview,
                status: ScriptStatus::Pending,
                created_at: Utc::now().to_rfc3339(),
                executed_at: None,
                error: None,
            };

            d.scripts.push(script);
            log_action(
                &mut d,
                pid,
                &platform.name,
                "publish",
                "queued",
                &format!("Content queued for review ({} chars)", content.len()),
            );

            results.push(PublishResult {
                platform_id: pid.clone(),
                platform_name: platform.name,
                success: true,
                script_id: Some(script_id),
                message: "Automation script queued for review".into(),
            });
        }
    }

    persist(&app).await?;
    Ok(results)
}

/// Sync the user's profile to selected platforms via CDP scripts.
#[tauri::command]
pub async fn autopub_sync_profile(
    app: tauri::AppHandle,
    platform_ids: Vec<String>,
    profile_data: serde_json::Value,
) -> Result<Vec<SyncResult>, String> {
    ensure_loaded(&app);

    let bio = profile_data
        .get("bio")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut results = Vec::new();

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;

        for pid in &platform_ids {
            let platform = match d.platforms.iter_mut().find(|p| p.id == *pid && p.enabled) {
                Some(p) => p,
                None => {
                    results.push(SyncResult {
                        platform_id: pid.clone(),
                        platform_name: pid.clone(),
                        success: false,
                        fields_synced: vec![],
                        message: format!("Platform '{pid}' not found or not enabled"),
                    });
                    continue;
                }
            };

            let steps = generate_steps(pid, &ActionType::UpdateProfile, &bio);
            let script_id = Uuid::new_v4().to_string();

            let script = QueuedScript {
                id: script_id.clone(),
                platform_id: pid.clone(),
                platform_name: platform.name.clone(),
                action_type: ActionType::UpdateProfile,
                steps,
                content_preview: format!("Profile sync: bio ({} chars)", bio.len()),
                status: ScriptStatus::Pending,
                created_at: Utc::now().to_rfc3339(),
                executed_at: None,
                error: None,
            };

            platform.last_synced = Some(Utc::now().to_rfc3339());
            platform.status = PlatformStatus::Connected;

            let name = platform.name.clone();
            d.scripts.push(script);
            log_action(&mut d, pid, &name, "sync_profile", "queued", "Profile sync script queued");

            results.push(SyncResult {
                platform_id: pid.clone(),
                platform_name: name,
                success: true,
                fields_synced: vec!["bio".into()],
                message: "Profile sync script queued for review".into(),
            });
        }
    }

    persist(&app).await?;
    Ok(results)
}

/// Execute a single automation action (generates and queues a script).
#[tauri::command]
pub async fn autopub_execute_action(
    app: tauri::AppHandle,
    action: AutoAction,
) -> Result<PublishResult, String> {
    ensure_loaded(&app);

    let content_str = action
        .content
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| action.content.get("body").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let result;
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;

        let platform = d
            .platforms
            .iter()
            .find(|p| p.id == action.platform_id)
            .ok_or_else(|| format!("Platform not found: {}", action.platform_id))?
            .clone();

        let steps = generate_steps(&action.platform_id, &action.action_type, &content_str);
        let script_id = Uuid::new_v4().to_string();

        let action_name = format!("{:?}", action.action_type);

        let script = QueuedScript {
            id: script_id.clone(),
            platform_id: action.platform_id.clone(),
            platform_name: platform.name.clone(),
            action_type: action.action_type,
            steps,
            content_preview: if content_str.len() > 100 {
                format!("{}...", &content_str[..100])
            } else {
                content_str
            },
            status: if action.requires_confirmation {
                ScriptStatus::Pending
            } else {
                ScriptStatus::Approved
            },
            created_at: Utc::now().to_rfc3339(),
            executed_at: None,
            error: None,
        };

        d.scripts.push(script);
        log_action(
            &mut d,
            &action.platform_id,
            &platform.name,
            &action_name,
            "queued",
            "Action script queued",
        );

        result = PublishResult {
            platform_id: action.platform_id,
            platform_name: platform.name,
            success: true,
            script_id: Some(script_id),
            message: "Automation script queued".into(),
        };
    }

    persist(&app).await?;
    Ok(result)
}

/// Get the automation log (last 100 entries).
#[tauri::command]
pub async fn autopub_get_log(app: tauri::AppHandle) -> Result<Vec<AutomationLogEntry>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut log = d.log.clone();
    log.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    log.truncate(100);
    Ok(log)
}

/// Get all queued scripts (pending review).
#[tauri::command]
pub async fn autopub_get_scripts(app: tauri::AppHandle) -> Result<Vec<QueuedScript>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut scripts = d.scripts.clone();
    scripts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(scripts)
}

/// Approve or cancel a queued script.
#[tauri::command]
pub async fn autopub_update_script(
    app: tauri::AppHandle,
    script_id: String,
    new_status: String,
) -> Result<QueuedScript, String> {
    ensure_loaded(&app);
    let script;
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let s = d
            .scripts
            .iter_mut()
            .find(|s| s.id == script_id)
            .ok_or_else(|| format!("Script not found: {script_id}"))?;

        s.status = match new_status.as_str() {
            "approved" => ScriptStatus::Approved,
            "cancelled" => ScriptStatus::Cancelled,
            "completed" => {
                s.executed_at = Some(Utc::now().to_rfc3339());
                ScriptStatus::Completed
            }
            "failed" => ScriptStatus::Failed,
            _ => return Err(format!("Invalid status: {new_status}")),
        };

        let platform_id = s.platform_id.clone();
        let platform_name = s.platform_name.clone();

        log_action(
            &mut d,
            &platform_id,
            &platform_name,
            "script_update",
            &new_status,
            &format!("Script {} updated to {}", script_id, new_status),
        );

        script = d
            .scripts
            .iter()
            .find(|s| s.id == script_id)
            .cloned()
            .unwrap();
    }
    persist(&app).await?;
    Ok(script)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_platforms_count() {
        let platforms = builtin_platforms();
        assert_eq!(platforms.len(), 8);
        assert!(platforms.iter().any(|p| p.id == "linkedin"));
        assert!(platforms.iter().any(|p| p.id == "twitter"));
        assert!(platforms.iter().any(|p| p.id == "github"));
        assert!(platforms.iter().any(|p| p.id == "fiverr"));
        assert!(platforms.iter().any(|p| p.id == "upwork"));
    }

    #[test]
    fn test_linkedin_post_steps() {
        let steps = linkedin_post_steps("Hello World");
        assert!(steps.len() >= 5);
        match &steps[0] {
            CdpStep::Navigate { url } => assert!(url.contains("linkedin.com")),
            _ => panic!("Expected Navigate step"),
        }
    }

    #[test]
    fn test_twitter_post_steps() {
        let steps = twitter_post_steps("Test tweet");
        assert!(!steps.is_empty());
        match &steps[0] {
            CdpStep::Navigate { url } => assert!(url.contains("x.com")),
            _ => panic!("Expected Navigate step"),
        }
    }

    #[test]
    fn test_github_readme_steps() {
        let steps = github_readme_steps("New bio");
        assert!(!steps.is_empty());
    }

    #[test]
    fn test_fiverr_profile_steps() {
        let steps = fiverr_profile_steps("I build AI apps");
        assert!(!steps.is_empty());
        match &steps[0] {
            CdpStep::Navigate { url } => assert!(url.contains("fiverr.com")),
            _ => panic!("Expected Navigate step"),
        }
    }

    #[test]
    fn test_upwork_profile_steps() {
        let steps = upwork_profile_steps("Full stack developer");
        assert!(!steps.is_empty());
        match &steps[0] {
            CdpStep::Navigate { url } => assert!(url.contains("upwork.com")),
            _ => panic!("Expected Navigate step"),
        }
    }

    #[test]
    fn test_generate_steps_unknown_platform() {
        let steps = generate_steps("unknown-platform", &ActionType::PostContent, "test");
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn test_platform_status_serialization() {
        let status = PlatformStatus::Error("timeout".into());
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Error"));
        assert!(json.contains("timeout"));
    }

    #[test]
    fn test_connected_platform_serialization() {
        let p = ConnectedPlatform {
            id: "test".into(),
            name: "Test".into(),
            platform_type: PlatformType::Custom,
            url: "https://example.com".into(),
            icon: "globe".into(),
            enabled: true,
            auto_sync_profile: false,
            auto_post: true,
            last_synced: None,
            status: PlatformStatus::Connected,
            added_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: ConnectedPlatform = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test");
        assert!(parsed.enabled);
        assert!(parsed.auto_post);
    }

    #[test]
    fn test_cdp_step_serialization() {
        let step = CdpStep::Fill {
            selector: "#bio".into(),
            value: "Hello".into(),
        };
        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("Fill"));
        assert!(json.contains("#bio"));
    }

    #[test]
    fn test_queued_script_serialization() {
        let script = QueuedScript {
            id: "s1".into(),
            platform_id: "linkedin".into(),
            platform_name: "LinkedIn".into(),
            action_type: ActionType::PostContent,
            steps: vec![CdpStep::Navigate {
                url: "https://linkedin.com".into(),
            }],
            content_preview: "Test".into(),
            status: ScriptStatus::Pending,
            created_at: "2026-01-01T00:00:00Z".into(),
            executed_at: None,
            error: None,
        };
        let json = serde_json::to_string(&script).unwrap();
        let parsed: QueuedScript = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, ScriptStatus::Pending);
    }

    #[test]
    fn test_log_action_caps_at_500() {
        let mut data = PlatformData::default();
        for i in 0..600 {
            log_action(&mut data, "test", "Test", "action", "ok", &format!("Entry {i}"));
        }
        assert_eq!(data.log.len(), 500);
    }

    #[test]
    fn test_publish_result_serialization() {
        let r = PublishResult {
            platform_id: "linkedin".into(),
            platform_name: "LinkedIn".into(),
            success: true,
            script_id: Some("abc-123".into()),
            message: "Queued".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("abc-123"));
    }

    #[test]
    fn test_automation_log_entry() {
        let entry = AutomationLogEntry {
            id: "1".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            platform_id: "github".into(),
            platform_name: "GitHub".into(),
            action_type: "publish".into(),
            status: "success".into(),
            details: "Content posted".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: AutomationLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.platform_name, "GitHub");
    }
}
