// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Three-Factor Hebbian/STDP Trust Scoring for ImpForge Standalone Orchestrator
//!
//! Extends classical STDP with a neuromodulatory third factor:
//! - Success within expected time → trust increases (LTP)
//! - Failure or timeout → trust decreases (LTD)
//! - Exponential decay over time (biological synapse weakening)
//! - **Three-Factor**: Dopamine × Novelty × Homeostasis gates plasticity
//!
//! The key equation: Δw = η × STDP(Δt) × M(t)
//! where M(t) = dopamine(reward) × novelty(runs) × homeostasis(avg_trust)
//!
//! References:
//! - Bi & Poo (1998): "Synaptic Modifications in Cultured Hippocampal Neurons"
//! - Dan & Poo (2004): "Spike Timing-Dependent Plasticity of Neural Circuits"
//! - Izhikevich (2007): "Solving the Distal Reward Problem" — dopamine-gated STDP
//! - Markram et al. (2012): "A History of Spike-Timing-Dependent Plasticity"
//! - Gerstner et al. (2018): Three-factor learning rules (Annual Rev Neurosci)
//! - ArXiv 2504.05341: Three-Factor Hebbian Learning for AI agents

#[cfg(not(feature = "engine"))]
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default trust score for new/unknown workers.
const DEFAULT_TRUST: f64 = 0.5;

// The remaining STDP/Hebbian constants and the full HebbianTrustManager
// implementation are only compiled for the community edition.  When the
// `engine` feature is active, `impforge_engine::trust::HebbianTrustManager`
// is used instead (see orchestrator/mod.rs conditional imports).

#[cfg(not(feature = "engine"))]
const MIN_TRUST: f64 = 0.1;
#[cfg(not(feature = "engine"))]
const MAX_TRUST: f64 = 1.0;

/// LTP (Long-Term Potentiation) — how much trust increases on success
#[cfg(not(feature = "engine"))]
const A_PLUS: f64 = 0.08;
/// LTD (Long-Term Depression) — how much trust decreases on failure
#[cfg(not(feature = "engine"))]
const A_MINUS: f64 = 0.12;
/// Time constant for STDP window (seconds)
#[cfg(not(feature = "engine"))]
const TAU_STDP: f64 = 3600.0;
/// Decay half-life (seconds) — trust decays if worker is idle
#[cfg(not(feature = "engine"))]
const DECAY_HALF_LIFE: f64 = 86400.0; // 24 hours

// Three-Factor Neuromodulatory Constants
// Reference: Izhikevich (2007), Gerstner et al. (2018), ArXiv 2504.05341
#[cfg(not(feature = "engine"))]
const DOPAMINE_REWARD: f64 = 1.5;       // Amplifies LTP on global success
#[cfg(not(feature = "engine"))]
const DOPAMINE_PUNISHMENT: f64 = 0.3;   // Amplifies LTD on global failure
#[cfg(not(feature = "engine"))]
const DOPAMINE_BASELINE: f64 = 1.0;     // Neutral — standard STDP behavior
#[cfg(not(feature = "engine"))]
const NOVELTY_BONUS: f64 = 1.4;         // Exploration bonus for rare workers
#[cfg(not(feature = "engine"))]
const NOVELTY_DECAY_RUNS: f64 = 20.0;   // After N runs, novelty → 1.0
#[cfg(not(feature = "engine"))]
const HOMEOSTASIS_TARGET: f64 = 0.6;    // Target average trust
#[cfg(not(feature = "engine"))]
const HOMEOSTASIS_STRENGTH: f64 = 0.02; // How strongly homeostasis pulls

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

/// Neuromodulatory third factor for gated plasticity (Rust standalone).
///
/// M(t) = dopamine(reward) × novelty(runs) × homeostasis(avg_trust)
///
/// This is the STANDALONE ImpForge implementation — completely independent.
/// No external dependencies on any other system or service.
#[cfg(not(feature = "engine"))]
struct ThreeFactorModulator {
    global_reward: f64,
    reward_history: Vec<(f64, DateTime<Utc>)>, // (reward, timestamp)
}

#[cfg(not(feature = "engine"))]
impl ThreeFactorModulator {
    fn new() -> Self {
        Self {
            global_reward: DOPAMINE_BASELINE,
            reward_history: Vec::new(),
        }
    }

    /// Compute combined neuromodulatory factor M(t).
    /// Returns a multiplier [0.1, 3.0] that gates STDP plasticity.
    fn compute_modulation(&self, wt: &WorkerTrust, avg_trust: f64, is_success: bool) -> f64 {
        let dopamine = self.compute_dopamine(is_success);
        let novelty = Self::compute_novelty(wt);
        let homeostasis = Self::compute_homeostasis(avg_trust, is_success);

        let modulation = dopamine * novelty * homeostasis;
        modulation.clamp(0.1, 3.0)
    }

    /// Update global reward signal from system-wide outcomes.
    fn record_global_outcome(&mut self, success: bool) {
        let now = Utc::now();
        let reward = if success { 1.0 } else { -1.0 };
        self.reward_history.push((reward, now));

        // Keep last 5 minutes
        let cutoff = now - chrono::Duration::seconds(300);
        self.reward_history.retain(|(_, t)| *t > cutoff);

        if !self.reward_history.is_empty() {
            let avg: f64 = self.reward_history.iter().map(|(r, _)| r).sum::<f64>()
                / self.reward_history.len() as f64;
            self.global_reward = DOPAMINE_BASELINE + avg * 0.3;
        }
    }

    /// Dopamine: reward-gated plasticity.
    fn compute_dopamine(&self, is_success: bool) -> f64 {
        if is_success {
            (self.global_reward * DOPAMINE_REWARD).max(DOPAMINE_BASELINE)
        } else {
            (self.global_reward * DOPAMINE_PUNISHMENT).min(DOPAMINE_BASELINE)
        }
    }

