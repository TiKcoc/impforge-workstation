# ImpForge AI Workstation Builder

## Comprehensive Product Document

**Version:** 0.7.2
**Date:** March 19, 2026
**Classification:** Confidential — For Investors, Partners, and Enterprise Customers
**Company:** AiImp Development (Karsten Schildgen, Germany)
**License:** Dual — Apache-2.0 (Application) + BUSL-1.1 (Engine)
**Repository:** github.com/TiKcoc/impforge-workstation

---

## Current Status (v0.7.2)

| Metric | Value |
|--------|-------|
| **Rust Backend LoC** | 31,881 |
| **Svelte/TS Frontend LoC** | 49,699 |
| **Total LoC** | ~140,000+ (incl. engine crate, tests, configs) |
| **Tauri IPC Commands** | 500+ registered |
| **Unit Tests** | 1,080 passing, 0 failing |
| **Feature Modules** | 31 |
| **Route Pages** | 27 |
| **Svelte Components** | 200+ |
| **Cargo Dependencies** | ~260 crates |
| **Tree-sitter Grammars** | 28 languages |
| **Phases Completed** | 5 of 5 |

---

## 1. Executive Summary

**ImpForge** is the world's first fully adaptive, offline-capable, personal AI workstation. A single native desktop application that replaces the entire AI development toolchain, Microsoft Office, Adobe Acrobat, social media management, freelancer CRM, and team collaboration — all without requiring a cloud subscription, external database, or internet connection.

Built from the ground up in **Rust + Tauri 2.10 + Svelte 5**, ImpForge delivers enterprise-grade AI orchestration with neuroscience-inspired agent trust, a complete office suite, browser automation, and P2P team collaboration.

### Mission

> Replace fragmented, expensive, cloud-dependent software with a single offline-first desktop application that gives users complete sovereignty over their data and AI stack.

### Key Differentiators

1. **31 modules in ONE app** — more than Microsoft 365 + Slack + Adobe combined
2. **100% offline-capable** — local AI via Ollama, zero cloud dependency for core features
3. **DSGVO/GDPR by design** — all data stays on the user's machine
4. **Adaptive UI** — interface adapts to user role (Developer, Office, Freelancer, etc.)
5. **Self-extending** — users add any program, website, or API as a native module
6. **500+ Tauri commands** — deep Rust backend for every feature
7. **Neuroscience-based agents** — Three-Factor Hebbian Trust, MAPE-K self-healing

---

## 2. Complete Module Inventory (31 Modules, 5 Phases)

### Phase 1 — Developer Workstation (13 Modules)

| # | Module | Description | Key Stats |
|---|--------|-------------|-----------|
| 1 | **Chat/TerminalUI** | Multi-model AI chat with IDE layout (Explorer\|Editor+Terminal\|Chat) | Ollama (local) + OpenRouter (28+ free cloud models), NDJSON streaming, ForgeMemory enrichment |
| 2 | **CodeForge IDE** | Full-featured IDE with 15 panels | LSP, Git, Debug (DAP), Terminal (PTY), DB Client, HTTP Client, Symbol Outline, Search, Command Palette, Collab (CRDT), Spec-Driven, Code Graph |
| 3 | **NeuralSwarm Agents** | AI agent orchestration with live dashboard | 42 workers, run/stop/logs, 5s polling, Hebbian Trust scoring |
| 4 | **Docker Dashboard** | Container management | Start/stop, logs, ports, image management via bollard (MIT) |
| 5 | **GitHub Integration** | Repository management | Repos, issues, PRs via octocrab (MIT) |
| 6 | **n8n Workflows** | Workflow automation integration | Service health monitoring, embedded access |
| 7 | **Browser Agent** | CDP browser automation + web scraping | Navigate, click, fill, screenshot, form automation, data extraction |
| 8 | **AI Models** | Model management + provider config | HuggingFace Hub download, 15+ providers, GPU detection |
| 9 | **AI News Feed** | Curated RSS from 6 AI sources | Hacker News, Tauri, Svelte, Rust, Anthropic, Ollama blogs |
| 10 | **Evaluation** | Agent output quality assessment | Agent-as-Judge, quality metrics, model comparison |
| 11 | **App Launcher** | Add external programs/websites/APIs as modules | Native process launch, WebView, MCP servers, .desktop scanner, health monitoring |
| 12 | **Adaptive Onboarding** | 6-step wizard with user profiling | 8 roles (Developer→Custom), 3 experience levels, adaptive navigation |
| 13 | **Guided Tour + Module Discovery** | Interactive product tour | Spotlight overlay, 3 role-based tours, module explorer for hidden features |

