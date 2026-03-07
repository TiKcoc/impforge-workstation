# Phase 2 Research: AI IDE Patterns, Model Routing, System Monitoring, WebView, SSE Streaming

**Date**: 2026-03-07
**Scope**: Literature review of 2024-2026 papers and best practices for Nexus Phase 2 implementation.

---

## 1. AI-Powered IDE / Workstation Design

### 1.1 Cursor Architecture

Cursor implements a **multi-turn agent loop** where the LLM (Claude) runs repeatedly until it produces a user-facing response. The client code (not the LLM) computes tool results and feeds them back -- cleanly decoupling reasoning from execution.

**Key patterns**:

| Pattern | Implementation |
|---------|---------------|
| Tool-use loop | 6 tools: `codebase_search`, `grep_search`, `file_search`, `read_file`, `write_file`, `run_command` |
| Semantic diffs | LLM produces comment-marked diffs; a cheaper "apply" model resolves them |
| Codebase indexing | Vectorstore with encoder LLM at index time, re-ranking at query time |
| Static prompt caching | System prompt is never personalized, enabling full prompt cache hits |
| Context assembly | `@file`/`@folder` tags inject full content in `<attached-files>` blocks |
| Forced reasoning | Non-functional parameters in tool schemas force chain-of-thought before calls |

