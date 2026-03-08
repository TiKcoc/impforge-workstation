# Video Analysis & Research Summary — Rust, Svelte, Enterprise Development

**Date**: 2026-03-08
**Source**: 5 YouTube Videos + Web Research
**Purpose**: Best practices for Nexus AI Workstation Builder (Tauri 2 + Svelte 5 + Rust)

---

## Video 1: Hugo in 100 Seconds (Fireship)

**URL**: https://www.youtube.com/watch?v=0RKpf3rK57I
**Channel**: Fireship | **Duration**: 2:33 | **Views**: 610K

### Key Takeaways
- Hugo is the fastest static site generator (sub-millisecond rendering, written in Go)
- Uses Go templates for dynamic content interpolation
- File-system-based routing and content management
- Taxonomy system for menus, tags, categories without plugins
- Used by Cloudflare Docs, Smashing Magazine, Fireship.io

### Relevance to Nexus
- **Documentation site**: Hugo could generate Nexus documentation/landing page
- **Pattern**: File-system-based routing is similar to SvelteKit's approach
- **License**: Apache-2.0 — commercially usable
- **Decision**: Use SvelteKit (already in stack) instead of Hugo for Nexus docs

---

## Video 2: OpenSpec — Spec-Driven Development (WorldofAI)

**URL**: https://www.youtube.com/watch?v=gHkdrO6IExM
**Channel**: WorldofAI | **Duration**: 11:57 | **Views**: 133K

### Key Takeaways
- **OpenSpec** replaces "vibe coding" with structured, spec-driven development
- Core loop: Specify → Plan → Execute → Verify
- Three pillars: **Intent** (what+why), **Constraints** (tech stack+patterns), **Acceptance Criteria** (testable conditions)
- "Brownfield-first" strategy — designed for evolving existing codebases, not just greenfield
- No API keys needed, lightweight, works with 20+ AI tools
- Separates "current truth" from "proposed updates" — auditable scope changes
- Better than Spec Kit for ongoing development (Spec Kit is best for new projects)

### Relevance to Nexus (CRITICAL)
- **MUST ADOPT for Nexus development** — spec-driven approach prevents AI hallucinations
- Use OpenSpec proposals for every Nexus feature change
- Structure: `/specs/current/` (truth) + `/specs/proposals/` (changes)
- **GitHub**: https://github.com/Fission-AI/OpenSpec (MIT License)
- **Best with**: Claude Opus 4.6 for planning, Sonnet 4.6 for implementation

### SDD Tool Comparison (2026)
| Tool | Best For | License |
|------|----------|---------|
| **OpenSpec** | Brownfield changes, evolving codebases | MIT |
| **Spec Kit** | New projects from scratch | MIT |
| **GSD** | Deep execution orchestration | MIT |
| **Taskmaster AI** | Task decomposition in Cursor | MIT |

---

## Video 3: Productive Arch Linux Setup (Oscar)

**URL**: https://www.youtube.com/watch?v=o03_cfOnl84
**Channel**: Oscar | **Duration**: 8:31 | **Views**: 610K

### Key Takeaways
- Productivity-focused Linux desktop with i3 (tiling WM), kitty terminal, Neovim
- Tools: dunst (notifications), feh (wallpaper), picom (compositor), flameshot (screenshots)
- Rose Pine GTK theme, NerdFonts, FiraCode/JetBrains Mono fonts
- System maintenance scripts for Arch (pacman cache, orphan cleanup)
- "The best way to be productive is to enjoy your work" — customize your tools
- Open source software = free + customizable

### Relevance to Nexus
- **ImpOS already implements this concept** — tiling WM (GTK4 + Relm4)
- **Fonts**: Already using FiraCode + JetBrains Mono in ImpOS
- **Pattern**: Profile-based desktop configs (Coding, Gaming, Creative)
- **Nexus angle**: Nexus users may want similar productivity setups → Setup Wizard

### Tools Referenced (All Open Source)
- i3wm (MIT), kitty (GPL-3.0), dunst (BSD), picom (MIT)
- Neovim (Apache-2.0), feh (MIT), flameshot (GPL-3.0)
- lxappearance, redshift, n3 file manager

---

## Video 4: SvelteKit in 100 Seconds (Fireship)

**URL**: https://www.youtube.com/watch?v=H1eEFfAkIik
**Channel**: Fireship | **Duration**: 2:45 | **Views**: 505K

