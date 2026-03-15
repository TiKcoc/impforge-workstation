// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Browser Agent Auto-Import — CDP-powered data import from external services.
//!
//! Turns ImpForge into a universal data aggregator: automatically export data
//! from external websites (Google Calendar, Outlook, Office 365, Google Sheets,
//! Google Drive, OneDrive, Dropbox, RSS feeds, etc.) using the built-in Browser
//! Agent (CDP) and import it into the appropriate Forge module.
//!
//! Architecture:
//! - Each source type has pre-built CDP automation scripts
//! - Users can preview steps before execution
//! - Auto-import runs on a configurable schedule (default: every 24h)
//! - All import history is logged for transparency
//!
//! Storage: tauri-plugin-store `.impforge-auto-import.json`
//! License: Apache-2.0 (commercial distribution safe)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

// Re-use the CdpStep enum from auto_publisher for consistency
use crate::auto_publisher::CdpStep;

// ============================================================================
// TYPES — Import Source & Job Models
// ============================================================================

/// The type of external service to import data from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceType {
    GoogleCalendar,
    OutlookCalendar,
    GoogleDrive,
    OneDrive,
    Dropbox,
    GoogleSheets,
    Office365,
    GenericWebsite,
    RssFeed,
    Custom,
}

impl ImportSourceType {
    /// Human-readable label for display in the UI.
    pub fn label(&self) -> &str {
        match self {
            Self::GoogleCalendar => "Google Calendar",
            Self::OutlookCalendar => "Outlook Calendar",
            Self::GoogleDrive => "Google Drive",
            Self::OneDrive => "OneDrive",
            Self::Dropbox => "Dropbox",
            Self::GoogleSheets => "Google Sheets",
            Self::Office365 => "Office 365",
            Self::GenericWebsite => "Generic Website",
            Self::RssFeed => "RSS Feed",
            Self::Custom => "Custom",
        }
    }

    /// Icon hint for the frontend (lucide icon name).
    pub fn icon(&self) -> &str {
        match self {
            Self::GoogleCalendar => "calendar",
            Self::OutlookCalendar => "calendar",
            Self::GoogleDrive => "hard-drive",
            Self::OneDrive => "cloud",
            Self::Dropbox => "box",
            Self::GoogleSheets => "table-2",
            Self::Office365 => "file-text",
            Self::GenericWebsite => "globe",
            Self::RssFeed => "rss",
            Self::Custom => "settings",
        }
    }

    /// Default target module for this source type.
    pub fn default_target(&self) -> &str {
        match self {
            Self::GoogleCalendar | Self::OutlookCalendar => "calendar",
            Self::GoogleDrive | Self::OneDrive | Self::Dropbox => "files",
            Self::GoogleSheets => "sheets",
            Self::Office365 => "writer",
            Self::GenericWebsite => "files",
            Self::RssFeed => "news",
            Self::Custom => "files",
        }
    }
}

/// Current status of an import source or job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "message")]
pub enum ImportStatus {
    Idle,
    Importing,
    Success,
    Failed(String),
}

/// A configured import source that can be triggered manually or on schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSource {
    pub id: String,
    pub name: String,
    pub source_type: ImportSourceType,
    pub url: String,
    pub auto_import: bool,
    pub import_interval_hours: u32,
    pub last_imported: Option<String>,
    pub status: ImportStatus,
    pub target_module: String,
    pub created_at: String,
}

/// A single import job execution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJob {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: ImportStatus,
    pub items_imported: u32,
    pub target_module: String,
    pub automation_steps: Vec<CdpStep>,
    pub error_details: Option<String>,
}

// ============================================================================
// PERSISTENT STORE
// ============================================================================

/// Top-level container serialised to `.impforge-auto-import.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ImportData {
    sources: Vec<ImportSource>,
    history: Vec<ImportJob>,
}

static DATA: once_cell::sync::Lazy<Mutex<ImportData>> =
    once_cell::sync::Lazy::new(|| Mutex::new(ImportData::default()));

/// Maximum history entries to retain (FIFO eviction).
const MAX_HISTORY: usize = 200;

/// Store filename for tauri-plugin-store.
const STORE_FILE: &str = ".impforge-auto-import.json";

/// Store key within the file.
const STORE_KEY: &str = "import_data";

