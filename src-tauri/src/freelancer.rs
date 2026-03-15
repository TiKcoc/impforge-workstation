// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Freelancer Hub -- Universal freelancer management dashboard.
//!
//! Provides gig portfolio management, client CRM-lite, AI-powered proposal
//! drafting (via Ollama), invoice generation, time tracking and earnings
//! analytics.  Storage is fully local via `tauri-plugin-store`, making it
//! work offline-first on any customer PC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

// ════════════════════════════════════════════════════════════════════════════
//  Data Models
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreelancerProfile {
    pub name: String,
    pub title: String,
    pub bio: String,
    pub skills: Vec<String>,
    pub hourly_rate: f64,
    pub portfolio_url: String,
    pub platforms: Vec<String>,
}

impl Default for FreelancerProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            title: String::new(),
            bio: String::new(),
            skills: Vec::new(),
            hourly_rate: 0.0,
            portfolio_url: String::new(),
            platforms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gig {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub price_min: f64,
    pub price_max: f64,
    pub delivery_days: u32,
    pub tags: Vec<String>,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub email: String,
    pub company: String,
    pub notes: String,
    pub total_spent: f64,
    pub projects_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub client_name: String,
    pub project_title: String,
    pub content: String,
    /// draft | sent | accepted | rejected
    pub status: String,
    pub amount: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub client_id: String,
    pub client_name: String,
    pub items: Vec<InvoiceItem>,
    pub total: f64,
    /// draft | sent | paid
    pub status: String,
    pub due_date: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: String,
    pub project: String,
    pub description: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_minutes: f64,
    pub billable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsSummary {
    pub weekly: f64,
    pub monthly: f64,
    pub yearly: f64,
    pub total_invoiced: f64,
    pub total_paid: f64,
    pub outstanding: f64,
    pub hours_this_month: f64,
    pub effective_rate: f64,
}

// ════════════════════════════════════════════════════════════════════════════
//  Persistent Store Wrapper
// ════════════════════════════════════════════════════════════════════════════

/// Top-level container serialised to `.impforge-freelancer.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FreelancerData {
    profile: FreelancerProfile,
    gigs: Vec<Gig>,
    clients: Vec<Client>,
    proposals: Vec<Proposal>,
    invoices: Vec<Invoice>,
    time_entries: Vec<TimeEntry>,
}

/// In-memory mirror.  The Mutex is cheap because all operations are
/// short-lived (microsecond locks, no async contention).
static DATA: once_cell::sync::Lazy<Mutex<FreelancerData>> =
    once_cell::sync::Lazy::new(|| Mutex::new(FreelancerData::default()));

/// Persist current state to the Tauri plugin-store file.
async fn persist(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store(".impforge-freelancer.json")
        .map_err(|e| format!("Store error: {e}"))?;
    let data = DATA.lock().map_err(|e| format!("Lock error: {e}"))?;
    store
        .set("freelancer_data", serde_json::to_value(&*data).map_err(|e| format!("Serialize: {e}"))?);
    store.save().map_err(|e| format!("Save: {e}"))?;
    Ok(())
}

/// Load persisted state on first access.
fn ensure_loaded(app: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;
    // Only run once (cheap idempotent check).
    let already_loaded = {
        let d = DATA.lock().unwrap_or_else(|e| e.into_inner());
        // Heuristic: if profile name is set or any data exists, already loaded.
        !d.profile.name.is_empty()
            || !d.gigs.is_empty()
            || !d.clients.is_empty()
            || !d.invoices.is_empty()
    };
    if already_loaded {
        return;
    }

    if let Ok(store) = app.store(".impforge-freelancer.json") {
        if let Some(val) = store.get("freelancer_data") {
            if let Ok(loaded) = serde_json::from_value::<FreelancerData>(val.clone()) {
                let mut d = DATA.lock().unwrap_or_else(|e| e.into_inner());
                *d = loaded;
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Tauri Commands
// ════════════════════════════════════════════════════════════════════════════

// ── Profile ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_get_profile(app: tauri::AppHandle) -> Result<FreelancerProfile, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    Ok(d.profile.clone())
}

#[tauri::command]
pub async fn freelancer_save_profile(
    app: tauri::AppHandle,
    profile: FreelancerProfile,
) -> Result<(), String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.profile = profile;
    }
    persist(&app).await
}

// ── Gigs ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_list_gigs(app: tauri::AppHandle) -> Result<Vec<Gig>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut gigs = d.gigs.clone();
    gigs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(gigs)
}

