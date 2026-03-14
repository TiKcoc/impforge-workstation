//! ImpForge Enterprise AI Model Recommendations
//!
//! Intelligent model recommendations based on:
//! - Hardware capabilities (VRAM, RAM, CPU cores)
//! - Use case requirements (coding, chat, reasoning, embedding)
//! - Performance metrics (tokens/sec, latency)
//! - Cost optimization (quantization, batch size)
//!
//! Based on: NVIDIA Best Practices, HuggingFace Benchmarks, llama.cpp optimization guides

use serde::{Deserialize, Serialize};

// ============================================================================
// MODEL DATABASE - Enterprise Curated List
// ============================================================================

/// Model category/use case
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCategory {
    /// General chat and conversation
    Chat,
    /// Code generation and completion
    Coding,
    /// Complex reasoning and analysis
    Reasoning,
    /// Creative writing
    Creative,
    /// Embeddings and similarity search
    Embedding,
    /// Vision/multimodal
    Vision,
    /// Fast/lightweight tasks
    Fast,
}

/// Model size tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModelTier {
    /// ~1-3B parameters (very fast, limited capability)
    Tiny,
    /// ~7B parameters (good balance)
    Small,
    /// ~13-14B parameters (high quality)
    Medium,
    /// ~30-34B parameters (very high quality)
    Large,
    /// ~70B+ parameters (state-of-the-art)
    XLarge,
}

/// Quantization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationRecommendation {
    pub format: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub quality_score: f32, // 0-100
    pub speed_multiplier: f32, // vs FP16
}

/// Recommended model with full details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedModel {
    /// Model name (e.g., "Qwen2.5-Coder-7B")
    pub name: String,
    /// HuggingFace model ID
    pub model_id: String,
    /// GGUF filename (for llama.cpp)
    pub gguf_filename: Option<String>,
    /// Ollama model name
    pub ollama_name: Option<String>,
    /// Model category
    pub category: ModelCategory,
    /// Size tier
    pub tier: ModelTier,
    /// Parameter count in billions
    pub params_b: f32,
    /// Context window size
    pub context_size: u32,
    /// Quality score (0-100 based on benchmarks)
    pub quality_score: f32,
    /// Speed score (0-100, higher = faster)
    pub speed_score: f32,
    /// Recommended quantizations
    pub quantizations: Vec<QuantizationRecommendation>,
    /// Why this model is recommended
    pub recommendation_reason: String,
    /// Benchmark scores (HumanEval, MMLU, etc.)
    pub benchmarks: std::collections::HashMap<String, f32>,
}

/// Hardware profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub cpu_cores: u32,
    pub gpu_vendor: String, // AMD, NVIDIA, Intel, None
    pub gpu_name: Option<String>,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub cpu_name: Option<String>,
}

/// GPU-specific optimization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuOptimization {
    /// GPU vendor
    pub vendor: String,
    /// Recommended backend for llama.cpp
    pub llama_cpp_backend: String,
    /// Environment variables to set
    pub env_vars: Vec<(String, String)>,
    /// Flash attention support
    pub supports_flash_attention: bool,
    /// Tensor cores available
    pub has_tensor_cores: bool,
    /// Recommended batch size
    pub optimal_batch_size: u32,
    /// Memory bandwidth (GB/s)
    pub memory_bandwidth_gbps: Option<f32>,
    /// Build flags for llama.cpp
    pub build_flags: String,
    /// Additional optimization tips
    pub optimization_tips: Vec<String>,
}

/// CPU-specific optimization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuOptimization {
    /// CPU architecture
    pub architecture: String,
    /// Recommended thread count
    pub optimal_threads: u32,
    /// Thread affinity recommendation
    pub thread_affinity: String,
    /// NUMA awareness
    pub numa_enabled: bool,
    /// Optimal batch size for CPU
    pub optimal_batch_size: u32,
    /// Memory mapping recommendation
    pub use_mmap: bool,
    /// Lock memory recommendation
    pub use_mlock: bool,
    /// Build flags for llama.cpp
    pub build_flags: String,
    /// Additional optimization tips
    pub optimization_tips: Vec<String>,
}

/// Combined hardware optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareOptimization {
    pub gpu: Option<GpuOptimization>,
    pub cpu: CpuOptimization,
    /// Recommended inference mode
    pub inference_mode: String, // "GPU", "CPU", "Hybrid"
    /// Complete llama.cpp command
    pub llama_cpp_command: String,
    /// Ollama configuration
    pub ollama_config: String,
    /// Expected performance
    pub expected_performance: String,
}

/// Model recommendation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub model: RecommendedModel,
    pub best_quantization: QuantizationRecommendation,
    pub expected_tokens_per_sec: f32,
    pub fits_in_vram: bool,
    pub fits_in_ram: bool,
    pub recommended_gpu_layers: i32,
    pub recommended_threads: u32,
    pub recommended_batch_size: u32,
    pub recommended_context: u32,
    pub llama_cpp_args: String,
    pub ollama_command: Option<String>,
}

// ============================================================================
// CURATED MODEL DATABASE (2025/2026 Best Models)
// ============================================================================

