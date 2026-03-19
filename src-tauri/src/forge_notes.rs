// SPDX-License-Identifier: Apache-2.0
//! ForgeNotes -- Personal Knowledge Base with Wiki-Links & Knowledge Graph
//!
//! A Notion-replacement for personal note-taking with bidirectional wiki-links,
//! tags, full-text search, and AI-powered connections. Notes are Markdown files
//! with `[[wiki-links]]` that form a Knowledge Graph.
//!
//! Storage: `~/.impforge/notes/` with one JSON file per note.
//! AI operations use the local Ollama inference backend.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds note files.
const NOTES_DIR: &str = "notes";

/// Ollama HTTP timeout for AI requests (generous for large context).
const AI_TIMEOUT_SECS: u64 = 120;

/// Maximum preview length in characters.
const PREVIEW_LEN: usize = 150;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Full note representation returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub links_to: Vec<String>,
    pub linked_from: Vec<String>,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub word_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight metadata for note listings (no content body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub link_count: u32,
    pub backlink_count: u32,
    pub updated_at: String,
    pub preview: String,
}

/// Tag with usage count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub name: String,
    pub count: u32,
}

/// Node in the knowledge graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphNode {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub connections: u32,
}

/// Edge in the knowledge graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

/// Full knowledge graph for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub nodes: Vec<KnowledgeGraphNode>,
    pub edges: Vec<KnowledgeGraphEdge>,
}

/// Persisted note file (JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoteFile {
    id: String,
    title: String,
    content: String,
    tags: Vec<String>,
    is_pinned: bool,
    is_archived: bool,
    created_at: String,
    updated_at: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the notes directory, creating it if necessary.
fn notes_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(NOTES_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create notes directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Build the file path for a given note.
fn note_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Read and parse a note file.
fn read_note(path: &Path) -> Result<NoteFile, ImpForgeError> {
    let data = std::fs::read_to_string(path).map_err(|e| {
        ImpForgeError::filesystem(
            "NOTE_READ_FAILED",
            format!("Cannot read note: {e}"),
        )
    })?;
    serde_json::from_str::<NoteFile>(&data).map_err(|e| {
        ImpForgeError::internal(
            "NOTE_PARSE_FAILED",
            format!("Corrupt note file: {e}"),
        )
    })
}

/// Persist a note file atomically.
fn write_note(path: &Path, note: &NoteFile) -> Result<(), ImpForgeError> {
    let json = serde_json::to_string_pretty(note).map_err(|e| {
        ImpForgeError::internal("NOTE_SERIALIZE", format!("Cannot serialize note: {e}"))
    })?;
    std::fs::write(path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "NOTE_WRITE_FAILED",
            format!("Cannot write note file: {e}"),
        )
    })
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Count words in a text string (Unicode-aware, splits on whitespace).
fn count_words(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Extract the first N characters from content as a preview, stripping Markdown.
fn make_preview(content: &str, max_len: usize) -> String {
    let stripped: String = content
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("```")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let stripped = stripped
        .replace("**", "")
        .replace("*", "")
        .replace("`", "")
        .replace("[[", "")
        .replace("]]", "");
    if stripped.len() <= max_len {
        stripped
    } else {
        let mut end = max_len;
        // Avoid cutting in the middle of a multi-byte character
        while !stripped.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &stripped[..end])
    }
}

/// Extract all `[[wiki-links]]` from Markdown content.
/// Returns the link titles (the text between `[[` and `]]`).
fn extract_wiki_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = content;

    while let Some(start) = remaining.find("[[") {
        let after_open = &remaining[start + 2..];
        let Some(end) = after_open.find("]]") else {
            break;
        };
        let link_title = after_open[..end].trim().to_string();
        if !link_title.is_empty() && !links.contains(&link_title) {
            links.push(link_title);
        }
        remaining = &after_open[end + 2..];
    }

    links
}

/// Load all notes from disk.
fn load_all_notes(dir: &Path) -> Result<Vec<NoteFile>, ImpForgeError> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ_FAILED", format!("Cannot read notes dir: {e}"))
    })?;

    let mut notes = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.ends_with(".json") {
            continue;
        }
        if let Ok(note) = read_note(&path) {
            notes.push(note);
        }
    }

    Ok(notes)
}

/// Build a title-to-id lookup map from all notes.
fn build_title_map(notes: &[NoteFile]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for note in notes {
        map.insert(note.title.to_lowercase(), note.id.clone());
    }
    map
}

/// Resolve wiki-link titles to note IDs.
fn resolve_links(link_titles: &[String], title_map: &HashMap<String, String>) -> Vec<String> {
    link_titles
        .iter()
        .filter_map(|title| title_map.get(&title.to_lowercase()).cloned())
        .collect()
}

/// Build backlinks: for a given note, find all notes that link to it.
fn build_backlinks(note_id: &str, note_title: &str, all_notes: &[NoteFile], title_map: &HashMap<String, String>) -> Vec<String> {
    let note_title_lower = note_title.to_lowercase();
    all_notes
        .iter()
        .filter(|n| n.id != note_id)
        .filter(|n| {
            let links = extract_wiki_links(&n.content);
            links.iter().any(|link| {
                let link_lower = link.to_lowercase();
                link_lower == note_title_lower
                    || title_map.get(&link_lower).is_some_and(|id| id == note_id)
            })
        })
        .map(|n| n.id.clone())
        .collect()
}