#[tauri::command]
pub async fn freelancer_add_gig(app: tauri::AppHandle, gig: Gig) -> Result<Gig, String> {
    ensure_loaded(&app);
    let new_gig = Gig {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now().to_rfc3339(),
        ..gig
    };
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.gigs.push(new_gig.clone());
    }
    persist(&app).await?;
    Ok(new_gig)
}

#[tauri::command]
pub async fn freelancer_update_gig(app: tauri::AppHandle, gig: Gig) -> Result<Gig, String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        if let Some(existing) = d.gigs.iter_mut().find(|g| g.id == gig.id) {
            *existing = gig.clone();
        } else {
            return Err(format!("Gig not found: {}", gig.id));
        }
    }
    persist(&app).await?;
    Ok(gig)
}

#[tauri::command]
pub async fn freelancer_delete_gig(app: tauri::AppHandle, gig_id: String) -> Result<(), String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.gigs.retain(|g| g.id != gig_id);
    }
    persist(&app).await
}

// ── Clients ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_list_clients(app: tauri::AppHandle) -> Result<Vec<Client>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut clients = d.clients.clone();
    clients.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(clients)
}

#[tauri::command]
pub async fn freelancer_add_client(
    app: tauri::AppHandle,
    client: Client,
) -> Result<Client, String> {
    ensure_loaded(&app);
    let new_client = Client {
        id: Uuid::new_v4().to_string(),
        total_spent: 0.0,
        projects_count: 0,
        created_at: Utc::now().to_rfc3339(),
        ..client
    };
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.clients.push(new_client.clone());
    }
    persist(&app).await?;
    Ok(new_client)
}

#[tauri::command]
pub async fn freelancer_update_client(
    app: tauri::AppHandle,
    client: Client,
) -> Result<Client, String> {
    ensure_loaded(&app);
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        if let Some(existing) = d.clients.iter_mut().find(|c| c.id == client.id) {
            *existing = client.clone();
        } else {
            return Err(format!("Client not found: {}", client.id));
        }
    }
    persist(&app).await?;
    Ok(client)
}

// ── Proposals ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_list_proposals(app: tauri::AppHandle) -> Result<Vec<Proposal>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut proposals = d.proposals.clone();
    proposals.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(proposals)
}

#[tauri::command]
pub async fn freelancer_save_proposal(
    app: tauri::AppHandle,
    proposal: Proposal,
) -> Result<Proposal, String> {
    ensure_loaded(&app);
    let saved = Proposal {
        id: if proposal.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            proposal.id.clone()
        },
        created_at: if proposal.created_at.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            proposal.created_at.clone()
        },
        ..proposal
    };
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        if let Some(existing) = d.proposals.iter_mut().find(|p| p.id == saved.id) {
            *existing = saved.clone();
        } else {
            d.proposals.push(saved.clone());
        }
    }
    persist(&app).await?;
    Ok(saved)
}

