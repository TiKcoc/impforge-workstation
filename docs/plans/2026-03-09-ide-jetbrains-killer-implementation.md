# ImpForge IDE — JetBrains Killer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade ImpForge IDE from 85% to 100%+ feature-completeness with AI-native features, full-stack tools, and BenikUI deep customization for every panel.

**Architecture:** Tauri 2.10 + Svelte 5 + Rust backend. BenikUI style engine (`styleEngine.getComponentStyle` + `componentToCSS`) applied to all panels with dual-path fallback (engine styles or default gx- classes). New Rust modules for git commit, database, HTTP client, agent sessions. New Svelte panels for search, symbols, spec-driven dev.

**Tech Stack:** Tauri 2.10, Svelte 5.51 (runes), Rust, Monaco 0.55.1, xterm.js 6.0, libgit2, lsp-types, sqlx, reqwest, mermaid.js

---

## Phase 1: Quick-Wins (85% → 95%)

### Task 1: Status Bar Wiring

**Files:**
- Modify: `src/routes/ide/IdeStatusBar.svelte`
- Modify: `src/routes/ide/+page.svelte` (where StatusBar is rendered)

**Context:** The StatusBar currently receives hardcoded props: `lspStatus="disconnected"`, `gitBranch=""`, `aiModel="Ollama"`. These must be wired to real Tauri backend calls. The backend commands `lsp_status`, `git_status` already exist and work.

**Step 1: Update IdeStatusBar to fetch live data**

Replace the Props-based approach with direct Tauri invocations + polling:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { GitBranch, Circle, Cpu, Type } from '@lucide/svelte';
  import { ide } from '$lib/stores/ide.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getSetting } from '$lib/stores/settings.svelte';
  import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

  const widgetId = 'ide-status-bar';

  let lspStatus = $state<string>('disconnected');
  let gitBranch = $state<string>('');

  // Style engine
  let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
  let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
  let containerStyle = $derived(
    hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
  );

  const cursorInfo = $derived(() => {
    const tab = ide.activeTab;
    if (!tab) return 'Ln 1, Col 1';
    return `Ln -, Col -`;
  });
  const language = $derived(ide.activeTab?.language || 'plaintext');
  const encoding = 'UTF-8';
  const aiModel = $derived(getSetting('selectedModel') || 'Ollama');

  const lspColor = $derived(
    lspStatus === 'running' ? 'text-gx-status-success' :
    lspStatus === 'starting' ? 'text-gx-status-warning' :
    'text-gx-text-disabled'
  );

  onMount(() => {
    const interval = setInterval(async () => {
      try {
        const status = await invoke<{ status: string }>('lsp_status');
        lspStatus = status.status;
      } catch { /* LSP not running */ }
      try {
        const git = await invoke<{ branch: string }>('git_status', { workspacePath: ide.currentDir });
        gitBranch = git.branch;
      } catch { /* not a git repo */ }
    }, 5000);
    return () => clearInterval(interval);
  });
</script>
```

**Step 2: Update the template with BenikUI dual-path**

```svelte
<div class="flex items-center h-6 px-2 border-t border-gx-border-subtle text-[11px] text-gx-text-muted shrink-0 gap-3 {hasEngineStyle ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
  <!-- same content as before but with live data -->
