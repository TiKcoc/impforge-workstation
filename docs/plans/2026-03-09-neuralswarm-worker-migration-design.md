# NeuralSwarm → ImpForge Worker Migration Design

**Date**: 2026-03-09
**Status**: APPROVED
**Approach**: Incremental Migration (5 Phases)

## Goal

Migrate ALL NeuralSwarm workers, agents, and subsystems from the ork-station Python
orchestrator into ImpForge as a standalone commercial Rust product. Every feature is
ported and adapted for SQLite + Tokio channels (no PostgreSQL/Redis dependency).
Premium features are gated by subscription tier via the BSL 1.1 engine crate.

## Architecture

- **Community (Apache-2.0)**: `src-tauri/` — basic workers, simple scheduling, simple trust/brain
- **Pro/Enterprise (BSL 1.1)**: `crates/impforge-engine/` — advanced algorithms, MOA, DyTopo, evaluation
- **Trait Bridge**: Apache-2.0 traits define interfaces; community provides simple impls; engine provides advanced impls
- **License Gate**: Runtime tier check determines which implementation is used
- **Storage**: SQLite WAL (rusqlite) replaces PostgreSQL; Tokio channels replace Redis Streams
- **GPU**: Runtime detection via nvml-wrapper (NVIDIA) + libamdgpu_top (AMD)
- **Browser**: chromiumoxide 0.9 + chromiumoxide_stealth replaces Playwright/Patchright
- **HTTP Stealth**: wreq replaces curl_cffi for Chrome TLS impersonation

## Subscription Tiers

| Feature | Community (Free) | Pro | Enterprise |
|---|:---:|:---:|:---:|
| System Workers (13) | Basic stubs | Full | Full |
| Self-Healing (10) | Health checks | MAPE-K + CB | Full |
| Advanced Workers (11) | — | Full | Full |
| Brain v2.0 | Simple intervals | FSRS-5 + CLS | Full |
| Trust System | Success rate | Hebbian/STDP | Full |
| Cascade Router | Single model | 5-Tier | Full |
| Evaluation | — | Agent-as-Judge | + Panel Voting |
| Scheduling | Duration | + Cron | Full |
| Resource Governor | — | VRAM/RAM | + Emergency |
| Git Auto-Push | — | Trust-classified | Full |
| MOA Pipeline | — | — | 5-Phase |
| Dynamic Topology | — | — | DyTopo DAG |
| Agent Scaling | — | — | LRU model mgmt |
| Routing & Scoring | — | — | 25-category |
| Social Media (6) | — | — | Full automation |
| Benchmarking | — | — | MT-Bench |
| Monitoring | Basic logs | Prometheus | + Alerts |

## Dependency Map

```
NeuralSwarm (ork-station)          →  ImpForge (Standalone)
─────────────────────────             ─────────────────────
PostgreSQL 16 + pgvector           →  SQLite WAL (rusqlite)
Redis 7 Streams                    →  Tokio broadcast channels + DashMap
systemd user services              →  Tokio scheduler + croner
Ollama (localhost:11434)           →  Ollama (customer's install)
Playwright/Patchright              →  chromiumoxide 0.9
curl_cffi (TLS spoof)             →  wreq (Chrome TLS impersonation)
BGE-M3 embeddings (PG)            →  FastEmbed + LanceDB
gh CLI                             →  octocrab 0.49 (already in Cargo.toml)
APScheduler (cron)                 →  croner 3.0 + custom Tokio loop
numpy cosine similarity            →  ndarray 0.16
asyncio.gather                     →  tokio::spawn + JoinSet
Redis consumer groups              →  Tokio mpsc + broadcast channels
```

## New Crate Dependencies

| Crate | Version | Purpose |
|---|---|---|
| croner | 3.0 | Cron expression parsing (MIT) |
| chromiumoxide_stealth | latest | Anti-bot CDP patches (MIT) |
| wreq | latest | Chrome TLS impersonation (MIT) |
| ndarray | 0.16 | Vectorized matrix ops for DyTopo (MIT/Apache-2.0) |

Already present (no new deps): octocrab 0.49, git2 0.20, chromiumoxide 0.9,
fastembed 5.12, sysinfo 0.38, reqwest 0.12, lancedb 0.26

