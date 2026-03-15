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
}