</div>
```

**Step 3: Remove hardcoded props from +page.svelte**

In `src/routes/ide/+page.svelte`, change:
```svelte
<!-- Before -->
<IdeStatusBar lspStatus="disconnected" gitBranch="" aiModel="Ollama" />
<!-- After -->
<IdeStatusBar />
```

**Step 4: Test manually** — Open IDE, verify status bar shows real LSP status, git branch, AI model.

**Step 5: Commit**
```bash
git add src/routes/ide/IdeStatusBar.svelte src/routes/ide/+page.svelte
git commit -m "feat(ide): wire status bar to live LSP, git, and AI model data"
```

---

### Task 2: Git Commit Command

**Files:**
- Modify: `src-tauri/src/ide/git.rs`
- Modify: `src/routes/ide/GitPanel.svelte`

**Context:** `git.rs` has `git_status`, `git_diff`, `git_log`, `git_stage`, `git_unstage` but NO `git_commit`. GitPanel has no commit UI. libgit2 (`git2` crate v0.20) is already a dependency.

**Step 1: Add git_commit command to git.rs**

Add after the `git_unstage` function:

```rust
#[tauri::command]
pub async fn git_commit(
    workspace_path: String,
    message: String,
    sign: bool,
) -> Result<String, String> {
    let path = workspace_path.clone();
    let msg = message.clone();
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&path).map_err(|e| e.to_string())?;
        let sig = repo.signature().map_err(|e| e.to_string())?;
        let tree_id = repo.index().map_err(|e| e.to_string())?
            .write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let parent = match repo.head() {
            Ok(head) => {
                let commit = head.peel_to_commit().map_err(|e| e.to_string())?;
                Some(commit)
            }
            Err(_) => None,
        };

        let parents: Vec<&git2::Commit> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();

        let oid = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &tree,
            &parents,
        ).map_err(|e| e.to_string())?;

        Ok(oid.to_string())
    }).await.map_err(|e| e.to_string())?
}
```

**Step 2: Register command in lib.rs**

In `src-tauri/src/lib.rs`, add `git_commit` to the `invoke_handler` list.

**Step 3: Add commit UI to GitPanel.svelte**

Add after the tab selector, before the content area:

```svelte
<!-- Commit input -->
{#if activeTab === 'changes' && stagedFiles.length > 0}
  <div class="px-2 py-1.5 border-b border-gx-border-subtle">
    <textarea
      bind:value={commitMessage}
      placeholder="Commit message..."
      rows="2"
      class="w-full bg-gx-bg-secondary border border-gx-border-default rounded px-2 py-1 text-xs text-gx-text-primary placeholder:text-gx-text-muted outline-none focus:border-gx-neon resize-none"
    ></textarea>
    <div class="flex gap-1 mt-1">
      <button
        onclick={doCommit}
        disabled={!commitMessage.trim() || committing}
        class="flex-1 py-1 text-xs rounded bg-gx-neon/20 text-gx-neon hover:bg-gx-neon/30 disabled:opacity-40 transition-colors"
      >
        {committing ? 'Committing...' : `Commit (${stagedFiles.length})`}
      </button>
    </div>
  </div>
{/if}
```

Add state variables and commit function:
```typescript
let commitMessage = $state('');
let committing = $state(false);

async function doCommit() {
  if (!commitMessage.trim() || !workspacePath) return;
  committing = true;
  try {
    await invoke('git_commit', { workspacePath, message: commitMessage, sign: false });
    commitMessage = '';
    await refresh();
  } catch (e) {
    console.error('Commit failed:', e);
  }
  committing = false;
}
```

**Step 4: Test** — Stage a file, write a commit message, click Commit. Verify commit appears in log.

**Step 5: Commit**
```bash
git add src-tauri/src/ide/git.rs src/routes/ide/GitPanel.svelte src-tauri/src/lib.rs
git commit -m "feat(ide): add git commit command and commit UI in GitPanel"
```

---

### Task 3: Find & Replace (Ctrl+H)

**Files:**
- Modify: `src/routes/ide/+page.svelte`
- Modify: `src/routes/ide/CodeEditor.svelte`

**Context:** Monaco Editor has built-in Find/Replace accessible via `editor.trigger('keyboard', 'editor.action.startFindReplaceAction')`. We just need to wire the keyboard shortcut.

**Step 1: Add Ctrl+H handler in +page.svelte keyboard handler**

In the existing `handleKeydown` function, add:
```typescript
// Find & Replace
if (e.ctrlKey && e.key === 'h') {
  e.preventDefault();
  // Monaco handles this internally when focused
  const editor = document.querySelector('.monaco-editor textarea');
  if (editor) (editor as HTMLElement).focus();
  // Trigger via a custom event the CodeEditor listens for
  window.dispatchEvent(new CustomEvent('ide-find-replace'));
}
```

**Step 2: In CodeEditor.svelte, listen for the event**

```typescript
onMount(() => {
  const handler = () => {
    if (editorInstance) {
      editorInstance.trigger('keyboard', 'editor.action.startFindReplaceAction', null);
    }
  };
  window.addEventListener('ide-find-replace', handler);
  return () => window.removeEventListener('ide-find-replace', handler);
});
```

**Step 3: Test** — Open a file in IDE, press Ctrl+H, verify Find/Replace dialog appears.

**Step 4: Commit**
```bash
git add src/routes/ide/+page.svelte src/routes/ide/CodeEditor.svelte
git commit -m "feat(ide): wire Ctrl+H for Find & Replace via Monaco built-in"
```

---

### Task 4: Editor Gutter Decorations

**Files:**
- Modify: `src/routes/ide/CodeEditor.svelte`

**Context:** Monaco supports gutter decorations via `deltaDecorations()`. We need: breakpoint click handler, error/warning markers from LSP, git diff indicators.

**Step 1: Add breakpoint click handler**

In CodeEditor.svelte, after editor creation:
```typescript
// Breakpoint gutter click
editorInstance.onMouseDown((e) => {
  if (e.target.type === monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) {
    const line = e.target.position?.lineNumber;
    if (line) toggleBreakpoint(line);
  }
});

let breakpoints = $state<Set<number>>(new Set());

function toggleBreakpoint(line: number) {
  const bp = new Set(breakpoints);
  if (bp.has(line)) bp.delete(line);
  else bp.add(line);
  breakpoints = bp;
  updateGutterDecorations();
}
```

**Step 2: Add decoration update function**

```typescript
let decorationIds: string[] = [];

function updateGutterDecorations() {
  if (!editorInstance) return;
  const decorations: monaco.editor.IModelDeltaDecoration[] = [];

  // Breakpoints
  for (const line of breakpoints) {
    decorations.push({
      range: new monaco.Range(line, 1, line, 1),
      options: {
        isWholeLine: true,
        glyphMarginClassName: 'breakpoint-glyph',
        glyphMarginHoverMessage: { value: 'Breakpoint' },
      },
    });
  }

  decorationIds = editorInstance.deltaDecorations(decorationIds, decorations);
}
```

**Step 3: Add CSS for breakpoint glyph**

In the component's `<style>`:
```css
:global(.breakpoint-glyph) {
  background: #ef4444;
  border-radius: 50%;
  width: 10px !important;
  height: 10px !important;
  margin-top: 4px;
  margin-left: 4px;
}
```

**Step 4: Enable glyph margin in Monaco options**

Add to editor creation options: `glyphMargin: true`

**Step 5: Commit**
```bash
git add src/routes/ide/CodeEditor.svelte
git commit -m "feat(ide): add breakpoint gutter click and decorations in Monaco"
```

---

### Task 5: Format Document (Ctrl+Shift+F)

**Files:**
- Modify: `src/routes/ide/+page.svelte`
- Modify: `src/routes/ide/CodeEditor.svelte`

**Step 1: Wire Ctrl+Shift+F in +page.svelte**

```typescript
if (e.ctrlKey && e.shiftKey && e.key === 'F') {
  e.preventDefault();
  window.dispatchEvent(new CustomEvent('ide-format'));
}
```

**Step 2: In CodeEditor.svelte, trigger Monaco format**

```typescript
const formatHandler = () => {
  if (editorInstance) {
    editorInstance.trigger('keyboard', 'editor.action.formatDocument', null);
  }
};
window.addEventListener('ide-format', formatHandler);
```

**Step 3: Commit**
```bash
git add src/routes/ide/+page.svelte src/routes/ide/CodeEditor.svelte
git commit -m "feat(ide): wire Ctrl+Shift+F for document formatting"
```

---

## Phase 2: BenikUI for ALL IDE Panels (13 panels)

### Task 6: BenikUI — FileExplorer

**Files:**
- Modify: `src/routes/ide/FileExplorer.svelte`

**Context:** The BenikUI pattern is:
1. Import `styleEngine, componentToCSS` from `$lib/stores/style-engine.svelte`
2. Define `widgetId` (e.g. `'ide-file-explorer'`)
3. Create `$derived` for `hasEngineStyle`, component styles, CSS strings
4. Apply dual-path: `class="{hasEngineStyle ? '' : 'bg-gx-bg-primary'}"` + `style={containerStyle}`

**Step 1: Add style engine imports and derived values**

At the top of the `<script>` block, add:
```typescript
import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

const widgetId = 'ide-file-explorer';

let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
let searchComponent = $derived(styleEngine.getComponentStyle(widgetId, 'search-input'));
let treeComponent = $derived(styleEngine.getComponentStyle(widgetId, 'file-tree'));
let pathBarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'path-bar'));

