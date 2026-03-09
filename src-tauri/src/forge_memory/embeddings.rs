#![allow(dead_code)]
//! Embedding providers for ForgeMemory
//!
//! Two-tier strategy (same pattern as AI completion cascading):
//!   1. fastembed-rs (default): all-MiniLM-L6-v2, 384-dim, ONNX Runtime, offline
//!      - Zero-config, no external dependencies, works on any customer machine
//!      - ~14MB model, auto-downloaded on first use to ~/.cache/fastembed/
//!      - Throughput: ~500 embeddings/sec on CPU (Wang et al. 2020)
//!   2. Ollama (optional upgrade): nomic-embed-text, 768-dim
//!      - Requires Ollama running locally, better quality for longer texts
//!      - Fallback: if Ollama is down, transparently falls back to fastembed
//!
//! Architecture:
//!   EmbeddingProvider trait → FastEmbedProvider | OllamaProvider
//!   ForgeEmbeddings (facade) → tries Ollama first, falls back to fastembed
//!
//! References:
//!   - Sentence-BERT (Reimers & Gurevych 2019, EMNLP)
//!   - all-MiniLM-L6-v2 (Wang et al. 2020, Microsoft Research)
//!   - Nomic Embed (Nussbaum et al. 2024, nomic.ai)
//!   - ONNX Runtime (Microsoft, cross-platform inference)

use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Provider Trait ────────────────────────────────────────────

/// Unified embedding provider interface.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of texts.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embedding dimensionality (384 for MiniLM, 768 for nomic).
    fn dimensions(&self) -> usize;

    /// Provider identifier for cache keying.
    fn model_id(&self) -> &str;

    /// Whether this provider is currently available.
    fn is_available(&self) -> bool;
}

// ── Error Type ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("FastEmbed error: {0}")]
    FastEmbed(String),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("No embedding provider available")]
    NoProvider,

    #[error("Empty input")]
    EmptyInput,
}

// ── FastEmbed Provider ────────────────────────────────────────

/// Offline embedding via fastembed-rs (ONNX Runtime).
/// Model: all-MiniLM-L6-v2 (384-dim, ~14MB, Apache-2.0)
pub struct FastEmbedProvider {
    model: parking_lot::Mutex<fastembed::TextEmbedding>,
    model_id: String,
    dimensions: usize,
}

impl FastEmbedProvider {
    /// Initialize with default model (all-MiniLM-L6-v2).
    pub fn new() -> Result<Self, EmbeddingError> {
        Self::with_model(fastembed::EmbeddingModel::AllMiniLML6V2)
    }

    /// Initialize with a specific fastembed model.
    pub fn with_model(model: fastembed::EmbeddingModel) -> Result<Self, EmbeddingError> {
        let dimensions = match model {
            fastembed::EmbeddingModel::AllMiniLML6V2 => 384,
            fastembed::EmbeddingModel::AllMiniLML12V2 => 384,
            fastembed::EmbeddingModel::BGESmallENV15 => 384,
            fastembed::EmbeddingModel::BGEBaseENV15 => 768,
            fastembed::EmbeddingModel::BGELargeENV15 => 1024,
            fastembed::EmbeddingModel::NomicEmbedTextV15 => 768,
            _ => 384,
        };
        let model_id = format!("fastembed:{:?}", model);

        let options = fastembed::InitOptions::new(model).with_show_download_progress(true);
        let model = fastembed::TextEmbedding::try_new(options)
            .map_err(|e| EmbeddingError::FastEmbed(e.to_string()))?;

        Ok(Self {
            model: parking_lot::Mutex::new(model),
            model_id,
            dimensions,
        })
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let mut model = self.model.lock();
        model
            .embed(owned, None)
            .map_err(|e| EmbeddingError::FastEmbed(e.to_string()))
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn is_available(&self) -> bool {
        true // Always available — offline, bundled
    }
}

// ── Ollama Provider ───────────────────────────────────────────

/// Ollama-based embedding via HTTP API.
/// Model: nomic-embed-text (768-dim, requires Ollama running)
pub struct OllamaProvider {
    base_url: String,
    model_name: String,
    dimensions: usize,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaProvider {
    /// Create Ollama provider with default settings.
    pub fn new() -> Self {
        Self::with_config("http://localhost:11434", "nomic-embed-text", 768)
    }

    /// Create with custom Ollama endpoint and model.
    pub fn with_config(base_url: &str, model_name: &str, dimensions: usize) -> Self {
        Self {
            base_url: base_url.to_string(),
            model_name: model_name.to_string(),
            dimensions,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Check if Ollama is reachable.
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .is_ok()
    }
}

impl EmbeddingProvider for OllamaProvider {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Use tokio runtime for async HTTP call from sync context
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| EmbeddingError::Ollama("No tokio runtime".into()))?;

        let input: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let req = OllamaEmbedRequest {
            model: self.model_name.clone(),
            input,
        };

        let client = self.client.clone();
        let url = format!("{}/api/embed", self.base_url);

        let result = rt.block_on(async {
            client
                .post(&url)
                .json(&req)
                .send()
                .await
                .map_err(|e| EmbeddingError::Ollama(e.to_string()))?
                .json::<OllamaEmbedResponse>()
                .await
                .map_err(|e| EmbeddingError::Ollama(e.to_string()))
        })?;

        Ok(result.embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model_name
    }

    fn is_available(&self) -> bool {
        // Quick sync check — try to connect
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                let client = self.client.clone();
                let url = format!("{}/api/tags", self.base_url);
                handle
                    .block_on(async { client.get(&url).send().await })
                    .is_ok()
            }
            Err(_) => false,
        }
    }
}

