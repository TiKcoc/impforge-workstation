#![allow(dead_code)]
//! Hybrid Search Pipeline with Reciprocal Rank Fusion (Cormack et al. 2009, SIGIR)
//!
//! Combines vector search (HNSW) for semantic similarity and BM25 for keyword
//! matching, fusing ranked result lists via Reciprocal Rank Fusion (RRF).
//!
//! RRF formula:
//!   RRF_score(d) = SUM_i  w_i / (k + rank_i(d))
//!
//! where:
//!   k   = smoothing constant (default 60, per the original paper)
//!   w_i = weight for retriever i (default 0.5 each)
//!   rank_i(d) = 1-based rank of document d in retriever i's result list
//!
//! The constant k=60 was empirically determined by Cormack, Clarke & Buettcher
//! (2009) to perform robustly across diverse retrieval tasks. It prevents
//! top-ranked documents from dominating the fused score.
//!
//! References:
//!   - Cormack, G. V., Clarke, C. L. A., & Buettcher, S. (2009).
//!     Reciprocal Rank Fusion outperforms Condorcet and individual Rank
//!     Learning Methods. SIGIR 2009.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::bm25::SharedBm25Engine;
use super::vector::SharedHnswIndex;

// ── Search result types ────────────────────────────────────────

/// The type of search that produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    /// Result from vector (HNSW) search only.
    Semantic,
    /// Result from keyword (BM25) search only.
    Keyword,
    /// Result from fused hybrid search (both retrievers contributed).
    Hybrid,
}

/// A single search result with scoring details.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document identifier.
    pub doc_id: String,
    /// Final combined score (RRF score for hybrid, raw score for single-mode).
    pub score: f64,
    /// Semantic similarity score (cosine similarity, 0.0-1.0), if available.
    pub semantic_score: Option<f64>,
    /// BM25 keyword score, if available.
    pub keyword_score: Option<f64>,
    /// Which search mode produced this result.
    pub search_type: SearchType,
}

// ── Hybrid search configuration ────────────────────────────────

/// Configuration for the hybrid search engine.
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// Weight for semantic (vector) results in RRF fusion. Default 0.5.
    pub semantic_weight: f64,
    /// Weight for keyword (BM25) results in RRF fusion. Default 0.5.
    pub keyword_weight: f64,
    /// RRF smoothing constant. Default 60.0 (per Cormack et al. 2009).
    pub rrf_k: f64,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.5,
            keyword_weight: 0.5,
            rrf_k: 60.0,
        }
    }
}

// ── Hybrid search engine ───────────────────────────────────────

/// Hybrid search engine combining HNSW vector search and BM25 keyword search
/// with Reciprocal Rank Fusion (RRF) for score merging.
///
/// Both sub-engines are shared references (`Arc`) so they can be used
/// concurrently by the rest of the application while the hybrid engine
/// fuses their outputs.
pub struct HybridSearchEngine {
    hnsw: Arc<SharedHnswIndex>,
    bm25: Arc<SharedBm25Engine>,
    config: HybridSearchConfig,
}