let headerStyle = $derived(hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : '');
let searchStyle = $derived(hasEngineStyle && searchComponent ? componentToCSS(searchComponent) : '');
let treeStyle = $derived(hasEngineStyle && treeComponent ? componentToCSS(treeComponent) : '');
let pathBarStyle = $derived(hasEngineStyle && pathBarComponent ? componentToCSS(pathBarComponent) : '');
```

**Step 2: Apply dual-path to each section**

Root container:
```svelte
<div class="flex flex-col h-full overflow-hidden {hasEngineStyle ? '' : 'bg-gx-bg-primary'}">
```

Header:
```svelte
<div class="flex items-center gap-1 px-2 py-1.5 border-b border-gx-border-default shrink-0 {hasEngineStyle ? '' : ''}" style={headerStyle}>
```

Search input wrapper:
```svelte
<div class="px-2 py-1 border-b border-gx-border-default" style={searchStyle}>
```

File tree:
```svelte
<div class="flex-1 overflow-auto text-xs" style={treeStyle}>
```

Path bar:
```svelte
<div class="px-2 py-1 border-t border-gx-border-default shrink-0" style={pathBarStyle}>
```

**Step 3: Commit**
```bash
git add src/routes/ide/FileExplorer.svelte
git commit -m "feat(ide): add BenikUI style engine to FileExplorer"
```

---

### Task 7: BenikUI — GitPanel

**Files:**
- Modify: `src/routes/ide/GitPanel.svelte`

**Same pattern as Task 6. Sub-components:**
- `widgetId = 'ide-git-panel'`
- Components: `header`, `tab-selector`, `staged-section`, `unstaged-section`, `diff-view`, `commit-input`

**Step 1:** Add imports + derived values (same pattern)
**Step 2:** Apply dual-path to header, tab selector, file sections, diff view
**Step 3:** Commit

---

### Task 8: BenikUI — ProblemsPanel

**Files:**
- Modify: `src/routes/ide/ProblemsPanel.svelte`

**Sub-components:** `header`, `file-group`, `diagnostic-item`
- `widgetId = 'ide-problems'`

---

### Task 9: BenikUI — DebugPanel

**Files:**
- Modify: `src/routes/ide/DebugPanel.svelte`

**Sub-components:** `header`, `controls`, `call-stack`, `variables`, `watch`, `console`
- `widgetId = 'ide-debug'`

---

### Task 10: BenikUI — AiAgent

**Files:**
- Modify: `src/routes/ide/AiAgent.svelte`

**Sub-components:** `header`, `message-area`, `tool-call`, `input-area`, `suggestions`
- `widgetId = 'ide-ai-agent'`

---

### Task 11: BenikUI — IdeTerminal

**Files:**
- Modify: `src/routes/ide/IdeTerminal.svelte`

**Sub-components:** `header`, `tab-bar`, `terminal-area`
- `widgetId = 'ide-terminal'`

---

### Task 12: BenikUI — CommandPalette

**Files:**
- Modify: `src/routes/ide/CommandPalette.svelte`

**Sub-components:** `overlay`, `search-input`, `result-item`
- `widgetId = 'ide-command-palette'`

---

### Task 13: BenikUI — CodeEditor + TabBar + IDE Layout

**Files:**
- Modify: `src/routes/ide/CodeEditor.svelte`
- Modify: `src/routes/ide/+page.svelte` (tab bar + layout)

**Sub-components for CodeEditor:** `container`, `minimap`
- `widgetId = 'ide-code-editor'`

**Sub-components for layout:** `tab-bar`, `tab-item`, `panel-handle`
- `widgetId = 'ide-layout'`

---

### Task 14: BenikUI — CollabPanel, ThemeImporter, PricingPanel

**Files:**
- Modify: `src/routes/ide/CollabPanel.svelte` — `widgetId = 'ide-collab'`
- Modify: `src/routes/ide/ThemeImporter.svelte` — `widgetId = 'ide-theme-importer'`
- Modify: `src/routes/ide/PricingPanel.svelte` — `widgetId = 'ide-pricing'`

Same pattern for all three.

---

### Task 15: BenikUI — Convergence Route

**Files:**
- Modify: `src/routes/convergence/+page.svelte`

Check if it has styleEngine. If not, add with `widgetId = 'convergence-page'`.

---

## Phase 3: Full-Stack Developer Tools

### Task 16: Complete Git Operations

**Files:**
- Modify: `src-tauri/src/ide/git.rs`
- Modify: `src/routes/ide/GitPanel.svelte`

**Step 1: Add git_push, git_pull, git_branch_create, git_checkout commands**

```rust
#[tauri::command]
pub async fn git_push(workspace_path: String) -> Result<String, String> {
    // Use Command::new("git") as libgit2 push requires auth setup
    let output = tokio::process::Command::new("git")
        .args(&["push"])
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub async fn git_pull(workspace_path: String) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(&["pull"])
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub async fn git_branch_create(workspace_path: String, name: String) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(&["checkout", "-b", &name])
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(format!("Created branch: {}", name))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub async fn git_checkout(workspace_path: String, branch: String) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(&["checkout", &branch])
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(format!("Switched to: {}", branch))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
pub async fn git_blame(workspace_path: String, file_path: String) -> Result<Vec<BlameLine>, String> {
    let path = workspace_path.clone();
    let fp = file_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&path).map_err(|e| e.to_string())?;
        let blame = repo.blame_file(std::path::Path::new(&fp), None).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            lines.push(BlameLine {
                commit_id: hunk.final_commit_id().to_string()[..7].to_string(),
                author: String::from_utf8_lossy(sig.name_bytes()).to_string(),
                line_start: hunk.final_start_line(),
                line_count: hunk.lines_in_hunk(),
            });
        }
        Ok(lines)
    }).await.map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub struct BlameLine {
    pub commit_id: String,
    pub author: String,
    pub line_start: usize,
    pub line_count: usize,
}
```

**Step 2:** Register all new commands in lib.rs

**Step 3:** Add Push/Pull/Branch buttons to GitPanel header

**Step 4: Commit**

---

### Task 17: Search Panel (Find in Files)

**Files:**
- Create: `src/routes/ide/SearchPanel.svelte`
- Modify: `src/routes/ide/+page.svelte` (add SearchPanel to bottom panel tabs)

**Step 1: Create SearchPanel.svelte**

Full component with: search input, regex toggle, include/exclude globs, result list with file grouping, click-to-navigate.

Uses existing `ide_search_files` Tauri command. BenikUI integrated from the start.

**Step 2:** Add "Search" tab to bottom panel in +page.svelte

**Step 3: Commit**

---

### Task 18: Symbol Outline Panel

**Files:**
- Create: `src/routes/ide/SymbolOutline.svelte`
- Modify: `src/routes/ide/+page.svelte`

**Step 1: Create SymbolOutline**

Tree of symbols in current file. Uses `index_codebase` / `search_codebase` for symbol data. Click navigates to line. BenikUI integrated.

**Step 2:** Wire to right sidebar as optional panel

**Step 3: Commit**

---

### Task 19: Database Client Panel

**Files:**
- Create: `src-tauri/src/ide/database.rs`
- Create: `src/routes/ide/DatabasePanel.svelte`
- Modify: `src-tauri/src/ide/mod.rs` (add module)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Step 1: Create database.rs**

```rust
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

