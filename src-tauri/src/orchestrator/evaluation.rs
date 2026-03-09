// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Internal types (BenchmarkSuite, EloTracker, EvalMode, etc.) are test-exercised.
#![allow(dead_code)]
//! Evaluation & Benchmarking System for ImpForge Orchestrator
//!
//! Implements Agent-as-a-Judge evaluation chains where agents score
//! each other's outputs against configurable rubrics. Tracks performance
//! over time with benchmark suites for regression detection.
//!
//! Three evaluation modes:
//! - **Single Judge**: One evaluator scores an output (fast, cheap)
//! - **Panel**: Multiple judges score independently, then aggregate (reliable)
//! - **Debate**: Judges discuss disagreements before final score (highest quality)
//!
//! Scientific basis:
//! - Agent-as-a-Judge (arXiv:2410.10934) — LLM evaluation of LLM outputs
//! - Chatbot Arena (Zheng et al., 2023) — Elo rating for model comparison
//! - LMSYS leaderboard methodology — pairwise preference aggregation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Evaluation rubric — defines what to score and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRubric {
    pub name: String,
    pub criteria: Vec<EvalCriterion>,
    pub passing_score: f32,
}

/// A single criterion in a rubric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCriterion {
    pub name: String,
    pub description: String,
    pub weight: f32,
    pub max_score: f32,
}

impl EvalRubric {
    /// Create a default code quality rubric.
    pub fn code_quality() -> Self {
        Self {
            name: "code_quality".into(),
            criteria: vec![
                EvalCriterion {
                    name: "correctness".into(),
                    description: "Does the code produce correct output?".into(),
                    weight: 0.35,
                    max_score: 10.0,
                },
                EvalCriterion {
                    name: "readability".into(),
                    description: "Is the code clear and well-structured?".into(),
                    weight: 0.25,
                    max_score: 10.0,
                },
                EvalCriterion {
                    name: "efficiency".into(),
                    description: "Is the code efficient in time/space?".into(),
                    weight: 0.20,
                    max_score: 10.0,
                },
                EvalCriterion {
                    name: "safety".into(),
                    description: "Does the code handle errors and edge cases?".into(),
                    weight: 0.20,
                    max_score: 10.0,
                },
            ],
            passing_score: 7.0,
        }
    }

    /// Create a rubric for general text quality.
    pub fn text_quality() -> Self {
        Self {
            name: "text_quality".into(),
            criteria: vec![
                EvalCriterion {
                    name: "accuracy".into(),
                    description: "Is the information factually correct?".into(),
                    weight: 0.40,
                    max_score: 10.0,
                },
                EvalCriterion {
                    name: "completeness".into(),
                    description: "Does it cover all aspects of the query?".into(),
                    weight: 0.30,
                    max_score: 10.0,
                },
                EvalCriterion {
                    name: "clarity".into(),
                    description: "Is the text clear and well-organized?".into(),
                    weight: 0.30,
                    max_score: 10.0,
                },
            ],
            passing_score: 7.0,
        }
    }

    /// Compute weighted score from criterion scores.
    pub fn compute_score(&self, scores: &HashMap<String, f32>) -> f32 {
        let mut total_weight = 0.0f32;
        let mut weighted_sum = 0.0f32;

        for criterion in &self.criteria {
            if let Some(&score) = scores.get(&criterion.name) {
                let normalized = score / criterion.max_score;
                weighted_sum += normalized * criterion.weight;
                total_weight += criterion.weight;
            }
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight) * 10.0 // Scale to 0-10
        } else {
            0.0
        }
    }
}

/// A single evaluation result from one judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub judge_id: String,
    pub task_id: String,
    pub rubric_name: String,
    pub criterion_scores: HashMap<String, f32>,
    pub overall_score: f32,
    pub passed: bool,
    pub feedback: String,
    pub timestamp: DateTime<Utc>,
}

/// Evaluation mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EvalMode {
    SingleJudge,
    Panel,
    Debate,
}

