# CodeForge IDE — Cursor-Killer Architecture Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a full IDE inside ImpForge that beats Cursor on Offline-First + Multi-Model + No Lock-in.

**Architecture:** Component-Split IDE with Event Bus. The `/ide` route is a thin PaneForge shell (~200 LoC) orchestrating 8 specialized Svelte 5 components. Rust backend provides PTY, LSP proxy, Git, Codebase Indexing, and Shadow Workspace via Tauri commands.

**Tech Stack:** Monaco Editor + monaco-languageclient v10 + monacopilot + xterm.js 6 + portable-pty + FastEmbed (Jina Code) + LanceDB + Tree-sitter + async-lsp + dap-types

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CodeForge IDE Shell                               │
│  /ide/+page.svelte (~200 LoC — PaneForge Layout + Keybindings)      │
├─────────────┬───────────────────────────────────┬───────────────────┤
│ FileExplorer │         PaneGroup (Horizontal)    │   AiAgent Panel  │
│   Panel      │  ┌───────────────────────────────┐│                  │
│              │  │  Monaco Editor                 ││  - Chat          │
│  - Tree      │  │  + monaco-languageclient (LSP) ││  - Inline Comp.  │
│  - Fuse.js   │  │  + monacopilot (AI Ghost Text) ││  - Shadow WS     │
│  - Git Files │  │  + DiffEditor (inline)         ││  - Context       │
│              │  ├───────────────────────────────┤│                  │
│              │  │  Bottom Tabs:                   ││                  │
│              │  │  Terminal │ Problems │ Output   ││                  │
│              │  │  xterm.js 6 + portable-pty      ││                  │
│              │  └───────────────────────────────┘│                  │
├─────────────┴───────────────────────────────────┴───────────────────┤
│  StatusBar: LSP ● | Git branch | Ln 42, Col 8 | UTF-8 | Rust | AI │
└─────────────────────────────────────────────────────────────────────┘
```

**Core Principle:** The `/ide` route is a thin shell. Every capability lives in its own Svelte 5 component with state managed in the extended `IdeStore` (Svelte 5 Runes class pattern).

---

## 2. Component Architecture (8 Components)

| # | Component | File | Responsibility | Est. LoC |
|---|-----------|------|----------------|----------|
| 1 | **Shell** | `+page.svelte` | PaneForge layout, global keybindings | ~200 |
| 2 | **CodeEditor** | `CodeEditor.svelte` | Monaco + LSP + AI Completion + DiffEditor | ~350 |
| 3 | **Terminal** | `IdeTerminal.svelte` | xterm.js 6 + PTY (real shell, multi-tab) | ~200 |
| 4 | **FileExplorer** | `FileExplorer.svelte` | Tree, Fuzzy Search (Fuse.js), Git file status | ~250 |
| 5 | **AiAgent** | `AiAgent.svelte` | Chat, Context Builder, Tool Use, Apply Code | ~300 |
| 6 | **GitPanel** | `GitPanel.svelte` | Status, Diff, Commit, Branch, Stash | ~250 |
| 7 | **ProblemsPanel** | `ProblemsPanel.svelte` | LSP Diagnostics list, clickable errors | ~120 |
| 8 | **StatusBar** | `IdeStatusBar.svelte` | LSP status, Git branch, cursor pos, AI model | ~100 |

**Total Frontend: ~1,770 LoC** (current: 416 → 4.3x growth, justified by feature set)

---

## 3. File Structure

### Frontend (Svelte 5)

```
src/routes/ide/
├── +page.svelte                  # Shell (PaneForge Layout only)
├── CodeEditor.svelte             # Monaco + LSP + AI Completion
├── IdeTerminal.svelte            # xterm.js 6 + PTY
├── FileExplorer.svelte           # Tree + Fuzzy Search
├── AiAgent.svelte                # Chat + Shadow Workspace
├── GitPanel.svelte               # Git Operations UI
├── ProblemsPanel.svelte          # LSP Diagnostics
└── IdeStatusBar.svelte           # Status Info Bar

