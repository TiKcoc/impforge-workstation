//! MemGPT-inspired Tiered Memory (Packer et al. 2023)
//!
//! Core Memory: ≤100 items, always loaded, highest importance
//! Recall Memory: Unlimited, searchable via hybrid search
//! Archival Memory: Compressed, FSRS-5 scheduled reviews
