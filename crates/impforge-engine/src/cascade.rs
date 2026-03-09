// SPDX-License-Identifier: BUSL-1.1
//! Cascade Inference Router — 5-Tier Model Selection
//!
//! Routes prompts through a cascade of models from free/local to paid/cloud.
//! The cascade prioritises offline-first (Ollama) and escalates only when
//! local models are unavailable or the task requires higher capability.
//!
//! Tier layout:
//!   0: dolphin3:8b        — Ollama, free, general purpose
//!   1: qwen2.5-coder:7b   — Ollama, free, code specialised
//!   2: devstral-small:free — OpenRouter, free, code
//!   3: llama-4-scout:free  — OpenRouter, free, general
//!   4: qwen3-235b-a22b     — OpenRouter, paid, premium
//!
//! This struct does NOT implement `impforge_lib::InferenceRouter` directly
//! because the engine crate must not depend on the app crate.  A trait-impl
//! bridge is wired in `src-tauri` when the Pro feature flag is active.

use serde::{Deserialize, Serialize};

// ════════════════════════════════════════════════════════════════
// CASCADE TIER
// ════════════════════════════════════════════════════════════════

/// A single tier in the cascade router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeTier {
    /// Tier ordinal (0 = cheapest/local, 4 = most capable/cloud).
    pub tier: u8,
    /// Model identifier (e.g. "dolphin3:8b").
    pub model_id: String,
    /// Provider backend: `"ollama"` or `"openrouter"`.
    pub provider: String,
    /// Whether the tier is free to use.
    pub is_free: bool,
    /// Maximum tokens supported by this tier.
    pub max_tokens: usize,
}

// ════════════════════════════════════════════════════════════════
// ROUTING DECISION (engine-local mirror)
// ════════════════════════════════════════════════════════════════

/// Routing decision returned by [`CascadeRouter::route`].
///
/// This mirrors the `RoutingDecision` type in `impforge_lib::traits` so
/// the bridge layer can perform a field-by-field copy without any unsafe
/// transmute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeDecision {
    /// Selected model identifier.
    pub model_id: String,
    /// Tier ordinal that was selected.
    pub tier: u8,
    /// Estimated confidence that this tier can handle the prompt \[0.0, 1.0\].
    pub confidence: f64,
    /// Estimated cost in USD (0.0 for free / local).
    pub estimated_cost: f64,
}

// ════════════════════════════════════════════════════════════════
// PROMPT ANALYSIS
// ════════════════════════════════════════════════════════════════

/// Code-related keywords and indicators used for prompt classification.
const CODE_INDICATORS: &[&str] = &[
    "fn ", "func ", "function ", "def ", "class ", "struct ", "impl ",
    "async ", "await ", "return ", "const ", "let ", "var ", "import ",
    "require(", "pub ", "mod ", "use ", "trait ", "enum ", "match ",
    "if ", "else ", "for ", "while ", "loop ", "println!", "console.log",
    "```", "//", "/*", "#[", "->", "=>", "::", "&&", "||",
];

/// Result of analysing a prompt for routing decisions.
#[derive(Debug, Clone)]
pub struct PromptAnalysis {
    /// Total number of whitespace-delimited words.
    pub word_count: usize,
    /// Number of code-related indicators found in the prompt.
    pub code_indicator_count: usize,
    /// Whether the prompt is classified as code-related.
    pub is_code: bool,
    /// Approximate complexity bucket: `"simple"`, `"moderate"`, or `"complex"`.
    pub complexity: String,
}

// ════════════════════════════════════════════════════════════════
// CASCADE ROUTER
// ════════════════════════════════════════════════════════════════

/// Multi-tier cascade inference router.
///
/// Selects the most cost-effective model that is available and capable
/// of handling the prompt.  Falls back up the cascade when lower tiers
/// are unavailable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeRouter {
    /// Ordered list of tiers (index = tier ordinal).
    pub tiers: Vec<CascadeTier>,
    /// Whether the local Ollama instance is reachable.
    pub ollama_available: bool,
    /// Optional OpenRouter API key.  `None` disables cloud tiers.
    pub openrouter_key: Option<String>,
}