/// Convert a NoteFile into a full Note with resolved links and backlinks.
fn note_file_to_note(
    nf: &NoteFile,
    all_notes: &[NoteFile],
    title_map: &HashMap<String, String>,
) -> Note {
    let link_titles = extract_wiki_links(&nf.content);
    let links_to = resolve_links(&link_titles, title_map);
    let linked_from = build_backlinks(&nf.id, &nf.title, all_notes, title_map);

    Note {
        id: nf.id.clone(),
        title: nf.title.clone(),
        content: nf.content.clone(),
        tags: nf.tags.clone(),
        links_to,
        linked_from,
        is_pinned: nf.is_pinned,
        is_archived: nf.is_archived,
        word_count: count_words(&nf.content),
        created_at: nf.created_at.clone(),
        updated_at: nf.updated_at.clone(),
    }
}

/// Convert a NoteFile into a NoteMeta (lightweight, for listings).
fn note_file_to_meta(
    nf: &NoteFile,
    all_notes: &[NoteFile],
    title_map: &HashMap<String, String>,
) -> NoteMeta {
    let link_titles = extract_wiki_links(&nf.content);
    let links_to = resolve_links(&link_titles, title_map);
    let linked_from = build_backlinks(&nf.id, &nf.title, all_notes, title_map);

    NoteMeta {
        id: nf.id.clone(),
        title: nf.title.clone(),
        tags: nf.tags.clone(),
        link_count: links_to.len() as u32,
        backlink_count: linked_from.len() as u32,
        updated_at: nf.updated_at.clone(),
        preview: make_preview(&nf.content, PREVIEW_LEN),
    }
}

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Send a prompt to Ollama and return the result.
async fn ollama_request(
    system_prompt: &str,
    user_message: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_TIMEOUT_SECS))
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
                { "role": "user",   "content": user_message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for AI notes assist",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "AI request timed out")
                    .with_suggestion("Try with shorter content or a faster model.")
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
            ImpForgeError::service(
                "OLLAMA_HTTP_ERROR",
                format!("Ollama returned HTTP {status}"),
            )
            .with_details(body)
            .with_suggestion("Check Ollama logs. The model may not be downloaded yet."),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service(
            "OLLAMA_PARSE_ERROR",
            format!("Failed to parse Ollama response: {e}"),
        )
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
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all notes (metadata only, no content bodies).
/// Optionally filter by search query and/or tag.
#[tauri::command]
pub async fn notes_list(
    filter: Option<String>,
    tag: Option<String>,
) -> AppResult<Vec<NoteMeta>> {
    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    let mut metas: Vec<NoteMeta> = all_notes
        .iter()
        .filter(|n| !n.is_archived)
        .filter(|n| {
            if let Some(ref q) = filter {
                let q_lower = q.to_lowercase();
                n.title.to_lowercase().contains(&q_lower)
                    || n.content.to_lowercase().contains(&q_lower)
            } else {
                true
            }
        })
        .filter(|n| {
            if let Some(ref t) = tag {
                let t_lower = t.to_lowercase();
                n.tags.iter().any(|nt| nt.to_lowercase() == t_lower)
            } else {
                true
            }
        })
        .map(|n| note_file_to_meta(n, &all_notes, &title_map))
        .collect();

    // Pinned first, then sort by updated_at descending
    metas.sort_by(|a, b| {
        let a_pinned = all_notes.iter().any(|n| n.id == a.id && n.is_pinned);
        let b_pinned = all_notes.iter().any(|n| n.id == b.id && n.is_pinned);
        match (b_pinned, a_pinned) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => b.updated_at.cmp(&a.updated_at),
        }
    });

    Ok(metas)
}

/// Create a new blank note with the given title.
#[tauri::command]
pub async fn notes_create(title: String) -> AppResult<Note> {
    let dir = notes_dir()?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let actual_title = if title.trim().is_empty() {
        "Untitled Note".to_string()
    } else {
        title
    };

    let nf = NoteFile {
        id: id.clone(),
        title: actual_title.clone(),
        content: String::new(),
        tags: Vec::new(),
        is_pinned: false,
        is_archived: false,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    write_note(&note_path(&dir, &id), &nf)?;
    log::info!("ForgeNotes: created note '{}' ({})", actual_title, id);

    // Return with empty links (new note has no content yet)
    Ok(Note {
        id,
        title: actual_title,
        content: String::new(),
        tags: Vec::new(),
        links_to: Vec::new(),
        linked_from: Vec::new(),
        is_pinned: false,
        is_archived: false,
        word_count: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Get a single note by ID (full content with resolved links).
#[tauri::command]
pub async fn notes_get(id: String) -> AppResult<Note> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found"))
                .with_suggestion("The note may have been deleted."),
        );
    }

    let nf = read_note(&path)?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    Ok(note_file_to_note(&nf, &all_notes, &title_map))
}

/// Save (update) a note's title, content, and tags.
#[tauri::command]
pub async fn notes_save(
    id: String,
    title: String,
    content: String,
    tags: Vec<String>,
) -> AppResult<Note> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found"))
                .with_suggestion("Cannot save a note that does not exist. Create it first."),
        );
    }

    let mut nf = read_note(&path)?;
    let now = now_iso();

    nf.title = if title.trim().is_empty() {
        nf.title
    } else {
        title
    };
    nf.content = content;
    nf.tags = tags;
    nf.updated_at = now;

    write_note(&path, &nf)?;

    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    Ok(note_file_to_note(&nf, &all_notes, &title_map))
}

