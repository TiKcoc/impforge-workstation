<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="ImpForge Logo" />
</p>

<h1 align="center">ImpForge AI Workstation</h1>

<p align="center">
  <strong>Your complete AI stack. One desktop app.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> |
  <a href="#pricing">Pricing</a> |
  <a href="#download">Download</a> |
  <a href="#tech-stack">Tech Stack</a>
</p>

---

## What is ImpForge?

ImpForge is a native desktop application that puts an entire AI development stack at your fingertips. Chat with 28+ free AI models, manage Docker containers, orchestrate agents, automate workflows, and write code -- all from a single, beautiful Opera GX-inspired interface.

No cloud dependencies required. ImpForge works fully offline with local models, or connects to cloud providers when you want more power.

## Features

### Intelligent Model Router
Automatically routes your prompts to the best available model based on task type. Code questions go to coding models, creative tasks to creative models -- all without manual switching. **Zero API cost by default** using free models.

### Multi-Model Chat (TerminalUI)
- **Ollama Local Inference** — stream from 100+ local models, auto-detected on startup
- **OpenRouter Cloud** — 28+ free models (Llama 4, Devstral, Qwen3, and more)
- **Smart Routing** — automatically selects Ollama (offline) or OpenRouter (cloud) based on availability
- **Chat/TerminalUI** — IDE-like 3-panel layout: Explorer | Editor+Terminal | Chat
- SSE/NDJSON streaming with real-time token display
- ForgeMemory context enrichment for every conversation
- Model selector with Local/Cloud badges

### NeuralSwarm Agent System
A pool of specialized AI agents with **full runtime management**:
- **Orchestrator** -- Decomposes complex tasks into subtasks
- **Code Agent** -- Generates, reviews, and refactors code
- **Research Agent** -- Searches docs, papers, and the web
- **Debug Agent** -- Traces errors and suggests fixes
- **DevOps Agent** -- Manages infrastructure and CI/CD
- **Review Agent** -- Audits code quality and security
- **Live Agent Dashboard** -- Start/stop agents, view logs, track task completion rates
- **Runtime State Tracking** -- Per-agent metrics: messages processed, tasks completed/failed
- **5-second polling** -- Real-time status updates in the UI

### Built-in Integrations
- **Docker Dashboard** -- Start, stop, inspect containers without leaving the app
- **GitHub Panel** -- Browse repos, issues, and pull requests
- **n8n Workflows** -- Embedded automation workflows
- **CodeForge IDE** -- Monaco-based editor with AI agent assistance
- **AI News** -- Curated feed of the latest AI developments

### System Monitoring
Live hardware metrics in the status bar -- CPU, RAM, GPU VRAM, temperature. Always know what your machine is doing.

## Pricing

| | Free | Core | Pro | Workstation | Lifetime |
|---|---|---|---|---|---|
| **Price** | **0** | **29/mo** | **59/mo** | **149/mo** | **299 once** |
| Free AI Models (28+) | Yes | Yes | Yes | Yes | Yes |
| Local Ollama Models | Yes | Yes | Yes | Yes | Yes |
| Docker Management | Yes | Yes | Yes | Yes | Yes |
| GitHub Integration | Yes | Yes | Yes | Yes | Yes |
| Intelligent Router | Basic | Full | Full | Full | Full |
| NeuralSwarm Agents | -- | 3 | 6 | Unlimited | Unlimited |
| CodeForge IDE | -- | Yes | Yes | Yes | Yes |
| Priority Support | -- | -- | Yes | Yes | Yes |
| Custom Agent Builder | -- | -- | -- | Yes | Yes |
| Enterprise API Access | -- | -- | -- | Yes | Yes |

## Download

Coming soon for **Linux**, **macOS**, and **Windows**.

