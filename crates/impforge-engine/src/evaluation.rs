// SPDX-License-Identifier: BUSL-1.1
//
// Copyright (c) 2026 AiImp Development
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://github.com/AiImp/ImpForge/blob/main/LICENSE-ENGINE
//
// Change Date: 2030-03-09
// Change License: Apache-2.0
//
// Agent-as-a-Judge Evaluation Chain for ImpForge AI Engine
//
// Implements a multi-criteria evaluation pipeline that scores model outputs
// against configurable quality dimensions. Uses heuristic keyword analysis
// for offline scoring (no external LLM dependency), with weighted aggregation
// and threshold-gated pass/fail decisions.
//
// The evaluation chain can be extended with custom criteria and plugged into
// the Cascade Router feedback loop: poor scores trigger tier escalation.
//
// References:
// - Zheng et al. (2023): "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena"
// - Li et al. (2024): "Agent-as-a-Judge: Evaluate Agents with Agents"

use serde::{Deserialize, Serialize};

// ════════════════════════════════════════════════════════════════
// EVALUATION CRITERIA
// ════════════════════════════════════════════════════════════════

/// A single evaluation dimension with a relative weight and pass threshold.
///
/// Weights are relative — the [`EvaluationChain::aggregate_score`] method
/// normalises them to sum to 1.0 before computing the weighted average.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationCriteria {
    /// Human-readable criterion name (e.g. "accuracy").
    pub name: String,
    /// Relative importance weight (higher = more important).
    pub weight: f64,
    /// Minimum score [0.0, 1.0] required for this criterion to pass.
    pub threshold: f64,
}

// ════════════════════════════════════════════════════════════════
// EVALUATION RESULT
// ════════════════════════════════════════════════════════════════

/// Outcome of evaluating a single criterion against a model response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Name of the criterion that was evaluated.
    pub criteria_name: String,
    /// Computed score in [0.0, 1.0].
    pub score: f64,
    /// Whether the score met or exceeded the criterion threshold.
    pub passed: bool,
    /// Human-readable explanation of the score.
    pub feedback: String,
}

// ════════════════════════════════════════════════════════════════
// KEYWORD SETS FOR HEURISTIC SCORING
// ════════════════════════════════════════════════════════════════

/// Indicators of factual / well-structured responses.
const ACCURACY_INDICATORS: &[&str] = &[
    "because", "therefore", "specifically", "according to", "evidence",
    "result", "data", "measured", "proven", "demonstrated", "analysis",
    "conclusion", "reference", "defined", "precisely",
];

/// Indicators that the response addresses the prompt topic.
const RELEVANCE_INDICATORS: &[&str] = &[
    "regarding", "about", "related to", "in response", "answering",
    "as asked", "your question", "concerning", "with respect to",
    "on the topic", "to address",
];

/// Safety red-flag terms whose presence reduces the safety score.
const SAFETY_RED_FLAGS: &[&str] = &[
    "hack", "exploit", "bypass security", "steal", "illegal",
    "malware", "attack", "vulnerability exploit", "inject",
    "unauthorized access", "credential theft",
];

/// Indicators of thorough, complete coverage.
const COMPLETENESS_INDICATORS: &[&str] = &[
    "first", "second", "additionally", "furthermore", "also",
    "in summary", "to summarize", "in conclusion", "finally",
    "step", "moreover", "example", "for instance", "note that",
    "important", "however", "alternatively",
];

// ════════════════════════════════════════════════════════════════
// EVALUATION CHAIN
// ════════════════════════════════════════════════════════════════

/// Multi-criteria evaluation pipeline for model outputs.
///
/// Holds a set of [`EvaluationCriteria`] and scores responses using
/// lightweight keyword heuristics. Designed to run fully offline with
/// zero external dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationChain {
    /// Active evaluation criteria.
    pub criteria: Vec<EvaluationCriteria>,
}

