// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Global Search -- Unified cross-module search for ImpForge.
//!
//! Queries all ImpForge modules simultaneously and returns ranked results.
//! Searches ForgeMemory, documents (writer, sheets, slides, notes, pdf),
//! calendar events, email subjects, workflow names, team ImpBook entries,
//! and freelancer data.
//!
//! All data lives in local JSON files under `~/.impforge/` or the platform
//! data directory. No network requests are made.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single search result from any module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResult {
    pub id: String,
    pub title: String,
    /// First 200 characters of content.
    pub preview: String,
    /// Source module name (e.g. "writer", "sheets", "notes").
    pub module: String,
    /// Kind of item (e.g. "document", "spreadsheet", "note", "event").
    pub result_type: String,
    /// Frontend route to navigate to (e.g. "/writer", "/sheets?id=xxx").
    pub route: String,
    /// Relevance score from 0.0 to 1.0.
    pub relevance: f32,
    /// ISO 8601 timestamp of last modification.
    pub updated_at: String,
}

/// Metadata about a searchable module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchableModule {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub item_count: u32,
}

// ---------------------------------------------------------------------------
// Data directory helpers
// ---------------------------------------------------------------------------

/// Resolve the ImpForge data directory (`~/.impforge/` or platform equivalent).
fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

/// Scan a directory for JSON files and return their paths.
fn list_json_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect()
}

/// Read a JSON file and return parsed value. Returns None on any error.
fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Truncate a string to at most `max_len` characters, appending "..." if truncated.
fn truncate_preview(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let boundary = s
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..boundary])
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Calculate a simple relevance score based on query term frequency in text.
/// Returns a value between 0.0 and 1.0.
fn score_match(query: &str, title: &str, body: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();
    if terms.is_empty() {
        return 0.0;
    }

    let title_lower = title.to_lowercase();
    let body_lower = body.to_lowercase();

    let mut score: f32 = 0.0;
    let term_weight = 1.0 / terms.len() as f32;

    for term in &terms {
        // Title matches are worth more than body matches.
        if title_lower.contains(term) {
            score += term_weight * 0.6;
        }
        if body_lower.contains(term) {
            score += term_weight * 0.4;
        }
    }

    // Exact title match bonus.
    if title_lower == query_lower {
        score += 0.3;
    } else if title_lower.starts_with(&query_lower) {
        score += 0.15;
    }

    score.min(1.0)
}

// ---------------------------------------------------------------------------
// Per-module scanners
// ---------------------------------------------------------------------------

