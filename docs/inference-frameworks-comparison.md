# LLM Inference Frameworks Comparison for NEXUS Integration

**Date**: 2026-03-07
**Purpose**: Comprehensive analysis of top open-source LLM inference frameworks for NEXUS integration
**Target Hardware**: AMD RX 7800 XT (16GB VRAM) with ROCm 7.1.1

---

## Executive Summary

This document analyzes five leading open-source LLM inference frameworks for integration with NEXUS, the ORK-Station AI orchestration layer. Each framework has distinct strengths:

| Framework | Best For | NEXUS Fit |
|-----------|----------|-----------|
| **llama.cpp** | Single-user, low latency, broad hardware | Primary backend (via Ollama) |
| **vLLM** | High-throughput production serving | Multi-user scenarios |
| **Ollama** | Developer experience, model management | Current integration |
| **TGI** | Enterprise production deployments | Cloud/scaling scenarios |
| **ExLlamaV2** | Maximum single-GPU performance | VRAM-constrained scenarios |

---

## 1. llama.cpp

**Repository**: [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp)
**Language**: C/C++
**License**: MIT

### Overview

llama.cpp enables LLM inference with minimal setup and state-of-the-art performance across diverse hardware. It serves as the foundation for Ollama and many other inference solutions.

### Latest Optimization Features (2026)

| Feature | Description | Benefit |
|---------|-------------|---------|
| **Split Mode Graph** | Multi-GPU execution with direct memory read | 3-4x speed improvement for multi-GPU |
| **CUDA Graph Optimization** | Optimized kernel execution | Up to 35% faster token generation |
| **Speculative Decoding** | Draft model acceleration | Significant speed gains without quality loss |
| **Flash Attention** | Memory-efficient attention | Longer context with less VRAM |
| **rocWMMA Integration** | AMD matrix operations | Enhanced performance on RDNA3+/CDNA |

### GPU Backend Support

| Backend | Status | Hardware | Notes |
|---------|--------|----------|-------|
| **CUDA** | Production | NVIDIA GPUs | Best optimized |
| **HIP/ROCm** | Production | AMD GPUs | Full support, needs `HSA_OVERRIDE_GFX_VERSION=11.0.0` for RX 7800 XT |
| **Metal** | Production | Apple Silicon | Native acceleration |
| **Vulkan** | Stable | Cross-platform | Good fallback option |
| **SYCL** | Stable | Intel GPUs | OneAPI integration |
| **OpenCL** | Beta | Adreno/others | Mobile GPU support |
| **WebGPU** | Development | Browsers | Emerging support |

### Server Mode Capabilities

**llama-server** provides:
- OpenAI-compatible REST API
- Chat completions endpoint (`/v1/chat/completions`)
- Embeddings endpoint (`/v1/embeddings`)
- Multimodal input support (images, audio)
- Constrained generation via custom grammars (GBNF)
- Streaming responses via SSE
- Batched inference
- LoRA adapter hot-swapping

### Quantization Formats

| Format | Bits | VRAM Savings | Quality Loss |
|--------|------|--------------|--------------|
| Q8_0 | 8-bit | ~50% | Minimal |
| Q6_K | 6-bit | ~63% | Very low |
| Q5_K_M | 5-bit | ~69% | Low |
| Q4_K_M | 4-bit | ~75% | Moderate |
| Q4_0 | 4-bit | ~75% | Moderate |
| Q3_K_M | 3-bit | ~81% | Noticeable |
| Q2_K | 2-bit | ~88% | Significant |
| IQ2_XXS | 1.5-bit | ~91% | High |

### NEXUS Integration Assessment

| Criterion | Score | Notes |
|-----------|-------|-------|
| AMD ROCm Support | 9/10 | Excellent with proper env vars |
| API Compatibility | 9/10 | OpenAI-compatible |
| Model Support | 10/10 | Broadest GGUF ecosystem |
| Performance | 9/10 | Near-optimal for single user |
| Memory Efficiency | 9/10 | Excellent quantization options |
| Ease of Integration | 8/10 | Use via Ollama recommended |

---

## 2. vLLM

