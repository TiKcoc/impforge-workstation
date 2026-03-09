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
pub mod engine;
pub mod context;
pub mod nlp;
pub mod llm_extract;
pub mod watch;
pub mod ingest;
pub mod digest;
pub mod tree_sitter_langs;
pub mod commands;
