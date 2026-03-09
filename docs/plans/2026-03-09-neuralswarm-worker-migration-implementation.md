# NeuralSwarm → ImpForge Worker Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port all NeuralSwarm workers and subsystems into ImpForge as standalone Rust, with premium features in the BSL 1.1 engine crate.

**Architecture:** Incremental 5-phase migration. Each phase produces a cargo-test-passing, shippable state. Community features in src-tauri/ (Apache-2.0), premium in crates/impforge-engine/ (BSL 1.1).

**Tech Stack:** Rust, Tauri 2.10, SQLite WAL (rusqlite), Tokio, croner 3.0, chromiumoxide 0.9, wreq, octocrab 0.49, ndarray 0.16, fastembed 5.12

---

## Phase 1: Foundation Infrastructure

### Task 1: Add croner dependency and Schedule enum

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/orchestrator/mod.rs`
- Create: `src-tauri/src/orchestrator/scheduler_cron.rs`

**Step 1:** Add `croner = "3.0"` to Cargo.toml under CODEFORGE IDE section.

**Step 2:** Create `scheduler_cron.rs` with:
```rust
use croner::Cron;
use std::time::Duration;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Schedule {
    Interval(Duration),
    Cron(String),
    EventDriven(String), // event type name
}

impl Schedule {
    pub fn next_run(&self) -> Option<Duration> {
        match self {
            Schedule::Interval(d) => Some(*d),
            Schedule::Cron(expr) => {
                let cron = Cron::new(expr).parse().ok()?;
                let next = cron.find_next_occurrence(&chrono::Utc::now(), false).ok()?;
                let delta = next - chrono::Utc::now();
                Some(Duration::from_secs(delta.num_seconds().max(1) as u64))
            }
            Schedule::EventDriven(_) => None, // triggered by events, not time
        }
    }
}
```

**Step 3:** Update TaskWorker trait in workers/mod.rs to use Schedule:
```rust
fn schedule(&self) -> Schedule {
    Schedule::Interval(self.interval())
}
```
Keep `interval()` as default fallback for backward compatibility.

**Step 4:** Write tests for Schedule::next_run with cron expressions.

**Step 5:** Run `cargo test` — all existing tests must still pass + new cron tests.

**Step 6:** Commit: `feat(orchestrator): add cron scheduling via croner`

---

### Task 2: Event-driven trigger system

**Files:**
- Modify: `src-tauri/src/orchestrator/events.rs`
- Modify: `src-tauri/src/orchestrator/mod.rs`

**Step 1:** Add trigger mapping to EventBus:
```rust
pub struct EventTrigger {
    pub event_type: EventType,
    pub worker_names: Vec<String>,
}

impl EventBus {
    pub fn register_trigger(&mut self, event_type: EventType, worker: &str) { ... }
    pub fn get_triggered_workers(&self, event_type: &EventType) -> Vec<String> { ... }
}
```

**Step 2:** Wire triggers in orchestrator loop:
- FileChanged → ["semantic_diff", "doc_sync", "test_runner", "security_sentinel"]
- ServiceDown → ["self_healer", "mcp_watchdog"]
- TaskCompleted → ["trust_scorer"]

**Step 3:** Write tests for trigger registration and worker lookup.

**Step 4:** Run `cargo test` — verify.

**Step 5:** Commit: `feat(orchestrator): add event-driven worker triggers`

---

### Task 3: Message bus with typed channels

**Files:**
- Create: `src-tauri/src/orchestrator/message_bus.rs`
- Modify: `src-tauri/src/orchestrator/mod.rs`

**Step 1:** Create message types:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskMessage { pub task_id: String, pub query: String, pub complexity: f32 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalMessage { pub task_id: String, pub agent_id: String, pub content: String, pub layer: u32 }

// ... CritiqueMessage, TopologyDescriptor, ResultMessage, HealthMessage, ToolMessage
```

**Step 2:** Create MessageBus:
```rust
pub struct MessageBus {
    task_tx: broadcast::Sender<TaskMessage>,
    proposal_tx: broadcast::Sender<ProposalMessage>,
    // ... all 7 channels
    db: Arc<OrchestratorStore>,
}
```

**Step 3:** Add SQLite persistence table for message audit trail.

**Step 4:** Write tests for publish/subscribe round-trip.

**Step 5:** Run `cargo test` — verify.

**Step 6:** Commit: `feat(orchestrator): add typed message bus`

---

### Task 4: Worker pool isolation

**Files:**
- Modify: `src-tauri/src/orchestrator/mod.rs`

**Step 1:** Add semaphore-based pool limits:
```rust
pub struct WorkerPoolConfig {
    pub cpu_max: usize,     // default 4
    pub gpu_max: usize,     // default 2
    pub shell_max: usize,   // default 3
    pub embed_max: usize,   // default 1
}
```

