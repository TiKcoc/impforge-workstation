// Public API — consumed via forge_memory::commands Tauri layer
#![allow(dead_code)]
//! Custom BM25 Scoring Engine (Robertson & Zaragoza 2009)
//!
//! Okapi BM25 full-text search with Unicode-aware tokenization, Porter stemming,
//! and stop-word removal. Inverted index stored in-memory with SQLite persistence.
//!
//! Scoring formula (per the Probabilistic Relevance Framework):
//!
//!   score(D, Q) = SUM_i  IDF(qi) * tf(qi,D) * (k1+1)
//!                         / (tf(qi,D) + k1 * (1 - b + b * |D| / avgdl))
//!
//! Where:
//!   IDF(qi) = ln( (N - n(qi) + 0.5) / (n(qi) + 0.5) + 1 )
//!   N       = total documents in corpus
//!   n(qi)   = number of documents containing term qi
//!   tf      = raw term frequency in document D
//!   |D|     = document length (total terms after processing)
//!   avgdl   = average document length across corpus
//!   k1      = term frequency saturation parameter (default 1.2)
//!   b       = length normalization parameter (default 0.75)
//!
//! References:
//!   - Robertson, S. E., & Zaragoza, H. (2009). The Probabilistic Relevance
//!     Framework: BM25 and Beyond. Foundations and Trends in Information Retrieval.
//!   - Robertson, S. E., Walker, S., Jones, S., Hancock-Beaulieu, M., & Gatford, M.
//!     (1994). Okapi at TREC-3. NIST Special Publication.

use parking_lot::RwLock;
use rusqlite::{params, Connection, Result as SqlResult};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;

/// Collection name used in the `doc_table` column for SQLite persistence.
const DOC_TABLE: &str = "forge_memory";

// ── Stop words ──────────────────────────────────────────────────

/// Top ~50 English stop words (based on Buckley et al., SIGIR 1995).
fn stop_words() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "do", "for",
        "from", "had", "has", "have", "he", "her", "his", "how", "i", "if",
        "in", "into", "is", "it", "its", "my", "no", "nor", "not", "of",
        "on", "or", "our", "out", "so", "than", "that", "the", "then",
        "there", "these", "they", "this", "to", "up", "was", "we", "what",
        "when", "which", "who", "will", "with", "you", "your",
    ]
    .into_iter()
    .collect()
}

// ── Text processing pipeline ────────────────────────────────────

/// Unicode-aware tokenization.
///
/// Uses `unicode-segmentation` word boundaries, filters to alphanumeric tokens,
/// and lowercases. Also splits camelCase and snake_case identifiers.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.unicode_word_indices() {
        let (_, segment) = word;
        // Split on underscores (snake_case)
        for part in segment.split('_') {
            if part.is_empty() {
                continue;
            }
            // Split camelCase: insert boundary before uppercase letters
            let mut current = String::new();
            for ch in part.chars() {
                if ch.is_uppercase() && !current.is_empty() {
                    let lower = current.to_lowercase();
                    if lower.chars().any(|c| c.is_alphanumeric()) {
                        tokens.push(lower);
                    }
                    current.clear();
                }
                current.push(ch);
            }
            if !current.is_empty() {
                let lower = current.to_lowercase();
                if lower.chars().any(|c| c.is_alphanumeric()) {
                    tokens.push(lower);
                }
            }
        }
    }
    tokens
}

/// Apply Porter stemmer (English) to a single token.
fn stem(token: &str) -> String {
    let stemmer = Stemmer::create(Algorithm::English);
    stemmer.stem(token).to_string()
}

/// Full text processing pipeline: tokenize -> lowercase -> remove stops -> stem.
fn process_text(text: &str) -> Vec<String> {
    let stops = stop_words();
    tokenize(text)
        .into_iter()
        .filter(|t| !stops.contains(t.as_str()))
        .map(|t| stem(&t))
        .collect()
}

// ── Inverted index entry ────────────────────────────────────────

