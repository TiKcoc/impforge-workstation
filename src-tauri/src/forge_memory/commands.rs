// Public API — all 30+ commands registered in lib.rs invoke_handler
//! Tauri command layer for ForgeMemory
//!
//! Thin async wrappers around ForgeMemoryEngine methods, exposed to the
//! Svelte frontend via `tauri::invoke()`. Each command:
//!   1. Receives parameters from the frontend (serde deserialization)
//!   2. Delegates to the ForgeMemoryEngine (managed Tauri state)
//!   3. Returns serializable results or error strings
//!
//! All commands are async so Tauri runs them on the Tokio threadpool
//! rather than the main thread, preventing UI blocking during
//! CPU-intensive operations (embedding, search, SQLite queries).
//!
//! Frontend usage (Svelte/TypeScript):
//! ```typescript
//! import { invoke } from '@tauri-apps/api/core';
//!
//! // Add a memory
//! const id = await invoke('forge_memory_add', {
//!   content: 'Rust ownership prevents data races',
//!   scope: 'core',
//!   importance: 0.9,
//!   category: 'programming'
//! });
//!
//! // Search
//! const results = await invoke('forge_memory_search', {
//!   query: 'ownership patterns',
//!   limit: 10
//! });
//! ```
//!
//! References:
//!   - Tauri 2 Commands: https://v2.tauri.app/develop/calling-rust/

use std::path::PathBuf;

use tauri::State;

use super::context::{self, LearnResult, MemoryContext};
use super::engine::{ForgeMemoryEngine, ForgeMemoryStatus, ForgeSearchResult, KgConnectedNode};
use super::ingest::{self, IngestionResult};
use super::memory::{ConsolidationReport, MemoryItem};
use super::graph::{GraphNode, SubGraph};
use super::watch::{self, DiscoveredPath, ForgeWatcher, ScanMode, WatchStatus, WatchedPath};
use super::digest::{self, DigestConfig, DigestResult, DigestSource};
use super::store::{ChatMessageRecord, ForgeMemoryStats};

// ═══════════════════════════════════════════════════════════════════
//  MEMORY COMMANDS (MemGPT Tiers + FSRS-5)
// ═══════════════════════════════════════════════════════════════════

/// Add a memory to the specified MemGPT tier.
///
/// Automatically embeds the content, stores in SQLite, and indexes
/// in both HNSW (semantic) and BM25 (keyword) search engines.
///
/// # Parameters
/// - `content`: The text content to memorize
/// - `scope`: MemGPT tier — "core", "recall", or "archival"
/// - `importance`: Importance weight [0.0, 1.0]
/// - `category`: Classification category (e.g., "programming", "preference")
#[tauri::command]
pub async fn forge_memory_add(
    engine: State<'_, ForgeMemoryEngine>,
    content: String,
    scope: String,
    importance: f64,
    category: String,
) -> Result<String, String> {
    engine.add_memory(&content, &scope, importance, &category)
}