/// Get curated list of best models for each category
pub fn get_model_database() -> Vec<RecommendedModel> {
    vec![
        // =====================================================================
        // CODING MODELS
        // =====================================================================
        RecommendedModel {
            name: "Qwen2.5-Coder-7B-Instruct".to_string(),
            model_id: "Qwen/Qwen2.5-Coder-7B-Instruct".to_string(),
            gguf_filename: Some("qwen2.5-coder-7b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("qwen2.5-coder:7b".to_string()),
            category: ModelCategory::Coding,
            tier: ModelTier::Small,
            params_b: 7.0,
            context_size: 32768,
            quality_score: 88.0,
            speed_score: 85.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 4.5, ram_gb: 6.0, quality_score: 95.0, speed_multiplier: 1.8 },
                QuantizationRecommendation { format: "Q5_K_M".to_string(), vram_gb: 5.5, ram_gb: 7.0, quality_score: 97.0, speed_multiplier: 1.5 },
                QuantizationRecommendation { format: "Q8_0".to_string(), vram_gb: 8.0, ram_gb: 10.0, quality_score: 99.0, speed_multiplier: 1.2 },
            ],
            recommendation_reason: "Best coding model under 8B. Outperforms GPT-4 on HumanEval. Excellent for code completion, refactoring, and debugging.".to_string(),
            benchmarks: [("HumanEval".to_string(), 88.4), ("MBPP".to_string(), 83.5), ("MultiPL-E".to_string(), 75.2)].into_iter().collect(),
        },
        RecommendedModel {
            name: "DeepSeek-Coder-V2-Lite-16B".to_string(),
            model_id: "deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct".to_string(),
            gguf_filename: Some("deepseek-coder-v2-lite-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("deepseek-coder-v2:16b".to_string()),
            category: ModelCategory::Coding,
            tier: ModelTier::Medium,
            params_b: 16.0,
            context_size: 128000,
            quality_score: 92.0,
            speed_score: 70.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 10.0, ram_gb: 12.0, quality_score: 94.0, speed_multiplier: 1.6 },
                QuantizationRecommendation { format: "Q5_K_M".to_string(), vram_gb: 12.0, ram_gb: 14.0, quality_score: 96.0, speed_multiplier: 1.4 },
            ],
            recommendation_reason: "128K context, MoE architecture. Excellent for large codebase understanding and long-form code generation.".to_string(),
            benchmarks: [("HumanEval".to_string(), 90.2), ("MBPP".to_string(), 87.3)].into_iter().collect(),
        },
        RecommendedModel {
            name: "Qwen2.5-Coder-32B-Instruct".to_string(),
            model_id: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(),
            gguf_filename: Some("qwen2.5-coder-32b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("qwen2.5-coder:32b".to_string()),
            category: ModelCategory::Coding,
            tier: ModelTier::Large,
            params_b: 32.0,
            context_size: 32768,
            quality_score: 95.0,
            speed_score: 50.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 20.0, ram_gb: 24.0, quality_score: 94.0, speed_multiplier: 1.5 },
                QuantizationRecommendation { format: "Q3_K_M".to_string(), vram_gb: 16.0, ram_gb: 20.0, quality_score: 90.0, speed_multiplier: 1.7 },
            ],
            recommendation_reason: "State-of-the-art open coding model. Rivals Claude 3.5 Sonnet on coding benchmarks.".to_string(),
            benchmarks: [("HumanEval".to_string(), 92.7), ("MBPP".to_string(), 90.1), ("LiveCodeBench".to_string(), 55.2)].into_iter().collect(),
        },

        // =====================================================================
        // CHAT / GENERAL MODELS
        // =====================================================================
        RecommendedModel {
            name: "Llama-3.3-70B-Instruct".to_string(),
            model_id: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
            gguf_filename: Some("llama-3.3-70b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("llama3.3:70b".to_string()),
            category: ModelCategory::Chat,
            tier: ModelTier::XLarge,
            params_b: 70.0,
            context_size: 131072,
            quality_score: 96.0,
            speed_score: 30.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 40.0, ram_gb: 48.0, quality_score: 94.0, speed_multiplier: 1.5 },
                QuantizationRecommendation { format: "Q3_K_M".to_string(), vram_gb: 32.0, ram_gb: 40.0, quality_score: 88.0, speed_multiplier: 1.8 },
                QuantizationRecommendation { format: "IQ2_M".to_string(), vram_gb: 22.0, ram_gb: 28.0, quality_score: 80.0, speed_multiplier: 2.0 },
            ],
            recommendation_reason: "Best open-weight 70B model. 128K context, excellent instruction following and reasoning.".to_string(),
            benchmarks: [("MMLU".to_string(), 86.0), ("HumanEval".to_string(), 88.7), ("GPQA".to_string(), 50.7)].into_iter().collect(),
        },
        RecommendedModel {
            name: "Mistral-Small-24B-Instruct".to_string(),
            model_id: "mistralai/Mistral-Small-Instruct-2409".to_string(),
            gguf_filename: Some("mistral-small-instruct-24b-q4_k_m.gguf".to_string()),
            ollama_name: Some("mistral-small:24b".to_string()),
            category: ModelCategory::Chat,
            tier: ModelTier::Large,
            params_b: 24.0,
            context_size: 32768,
            quality_score: 89.0,
            speed_score: 60.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 14.0, ram_gb: 18.0, quality_score: 95.0, speed_multiplier: 1.6 },
                QuantizationRecommendation { format: "Q5_K_M".to_string(), vram_gb: 17.0, ram_gb: 21.0, quality_score: 97.0, speed_multiplier: 1.4 },
            ],
            recommendation_reason: "Excellent balance of quality and speed. Strong multilingual support. Function calling built-in.".to_string(),
            benchmarks: [("MMLU".to_string(), 81.0), ("MT-Bench".to_string(), 8.4)].into_iter().collect(),
        },
        RecommendedModel {
            name: "Llama-3.2-3B-Instruct".to_string(),
            model_id: "meta-llama/Llama-3.2-3B-Instruct".to_string(),
            gguf_filename: Some("llama-3.2-3b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("llama3.2:3b".to_string()),
            category: ModelCategory::Fast,
            tier: ModelTier::Tiny,
            params_b: 3.0,
            context_size: 131072,
            quality_score: 72.0,
            speed_score: 98.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 2.0, ram_gb: 3.0, quality_score: 95.0, speed_multiplier: 2.0 },
                QuantizationRecommendation { format: "Q8_0".to_string(), vram_gb: 3.5, ram_gb: 4.5, quality_score: 99.0, speed_multiplier: 1.5 },
            ],
            recommendation_reason: "Ultra-fast for simple tasks. Great for autocomplete, summarization, quick Q&A.".to_string(),
            benchmarks: [("MMLU".to_string(), 63.4), ("IFEval".to_string(), 77.4)].into_iter().collect(),
        },

        // =====================================================================
        // REASONING MODELS
        // =====================================================================
        RecommendedModel {
            name: "Qwen2.5-72B-Instruct".to_string(),
            model_id: "Qwen/Qwen2.5-72B-Instruct".to_string(),
            gguf_filename: Some("qwen2.5-72b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("qwen2.5:72b".to_string()),
            category: ModelCategory::Reasoning,
            tier: ModelTier::XLarge,
            params_b: 72.0,
            context_size: 131072,
            quality_score: 97.0,
            speed_score: 28.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 42.0, ram_gb: 50.0, quality_score: 94.0, speed_multiplier: 1.5 },
                QuantizationRecommendation { format: "Q3_K_M".to_string(), vram_gb: 34.0, ram_gb: 42.0, quality_score: 88.0, speed_multiplier: 1.8 },
            ],
            recommendation_reason: "Best open reasoning model. Matches GPT-4 on math and logic benchmarks.".to_string(),
            benchmarks: [("MMLU".to_string(), 86.1), ("MATH".to_string(), 83.1), ("GPQA".to_string(), 49.0)].into_iter().collect(),
        },
        RecommendedModel {
            name: "DeepSeek-R1-Distill-Qwen-14B".to_string(),
            model_id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-14B".to_string(),
            gguf_filename: Some("deepseek-r1-distill-qwen-14b-q4_k_m.gguf".to_string()),
            ollama_name: Some("deepseek-r1:14b".to_string()),
            category: ModelCategory::Reasoning,
            tier: ModelTier::Medium,
            params_b: 14.0,
            context_size: 32768,
            quality_score: 90.0,
            speed_score: 65.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 9.0, ram_gb: 11.0, quality_score: 94.0, speed_multiplier: 1.6 },
                QuantizationRecommendation { format: "Q5_K_M".to_string(), vram_gb: 11.0, ram_gb: 13.0, quality_score: 96.0, speed_multiplier: 1.4 },
            ],
            recommendation_reason: "Distilled reasoning from DeepSeek-R1. Chain-of-thought built-in. Excellent math/logic.".to_string(),
            benchmarks: [("MATH".to_string(), 79.9), ("AIME".to_string(), 69.7), ("LiveCodeBench".to_string(), 53.1)].into_iter().collect(),
        },

        // =====================================================================
        // EMBEDDING MODELS
        // =====================================================================
        RecommendedModel {
            name: "BGE-M3".to_string(),
            model_id: "BAAI/bge-m3".to_string(),
            gguf_filename: None,
            ollama_name: Some("bge-m3".to_string()),
            category: ModelCategory::Embedding,
            tier: ModelTier::Tiny,
            params_b: 0.5,
            context_size: 8192,
            quality_score: 92.0,
            speed_score: 95.0,
            quantizations: vec![
                QuantizationRecommendation { format: "FP16".to_string(), vram_gb: 1.2, ram_gb: 1.5, quality_score: 100.0, speed_multiplier: 1.0 },
            ],
            recommendation_reason: "Best multilingual embedding model. Supports dense, sparse, and ColBERT retrieval.".to_string(),
            benchmarks: [("MTEB".to_string(), 68.2), ("MIRACL".to_string(), 71.5)].into_iter().collect(),
        },
        RecommendedModel {
            name: "Nomic-Embed-Text-v1.5".to_string(),
            model_id: "nomic-ai/nomic-embed-text-v1.5".to_string(),
            gguf_filename: Some("nomic-embed-text-v1.5-q8_0.gguf".to_string()),
            ollama_name: Some("nomic-embed-text".to_string()),
            category: ModelCategory::Embedding,
            tier: ModelTier::Tiny,
            params_b: 0.14,
            context_size: 8192,
            quality_score: 88.0,
            speed_score: 98.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q8_0".to_string(), vram_gb: 0.3, ram_gb: 0.4, quality_score: 99.0, speed_multiplier: 1.2 },
            ],
            recommendation_reason: "Lightweight, fast embeddings. Great for RAG with limited resources.".to_string(),
            benchmarks: [("MTEB".to_string(), 62.3)].into_iter().collect(),
        },

        // =====================================================================
        // VISION / MULTIMODAL
        // =====================================================================
        RecommendedModel {
            name: "LLaVA-v1.6-Mistral-7B".to_string(),
            model_id: "liuhaotian/llava-v1.6-mistral-7b".to_string(),
            gguf_filename: Some("llava-v1.6-mistral-7b-q4_k_m.gguf".to_string()),
            ollama_name: Some("llava:7b".to_string()),
            category: ModelCategory::Vision,
            tier: ModelTier::Small,
            params_b: 7.0,
            context_size: 4096,
            quality_score: 82.0,
            speed_score: 75.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 5.0, ram_gb: 7.0, quality_score: 94.0, speed_multiplier: 1.6 },
            ],
            recommendation_reason: "Best open vision model for 7B class. Image understanding, OCR, visual Q&A.".to_string(),
            benchmarks: [("VQAv2".to_string(), 81.8), ("TextVQA".to_string(), 64.9)].into_iter().collect(),
        },
        RecommendedModel {
            name: "Qwen2-VL-7B-Instruct".to_string(),
            model_id: "Qwen/Qwen2-VL-7B-Instruct".to_string(),
            gguf_filename: Some("qwen2-vl-7b-instruct-q4_k_m.gguf".to_string()),
            ollama_name: Some("qwen2-vl:7b".to_string()),
            category: ModelCategory::Vision,
            tier: ModelTier::Small,
            params_b: 7.0,
            context_size: 32768,
            quality_score: 87.0,
            speed_score: 70.0,
            quantizations: vec![
                QuantizationRecommendation { format: "Q4_K_M".to_string(), vram_gb: 5.5, ram_gb: 7.5, quality_score: 94.0, speed_multiplier: 1.5 },
            ],
            recommendation_reason: "State-of-the-art vision-language model. Video understanding, document analysis.".to_string(),
            benchmarks: [("DocVQA".to_string(), 94.5), ("ChartQA".to_string(), 83.0)].into_iter().collect(),
        },
    ]
}

