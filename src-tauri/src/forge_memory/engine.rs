#![allow(dead_code)]
//! ForgeMemoryEngine — Unified AI Memory Engine Facade
//!
//! Orchestrates all ForgeMemory subsystems behind a single thread-safe interface:
//!   - SQLite store (171 tables, WAL mode)
//!   - Embedding provider (fastembed offline + optional Ollama)
//!   - HNSW vector index for semantic search
//!   - BM25 scoring engine for keyword search
//!   - Hybrid RRF fusion combining both signals
//!   - MemGPT tiered memory with FSRS-5 spaced repetition
//!   - Knowledge graph with traversal and community detection
//!
//! Pattern: Facade (Gamma, Helm, Johnson & Vlissides 1994) — reduces
//! the coupling between the Tauri command layer and 7 internal modules.
//!
//! Thread-safety: All fields use interior mutability (parking_lot::Mutex,
//! parking_lot::RwLock) so the engine can be shared via `tauri::State<'_>`
//! with only `&self` references.
//!
//! References:
//!   - Tauri 2 State Management: managed state is wrapped in Arc automatically.
//!   - Contextual Retrieval (Anthropic 2024): prepend context to chunks for
//!     better embedding quality — future enhancement.
//!   - Matryoshka Representation Learning (Kusupati et al. 2024, NeurIPS):
//!     adaptive embedding dimensions — future enhancement.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::bm25::SharedBm25Engine;
use super::embeddings::ForgeEmbeddings;
use super::graph::{GraphNode, KnowledgeGraph, SubGraph};
use super::memory::{ConsolidationReport, MemoryItem, MemoryScope, ReviewRating, TieredMemory};
use super::search::{HybridSearchEngine, SearchResult as RawSearchResult};
use super::store::{ChatMessageRecord, ForgeMemoryStats, ForgeMemoryStore};
use super::vector::{HnswConfig, SharedHnswIndex};

/// Default BFS max depth for graph traversal.
const DEFAULT_MAX_DEPTH: usize = 20;

// ── Frontend-facing search result ────────────────────────────────

/// Enriched search result with resolved content for the Svelte frontend.
///
/// The internal search pipeline returns doc_ids + scores; this struct
/// resolves those ids to actual content from SQLite and adds metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeSearchResult {
    /// Unique identifier of the matched document.
    pub id: String,
    /// Source table: "memory" or "knowledge".
    pub source: String,
    /// Full text content of the matched document.
    pub content: String,
    /// Title (for knowledge items) or category (for memories).
    pub title: Option<String>,
    /// Final combined score (RRF for hybrid, raw for single-mode).
    pub score: f64,
    /// Cosine similarity from HNSW, if available.
    pub semantic_score: Option<f64>,
    /// BM25 score, if available.
    pub keyword_score: Option<f64>,
    /// Additional metadata (scope, tier, importance, category, etc.).
    pub metadata: serde_json::Value,
}

// ── Engine status ────────────────────────────────────────────────

/// Runtime health snapshot of the ForgeMemory engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMemoryStatus {
    pub initialized: bool,
    pub embedding_provider: Option<String>,
    pub embedding_dimensions: usize,
    pub hnsw_vector_count: usize,
    pub bm25_document_count: usize,
    pub kg_node_count: usize,
    pub kg_edge_count: usize,
    pub index_operations: u64,
}

// ── KG connected node (serializable for frontend) ────────────────

/// A node with its connection degree, for the "most connected" API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgConnectedNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub degree: usize,
}

// ── Doc ID prefixes ──────────────────────────────────────────────

const MEM_PREFIX: &str = "mem:";
const KB_PREFIX: &str = "kb:";

/// Number of index operations between auto-persist cycles.
const AUTO_PERSIST_INTERVAL: u64 = 100;

// ── ForgeMemoryEngine ────────────────────────────────────────────