async fn persist(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Store error: {e}"))?;
    let data = DATA.lock().map_err(|e| format!("Lock error: {e}"))?;
    store.set(
        STORE_KEY,
        serde_json::to_value(&*data).map_err(|e| format!("Serialize: {e}"))?,
    );
    store.save().map_err(|e| format!("Save: {e}"))?;
    Ok(())
}

fn ensure_loaded(app: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;
    let already = {
        let d = DATA.lock().unwrap_or_else(|e| e.into_inner());
        !d.sources.is_empty() || !d.history.is_empty()
    };
    if already {
        return;
    }

    if let Ok(store) = app.store(STORE_FILE) {
        if let Some(val) = store.get(STORE_KEY) {
            if let Ok(loaded) = serde_json::from_value::<ImportData>(val.clone()) {
                let mut d = DATA.lock().unwrap_or_else(|e| e.into_inner());
                *d = loaded;
            }
        }
    }
}

// ============================================================================
// CDP AUTOMATION SCRIPTS — Pre-built import step generators
// ============================================================================

/// Generate CDP steps to export a Google Calendar as ICS.
///
/// Flow: navigate to Google Calendar settings export page, click export,
/// wait for download. The resulting ZIP contains .ics files.
fn google_calendar_export_steps() -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://calendar.google.com/calendar/r/settings/export".into(),
        },
        CdpStep::WaitFor {
            selector: "div[data-view-heading='Export']".into(),
            timeout_ms: 8000,
        },
        CdpStep::Screenshot {
            label: "google-calendar-export-page".into(),
        },
        CdpStep::Click {
            selector: "button[aria-label='Export'], a[data-action='export']".into(),
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "google-calendar-export-complete".into(),
        },
    ]
}

/// Generate CDP steps to export an Outlook/Office365 calendar as ICS.
///
/// Flow: navigate to Outlook calendar sharing settings, generate ICS link,
/// download the file.
fn outlook_calendar_export_steps() -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://outlook.live.com/calendar/0/options/calendar/SharedCalendars".into(),
        },
        CdpStep::WaitFor {
            selector: "div[role='main']".into(),
            timeout_ms: 8000,
        },
        CdpStep::Screenshot {
            label: "outlook-calendar-settings".into(),
        },
        // Look for "Publish a calendar" section
        CdpStep::Click {
            selector: "button[aria-label='Publish a calendar'], a[href*='publish']".into(),
        },
        CdpStep::Wait { ms: 2000 },
        // Extract the ICS link
        CdpStep::ExecuteJs {
            script: r#"
                const link = document.querySelector('input[value*=".ics"], a[href*=".ics"]');
                if (link) {
                    const url = link.value || link.href;
                    window.__impforge_ics_url = url;
                    return url;
                }
                return 'no-ics-link-found';
            "#.into(),
        },
        CdpStep::Screenshot {
            label: "outlook-calendar-ics-link".into(),
        },
    ]
}

/// Generate CDP steps to export a Google Sheet as CSV.
///
/// Flow: navigate to the sheet, trigger File > Download > CSV.
fn google_sheets_export_steps(sheet_url: &str) -> Vec<CdpStep> {
    // Google Sheets export URL pattern: append /export?format=csv
    let export_url = if sheet_url.contains("/edit") {
        sheet_url.replace("/edit", "/export?format=csv")
    } else {
        format!("{}/export?format=csv", sheet_url.trim_end_matches('/'))
    };

    vec![
        CdpStep::Navigate {
            url: sheet_url.to_string(),
        },
        CdpStep::WaitFor {
            selector: "div#docs-editor".into(),
            timeout_ms: 10000,
        },
        CdpStep::Screenshot {
            label: "google-sheets-loaded".into(),
        },
        // Direct CSV export via URL
        CdpStep::Navigate {
            url: export_url,
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "google-sheets-csv-download".into(),
        },
    ]
}

/// Generate CDP steps to download a file from Google Drive.
///
/// Flow: navigate to the file, click download button.
fn google_drive_download_steps(file_url: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: file_url.to_string(),
        },
        CdpStep::WaitFor {
            selector: "div[data-target='doc']".into(),
            timeout_ms: 10000,
        },
        CdpStep::Screenshot {
            label: "google-drive-file-preview".into(),
        },
        // Click the download button in the toolbar
        CdpStep::Click {
            selector: "div[aria-label='Download'], button[aria-label='Download']".into(),
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "google-drive-download-started".into(),
        },
    ]
}