**Step 2:** Use `tokio::sync::Semaphore` per pool to limit concurrent workers.

**Step 3:** Wire pool checking into scheduler loop before worker dispatch.

**Step 4:** Write tests for pool saturation (submit more workers than pool allows).

**Step 5:** Run `cargo test` — verify.

**Step 6:** Commit: `feat(orchestrator): enforce worker pool limits with semaphores`

---

## Phase 2: Community Workers (Implement Stubs)

### Task 5: Implement DependencyAuditor worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Replace stub with real implementation:
- Scan Cargo.toml for outdated crate versions
- Check package.json if present
- Report outdated dependencies as WorkerResult

**Step 2:** Write test for dependency scanning.

**Step 3:** Commit: `feat(workers): implement DependencyAuditor`

---

### Task 6: Implement TestRunner worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Replace stub:
- Run `cargo test --workspace` via tokio::process::Command
- Parse output for pass/fail counts
- Report results

**Step 2:** Write test.

**Step 3:** Commit: `feat(workers): implement TestRunner`

---

### Task 7: Implement DocSync worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Replace stub:
- Use git2 to check for modified .md files
- Report documentation drift

**Step 2:** Write test.

**Step 3:** Commit: `feat(workers): implement DocSync`

---

### Task 8: Implement BackupAgent worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Replace stub:
- Copy SQLite database to backup location with timestamp
- Prune backups older than 30 days
- Use rusqlite backup API

**Step 2:** Write test with tempfile.

**Step 3:** Commit: `feat(workers): implement BackupAgent`

---

### Task 9: Implement ReleaseBuilder worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Replace stub:
- Use git2 to get latest tag
- Parse Cargo.toml version
- Check if version matches tag
- Report version status

**Step 2:** Write test.

**Step 3:** Commit: `feat(workers): implement ReleaseBuilder`

---

### Task 10: Implement remaining Tier 1-2 stubs (batch)

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

Implement these stubs (each ~20-40 LoC):
- TerminalDigester: Parse recent terminal output for actionable items
- KgEnricher: Find memories without tags in SQLite
- SelfHealer: HTTP health check + restart suggestion
- SemanticDiff: Use fastembed to compute diff embeddings
- TrustScorer: Call trust engine to recalculate scores
- DeadCode: Scan for unreferenced files via AST

**Step 1:** Implement each worker.

**Step 2:** Write tests for each.

**Step 3:** Commit: `feat(workers): implement Tier 1-2 stub workers`

---

### Task 11: Implement remaining Tier 3 stubs (batch)

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

Implement:
- ApiValidator: HTTP GET to configured endpoints
- MigrationPlanner: Check SQLite schema version
- StaleCleaner: DELETE completed tasks > 30 days
- EmbeddingRefresh: Count entries needing re-embedding
- ServiceMapper: Use sysinfo to map running processes

**Step 1:** Implement each.

**Step 2:** Write tests.

**Step 3:** Commit: `feat(workers): implement Tier 3 stub workers`

---

### Task 12: Implement Brain v2.0 stubs (batch)

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

Implement:
- AutoLabeler: Regex rules → embedding similarity → Ollama LLM (3-tier)
- KgTemporalUpdater: Add t_valid + t_created to SQLite edges
- DigestProcessor: Process pending digest queue
- RlmSessionManager: Track active sessions, pre-load popular
- ContextCacheWarmer: Pre-compute frequent query contexts

**Step 1:** Implement each with SQLite backend.

**Step 2:** Write tests.

**Step 3:** Commit: `feat(workers): implement Brain v2.0 stub workers`

---

## Phase 3: Pro Features (Engine Wiring)

### Task 13: Wire Trust workers to Hebbian engine

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`
- Modify: `src-tauri/src/orchestrator/mod.rs`

**Step 1:** Update TrustScorer worker to call engine's HebbianTrustManager via trait.

**Step 2:** Update orchestrator loop to record success/failure via trait.

**Step 3:** Test trust score changes after worker runs.

**Step 4:** Commit: `feat(orchestrator): wire trust workers to Hebbian engine`

---

### Task 14: Wire Brain workers to FSRS/CLS engine

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** MemoryDecayScorer → call engine FSRS5Scheduler.
**Step 2:** ClsReplay → call engine ClsReplayEngine.
**Step 3:** ZettelkastenIndexer → call engine ZettelkastenEngine.
**Step 4:** TelememPipeline → call engine TeleMemPipeline.

**Step 5:** Tests for each wiring.

**Step 6:** Commit: `feat(brain): wire memory workers to FSRS/CLS engine`

---

### Task 15: Implement Git Auto-Push worker

**Files:**
- Create: `src-tauri/src/orchestrator/git_autopush.rs`
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`
- Modify: `src-tauri/src/orchestrator/mod.rs`

