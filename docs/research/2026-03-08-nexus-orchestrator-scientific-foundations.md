# Nexus AI Orchestrator - Scientific Foundations Research

**Date**: 2026-03-08
**Purpose**: Key formulas and parameters for Rust implementation of the Nexus standalone AI Orchestrator
**Scope**: FSRS, Hebbian/STDP Trust, MAPE-K Self-Healing, CLS Replay, A-MEM Zettelkasten

---

## 1. FSRS (Free Spaced Repetition Scheduler)

### References
- Jarrett Ye et al., "A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition Scheduling" (open-spaced-repetition)
- Official Rust crate: `fsrs` on crates.io (also `rs-fsrs`)
- FSRS-5: 19 parameters | FSRS-6: 21 parameters

### Memory State Model (DSR)

Each item has two state variables:
- **S** (Stability): Storage strength in days. Higher S = slower forgetting.
- **D** (Difficulty): Intrinsic difficulty of the item, range [1.0, 10.0].
- **R** (Retrievability): Probability of successful recall, derived from S and elapsed time.

### 1.1 Forgetting Curve (Power-Law Decay)

```
R(t, S) = (1 + FACTOR * t / S) ^ DECAY
```

**Constants (FSRS-4.5 and later):**
```rust
const DECAY: f64 = -0.5;
const FACTOR: f64 = 19.0 / 81.0;  // 0.234568...
// Derived so that R(S, S) = 0.9 (90% retention when t = S)
// Proof: FACTOR = 0.9^(1/DECAY) - 1 = 0.9^(-2) - 1 = 100/81 - 1 = 19/81
```

**FSRS-6 generalized version (21 params):**
```
R(t, S) = (1 + factor * t / S) ^ (-w[20])
where factor = 0.9^(-1/w[20]) - 1
```
w[20] defaults to ~0.5, recovering the FSRS-4.5 formula.

**Rust implementation:**
```rust
fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    const DECAY: f64 = -0.5;
    const FACTOR: f64 = 19.0 / 81.0;
    (1.0 + FACTOR * elapsed_days / stability).powf(DECAY)
}
```

### 1.2 Optimal Interval Calculation

Invert the forgetting curve to find when R drops to desired_retention:
```
I(r, S) = (S / FACTOR) * (r^(1/DECAY) - 1)
```

**Rust:**
```rust
fn next_interval(stability: f64, desired_retention: f64) -> f64 {
    const DECAY: f64 = -0.5;
    const FACTOR: f64 = 19.0 / 81.0;
    (stability / FACTOR) * (desired_retention.powf(1.0 / DECAY) - 1.0)
}
// When desired_retention = 0.9: interval = S (by construction)
```

### 1.3 Initial Stability (First Review)

```
S_0(G) = w[G - 1]
```
where G in {1=Again, 2=Hard, 3=Good, 4=Easy}

**FSRS-5 defaults:**
```rust
const W: [f64; 19] = [
    0.40255,   // w[0]  - S_0 for Again
    1.18385,   // w[1]  - S_0 for Hard
    3.173,     // w[2]  - S_0 for Good
    15.69105,  // w[3]  - S_0 for Easy
    7.1949,    // w[4]  - D_0 base
    0.5345,    // w[5]  - D_0 grade factor
    1.4604,    // w[6]  - D update / S_r factor
    0.0046,    // w[7]  - D mean reversion weight (FSRS-5: used in S_r)
    // ... see full table below
];
```

### 1.4 Initial Difficulty

```
D_0(G) = w[4] - e^(w[5] * (G - 1)) + 1
```
Clamped to [1.0, 10.0].

**Rust:**
```rust
fn initial_difficulty(grade: u8, w: &[f64; 19]) -> f64 {
    let d = w[4] - (w[5] * (grade as f64 - 1.0)).exp() + 1.0;
    d.clamp(1.0, 10.0)
}
```

### 1.5 Difficulty Update

```
delta_D = -w[6] * (G - 3)
D' = D + delta_D * (10 - D) / 9       // Linear damping toward boundaries
D'' = w[7] * D_0(4) + (1 - w[7]) * D' // Mean reversion toward Easy baseline
```
Clamped to [1.0, 10.0].

### 1.6 Stability After Successful Recall

**FSRS-5 formula (19 params):**
```
S'_r(D, S, R, G) = S * (1 + e^(w[8]) * (11 - D)^(w[9]) * S^(-w[10]) * (e^(w[11]*(1-R)) - 1) * hard_or_easy_bonus)
```