/// Get all core memories (always loaded into AI context).
///
/// Returns up to 20 items sorted by importance descending,
/// following Miller's Law (~7 ± 2) scaled for AI context windows.
#[tauri::command]
pub async fn forge_memory_get_core(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<Vec<MemoryItem>, String> {
    engine.get_core_memories()
}

/// Hybrid search across all memory tiers and knowledge.
///
/// Combines HNSW semantic search (cosine similarity) with BM25 keyword
/// search via Reciprocal Rank Fusion (Cormack et al. 2009, k=60).
/// Falls back to BM25-only if embedding provider is unavailable.
///
/// # Parameters
/// - `query`: Natural language search query
/// - `limit`: Maximum results to return (default 10)
#[tauri::command]
pub async fn forge_memory_search(
    engine: State<'_, ForgeMemoryEngine>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ForgeSearchResult>, String> {
    engine.search(&query, limit.unwrap_or(10))
}

/// Promote a memory to a higher MemGPT tier (e.g., recall → core).
///
/// After promotion, tier limits are enforced — if core is full,
/// the lowest-importance item is automatically demoted to recall.
#[tauri::command]
pub async fn forge_memory_promote(
    engine: State<'_, ForgeMemoryEngine>,
    id: String,
    to_scope: String,
) -> Result<bool, String> {
    engine.promote_memory(&id, &to_scope)
}

/// Demote a memory to the next lower MemGPT tier.
///
/// Core → Recall → Archival. Returns error if already archival.
#[tauri::command]
pub async fn forge_memory_demote(
    engine: State<'_, ForgeMemoryEngine>,
    id: String,
) -> Result<bool, String> {
    engine.demote_memory(&id)
}

/// Record an FSRS-5 spaced repetition review.
///
/// Updates the memory's stability and difficulty parameters using:
///   S' = S × e^(w × (G − D + 1))
///
/// # Parameters
/// - `id`: Memory UUID
/// - `rating`: "again" (0), "hard" (1), "good" (2), or "easy" (3)
#[tauri::command]
pub async fn forge_memory_review(
    engine: State<'_, ForgeMemoryEngine>,
    id: String,
    rating: String,
) -> Result<bool, String> {
    engine.review_memory(&id, &rating)
}

/// Run memory consolidation (maintenance cycle).
///
/// Applies temporal importance decay (half-life: 7 days) and
/// enforces tier limits, demoting excess items automatically.
#[tauri::command]
pub async fn forge_memory_consolidate(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<ConsolidationReport, String> {
    engine.consolidate()
}

/// Delete a memory and remove it from all search indices.
#[tauri::command]
pub async fn forge_memory_delete(
    engine: State<'_, ForgeMemoryEngine>,
    id: String,
) -> Result<bool, String> {
    engine.delete_memory(&id)
}

// ═══════════════════════════════════════════════════════════════════
//  KNOWLEDGE COMMANDS
// ═══════════════════════════════════════════════════════════════════

/// Add a knowledge item with auto-embedding and search indexing.
///
/// # Parameters
/// - `title`: Short descriptive title
/// - `content`: Full knowledge content
/// - `tier`: Knowledge tier (e.g., "tier-000", "tier-0")
/// - `category`: Classification category
/// - `importance`: Priority level (1-5)
#[tauri::command]
pub async fn forge_memory_add_knowledge(
    engine: State<'_, ForgeMemoryEngine>,
    title: String,
    content: String,
    tier: String,
    category: String,
    importance: i32,
) -> Result<String, String> {
    engine.add_knowledge(&title, &content, &tier, &category, importance)
}

/// Search knowledge items only (excludes memories).
#[tauri::command]
pub async fn forge_memory_search_knowledge(
    engine: State<'_, ForgeMemoryEngine>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ForgeSearchResult>, String> {
    engine.search_knowledge(&query, limit.unwrap_or(10))
}

// ═══════════════════════════════════════════════════════════════════
//  KNOWLEDGE GRAPH COMMANDS
// ═══════════════════════════════════════════════════════════════════

/// Add a node to the knowledge graph.
///
/// # Parameters
/// - `id`: Unique node identifier
/// - `kind`: Node type — one of: file, symbol, concept, session, user,
///   pattern, crate, module, function, type
/// - `label`: Human-readable label
/// - `properties`: Optional JSON metadata
#[tauri::command]
pub async fn forge_memory_kg_add_node(
    engine: State<'_, ForgeMemoryEngine>,
    id: String,
    kind: String,
    label: String,
    properties: Option<String>,
) -> Result<(), String> {
    engine.kg_add_node(&id, &kind, &label, properties)
}

/// Add a directed edge between two nodes.
///
/// # Parameters
/// - `source_id`: Source node ID
/// - `target_id`: Target node ID
/// - `kind`: Edge type — one of: contains, references, co_changes,
///   depends_on, derives, similar, edits, owns, imports, calls
/// - `weight`: Edge weight [0.0, 1.0]
#[tauri::command]
pub async fn forge_memory_kg_add_edge(
    engine: State<'_, ForgeMemoryEngine>,
    source_id: String,
    target_id: String,
    kind: String,
    weight: Option<f64>,
) -> Result<String, String> {
    engine.kg_add_edge(&source_id, &target_id, &kind, weight.unwrap_or(1.0))
}

/// Get all neighbors of a node in the knowledge graph.
#[tauri::command]
pub async fn forge_memory_kg_neighbors(
    engine: State<'_, ForgeMemoryEngine>,
    node_id: String,
) -> Result<Vec<GraphNode>, String> {
    engine.kg_neighbors(&node_id)
}

/// Find shortest path between two nodes (BFS traversal).
///
/// Returns the sequence of node IDs from source to target,
/// or null if no path exists.
#[tauri::command]
pub async fn forge_memory_kg_traverse(
    engine: State<'_, ForgeMemoryEngine>,
    from_id: String,
    to_id: String,
) -> Result<Option<Vec<String>>, String> {
    engine.kg_traverse(&from_id, &to_id)
}

/// Detect communities in the knowledge graph (connected components).
///
/// Returns a list of communities, each being a list of node IDs.
#[tauri::command]
pub async fn forge_memory_kg_communities(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<Vec<Vec<String>>, String> {
    engine.kg_communities()
}

/// Extract ego-centric subgraph around a center node.
///
/// # Parameters
/// - `center_id`: The focal node
/// - `depth`: How many hops to include (default 2)
#[tauri::command]
pub async fn forge_memory_kg_subgraph(
    engine: State<'_, ForgeMemoryEngine>,
    center_id: String,
    depth: Option<usize>,
) -> Result<SubGraph, String> {
    engine.kg_subgraph(&center_id, depth.unwrap_or(2))
}

/// Get the most connected nodes in the knowledge graph.
///
/// Useful for identifying key concepts and central topics.
#[tauri::command]
pub async fn forge_memory_kg_most_connected(
    engine: State<'_, ForgeMemoryEngine>,
    limit: Option<usize>,
) -> Result<Vec<KgConnectedNode>, String> {
    engine.kg_most_connected(limit.unwrap_or(10))
}

// ═══════════════════════════════════════════════════════════════════
//  CHAT PERSISTENCE COMMANDS
// ═══════════════════════════════════════════════════════════════════

/// Create a new conversation.
#[tauri::command]
pub async fn forge_memory_create_conversation(
    engine: State<'_, ForgeMemoryEngine>,
    title: Option<String>,
    model_id: Option<String>,
) -> Result<String, String> {
    engine.create_conversation(title.as_deref(), model_id.as_deref())
}

/// Save a chat message to a conversation.
#[tauri::command]
pub async fn forge_memory_save_message(
    engine: State<'_, ForgeMemoryEngine>,
    conversation_id: String,
    role: String,
    content: String,
    model_id: Option<String>,
) -> Result<i64, String> {
    engine.save_message(&conversation_id, &role, &content, model_id.as_deref())
}

/// Get all messages in a conversation (ordered by time).
#[tauri::command]
pub async fn forge_memory_get_messages(
    engine: State<'_, ForgeMemoryEngine>,
    conversation_id: String,
) -> Result<Vec<ChatMessageRecord>, String> {
    engine.get_messages(&conversation_id)
}

// ═══════════════════════════════════════════════════════════════════
//  CONTEXT & AUTO-LEARNING COMMANDS (Contextual Retrieval)
// ═══════════════════════════════════════════════════════════════════

/// Build enriched context for an AI prompt (Contextual Retrieval).
///
/// Assembles core memories + relevant search results into a formatted
/// system prompt supplement. Call before sending a user message to the AI.
///
/// # Parameters
/// - `query`: The user's message (used for relevance search)
/// - `max_results`: Max search results to include (default 5)
#[tauri::command]
pub async fn forge_memory_get_context(
    engine: State<'_, ForgeMemoryEngine>,
    query: String,
    max_results: Option<usize>,
) -> Result<MemoryContext, String> {
    context::build_context(&engine, &query, max_results.unwrap_or(5))
}

/// Auto-learn from a conversation turn.
///
/// Extracts preferences, decisions, and explicit notes from the user
/// message using keyword patterns. Reinforces relevant existing memories
/// via FSRS-5 "good" review. Optionally persists messages to a conversation.
///
/// # Parameters
/// - `user_message`: What the user said
/// - `ai_response`: What the AI responded
/// - `conversation_id`: Optional conversation to persist messages to
#[tauri::command]
pub async fn forge_memory_auto_learn(
    engine: State<'_, ForgeMemoryEngine>,
    user_message: String,
    ai_response: String,
    conversation_id: Option<String>,
) -> Result<LearnResult, String> {
    context::auto_learn(&engine, &user_message, &ai_response, conversation_id.as_deref())
}

// ═══════════════════════════════════════════════════════════════════
//  STATS & LIFECYCLE COMMANDS
// ═══════════════════════════════════════════════════════════════════

/// Get database statistics (table row counts, schema version, etc.).
#[tauri::command]
pub async fn forge_memory_stats(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<ForgeMemoryStats, String> {
    engine.stats()
}

/// Get engine runtime status (embedding provider, index sizes, etc.).
#[tauri::command]
pub async fn forge_memory_status(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<ForgeMemoryStatus, String> {
    Ok(engine.status())
}

/// Persist in-memory indices (HNSW, BM25, KG) to SQLite.
///
/// Call before app shutdown or periodically for durability.
/// Auto-persist also triggers every 100 index operations.
#[tauri::command]
pub async fn forge_memory_persist(
    engine: State<'_, ForgeMemoryEngine>,
) -> Result<(), String> {
    engine.persist()
}

// ── ForgeWatch Commands ─────────────────────────────────────────

/// Auto-discover interesting directories to watch (git repos, projects).
///
/// Scans the given root path (typically $HOME) for project markers
/// like .git, Cargo.toml, package.json, pyproject.toml, go.mod.
/// Returns a list of discovered paths with reasons and project types.
#[tauri::command]
pub async fn forge_watch_discover(
    home_path: String,
) -> Result<Vec<DiscoveredPath>, String> {
    let path = PathBuf::from(&home_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {home_path}"));
    }
    Ok(watch::auto_discover(&path))
}

/// Add a path to the ForgeWatch watch list.
#[tauri::command]
pub async fn forge_watch_add_path(
    watcher: State<'_, ForgeWatcher>,
    path: String,
    label: Option<String>,
) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {path}"));
    }
    watcher.add_path(&path, label.as_deref(), ScanMode::Realtime);
    Ok(())
}

/// Remove a path from the ForgeWatch watch list.
#[tauri::command]
pub async fn forge_watch_remove_path(
    watcher: State<'_, ForgeWatcher>,
    path: String,
) -> Result<(), String> {
    if watcher.remove_path(&path) {
        Ok(())
    } else {
        Err(format!("Path not found in watch list: {path}"))
    }
}

/// List all currently watched paths.
#[tauri::command]
pub async fn forge_watch_list_paths(
    watcher: State<'_, ForgeWatcher>,
) -> Result<Vec<WatchedPath>, String> {
    Ok(watcher.list_paths())
}

/// Get ForgeWatch engine status.
#[tauri::command]
pub async fn forge_watch_status(
    watcher: State<'_, ForgeWatcher>,
) -> Result<WatchStatus, String> {
    Ok(watcher.status())
}

/// Manually reindex a specific file or directory.
#[tauri::command]
pub async fn forge_watch_reindex(
    engine: State<'_, ForgeMemoryEngine>,
    path: String,
) -> Result<serde_json::Value, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {path}"));
    }

    if p.is_file() {
        let result = ingest::ingest_file(&engine, &p)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    } else if p.is_dir() {
        let result = ingest::ingest_directory(&engine, &p, 1000)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    } else {
        Err("Path is neither a file nor a directory".to_string())
    }
}

/// Ingest a single file into ForgeMemory knowledge base.
#[tauri::command]
pub async fn forge_watch_ingest_file(
    engine: State<'_, ForgeMemoryEngine>,
    path: String,
) -> Result<IngestionResult, String> {
    let p = PathBuf::from(&path);
    ingest::ingest_file(&engine, &p)
}

// ═══════════════════════════════════════════════════════════════════
//  UNIVERSAL INPUT DIGEST COMMANDS
// ═══════════════════════════════════════════════════════════════════

/// Digest arbitrary text through the NLP pipeline.
///
/// Frontend sends text from any source (terminal, editor, clipboard)
/// and this command extracts knowledge, creates memories, and updates KG.
#[tauri::command]
pub async fn forge_digest_text(
    engine: State<'_, ForgeMemoryEngine>,
    text: String,
    source: String,
) -> Result<DigestResult, String> {
    let src = DigestSource::from_str(&source);
    let config = DigestConfig::default();
    digest::digest_text(&engine, &text, src, &config)
}

/// Digest text with custom configuration.
#[tauri::command]
pub async fn forge_digest_text_configured(
    engine: State<'_, ForgeMemoryEngine>,
    text: String,
    source: String,
    config: DigestConfig,
) -> Result<DigestResult, String> {
    let src = DigestSource::from_str(&source);
    digest::digest_text(&engine, &text, src, &config)
}

/// Get the default digest configuration.
#[tauri::command]
pub async fn forge_digest_get_config() -> Result<DigestConfig, String> {
    Ok(DigestConfig::default())
}
