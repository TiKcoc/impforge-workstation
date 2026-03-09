# ForgeMemory Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build ForgeMemory — ImpForge's custom AI memory engine with SQLite storage, HNSW vector index, BM25 full-text search, RRF hybrid search, MemGPT tiered memory, and knowledge graph.

**Architecture:** SQLite Core (WAL mode, ACID) + Custom Rust AI Layer (HNSW vector index, BM25 inverted index, RRF fusion pipeline, MemGPT-inspired tiered memory, knowledge graph engine). Embedding via fastembed-rs (offline, zero-config) with optional Ollama upgrade.

**Tech Stack:** Rust, rusqlite (bundled), fastembed 5.12, rust-stemmers, unicode-segmentation, sha2, parking_lot, serde, chrono, uuid

---

## Task 1: Add Dependencies + Create Module Skeleton

**Files:**
- Modify: `src-tauri/Cargo.toml` (add rust-stemmers, unicode-segmentation)
- Create: `src-tauri/src/forge_memory/mod.rs`
- Create: `src-tauri/src/forge_memory/store.rs`
- Create: `src-tauri/src/forge_memory/migration.rs`
- Modify: `src-tauri/src/lib.rs` (register forge_memory module)

**Step 1: Add dependencies to Cargo.toml**

Add after the `sha2` line in Cargo.toml:

```toml
# ForgeMemory — Custom AI Memory Engine
# BM25 stemming (Snowball algorithm — Robertson & Zaragoza 2009)
rust-stemmers = "1.2"
# Unicode-aware tokenization for BM25 inverted index
unicode-segmentation = "1.12"
```

**Step 2: Create module skeleton**

Create `src-tauri/src/forge_memory/mod.rs`:

```rust
//! ForgeMemory — ImpForge's Custom AI Memory Engine
//!
//! SQLite Core + Custom Rust AI Layer:
//! - HNSW Vector Index (Malkov & Yashunin 2018)
//! - BM25 Full-Text Search (Robertson & Zaragoza 2009)
//! - Hybrid Search with RRF Fusion (Cormack et al. 2009)
//! - MemGPT Tiered Memory (Packer et al. 2023)
//! - Knowledge Graph with Temporal Validity
//! - FSRS-5 Spaced Repetition (Jarrett Ye 2022-2024)
//!
//! Embedding: fastembed-rs (offline, zero-config) + optional Ollama
//! Storage: Single SQLite file (~/.impforge/forge_memory.db)

pub mod store;
pub mod migration;
pub mod embeddings;
pub mod vector;
pub mod bm25;
pub mod search;
pub mod memory;
pub mod graph;

use serde::{Deserialize, Serialize};
```

Create empty placeholder files for each submodule (embeddings.rs, vector.rs, bm25.rs, search.rs, memory.rs, graph.rs) with just a module doc comment.

**Step 3: Create migration.rs with version tracking**

```rust
//! Schema migration system for ForgeMemory
//!
//! Tracks schema versions and auto-applies migrations on startup.

use rusqlite::Connection;

pub const CURRENT_VERSION: i64 = 1;

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );"
    )?;

    let current: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .unwrap_or(0);

    if current < 1 {
        migrate_v1(conn)?;
    }

    Ok(())
}

fn migrate_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(include_str!("sql/v001_initial.sql"))?;
    conn.execute(
        "INSERT INTO schema_version (version, description) VALUES (1, 'Initial ForgeMemory schema')",
        [],
    )?;
    Ok(())
}
```

**Step 4: Register module in lib.rs**

Add `mod forge_memory;` after the `mod style_engine;` line in `src-tauri/src/lib.rs`.

**Step 5: Verify compilation**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo check 2>&1 | tail -5`
Expected: Warnings about unused modules, but no errors.

**Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/forge_memory/ src-tauri/src/lib.rs
git commit -m "feat(forge-memory): add module skeleton and dependencies"
```

---

## Task 2: SQLite Store + Full Schema (25 Tables)

**Files:**
- Create: `src-tauri/src/forge_memory/sql/v001_initial.sql`
- Create: `src-tauri/src/forge_memory/store.rs`