impl CascadeRouter {
    /// Create a router with the five default tiers.
    pub fn new() -> Self {
        let tiers = vec![
            CascadeTier {
                tier: 0,
                model_id: "dolphin3:8b".into(),
                provider: "ollama".into(),
                is_free: true,
                max_tokens: 4096,
            },
            CascadeTier {
                tier: 1,
                model_id: "qwen2.5-coder:7b".into(),
                provider: "ollama".into(),
                is_free: true,
                max_tokens: 32768,
            },
            CascadeTier {
                tier: 2,
                model_id: "devstral-small:free".into(),
                provider: "openrouter".into(),
                is_free: true,
                max_tokens: 32768,
            },
            CascadeTier {
                tier: 3,
                model_id: "llama-4-scout:free".into(),
                provider: "openrouter".into(),
                is_free: true,
                max_tokens: 65536,
            },
            CascadeTier {
                tier: 4,
                model_id: "qwen3-235b-a22b".into(),
                provider: "openrouter".into(),
                is_free: false,
                max_tokens: 131072,
            },
        ];

        Self {
            tiers,
            ollama_available: true,
            openrouter_key: None,
        }
    }

    /// Analyse a prompt to extract complexity signals for routing.
    ///
    /// The analysis examines word count, presence of code indicators,
    /// and overall complexity to inform tier selection and confidence
    /// estimation.
    pub fn analyze_prompt(prompt: &str) -> PromptAnalysis {
        let lower = prompt.to_lowercase();
        let word_count = prompt.split_whitespace().count();

        let code_indicator_count = CODE_INDICATORS
            .iter()
            .filter(|ind| lower.contains(&ind.to_lowercase()))
            .count();

        // A prompt is code-related if it has 2+ code indicators or an
        // explicit code-fence.
        let is_code = code_indicator_count >= 2 || lower.contains("```");

        let complexity = if word_count < 50 && code_indicator_count < 2 {
            "simple".to_string()
        } else if word_count <= 200 && code_indicator_count < 5 {
            "moderate".to_string()
        } else {
            "complex".to_string()
        };

        PromptAnalysis {
            word_count,
            code_indicator_count,
            is_code,
            complexity,
        }
    }

    /// Estimate confidence that the given tier can handle a prompt.
    ///
    /// Returns a score in \[0.0, 1.0\] where 1.0 means "very confident
    /// this tier will produce a good result" and 0.0 means "no chance".
    ///
    /// Heuristics (v2):
    /// - Short, simple prompts (< 50 words, no code) get high confidence
    ///   on local general models.
    /// - Code prompts get higher confidence on code-specialised tiers.
    /// - Long / complex prompts (> 200 words) reduce local confidence
    ///   because small models may struggle with extended reasoning.
    /// - Cloud / premium tiers receive a baseline confidence boost.
    pub fn estimate_confidence(tier: &CascadeTier, analysis: &PromptAnalysis) -> f64 {
        let is_local = tier.provider == "ollama";
        let is_code_model = tier.model_id.contains("coder")
            || tier.model_id.contains("devstral");

        // Start with a baseline that depends on model capability.
        let mut confidence: f64 = match tier.tier {
            0 => 0.85, // local general — good for chat
            1 => 0.80, // local code — specialised
            2 => 0.75, // free cloud code
            3 => 0.70, // free cloud general
            4 => 0.90, // premium cloud — highest capability
            _ => 0.50,
        };

        // Boost code models when the prompt is code-related.
        if analysis.is_code && is_code_model {
            confidence += 0.10;
        }

        // Penalise non-code models when the prompt is clearly code.
        if analysis.is_code && !is_code_model {
            confidence -= 0.15;
        }

        // Short, simple prompts: local models handle these well.
        if analysis.complexity == "simple" && is_local {
            confidence += 0.05;
        }

        // Long / complex prompts: local small models lose confidence.
        if analysis.complexity == "complex" && is_local {
            confidence -= 0.20;
        }

        // Moderate-length prompts get a small local penalty.
        if analysis.complexity == "moderate" && is_local {
            confidence -= 0.05;
        }

        // Clamp to [0.0, 1.0].
        confidence.clamp(0.0, 1.0)
    }