## File Structure

### Community (Apache-2.0): src-tauri/src/

```
traits/
├── mod.rs                          # Trait re-exports
├── trust_scorer.rs                 # trait TrustScorer
├── brain_engine.rs                 # trait BrainEngine
├── inference_router.rs             # trait InferenceRouter
├── moa_pipeline.rs          NEW    # trait MoAPipeline
├── topology_manager.rs      NEW    # trait TopologyManager
├── agent_scaler.rs          NEW    # trait AgentScaler
├── evaluation_judge.rs      NEW    # trait EvaluationJudge
└── community.rs                    # Simple implementations

orchestrator/
├── mod.rs                          # Scheduler + worker execution
├── store.rs                        # SQLite persistence
├── events.rs                       # Event bus (ring buffer)
├── health.rs                       # Basic health checks
├── trust.rs                        # Community trust (success rate)
├── brain.rs                        # Community brain (fixed intervals)
├── workers/mod.rs                  # 61 worker implementations
├── message_bus.rs           NEW    # Tokio broadcast channels
├── scheduler_cron.rs        NEW    # croner integration
├── git_autopush.rs          NEW    # Trust-classified git push
└── social_media/            NEW    # Social media module
    ├── mod.rs
    ├── base.rs                     # Rate limiting, credentials, content gen
    ├── profile.rs                  # Professional profile data (static)
    ├── github.rs                   # GitHubPromoter (octocrab)
    ├── linkedin.rs                 # LinkedInManager (chromiumoxide)
    ├── fiverr.rs                   # FiverrManager (chromiumoxide)
    ├── upwork.rs                   # UpworkManager (wreq + chromiumoxide)
    ├── hackernews.rs               # HackerNewsPublisher (reqwest)
    └── readme.rs                   # NexusReadmeManager (octocrab + git2)
```

### Engine (BSL 1.1): crates/impforge-engine/src/

```
# Existing (4,979 LoC)
brain.rs                            # FSRS-5, CLS, TeleMem, Zettel
trust.rs                            # Three-Factor Hebbian/STDP
cascade.rs                          # 5-Tier inference router
health.rs                           # MAPE-K autonomic loop
evaluation.rs                       # Agent-as-a-Judge
monitoring.rs                       # Prometheus metrics
browser.rs                          # SmartBrowser planner
license.rs                          # Ed25519 validation
license_keygen.rs                   # Key generation
license_store.rs                    # SQLite license cache

# NEW (~4,500 LoC estimated)
moa.rs                              # MOA Pipeline (5-phase orchestration)
moa_critique.rs                     # Cross-critique engine (AKV synthesis)
moa_aggregator.rs                   # Residual aggregator (skip connections)
moa_early_stop.rs                   # Convergence detection (embedding cosine)
topology.rs                         # DyTopo manager (5-phase cycle)
topology_embeddings.rs              # Cosine similarity matrix (ndarray)
scaling.rs                          # Model slot management (LRU eviction)
scaling_monitor.rs                  # Resource monitoring (5 emergency levels)
routing.rs                          # Category router (25 categories)
scoring.rs                          # Complexity scorer (multi-signal heuristic)
benchmarks.rs                       # Benchmark harness (MT-Bench + quality)
message_bus_pro.rs                  # Consumer group simulation
```

## Worker Inventory (61 Total)

### Tier 1: System Monitoring (13) — Community

| # | Worker | ImpForge Status | Action | NeuralSwarm Source |
|---|---|---|---|---|
| 1 | McpWatchdog | REAL (25 LoC) | Upgrade with NS logic | mcp_watchdog.py |
| 2 | VramManager | REAL (30 LoC) | Upgrade with NS logic | vram_manager.py |
| 3 | LogAnalyzer | REAL (30 LoC) | Upgrade with NS logic | log_analyzer.py |
| 4 | AnomalyDetector | REAL (25 LoC) | Upgrade with NS logic | anomaly_detector.py |
| 5 | TerminalDigester | STUB | Implement from NS | terminal_digester.py |
| 6 | ModelHealth | REAL (35 LoC) | Upgrade with NS logic | stubs.py |
| 7 | DependencyAuditor | STUB | Implement from NS | stubs.py |
| 8 | DocSync | STUB | Implement (adapt for ImpForge paths) | stubs.py |
| 9 | TestRunner | STUB | Implement (cargo test runner) | stubs.py |
| 10 | KgEnricher | STUB | Implement (SQLite-based) | stubs.py |
| 11 | BackupAgent | STUB | Implement (SQLite backup) | stubs.py |
| 12 | CodeQuality | REAL (55 LoC) | Keep | stubs.py |
| 13 | ReleaseBuilder | STUB | Implement from NS | stubs.py |