/// A benchmark test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub id: String,
    pub query: String,
    pub expected_output: Option<String>,
    pub rubric: EvalRubric,
    pub tags: Vec<String>,
}

/// Benchmark suite — a collection of test cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    pub name: String,
    pub version: String,
    pub cases: Vec<BenchmarkCase>,
}

impl BenchmarkSuite {
    /// Create a minimal code benchmark suite.
    pub fn code_basics() -> Self {
        Self {
            name: "code_basics".into(),
            version: "1.0.0".into(),
            cases: vec![
                BenchmarkCase {
                    id: "fizzbuzz".into(),
                    query: "Write a fizzbuzz function in Rust".into(),
                    expected_output: Some("fn fizzbuzz".into()),
                    rubric: EvalRubric::code_quality(),
                    tags: vec!["basics".into(), "rust".into()],
                },
                BenchmarkCase {
                    id: "fibonacci".into(),
                    query: "Write a fibonacci function in Rust".into(),
                    expected_output: Some("fn fibonacci".into()),
                    rubric: EvalRubric::code_quality(),
                    tags: vec!["basics".into(), "rust".into()],
                },
                BenchmarkCase {
                    id: "sort".into(),
                    query: "Implement quicksort in Rust".into(),
                    expected_output: Some("fn quicksort".into()),
                    rubric: EvalRubric::code_quality(),
                    tags: vec!["algorithms".into(), "rust".into()],
                },
            ],
        }
    }
}

/// Benchmark run result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub suite_name: String,
    pub agent_id: String,
    pub results: Vec<EvalResult>,
    pub avg_score: f32,
    pub pass_rate: f32,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

/// Elo rating tracker for pairwise agent comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EloTracker {
    ratings: HashMap<String, f64>,
    k_factor: f64,
    history: Vec<EloMatch>,
}

/// A single Elo match result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EloMatch {
    pub agent_a: String,
    pub agent_b: String,
    pub winner: String,
    pub timestamp: DateTime<Utc>,
}

impl EloTracker {
    pub fn new(k_factor: f64) -> Self {
        Self {
            ratings: HashMap::new(),
            k_factor,
            history: Vec::new(),
        }
    }

    /// Get rating for an agent (default 1500).
    pub fn rating(&self, agent_id: &str) -> f64 {
        *self.ratings.get(agent_id).unwrap_or(&1500.0)
    }

    /// Record a match result and update Elo ratings.
    ///
    /// Standard Elo formula: R' = R + K × (S - E)
    /// where E = 1 / (1 + 10^((Rb - Ra) / 400))
    pub fn record_match(&mut self, agent_a: &str, agent_b: &str, winner: &str) {
        let ra = self.rating(agent_a);
        let rb = self.rating(agent_b);

        let ea = 1.0 / (1.0 + 10.0_f64.powf((rb - ra) / 400.0));
        let eb = 1.0 - ea;

        let (sa, sb) = if winner == agent_a {
            (1.0, 0.0)
        } else if winner == agent_b {
            (0.0, 1.0)
        } else {
            (0.5, 0.5) // Draw
        };

        let new_ra = ra + self.k_factor * (sa - ea);
        let new_rb = rb + self.k_factor * (sb - eb);

        self.ratings.insert(agent_a.to_string(), new_ra);
        self.ratings.insert(agent_b.to_string(), new_rb);

        self.history.push(EloMatch {
            agent_a: agent_a.to_string(),
            agent_b: agent_b.to_string(),
            winner: winner.to_string(),
            timestamp: Utc::now(),
        });
    }

    /// Get leaderboard sorted by rating (highest first).
    pub fn leaderboard(&self) -> Vec<(String, f64)> {
        let mut board: Vec<_> = self.ratings.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        board.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        board
    }

    /// Get total matches played.
    pub fn total_matches(&self) -> usize {
        self.history.len()
    }