**Step 1:** Create trust-level classifier:
```rust
pub enum TrustLevel { Auto, Verify, Review }

pub fn classify_file(path: &str) -> TrustLevel {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("md" | "txt" | "yaml" | "json" | "lock") => TrustLevel::Auto,
        Some("rs" | "py" | "ts" | "svelte" | "css" | "html" | "js") => TrustLevel::Verify,
        Some("toml" | "env") => TrustLevel::Review,
        _ => TrustLevel::Verify,
    }
}
```

**Step 2:** Create GitAutoPush struct with git2 operations.

**Step 3:** Create GitAutoPushWorker that runs on schedule.

**Step 4:** Tests for trust classification + commit generation.

**Step 5:** Commit: `feat(orchestrator): add trust-classified git auto-push`

---

### Task 16: Implement Resource Governor worker

**Files:**
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Implement 5 emergency levels from NeuralSwarm:
- Level 0: Normal
- Level 1: VRAM < 4GB → suggest unload
- Level 2: VRAM < 2GB → force unload
- Level 3: RAM < 6GB → reduce models
- Level 4: RAM < 4GB → minimum config

**Step 2:** Read GPU info via existing VramManager patterns.

**Step 3:** Tests for each emergency level.

**Step 4:** Commit: `feat(workers): implement ResourceGovernor with 5 emergency levels`

---

## Phase 4: Enterprise Features (Engine Crate)

### Task 17: New trait interfaces for Enterprise features

**Files:**
- Modify: `src-tauri/src/traits/mod.rs`
- Create: `src-tauri/src/traits/moa_pipeline.rs`
- Create: `src-tauri/src/traits/topology_manager.rs`
- Create: `src-tauri/src/traits/agent_scaler.rs`
- Create: `src-tauri/src/traits/evaluation_judge.rs`

**Step 1:** Define 4 new traits with minimal signatures.

**Step 2:** Add simple community implementations (no-op or basic).

**Step 3:** Tests for trait compilation.

**Step 4:** Commit: `feat(traits): add MOA, Topology, Scaler, Judge trait interfaces`

---

### Task 18: MOA Pipeline in engine crate

**Files:**
- Create: `crates/impforge-engine/src/moa.rs`
- Create: `crates/impforge-engine/src/moa_critique.rs`
- Create: `crates/impforge-engine/src/moa_aggregator.rs`
- Create: `crates/impforge-engine/src/moa_early_stop.rs`
- Modify: `crates/impforge-engine/src/lib.rs`

**Step 1:** Implement 5-phase MOA pipeline (port from Python):
- Propose: parallel model calls via tokio::spawn
- Critique: cross-critique along DyTopo edges
- Refine: AKV synthesis (integrate critiques)
- Check: embedding cosine convergence detection
- Aggregate: residual skip connections

**Step 2:** Write tests for each phase.

**Step 3:** Commit: `feat(engine): add MOA Pipeline with 5-phase orchestration`

---

### Task 19: DyTopo in engine crate

**Files:**
- Create: `crates/impforge-engine/src/topology.rs`
- Create: `crates/impforge-engine/src/topology_embeddings.rs`
- Modify: `crates/impforge-engine/Cargo.toml` (add ndarray)
- Modify: `crates/impforge-engine/src/lib.rs`