/// Per-document posting for a single term.
#[derive(Debug, Clone)]
struct Posting {
    /// Raw term frequency in this document.
    tf: u32,
    /// Token positions where this term appears (0-indexed).
    positions: Vec<u32>,
}

/// Per-document metadata.
#[derive(Debug, Clone)]
struct DocStats {
    /// Total number of processed tokens in this document.
    length: u32,
}

// ── BM25 Engine ─────────────────────────────────────────────────

/// In-memory BM25 inverted index with configurable parameters.
///
/// Thread-safety: wrap in `parking_lot::RwLock` via [`SharedBm25Engine`].
///
/// # Example
/// ```ignore
/// let mut engine = Bm25Engine::new(1.2, 0.75);
/// engine.index_document("doc1", "The quick brown fox");
/// engine.index_document("doc2", "A lazy brown dog");
/// let results = engine.search("brown fox", 10);
/// assert_eq!(results[0].0, "doc1");
/// ```
pub struct Bm25Engine {
    /// Term frequency saturation. Higher values increase the impact of
    /// additional occurrences. Robertson & Zaragoza recommend 1.2-2.0.
    k1: f64,
    /// Length normalization factor in [0, 1]. At b=1, long documents are
    /// heavily penalized; at b=0, length is ignored.
    b: f64,
    /// Inverted index: term -> { doc_id -> Posting }.
    index: HashMap<String, HashMap<String, Posting>>,
    /// Document statistics: doc_id -> DocStats.
    doc_stats: HashMap<String, DocStats>,
    /// Sum of all document lengths (for computing avgdl).
    total_length: u64,
}

impl Bm25Engine {
    /// Create a new engine with the given BM25 parameters.
    ///
    /// # Arguments
    /// * `k1` - Term frequency saturation (default 1.2, typical range 1.2-2.0)
    /// * `b`  - Length normalization (default 0.75, range 0.0-1.0)
    pub fn new(k1: f64, b: f64) -> Self {
        Self {
            k1,
            b,
            index: HashMap::new(),
            doc_stats: HashMap::new(),
            total_length: 0,
        }
    }

    /// Create with default BM25 parameters (k1=1.2, b=0.75).
    pub fn default_params() -> Self {
        Self::new(1.2, 0.75)
    }

    /// Number of indexed documents.
    pub fn document_count(&self) -> usize {
        self.doc_stats.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.doc_stats.is_empty()
    }

    /// Average document length across the corpus.
    fn avg_doc_length(&self) -> f64 {
        let n = self.doc_stats.len();
        if n == 0 {
            return 0.0;
        }
        self.total_length as f64 / n as f64
    }

    /// Index a document. If a document with the same ID already exists,
    /// it is removed first and re-indexed.
    pub fn index_document(&mut self, doc_id: &str, content: &str) {
        // Remove existing entry if present (idempotent re-index)
        if self.doc_stats.contains_key(doc_id) {
            self.remove_document(doc_id);
        }

        let terms = process_text(content);
        let doc_length = terms.len() as u32;

        // Build postings for this document
        let mut term_positions: HashMap<String, Vec<u32>> = HashMap::new();
        for (pos, term) in terms.iter().enumerate() {
            term_positions
                .entry(term.clone())
                .or_default()
                .push(pos as u32);
        }

        // Insert into inverted index
        let doc_id_owned = doc_id.to_string();
        for (term, positions) in term_positions {
            let posting = Posting {
                tf: positions.len() as u32,
                positions,
            };
            self.index
                .entry(term)
                .or_default()
                .insert(doc_id_owned.clone(), posting);
        }

        // Update document stats
        self.doc_stats.insert(
            doc_id_owned,
            DocStats {
                length: doc_length,
            },
        );
        self.total_length += doc_length as u64;
    }

    /// Remove a document from the index. Returns `true` if the document existed.
    pub fn remove_document(&mut self, doc_id: &str) -> bool {
        let stats = match self.doc_stats.remove(doc_id) {
            Some(s) => s,
            None => return false,
        };

        self.total_length -= stats.length as u64;

        // Remove from inverted index, cleaning up empty term entries
        let mut empty_terms: Vec<String> = Vec::new();
        for (term, postings) in &mut self.index {
            postings.remove(doc_id);
            if postings.is_empty() {
                empty_terms.push(term.clone());
            }
        }
        for term in empty_terms {
            self.index.remove(&term);
        }

        true
    }