### Phase 2 — Business Automation (6 Modules)

| # | Module | Description | Key Stats |
|---|--------|-------------|-----------|
| 14 | **Social Media Hub** | Multi-platform content management | 6 platforms (LinkedIn, X, GitHub, HackerNews, Mastodon, Discord), AI content gen (StoryBrand + 7 Hook Archetypes), Golden Hour scheduling, content queue |
| 15 | **Freelancer Hub** | Complete freelancer management | Profile, gigs, clients (CRM-lite), AI proposal generation, invoices with line items, time tracking with live timer, earnings analytics |
| 16 | **Auto-Publisher** | CDP browser automation for publishing | Multi-platform auto-post, profile sync, pre-built CDP scripts for LinkedIn/X/Fiverr/Upwork/GitHub, automation log |
| 17 | **Platforms Manager** | Connected platform management | Per-platform toggles (☑ Enabled, ☑ Auto-Sync, ☑ Auto-Post), 10 pre-defined platforms, custom platforms |
| 18 | **ForgeMail** | AI-powered email client | Gmail-inspired 3-panel UI, 5 providers, AI compose/reply (4 tones), categorization, thread summarization, 15 commands |
| 19 | **Browser Auto-Import** | Automatic data export from external sites | CDP scripts for Google Calendar/Sheets/Drive, Outlook, Office 365, OneDrive, Dropbox export, auto-import scheduler |

### Phase 3 — Sovereign Office Suite (9 Modules)

| # | Module | Description | Key Stats |
|---|--------|-------------|-----------|
| 20 | **ForgeWriter** | AI document editor (Word replacement) | Markdown editor, 7 AI actions (improve, shorten, expand, fix grammar, translate EN/DE, summarize), auto-save 30s, export .md/.html/.txt |
| 21 | **ForgeSheets** | Spreadsheet engine (Excel replacement) | Formula engine (SUM, AVG, COUNT, MIN, MAX, IF, CONCAT + arithmetic), XLSX/CSV/ODS import via calamine, XLSX export via rust_xlsxwriter, AI NL→Formula (arXiv:2510.15585), Auto-EDA |
| 22 | **ForgePDF** | PDF viewer/converter (Acrobat replacement) | Byte-level text extraction, AI summarize/Q&A (RAG pattern), convert to .txt/.md, metadata display, 13 tests |
| 23 | **ForgeCanvas** | 3-panel AI document workspace | LEFT: drag-drop sources with chunks + rubber-band select. CENTER: AI-generated document with 21 backgrounds + templates. RIGHT: source inspector with formula breakdown (Rechenweg). BOTTOM: context-aware chat. Professional export (A4/Letter, footnotes). 8 templates (Business Report, Restaurant Menu, Invoice, etc.) |
| 24 | **ForgeSlides** | Presentation creator (PowerPoint replacement) | Markdown-based, 6 layouts, 6 themes (Corporate/Creative/Minimal/Tech/Nature), AI generate full presentations, AI improve slides, fullscreen present mode, HTML export |
| 25 | **ForgeNotes** | Knowledge base (Notion replacement) | [[Wiki-links]] with bidirectional backlinks, Knowledge Graph visualization (SVG force-directed), full-text search, AI generate/connect/summarize, tags with counts, auto-save 10s |
| 26 | **ForgeCalendar** | Calendar with ICS import | Own ICS parser (RFC 5545), import from Google/Outlook/Apple Calendar exports, Month/Week/Day views, AI daily briefing, AI free-time finder, AI meeting agenda generator, ICS URL subscription |
| 27 | **File Hub** | Universal file processor | 14 categories, 60+ formats detected (magic bytes + extension), DOCX/PPTX text extraction (ZIP→XML), universal converter (docx→md, xlsx→csv, json↔yaml, etc.), AI digest, auto-routing to correct Forge module |

### Phase 4 — Team Collaboration (3+ Modules)

| # | Module | Description | Key Stats |
|---|--------|-------------|-----------|
| 28 | **ForgeTeam** | P2P team management | Create/join teams (8-char invite code), member roles (Owner/Admin/Member/Viewer), online status, trust scores, contribution tracking |
| 29 | **ImpBook** | Shared AI knowledge workspace | 8 entry types (AgentResult, Document, Task, Idea, CodeReview, Report, Discussion, Milestone), emoji reactions (👍🎉🚀❤️🤔👀) with toggle, threaded comments, agent result auto-sharing, AI learning from feedback, smart suggestions |
| 30 | **Team Chat + Goals** | Real-time messaging + milestones | Reply-to, ImpBook entry references, team goals with progress bars (0-100%), deadline indicators (green/yellow/red), linked entries |