Where:
- `hard_or_easy_bonus` = w[15] if G=2(Hard), w[16] if G=4(Easy), 1.0 otherwise
- w[8] = 1.54575 (exponential scaling)
- w[9] = 0.1192 (difficulty exponent)
- w[10] = 0.1192 (stability decay exponent - note: different formula versions use different indices)
- w[11] = 1.9395 (retrievability scaling)

**Rust (simplified FSRS-4.5 version with clearer indices):**
```rust
fn stability_after_success(
    d: f64, s: f64, r: f64, grade: u8, w: &[f64]
) -> f64 {
    let hard_bonus = if grade == 2 { w[15] } else { 1.0 };
    let easy_bonus = if grade == 4 { w[16] } else { 1.0 };

    s * (w[8].exp()
        * (11.0 - d).powf(w[9])
        * s.powf(-w[10])
        * ((w[11] * (1.0 - r)).exp() - 1.0)
        * hard_bonus
        * easy_bonus
        + 1.0)
}
```

### 1.7 Stability After Lapse (Forgetting)

```
S'_f(D, S, R) = w[11] * D^(-w[12]) * ((S + 1)^w[13] - 1) * e^(w[14] * (1 - R))
```
Result is clamped: `min(S'_f, S)` (post-lapse stability cannot exceed pre-lapse).

**Rust:**
```rust
fn stability_after_lapse(d: f64, s: f64, r: f64, w: &[f64]) -> f64 {
    let s_f = w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0)
              * (w[14] * (1.0 - r)).exp();
    s_f.min(s)
}
```

### 1.8 Same-Day Review Stability (Short-term, FSRS-5)

```
S'(S, G) = S * e^(w[17] * (G - 3 + w[18]))
```

### 1.9 Complete FSRS-5 Default Parameters

```rust
const FSRS5_DEFAULTS: [f64; 19] = [
    0.40255,   // w[0]  S_0(Again)
    1.18385,   // w[1]  S_0(Hard)
    3.17300,   // w[2]  S_0(Good)
    15.69105,  // w[3]  S_0(Easy)
    7.19490,   // w[4]  D_0 base
    0.53450,   // w[5]  D_0 grade factor
    1.46040,   // w[6]  D update / delta_D scale
    0.00460,   // w[7]  D mean reversion weight
    1.54575,   // w[8]  S_r: exp scaling
    0.11920,   // w[9]  S_r: difficulty exponent
    1.01925,   // w[10] S_r: stability exponent
    1.93950,   // w[11] S_f/S_r: retrievability/lapse scale
    0.11000,   // w[12] S_f: difficulty exponent
    0.29605,   // w[13] S_f: stability exponent
    2.26980,   // w[14] S_f: retrievability exponent
    0.23150,   // w[15] Hard bonus multiplier
    2.98980,   // w[16] Easy bonus multiplier
    0.51655,   // w[17] Same-day: grade factor
    0.66210,   // w[18] Same-day: offset
];
```

### 1.10 FSRS-6 Default Parameters (21 params, latest)

```rust
const FSRS6_DEFAULTS: [f64; 21] = [
    0.2120, 1.2931, 2.3065, 8.2956,   // w[0-3]   S_0 per grade
    6.4133, 0.8334, 3.0194, 0.0010,   // w[4-7]   Difficulty
    1.8722, 0.1666, 0.7960, 1.4835,   // w[8-11]  Stability recall
    0.0614, 0.2629, 1.6483, 0.6014,   // w[12-15] Stability lapse + Hard
    1.8729, 0.5425, 0.0912, 0.0658,   // w[16-19] Easy + Same-day
    0.1542,                            // w[20]    Decay exponent
];
```

---

## 2. Hebbian/STDP Trust Scoring

### References
- Bi & Poo (1998), "Synaptic Modifications in Cultured Hippocampal Neurons", J. Neurosci. 18:10464-10472
- Song, Miller & Abbott (2000), "Competitive Hebbian Learning through STDP", Nature Neurosci. 3:919-926
- Gerstner & Sjoestroem (2010), Scholarpedia article on STDP

### 2.1 Classic STDP Formula

The weight change depends on the time difference between pre- and post-synaptic spikes:

```
delta_t = t_post - t_pre

if delta_t > 0 (causal: pre before post → potentiation/LTP):
    delta_w = A+ * exp(-delta_t / tau+)

if delta_t < 0 (anti-causal: post before pre → depression/LTD):
    delta_w = -A- * exp(delta_t / tau-)
```

### 2.2 Standard Parameters (Song, Miller & Abbott 2000)

