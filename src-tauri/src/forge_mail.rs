// SPDX-License-Identifier: Apache-2.0
//! ForgeMail -- AI-Powered Email Client
//!
//! Provides email account management, inbox browsing, AI-powered compose/reply,
//! and email categorization. Emails are cached in `~/.impforge/mail/` as JSON.
//! For the MVP, actual IMAP/SMTP is deferred -- the Browser Agent can open
//! Gmail/Outlook in ImpForge's WebView, and drafts are stored locally.
//!
//! This module is part of ImpForge Phase 3 (Office/Communication tools).

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds mail data.
const MAIL_DIR: &str = "mail";

/// Subdirectory for email message cache files.
const EMAILS_DIR: &str = "emails";

/// Subdirectory for draft messages.
const DRAFTS_DIR: &str = "drafts";

/// Accounts registry file name.
const ACCOUNTS_FILE: &str = "accounts.json";

/// Ollama HTTP timeout for AI compose/categorize requests.
const AI_COMPOSE_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported email providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmailProvider {
    Gmail,
    Outlook,
    Yahoo,
    ProtonMail,
    Custom {
        imap_host: String,
        imap_port: u16,
        smtp_host: String,
        smtp_port: u16,
    },
}

impl EmailProvider {
    /// Human-readable display name.
    fn display_name(&self) -> &str {
        match self {
            EmailProvider::Gmail => "Gmail",
            EmailProvider::Outlook => "Outlook",
            EmailProvider::Yahoo => "Yahoo",
            EmailProvider::ProtonMail => "ProtonMail",
            EmailProvider::Custom { .. } => "Custom IMAP",
        }
    }

    /// Webmail URL for browser-based access (MVP pattern).
    fn webmail_url(&self) -> Option<&str> {
        match self {
            EmailProvider::Gmail => Some("https://mail.google.com"),
            EmailProvider::Outlook => Some("https://outlook.live.com"),
            EmailProvider::Yahoo => Some("https://mail.yahoo.com"),
            EmailProvider::ProtonMail => Some("https://mail.proton.me"),
            EmailProvider::Custom { .. } => None,
        }
    }
}

/// An email account registered in ForgeMail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub provider: EmailProvider,
    pub connected: bool,
    pub created_at: String,
}

/// A full email message (cached locally).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: String,
    pub account_id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub body_html: String,
    pub date: String,
    pub is_read: bool,
    pub is_starred: bool,
    pub folder: String,
    pub labels: Vec<String>,
}

/// A draft email ready to send or save.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDraft {
    pub id: String,
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Email categorization result from AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailCategory {
    pub email_id: String,
    pub category: String,
    pub confidence: f64,
    pub summary: String,
}

/// Lightweight email listing (no body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailListItem {
    pub id: String,
    pub account_id: String,
    pub from: String,
    pub subject: String,
    pub preview: String,
    pub date: String,
    pub is_read: bool,
    pub is_starred: bool,
    pub folder: String,
    pub labels: Vec<String>,
}

/// Account listing with unread counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWithCounts {
    pub account: EmailAccount,
    pub unread_count: u32,
    pub total_count: u32,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the mail base directory, creating it if necessary.
fn mail_base_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(MAIL_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem("DIR_CREATE_FAILED", format!("Failed to create mail directory: {e}"))
        })?;
    }
    Ok(dir)
}

/// Ensure subdirectories exist.
fn ensure_subdirs(base: &Path) -> Result<(), ImpForgeError> {
    for sub in [EMAILS_DIR, DRAFTS_DIR] {
        let p = base.join(sub);
        if !p.exists() {
            std::fs::create_dir_all(&p).map_err(|e| {
                ImpForgeError::filesystem(
                    "DIR_CREATE_FAILED",
                    format!("Failed to create mail subdirectory: {e}"),
                )
            })?;
        }
    }
    Ok(())
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Load accounts from the JSON registry file.
fn load_accounts(base: &Path) -> Result<Vec<EmailAccount>, ImpForgeError> {
    let path = base.join(ACCOUNTS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem("ACCOUNTS_READ_FAILED", format!("Cannot read accounts file: {e}"))
    })?;
    serde_json::from_str::<Vec<EmailAccount>>(&data).map_err(|e| {
        ImpForgeError::internal("ACCOUNTS_PARSE_FAILED", format!("Corrupt accounts file: {e}"))
    })
}

