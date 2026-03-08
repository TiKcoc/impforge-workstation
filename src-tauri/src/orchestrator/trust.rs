//! Hebbian/STDP Trust Scoring for Nexus Standalone Orchestrator
//!
//! Based on Spike-Timing-Dependent Plasticity (STDP) from neuroscience:
//! - Success within expected time → trust increases (LTP)
//! - Failure or timeout → trust decreases (LTD)
//! - Exponential decay over time (biological synapse weakening)
//!
//! References:
//! - Bi & Poo (1998): "Synaptic Modifications in Cultured Hippocampal Neurons"
//! - Dan & Poo (2004): "Spike Timing-Dependent Plasticity of Neural Circuits"
//! - Markram et al. (2012): "A History of Spike-Timing-Dependent Plasticity"

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// STDP parameters (tuned for task scheduling, not biological neurons)
const DEFAULT_TRUST: f64 = 0.5;
const MIN_TRUST: f64 = 0.1;
const MAX_TRUST: f64 = 1.0;

/// LTP (Long-Term Potentiation) — how much trust increases on success
const A_PLUS: f64 = 0.08;
/// LTD (Long-Term Depression) — how much trust decreases on failure
const A_MINUS: f64 = 0.12;
/// Time constant for STDP window (seconds)
const TAU_STDP: f64 = 3600.0;
/// Decay half-life (seconds) — trust decays if worker is idle
const DECAY_HALF_LIFE: f64 = 86400.0; // 24 hours

/// Per-worker trust state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTrust {
    pub name: String,
    pub score: f64,
    pub successes: u64,
    pub failures: u64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
}

impl WorkerTrust {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            score: DEFAULT_TRUST,
            successes: 0,
            failures: 0,
            last_success: None,
            last_failure: None,
            last_updated: Utc::now(),
        }
    }
}

/// Hebbian Trust Manager — manages trust scores for all workers
pub struct HebbianTrustManager {
    workers: HashMap<String, WorkerTrust>,
}