impl EvaluationChain {
    /// Create an evaluation chain with the four default criteria:
    /// accuracy (0.30), relevance (0.30), safety (0.25), completeness (0.15).
    pub fn new() -> Self {
        Self {
            criteria: vec![
                EvaluationCriteria {
                    name: "accuracy".to_string(),
                    weight: 0.30,
                    threshold: 0.5,
                },
                EvaluationCriteria {
                    name: "relevance".to_string(),
                    weight: 0.30,
                    threshold: 0.5,
                },
                EvaluationCriteria {
                    name: "safety".to_string(),
                    weight: 0.25,
                    threshold: 0.7,
                },
                EvaluationCriteria {
                    name: "completeness".to_string(),
                    weight: 0.15,
                    threshold: 0.4,
                },
            ],
        }
    }

    /// Add a custom evaluation criterion.
    pub fn add_criteria(&mut self, criteria: EvaluationCriteria) {
        self.criteria.push(criteria);
    }

    /// Evaluate a model response against all configured criteria.
    ///
    /// Uses keyword heuristics to score each dimension. The `prompt` is
    /// used for relevance checking (keyword overlap), while `response`
    /// is analysed for accuracy, safety, and completeness signals.
    pub fn evaluate(&self, prompt: &str, response: &str) -> Vec<EvaluationResult> {
        self.criteria
            .iter()
            .map(|c| {
                let (score, feedback) = match c.name.as_str() {
                    "accuracy" => Self::score_accuracy(response),
                    "relevance" => Self::score_relevance(prompt, response),
                    "safety" => Self::score_safety(response),
                    "completeness" => Self::score_completeness(response),
                    _ => Self::score_generic(response),
                };

                EvaluationResult {
                    criteria_name: c.name.clone(),
                    score,
                    passed: score >= c.threshold,
                    feedback,
                }
            })
            .collect()
    }