/// Save accounts to the JSON registry file.
fn save_accounts(base: &Path, accounts: &[EmailAccount]) -> Result<(), ImpForgeError> {
    let path = base.join(ACCOUNTS_FILE);
    let json = serde_json::to_string_pretty(accounts).map_err(|e| {
        ImpForgeError::internal("ACCOUNTS_SERIALIZE", format!("Cannot serialize accounts: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem("ACCOUNTS_WRITE_FAILED", format!("Cannot write accounts file: {e}"))
    })
}

/// Load all cached emails for a given account from the emails directory.
fn load_emails_for_account(base: &Path, account_id: &str) -> Result<Vec<Email>, ImpForgeError> {
    let dir = base.join(EMAILS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut emails = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read emails dir: {e}"))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let email: Email = match serde_json::from_str(&data) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if email.account_id == account_id {
            emails.push(email);
        }
    }

    // Sort by date descending
    emails.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(emails)
}

/// Save a single email to the cache.
fn save_email(base: &Path, email: &Email) -> Result<(), ImpForgeError> {
    let dir = base.join(EMAILS_DIR);
    let path = dir.join(format!("{}.json", email.id));
    let json = serde_json::to_string_pretty(email).map_err(|e| {
        ImpForgeError::internal("EMAIL_SERIALIZE", format!("Cannot serialize email: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem("EMAIL_WRITE_FAILED", format!("Cannot save email: {e}"))
    })
}

/// Load a single email by ID.
fn load_email(base: &Path, id: &str) -> Result<Email, ImpForgeError> {
    let path = base.join(EMAILS_DIR).join(format!("{id}.json"));
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("EMAIL_NOT_FOUND", format!("Email '{id}' not found"))
                .with_suggestion("The email may have been deleted."),
        );
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem("EMAIL_READ_FAILED", format!("Cannot read email: {e}"))
    })?;
    serde_json::from_str::<Email>(&data).map_err(|e| {
        ImpForgeError::internal("EMAIL_PARSE_FAILED", format!("Corrupt email file: {e}"))
    })
}

/// Save a draft to the drafts directory.
fn save_draft(base: &Path, draft: &EmailDraft) -> Result<(), ImpForgeError> {
    let dir = base.join(DRAFTS_DIR);
    let path = dir.join(format!("{}.json", draft.id));
    let json = serde_json::to_string_pretty(draft).map_err(|e| {
        ImpForgeError::internal("DRAFT_SERIALIZE", format!("Cannot serialize draft: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem("DRAFT_WRITE_FAILED", format!("Cannot save draft: {e}"))
    })
}

/// Create a truncated preview of an email body.
fn make_preview(body: &str, max_len: usize) -> String {
    let clean = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if clean.len() > max_len {
        format!("{}...", &clean[..max_len.saturating_sub(3)])
    } else {
        clean
    }
}

/// Convert a full Email to a lightweight list item.
fn email_to_list_item(email: &Email) -> EmailListItem {
    EmailListItem {
        id: email.id.clone(),
        account_id: email.account_id.clone(),
        from: email.from.clone(),
        subject: email.subject.clone(),
        preview: make_preview(&email.body, 120),
        date: email.date.clone(),
        is_read: email.is_read,
        is_starred: email.is_starred,
        folder: email.folder.clone(),
        labels: email.labels.clone(),
    }
}

// ---------------------------------------------------------------------------
// AI (Ollama)
// ---------------------------------------------------------------------------

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Send an AI email composition/categorization prompt to Ollama.
async fn ollama_mail_assist(
    system_prompt: &str,
    user_message: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_COMPOSE_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for AI email assist",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "AI email assist timed out")
                    .with_suggestion("Try a shorter email or simpler request.")
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Ollama request failed: {e}"),
                )
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(
            ImpForgeError::service("OLLAMA_HTTP_ERROR", format!("Ollama returned HTTP {status}"))
                .with_details(body)
                .with_suggestion("Check Ollama logs. The model may not be downloaded yet."),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse Ollama response: {e}"))
    })?;

    let content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(ImpForgeError::service(
            "OLLAMA_EMPTY_RESPONSE",
            "Ollama returned an empty response",
        ));
    }

    Ok(content)
}