/// Generate CDP steps to export documents from Office 365.
///
/// Flow: navigate to the document, use File > Download a Copy.
fn office365_export_steps() -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: "https://www.office.com/launch/word".into(),
        },
        CdpStep::WaitFor {
            selector: "div[role='main']".into(),
            timeout_ms: 10000,
        },
        CdpStep::Screenshot {
            label: "office365-word-landing".into(),
        },
        // Click on recent documents list
        CdpStep::Click {
            selector: "div[data-automationid='FileList'] div[role='row']:first-child".into(),
        },
        CdpStep::Wait { ms: 3000 },
        // Open File menu
        CdpStep::Click {
            selector: "button[name='File'], button[aria-label='File']".into(),
        },
        CdpStep::WaitFor {
            selector: "button[name='Save As'], button[name='Download a Copy']".into(),
            timeout_ms: 3000,
        },
        CdpStep::Click {
            selector: "button[name='Download a Copy'], button[aria-label='Download a Copy']".into(),
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "office365-document-downloaded".into(),
        },
    ]
}

/// Generate CDP steps to download a file from OneDrive.
fn onedrive_download_steps(file_url: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: file_url.to_string(),
        },
        CdpStep::WaitFor {
            selector: "div[role='main']".into(),
            timeout_ms: 8000,
        },
        CdpStep::Screenshot {
            label: "onedrive-file-view".into(),
        },
        CdpStep::Click {
            selector: "button[data-automationid='downloadCommand'], button[aria-label='Download']".into(),
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "onedrive-download-complete".into(),
        },
    ]
}

/// Generate CDP steps to download a file from Dropbox.
fn dropbox_download_steps(file_url: &str) -> Vec<CdpStep> {
    // Dropbox direct download: replace dl=0 with dl=1
    let direct_url = if file_url.contains("dl=0") {
        file_url.replace("dl=0", "dl=1")
    } else if file_url.contains("dropbox.com") && !file_url.contains("dl=1") {
        format!("{}?dl=1", file_url.trim_end_matches('/'))
    } else {
        file_url.to_string()
    };

    vec![
        CdpStep::Navigate {
            url: file_url.to_string(),
        },
        CdpStep::WaitFor {
            selector: "div.preview-container, div[class*='preview']".into(),
            timeout_ms: 8000,
        },
        CdpStep::Screenshot {
            label: "dropbox-file-preview".into(),
        },
        CdpStep::Navigate {
            url: direct_url,
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "dropbox-download-complete".into(),
        },
    ]
}

/// Generate CDP steps to fetch an RSS feed.
///
/// RSS feeds are XML, so we extract content via JavaScript.
fn rss_feed_fetch_steps(feed_url: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: feed_url.to_string(),
        },
        CdpStep::Wait { ms: 2000 },
        CdpStep::ExecuteJs {
            script: r#"
                // Extract RSS/Atom items from the page XML
                const items = document.querySelectorAll('item, entry');
                const results = [];
                for (const item of items) {
                    const title = item.querySelector('title');
                    const link = item.querySelector('link');
                    const desc = item.querySelector('description, summary, content');
                    const pubDate = item.querySelector('pubDate, published, updated');
                    results.push({
                        title: title ? title.textContent : '',
                        link: link ? (link.getAttribute('href') || link.textContent) : '',
                        description: desc ? desc.textContent : '',
                        date: pubDate ? pubDate.textContent : ''
                    });
                }
                window.__impforge_rss_items = JSON.stringify(results);
                return JSON.stringify({ count: results.length });
            "#.into(),
        },
        CdpStep::Screenshot {
            label: "rss-feed-fetched".into(),
        },
    ]
}

/// Generate CDP steps for a generic file download.
fn generic_download_steps(url: &str) -> Vec<CdpStep> {
    vec![
        CdpStep::Navigate {
            url: url.to_string(),
        },
        CdpStep::Wait { ms: 3000 },
        CdpStep::Screenshot {
            label: "generic-page-loaded".into(),
        },
        CdpStep::ExecuteJs {
            script: r#"
                // Try to find downloadable content on the page
                const links = document.querySelectorAll('a[download], a[href$=".csv"], a[href$=".xlsx"], a[href$=".pdf"], a[href$=".ics"], a[href$=".json"]');
                const results = [];
                for (const a of links) {
                    results.push({
                        text: a.textContent.trim(),
                        href: a.href,
                        download: a.download || ''
                    });
                }
                window.__impforge_download_links = JSON.stringify(results);
                return JSON.stringify({ links_found: results.length });
            "#.into(),
        },
        CdpStep::Screenshot {
            label: "generic-download-links".into(),
        },
    ]
}

