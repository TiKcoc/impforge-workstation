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

    /// Select the best model for the given prompt and optional task hint.
    ///
    /// Current (v1) logic:
    /// - If Ollama is available, return tier 0 (local general model).
    /// - Otherwise, return tier 2 (first free cloud model).
    ///
    /// Future versions will inspect the prompt and task hint to decide
    /// whether to escalate to higher tiers.
    pub fn route(&self, _prompt: &str, _task_hint: Option<&str>) -> CascadeDecision {
        if self.ollama_available {
            let tier = &self.tiers[0];
            CascadeDecision {
                model_id: tier.model_id.clone(),
                tier: tier.tier,
                confidence: 0.8,
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
            CascadeDecision {
                model_id: tier.model_id.clone(),
                tier: tier.tier,
                confidence: if has_key { 0.7 } else { 0.0 },
                estimated_cost: 0.0,
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
    fn route_with_task_hint_still_works() {
        let router = CascadeRouter::new();
        let decision = router.route("write a function", Some("code"));

        // V1 ignores task_hint — just verify no panic
        assert_eq!(decision.tier, 0);
        assert_eq!(decision.model_id, "dolphin3:8b");
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
}
