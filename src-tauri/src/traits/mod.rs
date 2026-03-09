// SPDX-License-Identifier: Apache-2.0
//! ImpForge Trait Abstractions — Public API Boundary
//!
//! These traits define the contracts for ImpForge's AI engine components.
//! The community edition ships simple fallback implementations (see `community` module).
//! The proprietary `impforge-engine` crate (BSL 1.1) provides advanced implementations:
//!   - Three-Factor Hebbian/STDP trust scoring
//!   - FSRS-5 spaced repetition + CLS replay
//!   - Cascade inference routing with confidence estimation

pub mod community;

// ════════════════════════════════════════════════════════════════
// CORE TYPES
// ════════════════════════════════════════════════════════════════

/// Outcome of a task execution, used by the trust scorer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskOutcome {
    Success { duration_ms: u64 },
    Failure,
    Timeout,
    Skipped,
}

impl TaskOutcome {
    /// Whether this outcome counts as a successful execution
    pub fn is_success(&self) -> bool {
        matches!(self, TaskOutcome::Success { .. })
    }
}

// ════════════════════════════════════════════════════════════════
// TRUST SCORING
// ════════════════════════════════════════════════════════════════

/// Trust scoring engine for AI agent orchestration.
///
/// Community: simple success/failure average.
/// Pro: Three-Factor Hebbian (STDP + dopamine + homeostasis).
///
/// References:
/// - Bi & Poo (1998): STDP timing windows
/// - Gerstner et al. (2018): Three-factor learning rules
/// - ArXiv 2504.05341: Three-Factor Hebbian Learning for AI
pub trait TrustScorer: Send + Sync {
    /// Score a worker after a task outcome. Returns updated trust [0.0, 1.0].
    fn record_outcome(&mut self, worker_id: &str, outcome: TaskOutcome) -> f64;
    /// Get current trust score for a worker.
    fn get_score(&self, worker_id: &str) -> f64;
    /// Check if a worker should be allowed to run.
    fn should_run(&self, worker_id: &str, threshold: f64) -> bool {
        self.get_score(worker_id) >= threshold
    }
    /// Average trust across all workers.
    fn average(&self) -> f64;
    /// List all worker scores as (worker_id, score) pairs.
    fn all_scores(&self) -> Vec<(String, f64)>;
}

// ════════════════════════════════════════════════════════════════
// BRAIN / MEMORY SCHEDULING
// ════════════════════════════════════════════════════════════════

/// Memory scheduling engine (spaced repetition + consolidation).
///
/// Community: fixed intervals.
/// Pro: FSRS-5 + CLS replay.
///
/// References:
/// - Ye (2022-2024): FSRS — IEEE TKDE 2023
/// - McClelland et al. (1995): Complementary Learning Systems
pub trait BrainEngine: Send + Sync {
    /// Schedule the next review for a memory item. Returns days until review.
    fn schedule_review(&self, item_id: &str, grade: u8) -> f64;
    /// Get retrievability (probability of recall) for an item.
    fn retrievability(&self, item_id: &str) -> f64;
}

// ════════════════════════════════════════════════════════════════
// INFERENCE ROUTING
// ════════════════════════════════════════════════════════════════

/// A routing decision from the cascade inference router.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutingDecision {
    /// Which model/tier was selected
    pub model_id: String,
    /// Which cascade tier (0 = local free, 4 = cloud premium)
    pub tier: u8,
    /// Estimated confidence that this tier can handle the prompt [0.0, 1.0]
    pub confidence: f64,
    /// Estimated cost in USD (0.0 for free/local models)
    pub estimated_cost: f64,
}

/// Cascade inference router for multi-tier model selection.
///
/// Community: single model (Ollama local).
/// Pro: 5-tier cascade with confidence estimation.
///
/// Tiers:
///   0: dolphin3:8b (local, free, general)
///   1: qwen2.5-coder:7b (local, free, code)
///   2: devstral-small (OpenRouter free, code)
///   3: llama-4-scout (OpenRouter free, general)
///   4: qwen3-235b-a22b (OpenRouter paid, premium)
pub trait InferenceRouter: Send + Sync {
    /// Select the best model for a given prompt.
    fn route(&self, prompt: &str, task_hint: Option<&str>) -> RoutingDecision;
    /// List available tiers and their models.
    fn available_tiers(&self) -> Vec<(u8, String, bool)>; // (tier, model_id, available)
}
