# ImpForge Production-Readiness Plan

> **Goal**: Make ImpForge feel like JetBrains, not Cursor-alpha. Every feature works, every UI is polished, zero crashes.

**Date**: 2026-03-09
**Codebase**: 52,574 Rust LoC + 24,463 Svelte/TS LoC = ~77,000 LoC
**Build Status**: 0 errors (both cargo check + svelte-check)
**Warnings**: 17 Svelte a11y warnings, ~20 Cargo warnings

---

## TIER 1: CRITICAL (Crash Prevention + Core Functionality)

### T1.1: Eliminate unwrap()/expect() from production Rust code
- **668 occurrences** across 55 files
- Top offenders: forge_memory (350+), orchestrator (100+), theme_engine (29), serialization (8)
- Convert to proper `Result<T, E>` with meaningful error messages
- **Impact**: Every unwrap() is a potential app crash

### T1.2: Wire IDE placeholder actions
- `src/routes/ide/+page.svelte:102-114` — 5 placeholder comments:
  - Toggle file explorer (not wired)
  - Format document (not wired)
  - Go to line (not wired)
  - Find in files (not wired)
  - New file creation (not wired)
- **Impact**: Core IDE features that do nothing when clicked

### T1.3: Wire agent backend calls
- `src/lib/agents/index.ts:70-85` — 3 TODO stubs:
  - `runAgent()` — returns empty
  - `stopAgent()` — returns empty
  - `getAgentLogs()` — returns empty
- **Impact**: Agent page looks functional but does nothing

### T1.4: Replace evaluation mock data fallback
- `src/routes/evaluation/+page.svelte:149-158` — `generateMockResult()` used as fallback
- Should show proper error state when backend unavailable
- **Impact**: Users see fake data instead of real evaluation

### T1.5: Replace hardcoded news feed
- `src/routes/news/+page.svelte:33-143` — 10 hardcoded NewsItem entries
- Wire to actual RSS/API feed or remove page
- **Impact**: Static content in a dynamic app looks broken

### T1.6: Wire ONNX/GGUF inference TODOs
- `src-tauri/src/router/targets.rs:84,88` — TODO: local ONNX inference, image generation
- `src-tauri/src/inference/gguf.rs:85,100` — TODO: llama-cpp-2 model init + inference
- **Impact**: Model router can't actually run local models

---

## TIER 2: HIGH (Stability + Polish)

### T2.1: Fix all 17 Svelte a11y warnings
- `Mover.svelte:140` — noninteractive tabIndex
- `IdeTerminal.svelte:182` — button-in-button nesting
- `CommandPalette.svelte:231,260` — click without keyboard handler
- `PricingPanel.svelte:309` — button without label
- `ThemeImporter.svelte:595,597` — quoted component attributes
- `settings/+page.svelte` — 10x label-without-control
- **Impact**: Accessibility compliance, SSR hydration warnings

### T2.2: Standardize error handling pattern across all Tauri commands
- Current: mix of `.map_err(|e| e.to_string())` and raw strings
- Target: Unified `AppError` type with error codes, user-friendly messages
- **Impact**: Consistent error experience across entire app

### T2.3: Add loading/error/empty states to all route pages
- Many pages go from "loading" straight to content with no error UI
- Need: Skeleton loaders, error boundaries, empty states with CTAs
- Routes to check: ai, docker, github, agents, evaluation, n8n, browser
- **Impact**: Professional polish — no blank screens or silent failures

### T2.4: Unify color system — IDE panels vs routes
- IDE components use hardcoded colors: `#161b22`, `#00FF66`, `white/10`
- Route pages use BenikUI: `bg-gx-bg-secondary`, `text-gx-neon`
- Files: AiAgent.svelte, FileExplorer.svelte, DebugPanel.svelte, CollabPanel.svelte, CommandPalette.svelte, IdeTerminal.svelte
- **Impact**: Theme switching won't work for IDE panels

### T2.5: Cargo warnings cleanup
- Profile warnings in workspace Cargo.toml
- Unused import/variable warnings
- **Impact**: Clean build output = professional codebase

---

## TIER 3: MEDIUM (Feature Completion)

### T3.1: Complete Chat UI (3x3x3 system)
- See `docs/plans/2026-03-09-chat-terminal-browser-ui-design.md`
- 6 gaps: Block renderer, Enhanced input, Split stream, Mission control, Module registry, Token bar
- **Impact**: Core differentiator feature incomplete

### T3.2: Docker page — verify all Tauri commands work
- Container list, pull, start, stop, remove, logs
- Need real Docker daemon connection testing
- **Impact**: Core DevOps feature

### T3.3: GitHub page — verify OAuth and repo operations
- Clone, pull, push, branch management
- **Impact**: Core development feature

### T3.4: Browser Agent — verify CDP connection
- Navigation, element interaction, JavaScript execution, screenshot
- **Impact**: Web automation feature

### T3.5: Settings persistence — verify all settings actually save/load
- Theme, API keys, Ollama URL, model preferences
- Test: Change setting → restart app → verify persisted
- **Impact**: Settings that don't persist = broken UX

---

## TIER 4: NICE-TO-HAVE (JetBrains Quality)

### T4.1: Keyboard shortcuts for all major actions
- Global: Ctrl+P (command palette), Ctrl+Shift+P, Ctrl+B (sidebar)
- IDE: Ctrl+S (save), Ctrl+Z (undo), F5 (run), F12 (go to definition)
- Chat: Enter (send), Shift+Enter (newline), Escape (cancel)

### T4.2: Window management — remember size/position
- Save window dimensions on close, restore on open
- Multi-monitor support

### T4.3: Onboarding / First-run experience
- Welcome screen for new users
- Setup wizard: Ollama, API keys, workspace

### T4.4: About dialog with version, licenses, system info

### T4.5: Auto-update mechanism via Tauri updater

---

## Quick Stats

| Category | Count | Status |
|----------|-------|--------|
| unwrap()/expect() | 668 | Must fix |
| TODO/FIXME in Rust | 7 | Must resolve |
| TODO in TypeScript | 3 | Must resolve |
| Placeholder UI actions | 5 | Must wire |
| Mock data fallbacks | 2 | Must replace |
| a11y warnings | 17 | Must fix |
| Hardcoded colors in IDE | ~6 files | Must unify |
| Routes with real backend | ~8/13 | Must verify all |

## Execution Order

1. **T1.1** unwrap() elimination (biggest crash risk)
2. **T2.1** a11y warnings (quick wins, 0-error build)
3. **T1.2-T1.4** Wire placeholders (features that look broken)
4. **T2.4** Color unification (theme consistency)
5. **T2.3** Loading/error states (polish)
6. **T1.5-T1.6** Backend TODOs (feature completion)
7. **T3.x** Full feature verification
8. **T4.x** JetBrains-level polish
