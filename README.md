<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="NEXUS Logo" />
</p>

<h1 align="center">NEXUS AI Workstation</h1>

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

## What is NEXUS?

NEXUS is a native desktop application that puts an entire AI development stack at your fingertips. Chat with 28+ free AI models, manage Docker containers, orchestrate agents, automate workflows, and write code -- all from a single, beautiful Opera GX-inspired interface.

No cloud dependencies required. NEXUS works fully offline with local models, or connects to cloud providers when you want more power.

## Features

### Intelligent Model Router
Automatically routes your prompts to the best available model based on task type. Code questions go to coding models, creative tasks to creative models -- all without manual switching. **Zero API cost by default** using free models.

### Multi-Model Chat
- 28+ free models via OpenRouter (Llama 4, Devstral, Gemma 3, and more)
- Local models via Ollama (Qwen 2.5 Coder, Dolphin 3, Hermes 3)
- SSE streaming with real-time token display
- Conversation history and model switching

### NeuralSwarm Agent System
A pool of specialized AI agents that work together:
- **Orchestrator** -- Decomposes complex tasks into subtasks
- **Code Agent** -- Generates, reviews, and refactors code
- **Research Agent** -- Searches docs, papers, and the web
- **Debug Agent** -- Traces errors and suggests fixes
- **DevOps Agent** -- Manages infrastructure and CI/CD
- **Review Agent** -- Audits code quality and security

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
| AI | OpenRouter API, Ollama, SSE streaming |
| Data | tauri-plugin-store, local persistence |

## Screenshots

*Coming soon*

## Contributing

NEXUS is currently in active development. If you're interested in contributing, please reach out.

## License

MIT