src/lib/stores/
├── ide.svelte.ts                 # Extended (existing file)
├── lsp.svelte.ts                 # LSP connection state (NEW)
└── terminal.svelte.ts            # PTY session state (NEW)
```

### Backend (Rust / Tauri)

```
src-tauri/src/ide/
├── mod.rs                        # Existing (extended with new modules)
├── pty.rs                        # PTY spawn/write/resize/kill (NEW)
├── lsp.rs                        # LSP process lifecycle + IPC proxy (NEW)
├── git.rs                        # Git operations via git2 (NEW)
├── indexer.rs                    # Codebase indexer: Tree-sitter + FastEmbed + LanceDB (NEW)
└── shadow.rs                     # Shadow Workspace validation (NEW)
```

---

## 4. Terminal — xterm.js 6 + portable-pty (Real PTY)

### Current State
- Fake terminal: `<input>` + `<pre>` tags, string concatenation
- Commands dispatched via `ide_execute_command` (spawns `sh -c` per command)
- No ANSI escape codes, no interactive programs (vim, htop, etc.)

### Target State
- Real PTY via `portable-pty` crate (cross-platform: Linux, macOS, Windows)
- xterm.js 6 for ANSI rendering, colors, cursor movement
- Multi-terminal tabs (HashMap<u32, PtySession>)
- Bidirectional streaming via Tauri Events

### Architecture

```
┌──────────────────────────────────────────┐
│  Frontend (xterm.js 6)                    │
│  - Terminal.write(data)  ← pty-output     │
│  - Terminal.onData(cb)   → pty-input      │
│  - FitAddon for auto-resize               │
│  - WebLinksAddon for clickable URLs       │
│  - SearchAddon for Ctrl+F                 │
└──────────────┬───────────────────────────┘
               │ Tauri Events (bidirectional)
               │  emit("pty-input", {id, data})
               │  listen("pty-output-{id}", data)
┌──────────────┴───────────────────────────┐
│  Backend (portable-pty ^0.9)              │
│  - CommandBuilder::new(shell)             │
│  - pair = PtyPair::new(PtySize)           │
│  - child = pair.slave.spawn_command(cmd)  │
│  - master.read() → emit to frontend      │
│  - master.write(input) ← from frontend   │
│  - HashMap<u32, PtySession> in AppState   │
└──────────────────────────────────────────┘
```

### Tauri Commands

```rust
#[tauri::command] async fn pty_spawn(shell: Option<String>) -> Result<u32, String>;
#[tauri::command] async fn pty_write(id: u32, data: String) -> Result<(), String>;
#[tauri::command] async fn pty_resize(id: u32, cols: u16, rows: u16) -> Result<(), String>;
#[tauri::command] async fn pty_kill(id: u32) -> Result<(), String>;
```

### Why Tauri Events (not invoke)
PTY output is continuous stream data. Using `invoke` would require polling. Tauri Events provide push-based bidirectional communication with minimal latency — exactly what a terminal needs.

### npm Packages
- `@xterm/xterm` ^6.0.0 (already installed)
- `@xterm/addon-fit` ^0.11.0 (already installed)
- `@xterm/addon-web-links` ^0.12 (NEW — clickable URLs)
- `@xterm/addon-search` ^0.16 (NEW — Ctrl+F search in terminal)

### Cargo Dependencies
- `portable-pty = "0.9"` (NEW)

---

## 5. LSP Integration — Monaco ↔ Language Server

### Architecture: Tauri IPC (NOT WebSocket)

Research finding: For Tauri desktop apps, Tauri's native IPC is superior to WebSocket bridging. We implement custom `AbstractMessageReader` and `AbstractMessageWriter` classes that use Tauri's `invoke` and events.

```
┌────────────────────────┐     Tauri IPC       ┌──────────────────────┐
│  Monaco Editor          │ ◄────────────────► │  Tauri Backend        │
│  + monaco-languageclient│   invoke + events   │  (LspManager)         │
│  + TauriMessageReader   │                     │                       │
│  + TauriMessageWriter   │                     │  HashMap<Lang, Proc>  │
└────────────────────────┘                     └──────────┬─────────────┘
                                                           │ stdin/stdout
                                                           │ JSON-RPC 2.0
                                               ┌──────────┴─────────────┐
                                               │  Language Server Process │
                                               │  (spawned per language)  │
                                               └─────────────────────────┘