// ============================================================================
// RECOMMENDATION ENGINE
// ============================================================================

/// Get model recommendations based on hardware and use case
pub fn get_recommendations(
    hardware: &HardwareProfile,
    category: Option<ModelCategory>,
    max_results: usize,
) -> Vec<ModelRecommendation> {
    let models = get_model_database();
    let mut recommendations = Vec::new();

    for model in models {
        // Filter by category if specified
        if let Some(cat) = category {
            if model.category != cat {
                continue;
            }
        }

        // Find best quantization that fits
        let mut best_quant: Option<&QuantizationRecommendation> = None;
        for quant in &model.quantizations {
            let fits_vram = quant.vram_gb <= hardware.vram_gb;
            let fits_ram = quant.ram_gb <= hardware.ram_gb;

            if fits_vram || fits_ram {
                // Prefer higher quality if it fits
                if best_quant.map_or(true, |bq| quant.quality_score > bq.quality_score) {
                    best_quant = Some(quant);
                }
            }
        }

        // Skip if no quantization fits
        let Some(quant) = best_quant else { continue };

        // Calculate optimal settings
        let fits_in_vram = quant.vram_gb <= hardware.vram_gb;
        let fits_in_ram = quant.ram_gb <= hardware.ram_gb;

        // GPU layers: all if fits, partial if hybrid, 0 if CPU-only
        let gpu_layers = if fits_in_vram {
            -1 // All layers
        } else if hardware.vram_gb > 2.0 {
            // Partial offload: estimate layers based on available VRAM
            let available_for_model = hardware.vram_gb - 1.0; // Reserve 1GB
            let layers = (available_for_model / quant.vram_gb * 40.0) as i32;
            layers.max(0).min(60)
        } else {
            0 // CPU only
        };

        // Threads: physical cores - 2 for system
        let threads = (hardware.cpu_cores.saturating_sub(2)).max(1);

        // Batch size: larger for CPU, smaller for GPU
        let batch_size = if gpu_layers == -1 { 512 } else { 2048 };

        // Context: limit based on available memory
        let max_context = if fits_in_vram {
            model.context_size
        } else {
            model.context_size.min(8192)
        };

        // Estimate tokens/sec
        let base_tps = if fits_in_vram {
            match model.tier {
                ModelTier::Tiny => 150.0,
                ModelTier::Small => 80.0,
                ModelTier::Medium => 40.0,
                ModelTier::Large => 20.0,
                ModelTier::XLarge => 10.0,
            }
        } else {
            // CPU inference is slower
            match model.tier {
                ModelTier::Tiny => 50.0,
                ModelTier::Small => 20.0,
                ModelTier::Medium => 10.0,
                ModelTier::Large => 5.0,
                ModelTier::XLarge => 2.0,
            }
        };
        let expected_tps = base_tps * quant.speed_multiplier;

        // Build llama.cpp command
        let llama_cpp_args = format!(
            "-ngl {} -t {} -b {} -c {} --flash-attn",
            gpu_layers, threads, batch_size, max_context
        );

        // Build ollama command
        let ollama_command = model.ollama_name.as_ref().map(|name| {
            format!("ollama run {} --num-gpu {} --num-thread {}", name, gpu_layers.max(0), threads)
        });

        recommendations.push(ModelRecommendation {
            model: model.clone(),
            best_quantization: quant.clone(),
            expected_tokens_per_sec: expected_tps,
            fits_in_vram,
            fits_in_ram,
            recommended_gpu_layers: gpu_layers,
            recommended_threads: threads,
            recommended_batch_size: batch_size,
            recommended_context: max_context,
            llama_cpp_args,
            ollama_command,
        });
    }

    // Sort by quality * speed score
    recommendations.sort_by(|a, b| {
        let score_a = a.model.quality_score * a.model.speed_score;
        let score_b = b.model.quality_score * b.model.speed_score;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    recommendations.truncate(max_results);
    recommendations
}

/// Get optimal model for specific use case
pub fn recommend_for_task(
    hardware: &HardwareProfile,
    task: &str,
) -> Option<ModelRecommendation> {
    let task_lower = task.to_lowercase();

    let category = if task_lower.contains("code") || task_lower.contains("program") || task_lower.contains("debug") {
        ModelCategory::Coding
    } else if task_lower.contains("math") || task_lower.contains("reason") || task_lower.contains("logic") {
        ModelCategory::Reasoning
    } else if task_lower.contains("image") || task_lower.contains("vision") || task_lower.contains("picture") {
        ModelCategory::Vision
    } else if task_lower.contains("embed") || task_lower.contains("search") || task_lower.contains("rag") {
        ModelCategory::Embedding
    } else if task_lower.contains("fast") || task_lower.contains("quick") || task_lower.contains("simple") {
        ModelCategory::Fast
    } else if task_lower.contains("write") || task_lower.contains("story") || task_lower.contains("creative") {
        ModelCategory::Creative
    } else {
        ModelCategory::Chat
    };

    get_recommendations(hardware, Some(category), 1).into_iter().next()
}

/// Compare current model with recommended alternative
pub fn compare_with_alternative(
    current_model: &str,
    hardware: &HardwareProfile,
) -> Option<(RecommendedModel, String)> {
    let models = get_model_database();

    // Find current model in database
    let current = models.iter().find(|m| {
        m.name.to_lowercase().contains(&current_model.to_lowercase())
            || m.model_id.to_lowercase().contains(&current_model.to_lowercase())
    });

    let Some(current) = current else {
        return None;
    };

    // Find better alternative in same category
    let recommendations = get_recommendations(hardware, Some(current.category), 3);

    for rec in recommendations {
        if rec.model.name != current.name {
            if rec.model.quality_score > current.quality_score
                || (rec.model.quality_score >= current.quality_score - 5.0
                    && rec.model.speed_score > current.speed_score + 10.0)
            {
                let reason = if rec.model.quality_score > current.quality_score {
                    format!(
                        "Higher quality: {} vs {} on benchmarks",
                        rec.model.quality_score, current.quality_score
                    )
                } else {
                    format!(
                        "Much faster: {:.0} vs {:.0} tokens/sec estimated",
                        rec.expected_tokens_per_sec,
                        rec.expected_tokens_per_sec / rec.model.speed_score * current.speed_score
                    )
                };
                return Some((rec.model, reason));
            }
        }
    }

    None
}

// ============================================================================
// HARDWARE-SPECIFIC OPTIMIZATIONS
// ============================================================================

/// Get NVIDIA GPU optimization settings
pub fn get_nvidia_optimization(gpu_name: &str, vram_gb: f32) -> GpuOptimization {
    // Detect GPU generation and capabilities
    let is_rtx_40_series = gpu_name.contains("40") || gpu_name.contains("4090") || gpu_name.contains("4080");
    let is_rtx_30_series = gpu_name.contains("30") || gpu_name.contains("3090") || gpu_name.contains("3080");
    let has_tensor_cores = is_rtx_40_series || is_rtx_30_series || gpu_name.contains("RTX");

    // Memory bandwidth estimates
    let bandwidth = if is_rtx_40_series { Some(1000.0) }
        else if is_rtx_30_series { Some(760.0) }
        else { Some(500.0) };

    GpuOptimization {
        vendor: "NVIDIA".to_string(),
        llama_cpp_backend: "cuda".to_string(),
        env_vars: vec![
            ("CUDA_VISIBLE_DEVICES".to_string(), "0".to_string()),
            ("GGML_CUDA_NO_PINNED".to_string(), "0".to_string()),
        ],
        supports_flash_attention: true,
        has_tensor_cores,
        optimal_batch_size: if vram_gb >= 24.0 { 2048 } else if vram_gb >= 12.0 { 1024 } else { 512 },
        memory_bandwidth_gbps: bandwidth,
        build_flags: "-DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=native".to_string(),
        optimization_tips: vec![
            "Use flash attention (--flash-attn) for memory efficiency".to_string(),
            format!("Tensor cores {} for matrix operations", if has_tensor_cores { "available" } else { "not available" }),
            "Set CUDA_VISIBLE_DEVICES to select specific GPU".to_string(),
            "Use -ngl -1 for full GPU offload if VRAM allows".to_string(),
            if vram_gb >= 24.0 {
                "24GB+ VRAM: Can run 70B Q4 models with ~8K context".to_string()
            } else if vram_gb >= 16.0 {
                "16GB VRAM: Ideal for 13B-30B models".to_string()
            } else {
                "12GB VRAM: Best for 7B-13B models".to_string()
            },
        ],
    }
}

/// Get AMD GPU optimization settings
pub fn get_amd_optimization(gpu_name: &str, vram_gb: f32) -> GpuOptimization {
    // Detect GPU generation
    let is_rdna3 = gpu_name.contains("7900") || gpu_name.contains("7800") || gpu_name.contains("7700");
    let is_rdna2 = gpu_name.contains("6900") || gpu_name.contains("6800") || gpu_name.contains("6700");

    // GFX version for ROCm
    let gfx_version = if is_rdna3 { "11.0.0" }
        else if is_rdna2 { "10.3.0" }
        else { "10.1.0" };

    // Memory bandwidth estimates
    let bandwidth = if gpu_name.contains("7900 XTX") { Some(960.0) }
        else if gpu_name.contains("7900 XT") { Some(800.0) }
        else if gpu_name.contains("7800 XT") { Some(624.0) }
        else { Some(500.0) };

    GpuOptimization {
        vendor: "AMD".to_string(),
        llama_cpp_backend: "hip".to_string(),
        env_vars: vec![
            ("HSA_OVERRIDE_GFX_VERSION".to_string(), gfx_version.to_string()),
            ("HIP_VISIBLE_DEVICES".to_string(), "0".to_string()),
            ("ROCM_PATH".to_string(), "/opt/rocm".to_string()),
            ("HSA_ENABLE_SDMA".to_string(), "0".to_string()), // Fix for some ROCm issues
        ],
        supports_flash_attention: is_rdna3, // Flash attention works best on RDNA3
        has_tensor_cores: false, // AMD uses Wave Matrix (WMMA) instead
        optimal_batch_size: if vram_gb >= 24.0 { 2048 } else if vram_gb >= 16.0 { 1024 } else { 512 },
        memory_bandwidth_gbps: bandwidth,
        build_flags: "-DGGML_HIP=ON -DAMDGPU_TARGETS=gfx1100;gfx1030;gfx1010".to_string(),
        optimization_tips: vec![
            format!("Set HSA_OVERRIDE_GFX_VERSION={} for ROCm compatibility", gfx_version),
            "Use ROCm 6.0+ for best performance".to_string(),
            if is_rdna3 {
                "RDNA3: Flash attention supported, use --flash-attn".to_string()
            } else {
                "RDNA2: Flash attention may have issues, test carefully".to_string()
            },
            "AMD GPUs benefit from larger batch sizes (-b 2048)".to_string(),
            format!("{}GB VRAM: {}", vram_gb,
                if vram_gb >= 24.0 { "Can run 70B Q3/Q4 models" }
                else if vram_gb >= 16.0 { "Ideal for 13B-32B models" }
                else { "Best for 7B-13B models" }
            ),
            "Check rocm-smi for temperature and power monitoring".to_string(),
        ],
    }
}

/// Get Intel GPU optimization settings
pub fn get_intel_optimization(gpu_name: &str, vram_gb: f32) -> GpuOptimization {
    let is_arc = gpu_name.to_lowercase().contains("arc");

    GpuOptimization {
        vendor: "Intel".to_string(),
        llama_cpp_backend: "sycl".to_string(),
        env_vars: vec![
            ("ONEAPI_DEVICE_SELECTOR".to_string(), "level_zero:gpu".to_string()),
        ],
        supports_flash_attention: is_arc,
        has_tensor_cores: is_arc, // XMX units
        optimal_batch_size: 512,
        memory_bandwidth_gbps: Some(560.0), // Arc A770
        build_flags: "-DGGML_SYCL=ON -DCMAKE_C_COMPILER=icx -DCMAKE_CXX_COMPILER=icpx".to_string(),
        optimization_tips: vec![
            "Intel Arc GPUs use SYCL backend".to_string(),
            "Install Intel oneAPI toolkit for best performance".to_string(),
            "Arc A770 16GB can run 13B models comfortably".to_string(),
        ],
    }
}

/// Get CPU optimization settings
pub fn get_cpu_optimization(cpu_cores: u32, ram_gb: f32, cpu_name: Option<&str>) -> CpuOptimization {
    // Detect CPU capabilities
    let is_amd = cpu_name.map(|n| n.to_lowercase().contains("amd") || n.contains("Ryzen")).unwrap_or(false);
    let is_intel = cpu_name.map(|n| n.to_lowercase().contains("intel") || n.contains("Core")).unwrap_or(false);

    // Optimal thread count (physical cores - 2 for system)
    let optimal_threads = (cpu_cores / 2).saturating_sub(1).max(1);

    // NUMA detection (simplified)
    let numa_enabled = cpu_cores >= 16;

    // Memory requirements
    let use_mmap = ram_gb < 32.0; // Use mmap if limited RAM
    let use_mlock = ram_gb >= 64.0; // Lock in RAM if plenty available

    let architecture = if is_amd { "AMD Zen" } else if is_intel { "Intel Core" } else { "x86_64" };

    CpuOptimization {
        architecture: architecture.to_string(),
        optimal_threads,
        thread_affinity: if numa_enabled {
            "numactl --cpunodebind=0 --membind=0".to_string()
        } else {
            "taskset -c 0-{}".to_string().replace("{}", &(optimal_threads - 1).to_string())
        },
        numa_enabled,
        optimal_batch_size: if cpu_cores >= 16 { 2048 } else { 1024 },
        use_mmap,
        use_mlock,
        build_flags: if is_amd {
            "-DGGML_NATIVE=ON -DGGML_AVX2=ON -DGGML_FMA=ON".to_string()
        } else {
            "-DGGML_NATIVE=ON -DGGML_AVX512=ON -DGGML_AVX2=ON".to_string()
        },
        optimization_tips: vec![
            format!("Use -t {} threads (physical cores - 1)", optimal_threads),
            format!("RAM: {:.0}GB available, {}", ram_gb, if use_mmap { "using mmap for large models" } else { "can fit models in RAM" }),
            if is_amd {
                "AMD CPUs: AVX2 + FMA give best performance".to_string()
            } else if is_intel {
                "Intel CPUs: AVX-512 supported on recent models".to_string()
            } else {
                "Enable native optimizations with -DGGML_NATIVE=ON".to_string()
            },
            format!("Batch size: -b {} recommended for CPU inference", if cpu_cores >= 16 { 2048 } else { 1024 }),
            if numa_enabled {
                "Multi-socket: Use numactl for NUMA-aware execution".to_string()
            } else {
                "Single-socket: Thread affinity can improve cache hits".to_string()
            },
            "CPU inference: Q4_K_M offers best speed/quality balance".to_string(),
            format!("Expected speed: ~{} tokens/sec for 7B Q4 model", optimal_threads * 3),
        ],
    }
}

/// Get combined hardware optimization for llama.cpp
pub fn get_hardware_optimization(hardware: &HardwareProfile) -> HardwareOptimization {
    let vendor = hardware.gpu_vendor.to_lowercase();

    // Get GPU optimization if available
    let gpu_opt = if vendor.contains("nvidia") || vendor.contains("cuda") {
        Some(get_nvidia_optimization(
            hardware.gpu_name.as_deref().unwrap_or("NVIDIA GPU"),
            hardware.vram_gb
        ))
    } else if vendor.contains("amd") || vendor.contains("radeon") || vendor.contains("rocm") {
        Some(get_amd_optimization(
            hardware.gpu_name.as_deref().unwrap_or("AMD GPU"),
            hardware.vram_gb
        ))
    } else if vendor.contains("intel") || vendor.contains("arc") {
        Some(get_intel_optimization(
            hardware.gpu_name.as_deref().unwrap_or("Intel GPU"),
            hardware.vram_gb
        ))
    } else {
        None
    };

    // Get CPU optimization
    let cpu_opt = get_cpu_optimization(
        hardware.cpu_cores,
        hardware.ram_gb,
        hardware.cpu_name.as_deref()
    );

    // Determine best inference mode
    let inference_mode = if hardware.vram_gb >= 8.0 && gpu_opt.is_some() {
        "GPU".to_string()
    } else if hardware.vram_gb >= 4.0 && gpu_opt.is_some() {
        "Hybrid".to_string()
    } else {
        "CPU".to_string()
    };

    // Build llama.cpp command
    let llama_cpp_command = match &inference_mode[..] {
        "GPU" => format!(
            "llama-server -m model.gguf -ngl -1 -c 4096 --flash-attn -t {} -b {}",
            cpu_opt.optimal_threads,
            gpu_opt.as_ref().map(|g| g.optimal_batch_size).unwrap_or(512)
        ),
        "Hybrid" => {
            let gpu_layers = (hardware.vram_gb * 4.0) as i32; // ~4 layers per GB
            format!(
                "llama-server -m model.gguf -ngl {} -c 4096 -t {} -b {} {}",
                gpu_layers,
                cpu_opt.optimal_threads,
                cpu_opt.optimal_batch_size,
                if cpu_opt.use_mmap { "--mmap" } else { "" }
            )
        },
        _ => format!(
            "llama-server -m model.gguf -ngl 0 -c 4096 -t {} -b {} {} {}",
            cpu_opt.optimal_threads,
            cpu_opt.optimal_batch_size,
            if cpu_opt.use_mmap { "--mmap" } else { "" },
            if cpu_opt.use_mlock { "--mlock" } else { "" }
        ),
    };

    // Build Ollama config
    let ollama_config = format!(
        "OLLAMA_NUM_GPU={} OLLAMA_NUM_THREAD={} ollama run model",
        if inference_mode == "GPU" { -1 } else if inference_mode == "Hybrid" { (hardware.vram_gb * 4.0) as i32 } else { 0 },
        cpu_opt.optimal_threads
    );

    // Expected performance estimate
    let expected_performance = match &inference_mode[..] {
        "GPU" => format!(
            "GPU mode: ~{}-{} tokens/sec (7B Q4)",
            (hardware.vram_gb * 8.0) as u32,
            (hardware.vram_gb * 12.0) as u32
        ),
        "Hybrid" => format!(
            "Hybrid mode: ~{}-{} tokens/sec (7B Q4)",
            (cpu_opt.optimal_threads * 2) as u32,
            (cpu_opt.optimal_threads * 4) as u32
        ),
        _ => format!(
            "CPU mode: ~{}-{} tokens/sec (7B Q4)",
            cpu_opt.optimal_threads,
            cpu_opt.optimal_threads * 3
        ),
    };

    HardwareOptimization {
        gpu: gpu_opt,
        cpu: cpu_opt,
        inference_mode,
        llama_cpp_command,
        ollama_config,
        expected_performance,
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Tauri command: Get model recommendations
#[tauri::command]
pub fn cmd_get_model_recommendations(
    vram_gb: f32,
    ram_gb: f32,
    cpu_cores: u32,
    gpu_vendor: String,
    category: Option<String>,
) -> Vec<ModelRecommendation> {
    let hardware = HardwareProfile {
        vram_gb,
        ram_gb,
        cpu_cores,
        gpu_vendor,
        gpu_name: None,
        has_avx2: true, // Assume modern CPU
        has_avx512: false,
        cpu_name: None,
    };

    let cat = category.and_then(|c| match c.to_lowercase().as_str() {
        "coding" | "code" => Some(ModelCategory::Coding),
        "chat" => Some(ModelCategory::Chat),
        "reasoning" | "math" => Some(ModelCategory::Reasoning),
        "embedding" | "embed" => Some(ModelCategory::Embedding),
        "vision" | "image" => Some(ModelCategory::Vision),
        "fast" | "quick" => Some(ModelCategory::Fast),
        "creative" | "writing" => Some(ModelCategory::Creative),
        _ => None,
    });

    get_recommendations(&hardware, cat, 10)
}

/// Tauri command: Get model recommendations (extended version)
#[tauri::command]
pub fn cmd_get_model_recommendations_extended(
    vram_gb: f32,
    ram_gb: f32,
    cpu_cores: u32,
    gpu_vendor: String,
    gpu_name: Option<String>,
    cpu_name: Option<String>,
    category: Option<String>,
) -> (Vec<ModelRecommendation>, HardwareOptimization) {
    let hardware = HardwareProfile {
        vram_gb,
        ram_gb,
        cpu_cores,
        gpu_vendor: gpu_vendor.clone(),
        gpu_name,
        has_avx2: true,
        has_avx512: false,
        cpu_name,
    };

    let cat = category.and_then(|c| match c.to_lowercase().as_str() {
        "coding" | "code" => Some(ModelCategory::Coding),
        "chat" => Some(ModelCategory::Chat),
        "reasoning" | "math" => Some(ModelCategory::Reasoning),
        "embedding" | "embed" => Some(ModelCategory::Embedding),
        "vision" | "image" => Some(ModelCategory::Vision),
        "fast" | "quick" => Some(ModelCategory::Fast),
        "creative" | "writing" => Some(ModelCategory::Creative),
        _ => None,
    });

    let recommendations = get_recommendations(&hardware, cat, 10);
    let optimization = get_hardware_optimization(&hardware);

    (recommendations, optimization)
}

/// Tauri command: Get all available models in database
#[tauri::command]
pub fn cmd_get_model_database() -> Vec<RecommendedModel> {
    get_model_database()
}

/// Tauri command: Recommend model for specific task
#[tauri::command]
pub fn cmd_recommend_for_task(
    vram_gb: f32,
    ram_gb: f32,
    cpu_cores: u32,
    gpu_vendor: String,
    task: String,
) -> Option<ModelRecommendation> {
    let hardware = HardwareProfile {
        vram_gb,
        ram_gb,
        cpu_cores,
        gpu_vendor,
        gpu_name: None,
        has_avx2: true,
        has_avx512: false,
        cpu_name: None,
    };

    recommend_for_task(&hardware, &task)
}

/// Tauri command: Get hardware-specific optimization settings
#[tauri::command]
pub fn cmd_get_hardware_optimization(
    vram_gb: f32,
    ram_gb: f32,
    cpu_cores: u32,
    gpu_vendor: String,
    gpu_name: Option<String>,
    cpu_name: Option<String>,
) -> HardwareOptimization {
    let hardware = HardwareProfile {
        vram_gb,
        ram_gb,
        cpu_cores,
        gpu_vendor,
        gpu_name,
        has_avx2: true,
        has_avx512: false,
        cpu_name,
    };

    get_hardware_optimization(&hardware)
}

/// Tauri command: Get NVIDIA-specific optimization
#[tauri::command]
pub fn cmd_get_nvidia_optimization(gpu_name: String, vram_gb: f32) -> GpuOptimization {
    get_nvidia_optimization(&gpu_name, vram_gb)
}

/// Tauri command: Get AMD-specific optimization
#[tauri::command]
pub fn cmd_get_amd_optimization(gpu_name: String, vram_gb: f32) -> GpuOptimization {
    get_amd_optimization(&gpu_name, vram_gb)
}

/// Tauri command: Get CPU-specific optimization for llama.cpp
#[tauri::command]
pub fn cmd_get_cpu_optimization(cpu_cores: u32, ram_gb: f32, cpu_name: Option<String>) -> CpuOptimization {
    get_cpu_optimization(cpu_cores, ram_gb, cpu_name.as_deref())
}

/// Tauri command: Auto-detect hardware and get optimal settings
#[tauri::command]
pub fn cmd_auto_optimize() -> HardwareOptimization {
    use super::{get_cpu_info, get_memory_info, get_gpu_info};

    let cpu = get_cpu_info();
    let memory = get_memory_info();
    let gpus = get_gpu_info();

    let (gpu_vendor, gpu_name, vram_gb) = gpus.first()
        .map(|g| (g.vendor.clone(), Some(g.name.clone()), g.vram_total_bytes as f32 / (1024.0 * 1024.0 * 1024.0)))
        .unwrap_or(("None".to_string(), None, 0.0));

    let hardware = HardwareProfile {
        vram_gb,
        ram_gb: memory.total_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
        cpu_cores: cpu.logical_cores as u32,
        gpu_vendor,
        gpu_name,
        has_avx2: true,
        has_avx512: false,
        cpu_name: Some(cpu.name),
    };

    get_hardware_optimization(&hardware)
}