/// Generate a proposal draft using Ollama (offline-first AI).
#[tauri::command]
pub async fn freelancer_generate_proposal(
    client_name: String,
    project: String,
    requirements: String,
) -> Result<String, String> {
    if project.trim().is_empty() {
        return Err("Project description is required".to_string());
    }

    let system_prompt = format!(
        "You are a professional freelancer writing a project proposal.\n\
         Client: {client_name}\n\
         Write a compelling, professional proposal that:\n\
         1. Acknowledges the client's needs\n\
         2. Outlines your approach and methodology\n\
         3. Provides a clear timeline with milestones\n\
         4. Highlights relevant experience\n\
         5. Includes a professional closing\n\n\
         Keep it concise (300-500 words), professional, and persuasive.\n\
         Do NOT use markdown headers. Use plain text with clear paragraphs."
    );

    let prompt = format!(
        "Write a project proposal for:\n\
         Project: {project}\n\
         Requirements: {requirements}"
    );

    let ollama_url = std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let payload = serde_json::json!({
        "model": "dolphin3:8b",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt}
        ],
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 768
        }
    });

    let resp = client
        .post(format!("{ollama_url}/api/chat"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                "Ollama is not running. Start it with: ollama serve".to_string()
            } else {
                format!("Ollama request failed: {e}")
            }
        })?;

    if !resp.status().is_success() {
        return Err(format!(
            "Ollama returned HTTP {}. Ensure a model is available (ollama pull dolphin3:8b)",
            resp.status()
        ));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let generated = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("(AI generation returned empty content)")
        .to_string();

    Ok(generated)
}

// ── Invoices ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_list_invoices(app: tauri::AppHandle) -> Result<Vec<Invoice>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut invoices = d.invoices.clone();
    invoices.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(invoices)
}

#[tauri::command]
pub async fn freelancer_create_invoice(
    app: tauri::AppHandle,
    client_id: String,
    items: Vec<InvoiceItem>,
    due_date: String,
) -> Result<Invoice, String> {
    ensure_loaded(&app);

    let client_name = {
        let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.clients
            .iter()
            .find(|c| c.id == client_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown Client".to_string())
    };

    let total: f64 = items.iter().map(|i| i.quantity * i.unit_price).sum();
    let computed_items: Vec<InvoiceItem> = items
        .into_iter()
        .map(|i| InvoiceItem {
            total: i.quantity * i.unit_price,
            ..i
        })
        .collect();

    let invoice = Invoice {
        id: format!("INV-{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("0000")),
        client_id: client_id.clone(),
        client_name,
        items: computed_items,
        total,
        status: "draft".to_string(),
        due_date,
        created_at: Utc::now().to_rfc3339(),
    };

    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.invoices.push(invoice.clone());

        // Update client total_spent and projects_count
        if let Some(client) = d.clients.iter_mut().find(|c| c.id == client_id) {
            client.projects_count += 1;
        }
    }
    persist(&app).await?;
    Ok(invoice)
}

#[tauri::command]
pub async fn freelancer_update_invoice_status(
    app: tauri::AppHandle,
    invoice_id: String,
    status: String,
) -> Result<Invoice, String> {
    ensure_loaded(&app);
    let invoice = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let inv = d
            .invoices
            .iter_mut()
            .find(|i| i.id == invoice_id)
            .ok_or_else(|| format!("Invoice not found: {invoice_id}"))?;
        let old_status = inv.status.clone();
        inv.status = status.clone();

        // When marking as paid, update client total_spent
        if status == "paid" && old_status != "paid" {
            let client_id = inv.client_id.clone();
            let total = inv.total;
            let inv_clone = inv.clone();
            if let Some(client) = d.clients.iter_mut().find(|c| c.id == client_id) {
                client.total_spent += total;
            }
            inv_clone
        } else {
            inv.clone()
        }
    };
    persist(&app).await?;
    Ok(invoice)
}

// ── Time Tracking ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_time_entries(
    app: tauri::AppHandle,
    project: Option<String>,
) -> Result<Vec<TimeEntry>, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
    let mut entries: Vec<TimeEntry> = if let Some(ref proj) = project {
        d.time_entries
            .iter()
            .filter(|e| e.project.eq_ignore_ascii_case(proj))
            .cloned()
            .collect()
    } else {
        d.time_entries.clone()
    };
    entries.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(entries)
}