/// Select the appropriate CDP steps for a given source type and URL.
fn steps_for_source(source_type: &ImportSourceType, url: &str) -> Vec<CdpStep> {
    match source_type {
        ImportSourceType::GoogleCalendar => google_calendar_export_steps(),
        ImportSourceType::OutlookCalendar => outlook_calendar_export_steps(),
        ImportSourceType::GoogleSheets => google_sheets_export_steps(url),
        ImportSourceType::GoogleDrive => google_drive_download_steps(url),
        ImportSourceType::OneDrive => onedrive_download_steps(url),
        ImportSourceType::Dropbox => dropbox_download_steps(url),
        ImportSourceType::Office365 => office365_export_steps(),
        ImportSourceType::RssFeed => rss_feed_fetch_steps(url),
        ImportSourceType::GenericWebsite | ImportSourceType::Custom => generic_download_steps(url),
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// List all configured import sources.
#[tauri::command]
pub async fn autoimport_list_sources(app: tauri::AppHandle) -> Result<Vec<ImportSource>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    Ok(d.sources.clone())
}

/// Add a new import source.
#[tauri::command]
pub async fn autoimport_add_source(
    app: tauri::AppHandle,
    name: String,
    source_type: ImportSourceType,
    url: String,
    target_module: Option<String>,
) -> Result<ImportSource, String> {
    ensure_loaded(&app);

    let target = target_module.unwrap_or_else(|| source_type.default_target().to_string());

    let source = ImportSource {
        id: Uuid::new_v4().to_string(),
        name,
        source_type,
        url,
        auto_import: false,
        import_interval_hours: 24,
        last_imported: None,
        status: ImportStatus::Idle,
        target_module: target,
        created_at: Utc::now().to_rfc3339(),
    };

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.sources.push(source.clone());
    }

    persist(&app).await?;
    Ok(source)
}

/// Remove an import source by ID.
#[tauri::command]
pub async fn autoimport_remove_source(
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    ensure_loaded(&app);

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let before_len = d.sources.len();
        d.sources.retain(|s| s.id != id);
        if d.sources.len() == before_len {
            return Err(format!("Import source '{id}' not found"));
        }
    }

    persist(&app).await?;
    Ok(())
}

/// Toggle auto-import on/off for a source and optionally set the interval.
#[tauri::command]
pub async fn autoimport_toggle(
    app: tauri::AppHandle,
    id: String,
    auto_import: bool,
    interval_hours: Option<u32>,
) -> Result<ImportSource, String> {
    ensure_loaded(&app);

    let updated = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let source = d
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Import source '{id}' not found"))?;

        source.auto_import = auto_import;
        if let Some(hours) = interval_hours {
            source.import_interval_hours = hours.max(1); // minimum 1 hour
        }
        source.clone()
    };

    persist(&app).await?;
    Ok(updated)
}

/// Update the target module for a source.
#[tauri::command]
pub async fn autoimport_set_target(
    app: tauri::AppHandle,
    id: String,
    target_module: String,
) -> Result<ImportSource, String> {
    ensure_loaded(&app);

    let updated = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let source = d
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Import source '{id}' not found"))?;

        source.target_module = target_module;
        source.clone()
    };

    persist(&app).await?;
    Ok(updated)
}

/// Get the pre-built CDP automation steps for a given source type (preview).
#[tauri::command]
pub async fn autoimport_get_steps(
    source_type: ImportSourceType,
    url: Option<String>,
) -> Result<Vec<CdpStep>, String> {
    let url_str = url.as_deref().unwrap_or("");
    Ok(steps_for_source(&source_type, url_str))
}