**Throughput**: Composer (Cursor's proprietary model) achieves ~250 tokens/sec with most tasks under 30 seconds.

> Source: [How Cursor (AI IDE) Works](https://blog.sshh.io/p/how-cursor-ai-ide-works)

### 1.2 Windsurf / Codeium (Cascade)

Windsurf's **Cascade** engine maps the entire codebase into a neural-net-like graph for deep cross-file reasoning. Key differentiators:

- Multi-file edit planning with deep repository context
- Terminal integration for command execution within the agent loop
- "Memories" system for persistent project rules / coding standards
- Acquired by Cognition AI (Devin) in Dec 2025, integrating autonomous agent capabilities

> Source: [Windsurf Review](https://skywork.ai/skypage/en/Windsurf-(Formerly-Codeium)-Review-2025:-The-Agentic-IDE-Changing-the-Game/1973911680657846272)

### 1.3 Zed Editor

Zed is the most architecturally relevant for Rust-native implementations:

- **Custom streaming diff protocol** works with CRDT-based buffers for token-by-token edit streaming
- **Async Rust runtime** handles LSP communication without blocking UI (advantage over Electron)
- **Agent Client Protocol (ACP)** -- open standard (Apache License, Aug 2025) that is "LSP for AI agents"
- **Headless mode** -- full editor engine (Tree-sitter, LSP, multi-buffer) runs without GUI for programmatic control

> Sources: [Zed AI Blog](https://zed.dev/blog/zed-ai), [ACP: The LSP for AI Coding Agents](https://blog.promptlayer.com/agent-client-protocol-the-lsp-for-ai-coding-agents/)

### 1.4 MCP Telemetry Paper

Yang et al. (2025) propose "Agent-Integrated Development Environment (AIDE)" with real-time telemetry through MCP servers. The IDE manages prompts, evaluation metrics, LLM traces, and agent control in a standardized manner.

> Source: [Mind the Metrics (arXiv:2506.11019)](https://arxiv.org/html/2506.11019v1)

### Takeaways for Nexus

1. **Agent loop**: Client manages tool dispatch, LLM only reasons. Keep the loop in Rust.
2. **Vectorstore indexing**: Embed codebase files at index time, re-rank at query time.
3. **Static system prompts**: Enable prompt caching for cost/latency reduction.
4. **Streaming diffs**: Use CRDT or operational-transform buffers for live edits.
5. **ACP/MCP**: Standardize agent-IDE communication through open protocols.

---

## 2. Intelligent Model Routing

### 2.1 RouteLLM (ICLR 2025)

**Paper**: Ong et al., "RouteLLM: Learning to Route LLMs with Preference Data"

Routes queries between a strong model (e.g., GPT-4) and weak model (e.g., Mixtral) using trained routers. Four router architectures tested:

| Router | Method | Latency |
|--------|--------|---------|
| Similarity-weighted ranking | Cosine similarity to training examples | Low |
| Matrix factorization | Learned embeddings for queries and models | Low |
| BERT classifier | Binary classification on query text | Medium |
| Causal LLM classifier | LLM-based complexity assessment | High |

**Result**: Over 2x cost reduction while maintaining 95%+ of strong model quality. Generalizes across model pairs without retraining.

> Source: [RouteLLM (arXiv:2406.18665)](https://arxiv.org/abs/2406.18665), [ICLR Proceedings](https://proceedings.iclr.cc/paper_files/paper/2025/hash/5503a7c69d48a2f86fc00b3dc09de686-Abstract-Conference.html)

### 2.2 xRouter (Salesforce, Oct 2025)

**Paper**: Qian et al., "xRouter: Training Cost-Aware LLMs Orchestration System via Reinforcement Learning"

- Built on Qwen2.5-7B-Instruct as router backbone
- Uses **tool-calling** to invoke downstream models (the router itself can answer simple queries)
- Trained with DAPO (Distributional Advantage Policy Optimization) with cost-aware reward
- Near GPT-5 accuracy on hard benchmarks, 60-80% cost reduction

> Source: [xRouter (arXiv:2510.08439)](https://arxiv.org/abs/2510.08439), [GitHub](https://github.com/SalesforceAIResearch/xRouter)

### 2.3 Universal Model Routing (Feb 2025)

Proposes a unified cascade-routing framework that integrates routing (parallel model selection) and cascading (sequential fallback) into a theoretically optimal strategy.

> Source: [Universal Model Routing (arXiv:2502.08773)](https://arxiv.org/html/2502.08773v1)

### 2.4 Dynamic Routing Survey (Mar 2026)

Comprehensive survey covering routing taxonomies: contextual bandits, k-NN, matrix factorization, graph neural networks, and hybrid orchestration with Prompt Declaration Language (PDL).

> Source: [Dynamic Model Routing Survey (arXiv:2603.04445)](https://arxiv.org/html/2603.04445)

### 2.5 Keyword-Based vs Embedding-Based Routing

| Approach | Pros | Cons | Latency |
|----------|------|------|---------|
| **Keyword/regex** | Fast, deterministic, no model needed | Brittle, poor generalization | <1ms |
| **Embedding similarity** | Semantic understanding, generalizes well | Requires embedding model, vector DB | 5-20ms |
| **ML classifier (BERT)** | Learned complexity detection | Training data needed, medium latency | 10-50ms |
| **LLM-as-router** | Best accuracy, can reason about routing | Expensive, adds latency | 100ms+ |

**Production recommendation**: Start with keyword/regex for obvious cases (code vs chat vs search), use embedding similarity for nuanced routing, ML classifier for cost-sensitive production. xRouter's tool-calling approach is compelling for advanced setups.

### 2.6 MoE Surveys

Two comprehensive surveys on Mixture of Experts:
- [MoE in LLMs (arXiv:2507.11181)](https://arxiv.org/abs/2507.11181) -- covers gating, routing, hierarchical/sparse configurations
- [Survey on MoE in LLMs (TKDE 2025, arXiv:2407.06204)](https://arxiv.org/pdf/2407.06204) -- expert routing mechanisms and load balancing

### Takeaways for Nexus

1. **Hybrid routing**: Keyword tier (fast) -> embedding tier (semantic) -> classifier tier (complex)
2. **Cost-aware rewards**: Use RouteLLM's preference-data approach or xRouter's RL approach
3. **Cascading fallback**: Strong model only when weaker model confidence is low
4. **Cache routing decisions**: Similar queries should hit the same model

---

## 3. Real-Time System Monitoring in Desktop Apps

### 3.1 Recommended Stack

| Component | Technology | Notes |
|-----------|-----------|-------|
| Metrics collection | `sysinfo` crate (Rust) | CPU, RAM, disk, network, processes |
| GPU metrics | `rocm-smi` / `/sys/class/drm/` | AMD-specific; parse `gpu_busy_percent`, `mem_info_vram_used` |
| Frontend transport | Tauri IPC (`invoke`) or events (`emit`) | JSON serialization via serde |
| Polling strategy | `tokio::time::interval` in background task | Configurable intervals per metric type |

### 3.2 Non-Blocking Patterns

**Pattern A: Tauri Command Polling** (Simple, used by NeoHtop)
```
Frontend setInterval(1000) -> invoke('get_metrics') -> Rust #[tauri::command] -> sysinfo refresh -> JSON response
```

**Pattern B: Background Task + Event Emission** (Better for continuous monitoring)
```
Rust tokio::spawn -> loop { sysinfo.refresh(); app.emit("metrics", data); sleep(1s) }
Frontend: listen("metrics", callback)
```

**Pattern C: Shared State with Mutex** (Best for multi-consumer)
```
Arc<Mutex<System>> shared between background refresh task and on-demand query commands
```

### 3.3 Key sysinfo Considerations

- Reuse `System` instance (do not recreate per poll -- works on deltas)
- Use selective refresh: `refresh_cpu_specifics()`, `refresh_memory()` instead of `refresh_all()`
- Process list is expensive -- poll at 5s intervals, not 1s
- CPU usage requires two samples with delay between them

### 3.4 Reference Projects

- **NeoHtop**: Tauri + Svelte + Rust, real-time process monitor ([GitHub](https://github.com/Abdenasser/neohtop))
- **HardwareVisualizer**: Tauri-based with 30-day history graphs ([GitHub](https://github.com/shm11C3/HardwareVisualizer))
- **tauri-plugin-system-info**: Official plugin for system metrics ([crates.io](https://crates.io/crates/tauri-plugin-system-info))

### 3.5 Tauri vs Electron for Monitoring

| Metric | Tauri | Electron |
|--------|-------|----------|
| Binary size | 2.5-3 MB | 80+ MB |
| RAM usage | 30-40 MB | 100+ MB |
| Startup time | <500ms | 1-3s |
| Backend perf | Native Rust | Node.js |

### Takeaways for Nexus

1. Use `sysinfo` crate with a persistent `System` instance in a `tokio::spawn` background task
2. Emit events from Rust backend rather than polling from frontend for smoother UX
3. Differentiate poll rates: CPU/RAM at 1s, GPU at 2s, processes at 5s
4. For AMD GPU: parse `/sys/class/drm/card*/device/gpu_busy_percent` directly

---

## 4. Embedded Browser / WebView Patterns

### 4.1 Desktop App Approaches

| App | Technology | Pattern |
|-----|-----------|---------|
| Podman Desktop | Electron + WebView | iframe to local HTTP service |
| GitKraken | Electron | Full Electron app, native Node.js |
| Postman | Electron | Embedded web app with REST client |
| Tauri apps | System WebView (WebKitGTK/WebView2) | IPC bridge to Rust backend |

### 4.2 iframe vs Native WebView

| Aspect | iframe (in WebView) | Separate WebView |
|--------|---------------------|------------------|
| Isolation | CSP + sandbox attrs | Process-level isolation |
| Communication | postMessage API | IPC bridge |
| Security | Vulnerable to bridge leaks | Stronger boundary |
| Performance | Shares renderer | Separate renderer |
| Use case | Embedding dashboards/services | Full app panels |

### 4.3 Security Considerations

**Critical finding** from research: When JavaScript bridge objects are injected into all frames (including iframes), any child frame can call bridge methods regardless of origin. There is no reliable way to determine the sender's origin from the app side.

**Mitigations**:
- Enable `sandbox` attribute on iframes: `sandbox="allow-scripts allow-same-origin"`
- Never inject bridge objects into all frames -- restrict to top-level only
- Use CSP `frame-src` to whitelist allowed iframe sources
- For Tauri: configure CSP in `tauri.conf.json` security section

### 4.4 Tauri v2 CSP Configuration

```json
{
  "security": {
    "csp": {
      "default-src": "'self' customprotocol: asset:",
      "connect-src": "ipc: http://ipc.localhost http://localhost:*",
      "frame-src": "http://localhost:* https://trusted-service.local",
      "img-src": "'self' asset: http://asset.localhost blob: data:",
      "script-src": "'self' 'wasm-unsafe-eval'"
    }
  }
}
```

**Best practice**: Bundle web assets locally. Avoid CDN scripts. Use `connect-src` to whitelist only local service ports.

> Sources: [Tauri CSP Docs](https://v2.tauri.app/security/csp/), [WebView Security Best Practices (IJCTT 2024)](https://www.ijcttjournal.org/Volume-72%20Issue-12/IJCTT-V72I12P121.pdf)

### Takeaways for Nexus

1. Use Tauri's system WebView with strict CSP for embedding local web services
2. For Ollama/OpenRouter UIs: iframe to `localhost:PORT` with CSP `frame-src` whitelist
3. Never inject JS bridges into iframes -- use postMessage with origin validation
4. Consider WebView2 (Windows) / WebKitGTK (Linux) differences in sandbox behavior

---

## 5. Streaming SSE in Rust

### 5.1 Recommended Libraries

| Crate | Description | Maturity |
|-------|------------|----------|
| `reqwest-eventsource` | SSE wrapper for reqwest, auto-retry | Stable, well-maintained |
| `eventsource-stream` | Low-level SSE parser for byte streams | Foundation crate |
| `reqwest-sse` | Alternative with `.events()` trait method | Newer |
| `rust-eventsource-client` | LaunchDarkly's SSE client | Production-grade |

### 5.2 Pattern: reqwest-eventsource with OpenRouter/Ollama

```rust
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use futures_util::StreamExt;

async fn stream_completion(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();

    let request = client
        .post("http://localhost:11434/api/generate")  // Ollama
        .json(&serde_json::json!({
            "model": "qwen2.5-coder:7b",
            "prompt": prompt,
            "stream": true
        }));

    let mut es = EventSource::new(request)?;
    let mut full_response = String::new();

    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Open) => { /* connection established */ }
            Ok(Event::Message(msg)) => {
                // Parse SSE data field (JSON for both Ollama and OpenRouter)
                let chunk: serde_json::Value = serde_json::from_str(&msg.data)?;
                if let Some(text) = chunk["response"].as_str() {
                    full_response.push_str(text);
                    // Forward token to UI via channel/callback
                }
                if chunk["done"].as_bool() == Some(true) {
                    es.close();
                }
            }
            Err(err) => {
                eprintln!("SSE error: {}", err);
                es.close();
            }
        }
    }
    Ok(full_response)
}
```

### 5.3 OpenRouter SSE Format

OpenRouter uses OpenAI-compatible SSE with `data: [DONE]` sentinel:

```
data: {"choices":[{"delta":{"content":"Hello"}}]}
data: {"choices":[{"delta":{"content":" world"}}]}
data: [DONE]
```

> Source: [OpenRouter Streaming Docs](https://openrouter.ai/docs/api/reference/streaming)

### 5.4 Ollama SSE Format

Ollama uses newline-delimited JSON (not standard SSE). For true SSE, use the `/api/chat` endpoint with `stream: true`. Each line is a JSON object:

```json
{"model":"qwen2.5-coder:7b","response":"Hello","done":false}
{"model":"qwen2.5-coder:7b","response":"","done":true}
```

**Important**: Ollama's streaming is NDJSON, not SSE. Use `reqwest` with `.bytes_stream()` + manual line parsing, or wrap with `eventsource-stream` if adapting to SSE interface.

### 5.5 Production Patterns

| Concern | Solution |
|---------|----------|
| Backpressure | Use bounded `tokio::sync::mpsc` channel between SSE consumer and UI |
| Timeout | `tokio::time::timeout` wrapping the stream loop |
| Reconnection | `reqwest-eventsource` handles automatic retry |
| Cancellation | `es.close()` + `tokio::select!` with cancellation token |
| Multi-provider | Abstract over provider-specific JSON formats with a trait |

### Takeaways for Nexus

1. Use `reqwest-eventsource` for OpenRouter (true SSE) and raw `reqwest` byte stream for Ollama (NDJSON)
2. Abstract both behind a `StreamingProvider` trait that yields uniform token chunks
3. Use `tokio::sync::mpsc` bounded channel to bridge async stream to UI update loop
4. Implement cancellation via `CancellationToken` from `tokio-util`

---

## Summary: Key Papers and References

### Academic Papers

| Paper | Year | Venue | Key Contribution |
|-------|------|-------|-----------------|
| RouteLLM | 2024 | ICLR 2025 | Preference-data trained routers, 2x cost reduction |
| xRouter | 2025 | arXiv | RL-trained tool-calling router on Qwen2.5-7B |
| Universal Model Routing | 2025 | arXiv | Unified cascade+routing framework |
| Dynamic Routing Survey | 2026 | arXiv | Comprehensive routing taxonomy |
| MoE in LLMs Survey | 2025 | TKDE | Expert gating and routing mechanisms |
| MoE in LLMs (Zhang) | 2025 | arXiv | Hierarchical/sparse MoE configurations |
| Mind the Metrics (AIDE) | 2025 | arXiv | MCP-based IDE telemetry integration |
| WebView Security | 2024 | IJCTT | iframe bridge injection vulnerabilities |

### Industry References

| Source | Topic |
|--------|-------|
| [How Cursor Works](https://blog.sshh.io/p/how-cursor-ai-ide-works) | Agent loop, semantic diffs, vectorstore indexing |
| [Zed AI / ACP](https://zed.dev/blog/zed-ai) | CRDT streaming diffs, Agent Client Protocol |
| [Windsurf/Cascade](https://windsurf.com/) | Deep codebase graph, multi-file agent |
| [NeoHtop](https://github.com/Abdenasser/neohtop) | Tauri + sysinfo real-time monitoring |
| [Tauri CSP](https://v2.tauri.app/security/csp/) | WebView security configuration |
| [reqwest-eventsource](https://docs.rs/reqwest-eventsource/) | Rust SSE client library |
| [OpenRouter Streaming](https://openrouter.ai/docs/api/reference/streaming) | SSE API format |

---

## Action Items for Nexus Phase 2

1. **Model Router**: Implement 3-tier routing (keyword -> embedding -> classifier) using RouteLLM patterns
2. **Agent Loop**: Rust-native tool dispatch loop inspired by Cursor's architecture
3. **System Monitor**: Background `tokio::spawn` with `sysinfo` + event emission to frontend
4. **WebView Security**: Strict CSP config for embedding local services (Ollama UI, dashboards)
5. **SSE Abstraction**: Unified `StreamingProvider` trait over OpenRouter SSE and Ollama NDJSON
6. **Codebase Indexing**: pgvector-based file embedding with query-time re-ranking
