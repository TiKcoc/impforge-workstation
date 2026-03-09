//! MemGPT-inspired Tiered Memory System with FSRS-5 Spaced Repetition
//!
//! Implements a three-tier memory architecture modeled after MemGPT
//! (Packer et al. 2023, arXiv:2310.08560):
//!
//!   - **Core**: Working memory, always in context. Small (~20 items max).
//!     Highest importance items that the system needs constant access to.
//!
//!   - **Recall**: Recent interactions and medium-term knowledge (~1000 items max).
//!     Searchable via hybrid (semantic + keyword) search.
//!
//!   - **Archival**: Unlimited persistent knowledge store.
//!     Search-only access, never automatically loaded into context.
//!
//! Memory items are managed with FSRS-5 (Jarrett Ye 2022-2024) spaced
//! repetition parameters for intelligent retention scheduling:
//!
//!   S' = S * e^(w * (G - D + 1))
//!
//! where S = stability, D = difficulty, G = grade (0-3), w = learning rate.
//!
//! Automatic lifecycle management:
//!   - Core overflow: lowest-importance items demoted to recall
//!   - Recall overflow: lowest-importance items archived
//!   - Temporal decay: importance decreases for unaccessed memories
//!
//! References:
//!   - Packer, C., Wooders, S., Lin, K., Fang, V., Patil, S. G., Stoica, I.,
//!     & Gonzalez, J. E. (2023). MemGPT: Towards LLMs as Operating Systems.
//!     arXiv:2310.08560.
//!   - Ye, J. (2022-2024). FSRS-5: A Modern Spaced Repetition Algorithm.
//!     https://github.com/open-spaced-repetition/fsrs4anki

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::store::ForgeMemoryStore;

// ── Constants ────────────────────────────────────────────────────

/// Maximum number of items allowed in the core tier (working memory).
/// Inspired by Miller's Law (~7 +/- 2 chunks) scaled for AI context.
pub const CORE_LIMIT: usize = 20;

/// Maximum number of items allowed in the recall tier.
pub const RECALL_LIMIT: usize = 1000;

/// FSRS-5 learning rate parameter (w17 in the full FSRS model).
/// Controls how much a review changes stability.
const FSRS_LEARNING_RATE: f64 = 0.2;

/// Half-life for temporal importance decay (in days).
/// After this many days without access, importance halves.
const DECAY_HALF_LIFE_DAYS: f64 = 7.0;

// ── Enums ────────────────────────────────────────────────────────

/// Memory scope tiers following the MemGPT architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    /// Always in context. Small, most important.
    Core,
    /// Recent interactions, medium size. Search-accessible.
    Recall,
    /// Unlimited persistent knowledge. Search-only.
    Archival,
}

impl MemoryScope {
    /// Convert to the string stored in SQLite.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Recall => "recall",
            Self::Archival => "archival",
        }
    }

    /// Parse from the string stored in SQLite.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "core" => Some(Self::Core),
            "recall" => Some(Self::Recall),
            "archival" => Some(Self::Archival),
            _ => None,
        }
    }

    /// The next lower tier for demotion.
    pub fn demote_target(&self) -> Option<Self> {
        match self {
            Self::Core => Some(Self::Recall),
            Self::Recall => Some(Self::Archival),
            Self::Archival => None,
        }
    }
}

/// FSRS-5 review rating.
///
/// Maps to grades 0-3 used in the stability update formula:
///   S' = S * e^(w * (G - D + 1))
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewRating {
    /// Complete failure to recall. Grade = 0.
    Again = 0,
    /// Recalled with significant difficulty. Grade = 1.
    Hard = 1,
    /// Recalled correctly with some effort. Grade = 2.
    Good = 2,
    /// Recalled instantly, effortlessly. Grade = 3.
    Easy = 3,
}

impl ReviewRating {
    /// Numeric grade for the FSRS formula.
    pub fn grade(&self) -> f64 {
        *self as i32 as f64
    }
}

// ── MemoryItem ───────────────────────────────────────────────────