// ── Facade: ForgeEmbeddings ───────────────────────────────────

/// Main embedding facade — tries Ollama first, falls back to fastembed.
/// Thread-safe via Arc<dyn EmbeddingProvider>.
pub struct ForgeEmbeddings {
    fastembed: Arc<dyn EmbeddingProvider>,
    ollama: Option<Arc<dyn EmbeddingProvider>>,
    prefer_ollama: bool,
}

/// Embedding result with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub embeddings: Vec<Vec<f32>>,
    pub model_id: String,
    pub dimensions: usize,
}

impl ForgeEmbeddings {
    /// Create with fastembed only (default, zero-config).
    pub fn new() -> Result<Self, EmbeddingError> {
        let fastembed = Arc::new(FastEmbedProvider::new()?) as Arc<dyn EmbeddingProvider>;
        Ok(Self {
            fastembed,
            ollama: None,
            prefer_ollama: false,
        })
    }

    /// Create with Ollama as preferred provider, fastembed as fallback.
    pub fn with_ollama(ollama_url: &str, model: &str, dims: usize) -> Result<Self, EmbeddingError> {
        let fastembed = Arc::new(FastEmbedProvider::new()?) as Arc<dyn EmbeddingProvider>;
        let ollama = Arc::new(OllamaProvider::with_config(ollama_url, model, dims))
            as Arc<dyn EmbeddingProvider>;
        Ok(Self {
            fastembed,
            ollama: Some(ollama),
            prefer_ollama: true,
        })
    }

    /// Embed texts using the best available provider.
    /// Falls back from Ollama → fastembed if Ollama is unavailable.
    pub fn embed(&self, texts: &[&str]) -> Result<EmbeddingResult, EmbeddingError> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Try Ollama first if preferred and available
        if self.prefer_ollama {
            if let Some(ref ollama) = self.ollama {
                if ollama.is_available() {
                    match ollama.embed(texts) {
                        Ok(embeddings) => {
                            return Ok(EmbeddingResult {
                                embeddings,
                                model_id: ollama.model_id().to_string(),
                                dimensions: ollama.dimensions(),
                            });
                        }
                        Err(_) => {
                            // Fall through to fastembed
                            log::warn!("Ollama embedding failed, falling back to fastembed");
                        }
                    }
                }
            }
        }