### Phase 5 — Infrastructure (1 Module)

| # | Module | Description | Key Stats |
|---|--------|-------------|-----------|
| 31 | **Universal Connector** | Zero-config auto-discovery for all services and installed programs | 11 service types (Ollama, PostgreSQL, Redis, Docker, Git, MCP, Claude Code, n8n), 50+ program detection across 9 categories (AI/LLM, IDEs, Office, Adobe, Browsers, Dev Tools, Creative, Communication, System), parallel scanning <1s, arXiv:2506.01056 (MCP-Zero) |

---

## 3. Technical Architecture

### Stack

| Layer | Technology | License |
|-------|-----------|---------|
| **Frontend** | Svelte 5 (runes), SvelteKit, TypeScript | MIT |
| **Backend** | Tauri 2.10, Rust 2021 edition | MIT/Apache-2.0 |
| **Styling** | Tailwind CSS v4, BenikUI style engine | MIT |
| **UI Components** | bits-ui, PaneForge, Lucide icons | MIT |
| **AI Local** | Ollama, Candle, llama.cpp (via GGUF) | MIT/Apache-2.0 |
| **AI Cloud** | OpenRouter (28+ free models) | API Terms |
| **Database** | SQLite (rusqlite, WAL mode) | Public Domain |
| **Memory** | ForgeMemory (BM25 + HNSW vector + Knowledge Graph) | Apache-2.0 |
| **PDF** | lopdf (read), custom text extraction | MIT |
| **Spreadsheet** | calamine (read), rust_xlsxwriter (write) | MIT |
| **Archive** | zip crate (DOCX/PPTX parsing) | MIT |
| **Docker** | bollard crate | MIT/Apache-2.0 |
| **GitHub** | octocrab crate | MIT/Apache-2.0 |
| **VPN (future)** | boringtun (WireGuard-compatible) | BSD-3-Clause |

### ForgeMemory Engine (20 sub-modules)

| Component | Technology | Purpose |
|-----------|-----------|---------|
| BM25 Search | Robertson & Zaragoza 2009 | Full-text keyword search |
| Vector Search | HNSW (custom Rust) | Semantic similarity |
| Embeddings | FastEmbed (ONNX, local) | 384/1024-dim vectors |
| Knowledge Graph | Custom graph engine | Entity relationships |
| AST Chunking | tree-sitter (28 languages) | Code-aware splitting |
| NLP Pipeline | Custom tokenizer + stemmer | Text preprocessing |
| LLM Extract | Ollama integration | Entity extraction |
| File Watcher | notify crate | Auto-ingest on file change |
| Digest Engine | Ollama summarization | Document compression |
| Context Builder | Sliding window + relevance | Conversation enrichment |

### NeuralSwarm Orchestrator

| Component | Scientific Basis | Purpose |
|-----------|-----------------|---------|
| Three-Factor Hebbian Trust | arXiv:2504.05341, Izhikevich 2007 | Worker reliability scoring |
| MAPE-K Self-Healing | Kephart & Chess 2003 (IBM) | Autonomous recovery |
| FSRS-5 Scheduler | Ye et al. 2024 | Spaced repetition |
| CLS Replay | McClelland et al. 1995 | Memory consolidation |
| MoA Pipeline | arXiv:2601.16596 | Multi-model consensus |
| Agent Topology | arXiv:2602.06039 | Dynamic worker organization |
| AIMD Scaling | Chiu & Jain 1989 | Adaptive worker count |
| Circuit Breaker | Nygard 2007 | Failure isolation |

---

## 4. Competitive Analysis

### vs. AI IDEs

| Feature | ImpForge | Cursor | VS Code + Copilot | Windsurf |
|---------|----------|--------|-------------------|----------|
| Offline AI | Yes (Ollama) | No | No | No |
| Local models | Any GGUF/HF | No | No | No |
| 15+ AI providers | Yes | 2 | 1 | 2 |
| Docker built-in | Yes | No | Extension | No |
| Office suite | Yes (5 apps) | No | No | No |
| Social media | Yes | No | No | No |
| Team collab | Yes (ImpBook) | No | No | No |
| Freelancer CRM | Yes | No | No | No |
| Price (core) | Free | $20/mo | $10/mo | $15/mo |

### vs. Office Suites