**Step 1: Write the failing test**

In `store.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory_db() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let stats = store.stats().unwrap();
        assert_eq!(stats.total_memories, 0);
        assert_eq!(stats.total_knowledge, 0);
        assert_eq!(stats.total_kg_nodes, 0);
        assert_eq!(stats.schema_version, 1);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test -p impforge forge_memory::store::tests::test_open_memory_db 2>&1 | tail -10`
Expected: FAIL (ForgeMemoryStore not defined)

**Step 3: Create SQL schema file**

Create `src-tauri/src/forge_memory/sql/v001_initial.sql` with ALL 25 tables from the design doc (memories, knowledge_items, kg_nodes, kg_edges, sessions, session_summaries, tasks, task_events, task_dependencies, audit_log, code_chunks, file_index, completion_history, chat_conversations, chat_messages, evaluation_results, bm25_terms, bm25_doc_stats, bm25_corpus_stats, embeddings_cache, keywords, system_config, context_cache, model_configs, search_history).

Include all indexes from the design doc.

**Step 4: Implement ForgeMemoryStore**

In `store.rs`:

```rust
//! SQLite persistence layer for ForgeMemory
//!
//! Pattern: Same as orchestrator/store.rs — WAL mode, bundled SQLite,
//! parking_lot::Mutex for concurrent access, auto-migrations on startup.

use parking_lot::Mutex;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::migration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeMemoryStats {
    pub total_memories: u64,
    pub total_knowledge: u64,
    pub total_kg_nodes: u64,
    pub total_kg_edges: u64,
    pub total_code_chunks: u64,
    pub total_files_indexed: u64,
    pub total_chat_messages: u64,
    pub total_embeddings_cached: u64,
    pub schema_version: i64,
    pub db_size_bytes: u64,
}

pub struct ForgeMemoryStore {
    conn: Mutex<Connection>,
}

impl ForgeMemoryStore {
    pub fn open(db_path: &PathBuf) -> SqlResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path)?;
        Self::configure_and_migrate(conn)
    }

    pub fn open_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn)
    }

    fn configure_and_migrate(conn: Connection) -> SqlResult<Self> {
        // Production WAL PRAGMAs (same as orchestrator/store.rs)
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", "5000")?;
        conn.pragma_update(None, "cache_size", "-16000")?;  // 16MB
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "mmap_size", "268435456")?; // 256MB
        migration::run_migrations(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn optimize(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize;");
    }

    pub fn stats(&self) -> SqlResult<ForgeMemoryStats> {
        let conn = self.conn.lock();
        let count = |table: &str| -> u64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(0)
        };
        let version: i64 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
            .unwrap_or(0);
        Ok(ForgeMemoryStats {
            total_memories: count("memories"),
            total_knowledge: count("knowledge_items"),
            total_kg_nodes: count("kg_nodes"),
            total_kg_edges: count("kg_edges"),
            total_code_chunks: count("code_chunks"),
            total_files_indexed: count("file_index"),
            total_chat_messages: count("chat_messages"),
            total_embeddings_cached: count("embeddings_cache"),
            schema_version: version,
            db_size_bytes: 0, // Only meaningful for file-based DB
        })
    }
}
```