```rust
// Time constants (biological: milliseconds, adapted: arbitrary time units)
const TAU_PLUS: f64 = 20.0;   // ms - potentiation window
const TAU_MINUS: f64 = 20.0;  // ms - depression window

// Amplitude (maximum weight change at delta_t ≈ 0)
const A_PLUS: f64 = 0.01;     // LTP amplitude
const A_MINUS: f64 = 0.0105;  // LTD amplitude (slightly larger → stability)

// Weight bounds
const W_MAX: f64 = 1.0;       // Maximum synaptic weight
const W_MIN: f64 = 0.0;       // Minimum synaptic weight

// Key ratio: A- / A+ = 1.05 (depression slightly stronger than potentiation)
// This ratio ensures competitive dynamics and prevents runaway excitation.
```

### 2.3 Adaptation for Task Worker Trust Scoring

Map biological STDP to AI orchestrator trust management:

| Biological | Trust Analogy |
|------------|---------------|
| Pre-synaptic spike | Task assignment to worker |
| Post-synaptic spike | Task completion event |
| delta_t > 0 (LTP) | Success → strengthen trust |
| delta_t < 0 (LTD) | Failure → weaken trust |
| Synaptic weight | Trust score [0.0, 1.0] |
| tau+ / tau- | Recency decay (configurable half-life) |

**Adapted Trust STDP Formula:**

```rust
/// Trust update after a task result
fn update_trust(
    current_trust: f64,
    success: bool,
    time_since_last_event: f64, // in hours or task-counts
    config: &TrustConfig,
) -> f64 {
    let delta = if success {
        // Potentiation: success strengthens trust
        config.a_plus * (-time_since_last_event / config.tau_plus).exp()
    } else {
        // Depression: failure weakens trust
        -config.a_minus * (-time_since_last_event / config.tau_minus).exp()
    };

    (current_trust + delta).clamp(0.0, 1.0)
}

struct TrustConfig {
    a_plus: f64,      // Max trust increase (default: 0.10)
    a_minus: f64,     // Max trust decrease (default: 0.15)
    tau_plus: f64,     // Potentiation decay window (default: 24.0 hours)
    tau_minus: f64,    // Depression decay window (default: 12.0 hours)
    decay_rate: f64,   // Passive decay toward baseline (default: 0.001/hour)
    baseline: f64,     // Neutral trust level (default: 0.5)
}
```

### 2.4 Exponential Decay with Configurable Half-Life

Trust decays passively over time toward a baseline:

```
T(t) = baseline + (T_0 - baseline) * e^(-lambda * t)

where lambda = ln(2) / half_life
```

**Rust:**
```rust
fn decay_trust(
    trust: f64,
    elapsed_hours: f64,
    half_life_hours: f64,
    baseline: f64,
) -> f64 {
    let lambda = (2.0_f64).ln() / half_life_hours;
    baseline + (trust - baseline) * (-lambda * elapsed_hours).exp()
}

// Example: half_life = 168 hours (1 week)
// After 1 week: trust is halfway back to baseline
// After 2 weeks: trust is 75% back to baseline
```

### 2.5 Trace-Based Trust (Eligibility Traces)

Following Brian2/Song implementation, maintain running traces:

```rust
struct WorkerTrust {
    trust: f64,          // Current trust score [0, 1]
    success_trace: f64,  // Exponentially decaying success trace
    failure_trace: f64,  // Exponentially decaying failure trace
    last_update: f64,    // Timestamp of last event
}

impl WorkerTrust {
    fn on_task_complete(&mut self, success: bool, now: f64, cfg: &TrustConfig) {
        let dt = now - self.last_update;

        // Decay traces
        self.success_trace *= (-dt / cfg.tau_plus).exp();
        self.failure_trace *= (-dt / cfg.tau_minus).exp();

        if success {
            self.success_trace += cfg.a_plus;
            // Apply accumulated failure trace as negative update
            self.trust += self.success_trace;
            self.trust -= self.failure_trace * 0.5; // Cross-trace interaction
        } else {
            self.failure_trace += cfg.a_minus;
            self.trust -= self.failure_trace;
            self.trust += self.success_trace * 0.5;
        }

        self.trust = self.trust.clamp(0.0, 1.0);
        self.last_update = now;
    }
}
```

### 2.6 Recommended Default Parameters for Nexus

```rust
const NEXUS_TRUST_DEFAULTS: TrustConfig = TrustConfig {
    a_plus: 0.10,          // 10% max trust gain per success
    a_minus: 0.15,         // 15% max trust loss per failure (asymmetric)
    tau_plus: 24.0,        // Success recency: 24 hours
    tau_minus: 12.0,       // Failure recency: 12 hours (failures forgotten slower)
    decay_rate: 0.001,     // Passive decay per hour
    baseline: 0.5,         // Neutral trust
    half_life: 168.0,      // 1 week passive decay half-life
};
// Key: A-/A+ ratio = 1.5 (stronger depression than potentiation)
// Biological ratio: 1.05 (Song et al.)
// Higher ratio for AI = more cautious trust management
```