```

### LSP Server Lifecycle (Zed-inspired)

1. **Start on file open**: User opens `.rs` file → backend checks if `rust-analyzer` is running → if not, spawns it with workspace root as cwd
2. **LSP handshake**: Send `initialize` request → wait for `InitializeResult` → send `initialized` notification
3. **Share across files**: One LSP server per language per workspace (not per file)
4. **Stop on inactivity**: 60 seconds after last file of that language is closed → send `shutdown` → `exit` → kill
5. **Crash recovery**: Monitor stderr + process exit → respawn with backoff (max 3 retries in 60s)

### Supported Languages (Phase 1)

| Language | LSP Server | Install Command |
|----------|-----------|-----------------|
| Rust | rust-analyzer | `rustup component add rust-analyzer` |
| Python | pyright | `pip install pyright` |
| TypeScript/JS | typescript-language-server | `npm i -g typescript-language-server` |
| C/C++ | clangd | System package |
| Go | gopls | `go install golang.org/x/tools/gopls@latest` |
| Svelte | svelte-language-server | `npm i -g svelte-language-server` |

### Document Sync
- Use **Incremental sync** (TextDocumentSyncKind::Incremental) when server supports it
- Debounce `textDocument/didChange`: buffer 50-150ms, batch into single notification
- Debounce completion requests: 100-200ms after last keystroke
- Use `$/cancelRequest` aggressively for in-flight requests when user types

### Tauri Commands

```rust
#[tauri::command] async fn lsp_start(language: String, workspace: String) -> Result<LspInfo, String>;
#[tauri::command] async fn lsp_stop(language: String) -> Result<(), String>;
#[tauri::command] async fn lsp_send(language: String, message: String) -> Result<String, String>;
#[tauri::command] async fn lsp_status() -> Result<Vec<LspInfo>, String>;
```

### npm Packages
- `monaco-languageclient` ^10.7.0 (NEW)
- `vscode-ws-jsonrpc` ^4.0.0 (NEW — JSON-RPC message types, used by monaco-languageclient)
- `@codingame/monaco-vscode-api` ^27.0.0 (NEW — VS Code API compatibility layer)

### Cargo Dependencies
- `async-lsp` (NEW — modern LSP client+server framework, tower-based)
- `lsp-types = "0.97"` (NEW — LSP 3.16 type definitions with serde)

---

## 6. AI Inline Completion (monacopilot Pattern)

### How It Works (Like Cursor Tab, But Multi-Model + Offline)

```
User types code → Debounce 300ms → Context Builder → LLM Request → Ghost Text

Context Builder:
├── Current file (cursor position ±50 lines)
├── Open tabs (top 3 most relevant via TF-IDF)
├── Codebase index (top 5 chunks via LanceDB semantic search)
├── LSP symbols (current file outline)
└── Git diff (uncommitted changes)

→ Prompt Template with FIM (Fill-in-Middle):
   <|fim_prefix|>{before_cursor}<|fim_suffix|>{after_cursor}<|fim_middle|>
```

### Model Routing
- **Offline (default)**: Ollama → Qwen2.5-Coder 7B (FIM-capable, GPU)
- **Online fallback**: OpenRouter → Devstral / CodeLlama
- **Latency budget**: Max 500ms for ghost text (cache + streaming)

### monacopilot Integration
- Registers `InlineCompletionItemProvider` with Monaco
- Built-in caching (repeated patterns)
- Accept with Tab, dismiss with Esc
- Partial accept with Ctrl+Right (word by word)
- Multi-line completion support

### npm Packages
- `monacopilot` ^0.20+ (NEW)

---

## 7. Shadow Workspace — AI Code Validation

### Concept (Cursor's Killer Feature, Rebuilt Efficiently)

Cursor uses a hidden VS Code instance to run LSP on AI-generated code. We skip the hidden WebView entirely — we talk to the LSP server directly via stdin/stdout JSON-RPC, using ~80% less RAM.

```
User: "Write a function that does X"
         │
         ▼
┌─────────────────────────────────┐
│  AI Agent generates code         │
│  (Ollama / OpenRouter)           │
└─────────────┬───────────────────┘
              │
              ▼
┌─────────────────────────────────┐
│  Shadow Workspace                │
│  1. Create temp dir              │
│  2. Copy relevant project files  │
│  3. Insert AI-generated code     │
│  4. Spawn LSP in temp dir        │
│  5. Collect diagnostics          │
│  6. Clean up temp dir            │
└─────────────┬───────────────────┘
              │
         ┌────┴────┐
         │ Errors? │
         └────┬────┘
        Yes   │   No
    ┌─────────┤──────────┐
    ▼                    ▼
