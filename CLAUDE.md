# ImpForge — AI Workstation Builder

Native desktop application. Tauri 2.10 (Rust backend), Svelte 5 (frontend with runes), SQLite (rusqlite WAL), Ollama (local AI). ~135K LOC, 1,063 tests, 30 modules.

## Repository map
- `src-tauri/src/` — Rust backend (487 #[tauri::command] handlers)
  - `chat.rs` + `ollama.rs` — Chat streaming (Ollama local + OpenRouter cloud)
  - `forge_writer.rs` / `forge_sheets.rs` / `forge_pdf.rs` / `forge_slides.rs` — Sovereign Office
  - `forge_canvas.rs` — 3-panel AI document workspace
  - `forge_notes.rs` — Knowledge base with [[wiki-links]]
  - `forge_calendar.rs` — Calendar with ICS parser (RFC 5545)
  - `forge_mail.rs` — Email client with AI compose
  - `forge_team.rs` — ForgeTeam + ImpBook (P2P collaboration)
  - `social.rs` / `freelancer.rs` / `auto_publisher.rs` — Business automation
  - `app_launcher.rs` — External app integration
  - `auto_import.rs` / `file_processor.rs` — Universal file handling
  - `orchestrator/` — NeuralSwarm (42 workers, Hebbian Trust, MAPE-K)
  - `forge_memory/` — ForgeMemory engine (BM25 + HNSW + Knowledge Graph)
  - `inference/` — GGUF, Candle, Rig router, FSRS-5
  - `ide/` — CodeForge IDE (LSP, Git, Debug, Terminal, DB Client)
  - `agents/` — Agent management with runtime state
  - `error.rs` — Unified AppError with ImpForgeError
  - `lib.rs` — Module registration, Tauri setup
- `crates/impforge-engine/` — Core AI engine (BUSL-1.1)
- `src/` — Svelte 5 frontend
  - `src/routes/` — 27 page routes
  - `src/lib/components/` — 200+ Svelte components
  - `src/lib/stores/` — 26 reactive stores ($state, $derived)

## Commands
- `cd src-tauri && cargo check` — Verify Rust compilation
- `cd src-tauri && cargo test` — Run all Rust tests
- `cd src-tauri && cargo clippy` — Rust lint
- `pnpm tauri dev` — Full dev mode (Rust + Svelte hot-reload)
- `pnpm check` — svelte-check TypeScript diagnostics

## Verification after code changes
After ANY Rust edit: `cargo check`. After ANY Svelte/TS edit: verify no type errors. Before committing: `cargo test --workspace`.

## Rust conventions
- `ImpForgeError` from `error.rs` for all errors — NO unwrap() or expect() outside tests
- `AppResult<T>` = `Result<T, ImpForgeError>` for Tauri commands
- All #[tauri::command] handlers registered in `lib.rs` generate_handler![]
- Use `pub(crate)` over `pub` when possible

## Svelte 5 conventions
- Runes ONLY: `$state`, `$derived`, `$effect` — NEVER legacy `$:` syntax
- Frontend calls Rust via `invoke()` from `@tauri-apps/api/core`
- BenikUI style engine: `widgetId` + `styleEngine.getComponentStyle()` on every page
- Opera GX dark theme: `bg-gx-*`, `text-gx-*`, `border-gx-*` classes

## Architecture rules
- ImpForge is STANDALONE — NO dependencies on ork-station, NeuralSwarm, or ImpUI
- Offline-first: Ollama preferred, cloud as fallback
- All data in SQLite (rusqlite WAL) or JSON files in ~/.impforge/
- Cross-platform: Linux, Windows, macOS — no systemd, no platform-specific hacks
- License compliance: ONLY MIT/Apache-2.0/BSD deps — NEVER GPL/AGPL

## What NOT to do
- Never reference /opt/ork-station/ paths in ImpForge code
- Never import ork-station Python modules or MCP servers
- Never add GPL dependencies
- Never use .unwrap() in production Rust code
- Never use legacy Svelte $: syntax