        // fastembed (always available, offline)
        let embeddings = self.fastembed.embed(texts)?;
        Ok(EmbeddingResult {
            embeddings,
            model_id: self.fastembed.model_id().to_string(),
            dimensions: self.fastembed.dimensions(),
        })
    }

    /// Embed a single text.
    pub fn embed_one(&self, text: &str) -> Result<(Vec<f32>, String), EmbeddingError> {
        let result = self.embed(&[text])?;
        let vec = result
            .embeddings
            .into_iter()
            .next()
            .ok_or(EmbeddingError::EmptyInput)?;
        Ok((vec, result.model_id))
    }

    /// Get the active provider's dimensionality.
    pub fn dimensions(&self) -> usize {
        if self.prefer_ollama {
            if let Some(ref ollama) = self.ollama {
                if ollama.is_available() {
                    return ollama.dimensions();
                }
            }
        }
        self.fastembed.dimensions()
    }

    /// Get the active model ID.
    pub fn active_model(&self) -> &str {
        if self.prefer_ollama {
            if let Some(ref ollama) = self.ollama {
                if ollama.is_available() {
                    return ollama.model_id();
                }
            }
        }
        self.fastembed.model_id()
    }

    /// Convert f32 embedding to bytes for SQLite BLOB storage.
    pub fn to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect()
    }

    /// Convert SQLite BLOB bytes back to f32 embedding.
    pub fn from_bytes(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    /// Cosine similarity between two embeddings.
    /// Returns value in [-1.0, 1.0], where 1.0 = identical.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;
        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = ForgeEmbeddings::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = ForgeEmbeddings::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = ForgeEmbeddings::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bytes_roundtrip() {
        let original = vec![0.1f32, 0.2, 0.3, -0.5, 1.0];
        let bytes = ForgeEmbeddings::to_bytes(&original);
        assert_eq!(bytes.len(), 20); // 5 * 4 bytes
        let recovered = ForgeEmbeddings::from_bytes(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_bytes_roundtrip_384dim() {
        let original: Vec<f32> = (0..384).map(|i| i as f32 * 0.001).collect();
        let bytes = ForgeEmbeddings::to_bytes(&original);
        assert_eq!(bytes.len(), 384 * 4);
        let recovered = ForgeEmbeddings::from_bytes(&bytes);
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-7);
        }
    }

    #[test]
    fn test_empty_input_error() {
        // FastEmbed embed should fail with empty input
        let provider = FastEmbedProvider::new();
        if let Ok(p) = provider {
            let result = p.embed(&[]);
            assert!(result.is_err());
        }
        // Skip if fastembed model download not available in CI
    }

    #[test]
    fn test_cosine_similarity_edge_cases() {
        // Empty vectors
        assert_eq!(ForgeEmbeddings::cosine_similarity(&[], &[]), 0.0);
        // Different lengths
        assert_eq!(
            ForgeEmbeddings::cosine_similarity(&[1.0], &[1.0, 2.0]),
            0.0
        );
        // Zero vector
        assert_eq!(
            ForgeEmbeddings::cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]),
            0.0
        );
    }

    // Integration test: only runs if fastembed model is available
    #[test]
    fn test_fastembed_integration() {
        let provider = match FastEmbedProvider::new() {
            Ok(p) => p,
            Err(_) => return, // Skip if model download fails
        };

        assert_eq!(provider.dimensions(), 384);
        assert!(provider.is_available());

        let result = provider.embed(&["Hello world", "Testing embeddings"]);
        match result {
            Ok(embeddings) => {
                assert_eq!(embeddings.len(), 2);
                assert_eq!(embeddings[0].len(), 384);
                assert_eq!(embeddings[1].len(), 384);
                // Sanity: different texts should have different embeddings
                let sim = ForgeEmbeddings::cosine_similarity(&embeddings[0], &embeddings[1]);
                assert!(sim < 0.99, "Different texts should have different embeddings");
                assert!(sim > -1.0, "Similarity should be valid");
            }
            Err(_) => {} // Model download might fail in CI
        }
    }

    #[test]
    fn test_forge_embeddings_facade() {
        let forge = match ForgeEmbeddings::new() {
            Ok(f) => f,
            Err(_) => return, // Skip if model not available
        };

        assert_eq!(forge.dimensions(), 384);
        assert!(forge.active_model().contains("MiniLM"));

        match forge.embed_one("test embedding") {
            Ok((vec, model_id)) => {
                assert_eq!(vec.len(), 384);
                assert!(model_id.contains("MiniLM"));
            }
            Err(_) => {} // Model download might fail
        }
    }
}
