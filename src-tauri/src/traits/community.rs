// SPDX-License-Identifier: Apache-2.0
//! Community Edition Fallback Implementations
//!
//! Simple but functional implementations that ship with the free edition.
//! These are replaced by advanced implementations from `impforge-engine` (BSL 1.1)
//! when the Pro/Enterprise license is active.

use std::collections::HashMap;
use super::{TaskOutcome, TrustScorer, BrainEngine, InferenceRouter, RoutingDecision};

// ════════════════════════════════════════════════════════════════
// SIMPLE TRUST SCORER
// ════════════════════════════════════════════════════════════════

/// Simple trust scorer: tracks success rate as a running average.
/// Ships with the community edition (free).
pub struct SimpleTrustScorer {
    scores: HashMap<String, (f64, u64, u64)>, // (score, successes, failures)
}

impl SimpleTrustScorer {
    pub fn new() -> Self {
        Self { scores: HashMap::new() }
    }
}

impl Default for SimpleTrustScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl TrustScorer for SimpleTrustScorer {
    fn record_outcome(&mut self, worker_id: &str, outcome: TaskOutcome) -> f64 {
        let entry = self.scores.entry(worker_id.to_string()).or_insert((0.5, 0, 0));
        match outcome {
            TaskOutcome::Success { .. } => {
                entry.1 += 1;
            }
            TaskOutcome::Failure => {
                entry.2 += 1;
            }
            _ => {}
        }
        let total = entry.1 + entry.2;
        entry.0 = if total > 0 { entry.1 as f64 / total as f64 } else { 0.5 };
        entry.0
    }

    fn get_score(&self, worker_id: &str) -> f64 {
        self.scores.get(worker_id).map(|e| e.0).unwrap_or(0.5)
    }

    fn average(&self) -> f64 {
        if self.scores.is_empty() {
            return 0.5;
        }
        let sum: f64 = self.scores.values().map(|e| e.0).sum();
        sum / self.scores.len() as f64
    }

    fn all_scores(&self) -> Vec<(String, f64)> {
        self.scores.iter().map(|(k, v)| (k.clone(), v.0)).collect()
    }
}

// ════════════════════════════════════════════════════════════════
// SIMPLE BRAIN (fixed intervals)
// ════════════════════════════════════════════════════════════════

/// Fixed-interval memory scheduling for community edition.
/// Returns hardcoded review intervals instead of FSRS-5 adaptive scheduling.
pub struct SimpleBrain;

impl SimpleBrain {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleBrain {
    fn default() -> Self {
        Self::new()
    }
}

impl BrainEngine for SimpleBrain {
    fn schedule_review(&self, _item_id: &str, grade: u8) -> f64 {
        // Fixed intervals: Again=0.5d, Hard=1d, Good=3d, Easy=7d
        match grade {
            0 => 0.5,
            1 => 1.0,
            2 => 3.0,
            _ => 7.0,
        }
    }

    fn retrievability(&self, _item_id: &str) -> f64 {
        // Without FSRS state, assume moderate retrievability
        0.7
    }
}

// ════════════════════════════════════════════════════════════════
// SIMPLE ROUTER (single model)
// ════════════════════════════════════════════════════════════════

/// Single-model router for community edition.
/// Always routes to the first available local model (Ollama).
pub struct SimpleRouter {
    model_id: String,
}

impl SimpleRouter {
    pub fn new(model_id: impl Into<String>) -> Self {
        Self { model_id: model_id.into() }
    }
}

impl Default for SimpleRouter {
    fn default() -> Self {
        Self::new("dolphin3:8b")
    }
}

impl InferenceRouter for SimpleRouter {
    fn route(&self, _prompt: &str, _task_hint: Option<&str>) -> RoutingDecision {
        RoutingDecision {
            model_id: self.model_id.clone(),
            tier: 0,
            confidence: 1.0,
            estimated_cost: 0.0,
        }
    }

    fn available_tiers(&self) -> Vec<(u8, String, bool)> {
        vec![(0, self.model_id.clone(), true)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_trust_scorer_basics() {
        let mut scorer = SimpleTrustScorer::new();
        assert_eq!(scorer.get_score("w1"), 0.5);

        scorer.record_outcome("w1", TaskOutcome::Success { duration_ms: 100 });
        assert_eq!(scorer.get_score("w1"), 1.0);

        scorer.record_outcome("w1", TaskOutcome::Failure);
        assert_eq!(scorer.get_score("w1"), 0.5);

        assert!(scorer.should_run("w1", 0.4));
        assert!(!scorer.should_run("w1", 0.6));
    }

    #[test]
    fn simple_trust_scorer_average() {
        let mut scorer = SimpleTrustScorer::new();
        scorer.record_outcome("w1", TaskOutcome::Success { duration_ms: 0 });
        scorer.record_outcome("w2", TaskOutcome::Failure);
        assert_eq!(scorer.average(), 0.5);
    }

    #[test]
    fn simple_brain_fixed_intervals() {
        let brain = SimpleBrain::new();
        assert_eq!(brain.schedule_review("item1", 0), 0.5);
        assert_eq!(brain.schedule_review("item1", 1), 1.0);
        assert_eq!(brain.schedule_review("item1", 2), 3.0);
        assert_eq!(brain.schedule_review("item1", 3), 7.0);
        assert_eq!(brain.retrievability("item1"), 0.7);
    }

    #[test]
    fn simple_router_single_model() {
        let router = SimpleRouter::default();
        let decision = router.route("hello world", None);
        assert_eq!(decision.model_id, "dolphin3:8b");
        assert_eq!(decision.tier, 0);
        assert_eq!(decision.estimated_cost, 0.0);

        let tiers = router.available_tiers();
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].0, 0);
    }

    #[test]
    fn task_outcome_is_success() {
        assert!(TaskOutcome::Success { duration_ms: 100 }.is_success());
        assert!(!TaskOutcome::Failure.is_success());
        assert!(!TaskOutcome::Timeout.is_success());
        assert!(!TaskOutcome::Skipped.is_success());
    }
}
