// SPDX-License-Identifier: BUSL-1.1
//
// Copyright (c) 2026 AiImp Development. All rights reserved.
// Licensed under the Business Source License 1.1.
// See LICENSE-ENGINE in the repository root.

//! Brain v2.0 — Neuroscience-Inspired Memory Management
//!
//! Implements cognitive science algorithms for AI memory:
//!
//! 1. **FSRS** (Free Spaced Repetition Scheduler) — Jarrett Ye (2022-2024)
//!    Optimal review scheduling based on memory stability/retrievability.
//!    Paper: "A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition"
//!
//! 2. **CLS** (Complementary Learning Systems) — McClelland et al. (1995)
//!    Hippocampus (fast, episodic) → Neocortex (slow, consolidated) memory replay.
//!    Paper: "Why there are complementary learning systems in the hippocampus and neocortex"
//!
//! 3. **A-MEM Zettelkasten** — Cross-referencing knowledge cards
//!    Inspired by Niklas Luhmann's Zettelkasten method.
//!
//! 4. **TeleMem** — ADD/UPDATE/DELETE/NOOP memory management pipeline
//!    Determines optimal memory operations for incoming information.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ════════════════════════════════════════════════════════════════
// FSRS — Free Spaced Repetition Scheduler
// ════════════════════════════════════════════════════════════════

/// FSRS-5 parameters (optimized defaults from open-spaced-repetition/fsrs-rs)
///
/// These 19 parameters control the learning algorithm:
/// w[0..3]   = initial stability for each rating (again, hard, good, easy)
/// w[4..7]   = difficulty parameters
/// w[8..10]  = stability after lapse
/// w[11..18] = stability growth and decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsParams {
    pub w: [f64; 19],
    pub decay: f64,
    pub factor: f64,
    pub request_retention: f64,
}

impl Default for FsrsParams {
    fn default() -> Self {
        // FSRS-5 optimized defaults (from fsrs-rs v1.0)
        Self {
            w: [
                0.4072, 1.1829, 3.1262, 15.4722, // Initial stability
                7.2102, 0.5316, 1.0651, 0.0589,   // Difficulty
                1.5330, 0.1253, 1.0120,            // Post-lapse stability
                2.0044, 0.0275, 0.3564, 0.1367,    // Stability growth
                0.2323, 2.8214, 0.0542, 0.0600,    // Additional parameters
            ],
            decay: -0.5,              // Power law decay exponent
            factor: 19.0_f64 / 81.0, // Scaling factor (19/81)
            request_retention: 0.9,   // Target 90% retention
        }
    }
}

/// FSRS rating for a memory review
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

/// Memory card state for FSRS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsCard {
    pub stability: f64,
    pub difficulty: f64,
    pub elapsed_days: f64,
    pub scheduled_days: f64,
    pub reps: u32,
    pub lapses: u32,
    pub last_review: DateTime<Utc>,
}

impl Default for FsrsCard {
    fn default() -> Self {
        Self {
            stability: 0.0,
            difficulty: 0.0,
            elapsed_days: 0.0,
            scheduled_days: 0.0,
            reps: 0,
            lapses: 0,
            last_review: Utc::now(),
        }
    }
}

/// FSRS scheduler
pub struct FsrsScheduler {
    params: FsrsParams,
}

impl FsrsScheduler {
    pub fn new() -> Self {
        Self {
            params: FsrsParams::default(),
        }
    }

    pub fn with_params(params: FsrsParams) -> Self {
        Self { params }
    }

    /// Calculate retrievability (probability of recall) at time t
    ///
    /// R(t) = (1 + factor * t/S)^decay
    /// where S = stability, t = elapsed time in days
    pub fn retrievability(&self, stability: f64, elapsed_days: f64) -> f64 {
        if stability <= 0.0 {
            return 0.0;
        }
        (1.0 + self.params.factor * elapsed_days / stability).powf(self.params.decay)
    }

    /// Calculate next interval from stability and desired retention
    ///
    /// I(r, S) = S / factor * (r^(1/decay) - 1)
    pub fn next_interval(&self, stability: f64) -> f64 {
        let r = self.params.request_retention;
        let interval = stability / self.params.factor
            * (r.powf(1.0 / self.params.decay) - 1.0);
        interval.max(1.0).min(36500.0) // 1 day to 100 years
    }

