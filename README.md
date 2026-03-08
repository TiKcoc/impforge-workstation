<div align="center">

# NEXUS — AI Workstation Builder

**The all-in-one AI-powered desktop workstation for developers.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.83+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.10-purple.svg)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-5-ff3e00.svg)](https://svelte.dev/)

[Features](#features) · [Install](#installation) · [Screenshots](#screenshots) · [Architecture](#architecture) · [Contributing](CONTRIBUTING.md)

</div>

---

## What is NEXUS?

NEXUS is a **standalone AI workstation** that brings together chat, code intelligence, browser automation, Docker management, and GitHub integration — all in one native desktop app. Built with Rust + Tauri for performance, Svelte 5 for a reactive UI, and designed to work **offline-first** with local LLMs.

## Features

### Intelligent Model Router
1. The Intelligent Chat Router feature ensures that prompts are directed to the most suitable model - either a local Ollama or cloud-based one. This maximizes efficiency by streamlining the development process and minimizing unnecessary steps.

2. Built-in Browser with DevTools equips developers with a CDP-powered browser for seamless debugging and testing. It includes essential tools like the network waterfall, console, and more, all accessible within the AI Workstation desktop app.

3. The Customizable UI feature allows developers to tailor their workspace according to their preferences using an ElvUI-inspired theme engine. With a drag-drop layout and widget system, users can create a highly efficient and personalized environment for maximum productivity.

### Built-in Browser Engine
Full Chrome DevTools Protocol integration: live page preview, network waterfall, console output, cookie management, and AI-powered element selection.

### Customizable UI
ElvUI-inspired theme engine with WCAG 2.2 contrast validation, drag-and-drop layout manager, and a widget registry for building your perfect workspace.

### More Features
- **Docker Dashboard** — Monitor and control containers directly
- **GitHub Integration** — Issues, PRs, repos at a glance
- **Agent System** — Create and manage AI agents with custom prompts
- **Web Scraper** — Built-in scraping with metadata extraction
- **Evaluation Chain** — Agent-as-a-Judge quality scoring
- **System Health** — Real-time CPU, GPU, memory monitoring

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Backend** | Rust, Tauri 2.10, tokio |
| **Frontend** | Svelte 5, TypeScript, TailwindCSS |
| **AI** | Ollama, HuggingFace Hub, llama.cpp, Candle |
| **Browser** | chromiumoxide (CDP), reqwest |
| **Database** | SQLite (bundled, WAL mode) |
| **Build** | Cargo, pnpm, GitHub Actions |

## Metrics

| Metric | Value |
|--------|-------|
| Lines of Code | ~335,589 |
| Tauri Commands | 138 |
| Rust Modules | 23 |
| Test Coverage | 144 tests |
| Version | v0.5.1 |

## Installation

### Pre-built Binaries (Recommended)

Download from [Releases](https://github.com/AiImpDevelopment/nexus-workstation/releases):
- **Linux** — `.deb` (Ubuntu/Debian) or `.AppImage` (universal)
- **Windows** — `.msi` installer
- **macOS** — `.dmg` (Intel + Apple Silicon)

### Build from Source

```bash
# Prerequisites: Rust 1.83+, Node.js 20+, pnpm
git clone https://github.com/AiImpDevelopment/nexus-workstation.git
cd nexus-workstation/Nexus
pnpm install
cargo tauri build --release
```

## Screenshots

> Screenshots coming soon — see [Features](#features) for capabilities.

## Architecture

```
┌─────────────────────────────────────────┐
│           Svelte 5 Frontend             │
│  Chat · Browser · Docker · GitHub · IDE │
├─────────────────────────────────────────┤
│         Tauri 2 IPC Bridge              │
├─────────────────────────────────────────┤
│           Rust Backend                  │
│  Router · Orchestrator · CDP Engine     │
│  Agents · Scraper · Theme Engine        │
├─────────────────────────────────────────┤
│     SQLite · Ollama · HuggingFace       │
└─────────────────────────────────────────┘
```

## License

[MIT](LICENSE) — Free for personal and commercial use.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

---

<div align="center">
Built with Rust + Tauri + Svelte by <a href="https://github.com/AiImpDevelopment">AiImp Development</a>
</div>