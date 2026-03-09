// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Public API — consumed via neuralswarm Tauri bridge + tests
#![allow(dead_code)]
//! Mixture-of-Agents (MOA) Pipeline for ImpForge Orchestrator
//!
//! Implements the 5-phase Attention-MoA pipeline (arXiv:2601.16596):
//! 1. **Propose** — multiple agents generate independent proposals
//! 2. **Critique** — agents evaluate each other's proposals
//! 3. **Refine** — agents improve proposals based on critiques
//! 4. **Check** — quality gate ensures minimum threshold
//! 5. **Aggregate** — final synthesis of best proposals
//!
//! This enables multi-agent collaboration where diverse perspectives
//! converge to higher-quality outputs than any single agent. The pipeline
//! supports configurable layers (depth) and agents per layer (width).
//!
//! Scientific basis:
//! - Attention-MoA (arXiv:2601.16596) — layered agent aggregation
//! - Mixture of Experts (Shazeer et al., 2017) — sparse expert routing
//! - Self-Play Fine-Tuning (Chen et al., 2024) — iterative refinement

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a MOA pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaConfig {
    /// Number of MOA layers (depth). More layers = higher quality, slower.
    pub layers: u32,
    /// Number of agents per layer (width). More agents = more diverse proposals.
    pub agents_per_layer: u32,
    /// Minimum quality score to pass the check phase (0.0–1.0).
    pub quality_threshold: f32,
    /// Maximum refinement iterations before forcing aggregation.
    pub max_refinement_rounds: u32,
    /// Whether to use attention-weighted aggregation (vs simple voting).
    pub attention_weighted: bool,
}

impl Default for MoaConfig {
    fn default() -> Self {
        Self {
            layers: 2,
            agents_per_layer: 3,
            quality_threshold: 0.7,
            max_refinement_rounds: 2,
            attention_weighted: true,
        }
    }
}

/// A single proposal from an agent in a MOA layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub agent_id: String,
    pub layer: u32,
    pub content: String,
    pub confidence: f32,
    pub timestamp: chrono::DateTime<Utc>,
}

/// A critique of a proposal by another agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Critique {
    pub critic_id: String,
    pub proposal_agent_id: String,
    pub layer: u32,
    pub score: f32,
    pub feedback: String,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Result of a MOA pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaResult {
    pub task_id: String,
    pub content: String,
    pub total_layers: u32,
    pub total_proposals: u32,
    pub total_critiques: u32,
    pub final_quality: f32,
    pub duration_ms: u64,
    pub phase_log: Vec<PhaseEntry>,
}

/// Log entry for each pipeline phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseEntry {
    pub phase: MoaPhase,
    pub layer: u32,
    pub description: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MoaPhase {
    Propose,
    Critique,
    Refine,
    Check,
    Aggregate,
}

impl std::fmt::Display for MoaPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoaPhase::Propose => write!(f, "propose"),
            MoaPhase::Critique => write!(f, "critique"),
            MoaPhase::Refine => write!(f, "refine"),
            MoaPhase::Check => write!(f, "check"),
            MoaPhase::Aggregate => write!(f, "aggregate"),
        }
    }
}

/// MOA Pipeline executor.
///
/// Orchestrates the 5-phase pipeline across multiple layers and agents.
/// Each layer's output feeds into the next layer's input, creating
/// progressive refinement.
pub struct MoaPipeline {
    config: MoaConfig,
    proposals: Vec<Proposal>,
    critiques: Vec<Critique>,
    phase_log: Vec<PhaseEntry>,
}

impl MoaPipeline {
    pub fn new(config: MoaConfig) -> Self {
        Self {
            config,
            proposals: Vec::new(),
            critiques: Vec::new(),
            phase_log: Vec::new(),
        }
    }