**Step 5: Run test to verify it passes**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test -p impforge forge_memory::store::tests::test_open_memory_db 2>&1 | tail -10`
Expected: PASS

**Step 6: Write CRUD tests and implement**

Add tests for:
- `test_memory_crud` — insert, query, update, delete a memory
- `test_knowledge_crud` — insert, query by tier
- `test_kg_node_edge_crud` — insert nodes, connect edges, query
- `test_task_event_sourcing` — create task, change status, verify event log
- `test_chat_persistence` — save conversation, retrieve messages
- `test_code_chunk_crud` — index a code chunk, query by file

Implement the corresponding CRUD methods on ForgeMemoryStore.

**Step 7: Commit**

```bash
git add src-tauri/src/forge_memory/
git commit -m "feat(forge-memory): SQLite store with 25-table schema and CRUD"
```

---

## Task 3: Embedding Provider (fastembed + Ollama)

**Files:**
- Create: `src-tauri/src/forge_memory/embeddings.rs`

**Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_text_produces_384_dim() {
        let provider = EmbeddingProvider::new_default();
        let embedding = provider.embed("hello world").unwrap();
        assert_eq!(embedding.len(), 384);
        // Normalized vector: magnitude should be ~1.0
        let mag: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_embed_batch() {
        let provider = EmbeddingProvider::new_default();
        let texts = vec!["hello", "world", "test"];
        let embeddings = provider.embed_batch(&texts).unwrap();
        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
    }

    #[test]
    fn test_embedding_to_blob_roundtrip() {
        let original = vec![0.1_f32, 0.2, 0.3, -0.5];
        let blob = embedding_to_blob(&original);
        let restored = blob_to_embedding(&blob, 4);
        assert_eq!(original, restored);
    }
}
```

**Step 2: Implement embeddings.rs**

```rust
//! Embedding providers for ForgeMemory
//!
//! Default: fastembed-rs (all-MiniLM-L6-v2, 384-dim, ONNX, offline)
//! Optional: Ollama nomic-embed-text (768-dim, requires Ollama running)
//!
//! Caching: SHA-256 content hash → embedding vector (stored in embeddings_cache table)

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

pub struct EmbeddingProvider {
    model: Mutex<Option<TextEmbedding>>,
    dimensions: usize,
    ollama_url: Option<String>,
}

impl EmbeddingProvider {
    pub fn new_default() -> Self {
        // Lazy init — model loaded on first use
        Self {
            model: Mutex::new(None),
            dimensions: 384,
            ollama_url: None,
        }
    }

    pub fn with_ollama(ollama_url: String) -> Self {
        Self {
            model: Mutex::new(None),
            dimensions: 384,  // fastembed default, Ollama may override
            ollama_url: Some(ollama_url),
        }
    }

    pub fn dimensions(&self) -> usize { self.dimensions }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        self.ensure_model()?;
        let guard = self.model.lock();
        let model = guard.as_ref().unwrap();
        let results = model.embed(vec![text], None)
            .map_err(|e| format!("Embedding failed: {e}"))?;
        results.into_iter().next().ok_or_else(|| "No embedding returned".into())
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        self.ensure_model()?;
        let guard = self.model.lock();
        let model = guard.as_ref().unwrap();
        model.embed(texts.to_vec(), None)
            .map_err(|e| format!("Batch embedding failed: {e}"))
    }

    fn ensure_model(&self) -> Result<(), String> {
        let mut guard = self.model.lock();
        if guard.is_none() {
            let opts = InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(true);
            let model = TextEmbedding::try_new(opts)
                .map_err(|e| format!("Failed to load embedding model: {e}"))?;
            *guard = Some(model);
        }
        Ok(())
    }
}

// Helper: content hash for caching
pub fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

// Helper: cosine similarity
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { return 0.0; }
    dot / (mag_a * mag_b)
}

// Helper: f32 vec ↔ SQLite BLOB
pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn blob_to_embedding(blob: &[u8], dimensions: usize) -> Vec<f32> {
    blob.chunks_exact(4)
        .take(dimensions)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
```