/// The unified ForgeMemory engine. Managed as Tauri state.
///
/// All public methods take `&self` and are safe to call from any thread.
pub struct ForgeMemoryEngine {
    store: ForgeMemoryStore,
    embeddings: Option<ForgeEmbeddings>,
    hnsw: Arc<SharedHnswIndex>,
    bm25: Arc<SharedBm25Engine>,
    graph: RwLock<KnowledgeGraph>,
    dimensions: usize,
    /// Counter for auto-persist scheduling.
    ops_since_persist: AtomicU64,
}

impl ForgeMemoryEngine {
    /// Initialize the engine from a file-backed SQLite database.
    ///
    /// Creates the database and runs migrations if it doesn't exist.
    /// Loads persisted HNSW/BM25 indices from SQLite. Falls back to
    /// empty indices if loading fails.
    pub fn new(db_path: &PathBuf) -> Result<Self, String> {
        let store = ForgeMemoryStore::open(db_path)
            .map_err(|e| format!("Failed to open ForgeMemory database: {e}"))?;
        Self::init_from_store(store)
    }

    /// Initialize with an in-memory database (for testing).
    pub fn open_memory() -> Result<Self, String> {
        let store = ForgeMemoryStore::open_memory()
            .map_err(|e| format!("Failed to create in-memory ForgeMemory: {e}"))?;
        Self::init_from_store(store)
    }

    /// Shared initialization logic.
    fn init_from_store(store: ForgeMemoryStore) -> Result<Self, String> {
        // 1. Initialize embedding provider (graceful failure)
        let (embeddings, dimensions) = match ForgeEmbeddings::new() {
            Ok(emb) => {
                let dims = emb.dimensions();
                log::info!("ForgeMemory: embedding provider ready ({dims}-dim)");
                (Some(emb), dims)
            }
            Err(e) => {
                log::warn!("ForgeMemory: embedding init failed ({e}), BM25-only search");
                (None, 384) // Default dimension for future lazy init
            }
        };

        // 2. Load HNSW index from SQLite (or create empty)
        let hnsw = {
            let conn = store.conn.lock();
            match SharedHnswIndex::load_from_db(&conn) {
                Ok(Some(index)) => {
                    log::info!("ForgeMemory: HNSW index loaded ({} vectors)", index.len());
                    Arc::new(index)
                }
                Ok(None) => {
                    log::info!("ForgeMemory: HNSW index empty, starting fresh");
                    Arc::new(SharedHnswIndex::new(HnswConfig::default()))
                }
                Err(e) => {
                    log::warn!("ForgeMemory: HNSW load failed ({e}), starting fresh");
                    Arc::new(SharedHnswIndex::new(HnswConfig::default()))
                }
            }
        };

        // 3. Load BM25 index from SQLite (or create empty)
        let bm25 = {
            let conn = store.conn.lock();
            match SharedBm25Engine::load_from_db(&conn) {
                Ok(Some(engine)) => {
                    log::info!(
                        "ForgeMemory: BM25 index loaded ({} docs)",
                        engine.document_count()
                    );
                    Arc::new(engine)
                }
                Ok(None) => {
                    log::info!("ForgeMemory: BM25 index empty, starting fresh");
                    Arc::new(SharedBm25Engine::new())
                }
                Err(e) => {
                    log::warn!("ForgeMemory: BM25 load failed ({e}), starting fresh");
                    Arc::new(SharedBm25Engine::new())
                }
            }
        };

        // 4. Load knowledge graph from SQLite (or create empty)
        let graph = {
            let conn = store.conn.lock();
            match KnowledgeGraph::load_from_db(&conn) {
                Ok(kg) => {
                    log::info!(
                        "ForgeMemory: KG loaded ({} nodes, {} edges)",
                        kg.node_count(),
                        kg.edge_count()
                    );
                    kg
                }
                Err(e) => {
                    log::warn!("ForgeMemory: KG load failed ({e}), starting fresh");
                    KnowledgeGraph::new()
                }
            }
        };

        Ok(Self {
            store,
            embeddings,
            hnsw,
            bm25,
            graph: RwLock::new(graph),
            dimensions,
            ops_since_persist: AtomicU64::new(0),
        })
    }

    // ── Memory Operations (MemGPT Tiers) ─────────────────────────

