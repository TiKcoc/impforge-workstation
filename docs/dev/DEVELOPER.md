# NEXUS Developer Guide

> **Internal document** -- For TiKcoc team members only.

## Quick Start

```bash
# Prerequisites: Rust, Node 20+, pnpm 9+
cd Nexus
pnpm install
pnpm tauri dev
```

## Architecture Overview

```
Nexus/
├── src-tauri/src/         # Rust backend (Tauri commands)
│   ├── lib.rs             # App entry, command registration (36 commands)
│   ├── router/            # Intelligent Model Router
│   │   ├── mod.rs         # Keyword classifier (<10ms)
│   │   ├── targets.rs     # Model selection logic
│   │   └── streaming.rs   # SSE + NDJSON streaming
│   ├── agents/            # Agent CRUD (NeuralSwarm)
│   ├── docker/            # Docker Engine API client
│   ├── github/            # GitHub REST API client
│   ├── ide/               # CodeForge filesystem + agent tools
│   ├── inference/         # HuggingFace Hub + GGUF loading
│   ├── monitoring_quick/  # Pure sysfs system stats
│   └── settings/          # tauri-plugin-store persistence
├── src/                   # SvelteKit 5 frontend
│   ├── lib/
│   │   ├── components/ui/ # shadcn-svelte components
│   │   ├── stores/        # Svelte 5 Runes reactive stores
│   │   └── agents/        # Frontend agent message handlers
│   └── routes/            # 9 routes (chat, github, docker, agents, ide, ai, n8n, news, settings)
└── docs/
    ├── dev/               # This directory -- internal docs
    ├── plans/             # Implementation plans
    └── research/          # Research papers & competitive analysis
```

## Key Patterns

### Svelte 5 Runes Store Pattern
```typescript
// Class-based reactive store (preferred pattern)
class MyStore {
    data = $state<Data[]>([]);
    loading = $state(false);
    computed = $derived(this.data.length);
}
export const myStore = new MyStore();
```

### Tauri Command Pattern
```rust
#[tauri::command]
async fn my_command(arg: String) -> Result<MyType, String> {
    // All errors as String for frontend
    do_work(&arg).map_err(|e| e.to_string())
}
// Register in lib.rs invoke_handler
```

### Intelligent Router
- Keyword classifier in `router/mod.rs` -- classifies prompts into task types
- Target selection in `router/targets.rs` -- picks best model per task
- Free models preferred (OpenRouter free tier + local Ollama)
- Streaming via Tauri events (`chat-stream` event name)

## Environment Variables

| Variable | Purpose | Required |
|---|---|---|
| `OPENROUTER_API_KEY` | OpenRouter API access | No (free models work without) |
| `GITHUB_TOKEN` | GitHub API (higher rate limits) | No |

## Build & Release

```bash
# Type check
pnpm check

# Rust check
cd src-tauri && cargo check

# Production build
pnpm tauri build
```

CI/CD: `.github/workflows/test-build.yml` runs on every push. Release workflow builds platform binaries on tag push.

## Current Stats
- 0 errors, 2 warnings (pnpm check)
- 11 warnings (cargo check, all pre-existing dead code)
- 151 frontend files, 19 Rust files
- 36 Tauri commands registered

## Contacts
- **Lead**: TiKcoc (GitHub)
- **Dev**: Christof Treitges (christof.treitges@outlook.de)