    /// Calculate initial stability for a rating
    fn initial_stability(&self, rating: Rating) -> f64 {
        let idx = rating as usize - 1;
        self.params.w[idx].max(0.1)
    }

    /// Calculate initial difficulty for a rating
    fn initial_difficulty(&self, rating: Rating) -> f64 {
        let d = self.params.w[4] - (rating as i32 - 3) as f64 * self.params.w[5];
        d.clamp(1.0, 10.0)
    }

    /// Update difficulty after a review
    fn next_difficulty(&self, difficulty: f64, rating: Rating) -> f64 {
        let delta = -(self.params.w[6] * (rating as i32 - 3) as f64);
        let new_d = difficulty + delta;
        // Mean reversion toward initial difficulty
        let w7 = self.params.w[7];
        let mean_revert = w7 * self.initial_difficulty(Rating::Easy) + (1.0 - w7) * new_d;
        mean_revert.clamp(1.0, 10.0)
    }

    /// Calculate new stability after successful recall
    fn next_recall_stability(&self, d: f64, s: f64, r: f64, rating: Rating) -> f64 {
        let modifier = match rating {
            Rating::Hard => self.params.w[15],
            Rating::Easy => self.params.w[16],
            _ => 1.0,
        };
        s * (1.0
            + (self.params.w[8]).exp()
                * (11.0 - d)
                * s.powf(-self.params.w[9])
                * ((1.0 - r).powf(self.params.w[10]) * modifier - 1.0))
    }

    /// Calculate new stability after a lapse (forgetting)
    fn next_forget_stability(&self, d: f64, s: f64, r: f64) -> f64 {
        self.params.w[11]
            * d.powf(-self.params.w[12])
            * ((s + 1.0).powf(self.params.w[13]) - 1.0)
            * (1.0 - r).powf(self.params.w[14])
            .max(0.1)
    }

    /// Process a review and return the updated card
    pub fn review(&self, card: &FsrsCard, rating: Rating) -> FsrsCard {
        let now = Utc::now();
        let elapsed = (now - card.last_review).num_seconds() as f64 / 86400.0;

        if card.reps == 0 {
            // New card
            let s = self.initial_stability(rating);
            let d = self.initial_difficulty(rating);
            let interval = self.next_interval(s);
            return FsrsCard {
                stability: s,
                difficulty: d,
                elapsed_days: 0.0,
                scheduled_days: interval,
                reps: 1,
                lapses: if rating == Rating::Again { 1 } else { 0 },
                last_review: now,
            };
        }

        let r = self.retrievability(card.stability, elapsed);
        let new_d = self.next_difficulty(card.difficulty, rating);

        let (new_s, new_lapses) = if rating == Rating::Again {
            // Lapse
            let s = self.next_forget_stability(card.difficulty, card.stability, r);
            (s, card.lapses + 1)
        } else {
            // Successful recall
            let s = self.next_recall_stability(card.difficulty, card.stability, r, rating);
            (s, card.lapses)
        };

        let interval = self.next_interval(new_s);

        FsrsCard {
            stability: new_s,
            difficulty: new_d,
            elapsed_days: elapsed,
            scheduled_days: interval,
            reps: card.reps + 1,
            lapses: new_lapses,
            last_review: now,
        }
    }
}

impl Default for FsrsScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════
// CLS — Complementary Learning Systems
// ════════════════════════════════════════════════════════════════

/// Memory layer (hippocampus = fast/episodic, neocortex = slow/consolidated)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MemoryLayer {
    Hippocampus,
    Neocortex,
}

/// A memory in the CLS system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClsMemory {
    pub key: String,
    pub content: String,
    pub layer: MemoryLayer,
    pub importance: f64,
    pub access_count: u32,
    pub created_at: DateTime<Utc>,
    pub consolidated_at: Option<DateTime<Utc>>,
}

/// CLS Replay Engine — consolidates memories from hippocampus to neocortex
pub struct ClsReplayEngine {
    /// Minimum access count before consolidation eligible
    pub consolidation_threshold: u32,
    /// Minimum age (hours) before consolidation
    pub min_age_hours: i64,
    /// Maximum hippocampus size before forced consolidation
    pub max_hippocampus_size: usize,
}

impl Default for ClsReplayEngine {
    fn default() -> Self {
        Self {
            consolidation_threshold: 3,
            min_age_hours: 24,
            max_hippocampus_size: 1000,
        }
    }
}