/// Scan ForgeWriter documents.
fn scan_writer(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("writer");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(doc) = read_json(&path) else { continue };
        let title = doc["title"].as_str().unwrap_or("Untitled");
        let body = doc["content"].as_str().unwrap_or("");
        let id = doc["id"].as_str().unwrap_or("");
        let updated = doc["updated_at"]
            .as_str()
            .or_else(|| doc["created_at"].as_str())
            .unwrap_or("");

        let rel = score_match(query, title, body);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: title.to_string(),
                preview: truncate_preview(body, 200),
                module: "writer".to_string(),
                result_type: "document".to_string(),
                route: format!("/writer?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgeSheets spreadsheets.
fn scan_sheets(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("sheets");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(doc) = read_json(&path) else { continue };
        let title = doc["name"].as_str().unwrap_or("Untitled Spreadsheet");
        let id = doc["id"].as_str().unwrap_or("");
        let updated = doc["updated_at"]
            .as_str()
            .or_else(|| doc["created_at"].as_str())
            .unwrap_or("");

        // Build a search body from sheet names and cell values.
        let mut body = String::new();
        if let Some(sheets) = doc["sheets"].as_array() {
            for sheet in sheets {
                if let Some(name) = sheet["name"].as_str() {
                    body.push_str(name);
                    body.push(' ');
                }
            }
        }

        let rel = score_match(query, title, &body);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: title.to_string(),
                preview: truncate_preview(&body, 200),
                module: "sheets".to_string(),
                result_type: "spreadsheet".to_string(),
                route: format!("/sheets?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgeNotes.
fn scan_notes(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("notes");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(doc) = read_json(&path) else { continue };
        let title = doc["title"].as_str().unwrap_or("Untitled Note");
        let body = doc["content"].as_str().unwrap_or("");
        let id = doc["id"].as_str().unwrap_or("");
        let updated = doc["updated_at"]
            .as_str()
            .or_else(|| doc["created_at"].as_str())
            .unwrap_or("");

        let mut tags_text = String::new();
        if let Some(tags) = doc["tags"].as_array() {
            for tag in tags {
                if let Some(t) = tag.as_str() {
                    tags_text.push_str(t);
                    tags_text.push(' ');
                }
            }
        }
        let full_body = format!("{body} {tags_text}");

        let rel = score_match(query, title, &full_body);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: title.to_string(),
                preview: truncate_preview(body, 200),
                module: "notes".to_string(),
                result_type: "note".to_string(),
                route: format!("/notes?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgeSlides presentations.
fn scan_slides(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("slides");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(doc) = read_json(&path) else { continue };
        let title = doc["title"].as_str().unwrap_or("Untitled Presentation");
        let id = doc["id"].as_str().unwrap_or("");
        let updated = doc["updated_at"]
            .as_str()
            .or_else(|| doc["created_at"].as_str())
            .unwrap_or("");

        let mut body = String::new();
        if let Some(slides) = doc["slides"].as_array() {
            for slide in slides {
                if let Some(content) = slide["content"].as_str() {
                    body.push_str(content);
                    body.push(' ');
                }
            }
        }

        let rel = score_match(query, title, &body);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: title.to_string(),
                preview: truncate_preview(&body, 200),
                module: "slides".to_string(),
                result_type: "presentation".to_string(),
                route: format!("/slides?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgePDF documents.
fn scan_pdf(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("pdf");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(doc) = read_json(&path) else { continue };
        let title = doc["filename"]
            .as_str()
            .or_else(|| doc["title"].as_str())
            .unwrap_or("Untitled PDF");
        let id = doc["id"].as_str().unwrap_or("");
        let updated = doc["imported_at"].as_str().unwrap_or("");

        let body = doc["text_preview"].as_str().unwrap_or("");

        let rel = score_match(query, title, body);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: title.to_string(),
                preview: truncate_preview(body, 200),
                module: "pdf".to_string(),
                result_type: "pdf".to_string(),
                route: format!("/pdf?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgeCalendar events.
fn scan_calendar(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("calendar");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(cal) = read_json(&path) else { continue };
        let events = cal["events"].as_array();
        let cal_name = cal["name"].as_str().unwrap_or("Calendar");

        if let Some(events) = events {
            for event in events {
                let title = event["title"]
                    .as_str()
                    .or_else(|| event["summary"].as_str())
                    .unwrap_or("Untitled Event");
                let id = event["id"].as_str().unwrap_or("");
                let description = event["description"].as_str().unwrap_or("");
                let location = event["location"].as_str().unwrap_or("");
                let start = event["start"].as_str().unwrap_or("");

                let body = format!("{description} {location} {cal_name}");
                let rel = score_match(query, title, &body);
                if rel > 0.05 {
                    results.push(GlobalSearchResult {
                        id: id.to_string(),
                        title: title.to_string(),
                        preview: truncate_preview(&format!("{start} - {description}"), 200),
                        module: "calendar".to_string(),
                        result_type: "event".to_string(),
                        route: "/calendar".to_string(),
                        relevance: rel,
                        updated_at: start.to_string(),
                    });
                }
            }
        }
    }
    results
}

/// Scan ForgeMail email subjects.
fn scan_mail(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("mail");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(account) = read_json(&path) else { continue };
        let emails = account["emails"].as_array();

        if let Some(emails) = emails {
            for email in emails {
                let subject = email["subject"].as_str().unwrap_or("(no subject)");
                let id = email["id"].as_str().unwrap_or("");
                let from = email["from"].as_str().unwrap_or("");
                let snippet = email["snippet"]
                    .as_str()
                    .or_else(|| email["body_preview"].as_str())
                    .unwrap_or("");
                let date = email["date"].as_str().unwrap_or("");

                let body = format!("{from} {snippet}");
                let rel = score_match(query, subject, &body);
                if rel > 0.05 {
                    results.push(GlobalSearchResult {
                        id: id.to_string(),
                        title: subject.to_string(),
                        preview: truncate_preview(&format!("From: {from} - {snippet}"), 200),
                        module: "mail".to_string(),
                        result_type: "email".to_string(),
                        route: format!("/mail?id={id}"),
                        relevance: rel,
                        updated_at: date.to_string(),
                    });
                }
            }
        }
    }
    results
}

/// Scan ForgeFlow workflows.
fn scan_workflows(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("workflows");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(wf) = read_json(&path) else { continue };
        let name = wf["name"].as_str().unwrap_or("Untitled Workflow");
        let id = wf["id"].as_str().unwrap_or("");
        let description = wf["description"].as_str().unwrap_or("");
        let updated = wf["updated_at"]
            .as_str()
            .or_else(|| wf["created_at"].as_str())
            .unwrap_or("");

        let rel = score_match(query, name, description);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: name.to_string(),
                preview: truncate_preview(description, 200),
                module: "workflows".to_string(),
                result_type: "workflow".to_string(),
                route: format!("/workflows?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

/// Scan ForgeCanvas projects.
fn scan_canvas(query: &str) -> Vec<GlobalSearchResult> {
    let dir = data_dir().join("canvas");
    let mut results = Vec::new();

    for path in list_json_files(&dir) {
        let Some(proj) = read_json(&path) else { continue };
        let name = proj["name"].as_str().unwrap_or("Untitled Canvas");
        let id = proj["id"].as_str().unwrap_or("");
        let updated = proj["updated_at"]
            .as_str()
            .or_else(|| proj["created_at"].as_str())
            .unwrap_or("");
        let output = proj["output_text"].as_str().unwrap_or("");

        let rel = score_match(query, name, output);
        if rel > 0.05 {
            results.push(GlobalSearchResult {
                id: id.to_string(),
                title: name.to_string(),
                preview: truncate_preview(output, 200),
                module: "canvas".to_string(),
                result_type: "canvas_project".to_string(),
                route: format!("/canvas?id={id}"),
                relevance: rel,
                updated_at: updated.to_string(),
            });
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Public API (for other modules, e.g. command_palette)
// ---------------------------------------------------------------------------

/// Search across all local modules and return ranked results.
/// This is the non-Tauri-command version usable by other Rust modules.
pub fn search_all_modules(query: &str) -> Vec<GlobalSearchResult> {
    let mut all_results: Vec<GlobalSearchResult> = Vec::new();
    all_results.extend(scan_writer(query));
    all_results.extend(scan_sheets(query));
    all_results.extend(scan_notes(query));
    all_results.extend(scan_slides(query));
    all_results.extend(scan_pdf(query));
    all_results.extend(scan_calendar(query));
    all_results.extend(scan_mail(query));
    all_results.extend(scan_workflows(query));
    all_results.extend(scan_canvas(query));

    all_results.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    all_results.truncate(30);
    all_results
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Search across all ImpForge modules simultaneously.
///
/// Returns results sorted by relevance (descending), limited to `limit`
/// entries (default 20).
#[tauri::command]
pub async fn global_search(query: String, limit: Option<u32>) -> AppResult<Vec<GlobalSearchResult>> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_QUERY",
            "Search query cannot be empty",
        ));
    }

    let max = limit.unwrap_or(20).min(100) as usize;

    // Run all module scanners (all local filesystem, no async needed).
    let mut all_results: Vec<GlobalSearchResult> = Vec::new();
    all_results.extend(scan_writer(&query));
    all_results.extend(scan_sheets(&query));
    all_results.extend(scan_notes(&query));
    all_results.extend(scan_slides(&query));
    all_results.extend(scan_pdf(&query));
    all_results.extend(scan_calendar(&query));
    all_results.extend(scan_mail(&query));
    all_results.extend(scan_workflows(&query));
    all_results.extend(scan_canvas(&query));

    // Sort by relevance descending, then by updated_at descending.
    all_results.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    all_results.truncate(max);

    Ok(all_results)
}

/// Return metadata about all searchable modules with item counts.
#[tauri::command]
pub async fn global_search_modules() -> AppResult<Vec<SearchableModule>> {
    let base = data_dir();

    let modules = vec![
        ("writer", "ForgeWriter", "file-edit"),
        ("sheets", "ForgeSheets", "table-2"),
        ("notes", "ForgeNotes", "book-open"),
        ("slides", "ForgeSlides", "presentation"),
        ("pdf", "ForgePDF", "file-text"),
        ("calendar", "Calendar", "calendar-days"),
        ("mail", "ForgeMail", "mail"),
        ("workflows", "ForgeFlow", "workflow"),
        ("canvas", "ForgeCanvas", "pen-tool"),
    ];

    let result: Vec<SearchableModule> = modules
        .into_iter()
        .map(|(id, name, icon)| {
            let dir = base.join(id);
            let count = list_json_files(&dir).len() as u32;
            SearchableModule {
                id: id.to_string(),
                name: name.to_string(),
                icon: icon.to_string(),
                item_count: count,
            }
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_exact_title_match() {
        let score = score_match("project plan", "Project Plan", "");
        assert!(score > 0.8, "Exact title match should score high: {score}");
    }

    #[test]
    fn test_score_partial_match() {
        let score = score_match("rust", "Rust Programming", "A guide to Rust");
        assert!(score > 0.5, "Partial match should score reasonably: {score}");
    }

    #[test]
    fn test_score_no_match() {
        let score = score_match("quantum physics", "Grocery List", "eggs milk bread");
        assert!(score < 0.01, "No match should score near zero: {score}");
    }

    #[test]
    fn test_score_body_only() {
        let score = score_match("budget", "Quarterly Report", "the budget was exceeded");
        assert!(score > 0.1, "Body-only match should still score: {score}");
        assert!(score < 0.7, "Body-only should score lower than title: {score}");
    }

    #[test]
    fn test_score_empty_query() {
        assert_eq!(score_match("", "title", "body"), 0.0);
    }

    #[test]
    fn test_score_multi_term() {
        let score = score_match("rust async", "Async Rust Guide", "Learn async in Rust");
        assert!(score > 0.7, "Multi-term match should score well: {score}");
    }

    #[test]
    fn test_truncate_preview_short() {
        assert_eq!(truncate_preview("hello", 200), "hello");
    }

    #[test]
    fn test_truncate_preview_long() {
        let long = "a".repeat(300);
        let result = truncate_preview(&long, 200);
        assert!(result.len() <= 204); // 200 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_exact() {
        let exact = "b".repeat(200);
        let result = truncate_preview(&exact, 200);
        assert_eq!(result.len(), 200);
        assert!(!result.ends_with("..."));
    }

    #[test]
    fn test_list_json_files_missing_dir() {
        let files = list_json_files(std::path::Path::new("/nonexistent/path"));
        assert!(files.is_empty());
    }

    #[test]
    fn test_searchable_module_serialization() {
        let m = SearchableModule {
            id: "writer".to_string(),
            name: "ForgeWriter".to_string(),
            icon: "file-edit".to_string(),
            item_count: 5,
        };
        let json = serde_json::to_string(&m).expect("should serialize");
        assert!(json.contains("ForgeWriter"));
        assert!(json.contains("file-edit"));
    }

    #[test]
    fn test_global_search_result_serialization() {
        let r = GlobalSearchResult {
            id: "abc-123".to_string(),
            title: "My Document".to_string(),
            preview: "First 200 chars...".to_string(),
            module: "writer".to_string(),
            result_type: "document".to_string(),
            route: "/writer?id=abc-123".to_string(),
            relevance: 0.85,
            updated_at: "2026-03-18T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&r).expect("should serialize");
        let parsed: GlobalSearchResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(parsed.module, "writer");
        assert!((parsed.relevance - 0.85).abs() < f32::EPSILON);
    }
}