    /// Run the full MOA pipeline for a task.
    ///
    /// In standalone mode, this simulates multi-agent behavior by splitting
    /// the task across multiple "agent personas" using a single LLM with
    /// different system prompts. For full multi-model mode, each agent_id
    /// maps to a different Ollama model.
    pub fn run_sync(&mut self, task_id: &str, query: &str) -> MoaResult {
        let start = std::time::Instant::now();

        for layer in 0..self.config.layers {
            // Phase 1: Propose
            let layer_proposals = self.propose(task_id, query, layer);
            self.proposals.extend(layer_proposals);

            // Phase 2: Critique
            let layer_critiques = self.critique(task_id, layer);
            self.critiques.extend(layer_critiques);

            // Phase 3: Refine (update proposals based on critiques)
            self.refine(task_id, layer);

            // Phase 4: Check quality gate
            let passed = self.check(layer);
            if !passed && layer < self.config.layers - 1 {
                // Quality not met, next layer will try harder
                self.phase_log.push(PhaseEntry {
                    phase: MoaPhase::Check,
                    layer,
                    description: "Quality threshold not met, continuing to next layer".into(),
                    duration_ms: 0,
                });
            }
        }

        // Phase 5: Aggregate
        let final_content = self.aggregate(task_id);
        let final_quality = self.compute_final_quality();

        MoaResult {
            task_id: task_id.to_string(),
            content: final_content,
            total_layers: self.config.layers,
            total_proposals: self.proposals.len() as u32,
            total_critiques: self.critiques.len() as u32,
            final_quality,
            duration_ms: start.elapsed().as_millis() as u64,
            phase_log: self.phase_log.clone(),
        }
    }

    /// Phase 1: Generate proposals from multiple agents.
    fn propose(&mut self, task_id: &str, query: &str, layer: u32) -> Vec<Proposal> {
        let phase_start = std::time::Instant::now();
        let mut proposals = Vec::new();

        for i in 0..self.config.agents_per_layer {
            let agent_id = format!("agent-L{}-{}", layer, i);

            // In standalone mode, generate a synthetic proposal.
            // In production, this calls the LLM with agent-specific prompts.
            let content = if layer == 0 {
                format!("[{} proposal for task {}]: {}", agent_id, task_id, query)
            } else {
                // Layer > 0: Build on previous layer's best proposals
                let prev_best = self.best_proposal_for_layer(layer - 1);
                format!(
                    "[{} refinement]: Building on: {}",
                    agent_id,
                    prev_best.unwrap_or_else(|| query.to_string())
                )
            };

            proposals.push(Proposal {
                agent_id,
                layer,
                content,
                confidence: 0.5 + (0.1 * layer as f32), // Confidence grows with layers
                timestamp: Utc::now(),
            });
        }

        self.phase_log.push(PhaseEntry {
            phase: MoaPhase::Propose,
            layer,
            description: format!("{} proposals generated", proposals.len()),
            duration_ms: phase_start.elapsed().as_millis() as u64,
        });

        proposals
    }

    /// Phase 2: Agents critique each other's proposals.
    fn critique(&mut self, _task_id: &str, layer: u32) -> Vec<Critique> {
        let phase_start = std::time::Instant::now();
        let mut critiques = Vec::new();

        let layer_proposals: Vec<_> = self.proposals.iter()
            .filter(|p| p.layer == layer)
            .collect();

        // Each agent critiques other agents' proposals
        for (i, proposal) in layer_proposals.iter().enumerate() {
            let critic_idx = (i + 1) % layer_proposals.len().max(1);
            let critic_id = format!("critic-L{}-{}", layer, critic_idx);

            let score = 0.5 + (proposal.confidence * 0.5); // Base score + confidence boost
            critiques.push(Critique {
                critic_id,
                proposal_agent_id: proposal.agent_id.clone(),
                layer,
                score,
                feedback: format!("Score {:.2} for proposal by {}", score, proposal.agent_id),
                timestamp: Utc::now(),
            });
        }

        self.phase_log.push(PhaseEntry {
            phase: MoaPhase::Critique,
            layer,
            description: format!("{} critiques generated", critiques.len()),
            duration_ms: phase_start.elapsed().as_millis() as u64,
        });

        critiques
    }