### Key Takeaways
- SvelteKit = metaframework for server-rendered Svelte apps (like Next.js for React)
- Created by Rich Harris, v1.0 in 2022, now at Svelte 5 with runes
- **Key features**:
  - File-system-based routing (pages, layouts, server files)
  - Load functions for data fetching with automatic type safety
  - Progressive enhancement (forms work without JS, enhanced with JS)
  - Layouts share UI across child routes with own data fetching
  - Server files export GET/POST/PATCH/DELETE for RESTful APIs
  - Uses Vite as build tool
- Svelte store for state management (accessed from any component)

### Relevance to Nexus (CRITICAL)
- **Nexus uses SvelteKit** — this is our core frontend framework
- **Tauri integration**: Must disable SSR (`ssr = false` in root layout)
- **Pattern**: Use `+page.ts` for client-side data, `+page.server.ts` only for SSR
- **State management**: Svelte 5 runes ($state, $derived, $effect) replace stores
- **Forms**: Progressive enhancement pattern for offline-first
- **Routing**: File-system routing maps perfectly to Nexus page structure

### Svelte 5 Runes (Current Best Practice)
```svelte
<script>
  let count = $state(0);
  let doubled = $derived(count * 2);
  $effect(() => { console.log(count); });
</script>
```

---

## Video 5: Build Your Entire Tech Stack in Rust (Let's Get Rusty)

**URL**: https://www.youtube.com/watch?v=luOgEhLE2sg
**Channel**: Let's Get Rusty | **Duration**: 7:22 | **Views**: 272K

### Key Takeaways
- **Rusty Stack**: Rocket + SurrealDB + Tauri + Yew = full Rust stack
- **Rocket**: Convention-over-configuration web framework, macro-based routing
- **SurrealDB**: Multi-paradigm DB (relational + graph + document), SurrealQL ≈ SQL
- **Tauri**: Desktop apps with web frontend + Rust backend (smaller, faster than Electron)
- **Yew**: React-like component framework compiled to WASM
- All four components use Rust → single language for entire application
- GitHub: https://github.com/letsgetrusty/rsty-stack-example

### Relevance to Nexus (CRITICAL)
- **Nexus already uses Tauri** — confirmed as optimal choice
- **Backend decision**: Nexus uses embedded Rust (no separate web server needed)
- **Database**: Using SQLite (rusqlite) instead of SurrealDB — simpler, no server
- **Frontend**: Using Svelte 5 instead of Yew — better DX, faster iteration
- **Key insight**: Tauri bridges Rust ↔ Web seamlessly via IPC commands

### Framework Comparison for Nexus
| Component | Video Choice | Nexus Choice | Reason |
|-----------|-------------|--------------|--------|
| Backend | Rocket | Tauri IPC | No separate server needed |
| Database | SurrealDB | SQLite (rusqlite) | Embedded, no daemon |
| Desktop | Tauri | Tauri 2.10 | Same — optimal |
| Frontend | Yew (WASM) | Svelte 5 | Better DX, faster dev |

---

## Web Research: Latest Best Practices (2025-2026)

### Tauri 2 + Svelte 5 Production Patterns

1. **Disable SSR**: Add `export const ssr = false` in root `+layout.ts`
2. **SPA mode**: No prerendering — load functions run only in webview
3. **Tauri APIs**: Access via `@tauri-apps/api` — window, fs, shell, dialog
4. **Binary size**: Tauri apps are 600KB-2MB vs Electron's 150MB+
5. **WebView differences**: WebKit (Linux/macOS) vs WebView2 (Windows) — test cross-platform
6. **Sidecar pattern**: Bundle additional binaries (Ollama, llama.cpp) with Tauri app

### Rust Web Framework Rankings (2026)

| Framework | Stars | Performance | DX | Use Case |
|-----------|-------|------------|-----|----------|
| **Axum** | 20K+ | Excellent | Good | APIs, microservices |
| **Actix Web** | 22K+ | Best | Medium | High-performance APIs |
| **Rocket** | 24K+ | Good | Best | Rapid prototyping |
| **Leptos** | 18K+ | Excellent | Good | Full-stack Rust web apps |
| **Yew** | 31K+ | Good | Good | React-like WASM apps |
| **Dioxus** | 22K+ | Good | Good | Cross-platform (web+desktop+mobile) |

### OpenSpec SDD Workflow for Nexus

