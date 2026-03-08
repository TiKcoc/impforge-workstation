# ImpForge Licensing & Architecture Design

**Date**: 2026-03-08
**Status**: Approved
**License Model**: MIT (public) + Proprietary Private Crate (IP protection)

## 1. License Strategy

### Decision: MIT + Private Crate

ImpForge uses a dual-structure approach:

- **Public repository** (MIT): The Tauri shell, basic UI, Ollama bridge, SQLite store,
  settings, event bus interfaces, and all Svelte frontend code.
- **Private crate `impforge-engine`** (All Rights Reserved): The novel AI algorithms
  that constitute ImpForge's competitive advantage. This crate is never published.

### Why Not BSL?

BSL 1.1 was designed by MariaDB to prevent cloud providers from hosting open-source
databases as a service. ImpForge is a desktop application -- the threat model BSL
addresses (AWS/Azure hosting your product) does not apply. Additionally:

- BSL requires legal review for the Additional Use Grant wording (~$5-15K)
- BSL has no court precedent (legally untested as of 2026)
- MariaDB and CockroachDB have both moved away from BSL
- The real protection is architectural (private code), not legal (license text)

### Why MIT over Apache 2.0?

MIT was chosen for simplicity and ecosystem compatibility. Apache 2.0's patent clause
provides marginal benefit for a project that doesn't accept external contributions at
scale yet. If ImpForge grows to accept many contributors, upgrading to Apache 2.0 is
a non-breaking change (MIT is compatible with Apache 2.0).

## 2. Module License Map

### MIT (Public -- commodity code anyone could rebuild)

| Module | Path | Rationale |
|--------|------|-----------|
| Tauri Shell | `lib.rs`, `main.rs` | Framework boilerplate |
| Basic Chat | `chat.rs` | Standard LLM UI |
| Settings | `settings.rs` | Preferences storage |
| Error Types | `error.rs` | Common error handling |
| AI Providers | `ai/mod.rs`, `ai/providers.rs` | Ollama/API bridge |
| Inference | `inference/*.rs` | HF Hub, GGUF loading |
| Docker | `docker/mod.rs` | Bollard wrapper |
| GitHub | `github/mod.rs` | Octocrab wrapper |
| IDE | `ide/mod.rs` | IDE integration |
| Monitoring | `monitoring/*.rs` | sysinfo/GPU stats |
| System Agent | `system_agent.rs` | System info |
| Widget Registry | `widget_registry.rs` | UI widget catalog |
| Orchestrator Traits | `orchestrator/mod.rs` | Interfaces only |
| Orchestrator Store | `orchestrator/store.rs` | SQLite state |
| Orchestrator Events | `orchestrator/events.rs` | Basic event bus |
| Svelte Frontend | `src/**/*.svelte`, `src/**/*.ts` | All UI code |

### Proprietary (Private Crate -- novel research, never published)

| Module | Future Location | Scientific Foundation |
|--------|----------------|----------------------|
| Three-Factor Hebbian Trust | `impforge-engine/src/trust.rs` | arXiv:2504.05341, Bi & Poo 1998, Turrigiano 2008 |
| Brain v2.0 (FSRS-5 + CLS) | `impforge-engine/src/brain.rs` | IEEE TKDE 2023, McClelland et al. 1995 |
| Intelligent Router | `impforge-engine/src/router.rs` | RouteLLM ICLR 2025, arXiv:2512.22402 |
| MAPE-K Self-Healing | `impforge-engine/src/health.rs` | IBM 2003, ECSA 2025 |
| Worker Implementations | `impforge-engine/src/workers/` | 42 specialized task workers |

## 3. Architecture: Trait-Based Separation

The public repo defines traits (interfaces). The private crate implements them.

```rust
// PUBLIC: orchestrator/mod.rs (MIT)
pub trait TrustScorer: Send + Sync {
    fn score(&self, worker_id: &str, outcome: TaskOutcome) -> f64;
}

pub trait BrainEngine: Send + Sync {
    fn schedule_review(&self, item_id: &str) -> Option<DateTime<Utc>>;
    fn record_outcome(&mut self, item_id: &str, grade: Grade);
}

pub trait TaskRouter: Send + Sync {
    fn route(&self, input: &str, context: &TaskContext) -> RoutingDecision;
}

pub trait HealthMonitor: Send + Sync {
    fn check(&self) -> SystemHealth;
    fn heal(&mut self, issue: &HealthIssue) -> HealResult;
}
```

