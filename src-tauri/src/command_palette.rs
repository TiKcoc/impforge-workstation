// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Enhanced Command Palette — AI-powered search across modules, actions, and content.
//!
//! Provides ranked results from multiple sources:
//! - Module navigation shortcuts (type "writer" -> ForgeWriter)
//! - Action shortcuts (type "new document" -> creates doc)
//! - Recent actions (most recent first)
//! - Document content search (via global_search)
//!
//! All data is local — no network requests.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single result returned by the command palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub action_type: String, // "navigate", "create", "open_document", "run_command"
    pub route: Option<String>,
    pub score: f32,
}

/// A persisted recent-action entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentAction {
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub action_type: String,
    pub route: Option<String>,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Static registries
// ---------------------------------------------------------------------------

struct ModuleEntry {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    route: &'static str,
    aliases: &'static [&'static str],
}

const MODULES: &[ModuleEntry] = &[
    ModuleEntry { id: "home", label: "Dashboard", icon: "LayoutDashboard", route: "/", aliases: &["dashboard", "overview", "start"] },
    ModuleEntry { id: "chat", label: "Chat", icon: "MessageSquare", route: "/chat", aliases: &["ai", "ask", "conversation"] },
    ModuleEntry { id: "github", label: "GitHub", icon: "GitBranch", route: "/github", aliases: &["git", "repos", "code"] },
    ModuleEntry { id: "docker", label: "Docker", icon: "Container", route: "/docker", aliases: &["containers", "images"] },
    ModuleEntry { id: "workflows", label: "ForgeFlow", icon: "Workflow", route: "/workflows", aliases: &["automation", "flow", "pipeline"] },
    ModuleEntry { id: "ide", label: "CodeForge IDE", icon: "Code2", route: "/ide", aliases: &["editor", "code", "develop"] },
    ModuleEntry { id: "agents", label: "NeuralSwarm", icon: "Network", route: "/agents", aliases: &["agents", "swarm", "orchestrator"] },
    ModuleEntry { id: "evaluation", label: "Evaluation", icon: "Shield", route: "/evaluation", aliases: &["eval", "judge", "quality"] },
    ModuleEntry { id: "ai", label: "AI Models", icon: "Brain", route: "/ai", aliases: &["models", "llm", "inference"] },
    ModuleEntry { id: "ai-lab", label: "AI Lab", icon: "FlaskConical", route: "/ai-lab", aliases: &["experiment", "lab", "moa"] },
    ModuleEntry { id: "router", label: "Model Router", icon: "Route", route: "/router", aliases: &["routing", "model selection"] },
    ModuleEntry { id: "browser", label: "Browser Agent", icon: "Globe", route: "/browser", aliases: &["web", "browse", "scrape"] },
    ModuleEntry { id: "news", label: "AI News", icon: "Newspaper", route: "/news", aliases: &["feed", "rss", "articles"] },
    ModuleEntry { id: "social", label: "Social Media", icon: "Share2", route: "/social", aliases: &["posting", "linkedin", "twitter"] },
    ModuleEntry { id: "writer", label: "ForgeWriter", icon: "FileEdit", route: "/writer", aliases: &["document", "write", "editor", "word"] },
    ModuleEntry { id: "notes", label: "ForgeNotes", icon: "BookOpen", route: "/notes", aliases: &["knowledge", "wiki", "notebook"] },
    ModuleEntry { id: "freelancer", label: "Freelancer", icon: "Briefcase", route: "/freelancer", aliases: &["gig", "client", "invoice"] },
    ModuleEntry { id: "sheets", label: "ForgeSheets", icon: "Table2", route: "/sheets", aliases: &["spreadsheet", "excel", "table"] },
    ModuleEntry { id: "pdf", label: "ForgePDF", icon: "FileText", route: "/pdf", aliases: &["reader", "pdf viewer"] },
    ModuleEntry { id: "canvas", label: "ForgeCanvas", icon: "PenTool", route: "/canvas", aliases: &["design", "draw", "workspace"] },
    ModuleEntry { id: "slides", label: "ForgeSlides", icon: "Presentation", route: "/slides", aliases: &["presentation", "powerpoint", "deck"] },
    ModuleEntry { id: "mail", label: "ForgeMail", icon: "Mail", route: "/mail", aliases: &["email", "inbox", "compose"] },
    ModuleEntry { id: "team", label: "ForgeTeam", icon: "Users", route: "/team", aliases: &["collaboration", "impbook", "p2p"] },
    ModuleEntry { id: "calendar", label: "Calendar", icon: "CalendarDays", route: "/calendar", aliases: &["schedule", "events", "ical"] },
    ModuleEntry { id: "files", label: "File Hub", icon: "FolderOpen", route: "/files", aliases: &["explorer", "filesystem", "folders"] },
    ModuleEntry { id: "settings", label: "Settings", icon: "Settings", route: "/settings", aliases: &["preferences", "config", "options"] },
    ModuleEntry { id: "health", label: "System Health", icon: "Heart", route: "/health", aliases: &["status", "monitoring", "diagnostics"] },
    ModuleEntry { id: "healing", label: "Self-Healing", icon: "ShieldCheck", route: "/healing", aliases: &["repair", "mape-k", "autonomous"] },
];