    /// Select the best model for the given prompt and optional task hint.
    ///
    /// v2 logic:
    /// - Analyse the prompt for complexity and code indicators.
    /// - If the prompt is code-related and Ollama is available, prefer
    ///   the code-specialised local model (tier 1).
    /// - Otherwise, if Ollama is available, use tier 0 (general local).
    /// - If Ollama is unavailable, fall back to the first free cloud tier.
    /// - Confidence is estimated from prompt analysis, not hardcoded.
    pub fn route(&self, prompt: &str, task_hint: Option<&str>) -> CascadeDecision {
        let analysis = Self::analyze_prompt(prompt);

        // Check if the task hint explicitly requests code routing.
        let hint_is_code = task_hint
            .map(|h| {
                let lh = h.to_lowercase();
                lh.contains("code") || lh.contains("programming") || lh.contains("implement")
            })
            .unwrap_or(false);

        let wants_code = analysis.is_code || hint_is_code;

        if self.ollama_available {
            // Select code model (tier 1) for code prompts, otherwise
            // general model (tier 0).
            let tier = if wants_code {
                &self.tiers[1]
            } else {
                &self.tiers[0]
            };

            let confidence = Self::estimate_confidence(tier, &analysis);
            CascadeDecision {
                model_id: tier.model_id.clone(),
                tier: tier.tier,
                confidence,
                estimated_cost: 0.0,
            }
        } else {
            // Ollama unavailable — fall back to first free cloud tier.
            let tier = self
                .tiers
                .iter()
                .find(|t| t.provider == "openrouter" && t.is_free)
                .unwrap_or(&self.tiers[0]);

            let has_key = self.openrouter_key.is_some();
            if has_key {
                let confidence = Self::estimate_confidence(tier, &analysis);
                CascadeDecision {
                    model_id: tier.model_id.clone(),
                    tier: tier.tier,
                    confidence,
                    estimated_cost: 0.0,
                }
            } else {
                // No API key — provider cannot actually run; zero confidence.
                CascadeDecision {
                    model_id: tier.model_id.clone(),
                    tier: tier.tier,
                    confidence: 0.0,
                    estimated_cost: 0.0,
                }
            }
        }
    }

    /// List all tiers with their availability status.
    ///
    /// Returns `(tier_ordinal, model_id, is_available)` tuples matching
    /// the `InferenceRouter::available_tiers` signature.
    pub fn available_tiers(&self) -> Vec<(u8, String, bool)> {
        self.tiers
            .iter()
            .map(|t| {
                let available = match t.provider.as_str() {
                    "ollama" => self.ollama_available,
                    "openrouter" => {
                        if t.is_free {
                            self.openrouter_key.is_some()
                        } else {
                            self.openrouter_key.is_some()
                        }
                    }
                    _ => false,
                };
                (t.tier, t.model_id.clone(), available)
            })
            .collect()
    }

    /// Update Ollama availability status.
    pub fn set_ollama_available(&mut self, available: bool) {
        self.ollama_available = available;
    }

    /// Set the OpenRouter API key.  Pass `None` to disable cloud tiers.
    pub fn set_openrouter_key(&mut self, key: Option<String>) {
        self.openrouter_key = key;
    }
}

impl Default for CascadeRouter {
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
    fn new_creates_five_tiers() {
        let router = CascadeRouter::new();
        assert_eq!(router.tiers.len(), 5);

        // Verify tier ordinals
        for (i, tier) in router.tiers.iter().enumerate() {
            assert_eq!(tier.tier, i as u8, "tier ordinal mismatch at index {i}");
        }

        // Verify providers
        assert_eq!(router.tiers[0].provider, "ollama");
        assert_eq!(router.tiers[1].provider, "ollama");
        assert_eq!(router.tiers[2].provider, "openrouter");
        assert_eq!(router.tiers[3].provider, "openrouter");
        assert_eq!(router.tiers[4].provider, "openrouter");

        // Verify model IDs
        assert_eq!(router.tiers[0].model_id, "dolphin3:8b");
        assert_eq!(router.tiers[1].model_id, "qwen2.5-coder:7b");
        assert_eq!(router.tiers[2].model_id, "devstral-small:free");
        assert_eq!(router.tiers[3].model_id, "llama-4-scout:free");
        assert_eq!(router.tiers[4].model_id, "qwen3-235b-a22b");

        // Verify free/paid
        assert!(router.tiers[0].is_free);
        assert!(router.tiers[1].is_free);
        assert!(router.tiers[2].is_free);
        assert!(router.tiers[3].is_free);
        assert!(!router.tiers[4].is_free, "tier 4 should be paid");
    }