impl HybridSearchEngine {
    /// Create a new hybrid search engine with default configuration.
    ///
    /// # Arguments
    /// * `hnsw` - Shared HNSW vector index for semantic search
    /// * `bm25` - Shared BM25 engine for keyword search
    pub fn new(hnsw: Arc<SharedHnswIndex>, bm25: Arc<SharedBm25Engine>) -> Self {
        Self {
            hnsw,
            bm25,
            config: HybridSearchConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        hnsw: Arc<SharedHnswIndex>,
        bm25: Arc<SharedBm25Engine>,
        config: HybridSearchConfig,
    ) -> Self {
        Self { hnsw, bm25, config }
    }

    /// Update the fusion weights.
    pub fn set_weights(&mut self, semantic_weight: f64, keyword_weight: f64) {
        self.config.semantic_weight = semantic_weight;
        self.config.keyword_weight = keyword_weight;
    }

    /// Update the RRF constant k.
    pub fn set_rrf_k(&mut self, k: f64) {
        self.config.rrf_k = k;
    }

    /// Get the current configuration.
    pub fn config(&self) -> &HybridSearchConfig {
        &self.config
    }

    // ── Indexing ────────────────────────────────────────────────

    /// Index a document in both the vector and keyword engines.
    ///
    /// # Arguments
    /// * `doc_id`    - Unique document identifier
    /// * `content`   - Text content for BM25 indexing
    /// * `embedding` - Vector embedding for HNSW indexing
    pub fn index_document(&self, doc_id: &str, content: &str, embedding: &[f32]) {
        self.hnsw.insert(doc_id, embedding);
        self.bm25.index_document(doc_id, content);
    }

    /// Remove a document from both engines. Returns `true` if removed
    /// from at least one engine.
    pub fn remove_document(&self, doc_id: &str) -> bool {
        let removed_hnsw = self.hnsw.remove(doc_id);
        let removed_bm25 = self.bm25.remove_document(doc_id);
        removed_hnsw || removed_bm25
    }

    // ── Search operations ──────────────────────────────────────

    /// Perform a hybrid search combining semantic and keyword retrieval
    /// via Reciprocal Rank Fusion.
    ///
    /// Both `query_text` and `query_embedding` are used. If one is empty/absent,
    /// the engine gracefully falls back to the available signal.
    ///
    /// # Arguments
    /// * `query_text`      - Text query for BM25 keyword search
    /// * `query_embedding` - Vector query for HNSW semantic search
    /// * `limit`           - Maximum number of results to return
    pub fn search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Vec<SearchResult> {
        if limit == 0 {
            return Vec::new();
        }

        let has_text = !query_text.is_empty();
        let has_embedding = !query_embedding.is_empty();

        // Fallback: if only one signal is available, use single-mode search
        if !has_text && !has_embedding {
            return Vec::new();
        }
        if !has_text {
            return self.search_semantic(query_embedding, limit);
        }
        if !has_embedding {
            return self.search_keyword(query_text, limit);
        }

        // Fetch candidates from both engines.
        // Request more candidates than the final limit to improve fusion quality.
        let fetch_limit = (limit * 3).max(50);

        let semantic_results = self.hnsw.search(query_embedding, fetch_limit);
        let keyword_results = self.bm25.search(query_text, fetch_limit);

        // If one engine returns nothing, fall back to the other
        if semantic_results.is_empty() && keyword_results.is_empty() {
            return Vec::new();
        }
        if semantic_results.is_empty() {
            return self.to_keyword_results(&keyword_results, limit);
        }
        if keyword_results.is_empty() {
            return self.to_semantic_results(&semantic_results, limit);
        }

        // Apply RRF fusion
        self.fuse_rrf(&semantic_results, &keyword_results, limit)
    }

    /// Perform semantic-only search using the HNSW vector index.
    ///
    /// Returns results ranked by cosine similarity (converted from angular distance).
    pub fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Vec<SearchResult> {
        if query_embedding.is_empty() || limit == 0 {
            return Vec::new();
        }

        let results = self.hnsw.search(query_embedding, limit);
        self.to_semantic_results(&results, limit)
    }

    /// Perform keyword-only search using the BM25 engine.
    ///
    /// Returns results ranked by BM25 score.
    pub fn search_keyword(&self, query_text: &str, limit: usize) -> Vec<SearchResult> {
        if query_text.is_empty() || limit == 0 {
            return Vec::new();
        }

        let results = self.bm25.search(query_text, limit);
        self.to_keyword_results(&results, limit)
    }

    // ── RRF fusion (private) ───────────────────────────────────

    /// Fuse two ranked result lists using Reciprocal Rank Fusion.
    ///
    /// For each document d appearing in either list:
    ///   RRF(d) = w_sem / (k + rank_sem(d))  +  w_kw / (k + rank_kw(d))
    ///
    /// Documents appearing in only one list receive a score contribution
    /// only from that list.
    fn fuse_rrf(
        &self,
        semantic_results: &[(String, f32)],
        keyword_results: &[(String, f64)],
        limit: usize,
    ) -> Vec<SearchResult> {
        let k = self.config.rrf_k;
        let w_sem = self.config.semantic_weight;
        let w_kw = self.config.keyword_weight;

        // Build semantic score map: doc_id -> (rank, cosine_similarity)
        let mut sem_map: HashMap<&str, (usize, f64)> = HashMap::new();
        for (rank, (doc_id, distance)) in semantic_results.iter().enumerate() {
            // Convert angular distance [0, 2] to similarity [0, 1]:
            //   similarity = 1.0 - distance
            //   Clamp to [0, 1] for safety.
            let similarity = (1.0 - *distance as f64).clamp(0.0, 1.0);
            sem_map.insert(doc_id.as_str(), (rank + 1, similarity)); // 1-based rank
        }

        // Build keyword score map: doc_id -> (rank, bm25_score)
        let mut kw_map: HashMap<&str, (usize, f64)> = HashMap::new();
        for (rank, (doc_id, score)) in keyword_results.iter().enumerate() {
            kw_map.insert(doc_id.as_str(), (rank + 1, *score)); // 1-based rank
        }

        // Collect all unique document IDs (preserving discovery order)
        let mut all_doc_ids: Vec<&str> = Vec::new();
        {
            let mut seen: HashSet<&str> = HashSet::new();
            for (doc_id, _) in semantic_results {
                if seen.insert(doc_id.as_str()) {
                    all_doc_ids.push(doc_id.as_str());
                }
            }
            for (doc_id, _) in keyword_results {
                if seen.insert(doc_id.as_str()) {
                    all_doc_ids.push(doc_id.as_str());
                }
            }
        }

        // Compute RRF scores
        let mut fused: Vec<SearchResult> = all_doc_ids
            .into_iter()
            .map(|doc_id| {
                let mut rrf_score = 0.0;
                let mut semantic_score = None;
                let mut keyword_score = None;

                if let Some(&(rank, sim)) = sem_map.get(doc_id) {
                    rrf_score += w_sem / (k + rank as f64);
                    semantic_score = Some(sim);
                }

                if let Some(&(rank, bm25)) = kw_map.get(doc_id) {
                    rrf_score += w_kw / (k + rank as f64);
                    keyword_score = Some(bm25);
                }

                let search_type = match (semantic_score.is_some(), keyword_score.is_some()) {
                    (true, true) => SearchType::Hybrid,
                    (true, false) => SearchType::Semantic,
                    (false, true) => SearchType::Keyword,
                    (false, false) => unreachable!(),
                };

                SearchResult {
                    doc_id: doc_id.to_string(),
                    score: rrf_score,
                    semantic_score,
                    keyword_score,
                    search_type,
                }
            })
            .collect();

        // Sort by RRF score descending (stable sort preserves order for ties)
        fused.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        fused.truncate(limit);
        fused
    }

    // ── Result conversion helpers (private) ────────────────────

    /// Convert HNSW results (doc_id, angular_distance) to SearchResult.
    fn to_semantic_results(
        &self,
        results: &[(String, f32)],
        limit: usize,
    ) -> Vec<SearchResult> {
        results
            .iter()
            .take(limit)
            .map(|(doc_id, distance)| {
                let similarity = (1.0 - *distance as f64).clamp(0.0, 1.0);
                SearchResult {
                    doc_id: doc_id.clone(),
                    score: similarity,
                    semantic_score: Some(similarity),
                    keyword_score: None,
                    search_type: SearchType::Semantic,
                }
            })
            .collect()
    }

    /// Convert BM25 results (doc_id, score) to SearchResult.
    fn to_keyword_results(
        &self,
        results: &[(String, f64)],
        limit: usize,
    ) -> Vec<SearchResult> {
        results
            .iter()
            .take(limit)
            .map(|(doc_id, bm25_score)| SearchResult {
                doc_id: doc_id.clone(),
                score: *bm25_score,
                semantic_score: None,
                keyword_score: Some(*bm25_score),
                search_type: SearchType::Keyword,
            })
            .collect()
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_memory::bm25::SharedBm25Engine;
    use crate::forge_memory::vector::{HnswConfig, SharedHnswIndex};

    /// Helper: create a test HNSW index and BM25 engine pair.
    fn test_engines() -> (Arc<SharedHnswIndex>, Arc<SharedBm25Engine>) {
        let hnsw = Arc::new(SharedHnswIndex::new(HnswConfig::default()));
        let bm25 = Arc::new(SharedBm25Engine::new());
        (hnsw, bm25)
    }

    /// Helper: create a simple 4-dimensional embedding from a seed.
    /// Produces a deterministic normalized vector for test reproducibility.
    fn make_embedding(seed: u32) -> Vec<f32> {
        let s = seed as f32;
        let v = vec![
            (s * 0.1).sin(),
            (s * 0.2).cos(),
            (s * 0.3).sin(),
            (s * 0.4).cos(),
        ];
        // Normalize to unit length
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < f32::EPSILON {
            return vec![1.0, 0.0, 0.0, 0.0];
        }
        v.into_iter().map(|x| x / norm).collect()
    }

    // ── 1. Hybrid search returns fused results ──────────────────

    #[test]
    fn test_hybrid_search_returns_fused_results() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb1 = make_embedding(1);
        let emb2 = make_embedding(2);
        let emb3 = make_embedding(3);

        engine.index_document("doc1", "rust programming language", &emb1);
        engine.index_document("doc2", "python scripting language", &emb2);
        engine.index_document("doc3", "rust systems programming", &emb3);

        let query_emb = make_embedding(1); // Close to doc1
        let results = engine.search("rust programming", &query_emb, 10);

        assert!(!results.is_empty(), "Hybrid search should return results");
        // At least some results should be Hybrid type (found in both engines)
        let hybrid_count = results
            .iter()
            .filter(|r| r.search_type == SearchType::Hybrid)
            .count();
        assert!(
            hybrid_count > 0,
            "Should have at least one Hybrid result, got {hybrid_count}"
        );
    }

    // ── 2. RRF scoring correctness (manual calculation) ─────────

    #[test]
    fn test_rrf_scoring_correctness() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        // Create embeddings that produce a known ranking order.
        let query_emb = vec![1.0, 0.0, 0.0, 0.0];
        let emb1 = vec![0.99, 0.01, 0.0, 0.0]; // Very close to query
        let emb2 = vec![0.5, 0.5, 0.5, 0.5]; // Medium distance
        let emb3 = vec![0.0, 0.0, 0.0, 1.0]; // Far from query

        engine.index_document("alpha", "rust programming fast systems", &emb1);
        engine.index_document("beta", "rust web framework", &emb2);
        engine.index_document("gamma", "python data science", &emb3);

        let results = engine.search("rust programming", &query_emb, 10);

        // All scores should be positive
        for r in &results {
            assert!(r.score > 0.0, "RRF score should be positive for {}", r.doc_id);
        }

        // "alpha" should rank first -- it matches both "rust programming" (BM25)
        // and has the closest embedding (semantic)
        assert_eq!(
            results[0].doc_id, "alpha",
            "alpha should rank first in hybrid search"
        );

        // Verify the score is in the expected RRF range
        // Max possible RRF = 0.5/(60+1) + 0.5/(60+1) = 1/61 ~ 0.01639
        let max_rrf = 1.0 / (60.0 + 1.0);
        assert!(
            results[0].score <= max_rrf + 1e-10,
            "Score {} should be <= max RRF {}",
            results[0].score,
            max_rrf
        );
    }