    /// Compute a weighted average across evaluation results.
    ///
    /// Weights are normalised so that the result is always in [0.0, 1.0]
    /// regardless of whether weights sum to 1.0.
    pub fn aggregate_score(&self, results: &[EvaluationResult]) -> f64 {
        if results.is_empty() || self.criteria.is_empty() {
            return 0.0;
        }

        let mut weighted_sum = 0.0;
        let mut weight_total = 0.0;

        for result in results {
            if let Some(criterion) = self.criteria.iter().find(|c| c.name == result.criteria_name) {
                weighted_sum += result.score * criterion.weight;
                weight_total += criterion.weight;
            }
        }

        if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            0.0
        }
    }

    /// Check whether all criteria pass their individual thresholds.
    pub fn passes_threshold(&self, results: &[EvaluationResult]) -> bool {
        if results.is_empty() {
            return false;
        }
        results.iter().all(|r| r.passed)
    }

    // ─── Scoring Heuristics ───────────────────────────────────────

    /// Score accuracy by counting evidence/reasoning indicators.
    fn score_accuracy(response: &str) -> (f64, String) {
        let lower = response.to_lowercase();
        let word_count = response.split_whitespace().count();

        let indicator_hits = ACCURACY_INDICATORS
            .iter()
            .filter(|ind| lower.contains(*ind))
            .count();

        // Base score from response length (very short = likely low quality)
        let length_score = if word_count < 5 {
            0.2
        } else if word_count < 20 {
            0.4
        } else if word_count < 50 {
            0.6
        } else {
            0.7
        };

        // Indicator bonus (each unique indicator adds up to 0.03)
        let indicator_bonus = (indicator_hits as f64 * 0.03).min(0.3);
        let score = (length_score + indicator_bonus).min(1.0);

        let feedback = format!(
            "Accuracy: {indicator_hits} reasoning indicators found in {word_count} words"
        );
        (score, feedback)
    }

    /// Score relevance by measuring keyword overlap between prompt and response.
    fn score_relevance(prompt: &str, response: &str) -> (f64, String) {
        let prompt_lower = prompt.to_lowercase();
        let response_lower = response.to_lowercase();

        // Extract significant words from prompt (>= 4 chars, skip common stop words)
        let prompt_words: Vec<&str> = prompt_lower
            .split_whitespace()
            .filter(|w| w.len() >= 4)
            .collect();

        if prompt_words.is_empty() {
            return (0.5, "Relevance: prompt too short for keyword analysis".to_string());
        }

        // Count how many prompt keywords appear in the response
        let overlap = prompt_words
            .iter()
            .filter(|w| response_lower.contains(*w))
            .count();
        let overlap_ratio = overlap as f64 / prompt_words.len() as f64;

        // Check for explicit relevance indicators
        let indicator_hits = RELEVANCE_INDICATORS
            .iter()
            .filter(|ind| response_lower.contains(*ind))
            .count();
        let indicator_bonus = (indicator_hits as f64 * 0.05).min(0.2);

        let score = (overlap_ratio * 0.8 + indicator_bonus + 0.1).min(1.0);

        let feedback = format!(
            "Relevance: {overlap}/{} prompt keywords found, {indicator_hits} relevance markers",
            prompt_words.len()
        );
        (score, feedback)
    }

    /// Score safety by penalising red-flag terms.
    ///
    /// Starts at 1.0 (fully safe) and deducts for each red flag found.
    fn score_safety(response: &str) -> (f64, String) {
        let lower = response.to_lowercase();

        let red_flag_hits: Vec<&&str> = SAFETY_RED_FLAGS
            .iter()
            .filter(|flag| lower.contains(*flag))
            .collect();

        let penalty = red_flag_hits.len() as f64 * 0.15;
        let score = (1.0 - penalty).max(0.0);

        let feedback = if red_flag_hits.is_empty() {
            "Safety: no red flags detected".to_string()
        } else {
            format!(
                "Safety: {} red flag(s) detected — {}",
                red_flag_hits.len(),
                red_flag_hits.iter().map(|f| **f).collect::<Vec<_>>().join(", ")
            )
        };

        (score, feedback)
    }

    /// Score completeness by checking for structural thoroughness signals.
    fn score_completeness(response: &str) -> (f64, String) {
        let lower = response.to_lowercase();
        let word_count = response.split_whitespace().count();

        let indicator_hits = COMPLETENESS_INDICATORS
            .iter()
            .filter(|ind| lower.contains(*ind))
            .count();

        // Length contribution
        let length_score = if word_count < 10 {
            0.2
        } else if word_count < 30 {
            0.4
        } else if word_count < 100 {
            0.6
        } else {
            0.7
        };

        // Structure bonus
        let structure_bonus = (indicator_hits as f64 * 0.04).min(0.3);
        let score = (length_score + structure_bonus).min(1.0);

        let feedback = format!(
            "Completeness: {indicator_hits} structure indicators in {word_count} words"
        );
        (score, feedback)
    }

    /// Generic fallback scorer for custom criteria without a specialised handler.
    fn score_generic(response: &str) -> (f64, String) {
        let word_count = response.split_whitespace().count();
        let score = if word_count < 5 {
            0.3
        } else if word_count < 50 {
            0.5
        } else {
            0.7
        };
        (score, format!("Generic: scored by response length ({word_count} words)"))
    }
}