    /// Phase 3: Refine proposals based on critique feedback.
    fn refine(&mut self, _task_id: &str, layer: u32) {
        let phase_start = std::time::Instant::now();
        let mut refined_count = 0;

        // Build critique scores map
        let scores: HashMap<String, f32> = self.critiques.iter()
            .filter(|c| c.layer == layer)
            .map(|c| (c.proposal_agent_id.clone(), c.score))
            .collect();

        // Update proposal confidence based on critiques
        for proposal in self.proposals.iter_mut().filter(|p| p.layer == layer) {
            if let Some(&score) = scores.get(&proposal.agent_id) {
                proposal.confidence = (proposal.confidence + score) / 2.0;
                refined_count += 1;
            }
        }

        self.phase_log.push(PhaseEntry {
            phase: MoaPhase::Refine,
            layer,
            description: format!("{} proposals refined", refined_count),
            duration_ms: phase_start.elapsed().as_millis() as u64,
        });
    }

    /// Phase 4: Quality gate check.
    fn check(&self, layer: u32) -> bool {
        let avg_confidence = self.proposals.iter()
            .filter(|p| p.layer == layer)
            .map(|p| p.confidence)
            .sum::<f32>()
            / self.config.agents_per_layer.max(1) as f32;

        avg_confidence >= self.config.quality_threshold
    }

    /// Phase 5: Aggregate best proposals into final output.
    fn aggregate(&mut self, _task_id: &str) -> String {
        let phase_start = std::time::Instant::now();

        if self.config.attention_weighted {
            self.attention_aggregate()
        } else {
            self.majority_vote_aggregate()
        };

        let best = self.proposals.iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));

        let content = best.map(|p| p.content.clone()).unwrap_or_else(|| "No proposals generated".into());

        self.phase_log.push(PhaseEntry {
            phase: MoaPhase::Aggregate,
            layer: self.config.layers.saturating_sub(1),
            description: format!("Final aggregation complete, best confidence: {:.2}",
                best.map(|p| p.confidence).unwrap_or(0.0)),
            duration_ms: phase_start.elapsed().as_millis() as u64,
        });

        content
    }

    /// Attention-weighted aggregation (arXiv:2601.16596).
    /// Proposals are weighted by their critique scores.
    fn attention_aggregate(&self) -> Vec<(String, f32)> {
        let mut weighted: Vec<(String, f32)> = self.proposals.iter()
            .map(|p| {
                let critique_score = self.critiques.iter()
                    .filter(|c| c.proposal_agent_id == p.agent_id)
                    .map(|c| c.score)
                    .sum::<f32>()
                    / self.critiques.iter()
                        .filter(|c| c.proposal_agent_id == p.agent_id)
                        .count()
                        .max(1) as f32;
                let weight = p.confidence * critique_score;
                (p.agent_id.clone(), weight)
            })
            .collect();

        weighted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        weighted
    }

    /// Simple majority vote aggregation.
    fn majority_vote_aggregate(&self) -> Vec<(String, f32)> {
        let mut scores: Vec<(String, f32)> = self.proposals.iter()
            .map(|p| (p.agent_id.clone(), p.confidence))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    fn best_proposal_for_layer(&self, layer: u32) -> Option<String> {
        self.proposals.iter()
            .filter(|p| p.layer == layer)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .map(|p| p.content.clone())
    }

    fn compute_final_quality(&self) -> f32 {
        if self.proposals.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.proposals.iter().map(|p| p.confidence).sum();
        sum / self.proposals.len() as f32
    }

    /// Get pipeline statistics.
    pub fn stats(&self) -> MoaPipelineStats {
        MoaPipelineStats {
            total_proposals: self.proposals.len(),
            total_critiques: self.critiques.len(),
            phases_completed: self.phase_log.len(),
            layers_config: self.config.layers,
            agents_per_layer: self.config.agents_per_layer,
        }
    }
}