┌──────────┐      ┌──────────────┐
│ Auto-Fix │      │ Show to User │
│ (1 Retry)│      │ (Validated!) │
└──────────┘      └──────────────┘
```

### Implementation
- `shadow_validate` Tauri command: creates temp dir, copies relevant files, inserts AI code
- Spawns LSP in temp dir (reuses same lifecycle logic from `lsp.rs`)
- Collects `textDocument/publishDiagnostics` for error/warning count
- On 0 errors → code presented with green "LSP Validated" badge
- On errors → one auto-fix attempt, then display with yellow warning
- Temp dir cleaned up after validation completes

### Tauri Commands

```rust
#[tauri::command] async fn shadow_validate(
    code: String, language: String, file_path: String, workspace: String
) -> Result<ShadowResult, String>;
```

---

## 8. Codebase Indexing — Tree-sitter + FastEmbed + LanceDB

### Why LanceDB Instead of Qdrant

Research finding: The `qdrant-client` Rust crate has NO true embedded mode — it's a gRPC client only. LanceDB (`lancedb = "0.26"`, already in Cargo.toml) runs fully in-process with no external server. This is critical for offline desktop apps.

### Architecture

```
┌──────────────────────────────────────────┐
│  File Watcher (notify ^8.0)               │
│  Detects changes → incremental re-index   │
│  File hash map (path → SHA256) on disk    │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│  Tree-sitter Code Chunker                 │
│  - Parse file into AST                    │
│  - Extract: functions, classes, structs,  │
│    methods, impls, enums, traits          │
│  - Chunk size: 200-500 tokens             │
│  - Metadata: file, start_line, end_line,  │
│    node_type, parent_name, language       │
│  - Fallback: sliding window for non-code  │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│  FastEmbed 5.12 (ONNX Runtime, local)     │
│  Model: JinaEmbeddingsV2BaseCode          │
│  - 768-dim embeddings                     │
│  - 8192 token sequence length             │
│  - Trained on 150M+ code Q&A pairs       │
│  - ~10ms/chunk, ~300MB model (cached)     │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│  LanceDB (Embedded, In-Process)           │
│  - No external server needed              │
│  - Storage: ~/.impforge/index/            │
│  - IVF_PQ indexing                        │
│  - Cosine similarity search               │
│  - Metadata filtering (language, path)    │
│  - Arrow-based (efficient columnar)       │
└──────────────────────────────────────────┘
```

### Incremental Indexing Strategy (Cursor-inspired)

1. Maintain file hash map (`path → SHA256`) persisted to SQLite
2. On file change event (from `notify` crate), compare hash
3. If changed: re-parse with tree-sitter (incremental), re-chunk, re-embed only changed chunks
4. Delete old chunks for that file from LanceDB, insert new ones
5. Background thread — never blocks the editor UI

### Tree-sitter Chunk Targets by Language

| Language | AST Node Types |
|----------|---------------|
| Rust | `function_item`, `impl_item`, `struct_item`, `enum_item`, `trait_item`, `mod_item` |
| Python | `function_definition`, `class_definition`, `async_function_definition` |
| TypeScript/JS | `function_declaration`, `class_declaration`, `method_definition`, `arrow_function` |
| C# | `method_declaration`, `class_declaration`, `namespace_declaration` |
| Go | `function_declaration`, `method_declaration`, `type_declaration` |

### Use Cases

1. **AI Context**: Top-5 relevant code chunks as context for inline completion
2. **Semantic Search**: "Where is user authentication handled?" → relevant files + line numbers
3. **Similar Code**: Find similar functions across codebase (clone detection light)
4. **AI Agent Context**: When user asks "refactor this", inject related code as context

### Tauri Commands

```rust
#[tauri::command] async fn index_codebase(workspace: String) -> Result<IndexStatus, String>;
#[tauri::command] async fn search_semantic(query: String, limit: u32) -> Result<Vec<CodeChunk>, String>;
#[tauri::command] async fn index_status() -> Result<IndexStatus, String>;
```

### Cargo Dependencies
- `tree-sitter` (NEW — core parser)
- `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, `tree-sitter-javascript` (NEW — grammar crates)
- `notify = "8.0"` (NEW — file watcher)
- `fastembed = "5.12"` (EXISTING — already in Cargo.toml)
- `lancedb = "0.26"` (EXISTING — already in Cargo.toml, replaces Qdrant for embedded use)