### Tier 2: Self-Healing & Intelligence (10) — Pro

| # | Worker | ImpForge Status | Action | NeuralSwarm Source |
|---|---|---|---|---|
| 14 | SelfHealer | STUB | Implement MAPE-K remediation | stubs.py |
| 15 | SemanticDiff | STUB | Implement with local embeddings | stubs.py |
| 16 | ConfigDrift | REAL (45 LoC) | Keep | — |
| 17 | PerfTracker | REAL (30 LoC) | Upgrade with NS trends | stubs.py |
| 18 | SecuritySentinel | REAL (50 LoC) | Keep | stubs.py |
| 19 | TrustScorer | STUB | Wire to Hebbian trust engine | stubs.py |
| 20 | DeadCode | STUB | Implement (AST-based) | stubs.py |
| 21 | CrossRepo | REAL (45 LoC) | Keep | stubs.py |
| 22 | CachePruner | REAL (60 LoC) | Keep | stubs.py |
| 23 | CommitGate | REAL (35 LoC) | Upgrade with trust levels | stubs.py |

### Tier 3: Advanced Automation (11) — Pro

| # | Worker | ImpForge Status | Action | NeuralSwarm Source |
|---|---|---|---|---|
| 24 | ChangelogGen | REAL (40 LoC) | Keep | stubs.py |
| 25 | ApiValidator | STUB | Implement (HTTP health) | stubs.py |
| 26 | ResourceForecast | REAL (25 LoC) | Upgrade with NS trends | stubs.py |
| 27 | MigrationPlanner | STUB | Implement (SQLite migrations) | stubs.py |
| 28 | StaleCleaner | STUB | Implement (SQLite cleanup) | stubs.py |
| 29 | EmbeddingRefresh | STUB | Implement (FastEmbed re-embed) | stubs.py |
| 30 | ServiceMapper | STUB | Implement (process-based) | stubs.py |
| 31 | SystemSnapshot | REAL (35 LoC) | Upgrade with NS format | stubs.py |
| 32 | DedupSweeper | REAL (140 LoC) | Enhance (add semantic tier) | dedup_sweeper.py |
| 33 | BuildVerifier | REAL (85 LoC) | Keep | — |
| 34 | GitAutoPush | NEW | Implement trust-classified push | git_autopush.py |

### Brain v2.0 (9) — Pro

| # | Worker | ImpForge Status | Action | NeuralSwarm Source |
|---|---|---|---|---|
| 35 | MemoryDecayScorer | REAL (65 LoC) | Wire to FSRS engine | memory_decay.py |
| 36 | ClsReplay | REAL (70 LoC) | Wire to CLS engine | cls_replay.py |
| 37 | AutoLabeler | STUB | Implement 3-tier labeling | auto_labeler.py (720 LoC) |
| 38 | ContextEnricher | REAL (55 LoC) | Upgrade with Anthropic method | context_enricher.py |
| 39 | KgTemporalUpdater | STUB | Implement bi-temporal edges | kg_temporal.py |
| 40 | DigestProcessor | STUB | Implement digest pipeline | digest_processor.py |
| 41 | RlmSessionManager | STUB | Implement session lifecycle | rlm_session_manager.py |
| 42 | ContextCacheWarmer | STUB | Implement pre-compute cache | context_cache_warmer.py |
| 43 | ZettelkastenIndexer | REAL (65 LoC) | Enhance with A-MEM links | zettelkasten_indexer.py |
| 44 | TelememPipeline | REAL (55 LoC) | Wire to TeleMem engine | telemem_pipeline.py |

### Social Media (6) — Enterprise