---

## 3. MAPE-K Self-Healing Loop

### References
- Kephart & Chess (2003), "The Vision of Autonomic Computing", IEEE Computer 36(1):41-50
- IBM Autonomic Computing Blueprint (2001)
- Arcaini et al. (2015), "Modeling and Analyzing MAPE-K Feedback Loops for Self-adaptation"
- Malburg et al. (2023), "Applying MAPE-K control loops for adaptive workflow management"

### 3.1 Architecture Overview

```
                    ┌──────────────────────────────────┐
                    │         KNOWLEDGE BASE           │
                    │  - Topology/Config               │
                    │  - Historical metrics            │
                    │  - Policies & Rules              │
                    │  - Symptom catalog               │
                    │  - Adaptation plans              │
                    └───────┬────┬────┬────┬───────────┘
                            │    │    │    │
              ┌─────────────┤    │    │    ├─────────────┐
              │             │    │    │    │             │
              ▼             ▼    │    ▼    ▼             │
        ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
        │ MONITOR  │→ │ ANALYZE  │→ │   PLAN   │→ │ EXECUTE  │
        │          │  │          │  │          │  │          │
        │ Sensors  │  │ Symptoms │  │ Actions  │  │ Effectors│
        │ Probes   │  │ Patterns │  │ Strategy │  │ Actuators│
        │ Metrics  │  │ Rules    │  │ Schedule │  │ Rollback │
        └──────────┘  └──────────┘  └──────────┘  └──────────┘
              ▲                                         │
              │         MANAGED ELEMENT                 │
              │    (Services, Workers, Models)          │
              └─────────────────────────────────────────┘
```

### 3.2 Phase Details for Desktop App Implementation

#### Monitor Phase
```rust
struct MonitorPhase {
    /// Collect metrics at regular intervals
    probe_interval: Duration,        // Default: 5 seconds
    /// Metrics to track per managed element
    metrics: Vec<MetricType>,
    /// Sliding window for recent history
    window_size: usize,              // Default: 60 samples
}

enum MetricType {
    Heartbeat,           // Is service alive?
    ResponseTime,        // Latency in ms
    ErrorRate,           // Errors per minute
    ResourceUsage,       // CPU/Memory/GPU %
    QueueDepth,          // Pending tasks
    ThroughputRate,      // Tasks per minute
}

// Health check pattern
struct HealthProbe {
    target: ServiceId,
    endpoint: String,          // e.g., "http://localhost:PORT/health"
    timeout: Duration,         // Default: 3 seconds
    consecutive_failures: u32, // Before marking unhealthy
    max_failures: u32,         // Default: 3 (circuit breaker threshold)
}
```

#### Analyze Phase
```rust
enum Symptom {
    ServiceDown { service: ServiceId, since: Instant },
    HighLatency { service: ServiceId, p99_ms: f64, threshold_ms: f64 },
    ErrorSpike { service: ServiceId, rate: f64, baseline: f64 },
    ResourceExhaustion { resource: ResourceType, usage_pct: f64 },
    QueueBacklog { queue: String, depth: usize, max: usize },
    ModelDegradation { model: String, quality_score: f64 },
}

// Analysis rules (threshold-based)
struct AnalysisRule {
    condition: Box<dyn Fn(&[MetricSample]) -> Option<Symptom>>,
    severity: Severity,      // Info, Warning, Critical
    cooldown: Duration,      // Min time between alerts (default: 60s)
}

// Statistical analysis
fn detect_anomaly(samples: &[f64], z_threshold: f64) -> bool {
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                   / samples.len() as f64;
    let std_dev = variance.sqrt();
    let latest = samples.last().unwrap();
    ((latest - mean) / std_dev).abs() > z_threshold // default z = 3.0
}
```

#### Plan Phase
```rust
enum HealingAction {
    RestartService { service: ServiceId },
    ScaleResource { service: ServiceId, factor: f64 },
    FailoverToBackup { primary: ServiceId, backup: ServiceId },
    ReduceLoad { service: ServiceId, max_concurrent: usize },
    SwitchModel { from: String, to: String },      // Model fallback
    PurgeQueue { queue: String, older_than: Duration },
    ReconfigureParams { service: ServiceId, params: HashMap<String, String> },
    AlertUser { message: String, severity: Severity },
}

struct HealingPlan {
    symptom: Symptom,
    actions: Vec<HealingAction>,
    rollback: Vec<HealingAction>,    // Undo if plan fails
    max_retries: u32,                // Default: 3
    timeout: Duration,               // Max time for plan execution
    requires_approval: bool,         // Some actions need user confirmation
}
```