---

## 9. Git Panel — git2 Operations

### Features
- **Status**: Modified/staged/untracked files with diff preview
- **Diff**: Monaco DiffEditor (built-in, no extra package) for side-by-side and inline diff
- **Commit**: Stage files, write commit message, commit
- **Branch**: Create, switch, delete branches
- **Stash**: Stash/pop/apply/drop
- **Log**: Commit history with graph

### Tauri Commands (using git2, already available via octocrab dependency)

```rust
#[tauri::command] async fn git_status(workspace: String) -> Result<GitStatus, String>;
#[tauri::command] async fn git_diff(workspace: String, path: Option<String>) -> Result<String, String>;
#[tauri::command] async fn git_commit(workspace: String, message: String, paths: Vec<String>) -> Result<String, String>;
#[tauri::command] async fn git_branch_list(workspace: String) -> Result<Vec<BranchInfo>, String>;
#[tauri::command] async fn git_branch_create(workspace: String, name: String) -> Result<(), String>;
#[tauri::command] async fn git_branch_switch(workspace: String, name: String) -> Result<(), String>;
#[tauri::command] async fn git_stash(workspace: String, action: StashAction) -> Result<(), String>;
#[tauri::command] async fn git_log(workspace: String, limit: u32) -> Result<Vec<CommitInfo>, String>;
```

### npm Packages
- `diff2html` ^3.4 (NEW — beautiful diff rendering as fallback for non-Monaco views)

---

## 10. DAP (Debug Adapter Protocol) — Phase 2

### Why Phase 2
DAP requires building custom UI (Monaco has no built-in debug support). This is significant work that should not block the core IDE launch. Phase 1 delivers a fully functional IDE without debugging; Phase 2 adds it.

### Architecture (When Implemented)
- DAP types via `dap-types` crate (from Lapce, battle-tested)
- Breakpoint gutters via Monaco's Decoration API
- Call stack, variables, watch panels in Svelte
- Debug adapters: codelldb (Rust/C++), debugpy (Python), dlv dap (Go)
- DAP communication via Tauri IPC (same pattern as LSP)

---

## 11. Dependencies Summary

### New npm Packages

| Package | Version | Purpose | Size |
|---------|---------|---------|------|
| `monaco-languageclient` | ^10.7.0 | Monaco ↔ LSP bridge | ~200KB |
| `vscode-ws-jsonrpc` | ^4.0.0 | JSON-RPC message types | ~20KB |
| `@codingame/monaco-vscode-api` | ^27.0.0 | VS Code API compatibility | ~300KB |
| `monacopilot` | ^0.20+ | AI ghost text completion | ~50KB |
| `fuse.js` | ^7.1 | Fuzzy file/symbol search | ~25KB |
| `diff2html` | ^3.4 | Git diff rendering | ~150KB |
| `@xterm/addon-web-links` | ^0.12 | Clickable URLs in terminal | ~10KB |
| `@xterm/addon-search` | ^0.16 | Terminal search (Ctrl+F) | ~15KB |

**Total: ~770KB additional** (acceptable for desktop app)

### New Cargo Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `portable-pty` | ^0.9 | PTY for real terminal |
| `async-lsp` | latest | LSP client framework |
| `lsp-types` | ^0.97 | LSP type definitions |
| `tree-sitter` | latest | Code parser for chunking |
| `tree-sitter-rust` | latest | Rust grammar |
| `tree-sitter-python` | latest | Python grammar |
| `tree-sitter-typescript` | latest | TS grammar |
| `tree-sitter-javascript` | latest | JS grammar |
| `notify` | ^8.0 | File system watcher |

### Already Present (Only Need Wiring)

| Crate | Version | Current State |
|-------|---------|---------------|
| `fastembed` | 5.12 | In Cargo.toml, not wired |
| `lancedb` | 0.26 | In Cargo.toml, not wired |
| `git2` | (via octocrab) | Available, not used for IDE |

---

## 12. Cursor-Killer Competitive Analysis