impl ClsReplayEngine {
    /// Determine which memories should be consolidated (replayed to neocortex)
    pub fn select_for_consolidation(&self, memories: &[ClsMemory]) -> Vec<String> {
        let now = Utc::now();
        memories
            .iter()
            .filter(|m| {
                m.layer == MemoryLayer::Hippocampus
                    && m.access_count >= self.consolidation_threshold
                    && (now - m.created_at).num_hours() >= self.min_age_hours
            })
            .map(|m| m.key.clone())
            .collect()
    }

    /// Check if forced consolidation is needed (hippocampus overflow)
    pub fn needs_forced_consolidation(&self, hippocampus_count: usize) -> bool {
        hippocampus_count > self.max_hippocampus_size
    }

    /// Score a memory for consolidation priority (higher = consolidate first)
    pub fn consolidation_priority(&self, memory: &ClsMemory) -> f64 {
        let age_factor = (Utc::now() - memory.created_at).num_hours() as f64 / 24.0;
        let access_factor = memory.access_count as f64;
        memory.importance * access_factor * (1.0 + age_factor.ln().max(0.0))
    }
}

// ════════════════════════════════════════════════════════════════
// TeleMem — Memory Management Pipeline
// ════════════════════════════════════════════════════════════════

/// TeleMem operation type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TeleMemOp {
    Add,
    Update,
    Delete,
    Noop,
}

/// TeleMem decision for an incoming piece of information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleMemDecision {
    pub operation: TeleMemOp,
    pub key: String,
    pub reason: String,
    pub confidence: f64,
}

/// TeleMem Pipeline — decides what to do with incoming information
pub struct TeleMemPipeline;

