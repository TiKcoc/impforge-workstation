# NEXUS CDP Browser Engine — Design Document

**Date**: 2026-03-08
**Status**: Phase 1 COMPLETE, Phase 2-3 PLANNED
**License**: MIT/Apache-2.0 (all dependencies verified)

## Overview

Full browser automation for NEXUS using Chrome DevTools Protocol (CDP) via chromiumoxide (Rust-native, MIT/Apache-2.0). Replaces the HTTP-only BrowserEngine with dual-mode CDP+HTTP for complete browser control.

## Architecture

### Phase 1: CDP Browser Engine (DONE)

```
                    NEXUS Browser System
    ┌─────────────────────────────────────────────────┐
    │  browser/+page.svelte (5 tabs, Opera GX theme)  │
    │  ┌──────┬──────────┬────────┬────────┬────────┐ │
    │  │Browse│Playground│AI Agent│ Import │Webhooks│ │
    │  └──┬───┴────┬─────┴───┬────┴───┬────┴───┬────┘ │
    │     │        │         │        │        │       │
    │  cdp.svelte  │  agent  │  import│  webhook│      │
    │  .ts store   │  store  │  store │  store  │      │
    └─────┼────────┼─────────┼────────┼────────┼──────┘
          │ Tauri IPC        │        │        │
    ┌─────┼────────┼─────────┼────────┼────────┼──────┐
    │  cdp_engine  │  browser│browser │browser │      │
    │  .rs         │  _agent │_import │_agent  │      │
    │              │  .rs    │.rs     │.rs     │      │
    │  ┌───────────┼─────────┼────────┘        │      │
    │  │ chromiumoxide (CDP) │ HTTP fallback    │      │
    │  │ ┌──────┐  ┌──────┐ │ ┌──────┐         │      │
    │  │ │Page 1│  │Page 2│ │ │reqwest│         │      │
    │  │ └──────┘  └──────┘ │ └──────┘         │      │
    │  └───────────┼────────┘                   │      │
    │              │                            │      │
    │     Chrome/Brave/Chromium (CDP WebSocket)  │      │
    └────────────────────────────────────────────┘
```

### Key Decisions

1. **Lazy Browser Init**: `tokio::sync::OnceCell<CdpState>` — browser launches on first CDP request, stays alive for app lifetime
2. **Handler Pattern**: chromiumoxide requires a WebSocket handler in a separate `tokio::spawn` task
3. **Dual-Mode**: CDP actions fall back to HTTP when no browser is installed (graceful degradation)
4. **Pages as Sessions**: Each CDP Page maps to a session ID, stored in `HashMap<String, Page>` behind `Mutex`
5. **Browser Detection**: Cross-platform scanning of known install paths (no registry on Linux, LOCALAPPDATA on Windows, /Applications on macOS)

### Files Created/Modified

| File | Lines | Purpose |
|------|-------|---------|
| `cdp_engine.rs` | ~380 | CDP engine, browser detection, 12 Tauri commands |
| `browser_import.rs` | ~380 | Profile detection, bookmark/history import, 4 commands |
| `browser_agent.rs` | Modified | CDP-aware execute_action with fallback |
| `cdp.svelte.ts` | ~220 | CDP store (pages, navigate, click, fill, screenshot, JS) |
| `browser-import.svelte.ts` | ~140 | Import store (profiles, bookmarks, history) |
| `browser/+page.svelte` | ~650 | 5-tab Opera GX UI |

### Dependencies

| Crate | Version | License | Purpose |
|-------|---------|---------|---------|
| chromiumoxide | 0.9.1 | MIT/Apache-2.0 | CDP browser control |
| base64 | 0.22 | MIT/Apache-2.0 | Screenshot encoding |
| rusqlite | 0.36 | MIT | Browser data reading |
| scraper | 0.25 | ISC | HTML parsing (HTTP fallback) |
| fast_html2md | 0.0.58 | MIT | Markdown conversion |

### Test Coverage

- 80 total tests (12 new: 5 CDP + 7 import)
- 0 Svelte errors

## Phase 2: Opera GX Browser Playground UI (PLANNED)

- Live page preview in Tauri WebView
- Visual element picker (click-to-select CSS selectors)
- Network waterfall (CDP network events)
- Console output panel
- Performance metrics (CDP Performance domain)
- Cookie viewer/editor

## Phase 3: Modular Component System (PLANNED)

- ElvUI/BenikUI-inspired theme engine
- Layout Manager (drag-and-drop widget arrangement)
- Widget Registry (registerWidget, createWidget)
- Profile Export/Import (zstd + base64, ElvUI pattern)
- Custom CSS variable overrides
- Component library (Button, Card, Panel, TabBar, StatusBar)

## Scientific References

- OpAgent (arXiv 2602.13559): Planner-Grounder-Reflector-Summarizer
- BrowserAgent (arXiv 2510.10666): Think-Summarize-Act with memory
- WALT (arXiv 2510.01524): Web agents that learn tools
- CoAT: Chain-of-Action-Thought for web navigation
- WebArena (2024): Web agent evaluation benchmark