#### Execute Phase
```rust
struct ExecutePhase {
    /// Execute actions with rollback capability
    async fn execute_plan(&self, plan: &HealingPlan) -> Result<(), HealError> {
        let mut completed_actions = Vec::new();

        for action in &plan.actions {
            match self.execute_action(action).await {
                Ok(()) => completed_actions.push(action),
                Err(e) => {
                    // Rollback completed actions in reverse order
                    for completed in completed_actions.iter().rev() {
                        self.rollback_action(completed).await.ok();
                    }
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}
```

#### Knowledge Base
```rust
struct KnowledgeBase {
    topology: HashMap<ServiceId, ServiceConfig>,
    metrics_history: RingBuffer<MetricSample>,    // Last N hours
    symptom_log: Vec<(Instant, Symptom)>,
    policies: Vec<AnalysisRule>,
    action_history: Vec<(Instant, HealingPlan, bool)>,  // (when, what, success)

    // Learning: track which plans worked
    plan_success_rate: HashMap<String, (u32, u32)>,  // (successes, total)
}
```

### 3.3 Self-* Properties Implementation

| Property | Implementation |
|----------|---------------|
| **Self-Configuration** | Auto-detect available Ollama models, GPU capabilities, available ports |
| **Self-Healing** | Restart crashed services, failover to backup models, circuit breaker |
| **Self-Optimization** | Adjust batch sizes based on latency, tune model parameters |
| **Self-Protection** | Rate limiting, resource caps, queue depth limits |

### 3.4 Loop Timing Parameters

```rust
struct MapeKConfig {
    monitor_interval: Duration,     // 5 seconds
    analyze_interval: Duration,     // 10 seconds (every 2nd monitor cycle)
    plan_cooldown: Duration,        // 30 seconds (prevent plan storms)
    execute_timeout: Duration,      // 60 seconds per action
    knowledge_retention: Duration,  // 24 hours of history
    circuit_breaker_threshold: u32, // 5 consecutive failures
    circuit_breaker_reset: Duration,// 60 seconds before retry
}
```

---

## 4. CLS (Complementary Learning Systems) Theory

### References
- McClelland, McNaughton & O'Reilly (1995), "Why there are complementary learning systems in the hippocampus and neocortex", Psychological Review 102(3):419-457
- O'Reilly (2014), "Complementary Learning Systems", Cognitive Science
- Kumaran, Hassabis & McClelland (2016), "What Learning Systems do Intelligent Agents Need?"

### 4.1 Core Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   HIPPOCAMPUS                           │
│  (Fast Learning, Episodic, Sparse, Pattern-Separated)  │
│                                                         │
│  Properties:                                            │
│  - High learning rate (alpha_h = 0.1 - 0.5)           │
│  - One-shot learning capability                         │
│  - Sparse activation (few neurons per memory)           │
│  - Pattern separation (orthogonal representations)      │
│  - Quick binding of arbitrary associations              │
│  - Capacity-limited (catastrophic interference)         │
└──────────────────────┬──────────────────────────────────┘
                       │ REPLAY (Sleep/Offline)
                       │ Interleaved, gradual transfer
                       ▼
┌─────────────────────────────────────────────────────────┐
│                    NEOCORTEX                            │
│  (Slow Learning, Semantic, Distributed, Overlapping)    │
│                                                         │
│  Properties:                                            │
│  - Low learning rate (alpha_c = 0.001 - 0.01)          │
│  - Gradual extraction of statistical structure           │
│  - Distributed representations (many neurons per memory)│
│  - Pattern completion (fill in missing information)     │
│  - Interleaved learning prevents catastrophic forgetting│
│  - Long-term retention, schema formation                │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Key Parameters

```rust
struct ClsConfig {
    // Hippocampal system (fast store)
    hippo_learning_rate: f64,        // 0.1 - 0.5 (fast)
    hippo_capacity: usize,           // Max episodic memories before overflow
    hippo_sparsity: f64,             // 0.05 - 0.10 (5-10% activation)
    hippo_decay_rate: f64,           // Forgetting rate for unconsolidated memories

    // Neocortical system (slow store)
    neo_learning_rate: f64,          // 0.001 - 0.01 (slow)
    neo_consolidation_threshold: f64, // Min importance to consolidate (0.3)
    neo_interleave_ratio: f64,       // Mix of new vs old during replay (0.3 new)

    // Replay parameters
    replay_batch_size: usize,        // Memories replayed per cycle (default: 10)
    replay_interval: Duration,       // Time between consolidation cycles
    replay_priority_decay: f64,      // How quickly replay priority decreases

    // Learning rate ratio (CRITICAL)
    // McClelland et al.: neocortex learns ~10-100x slower than hippocampus
    // Typical: alpha_c / alpha_h = 0.01 to 0.1
}
```