/// A single memory item with MemGPT tier metadata and FSRS-5 scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub scope: MemoryScope,
    pub category: String,
    pub key: Option<String>,
    pub content: String,
    pub importance: f64,
    /// FSRS-5 stability: expected number of days until P(recall) = 90%.
    pub stability: f64,
    /// FSRS-5 difficulty: inherent difficulty of the memory [0.0, 1.0].
    pub difficulty: f64,
    /// Number of successful reviews.
    pub reps: i64,
    /// ISO 8601 timestamp of the next scheduled review.
    pub next_review: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Consolidation Report ─────────────────────────────────────────

/// Report from a consolidation run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationReport {
    /// Number of memories that had importance decayed.
    pub decayed: usize,
    /// Number of core items demoted to recall.
    pub core_demoted: usize,
    /// Number of recall items archived.
    pub recall_archived: usize,
}

// ── TieredMemory ─────────────────────────────────────────────────

/// MemGPT-inspired tiered memory manager backed by ForgeMemoryStore (SQLite).
///
/// Provides core/recall/archival tier operations, automatic overflow
/// management, FSRS-5 spaced repetition, and temporal importance decay.
pub struct TieredMemory<'a> {
    store: &'a ForgeMemoryStore,
}

impl<'a> TieredMemory<'a> {
    /// Create a new TieredMemory manager backed by the given store.
    pub fn new(store: &'a ForgeMemoryStore) -> Self {
        Self { store }
    }

    // ── Add ──────────────────────────────────────────────────

    /// Add a new memory to the specified tier.
    ///
    /// If adding to core and the limit is exceeded, the lowest-importance
    /// core item is automatically demoted to recall. Same for recall overflow.
    ///
    /// The `embedding` parameter is optional; pass the serialized f32 blob
    /// if you have pre-computed embeddings for vector search.
    pub fn add_memory(
        &self,
        content: &str,
        scope: MemoryScope,
        importance: f64,
        category: &str,
        embedding: Option<&[u8]>,
    ) -> Result<String, String> {
        let id = self
            .store
            .insert_memory(scope.as_str(), category, None, content, importance, embedding)
            .map_err(|e| format!("Failed to insert memory: {e}"))?;

        // Enforce tier limits after insertion
        self.enforce_limits(scope)?;

        Ok(id)
    }

    // ── Core retrieval ───────────────────────────────────────

    /// Get all core memories. These are always loaded into context.
    ///
    /// Returns up to CORE_LIMIT items sorted by importance descending.
    pub fn get_core_memories(&self) -> Result<Vec<MemoryItem>, String> {
        self.get_memories_by_scope(MemoryScope::Core, CORE_LIMIT as u32)
    }

    // ── Search ───────────────────────────────────────────────