Built with [Tauri 2](https://tauri.app) -- native performance, tiny bundle size, no Electron bloat.

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend | SvelteKit 5, Svelte 5 Runes, TypeScript |
| Backend | Tauri 2.10, Rust |
| Styling | Tailwind CSS v4, Opera GX dark theme |
| UI | bits-ui, PaneForge, Lucide icons |
| AI | OpenRouter API, Ollama (local), NDJSON/SSE streaming |
| Memory | ForgeMemory (SQLite + BM25 + HNSW vector search) |
| Data | tauri-plugin-store, local persistence |

## Screenshots

*Coming soon*

## NeuralSwarm Orchestrator (Standalone)

ImpForge includes its own **Rust-native AI orchestrator** — no external dependencies required. This is a complete reimplementation inspired by neuroscience research:

| Component | Scientific Basis | Purpose |
|---|---|---|
| **FSRS-5 Scheduler** | Jarrett Ye et al. — Power-law forgetting curve | Optimal memory review timing |
| **Hebbian/STDP Trust** | Bi & Poo 1998, Song/Miller/Abbott 2000 | Per-worker trust scoring |
| **MAPE-K Self-Healing** | Kephart & Chess 2003 (IBM) | Autonomous service recovery |
| **CLS Replay** | McClelland et al. 1995 | Fast→slow memory consolidation |
| **A-MEM Zettelkasten** | Xu et al., NeurIPS 2025 | Cross-referenced knowledge notes |

- 42 task workers (system monitoring, code quality, security, Git automation, and more)
- Trust-gated execution — unreliable workers are automatically throttled
- SQLite persistence (bundled, WAL mode) — no PostgreSQL or Redis needed
- Cross-platform: Linux, macOS, Windows

## Legal & Compliance

ImpForge is designed for commercial distribution in the EU and globally. Full legal research is maintained in [`docs/legal/`](docs/legal/).

### EU AI Act (Regulation 2024/1689)
- **Risk Classification**: Limited/minimal risk (not high-risk per Annex III)
- **Art. 50 Transparency**: Users are informed they interact with AI systems
- **Full applicability**: 2 August 2026

### GDPR / DSGVO (Regulation 2016/679)
- **Privacy by Design** (Art. 25): Local-first architecture — data stays on your device
- **Data Minimization** (Art. 5): Only processes data you explicitly provide
- **Right to Erasure** (Art. 17): One-click deletion of all AI data
- **Lawful Basis** (Art. 6): Contract for core features, consent for optional processing

### Open Source License Compliance
- All dependencies audited via `cargo-deny` (MIT, Apache-2.0, BSD permitted; GPL/AGPL denied)
- Third-party licenses bundled in `THIRD_PARTY_LICENSES`
- Model licenses displayed per-model before download (Llama Community License, Apache 2.0, etc.)

### German Software Distribution Law
- **Impressum** (DDG §5): Legal notice in app and website
- **AGB** (BGB §§305-310): Consumer-compliant terms of service
- **Updatepflicht** (BGB §327f): Minimum 2-year security/functional update commitment
- **Widerrufsrecht** (BGB §§355-356): 14-day withdrawal with digital download exception
- **Produkthaftung**: Compliant with EU Product Liability Directive 2024/2853

### Docker Integration
- Docker Engine: Apache 2.0 — free for all use
- Docker Desktop: Free for small business (<250 employees, <$10M revenue); paid otherwise
- ImpForge uses `bollard` crate (MIT) to communicate with Docker Engine API directly
- No Docker Desktop dependency — works with any OCI-compatible container runtime

### GitHub Integration
- GitHub API: Subject to [GitHub Terms of Service](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service) and [API Terms](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features)
- OAuth App / GitHub App: Users authenticate with their own credentials
- Rate limits: 5,000 req/hour (authenticated), respects `X-RateLimit` headers
- ImpForge uses `octocrab` crate (MIT/Apache-2.0)

### n8n Workflow Automation
- n8n uses "Sustainable Use License" (NOT traditional open source)
- ImpForge provides browser-based access to user's own n8n instance — no bundling/redistribution
- Users must separately install and license n8n according to [n8n's Fair-code terms](https://docs.n8n.io/hosting/)
- Enterprise use of n8n requires separate n8n Enterprise license

> **Disclaimer**: This is legal research, not legal advice. Consult qualified legal counsel before commercial launch. Full legal documentation: [`docs/legal/2026-03-08-nexus-legal-foundations.md`](docs/legal/2026-03-08-nexus-legal-foundations.md)

## Development Methodology

ImpForge follows **OpenSpec Spec-Driven Development (SDD)** — every feature goes through a structured pipeline:

1. **Proposal** → `specs/proposals/NNN-feature-name.md` (Intent, Constraints, Acceptance Criteria)
2. **Implementation** → Code following Svelte 5 runes + Tauri 2 IPC patterns
3. **Verification** → `cargo clippy`, `cargo test`, `pnpm check`, cross-platform build
4. **Promotion** → Move spec to `specs/current/`

### Research & Best Practices

Development decisions are backed by extensive research:

| Document | Focus |
|---|---|
| [`docs/research/2026-03-08-video-analysis-rust-svelte-enterprise.md`](docs/research/2026-03-08-video-analysis-rust-svelte-enterprise.md) | Rust + Svelte 5 + Tauri enterprise patterns |
| [`docs/research/2026-03-08-nexus-orchestrator-scientific-foundations.md`](docs/research/2026-03-08-nexus-orchestrator-scientific-foundations.md) | Neuroscience-inspired orchestrator design |
| [`docs/legal/2026-03-08-nexus-legal-foundations.md`](docs/legal/2026-03-08-nexus-legal-foundations.md) | EU AI Act, GDPR, commercial distribution |

### Key Patterns

- **Svelte 5 Runes** — `$state`, `$derived`, `$effect` (no legacy stores)
- **SSR disabled** — `export const ssr = false` in root layout (mandatory for Tauri)
- **Tauri IPC** — `invoke()` for Rust↔Frontend communication
- **SQLite WAL** — Embedded database, no external daemon
- **Sidecar pattern** — Ollama bundled as platform-specific binary

## Contributing

ImpForge is currently in active development. If you're interested in contributing, please reach out.

## License

MIT