/// Execute an import job for a given source.
///
/// This generates the CDP steps, records the job, simulates execution
/// (actual CDP execution is handled by the CDP engine on the frontend side),
/// and updates the source status.
#[tauri::command]
pub async fn autoimport_run(
    app: tauri::AppHandle,
    source_id: String,
) -> Result<ImportJob, String> {
    ensure_loaded(&app);

    // Look up the source
    let (source_name, source_type, url, target_module) = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let source = d
            .sources
            .iter_mut()
            .find(|s| s.id == source_id)
            .ok_or_else(|| format!("Import source '{source_id}' not found"))?;

        // Mark as importing
        source.status = ImportStatus::Importing;

        (
            source.name.clone(),
            source.source_type.clone(),
            source.url.clone(),
            source.target_module.clone(),
        )
    };

    // Generate the automation steps
    let steps = steps_for_source(&source_type, &url);

    // Create the job record
    let job = ImportJob {
        id: Uuid::new_v4().to_string(),
        source_id: source_id.clone(),
        source_name: source_name.clone(),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
        status: ImportStatus::Importing,
        items_imported: 0,
        target_module: target_module.clone(),
        automation_steps: steps,
        error_details: None,
    };

    // Record the job in history
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.history.push(job.clone());

        // Evict old history entries
        if d.history.len() > MAX_HISTORY {
            let drain_count = d.history.len() - MAX_HISTORY;
            d.history.drain(..drain_count);
        }
    }

    persist(&app).await?;
    Ok(job)
}

/// Mark a running import job as completed (called by the frontend after
/// CDP steps have been executed and the downloaded file has been processed).
#[tauri::command]
pub async fn autoimport_complete(
    app: tauri::AppHandle,
    job_id: String,
    success: bool,
    items_imported: u32,
    error_message: Option<String>,
) -> Result<ImportJob, String> {
    ensure_loaded(&app);

    let updated = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;

        // Update the job
        let job = d
            .history
            .iter_mut()
            .find(|j| j.id == job_id)
            .ok_or_else(|| format!("Import job '{job_id}' not found"))?;

        let now = Utc::now().to_rfc3339();
        job.completed_at = Some(now.clone());
        job.items_imported = items_imported;
        job.status = if success {
            ImportStatus::Success
        } else {
            ImportStatus::Failed(error_message.clone().unwrap_or_else(|| "Unknown error".into()))
        };
        job.error_details = error_message.clone();

        let source_id = job.source_id.clone();
        let job_clone = job.clone();

        // Update the source status
        if let Some(source) = d.sources.iter_mut().find(|s| s.id == source_id) {
            source.last_imported = Some(now);
            source.status = if success {
                ImportStatus::Success
            } else {
                ImportStatus::Failed(error_message.unwrap_or_else(|| "Unknown error".into()))
            };
        }

        job_clone
    };

    persist(&app).await?;
    Ok(updated)
}

/// Get import history, most recent first.
#[tauri::command]
pub async fn autoimport_history(
    app: tauri::AppHandle,
    limit: Option<u32>,
) -> Result<Vec<ImportJob>, String> {
    ensure_loaded(&app);

    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let max = limit.unwrap_or(50) as usize;

    let mut jobs: Vec<ImportJob> = d.history.clone();
    jobs.reverse(); // most recent first
    jobs.truncate(max);
    Ok(jobs)
}

/// Get all available source type definitions (for the "Add Source" dialog).
#[tauri::command]
pub async fn autoimport_source_types() -> Result<Vec<serde_json::Value>, String> {
    let types = vec![
        ImportSourceType::GoogleCalendar,
        ImportSourceType::OutlookCalendar,
        ImportSourceType::GoogleDrive,
        ImportSourceType::OneDrive,
        ImportSourceType::Dropbox,
        ImportSourceType::GoogleSheets,
        ImportSourceType::Office365,
        ImportSourceType::GenericWebsite,
        ImportSourceType::RssFeed,
        ImportSourceType::Custom,
    ];

    let result: Vec<serde_json::Value> = types
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": t,
                "label": t.label(),
                "icon": t.icon(),
                "default_target": t.default_target(),
            })
        })
        .collect();

    Ok(result)
}

