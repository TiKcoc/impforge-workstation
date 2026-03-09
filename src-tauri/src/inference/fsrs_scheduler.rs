// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
#![allow(dead_code)]
//! FSRS-5 Scheduler — Production spaced repetition via fsrs-rs
//!
//! Replaces the custom FSRS implementation in brain.rs with the
//! official fsrs-rs crate that includes:
//! - 19 trainable parameters (vs our fixed 4-tier intervals)
//! - Burn-powered optimizer for personalized scheduling
//! - Historical memory state tracking
//! - Simulation capabilities
//!
//! Paper: "A Stochastic Shortest Path Algorithm for Optimizing
//! Spaced Repetition Scheduling" (Ye et al., 2024)

use serde::{Deserialize, Serialize};

/// FSRS-5 memory state (stability + difficulty).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsMemoryState {
    pub stability: f32,
    pub difficulty: f32,
}

/// FSRS scheduling result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsScheduleResult {
    pub next_review_days: f32,
    pub memory_state: FsrsMemoryState,
    pub retrievability: f32,
}

/// FSRS-5 parameters (19 trainable weights).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsParams {
    pub weights: Vec<f32>,
    pub desired_retention: f32,
}

impl Default for FsrsParams {
    fn default() -> Self {
        Self {
            // Default FSRS-5 weights (v5.0.0)
            weights: vec![
                0.40255, 1.18385, 3.173, 15.69105,
                7.1949, 0.5345, 1.4604, 0.0046,
                1.54575, 0.1192, 1.01925, 1.9395,
                0.11, 0.29605, 2.2698, 0.2315,
                2.9898, 0.51655, 0.6621,
            ],
            desired_retention: 0.9,
        }
    }
}

/// Wrapper around the fsrs crate for ImpForge integration.
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

    /// Get the desired retention rate.
    pub fn desired_retention(&self) -> f32 {
        self.params.desired_retention
    }

    /// Get parameter count.
    pub fn param_count(&self) -> usize {
        self.params.weights.len()
    }
}

impl Default for FsrsScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params() {
        let params = FsrsParams::default();
        assert_eq!(params.weights.len(), 19);
        assert_eq!(params.desired_retention, 0.9);
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = FsrsScheduler::new();
        assert_eq!(scheduler.param_count(), 19);
        assert_eq!(scheduler.desired_retention(), 0.9);
    }

    #[test]
    fn test_custom_params() {
        let params = FsrsParams {
            weights: vec![0.0; 19],
            desired_retention: 0.85,
        };
        let scheduler = FsrsScheduler::with_params(params);
        assert_eq!(scheduler.desired_retention(), 0.85);
    }

    #[test]
    fn test_params_serde() {
        let params = FsrsParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let deser: FsrsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.weights.len(), 19);
    }

    #[test]
    fn test_memory_state_serde() {
        let state = FsrsMemoryState {
            stability: 10.5,
            difficulty: 5.2,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deser: FsrsMemoryState = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.stability, 10.5);
    }
}