```
nexus/
├── specs/
│   ├── current/           # Current truth (what IS built)
│   │   ├── orchestrator.md
│   │   ├── chat-system.md
│   │   └── docker-manager.md
│   └── proposals/         # Proposed changes (what WILL change)
│       ├── 001-ai-stack-wizard.md
│       └── 002-worker-implementations.md
├── .openspec/
│   ├── config.yaml        # OpenSpec configuration
│   └── templates/         # Spec templates
```

---

## Actionable Recommendations for Nexus

### MUST DO (Immediate)
1. **Adopt OpenSpec** for all Nexus feature development (MIT, free, no API keys)
2. **Keep Tauri 2 + Svelte 5** — confirmed as best stack by all sources
3. **SQLite (rusqlite)** over SurrealDB — embedded, no daemon dependency
4. **Disable SSR** in SvelteKit config for Tauri
5. **Use Svelte 5 runes** ($state, $derived, $effect) instead of stores

### SHOULD DO (Soon)
6. **Sidecar pattern** for bundling Ollama with Nexus
7. **Progressive enhancement** for forms (works offline, enhanced with JS)
8. **Cross-platform testing** (WebKit vs WebView2 differences)
9. **shadcn-svelte** for UI components (MIT license, Tailwind-based)
10. **Automated CI/CD** for Windows/Linux/macOS builds

### CONSIDER (Future)
11. **Leptos** for any Rust-native UI components that need WASM performance
12. **Dioxus** for mobile companion app (cross-platform from single codebase)
13. **SurrealDB** if Nexus needs graph queries (KG integration)

---

## Open Source Frameworks — Commercial License Check

| Framework | License | Commercial Use | Notes |
|-----------|---------|---------------|-------|
| **Tauri** | MIT + Apache-2.0 | YES | Core framework |
| **Svelte/SvelteKit** | MIT | YES | Frontend framework |
| **rusqlite** | MIT | YES | SQLite bindings |
| **Ollama** | MIT | YES | LLM runtime |
| **OpenSpec** | MIT | YES | Spec-driven dev |
| **shadcn-svelte** | MIT | YES | UI components |
| **Tailwind CSS** | MIT | YES | Utility CSS |
| **tokio** | MIT | YES | Async runtime |
| **serde** | MIT + Apache-2.0 | YES | Serialization |
| **reqwest** | MIT + Apache-2.0 | YES | HTTP client |
| **FastEmbed** | Apache-2.0 | YES | Local embeddings |
| **llama.cpp** | MIT | YES | LLM inference |
| **Rocket** | MIT + Apache-2.0 | YES | Web framework (if needed) |
| **Hugo** | Apache-2.0 | YES | Static site gen (docs only) |

**ALL frameworks are MIT or Apache-2.0 — safe for commercial distribution.**

---

## Research Papers & Sources

### Spec-Driven Development
- [OpenSpec GitHub](https://github.com/Fission-AI/OpenSpec) — MIT License
- [GitHub Blog: Spec-Driven Development](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/)
- [SDD Tool Comparison 2026](https://medium.com/@richardhightower/agentic-coding-gsd-vs-spec-kit-vs-openspec-vs-taskmaster-ai-where-sdd-tools-diverge-0414dcb97e46)

### Tauri + Svelte
- [Tauri v2 SvelteKit Guide](https://v2.tauri.app/start/frontend/sveltekit/)
- [Tauri 2 + Svelte 5 + shadcn Template](https://github.com/alysonhower/tauri2-svelte5-shadcn)
- [Evil Martians: Tauri + Rust Sidecar](https://evilmartians.com/chronicles/making-desktop-apps-with-revved-up-potential-rust-tauri-sidecar)

### Rust Frameworks
- [Rust Web Frameworks 2026](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2)
- [Rust Web Framework Comparison](https://github.com/flosse/rust-web-framework-comparison)
- [Leptos GitHub](https://github.com/leptos-rs/leptos)
- [Rusty Stack Example](https://github.com/letsgetrusty/rsty-stack-example)

### Video Sources
1. [Hugo in 100 Seconds](https://www.youtube.com/watch?v=0RKpf3rK57I) — Fireship
2. [OpenSpec: NEW Toolkit Ends Vibe Coding](https://www.youtube.com/watch?v=gHkdrO6IExM) — WorldofAI
3. [An Actually Productive Arch Linux Setup](https://www.youtube.com/watch?v=o03_cfOnl84) — Oscar
4. [SvelteKit in 100 Seconds](https://www.youtube.com/watch?v=H1eEFfAkIik) — Fireship
5. [Build Your Entire Tech Stack in Rust](https://www.youtube.com/watch?v=luOgEhLE2sg) — Let's Get Rusty