/// Get available target modules for the import destination picker.
#[tauri::command]
pub async fn autoimport_target_modules() -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![
        serde_json::json!({ "id": "calendar", "label": "ForgeCalendar", "icon": "calendar-days" }),
        serde_json::json!({ "id": "sheets", "label": "ForgeSheets", "icon": "table-2" }),
        serde_json::json!({ "id": "writer", "label": "ForgeWriter", "icon": "file-edit" }),
        serde_json::json!({ "id": "pdf", "label": "ForgePDF", "icon": "file-text" }),
        serde_json::json!({ "id": "files", "label": "File Hub", "icon": "folder-open" }),
        serde_json::json!({ "id": "news", "label": "AI News", "icon": "newspaper" }),
        serde_json::json!({ "id": "canvas", "label": "ForgeCanvas", "icon": "pen-tool" }),
    ])
}

/// Reset a source status back to Idle (e.g. after a failed import).
#[tauri::command]
pub async fn autoimport_reset_status(
    app: tauri::AppHandle,
    id: String,
) -> Result<ImportSource, String> {
    ensure_loaded(&app);

    let updated = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let source = d
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Import source '{id}' not found"))?;
        source.status = ImportStatus::Idle;
        source.clone()
    };

    persist(&app).await?;
    Ok(updated)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_labels() {
        assert_eq!(ImportSourceType::GoogleCalendar.label(), "Google Calendar");
        assert_eq!(ImportSourceType::RssFeed.label(), "RSS Feed");
        assert_eq!(ImportSourceType::Custom.label(), "Custom");
    }

    #[test]
    fn test_source_type_defaults() {
        assert_eq!(ImportSourceType::GoogleCalendar.default_target(), "calendar");
        assert_eq!(ImportSourceType::GoogleSheets.default_target(), "sheets");
        assert_eq!(ImportSourceType::RssFeed.default_target(), "news");
        assert_eq!(ImportSourceType::GenericWebsite.default_target(), "files");
    }

    #[test]
    fn test_google_calendar_steps() {
        let steps = google_calendar_export_steps();
        assert!(!steps.is_empty());
        // First step should navigate to Google Calendar export
        match &steps[0] {
            CdpStep::Navigate { url } => {
                assert!(url.contains("calendar.google.com"));
            }
            _ => panic!("First step should be Navigate"),
        }
    }

    #[test]
    fn test_google_sheets_steps_with_edit_url() {
        let steps = google_sheets_export_steps(
            "https://docs.google.com/spreadsheets/d/abc123/edit#gid=0",
        );
        assert!(!steps.is_empty());
        // Should have a step that navigates to the export URL
        let has_export = steps.iter().any(|s| match s {
            CdpStep::Navigate { url } => url.contains("export?format=csv"),
            _ => false,
        });
        assert!(has_export, "Should contain CSV export URL");
    }

    #[test]
    fn test_dropbox_direct_download_url() {
        let steps = dropbox_download_steps("https://www.dropbox.com/s/abc123/file.csv?dl=0");
        let has_direct = steps.iter().any(|s| match s {
            CdpStep::Navigate { url } => url.contains("dl=1"),
            _ => false,
        });
        assert!(has_direct, "Should convert dl=0 to dl=1");
    }

    #[test]
    fn test_steps_for_source_routing() {
        let types_and_expectations: Vec<(ImportSourceType, &str)> = vec![
            (ImportSourceType::GoogleCalendar, "calendar.google.com"),
            (ImportSourceType::OutlookCalendar, "outlook.live.com"),
            (ImportSourceType::Office365, "office.com"),
        ];

        for (source_type, expected_domain) in types_and_expectations {
            let steps = steps_for_source(&source_type, "");
            let has_domain = steps.iter().any(|s| match s {
                CdpStep::Navigate { url } => url.contains(expected_domain),
                _ => false,
            });
            assert!(
                has_domain,
                "{:?} should navigate to {}",
                source_type, expected_domain
            );
        }
    }

    #[test]
    fn test_rss_feed_steps_have_js_extraction() {
        let steps = rss_feed_fetch_steps("https://example.com/feed.xml");
        let has_js = steps.iter().any(|s| matches!(s, CdpStep::ExecuteJs { .. }));
        assert!(has_js, "RSS steps should include JS extraction");
    }

    #[test]
    fn test_import_status_serialization() {
        let idle = serde_json::to_string(&ImportStatus::Idle).unwrap_or_default();
        assert!(idle.contains("Idle"));

        let failed = serde_json::to_string(&ImportStatus::Failed("timeout".into()))
            .unwrap_or_default();
        assert!(failed.contains("timeout"));
    }
}