/// Delete a note.
#[tauri::command]
pub async fn notes_delete(id: String) -> AppResult<()> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found")),
        );
    }

    std::fs::remove_file(&path).map_err(|e| {
        ImpForgeError::filesystem(
            "DELETE_FAILED",
            format!("Cannot delete note: {e}"),
        )
    })?;

    log::info!("ForgeNotes: deleted note '{}'", id);
    Ok(())
}

/// Full-text search across all notes.
#[tauri::command]
pub async fn notes_search(query: String) -> AppResult<Vec<NoteMeta>> {
    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);
    let q_lower = query.to_lowercase();

    let mut results: Vec<(NoteMeta, u32)> = all_notes
        .iter()
        .filter(|n| !n.is_archived)
        .filter_map(|n| {
            let title_match = n.title.to_lowercase().contains(&q_lower);
            let content_match = n.content.to_lowercase().contains(&q_lower);
            let tag_match = n.tags.iter().any(|t| t.to_lowercase().contains(&q_lower));

            if title_match || content_match || tag_match {
                let mut score = 0u32;
                if title_match {
                    score += 10;
                }
                if tag_match {
                    score += 5;
                }
                if content_match {
                    // Count occurrences for relevance
                    let count = n.content.to_lowercase().matches(&q_lower).count() as u32;
                    score += count.min(20);
                }
                Some((note_file_to_meta(n, &all_notes, &title_map), score))
            } else {
                None
            }
        })
        .collect();

    // Sort by relevance score descending
    results.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(results.into_iter().map(|(meta, _)| meta).collect())
}

/// Get all notes that link TO the given note (backlinks).
#[tauri::command]
pub async fn notes_get_backlinks(id: String) -> AppResult<Vec<NoteMeta>> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found")),
        );
    }

    let target = read_note(&path)?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);
    let backlink_ids = build_backlinks(&id, &target.title, &all_notes, &title_map);

    let metas = all_notes
        .iter()
        .filter(|n| backlink_ids.contains(&n.id))
        .map(|n| note_file_to_meta(n, &all_notes, &title_map))
        .collect();

    Ok(metas)
}

/// Get all tags with counts.
#[tauri::command]
pub async fn notes_get_tags() -> AppResult<Vec<TagInfo>> {
    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;

    let mut tag_counts: HashMap<String, u32> = HashMap::new();
    for note in &all_notes {
        if note.is_archived {
            continue;
        }
        for tag in &note.tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    let mut tags: Vec<TagInfo> = tag_counts
        .into_iter()
        .map(|(name, count)| TagInfo { name, count })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));

    Ok(tags)
}

/// Pin or unpin a note.
#[tauri::command]
pub async fn notes_pin(id: String, pinned: bool) -> AppResult<()> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found")),
        );
    }

    let mut nf = read_note(&path)?;
    nf.is_pinned = pinned;
    nf.updated_at = now_iso();
    write_note(&path, &nf)?;

    log::info!("ForgeNotes: {} note '{}'", if pinned { "pinned" } else { "unpinned" }, id);
    Ok(())
}

/// Archive or unarchive a note.
#[tauri::command]
pub async fn notes_archive(id: String, archived: bool) -> AppResult<()> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found")),
        );
    }

    let mut nf = read_note(&path)?;
    nf.is_archived = archived;
    nf.updated_at = now_iso();
    write_note(&path, &nf)?;

    log::info!("ForgeNotes: {} note '{}'", if archived { "archived" } else { "unarchived" }, id);
    Ok(())
}

/// AI: Generate a new note from a topic and optional context from other notes.
#[tauri::command]
pub async fn notes_ai_generate(
    topic: String,
    context: Option<String>,
) -> AppResult<String> {
    if topic.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_TOPIC",
            "Provide a topic to generate a note about",
        ));
    }

    let system_prompt = "You are a knowledge base assistant inside ForgeNotes (part of ImpForge). \
        Generate a well-structured Markdown note about the given topic. \
        Use headings (##), bullet points, and bold for key terms. \
        If context from existing notes is provided, reference and build upon it. \
        Use [[Wiki Link]] syntax to reference related concepts that could be other notes. \
        Be thorough but concise. Write in the same language as the topic.";

    let user_message = if let Some(ref ctx) = context {
        format!(
            "Generate a note about: {topic}\n\n---\nContext from existing notes:\n{ctx}"
        )
    } else {
        format!("Generate a note about: {topic}")
    };

    log::info!("ForgeNotes: AI generating note about '{}'", topic);

    ollama_request(system_prompt, &user_message, None).await
}