| Feature | Cursor | CodeForge (Ours) | Winner |
|---------|--------|-------------------|--------|
| Editor | Monaco (VS Code fork) | Monaco (standalone) | Tie |
| AI Completion | GPT-4/Claude (cloud only) | Multi-Model (Offline + Cloud) | **Us: Offline-First** |
| Shadow Workspace | Hidden VS Code instance | Direct LSP (80% less RAM) | **Us: More Efficient** |
| Terminal | VS Code terminal | xterm.js 6 + PTY | Tie |
| Codebase Index | Proprietary (cloud, Turbopuffer) | FastEmbed + LanceDB (local) | **Us: Privacy** |
| Context Window | 272K tokens | Unlimited (RLM pattern) | **Us: More** |
| Pricing | $20/month (one tier) | €25-90/month (5 tiers) | **Us: Flexible** |
| Lock-in | Cursor account required | No lock-in, any model | **Us: Freedom** |
| Team Dev | Basic (no real collab) | Real-time collab + shared workspace | **Us: Enterprise** |
| Models | GPT-4, Claude (limited) | All (Ollama, OR, Anthropic, OpenAI) | **Us: Multi-Model** |
| Platform | Electron (~300MB) | Tauri (~30MB) | **Us: 10x Smaller** |
| Code Embedding | cloud-dependent (Turbopuffer) | JinaCode 768-dim + LanceDB (local) | **Us: Offline** |
| Chunk Strategy | Proprietary | Tree-sitter AST (open, research-backed) | **Us: Transparent** |