    /// Add a memory to the specified tier with auto-embedding and indexing.
    ///
    /// Embeds the content (if provider available), stores in SQLite via
    /// TieredMemory, and indexes in both HNSW + BM25 for future search.
    pub fn add_memory(
        &self,
        content: &str,
        scope: &str,
        importance: f64,
        category: &str,
    ) -> Result<String, String> {
        let scope_enum =
            MemoryScope::parse(scope).ok_or_else(|| format!("Invalid scope: {scope}"))?;

        // Embed the content
        let embedding_f32 = self.embed_text(content);
        let embedding_bytes = embedding_f32.as_ref().map(|e| Self::f32_to_bytes(e));

        // Store in SQLite via TieredMemory
        let tiered = TieredMemory::new(&self.store);
        let id = tiered.add_memory(
            content,
            scope_enum,
            importance,
            category,
            embedding_bytes.as_deref(),
        )?;

        // Index in search engines
        let doc_id = format!("{MEM_PREFIX}{id}");
        if let Some(ref emb) = embedding_f32 {
            self.hnsw.insert(&doc_id, emb);
        }
        self.bm25.index_document(&doc_id, content);
        self.maybe_auto_persist();

        Ok(id)
    }

    /// Get all core memories (always in context).
    pub fn get_core_memories(&self) -> Result<Vec<MemoryItem>, String> {
        let tiered = TieredMemory::new(&self.store);
        tiered.get_core_memories()
    }

    /// Hybrid search across all memory tiers.
    ///
    /// Embeds the query, runs HNSW + BM25 via RRF fusion, and resolves
    /// doc_ids to actual content from SQLite.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ForgeSearchResult>, String> {
        let query_embedding = self.embed_text(query).unwrap_or_default();

        let search_engine =
            HybridSearchEngine::new(Arc::clone(&self.hnsw), Arc::clone(&self.bm25));

        let raw_results = search_engine.search(query, &query_embedding, limit);