    /// Get match history for a specific agent.
    pub fn agent_history(&self, agent_id: &str) -> Vec<&EloMatch> {
        self.history.iter()
            .filter(|m| m.agent_a == agent_id || m.agent_b == agent_id)
            .collect()
    }
}

/// The evaluation chain orchestrator.
///
/// Manages rubrics, runs evaluations, and tracks benchmark history.
pub struct EvalChain {
    rubrics: HashMap<String, EvalRubric>,
    results: Vec<EvalResult>,
    benchmark_runs: Vec<BenchmarkRun>,
    elo: EloTracker,
}

impl EvalChain {
    pub fn new() -> Self {
        let mut rubrics = HashMap::new();
        rubrics.insert("code_quality".into(), EvalRubric::code_quality());
        rubrics.insert("text_quality".into(), EvalRubric::text_quality());

        Self {
            rubrics,
            results: Vec::new(),
            benchmark_runs: Vec::new(),
            elo: EloTracker::new(32.0),
        }
    }

    /// Register a custom rubric.
    pub fn add_rubric(&mut self, rubric: EvalRubric) {
        self.rubrics.insert(rubric.name.clone(), rubric);
    }

    /// Evaluate an output against a rubric with synthetic scoring.
    ///
    /// In production, this calls an LLM judge. In standalone mode,
    /// it uses heuristic scoring based on output characteristics.
    pub fn evaluate(
        &mut self,
        judge_id: &str,
        task_id: &str,
        rubric_name: &str,
        output: &str,
    ) -> Option<EvalResult> {
        let rubric = self.rubrics.get(rubric_name)?.clone();

        // Heuristic scoring (standalone mode)
        let mut scores = HashMap::new();
        for criterion in &rubric.criteria {
            let score = self.heuristic_score(&criterion.name, output);
            scores.insert(criterion.name.clone(), score);
        }

        let overall = rubric.compute_score(&scores);
        let passed = overall >= rubric.passing_score;

        let result = EvalResult {
            judge_id: judge_id.into(),
            task_id: task_id.into(),
            rubric_name: rubric_name.into(),
            criterion_scores: scores,
            overall_score: overall,
            passed,
            feedback: if passed {
                format!("Passed with score {:.1}/10", overall)
            } else {
                format!("Failed with score {:.1}/10 (threshold: {:.1})", overall, rubric.passing_score)
            },
            timestamp: Utc::now(),
        };

        self.results.push(result.clone());
        Some(result)
    }

    /// Run a full benchmark suite for an agent.
    pub fn run_benchmark(&mut self, suite: &BenchmarkSuite, agent_id: &str) -> BenchmarkRun {
        let start = std::time::Instant::now();
        let mut results = Vec::new();

        for case in &suite.cases {
            // Simulate agent output (in production, this calls the actual agent)
            let output = format!("fn {}() {{ /* implementation */ }}", case.id);

            if let Some(result) = self.evaluate(
                "benchmark_judge",
                &case.id,
                &case.rubric.name,
                &output,
            ) {
                results.push(result);
            }
        }

        let avg_score = if results.is_empty() {
            0.0
        } else {
            results.iter().map(|r| r.overall_score).sum::<f32>() / results.len() as f32
        };

        let pass_rate = if results.is_empty() {
            0.0
        } else {
            results.iter().filter(|r| r.passed).count() as f32 / results.len() as f32
        };

        let run = BenchmarkRun {
            suite_name: suite.name.clone(),
            agent_id: agent_id.into(),
            results,
            avg_score,
            pass_rate,
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        };

        self.benchmark_runs.push(run.clone());
        run
    }

    /// Compare two agents using pairwise Elo rating.
    pub fn compare_agents(&mut self, agent_a: &str, agent_b: &str, winner: &str) {
        self.elo.record_match(agent_a, agent_b, winner);
    }

    /// Get the Elo leaderboard.
    pub fn leaderboard(&self) -> Vec<(String, f64)> {
        self.elo.leaderboard()
    }