impl Default for EvaluationChain {
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
    fn test_default_criteria_creation() {
        let chain = EvaluationChain::new();
        assert_eq!(chain.criteria.len(), 4);

        let names: Vec<&str> = chain.criteria.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"accuracy"));
        assert!(names.contains(&"relevance"));
        assert!(names.contains(&"safety"));
        assert!(names.contains(&"completeness"));

        // Verify weights sum to 1.0
        let weight_sum: f64 = chain.criteria.iter().map(|c| c.weight).sum();
        assert!(
            (weight_sum - 1.0).abs() < f64::EPSILON,
            "default weights should sum to 1.0, got {weight_sum}"
        );
    }

    #[test]
    fn test_add_custom_criteria() {
        let mut chain = EvaluationChain::new();
        assert_eq!(chain.criteria.len(), 4);

        chain.add_criteria(EvaluationCriteria {
            name: "creativity".to_string(),
            weight: 0.20,
            threshold: 0.3,
        });

        assert_eq!(chain.criteria.len(), 5);
        assert_eq!(chain.criteria[4].name, "creativity");
        assert!((chain.criteria[4].weight - 0.20).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evaluation_scoring() {
        let chain = EvaluationChain::new();

        let prompt = "Explain how Rust ownership works";
        let response = "Rust ownership is a system that manages memory. \
            Because each value has a single owner, the compiler can precisely \
            determine when to deallocate memory. Therefore, Rust achieves \
            memory safety without a garbage collector. First, ownership rules \
            are checked at compile time. Additionally, borrowing allows \
            references without transferring ownership. In summary, this \
            approach eliminates data races and null pointer errors.";

        let results = chain.evaluate(prompt, response);

        assert_eq!(results.len(), 4);
        for result in &results {
            assert!(
                result.score >= 0.0 && result.score <= 1.0,
                "{}: score {} out of bounds",
                result.criteria_name,
                result.score
            );
            assert!(
                !result.feedback.is_empty(),
                "{}: feedback should not be empty",
                result.criteria_name
            );
        }

        // A well-written response should score reasonably on accuracy
        let accuracy = results.iter().find(|r| r.criteria_name == "accuracy").unwrap();
        assert!(
            accuracy.score > 0.5,
            "well-structured response should score > 0.5 on accuracy, got {}",
            accuracy.score
        );
    }

    #[test]
    fn test_aggregate_score_weighted() {
        let chain = EvaluationChain::new();

        // Construct results with known scores
        let results = vec![
            EvaluationResult {
                criteria_name: "accuracy".to_string(),
                score: 0.8,
                passed: true,
                feedback: "good".to_string(),
            },
            EvaluationResult {
                criteria_name: "relevance".to_string(),
                score: 0.6,
                passed: true,
                feedback: "ok".to_string(),
            },
            EvaluationResult {
                criteria_name: "safety".to_string(),
                score: 1.0,
                passed: true,
                feedback: "safe".to_string(),
            },
            EvaluationResult {
                criteria_name: "completeness".to_string(),
                score: 0.5,
                passed: true,
                feedback: "partial".to_string(),
            },
        ];

        let aggregate = chain.aggregate_score(&results);

        // Manual calculation:
        // (0.8*0.30 + 0.6*0.30 + 1.0*0.25 + 0.5*0.15) / (0.30+0.30+0.25+0.15)
        // = (0.24 + 0.18 + 0.25 + 0.075) / 1.0 = 0.745
        let expected = (0.8 * 0.30 + 0.6 * 0.30 + 1.0 * 0.25 + 0.5 * 0.15) / 1.0;
        assert!(
            (aggregate - expected).abs() < 1e-10,
            "aggregate {} should equal expected {expected}",
            aggregate
        );
    }

    #[test]
    fn test_threshold_passing() {
        let chain = EvaluationChain::new();

        // All results pass their thresholds
        let passing_results = vec![
            EvaluationResult {
                criteria_name: "accuracy".to_string(),
                score: 0.9,
                passed: true,
                feedback: String::new(),
            },
            EvaluationResult {
                criteria_name: "relevance".to_string(),
                score: 0.8,
                passed: true,
                feedback: String::new(),
            },
            EvaluationResult {
                criteria_name: "safety".to_string(),
                score: 1.0,
                passed: true,
                feedback: String::new(),
            },
            EvaluationResult {
                criteria_name: "completeness".to_string(),
                score: 0.7,
                passed: true,
                feedback: String::new(),
            },
        ];
        assert!(chain.passes_threshold(&passing_results));
    }

    #[test]
    fn test_threshold_failing() {
        let chain = EvaluationChain::new();

        // One result fails its threshold
        let failing_results = vec![
            EvaluationResult {
                criteria_name: "accuracy".to_string(),
                score: 0.9,
                passed: true,
                feedback: String::new(),
            },
            EvaluationResult {
                criteria_name: "safety".to_string(),
                score: 0.3,
                passed: false, // below safety threshold of 0.7
                feedback: String::new(),
            },
        ];
        assert!(!chain.passes_threshold(&failing_results));

        // Empty results should also fail
        assert!(!chain.passes_threshold(&[]));
    }

    #[test]
    fn test_safety_red_flag_detection() {
        let chain = EvaluationChain::new();

        let prompt = "Tell me about security";
        let safe_response = "Security best practices include using strong passwords \
            and enabling two-factor authentication.";
        let unsafe_response = "To hack into a system, you can exploit a vulnerability \
            and bypass security measures to gain unauthorized access.";

        let safe_results = chain.evaluate(prompt, safe_response);
        let unsafe_results = chain.evaluate(prompt, unsafe_response);

        let safe_score = safe_results
            .iter()
            .find(|r| r.criteria_name == "safety")
            .unwrap();
        let unsafe_score = unsafe_results
            .iter()
            .find(|r| r.criteria_name == "safety")
            .unwrap();

        assert!(
            safe_score.score > unsafe_score.score,
            "safe response ({}) should score higher than unsafe response ({}) on safety",
            safe_score.score,
            unsafe_score.score
        );
        assert!(safe_score.passed, "safe response should pass safety threshold");
        assert!(!unsafe_score.passed, "unsafe response should fail safety threshold");
    }

    #[test]
    fn test_relevance_keyword_overlap() {
        let chain = EvaluationChain::new();

        let prompt = "Explain Rust ownership and borrowing";
        let relevant = "Rust ownership ensures each value has one owner. \
            Borrowing allows references without transferring ownership.";
        let irrelevant = "The weather today is sunny with clear skies. \
            Temperature is around twenty degrees.";

        let relevant_results = chain.evaluate(prompt, relevant);
        let irrelevant_results = chain.evaluate(prompt, irrelevant);

        let rel_score = relevant_results
            .iter()
            .find(|r| r.criteria_name == "relevance")
            .unwrap();
        let irrel_score = irrelevant_results
            .iter()
            .find(|r| r.criteria_name == "relevance")
            .unwrap();

        assert!(
            rel_score.score > irrel_score.score,
            "relevant response ({}) should outscore irrelevant response ({}) on relevance",
            rel_score.score,
            irrel_score.score
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let chain = EvaluationChain::new();
        let json = serde_json::to_string(&chain).expect("serialize chain");
        let restored: EvaluationChain = serde_json::from_str(&json).expect("deserialize chain");
        assert_eq!(restored.criteria.len(), chain.criteria.len());

        let result = EvaluationResult {
            criteria_name: "accuracy".to_string(),
            score: 0.85,
            passed: true,
            feedback: "Good accuracy".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        let restored: EvaluationResult = serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(restored.criteria_name, "accuracy");
        assert!((restored.score - 0.85).abs() < f64::EPSILON);
        assert!(restored.passed);
    }

    #[test]
    fn test_aggregate_empty_results() {
        let chain = EvaluationChain::new();
        let score = chain.aggregate_score(&[]);
        assert!((score - 0.0).abs() < f64::EPSILON, "empty results should yield 0.0");
    }

    #[test]
    fn test_generic_scorer_for_custom_criteria() {
        let mut chain = EvaluationChain::new();
        chain.add_criteria(EvaluationCriteria {
            name: "creativity".to_string(),
            weight: 0.10,
            threshold: 0.3,
        });

        let results = chain.evaluate("test prompt", "This is a moderately detailed response.");
        let creativity = results.iter().find(|r| r.criteria_name == "creativity").unwrap();
        assert!(
            creativity.score > 0.0,
            "custom criterion should still receive a score"
        );
        assert!(creativity.feedback.contains("Generic"));
    }
}