### 4.3 Replay Consolidation Algorithm

```rust
/// CLS-inspired memory consolidation
struct MemoryConsolidator {
    hippocampus: Vec<EpisodicMemory>,    // Fast store (recent, detailed)
    neocortex: Vec<SemanticMemory>,      // Slow store (consolidated, structured)
}

struct EpisodicMemory {
    content: String,
    embedding: Vec<f32>,
    timestamp: Instant,
    access_count: u32,
    importance: f64,         // [0, 1]
    consolidated: bool,      // Has been replayed to neocortex?
    replay_count: u32,       // Times replayed
}

struct SemanticMemory {
    concept: String,
    embedding: Vec<f32>,
    strength: f64,           // Accumulated learning strength
    source_episodes: Vec<u64>, // Which episodes contributed
    last_reinforced: Instant,
    tags: Vec<String>,
    links: Vec<u64>,         // Cross-references to other semantic memories
}

impl MemoryConsolidator {
    /// Replay consolidation cycle (run periodically, analogous to sleep)
    fn consolidate(&mut self, cfg: &ClsConfig) {
        // 1. Select memories for replay (priority queue)
        let mut candidates: Vec<&EpisodicMemory> = self.hippocampus
            .iter()
            .filter(|m| !m.consolidated || m.importance > cfg.neo_consolidation_threshold)
            .collect();

        // Sort by replay priority: importance * recency * novelty
        candidates.sort_by(|a, b| {
            let score_a = a.importance * replay_priority(a, cfg);
            let score_b = b.importance * replay_priority(b, cfg);
            score_b.partial_cmp(&score_a).unwrap()
        });

        // 2. Take top-k for this replay batch
        let batch = &candidates[..cfg.replay_batch_size.min(candidates.len())];

        // 3. Interleaved replay: mix new and already-consolidated
        for episode in batch {
            // Find or create matching semantic memory
            let semantic = self.find_or_create_semantic(episode);

            // Gradual weight update (slow learning rate)
            semantic.strength += cfg.neo_learning_rate * episode.importance;
            semantic.embedding = weighted_average(
                &semantic.embedding,
                &episode.embedding,
                1.0 - cfg.neo_learning_rate,  // Keep most of old
                cfg.neo_learning_rate,         // Add small amount of new
            );
        }

        // 4. Prune hippocampus (forget old, consolidated memories)
        self.hippocampus.retain(|m| {
            let age = m.timestamp.elapsed();
            !m.consolidated || age < cfg.hippo_max_age || m.importance > 0.8
        });
    }
}

fn replay_priority(memory: &EpisodicMemory, cfg: &ClsConfig) -> f64 {
    let recency = (-memory.timestamp.elapsed().as_secs_f64()
                   * cfg.replay_priority_decay).exp();
    let novelty = 1.0 / (1.0 + memory.replay_count as f64);
    recency * novelty
}
```

### 4.4 Key Insight for AI Implementation

The critical CLS principle: **new information must be learned slowly and interleaved with old information** to prevent catastrophic forgetting. In practice:

1. **Fast path** (hippocampus): Immediately store task results, chat interactions, errors
2. **Slow path** (neocortex): Periodically consolidate into structured knowledge
3. **Replay**: During idle periods, replay recent episodic memories to update semantic store
4. **Interleaving**: When replaying, mix 30% new memories with 70% old ones

---

## 5. A-MEM Zettelkasten for AI Knowledge Management

### References
- Xu, Liang et al. (2025), "A-MEM: Agentic Memory for LLM Agents", arXiv:2502.12110 (NeurIPS 2025)
- GitHub: agiresearch/A-mem, WujiangXu/A-mem
- Zettelkasten method (Niklas Luhmann)

### 5.1 Note Structure

Each memory note has 7 attributes:

```rust
struct ZettelNote {
    id: u64,                      // Unique identifier
    content: String,              // c_i: Original content
    timestamp: Instant,           // t_i: Creation time
    keywords: Vec<String>,        // K_i: Key concepts extracted via LLM
    tags: Vec<String>,            // G_i: Categorical labels
    context: String,              // X_i: Rich semantic description
    embedding: Vec<f32>,          // e_i: Dense vector (e.g., 1024-dim)
    links: HashSet<u64>,          // L_i: Connected note references

    // Extension fields for Nexus
    importance: f64,              // [0, 1] for prioritization
    access_count: u32,            // For usage-based ranking
    last_accessed: Instant,       // Recency tracking
    source: NoteSource,           // Where this memory came from
    version: u32,                 // Evolution counter
}

enum NoteSource {
    TaskResult { task_id: u64, worker: String },
    UserInteraction { session_id: String },
    SystemObservation { component: String },
    Consolidation { source_notes: Vec<u64> },
}
```

### 5.2 Note Construction Process

When a new memory arrives:

```rust
impl ZettelGraph {
    fn add_memory(&mut self, raw_content: &str) -> u64 {
        // Step 1: Generate structured note
        let note = ZettelNote {
            id: self.next_id(),
            content: raw_content.to_string(),
            timestamp: Instant::now(),
            keywords: self.extract_keywords(raw_content),    // LLM or NLP extraction
            tags: self.generate_tags(raw_content),           // Categorical classification
            context: self.generate_context(raw_content),     // Semantic summary
            embedding: self.encode(raw_content),             // Vector embedding
            links: HashSet::new(),
            importance: self.estimate_importance(raw_content),
            access_count: 0,
            last_accessed: Instant::now(),
            source: NoteSource::UserInteraction { session_id: "...".into() },
            version: 1,
        };

        // Step 2: Generate links to existing notes
        let linked_ids = self.generate_links(&note);

        // Step 3: Evolve existing notes if needed
        self.evolve_related_notes(&note, &linked_ids);

        let id = note.id;
        self.notes.insert(id, note);
        id
    }
}
```

### 5.3 Link Generation (Two-Stage)

```rust
impl ZettelGraph {
    fn generate_links(&mut self, new_note: &ZettelNote) -> Vec<u64> {
        // Stage 1: Similarity retrieval (fast, embedding-based)
        let candidates = self.find_similar(
            &new_note.embedding,
            10,                          // top-k candidates
            0.5,                         // min cosine similarity threshold
        );

        // Stage 2: Semantic validation (slower, keyword/tag overlap)
        let validated_links: Vec<u64> = candidates
            .iter()
            .filter(|candidate| {
                let keyword_overlap = jaccard_similarity(
                    &new_note.keywords,
                    &candidate.keywords
                );
                let tag_overlap = jaccard_similarity(
                    &new_note.tags,
                    &candidate.tags
                );
                let cosine_sim = cosine_similarity(
                    &new_note.embedding,
                    &candidate.embedding
                );

                // Link if strong semantic connection
                // Threshold: combined score > 0.4
                (keyword_overlap * 0.3 + tag_overlap * 0.3 + cosine_sim * 0.4) > 0.4
            })
            .map(|c| c.id)
            .collect();

        // Create bidirectional links
        for &linked_id in &validated_links {
            self.notes.get_mut(&linked_id).unwrap().links.insert(new_note.id);
        }

        validated_links
    }
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let set_a: HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 { 0.0 } else { intersection / union }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let mag_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}
```

### 5.4 Memory Retrieval

```rust
impl ZettelGraph {
    fn retrieve(&self, query: &str, top_k: usize) -> Vec<&ZettelNote> {
        let query_embedding = self.encode(query);
        let query_keywords = self.extract_keywords(query);

        let mut scored: Vec<(f64, &ZettelNote)> = self.notes.values()
            .map(|note| {
                let embedding_sim = cosine_similarity(&query_embedding, &note.embedding);
                let keyword_sim = jaccard_similarity(
                    &query_keywords.iter().map(|s| s.as_str().to_string()).collect::<Vec<_>>(),
                    &note.keywords
                );
                let recency_boost = recency_score(note.last_accessed);
                let importance_boost = note.importance;

                let score = embedding_sim * 0.4
                          + keyword_sim * 0.3
                          + recency_boost * 0.15
                          + importance_boost * 0.15;
                (score, note)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.into_iter().take(top_k).map(|(_, note)| note).collect()
    }
}

fn recency_score(last_accessed: Instant) -> f64 {
    let hours = last_accessed.elapsed().as_secs_f64() / 3600.0;
    (-hours / 168.0).exp()  // Half-life of 1 week
}
```

### 5.5 Memory Evolution

When new notes arrive, related existing notes can evolve:

```rust
impl ZettelGraph {
    fn evolve_related_notes(&mut self, new_note: &ZettelNote, linked_ids: &[u64]) {
        for &id in linked_ids {
            if let Some(existing) = self.notes.get_mut(&id) {
                // Merge new keywords that are relevant
                for kw in &new_note.keywords {
                    if !existing.keywords.contains(kw) {
                        // Check if keyword is semantically related
                        if self.is_keyword_relevant(kw, &existing.content) {
                            existing.keywords.push(kw.clone());
                        }
                    }
                }

                // Update context description with new relationship info
                existing.context = format!(
                    "{} [Updated: connected to note #{} about {}]",
                    existing.context,
                    new_note.id,
                    new_note.tags.join(", ")
                );

                existing.version += 1;
                existing.last_accessed = Instant::now();
            }
        }
    }
}
```

### 5.6 Zettelkasten Principles for AI

| Principle | Implementation |
|-----------|---------------|
| **Atomicity** | Each note = one concept/event/result |
| **Unique ID** | Monotonic u64 identifiers |
| **Linking** | Bidirectional cosine + keyword validated links |
| **Tags** | Flat tag system, no rigid hierarchy |
| **Fleeting Notes** | Episodic memories (CLS hippocampus) |
| **Permanent Notes** | Consolidated semantic memories (CLS neocortex) |
| **Literature Notes** | External knowledge references |
| **Index Notes** | High-level topic aggregations |

---

## Integration: How These Systems Work Together in Nexus

```
┌─────────────────────────────────────────────────────────────┐
│                    NEXUS ORCHESTRATOR                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────┐    ┌───────────┐    ┌──────────────────────┐  │
│  │  MAPE-K │───→│  Workers  │───→│  STDP Trust Scores   │  │
│  │  Loop   │    │  (Ollama  │    │  (per-worker trust    │  │
│  │         │    │  Models)  │    │   updated on each     │  │
│  │ Monitor │    └───────────┘    │   task completion)    │  │
│  │ Analyze │                     └──────────┬───────────┘  │
│  │ Plan    │                                │              │
│  │ Execute │                                ▼              │
│  └────┬────┘                     ┌──────────────────────┐  │
│       │                          │  FSRS Scheduling     │  │
│       │                          │  (When to re-check   │  │
│       │                          │   workers, when to   │  │
│       │                          │   review knowledge)  │  │
│       │                          └──────────┬───────────┘  │
│       │                                     │              │
│       ▼                                     ▼              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              A-MEM ZETTELKASTEN                      │  │
│  │  (All knowledge stored as linked notes)              │  │
│  │                                                      │  │
│  │  ┌──────────────┐         ┌───────────────────────┐  │  │
│  │  │ Hippocampus  │ REPLAY  │    Neocortex          │  │  │
│  │  │ (Fast store) │────────→│    (Slow store)       │  │  │
│  │  │ Recent tasks │  CLS    │    Consolidated       │  │  │
│  │  │ Raw events   │ Theory  │    knowledge          │  │  │
│  │  └──────────────┘         └───────────────────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Integration Flow

1. **MAPE-K** monitors all services (Ollama, workers, queues)
2. **STDP Trust** scores workers based on task success/failure history
3. **FSRS** schedules when to re-evaluate trust, when to consolidate memories
4. **CLS Replay** consolidates episodic task results into semantic knowledge
5. **A-MEM Zettelkasten** stores everything as linked, evolving notes

### Key Constants Summary

```rust
// FSRS
const FSRS_DECAY: f64 = -0.5;
const FSRS_FACTOR: f64 = 19.0 / 81.0;  // 0.234568

// STDP Trust
const TRUST_A_PLUS: f64 = 0.10;
const TRUST_A_MINUS: f64 = 0.15;
const TRUST_TAU_PLUS: f64 = 24.0;      // hours
const TRUST_TAU_MINUS: f64 = 12.0;     // hours
const TRUST_HALF_LIFE: f64 = 168.0;    // hours (1 week)

// CLS
const HIPPO_LEARNING_RATE: f64 = 0.3;
const NEO_LEARNING_RATE: f64 = 0.005;
const REPLAY_BATCH_SIZE: usize = 10;
const INTERLEAVE_RATIO: f64 = 0.3;     // 30% new, 70% old

// MAPE-K
const MONITOR_INTERVAL_SECS: u64 = 5;
const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;
const CIRCUIT_BREAKER_RESET_SECS: u64 = 60;

// A-MEM Zettelkasten
const LINK_SIMILARITY_THRESHOLD: f64 = 0.4;
const RETRIEVAL_TOP_K: usize = 10;
const RECENCY_HALF_LIFE_HOURS: f64 = 168.0;
```