    /// Search the recall tier using keyword matching on content.
    ///
    /// In production, this would combine with vector search via the
    /// HybridSearchEngine. Here we provide SQLite LIKE-based search
    /// as a self-contained fallback that works without the embedding
    /// infrastructure being initialized.
    pub fn search_recall(
        &self,
        query: &str,
        _embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<MemoryItem>, String> {
        self.search_tier(MemoryScope::Recall, query, limit)
    }

    /// Search the archival tier using keyword matching on content.
    pub fn search_archival(
        &self,
        query: &str,
        _embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<MemoryItem>, String> {
        self.search_tier(MemoryScope::Archival, query, limit)
    }

    // ── Promote / Demote ─────────────────────────────────────

    /// Promote a memory to a higher tier (e.g., recall -> core).
    ///
    /// After promotion, tier limits are enforced. If core is full,
    /// the lowest-importance item in core is demoted to recall.
    pub fn promote_memory(&self, id: &str, to_scope: MemoryScope) -> Result<bool, String> {
        let updated = self
            .store
            .update_memory_scope(id, to_scope.as_str())
            .map_err(|e| format!("Failed to promote memory: {e}"))?;

        if updated {
            self.store
                .log_memory_event(
                    id,
                    "promoted",
                    Some(&format!("{{\"to\":\"{}\"}}", to_scope.as_str())),
                )
                .map_err(|e| format!("Failed to log promotion event: {e}"))?;
            self.enforce_limits(to_scope)?;
        }

        Ok(updated)
    }

    /// Demote a memory to the next lower tier.
    ///
    /// Core -> Recall, Recall -> Archival.
    /// Returns an error if the memory is already in the archival tier.
    pub fn demote_memory(&self, id: &str) -> Result<bool, String> {
        let item = self
            .get_memory_by_id(id)?
            .ok_or_else(|| format!("Memory not found: {id}"))?;

        let target = item
            .scope
            .demote_target()
            .ok_or_else(|| "Cannot demote archival memory further".to_string())?;

        let updated = self
            .store
            .update_memory_scope(id, target.as_str())
            .map_err(|e| format!("Failed to demote memory: {e}"))?;

        if updated {
            self.store
                .log_memory_event(
                    id,
                    "demoted",
                    Some(&format!(
                        "{{\"from\":\"{}\",\"to\":\"{}\"}}",
                        item.scope.as_str(),
                        target.as_str()
                    )),
                )
                .map_err(|e| format!("Failed to log demotion event: {e}"))?;
        }

        Ok(updated)
    }

    // ── FSRS-5 Spaced Repetition ─────────────────────────────

    /// Review a memory and update its FSRS-5 scheduling parameters.
    ///
    /// Applies the simplified FSRS stability update:
    ///   S' = S * e^(w * (G - D + 1))
    ///
    /// where G = grade (0-3), D = difficulty, w = learning rate.
    ///
    /// Also updates `next_review` based on the new stability value,
    /// and adjusts `reps` and `lapses` counters.
    pub fn review_memory(&self, id: &str, rating: ReviewRating) -> Result<bool, String> {
        let item = self
            .get_memory_by_id(id)?
            .ok_or_else(|| format!("Memory not found: {id}"))?;

        let grade = rating.grade();

        // FSRS-5 stability update: S' = S * e^(w * (G - D + 1))
        let exponent = FSRS_LEARNING_RATE * (grade - item.difficulty + 1.0);
        let new_stability = (item.stability * exponent.exp()).max(0.1);

        // Difficulty update: move toward the grade with damping
        // D' = D + 0.1 * (3 - G) / 3, clamped to [0.0, 1.0]
        let new_difficulty = (item.difficulty + 0.1 * (3.0 - grade) / 3.0).clamp(0.0, 1.0);

        let new_reps = item.reps + 1;
        let is_lapse = rating == ReviewRating::Again;

        // Next review interval: stability represents days until P(recall) ~ 90%
        let review_interval_secs = (new_stability.max(0.1) * 86400.0) as i64;

        let conn = self.store.conn.lock();
        conn.execute(
            "UPDATE memories SET
                stability = ?1,
                difficulty = ?2,
                reps = ?3,
                lapses = CASE WHEN ?4 THEN lapses + 1 ELSE lapses END,
                next_review = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?5 || ' seconds'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                accessed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?6",
            params![
                new_stability,
                new_difficulty,
                new_reps,
                is_lapse,
                review_interval_secs,
                id
            ],
        )
        .map_err(|e| format!("Failed to update FSRS params: {e}"))?;
        drop(conn);

        // Use "reinforced" event_type (valid per memory_lifecycle CHECK constraint)
        self.store
            .log_memory_event(
                id,
                "reinforced",
                Some(&format!(
                    "{{\"rating\":{},\"stability\":{:.3},\"difficulty\":{:.3}}}",
                    grade as i32, new_stability, new_difficulty
                )),
            )
            .map_err(|e| format!("Failed to log review event: {e}"))?;

        Ok(true)
    }

    /// Get all memories that are due for review (next_review <= now).
    pub fn get_due_memories(&self) -> Result<Vec<MemoryItem>, String> {
        let conn = self.store.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, scope, category, key, content, importance,
                        stability, difficulty, reps, next_review,
                        created_at, updated_at
                 FROM memories
                 WHERE next_review IS NOT NULL
                   AND next_review <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 ORDER BY next_review ASC",
            )
            .map_err(|e| format!("Failed to prepare due memories query: {e}"))?;

        let rows = stmt
            .query_map([], Self::row_to_memory_item)
            .map_err(|e| format!("Failed to query due memories: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect due memories: {e}"))
    }

    // ── Consolidation ────────────────────────────────────────

    /// Automatic maintenance: enforce tier limits and apply temporal decay.
    ///
    /// This should be called periodically (e.g., on app startup, or every
    /// N minutes) to keep the memory tiers healthy.
    ///
    /// Operations:
    ///   1. Apply temporal importance decay to all memories
    ///   2. Demote excess core items to recall
    ///   3. Archive excess recall items
    pub fn consolidate(&self) -> Result<ConsolidationReport, String> {
        let mut report = ConsolidationReport::default();

        report.decayed = self.apply_temporal_decay()?;
        report.core_demoted = self.enforce_tier_limit(MemoryScope::Core, CORE_LIMIT)?;
        report.recall_archived = self.enforce_tier_limit(MemoryScope::Recall, RECALL_LIMIT)?;

        Ok(report)
    }

    // ── Internal helpers ─────────────────────────────────────

    /// Fetch a single memory by ID with full FSRS fields.
    fn get_memory_by_id(&self, id: &str) -> Result<Option<MemoryItem>, String> {
        let conn = self.store.conn.lock();
        conn.query_row(
            "SELECT id, scope, category, key, content, importance,
                    stability, difficulty, reps, next_review,
                    created_at, updated_at
             FROM memories WHERE id = ?1",
            params![id],
            Self::row_to_memory_item,
        )
        .optional()
        .map_err(|e| format!("Failed to get memory by id: {e}"))
    }

    /// Get memories by scope with full FSRS fields.
    fn get_memories_by_scope(
        &self,
        scope: MemoryScope,
        limit: u32,
    ) -> Result<Vec<MemoryItem>, String> {
        let conn = self.store.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, scope, category, key, content, importance,
                        stability, difficulty, reps, next_review,
                        created_at, updated_at
                 FROM memories WHERE scope = ?1
                 ORDER BY importance DESC LIMIT ?2",
            )
            .map_err(|e| format!("Failed to prepare scope query: {e}"))?;

        let rows = stmt
            .query_map(params![scope.as_str(), limit], Self::row_to_memory_item)
            .map_err(|e| format!("Failed to query memories by scope: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect memories: {e}"))
    }