// ---------------------------------------------------------------------------
// Tauri Commands — Account Management
// ---------------------------------------------------------------------------

/// List all registered email accounts with unread/total counts.
#[tauri::command]
pub async fn mail_list_accounts() -> AppResult<Vec<AccountWithCounts>> {
    let base = mail_base_dir()?;
    ensure_subdirs(&base)?;
    let accounts = load_accounts(&base)?;

    let mut result = Vec::with_capacity(accounts.len());
    for account in accounts {
        let emails = load_emails_for_account(&base, &account.id)?;
        let unread_count = emails.iter().filter(|e| !e.is_read && e.folder == "inbox").count() as u32;
        let total_count = emails.len() as u32;
        result.push(AccountWithCounts {
            account,
            unread_count,
            total_count,
        });
    }

    Ok(result)
}

/// Add a new email account.
#[tauri::command]
pub async fn mail_add_account(
    name: String,
    email: String,
    provider: EmailProvider,
) -> AppResult<EmailAccount> {
    let base = mail_base_dir()?;
    ensure_subdirs(&base)?;

    if email.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_EMAIL",
            "Email address cannot be empty",
        ));
    }

    let mut accounts = load_accounts(&base)?;

    // Check for duplicate email
    if accounts.iter().any(|a| a.email == email) {
        return Err(ImpForgeError::validation(
            "DUPLICATE_EMAIL",
            format!("Account with email '{email}' already exists"),
        ));
    }

    let account = EmailAccount {
        id: Uuid::new_v4().to_string(),
        name: if name.trim().is_empty() {
            email.clone()
        } else {
            name
        },
        email,
        provider,
        connected: false,
        created_at: now_iso(),
    };

    accounts.push(account.clone());
    save_accounts(&base, &accounts)?;

    log::info!("ForgeMail: added account '{}' ({})", account.name, account.email);
    Ok(account)
}

