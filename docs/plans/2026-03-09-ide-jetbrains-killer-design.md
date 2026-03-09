# ImpForge IDE — JetBrains Killer Upgrade Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade ImpForge's IDE from 85% to 100%+ feature-completeness, surpassing JetBrains ($120/yr) with AI-native features no competitor has.

**Architecture:** Four-phase approach — Quick-Wins (wire existing), AI-First Features (new modules), Full-Stack Tools (new panels), BenikUI Deep Customization (style engine for all 13 IDE panels).

**Tech Stack:** Tauri 2.10, Svelte 5.51, Rust, Monaco Editor, xterm.js, libgit2, LSP, DAP, SQLite

---

## Research Foundation

Based on comprehensive web research (40+ sources, see appendix):

- **DORA 2025**: 69% of devs lose 8+ hours/week to tool inefficiencies
- **METR RCT 2025**: AI helps most with *understanding* code, not writing it faster
- **Augment Code**: 23K hours saved via semantic search, not code generation
- **JetBrains moat**: Deep semantic analysis (25yr investment)
- **Cursor moat**: AI-native UX (background agents, visual editing)
- **ImpForge moat**: Offline-first + multi-agent orchestration + full customizability

---

## Phase 1: Quick-Wins (85% → 95%)

### 1.1 Status Bar Wiring
- Wire `lsp_status` → LSP indicator (green/yellow/red)
- Wire `git_status` → branch name display
- Wire active AI model name from router/settings
- Wire cursor position from Monaco editor events

### 1.2 Git Commit
- Add `git_commit` Tauri command in `git.rs` (libgit2)
- Add commit message input + "Commit" button in GitPanel.svelte
- Add AI commit message generation from diff

### 1.3 Find & Replace (Ctrl+H)
- Use Monaco's built-in Find/Replace widget (`editor.getAction('editor.action.startFindReplaceAction')`)
- Wire Ctrl+H keyboard shortcut in +page.svelte

### 1.4 Editor Gutter Decorations
- Breakpoint click → Monaco gutter decoration + `debug_set_breakpoints` call
- Error/Warning markers from LSP diagnostics in gutter
- Git diff indicators (green = added, red = deleted) in gutter

### 1.5 Format Document
- Wire Ctrl+Shift+F → `lsp_format` → Monaco `editor.getAction('editor.action.formatDocument')`
- Add `lsp_format` command that sends `textDocument/formatting` request

---

## Phase 2: AI-First Dominance (The USP)

### 2.1 Autonomous Multi-Agent Sessions
**New files:**
- `src-tauri/src/ide/agent_sessions.rs` — manages N parallel agents
- `src/routes/ide/AgentSessions.svelte` — tab-based multi-agent view

**Architecture:**
- Each session: own PTY (via existing `pty.rs`), own working dir, own chat context
- Agent can: read/write files, run commands, check LSP diagnostics, run tests
- Self-healing loop: detect error → analyze → fix → re-run → verify
- Frontend: sidebar tabs showing each agent's status (idle/working/reviewing/done)
- Max 3 concurrent agents (configurable)

### 2.2 Next-Edit Prediction
**Extends:** `ai_complete.rs`

- New command `predict_next_edit(recent_edits: Vec<EditDelta>) -> NextEditPrediction`
- Track last 5 edits as context (file, position, before/after diff)
- Route to local model (Qwen) for <200ms latency
- Monaco decoration: dim ghost text at predicted edit location
- Tab accepts prediction, Shift+Tab cycles through alternatives

### 2.3 Spec-Driven Development
**New files:**
- `src-tauri/src/ide/spec_engine.rs` — spec parsing, Mermaid generation, code↔spec mapping
- `src/routes/ide/SpecPanel.svelte` — spec editor with live Mermaid preview

**Architecture:**
- Write requirements in markdown → AI generates structured spec with tasks
- Mermaid.js diagrams auto-generated from code structure (via indexer.rs)
- Spec violations surface as LSP-style diagnostics
- Bidirectional: edit code → spec updates; edit spec → generate tasks

### 2.4 Knowledge Graph Code Visualization
**Extends:** `indexer.rs`

- Build symbol relationship graph during indexing (calls, imports, inherits)
- New command `get_code_graph(scope: GraphScope) -> CodeGraph`
- Frontend: Canvas-based graph view (lightweight SVG, no heavy d3)
- Nodes = symbols (functions, classes, modules), Edges = relationships
- Click node → jump to definition
- AI query: "Show all callers of function X"