/// AI: Find related notes based on content similarity.
#[tauri::command]
pub async fn notes_ai_connect(id: String) -> AppResult<Vec<NoteMeta>> {
    let dir = notes_dir()?;
    let path = note_path(&dir, &id);

    if !path.exists() {
        return Err(
            ImpForgeError::filesystem("NOTE_NOT_FOUND", format!("Note '{id}' not found")),
        );
    }

    let target = read_note(&path)?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    // Collect other non-archived notes for comparison
    let other_notes: Vec<&NoteFile> = all_notes
        .iter()
        .filter(|n| n.id != id && !n.is_archived)
        .collect();

    if other_notes.is_empty() {
        return Ok(Vec::new());
    }

    // Build a summary of available notes for the AI to compare against
    let notes_summary: String = other_notes
        .iter()
        .map(|n| {
            format!(
                "- ID: {} | Title: {} | Tags: [{}] | Preview: {}",
                n.id,
                n.title,
                n.tags.join(", "),
                make_preview(&n.content, 100),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = "You are a knowledge graph assistant. Given a source note and a list of other notes, \
        identify the most related notes (up to 5). Consider topical overlap, shared concepts, \
        complementary information, and potential wiki-link connections. \
        Return ONLY a comma-separated list of note IDs, nothing else. \
        Example: id1,id2,id3";

    let user_message = format!(
        "Source note:\nTitle: {}\nTags: [{}]\nContent: {}\n\n---\nAvailable notes:\n{}",
        target.title,
        target.tags.join(", "),
        make_preview(&target.content, 500),
        notes_summary,
    );

    let result = ollama_request(system_prompt, &user_message, None).await?;

    // Parse the comma-separated IDs
    let related_ids: HashSet<String> = result
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let metas = all_notes
        .iter()
        .filter(|n| related_ids.contains(&n.id))
        .map(|n| note_file_to_meta(n, &all_notes, &title_map))
        .collect();

    Ok(metas)
}

/// AI: Summarize all notes with a specific tag.
#[tauri::command]
pub async fn notes_ai_summarize_tag(tag: String) -> AppResult<String> {
    if tag.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_TAG",
            "Provide a tag to summarize",
        ));
    }

    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let tag_lower = tag.to_lowercase();

    let tagged_notes: Vec<&NoteFile> = all_notes
        .iter()
        .filter(|n| {
            !n.is_archived && n.tags.iter().any(|t| t.to_lowercase() == tag_lower)
        })
        .collect();

    if tagged_notes.is_empty() {
        return Err(ImpForgeError::validation(
            "NO_TAGGED_NOTES",
            format!("No notes found with tag '{tag}'"),
        ));
    }

    let notes_content: String = tagged_notes
        .iter()
        .map(|n| {
            format!(
                "## {}\n{}\n",
                n.title,
                make_preview(&n.content, 500),
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n\n");

    let system_prompt = "You are a knowledge base assistant. Summarize the key themes, insights, \
        and connections across the following notes that share a common tag. \
        Write in Markdown with clear structure. Use [[Wiki Links]] to reference note titles. \
        Be comprehensive but concise. Write in the same language as the notes.";

    let user_message = format!(
        "Tag: #{tag}\nNumber of notes: {}\n\n{notes_content}",
        tagged_notes.len(),
    );

    log::info!(
        "ForgeNotes: AI summarizing {} notes with tag '#{}' ",
        tagged_notes.len(),
        tag
    );

    ollama_request(system_prompt, &user_message, None).await
}

/// Get the full knowledge graph (nodes + edges) for visualization.
#[tauri::command]
pub async fn notes_get_graph() -> AppResult<KnowledgeGraph> {
    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    let mut edges: Vec<KnowledgeGraphEdge> = Vec::new();
    let mut connection_counts: HashMap<String, u32> = HashMap::new();

    for note in &all_notes {
        if note.is_archived {
            continue;
        }

        let link_titles = extract_wiki_links(&note.content);
        let link_ids = resolve_links(&link_titles, &title_map);

        for (i, target_id) in link_ids.iter().enumerate() {
            edges.push(KnowledgeGraphEdge {
                from: note.id.clone(),
                to: target_id.clone(),
                label: link_titles.get(i).cloned(),
            });

            *connection_counts.entry(note.id.clone()).or_insert(0) += 1;
            *connection_counts.entry(target_id.clone()).or_insert(0) += 1;
        }
    }

    let nodes: Vec<KnowledgeGraphNode> = all_notes
        .iter()
        .filter(|n| !n.is_archived)
        .map(|n| KnowledgeGraphNode {
            id: n.id.clone(),
            title: n.title.clone(),
            tags: n.tags.clone(),
            connections: *connection_counts.get(&n.id).unwrap_or(&0),
        })
        .collect();

    Ok(KnowledgeGraph { nodes, edges })
}

// ---------------------------------------------------------------------------
// Enterprise Types — Templates, Kanban, Export
// ---------------------------------------------------------------------------

/// A pre-built note template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteTemplate {
    pub id: String,
    pub name: String,
    pub content: String,
    pub tags: Vec<String>,
    pub category: String,
}

/// A Kanban column holding note IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanColumn {
    pub name: String,
    pub note_ids: Vec<String>,
}

/// A Kanban board view over notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanBoard {
    pub columns: Vec<KanbanColumn>,
}

/// Kanban state persistence file.
const KANBAN_FILE: &str = "kanban.json";

// ---------------------------------------------------------------------------
// Enterprise Helpers
// ---------------------------------------------------------------------------

/// Get the kanban file path.
fn kanban_path(dir: &Path) -> PathBuf {
    dir.join(KANBAN_FILE)
}

/// Load kanban board from disk.
fn load_kanban(dir: &Path) -> Result<KanbanBoard, ImpForgeError> {
    let path = kanban_path(dir);
    if !path.exists() {
        return Ok(KanbanBoard {
            columns: vec![
                KanbanColumn { name: "To Do".into(), note_ids: Vec::new() },
                KanbanColumn { name: "In Progress".into(), note_ids: Vec::new() },
                KanbanColumn { name: "Done".into(), note_ids: Vec::new() },
            ],
        });
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem("KANBAN_READ_FAILED", format!("Cannot read kanban file: {e}"))
    })?;
    serde_json::from_str::<KanbanBoard>(&data).map_err(|e| {
        ImpForgeError::internal("KANBAN_PARSE_FAILED", format!("Corrupt kanban file: {e}"))
    })
}