    #[test]
    fn route_selects_tier_0_when_ollama_available() {
        let router = CascadeRouter::new();
        let decision = router.route("hello world", None);

        assert_eq!(decision.model_id, "dolphin3:8b");
        assert_eq!(decision.tier, 0);
        assert_eq!(decision.estimated_cost, 0.0);
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn route_falls_back_to_cloud_when_ollama_unavailable() {
        let mut router = CascadeRouter::new();
        router.set_ollama_available(false);
        router.set_openrouter_key(Some("test-key".into()));

        let decision = router.route("hello world", None);

        assert_eq!(decision.model_id, "devstral-small:free");
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.estimated_cost, 0.0);
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn route_zero_confidence_without_api_key() {
        let mut router = CascadeRouter::new();
        router.set_ollama_available(false);
        // No OpenRouter key set

        let decision = router.route("hello", None);

        // Should still select a cloud tier, but with zero confidence
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.confidence, 0.0);
    }

    #[test]
    fn route_with_code_task_hint_selects_code_model() {
        let router = CascadeRouter::new();
        let decision = router.route("write a function", Some("code"));

        // v2 uses task_hint — "code" hint routes to code model (tier 1).
        assert_eq!(decision.tier, 1);
        assert_eq!(decision.model_id, "qwen2.5-coder:7b");
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn available_tiers_reflects_ollama_status() {
        let mut router = CascadeRouter::new();
        let tiers = router.available_tiers();

        // Ollama tiers available, OpenRouter tiers unavailable (no key)
        assert_eq!(tiers.len(), 5);
        assert!(tiers[0].2, "tier 0 (ollama) should be available");
        assert!(tiers[1].2, "tier 1 (ollama) should be available");
        assert!(!tiers[2].2, "tier 2 (openrouter) should be unavailable without key");
        assert!(!tiers[3].2, "tier 3 (openrouter) should be unavailable without key");
        assert!(!tiers[4].2, "tier 4 (openrouter) should be unavailable without key");

        // Disable Ollama, enable OpenRouter
        router.set_ollama_available(false);
        router.set_openrouter_key(Some("key".into()));
        let tiers = router.available_tiers();

        assert!(!tiers[0].2, "tier 0 (ollama) should be unavailable");
        assert!(!tiers[1].2, "tier 1 (ollama) should be unavailable");
        assert!(tiers[2].2, "tier 2 (openrouter) should be available with key");
        assert!(tiers[3].2, "tier 3 (openrouter) should be available with key");
        assert!(tiers[4].2, "tier 4 (openrouter) should be available with key");
    }

    #[test]
    fn available_tiers_all_available() {
        let mut router = CascadeRouter::new();
        router.set_openrouter_key(Some("key".into()));

        let tiers = router.available_tiers();
        for (i, (_, _, available)) in tiers.iter().enumerate() {
            assert!(available, "tier {i} should be available when both providers are up");
        }
    }

    #[test]
    fn default_matches_new() {
        let a = CascadeRouter::new();
        let b = CascadeRouter::default();
        assert_eq!(a.tiers.len(), b.tiers.len());
        assert_eq!(a.ollama_available, b.ollama_available);
        assert_eq!(a.openrouter_key, b.openrouter_key);
    }

    #[test]
    fn cascade_tier_serde_roundtrip() {
        let tier = CascadeTier {
            tier: 2,
            model_id: "devstral-small:free".into(),
            provider: "openrouter".into(),
            is_free: true,
            max_tokens: 32768,
        };
        let json = serde_json::to_string(&tier).expect("serialize");
        let restored: CascadeTier = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.tier, 2);
        assert_eq!(restored.model_id, "devstral-small:free");
        assert_eq!(restored.provider, "openrouter");
        assert!(restored.is_free);
        assert_eq!(restored.max_tokens, 32768);
    }

