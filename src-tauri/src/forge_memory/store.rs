//! SQLite persistence layer for ForgeMemory
//!
//! Pattern: Same as orchestrator/store.rs — WAL mode, bundled SQLite,
//! parking_lot::Mutex for concurrent access, auto-migrations on startup.
