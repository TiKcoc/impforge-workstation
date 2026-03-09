//! Hybrid Search Pipeline — BM25 + Vector + RRF Fusion
//!
//! Reciprocal Rank Fusion (Cormack et al. 2009)
//! Pipeline: Query → [BM25 top-50] + [HNSW top-50] → RRF merge → top-K