#[tauri::command]
pub async fn freelancer_start_timer(
    app: tauri::AppHandle,
    project: String,
    description: String,
    billable: Option<bool>,
) -> Result<TimeEntry, String> {
    ensure_loaded(&app);
    let entry = TimeEntry {
        id: Uuid::new_v4().to_string(),
        project,
        description,
        start_time: Utc::now().to_rfc3339(),
        end_time: None,
        duration_minutes: 0.0,
        billable: billable.unwrap_or(true),
    };
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.time_entries.push(entry.clone());
    }
    persist(&app).await?;
    Ok(entry)
}

#[tauri::command]
pub async fn freelancer_stop_timer(
    app: tauri::AppHandle,
    entry_id: String,
) -> Result<TimeEntry, String> {
    ensure_loaded(&app);
    let now = Utc::now();
    let entry = {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        let te = d
            .time_entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| format!("Time entry not found: {entry_id}"))?;

        if te.end_time.is_some() {
            return Err("Timer already stopped".to_string());
        }

        te.end_time = Some(now.to_rfc3339());

        // Calculate duration
        if let Ok(start) = DateTime::parse_from_rfc3339(&te.start_time) {
            let duration = now.signed_duration_since(start.with_timezone(&Utc));
            te.duration_minutes = duration.num_seconds() as f64 / 60.0;
        }
        te.clone()
    };
    persist(&app).await?;
    Ok(entry)
}

#[tauri::command]
pub async fn freelancer_add_time_entry(
    app: tauri::AppHandle,
    entry: TimeEntry,
) -> Result<TimeEntry, String> {
    ensure_loaded(&app);
    let new_entry = TimeEntry {
        id: Uuid::new_v4().to_string(),
        ..entry
    };
    {
        let mut d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;
        d.time_entries.push(new_entry.clone());
    }
    persist(&app).await?;
    Ok(new_entry)
}

// ── Earnings Summary ────────────────────────────────────────────────────

#[tauri::command]
pub async fn freelancer_earnings_summary(
    app: tauri::AppHandle,
) -> Result<EarningsSummary, String> {
    ensure_loaded(&app);
    let d = DATA.lock().map_err(|e| format!("Lock: {e}"))?;

    let now = Utc::now();
    let week_ago = now - chrono::Duration::days(7);
    let month_ago = now - chrono::Duration::days(30);
    let year_ago = now - chrono::Duration::days(365);

    let mut weekly = 0.0_f64;
    let mut monthly = 0.0_f64;
    let mut yearly = 0.0_f64;
    let mut total_invoiced = 0.0_f64;
    let mut total_paid = 0.0_f64;
    let mut outstanding = 0.0_f64;

    for inv in &d.invoices {
        total_invoiced += inv.total;
        if inv.status == "paid" {
            total_paid += inv.total;
            if let Ok(dt) = DateTime::parse_from_rfc3339(&inv.created_at) {
                let dt_utc = dt.with_timezone(&Utc);
                if dt_utc >= week_ago {
                    weekly += inv.total;
                }
                if dt_utc >= month_ago {
                    monthly += inv.total;
                }
                if dt_utc >= year_ago {
                    yearly += inv.total;
                }
            }
        } else if inv.status != "draft" {
            outstanding += inv.total;
        }
    }

    // Hours this month from time entries
    let mut hours_this_month = 0.0_f64;
    for te in &d.time_entries {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&te.start_time) {
            let dt_utc = dt.with_timezone(&Utc);
            if dt_utc >= month_ago && te.billable {
                hours_this_month += te.duration_minutes / 60.0;
            }
        }
    }

    let effective_rate = if hours_this_month > 0.0 {
        monthly / hours_this_month
    } else {
        d.profile.hourly_rate
    };

    Ok(EarningsSummary {
        weekly,
        monthly,
        yearly,
        total_invoiced,
        total_paid,
        outstanding,
        hours_this_month,
        effective_rate,
    })
}