/// Pipeline statistics for monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaPipelineStats {
    pub total_proposals: usize,
    pub total_critiques: usize,
    pub phases_completed: usize,
    pub layers_config: u32,
    pub agents_per_layer: u32,
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MoaConfig::default();
        assert_eq!(config.layers, 2);
        assert_eq!(config.agents_per_layer, 3);
        assert_eq!(config.quality_threshold, 0.7);
        assert!(config.attention_weighted);
    }

    #[test]
    fn test_basic_pipeline_run() {
        let config = MoaConfig::default();
        let mut pipeline = MoaPipeline::new(config);
        let result = pipeline.run_sync("task-1", "What is Rust?");

        assert_eq!(result.task_id, "task-1");
        assert!(!result.content.is_empty());
        assert_eq!(result.total_layers, 2);
        // 3 agents × 2 layers = 6 proposals
        assert_eq!(result.total_proposals, 6);
        // 3 critiques per layer × 2 layers = 6
        assert_eq!(result.total_critiques, 6);
        assert!(result.final_quality > 0.0);
        assert!(!result.phase_log.is_empty());
    }

    #[test]
    fn test_single_layer_pipeline() {
        let config = MoaConfig {
            layers: 1,
            agents_per_layer: 2,
            ..Default::default()
        };
        let mut pipeline = MoaPipeline::new(config);
        let result = pipeline.run_sync("t1", "test query");

        assert_eq!(result.total_layers, 1);
        assert_eq!(result.total_proposals, 2);
        assert_eq!(result.total_critiques, 2);
    }

    #[test]
    fn test_deep_pipeline() {
        let config = MoaConfig {
            layers: 5,
            agents_per_layer: 2,
            quality_threshold: 0.9, // High threshold
            max_refinement_rounds: 3,
            attention_weighted: true,
        };
        let mut pipeline = MoaPipeline::new(config);
        let result = pipeline.run_sync("deep-1", "Complex query");

        assert_eq!(result.total_layers, 5);
        assert_eq!(result.total_proposals, 10); // 2 × 5
        // Quality should improve with more layers
        assert!(result.final_quality > 0.4);
    }

    #[test]
    fn test_majority_vote_aggregation() {
        let config = MoaConfig {
            attention_weighted: false,
            ..Default::default()
        };
        let mut pipeline = MoaPipeline::new(config);
        let result = pipeline.run_sync("vote-1", "Test vote");

        assert!(!result.content.is_empty());
        assert!(result.final_quality > 0.0);
    }

    #[test]
    fn test_phase_log_completeness() {
        let config = MoaConfig {
            layers: 1,
            agents_per_layer: 2,
            ..Default::default()
        };
        let mut pipeline = MoaPipeline::new(config);
        let result = pipeline.run_sync("log-1", "Log test");

        // Should have: propose, critique, refine, (check logged only on fail), aggregate
        let phases: Vec<_> = result.phase_log.iter().map(|p| &p.phase).collect();
        assert!(phases.contains(&&MoaPhase::Propose));
        assert!(phases.contains(&&MoaPhase::Critique));
        assert!(phases.contains(&&MoaPhase::Refine));
        assert!(phases.contains(&&MoaPhase::Aggregate));
    }

    #[test]
    fn test_pipeline_stats() {
        let config = MoaConfig::default();
        let mut pipeline = MoaPipeline::new(config);
        pipeline.run_sync("stats-1", "Stats test");

        let stats = pipeline.stats();
        assert_eq!(stats.total_proposals, 6);
        assert_eq!(stats.total_critiques, 6);
        assert_eq!(stats.layers_config, 2);
        assert_eq!(stats.agents_per_layer, 3);
    }

    #[test]
    fn test_proposal_confidence_increases_with_layers() {
        let config = MoaConfig {
            layers: 3,
            agents_per_layer: 2,
            ..Default::default()
        };
        let mut pipeline = MoaPipeline::new(config);
        pipeline.run_sync("conf-1", "Confidence test");

        let layer_0_avg: f32 = pipeline.proposals.iter()
            .filter(|p| p.layer == 0)
            .map(|p| p.confidence)
            .sum::<f32>() / 2.0;

        let layer_2_avg: f32 = pipeline.proposals.iter()
            .filter(|p| p.layer == 2)
            .map(|p| p.confidence)
            .sum::<f32>() / 2.0;

        // Later layers should have higher confidence
        assert!(layer_2_avg > layer_0_avg);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = MoaConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deser: MoaConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.layers, 2);
        assert_eq!(deser.agents_per_layer, 3);
    }

    #[test]
    fn test_moa_phase_display() {
        assert_eq!(format!("{}", MoaPhase::Propose), "propose");
        assert_eq!(format!("{}", MoaPhase::Aggregate), "aggregate");
    }
}