**Step 3: Run tests**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test -p impforge forge_memory::embeddings::tests -- --test-threads=1 2>&1 | tail -10`
Expected: PASS (4 tests). Note: first run downloads ~23MB model.

**Step 4: Commit**

```bash
git add src-tauri/src/forge_memory/embeddings.rs
git commit -m "feat(forge-memory): fastembed embedding provider with cosine similarity"
```

---

## Task 4: Custom HNSW Vector Index (Malkov & Yashunin 2018)

**Files:**
- Create: `src-tauri/src/forge_memory/vector.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let mut index = HnswIndex::new(HnswConfig::default());
        index.insert("doc1".into(), vec![1.0, 0.0, 0.0]);
        index.insert("doc2".into(), vec![0.0, 1.0, 0.0]);
        index.insert("doc3".into(), vec![0.9, 0.1, 0.0]);

        let results = index.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "doc1");  // exact match
        assert_eq!(results[1].id, "doc3");  // closest
    }

    #[test]
    fn test_empty_index() {
        let index = HnswIndex::new(HnswConfig::default());
        let results = index.search(&[1.0, 0.0, 0.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_delete() {
        let mut index = HnswIndex::new(HnswConfig::default());
        index.insert("a".into(), vec![1.0, 0.0]);
        index.insert("b".into(), vec![0.0, 1.0]);
        assert_eq!(index.len(), 2);
        index.remove("a");
        assert_eq!(index.len(), 1);
        let results = index.search(&[1.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "b");
    }

    #[test]
    fn test_large_index_recall() {
        let mut index = HnswIndex::new(HnswConfig { m: 16, ef_construction: 200, ef_search: 50 });
        // Insert 1000 random-ish vectors
        for i in 0..1000 {
            let angle = (i as f32) * 0.00628; // spread around unit circle
            index.insert(format!("v{i}"), vec![angle.cos(), angle.sin(), 0.0]);
        }
        let results = index.search(&[1.0, 0.0, 0.0], 10);
        assert_eq!(results.len(), 10);
        // The closest should be near angle=0 (v0)
        assert_eq!(results[0].id, "v0");
    }
}
```

**Step 2: Implement HNSW**

Key structures:
- `HnswConfig { m: usize, ef_construction: usize, ef_search: usize }`
- `HnswNode { id: String, vector: Vec<f32>, neighbors: Vec<Vec<String>> }` (per layer)
- `HnswIndex { config, nodes: HashMap<String, HnswNode>, max_layer: usize, entry_point: Option<String> }`
- `SearchResult { id: String, distance: f32 }`

Algorithm (from paper):
1. **Insert**: Select random layer l = floor(-ln(rand) * mL). Greedy search from top to layer l+1. At layers l..0, connect to M closest neighbors using heuristic neighbor selection.
2. **Search**: Greedy descent from top layer to layer 0, at layer 0 do beam search with ef_search candidates.
3. **Distance**: Cosine distance = 1 - cosine_similarity.

**Step 3: Run tests, iterate until passing**

Run: `cargo test -p impforge forge_memory::vector::tests -v 2>&1 | tail -20`

**Step 4: Commit**

```bash
git add src-tauri/src/forge_memory/vector.rs
git commit -m "feat(forge-memory): custom HNSW vector index (Malkov & Yashunin 2018)"
```

---

## Task 5: Custom BM25 Scoring Engine (Robertson & Zaragoza 2009)

**Files:**
- Create: `src-tauri/src/forge_memory/bm25.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_code() {
        let tokens = tokenize("fn hello_world(x: i32) -> bool");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"bool".to_string()));
    }

    #[test]
    fn test_tokenize_camel_case() {
        let tokens = tokenize("getUserName");
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"name".to_string()));
    }

    #[test]
    fn test_bm25_score_basic() {
        let mut engine = Bm25Engine::new(Bm25Config::default());
        engine.add_document("d1", "the quick brown fox");
        engine.add_document("d2", "the lazy dog");
        engine.add_document("d3", "quick fox jumps");
        engine.rebuild_stats();

        let results = engine.search("quick fox", 10);
        // d1 and d3 should rank highest (both have "quick" and "fox")
        assert!(results.len() >= 2);
        let top_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
        assert!(top_ids.contains(&"d1"));
        assert!(top_ids.contains(&"d3"));
    }

    #[test]
    fn test_bm25_idf() {
        // IDF should be higher for rare terms
        let mut engine = Bm25Engine::new(Bm25Config::default());
        engine.add_document("d1", "common common common rare");
        engine.add_document("d2", "common common common");
        engine.add_document("d3", "common common");
        engine.rebuild_stats();

        let idf_common = engine.idf("common");
        let idf_rare = engine.idf("rare");
        assert!(idf_rare > idf_common);
    }
}
```

**Step 2: Implement BM25**

Key structures:
- `Bm25Config { k1: f64, b: f64 }` — default k1=1.5, b=0.75
- `Bm25Engine { config, inverted_index: HashMap<String, Vec<Posting>>, doc_lengths: HashMap<String, u32>, avg_doc_length: f64, total_docs: u64 }`
- `Posting { doc_id: String, tf: u32, positions: Vec<u32> }`
- `Bm25Result { doc_id: String, score: f64 }`

BM25 formula:
```
score(D,Q) = Σ IDF(qi) * (f(qi,D) * (k1+1)) / (f(qi,D) + k1*(1-b+b*|D|/avgdl))
IDF(qi) = ln((N - n(qi) + 0.5) / (n(qi) + 0.5) + 1)
```

Tokenizer features:
- Split on whitespace and punctuation
- Split camelCase → ["camel", "case"]
- Split snake_case → ["snake", "case"]
- Lowercase all tokens
- Optional stemming via rust-stemmers (Snowball English)
- Remove single-character tokens (except meaningful ones like "i", "x")

**Step 3: Run tests**

Run: `cargo test -p impforge forge_memory::bm25::tests -v 2>&1 | tail -20`

**Step 4: Commit**

```bash
git add src-tauri/src/forge_memory/bm25.rs
git commit -m "feat(forge-memory): custom BM25 scoring engine (Robertson & Zaragoza 2009)"
```

---

## Task 6: Hybrid Search Pipeline (RRF Fusion)

**Files:**
- Create: `src-tauri/src/forge_memory/search.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge() {
        let bm25_results = vec![
            SearchHit { id: "a".into(), score: 3.0 },
            SearchHit { id: "b".into(), score: 2.0 },
            SearchHit { id: "c".into(), score: 1.0 },
        ];
        let vector_results = vec![
            SearchHit { id: "b".into(), score: 0.95 },
            SearchHit { id: "d".into(), score: 0.90 },
            SearchHit { id: "a".into(), score: 0.85 },
        ];

        let merged = rrf_merge(&bm25_results, &vector_results, 60);
        // "a" and "b" appear in both, should rank highest
        assert!(merged.len() >= 3);
        let top2: Vec<&str> = merged.iter().take(2).map(|h| h.id.as_str()).collect();
        assert!(top2.contains(&"a"));
        assert!(top2.contains(&"b"));
    }

    #[test]
    fn test_rrf_empty_inputs() {
        let merged = rrf_merge(&[], &[], 60);
        assert!(merged.is_empty());

        let bm25_only = vec![SearchHit { id: "x".into(), score: 1.0 }];
        let merged = rrf_merge(&bm25_only, &[], 60);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "x");
    }
}
```

**Step 2: Implement search.rs**

RRF formula: `score(d) = Σ 1/(k + rank_i(d))` with k=60

Pipeline:
1. Accept query string
2. Tokenize + embed query (parallel)
3. BM25 search → top 50
4. HNSW vector search → top 50
5. RRF merge → deduplicated, re-ranked
6. Optional: temporal decay boost, keyword boost
7. Return top-K

**Step 3: Run tests, commit**

```bash
git add src-tauri/src/forge_memory/search.rs
git commit -m "feat(forge-memory): hybrid search pipeline with RRF fusion (Cormack 2009)"
```

---

## Task 7: MemGPT Tiered Memory (Packer et al. 2023)

**Files:**
- Create: `src-tauri/src/forge_memory/memory.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_recall_core_memory() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let embedder = EmbeddingProvider::new_default();
        let mut mem = TieredMemory::new(store, embedder);

        mem.store_core("project_name", "ImpForge", 0.9).unwrap();
        let result = mem.recall_core("project_name").unwrap();
        assert_eq!(result.unwrap().content, "ImpForge");
    }

    #[test]
    fn test_core_memory_limit() {
        // Core memory is limited to MAX_CORE items
        let store = ForgeMemoryStore::open_memory().unwrap();
        let embedder = EmbeddingProvider::new_default();
        let mut mem = TieredMemory::new(store, embedder);

        for i in 0..105 {
            mem.store_core(&format!("key_{i}"), &format!("val_{i}"), 0.5).unwrap();
        }
        // Oldest/lowest-importance items should have been demoted to recall
        let core_count = mem.core_count().unwrap();
        assert!(core_count <= 100);
    }

    #[test]
    fn test_recall_semantic_search() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let embedder = EmbeddingProvider::new_default();
        let mut mem = TieredMemory::new(store, embedder);

        mem.store_recall("Rust is a systems programming language", 0.7).unwrap();
        mem.store_recall("Python is great for data science", 0.6).unwrap();
        mem.store_recall("SQLite is an embedded database", 0.8).unwrap();

        let results = mem.search_recall("database engine", 2).unwrap();
        assert!(!results.is_empty());
        // SQLite result should be most relevant
        assert!(results[0].content.contains("SQLite"));
    }
}
```

**Step 2: Implement tiered memory**

- `TieredMemory { store: ForgeMemoryStore, embedder: EmbeddingProvider }`
- Core: ≤100 KV pairs, always loaded, highest importance
- Recall: Unlimited, hybrid searchable (BM25 + vector)
- Archival: Compressed, FSRS-scheduled review (reuse brain.rs FSRS-5)
- Auto promotion/demotion between tiers

**Step 3: Run tests, commit**

```bash
git add src-tauri/src/forge_memory/memory.rs
git commit -m "feat(forge-memory): MemGPT tiered memory (Packer et al. 2023)"
```

---

## Task 8: Knowledge Graph Engine

**Files:**
- Create: `src-tauri/src/forge_memory/graph.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node_and_edge() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let mut graph = KnowledgeGraph::new(store);

        graph.add_node("file:main.rs", NodeKind::File, "main.rs", None).unwrap();
        graph.add_node("fn:main", NodeKind::Function, "main", None).unwrap();
        graph.add_edge("file:main.rs", "fn:main", EdgeKind::Contains, 1.0).unwrap();

        let neighbors = graph.neighbors("file:main.rs", None).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].id, "fn:main");
    }

    #[test]
    fn test_bfs_traversal() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let mut graph = KnowledgeGraph::new(store);

        graph.add_node("a", NodeKind::Concept, "A", None).unwrap();
        graph.add_node("b", NodeKind::Concept, "B", None).unwrap();
        graph.add_node("c", NodeKind::Concept, "C", None).unwrap();
        graph.add_edge("a", "b", EdgeKind::References, 1.0).unwrap();
        graph.add_edge("b", "c", EdgeKind::References, 1.0).unwrap();

        let reachable = graph.traverse("a", 2, None).unwrap();
        assert_eq!(reachable.len(), 3);  // a, b, c
    }

    #[test]
    fn test_analytics() {
        let store = ForgeMemoryStore::open_memory().unwrap();
        let mut graph = KnowledgeGraph::new(store);
        graph.add_node("n1", NodeKind::File, "f1", None).unwrap();
        graph.add_node("n2", NodeKind::Symbol, "s1", None).unwrap();
        graph.add_edge("n1", "n2", EdgeKind::Contains, 1.0).unwrap();

        let stats = graph.analytics().unwrap();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
    }
}
```

**Step 2: Implement knowledge graph**

SQLite-backed (not just in-memory like collab.rs):
- `KnowledgeGraph { store }` — all data persisted to kg_nodes/kg_edges tables
- Node CRUD with kind validation
- Edge CRUD with temporal validity (valid_at, invalid_at)
- BFS traversal with max_hops and optional edge_kind filter
- Analytics: node_count, edge_count, by_kind breakdown, hotspots

**Step 3: Run tests, commit**

```bash
git add src-tauri/src/forge_memory/graph.rs
git commit -m "feat(forge-memory): knowledge graph engine with BFS traversal"
```

---

## Task 9: Tauri Commands + Wire Everything Together

**Files:**
- Modify: `src-tauri/src/forge_memory/mod.rs` (add all Tauri commands)
- Modify: `src-tauri/src/lib.rs` (register commands + manage state)

**Step 1: Add ForgeMemory as Tauri managed state**

In `lib.rs` setup:
```rust
// In setup closure, after other app.manage() calls:
let memory_db = dirs::data_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("."))
    .join("ImpForge")
    .join("forge_memory.db");