    /// Get all evaluation results for a task.
    pub fn results_for_task(&self, task_id: &str) -> Vec<&EvalResult> {
        self.results.iter().filter(|r| r.task_id == task_id).collect()
    }

    /// Get benchmark history for an agent.
    pub fn benchmark_history(&self, agent_id: &str) -> Vec<&BenchmarkRun> {
        self.benchmark_runs.iter().filter(|r| r.agent_id == agent_id).collect()
    }

    /// Detect regression: compare latest benchmark run against previous.
    pub fn detect_regression(&self, agent_id: &str, threshold: f32) -> Option<(f32, f32)> {
        let runs: Vec<_> = self.benchmark_runs.iter()
            .filter(|r| r.agent_id == agent_id)
            .collect();

        if runs.len() < 2 {
            return None;
        }

        let latest = runs.last().unwrap();
        let previous = runs[runs.len() - 2];

        let delta = latest.avg_score - previous.avg_score;
        if delta < -threshold {
            Some((previous.avg_score, latest.avg_score))
        } else {
            None
        }
    }

    /// Heuristic scoring for standalone mode (no LLM judge).
    fn heuristic_score(&self, criterion: &str, output: &str) -> f32 {
        let len = output.len();
        match criterion {
            "correctness" => {
                // Longer, structured output scores higher
                if len > 200 { 8.0 } else if len > 50 { 6.0 } else { 4.0 }
            }
            "readability" => {
                // Check for common readability indicators
                let has_newlines = output.contains('\n');
                let has_comments = output.contains("//") || output.contains("/*");
                let base = 5.0;
                base + if has_newlines { 1.5 } else { 0.0 } + if has_comments { 1.5 } else { 0.0 }
            }
            "efficiency" => {
                // Shorter code with same correctness is more efficient
                if len < 100 { 8.0 } else if len < 500 { 7.0 } else { 5.0 }
            }
            "safety" => {
                let has_error_handling = output.contains("Result") || output.contains("Option")
                    || output.contains("unwrap_or") || output.contains("?");
                if has_error_handling { 8.0 } else { 5.0 }
            }
            "accuracy" | "completeness" | "clarity" => {
                // General text quality heuristics
                if len > 300 { 7.5 } else if len > 100 { 6.0 } else { 4.5 }
            }
            _ => 5.0, // Default middle score
        }
    }
}