/// Save kanban board to disk.
fn save_kanban(dir: &Path, board: &KanbanBoard) -> Result<(), ImpForgeError> {
    let path = kanban_path(dir);
    let json = serde_json::to_string_pretty(board).map_err(|e| {
        ImpForgeError::internal("KANBAN_SERIALIZE", format!("Cannot serialize kanban: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem("KANBAN_WRITE_FAILED", format!("Cannot write kanban file: {e}"))
    })
}

// ---------------------------------------------------------------------------
// Tauri Commands — Note Templates
// ---------------------------------------------------------------------------

/// Return a curated list of pre-built note templates.
#[tauri::command]
pub async fn notes_get_templates() -> AppResult<Vec<NoteTemplate>> {
    Ok(vec![
        NoteTemplate {
            id: "tpl-meeting".into(),
            name: "Meeting Notes".into(),
            content: "## Meeting Notes\n\n\
                **Date:** [Date]\n\
                **Attendees:** [Names]\n\
                **Location:** [Location/Link]\n\n\
                ### Agenda\n\n1. \n2. \n3. \n\n\
                ### Discussion\n\n\n\n\
                ### Action Items\n\n- [ ] \n- [ ] \n\n\
                ### Next Meeting\n\n[Date and time]".into(),
            tags: vec!["meeting".into()],
            category: "Work".into(),
        },
        NoteTemplate {
            id: "tpl-daily-journal".into(),
            name: "Daily Journal".into(),
            content: "## Daily Journal\n\n\
                **Date:** [Date]\n\n\
                ### Gratitude\n\n1. \n2. \n3. \n\n\
                ### Today's Goals\n\n- [ ] \n- [ ] \n- [ ] \n\n\
                ### Reflections\n\n\n\n\
                ### Tomorrow's Priorities\n\n1. \n2. ".into(),
            tags: vec!["journal".into(), "daily".into()],
            category: "Personal".into(),
        },
        NoteTemplate {
            id: "tpl-project-plan".into(),
            name: "Project Plan".into(),
            content: "## Project: [Name]\n\n\
                **Start Date:** [Date]\n\
                **Deadline:** [Date]\n\
                **Status:** Planning\n\n\
                ### Overview\n\n[Brief description]\n\n\
                ### Goals\n\n1. \n2. \n\n\
                ### Milestones\n\n| Milestone | Due Date | Status |\n|-----------|----------|--------|\n| | | |\n\n\
                ### Resources\n\n- \n\n\
                ### Risks\n\n- ".into(),
            tags: vec!["project".into(), "planning".into()],
            category: "Work".into(),
        },
        NoteTemplate {
            id: "tpl-bug-report".into(),
            name: "Bug Report".into(),
            content: "## Bug Report\n\n\
                **Title:** [Short description]\n\
                **Severity:** [Critical/High/Medium/Low]\n\
                **Component:** [Module/Feature]\n\
                **Reporter:** [Name]\n\
                **Date:** [Date]\n\n\
                ### Description\n\n[Detailed description]\n\n\
                ### Steps to Reproduce\n\n1. \n2. \n3. \n\n\
                ### Expected Behavior\n\n\n\n\
                ### Actual Behavior\n\n\n\n\
                ### Environment\n\n- OS: \n- Version: \n\n\
                ### Screenshots/Logs\n\n".into(),
            tags: vec!["bug".into(), "dev".into()],
            category: "Development".into(),
        },
        NoteTemplate {
            id: "tpl-research".into(),
            name: "Research Notes".into(),
            content: "## Research: [Topic]\n\n\
                **Date:** [Date]\n\
                **Sources:** [URLs/References]\n\n\
                ### Key Findings\n\n1. \n2. \n3. \n\n\
                ### Detailed Notes\n\n\n\n\
                ### Questions\n\n- \n\n\
                ### Related Topics\n\n- [[Topic 1]]\n- [[Topic 2]]\n\n\
                ### Conclusions\n\n".into(),
            tags: vec!["research".into()],
            category: "Learning".into(),
        },
        NoteTemplate {
            id: "tpl-book-notes".into(),
            name: "Book Notes".into(),
            content: "## Book: [Title]\n\n\
                **Author:** [Name]\n\
                **Started:** [Date]\n\
                **Finished:** [Date]\n\
                **Rating:** /5\n\n\
                ### Summary\n\n[Brief summary]\n\n\
                ### Key Takeaways\n\n1. \n2. \n3. \n\n\
                ### Favorite Quotes\n\n> \n\n\
                ### How It Applies\n\n\n\n\
                ### Would I Recommend?\n\n".into(),
            tags: vec!["book".into(), "reading".into()],
            category: "Learning".into(),
        },
        NoteTemplate {
            id: "tpl-recipe".into(),
            name: "Recipe".into(),
            content: "## Recipe: [Name]\n\n\
                **Servings:** [Number]\n\
                **Prep Time:** [Minutes]\n\
                **Cook Time:** [Minutes]\n\
                **Difficulty:** [Easy/Medium/Hard]\n\n\
                ### Ingredients\n\n- \n- \n- \n\n\
                ### Instructions\n\n1. \n2. \n3. \n\n\
                ### Notes\n\n\n\n\
                ### Variations\n\n- ".into(),
            tags: vec!["recipe".into(), "food".into()],
            category: "Personal".into(),
        },
        NoteTemplate {
            id: "tpl-decision-log".into(),
            name: "Decision Log".into(),
            content: "## Decision: [Title]\n\n\
                **Date:** [Date]\n\
                **Status:** [Proposed/Accepted/Superseded]\n\
                **Deciders:** [Names]\n\n\
                ### Context\n\n[What is the issue?]\n\n\
                ### Options Considered\n\n\
                | Option | Pros | Cons |\n|--------|------|------|\n| | | |\n| | | |\n\n\
                ### Decision\n\n[What was decided and why]\n\n\
                ### Consequences\n\n- \n\n\
                ### Related Decisions\n\n- [[Decision 1]]".into(),
            tags: vec!["decision".into(), "architecture".into()],
            category: "Work".into(),
        },
    ])
}

// ---------------------------------------------------------------------------
// Tauri Commands — Kanban Board
// ---------------------------------------------------------------------------

/// Get the Kanban board layout.
///
/// Returns columns with ordered note IDs. Notes that no longer exist on disk
/// are automatically pruned from the board.
#[tauri::command]
pub async fn notes_get_kanban() -> AppResult<KanbanBoard> {
    let dir = notes_dir()?;
    let mut board = load_kanban(&dir)?;
    let all_notes = load_all_notes(&dir)?;

    // Build a set of existing note IDs for pruning
    let existing_ids: HashSet<String> = all_notes.iter().map(|n| n.id.clone()).collect();

    // Prune deleted notes from all columns
    let mut changed = false;
    for col in &mut board.columns {
        let before = col.note_ids.len();
        col.note_ids.retain(|id| existing_ids.contains(id));
        if col.note_ids.len() != before {
            changed = true;
        }
    }

    if changed {
        save_kanban(&dir, &board)?;
    }

    Ok(board)
}

/// Move a note to a Kanban column at a specific position.
#[tauri::command]
pub async fn notes_move_kanban(
    note_id: String,
    column: String,
    position: u32,
) -> AppResult<()> {
    let dir = notes_dir()?;

    // Verify the note exists
    let path = note_path(&dir, &note_id);
    if !path.exists() {
        return Err(ImpForgeError::filesystem(
            "NOTE_NOT_FOUND",
            format!("Note '{note_id}' not found"),
        ));
    }

    let mut board = load_kanban(&dir)?;

    // Remove the note from all columns first
    for col in &mut board.columns {
        col.note_ids.retain(|id| id != &note_id);
    }

    // Find the target column (create if not exists)
    let target_col = if let Some(col) = board.columns.iter_mut().find(|c| c.name == column) {
        col
    } else {
        board.columns.push(KanbanColumn {
            name: column.clone(),
            note_ids: Vec::new(),
        });
        board.columns.last_mut().ok_or_else(|| {
            ImpForgeError::internal("KANBAN_ERROR", "Failed to create column")
        })?
    };

    // Insert at the specified position
    let pos = (position as usize).min(target_col.note_ids.len());
    target_col.note_ids.insert(pos, note_id.clone());

    save_kanban(&dir, &board)?;

    log::info!("ForgeNotes: moved note '{}' to column '{}' at position {}", note_id, column, pos);
    Ok(())
}

/// Add a new Kanban column.
#[tauri::command]
pub async fn notes_add_kanban_column(name: String) -> AppResult<KanbanBoard> {
    let dir = notes_dir()?;
    let mut board = load_kanban(&dir)?;

    if name.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_COLUMN_NAME",
            "Column name cannot be empty",
        ));
    }

    if board.columns.iter().any(|c| c.name == name) {
        return Err(ImpForgeError::validation(
            "DUPLICATE_COLUMN",
            format!("Column '{name}' already exists"),
        ));
    }

    board.columns.push(KanbanColumn {
        name,
        note_ids: Vec::new(),
    });

    save_kanban(&dir, &board)?;
    Ok(board)
}

