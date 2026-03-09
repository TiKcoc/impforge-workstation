# impforge-engine

**ImpForge AI Engine** — Production-grade AI orchestration for workstation builders.

## Features

- **Neural Trust (Three-Factor Hebbian)** — Biologically-inspired trust scoring using dopamine, novelty, and homeostasis factors (ArXiv 2504.05341, Bi & Poo 1998)
- **Brain v2.0** — FSRS-5 spaced repetition, CLS replay, auto-labeling, context enrichment, Zettelkasten knowledge linking
- **Cascade Router** — Intelligent multi-model routing with fallback chains, cost optimization, and latency-aware selection
- **MAPE-K Self-Healing** — Monitor-Analyze-Plan-Execute feedback loop (IBM 2003) with circuit breakers and automatic recovery
- **Evaluation Pipeline** — Agent-as-a-Judge quality assessment with configurable scoring rubrics
- **System Monitoring** — CPU/GPU temperature, memory, disk, and process health tracking

## Usage

```rust
use impforge_engine::trust::TrustEngine;
use impforge_engine::health::HealthMonitor;
use impforge_engine::cascade::CascadeRouter;

// Initialize trust-gated task execution
let trust = TrustEngine::new();
let health = HealthMonitor::new();
let router = CascadeRouter::new();
```

## License

Business Source License 1.1 (BUSL-1.1). See [LICENSE](LICENSE) for details.

After the change date, the software converts to Apache 2.0.

## Part of ImpForge

This crate is the core engine behind [ImpForge](https://github.com/AiImpDevelopment/impforge) — the AI Workstation Builder. One app. Your complete AI stack.