impl TeleMemPipeline {
    /// Analyze incoming content and decide the memory operation
    ///
    /// Rules:
    /// - New unique content -> ADD
    /// - Similar content exists but with updates -> UPDATE
    /// - Content contradicts existing memory -> UPDATE (replace)
    /// - Content is duplicate -> NOOP
    /// - Content is outdated/irrelevant -> DELETE existing
    pub fn decide(
        key: &str,
        content: &str,
        existing: Option<&str>,
        similarity: f64,
    ) -> TeleMemDecision {
        if existing.is_none() {
            return TeleMemDecision {
                operation: TeleMemOp::Add,
                key: key.to_string(),
                reason: "New unique content".to_string(),
                confidence: 0.9,
            };
        }

        let existing = existing.unwrap();

        if similarity > 0.95 {
            return TeleMemDecision {
                operation: TeleMemOp::Noop,
                key: key.to_string(),
                reason: "Near-duplicate content".to_string(),
                confidence: similarity,
            };
        }

        if similarity > 0.7 {
            return TeleMemDecision {
                operation: TeleMemOp::Update,
                key: key.to_string(),
                reason: "Similar content with updates".to_string(),
                confidence: similarity,
            };
        }

        // Low similarity — content is substantially different
        if content.len() > existing.len() * 2 {
            TeleMemDecision {
                operation: TeleMemOp::Update,
                key: key.to_string(),
                reason: "Substantially expanded content".to_string(),
                confidence: 0.8,
            }
        } else {
            TeleMemDecision {
                operation: TeleMemOp::Add,
                key: key.to_string(),
                reason: "Different content for same key".to_string(),
                confidence: 0.7,
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Zettelkasten — Cross-Referenced Knowledge Cards
// ════════════════════════════════════════════════════════════════

/// A Zettelkasten note (knowledge card)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZettelNote {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Zettelkasten index for cross-referencing
pub struct ZettelIndex {
    notes: Vec<ZettelNote>,
    tag_index: std::collections::HashMap<String, Vec<String>>,
}

impl ZettelIndex {
    pub fn new() -> Self {
        Self {
            notes: Vec::new(),
            tag_index: std::collections::HashMap::new(),
        }
    }

    /// Add a note and update tag index
    pub fn add_note(&mut self, note: ZettelNote) {
        for tag in &note.tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(note.id.clone());
        }
        self.notes.push(note);
    }

    /// Find notes by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&ZettelNote> {
        match self.tag_index.get(tag) {
            Some(ids) => self
                .notes
                .iter()
                .filter(|n| ids.contains(&n.id))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Find related notes (notes that share tags with a given note)
    pub fn find_related(&self, note_id: &str) -> Vec<&ZettelNote> {
        let note = match self.notes.iter().find(|n| n.id == note_id) {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut related_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tag in &note.tags {
            if let Some(ids) = self.tag_index.get(tag) {
                for id in ids {
                    if id != note_id {
                        related_ids.insert(id.clone());
                    }
                }
            }
        }

        self.notes
            .iter()
            .filter(|n| related_ids.contains(&n.id))
            .collect()
    }

    /// Get all tags with note counts
    pub fn tag_stats(&self) -> Vec<(String, usize)> {
        let mut stats: Vec<_> = self
            .tag_index
            .iter()
            .map(|(tag, ids)| (tag.clone(), ids.len()))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }
}

impl Default for ZettelIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_fsrs_retrievability() {
        let fsrs = FsrsScheduler::new();
        // At t=0, retrievability should be 1.0
        let r0 = fsrs.retrievability(10.0, 0.0);
        assert!((r0 - 1.0).abs() < 0.01);

        // At t=stability, retrievability should be ~0.9
        let r1 = fsrs.retrievability(10.0, 10.0);
        assert!(r1 > 0.85 && r1 < 0.95);

        // Retrievability decreases over time
        let r2 = fsrs.retrievability(10.0, 30.0);
        assert!(r2 < r1);
    }

    #[test]
    fn test_fsrs_review_new_card() {
        let fsrs = FsrsScheduler::new();
        let card = FsrsCard::default();
        let reviewed = fsrs.review(&card, Rating::Good);
        assert!(reviewed.stability > 0.0);
        assert!(reviewed.difficulty > 0.0);
        assert_eq!(reviewed.reps, 1);
    }

    #[test]
    fn test_fsrs_easy_gets_longer_interval() {
        let fsrs = FsrsScheduler::new();
        let card = FsrsCard::default();
        let easy = fsrs.review(&card, Rating::Easy);
        let good = fsrs.review(&card, Rating::Good);
        assert!(easy.scheduled_days > good.scheduled_days);
    }

    #[test]
    fn test_fsrs_again_counts_as_lapse() {
        let fsrs = FsrsScheduler::new();
        let card = FsrsCard::default();
        let reviewed = fsrs.review(&card, Rating::Again);
        assert_eq!(reviewed.lapses, 1);
    }

    #[test]
    fn test_cls_consolidation() {
        let engine = ClsReplayEngine::default();
        let old_memory = ClsMemory {
            key: "old".to_string(),
            content: "test".to_string(),
            layer: MemoryLayer::Hippocampus,
            importance: 0.8,
            access_count: 5,
            created_at: Utc::now() - Duration::hours(48),
            consolidated_at: None,
        };
        let new_memory = ClsMemory {
            key: "new".to_string(),
            content: "test2".to_string(),
            layer: MemoryLayer::Hippocampus,
            importance: 0.5,
            access_count: 1,
            created_at: Utc::now(),
            consolidated_at: None,
        };
        let candidates = engine.select_for_consolidation(&[old_memory, new_memory]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], "old");
    }

    #[test]
    fn test_telemem_add() {
        let decision = TeleMemPipeline::decide("key1", "new content", None, 0.0);
        assert_eq!(decision.operation, TeleMemOp::Add);
    }

    #[test]
    fn test_telemem_noop_duplicate() {
        let decision = TeleMemPipeline::decide("key1", "same", Some("same"), 0.98);
        assert_eq!(decision.operation, TeleMemOp::Noop);
    }

    #[test]
    fn test_telemem_update() {
        let decision = TeleMemPipeline::decide("key1", "updated", Some("original"), 0.85);
        assert_eq!(decision.operation, TeleMemOp::Update);
    }

    #[test]
    fn test_zettel_cross_reference() {
        let mut index = ZettelIndex::new();
        index.add_note(ZettelNote {
            id: "1".to_string(),
            title: "Rust".to_string(),
            content: "Rust lang".to_string(),
            tags: vec!["programming".to_string(), "systems".to_string()],
            links: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        index.add_note(ZettelNote {
            id: "2".to_string(),
            title: "C++".to_string(),
            content: "C++ lang".to_string(),
            tags: vec!["programming".to_string(), "legacy".to_string()],
            links: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        let related = index.find_related("1");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].title, "C++");
    }
}