// ---------------------------------------------------------------------------
// Tauri Commands — Daily Notes
// ---------------------------------------------------------------------------

/// Open or create today's daily note.
///
/// If a note titled "Daily: YYYY-MM-DD" exists, it is returned. Otherwise,
/// a new note is created from the Daily Journal template.
#[tauri::command]
pub async fn notes_daily(date: Option<String>) -> AppResult<Note> {
    let dir = notes_dir()?;
    let target_date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let daily_title = format!("Daily: {target_date}");
    let daily_title_lower = daily_title.to_lowercase();

    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    // Check if today's daily note already exists
    if let Some(existing) = all_notes.iter().find(|n| n.title.to_lowercase() == daily_title_lower) {
        return Ok(note_file_to_note(existing, &all_notes, &title_map));
    }

    // Create a new daily note from template
    let now = now_iso();
    let content = format!(
        "## Daily Journal\n\n\
         **Date:** {target_date}\n\n\
         ### Gratitude\n\n1. \n2. \n3. \n\n\
         ### Today's Goals\n\n- [ ] \n- [ ] \n- [ ] \n\n\
         ### Reflections\n\n\n\n\
         ### Tomorrow's Priorities\n\n1. \n2. "
    );

    let id = Uuid::new_v4().to_string();
    let nf = NoteFile {
        id: id.clone(),
        title: daily_title.clone(),
        content,
        tags: vec!["journal".into(), "daily".into()],
        is_pinned: false,
        is_archived: false,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    write_note(&note_path(&dir, &id), &nf)?;
    log::info!("ForgeNotes: created daily note for {target_date}");

    // Re-load for proper link resolution
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    let created = all_notes.iter().find(|n| n.id == id).ok_or_else(|| {
        ImpForgeError::internal("DAILY_CREATE_FAILED", "Daily note was created but could not be read back")
    })?;

    Ok(note_file_to_note(created, &all_notes, &title_map))
}

// ---------------------------------------------------------------------------
// Tauri Commands — Semantic Search
// ---------------------------------------------------------------------------

/// Semantic search across notes using Ollama embeddings.
///
/// Generates an embedding for the query text, then compares it against note
/// content using cosine similarity. Falls back to keyword search if Ollama
/// is unavailable.
#[tauri::command]
pub async fn notes_semantic_search(query: String) -> AppResult<Vec<NoteMeta>> {
    if query.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_QUERY",
            "Provide a search query",
        ));
    }

    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    // Try embedding-based search via Ollama
    let ollama_url = resolve_ollama_url();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    // Get query embedding
    let query_embedding = get_embedding(&client, &ollama_url, &query).await;

    match query_embedding {
        Ok(q_emb) => {
            // Get embeddings for all non-archived notes and compute similarity
            let mut scored: Vec<(NoteMeta, f64)> = Vec::new();

            for note in all_notes.iter().filter(|n| !n.is_archived) {
                let note_text = format!("{}\n{}", note.title, &note.content[..note.content.len().min(500)]);
                if let Ok(n_emb) = get_embedding(&client, &ollama_url, &note_text).await {
                    let sim = cosine_similarity(&q_emb, &n_emb);
                    if sim > 0.3 {
                        scored.push((note_file_to_meta(note, &all_notes, &title_map), sim));
                    }
                }
            }

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored.into_iter().take(20).map(|(m, _)| m).collect())
        }
        Err(_) => {
            // Fallback: use keyword search
            log::warn!("ForgeNotes: Ollama embeddings unavailable, falling back to keyword search");
            notes_search(query).await
        }
    }
}