        self.resolve_search_results(&raw_results)
    }

    /// Promote a memory to a higher tier.
    pub fn promote_memory(&self, id: &str, to_scope: &str) -> Result<bool, String> {
        let scope =
            MemoryScope::parse(to_scope).ok_or_else(|| format!("Invalid scope: {to_scope}"))?;
        let tiered = TieredMemory::new(&self.store);
        tiered.promote_memory(id, scope)
    }

    /// Demote a memory to the next lower tier.
    pub fn demote_memory(&self, id: &str) -> Result<bool, String> {
        let tiered = TieredMemory::new(&self.store);
        tiered.demote_memory(id)
    }

    /// Record an FSRS-5 spaced repetition review.
    pub fn review_memory(&self, id: &str, rating: &str) -> Result<bool, String> {
        let review = match rating {
            "again" => ReviewRating::Again,
            "hard" => ReviewRating::Hard,
            "good" => ReviewRating::Good,
            "easy" => ReviewRating::Easy,
            _ => return Err(format!("Invalid rating: {rating} (use again/hard/good/easy)")),
        };
        let tiered = TieredMemory::new(&self.store);
        tiered.review_memory(id, review)
    }

    /// Run maintenance: apply temporal decay, enforce tier limits.
    pub fn consolidate(&self) -> Result<ConsolidationReport, String> {
        let tiered = TieredMemory::new(&self.store);
        tiered.consolidate()
    }

    /// Delete a memory and remove it from search indices.
    pub fn delete_memory(&self, id: &str) -> Result<bool, String> {
        let doc_id = format!("{MEM_PREFIX}{id}");
        self.hnsw.remove(&doc_id);
        self.bm25.remove_document(&doc_id);
        self.store
            .delete_memory(id)
            .map_err(|e| format!("Failed to delete memory: {e}"))
    }

    // ── Knowledge Operations ─────────────────────────────────────

    /// Add a knowledge item with auto-embedding and indexing.
    pub fn add_knowledge(
        &self,
        title: &str,
        content: &str,
        tier: &str,
        category: &str,
        importance: i32,
    ) -> Result<String, String> {
        let embedding_f32 = self.embed_text(content);
        let embedding_bytes = embedding_f32.as_ref().map(|e| Self::f32_to_bytes(e));

        let id = self
            .store
            .insert_knowledge(title, content, tier, category, importance, embedding_bytes.as_deref())
            .map_err(|e| format!("Failed to insert knowledge: {e}"))?;

        // Index for search
        let doc_id = format!("{KB_PREFIX}{id}");
        let searchable = format!("{title} {content}");
        if let Some(ref emb) = embedding_f32 {
            self.hnsw.insert(&doc_id, emb);
        }
        self.bm25.index_document(&doc_id, &searchable);
        self.maybe_auto_persist();

        Ok(id)
    }

    /// Search knowledge items only (filters by KB prefix).
    pub fn search_knowledge(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ForgeSearchResult>, String> {
        // Search everything, then filter to knowledge items
        let all = self.search(query, limit * 3)?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|r| r.source == "knowledge")
            .take(limit)
            .collect();
        Ok(filtered)
    }

    // ── Knowledge Graph Operations ───────────────────────────────

    /// Add a node to the knowledge graph.
    pub fn kg_add_node(
        &self,
        id: &str,
        kind: &str,
        label: &str,
        properties: Option<String>,
    ) -> Result<(), String> {
        // Persist to SQLite
        self.store
            .insert_kg_node(id, kind, label, properties.as_deref(), None)
            .map_err(|e| format!("Failed to insert KG node: {e}"))?;

        // Add to in-memory graph
        let mut graph = self.graph.write();
        graph
            .add_node(id, kind, label, properties.as_deref())
            .map_err(|e| format!("Graph error: {e}"))
    }

    /// Add an edge to the knowledge graph.
    pub fn kg_add_edge(
        &self,
        source_id: &str,
        target_id: &str,
        kind: &str,
        weight: f64,
    ) -> Result<String, String> {
        // Persist to SQLite
        let id = self
            .store
            .insert_kg_edge(source_id, target_id, kind, weight)
            .map_err(|e| format!("Failed to insert KG edge: {e}"))?;

        // Add to in-memory graph
        let mut graph = self.graph.write();
        graph
            .add_edge(source_id, target_id, kind, weight)
            .map_err(|e| format!("Graph error: {e}"))?;

        Ok(id)
    }

    /// Get neighbors of a node (with their connecting edges).
    pub fn kg_neighbors(&self, node_id: &str) -> Result<Vec<GraphNode>, String> {
        let graph = self.graph.read();
        let pairs = graph.get_neighbors(node_id, None);
        Ok(pairs.into_iter().map(|(node, _edge)| node).collect())
    }

    /// Find shortest path between two nodes (BFS, max 20 hops).
    pub fn kg_traverse(
        &self,
        from_id: &str,
        to_id: &str,
    ) -> Result<Option<Vec<String>>, String> {
        let graph = self.graph.read();
        Ok(graph.find_path(from_id, to_id, DEFAULT_MAX_DEPTH))
    }

    /// Detect communities (connected components) in the graph.
    pub fn kg_communities(&self) -> Result<Vec<Vec<String>>, String> {
        let graph = self.graph.read();
        Ok(graph.find_communities())
    }

    /// Extract ego-centric subgraph around a center node.
    pub fn kg_subgraph(&self, center_id: &str, depth: usize) -> Result<SubGraph, String> {
        let graph = self.graph.read();
        Ok(graph.get_subgraph(center_id, depth))
    }

    /// Get the most connected nodes in the graph.
    ///
    /// Returns `(node_id, degree)` pairs sorted by degree descending.
    pub fn kg_most_connected(&self, limit: usize) -> Result<Vec<KgConnectedNode>, String> {
        let graph = self.graph.read();
        let connected = graph.most_connected(limit);
        Ok(connected
            .into_iter()
            .map(|(node, degree)| KgConnectedNode {
                id: node.id,
                label: node.label,
                kind: node.kind,
                degree,
            })
            .collect())
    }

    // ── Chat Persistence ─────────────────────────────────────────

    /// Create a new conversation.
    pub fn create_conversation(
        &self,
        title: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<String, String> {
        self.store
            .create_conversation(title, model_id)
            .map_err(|e| format!("Failed to create conversation: {e}"))
    }

    /// Persist a chat message.
    pub fn save_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        model_id: Option<&str>,
    ) -> Result<i64, String> {
        self.store
            .insert_chat_message(conversation_id, role, content, model_id)
            .map_err(|e| format!("Failed to save message: {e}"))
    }

    /// Get all messages in a conversation.
    pub fn get_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessageRecord>, String> {
        self.store
            .get_chat_messages(conversation_id)
            .map_err(|e| format!("Failed to get messages: {e}"))
    }

    // ── Dedup ───────────────────────────────────────────────────

    /// Check if a similar memory already exists (search score > 0.92).
    pub fn is_duplicate(&self, content: &str) -> Result<bool, String> {
        let results = self.search(content, 1)?;
        Ok(results.first().map(|r| r.score > 0.92).unwrap_or(false))
    }

    // ── Stats & Lifecycle ────────────────────────────────────────

    /// Get database statistics.
    pub fn stats(&self) -> Result<ForgeMemoryStats, String> {
        self.store
            .stats()
            .map_err(|e| format!("Failed to get stats: {e}"))
    }

    /// Get engine runtime status.
    pub fn status(&self) -> ForgeMemoryStatus {
        let graph = self.graph.read();
        ForgeMemoryStatus {
            initialized: true,
            embedding_provider: self.embeddings.as_ref().map(|e| e.active_model().to_string()),
            embedding_dimensions: self.dimensions,
            hnsw_vector_count: self.hnsw.len(),
            bm25_document_count: self.bm25.document_count(),
            kg_node_count: graph.node_count(),
            kg_edge_count: graph.edge_count(),
            index_operations: self.ops_since_persist.load(Ordering::Relaxed),
        }
    }

    /// Persist in-memory indices (HNSW, BM25, KG) to SQLite.
    ///
    /// Call this periodically or before app shutdown to ensure durability.
    /// The SQLite store itself is always durable (WAL mode); this persists
    /// the in-memory search indices that live outside SQLite.
    pub fn persist(&self) -> Result<(), String> {
        let conn = self.store.conn.lock();

        self.hnsw
            .save_to_db(&conn)
            .map_err(|e| format!("HNSW persist failed: {e}"))?;

        self.bm25
            .save_to_db(&conn)
            .map_err(|e| format!("BM25 persist failed: {e}"))?;

        self.graph
            .read()
            .sync_to_db(&conn)
            .map_err(|e| format!("KG persist failed: {e}"))?;

        self.ops_since_persist.store(0, Ordering::Relaxed);
        log::info!("ForgeMemory: indices persisted to SQLite");
        Ok(())
    }

    /// Run PRAGMA optimize for query planner statistics.
    pub fn optimize(&self) {
        self.store.optimize();
    }

    // ── Internal helpers ─────────────────────────────────────────

    /// Embed a single text string. Returns None if no provider available.
    fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        let emb = self.embeddings.as_ref()?;
        match emb.embed_one(text) {
            Ok((vec, _model_id)) => Some(vec),
            Err(e) => {
                log::warn!("Embedding failed: {e}");
                None
            }
        }
    }

    /// Convert f32 vector to byte slice for SQLite BLOB storage.
    fn f32_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect()
    }

    /// Auto-persist indices every N operations.
    fn maybe_auto_persist(&self) {
        let ops = self.ops_since_persist.fetch_add(1, Ordering::Relaxed);
        if ops > 0 && ops % AUTO_PERSIST_INTERVAL == 0 {
            if let Err(e) = self.persist() {
                log::warn!("Auto-persist failed: {e}");
            }
        }
    }

    /// Resolve raw search results (doc_ids + scores) to enriched results
    /// with actual content from SQLite.
    fn resolve_search_results(
        &self,
        raw: &[RawSearchResult],
    ) -> Result<Vec<ForgeSearchResult>, String> {
        let conn = self.store.conn.lock();
        let mut results = Vec::with_capacity(raw.len());

        for r in raw {
            if let Some(uuid) = r.doc_id.strip_prefix(MEM_PREFIX) {
                // Resolve memory
                if let Ok(Some(row)) = conn
                    .query_row(
                        "SELECT id, scope, category, key, content, importance, created_at
                         FROM memories WHERE id = ?1",
                        params![uuid],
                        |row| {
                            Ok(Some((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, Option<String>>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, f64>(5)?,
                                row.get::<_, String>(6)?,
                            )))
                        },
                    )
                    .or_else(|_: rusqlite::Error| Ok::<_, rusqlite::Error>(None))
                {
                    let (id, scope, category, key, content, importance, created_at) = row;
                    results.push(ForgeSearchResult {
                        id,
                        source: "memory".to_string(),
                        content,
                        title: key,
                        score: r.score,
                        semantic_score: r.semantic_score,
                        keyword_score: r.keyword_score,
                        metadata: serde_json::json!({
                            "scope": scope,
                            "category": category,
                            "importance": importance,
                            "created_at": created_at,
                        }),
                    });
                }
            } else if let Some(uuid) = r.doc_id.strip_prefix(KB_PREFIX) {
                // Resolve knowledge item
                if let Ok(Some(row)) = conn
                    .query_row(
                        "SELECT id, title, content, tier, category, importance, created_at
                         FROM knowledge_items WHERE id = ?1",
                        params![uuid],
                        |row| {
                            Ok(Some((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, i32>(5)?,
                                row.get::<_, String>(6)?,
                            )))
                        },
                    )
                    .or_else(|_: rusqlite::Error| Ok::<_, rusqlite::Error>(None))
                {
                    let (id, title, content, tier, category, importance, created_at) = row;
                    results.push(ForgeSearchResult {
                        id,
                        source: "knowledge".to_string(),
                        content,
                        title: Some(title),
                        score: r.score,
                        semantic_score: r.semantic_score,
                        keyword_score: r.keyword_score,
                        metadata: serde_json::json!({
                            "tier": tier,
                            "category": category,
                            "importance": importance,
                            "created_at": created_at,
                        }),
                    });
                }
            }
        }

        Ok(results)
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> ForgeMemoryEngine {
        // Use in-memory DB, skip embeddings for fast tests
        let store = ForgeMemoryStore::open_memory().unwrap();
        ForgeMemoryEngine {
            store,
            embeddings: None, // BM25-only in tests
            hnsw: Arc::new(SharedHnswIndex::new(HnswConfig::default())),
            bm25: Arc::new(SharedBm25Engine::new()),
            graph: RwLock::new(KnowledgeGraph::new()),
            dimensions: 384,
            ops_since_persist: AtomicU64::new(0),
        }
    }

    #[test]
    fn test_add_and_search_memory() {
        let engine = test_engine();

        let id = engine
            .add_memory("Rust ownership prevents data races", "core", 0.9, "programming")
            .unwrap();
        assert!(!id.is_empty());

        // BM25-only search (no embeddings in test)
        let results = engine.search("Rust ownership", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].source, "memory");
        assert!(results[0].content.contains("ownership"));
    }

    #[test]
    fn test_add_and_search_knowledge() {
        let engine = test_engine();

        let id = engine
            .add_knowledge(
                "HNSW Algorithm",
                "Hierarchical Navigable Small World graphs for ANN search",
                "verified",
                "algorithms",
                3,
            )
            .unwrap();
        assert!(!id.is_empty());

        let results = engine.search_knowledge("HNSW vector search", 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].source, "knowledge");
    }

    #[test]
    fn test_memory_lifecycle() {
        let engine = test_engine();

        let id = engine
            .add_memory("Temporary info", "recall", 0.3, "temp")
            .unwrap();

        // Promote to core
        let promoted = engine.promote_memory(&id, "core").unwrap();
        assert!(promoted);

        // Should appear in core
        let core = engine.get_core_memories().unwrap();
        assert!(core.iter().any(|m| m.id == id));

        // Demote back
        let demoted = engine.demote_memory(&id).unwrap();
        assert!(demoted);

        // Delete
        let deleted = engine.delete_memory(&id).unwrap();
        assert!(deleted);
    }

    #[test]
    fn test_fsrs_review() {
        let engine = test_engine();

        let id = engine
            .add_memory("Spaced repetition fact", "recall", 0.5, "learning")
            .unwrap();

        // Review with different ratings
        engine.review_memory(&id, "good").unwrap();
        engine.review_memory(&id, "easy").unwrap();

        // Invalid rating should fail
        assert!(engine.review_memory(&id, "invalid").is_err());
    }

    #[test]
    fn test_consolidation() {
        let engine = test_engine();

        // Add some memories
        for i in 0..5 {
            engine
                .add_memory(&format!("Memory {i}"), "recall", 0.5, "test")
                .unwrap();
        }

        let report = engine.consolidate().unwrap();
        // Report should be valid (may have no changes with fresh data)
        assert!(report.decayed >= 0);
    }

    #[test]
    fn test_knowledge_graph() {
        let engine = test_engine();

        // Add nodes
        engine
            .kg_add_node("rust", "concept", "Rust Language", None)
            .unwrap();
        engine
            .kg_add_node("ownership", "concept", "Ownership Model", None)
            .unwrap();
        engine
            .kg_add_node("borrowing", "concept", "Borrowing Rules", None)
            .unwrap();

        // Add edges
        engine
            .kg_add_edge("rust", "ownership", "contains", 1.0)
            .unwrap();
        engine
            .kg_add_edge("ownership", "borrowing", "derives", 0.8)
            .unwrap();

        // Traverse
        let path = engine.kg_traverse("rust", "borrowing").unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() >= 2);

        // Neighbors
        let neighbors = engine.kg_neighbors("rust").unwrap();
        assert!(!neighbors.is_empty());

        // Communities
        let communities = engine.kg_communities().unwrap();
        assert!(!communities.is_empty());

        // Most connected
        let top = engine.kg_most_connected(5).unwrap();
        assert!(!top.is_empty());
        assert!(top[0].degree > 0);
    }

    #[test]
    fn test_chat_persistence() {
        let engine = test_engine();

        let conv_id = engine.create_conversation(Some("Test Chat"), None).unwrap();
        assert!(!conv_id.is_empty());

        let msg_id = engine
            .save_message(&conv_id, "user", "Hello!", None)
            .unwrap();
        assert!(msg_id > 0);

        let messages = engine.get_messages(&conv_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello!");
    }

    #[test]
    fn test_stats_and_status() {
        let engine = test_engine();

        let stats = engine.stats().unwrap();
        assert_eq!(stats.schema_version, 3);

        let status = engine.status();
        assert!(status.initialized);
        assert_eq!(status.embedding_dimensions, 384);
    }

    #[test]
    fn test_persist_and_optimize() {
        let engine = test_engine();

        // Add some data
        engine
            .add_memory("Persist test", "core", 0.9, "test")
            .unwrap();

        // Persist should succeed
        engine.persist().unwrap();

        // Optimize should not panic
        engine.optimize();
    }

    #[test]
    fn test_kg_subgraph() {
        let engine = test_engine();

        engine
            .kg_add_node("center", "concept", "Center", None)
            .unwrap();
        engine
            .kg_add_node("leaf1", "concept", "Leaf 1", None)
            .unwrap();
        engine
            .kg_add_node("leaf2", "concept", "Leaf 2", None)
            .unwrap();
        engine
            .kg_add_edge("center", "leaf1", "contains", 1.0)
            .unwrap();
        engine
            .kg_add_edge("center", "leaf2", "references", 0.5)
            .unwrap();

        let sub = engine.kg_subgraph("center", 1).unwrap();
        assert!(!sub.nodes.is_empty());
    }

    #[test]
    fn test_empty_search() {
        let engine = test_engine();

        // Search on empty engine should return empty, not error
        let results = engine.search("anything", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_invalid_scope() {
        let engine = test_engine();

        let result = engine.add_memory("test", "invalid_scope", 0.5, "test");
        assert!(result.is_err());
    }
}