    // ── 3. Semantic-only search works ───────────────────────────

    #[test]
    fn test_semantic_only_search() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb1 = vec![1.0, 0.0, 0.0, 0.0];
        let emb2 = vec![0.0, 1.0, 0.0, 0.0];

        engine.index_document("doc1", "hello world", &emb1);
        engine.index_document("doc2", "goodbye world", &emb2);

        let query_emb = vec![0.9, 0.1, 0.0, 0.0]; // Close to doc1
        let results = engine.search_semantic(&query_emb, 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "doc1");
        assert_eq!(results[0].search_type, SearchType::Semantic);
        assert!(results[0].semantic_score.is_some());
        assert!(results[0].keyword_score.is_none());
    }

    // ── 4. Keyword-only search works ────────────────────────────

    #[test]
    fn test_keyword_only_search() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb1 = make_embedding(10);
        let emb2 = make_embedding(20);

        engine.index_document("doc1", "rust async programming tokio", &emb1);
        engine.index_document("doc2", "python machine learning scipy", &emb2);

        let results = engine.search_keyword("rust async tokio", 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "doc1");
        assert_eq!(results[0].search_type, SearchType::Keyword);
        assert!(results[0].keyword_score.is_some());
        assert!(results[0].semantic_score.is_none());
    }

    // ── 5. Document indexing in both engines ────────────────────

    #[test]
    fn test_document_indexed_in_both_engines() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(Arc::clone(&hnsw), Arc::clone(&bm25));

        engine.index_document("d1", "test content alpha", &make_embedding(1));
        engine.index_document("d2", "test content beta", &make_embedding(2));

        // Verify HNSW has the documents
        assert_eq!(hnsw.len(), 2);

        // Verify BM25 has the documents
        assert_eq!(bm25.document_count(), 2);

        // Both engines should return results for their respective queries
        let sem_results = hnsw.search(&make_embedding(1), 10);
        assert!(!sem_results.is_empty());

        let kw_results = bm25.search("alpha", 10);
        assert_eq!(kw_results.len(), 1);
        assert_eq!(kw_results[0].0, "d1");
    }

    // ── 6. Remove document from both engines ────────────────────

    #[test]
    fn test_remove_document_from_both() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(Arc::clone(&hnsw), Arc::clone(&bm25));

        engine.index_document("d1", "remove me please", &make_embedding(1));
        engine.index_document("d2", "keep me around", &make_embedding(2));

        assert_eq!(bm25.document_count(), 2);

        let removed = engine.remove_document("d1");
        assert!(removed, "Should return true when document exists");

        // BM25 physically removes
        assert_eq!(bm25.document_count(), 1);

        // Search should not return removed document
        let results = engine.search_keyword("remove", 10);
        assert!(
            results.is_empty(),
            "Removed document should not appear in keyword results"
        );
    }

    // ── 7. Empty results handling ───────────────────────────────

    #[test]
    fn test_empty_results_handling() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        // Search on empty index
        let results = engine.search("anything", &make_embedding(1), 10);
        assert!(results.is_empty());

        let results = engine.search_semantic(&make_embedding(1), 10);
        assert!(results.is_empty());

        let results = engine.search_keyword("anything", 10);
        assert!(results.is_empty());

        // Empty query
        let results = engine.search("", &[], 10);
        assert!(results.is_empty());

        // Zero limit
        let results = engine.search("test", &make_embedding(1), 0);
        assert!(results.is_empty());
    }

    // ── 8. Custom weights affect ranking ────────────────────────

    #[test]
    fn test_custom_weights_affect_ranking() {
        let (hnsw1, bm25_1) = test_engines();
        let (hnsw2, bm25_2) = test_engines();

        // Semantic-heavy config
        let engine_sem = HybridSearchEngine::with_config(
            Arc::clone(&hnsw1),
            Arc::clone(&bm25_1),
            HybridSearchConfig {
                semantic_weight: 0.9,
                keyword_weight: 0.1,
                rrf_k: 60.0,
            },
        );

        // Keyword-heavy config
        let engine_kw = HybridSearchEngine::with_config(
            Arc::clone(&hnsw2),
            Arc::clone(&bm25_2),
            HybridSearchConfig {
                semantic_weight: 0.1,
                keyword_weight: 0.9,
                rrf_k: 60.0,
            },
        );

        // doc_a: semantically close to query, weak keyword match
        // doc_b: semantically far from query, strong keyword match
        let query_emb = vec![1.0, 0.0, 0.0, 0.0];
        let emb_close = vec![0.95, 0.05, 0.0, 0.0]; // Very close semantically
        let emb_far = vec![0.0, 0.0, 1.0, 0.0]; // Far semantically

        // Index same docs in both engine pairs
        for (h, b) in [
            (Arc::clone(&hnsw1), Arc::clone(&bm25_1)),
            (Arc::clone(&hnsw2), Arc::clone(&bm25_2)),
        ] {
            h.insert("doc_a", &emb_close);
            b.index_document("doc_a", "general text");
            h.insert("doc_b", &emb_far);
            b.index_document("doc_b", "rust programming systems rust language");
        }

        let results_sem = engine_sem.search("rust programming", &query_emb, 10);
        let results_kw = engine_kw.search("rust programming", &query_emb, 10);

        // With semantic-heavy weights, doc_a (close embedding) should score higher
        // relative to doc_b compared to keyword-heavy weights
        if results_sem.len() >= 2 && results_kw.len() >= 2 {
            let sem_a = results_sem.iter().find(|r| r.doc_id == "doc_a").map(|r| r.score);
            let sem_b = results_sem.iter().find(|r| r.doc_id == "doc_b").map(|r| r.score);
            let kw_a = results_kw.iter().find(|r| r.doc_id == "doc_a").map(|r| r.score);
            let kw_b = results_kw.iter().find(|r| r.doc_id == "doc_b").map(|r| r.score);

            if let (Some(sa), Some(sb), Some(ka), Some(kb)) = (sem_a, sem_b, kw_a, kw_b) {
                // Ratio of doc_a/doc_b should be higher with semantic-heavy weights
                let ratio_sem = sa / sb;
                let ratio_kw = ka / kb;
                // Both configurations should produce valid positive scores
                assert!(sa > 0.0 && sb > 0.0 && ka > 0.0 && kb > 0.0);
                // The ratios should differ because weights differ
                assert!(
                    (ratio_sem - ratio_kw).abs() > 1e-10,
                    "Different weights should produce different relative rankings: \
                     sem_ratio={ratio_sem:.6}, kw_ratio={ratio_kw:.6}"
                );
            }
        }
    }

    // ── 9. Results are properly deduplicated in fusion ───────────

    #[test]
    fn test_results_deduplicated_in_fusion() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb = make_embedding(42);
        engine.index_document("unique_doc", "unique content test", &emb);

        let results = engine.search("unique content", &emb, 10);

        // The document should appear exactly once, not duplicated
        let count = results
            .iter()
            .filter(|r| r.doc_id == "unique_doc")
            .count();
        assert_eq!(
            count, 1,
            "Document should appear exactly once in fused results, got {count}"
        );

        // And it should be marked as Hybrid (found in both)
        let result = results.iter().find(|r| r.doc_id == "unique_doc").unwrap();
        assert_eq!(result.search_type, SearchType::Hybrid);
    }

    // ── 10. Results sorted by score descending ──────────────────

    #[test]
    fn test_results_sorted_by_score_descending() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        for i in 0..10u32 {
            let emb = make_embedding(i);
            engine.index_document(
                &format!("doc_{i}"),
                &format!("document number {i} about testing search"),
                &emb,
            );
        }

        let query_emb = make_embedding(0);
        let results = engine.search("testing search", &query_emb, 10);

        for i in 1..results.len() {
            assert!(
                results[i].score <= results[i - 1].score + 1e-12,
                "Results must be sorted descending by score: \
                 results[{}].score={} > results[{}].score={}",
                i,
                results[i].score,
                i - 1,
                results[i - 1].score
            );
        }
    }

    // ── 11. Limit parameter respected ───────────────────────────

    #[test]
    fn test_limit_parameter_respected() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        for i in 0..20u32 {
            let emb = make_embedding(i);
            engine.index_document(
                &format!("doc_{i}"),
                &format!("common keyword content {i}"),
                &emb,
            );
        }

        let query_emb = make_embedding(0);

        let results_5 = engine.search("common keyword", &query_emb, 5);
        assert!(
            results_5.len() <= 5,
            "Should return at most 5 results, got {}",
            results_5.len()
        );

        let results_3 = engine.search("common keyword", &query_emb, 3);
        assert!(
            results_3.len() <= 3,
            "Should return at most 3 results, got {}",
            results_3.len()
        );

        let results_1 = engine.search("common keyword", &query_emb, 1);
        assert!(
            results_1.len() <= 1,
            "Should return at most 1 result, got {}",
            results_1.len()
        );
    }

    // ── 12. Search with no embedding (keyword fallback) ─────────

    #[test]
    fn test_search_no_embedding_keyword_fallback() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        engine.index_document("doc1", "rust async systems", &make_embedding(1));
        engine.index_document("doc2", "python web framework", &make_embedding(2));

        // Empty embedding -> should fall back to keyword-only
        let results = engine.search("rust async", &[], 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "doc1");
        assert_eq!(results[0].search_type, SearchType::Keyword);
        assert!(results[0].keyword_score.is_some());
        assert!(results[0].semantic_score.is_none());
    }

    // ── 13. Search with no text (semantic fallback) ─────────────

    #[test]
    fn test_search_no_text_semantic_fallback() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb1 = vec![1.0, 0.0, 0.0, 0.0];
        let emb2 = vec![0.0, 1.0, 0.0, 0.0];

        engine.index_document("doc1", "some text", &emb1);
        engine.index_document("doc2", "other text", &emb2);

        // Empty text -> should fall back to semantic-only
        let query_emb = vec![0.9, 0.1, 0.0, 0.0];
        let results = engine.search("", &query_emb, 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "doc1");
        assert_eq!(results[0].search_type, SearchType::Semantic);
        assert!(results[0].semantic_score.is_some());
        assert!(results[0].keyword_score.is_none());
    }

    // ── 14. Large result set fusion (50+ docs) ──────────────────

    #[test]
    fn test_large_result_set_fusion() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        // Index 60 documents
        for i in 0..60u32 {
            let emb = make_embedding(i);
            let topic = match i % 3 {
                0 => "machine learning algorithms",
                1 => "database systems optimization",
                _ => "network security protocols",
            };
            engine.index_document(
                &format!("doc_{i}"),
                &format!("{topic} document {i}"),
                &emb,
            );
        }

        let query_emb = make_embedding(0);
        let results = engine.search("machine learning", &query_emb, 20);

        assert!(
            !results.is_empty(),
            "Large corpus hybrid search should return results"
        );
        assert!(
            results.len() <= 20,
            "Should respect limit of 20, got {}",
            results.len()
        );

        // All doc IDs should be unique
        let unique_ids: HashSet<&str> =
            results.iter().map(|r| r.doc_id.as_str()).collect();
        assert_eq!(
            unique_ids.len(),
            results.len(),
            "All results should have unique doc IDs"
        );

        // Scores should be sorted descending
        for i in 1..results.len() {
            assert!(results[i].score <= results[i - 1].score + 1e-12);
        }
    }

    // ── 15. Score normalization between engines ─────────────────

    #[test]
    fn test_score_normalization() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb = vec![1.0, 0.0, 0.0, 0.0];
        engine.index_document("doc1", "test normalization scoring", &emb);

        let results = engine.search("test normalization", &emb, 10);
        assert!(!results.is_empty());

        let r = &results[0];
        // Semantic score should be in [0, 1] (cosine similarity)
        if let Some(sem) = r.semantic_score {
            assert!(
                (0.0..=1.0).contains(&sem),
                "Semantic score should be in [0,1], got {sem}"
            );
        }

        // Keyword score should be non-negative
        if let Some(kw) = r.keyword_score {
            assert!(kw >= 0.0, "Keyword score should be non-negative, got {kw}");
        }

        // RRF score should be positive and bounded
        assert!(r.score > 0.0, "RRF score should be positive");
        // Max RRF for rank-1 in both with w=0.5 each: 0.5/61 + 0.5/61 = 1/61
        assert!(
            r.score <= 1.0 / 61.0 + 1e-10,
            "RRF score {} exceeds theoretical max {}",
            r.score,
            1.0 / 61.0
        );
    }

    // ── 16. Hybrid result has both scores populated ─────────────

    #[test]
    fn test_hybrid_result_has_both_scores() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let emb = make_embedding(7);
        engine.index_document("doc1", "alpha beta gamma delta", &emb);

        let results = engine.search("alpha beta", &emb, 10);
        assert!(!results.is_empty());

        // The single doc should be found by both engines
        let r = &results[0];
        assert_eq!(r.search_type, SearchType::Hybrid);
        assert!(
            r.semantic_score.is_some(),
            "Hybrid result should have semantic score"
        );
        assert!(
            r.keyword_score.is_some(),
            "Hybrid result should have keyword score"
        );
    }

    // ── 17. Set weights dynamically ─────────────────────────────

    #[test]
    fn test_set_weights_dynamically() {
        let (hnsw, bm25) = test_engines();
        let mut engine = HybridSearchEngine::new(hnsw, bm25);

        assert!((engine.config().semantic_weight - 0.5).abs() < f64::EPSILON);
        assert!((engine.config().keyword_weight - 0.5).abs() < f64::EPSILON);

        engine.set_weights(0.8, 0.2);
        assert!((engine.config().semantic_weight - 0.8).abs() < f64::EPSILON);
        assert!((engine.config().keyword_weight - 0.2).abs() < f64::EPSILON);

        engine.set_rrf_k(30.0);
        assert!((engine.config().rrf_k - 30.0).abs() < f64::EPSILON);
    }

    // ── 18. Remove nonexistent document returns false ────────────

    #[test]
    fn test_remove_nonexistent_document() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        let removed = engine.remove_document("does_not_exist");
        assert!(!removed, "Removing nonexistent doc should return false");
    }

    // ── 19. RRF k constant affects score magnitude ──────────────

    #[test]
    fn test_rrf_k_affects_score_magnitude() {
        let (hnsw1, bm25_1) = test_engines();
        let (hnsw2, bm25_2) = test_engines();

        let engine_k10 = HybridSearchEngine::with_config(
            Arc::clone(&hnsw1),
            Arc::clone(&bm25_1),
            HybridSearchConfig {
                semantic_weight: 0.5,
                keyword_weight: 0.5,
                rrf_k: 10.0, // Small k -> higher scores
            },
        );

        let engine_k100 = HybridSearchEngine::with_config(
            Arc::clone(&hnsw2),
            Arc::clone(&bm25_2),
            HybridSearchConfig {
                semantic_weight: 0.5,
                keyword_weight: 0.5,
                rrf_k: 100.0, // Large k -> lower scores
            },
        );

        let emb = make_embedding(5);
        // Index same doc in both
        for (h, b) in [
            (Arc::clone(&hnsw1), Arc::clone(&bm25_1)),
            (Arc::clone(&hnsw2), Arc::clone(&bm25_2)),
        ] {
            h.insert("doc1", &emb);
            b.index_document("doc1", "test document content");
        }

        let results_k10 = engine_k10.search("test document", &emb, 10);
        let results_k100 = engine_k100.search("test document", &emb, 10);

        assert!(!results_k10.is_empty());
        assert!(!results_k100.is_empty());

        // With smaller k, scores should be higher (1/(10+1) > 1/(100+1))
        assert!(
            results_k10[0].score > results_k100[0].score,
            "Smaller k should produce higher RRF scores: k10={}, k100={}",
            results_k10[0].score,
            results_k100[0].score
        );
    }

    // ── 20. Multiple documents with varying relevance ───────────

    #[test]
    fn test_varying_relevance_ranking() {
        let (hnsw, bm25) = test_engines();
        let engine = HybridSearchEngine::new(hnsw, bm25);

        // doc_a: strong semantic match, strong keyword match
        let query_emb = vec![1.0, 0.0, 0.0, 0.0];
        engine.index_document(
            "doc_a",
            "rust systems programming performance memory safety",
            &[0.98, 0.02, 0.0, 0.0],
        );
        // doc_b: weak semantic match, strong keyword match
        engine.index_document(
            "doc_b",
            "rust systems programming language design",
            &[0.0, 1.0, 0.0, 0.0],
        );
        // doc_c: strong semantic match, no keyword match
        engine.index_document(
            "doc_c",
            "completely unrelated topic about cooking recipes",
            &[0.97, 0.03, 0.0, 0.0],
        );
        // doc_d: no match at all
        engine.index_document(
            "doc_d",
            "gardening tips for spring flowers",
            &[0.0, 0.0, 1.0, 0.0],
        );

        let results = engine.search("rust systems programming", &query_emb, 10);

        assert!(results.len() >= 2);

        // doc_a should rank first (matches both strongly)
        assert_eq!(
            results[0].doc_id, "doc_a",
            "doc_a should rank first (strong in both dimensions)"
        );
    }
}