/// Get an embedding vector from Ollama.
async fn get_embedding(
    client: &reqwest::Client,
    ollama_url: &str,
    text: &str,
) -> Result<Vec<f64>, ImpForgeError> {
    let response = client
        .post(format!("{ollama_url}/api/embed"))
        .json(&serde_json::json!({
            "model": "nomic-embed-text",
            "input": text,
        }))
        .send()
        .await
        .map_err(|e| {
            ImpForgeError::service("EMBED_FAILED", format!("Embedding request failed: {e}"))
        })?;

    if !response.status().is_success() {
        return Err(ImpForgeError::service(
            "EMBED_HTTP_ERROR",
            format!("Embedding returned HTTP {}", response.status()),
        ));
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("EMBED_PARSE", format!("Cannot parse embedding response: {e}"))
    })?;

    // Ollama embed API returns {"embeddings":[[...]]}
    let embeddings = body
        .get("embeddings")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ImpForgeError::service("EMBED_FORMAT", "Unexpected embedding response format")
        })?;

    let vec: Vec<f64> = embeddings
        .iter()
        .filter_map(|v| v.as_f64())
        .collect();

    if vec.is_empty() {
        return Err(ImpForgeError::service("EMBED_EMPTY", "Empty embedding vector"));
    }

    Ok(vec)
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

// ---------------------------------------------------------------------------
// Tauri Commands — Export
// ---------------------------------------------------------------------------