### Our Unique Advantages
1. **Offline-First**: Full IDE features without internet
2. **Multi-Model Freedom**: Switch between Ollama, OpenRouter, Anthropic, OpenAI anytime
3. **Flexible Pricing**: 5 subscription tiers from €25-90/month (vs Cursor's flat $20)
4. **Privacy**: Code never leaves the machine (local embedding + local vector DB)
5. **Tauri**: 10x smaller binary, 5x less RAM than Electron-based IDEs
6. **Open Architecture**: BSL 1.1 engine (open source after 4 years)
7. **Team Development**: Real-time collaboration, shared workspaces, team AI context

### Subscription Tiers

| Tier | Price/Month | Features |
|------|------------|----------|
| **Starter** | €25-30 | CodeForge IDE, 1 user, Ollama (local models only), basic AI completion |
| **Pro** | €45 | + Cloud models (OpenRouter), Shadow Workspace, Codebase Indexing, priority support |
| **Team** | €60/user | + Team collaboration, shared workspaces, shared codebase index, team analytics |
| **Business** | €75/user | + SSO/SAML, audit logs, custom model endpoints, admin dashboard, SLA |
| **Enterprise** | €90/user | + On-premise deployment, dedicated support, custom integrations, unlimited API |

**Free Community Edition**: Apache-2.0, local models only, no cloud features, no team collab.
**BSL Engine**: Pro+ features gated behind BSL 1.1 license (→ Apache-2.0 after 4 years).

---

## 12.5. Team Development Features

### Real-Time Collaboration (Team+ Tiers)

```
┌─────────────────────────────────────┐
│  Developer A (CodeForge)             │
│  - Editing main.rs                   │
│  - Cursor at line 42                 │
└──────────┬──────────────────────────┘
           │ WebSocket (Automerge CRDT)
           │
┌──────────┴──────────────────────────┐
│  ImpForge Relay Server               │
│  - Automerge document sync           │
│  - Presence (cursors, selections)    │
│  - Permission management             │
│  - Session recording                 │
└──────────┬──────────────────────────┘
           │
┌──────────┴──────────────────────────┐
│  Developer B (CodeForge)             │
│  - Sees A's cursor (colored)         │
│  - Real-time edits merge via CRDT    │
└─────────────────────────────────────┘
```

### Team Features

| Feature | Tier | Description |
|---------|------|-------------|
| **Shared Workspace** | Team+ | Multiple users edit same project |
| **Live Cursors** | Team+ | See teammates' cursor positions + selections |
| **Conflict-Free Editing** | Team+ | Automerge CRDT for real-time merge |
| **Shared Codebase Index** | Team+ | Team shares one LanceDB index (hosted) |
| **Team AI Context** | Team+ | AI completion aware of team patterns |
| **Code Review** | Team+ | Inline review comments, approval workflow |
| **Session Recording** | Business+ | Record and replay coding sessions |
| **SSO/SAML** | Business+ | Enterprise identity provider integration |
| **Audit Logs** | Business+ | Who edited what, when, compliance |
| **Admin Dashboard** | Business+ | Team usage, model costs, analytics |
| **On-Premise** | Enterprise | Self-hosted relay server, air-gapped |
| **Custom Models** | Enterprise | Private fine-tuned models, custom endpoints |

### Technical Implementation
- **CRDT**: Automerge (Rust + JS libraries) for conflict-free real-time editing
- **Relay Server**: Rust async server (axum + tokio) for document sync
- **Presence**: WebSocket heartbeat with cursor position, selection, file
- **Auth**: JWT tokens, OAuth2 (GitHub/GitLab/Google), SSO/SAML for Business+
- **Hosting**: ImpForge Cloud (relay server) or On-Premise (Enterprise)

---

## 13. Implementation Phases

### Phase 1: Core IDE (Priority — Ship First)
1. Terminal: xterm.js 6 + portable-pty (replace fake terminal)
2. FileExplorer: Extract from page, add Fuse.js fuzzy search
3. CodeEditor: Extract Monaco, add LSP via monaco-languageclient
4. StatusBar: LSP status, git branch, cursor position
5. Shell: PaneForge layout with resizable panels

### Phase 2: AI Intelligence
6. AI Inline Completion: monacopilot + Ollama/OpenRouter
7. AI Agent: Enhanced chat with code apply, context from index
8. Codebase Indexer: Tree-sitter + FastEmbed + LanceDB
9. Shadow Workspace: LSP validation of AI-generated code

### Phase 3: Git & Polish
10. Git Panel: Status, diff, commit, branch, stash
11. Problems Panel: LSP diagnostics aggregation
12. Keybindings: Ctrl+P (quick open), Ctrl+Shift+P (command palette), Ctrl+` (terminal)
13. Themes: Import VS Code themes for Monaco

### Phase 4: Debug (Post-Launch)
14. DAP Integration: Breakpoints, call stack, variables
15. Debug adapters: codelldb, debugpy, dlv dap

### Phase 5: Team Development (Enterprise)
16. Automerge CRDT integration for real-time editing
17. Relay server (axum + tokio + WebSocket)
18. Live cursors + presence system
19. Shared codebase index (hosted LanceDB)
20. SSO/SAML + JWT auth + admin dashboard
21. Session recording + audit logs

### Phase 6: Subscription & Billing
22. Stripe integration for subscription management
23. License tier gating (Starter/Pro/Team/Business/Enterprise)
24. Usage metering (AI completions, cloud API calls)
25. Team management UI (invite, roles, permissions)

---

## 14. Scientific References

| Paper/Source | Finding | Application |
|---|---|---|
| cAST (arXiv:2506.15655) | AST-based chunking: +4.3 Recall@5 vs line-based | Tree-sitter code chunking strategy |
| Jina Embeddings v2 Code | 768-dim, 8192 seq, 150M+ code pairs | FastEmbed model choice |
| Cursor Architecture (TDS 2025) | Merkle tree incremental sync, Turbopuffer | Incremental indexing design |
| Zed LSP Architecture | One server per language per workspace, crash recovery | LSP lifecycle management |
| monacopilot (TypeFox) | InlineCompletionItemProvider registration | AI completion integration |
| Qwen2.5-Coder FIM | Native `<|fim_prefix|>/<|fim_suffix|>/<|fim_middle|>` support | Offline completion prompting |

---

## 15. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| monaco-languageclient version conflicts | Pin exact versions, test with Monaco 0.55.1 |
| portable-pty Windows compatibility | Crate supports Windows via conpty — test in CI |
| FastEmbed model download on first use | Show progress bar, cache in ~/.impforge/models/ |
| LanceDB API instability (pre-1.0) | Pin version, wrap in abstraction layer |
| LSP server not installed | Auto-detect, show install prompt with one-click command |
| Large codebase indexing time | Background thread, incremental only, progress indicator |

---

## Appendix A: Existing Code to Preserve

The current IDE implementation (`+page.svelte` 416 LoC + `ide.svelte.ts` 285 LoC + `ide/mod.rs` 381 LoC) provides a solid foundation. Key elements to preserve and extend:

- **IdeStore class pattern** (Svelte 5 Runes) → extend with LSP/terminal state
- **Monaco "impforge-dark" theme** → keep as-is, add more token rules
- **validate_path()** security check → reuse in all new commands
- **AgentTool enum** → extend with new tools (git, index, debug)
- **is_text_file()** → reuse for indexer file filtering
- **AI Agent chat** → extract into AiAgent.svelte, enhance context builder