/// Remove an email account and its cached emails.
#[tauri::command]
pub async fn mail_remove_account(id: String) -> AppResult<()> {
    let base = mail_base_dir()?;
    let mut accounts = load_accounts(&base)?;

    let initial_len = accounts.len();
    accounts.retain(|a| a.id != id);

    if accounts.len() == initial_len {
        return Err(
            ImpForgeError::filesystem("ACCOUNT_NOT_FOUND", format!("Account '{id}' not found"))
                .with_suggestion("The account may have already been removed."),
        );
    }

    save_accounts(&base, &accounts)?;

    // Remove cached emails for this account
    let emails_dir = base.join(EMAILS_DIR);
    if emails_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&emails_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(email) = serde_json::from_str::<Email>(&data) {
                        if email.account_id == id {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    log::info!("ForgeMail: removed account '{}'", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri Commands — Email Operations
// ---------------------------------------------------------------------------

/// List emails for an account, optionally filtered by folder.
#[tauri::command]
pub async fn mail_list_emails(
    account_id: String,
    folder: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<EmailListItem>> {
    let base = mail_base_dir()?;
    ensure_subdirs(&base)?;
    let mut emails = load_emails_for_account(&base, &account_id)?;

    // Filter by folder if specified
    if let Some(ref f) = folder {
        emails.retain(|e| e.folder.eq_ignore_ascii_case(f));
    }

    // Apply limit
    let max = limit.unwrap_or(50) as usize;
    emails.truncate(max);

    Ok(emails.iter().map(email_to_list_item).collect())
}

/// Get a single email by ID (full content).
#[tauri::command]
pub async fn mail_get_email(id: String) -> AppResult<Email> {
    let base = mail_base_dir()?;
    load_email(&base, &id)
}

/// Mark an email as read or unread.
#[tauri::command]
pub async fn mail_mark_read(id: String, is_read: bool) -> AppResult<()> {
    let base = mail_base_dir()?;
    let mut email = load_email(&base, &id)?;
    email.is_read = is_read;
    save_email(&base, &email)?;
    Ok(())
}

/// Toggle star on an email.
#[tauri::command]
pub async fn mail_star(id: String, starred: bool) -> AppResult<()> {
    let base = mail_base_dir()?;
    let mut email = load_email(&base, &id)?;
    email.is_starred = starred;
    save_email(&base, &email)?;
    Ok(())
}

/// Delete an email (moves to trash, or permanently deletes from trash).
#[tauri::command]
pub async fn mail_delete(id: String) -> AppResult<()> {
    let base = mail_base_dir()?;
    let email = load_email(&base, &id)?;

    if email.folder == "trash" {
        // Permanently delete
        let path = base.join(EMAILS_DIR).join(format!("{id}.json"));
        std::fs::remove_file(&path).map_err(|e| {
            ImpForgeError::filesystem("DELETE_FAILED", format!("Cannot delete email: {e}"))
        })?;
        log::info!("ForgeMail: permanently deleted email '{}'", id);
    } else {
        // Move to trash
        let mut email = email;
        email.folder = "trash".to_string();
        save_email(&base, &email)?;
        log::info!("ForgeMail: moved email '{}' to trash", id);
    }

    Ok(())
}

/// Move an email to a different folder.
#[tauri::command]
pub async fn mail_move(id: String, folder: String) -> AppResult<()> {
    let base = mail_base_dir()?;
    let mut email = load_email(&base, &id)?;

    let valid_folders = ["inbox", "sent", "drafts", "starred", "trash", "archive", "spam"];
    let folder_lower = folder.to_ascii_lowercase();
    if !valid_folders.contains(&folder_lower.as_str()) {
        return Err(ImpForgeError::validation(
            "INVALID_FOLDER",
            format!(
                "Invalid folder: '{folder}'. Valid folders: {}",
                valid_folders.join(", ")
            ),
        ));
    }

    email.folder = folder_lower;
    save_email(&base, &email)?;
    Ok(())
}

/// Search emails by query string (subject/from/body).
#[tauri::command]
pub async fn mail_search(
    account_id: String,
    query: String,
) -> AppResult<Vec<EmailListItem>> {
    let base = mail_base_dir()?;
    let emails = load_emails_for_account(&base, &account_id)?;
    let q = query.to_ascii_lowercase();

    let matches: Vec<EmailListItem> = emails
        .iter()
        .filter(|e| {
            e.subject.to_ascii_lowercase().contains(&q)
                || e.from.to_ascii_lowercase().contains(&q)
                || e.body.to_ascii_lowercase().contains(&q)
        })
        .take(50)
        .map(email_to_list_item)
        .collect();

    Ok(matches)
}

// ---------------------------------------------------------------------------
// Tauri Commands — AI Compose / Categorize
// ---------------------------------------------------------------------------

/// AI-powered email composition.
///
/// `action` can be: "reply", "compose", "forward", "summarize"
/// `context` provides the original email thread or instructions.
/// `tone` optionally specifies: "professional", "casual", "formal", "friendly"
#[tauri::command]
pub async fn mail_ai_compose(
    context: String,
    action: String,
    tone: Option<String>,
) -> AppResult<String> {
    if context.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_CONTEXT",
            "Provide context or instructions for AI compose",
        ));
    }

    let tone_desc = tone.as_deref().unwrap_or("professional");
    let valid_tones = ["professional", "casual", "formal", "friendly"];
    if !valid_tones.contains(&tone_desc) {
        return Err(ImpForgeError::validation(
            "INVALID_TONE",
            format!("Invalid tone: '{tone_desc}'. Valid: {}", valid_tones.join(", ")),
        ));
    }

    let system_prompt = format!(
        "You are a professional email assistant inside ImpForge, an AI Workstation. \
         Write emails in a {tone_desc} tone. \
         Return ONLY the email text -- no explanations, no markdown fences, no preamble. \
         Include an appropriate greeting and sign-off."
    );

    let user_prompt = match action.as_str() {
        "reply" => format!(
            "Write a {tone_desc} reply to this email:\n\n---\n\n{context}"
        ),
        "compose" => format!(
            "Compose a {tone_desc} email based on these instructions:\n\n{context}"
        ),
        "forward" => format!(
            "Write a {tone_desc} forwarding message for this email:\n\n---\n\n{context}"
        ),
        "summarize" => format!(
            "Summarize this email thread in 2-4 concise bullet points:\n\n---\n\n{context}"
        ),
        other => {
            return Err(ImpForgeError::validation(
                "INVALID_ACTION",
                format!(
                    "Unknown AI action: '{other}'. Valid actions: reply, compose, forward, summarize"
                ),
            ));
        }
    };

    log::info!("ForgeMail: AI compose action='{}' tone='{}'", action, tone_desc);

    ollama_mail_assist(&system_prompt, &user_prompt, None).await
}

/// AI-powered email categorization.
///
/// Takes a list of email IDs and returns category assignments.
#[tauri::command]
pub async fn mail_ai_categorize(
    email_ids: Vec<String>,
) -> AppResult<Vec<EmailCategory>> {
    let base = mail_base_dir()?;

    if email_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Load emails and build a summary list
    let mut summaries = Vec::new();
    let mut loaded_ids = Vec::new();
    for id in &email_ids {
        if let Ok(email) = load_email(&base, id) {
            summaries.push(format!(
                "ID: {}\nFrom: {}\nSubject: {}\nPreview: {}",
                email.id,
                email.from,
                email.subject,
                make_preview(&email.body, 200),
            ));
            loaded_ids.push(id.clone());
        }
    }

    if summaries.is_empty() {
        return Ok(Vec::new());
    }

    let system_prompt = "You are an email categorization AI. \
        Categorize each email into ONE of: Important, Action Required, FYI, Newsletter, Spam, Personal. \
        Respond in JSON array format: [{\"id\":\"...\",\"category\":\"...\",\"confidence\":0.9,\"summary\":\"...\"}]. \
        The summary should be one sentence. Return ONLY the JSON array.";

    let user_prompt = format!(
        "Categorize these {} emails:\n\n{}",
        summaries.len(),
        summaries.join("\n\n---\n\n")
    );

    let result = ollama_mail_assist(system_prompt, &user_prompt, None).await?;

    // Parse the JSON response
    let categories: Vec<EmailCategory> = serde_json::from_str(&result).unwrap_or_else(|_| {
        // Fallback: create default categories if AI response is not valid JSON
        loaded_ids
            .iter()
            .map(|id| EmailCategory {
                email_id: id.clone(),
                category: "FYI".to_string(),
                confidence: 0.5,
                summary: "Could not categorize — AI response was not valid JSON.".to_string(),
            })
            .collect()
    });

    Ok(categories)
}

// ---------------------------------------------------------------------------
// Tauri Commands — Draft / Send
// ---------------------------------------------------------------------------

/// Save or send a draft email.
/// For MVP: saves the draft locally. Returns a status message.
/// Future: will send via SMTP.
#[tauri::command]
pub async fn mail_send_draft(
    account_id: String,
    to: Vec<String>,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    subject: String,
    body: String,
    reply_to: Option<String>,
) -> AppResult<String> {
    let base = mail_base_dir()?;
    ensure_subdirs(&base)?;

    if to.is_empty() {
        return Err(ImpForgeError::validation(
            "NO_RECIPIENTS",
            "At least one recipient is required",
        ));
    }

    // Validate that the account exists
    let accounts = load_accounts(&base)?;
    let account = accounts.iter().find(|a| a.id == account_id).ok_or_else(|| {
        ImpForgeError::filesystem(
            "ACCOUNT_NOT_FOUND",
            format!("Account '{account_id}' not found"),
        )
    })?;

    let now = now_iso();
    let draft = EmailDraft {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.clone(),
        to: to.clone(),
        cc: cc.unwrap_or_default(),
        bcc: bcc.unwrap_or_default(),
        subject: subject.clone(),
        body: body.clone(),
        reply_to,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    save_draft(&base, &draft)?;

    // Also save as an email in the "sent" folder (for local record)
    let sent_email = Email {
        id: draft.id.clone(),
        account_id,
        from: account.email.clone(),
        to,
        cc: draft.cc.clone(),
        subject,
        body,
        body_html: String::new(),
        date: now,
        is_read: true,
        is_starred: false,
        folder: "drafts".to_string(),
        labels: Vec::new(),
    };
    save_email(&base, &sent_email)?;

    // MVP: return instructions to send manually
    let webmail_msg = account
        .provider
        .webmail_url()
        .map(|url| format!(" Open {} to send it.", url))
        .unwrap_or_default();

    log::info!("ForgeMail: draft saved for account '{}'", account.name);

    Ok(format!(
        "Draft saved successfully.{} SMTP sending will be available in a future update.",
        webmail_msg
    ))
}

/// Get the webmail URL for an account (for Browser Agent integration).
#[tauri::command]
pub async fn mail_webmail_url(account_id: String) -> AppResult<Option<String>> {
    let base = mail_base_dir()?;
    let accounts = load_accounts(&base)?;
    let account = accounts.iter().find(|a| a.id == account_id).ok_or_else(|| {
        ImpForgeError::filesystem("ACCOUNT_NOT_FOUND", format!("Account '{account_id}' not found"))
    })?;
    Ok(account.provider.webmail_url().map(String::from))
}

/// Get folder counts for an account (inbox, sent, drafts, trash, etc.).
#[tauri::command]
pub async fn mail_folder_counts(account_id: String) -> AppResult<Vec<(String, u32, u32)>> {
    let base = mail_base_dir()?;
    ensure_subdirs(&base)?;
    let emails = load_emails_for_account(&base, &account_id)?;

    let folders = ["inbox", "sent", "drafts", "starred", "trash", "archive", "spam"];
    let mut counts: Vec<(String, u32, u32)> = Vec::new();

    for folder in folders {
        let folder_emails: Vec<_> = emails.iter().filter(|e| e.folder == folder).collect();
        let total = folder_emails.len() as u32;
        let unread = folder_emails.iter().filter(|e| !e.is_read).count() as u32;
        if total > 0 || folder == "inbox" || folder == "sent" || folder == "drafts" || folder == "trash" {
            counts.push((folder.to_string(), total, unread));
        }
    }

    Ok(counts)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_provider_display() {
        assert_eq!(EmailProvider::Gmail.display_name(), "Gmail");
        assert_eq!(EmailProvider::Outlook.display_name(), "Outlook");
        assert_eq!(
            EmailProvider::Custom {
                imap_host: "mail.example.com".into(),
                imap_port: 993,
                smtp_host: "smtp.example.com".into(),
                smtp_port: 587,
            }
            .display_name(),
            "Custom IMAP"
        );
    }

    #[test]
    fn test_email_provider_webmail_url() {
        assert_eq!(
            EmailProvider::Gmail.webmail_url(),
            Some("https://mail.google.com")
        );
        assert_eq!(
            EmailProvider::Custom {
                imap_host: "x".into(),
                imap_port: 993,
                smtp_host: "y".into(),
                smtp_port: 587,
            }
            .webmail_url(),
            None
        );
    }

    #[test]
    fn test_make_preview_short() {
        let body = "Hello world. This is a test.";
        let preview = make_preview(body, 100);
        assert_eq!(preview, "Hello world. This is a test.");
    }

    #[test]
    fn test_make_preview_long() {
        let body = "A ".repeat(200);
        let preview = make_preview(&body, 50);
        assert!(preview.len() <= 50);
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_make_preview_multiline() {
        let body = "Line one.\n\nLine two.\n\nLine three.";
        let preview = make_preview(body, 100);
        assert_eq!(preview, "Line one. Line two. Line three.");
    }

    #[test]
    fn test_email_to_list_item() {
        let email = Email {
            id: "test-1".into(),
            account_id: "acc-1".into(),
            from: "alice@example.com".into(),
            to: vec!["bob@example.com".into()],
            cc: vec![],
            subject: "Test Subject".into(),
            body: "Hello Bob, this is a test email.".into(),
            body_html: String::new(),
            date: "2026-03-15T10:00:00Z".into(),
            is_read: false,
            is_starred: true,
            folder: "inbox".into(),
            labels: vec!["important".into()],
        };
        let item = email_to_list_item(&email);
        assert_eq!(item.id, "test-1");
        assert_eq!(item.from, "alice@example.com");
        assert_eq!(item.subject, "Test Subject");
        assert!(!item.preview.is_empty());
        assert!(!item.is_read);
        assert!(item.is_starred);
    }

    #[test]
    fn test_email_provider_serialize() {
        let provider = EmailProvider::Gmail;
        let json = serde_json::to_string(&provider).expect("serialize");
        assert!(json.contains("gmail"));

        let custom = EmailProvider::Custom {
            imap_host: "mail.example.com".into(),
            imap_port: 993,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
        };
        let json = serde_json::to_string(&custom).expect("serialize custom");
        assert!(json.contains("custom"));
        assert!(json.contains("mail.example.com"));
    }

    #[test]
    fn test_email_provider_deserialize() {
        let json = r#"{"type":"gmail"}"#;
        let p: EmailProvider = serde_json::from_str(json).expect("deserialize");
        assert_eq!(p, EmailProvider::Gmail);

        let json = r#"{"type":"custom","imap_host":"mail.x.com","imap_port":993,"smtp_host":"smtp.x.com","smtp_port":587}"#;
        let p: EmailProvider = serde_json::from_str(json).expect("deserialize custom");
        match p {
            EmailProvider::Custom { imap_host, .. } => assert_eq!(imap_host, "mail.x.com"),
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn test_email_category_serialize() {
        let cat = EmailCategory {
            email_id: "e1".into(),
            category: "Important".into(),
            confidence: 0.95,
            summary: "Needs immediate attention.".into(),
        };
        let json = serde_json::to_string(&cat).expect("serialize");
        assert!(json.contains("Important"));
        assert!(json.contains("0.95"));
    }

    #[test]
    fn test_email_serialize_roundtrip() {
        let email = Email {
            id: "test-rt".into(),
            account_id: "acc-rt".into(),
            from: "test@example.com".into(),
            to: vec!["recv@example.com".into()],
            cc: vec![],
            subject: "Roundtrip test".into(),
            body: "Body content here.".into(),
            body_html: "<p>Body content here.</p>".into(),
            date: "2026-03-15T12:00:00Z".into(),
            is_read: true,
            is_starred: false,
            folder: "sent".into(),
            labels: vec!["work".into()],
        };
        let json = serde_json::to_string(&email).expect("serialize");
        let parsed: Email = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, email.id);
        assert_eq!(parsed.subject, email.subject);
        assert_eq!(parsed.folder, "sent");
    }
}
