# Enterprise LLM Inference Optimization Guide (2025-2026)

**Research Date**: March 2026
**Target**: ORK-Station Enterprise AI Infrastructure
**Hardware Reference**: AMD RX 7800 XT (16GB VRAM), ROCm 7.1.1

---

## Table of Contents

1. [Enterprise LLM Inference Optimization](#1-enterprise-llm-inference-optimization)
   - [Continuous Batching](#11-continuous-batching)
   - [KV Cache Optimization](#12-kv-cache-optimization)
   - [Speculative Decoding](#13-speculative-decoding)
   - [Model Parallelism Strategies](#14-model-parallelism-strategies)
2. [GPU/CPU Hybrid Inference](#2-gpucpu-hybrid-inference)
   - [Layer Distribution](#21-optimal-layer-distribution)
   - [Memory Bandwidth Optimization](#22-memory-bandwidth-optimization)
   - [NUMA Awareness](#23-numa-awareness-and-thread-affinity)
3. [Quantization Quality vs Speed](#3-quantization-quality-vs-speed-tradeoffs)
   - [GGUF Formats](#31-gguf-quantization-formats)
   - [GPTQ vs AWQ vs EXL2](#32-gptq-vs-awq-vs-exl2)
   - [Perplexity Analysis](#33-perplexity-degradation-analysis)
4. [Hardware Recommendations](#4-hardware-recommendations-for-local-llm)
   - [GPU Comparisons](#41-gpu-comparison-2025-2026)
   - [RAM Requirements](#42-ram-requirements-by-model-size)
   - [Power Efficiency](#43-power-efficiency-considerations)
5. [Inference Engine Comparison](#5-inference-engine-comparison)
6. [ORK-Station Implementation](#6-ork-station-implementation-recommendations)

---

## 1. Enterprise LLM Inference Optimization

### 1.1 Continuous Batching

Continuous batching is the cornerstone of modern LLM inference optimization, enabling **2-4x more requests per GPU** compared to static batching.

#### Key Principles

| Technique | Description | Benefit |
|-----------|-------------|---------|
| **Dynamic Request Insertion** | New requests enter mid-batch | Eliminates waiting time |
| **Ragged Batching** | Variable-length sequences without padding | Removes padding waste |
| **Chunked Prefill** | Breaks long prompts into manageable chunks | Stays within memory limits |

#### Performance Impact

According to 2025 MLPerf benchmarks:
- Traditional serving: **15-20 tokens/sec** per user on A100 GPUs
- vLLM continuous batching: **60+ tokens/sec** - a **3-4x improvement**

```python
# vLLM example configuration for continuous batching
from vllm import LLM, SamplingParams

llm = LLM(
    model="meta-llama/Llama-3.1-8B-Instruct",
    max_num_batched_tokens=8192,
    max_num_seqs=256,  # Maximum concurrent sequences
    enable_chunked_prefill=True,
    max_model_len=32768
)
```

#### Sources
- [Anyscale: 23x LLM Inference Throughput](https://www.anyscale.com/blog/continuous-batching-llm-inference)
- [Hugging Face: Continuous Batching from First Principles](https://huggingface.co/blog/continuous_batching)
- [vLLM: Memory, Scheduling & Batching Strategies](https://www.javacodegeeks.com/2025/10/under-the-hood-of-vllm-memory-scheduling-batching-strategies.html)

---

### 1.2 KV Cache Optimization

The KV (Key-Value) cache stores attention states from previous tokens, eliminating redundant computation. Modern optimization focuses on memory efficiency and sharing.

#### PagedAttention

PagedAttention revolutionized KV cache management by borrowing concepts from OS virtual memory:

| Feature | Traditional | PagedAttention |
|---------|-------------|----------------|
| Memory Layout | Contiguous | Non-contiguous blocks |
| Fragmentation | High (~40%) | Near-zero |
| Memory Reuse | Limited | Immediate recycling |
| Sharing | None | Copy-on-write |

**Impact**: Adding PagedAttention and continuous batching enables **2-4x more requests per GPU**, with KV-cache-aware routing reducing costs another **30-50%**.

#### LMCache (2025)

LMCache introduces enterprise-scale KV cache management with:
- Up to **15x throughput improvement** for document analysis
- Cross-request KV cache sharing
- Efficient prefix caching for multi-round conversations

```python
# LMCache integration with vLLM
import lmcache

cache = lmcache.KVCache(
    max_size_gb=32,
    eviction_policy="lru",
    prefix_sharing=True
)
```

#### FlashAttention-3/4

FlashAttention eliminates the O(N^2) memory bottleneck:

| Version | GPU Utilization | Memory Reduction | Hardware |
|---------|-----------------|------------------|----------|
| FlashAttention-2 | 35% | 60% | Ampere+ |
| FlashAttention-3 | **75%** | **80%** | Hopper (H100) |
| FlashAttention-4 | 85%+ | 85% | Hopper/Blackwell |

FlashAttention-3 uses FP8 precision while maintaining accuracy, making **32K+ token contexts practical on consumer hardware**.

#### Sources
- [LMCache Technical Report](https://lmcache.ai/tech_report.pdf)
- [arXiv: LMCache for Enterprise-Scale LLM Inference](https://arxiv.org/pdf/2510.09665)
- [FlashAttention-3 PyTorch Blog](https://pytorch.org/blog/flashattention-3/)
- [Tri Dao: FlashAttention-3](https://tridao.me/blog/2024/flash3/)

---

### 1.3 Speculative Decoding

Speculative decoding introduces intra-request parallelism by using a smaller "draft" model to propose tokens, which the target model verifies in parallel.

#### Key Techniques (2025-2026)

| Technique | Paper | Speedup | Notes |
|-----------|-------|---------|-------|
| **ReDrafter** | Apple ML Research | **2.8x** (H100), 2.3x (Apple Silicon) | Recurrent neural network draft model |
| **LongSpec** | Feb 2025 | 2.1x | Optimized for long-context |
| **Online Speculative Decoding (OSD)** | UC Berkeley | Adaptive | Continuously adapts to query distribution |
| **TurboSpec** | UC Berkeley | Variable | Closed-loop control for dynamic optimization |

#### Best Practices

```python
# Speculative decoding configuration
spec_config = {
    "draft_model": "meta-llama/Llama-3.2-1B-Instruct",
    "target_model": "meta-llama/Llama-3.1-70B-Instruct",
    "num_speculative_tokens": 5,
    "acceptance_threshold": 0.8,
    "adaptive": True  # OSD-style adaptation
}
```

**Critical Insight**: Speculative decoding effectiveness depends sensitively on:
- Workload characteristics
- Batch sizes
- Model configurations
- System conditions

Recent analysis reframes it as a **verification efficiency problem**, not just a drafting problem.

#### Sources
- [Google Research: Looking Back at Speculative Decoding](https://research.google/blog/looking-back-at-speculative-decoding/)
- [Apple: Mirror Speculative Decoding](https://machinelearning.apple.com/research/mirror)
- [Apple: Recurrent Drafter](https://machinelearning.apple.com/research/recurrent-drafter)
- [UC Berkeley: Efficient LLM System with Speculative Decoding](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2025/EECS-2025-224.html)
- [ICLR 2026: Speculative Speculative Decoding](https://openreview.net/pdf?id=aL1Wnml9Ef)

---

### 1.4 Model Parallelism Strategies

For models too large for a single GPU, parallelism strategies distribute computation effectively.

#### Parallelism Types

| Strategy | How It Works | Best For |
|----------|--------------|----------|
| **Tensor Parallelism (TP)** | Splits individual layers across GPUs | Single-node, high-bandwidth |
| **Pipeline Parallelism (PP)** | Splits model vertically by layers | Multi-node, lower bandwidth |
| **Data Parallelism (DP)** | Replicates model, splits data | High-throughput serving |
| **Expert Parallelism (EP)** | Distributes MoE experts | Mixture-of-Experts models |

#### N-D Parallelism (2025-2026 Trend)

Modern systems combine multiple parallelism types:

```
CP (Context) + PP (Pipeline) + EP (Expert) + TP (Tensor) across nodes
                    |
              Separate DP (Data)
```

**Key Innovation**: Disaggregating prefill and decoding tiers allows:
- Compute-heavy hardware for prefill
- Memory bandwidth-heavy hardware for decoding
- Better resource balancing with heterogeneous hardware

#### vLLM Configuration Example

```python
# Multi-GPU tensor parallelism
llm = LLM(
    model="meta-llama/Llama-3.1-70B-Instruct",
    tensor_parallel_size=4,  # 4 GPUs
    pipeline_parallel_size=1,
    distributed_executor_backend="ray"
)
```

#### Sources
- [AMD ROCm: Tensor Parallelism Analysis](https://rocm.blogs.amd.com/artificial-intelligence/tensor-parallelism/README.html)
- [Meta Engineering: Scaling LLM Inference](https://engineering.fb.com/2025/10/17/ai-research/scaling-llm-inference-innovations-tensor-parallelism-context-parallelism-expert-parallelism/)
- [vLLM: Parallelism and Scaling](https://docs.vllm.ai/en/stable/serving/parallelism_scaling/)
- [BentoML: Parallelisms Guide](https://bentoml.com/llm/inference-optimization/data-tensor-pipeline-expert-hybrid-parallelism)

---

## 2. GPU/CPU Hybrid Inference

### 2.1 Optimal Layer Distribution

Hybrid GPU/CPU inference enables running larger models than GPU VRAM allows.

#### Distribution Strategy

| Component | Optimal Location | Reason |
|-----------|------------------|--------|
| Attention layers | GPU | Parallel computation intensive |
| MLP layers | GPU preferred | Dense matrix multiplication |
| Embedding | CPU or GPU | Can offload if needed |
| KV Cache | Hybrid | GPU for active, CPU for overflow |

#### Layer Offloading in llama.cpp

```bash
# Partial GPU offloading with llama.cpp
./llama-server \
    --model ./models/llama-3.1-70b-q4_k_m.gguf \
    --n-gpu-layers 40 \  # Offload 40 layers to GPU
    --ctx-size 8192 \
    --batch-size 512
```

**Rule of Thumb** for 16GB VRAM (RX 7800 XT):
- 7B model: Full GPU (all layers)
- 13B model: ~35-40 layers on GPU
- 70B Q4: ~25-30 layers on GPU

#### Sources
- [NVIDIA: KV Cache Offload with CPU-GPU Memory Sharing](https://developer.nvidia.com/blog/accelerate-large-scale-llm-inference-and-kv-cache-offload-with-cpu-gpu-memory-sharing/)
- [arXiv: Mind the Memory Gap](https://arxiv.org/html/2503.08311v2)

---

### 2.2 Memory Bandwidth Optimization

Memory bandwidth is often the primary bottleneck in LLM inference, especially during the decode phase.

#### Bandwidth Comparison (2025-2026)

| Hardware | Memory | Bandwidth | Notes |
|----------|--------|-----------|-------|
| RTX 4090 | 24GB GDDR6X | 1008 GB/s | Consumer king |
| RX 7900 XTX | 24GB GDDR6 | 960 GB/s | Competitive |
| RX 7800 XT | 16GB GDDR6 | 624 GB/s | Good budget option |
| H100 SXM | 80GB HBM3 | 3350 GB/s | Datacenter standard |
| H200 | 141GB HBM3e | 4800 GB/s | +76% memory, +43% bandwidth |

#### Optimization Techniques

1. **Batch Size Optimization**: Larger batches amortize memory bandwidth costs
2. **Quantization**: Reduces memory footprint and bandwidth requirements
3. **KV Cache Compression**: Reduces attention memory traffic
4. **Prefetching**: Overlap computation with memory access

```python
# Bandwidth-aware batch sizing
optimal_batch_size = min(
    max_batch_by_vram,
    memory_bandwidth_gb_s / (model_size_gb / decode_latency_s)
)
```

#### Sources
- [arXiv: Efficient LLM Inference - Bandwidth, Compute, Synchronization](https://arxiv.org/html/2507.14397v1)
- [arXiv: Systematic Characterization of LLM Inference on GPUs](https://arxiv.org/html/2512.01644v1)

---

### 2.3 NUMA Awareness and Thread Affinity

For CPU inference or hybrid workloads, NUMA awareness is critical for performance.

#### Key Optimizations

| Technique | Impact | Implementation |
|-----------|--------|----------------|
| **Socket Pinning** | +30-60% | Dedicate socket to LLM |
| **Memory Affinity** | +20-40% | Keep data on local NUMA node |
| **Thread Binding** | +15-25% | Pin threads to cores |
| **MoE Partitioning** | Near-linear scaling | Distribute experts across sockets |

#### Implementation

```bash
# NUMA-aware llama.cpp execution
numactl --cpunodebind=0 --membind=0 \
    ./llama-server \
    --model ./model.gguf \
    --threads 16 \
    --threads-batch 16
```

#### IPEX-LLM Approach (Intel)

Intel's IPEX-LLM uses Numactl for strategic allocation:
- One socket for LLM intensive tasks
- Other sockets for general-purpose tasks
- Result: **92 tokens/sec** with NUMA-aware pinning (+60% vs standard)

#### Advanced: ghOSt Scheduling

Google's ghOSt framework allows custom kernel-level scheduling:
- Reserve cores for inference only
- Accelerate wake-ups for latency-critical requests
- Fine-tune time slices and load balancing

#### Sources
- [OS-Level Challenges in LLM Inference](https://eunomia.dev/blog/2025/02/18/os-level-challenges-in-llm-inference-and-optimizations/)
- [Intel: Optimizing LLM Inference Using IPEX-LLM](https://cdrdv2-public.intel.com/834133/Intel%20AI_LLM%20Model%20Inference%20Using%20IPEX-LLM_Whitepaper_rev1.0.pdf)
- [The Crucial Role of NUMA Awareness](https://towardsdatascience.com/the-crucial-role-of-numa-awareness-in-high-performance-deep-learning/)

---

## 3. Quantization Quality vs Speed Tradeoffs

### 3.1 GGUF Quantization Formats

GGUF (GPT-Generated Unified Format) is the standard for llama.cpp and Ollama.

#### Format Comparison

| Format | Bits | Size Reduction | Quality Retained | Speed | Best For |
|--------|------|----------------|------------------|-------|----------|
| Q2_K | 2 | ~88% | ~85% | Fastest | Extreme constraints |
| Q3_K_S | 3 | ~82% | ~88% | Very Fast | Testing |
| Q4_K_S | 4 | ~75% | ~90% | Fast | Low VRAM |
| **Q4_K_M** | 4 | ~75% | **~92%** | Fast | **Sweet spot** |
| Q5_K_S | 5 | ~68% | ~94% | Medium | Balanced |
| **Q5_K_M** | 5 | ~68% | **~95%** | Medium | **Critical apps** |
| Q6_K | 6 | ~62% | ~97% | Slower | High quality |
| **Q8_0** | 8 | ~50% | **~99%** | Slowest | Near-lossless |

#### VRAM Requirements (Approximate)

| Model | FP16 | Q8_0 | Q5_K_M | Q4_K_M |
|-------|------|------|--------|--------|
| 7B | 14GB | 8GB | 6GB | **5GB** |
| 13B | 26GB | 14GB | 10GB | **8GB** |
| 34B | 68GB | 36GB | 26GB | **20GB** |
| 70B | 140GB | 75GB | 50GB | **40GB** |

#### Sources
- [Local AI Zone: Quantization Guide 2025](https://local-ai-zone.github.io/guides/what-is-ai-quantization-q4-k-m-q8-gguf-guide-2025.html)
- [LocalLLM.in: Complete Guide to LLM Quantization](https://localllm.in/blog/quantization-explained)

---

### 3.2 GPTQ vs AWQ vs EXL2

#### Benchmark Comparison

| Method | Perplexity | Output Speed | VRAM | Quality |
|--------|------------|--------------|------|---------|
| FP16 Baseline | 6.55 | 100% | 100% | 100% |
| **GGUF Q4_K_M** | **6.74** | ~85% | 25% | **92%** |
| GPTQ-4bit | 6.90 | ~90% | 25% | 89% |
| **AWQ-4bit** | **6.84** | ~95% | 25% | **91%** |
| Marlin-AWQ | 6.84 | **147%** | 25% | 91% |
| Marlin-GPTQ | 6.97 | **142%** | 25% | 88% |
| **EXL2** | Variable | **185%** | Tunable | Variable |

#### Key Findings

1. **AWQ vs GPTQ**: AWQ consistently outperforms GPTQ due to activation-aware weight preservation
2. **Marlin Kernels**: Provide massive speedups (2.6x for GPTQ, **10.9x for AWQ**)
3. **EXL2**: Most flexible, fastest for generation, best for prompt processing

```
Performance Winner: Marlin-AWQ (741 tok/s) > Marlin-GPTQ (712 tok/s) > EXL2 > AWQ > GPTQ
Quality Winner: GGUF Q4_K_M (6.74) > AWQ (6.84) > GPTQ (6.90)
```

#### Use Case Recommendations

| Scenario | Recommended Format |
|----------|-------------------|
| Local inference (Ollama/llama.cpp) | **Q4_K_M** or Q5_K_M |
| GPU serving (vLLM) | **Marlin-AWQ** |
| Maximum throughput | **EXL2** |
| Quality-critical | **Q8_0** or FP16 |
| Memory constrained | Q4_K_S or Q3_K_M |

#### Sources
- [oobabooga: GPTQ vs AWQ vs EXL2 Comparison](https://oobabooga.github.io/blog/posts/gptq-awq-exl2-llamacpp/)
- [JarvisLabs: vLLM Quantization Complete Guide](https://docs.jarvislabs.ai/blog/vllm-quantization-complete-guide-benchmarks)
- [LocalAIMaster: GGUF vs GPTQ vs AWQ 2026](https://localaimaster.com/blog/quantization-explained)

---

### 3.3 Perplexity Degradation Analysis

#### Perplexity by Quantization Level

| Method | Perplexity | Degradation | Acceptable? |
|--------|------------|-------------|-------------|
| FP16 | 6.55 | Baseline | Yes |
| Q8_0 | 6.58 | +0.5% | Yes |
| Q6_K | 6.62 | +1.1% | Yes |
| Q5_K_M | 6.67 | +1.8% | Yes |
| **Q4_K_M** | **6.74** | **+2.9%** | **Yes** |
| Q4_K_S | 6.82 | +4.1% | Usually |
| Q3_K_M | 7.21 | +10.1% | Task-dependent |
| Q2_K | 8.15 | +24.4% | Limited |

#### Task-Specific Impact

Different tasks have varying sensitivity to quantization:

| Task | Q4_K_M vs FP16 | Notes |
|------|----------------|-------|
| Code generation | -2-3% | Very tolerant |
| Summarization | -1-2% | Highly tolerant |
| Math reasoning | -5-8% | More sensitive |
| Factual QA | -3-5% | Moderate sensitivity |
| Creative writing | -1-2% | Very tolerant |

**Recommendation**: Use **Q5_K_M** for math/reasoning tasks, **Q4_K_M** for everything else.

---

## 4. Hardware Recommendations for Local LLM

### 4.1 GPU Comparison (2025-2026)

#### Consumer GPUs

| GPU | VRAM | Bandwidth | tok/s (Llama-3-8B Q4) | Price | Best For |
|-----|------|-----------|----------------------|-------|----------|
| **RTX 4090** | 24GB | 1008 GB/s | **41.2** | $1600 | Maximum performance |
| RTX 4080 Super | 16GB | 736 GB/s | 32.5 | $1000 | Good balance |
| RTX 4070 Ti Super | 16GB | 672 GB/s | 28.3 | $800 | Budget high-end |
| RX 7900 XTX | 24GB | 960 GB/s | 33.7 | $900 | Best value (VRAM) |
| **RX 7800 XT** | **16GB** | **624 GB/s** | **~25** | **$450** | **Budget winner** |
| RTX 3090 | 24GB | 936 GB/s | 29.8 | $800 (used) | Good used option |

#### RTX 4090 vs RX 7900 XTX

| Metric | RTX 4090 | RX 7900 XTX | Winner |
|--------|----------|-------------|--------|
| Token/sec | 41.2 | 33.7 | RTX 4090 (+22%) |
| Watts/token | 6.5 | 7.7 | RTX 4090 |
| Software maturity | 92% plug-and-play | 68% needs intervention | RTX 4090 |
| Junction temp | 78C | 89C (throttles) | RTX 4090 |
| Price/performance | Good | **Better** | RX 7900 XTX |

**For ORK-Station (RX 7800 XT)**: Expect **15-25 tok/s** for 7B-13B Q4 models, which is excellent for local development.

#### Sources
- [Corelab: Best GPUs for Local LLM Inference 2026](https://corelab.tech/llmgpu/)
- [IntuitionLabs: Local LLM Deployment on 24GB GPUs](https://intuitionlabs.ai/articles/local-llm-deployment-24gb-gpu-optimization)
- [Alibaba: RTX 4090 vs RX 7900 XTX Benchmarks](https://www.alibaba.com/product-insights/nvidia-rtx-4090-vs-amd-rx-7900-xtx-for-local-llm-inference-benchmarks-that-matter.html)

---

### 4.2 RAM Requirements by Model Size

#### System RAM Recommendations

| Model Size | Minimum RAM | Recommended | Ideal |
|------------|-------------|-------------|-------|
| 7B | 8GB | **16GB** | 32GB |
| 13B | 16GB | **32GB** | 64GB |
| 34B | 32GB | 64GB | 128GB |
| 70B | 64GB | **128GB** | 256GB |

#### VRAM Requirements (Quantized)

| Model | Q4_K_M | Q5_K_M | Q8_0 | FP16 |
|-------|--------|--------|------|------|
| 7B | **5-6GB** | 6-7GB | 8-10GB | 14GB |
| 8B | **6-7GB** | 7-8GB | 10-12GB | 16GB |
| 13B | **8-10GB** | 10-12GB | 15-18GB | 26GB |
| 34B | **20-22GB** | 24-26GB | 36-40GB | 68GB |
| 70B | **40-45GB** | 50-55GB | 75-80GB | 140GB |

#### Context Length Impact

| Context | Additional VRAM per 1K tokens |
|---------|-------------------------------|
| 4K | ~100MB |
| 8K | ~200MB |
| 16K | ~400MB |
| 32K | ~800MB |
| 128K | ~3.2GB |

**For ORK-Station (16GB VRAM)**:
- Full models: Up to **13B Q4_K_M** with 8K context
- Partial offload: **34B Q4** with ~30 layers on GPU
- Not recommended: 70B without significant CPU offload

#### Sources
- [MLJourney: VRAM for LLMs 7B-70B](https://mljourney.com/how-much-vram-do-you-really-need-for-llms-7b-70b-explained/)
- [LocalAIMaster: AI RAM Requirements 2026](https://localaimaster.com/blog/ram-requirements-local-ai)
- [Ollama: VRAM Requirements Guide 2026](https://localllm.in/blog/ollama-vram-requirements-for-local-llms)

---

### 4.3 Power Efficiency Considerations

#### Tokens per Watt Comparison

| Hardware | Model | tok/s | Watts | tok/W |
|----------|-------|-------|-------|-------|
| RTX 4090 | Llama-3.1-8B | 41.2 | 295W | 0.14 |
| RX 7900 XTX | Llama-3.1-8B | 33.7 | 258W | 0.13 |
| H100 SXM | Llama-3.1-8B | 180 | 700W | 0.26 |
| Positron Atlas | Llama-3.1-8B | 280 | 2000W | 0.14 |

#### Energy per Query

| Model Size | Energy per Query | Cost (@ $0.15/kWh) |
|------------|------------------|-------------------|
| 8B | 0.3-0.5 Wh | $0.00005 |
| 70B | 1.5-2.5 Wh | $0.00030 |
| 175B+ | 5-10 Wh | $0.00120 |

**Key Insight**: GPU inference is typically **2-3x more energy-efficient** than CPU-only inference at comparable batch sizes.

#### Emerging Hardware (2026)

| Hardware | Memory | Target | Release |
|----------|--------|--------|---------|
| NVIDIA Rubin CPX | 128GB GDDR7 | Long-context | 2026 |
| Intel Crescent Island | 160GB | Inference-only | Late 2026 |
| Positron Asimov | 2TB | 16T parameter models | 2026 |

#### Sources
- [arXiv: TokenPowerBench](https://arxiv.org/html/2512.03024v1)
- [Nature: GPU Energy in LLM Text Generation](https://www.nature.com/articles/s41598-025-31896-0)
- [Muxup: Per-query Energy Consumption](https://muxup.com/2026q1/per-query-energy-consumption-of-llms)

---

## 5. Inference Engine Comparison

### Performance Benchmarks (2026)

| Engine | H100 tok/s | Llama-4-70B FP8 | Best For |
|--------|------------|-----------------|----------|
| **SGLang** | **16,215** | - | Multi-turn conversations |
| **LMDeploy** | **16,132** | - | Quantized serving |
| vLLM | 12,553 | 3,400 | General production |
| TensorRT-LLM | - | **4,800** | Maximum throughput |
| TGI | - | 2,900 | HuggingFace ecosystem |

### Market Share (2026)

Production open-model inference is dominated by:
1. **vLLM** (most mature ecosystem)
2. **TensorRT-LLM** (NVIDIA optimized)
3. **TGI** (HuggingFace integration)
4. SGLang (research workloads)

### Latency Characteristics

| Engine | Time to First Token | Per-Token Latency |
|--------|---------------------|-------------------|
| vLLM | **Fastest** | 4-21ms |
| SGLang | Medium | **Most stable** |
| TensorRT-LLM | Slowest | Competitive |

### Recommendation Matrix

| Use Case | Recommended Engine |
|----------|-------------------|
| General production | **vLLM** |
| NVIDIA-only, max throughput | **TensorRT-LLM** |
| Multi-turn chat | **SGLang** |
| Quantized models | **LMDeploy** |
| HuggingFace ecosystem | **TGI** |
| Local development | **Ollama** (llama.cpp) |

#### Sources
- [PremAI: vLLM vs SGLang vs LMDeploy 2026](https://blog.premai.io/vllm-vs-sglang-vs-lmdeploy-fastest-llm-inference-engine-in-2026/)
- [Clarifai: SGLang vs vLLM vs TensorRT-LLM](https://www.clarifai.com/blog/comparing-sglang-vllm-and-tensorrt-llm-with-gpt-oss-120b)
- [Kanerika: SGLang vs vLLM 2026](https://kanerika.com/blogs/sglang-vs-vllm/)

---

## 6. ORK-Station Implementation Recommendations

Based on the research findings, here are specific recommendations for ORK-Station with **RX 7800 XT (16GB VRAM)**.

### Optimal Model Configuration

| Purpose | Model | Quantization | VRAM | Expected tok/s |
|---------|-------|--------------|------|----------------|
| General Assistant | Hermes-3-8B | **Q4_K_M** | ~5GB | 25-30 |
| Code Generation | Qwen2.5-Coder-7B | **Q5_K_M** | ~6GB | 22-28 |
| Long Context | Qwen2.5-7B-128K | Q4_K_M | ~7GB | 18-24 |
| Reasoning | Dolphin-3.0-8B | Q4_K_M | ~5GB | 25-30 |

### llama.cpp Optimization

```bash
# Optimal settings for RX 7800 XT
export HSA_OVERRIDE_GFX_VERSION=11.0.0
export HIP_VISIBLE_DEVICES=0

./llama-server \
    --model ./models/qwen2.5-coder-7b-instruct-q5_k_m.gguf \
    --n-gpu-layers 99 \           # Full GPU offload
    --ctx-size 8192 \             # 8K context
    --batch-size 512 \            # Optimal for 16GB
    --threads 8 \                 # CPU threads for prefill
    --flash-attn \                # Enable FlashAttention
    --mlock                       # Lock memory
```

### Ollama Configuration

```yaml
# ~/.ollama/modelfile
FROM qwen2.5-coder:7b-instruct-q5_K_M

PARAMETER num_ctx 8192
PARAMETER num_batch 512
PARAMETER num_gpu 99
PARAMETER num_thread 8
```

### RLM Integration

For files >25K tokens, use the RLM system:

```python
# Load large file into RLM
result = mcp__ork-unlimited-context__rlm_load_file(
    file_path="/path/to/large/codebase/"
)

# Query with parallel worker processing
answer = mcp__ork-unlimited-context__rlm_query(
    query="Analyze the main architecture",
    variable_id=result["variable_id"],
    parallel=True
)
```

### Monitoring

```bash
# GPU utilization
watch -n 1 rocm-smi

# Temperature and power
rocm-smi --showtemp --showpower

# Memory usage
rocm-smi --showmeminfo vram
```

### Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Token throughput | 20-30 tok/s | 7B-8B Q4/Q5 models |
| Memory utilization | 80-90% | Leave headroom for KV cache |
| Temperature | <85C junction | Thermal throttling at 89C |
| Context length | 8K-16K | Balance quality and speed |

---

## References

### Academic Papers
- [LMCache: Enterprise-Scale KV Cache (arXiv 2510.09665)](https://arxiv.org/pdf/2510.09665)
- [FlashAttention-3 (arXiv 2205.14135)](https://arxiv.org/abs/2205.14135)
- [Mind the Memory Gap (arXiv 2503.08311)](https://arxiv.org/html/2503.08311v2)
- [TokenPowerBench (arXiv 2512.03024)](https://arxiv.org/html/2512.03024v1)
- [Speculative Speculative Decoding (ICLR 2026)](https://openreview.net/pdf?id=aL1Wnml9Ef)

### Technical Blogs
- [NVIDIA: Mastering LLM Inference Optimization](https://developer.nvidia.com/blog/mastering-llm-techniques-inference-optimization/)
- [Red Hat: How vLLM Accelerates AI Inference](https://www.redhat.com/en/topics/ai/how-vllm-accelerates-ai-inference-3-enterprise-use-cases)
- [AMD ROCm: Tensor Parallelism Analysis](https://rocm.blogs.amd.com/artificial-intelligence/tensor-parallelism/README.html)
- [Meta Engineering: Scaling LLM Inference](https://engineering.fb.com/2025/10/17/ai-research/scaling-llm-inference-innovations-tensor-parallelism-context-parallelism-expert-parallelism/)

### Guides and Benchmarks
- [Corelab: Best GPUs for LLM 2026](https://corelab.tech/llmgpu/)
- [LocalAIMaster: Quantization Explained](https://localaimaster.com/blog/quantization-explained)
- [PremAI: Inference Engine Comparison 2026](https://blog.premai.io/vllm-vs-sglang-vs-lmdeploy-fastest-llm-inference-engine-in-2026/)
- [oobabooga: Quantization Benchmark](https://oobabooga.github.io/blog/posts/gptq-awq-exl2-llamacpp/)

---

*Document generated: March 2026*
*Last updated: 2026-03-07*
*ORK-Station Enterprise AI Infrastructure*