#[derive(serde::Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub affected: u64,
    pub elapsed_ms: u64,
}

#[tauri::command]
pub async fn db_connect(connection_string: String) -> Result<String, String> {
    // Store connection in app state
    SqlitePool::connect(&connection_string)
        .await
        .map(|_| "Connected".to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn db_query(connection_string: String, query: String) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();
    let pool = SqlitePool::connect(&connection_string).await.map_err(|e| e.to_string())?;
    let rows = sqlx::query(&query).fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let columns: Vec<String> = if let Some(row) = rows.first() {
        row.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        vec![]
    };

    let data: Vec<Vec<serde_json::Value>> = rows.iter().map(|row| {
        columns.iter().enumerate().map(|(i, _)| {
            row.try_get::<String, _>(i)
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        }).collect()
    }).collect();

    Ok(QueryResult {
        columns,
        rows: data,
        affected: rows.len() as u64,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
pub async fn db_tables(connection_string: String) -> Result<Vec<String>, String> {
    let pool = SqlitePool::connect(&connection_string).await.map_err(|e| e.to_string())?;
    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(&pool).await.map_err(|e| e.to_string())?;
    Ok(rows.iter().map(|r| r.get::<String, _>(0)).collect())
}
```

**Step 2: Create DatabasePanel.svelte** with BenikUI, schema browser, query editor, result table

**Step 3: Add sqlx dependency** to Cargo.toml: `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }`

**Step 4: Register commands, add to bottom panel tabs**

**Step 5: Commit**

---

### Task 20: HTTP Client Panel

**Files:**
- Create: `src-tauri/src/ide/http_client.rs`
- Create: `src/routes/ide/HttpClient.svelte`

**Step 1: Create http_client.rs**

```rust
#[derive(serde::Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(serde::Serialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub elapsed_ms: u64,
    pub size_bytes: usize,
}

#[tauri::command]
pub async fn http_send(request: HttpRequest) -> Result<HttpResponse, String> {
    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let method = request.method.parse::<reqwest::Method>().map_err(|e| e.to_string())?;

    let mut builder = client.request(method, &request.url);
    for (k, v) in &request.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let resp = builder.send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let headers: Vec<(String, String)> = resp.headers().iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp.text().await.map_err(|e| e.to_string())?;

    Ok(HttpResponse {
        status,
        headers,
        size_bytes: body.len(),
        body,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}
```

**Step 2: Create HttpClient.svelte** — Method selector, URL input, headers editor, body editor, response viewer, status badge. BenikUI from start.

**Step 3: Add reqwest** to Cargo.toml (if not present)

**Step 4: Register, add to panels**

**Step 5: Commit**

---

## Phase 4: AI-First Features

### Task 21: Multi-Agent Sessions (Foundation)

**Files:**
- Create: `src-tauri/src/ide/agent_sessions.rs`
- Create: `src/routes/ide/AgentSessions.svelte`

This is the most complex task. The agent session manager:
- Spawns independent agent contexts
- Each agent has own PTY, file access, tool-use capability
- Reports progress via Tauri events
- Self-healing: detect test failure → analyze → fix → re-run

**Step 1: Create agent_sessions.rs** with session lifecycle (create, run, pause, stop)
**Step 2: Create AgentSessions.svelte** with tab-based agent view, BenikUI integrated
**Step 3: Wire to right sidebar
**Step 4: Commit**

---

### Task 22: Next-Edit Prediction

**Files:**
- Modify: `src-tauri/src/ide/ai_complete.rs`
- Modify: `src/routes/ide/CodeEditor.svelte`

**Step 1: Add predict_next_edit command**
**Step 2: Track edit history in CodeEditor (last 5 edits with position + diff)
**Step 3: Show ghost decoration at predicted location
**Step 4: Tab accepts, Shift+Tab cycles
**Step 5: Commit**

---

### Task 23: Spec-Driven Development Panel

**Files:**
- Create: `src-tauri/src/ide/spec_engine.rs`
- Create: `src/routes/ide/SpecPanel.svelte`

**Step 1: Create spec_engine.rs** — parse requirements, generate structured spec
**Step 2: Create SpecPanel.svelte** — markdown editor + Mermaid preview + task list. BenikUI integrated.
**Step 3: Wire to right sidebar
**Step 4: Commit**

---

### Task 24: Knowledge Graph Code Visualization

**Files:**
- Modify: `src-tauri/src/ide/indexer.rs`
- Create: `src/routes/ide/CodeGraph.svelte`

**Step 1: Extend indexer** to build symbol relationship graph
**Step 2: Create CodeGraph.svelte** — SVG-based graph view, BenikUI integrated
**Step 3: Wire to bottom panel tabs
**Step 4: Commit**

---

## Verification

After all tasks:

```bash
# Rust build + tests
cd /opt/ork-station/ImpForge && cargo check 2>&1 | tail -5
cargo test 2>&1 | tail -10

# Svelte check
cd /opt/ork-station/ImpForge && npx svelte-check 2>&1 | tail -5

# Verify all BenikUI integrations
grep -r "widgetId.*=.*'ide-" src/routes/ide/ | wc -l
# Expected: 13+ (one per panel)
```