    /// Compute the IDF (Inverse Document Frequency) for a term.
    ///
    /// Uses the Robertson-Sparck Jones formula with +1 smoothing to avoid
    /// negative values for very common terms:
    ///
    ///   IDF(q) = ln( (N - n(q) + 0.5) / (n(q) + 0.5) + 1 )
    fn idf(&self, term: &str) -> f64 {
        let n = self.doc_stats.len() as f64;
        let df = self
            .index
            .get(term)
            .map(|postings| postings.len() as f64)
            .unwrap_or(0.0);

        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Search the index and return the top-k results ranked by BM25 score.
    ///
    /// Returns a vector of (doc_id, score) pairs, sorted descending by score.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        if self.doc_stats.is_empty() || query.is_empty() {
            return Vec::new();
        }

        let query_terms = process_text(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        let avgdl = self.avg_doc_length();
        let mut scores: HashMap<String, f64> = HashMap::new();

        for qt in &query_terms {
            let idf = self.idf(qt);
            if let Some(postings) = self.index.get(qt) {
                for (doc_id, posting) in postings {
                    let doc_len = self
                        .doc_stats
                        .get(doc_id)
                        .map(|s| s.length as f64)
                        .unwrap_or(0.0);

                    let tf = posting.tf as f64;
                    let numerator = tf * (self.k1 + 1.0);
                    let denominator =
                        tf + self.k1 * (1.0 - self.b + self.b * doc_len / avgdl);

                    let term_score = idf * numerator / denominator;
                    *scores.entry(doc_id.clone()).or_insert(0.0) += term_score;
                }
            }
        }

        // Sort by score descending
        let mut results: Vec<(String, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    // ── SQLite persistence ──────────────────────────────────────

    /// Save the entire index to SQLite using the existing v001 tables:
    /// `bm25_terms`, `bm25_doc_stats`, `bm25_corpus_stats`.
    ///
    /// This performs a full snapshot (delete + re-insert) within a transaction.
    pub fn save_to_db(&self, conn: &Connection) -> SqlResult<()> {
        let tx = conn.unchecked_transaction()?;

        // Clear existing data for our collection
        tx.execute(
            "DELETE FROM bm25_terms WHERE doc_table = ?1",
            params![DOC_TABLE],
        )?;
        tx.execute(
            "DELETE FROM bm25_doc_stats WHERE doc_table = ?1",
            params![DOC_TABLE],
        )?;
        tx.execute(
            "DELETE FROM bm25_corpus_stats WHERE doc_table = ?1",
            params![DOC_TABLE],
        )?;

        // Write inverted index
        {
            let mut stmt = tx.prepare(
                "INSERT INTO bm25_terms (term, doc_id, doc_table, tf, positions)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for (term, postings) in &self.index {
                for (doc_id, posting) in postings {
                    let positions_json = serde_json::to_string(&posting.positions)
                        .unwrap_or_else(|_| "[]".to_string());
                    stmt.execute(params![
                        term,
                        doc_id,
                        DOC_TABLE,
                        posting.tf as i64,
                        positions_json,
                    ])?;
                }
            }
        }

        // Write document stats
        {
            let mut stmt = tx.prepare(
                "INSERT INTO bm25_doc_stats (doc_id, doc_table, doc_length)
                 VALUES (?1, ?2, ?3)",
            )?;

            for (doc_id, stats) in &self.doc_stats {
                stmt.execute(params![doc_id, DOC_TABLE, stats.length as i64])?;
            }
        }

        // Write corpus stats
        tx.execute(
            "INSERT INTO bm25_corpus_stats (doc_table, total_docs, avg_doc_length)
             VALUES (?1, ?2, ?3)",
            params![DOC_TABLE, self.doc_stats.len() as i64, self.avg_doc_length()],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Load the index from SQLite. Returns `None` if no data exists for
    /// the `forge_memory` collection.
    ///
    /// Reconstructs the in-memory inverted index, document stats, and
    /// corpus metadata from the three BM25 tables.
    pub fn load_from_db(conn: &Connection) -> SqlResult<Option<Self>> {
        Self::load_from_db_with_params(conn, 1.2, 0.75)
    }

    /// Load from SQLite with custom BM25 parameters.
    pub fn load_from_db_with_params(
        conn: &Connection,
        k1: f64,
        b: f64,
    ) -> SqlResult<Option<Self>> {
        // Check if the terms table exists at all
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='bm25_terms'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        if !table_exists {
            return Ok(None);
        }

        // Load document stats
        let mut doc_stats: HashMap<String, DocStats> = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT doc_id, doc_length FROM bm25_doc_stats WHERE doc_table = ?1",
            )?;
            let rows = stmt.query_map(params![DOC_TABLE], |row| {
                let doc_id: String = row.get(0)?;
                let length: i64 = row.get(1)?;
                Ok((doc_id, length as u32))
            })?;
            for row in rows {
                let (doc_id, length) = row?;
                doc_stats.insert(doc_id, DocStats { length });
            }
        }

        if doc_stats.is_empty() {
            return Ok(None);
        }

        // Load inverted index
        let mut index: HashMap<String, HashMap<String, Posting>> = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT term, doc_id, tf, positions FROM bm25_terms WHERE doc_table = ?1",
            )?;
            let rows = stmt.query_map(params![DOC_TABLE], |row| {
                let term: String = row.get(0)?;
                let doc_id: String = row.get(1)?;
                let tf: i64 = row.get(2)?;
                let positions_json: String = row.get::<_, String>(3).unwrap_or_default();
                Ok((term, doc_id, tf as u32, positions_json))
            })?;
            for row in rows {
                let (term, doc_id, tf, positions_json) = row?;
                let positions: Vec<u32> =
                    serde_json::from_str(&positions_json).unwrap_or_default();
                let posting = Posting { tf, positions };
                index.entry(term).or_default().insert(doc_id, posting);
            }
        }

        // Compute total_length from doc_stats
        let total_length: u64 = doc_stats.values().map(|s| s.length as u64).sum();

        Ok(Some(Self {
            k1,
            b,
            index,
            doc_stats,
            total_length,
        }))
    }
}