| # | Worker | Action | NeuralSwarm Source | Rust Approach |
|---|---|---|---|---|
| 45 | GitHubPromoter | NEW | github_promoter.py (1,274 LoC) | octocrab (no browser needed) |
| 46 | LinkedInManager | NEW | linkedin_manager.py (1,002 LoC) | chromiumoxide + stealth |
| 47 | FiverrManager | NEW | fiverr_manager.py (750 LoC) | chromiumoxide + stealth |
| 48 | UpworkManager | NEW | upwork_manager.py (605 LoC) | wreq + chromiumoxide |
| 49 | HackerNewsPublisher | NEW | hackernews_publisher.py (480 LoC) | reqwest (HTTP only) |
| 50 | NexusReadmeManager | NEW | nexus_readme_manager.py (540 LoC) | octocrab + git2 |

### Enterprise Features (11 workers) — Enterprise

| # | Worker/Module | Action | NeuralSwarm Source | LoC Est. |
|---|---|---|---|---|
| 51 | MoAPipeline | NEW in engine | moa/pipeline.py (248 LoC) | ~400 Rust |
| 52 | MoACritique | NEW in engine | moa/critique.py (439 LoC) | ~600 Rust |
| 53 | MoAAggregator | NEW in engine | moa/aggregator.py (189 LoC) | ~300 Rust |
| 54 | MoAEarlyStop | NEW in engine | moa/early_stop.py (188 LoC) | ~250 Rust |
| 55 | TopologyManager | NEW in engine | topology/manager.py (278 LoC) | ~400 Rust |
| 56 | TopologyEmbeddings | NEW in engine | topology/embeddings.py (341 LoC) | ~350 Rust |
| 57 | ModelScaler | NEW in engine | scaling/model_manager.py (233 LoC) | ~350 Rust |
| 58 | ResourceMonitor | NEW in engine | scaling/resource_monitor.py (212 LoC) | ~300 Rust |
| 59 | CategoryRouter | NEW in engine | routing/router.py (134 LoC) | ~200 Rust |
| 60 | ComplexityScorer | NEW in engine | scoring/complexity.py (284 LoC) | ~350 Rust |
| 61 | BenchmarkHarness | NEW in engine | benchmarks/harness.py (146 LoC) | ~200 Rust |

## Scheduling Upgrade

```rust
// Current (Duration only)
fn interval(&self) -> Duration;

// New (Cron + Duration hybrid, parsed by croner 3.0)
pub enum Schedule {
    Interval(Duration),
    Cron(String),       // "0 */6 * * *"
    EventDriven(EventType),  // Triggered by event bus
}

fn schedule(&self) -> Schedule;
```

Event-driven trigger mapping (from NeuralSwarm config):
- FileChanged → semantic_diff, doc_sync, test_runner, security_sentinel, embedding_refresh
- ServiceDown → self_healer, mcp_watchdog
- CommitReady → commit_gate
- TagRelease → release_builder, changelog_gen

## Message Bus Architecture

```rust
pub struct MessageBus {
    // Typed broadcast channels (replace Redis Streams)
    task_tx: broadcast::Sender<TaskMessage>,
    proposal_tx: broadcast::Sender<ProposalMessage>,
    critique_tx: broadcast::Sender<CritiqueMessage>,
    topology_tx: broadcast::Sender<TopologyDescriptor>,
    result_tx: broadcast::Sender<ResultMessage>,
    health_tx: broadcast::Sender<HealthMessage>,
    tool_tx: broadcast::Sender<ToolMessage>,

    // SQLite persistence for audit trail
    db: Arc<OrchestratorStore>,
}

// Consumer group simulation (Enterprise)
pub struct ConsumerGroup {
    name: String,
    rx: broadcast::Receiver<Message>,
    pending: HashMap<String, Message>,
}
```

## CI/CD Matrix