let forge_store = forge_memory::store::ForgeMemoryStore::open(&memory_db)
    .expect("Failed to open ForgeMemory database");
let embedder = forge_memory::embeddings::EmbeddingProvider::new_default();
app.manage(forge_memory::ForgeMemoryEngine::new(forge_store, embedder));
```

**Step 2: Define ForgeMemoryEngine wrapper**

In `mod.rs`:
```rust
pub struct ForgeMemoryEngine {
    pub store: store::ForgeMemoryStore,
    pub embedder: embeddings::EmbeddingProvider,
    pub hnsw: parking_lot::Mutex<vector::HnswIndex>,
    pub bm25: parking_lot::Mutex<bm25::Bm25Engine>,
}
```

**Step 3: Add Tauri commands**

```rust
#[tauri::command]
pub async fn forge_memory_store(...) -> Result<String, String> { ... }

#[tauri::command]
pub async fn forge_memory_recall(...) -> Result<Vec<...>, String> { ... }

#[tauri::command]
pub async fn forge_search(...) -> Result<Vec<...>, String> { ... }

// ... all 20+ commands from the design
```

**Step 4: Register all commands in lib.rs invoke_handler**

Add all `forge_memory::*` commands to the `tauri::generate_handler![]` macro.

**Step 5: Run full test suite**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test -p impforge forge_memory 2>&1 | tail -20`
Expected: ALL tests pass