struct ActionEntry {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    action_type: &'static str,
    route: &'static str,
    aliases: &'static [&'static str],
}

const ACTIONS: &[ActionEntry] = &[
    ActionEntry { id: "new_chat", label: "New Chat", icon: "MessageSquare", action_type: "create", route: "/chat", aliases: &["start chat", "ask ai", "conversation"] },
    ActionEntry { id: "new_document", label: "New Document", icon: "FileEdit", action_type: "create", route: "/writer", aliases: &["create doc", "write", "blank document"] },
    ActionEntry { id: "new_spreadsheet", label: "New Spreadsheet", icon: "Table2", action_type: "create", route: "/sheets", aliases: &["create sheet", "excel", "table"] },
    ActionEntry { id: "new_note", label: "New Note", icon: "BookOpen", action_type: "create", route: "/notes", aliases: &["create note", "wiki", "knowledge"] },
    ActionEntry { id: "new_presentation", label: "New Presentation", icon: "Presentation", action_type: "create", route: "/slides", aliases: &["create slides", "deck", "powerpoint"] },
    ActionEntry { id: "new_workflow", label: "New Workflow", icon: "Workflow", action_type: "create", route: "/workflows", aliases: &["create flow", "automation", "pipeline"] },
    ActionEntry { id: "new_canvas", label: "New Canvas", icon: "PenTool", action_type: "create", route: "/canvas", aliases: &["create canvas", "workspace", "design"] },
    ActionEntry { id: "compose_email", label: "Compose Email", icon: "Mail", action_type: "create", route: "/mail", aliases: &["new email", "write email", "send mail"] },
    ActionEntry { id: "run_agent", label: "Start Agent Task", icon: "Bot", action_type: "run_command", route: "/agents", aliases: &["agent", "task", "swarm"] },
    ActionEntry { id: "toggle_agent_panel", label: "Toggle Agent Panel", icon: "PanelRightOpen", action_type: "run_command", route: "__toggle_agent_panel", aliases: &["side panel", "agent panel"] },
    ActionEntry { id: "toggle_layout", label: "Toggle Layout Edit", icon: "Grid3x3", action_type: "run_command", route: "__toggle_layout", aliases: &["edit mode", "customize", "layout"] },
    ActionEntry { id: "check_health", label: "Run Health Check", icon: "Heart", action_type: "run_command", route: "/health", aliases: &["diagnostics", "system check"] },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

fn recents_path() -> PathBuf {
    data_dir().join("command_palette_recents.json")
}

fn load_recents() -> Vec<RecentAction> {
    let path = recents_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_recents(recents: &[RecentAction]) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let json = serde_json::to_string_pretty(recents).unwrap_or_default();
    let _ = std::fs::write(recents_path(), json);
}

/// Simple fuzzy score: exact prefix > contains > alias match.
fn fuzzy_score(query: &str, label: &str, aliases: &[&str]) -> f32 {
    let q = query.to_lowercase();
    let l = label.to_lowercase();

    if l == q {
        return 1.0;
    }
    if l.starts_with(&q) {
        return 0.9;
    }
    if l.contains(&q) {
        return 0.7;
    }

    // Check aliases
    for alias in aliases {
        let a = alias.to_lowercase();
        if a == q {
            return 0.85;
        }
        if a.starts_with(&q) {
            return 0.75;
        }
        if a.contains(&q) {
            return 0.55;
        }
    }

    // Check individual query terms
    let terms: Vec<&str> = q.split_whitespace().collect();
    if terms.len() > 1 {
        let mut matched = 0;
        for term in &terms {
            if l.contains(term) || aliases.iter().any(|a| a.to_lowercase().contains(term)) {
                matched += 1;
            }
        }
        if matched > 0 {
            return (matched as f32 / terms.len() as f32) * 0.5;
        }
    }

    0.0
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Search the command palette: modules, actions, recent actions, and documents.
#[tauri::command]
pub async fn palette_search(query: String) -> AppResult<Vec<PaletteResult>> {
    let q = query.trim();
    if q.is_empty() {
        // Return recent actions when query is empty
        let recents = load_recents();
        let results: Vec<PaletteResult> = recents
            .into_iter()
            .take(8)
            .enumerate()
            .map(|(i, r)| PaletteResult {
                id: format!("recent_{i}"),
                title: r.title,
                subtitle: r.subtitle,
                icon: r.icon,
                action_type: r.action_type,
                route: r.route,
                score: 1.0 - (i as f32 * 0.05),
            })
            .collect();
        return Ok(results);
    }

    let mut results = Vec::new();

    // 1. Search modules
    for m in MODULES {
        let score = fuzzy_score(q, m.label, m.aliases);
        if score > 0.0 {
            results.push(PaletteResult {
                id: format!("mod_{}", m.id),
                title: m.label.to_string(),
                subtitle: format!("Go to {}", m.label),
                icon: m.icon.to_string(),
                action_type: "navigate".to_string(),
                route: Some(m.route.to_string()),
                score,
            });
        }
    }

    // 2. Search actions
    for a in ACTIONS {
        let score = fuzzy_score(q, a.label, a.aliases);
        if score > 0.0 {
            results.push(PaletteResult {
                id: format!("act_{}", a.id),
                title: a.label.to_string(),
                subtitle: match a.action_type {
                    "create" => format!("Create new {}", a.label.replace("New ", "")),
                    "run_command" => format!("Run: {}", a.label),
                    _ => a.label.to_string(),
                },
                icon: a.icon.to_string(),
                action_type: a.action_type.to_string(),
                route: Some(a.route.to_string()),
                score: score * 0.95, // Slightly below module matches
            });
        }
    }

    // 3. Search recent actions
    let recents = load_recents();
    for (i, r) in recents.iter().enumerate() {
        let score = fuzzy_score(q, &r.title, &[]);
        if score > 0.0 {
            results.push(PaletteResult {
                id: format!("recent_{i}"),
                title: r.title.clone(),
                subtitle: format!("Recent: {}", r.subtitle),
                icon: r.icon.clone(),
                action_type: r.action_type.clone(),
                route: r.route.clone(),
                score: score * 0.8, // Recent actions rank below direct matches
            });
        }
    }

    // 4. Search document content via global_search scanner
    if q.len() >= 2 {
        let search_results = crate::global_search::search_all_modules(q);
        for sr in search_results.into_iter().take(10) {
            results.push(PaletteResult {
                id: format!("doc_{}", sr.id),
                title: sr.title,
                subtitle: sr.preview,
                icon: module_icon(&sr.module),
                action_type: "open_document".to_string(),
                route: Some(sr.route),
                score: sr.relevance * 0.7, // Content results rank below navigation
            });
        }
    }

    // Sort by score descending, cap at 20 results
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(20);

    Ok(results)
}

/// Record a recent action so it appears in the "Recent" section.
#[tauri::command]
pub async fn palette_record_action(
    title: String,
    subtitle: String,
    icon: String,
    action_type: String,
    route: Option<String>,
) -> AppResult<()> {
    let entry = RecentAction {
        title: title.clone(),
        subtitle,
        icon,
        action_type,
        route,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let mut recents = load_recents();

    // Remove duplicate (same title)
    recents.retain(|r| r.title != title);

    // Prepend new entry
    recents.insert(0, entry);

    // Keep at most 30 entries
    recents.truncate(30);

    save_recents(&recents);
    Ok(())
}

/// Get recent actions for the palette "Recent" section.
#[tauri::command]
pub async fn palette_recent_actions(limit: Option<u32>) -> AppResult<Vec<RecentAction>> {
    let recents = load_recents();
    let max = limit.unwrap_or(8) as usize;
    Ok(recents.into_iter().take(max).collect())
}

/// Map module id to icon name.
fn module_icon(module: &str) -> String {
    MODULES
        .iter()
        .find(|m| m.id == module)
        .map(|m| m.icon.to_string())
        .unwrap_or_else(|| "Search".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_score_exact() {
        assert!((fuzzy_score("ForgeWriter", "ForgeWriter", &[]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_fuzzy_score_prefix() {
        let score = fuzzy_score("writ", "ForgeWriter", &["write", "document"]);
        assert!(score > 0.0, "Expected alias match, got {score}");
    }

    #[test]
    fn test_fuzzy_score_alias() {
        let score = fuzzy_score("excel", "ForgeSheets", &["spreadsheet", "excel", "table"]);
        assert!(score >= 0.8, "Expected alias match >= 0.8, got {score}");
    }

    #[test]
    fn test_fuzzy_score_no_match() {
        let score = fuzzy_score("xyznotexist", "ForgeWriter", &["write", "document"]);
        assert!((score - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_module_icon_known() {
        assert_eq!(module_icon("writer"), "FileEdit");
    }

    #[test]
    fn test_module_icon_unknown() {
        assert_eq!(module_icon("nonexistent"), "Search");
    }
}