```yaml
strategy:
  matrix:
    include:
      - platform: ubuntu-22.04    # deb (Ubuntu/Debian/PopOS)
        features: ""
      - platform: ubuntu-22.04    # deb + NVIDIA
        features: "--features nvidia"
      - platform: ubuntu-22.04    # rpm (Fedora/RHEL)
        bundle: "rpm"
      - platform: ubuntu-22.04    # AppImage (universal)
        bundle: "appimage"
      - platform: windows-latest  # NSIS installer
        features: ""
      - platform: macos-latest    # dmg (Apple Silicon)
        target: "aarch64-apple-darwin"
      - platform: macos-latest    # dmg (Intel)
        target: "x86_64-apple-darwin"
```

Arch Linux: AUR PKGBUILD (builds from source or repackages deb)

## Scientific Foundations

| Feature | Paper | Key Innovation |
|---|---|---|
| MOA Pipeline | arXiv:2601.16596 | Residual skip connections prevent layer degradation |
| DyTopo | arXiv:2602.06039 | Sparse DAG saves 30-70% inter-agent messages |
| Complexity Scoring | arXiv:2406.07155 (MacNet) | Multi-signal heuristic <1ms |
| Evaluation | arXiv:2306.05685 (MT-Bench) | Position debiasing eliminates LLM bias |
| Panel Judging | arXiv:2411.15594 (CPAD) | Ensemble majority voting |
| Trust | arXiv:2504.05341 + Bi&Poo 1998 | Three-Factor Hebbian/STDP |
| Brain FSRS-5 | Open Spaced Repetition | 19-parameter optimal scheduling |
| CLS Replay | McClelland et al. 1995 | Hippocampus to neocortex consolidation |
| A-MEM Zettel | arXiv:2409.07625 | Bidirectional knowledge linking |
| TeleMem | Ebbinghaus (1885) + FSRS | Memory lifecycle management |
| Resource Scaling | arXiv:2309.06180 (PagedAttention) | LRU model eviction |
| Agent Scaling | arXiv:2311.03285 (S-LoRA) | Dynamic multi-model management |
| Category Routing | MasRouter (ACL 2025) | Two-stage cascaded routing |
| Anti-Bot | Chrome TLS impersonation | wreq + chromiumoxide_stealth |

## Migration Phases

### Phase 1: Foundation (~800 LoC)
- Cron scheduler (croner integration)
- Message bus (Tokio broadcast channels)
- Worker pool isolation (cpu/gpu/shell/embed)
- Event-driven triggers (FileChanged, ServiceDown, etc.)
- Schedule enum (Interval + Cron + EventDriven)

### Phase 2: Community Workers (~2,000 LoC)
- Implement 20 stub workers with NeuralSwarm logic
- Upgrade 18 existing real workers
- All workers pass cargo test

### Phase 3: Pro Features (~1,500 LoC in engine)
- Wire Brain v2.0 workers to FSRS/CLS engine
- Wire Trust workers to Hebbian engine
- Git Auto-Push with trust classification
- Resource Governor (VRAM/RAM management)
- Enhanced evaluation (Agent-as-a-Judge)

### Phase 4: Enterprise Features (~3,500 LoC in engine)
- MOA Pipeline (5-phase: propose, critique, refine, check, aggregate)
- DyTopo (5-phase: collect, embed, compute, threshold, sort)
- Agent Scaling (LRU model slots + 5 emergency levels)
- Routing & Scoring (25-category + complexity heuristic)
- Benchmarking (MT-Bench harness)
- Consumer group simulation

### Phase 5: Social Media + CI/CD (~2,500 LoC)
- 6 social media workers (chromiumoxide + wreq + octocrab)
- Social media base module (rate limiting, credentials, content gen)
- Professional profile config (embedded Rust structs)
- Multi-distro CI/CD (deb, rpm, AppImage, AUR)
- GPU variant builds (NVIDIA feature flag)

## Key Decisions

1. **Workers copied from NeuralSwarm, adapted for Rust** (originals stay in AiImp)
2. **Only universal workers ported** (ork-station-specific ones adapted or skipped)
3. **Cron expressions via croner** (replaces Duration-only scheduling)
4. **Runtime GPU detection** (single binary per platform, GPU detected at startup)
5. **All social media in Rust** (chromiumoxide replaces Playwright)
6. **SQLite for everything** (no external DB dependency)
7. **Trait bridge pattern** (Apache-2.0 traits, BSL engine implementations)
8. **5-phase incremental** (each phase independently testable and shippable)