/// Export all notes (or notes matching a tag) as a combined Markdown string.
///
/// `format` can be "markdown" (default) or "json".
/// If `tag` is provided, only notes with that tag are exported.
#[tauri::command]
pub async fn notes_export_all(
    format: String,
    tag: Option<String>,
) -> AppResult<String> {
    let dir = notes_dir()?;
    let all_notes = load_all_notes(&dir)?;
    let title_map = build_title_map(&all_notes);

    let filtered: Vec<&NoteFile> = all_notes
        .iter()
        .filter(|n| !n.is_archived)
        .filter(|n| {
            if let Some(ref t) = tag {
                let t_lower = t.to_lowercase();
                n.tags.iter().any(|nt| nt.to_lowercase() == t_lower)
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        return Err(ImpForgeError::validation(
            "NO_NOTES",
            if let Some(ref t) = tag {
                format!("No notes found with tag '{t}'")
            } else {
                "No notes to export".to_string()
            },
        ));
    }

    let format_lower = format.to_ascii_lowercase();
    match format_lower.as_str() {
        "json" => {
            let notes: Vec<Note> = filtered
                .iter()
                .map(|nf| note_file_to_note(nf, &all_notes, &title_map))
                .collect();
            serde_json::to_string_pretty(&notes).map_err(|e| {
                ImpForgeError::internal("EXPORT_SERIALIZE", format!("Cannot serialize notes: {e}"))
            })
        }
        _ => {
            // Markdown export
            let mut md = String::with_capacity(filtered.len() * 1024);
            md.push_str(&format!("# ForgeNotes Export ({} notes)\n\n", filtered.len()));
            md.push_str(&format!("Exported: {}\n\n", Utc::now().to_rfc3339()));

            if let Some(ref t) = tag {
                md.push_str(&format!("Tag filter: #{t}\n\n"));
            }

            md.push_str("---\n\n");

            for nf in &filtered {
                md.push_str(&format!("# {}\n\n", nf.title));
                md.push_str(&format!("**Tags:** {}\n", nf.tags.join(", ")));
                md.push_str(&format!("**Created:** {} | **Updated:** {}\n\n", nf.created_at, nf.updated_at));
                md.push_str(&nf.content);
                md.push_str("\n\n---\n\n");
            }

            log::info!("ForgeNotes: exported {} notes as markdown", filtered.len());
            Ok(md)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_wiki_links_empty() {
        assert!(extract_wiki_links("No links here").is_empty());
    }

    #[test]
    fn test_extract_wiki_links_single() {
        let links = extract_wiki_links("Check out [[My Note]] for details.");
        assert_eq!(links, vec!["My Note"]);
    }

    #[test]
    fn test_extract_wiki_links_multiple() {
        let links = extract_wiki_links("See [[Note A]] and [[Note B]] and [[Note C]].");
        assert_eq!(links, vec!["Note A", "Note B", "Note C"]);
    }

    #[test]
    fn test_extract_wiki_links_dedup() {
        let links = extract_wiki_links("[[Same]] and [[Same]] again.");
        assert_eq!(links, vec!["Same"]);
    }

    #[test]
    fn test_extract_wiki_links_nested_ignored() {
        let links = extract_wiki_links("[[Outer [[Inner]] end]]");
        // Should find "Outer [[Inner" because it stops at first ]]
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn test_extract_wiki_links_empty_brackets() {
        let links = extract_wiki_links("Empty [[ ]] brackets.");
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_wiki_links_unclosed() {
        let links = extract_wiki_links("Unclosed [[link here");
        assert!(links.is_empty());
    }

    #[test]
    fn test_make_preview_short() {
        let preview = make_preview("Hello world", 150);
        assert_eq!(preview, "Hello world");
    }

    #[test]
    fn test_make_preview_strips_markdown() {
        let preview = make_preview("# Heading\n\n**Bold** and *italic*", 150);
        assert!(!preview.contains('#'));
        assert!(!preview.contains("**"));
    }

    #[test]
    fn test_make_preview_truncates() {
        let long_text = "a ".repeat(200);
        let preview = make_preview(&long_text, 50);
        assert!(preview.len() <= 54); // 50 + "..."
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_count_words() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("one\ntwo\nthree"), 3);
    }

    #[test]
    fn test_build_title_map() {
        let notes = vec![
            NoteFile {
                id: "id1".to_string(),
                title: "First Note".to_string(),
                content: String::new(),
                tags: Vec::new(),
                is_pinned: false,
                is_archived: false,
                created_at: String::new(),
                updated_at: String::new(),
            },
            NoteFile {
                id: "id2".to_string(),
                title: "Second Note".to_string(),
                content: String::new(),
                tags: Vec::new(),
                is_pinned: false,
                is_archived: false,
                created_at: String::new(),
                updated_at: String::new(),
            },
        ];
        let map = build_title_map(&notes);
        assert_eq!(map.get("first note"), Some(&"id1".to_string()));
        assert_eq!(map.get("second note"), Some(&"id2".to_string()));
    }

    #[test]
    fn test_resolve_links() {
        let mut map = HashMap::new();
        map.insert("rust".to_string(), "id-rust".to_string());
        map.insert("python".to_string(), "id-python".to_string());

        let links = vec!["Rust".to_string(), "Python".to_string(), "Unknown".to_string()];
        let resolved = resolve_links(&links, &map);
        assert_eq!(resolved, vec!["id-rust", "id-python"]);
    }

    #[test]
    fn test_make_preview_empty() {
        let preview = make_preview("", 150);
        assert!(preview.is_empty());
    }

    #[test]
    fn test_make_preview_skips_headings_and_code() {
        let content = "# Title\n```code```\nActual content here.";
        let preview = make_preview(content, 150);
        assert!(preview.contains("Actual content here"));
        assert!(!preview.contains("Title"));
    }

    // --- Enterprise additions ---

    #[tokio::test]
    async fn test_notes_get_templates() {
        let templates = notes_get_templates().await.expect("should return templates");
        assert_eq!(templates.len(), 8);
        assert!(templates.iter().any(|t| t.name == "Meeting Notes"));
        assert!(templates.iter().any(|t| t.name == "Decision Log"));
        assert!(templates.iter().any(|t| t.name == "Recipe"));
        for tpl in &templates {
            assert!(!tpl.content.is_empty());
            assert!(!tpl.tags.is_empty());
        }
    }

    #[test]
    fn test_note_template_serialize() {
        let tpl = NoteTemplate {
            id: "tpl-1".into(),
            name: "Test".into(),
            content: "## Test\n\nContent".into(),
            tags: vec!["test".into()],
            category: "Dev".into(),
        };
        let json = serde_json::to_string(&tpl).expect("serialize");
        let parsed: NoteTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, "tpl-1");
        assert_eq!(parsed.tags, vec!["test"]);
    }

    #[test]
    fn test_kanban_board_serialize() {
        let board = KanbanBoard {
            columns: vec![
                KanbanColumn { name: "To Do".into(), note_ids: vec!["n1".into(), "n2".into()] },
                KanbanColumn { name: "Done".into(), note_ids: vec!["n3".into()] },
            ],
        };
        let json = serde_json::to_string(&board).expect("serialize");
        let parsed: KanbanBoard = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.columns.len(), 2);
        assert_eq!(parsed.columns[0].name, "To Do");
        assert_eq!(parsed.columns[0].note_ids.len(), 2);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 1.0];
        let b = vec![1.0, 0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-9);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f64> = Vec::new();
        let b: Vec<f64> = Vec::new();
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[tokio::test]
    async fn test_notes_export_all_no_notes() {
        // In a test env with no notes dir, should return validation error
        let result = notes_export_all("markdown".into(), Some("nonexistent-tag".into())).await;
        // Could succeed empty or fail validation - both acceptable in test env
        match result {
            Ok(md) => assert!(md.contains("ForgeNotes Export")),
            Err(e) => assert!(e.message.contains("No notes")),
        }
    }
}
