# ImpForge AI Workstation Builder

## Comprehensive Product Document

**Version:** 0.7.0
**Date:** March 2026
**Classification:** Confidential -- For Investors, Partners, and Enterprise Customers
**Company:** AiImp Development (Karsten Schildgen, Germany)
**License:** Dual -- Apache-2.0 (Application) + BUSL-1.1 (Engine)
**Repository:** github.com/TiKcoc/impforge-workstation

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Product Vision and Roadmap](#2-product-vision-and-roadmap)
3. [Complete Feature Inventory](#3-complete-feature-inventory)
4. [Technical Architecture](#4-technical-architecture)
5. [Sovereign Office Suite](#5-sovereign-office-suite)
6. [AI and Neuroscience Architecture](#6-ai-and-neuroscience-architecture)
7. [Competitive Analysis](#7-competitive-analysis)
8. [Pricing Strategy](#8-pricing-strategy)
9. [Legal Framework](#9-legal-framework)
10. [Target Market](#10-target-market)
11. [Key Metrics and KPIs](#11-key-metrics-and-kpis)

---

## 1. Executive Summary

### Company

**AiImp Development** is a software company founded by Karsten Schildgen in Germany, focused on building the next generation of AI-powered productivity tools. The company operates under the conviction that AI development infrastructure should be owned by the user, not rented from a cloud provider. Every line of code is written for commercial distribution -- no GPL, no cloud lock-in, no external database dependencies.

### Product

**ImpForge AI Workstation Builder** is a standalone desktop application that replaces the entire AI development and business toolchain with a single, offline-first application. Built from the ground up in Rust (Tauri 2.10) and Svelte 5, ImpForge delivers enterprise-grade AI orchestration, a full IDE, browser automation, Docker management, a sovereign office suite, social media automation, freelancer CRM, team collaboration, and intelligent agent coordination -- all without requiring a cloud subscription, external database, or internet connection for core functionality.

ImpForge is not an IDE plugin. It is not a chat wrapper. It is a complete AI-native operating environment for developers, freelancers, small businesses, and teams who refuse to give up control of their tools or their data.

### Mission

> Democratize AI development by shipping a complete, offline-first workstation that any developer or business professional can install, own, and extend -- without vendor lock-in, without recurring fees for core features, and without compromising on enterprise-grade capabilities.

### Current Status (v0.7.0 -- March 2026)

| Metric | Value |
|--------|-------|
| Version | 0.7.0 |
| Total Lines of Code | ~130,000+ |
| Rust Backend LoC | ~90,000 |
| Svelte/TypeScript Frontend LoC | ~40,000 |
| Test Suite | 1,012 passing tests |
| Tauri IPC Commands | 250+ |
| Svelte Components | 200+ |
| Route Pages | 26 |
| Feature Modules | 26 (4 phases complete) |
| Supported AI Providers | 15+ (local + cloud) |
| Supported Languages (AST) | 28 (tree-sitter grammars) |
| Build Targets | Linux, Windows, macOS |
| External DB Required | None (SQLite bundled) |
| External Runtime Required | None (self-contained binary) |

### Differentiators

1. **Offline-First AI**: Ollama sidecar, llama.cpp (GGUF), Candle, FastEmbed -- all run locally without internet
2. **Zero External Dependencies**: SQLite bundled via rusqlite, no PostgreSQL, Redis, or Docker required for core operation
3. **Sovereign Office Suite**: ForgeWriter, ForgeSheets, ForgePDF, ForgeCanvas, ForgeSlides -- AI-native alternatives to Microsoft 365
4. **Neuroscience-Inspired Orchestration**: Three-Factor Hebbian Trust, MAPE-K self-healing, FSRS-5 spaced repetition
5. **26 Integrated Modules**: From code editor to email client to team collaboration, in a single binary
6. **Dual License**: Apache-2.0 application layer enables community contribution; BUSL-1.1 engine protects commercial IP

---

## 2. Product Vision and Roadmap

### Phase Architecture

ImpForge is organized into four product phases, each targeting a distinct user persona. All four phases are implemented and shipping in v0.7.0.

```
+------------------------------------------------------------------+
|                    IMPFORGE v0.7.0 ARCHITECTURE                  |
+------------------------------------------------------------------+
|                                                                  |
|  Phase 1: DEVELOPER (11 modules)                                 |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  | Chat/AI   |  | CodeForge|  | NeuralSwarm |  | Docker    |    |
|  | Terminal  |  | IDE      |  | Agents      |  | Dashboard |    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  | GitHub    |  | App      |  | Onboarding  |  | Guided    |    |
|  | Integr.   |  | Launcher |  | (Adaptive)  |  | Tour      |    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  +-----------+  +----------+  +-------------+                    |
|  | n8n       |  | Browser  |  | AI Models   |                    |
|  | Workflows |  | Agent    |  | Management  |                    |
|  +-----------+  +----------+  +-------------+                    |
|                                                                  |
|  Phase 2: BUSINESS (5 modules)                                   |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  | Social    |  | Freelance|  | Auto-       |  | Forge     |    |
|  | Media Hub |  | r Hub    |  | Publisher   |  | Mail      |    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  +-----------+                                                   |
|  | Platforms |                                                   |
|  | Manager   |                                                   |
|  +-----------+                                                   |
|                                                                  |
|  Phase 3: OFFICE -- Sovereign Office Suite (6 modules)           |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  | Forge     |  | Forge    |  | Forge       |  | Forge     |    |
|  | Writer    |  | Sheets   |  | PDF         |  | Canvas    |    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  +-----------+  +----------+                                     |
|  | Forge     |  | File     |                                     |
|  | Slides    |  | Hub      |                                     |
|  +-----------+  +----------+                                     |
|                                                                  |
|  Phase 4: TEAM (4 modules)                                       |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|  | Forge     |  | ImpBook  |  | Team        |  | Team      |    |
|  | Team      |  | (Shared) |  | Chat        |  | Goals     |    |
|  +-----------+  +----------+  +-------------+  +-----------+    |
|                                                                  |
+------------------------------------------------------------------+
|  INFRASTRUCTURE LAYER                                            |
|  +--------+  +--------+  +---------+  +--------+  +--------+    |
|  | Tauri  |  | SQLite |  | Forge   |  | Router |  | Orchest|    |
|  | 2.10   |  | (WAL)  |  | Memory  |  | Engine |  | rator  |    |
|  +--------+  +--------+  +---------+  +--------+  +--------+    |
+------------------------------------------------------------------+
```

### Roadmap Summary

| Phase | Name | Modules | Target User | Status |
|-------|------|---------|-------------|--------|
| 1 | Developer | 11 | Solo devs, indie hackers | COMPLETE |
| 2 | Business | 5 | Freelancers, solopreneurs | COMPLETE |
| 3 | Office (Sovereign) | 6 | Knowledge workers, SMBs | COMPLETE |
| 4 | Team | 4 | Small teams, agencies | COMPLETE |

### Version History

| Version | Date | Milestone |
|---------|------|-----------|
| 0.1.0 | Jan 2026 | Initial Tauri + Svelte scaffold, Chat UI |
| 0.2.0 | Jan 2026 | Router engine, Docker, GitHub, Agent dashboard |
| 0.3.0 | Feb 2026 | Browser agent, n8n, App Launcher, Onboarding |
| 0.4.0 | Feb 2026 | Standalone Orchestrator, 43 workers, Brain v2.0 |
| 0.5.0 | Feb 2026 | Social Media Hub, Freelancer Hub, Auto-Publisher |
| 0.6.0 | Mar 2026 | Sovereign Office Suite, ForgeMemory, CodeForge IDE |
| 0.7.0 | Mar 2026 | ForgeTeam, ImpBook, Team Chat, Team Goals, 1K+ tests |

---

## 3. Complete Feature Inventory

### Phase 1 -- Developer (11 Modules)

#### 1. Chat / Terminal UI

The primary AI interaction surface. Supports both local inference (Ollama) and cloud providers (OpenRouter, OpenAI, Anthropic, Mistral, Groq, Google, Cohere, xAI, DeepSeek) through a unified routing engine.

| Capability | Implementation |
|-----------|---------------|
| Local models | Ollama API (streaming SSE) |
| Cloud models | OpenRouter, OpenAI, Anthropic, Mistral via `llm` + `genai` crates |
| Streaming | Token-by-token via Tauri events (`chat-stream`) |
| Conversation history | SQLite (WAL mode) with full message persistence |
| Model switching | Runtime model selection, per-conversation |
| ForgeMemory integration | Automatic context injection from project knowledge |
| Code detection | Syntax-highlighted code blocks with copy-to-clipboard |
| Message routing | Cascade Router classifies intent and selects optimal provider |

**Rust modules:** `chat.rs`, `ollama.rs`, `router/`, `ai/`
**Frontend:** `/routes/chat/+page.svelte`

#### 2. CodeForge IDE

A full-featured integrated development environment embedded within ImpForge. Designed to replace VS Code for AI-centric workflows.

| Panel | Capability |
|-------|-----------|
| File Explorer | Project tree with icons, lazy-load, search |
| Code Editor | Monaco-grade editing with syntax highlighting (28 languages via tree-sitter) |
| Integrated Terminal | Real PTY via `portable-pty` (bash, zsh, PowerShell) |
| Git Panel | Diff viewer, staging, commit, branch management via `git2` |
| Debug Panel | Breakpoint management, variable inspection, call stack |
| LSP Client | Language Server Protocol integration via `lsp-types` |
| AI Autocomplete | Context-aware completions using ForgeMemory + local/cloud LLM |
| DB Client | SQLite/PostgreSQL query runner with result tables |
| HTTP Client | REST/GraphQL request builder with response visualization |
| Shadow Workspace | Background analysis for AI-powered suggestions |
| Code Indexer | AST-aware indexing across 28 languages (tree-sitter) |
| Billing Tracker | Time tracking per project (freelancer integration) |
| Collaboration | Real-time shared editing (Phase 4 P2P) |
| Search | Project-wide fuzzy search, symbol search, reference search |
| Minimap | Code overview with scroll position indicator |

**Rust modules:** `ide/mod.rs`, `ide/pty.rs`, `ide/lsp.rs`, `ide/git.rs`, `ide/debug.rs`, `ide/indexer.rs`, `ide/ai_complete.rs`, `ide/shadow.rs`, `ide/db_client.rs`, `ide/http_client.rs`, `ide/billing.rs`, `ide/collab.rs`
**Frontend:** `/routes/ide/+page.svelte`

#### 3. NeuralSwarm Agents

An autonomous multi-agent orchestration system with 42 specialized workers. Each worker has a defined responsibility, trust score, and health status. The system uses neuroscience-inspired trust dynamics and self-healing control loops.

| Capability | Detail |
|-----------|--------|
| Workers | 42 specialized async workers (filesystem, process, network, AI, social) |
| Trust model | Three-Factor Hebbian (dopamine x novelty x homeostasis) |
| Self-healing | MAPE-K control loop (Monitor, Analyze, Plan, Execute, Knowledge) |
| Scheduling | Cron-based via `croner` (extended syntax with L, #, W, seconds, timezone) |
| Message bus | 7 typed channels (TaskSubmit, TaskComplete, StatusUpdate, Alert, Metric, Command, Broadcast) |
| Worker pool | Semaphore-limited concurrent execution |
| Topology | Dynamic agent graph with connection strength tracking |
| MoA pipeline | Mixture-of-Agents inference (arXiv:2601.16596) |
| Evaluation | Agent-as-Judge quality assessment (arXiv:2504.05341) |
| Scaling | AIMD-based auto-scaling (Additive Increase / Multiplicative Decrease) |
| Resource governor | Circuit breakers, GPU detection (NVIDIA via NVML, AMD via libamdgpu_top) |
| Brain v2.0 | FSRS-5 spaced repetition, CLS replay, AutoLabeler, Zettelkasten |

**Rust modules:** `orchestrator/`, `neuralswarm.rs`, `orchestrator/workers/mod.rs`
**Frontend:** `/routes/agents/+page.svelte`

#### 4. Docker Dashboard

Full Docker container management via the Bollard API (Rust-native Docker client). No Docker CLI parsing -- direct API integration.

| Capability | Detail |
|-----------|--------|
| Container list | Running, stopped, all containers with status |
| Container actions | Start, stop, restart, remove, inspect |
| Image management | List, pull, remove, inspect images |
| Log streaming | Real-time container log tailing |
| Stats | CPU, memory, network I/O per container |
| Compose awareness | Recognize and group docker-compose services |

**Rust module:** `docker/mod.rs`
**Frontend:** `/routes/docker/+page.svelte`

#### 5. GitHub Integration

Native GitHub integration via the Octocrab API client, providing repository, issue, and pull request management without leaving ImpForge.

| Capability | Detail |
|-----------|--------|
| Repository browser | List, search, filter user/org repositories |
| Issue management | Create, edit, comment, close issues |
| Pull requests | View diffs, review, merge PRs |
| Commit history | Browse commits with diff visualization |
| Actions status | CI/CD workflow run status |
| Authentication | Personal access token, OAuth app flow |

**Rust module:** `github/mod.rs`
**Frontend:** `/routes/github/+page.svelte`

#### 6. App Launcher

A Steam-inspired universal application launcher that lets users register, launch, and manage external programs, WebView applications, and MCP server connections from within ImpForge.

| Capability | Detail |
|-----------|--------|
| App registry | Self-extending catalog of installed applications |
| Launch profiles | Per-app environment variables, working directory |
| WebView apps | Embed web applications as first-class panels |
| MCP servers | Connect to Model Context Protocol servers |
| Categories | Developer tools, creative apps, system utilities |
| Recent/favorites | Quick access to frequently used applications |

**Rust module:** `app_launcher.rs`
**Frontend:** `/routes/apps/+page.svelte`, `/routes/apps/[id]/+page.svelte`

#### 7. Adaptive Onboarding

A 5-question profiling system that personalizes the ImpForge experience based on user role, skill level, and primary use case. Supports 8 distinct user personas.

| User Role | Unlocked Modules |
|-----------|-----------------|
| Solo Developer | IDE, Chat, Docker, GitHub, Agents |
| Freelancer | IDE, Chat, Freelancer Hub, Social Media, Invoicing |
| Team Lead | IDE, Chat, ForgeTeam, ImpBook, Goals |
| Data Scientist | IDE, Chat, AI Models, Notebooks, Evaluation |
| Content Creator | Chat, ForgeWriter, Social Media, Auto-Publisher |
| Business Owner | Freelancer Hub, Social Media, ForgeMail, Platforms |
| Student | IDE, Chat, ForgeWriter, AI Models |
| DevOps Engineer | Docker, GitHub, CI/CD, Agents, Monitoring |

**Frontend:** Integrated into initial app launch flow

#### 8. Guided Tour and Module Discovery

A spotlight-overlay guided tour system with progressive disclosure. New users are walked through available modules, while experienced users can discover new features through contextual hints.

| Capability | Detail |
|-----------|--------|
| Spotlight overlay | Focused highlight on target UI elements |
| Step sequences | Multi-step tours per module |
| Progressive disclosure | Show features as users need them |
| Completion tracking | Remember which tours have been completed |
| Context triggers | Show relevant tours when users encounter new areas |

**Frontend:** Overlay components integrated into layout

#### 9. n8n Workflow Integration

Embeds the n8n workflow automation platform as a WebView panel within ImpForge. Users can build, test, and deploy automation workflows without leaving the application.

| Capability | Detail |
|-----------|--------|
| WebView embed | Full n8n interface in an ImpForge tab |
| Connection manager | Configure n8n server URL and credentials |
| Workflow triggers | Launch workflows from ImpForge events |
| Status monitoring | View running workflow executions |

**Frontend:** `/routes/n8n/+page.svelte`

#### 10. Browser Agent

An AI-powered web automation agent inspired by the OpAgent architecture. Uses Chrome DevTools Protocol (CDP) via `chromiumoxide` for headless and headed browser control.

| Capability | Detail |
|-----------|--------|
| CDP automation | Navigate, click, fill forms, screenshot via DevTools Protocol |
| Web scraping | HTML parsing (scraper) + HTML-to-Markdown (fast_html2md) |
| Network monitoring | HTTP waterfall capture, request/response interception |
| Cookie management | Read, write, delete browser cookies |
| Console capture | Capture browser console logs and errors |
| Performance metrics | Page load timing, resource waterfall |
| Data import | Auto-detect and import bookmarks/history from installed browsers |
| Screenshot | Full-page and element-level screenshot capture |

**Rust modules:** `browser_agent.rs`, `cdp_engine.rs`, `cdp_network.rs`, `cdp_devtools.rs`, `web_scraper.rs`, `browser.rs`, `browser_import.rs`
**Frontend:** `/routes/browser/+page.svelte`

#### 11. AI Models Management

A HuggingFace Hub-integrated model browser and manager. Users can discover, download, and manage AI models for local inference.

| Capability | Detail |
|-----------|--------|
| HuggingFace Hub | Browse, search, download models via `hf-hub` |
| GGUF support | Parse and load quantized GGUF models via `llama-cpp-2` |
| Candle inference | Run HuggingFace models natively via Candle |
| Model cards | Display model metadata, benchmarks, license info |
| Download manager | Background download with progress, resume support |
| Local model list | Manage downloaded models with disk usage tracking |
| Provider status | Health check for Ollama, OpenRouter, OpenAI endpoints |

**Rust modules:** `inference/mod.rs`, `inference/hub.rs`, `inference/gguf.rs`, `inference/candle_engine.rs`, `inference/rig_router.rs`, `inference/fsrs_scheduler.rs`
**Frontend:** `/routes/ai/+page.svelte`

---

### Phase 2 -- Business (5 Modules)

#### 12. Social Media Hub

A comprehensive social media management platform with AI-powered content generation. Supports 6 platforms with scheduling, analytics, and brand-aligned content creation using StoryBrand methodology.

| Capability | Detail |
|-----------|--------|
| Platforms | LinkedIn, GitHub, Twitter/X, Fiverr, Upwork, Hacker News |
| AI content generation | 5 content generation prompts with StoryBrand framework |
| Golden Hour scheduling | AI-optimized posting times per platform |
| Content calendar | Visual timeline of scheduled posts |
| Analytics dashboard | Engagement metrics, growth tracking |
| Hook archetypes | 7 proven hook patterns for content creation |
| Multi-post | Cross-platform simultaneous publishing |

**Rust module:** `social.rs`, `orchestrator/social_media.rs`
**Frontend:** `/routes/social/+page.svelte`

#### 13. Freelancer Hub

A CRM-lite system built for solo freelancers and small agencies. Manages clients, gigs, proposals, invoices, and time tracking in a single integrated view.

| Capability | Detail |
|-----------|--------|
| Client management | Contact database with project history |
| Gig tracking | Active gigs with status, deadlines, revenue |
| AI proposals | Generate client proposals using AI + project context |
| Invoice generation | Create and export professional invoices |
| Time tracking | Per-project time logging with IDE integration |
| Pipeline view | Kanban-style deal pipeline |
| Revenue analytics | Monthly/quarterly revenue tracking and projections |

**Rust module:** `freelancer.rs`
**Frontend:** `/routes/freelancer/+page.svelte`

#### 14. Auto-Publisher

A CDP-based universal cross-platform automation engine. Automates repetitive publishing tasks with user approval. Performs real browser actions (not API scraping) for platform compatibility.

| Capability | Detail |
|-----------|--------|
| Browser automation | Navigate, fill, click, submit via CDP |
| Platform adapters | LinkedIn, GitHub, Fiverr, Upwork |
| Approval queue | User reviews and approves all actions before execution |
| Profile sync | Synchronize professional profile across platforms |
| Post scheduling | Queue and schedule posts for optimal timing |
| Error recovery | Retry failed actions with exponential backoff |

**Rust module:** `auto_publisher.rs`
**Frontend:** Integrated into Social Media Hub and Platforms Manager

#### 15. ForgeMail

An AI-powered email client integrated into ImpForge. Compose, categorize, and manage email with AI assistance for drafting, replying, and prioritization.

| Capability | Detail |
|-----------|--------|
| AI compose | Generate email drafts from bullet points or intent |
| AI reply | Contextual reply suggestions based on thread history |
| Categorization | AI-powered email categorization and priority sorting |
| Templates | Reusable email templates with variable substitution |
| Search | Full-text search across email history |
| Attachments | File attachment with preview |

**Rust module:** `forge_mail.rs`
**Frontend:** `/routes/mail/+page.svelte`

#### 16. Platforms Manager

A centralized hub for managing connected platforms. View sync status, manage API keys, and monitor connected service health from a single dashboard.

| Capability | Detail |
|-----------|--------|
| Connected platforms | Visual status of all linked services |
| API key management | Secure storage and rotation of platform credentials |
| Sync status | Last sync time, pending changes, error states |
| Auto-sync profile | Push professional profile updates across all platforms |
| Health monitoring | Connectivity and API quota tracking |

**Frontend:** `/routes/platforms/+page.svelte`

---

### Phase 3 -- Sovereign Office Suite (6 Modules)

The Sovereign Office Suite is ImpForge's answer to Microsoft 365 and Google Workspace. Every document tool is AI-native from the ground up. All data stays local. All formats are interoperable.

See [Section 5](#5-sovereign-office-suite) for detailed architecture.

#### 17. ForgeWriter

A Markdown-based document editor with AI writing assistance. Supports real-time preview, auto-save, and multi-format export.

| Capability | Detail |
|-----------|--------|
| Markdown editing | Full CommonMark + GFM support |
| AI assist | Rewrite, expand, summarize, translate selected text |
| Auto-save | Periodic save to SQLite with version history |
| Export | PDF, HTML, DOCX, plain text |
| Templates | Document templates for common use cases |
| Focus mode | Distraction-free writing environment |

**Rust module:** `forge_writer.rs`
**Frontend:** `/routes/writer/+page.svelte`

#### 18. ForgeSheets

A clean-room spreadsheet engine built entirely in Rust. No reverse-engineered Excel code -- clean implementation with formula evaluation, XLSX/CSV import/export, and AI-powered natural language to formula conversion.

| Capability | Detail |
|-----------|--------|
| Formula engine | Custom evaluator supporting 50+ functions |
| Cell formatting | Number, currency, date, percentage, conditional |
| Import | XLSX, XLS, ODS, XLSB, CSV (via `calamine`) |
| Export | XLSX (via `rust_xlsxwriter`), CSV |
| AI NL-to-Formula | Describe what you want in natural language, get a formula |
| Charts | Basic chart rendering from data ranges |
| Multi-sheet | Tabbed workbook with cross-sheet references |
| Sorting/Filtering | Column sort, auto-filter, custom filter rules |

**Rust module:** `forge_sheets.rs`
**Frontend:** `/routes/sheets/+page.svelte`

#### 19. ForgePDF

A PDF viewer and analysis tool with AI-powered summarization, question answering, and format conversion.

| Capability | Detail |
|-----------|--------|
| PDF viewing | Render and navigate multi-page PDF documents |
| Text extraction | Extract text content from PDF pages |
| AI summarize | Generate executive summaries of long documents |
| AI Q&A | Ask questions about document content |
| Annotations | Highlight, underline, note annotations |
| Convert | PDF to Markdown, PDF to text, images to PDF |

**Rust module:** `forge_pdf.rs`
**Frontend:** `/routes/pdf/+page.svelte`

#### 20. ForgeCanvas

A 3-panel AI document workspace for complex knowledge work. Users drag source documents into the canvas, link related content, and generate output using AI -- all in a spatial, visual interface.

| Capability | Detail |
|-----------|--------|
| 3-panel layout | Source panel, workspace canvas, output panel |
| Source linking | Drag and link documents, code, URLs as context |
| Rubber-band select | Multi-select canvas elements for batch operations |
| AI generation | Generate summaries, reports, code from linked sources |
| Professional export | Export canvas output as PDF, Markdown, HTML |
| Spatial layout | Free-form positioning with snap-to-grid |

**Rust module:** `forge_canvas.rs`
**Frontend:** `/routes/canvas/+page.svelte`

#### 21. ForgeSlides

A Markdown-based presentation creator with AI-powered slide generation, 6 professional themes, and a fullscreen present mode.

| Capability | Detail |
|-----------|--------|
| Markdown slides | Write slides in Markdown, render as presentation |
| AI generation | Generate entire presentations from a topic or outline |
| 6 themes | Professional, Dark, Light, Corporate, Academic, Creative |
| Present mode | Fullscreen presentation with speaker notes |
| Export | PDF slides, HTML presentation, image sequence |
| Transitions | Slide transitions and build animations |

**Rust module:** `forge_slides.rs`
**Frontend:** `/routes/slides/+page.svelte`

#### 22. File Hub

A universal file processor supporting 60+ formats with auto-detection, preview, conversion, and AI-powered digest generation.

| Capability | Detail |
|-----------|--------|
| Format detection | Auto-detect 60+ file types by extension and magic bytes |
| Preview | In-app preview for text, code, images, PDF, spreadsheets |
| Universal converter | Convert between supported formats (Markdown to PDF, CSV to XLSX, etc.) |
| AI digest | Generate summaries and key points from any document |
| Batch processing | Process multiple files simultaneously |
| Format routing | Route files to the appropriate Forge tool (Writer, Sheets, PDF) |

**Rust module:** `file_processor.rs`
**Frontend:** `/routes/files/+page.svelte`

---

### Phase 4 -- Team (4 Modules)

#### 23. ForgeTeam

Team management and collaboration infrastructure. Create teams, invite members via codes, manage roles, and track member status.

| Capability | Detail |
|-----------|--------|
| Team creation | Create and configure team workspaces |
| Invite codes | Generate shareable invite codes for team joining |
| Member management | Roles (admin, member, viewer), status tracking |
| Team settings | Shared configuration, branding, permissions |
| Activity feed | Real-time team activity stream |

**Rust module:** `forge_team.rs`
**Frontend:** `/routes/team/+page.svelte`

#### 24. ImpBook (Shared Knowledge Workspace)

A shared workspace for team knowledge management. Team members create entries, leave comments, add reactions, and share AI agent configurations.

| Capability | Detail |
|-----------|--------|
| Entries | Create and organize knowledge entries (Markdown) |
| Comments | Threaded comments on entries |
| Reactions | Emoji reactions for quick feedback |
| Agent sharing | Share custom AI agent configurations across the team |
| Search | Full-text search across all team knowledge |
| Categories | Tag-based organization with custom taxonomies |

**Rust module:** Part of `forge_team.rs`
**Frontend:** Integrated into `/routes/team/+page.svelte`

#### 25. Team Chat

Real-time messaging for team communication. Direct messages and channels with message history, search, and file sharing.

| Capability | Detail |
|-----------|--------|
| Direct messages | One-to-one messaging |
| Channels | Topic-based group channels |
| Message history | Full message persistence with search |
| File sharing | Share files directly in chat |
| Notifications | Desktop notifications for new messages |
| Mentions | @mention team members for attention |

**Rust module:** Part of `forge_team.rs`
**Frontend:** Integrated into `/routes/team/+page.svelte`

#### 26. Team Goals

Milestone and progress tracking for team objectives. Define goals, break them into milestones, and track completion across the team.

| Capability | Detail |
|-----------|--------|
| Goal creation | Define team objectives with descriptions |
| Milestones | Break goals into trackable milestones |
| Progress tracking | Visual progress bars, completion percentage |
| Assignments | Assign milestones to team members |
| Deadlines | Due dates with overdue alerts |
| Analytics | Goal completion velocity and trend tracking |

**Rust module:** Part of `forge_team.rs`
**Frontend:** Integrated into `/routes/team/+page.svelte`

---

### Additional Capabilities (Cross-Cutting)

| Feature | Module | Description |
|---------|--------|-------------|
| News Feed | `news_feed.rs` | AI/Dev news aggregator (RSS/Atom feeds, offline-capable) |
| System Agent | `system_agent.rs` | Background system monitoring and health checks |
| Theme Engine | `theme_engine.rs` | ElvUI/BenikUI-inspired deep UI customization |
| Widget Registry | `widget_registry.rs` | Modular dashboard components for layout manager |
| Style Engine | `style_engine.rs` | Sub-component level styling (every widget decomposes) |
| Settings | `settings.rs` | Centralized configuration management |
| Sunshine | `sunshine.rs` | Moonlight remote access manager (game streaming) |
| Evaluation | `evaluation/mod.rs` | Model and agent quality evaluation framework |
| Convergence | `/routes/convergence/` | Unified search and AI convergence interface |

---

## 4. Technical Architecture

### Stack Overview

| Layer | Technology | Version | License | Role |
|-------|-----------|---------|---------|------|
| Desktop Framework | Tauri | 2.10 | MIT + Apache-2.0 | Window management, IPC, native APIs |
| Frontend | Svelte 5 + SvelteKit | 5.51 | MIT | Reactive UI with runes |
| UI Components | shadcn-svelte + Tailwind CSS | Latest | MIT | Design system |
| Backend | Rust | 1.80+ | - | Business logic, AI, data |
| Database | SQLite via rusqlite | 0.36 | MIT | Bundled, WAL mode |
| AI Runtime (Local) | Ollama (sidecar) | Latest | MIT | Local model serving |
| AI Runtime (Local) | llama.cpp via llama-cpp-2 | 0.1 | MIT | Direct GGUF inference |
| AI Runtime (Local) | Candle | 0.9 | Apache-2.0 | HuggingFace model inference |
| Embeddings | FastEmbed (ONNX) | 5.12 | Apache-2.0 | Local embedding generation |
| Vector Search | HNSW (custom) + Qdrant + LanceDB | - | MIT/Apache-2.0 | Similarity search |
| AI Providers (Cloud) | llm + genai crates | 1.3 / 0.6 | MIT | 15+ cloud providers |
| Docker | Bollard | 0.18 | MIT | Docker Engine API |
| Git | git2 (libgit2) | 0.20 | MIT | Git operations |
| Browser Automation | chromiumoxide | 0.9 | MIT/Apache-2.0 | CDP control |
| HTML Processing | scraper + fast_html2md | 0.25/0.58 | ISC/MIT | Parsing + conversion |
| Spreadsheets | calamine + rust_xlsxwriter | 0.26/0.79 | MIT | XLSX read/write |
| Terminal | portable-pty | 0.9 | MIT | Real PTY support |
| AST Parsing | tree-sitter + 28 grammars | 0.26 | MIT | Code understanding |
| Serialization | serde + rmp-serde | 1.0/1.3 | MIT | JSON + MessagePack |
| Cron | croner | 3 | MIT | Task scheduling |
| Concurrency | parking_lot | 0.12 | MIT/Apache-2.0 | High-performance mutexes |
| System Info | sysinfo + systemstat | 0.38/0.2 | MIT | CPU/Memory/Disk monitoring |
| GPU (NVIDIA) | nvml-wrapper | 0.12 | MIT | NVIDIA GPU monitoring |
| GPU (AMD) | libamdgpu_top | 0.11 | MIT | AMD GPU monitoring |
| Compression | zstd | 0.13 | MIT | Profile export compression |
| Crypto | hmac + sha2 | 0.12/0.10 | MIT/Apache-2.0 | Integrity verification |
| Tokenization | tokenizers | 0.22 | Apache-2.0 | HuggingFace tokenizers |
| Text Chunking | text-splitter | 0.29 | MIT | Semantic RAG chunking |
| GitHub API | octocrab | 0.49 | Apache-2.0 | GitHub REST API client |

### Tauri IPC Architecture

All frontend-backend communication uses Tauri's IPC command system. The frontend calls Rust functions via `invoke()`, and the backend responds with typed results. Streaming responses use Tauri events.

```
Svelte 5 Frontend                    Rust Backend
+-----------------+                  +------------------+
| invoke('cmd',   | ---- IPC ----->  | #[tauri::command] |
|   { args })     |                  | async fn cmd()    |
|                 |                  |   -> Result<T>    |
| on('event',     | <--- Event ---   | app.emit('event', |
|   callback)     |                  |   payload)        |
+-----------------+                  +------------------+
```

**250+ Tauri commands** span all 26 modules, organized by module:

| Module Group | Approximate Command Count |
|-------------|--------------------------|
| Chat / Router / AI | ~30 |
| CodeForge IDE | ~40 |
| NeuralSwarm / Orchestrator | ~35 |
| Docker | ~15 |
| GitHub | ~15 |
| Browser / CDP | ~25 |
| ForgeMemory | ~30 |
| Social / Freelancer / Publisher | ~25 |
| Office Suite (Writer/Sheets/PDF/Canvas/Slides) | ~35 |
| ForgeTeam / ImpBook | ~15 |
| Settings / Theme / Monitoring | ~15 |

### Frontend Architecture (Svelte 5)

ImpForge uses Svelte 5 with runes exclusively. No legacy stores. No `writable()`.

```svelte
<!-- Mandatory Svelte 5 pattern -->
<script>
  import { invoke } from '@tauri-apps/api/core';

  let result = $state('');
  let loading = $state(false);
  let items = $state([]);
  let count = $derived(items.length);

  $effect(() => {
    console.log('Count changed:', count);
  });

  async function fetchData() {
    loading = true;
    result = await invoke('get_data', { query: 'test' });
    loading = false;
  }
</script>
```

**SvelteKit configuration for Tauri:**
```typescript
// src/routes/+layout.ts
export const ssr = false;       // MANDATORY for Tauri
export const prerender = false; // No static generation
```

### Route Architecture (26 Pages)

| Route | Module |
|-------|--------|
| `/` | Home / Dashboard |
| `/chat` | Chat / Terminal UI |
| `/ide` | CodeForge IDE |
| `/agents` | NeuralSwarm Agents |
| `/docker` | Docker Dashboard |
| `/github` | GitHub Integration |
| `/apps` | App Launcher |
| `/apps/[id]` | App Detail View |
| `/browser` | Browser Agent |
| `/ai` | AI Models Management |
| `/n8n` | n8n Workflow Integration |
| `/evaluation` | Model Evaluation |
| `/social` | Social Media Hub |
| `/freelancer` | Freelancer Hub |
| `/platforms` | Platforms Manager |
| `/mail` | ForgeMail |
| `/writer` | ForgeWriter |
| `/sheets` | ForgeSheets |
| `/pdf` | ForgePDF |
| `/canvas` | ForgeCanvas |
| `/slides` | ForgeSlides |
| `/files` | File Hub |
| `/team` | ForgeTeam / ImpBook / Chat / Goals |
| `/news` | News Feed |
| `/settings` | Settings |
| `/convergence` | Convergence Search |

### Database Architecture (SQLite, WAL Mode)

ImpForge uses SQLite exclusively. No PostgreSQL, no Redis, no external database of any kind. The database is bundled and created on first launch.

```rust
// Database initialization (rusqlite, WAL mode)
fn init_db() -> Result<Connection> {
    let conn = Connection::open("impforge.db")?;
    conn.execute_batch("
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-64000;
    ")?;
    Ok(conn)
}
```

**Key tables:**
- `conversations` / `messages` -- Chat history
- `forge_memory_*` -- ForgeMemory knowledge graph, embeddings, BM25 index
- `agents` / `worker_trust` / `task_queue` -- NeuralSwarm state
- `settings` -- User configuration
- `documents` / `sheets` / `slides` -- Office document storage
- `team_*` -- Team, members, goals, messages

### Error Handling

Typed errors throughout the Rust backend using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
enum ImpForgeError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("AI inference failed: {0}")]
    Inference(String),
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}
```

### Cross-Platform Support

| Platform | WebView | GPU Detection | Terminal |
|----------|---------|---------------|----------|
| Linux | WebKitGTK | AMD (libamdgpu_top) + NVIDIA (NVML) | bash/zsh via PTY |
| Windows | WebView2 (Edge) | NVIDIA (NVML) + DirectX fallback | PowerShell via PTY |
| macOS | WKWebView | Apple Silicon (Metal) | zsh via PTY |

All file paths use `std::path::PathBuf` and `dirs` crate for cross-platform directory resolution. No hardcoded `/` paths.

---

## 5. Sovereign Office Suite

### Vision

The Sovereign Office Suite is ImpForge's answer to Microsoft 365 and Google Workspace. Every application is AI-native from the ground up, meaning AI is not a sidebar add-on but a core part of every interaction. All data stays on the user's machine. All formats are interoperable. No subscriptions required.

### Clean Room Implementation

Every component of the Sovereign Office Suite was built from scratch in Rust:

| Component | Clean Room Approach | Key Crate |
|-----------|-------------------|-----------|
| ForgeSheets | Custom formula evaluator, no Excel reverse-engineering | `calamine` (read), `rust_xlsxwriter` (write) |
| ForgeWriter | Markdown AST rendering, not a Word clone | Custom Markdown parser |
| ForgePDF | Rust-native PDF rendering and text extraction | Custom PDF engine |
| ForgeCanvas | Original spatial document concept | Custom canvas engine |
| ForgeSlides | Markdown-to-slides pipeline | Custom slide renderer |
| File Hub | Format router, not a file manager | `calamine`, `csv`, `zip` |

### AI Integration in Every Tool

| Tool | AI Capability |
|------|--------------|
| ForgeWriter | Rewrite, expand, summarize, translate, generate from outline |
| ForgeSheets | Natural language to formula, data analysis, pattern detection |
| ForgePDF | Summarize document, answer questions, extract structured data |
| ForgeCanvas | Generate outputs from linked source documents |
| ForgeSlides | Generate entire presentations from topic or outline |
| File Hub | Auto-digest any document, intelligent format routing |

### Format Support Matrix

| Format | Read | Write | AI Analysis |
|--------|------|-------|-------------|
| Markdown (.md) | Yes | Yes | Yes |
| XLSX (.xlsx) | Yes | Yes | Yes |
| XLS (.xls) | Yes | - | Yes |
| ODS (.ods) | Yes | - | Yes |
| XLSB (.xlsb) | Yes | - | Yes |
| CSV (.csv) | Yes | Yes | Yes |
| PDF (.pdf) | Yes | - | Yes |
| HTML (.html) | Yes | Yes | Yes |
| Plain Text (.txt) | Yes | Yes | Yes |
| DOCX (.docx) | Yes (via ZIP) | - | Yes |
| PPTX (.pptx) | Yes (via ZIP) | - | Yes |
| JSON (.json) | Yes | Yes | Yes |
| YAML (.yaml) | Yes | Yes | Yes |
| Code (28 langs) | Yes | Yes | Yes |

---

## 6. AI and Neuroscience Architecture

### ForgeMemory -- Custom AI Memory Engine

ForgeMemory is ImpForge's proprietary knowledge management engine. It combines multiple retrieval strategies to provide context-aware AI responses grounded in the user's actual project data.

```
+------------------------------------------------------------------+
|                        FORGEMEMORY ENGINE                         |
+------------------------------------------------------------------+
|                                                                  |
|  +-----------+  +----------+  +----------+  +-----------+        |
|  | BM25      |  | HNSW     |  | Knowledge|  | MemGPT    |        |
|  | Inverted  |  | Vector   |  | Graph    |  | Tiered    |        |
|  | Index     |  | Search   |  | (Entity  |  | Memory    |        |
|  | (Snowball)|  | (FastEmb)|  |  +Rel)   |  | (Hot/Cold)|        |
|  +-----------+  +----------+  +----------+  +-----------+        |
|       |              |             |              |               |
|  +----+--------------+-------------+--------------+----+          |
|  |              HYBRID RETRIEVAL FUSION                |          |
|  |     BM25 scores + vector similarity + graph hops   |          |
|  +-----------------------------------------------------+         |
|                         |                                         |
|  +-----------------------------------------------------+         |
|  |           CONTEXT WINDOW ASSEMBLY                    |         |
|  |  Ranked chunks + source attribution + relevance      |         |
|  +-----------------------------------------------------+         |
|                         |                                         |
|  +-----------------------------------------------------+         |
|  |              LLM PROMPT INJECTION                    |         |
|  |  System prompt + context + user query = grounded AI  |         |
|  +-----------------------------------------------------+         |
|                                                                  |
+------------------------------------------------------------------+
```

**Modules (20 Rust files):**

| Module | Responsibility |
|--------|---------------|
| `engine.rs` | Core ForgeMemory orchestration |
| `store.rs` | SQLite persistence layer |
| `embeddings.rs` | FastEmbed ONNX embedding generation |
| `vector.rs` | HNSW vector index for similarity search |
| `bm25.rs` | BM25 inverted index (Snowball stemming, Robertson & Zaragoza 2009) |
| `search.rs` | Hybrid retrieval fusion (BM25 + vector + graph) |
| `graph.rs` | Knowledge graph (entities, relationships, traversal) |
| `memory.rs` | MemGPT-inspired tiered memory (hot/warm/cold) |
| `ingest.rs` | Document ingestion pipeline |
| `chunk_context.rs` | Context-aware chunking for RAG |
| `context.rs` | Context window assembly for LLM prompts |
| `digest.rs` | AI-powered document summarization |
| `nlp.rs` | NLP utilities (tokenization, entity extraction) |
| `llm_extract.rs` | LLM-based structured data extraction |
| `watch.rs` | File system watcher for automatic re-indexing |
| `migration.rs` | Schema migration management |
| `tree_sitter_langs.rs` | 28-language AST-aware code chunking (cAST, arXiv:2506.15655) |
| `commands.rs` | Tauri IPC command layer |
| `mod.rs` | Module organization |

**Supported tree-sitter grammars (28 languages):**
Rust, Python, JavaScript, TypeScript, C, C++, Go, Java, C#, Ruby, PHP, Swift, Scala, Lua, R, Julia, Elixir, Bash, HTML, CSS, JSON, YAML, XML, GraphQL, Zig, Kotlin, Svelte, TOML, Markdown, Dart, SQL, Vue

### Cascade Router -- Intelligent Model Selection

The Cascade Router classifies user intent and selects the optimal AI provider (local or cloud) based on task complexity, available resources, and user preferences.

```
User Input
    |
    v
+-------------------+
| Fast Classifier   |  <-- Rule-based keyword + pattern matching
| (classifier.rs)   |     No LLM call, <1ms latency
+-------------------+
    |
    v
+-------------------+
| Task Type         |  Code, Chat, Creative, Analysis, Search, System
+-------------------+
    |
    v
+-------------------+
| Target Selection  |  Local Ollama (free) -> OpenRouter (paid) -> Direct API
| (targets.rs)      |  Considers: VRAM, model size, task complexity
+-------------------+
    |
    v
+-------------------+
| Execute           |  Streaming or batch, with conversation context
+-------------------+
```

### NeuralSwarm Orchestrator -- Neuroscience-Inspired Multi-Agent System

The NeuralSwarm Orchestrator is ImpForge's autonomous agent coordination system. It draws from computational neuroscience research to create a self-organizing, self-healing multi-agent system.

#### Three-Factor Hebbian Trust Model

Trust between agents is computed using a biologically-inspired formula:

```
delta_w = eta * dopamine * novelty * homeostasis

Where:
  eta         = learning rate (0.01)
  dopamine    = reward signal (task success/failure)
  novelty     = information novelty factor
  homeostasis = system stability factor (prevents runaway trust)
```

**Scientific basis:** Izhikevich (2007) "Solving the Distal Reward Problem through Linkage of STDP and Dopamine Signaling"; Gerstner et al. (2018) "Eligibility Traces and Plasticity on Behavioral Time Scales"; arXiv:2504.05341.

**Rust module:** `orchestrator/trust.rs`

#### MAPE-K Self-Healing Control Loop

```
+----------+     +---------+     +------+     +---------+
| Monitor  | --> | Analyze | --> | Plan | --> | Execute |
| (Metrics)|    | (Detect) |    |(Strategy)| | (Act)   |
+----------+     +---------+     +------+     +---------+
     ^                                              |
     |              +------------+                  |
     +--------------| Knowledge  |<-----------------+
                    | (Shared DB)|
                    +------------+
```

- **Monitor**: Collect metrics from all 42 workers (latency, error rate, throughput)
- **Analyze**: Detect anomalies, degradation, failures
- **Plan**: Select recovery strategy (restart, reassign, scale, circuit-break)
- **Execute**: Apply the recovery action
- **Knowledge**: Shared state in SQLite for cross-cycle learning

**Rust module:** `orchestrator/health.rs`

#### FSRS-5 Spaced Repetition (Brain v2.0)

The Brain v2.0 module uses FSRS-5 (Free Spaced Repetition Scheduler, version 5) for intelligent knowledge retention scheduling. This is the same algorithm used by Anki, adapted for AI agent knowledge management.

**Paper:** "A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition Scheduling" (Ye et al., 2024)

**Rust module:** `inference/fsrs_scheduler.rs`

#### Mixture-of-Agents (MoA) Pipeline

Multi-model inference pipeline where multiple local/cloud models generate responses that are aggregated for higher quality output.

**Paper basis:** arXiv:2601.16596 -- "Mixture-of-Agents Enhances Large Language Model Capabilities"

**Rust module:** `orchestrator/moa_pipeline.rs`

#### Agent-as-Judge Evaluation

Quality assessment system where AI agents evaluate each other's outputs, providing calibrated quality scores without human labeling.

**Paper basis:** arXiv:2504.05341

**Rust module:** `orchestrator/evaluation.rs`

#### Topology and Scaling

| Component | Algorithm | Reference |
|-----------|-----------|-----------|
| Agent topology | Dynamic graph with connection strengths | arXiv:2602.06039 |
| Auto-scaling | AIMD (Additive Increase / Multiplicative Decrease) | Chiu & Jain (1989) |
| Resource governance | Circuit breakers, semaphore pools | Nygard (2007) |
| Git operations | Conventional Commits, SemVer 2.0.0 | conventionalcommits.org |
| CI/CD pipeline | 5-stage (lint, test, build, package, deploy) | - |

**Rust modules:** `orchestrator/topology.rs`, `orchestrator/agent_scaling.rs`, `orchestrator/resource_governor.rs`, `orchestrator/git_ops.rs`, `orchestrator/ci_cd.rs`

### AI Provider Matrix

ImpForge supports 15+ AI providers through three unified crate interfaces:

| Provider | Crate | Local/Cloud | Streaming | Function Calling |
|----------|-------|-------------|-----------|-----------------|
| Ollama | `llm`, direct HTTP | Local | Yes | Yes |
| llama.cpp | `llama-cpp-2` | Local | Yes | - |
| Candle (HF) | `candle-*` | Local | Yes | - |
| OpenRouter | `llm`, `genai` | Cloud | Yes | Yes |
| OpenAI | `async-openai` | Cloud | Yes | Yes |
| Anthropic | `anthropic-sdk-rust` | Cloud | Yes | Yes |
| Mistral | `mistralai-client` | Cloud | Yes | Yes |
| Google Gemini | `genai` | Cloud | Yes | Yes |
| Groq | `genai`, `llm` | Cloud | Yes | Yes |
| Cohere | `genai` | Cloud | Yes | Yes |
| xAI (Grok) | `genai` | Cloud | Yes | - |
| DeepSeek | `genai` | Cloud | Yes | - |
| Together AI | `llm` | Cloud | Yes | Yes |
| Perplexity | `llm` | Cloud | Yes | - |
| Fireworks AI | `llm` | Cloud | Yes | Yes |

---

## 7. Competitive Analysis

### vs. AI-First IDEs

| Feature | ImpForge | Cursor | Windsurf | GitHub Copilot |
|---------|----------|--------|----------|----------------|
| Offline AI inference | Yes (Ollama, llama.cpp, Candle) | No | No | No |
| Local model support | Yes (any GGUF/HF model) | No | No | No |
| Multi-provider routing | Yes (15+ providers) | GPT-4/Claude only | GPT-4/Claude only | GPT-4 only |
| Docker management | Yes (built-in) | No | No | No |
| GitHub integration | Yes (built-in) | Extension | No | Yes |
| Agent orchestration | Yes (42 workers) | No | No | No |
| Browser automation | Yes (CDP) | No | No | No |
| Office suite | Yes (5 apps) | No | No | No |
| Social media | Yes (6 platforms) | No | No | No |
| Team collaboration | Yes (built-in) | No | No | No |
| Pricing (core) | Free / $9-29/mo | $20/mo | $15/mo | $10/mo |
| Data sovereignty | 100% local | Cloud-dependent | Cloud-dependent | Cloud-dependent |

### vs. AI Development Platforms

| Feature | ImpForge | LM Studio | Jan.ai | Msty |
|---------|----------|-----------|--------|------|
| IDE | Yes (15-panel) | No | No | No |
| Agent system | Yes (42 workers) | No | No | No |
| Office suite | Yes (5 apps) | No | No | No |
| Docker | Yes | No | No | No |
| GitHub | Yes | No | No | No |
| Social media | Yes | No | No | No |
| Freelancer CRM | Yes | No | No | No |
| Team collab | Yes | No | No | No |
| Local inference | Yes | Yes | Yes | Yes |
| Model management | Yes (HF Hub) | Yes | Yes | Yes |
| Vector/RAG | Yes (ForgeMemory) | No | Basic | No |
| Knowledge graph | Yes | No | No | No |

### vs. Office Suites

| Feature | ImpForge Sovereign | Microsoft 365 | Google Workspace | LibreOffice |
|---------|--------------------|---------------|------------------|-------------|
| AI-native | Every tool | Copilot add-on | Gemini add-on | None |
| Offline | 100% | Limited | No | Yes |
| Local data | Yes | OneDrive | Google Drive | Yes |
| Subscription | Optional (Pro+) | $6-22/user/mo | $6-18/user/mo | Free |
| IDE included | Yes | No | No | No |
| Agent system | Yes | No | No | No |
| Code editor | Yes | VS Code separate | No | No |
| XLSX support | Read + Write | Native | Native | Yes |
| PDF tools | Built-in | Acrobat separate | Limited | Limited |
| Presentations | ForgeSlides | PowerPoint | Slides | Impress |
| Spreadsheet | ForgeSheets | Excel | Sheets | Calc |

### Unique Market Position

ImpForge occupies a unique intersection that no competitor addresses:

```
                    AI IDE
                      |
                      |
         Cursor ------+------ ImpForge
                      |          |
                      |          |
    LM Studio --------+----------+-------- Microsoft 365
                      |          |
                      |          |
                  Local AI    Office Suite
```

ImpForge is the only product that combines a full IDE, local AI inference, office productivity, social media automation, freelancer CRM, and team collaboration into a single, offline-first desktop application.

---

## 8. Pricing Strategy

### Tier Structure

| Tier | Price | Target | Key Features |
|------|-------|--------|-------------|
| **Free** | $0/forever | Students, hobbyists | Chat, basic IDE, 1 local model, ForgeWriter |
| **Solo** | $9/mo | Solo developers | Full IDE, unlimited local models, ForgeMemory, Docker |
| **Pro** | $19/mo | Professional devs | All Phase 1-2 modules, cloud AI routing, Social Media Hub |
| **Business** | $29/mo | Freelancers, SMBs | Full Office Suite, Freelancer Hub, Auto-Publisher |
| **Team** | $15/user/mo | Small teams | All modules + ForgeTeam, ImpBook, Team Chat, Goals |
| **Enterprise** | Custom | Organizations | SSO, audit logs, priority support, custom integrations |

### Revenue Model

1. **Subscription tiers** (primary revenue) -- recurring monthly/annual
2. **Cloud AI credits** (usage-based) -- pay-per-token for cloud model access
3. **Enterprise licenses** (high-value) -- volume licensing for organizations
4. **Marketplace** (future) -- community extensions and agent templates

### Key Pricing Principles

- **Core AI stays free**: Local inference with Ollama is always free. Users never pay to use models they run themselves.
- **No feature gates on data**: Users own their data regardless of tier. Export is never restricted.
- **Annual discount**: 20% discount for annual commitment on all tiers.
- **Education**: Free Pro tier for verified students and educators.

---

## 9. Legal Framework

### Licensing Strategy

ImpForge uses a dual-license model:

| Component | License | Rationale |
|-----------|---------|-----------|
| `src-tauri/src/` (Application) | Apache-2.0 | Maximum community adoption, compatible with commercial use |
| `crates/impforge-engine/` (Engine) | BUSL-1.1 | Protects core IP (Neural Trust, Brain v2.0, Cascade Router, MAPE-K) while allowing inspection |

**BUSL-1.1 (Business Source License 1.1):**
- Source code is fully visible and auditable
- Free for non-production use (development, testing, education)
- Commercial use requires a license (included in Pro+ tiers)
- Converts to Apache-2.0 after the change date (typically 3-4 years)

### Dependency License Compliance

Every dependency in ImpForge has been verified for commercial distribution compatibility. Only the following licenses are permitted:

| Allowed License | Count in Dependency Tree |
|----------------|------------------------|
| MIT | ~180 crates |
| Apache-2.0 | ~60 crates |
| BSD-2-Clause / BSD-3-Clause | ~15 crates |
| ISC | ~5 crates |
| MPL-2.0 | ~3 crates |
| Unlicense | ~2 crates |

**Prohibited licenses (NEVER used):**
- GPL (any version) -- copyleft incompatible with commercial distribution
- AGPL -- network copyleft, even more restrictive
- LGPL -- dynamic linking requirements conflict with static Rust builds
- SSPL -- Server Side Public License (MongoDB-style)
- CC-BY-SA -- Share-alike for documentation

### EU AI Act Compliance

ImpForge is designed for compliance with the EU AI Act (Regulation 2024/1689):

| Requirement | ImpForge Implementation |
|------------|------------------------|
| Transparency | All AI outputs are labeled as AI-generated |
| Human oversight | Approval queue for Auto-Publisher; user confirms all automated actions |
| Data governance | All data local; no training on user data; clear data lineage |
| Technical documentation | Full architecture documentation (this document) |
| Risk classification | General-purpose AI system (limited risk category) |
| Record keeping | Full audit trail in SQLite for all AI decisions |

### GDPR / DSGVO Compliance

| Principle | Implementation |
|-----------|---------------|
| Data minimization | No telemetry, no usage tracking, no cloud data by default |
| Right to erasure | Single command deletes all user data (SQLite file) |
| Data portability | Export all data as JSON/CSV/Markdown |
| Privacy by design | Offline-first architecture means data never leaves the machine |
| Consent | Cloud AI providers require explicit opt-in |
| Processing records | All AI processing logged locally with timestamps |

### Clean Room Development

The Sovereign Office Suite was developed using clean room methodology:

1. **No reverse engineering**: ForgeSheets formula engine was built from the OpenDocument specification, not by reverse-engineering Excel
2. **No proprietary format parsing**: XLSX reading uses the MIT-licensed `calamine` crate which implements the OOXML standard
3. **No GPL contamination**: All code and dependencies verified by automated license scanning
4. **Documentation**: All design decisions documented with rationale and sources

### Security

| Measure | Implementation |
|---------|---------------|
| VPN/Tunnel | boringtun (WireGuard, MIT) for team P2P connectivity |
| API key storage | Tauri plugin-store (encrypted local storage) |
| Network security | rustls-tls (no OpenSSL dependency) |
| Integrity | HMAC-SHA256 for profile export verification |
| Compression | zstd for secure, efficient data transport |
| No telemetry | Zero data collection -- verifiable by source audit |

---

## 10. Target Market

### Total Addressable Market (TAM)

| Segment | Global Market Size (2026) | Source |
|---------|--------------------------|--------|
| Developer Tools & IDEs | $22.7B | Gartner, 2025 |
| AI Development Platforms | $45.3B | MarketsandMarkets, 2025 |
| Office Productivity Software | $55.8B | Fortune Business Insights, 2025 |
| Freelancer Management Tools | $8.2B | Grand View Research, 2025 |
| **Combined TAM** | **~$132B** | |

### Serviceable Addressable Market (SAM)

ImpForge targets the intersection of developers and knowledge workers who:
- Prefer offline-first or privacy-conscious tools
- Use AI in their daily workflow
- Are dissatisfied with tool fragmentation (multiple subscriptions)
- Work as freelancers, solo developers, or in small teams (<50 people)

| Segment | SAM Estimate |
|---------|-------------|
| Privacy-conscious developers using AI tools | $3.2B |
| Freelancers seeking integrated business tools | $1.8B |
| Small teams seeking affordable alternatives to M365 | $2.5B |
| **Combined SAM** | **~$7.5B** |

### Serviceable Obtainable Market (SOM) -- Year 1-3

| Year | Target Users | Revenue Target | Strategy |
|------|-------------|----------------|----------|
| Y1 | 5,000 | $450K ARR | Developer community, open source presence |
| Y2 | 25,000 | $3.5M ARR | Product Hunt, dev conferences, content marketing |
| Y3 | 100,000 | $18M ARR | Enterprise sales, team adoption, marketplace |

### Target Personas

| Persona | Age | Pain Point | ImpForge Solution |
|---------|-----|-----------|-------------------|
| **Indie Developer** | 22-35 | Too many tools, expensive AI subscriptions | All-in-one, local AI is free |
| **Freelance Developer** | 25-45 | Separate tools for code, clients, invoices, social | Unified workflow from code to invoice |
| **Tech Lead** | 30-50 | Team coordination across scattered tools | ForgeTeam + shared ImpBook + Goals |
| **Privacy Advocate** | 25-55 | Cloud tools mine user data | 100% offline, zero telemetry |
| **EU Enterprise** | - | GDPR compliance, data sovereignty | Local-only data, EU AI Act ready |
| **Content Creator** | 20-40 | Manual posting across platforms | Social Media Hub + Auto-Publisher |
| **Student** | 18-28 | Expensive tools, learning curve | Free tier, adaptive onboarding |
| **SMB Knowledge Worker** | 28-55 | Microsoft 365 costs per seat | Sovereign Office Suite at fraction of cost |

### Geographic Focus

| Priority | Region | Rationale |
|----------|--------|-----------|
| 1 | DACH (Germany, Austria, Switzerland) | Home market, GDPR-native, strong dev community |
| 2 | EU / EEA | Data sovereignty resonance, AI Act alignment |
| 3 | North America | Largest dev tool market, AI early adopters |
| 4 | Asia-Pacific | Growing developer population, cost-sensitive |

---

## 11. Key Metrics and KPIs

### Codebase Metrics (v0.7.0)

| Metric | Value |
|--------|-------|
| Total Lines of Code | ~130,000+ |
| Rust Backend LoC | ~90,000 |
| Svelte/TypeScript Frontend LoC | ~40,000 |
| Rust Source Files (src-tauri/src/) | 95+ |
| Svelte Components | 200+ |
| Route Pages | 26 |
| Tauri IPC Commands | 250+ |
| Feature Modules | 26 |
| Crate Dependencies | ~260 |
| Tree-sitter Grammars | 28 languages |

### Test Suite

| Category | Count |
|----------|-------|
| Unit tests (Rust) | ~850 |
| Integration tests (Rust) | ~100 |
| Engine tests (impforge-engine) | ~62 |
| **Total passing** | **1,012** |

### Build Metrics

| Metric | Value |
|--------|-------|
| Rust edition | 2021 |
| Minimum Rust version | 1.80.0 |
| Tauri version | 2.10 |
| Svelte version | 5.51 |
| Debug build time | ~3-5 minutes (cold) |
| Release build time | ~8-12 minutes (cold) |
| Binary size (Linux, release) | ~45-60 MB |

### Quality Gates

All of the following must pass before any release:

| Gate | Command | Status |
|------|---------|--------|
| Rust linting | `cargo clippy` | Zero warnings |
| Rust tests | `cargo test` | 1,012 passing |
| Svelte type check | `pnpm check` | Clean |
| Svelte lint | `pnpm lint` | Clean |
| License audit | `cargo license` | No GPL/AGPL/LGPL |
| Cross-platform build | CI matrix (Linux, Windows, macOS) | Passing |

### ForgeMemory Metrics

| Metric | Value |
|--------|-------|
| Embedding model | FastEmbed (ONNX, local) |
| Embedding dimensions | 384 (MiniLM) / 1024 (BGE) configurable |
| BM25 algorithm | Snowball stemming (Robertson & Zaragoza 2009) |
| Vector index | HNSW (custom Rust implementation) |
| Supported languages | 28 (via tree-sitter) |
| Chunking strategy | AST-aware (cAST, arXiv:2506.15655) |
| Memory tiers | Hot / Warm / Cold (MemGPT-inspired) |

### Performance Targets

| Operation | Target Latency | Actual |
|-----------|---------------|--------|
| Router classification | <1ms | <0.5ms |
| ForgeMemory search (BM25) | <10ms | ~5ms |
| ForgeMemory search (vector) | <50ms | ~30ms |
| ForgeMemory search (hybrid) | <100ms | ~60ms |
| Local model inference (first token) | <500ms | ~300ms (8B model) |
| App cold start | <3s | ~2s |
| SQLite write (WAL) | <1ms | ~0.3ms |

---

## Appendix A: Scientific References

| Paper / Source | Application in ImpForge |
|---------------|------------------------|
| Izhikevich (2007). "Solving the Distal Reward Problem through Linkage of STDP and Dopamine Signaling" | Three-Factor Hebbian Trust model |
| Gerstner et al. (2018). "Eligibility Traces and Plasticity on Behavioral Time Scales" | Neuromodulated trust dynamics |
| arXiv:2504.05341 | Agent-as-Judge evaluation framework |
| arXiv:2601.16596 | Mixture-of-Agents (MoA) pipeline |
| arXiv:2602.06039 | Dynamic agent topology |
| arXiv:2506.15655 | cAST: AST-aware code chunking for RAG |
| Robertson & Zaragoza (2009). "The Probabilistic Relevance Framework: BM25 and Beyond" | BM25 search in ForgeMemory |
| Ye et al. (2024). "A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition Scheduling" | FSRS-5 in Brain v2.0 |
| Chiu & Jain (1989). "Analysis of the Increase and Decrease Algorithms for Congestion Avoidance" | AIMD agent auto-scaling |
| Nygard (2007). "Release It!" | Circuit breaker pattern |
| Miller (2021). "Building a StoryBrand" | Content generation framework |

---

## Appendix B: Repository Structure

```
impforge-workstation/
|
+-- src-tauri/                     # Rust backend (Apache-2.0)
|   +-- src/
|   |   +-- main.rs               # Entry point
|   |   +-- lib.rs                 # Module registration, Tauri setup
|   |   +-- error.rs              # Typed error hierarchy
|   |   +-- chat.rs               # Chat / Terminal UI
|   |   +-- ollama.rs             # Ollama API client
|   |   +-- router/               # Cascade Router (classifier, targets)
|   |   +-- ai/                   # AI provider abstraction
|   |   +-- agents/               # Agent definitions
|   |   +-- ide/                  # CodeForge IDE (12 sub-modules)
|   |   +-- orchestrator/         # NeuralSwarm (14 sub-modules)
|   |   +-- forge_memory/         # ForgeMemory engine (20 files)
|   |   +-- inference/            # AI inference (GGUF, Candle, FSRS, Rig)
|   |   +-- docker/               # Docker integration
|   |   +-- github/               # GitHub integration
|   |   +-- browser_agent.rs      # Browser Agent
|   |   +-- cdp_engine.rs         # CDP browser control
|   |   +-- cdp_network.rs        # CDP network monitoring
|   |   +-- cdp_devtools.rs       # CDP DevTools (console, perf)
|   |   +-- web_scraper.rs        # Built-in web scraper
|   |   +-- browser.rs            # Browser UI integration
|   |   +-- browser_import.rs     # Bookmark/history import
|   |   +-- social.rs             # Social Media Hub
|   |   +-- freelancer.rs         # Freelancer Hub
|   |   +-- auto_publisher.rs     # Auto-Publisher
|   |   +-- forge_mail.rs         # ForgeMail
|   |   +-- forge_writer.rs       # ForgeWriter
|   |   +-- forge_sheets.rs       # ForgeSheets
|   |   +-- forge_pdf.rs          # ForgePDF
|   |   +-- forge_canvas.rs       # ForgeCanvas
|   |   +-- forge_slides.rs       # ForgeSlides
|   |   +-- file_processor.rs     # File Hub
|   |   +-- forge_team.rs         # ForgeTeam / ImpBook / Chat / Goals
|   |   +-- app_launcher.rs       # App Launcher
|   |   +-- system_agent.rs       # System monitoring agent
|   |   +-- neuralswarm.rs        # NeuralSwarm UI integration
|   |   +-- theme_engine.rs       # Theme customization
|   |   +-- style_engine.rs       # Sub-component styling
|   |   +-- widget_registry.rs    # Dashboard widget registry
|   |   +-- settings.rs           # Configuration management
|   |   +-- news_feed.rs          # News aggregator
|   |   +-- sunshine.rs           # Remote access (Moonlight)
|   |   +-- serialization.rs      # MessagePack serialization
|   |   +-- traits/               # Shared trait definitions
|   |   +-- monitoring/           # System monitoring
|   |   +-- evaluation/           # Model evaluation
|   |   +-- monitoring_quick.rs   # Quick sysfs-based monitoring
|   +-- Cargo.toml                # Dependencies (~85 direct crates)
|
+-- crates/
|   +-- impforge-engine/           # Core AI engine (BUSL-1.1)
|       +-- src/                   # Neural Trust, Brain v2.0, MAPE-K
|       +-- Cargo.toml
|
+-- src/                           # Svelte 5 frontend (Apache-2.0)
|   +-- routes/                    # 26 page routes
|   |   +-- +page.svelte          # Home / Dashboard
|   |   +-- chat/                 # Chat UI
|   |   +-- ide/                  # CodeForge IDE
|   |   +-- agents/               # NeuralSwarm dashboard
|   |   +-- docker/               # Docker dashboard
|   |   +-- github/               # GitHub integration
|   |   +-- apps/                 # App Launcher
|   |   +-- browser/              # Browser Agent
|   |   +-- ai/                   # AI Models Management
|   |   +-- n8n/                  # n8n Workflows
|   |   +-- evaluation/           # Evaluation
|   |   +-- social/               # Social Media Hub
|   |   +-- freelancer/           # Freelancer Hub
|   |   +-- platforms/            # Platforms Manager
|   |   +-- mail/                 # ForgeMail
|   |   +-- writer/               # ForgeWriter
|   |   +-- sheets/               # ForgeSheets
|   |   +-- pdf/                  # ForgePDF
|   |   +-- canvas/               # ForgeCanvas
|   |   +-- slides/               # ForgeSlides
|   |   +-- files/                # File Hub
|   |   +-- team/                 # ForgeTeam
|   |   +-- news/                 # News Feed
|   |   +-- settings/             # Settings
|   |   +-- convergence/          # Convergence Search
|   +-- lib/
|       +-- components/
|           +-- ui/               # shadcn-svelte components (90+)
|
+-- docs/                          # Documentation
+-- specs/                         # OpenSpec SDD specifications
```

---

## Appendix C: Contact and Legal

**Company:** AiImp Development
**Founder:** Karsten Schildgen
**Country:** Germany
**Repository:** github.com/TiKcoc/impforge-workstation
**License:** Apache-2.0 (Application) + BUSL-1.1 (Engine)
**Classification:** Confidential -- For authorized parties only

This document contains proprietary information about ImpForge AI Workstation Builder. Distribution is restricted to investors, partners, and enterprise customers under NDA. Unauthorized distribution or reproduction is prohibited.

---

*Document version: 0.7.0 | Last updated: March 15, 2026 | Generated for internal and partner review*