```rust
// PUBLIC: fallback implementations (MIT)
// These ship with the community build -- functional but basic
pub struct SimpleTrust;
impl TrustScorer for SimpleTrust {
    fn score(&self, _id: &str, outcome: TaskOutcome) -> f64 {
        match outcome {
            TaskOutcome::Success => 0.8,
            TaskOutcome::Failure => 0.3,
            TaskOutcome::Timeout => 0.5,
        }
    }
}
```

```rust
// PRIVATE: impforge-engine/src/trust.rs (proprietary, never published)
pub struct ThreeFactorTrust {
    stdp_window: ExponentialDecay,
    dopamine: f64,
    homeostasis: HomeostaticRegulator,
    bcm_threshold: f64,
}
impl TrustScorer for ThreeFactorTrust { /* ... */ }
```

## 4. Cargo.toml Feature Flags

```toml
[features]
default = ["custom-protocol", "community"]
custom-protocol = ["tauri/custom-protocol"]
nvidia = ["nvml-wrapper"]

# Tier: Community (MIT modules only, basic fallbacks)
community = []

# Tier: Pro (includes private engine crate)
pro = ["dep:impforge-engine"]

[dependencies]
impforge-engine = { path = "../impforge-engine", optional = true }
```

## 5. Pricing Tiers (Future)

| | Community (Free) | Pro ($19/mo) | Enterprise (Custom) |
|---|---|---|---|
| License | MIT (public repo) | Pro binary | Pro + SLA |
| AI Models | Unlimited (Ollama) | + Cloud providers | + Custom models |
| Trust Engine | SimpleTrust | Three-Factor Hebbian | + Custom rules |
| Brain | No scheduling | FSRS-5 + CLS | + Analytics |
| Router | Round-robin | Intelligent (ML) | + Custom classifiers |
| Self-Healing | Basic restart | Full MAPE-K | + Custom policies |
| Workers | 5 basic | 42 full | Unlimited |
| Support | GitHub Issues | Email | Priority SLA |

## 6. CI/CD: License Header Check

```yaml
# .github/workflows/license-check.yml
name: License Headers
on: [push, pull_request]
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Verify MIT headers in public modules
        run: |
          for f in src-tauri/src/lib.rs src-tauri/src/main.rs \
                   src-tauri/src/chat.rs src-tauri/src/settings.rs \
                   src-tauri/src/error.rs; do
            head -3 "$f" | grep -q "MIT" || echo "WARNING: $f missing MIT header"
          done
```

## 7. What a Fork Gets

Someone who forks the public repo gets:
- Working Tauri desktop app with basic chat UI
- Ollama integration (connect to any local LLM)
- SQLite storage, settings, Docker integration
- GitHub integration, IDE hooks, system monitoring
- All Svelte UI components and routes
- Basic orchestrator with SimpleTrust and round-robin routing

Someone who forks does NOT get:
- Three-Factor Hebbian trust scoring (novel research)
- FSRS-5 brain with CLS replay (novel combination)
- DistilBERT intelligent router (ML-based)
- MAPE-K self-healing loop
- 42 specialized worker implementations
- The algorithms that make ImpForge intelligent

## 8. References

- RouteLLM (ICLR 2025): 3.66x cost savings with learned routers
- Three-Factor Learning in SNNs (arXiv:2504.05341)
- FSRS-5 (IEEE TKDE 2023): Spaced repetition for AI knowledge
- MAPE-K + Agentic AI (ECSA 2025)
- Bi & Poo 1998: STDP timing windows
- Turrigiano 2008: Homeostatic plasticity
- McClelland et al. 1995: Complementary Learning Systems
- Pick and Spin (arXiv:2512.22402): Multi-model orchestration
- RLM (arXiv:2512.24601): Recursive Language Models