    /// Novelty: exploration bonus for underused workers.
    fn compute_novelty(wt: &WorkerTrust) -> f64 {
        let total = wt.successes + wt.failures;
        if total == 0 {
            return NOVELTY_BONUS;
        }
        1.0 + (NOVELTY_BONUS - 1.0) * (-(total as f64) / NOVELTY_DECAY_RUNS).exp()
    }

    /// Homeostasis: prevents runaway trust scores.
    fn compute_homeostasis(avg_trust: f64, is_success: bool) -> f64 {
        let deviation = avg_trust - HOMEOSTASIS_TARGET;
        if is_success && deviation > 0.0 {
            (1.0 - deviation * HOMEOSTASIS_STRENGTH * 10.0).max(0.5)
        } else if !is_success && deviation < 0.0 {
            (1.0 + deviation * HOMEOSTASIS_STRENGTH * 10.0).max(0.5)
        } else {
            1.0
        }
    }
}

/// Three-Factor Hebbian Trust Manager — manages trust scores for all workers
#[cfg(not(feature = "engine"))]
pub struct HebbianTrustManager {
    workers: HashMap<String, WorkerTrust>,
    modulator: ThreeFactorModulator,
}

#[cfg(not(feature = "engine"))]
impl HebbianTrustManager {
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
            modulator: ThreeFactorModulator::new(),
        }
    }

    /// Get current trust score for a worker (with time-based decay applied)
    pub fn get_trust(&self, worker: &str) -> f64 {
        match self.workers.get(worker) {
            Some(wt) => self.apply_decay(wt),
            None => DEFAULT_TRUST,
        }
    }

    /// Get full trust state for a worker.
    /// Used by module tests; worker_trust_detail uses TrustScorer::get_score.
    #[allow(dead_code)]
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
    /// Three-Factor STDP: Δw = A+ × exp(-Δt / τ) × speed_bonus × M(t)
    /// where M(t) = dopamine × novelty × homeostasis (third factor)
    pub fn record_success(&mut self, worker: &str, duration_ms: u64) {
        let avg = self.average_trust();

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

        // Three-factor modulation: gates the plasticity update
        let modulation = self.modulator.compute_modulation(wt, avg, true);
        self.modulator.record_global_outcome(true);

        wt.score = (wt.score + stdp_factor * speed_bonus * modulation).min(MAX_TRUST);
        wt.last_success = Some(now);
        wt.last_updated = now;
    }

    /// Record a failed task execution
    ///
    /// Three-Factor STDP: Δw = -A- × exp(-Δt / τ) × M(t)
    /// where M(t) modulates the depression strength
    pub fn record_failure(&mut self, worker: &str) {
        let avg = self.average_trust();

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

        // Three-factor modulation
        let modulation = self.modulator.compute_modulation(wt, avg, false);
        self.modulator.record_global_outcome(false);

        wt.score = (wt.score - stdp_factor * modulation).max(MIN_TRUST);
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

#[cfg(not(feature = "engine"))]
impl Default for HebbianTrustManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, not(feature = "engine")))]
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

    // Three-Factor Hebbian Tests

    #[test]
    fn test_three_factor_novelty_bonus() {
        // New workers should get a novelty bonus (higher trust increase)
        let mut tm1 = HebbianTrustManager::new();
        let mut tm2 = HebbianTrustManager::new();

        // First worker: brand new (novelty bonus applies)
        tm1.record_success("new_worker", 100);
        let new_trust = tm1.get_trust("new_worker");

        // Second worker: has 50 prior runs (novelty decayed)
        let wt = tm2.workers.entry("veteran".to_string())
            .or_insert_with(|| WorkerTrust::new("veteran"));
        wt.successes = 50;
        wt.failures = 0;
        let veteran_base = wt.score;
        tm2.record_success("veteran", 100);
        let veteran_delta = tm2.get_trust("veteran") - veteran_base;
        let new_delta = new_trust - DEFAULT_TRUST;

        // New worker should gain MORE trust per success than veteran
        assert!(new_delta > veteran_delta, "Novelty bonus not applied");
    }

    #[test]
    fn test_three_factor_modulation_bounded() {
        let mod_ = ThreeFactorModulator::new();
        let wt = WorkerTrust::new("test");

        // Modulation should always be in [0.1, 3.0]
        let m1 = mod_.compute_modulation(&wt, 0.0, true);
        assert!(m1 >= 0.1 && m1 <= 3.0);

        let m2 = mod_.compute_modulation(&wt, 1.0, false);
        assert!(m2 >= 0.1 && m2 <= 3.0);

        let m3 = mod_.compute_modulation(&wt, 0.5, true);
        assert!(m3 >= 0.1 && m3 <= 3.0);
    }

    #[test]
    fn test_three_factor_homeostasis() {
        // When avg_trust is high and success → homeostasis dampens LTP
        let m_high = ThreeFactorModulator::compute_homeostasis(0.95, true);
        assert!(m_high < 1.0, "Homeostasis should dampen when avg trust is high");

        // When avg_trust is at target → homeostasis is neutral
        let m_target = ThreeFactorModulator::compute_homeostasis(HOMEOSTASIS_TARGET, true);
        assert!((m_target - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_three_factor_dopamine_reward_history() {
        let mut mod_ = ThreeFactorModulator::new();

        // Many successes → global reward rises
        for _ in 0..10 {
            mod_.record_global_outcome(true);
        }
        assert!(mod_.global_reward > DOPAMINE_BASELINE);

        // Many failures → global reward drops
        for _ in 0..20 {
            mod_.record_global_outcome(false);
        }
        assert!(mod_.global_reward < DOPAMINE_BASELINE);
    }
}