**Step 1:** Implement 5-phase topology cycle:
- Collect descriptors (query + key per agent)
- Embed via local model (fastembed or Ollama)
- Compute cosine similarity matrix (ndarray)
- Threshold τ=0.35 to build DAG
- Topological sort (Kahn's algorithm)

**Step 2:** Tests for DAG construction + sort.

**Step 3:** Commit: `feat(engine): add DyTopo dynamic topology with DAG routing`

---

### Task 20: Agent Scaling in engine crate

**Files:**
- Create: `crates/impforge-engine/src/scaling.rs`
- Create: `crates/impforge-engine/src/scaling_monitor.rs`
- Modify: `crates/impforge-engine/src/lib.rs`

**Step 1:** Implement ModelManager with LRU slots:
- GPU slots (configurable, default 3)
- CPU slots (configurable, default 3)
- Resident vs dynamic models
- LRU eviction policy

**Step 2:** Implement ResourceMonitor with 5 emergency levels.

**Step 3:** Tests for slot management + eviction.

**Step 4:** Commit: `feat(engine): add agent scaling with LRU model management`

---

### Task 21: Routing, Scoring, Benchmarks in engine crate

**Files:**
- Create: `crates/impforge-engine/src/routing.rs`
- Create: `crates/impforge-engine/src/scoring.rs`
- Create: `crates/impforge-engine/src/benchmarks.rs`
- Modify: `crates/impforge-engine/src/lib.rs`

**Step 1:** Implement ComplexityScorer (multi-signal heuristic).

**Step 2:** Implement CategoryRouter (25 categories).

**Step 3:** Implement BenchmarkHarness (prompts + quality scoring).

**Step 4:** Tests for scoring accuracy, routing, benchmarks.

**Step 5:** Commit: `feat(engine): add routing, scoring, and benchmark harness`

---

### Task 22: Consumer group simulation in engine

**Files:**
- Create: `crates/impforge-engine/src/message_bus_pro.rs`
- Modify: `crates/impforge-engine/src/lib.rs`

**Step 1:** Implement ConsumerGroup with pending/ack:
```rust
pub struct ConsumerGroup {
    name: String,
    rx: broadcast::Receiver<Message>,
    pending: HashMap<String, Message>,
}
```

**Step 2:** Tests for claim/ack/reclaim.

**Step 3:** Commit: `feat(engine): add consumer group message bus`

---

## Phase 5: Social Media + CI/CD

### Task 23: Social media base module

**Files:**
- Create: `src-tauri/src/orchestrator/social_media/mod.rs`
- Create: `src-tauri/src/orchestrator/social_media/base.rs`
- Create: `src-tauri/src/orchestrator/social_media/profile.rs`

**Step 1:** Implement base utilities:
- Rate limiter (5 actions/hour per platform)
- Credential manager (read from config file)
- Content generator (call Ollama for text)

**Step 2:** Embed professional profile as Rust const structs.

**Step 3:** Tests for rate limiting and profile data.

**Step 4:** Commit: `feat(social): add social media base module`

---

### Task 24: GitHubPromoter worker (octocrab)

**Files:**
- Create: `src-tauri/src/orchestrator/social_media/github.rs`
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Implement GitHub promoter via octocrab:
- Update repo description, topics, homepage
- Update user bio
- Pin repositories
- Create/update README
- Create releases

**Step 2:** Tests with mock octocrab responses.

**Step 3:** Commit: `feat(social): add GitHubPromoter worker`

---

### Task 25: LinkedInManager + FiverrManager + UpworkManager workers

**Files:**
- Create: `src-tauri/src/orchestrator/social_media/linkedin.rs`
- Create: `src-tauri/src/orchestrator/social_media/fiverr.rs`
- Create: `src-tauri/src/orchestrator/social_media/upwork.rs`
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** Add chromiumoxide_stealth and wreq to Cargo.toml.

**Step 2:** Implement each browser-automation worker:
- LinkedIn: Auto-post, pre-engagement, analytics
- Fiverr: Profile management, gig creation
- Upwork: Profile, job scanning, proposal drafts

**Step 3:** Tests for page navigation patterns.

**Step 4:** Commit: `feat(social): add LinkedIn, Fiverr, Upwork workers`

---

### Task 26: HackerNewsPublisher + NexusReadmeManager workers

**Files:**
- Create: `src-tauri/src/orchestrator/social_media/hackernews.rs`
- Create: `src-tauri/src/orchestrator/social_media/readme.rs`
- Modify: `src-tauri/src/orchestrator/workers/mod.rs`

**Step 1:** HackerNews: HTTP-only via reqwest (submit, comment).

**Step 2:** NexusReadme: octocrab + git2 for README management.

**Step 3:** Tests.

**Step 4:** Commit: `feat(social): add HackerNews and NexusReadme workers`

---

### Task 27: Multi-distro CI/CD pipeline

**Files:**
- Modify: `.github/workflows/test-build.yml`
- Modify: `.github/workflows/release.yml`

**Step 1:** Update test-build.yml matrix:
- ubuntu-22.04 (deb)
- ubuntu-22.04 + --features nvidia
- Arch Linux (via container)

**Step 2:** Update release.yml:
- deb bundle (Ubuntu/Debian/PopOS)
- rpm bundle (Fedora/RHEL)
- AppImage (universal)
- Windows NSIS
- macOS dmg (aarch64 + x86_64)
- NVIDIA variant builds

**Step 3:** Create AUR PKGBUILD template.

**Step 4:** Test CI matrix locally with `act`.

**Step 5:** Commit: `feat(ci): add multi-distro multi-GPU release pipeline`

---

### Task 28: Final integration test + cleanup

**Files:**
- All orchestrator files

**Step 1:** Run full test suite: `cargo test --workspace`

**Step 2:** Run with engine: `cargo test --workspace --features engine`

**Step 3:** Fix any failures.

**Step 4:** Update worker count in README and docs.

**Step 5:** Final commit: `feat(orchestrator): complete NeuralSwarm worker migration — 61 workers`