**Repository**: [vllm-project/vllm](https://github.com/vllm-project/vllm)
**Language**: Python/C++/CUDA
**License**: Apache 2.0

### Overview

vLLM is a high-throughput LLM serving engine featuring PagedAttention for efficient memory management and continuous batching for maximum GPU utilization under concurrent load.

### Continuous Batching Implementation

vLLM's continuous batching aggregates concurrent requests into unified GPU operations:

| Concurrency | vLLM (tok/s) | Traditional (tok/s) | Improvement |
|-------------|--------------|---------------------|-------------|
| 1 user | ~180 | ~170 | 1.06x |
| 10 users | ~485 | ~148 | 3.3x |
| 50 users | ~920 | ~155 | 5.9x |

**Key Innovation**: PagedAttention manages KV cache like virtual memory, eliminating fragmentation and enabling efficient memory sharing across requests.

### Multi-GPU Support

| Parallelism Type | Description | Use Case |
|------------------|-------------|----------|
| **Tensor Parallel** | Split layers across GPUs | Large model serving |
| **Pipeline Parallel** | Split model stages | Extreme scale |
| **Data Parallel** | Replicate for throughput | High-concurrency |
| **Expert Parallel** | MoE expert distribution | Mixtral, DeepSeek |

Configuration example:
```bash
vllm serve model --tensor-parallel-size 2 --pipeline-parallel-size 1
```

### HuggingFace Compatibility

| Feature | Status | Notes |
|---------|--------|-------|
| Transformer models | Full | Llama, Mistral, Falcon, etc. |
| MoE models | Full | Mixtral, DeepSeek-V2/V3 |
| Multi-modal | Full | LLaVA, Qwen-VL |
| Embedding models | Full | E5-Mistral, GTE |
| Custom architectures | Partial | Via plugin system |

### Quantization Support

| Method | Bits | Integration |
|--------|------|-------------|
| GPTQ | 4-bit | Native |
| AWQ | 4-bit | Native |
| AutoRound | Mixed | Native |
| FP8 | 8-bit | Native |
| INT8 | 8-bit | Native |
| INT4 | 4-bit | Native |
| BitsAndBytes | 4/8-bit | Via integration |

### AMD ROCm Status

| Aspect | Status | Notes |
|--------|--------|-------|
| HIP Backend | Stable | ROCm 6.0+ required |
| MI250/MI300 | Production | Full optimization |
| RDNA3 (RX 7000) | Experimental | Limited official support |
| Flash Attention | Partial | Composable kernel needed |

### NEXUS Integration Assessment

| Criterion | Score | Notes |
|-----------|-------|-------|
| AMD ROCm Support | 6/10 | Focus on MI series, limited RDNA3 |
| API Compatibility | 10/10 | Full OpenAI compatibility |
| Model Support | 9/10 | Excellent HuggingFace integration |
| Performance | 10/10 | Best for concurrent users |
| Memory Efficiency | 9/10 | PagedAttention is excellent |
| Ease of Integration | 7/10 | Python dependency, complex setup |

---

## 3. Ollama

**Repository**: [ollama/ollama](https://github.com/ollama/ollama)
**Language**: Go (wrapping llama.cpp)
**License**: MIT

### Overview

Ollama provides the simplest path to running LLMs locally with a Docker-like model management experience. It wraps llama.cpp with a user-friendly CLI and REST API.

### API Design and Extensibility

**Core Endpoints**:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/chat` | POST | Conversational completions |
| `/api/generate` | POST | Text generation |
| `/api/embeddings` | POST | Vector embeddings |
| `/api/pull` | POST | Download models |
| `/api/push` | POST | Upload custom models |
| `/api/create` | POST | Create from Modelfile |
| `/api/list` | GET | List local models |
| `/api/show` | POST | Model details |
| `/api/delete` | DELETE | Remove model |

**OpenAI Compatibility**:
```bash
# Drop-in replacement
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "llama3", "messages": [{"role": "user", "content": "Hello"}]}'
```

### Model Management System

**Modelfile DSL**:
```dockerfile
FROM llama3.2
PARAMETER temperature 0.7
PARAMETER num_ctx 4096
SYSTEM "You are a helpful assistant."
TEMPLATE "{{ .System }}\n\n{{ .Prompt }}"
```

**Key Features**:
- Pull-and-run experience (like Docker)
- Automatic quantization selection
- Layer caching for fast model switching
- Custom model creation and sharing
- Ollama Hub registry integration

### Performance Characteristics

| Scenario | Performance | Notes |
|----------|-------------|-------|
| Single user | ~170 tok/s (7B) | Excellent latency |
| 3 concurrent | ~140 tok/s each | Slight degradation |
| 10 concurrent | ~45 tok/s each | Significant queueing |
| 50 concurrent | ~15 tok/s each | Consider vLLM |

**Memory Management**:
- Static VRAM allocation per model
- Hot model caching (configurable timeout)
- Multi-model support with memory pressure handling
- GPU layer offloading (partial offload supported)

### GPU Support

| Backend | Status | Configuration |
|---------|--------|---------------|
| NVIDIA CUDA | Production | Automatic detection |
| AMD ROCm | Production | `HSA_OVERRIDE_GFX_VERSION=11.0.0` |
| Apple Metal | Production | Automatic |
| CPU | Production | AVX2 optimized |

### NEXUS Integration Assessment

| Criterion | Score | Notes |
|-----------|-------|-------|
| AMD ROCm Support | 9/10 | Inherited from llama.cpp |
| API Compatibility | 9/10 | OpenAI + native API |
| Model Support | 9/10 | GGUF ecosystem |
| Performance | 7/10 | Optimized for simplicity over throughput |
| Memory Efficiency | 8/10 | Static allocation overhead |
| Ease of Integration | 10/10 | Single binary, minimal config |

---

## 4. Text Generation Inference (TGI)

**Repository**: [huggingface/text-generation-inference](https://github.com/huggingface/text-generation-inference)
**Language**: Rust/Python
**License**: Apache 2.0 (HFOIL for some components)

### Overview

TGI is HuggingFace's production-grade inference server, powering Hugging Chat and Inference Endpoints. Built in Rust for performance with Python model loading.

### Production Deployment Features

| Feature | Description |
|---------|-------------|
| **Tensor Parallelism** | Multi-GPU distribution |
| **Continuous Batching** | Dynamic request aggregation |
| **Token Streaming** | SSE-based real-time output |
| **Speculation** | ~2x latency reduction |
| **Safetensors** | Secure weight loading |
| **Watermarking** | Output provenance tracking |

### Quantization Support

| Method | Bits | Loading | Notes |
|--------|------|---------|-------|
| BitsAndBytes | 4/8 | On-the-fly | NF4, FP4 formats |
| GPTQ | 4 | Pre-quantized | Marlin kernel support |
| AWQ | 4 | Pre-quantized | Fast inference |
| EETQ | 8 | On-the-fly | Efficient INT8 |
| FP8 | 8 | Native | Hopper GPUs optimal |
| Marlin | 4 | Pre-quantized | Maximum throughput |

### Monitoring Capabilities

| Component | Integration |
|-----------|-------------|
| **Metrics** | Prometheus endpoint (`/metrics`) |
| **Tracing** | OpenTelemetry (Jaeger compatible) |
| **Logging** | Structured JSON logs |
| **Health** | `/health` and `/info` endpoints |

**Key Metrics Exposed**:
- `tgi_queue_size` - Pending requests
- `tgi_batch_current_size` - Active batch size
- `tgi_request_duration_seconds` - Latency histogram
- `tgi_request_generated_tokens` - Token counts
- `tgi_gpu_memory_used` - VRAM utilization

### Hardware Support

| Platform | Status | Notes |
|----------|--------|-------|
| NVIDIA CUDA | Production | Optimal path |
| AMD ROCm | Production | MI250/MI300 focus |
| Intel Gaudi | Production | HPU support |
| AWS Inferentia | Production | Neuron SDK |
| Google TPU | Beta | JAX backend |

### NEXUS Integration Assessment

| Criterion | Score | Notes |
|-----------|-------|-------|
| AMD ROCm Support | 7/10 | MI series focus, RDNA3 limited |
| API Compatibility | 9/10 | OpenAI Messages API |
| Model Support | 9/10 | HuggingFace native |
| Performance | 9/10 | Excellent production throughput |
| Memory Efficiency | 8/10 | Good quantization options |
| Ease of Integration | 6/10 | Docker-centric, complex config |

---

## 5. ExLlamaV2

**Repository**: [turboderp-org/exllamav2](https://github.com/turboderp-org/exllamav2)
**Language**: Python/CUDA
**License**: MIT

### Overview

ExLlamaV2 is an inference library optimized for maximum performance on consumer GPUs, featuring the EXL2 quantization format with variable bit-width per layer.

### EXL2 Quantization Benefits

**Variable Bit-Width Architecture**:
- 2-8 bits per weight (configurable per layer)
- Automatic calibration for optimal bit allocation
- Sparse quantization for important weights
- Mixed precision within single model

| bpw | Quality | VRAM (7B) | Speed |
|-----|---------|-----------|-------|
| 8.0 | Baseline | ~8.5 GB | 100% |
| 5.0 | Excellent | ~6.3 GB | 110% |
| 4.0 | Good | ~5.2 GB | 115% |
| 3.0 | Moderate | ~4.0 GB | 120% |
| 2.5 | Usable | ~3.5 GB | 125% |

**Comparison with Other Formats**:

| Format | Quality/Size | Speed | Ecosystem |
|--------|--------------|-------|-----------|
| EXL2 | Excellent | Fastest | Growing |
| GPTQ | Good | Fast | Large |
| AWQ | Good | Fast | Large |
| GGUF | Good | Moderate | Largest |

### VRAM Optimization

**Extreme Compression Results**:
- Llama2 70B at 2.55 bpw: Runs on single 24GB GPU
- 13B models at 2.65 bpw: Fits in 8GB VRAM
- Context limited to 2048 tokens for extreme compression

**Memory Features**:
- KV cache deduplication
- Smart prompt caching
- Dynamic batching
- Speculative decoding support

### Speed Benchmarks

**RTX 4090 Performance** (single user):

| Model | Quantization | Tokens/sec |
|-------|--------------|------------|
| Llama 7B | GPTQ 4-bit | 205 |
| Llama 13B | EXL2 4.0bpw | 120 |
| Llama2 70B | EXL2 2.5bpw | 38 |
| TinyLlama 1.1B | EXL2 3.0bpw | 770 |

**vs GGUF** (when model fits in VRAM):
- EXL2 runs **2-3x faster** at equivalent quality
- GGUF wins for CPU offloading scenarios

### Limitations

| Limitation | Impact |
|------------|--------|
| CUDA-only | No AMD/Metal support |
| Python required | Not standalone binary |
| Limited batching | Single-user optimized |
| Ecosystem | Smaller than GGUF |

### NEXUS Integration Assessment

| Criterion | Score | Notes |
|-----------|-------|-------|
| AMD ROCm Support | 0/10 | CUDA only - not suitable |
| API Compatibility | 6/10 | Via TabbyAPI wrapper |
| Model Support | 7/10 | Growing EXL2 ecosystem |
| Performance | 10/10 | Fastest single-user inference |
| Memory Efficiency | 10/10 | Best VRAM optimization |
| Ease of Integration | 5/10 | Python, custom format |

---

## Comparison Matrix

### Feature Comparison

| Feature | llama.cpp | vLLM | Ollama | TGI | ExLlamaV2 |
|---------|-----------|------|--------|-----|-----------|
| **AMD ROCm** | Yes | Partial | Yes | Partial | No |
| **OpenAI API** | Yes | Yes | Yes | Yes | Via wrapper |
| **Continuous Batching** | Basic | Advanced | Basic | Advanced | Basic |
| **Multi-GPU** | Yes | Yes | Limited | Yes | Limited |
| **Speculative Decoding** | Yes | Yes | No | Yes | Yes |
| **Flash Attention** | Yes | Yes | Yes | Yes | Yes |
| **Streaming** | Yes | Yes | Yes | Yes | Yes |
| **LoRA** | Yes | Yes | Yes | Yes | Yes |
| **Embeddings** | Yes | Yes | Yes | Yes | Limited |
| **Multimodal** | Yes | Yes | Yes | Yes | No |

### Performance Comparison

| Scenario | Best Choice | Alternative |
|----------|-------------|-------------|
| Single user, low latency | llama.cpp/Ollama | ExLlamaV2 (NVIDIA) |
| 10+ concurrent users | vLLM | TGI |
| Production deployment | TGI | vLLM |
| Developer experience | Ollama | llama.cpp |
| VRAM constrained | ExLlamaV2 | llama.cpp Q2/Q3 |
| AMD consumer GPU | Ollama | llama.cpp direct |
| Enterprise monitoring | TGI | vLLM |

### Quantization Comparison

| Format | Size | Quality | Speed | Ecosystem |
|--------|------|---------|-------|-----------|
| GGUF Q4_K_M | 4.9 GB/8B | Good | Moderate | Largest |
| EXL2 5.0bpw | 6.3 GB/8B | Excellent | Fast | Growing |
| GPTQ 4-bit | 4.5 GB/8B | Good | Fast | Large |
| AWQ 4-bit | 4.5 GB/8B | Good | Fast | Large |
| FP8 | 8.5 GB/8B | Excellent | Fast | Limited |

---

## NEXUS Integration Recommendation

### Primary Stack (Current)

```
NEXUS
  |
  +-- Ollama (localhost:11434)
  |     |
  |     +-- llama.cpp backend
  |     +-- GGUF models (dolphin3, hermes3, qwen2.5-coder)
  |     +-- ROCm/HIP acceleration
  |
  +-- ork-offline-coding MCP (port 8004)
        |
        +-- Model orchestration
        +-- Tool calling integration
```

**Rationale**:
- Ollama provides best developer experience
- Full AMD ROCm support with RX 7800 XT
- GGUF ecosystem has widest model availability
- OpenAI-compatible API for easy integration
- Single binary deployment

### Future Considerations

1. **High Concurrency Scenarios**: Consider vLLM when serving 10+ concurrent users
2. **Enterprise Monitoring**: TGI for production deployments requiring Prometheus/OpenTelemetry
3. **Maximum Single-User Speed**: ExLlamaV2 if switching to NVIDIA GPU

### Configuration Recommendations

**Ollama Environment** (for RX 7800 XT):
```bash
export HSA_OVERRIDE_GFX_VERSION=11.0.0
export HIP_VISIBLE_DEVICES=0
export OLLAMA_NUM_GPU=999  # Use all GPU layers
export OLLAMA_FLASH_ATTENTION=1
```

**Model Selection** (16GB VRAM budget):
| Use Case | Model | VRAM |
|----------|-------|------|
| Coding | qwen2.5-coder:14b-instruct-q4_K_M | ~9 GB |
| Chat | dolphin3:8b-llama3.1-q4_K_M | ~5 GB |
| Orchestration | hermes3:8b-q4_K_M | ~5 GB |
| Multi-model | 2x 7B models concurrent | ~10 GB |

---

## Sources

### llama.cpp
- [ggml-org/llama.cpp GitHub](https://github.com/ggml-org/llama.cpp)
- [ROCm Documentation - llama.cpp](https://rocm.docs.amd.com/en/latest/compatibility/ml-compatibility/llama-cpp-compatibility.html)
- [llama.cpp Multi-GPU Performance Breakthrough](https://medium.com/@jagusztinl/llama-cpp-performance-breakthrough-for-multi-gpu-setups-04c83a66feb2)
- [AMD ROCm Blog - Llama.cpp Meets Instinct](https://rocm.blogs.amd.com/ecosystems-and-partners/llama-cpp/README.html)

### vLLM
- [vLLM Official Site](https://vllm.ai)
- [vLLM Documentation](https://docs.vllm.ai/en/latest/)
- [Anyscale - Continuous Batching](https://www.anyscale.com/blog/continuous-batching-llm-inference)
- [vLLM Architecture Deep Dive](https://blog.vllm.ai/2025/09/05/anatomy-of-vllm.html)
- [Ollama vs vLLM Benchmark 2026](https://www.sitepoint.com/ollama-vs-vllm-performance-benchmark-2026/)

### Ollama
- [ollama/ollama GitHub](https://github.com/ollama/ollama)
- [Ollama API Documentation](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Complete Ollama Tutorial 2026](https://dev.to/proflead/complete-ollama-tutorial-2026-llms-via-cli-cloud-python-3m97)
- [Ollama Model Management Guide](https://oneuptime.com/blog/post/2026-02-02-ollama-model-management/view)

### TGI
- [huggingface/text-generation-inference GitHub](https://github.com/huggingface/text-generation-inference)
- [TGI Documentation](https://huggingface.co/docs/text-generation-inference/en/index)
- [TGI Quantization Guide](https://huggingface.co/docs/text-generation-inference/en/conceptual/quantization)
- [TGI Kubernetes Deployment](https://oneuptime.com/blog/post/2026-02-09-huggingface-tgi-kubernetes/view)

### ExLlamaV2
- [turboderp-org/exllamav2 GitHub](https://github.com/turboderp-org/exllamav2)
- [ExLlamaV2: The Fastest Library to Run LLMs](https://medium.com/data-science/exllamav2-the-fastest-library-to-run-llms-32aeda294d26)
- [GPTQ, AWQ, EXL2 Comparison](https://oobabooga.github.io/blog/posts/gptq-awq-exl2-llamacpp/)
- [Quantization Explained: Consumer GPUs](https://www.sitepoint.com/quantization-explained-consumer-gpu/)

---

*Document generated for NEXUS integration planning - ORK-Station Enterprise++++++++*