// ── Thread-safe wrapper ─────────────────────────────────────────

/// Thread-safe BM25 engine wrapper using `parking_lot::RwLock`.
///
/// Multiple readers can search concurrently; writers get exclusive access.
pub struct SharedBm25Engine {
    inner: RwLock<Bm25Engine>,
}

impl SharedBm25Engine {
    /// Create a new shared engine with default parameters (k1=1.2, b=0.75).
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Bm25Engine::default_params()),
        }
    }

    /// Create with custom parameters.
    pub fn with_params(k1: f64, b: f64) -> Self {
        Self {
            inner: RwLock::new(Bm25Engine::new(k1, b)),
        }
    }

    pub fn index_document(&self, doc_id: &str, content: &str) {
        self.inner.write().index_document(doc_id, content);
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        self.inner.read().search(query, limit)
    }

    pub fn remove_document(&self, doc_id: &str) -> bool {
        self.inner.write().remove_document(doc_id)
    }

    pub fn document_count(&self) -> usize {
        self.inner.read().document_count()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    pub fn save_to_db(&self, conn: &Connection) -> SqlResult<()> {
        self.inner.read().save_to_db(conn)
    }

    pub fn load_from_db(conn: &Connection) -> SqlResult<Option<Self>> {
        match Bm25Engine::load_from_db(conn)? {
            Some(engine) => Ok(Some(Self {
                inner: RwLock::new(engine),
            })),
            None => Ok(None),
        }
    }
}