| Feature | ImpForge | Microsoft 365 | Google Workspace | LibreOffice |
|---------|----------|--------------|-----------------|-------------|
| AI-native | Every tool | Copilot add-on | Gemini add-on | None |
| Offline | 100% | Limited | No | Yes |
| IDE included | Yes (15-panel) | No | No | No |
| Agent system | Yes (42 workers) | No | No | No |
| Calendar | Yes (ICS import) | Outlook | Google Cal | No |
| Email | Yes (ForgeMail) | Outlook | Gmail | No |
| Knowledge base | Yes (ForgeNotes) | OneNote | Keep | No |
| Subscription | Optional | $6-22/user/mo | $6-18/user/mo | Free |

### Unique Market Position

ImpForge is the **ONLY product** that combines: IDE + Local AI + Office Suite + Social Media + Freelancer CRM + Team Collaboration in a single offline-first desktop application.

---

## 5. Pricing Strategy

| Tier | Price | Target | Key Features |
|------|-------|--------|-------------|
| **Free** | $0/forever | Students, hobbyists | Chat, basic IDE, 1 local model, ForgeWriter |
| **Solo** | $9/mo | Solo developers | Full IDE, unlimited local models, ForgeMemory, Docker |
| **Pro** | $19/mo | Professional devs | All Phase 1-2, cloud routing, Social Media Hub |
| **Business** | $29/mo | Freelancers, SMBs | Full Office Suite, Freelancer Hub, Auto-Publisher |
| **Team** | $15/user/mo | Small teams | All + ForgeTeam, ImpBook, Chat, Goals |
| **Enterprise** | Custom | Organizations | SSO, audit logs, priority support, on-premise |

---

## 6. Legal Framework

### Licensing
- **src-tauri/**: Apache-2.0 (open source, commercial-friendly)
- **crates/impforge-engine/**: BUSL-1.1 (visible source, converts to Apache-2.0 after 4 years)
- **Dependencies**: 100% MIT/Apache-2.0/BSD (verified, zero GPL/AGPL/LGPL)

### Compliance
- **EU AI Act (2024/1689)**: Limited risk classification, Art. 50 transparency
- **DSGVO/GDPR**: Privacy by Design, data minimization, right to erasure
- **Clean Room**: No Microsoft/Adobe code, own implementations from open specifications
- **EU Product Liability (2024/2853)**: Software as product, 3-year update commitment

---

## 7. Target Market

| Segment | TAM (2026) |
|---------|-----------|
| Developer Tools & IDEs | $22.7B |
| AI Development Platforms | $45.3B |
| Office Productivity | $55.8B |
| Freelancer Tools | $8.2B |
| **Combined TAM** | **~$132B** |

### SOM (Year 1-3)

| Year | Users | ARR | Strategy |
|------|-------|-----|----------|
| Y1 | 5,000 | $450K | Dev community, open source |
| Y2 | 25,000 | $3.5M | Product Hunt, conferences |
| Y3 | 100,000 | $18M | Enterprise sales, marketplace |

---

## 8. Quality Gates

| Gate | Command | Status |
|------|---------|--------|
| Rust compilation | `cargo check` | 0 errors |
| Rust tests | `cargo test` | 1,080 passing |
| Svelte check | `pnpm check` | Clean |
| License audit | `cargo license` | No GPL/AGPL |

---

## 9. Scientific References

| Paper | Application |
|-------|------------|
| arXiv:2504.05341 | Agent-as-Judge evaluation |
| arXiv:2601.16596 | Mixture-of-Agents pipeline |
| arXiv:2602.06039 | Dynamic agent topology |
| arXiv:2412.16837 | Adaptive UI via RL (onboarding) |
| arXiv:2510.15585 | TDD + LLM spreadsheet formulas |
| arXiv:2501.06322 | Multi-Agent Collaboration Survey |
| Izhikevich 2007 | Three-Factor Hebbian Trust |
| Robertson & Zaragoza 2009 | BM25 search algorithm |
| Ye et al. 2024 | FSRS-5 spaced repetition |
| Miller 2021 | StoryBrand content framework |
| Kephart & Chess 2003 | MAPE-K self-healing |
| arXiv:2506.01056 | MCP-Zero auto-discovery (Universal Connector) |

---

**Company:** AiImp Development | **Founder:** Karsten Schildgen | **Country:** Germany
**Repository:** github.com/TiKcoc/impforge-workstation
**Classification:** Confidential — authorized parties only

*Document version: 0.7.2 | Last updated: March 19, 2026*