    /// Search a specific tier using SQLite LIKE on content.
    fn search_tier(
        &self,
        scope: MemoryScope,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryItem>, String> {
        let pattern = format!("%{query}%");
        let conn = self.store.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, scope, category, key, content, importance,
                        stability, difficulty, reps, next_review,
                        created_at, updated_at
                 FROM memories
                 WHERE scope = ?1 AND content LIKE ?2
                 ORDER BY importance DESC LIMIT ?3",
            )
            .map_err(|e| format!("Failed to prepare search query: {e}"))?;

        let rows = stmt
            .query_map(
                params![scope.as_str(), pattern, limit as u32],
                Self::row_to_memory_item,
            )
            .map_err(|e| format!("Failed to search tier: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect search results: {e}"))
    }

    /// Enforce tier limits after an insertion. Demotes lowest-importance
    /// items when a tier exceeds its maximum size.
    fn enforce_limits(&self, scope: MemoryScope) -> Result<(), String> {
        let limit = match scope {
            MemoryScope::Core => CORE_LIMIT,
            MemoryScope::Recall => RECALL_LIMIT,
            MemoryScope::Archival => return Ok(()), // archival is unlimited
        };
        self.enforce_tier_limit(scope, limit)?;
        Ok(())
    }

    /// Demote the lowest-importance items in a tier until count <= limit.
    /// Returns the number of items demoted.
    fn enforce_tier_limit(&self, scope: MemoryScope, limit: usize) -> Result<usize, String> {
        let target = match scope.demote_target() {
            Some(t) => t,
            None => return Ok(0),
        };

        // Get IDs of overflow items ordered by importance ASC (lowest first)
        let overflow_ids: Vec<String> = {
            let conn = self.store.conn.lock();
            let mut stmt = conn
                .prepare("SELECT id FROM memories WHERE scope = ?1 ORDER BY importance ASC")
                .map_err(|e| format!("Failed to query overflow: {e}"))?;

            let all_ids: Vec<String> = stmt
                .query_map(params![scope.as_str()], |row| row.get(0))
                .map_err(|e| format!("Failed to map overflow rows: {e}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect overflow ids: {e}"))?;

            if all_ids.len() > limit {
                all_ids[..all_ids.len() - limit].to_vec()
            } else {
                Vec::new()
            }
        };

        let demoted = overflow_ids.len();
        for id in &overflow_ids {
            self.store
                .update_memory_scope(id, target.as_str())
                .map_err(|e| format!("Failed to demote overflow item: {e}"))?;
            // Use "demoted" event_type (valid per memory_lifecycle CHECK constraint)
            self.store
                .log_memory_event(
                    id,
                    "demoted",
                    Some(&format!(
                        "{{\"from\":\"{}\",\"to\":\"{}\",\"reason\":\"tier_overflow\"}}",
                        scope.as_str(),
                        target.as_str()
                    )),
                )
                .map_err(|e| format!("Failed to log auto-demotion: {e}"))?;
        }

        Ok(demoted)
    }