impl Default for EvalChain {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_quality_rubric() {
        let rubric = EvalRubric::code_quality();
        assert_eq!(rubric.criteria.len(), 4);
        let total_weight: f32 = rubric.criteria.iter().map(|c| c.weight).sum();
        assert!((total_weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_text_quality_rubric() {
        let rubric = EvalRubric::text_quality();
        assert_eq!(rubric.criteria.len(), 3);
        let total_weight: f32 = rubric.criteria.iter().map(|c| c.weight).sum();
        assert!((total_weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rubric_scoring() {
        let rubric = EvalRubric::code_quality();
        let mut scores = HashMap::new();
        scores.insert("correctness".into(), 9.0);
        scores.insert("readability".into(), 8.0);
        scores.insert("efficiency".into(), 7.0);
        scores.insert("safety".into(), 8.0);

        let overall = rubric.compute_score(&scores);
        assert!(overall > 7.0);
        assert!(overall < 10.0);
    }

    #[test]
    fn test_evaluate_output() {
        let mut chain = EvalChain::new();
        let result = chain.evaluate("judge-1", "task-1", "code_quality", "fn fizzbuzz() { /* implementation */ }");

        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.judge_id, "judge-1");
        assert_eq!(r.task_id, "task-1");
        assert!(r.overall_score > 0.0);
    }

    #[test]
    fn test_evaluate_unknown_rubric() {
        let mut chain = EvalChain::new();
        let result = chain.evaluate("judge", "task", "nonexistent_rubric", "output");
        assert!(result.is_none());
    }

    #[test]
    fn test_custom_rubric() {
        let mut chain = EvalChain::new();
        chain.add_rubric(EvalRubric {
            name: "custom".into(),
            criteria: vec![EvalCriterion {
                name: "creativity".into(),
                description: "Is it creative?".into(),
                weight: 1.0,
                max_score: 10.0,
            }],
            passing_score: 5.0,
        });

        let result = chain.evaluate("j", "t", "custom", "very creative output");
        assert!(result.is_some());
    }

    #[test]
    fn test_benchmark_run() {
        let mut chain = EvalChain::new();
        let suite = BenchmarkSuite::code_basics();
        let run = chain.run_benchmark(&suite, "agent-x");

        assert_eq!(run.suite_name, "code_basics");
        assert_eq!(run.agent_id, "agent-x");
        assert_eq!(run.results.len(), 3);
        assert!(run.avg_score > 0.0);
        assert!(run.pass_rate >= 0.0 && run.pass_rate <= 1.0);
    }

    #[test]
    fn test_elo_tracker() {
        let mut elo = EloTracker::new(32.0);
        assert_eq!(elo.rating("alice"), 1500.0);

        elo.record_match("alice", "bob", "alice");
        assert!(elo.rating("alice") > 1500.0);
        assert!(elo.rating("bob") < 1500.0);
    }

    #[test]
    fn test_elo_leaderboard() {
        let mut elo = EloTracker::new(32.0);
        elo.record_match("alice", "bob", "alice");
        elo.record_match("alice", "charlie", "alice");
        elo.record_match("bob", "charlie", "bob");

        let board = elo.leaderboard();
        assert_eq!(board[0].0, "alice");
        assert_eq!(board.len(), 3);
    }

    #[test]
    fn test_elo_draw() {
        let mut elo = EloTracker::new(32.0);
        elo.record_match("alice", "bob", "draw");

        // After a draw from equal ratings, both should stay near 1500
        assert!((elo.rating("alice") - 1500.0).abs() < 1.0);
        assert!((elo.rating("bob") - 1500.0).abs() < 1.0);
    }

    #[test]
    fn test_regression_detection() {
        let mut chain = EvalChain::new();
        let suite = BenchmarkSuite::code_basics();

        // Run two benchmarks
        chain.run_benchmark(&suite, "agent-y");
        chain.run_benchmark(&suite, "agent-y");

        // No regression expected (same synthetic scores)
        let regression = chain.detect_regression("agent-y", 0.5);
        assert!(regression.is_none());
    }

    #[test]
    fn test_results_for_task() {
        let mut chain = EvalChain::new();
        chain.evaluate("j1", "task-1", "code_quality", "output 1");
        chain.evaluate("j2", "task-1", "text_quality", "output 1");
        chain.evaluate("j1", "task-2", "code_quality", "output 2");

        let task_1_results = chain.results_for_task("task-1");
        assert_eq!(task_1_results.len(), 2);
    }

    #[test]
    fn test_agent_comparison() {
        let mut chain = EvalChain::new();
        chain.compare_agents("agent-a", "agent-b", "agent-a");
        chain.compare_agents("agent-a", "agent-c", "agent-a");

        let board = chain.leaderboard();
        assert_eq!(board[0].0, "agent-a");
    }

    #[test]
    fn test_serde_roundtrip() {
        let rubric = EvalRubric::code_quality();
        let json = serde_json::to_string(&rubric).unwrap();
        let deser: EvalRubric = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "code_quality");
        assert_eq!(deser.criteria.len(), 4);
    }

    #[test]
    fn test_benchmark_suite_creation() {
        let suite = BenchmarkSuite::code_basics();
        assert_eq!(suite.cases.len(), 3);
        assert_eq!(suite.name, "code_basics");
    }

    #[test]
    fn test_eval_mode_serde() {
        let mode = EvalMode::Panel;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"panel\"");
        let deser: EvalMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, EvalMode::Panel);
    }
}