impl Default for SharedBm25Engine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory SQLite database with the BM25 tables from v001.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;

             CREATE TABLE IF NOT EXISTS bm25_terms (
                 term      TEXT NOT NULL,
                 doc_id    TEXT NOT NULL,
                 doc_table TEXT NOT NULL,
                 tf        INTEGER NOT NULL,
                 positions TEXT,
                 PRIMARY KEY (term, doc_id, doc_table)
             );
             CREATE INDEX IF NOT EXISTS idx_bm25_term ON bm25_terms(term);
             CREATE INDEX IF NOT EXISTS idx_bm25_doc ON bm25_terms(doc_id, doc_table);

             CREATE TABLE IF NOT EXISTS bm25_doc_stats (
                 doc_id     TEXT NOT NULL,
                 doc_table  TEXT NOT NULL,
                 doc_length INTEGER NOT NULL,
                 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                 PRIMARY KEY (doc_id, doc_table)
             );

             CREATE TABLE IF NOT EXISTS bm25_corpus_stats (
                 doc_table      TEXT PRIMARY KEY,
                 total_docs     INTEGER NOT NULL DEFAULT 0,
                 avg_doc_length REAL NOT NULL DEFAULT 0.0,
                 updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             );",
        )
        .unwrap();
        conn
    }

    // ── 1. Basic indexing and search ────────────────────────────

    #[test]
    fn test_basic_index_and_search() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "the quick brown fox jumps over the lazy dog");
        engine.index_document("d2", "a fast brown car races down the highway");

        assert_eq!(engine.document_count(), 2);

        let results = engine.search("brown fox", 10);
        assert!(!results.is_empty());
        // "d1" has both "brown" and "fox" — should rank first
        assert_eq!(results[0].0, "d1");
        assert!(results[0].1 > 0.0);
    }

    // ── 2. Multi-document ranking (correct order) ───────────────

    #[test]
    fn test_multi_document_ranking() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("a", "rust programming language systems");
        engine.index_document("b", "rust rust rust is great for systems programming");
        engine.index_document("c", "python is a popular language");

        let results = engine.search("rust programming", 10);
        // "b" has more "rust" occurrences and also "programming" -> should rank first
        assert!(results.len() >= 2);
        assert_eq!(results[0].0, "b");
        assert_eq!(results[1].0, "a");
        // "c" has no "rust" — its score should be lower than both a and b
        if results.len() == 3 {
            assert!(results[2].1 < results[1].1);
        }
    }

    // ── 3. Stop word removal ────────────────────────────────────

    #[test]
    fn test_stop_word_removal() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "the cat sat on the mat");

        // Searching for only stop words should return nothing
        let results = engine.search("the on", 10);
        assert!(results.is_empty());

        // "cat" is not a stop word
        let results = engine.search("cat", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "d1");
    }

    // ── 4. Stemming ─────────────────────────────────────────────

    #[test]
    fn test_stemming() {
        // Verify "running" stems to "run"
        assert_eq!(stem("running"), "run");
        assert_eq!(stem("jumps"), "jump");
        // Porter stemmer output for "quickly" — verify actual stem
        let quickly_stem = stem("quickly");
        assert!(
            quickly_stem == "quick" || quickly_stem == "quickli",
            "Expected 'quick' or 'quickli', got '{quickly_stem}'"
        );

        // Documents should match on stemmed forms
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "The runners are running quickly");
        engine.index_document("d2", "She ran in the marathon");

        // "runs" should stem to "run" and match "running"/"runners" in d1
        let results = engine.search("runs", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "d1");
    }

    // ── 5. Empty query / empty index ────────────────────────────

    #[test]
    fn test_empty_query() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "some content");

        let results = engine.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_index() {
        let engine = Bm25Engine::default_params();

        let results = engine.search("anything", 10);
        assert!(results.is_empty());
        assert_eq!(engine.document_count(), 0);
        assert!(engine.is_empty());
    }

    // ── 6. Remove document and re-search ────────────────────────

    #[test]
    fn test_remove_document() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "alpha beta gamma");
        engine.index_document("d2", "beta gamma delta");
        engine.index_document("d3", "gamma delta epsilon");

        assert_eq!(engine.document_count(), 3);

        // Remove d2
        assert!(engine.remove_document("d2"));
        assert_eq!(engine.document_count(), 2);

        // Removing again returns false
        assert!(!engine.remove_document("d2"));

        // Search should not return d2
        let results = engine.search("beta", 10);
        for (id, _) in &results {
            assert_ne!(id, "d2");
        }
        // d1 still has "beta"
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "d1");
    }

    // ── 7. Term frequency saturation (k1 effect) ────────────────

    #[test]
    fn test_k1_saturation_effect() {
        // With very high k1, more term occurrences should have greater impact
        let mut engine_high_k1 = Bm25Engine::new(100.0, 0.75);
        // With low k1, diminishing returns kick in faster
        let mut engine_low_k1 = Bm25Engine::new(0.1, 0.75);

        let doc_many = "rust rust rust rust rust rust rust rust rust rust";
        let doc_few = "rust programming";

        engine_high_k1.index_document("many", doc_many);
        engine_high_k1.index_document("few", doc_few);
        engine_low_k1.index_document("many", doc_many);
        engine_low_k1.index_document("few", doc_few);

        let results_high = engine_high_k1.search("rust", 10);
        let results_low = engine_low_k1.search("rust", 10);

        // With high k1, "many" should score much higher relative to "few"
        let ratio_high = results_high[0].1 / results_high[1].1;
        let ratio_low = results_low[0].1 / results_low[1].1;

        // The ratio difference between high-tf and low-tf docs should be
        // larger with high k1 (less saturation)
        assert!(
            ratio_high > ratio_low,
            "Higher k1 should amplify tf differences: high_ratio={ratio_high:.3}, low_ratio={ratio_low:.3}"
        );
    }

    // ── 8. Length normalization (b effect) ───────────────────────

    #[test]
    fn test_b_length_normalization() {
        // b=0: no length normalization — long docs not penalized
        let mut engine_b0 = Bm25Engine::new(1.2, 0.0);
        // b=1: full length normalization — long docs heavily penalized
        let mut engine_b1 = Bm25Engine::new(1.2, 1.0);

        let short_doc = "rust is great";
        let long_doc = "rust is a systems programming language that runs fast \
                        and provides memory safety without garbage collection \
                        while being efficient and reliable for production use";

        engine_b0.index_document("short", short_doc);
        engine_b0.index_document("long", long_doc);
        engine_b1.index_document("short", short_doc);
        engine_b1.index_document("long", long_doc);

        let results_b0 = engine_b0.search("rust", 10);
        let results_b1 = engine_b1.search("rust", 10);

        // With b=1, the short document should score relatively better
        // because the long document is penalized for its length
        let short_score_b0 = results_b0.iter().find(|(id, _)| id == "short").unwrap().1;
        let long_score_b0 = results_b0.iter().find(|(id, _)| id == "long").unwrap().1;
        let short_score_b1 = results_b1.iter().find(|(id, _)| id == "short").unwrap().1;
        let long_score_b1 = results_b1.iter().find(|(id, _)| id == "long").unwrap().1;

        // Ratio of short/long should be higher with b=1 (short favored more)
        let ratio_b0 = short_score_b0 / long_score_b0;
        let ratio_b1 = short_score_b1 / long_score_b1;
        assert!(
            ratio_b1 > ratio_b0,
            "b=1 should favor shorter docs more: ratio_b0={ratio_b0:.3}, ratio_b1={ratio_b1:.3}"
        );
    }

    // ── 9. Unicode text handling ────────────────────────────────

    #[test]
    fn test_unicode_handling() {
        let mut engine = Bm25Engine::default_params();

        engine.index_document("de", "Kuenstliche Intelligenz ist faszinierend");
        engine.index_document("fr", "Intelligence artificielle est fascinante");
        engine.index_document("mixed", "AI and kuenstliche intelligence together");

        let results = engine.search("intelligenz", 10);
        assert!(!results.is_empty());
        // "de" has the exact match
        assert_eq!(results[0].0, "de");

        // CJK-like and accented characters should tokenize without panic
        engine.index_document("accent", "cafe resume naive");
        let results = engine.search("cafe", 10);
        assert!(!results.is_empty());
    }

    // ── 10. Persistence roundtrip ───────────────────────────────

    #[test]
    fn test_persistence_roundtrip() {
        let conn = test_db();

        let mut engine = Bm25Engine::default_params();
        engine.index_document("alpha", "machine learning algorithms for text");
        engine.index_document("beta", "deep learning neural networks");
        engine.index_document("gamma", "natural language processing text mining");

        // Save
        engine.save_to_db(&conn).unwrap();

        // Load
        let loaded = Bm25Engine::load_from_db(&conn).unwrap().unwrap();
        assert_eq!(loaded.document_count(), 3);

        // Search results should be identical
        let query = "text learning";
        let original_results = engine.search(query, 10);
        let loaded_results = loaded.search(query, 10);

        assert_eq!(original_results.len(), loaded_results.len());
        for (orig, load) in original_results.iter().zip(loaded_results.iter()) {
            assert_eq!(orig.0, load.0, "doc_id mismatch");
            assert!(
                (orig.1 - load.1).abs() < 1e-10,
                "score mismatch: {} vs {}",
                orig.1,
                load.1
            );
        }
    }

    // ── 11. Persistence empty index ─────────────────────────────

    #[test]
    fn test_persistence_empty() {
        let conn = test_db();

        let engine = Bm25Engine::default_params();
        engine.save_to_db(&conn).unwrap();

        let loaded = Bm25Engine::load_from_db(&conn).unwrap();
        assert!(loaded.is_none(), "Empty index should load as None");
    }

    // ── 12. Large corpus (100+ documents) ───────────────────────

    #[test]
    fn test_large_corpus() {
        let mut engine = Bm25Engine::default_params();

        let topics = [
            "machine learning", "deep learning", "neural networks",
            "natural language processing", "computer vision",
            "reinforcement learning", "generative models",
            "transformers architecture", "graph neural networks",
            "federated learning",
        ];

        // Index 150 documents
        for i in 0..150 {
            let topic = topics[i % topics.len()];
            let content = format!(
                "Document {i} about {topic}. This covers advanced research \
                 in {topic} with applications to real world problems. \
                 Additional context number {i}.",
            );
            engine.index_document(&format!("doc_{i}"), &content);
        }

        assert_eq!(engine.document_count(), 150);

        let results = engine.search("neural networks deep learning", 10);
        assert_eq!(results.len(), 10);

        // Scores should be positive and descending
        for i in 1..results.len() {
            assert!(
                results[i].1 <= results[i - 1].1 + 1e-10,
                "Results should be sorted descending by score"
            );
        }

        // All returned doc IDs should be unique
        let unique_ids: HashSet<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(unique_ids.len(), results.len());
    }

    // ── 13. Re-index same document (idempotent) ─────────────────

    #[test]
    fn test_reindex_document() {
        let mut engine = Bm25Engine::default_params();

        engine.index_document("d1", "original content about rust");
        let results1 = engine.search("rust", 10);
        assert_eq!(results1.len(), 1);

        // Re-index with different content
        engine.index_document("d1", "updated content about python");
        assert_eq!(engine.document_count(), 1);

        // "rust" should no longer match
        let results2 = engine.search("rust", 10);
        assert!(results2.is_empty());

        // "python" should match
        let results3 = engine.search("python", 10);
        assert_eq!(results3.len(), 1);
        assert_eq!(results3[0].0, "d1");
    }

    // ── 14. IDF correctness ─────────────────────────────────────

    #[test]
    fn test_idf_rare_vs_common() {
        let mut engine = Bm25Engine::default_params();

        // "common" appears in all docs, "rare" in only one
        engine.index_document("d1", "common rare");
        engine.index_document("d2", "common word");
        engine.index_document("d3", "common term");
        engine.index_document("d4", "common phrase");

        let idf_common = engine.idf(&stem("common"));
        let idf_rare = engine.idf(&stem("rare"));

        // Rare terms should have higher IDF
        assert!(
            idf_rare > idf_common,
            "Rare term should have higher IDF: rare={idf_rare:.4}, common={idf_common:.4}"
        );
        // IDF should be non-negative (due to +1 smoothing)
        assert!(idf_common >= 0.0);
        assert!(idf_rare > 0.0);
    }

    // ── 15. SharedBm25Engine (thread-safe wrapper) ──────────────

    #[test]
    fn test_shared_engine_basic() {
        let shared = SharedBm25Engine::new();

        shared.index_document("s1", "concurrent search engine");
        shared.index_document("s2", "parallel indexing system");

        assert_eq!(shared.document_count(), 2);
        assert!(!shared.is_empty());

        let results = shared.search("search engine", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "s1");

        assert!(shared.remove_document("s1"));
        assert_eq!(shared.document_count(), 1);
    }

    // ── 16. SharedBm25Engine persistence ────────────────────────

    #[test]
    fn test_shared_engine_persistence() {
        let conn = test_db();

        let shared = SharedBm25Engine::new();
        shared.index_document("p1", "persistence test alpha");
        shared.index_document("p2", "persistence test beta");

        shared.save_to_db(&conn).unwrap();

        let loaded = SharedBm25Engine::load_from_db(&conn).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.document_count(), 2);

        let results = loaded.search("alpha", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "p1");
    }

    // ── 17. Tokenizer: camelCase and snake_case splitting ───────

    #[test]
    fn test_tokenizer_code_splitting() {
        let tokens = tokenize("getUserName");
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"name".to_string()));

        let tokens = tokenize("my_function_name");
        assert!(tokens.contains(&"my".to_string()));
        assert!(tokens.contains(&"function".to_string()));
        assert!(tokens.contains(&"name".to_string()));
    }

    // ── 18. Query with only unknown terms ───────────────────────

    #[test]
    fn test_query_no_matching_terms() {
        let mut engine = Bm25Engine::default_params();
        engine.index_document("d1", "alpha beta gamma");

        let results = engine.search("zzznonexistent qqqqmissing", 10);
        assert!(results.is_empty());
    }

    // ── 19. Limit parameter ─────────────────────────────────────

    #[test]
    fn test_search_limit() {
        let mut engine = Bm25Engine::default_params();
        for i in 0..20 {
            engine.index_document(&format!("d{i}"), &format!("keyword document {i}"));
        }

        let results = engine.search("keyword", 5);
        assert_eq!(results.len(), 5);

        let results = engine.search("keyword", 100);
        assert_eq!(results.len(), 20);
    }

    // ── 20. Large corpus persistence roundtrip ──────────────────

    #[test]
    fn test_large_corpus_persistence() {
        let conn = test_db();
        let mut engine = Bm25Engine::default_params();

        // Use distinct content so each document has a unique score
        for i in 0..100 {
            let extra: String = (0..i).map(|j| format!("extra{j}")).collect::<Vec<_>>().join(" ");
            engine.index_document(
                &format!("doc_{i}"),
                &format!("document about topic {} {extra}", i % 7),
            );
        }

        engine.save_to_db(&conn).unwrap();
        let loaded = Bm25Engine::load_from_db(&conn).unwrap().unwrap();
        assert_eq!(loaded.document_count(), 100);

        // Use a specific single-term query to reduce score ties
        let query = "topic";
        let orig = engine.search(query, 100);
        let reloaded = loaded.search(query, 100);

        assert_eq!(orig.len(), reloaded.len());

        // Build score maps and compare — every doc should have the same score
        let orig_map: HashMap<&str, f64> =
            orig.iter().map(|(id, s)| (id.as_str(), *s)).collect();
        let reload_map: HashMap<&str, f64> =
            reloaded.iter().map(|(id, s)| (id.as_str(), *s)).collect();

        assert_eq!(orig_map.len(), reload_map.len());
        for (id, score) in &orig_map {
            let reloaded_score = reload_map.get(id).expect("doc missing after reload");
            assert!(
                (score - reloaded_score).abs() < 1e-10,
                "score mismatch for {id}: {score} vs {reloaded_score}"
            );
        }
    }
}