**Step 6: Commit**

```bash
git add src-tauri/src/forge_memory/ src-tauri/src/lib.rs
git commit -m "feat(forge-memory): wire Tauri commands and ForgeMemoryEngine state"
```

---

## Task 10: Integration — Connect Existing Systems to ForgeMemory

**Files:**
- Modify: `src-tauri/src/ide/indexer.rs` (persist chunks to ForgeMemory)
- Modify: `src-tauri/src/ide/ai_complete.rs` (cache completions in ForgeMemory)
- Modify: `src-tauri/src/chat.rs` (persist messages to ForgeMemory)
- Modify: `src-tauri/src/evaluation/mod.rs` (persist evals to ForgeMemory)

**Step 1: Indexer integration**

After indexing code chunks in `indexer.rs`, also persist them to ForgeMemory:
- Store each CodeChunk in code_chunks table with embedding
- Update file_index table with mtime/hash for incremental indexing
- Feed BM25 engine with code content
- Insert HNSW vectors for semantic code search

**Step 2: AI completion integration**

In `ai_complete.rs`, after a completion is accepted:
- Store in completion_history table for analytics
- Track model performance over time

**Step 3: Chat persistence**

In `chat.rs`, wrap existing streaming with DB persistence:
- Create conversation on first message
- Store each message with role, content, model_id
- Embed messages for later semantic search

**Step 4: Evaluation persistence**

In `evaluation/mod.rs`, after eval completes:
- Store in evaluation_results table
- No more loss on app restart

**Step 5: Run full test suite + cargo check**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test 2>&1 | tail -20`
Expected: ALL existing + new tests pass

**Step 6: Commit**

```bash
git add src-tauri/src/ide/ src-tauri/src/chat.rs src-tauri/src/evaluation/
git commit -m "feat(forge-memory): integrate with indexer, completions, chat, evaluation"
```

---

## Summary

| Task | Component | Est. LoC | Tests |
|------|-----------|----------|-------|
| 1 | Module skeleton + deps | ~50 | 0 |
| 2 | SQLite store + schema | ~700 | 8+ |
| 3 | Embedding provider | ~150 | 4 |
| 4 | HNSW vector index | ~400 | 4+ |
| 5 | BM25 scoring engine | ~350 | 4+ |
| 6 | Hybrid search (RRF) | ~200 | 3+ |
| 7 | MemGPT tiered memory | ~300 | 3+ |
| 8 | Knowledge graph | ~350 | 3+ |
| 9 | Tauri commands + wiring | ~300 | 2+ |
| 10 | Integration | ~200 | 4+ |
| **Total** | | **~3,000** | **35+** |