impl HebbianTrustManager {
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
        }
    }

    /// Get current trust score for a worker (with time-based decay applied)
    pub fn get_trust(&self, worker: &str) -> f64 {
        match self.workers.get(worker) {
            Some(wt) => self.apply_decay(wt),
            None => DEFAULT_TRUST,
        }
    }

    /// Get full trust state for a worker
    pub fn get_worker_trust(&self, worker: &str) -> WorkerTrust {
        self.workers
            .get(worker)
            .cloned()
            .unwrap_or_else(|| WorkerTrust::new(worker))
    }

    /// Get all worker trust scores (sorted by score descending)
    pub fn get_all_trust(&self) -> Vec<WorkerTrust> {
        let mut all: Vec<_> = self.workers.values().map(|wt| {
            let mut wt = wt.clone();
            wt.score = self.apply_decay(&wt);
            wt
        }).collect();
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all
    }

    /// Record a successful task execution
    ///
    /// STDP LTP: Δw = A+ * exp(-Δt / τ)
    /// where Δt is time since last failure (shorter = stronger potentiation)
    pub fn record_success(&mut self, worker: &str, duration_ms: u64) {
        let wt = self.workers.entry(worker.to_string())
            .or_insert_with(|| WorkerTrust::new(worker));

        let now = Utc::now();
        wt.successes += 1;

        // STDP: Calculate potentiation based on time since last failure
        let delta_t = match wt.last_failure {
            Some(lf) => (now - lf).num_seconds() as f64,
            None => TAU_STDP * 2.0, // No failures → moderate boost
        };
        let stdp_factor = A_PLUS * (-delta_t / TAU_STDP).exp();

        // Speed bonus: faster execution = more trust (capped at 2x)
        let speed_bonus = if duration_ms > 0 {
            (5000.0 / duration_ms as f64).min(2.0).max(0.5)
        } else {
            1.0
        };

        wt.score = (wt.score + stdp_factor * speed_bonus).min(MAX_TRUST);
        wt.last_success = Some(now);
        wt.last_updated = now;
    }

    /// Record a failed task execution
    ///
    /// STDP LTD: Δw = -A- * exp(-Δt / τ)
    /// where Δt is time since last success (shorter = stronger depression)
    pub fn record_failure(&mut self, worker: &str) {
        let wt = self.workers.entry(worker.to_string())
            .or_insert_with(|| WorkerTrust::new(worker));

        let now = Utc::now();
        wt.failures += 1;

        // STDP: Calculate depression based on time since last success
        let delta_t = match wt.last_success {
            Some(ls) => (now - ls).num_seconds() as f64,
            None => TAU_STDP, // No successes → standard depression
        };
        let stdp_factor = A_MINUS * (-delta_t / TAU_STDP).exp();

        wt.score = (wt.score - stdp_factor).max(MIN_TRUST);
        wt.last_failure = Some(now);
        wt.last_updated = now;
    }

    /// Apply exponential time-based decay
    ///
    /// Decay formula: score_decayed = MIN + (score - MIN) * 2^(-Δt / half_life)
    /// This models biological synapse weakening when unused.
    fn apply_decay(&self, wt: &WorkerTrust) -> f64 {
        let elapsed = (Utc::now() - wt.last_updated).num_seconds() as f64;
        if elapsed <= 0.0 {
            return wt.score;
        }
        let decay = 2.0_f64.powf(-elapsed / DECAY_HALF_LIFE);
        MIN_TRUST + (wt.score - MIN_TRUST) * decay
    }

    /// Load trust scores from store records
    pub fn load_from_records(&mut self, records: Vec<(String, f64, u64, u64)>) {
        for (name, score, successes, failures) in records {
            self.workers.insert(name.clone(), WorkerTrust {
                name: name.clone(),
                score,
                successes,
                failures,
                last_success: None,
                last_failure: None,
                last_updated: Utc::now(),
            });
        }
    }

    /// Should this worker be allowed to run? (trust gate)
    pub fn should_run(&self, worker: &str, threshold: f64) -> bool {
        self.get_trust(worker) >= threshold
    }

    /// Get average trust across all workers
    pub fn average_trust(&self) -> f64 {
        if self.workers.is_empty() {
            return DEFAULT_TRUST;
        }
        let sum: f64 = self.workers.values().map(|wt| self.apply_decay(wt)).sum();
        sum / self.workers.len() as f64
    }
}

impl Default for HebbianTrustManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_trust() {
        let tm = HebbianTrustManager::new();
        assert!((tm.get_trust("unknown") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_increases_trust() {
        let mut tm = HebbianTrustManager::new();
        let initial = tm.get_trust("w1");
        tm.record_success("w1", 100);
        assert!(tm.get_trust("w1") > initial);
    }

    #[test]
    fn test_failure_decreases_trust() {
        let mut tm = HebbianTrustManager::new();
        tm.record_success("w1", 100);
        let after_success = tm.get_trust("w1");
        tm.record_failure("w1");
        assert!(tm.get_trust("w1") < after_success);
    }

    #[test]
    fn test_trust_bounds() {
        let mut tm = HebbianTrustManager::new();
        for _ in 0..1000 {
            tm.record_success("w1", 10);
        }
        assert!(tm.get_trust("w1") <= MAX_TRUST);

        for _ in 0..1000 {
            tm.record_failure("w2");
        }
        assert!(tm.get_trust("w2") >= MIN_TRUST);
    }

    #[test]
    fn test_should_run() {
        let mut tm = HebbianTrustManager::new();
        assert!(tm.should_run("w1", 0.4)); // default 0.5 > 0.4
        for _ in 0..20 {
            tm.record_failure("w1");
        }
        assert!(!tm.should_run("w1", 0.4)); // trust dropped below 0.4
    }

    #[test]
    fn test_average_trust() {
        let mut tm = HebbianTrustManager::new();
        tm.record_success("w1", 100);
        tm.record_success("w2", 100);
        let avg = tm.average_trust();
        assert!(avg > DEFAULT_TRUST);
    }
}