    #[test]
    fn cascade_decision_serde_roundtrip() {
        let decision = CascadeDecision {
            model_id: "dolphin3:8b".into(),
            tier: 0,
            confidence: 0.8,
            estimated_cost: 0.0,
        };
        let json = serde_json::to_string(&decision).expect("serialize");
        let restored: CascadeDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.model_id, "dolphin3:8b");
        assert_eq!(restored.tier, 0);
        assert!((restored.confidence - 0.8).abs() < f64::EPSILON);
        assert_eq!(restored.estimated_cost, 0.0);
    }

    // ── Prompt analysis tests ─────────────────────────────────────

    #[test]
    fn analyze_prompt_simple_chat() {
        let analysis = CascadeRouter::analyze_prompt("Hello, how are you?");
        assert_eq!(analysis.word_count, 4);
        assert_eq!(analysis.code_indicator_count, 0);
        assert!(!analysis.is_code);
        assert_eq!(analysis.complexity, "simple");
    }

    #[test]
    fn analyze_prompt_detects_code() {
        let analysis = CascadeRouter::analyze_prompt(
            "fn main() { let x = 42; println!(\"hello\"); }",
        );
        assert!(analysis.is_code, "should detect code indicators");
        assert!(
            analysis.code_indicator_count >= 2,
            "expected >= 2 code indicators, got {}",
            analysis.code_indicator_count
        );
    }

    #[test]
    fn analyze_prompt_complex_long() {
        // Build a prompt that exceeds 200 words with several code indicators.
        let filler = "word ".repeat(210);
        let prompt = format!("{filler} fn struct impl class trait enum");
        let analysis = CascadeRouter::analyze_prompt(&prompt);
        assert!(analysis.word_count > 200);
        assert_eq!(analysis.complexity, "complex");
    }

    // ── Confidence estimation tests ───────────────────────────────

    #[test]
    fn test_short_prompt_high_confidence() {
        let router = CascadeRouter::new();
        let decision = router.route("What is the weather today?", None);

        // Short, simple, non-code prompt on local general model (tier 0).
        assert_eq!(decision.tier, 0);
        assert_eq!(decision.model_id, "dolphin3:8b");
        // Baseline 0.85 + 0.05 (simple+local) = 0.90
        assert!(
            decision.confidence >= 0.85,
            "short prompt should yield high confidence, got {}",
            decision.confidence
        );
    }

    #[test]
    fn test_long_prompt_lower_confidence() {
        let router = CascadeRouter::new();
        // Build a long, complex prompt (> 200 words) without code indicators.
        let long_prompt = "Please analyze ".to_string() + &"the situation ".repeat(120);
        let decision = router.route(&long_prompt, None);

        // Should still route to local (Ollama available) but with reduced
        // confidence because the prompt is classified as "complex".
        assert_eq!(decision.tier, 0);
        assert!(
            decision.confidence < 0.80,
            "long/complex prompt should yield lower local confidence, got {}",
            decision.confidence
        );
    }

    #[test]
    fn test_code_prompt_routes_to_code_model() {
        let router = CascadeRouter::new();
        let decision = router.route(
            "```rust\nfn main() {\n    let x = 42;\n    println!(\"{x}\");\n}\n```",
            None,
        );

        // Code fence + multiple code indicators → routes to code model.
        assert_eq!(decision.tier, 1);
        assert_eq!(decision.model_id, "qwen2.5-coder:7b");
        assert!(
            decision.confidence > 0.0,
            "code prompt on code model should have positive confidence"
        );
    }

    // ── Integration tests (Task 14) ───────────────────────────────

    #[test]
    fn test_cascade_escalation_flow() {
        // A complex prompt with many code indicators should route to a
        // higher tier (code model, tier 1) compared to a trivial chat
        // prompt which stays on tier 0.
        let router = CascadeRouter::new();

        let simple_decision = router.route("Hi there!", None);
        let complex_decision = router.route(
            "```rust\nuse std::collections::HashMap;\n\n\
             impl MyStruct {\n    pub async fn process(&self) -> Result<(), Error> {\n\
                 let mut map = HashMap::new();\n        \
                 for item in self.items.iter() {\n            \
                     match item.kind {\n                \
                         Kind::A => map.insert(item.id, item.clone()),\n                \
                         _ => None,\n            \
                     };\n        \
                 }\n        Ok(())\n    }\n}\n```",
            None,
        );

        assert_eq!(simple_decision.tier, 0, "simple chat should stay on tier 0");
        assert!(
            complex_decision.tier >= simple_decision.tier,
            "complex code prompt should route to an equal or higher tier than simple chat"
        );
        assert_eq!(
            complex_decision.tier, 1,
            "complex code prompt should escalate to the code model (tier 1)"
        );
    }

    #[test]
    fn test_cascade_all_tiers_configured() {
        // Verify the default router has exactly 5 tiers and every tier
        // has a non-empty model_id and a recognised provider.
        let router = CascadeRouter::new();
        assert_eq!(router.tiers.len(), 5, "default router must have 5 tiers");

        for (i, tier) in router.tiers.iter().enumerate() {
            assert_eq!(
                tier.tier, i as u8,
                "tier ordinal must match its position in the vec"
            );
            assert!(
                !tier.model_id.is_empty(),
                "tier {i} must have a non-empty model_id"
            );
            assert!(
                tier.provider == "ollama" || tier.provider == "openrouter",
                "tier {i} provider '{}' must be 'ollama' or 'openrouter'",
                tier.provider
            );
            assert!(
                tier.max_tokens > 0,
                "tier {i} max_tokens must be greater than zero"
            );
        }
    }

    #[test]
    fn test_cascade_confidence_bounds() {
        // Verify that estimate_confidence always returns a value in
        // [0.0, 1.0] regardless of prompt characteristics.
        let router = CascadeRouter::new();

        let complex_filler = "complex ".repeat(300);
        let code_block = format!(
            "```rust\n{}\n```",
            "impl Foo { fn bar(&self) -> i32 { 42 } }\n".repeat(50)
        );

        let prompts: Vec<&str> = vec![
            "",
            "hi",
            "Write me a very long essay about the history of computing.",
            "fn main() { let x = 42; println!(\"{x}\"); }",
            &complex_filler,
            &code_block,
        ];

        for prompt in &prompts {
            let analysis = CascadeRouter::analyze_prompt(prompt);
            for tier in &router.tiers {
                let conf = CascadeRouter::estimate_confidence(tier, &analysis);
                assert!(
                    (0.0..=1.0).contains(&conf),
                    "confidence {} out of bounds for tier {} with prompt len {}",
                    conf,
                    tier.tier,
                    prompt.len()
                );
            }
        }
    }

    #[test]
    fn test_cascade_custom_tier_ordering() {
        // Create a router, push a custom tier, and verify it appears
        // in the tier list returned by available_tiers().
        let mut router = CascadeRouter::new();
        let custom_tier = CascadeTier {
            tier: 5,
            model_id: "custom-model:latest".into(),
            provider: "ollama".into(),
            is_free: true,
            max_tokens: 8192,
        };
        router.tiers.push(custom_tier);

        assert_eq!(router.tiers.len(), 6, "should have 6 tiers after push");

        let available = router.available_tiers();
        assert_eq!(available.len(), 6);

        let last = &available[5];
        assert_eq!(last.0, 5, "custom tier ordinal should be 5");
        assert_eq!(last.1, "custom-model:latest");
        assert!(
            last.2,
            "custom ollama tier should be available when ollama is up"
        );
    }

    #[test]
    fn test_cascade_empty_prompt_handling() {
        // An empty prompt should still return a valid decision without
        // panicking or producing NaN / out-of-bounds values.
        let router = CascadeRouter::new();
        let decision = router.route("", None);

        assert!(
            !decision.model_id.is_empty(),
            "model_id must not be empty even for empty prompt"
        );
        assert!(
            (0.0..=1.0).contains(&decision.confidence),
            "confidence {} must be in [0.0, 1.0]",
            decision.confidence
        );
        assert!(
            decision.estimated_cost >= 0.0,
            "estimated_cost must be non-negative"
        );

        // Empty prompt is trivially "simple" and non-code.
        let analysis = CascadeRouter::analyze_prompt("");
        assert_eq!(analysis.word_count, 0);
        assert_eq!(analysis.code_indicator_count, 0);
        assert!(!analysis.is_code);
        assert_eq!(analysis.complexity, "simple");
    }

    #[test]
    fn test_cascade_very_long_prompt() {
        // A prompt with 10 000+ characters should route successfully
        // and be classified as "complex" due to word count.
        let long_prompt = "Explain ".to_string() + &"the detailed rationale ".repeat(600);
        assert!(
            long_prompt.len() > 10_000,
            "prompt should exceed 10 000 chars"
        );

        let router = CascadeRouter::new();
        let decision = router.route(&long_prompt, None);

        // Must return a valid decision.
        assert!(
            !decision.model_id.is_empty(),
            "model_id must not be empty for long prompt"
        );
        assert!(
            (0.0..=1.0).contains(&decision.confidence),
            "confidence {} must be in [0.0, 1.0]",
            decision.confidence
        );

        // Prompt analysis should classify it as complex.
        let analysis = CascadeRouter::analyze_prompt(&long_prompt);
        assert!(
            analysis.word_count > 200,
            "long prompt should have > 200 words, got {}",
            analysis.word_count
        );
        assert_eq!(
            analysis.complexity, "complex",
            "long prompt should be classified as complex"
        );

        // Complex prompt on local model should have reduced confidence.
        assert!(
            decision.confidence < 0.85,
            "long/complex prompt on local model should have reduced confidence, got {}",
            decision.confidence
        );
    }
}
