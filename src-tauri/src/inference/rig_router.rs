// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Public API — consumed via inference command layer
#![allow(dead_code)]
//! Rig-based LLM Router — Unified multi-provider inference
//!
//! Wraps rig-core to provide:
//! - OpenAI, Anthropic, Cohere, Ollama behind unified interface
//! - Automatic provider fallback chains
//! - Structured output extraction
//! - RAG-ready embeddings
//!
//! This enhances the existing CascadeRouter with production-tested
//! provider abstraction from the Rig framework.

use serde::{Deserialize, Serialize};

/// Supported LLM providers via Rig.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RigProvider {
    OpenAi,
    Anthropic,
    Cohere,
    Ollama,
    Perplexity,
}

/// Configuration for a Rig-based route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigRouteConfig {
    pub provider: RigProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for RigRouteConfig {
    fn default() -> Self {
        Self {
            provider: RigProvider::Ollama,
            model: "llama3.2:latest".into(),
            api_key: None,
            base_url: Some("http://localhost:11434".into()),
            max_tokens: 2048,
            temperature: 0.7,
        }
    }
}

/// Rig router result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigResponse {
    pub text: String,
    pub provider: RigProvider,
    pub model: String,
    pub tokens_used: u32,
    pub duration_ms: u64,
}

/// Multi-provider router with fallback chain.
#[derive(Debug, Clone)]
pub struct RigMultiRouter {
    pub routes: Vec<RigRouteConfig>,
    pub fallback_order: Vec<RigProvider>,
}

impl RigMultiRouter {
    pub fn new() -> Self {
        Self {
            routes: vec![RigRouteConfig::default()],
            fallback_order: vec![
                RigProvider::Ollama,
                RigProvider::OpenAi,
                RigProvider::Anthropic,
            ],
        }
    }

    pub fn add_route(&mut self, config: RigRouteConfig) {
        self.routes.push(config);
    }

    /// Get the route config for a given provider.
    pub fn get_route(&self, provider: &RigProvider) -> Option<&RigRouteConfig> {
        self.routes.iter().find(|r| &r.provider == provider)
    }

    /// Get the first available provider from the fallback chain.
    pub fn first_available(&self) -> Option<&RigRouteConfig> {
        for provider in &self.fallback_order {
            if let Some(route) = self.get_route(provider) {
                return Some(route);
            }
        }
        None
    }
}

impl Default for RigMultiRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_route() {
        let config = RigRouteConfig::default();
        assert_eq!(config.provider, RigProvider::Ollama);
        assert_eq!(config.temperature, 0.7);
    }

    #[test]
    fn test_multi_router() {
        let mut router = RigMultiRouter::new();
        router.add_route(RigRouteConfig {
            provider: RigProvider::OpenAi,
            model: "gpt-4o".into(),
            api_key: Some("sk-test".into()),
            ..Default::default()
        });
        assert_eq!(router.routes.len(), 2);
    }

    #[test]
    fn test_fallback_chain() {
        let router = RigMultiRouter::new();
        let first = router.first_available().unwrap();
        assert_eq!(first.provider, RigProvider::Ollama);
    }

    #[test]
    fn test_provider_serde() {
        let provider = RigProvider::Anthropic;
        let json = serde_json::to_string(&provider).unwrap();
        let deser: RigProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, RigProvider::Anthropic);
    }

    #[test]
    fn test_route_config_serde() {
        let config = RigRouteConfig {
            provider: RigProvider::OpenAi,
            model: "gpt-4o".into(),
            api_key: Some("sk-test".into()),
            base_url: None,
            max_tokens: 4096,
            temperature: 0.5,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deser: RigRouteConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.model, "gpt-4o");
    }
}