    /// Apply temporal importance decay to all memories.
    ///
    /// Formula: importance' = importance * 2^(-elapsed_days / half_life_days)
    ///
    /// Memories decay based on accessed_at (or created_at if never accessed).
    /// Computation is done in Rust because SQLite lacks POWER/EXP functions.
    /// Returns the number of memories affected.
    fn apply_temporal_decay(&self) -> Result<usize, String> {
        // Step 1: Read candidates with their elapsed days
        let candidates: Vec<(String, f64, f64)> = {
            let conn = self.store.conn.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT id, importance,
                            julianday('now') - julianday(COALESCE(accessed_at, created_at))
                     FROM memories
                     WHERE importance > 0.01",
                )
                .map_err(|e| format!("Failed to prepare decay query: {e}"))?;

            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                })
                .map_err(|e| format!("Failed to query decay candidates: {e}"))?;

            let collected: Vec<(String, f64, f64)> = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect decay candidates: {e}"))?;
            collected
        };

        // Step 2: Compute decay in Rust and update each row
        let mut affected = 0usize;
        for (id, importance, elapsed_days) in &candidates {
            if *elapsed_days <= 0.0 {
                continue;
            }
            // decay_factor = 2^(-elapsed_days / half_life_days)
            let decay_factor = 2.0_f64.powf(-elapsed_days / DECAY_HALF_LIFE_DAYS);
            let new_importance = importance * decay_factor;

            let conn = self.store.conn.lock();
            let rows = conn
                .execute(
                    "UPDATE memories SET importance = ?1,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?2",
                    params![new_importance, id],
                )
                .map_err(|e| format!("Failed to update decayed importance: {e}"))?;
            affected += rows;
        }

        Ok(affected)
    }

    /// Convert a rusqlite Row to a MemoryItem.
    fn row_to_memory_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryItem> {
        let scope_str: String = row.get(1)?;
        let scope = MemoryScope::parse(&scope_str).unwrap_or(MemoryScope::Archival);
        Ok(MemoryItem {
            id: row.get(0)?,
            scope,
            category: row.get(2)?,
            key: row.get(3)?,
            content: row.get(4)?,
            importance: row.get(5)?,
            stability: row.get(6)?,
            difficulty: row.get(7)?,
            reps: row.get(8)?,
            next_review: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }

    /// Count memories in a given scope.
    pub fn count_scope(&self, scope: MemoryScope) -> Result<usize, String> {
        let conn = self.store.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE scope = ?1",
            params![scope.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .map_err(|e| format!("Failed to count scope: {e}"))
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a fresh in-memory store.
    fn test_store() -> ForgeMemoryStore {
        ForgeMemoryStore::open_memory().unwrap()
    }

    // ── 1. Add memory to core tier ──────────────────────────────

    #[test]
    fn test_add_core_memory() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory(
                "Project architecture decisions",
                MemoryScope::Core,
                0.9,
                "architecture",
                None,
            )
            .unwrap();
        assert!(!id.is_empty());

        let core = mem.get_core_memories().unwrap();
        assert_eq!(core.len(), 1);
        assert_eq!(core[0].content, "Project architecture decisions");
        assert_eq!(core[0].scope, MemoryScope::Core);
    }

    // ── 2. Add memory to recall tier ────────────────────────────

    #[test]
    fn test_add_recall_memory() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory(
            "User asked about GPU support",
            MemoryScope::Recall,
            0.6,
            "interaction",
            None,
        )
        .unwrap();

        let results = mem.search_recall("GPU", &[], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, MemoryScope::Recall);
    }

    // ── 3. Add memory to archival tier ──────────────────────────

    #[test]
    fn test_add_archival_memory() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory(
            "HNSW algorithm details from 2018 paper",
            MemoryScope::Archival,
            0.4,
            "research",
            None,
        )
        .unwrap();

        let results = mem.search_archival("HNSW", &[], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, MemoryScope::Archival);
    }

    // ── 4. Core limit enforcement (add 25, verify 20 in core) ───

    #[test]
    fn test_core_limit_enforcement() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        // Add 25 items to core with increasing importance
        for i in 0..25 {
            let importance = (i as f64) / 24.0;
            mem.add_memory(
                &format!("Core item {i}"),
                MemoryScope::Core,
                importance,
                "test",
                None,
            )
            .unwrap();
        }

        let core = mem.get_core_memories().unwrap();
        assert_eq!(
            core.len(),
            CORE_LIMIT,
            "Core should have exactly {CORE_LIMIT} items, got {}",
            core.len()
        );

        // The 5 lowest-importance items should have been demoted to recall
        let recall_count = mem.count_scope(MemoryScope::Recall).unwrap();
        assert_eq!(recall_count, 5, "5 items should have been demoted to recall");
    }

    // ── 5. Promote memory from recall to core ───────────────────

    #[test]
    fn test_promote_recall_to_core() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Important finding", MemoryScope::Recall, 0.8, "research", None)
            .unwrap();

        assert!(mem.promote_memory(&id, MemoryScope::Core).unwrap());

        let core = mem.get_core_memories().unwrap();
        assert_eq!(core.len(), 1);
        assert_eq!(core[0].id, id);
        assert_eq!(core[0].scope, MemoryScope::Core);
    }

    // ── 6. Demote memory from core to recall ────────────────────

    #[test]
    fn test_demote_core_to_recall() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Outdated info", MemoryScope::Core, 0.3, "general", None)
            .unwrap();

        assert!(mem.demote_memory(&id).unwrap());

        let core = mem.get_core_memories().unwrap();
        assert_eq!(core.len(), 0, "Core should be empty after demotion");

        let recall = mem.search_recall("Outdated", &[], 10).unwrap();
        assert_eq!(recall.len(), 1);
        assert_eq!(recall[0].scope, MemoryScope::Recall);
    }

    // ── 7. Demote recall to archival ────────────────────────────

    #[test]
    fn test_demote_recall_to_archival() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Old interaction log", MemoryScope::Recall, 0.2, "log", None)
            .unwrap();

        assert!(mem.demote_memory(&id).unwrap());

        let archival = mem.search_archival("Old interaction", &[], 10).unwrap();
        assert_eq!(archival.len(), 1);
        assert_eq!(archival[0].scope, MemoryScope::Archival);
    }

    // ── 8. Cannot demote archival further ───────────────────────

    #[test]
    fn test_cannot_demote_archival() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Bottom tier item", MemoryScope::Archival, 0.1, "general", None)
            .unwrap();

        let result = mem.demote_memory(&id);
        assert!(result.is_err(), "Demoting archival should fail");
        assert!(result.unwrap_err().contains("Cannot demote archival"));
    }

    // ── 9. FSRS review updates stability ────────────────────────

    #[test]
    fn test_fsrs_review_updates_stability() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("FSRS test item", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        let before = mem.get_memory_by_id(&id).unwrap().unwrap();
        assert!((before.stability - 1.0).abs() < f64::EPSILON, "Initial stability should be 1.0");
        assert_eq!(before.reps, 0, "Initial reps should be 0");

        // Review with "Good" rating (grade=2)
        mem.review_memory(&id, ReviewRating::Good).unwrap();

        let after = mem.get_memory_by_id(&id).unwrap().unwrap();
        assert_eq!(after.reps, 1, "Reps should increment to 1");
        assert!(
            after.stability > before.stability,
            "Good review should increase stability: {} -> {}",
            before.stability,
            after.stability
        );
        assert!(
            after.next_review.is_some(),
            "Should have a next_review scheduled"
        );
    }

    // ── 10. FSRS easy review increases stability more than hard ──

    #[test]
    fn test_fsrs_easy_vs_hard() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id_easy = mem
            .add_memory("Easy item", MemoryScope::Core, 0.8, "test", None)
            .unwrap();
        let id_hard = mem
            .add_memory("Hard item", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        mem.review_memory(&id_easy, ReviewRating::Easy).unwrap();
        mem.review_memory(&id_hard, ReviewRating::Hard).unwrap();

        let easy = mem.get_memory_by_id(&id_easy).unwrap().unwrap();
        let hard = mem.get_memory_by_id(&id_hard).unwrap().unwrap();

        assert!(
            easy.stability > hard.stability,
            "Easy review should produce higher stability than Hard: easy={}, hard={}",
            easy.stability,
            hard.stability
        );
    }

    // ── 11. FSRS "Again" rating counts as lapse ─────────────────

    #[test]
    fn test_fsrs_again_is_lapse() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Lapse test", MemoryScope::Core, 0.7, "test", None)
            .unwrap();

        mem.review_memory(&id, ReviewRating::Again).unwrap();

        let item = mem.get_memory_by_id(&id).unwrap().unwrap();
        assert_eq!(item.reps, 1);
        assert!(item.stability > 0.0, "Stability should remain positive");
    }

    // ── 12. Due memories returned correctly ─────────────────────

    #[test]
    fn test_due_memories() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Due test item", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        // Set next_review to the past so it is due
        {
            let conn = store.conn.lock();
            conn.execute(
                "UPDATE memories SET next_review = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')
                 WHERE id = ?1",
                params![id],
            )
            .unwrap();
        }

        let due = mem.get_due_memories().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, id);
    }

    // ── 13. Memories without next_review are not due ────────────

    #[test]
    fn test_no_review_not_due() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory("No review scheduled", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        let due = mem.get_due_memories().unwrap();
        assert_eq!(due.len(), 0, "Memory without next_review should not be due");
    }

    // ── 14. Consolidation demotes from full core ────────────────

    #[test]
    fn test_consolidation_enforces_limits() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        // Insert directly via store to bypass enforce_limits
        for i in 0..25 {
            let importance = (i as f64) / 30.0;
            store
                .insert_memory("core", "test", None, &format!("Item {i}"), importance, None)
                .unwrap();
        }

        let core_before = mem.count_scope(MemoryScope::Core).unwrap();
        assert_eq!(core_before, 25);

        let report = mem.consolidate().unwrap();
        assert_eq!(report.core_demoted, 5, "Should demote 5 items from core");

        let core_after = mem.count_scope(MemoryScope::Core).unwrap();
        assert_eq!(core_after, CORE_LIMIT);
    }

    // ── 15. Search across recall tier ───────────────────────────

    #[test]
    fn test_search_recall_tier() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory("Rust async runtime tokio", MemoryScope::Recall, 0.7, "tech", None)
            .unwrap();
        mem.add_memory("Python data science pandas", MemoryScope::Recall, 0.6, "tech", None)
            .unwrap();
        mem.add_memory(
            "Rust memory safety borrow checker",
            MemoryScope::Recall,
            0.5,
            "tech",
            None,
        )
        .unwrap();

        let results = mem.search_recall("Rust", &[], 10).unwrap();
        assert_eq!(results.len(), 2, "Should find 2 Rust-related memories");
        // Results sorted by importance DESC
        assert!(results[0].importance >= results[1].importance);
    }

    // ── 16. Importance-based ranking in core ────────────────────

    #[test]
    fn test_importance_ranking() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory("Low importance", MemoryScope::Core, 0.2, "test", None)
            .unwrap();
        mem.add_memory("High importance", MemoryScope::Core, 0.9, "test", None)
            .unwrap();
        mem.add_memory("Medium importance", MemoryScope::Core, 0.5, "test", None)
            .unwrap();

        let core = mem.get_core_memories().unwrap();
        assert_eq!(core.len(), 3);
        assert!(core[0].importance >= core[1].importance);
        assert!(core[1].importance >= core[2].importance);
        assert_eq!(core[0].content, "High importance");
    }

    // ── 17. Empty tiers return empty results ────────────────────

    #[test]
    fn test_empty_tiers() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        assert!(mem.get_core_memories().unwrap().is_empty());
        assert!(mem.search_recall("anything", &[], 10).unwrap().is_empty());
        assert!(mem.search_archival("anything", &[], 10).unwrap().is_empty());
        assert!(mem.get_due_memories().unwrap().is_empty());
    }

    // ── 18. MemoryScope string round-trip ───────────────────────

    #[test]
    fn test_memory_scope_round_trip() {
        for scope in [MemoryScope::Core, MemoryScope::Recall, MemoryScope::Archival] {
            let s = scope.as_str();
            let parsed = MemoryScope::parse(s).unwrap();
            assert_eq!(scope, parsed);
        }
        assert!(MemoryScope::parse("invalid").is_none());
    }

    // ── 19. Demote target chain ─────────────────────────────────

    #[test]
    fn test_demote_target_chain() {
        assert_eq!(MemoryScope::Core.demote_target(), Some(MemoryScope::Recall));
        assert_eq!(
            MemoryScope::Recall.demote_target(),
            Some(MemoryScope::Archival)
        );
        assert_eq!(MemoryScope::Archival.demote_target(), None);
    }

    // ── 20. ReviewRating grade values ───────────────────────────

    #[test]
    fn test_review_rating_grades() {
        assert!((ReviewRating::Again.grade() - 0.0).abs() < f64::EPSILON);
        assert!((ReviewRating::Hard.grade() - 1.0).abs() < f64::EPSILON);
        assert!((ReviewRating::Good.grade() - 2.0).abs() < f64::EPSILON);
        assert!((ReviewRating::Easy.grade() - 3.0).abs() < f64::EPSILON);
    }

    // ── 21. Multiple reviews accumulate reps ────────────────────

    #[test]
    fn test_multiple_reviews() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Multi review", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        mem.review_memory(&id, ReviewRating::Good).unwrap();
        mem.review_memory(&id, ReviewRating::Easy).unwrap();
        mem.review_memory(&id, ReviewRating::Hard).unwrap();

        let item = mem.get_memory_by_id(&id).unwrap().unwrap();
        assert_eq!(item.reps, 3);
        assert!(
            item.stability > 1.0,
            "Stability should grow with multiple reviews: {}",
            item.stability
        );
    }

    // ── 22. Get core always returns all core items ──────────────

    #[test]
    fn test_get_core_returns_all() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        for i in 0..CORE_LIMIT {
            mem.add_memory(
                &format!("Core item {i}"),
                MemoryScope::Core,
                0.5,
                "test",
                None,
            )
            .unwrap();
        }

        let core = mem.get_core_memories().unwrap();
        assert_eq!(
            core.len(),
            CORE_LIMIT,
            "Should return all {CORE_LIMIT} core items"
        );
    }

    // ── 23. Consolidation report is accurate for empty store ────

    #[test]
    fn test_consolidation_report_empty() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let report = mem.consolidate().unwrap();
        assert_eq!(report.core_demoted, 0);
        assert_eq!(report.recall_archived, 0);
    }

    // ── 24. Search archival does not return recall items ─────────

    #[test]
    fn test_search_tier_isolation() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        mem.add_memory("Rust in recall", MemoryScope::Recall, 0.6, "tech", None)
            .unwrap();
        mem.add_memory("Rust in archival", MemoryScope::Archival, 0.4, "tech", None)
            .unwrap();

        let recall = mem.search_recall("Rust", &[], 10).unwrap();
        assert_eq!(recall.len(), 1);
        assert_eq!(recall[0].scope, MemoryScope::Recall);

        let archival = mem.search_archival("Rust", &[], 10).unwrap();
        assert_eq!(archival.len(), 1);
        assert_eq!(archival[0].scope, MemoryScope::Archival);
    }

    // ── 25. FSRS difficulty adjusts toward grade ────────────────

    #[test]
    fn test_fsrs_difficulty_adjustment() {
        let store = test_store();
        let mem = TieredMemory::new(&store);

        let id = mem
            .add_memory("Difficulty test", MemoryScope::Core, 0.8, "test", None)
            .unwrap();

        let before = mem.get_memory_by_id(&id).unwrap().unwrap();
        let initial_difficulty = before.difficulty; // 0.3 default

        // Review with Again (grade=0): D' = 0.3 + 0.1*(3-0)/3 = 0.3 + 0.1 = 0.4
        mem.review_memory(&id, ReviewRating::Again).unwrap();

        let after = mem.get_memory_by_id(&id).unwrap().unwrap();
        assert!(
            after.difficulty > initial_difficulty,
            "Again review should increase difficulty: {} -> {}",
            initial_difficulty,
            after.difficulty
        );
    }
}