---

## Phase 3: Full-Stack Developer Tools

### 3.1 Database Client
**New files:**
- `src-tauri/src/ide/database.rs` — SQLite/PostgreSQL/MySQL via sqlx
- `src/routes/ide/DatabasePanel.svelte` — schema browser, query editor, result table

### 3.2 HTTP Client
**New files:**
- `src-tauri/src/ide/http_client.rs` — REST/GraphQL via reqwest
- `src/routes/ide/HttpClient.svelte` — request builder, response viewer, history

### 3.3 Git Vollständig
**Extends:** `git.rs`
- `git_commit`, `git_push`, `git_pull`, `git_branch_create`, `git_checkout`, `git_merge`
- AI commit message generation
- Inline blame (`git_blame` command + editor decoration)

### 3.4 Search Panel (Find in Files)
**New files:**
- `src/routes/ide/SearchPanel.svelte` — dedicated search with regex, globs, replace-all
**Extends:** `mod.rs` `ide_search_files` with replace capability

### 3.5 Breadcrumb / Symbol Outline
**New files:**
- `src/routes/ide/SymbolOutline.svelte` — tree of symbols in current file
**Extends:** `indexer.rs` for per-file symbol extraction

---

## Phase 4: BenikUI Deep Customization (ALL 13 IDE Panels)

### Pattern (from ChatMessage.svelte / chat/+page.svelte):

```svelte
<script>
  import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
  const widgetId = 'ide-file-explorer'; // unique per panel

  let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
  let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
  let headerStyle = $derived(
    hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : ''
  );
</script>

<div class="{hasEngineStyle ? '' : 'bg-gx-bg-primary'}" style={headerStyle}>
```

### Panels to Upgrade (13 total):

| Panel | widgetId | Sub-Components |
|-------|----------|----------------|
| FileExplorer | `ide-file-explorer` | header, search-input, file-tree, file-item, folder-item, path-bar |
| CodeEditor | `ide-code-editor` | container, tab-bar, tab-item, editor-area, minimap |
| IdeTerminal | `ide-terminal` | header, tab-bar, terminal-area |
| GitPanel | `ide-git-panel` | header, tab-selector, staged-section, unstaged-section, diff-view, commit-log |
| ProblemsPanel | `ide-problems` | header, file-group, diagnostic-item |
| DebugPanel | `ide-debug` | header, controls, call-stack, variables, watch, console |
| AiAgent | `ide-ai-agent` | header, message-bubble, tool-call, input-area, suggestions |
| CollabPanel | `ide-collab` | header, peer-list, activity |
| CommandPalette | `ide-command-palette` | overlay, search-input, result-item |
| ThemeImporter | `ide-theme-importer` | container, preview, controls |
| PricingPanel | `ide-pricing` | container, plan-card, feature-matrix |
| IdeStatusBar | `ide-status-bar` | container, lsp-indicator, git-info, cursor-info, language, ai-model |
| +page.svelte | `ide-layout` | container, tab-bar, panel-handle |

### New Panels (Phase 2-3):

| Panel | widgetId | Sub-Components |
|-------|----------|----------------|
| AgentSessions | `ide-agent-sessions` | header, agent-tab, agent-status, terminal, file-changes |
| SpecPanel | `ide-spec-panel` | header, spec-editor, mermaid-preview, task-list |
| DatabasePanel | `ide-database` | header, schema-tree, query-editor, result-table |
| HttpClient | `ide-http-client` | header, method-selector, url-input, body-editor, response-view |
| SearchPanel | `ide-search` | header, search-input, filters, result-list |
| SymbolOutline | `ide-symbols` | header, symbol-tree, symbol-item |

---

## Appendix: Sources

- JetBrains Junie: https://www.jetbrains.com/junie/
- Cursor Features: https://cursor.com/features
- Zed IDE: https://zed.dev/
- Kiro Spec-Driven: https://kiro.dev/
- Agent Client Protocol: https://www.jetbrains.com/acp/
- DORA 2025: https://dora.dev/research/2025/dora-report/
- METR RCT: https://arxiv.org/abs/2507.09089
- Augment Code: https://www.augmentcode.com/context-engine
- Cognitive Load Theory: Sweller (1988)
- GitHub Copilot Study: https://arxiv.org/abs/2302.06590
