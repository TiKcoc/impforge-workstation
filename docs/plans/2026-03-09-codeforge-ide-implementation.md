# CodeForge IDE Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform the current basic IDE page (416 LoC) into a full Cursor-killer IDE with real terminal, LSP, AI completion, codebase indexing, and team collaboration.

**Architecture:** Component-Split IDE. The `/ide` route becomes a thin PaneForge shell orchestrating 8 Svelte 5 components. Rust backend extended with PTY, LSP proxy, Git, Codebase Indexing, and Shadow Workspace modules.

**Tech Stack:** Tauri 2.10, Svelte 5 (runes), Rust, Monaco Editor, xterm.js 6, portable-pty, monaco-languageclient v10, monacopilot, FastEmbed 5.12, LanceDB 0.26, Tree-sitter, async-lsp

**Design Doc:** `docs/plans/2026-03-09-codeforge-ide-design.md`

---

## Phase 1: Core IDE (Tasks 1–10)

The foundation. After Phase 1, CodeForge has: real terminal, file explorer with fuzzy search, Monaco editor in resizable panels, and a status bar.

---

### Task 1: Install npm Dependencies for Phase 1

**Files:**
- Modify: `package.json`

**Step 1: Install terminal addons and fuzzy search**

Run:
```bash
cd /opt/ork-station/ImpForge
pnpm add @xterm/addon-web-links @xterm/addon-search fuse.js
```

Expected: packages added to `dependencies` in `package.json`

**Step 2: Verify installation**

Run:
```bash
cd /opt/ork-station/ImpForge
pnpm ls @xterm/addon-web-links @xterm/addon-search fuse.js
```

Expected: All three packages listed with versions

**Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "deps: add xterm addons and fuse.js for CodeForge IDE"
```

---

### Task 2: Add portable-pty Cargo Dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add portable-pty to [dependencies]**

In `src-tauri/Cargo.toml`, add after the existing dependencies block:

```toml
# Terminal PTY
portable-pty = "0.9"
```

**Step 2: Verify it compiles**

Run:
```bash
cd /opt/ork-station/ImpForge/src-tauri
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors (warnings OK)

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: add portable-pty for real terminal support"
```

---

### Task 3: Rust PTY Backend (pty.rs)

**Files:**
- Create: `src-tauri/src/ide/pty.rs`
- Modify: `src-tauri/src/ide/mod.rs` (add `pub mod pty;`)
- Modify: `src-tauri/src/lib.rs` (register new commands in `invoke_handler`)

**Step 1: Create pty.rs with PTY management**

Create `src-tauri/src/ide/pty.rs`:

```rust
//! PTY management for real terminal support
//!
//! Uses portable-pty to spawn shell processes with full PTY support.
//! Each terminal tab gets its own PTY session with a unique ID.
//! Communication with the frontend happens via Tauri events (bidirectional).

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

static NEXT_PTY_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PtyOutput {
    pub id: u32,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PtyInfo {
    pub id: u32,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
}

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send>,
    shell: String,
    cols: u16,
    rows: u16,
}

pub struct PtyManager {
    sessions: Mutex<HashMap<u32, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    shell: Option<String>,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<PtyInfo, String> {
    let pty_system = native_pty_system();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let shell_path = shell.clone().unwrap_or_else(|| {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    });

    let mut cmd = CommandBuilder::new(&shell_path);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    if let Some(dir) = &cwd {
        cmd.cwd(dir);
    } else if let Some(home) = dirs::home_dir() {
        cmd.cwd(home);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let id = NEXT_PTY_ID.fetch_add(1, Ordering::SeqCst);

    // Spawn reader thread that emits PTY output to frontend
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let app_clone = app.clone();
    let event_name = format!("pty-output-{}", id);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit(&event_name, PtyOutput { id, data });
                }
                Err(_) => break,
            }
        }
        // PTY closed — notify frontend
        let _ = app_clone.emit(&format!("pty-exit-{}", id), id);
    });

    let info = PtyInfo {
        id,
        shell: shell_path.clone(),
        cols,
        rows,
    };

    let manager = app.state::<PtyManager>();
    manager.sessions.lock().await.insert(
        id,
        PtySession {
            master: pair.master,
            _child: child,
            shell: shell_path,
            cols,
            rows,
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn pty_write(app: AppHandle, id: u32, data: String) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("PTY session {} not found", id))?;

    session
        .master
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write to PTY: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn pty_resize(app: AppHandle, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("PTY session {} not found", id))?;

    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to resize PTY: {}", e))?;

    session.cols = cols;
    session.rows = rows;

    Ok(())
}

#[tauri::command]
pub async fn pty_kill(app: AppHandle, id: u32) -> Result<(), String> {
    let manager = app.state::<PtyManager>();
    let mut sessions = manager.sessions.lock().await;

    if sessions.remove(&id).is_some() {
        Ok(())
    } else {
        Err(format!("PTY session {} not found", id))
    }
}

#[tauri::command]
pub async fn pty_list(app: AppHandle) -> Result<Vec<PtyInfo>, String> {
    let manager = app.state::<PtyManager>();
    let sessions = manager.sessions.lock().await;

    Ok(sessions
        .iter()
        .map(|(id, s)| PtyInfo {
            id: *id,
            shell: s.shell.clone(),
            cols: s.cols,
            rows: s.rows,
        })
        .collect())
}
```

**Step 2: Register pty module in ide/mod.rs**

Add at the top of `src-tauri/src/ide/mod.rs`:

```rust
pub mod pty;
```

**Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, find the `.invoke_handler(tauri::generate_handler![` block and add after the existing ide commands:

```rust
            ide::pty::pty_spawn,
            ide::pty::pty_write,
            ide::pty::pty_resize,
            ide::pty::pty_kill,
            ide::pty::pty_list,
```

Also add the PtyManager state. Find the `.setup(|app| {` block and add:

```rust
            app.manage(ide::pty::PtyManager::new());
```

**Step 4: Verify it compiles**

Run:
```bash
cd /opt/ork-station/ImpForge/src-tauri
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors

**Step 5: Commit**

```bash
git add src-tauri/src/ide/pty.rs src-tauri/src/ide/mod.rs src-tauri/src/lib.rs
git commit -m "feat(ide): add PTY backend with portable-pty for real terminal"
```

---

### Task 4: Terminal Store (terminal.svelte.ts)

**Files:**
- Create: `src/lib/stores/terminal.svelte.ts`

**Step 1: Create the terminal store**

Create `src/lib/stores/terminal.svelte.ts`:

```typescript
/**
 * Terminal Store — PTY session management for xterm.js
 *
 * Manages multiple terminal tabs, each backed by a real PTY process.
 * Uses Tauri events for bidirectional PTY communication.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';

export interface PtyInfo {
	id: number;
	shell: string;
	cols: number;
	rows: number;
}

export interface TerminalTab {
	ptyId: number;
	title: string;
	shell: string;
	unlisten?: UnlistenFn;
	unlistenExit?: UnlistenFn;
}

class TerminalStore {
	tabs = $state<TerminalTab[]>([]);
	activeIndex = $state(0);
	onData = $state<((id: number, data: string) => void) | null>(null);
	onExit = $state<((id: number) => void) | null>(null);

	get activeTab(): TerminalTab | null {
		return this.tabs[this.activeIndex] || null;
	}

	async spawn(cwd?: string): Promise<number> {
		const info = await invoke<PtyInfo>('pty_spawn', { shell: null, cwd, cols: 80, rows: 24 });

		const unlisten = await listen<{ id: number; data: string }>(
			`pty-output-${info.id}`,
			(event) => {
				this.onData?.(event.payload.id, event.payload.data);
			}
		);

		const unlistenExit = await listen<number>(`pty-exit-${info.id}`, (event) => {
			this.onExit?.(event.payload);
			this.removeTab(info.id);
		});

		const tab: TerminalTab = {
			ptyId: info.id,
			title: `Terminal ${this.tabs.length + 1}`,
			shell: info.shell,
			unlisten,
			unlistenExit,
		};

		this.tabs = [...this.tabs, tab];
		this.activeIndex = this.tabs.length - 1;

		return info.id;
	}

	async write(id: number, data: string): Promise<void> {
		await invoke('pty_write', { id, data });
	}

	async resize(id: number, cols: number, rows: number): Promise<void> {
		await invoke('pty_resize', { id, cols, rows });
	}

	async kill(id: number): Promise<void> {
		const tab = this.tabs.find((t) => t.ptyId === id);
		if (tab) {
			tab.unlisten?.();
			tab.unlistenExit?.();
		}
		await invoke('pty_kill', { id });
		this.removeTab(id);
	}

	private removeTab(ptyId: number) {
		this.tabs = this.tabs.filter((t) => t.ptyId !== ptyId);
		if (this.activeIndex >= this.tabs.length) {
			this.activeIndex = Math.max(0, this.tabs.length - 1);
		}
	}

	async killAll(): Promise<void> {
		for (const tab of this.tabs) {
			tab.unlisten?.();
			tab.unlistenExit?.();
			try {
				await invoke('pty_kill', { id: tab.ptyId });
			} catch {
				// Already dead
			}
		}
		this.tabs = [];
		this.activeIndex = 0;
	}
}

export const terminalStore = new TerminalStore();
```

**Step 2: Commit**

```bash
git add src/lib/stores/terminal.svelte.ts
git commit -m "feat(ide): add terminal store for PTY session management"
```

---

### Task 5: IdeTerminal Component (xterm.js + PTY)

**Files:**
- Create: `src/routes/ide/IdeTerminal.svelte`

**Step 1: Create the terminal component**

Create `src/routes/ide/IdeTerminal.svelte`:

```svelte
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Plus, X } from '@lucide/svelte';
	import { terminalStore } from '$lib/stores/terminal.svelte';

	let terminalContainer = $state<HTMLDivElement>(undefined!);
	let terminal: any = null;
	let fitAddon: any = null;
	let webLinksAddon: any = null;
	let searchAddon: any = null;
	let terminalInstances = new Map<number, any>();
	let resizeObserver: ResizeObserver | null = null;

	onMount(async () => {
		const { Terminal } = await import('@xterm/xterm');
		const { FitAddon } = await import('@xterm/addon-fit');
		const { WebLinksAddon } = await import('@xterm/addon-web-links');
		const { SearchAddon } = await import('@xterm/addon-search');

		// Set up data callback
		terminalStore.onData = (id: number, data: string) => {
			const term = terminalInstances.get(id);
			if (term) term.write(data);
		};

		// Spawn first terminal
		const ptyId = await terminalStore.spawn();
		createTerminalInstance(ptyId, Terminal, FitAddon, WebLinksAddon, SearchAddon);

		// Auto-resize on container change
		resizeObserver = new ResizeObserver(() => {
			if (fitAddon && terminal) {
				fitAddon.fit();
				const dims = fitAddon.proposeDimensions();
				if (dims && terminalStore.activeTab) {
					terminalStore.resize(terminalStore.activeTab.ptyId, dims.cols, dims.rows);
				}
			}
		});
		resizeObserver.observe(terminalContainer);
	});

	function createTerminalInstance(
		ptyId: number,
		Terminal: any,
		FitAddon: any,
		WebLinksAddon: any,
		SearchAddon: any
	) {
		const term = new Terminal({
			cursorBlink: true,
			cursorStyle: 'bar',
			fontSize: 13,
			fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
			theme: {
				background: '#0a0e14',
				foreground: '#e0e0e0',
				cursor: '#00FF66',
				cursorAccent: '#0a0e14',
				selectionBackground: '#1a3a5c',
				black: '#1a1e28',
				red: '#ff5370',
				green: '#00FF66',
				yellow: '#ffcb6b',
				blue: '#82aaff',
				magenta: '#c792ea',
				cyan: '#89ddff',
				white: '#e0e0e0',
			},
			allowProposedApi: true,
		});

		const fit = new FitAddon();
		const webLinks = new WebLinksAddon();
		const search = new SearchAddon();

		term.loadAddon(fit);
		term.loadAddon(webLinks);
		term.loadAddon(search);

		term.open(terminalContainer);
		fit.fit();

		term.onData((data: string) => {
			terminalStore.write(ptyId, data);
		});

		terminalInstances.set(ptyId, term);
		terminal = term;
		fitAddon = fit;
		searchAddon = search;

		// Resize after mount
		requestAnimationFrame(() => {
			fit.fit();
			const dims = fit.proposeDimensions();
			if (dims) {
				terminalStore.resize(ptyId, dims.cols, dims.rows);
			}
		});
	}

	async function addTerminal() {
		const { Terminal } = await import('@xterm/xterm');
		const { FitAddon } = await import('@xterm/addon-fit');
		const { WebLinksAddon } = await import('@xterm/addon-web-links');
		const { SearchAddon } = await import('@xterm/addon-search');

		const ptyId = await terminalStore.spawn();

		// Hide current terminal
		if (terminal) {
			terminal.element?.remove();
		}

		createTerminalInstance(ptyId, Terminal, FitAddon, WebLinksAddon, SearchAddon);
	}

	function switchTab(index: number) {
		if (terminalStore.activeIndex === index) return;

		// Hide current
		if (terminal) terminal.element?.remove();

		terminalStore.activeIndex = index;
		const tab = terminalStore.activeTab;
		if (tab) {
			const term = terminalInstances.get(tab.ptyId);
			if (term) {
				term.open(terminalContainer);
				terminal = term;
				fitAddon?.fit();
			}
		}
	}

	async function closeTab(index: number) {
		const tab = terminalStore.tabs[index];
		if (!tab) return;

		const term = terminalInstances.get(tab.ptyId);
		if (term) {
			term.dispose();
			terminalInstances.delete(tab.ptyId);
		}

		await terminalStore.kill(tab.ptyId);

		// Switch to remaining tab
		if (terminalStore.activeTab) {
			const t = terminalInstances.get(terminalStore.activeTab.ptyId);
			if (t) {
				t.open(terminalContainer);
				terminal = t;
				fitAddon?.fit();
			}
		}
	}

	onDestroy(() => {
		resizeObserver?.disconnect();
		terminalInstances.forEach((t) => t.dispose());
		terminalStore.killAll();
	});
</script>

<div class="flex flex-col h-full">
	<!-- Terminal tabs -->
	<div class="flex items-center h-7 bg-gx-bg-secondary border-b border-gx-border-default px-1 shrink-0">
		{#each terminalStore.tabs as tab, i}
			<button
				onclick={() => switchTab(i)}
				class="flex items-center gap-1 px-2 h-full text-[11px] border-r border-gx-border-default
					{i === terminalStore.activeIndex
						? 'text-gx-neon bg-gx-bg-primary'
						: 'text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				<span class="truncate max-w-[80px]">{tab.title}</span>
				{#if terminalStore.tabs.length > 1}
					<button
						onclick={(e) => { e.stopPropagation(); closeTab(i); }}
						class="ml-0.5 p-0.5 rounded hover:bg-gx-bg-elevated"
					>
						<X size={10} />
					</button>
				{/if}
			</button>
		{/each}
		<button
			onclick={addTerminal}
			class="p-1 text-gx-text-muted hover:text-gx-neon"
			title="New Terminal"
		>
			<Plus size={12} />
		</button>
	</div>

	<!-- Terminal output -->
	<div bind:this={terminalContainer} class="flex-1 min-h-0 bg-[#0a0e14]"></div>
</div>

<style>
	:global(.xterm) {
		padding: 4px;
	}
</style>
```

**Step 2: Commit**

```bash
git add src/routes/ide/IdeTerminal.svelte
git commit -m "feat(ide): add IdeTerminal component with xterm.js 6 + PTY"
```

---

### Task 6: FileExplorer Component (Extract + Fuse.js)

**Files:**
- Create: `src/routes/ide/FileExplorer.svelte`

**Step 1: Create FileExplorer component**

Extract the file tree from `+page.svelte` into its own component. Create `src/routes/ide/FileExplorer.svelte`:

```svelte
<script lang="ts">
	import {
		FolderOpen, FolderClosed, Search, ChevronRight,
		ChevronDown, ArrowUp, RotateCcw, Loader2, FileText
	} from '@lucide/svelte';
	import { ide, type FileEntry } from '$lib/stores/ide.svelte';
	import Fuse from 'fuse.js';

	let searchQuery = $state('');
	let searchMode = $state(false);
	let fuseResults = $state<FileEntry[]>([]);

	const fuse = $derived(
		new Fuse(collectAllFiles(), {
			keys: ['name', 'path'],
			threshold: 0.4,
			includeScore: true,
		})
	);

	function collectAllFiles(): FileEntry[] {
		const all: FileEntry[] = [...ide.files];
		for (const [, entries] of ide.subDirFiles) {
			all.push(...entries);
		}
		return all.filter((f) => !f.is_dir);
	}

	function handleSearch() {
		if (!searchQuery.trim()) {
			fuseResults = [];
			return;
		}
		fuseResults = fuse.search(searchQuery).map((r) => r.item).slice(0, 20);
	}

	function goUp() {
		const parent = ide.currentDir.split('/').slice(0, -1).join('/') || '/';
		ide.loadDirectory(parent);
	}

	function getFileIcon(entry: FileEntry): string {
		if (entry.is_dir) return '';
		const ext = entry.extension?.toLowerCase() || '';
		const icons: Record<string, string> = {
			rs: '🦀', py: '🐍', ts: '💎', js: '⚡', svelte: '🔥',
			json: '{}', toml: '⚙️', md: '📝', css: '🎨', html: '🌐',
		};
		return icons[ext] || '📄';
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes}B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
		return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
	}
</script>

<div class="flex flex-col h-full bg-gx-bg-secondary overflow-hidden">
	<!-- Header -->
	<div class="flex items-center gap-1 px-2 py-1.5 border-b border-gx-border-default shrink-0">
		<span class="text-[11px] font-semibold text-gx-text-muted uppercase tracking-wider">Explorer</span>
		<div class="flex-1"></div>
		<button onclick={() => searchMode = !searchMode} class="p-0.5 text-gx-text-muted hover:text-gx-neon">
			<Search size={12} />
		</button>
		<button onclick={goUp} class="p-0.5 text-gx-text-muted hover:text-gx-neon">
			<ArrowUp size={12} />
		</button>
		<button onclick={() => ide.loadDirectory(ide.currentDir)} class="p-0.5 text-gx-text-muted hover:text-gx-neon">
			<RotateCcw size={12} />
		</button>
	</div>

	<!-- Fuzzy search -->
	{#if searchMode}
		<div class="px-2 py-1 border-b border-gx-border-default">
			<input
				type="text"
				bind:value={searchQuery}
				oninput={handleSearch}
				placeholder="Search files..."
				class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded px-2 py-1 text-xs text-gx-text-primary placeholder:text-gx-text-muted outline-none focus:border-gx-neon"
			/>
		</div>
		{#if fuseResults.length > 0}
			<div class="max-h-40 overflow-auto border-b border-gx-border-default">
				{#each fuseResults as entry}
					<button
						onclick={() => ide.openFile(entry.path, entry.name)}
						class="flex items-center gap-1.5 w-full px-3 py-1 text-xs hover:bg-gx-bg-hover text-left"
					>
						<span class="text-[10px] shrink-0">{getFileIcon(entry)}</span>
						<span class="text-gx-accent-cyan truncate">{entry.name}</span>
						<span class="text-[9px] text-gx-text-muted truncate ml-auto">{entry.path.split('/').slice(-2, -1)[0]}</span>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	<!-- File tree -->
	<div class="flex-1 overflow-auto text-xs">
		{#if ide.loading}
			<div class="flex items-center justify-center py-8">
				<Loader2 size={16} class="animate-spin text-gx-text-muted" />
			</div>
		{:else}
			{#each ide.files as entry}
				{@render fileTreeEntry(entry, 0)}
			{/each}
		{/if}
	</div>

	<!-- Current path -->
	<div class="px-2 py-1 border-t border-gx-border-default shrink-0">
		<span class="text-[10px] text-gx-text-muted truncate block">{ide.currentDir}</span>
	</div>
</div>

{#snippet fileTreeEntry(entry: FileEntry, depth: number)}
	<button
		onclick={() => entry.is_dir ? ide.toggleDir(entry) : ide.openFile(entry.path, entry.name)}
		class="flex items-center gap-1.5 w-full px-2 py-1 hover:bg-gx-bg-hover text-left group"
		style="padding-left: {8 + depth * 16}px"
	>
		{#if entry.is_dir}
			{#if ide.expandedDirs.has(entry.path)}
				<ChevronDown size={12} class="text-gx-text-muted shrink-0" />
				<FolderOpen size={14} class="text-gx-accent-orange shrink-0" />
			{:else}
				<ChevronRight size={12} class="text-gx-text-muted shrink-0" />
				<FolderClosed size={14} class="text-gx-accent-orange shrink-0" />
			{/if}
			<span class="text-gx-text-secondary truncate">{entry.name}</span>
		{:else}
			<span class="w-3 shrink-0"></span>
			<span class="text-[10px] shrink-0">{getFileIcon(entry)}</span>
			<span class="text-gx-text-secondary truncate">{entry.name}</span>
			<span class="ml-auto text-[10px] text-gx-text-muted opacity-0 group-hover:opacity-100">
				{formatSize(entry.size)}
			</span>
		{/if}
	</button>

	{#if entry.is_dir && ide.expandedDirs.has(entry.path)}
		{#each ide.subDirFiles.get(entry.path) || [] as subEntry}
			{@render fileTreeEntry(subEntry, depth + 1)}
		{/each}
	{/if}
{/snippet}
```

**Step 2: Commit**

```bash
git add src/routes/ide/FileExplorer.svelte
git commit -m "feat(ide): extract FileExplorer component with Fuse.js fuzzy search"
```

---

### Task 7: StatusBar Component

**Files:**
- Create: `src/routes/ide/IdeStatusBar.svelte`

**Step 1: Create StatusBar component**

Create `src/routes/ide/IdeStatusBar.svelte`:

```svelte
<script lang="ts">
	import { GitBranch, Circle, Cpu } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	interface Props {
		lspStatus?: string;
		gitBranch?: string;
		aiModel?: string;
	}

	let { lspStatus = 'disconnected', gitBranch = '', aiModel = 'Ollama' }: Props = $props();

	const cursorInfo = $derived(() => {
		const tab = ide.activeTab;
		if (!tab) return 'Ln 1, Col 1';
		return `Ln -, Col -`; // Updated by Monaco cursor events
	});

	const language = $derived(ide.activeTab?.language || 'plaintext');
	const encoding = 'UTF-8';

	const lspColor = $derived(
		lspStatus === 'running' ? 'text-green-400' :
		lspStatus === 'starting' ? 'text-yellow-400' :
		'text-gx-text-muted'
	);
</script>

<div class="flex items-center h-6 px-2 bg-gx-bg-secondary border-t border-gx-border-default text-[11px] text-gx-text-muted shrink-0 gap-3">
	<!-- LSP Status -->
	<div class="flex items-center gap-1">
		<Circle size={8} class="{lspColor} fill-current" />
		<span>LSP</span>
	</div>

	<!-- Git Branch -->
	{#if gitBranch}
		<div class="flex items-center gap-1">
			<GitBranch size={12} />
			<span>{gitBranch}</span>
		</div>
	{/if}

	<div class="flex-1"></div>

	<!-- Cursor Position -->
	<span>{cursorInfo()}</span>

	<!-- Encoding -->
	<span>{encoding}</span>

	<!-- Language -->
	<span class="text-gx-accent-cyan">{language}</span>

	<!-- AI Model -->
	<div class="flex items-center gap-1">
		<Cpu size={10} class="text-gx-neon" />
		<span class="text-gx-neon">{aiModel}</span>
	</div>
</div>
```

**Step 2: Commit**

```bash
git add src/routes/ide/IdeStatusBar.svelte
git commit -m "feat(ide): add StatusBar component with LSP, git, cursor info"
```

---

### Task 8: CodeEditor Component (Extract Monaco)

**Files:**
- Create: `src/routes/ide/CodeEditor.svelte`

**Step 1: Create CodeEditor component**

Extract Monaco editor logic from `+page.svelte` into its own component. Create `src/routes/ide/CodeEditor.svelte`:

```svelte
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Code2 } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	interface Props {
		onCursorChange?: (line: number, col: number) => void;
	}

	let { onCursorChange }: Props = $props();

	let editorContainer = $state<HTMLDivElement>(undefined!);
	let monacoEditor: any = null;
	let monacoModule: any = null;

	onMount(async () => {
		monacoModule = await import('monaco-editor');

		monacoModule.editor.defineTheme('impforge-dark', {
			base: 'vs-dark',
			inherit: true,
			rules: [
				{ token: 'comment', foreground: '5a6a7a', fontStyle: 'italic' },
				{ token: 'keyword', foreground: 'c792ea' },
				{ token: 'string', foreground: 'c3e88d' },
				{ token: 'number', foreground: 'f78c6c' },
				{ token: 'type', foreground: 'ffcb6b' },
				{ token: 'function', foreground: '82aaff' },
				{ token: 'variable', foreground: 'eeffff' },
				{ token: 'operator', foreground: '89ddff' },
				{ token: 'constant', foreground: 'f78c6c' },
				{ token: 'tag', foreground: 'ff5370' },
				{ token: 'attribute.name', foreground: 'ffcb6b' },
				{ token: 'attribute.value', foreground: 'c3e88d' },
			],
			colors: {
				'editor.background': '#0a0e14',
				'editor.foreground': '#e0e0e0',
				'editor.lineHighlightBackground': '#141820',
				'editor.selectionBackground': '#1a3a5c',
				'editorCursor.foreground': '#00FF66',
				'editorLineNumber.foreground': '#3a4a5a',
				'editorLineNumber.activeForeground': '#00FF66',
				'editor.selectionHighlightBackground': '#1a3a5c55',
				'editorIndentGuide.background': '#1a1e28',
				'editorIndentGuide.activeBackground': '#2a3a4a',
				'editorBracketMatch.background': '#1a3a5c44',
				'editorBracketMatch.border': '#00FF6644',
			},
		});
	});

	$effect(() => {
		const tab = ide.activeTab;
		if (tab && editorContainer && monacoModule) {
			if (monacoEditor) {
				const model = monacoEditor.getModel();
				if (model) {
					const currentValue = model.getValue();
					if (currentValue !== tab.content) model.setValue(tab.content);
					monacoModule.editor.setModelLanguage(model, tab.language);
				}
			} else {
				monacoEditor = monacoModule.editor.create(editorContainer, {
					value: tab.content,
					language: tab.language,
					theme: 'impforge-dark',
					fontSize: 13,
					fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
					fontLigatures: true,
					minimap: { enabled: true, maxColumn: 80 },
					scrollBeyondLastLine: false,
					smoothScrolling: true,
					cursorBlinking: 'smooth',
					cursorSmoothCaretAnimation: 'on',
					renderLineHighlight: 'all',
					bracketPairColorization: { enabled: true },
					automaticLayout: true,
					padding: { top: 8 },
					lineNumbers: 'on',
					wordWrap: 'on',
					tabSize: 2,
					suggest: {
						showMethods: true,
						showFunctions: true,
						showVariables: true,
						showClasses: true,
						preview: true,
					},
				});

				monacoEditor.onDidChangeModelContent(() => {
					const newContent = monacoEditor.getModel()?.getValue() || '';
					ide.updateTabContent(ide.activeTabIndex, newContent);
				});

				monacoEditor.onDidChangeCursorPosition((e: any) => {
					onCursorChange?.(e.position.lineNumber, e.position.column);
				});

				monacoEditor.addCommand(
					monacoModule.KeyMod.CtrlCmd | monacoModule.KeyCode.KeyS,
					() => ide.saveFile(ide.activeTabIndex)
				);
			}
		}
	});

	onDestroy(() => {
		if (monacoEditor) monacoEditor.dispose();
	});

	export function getEditor() {
		return monacoEditor;
	}

	export function getMonaco() {
		return monacoModule;
	}
</script>

<div class="flex-1 min-h-0 relative">
	{#if ide.openTabs.length > 0}
		<div bind:this={editorContainer} class="absolute inset-0"></div>
	{:else}
		<div class="flex flex-col items-center justify-center h-full text-gx-text-muted gap-4">
			<Code2 size={48} class="opacity-20" />
			<div class="text-center">
				<p class="text-sm font-medium">CodeForge IDE</p>
				<p class="text-xs mt-1">Open a file from the explorer or press Ctrl+P</p>
			</div>
			<div class="flex flex-col gap-1 text-xs text-gx-text-muted mt-2">
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+S</kbd> Save file</span>
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+P</kbd> Quick Open</span>
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+`</kbd> Toggle Terminal</span>
			</div>
		</div>
	{/if}
</div>
```

**Step 2: Commit**

```bash
git add src/routes/ide/CodeEditor.svelte
git commit -m "feat(ide): extract CodeEditor component with enhanced Monaco theme"
```

---

### Task 9: AiAgent Component (Extract Chat)

**Files:**
- Create: `src/routes/ide/AiAgent.svelte`

**Step 1: Create AiAgent component**

Extract AI agent chat from `+page.svelte`. Create `src/routes/ide/AiAgent.svelte`:

```svelte
<script lang="ts">
	import { Bot, Play, Send, Loader2, Sparkles } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	let agentInput = $state('');

	async function handleSend() {
		if (agentInput.trim() && !ide.agentLoading) {
			const msg = agentInput;
			agentInput = '';
			await ide.sendAgentMessage(msg);
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	const suggestions = [
		'Read this file',
		'Find TODO comments',
		'Explain this code',
		'Write tests for this',
		'Refactor this function',
		'Find similar code',
	];
</script>

<div class="flex flex-col h-full bg-gx-bg-secondary">
	<!-- Header -->
	<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default shrink-0">
		<Sparkles size={14} class="text-gx-neon" />
		<span class="text-xs font-semibold text-gx-text-primary">AI Agent</span>
	</div>

	<!-- Messages -->
	<div class="flex-1 overflow-auto p-2 space-y-2">
		{#if ide.agentMessages.length === 0}
			<div class="text-center py-6">
				<Bot size={32} class="mx-auto text-gx-text-muted opacity-30 mb-2" />
				<p class="text-xs text-gx-text-muted">AI coding agent ready.</p>
				<p class="text-[10px] text-gx-text-muted mt-1">Can read, write, search, and execute.</p>
				<div class="flex flex-wrap gap-1.5 justify-center mt-3">
					{#each suggestions as suggestion}
						<button
							onclick={() => { agentInput = suggestion; handleSend(); }}
							class="text-[11px] px-2 py-1 bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/50 transition-all"
						>
							{suggestion}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		{#each ide.agentMessages as msg}
			<div class="flex gap-2 {msg.role === 'user' ? 'justify-end' : ''}">
				{#if msg.role === 'user'}
					<div class="max-w-[85%] px-3 py-1.5 rounded-gx bg-gx-neon/10 border border-gx-neon/20 text-xs text-gx-text-primary">
						{msg.content}
					</div>
				{:else if msg.role === 'tool'}
					<div class="max-w-[95%] px-2 py-1 rounded bg-gx-bg-tertiary border border-gx-border-default text-[11px] font-mono">
						<div class="flex items-center gap-1 text-gx-accent-cyan mb-0.5">
							<Play size={10} />
							{msg.toolCall?.tool}
						</div>
						<pre class="text-gx-text-muted whitespace-pre-wrap max-h-24 overflow-auto">{msg.content.slice(0, 500)}{msg.content.length > 500 ? '...' : ''}</pre>
					</div>
				{:else}
					<div class="max-w-[95%] px-3 py-1.5 rounded-gx bg-gx-bg-elevated border border-gx-border-default text-xs text-gx-text-secondary">
						<pre class="whitespace-pre-wrap font-sans">{msg.content}</pre>
					</div>
				{/if}
			</div>
		{/each}

		{#if ide.agentLoading}
			<div class="flex items-center gap-2 text-xs text-gx-text-muted">
				<Loader2 size={12} class="animate-spin" />
				<span>Thinking...</span>
			</div>
		{/if}
	</div>

	<!-- Input -->
	<div class="flex items-end gap-2 px-2 py-1.5 border-t border-gx-border-default">
		<textarea
			bind:value={agentInput}
			onkeydown={handleKeydown}
			placeholder="Ask the AI agent..."
			rows="1"
			class="flex-1 bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-2 py-1.5 text-xs text-gx-text-primary placeholder:text-gx-text-muted resize-none outline-none focus:border-gx-neon transition-colors"
		></textarea>
		<button
			onclick={handleSend}
			disabled={ide.agentLoading || !agentInput.trim()}
			class="p-1.5 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
		>
			<Send size={14} />
		</button>
	</div>
</div>
```

**Step 2: Commit**

```bash
git add src/routes/ide/AiAgent.svelte
git commit -m "feat(ide): extract AiAgent component with enhanced suggestions"
```

---

### Task 10: IDE Shell — PaneForge Assembly

**Files:**
- Modify: `src/routes/ide/+page.svelte` (complete rewrite to shell)

**Step 1: Rewrite +page.svelte as thin PaneForge shell**

Replace the entire content of `src/routes/ide/+page.svelte` with the PaneForge shell that assembles all components:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import { Code2, Bot, Terminal, AlertTriangle, X } from '@lucide/svelte';
	import { Pane } from 'paneforge';
	import { PaneGroup, Handle } from '$lib/components/ui/resizable/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { ide } from '$lib/stores/ide.svelte';
	import FileExplorer from './FileExplorer.svelte';
	import CodeEditor from './CodeEditor.svelte';
	import IdeTerminal from './IdeTerminal.svelte';
	import AiAgent from './AiAgent.svelte';
	import IdeStatusBar from './IdeStatusBar.svelte';

	// Panel visibility
	let showAiPanel = $state(true);
	let bottomPanel = $state<'terminal' | 'problems'>('terminal');
	let cursorLine = $state(1);
	let cursorCol = $state(1);

	onMount(() => {
		ide.loadDirectory('/home');
	});

	function handleCursorChange(line: number, col: number) {
		cursorLine = line;
		cursorCol = col;
	}
</script>

<div class="flex flex-col h-full overflow-hidden">
	<!-- IDE Top Bar -->
	<div class="flex items-center h-9 px-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0 gap-2">
		<Code2 size={16} class="text-gx-neon" />
		<span class="text-sm font-semibold text-gx-neon">CodeForge</span>
		<Separator orientation="vertical" class="h-4 bg-gx-border-default" />
		<span class="text-xs text-gx-text-muted truncate">{ide.currentDir}</span>
		<div class="flex-1"></div>
		<button
			onclick={() => showAiPanel = !showAiPanel}
			class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
			title="Toggle AI Panel"
		>
			<Bot size={14} />
		</button>
	</div>

	<!-- Main IDE Layout -->
	<div class="flex-1 min-h-0">
		<PaneGroup direction="horizontal">
			<!-- File Explorer -->
			<Pane defaultSize={18} minSize={12} maxSize={30}>
				<FileExplorer />
			</Pane>

			<Handle />

			<!-- Editor + Bottom Panel -->
			<Pane defaultSize={showAiPanel ? 57 : 82} minSize={30}>
				<PaneGroup direction="vertical">
					<!-- Editor area -->
					<Pane defaultSize={65} minSize={20}>
						<div class="flex flex-col h-full">
							<!-- Tab bar -->
							<div class="flex items-center h-8 bg-gx-bg-secondary border-b border-gx-border-default overflow-x-auto shrink-0">
								{#each ide.openTabs as tab, i}
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<div
										onclick={() => ide.activeTabIndex = i}
										onkeydown={(e) => e.key === 'Enter' && (ide.activeTabIndex = i)}
										role="tab"
										tabindex="0"
										class="flex items-center gap-1.5 px-3 h-full text-xs border-r border-gx-border-default shrink-0 cursor-pointer
											{i === ide.activeTabIndex
												? 'bg-gx-bg-primary text-gx-text-primary border-t-2 border-t-gx-neon'
												: 'text-gx-text-muted hover:bg-gx-bg-hover'}"
									>
										{#if tab.modified}
											<span class="w-2 h-2 rounded-full bg-gx-accent-orange shrink-0"></span>
										{/if}
										<span class="truncate max-w-[120px]">{tab.name}</span>
										<button
											onclick={(e) => { e.stopPropagation(); ide.closeTab(i); }}
											class="ml-1 p-0.5 rounded hover:bg-gx-bg-elevated text-gx-text-muted hover:text-gx-text-primary"
										>
											<X size={12} />
										</button>
									</div>
								{/each}
							</div>

							<!-- Monaco Editor -->
							<CodeEditor onCursorChange={handleCursorChange} />
						</div>
					</Pane>

					<Handle />

					<!-- Bottom Panel (Terminal / Problems) -->
					<Pane defaultSize={35} minSize={10} maxSize={60}>
						<div class="flex flex-col h-full">
							<div class="flex items-center h-7 bg-gx-bg-secondary border-b border-gx-border-default px-1 shrink-0">
								<button
									onclick={() => bottomPanel = 'terminal'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'terminal' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Terminal size={13} />
									Terminal
								</button>
								<button
									onclick={() => bottomPanel = 'problems'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'problems' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<AlertTriangle size={13} />
									Problems
								</button>
							</div>

							<div class="flex-1 min-h-0">
								{#if bottomPanel === 'terminal'}
									<IdeTerminal />
								{:else}
									<div class="p-3 text-xs text-gx-text-muted">
										No problems detected.
									</div>
								{/if}
							</div>
						</div>
					</Pane>
				</PaneGroup>
			</Pane>

			<!-- AI Agent Panel (collapsible) -->
			{#if showAiPanel}
				<Handle />
				<Pane defaultSize={25} minSize={15} maxSize={40}>
					<AiAgent />
				</Pane>
			{/if}
		</PaneGroup>
	</div>

	<!-- Status Bar -->
	<IdeStatusBar
		lspStatus="disconnected"
		gitBranch=""
		aiModel="Ollama"
	/>
</div>
```

**Step 2: Verify the app builds**

Run:
```bash
cd /opt/ork-station/ImpForge
pnpm build 2>&1 | tail -10
```

Expected: Build succeeds without errors

**Step 3: Commit**

```bash
git add src/routes/ide/+page.svelte
git commit -m "feat(ide): rewrite IDE shell with PaneForge resizable layout

Assembles 5 components: FileExplorer, CodeEditor, IdeTerminal,
AiAgent, IdeStatusBar into a professional IDE layout with
resizable panels."
```

---

## Phase 2: AI Intelligence (Tasks 11–16)

After Phase 2: AI inline completion (ghost text), enhanced AI agent, codebase semantic search, and shadow workspace for code validation.

---

### Task 11: Install Phase 2 npm Dependencies

**Files:**
- Modify: `package.json`

**Step 1: Install AI and LSP packages**

```bash
cd /opt/ork-station/ImpForge
pnpm add monacopilot monaco-languageclient vscode-ws-jsonrpc
```

**Step 2: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "deps: add monacopilot and monaco-languageclient for AI + LSP"
```

---

### Task 12: Add Phase 2 Cargo Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add LSP, tree-sitter, and file watcher crates**

Add to `src-tauri/Cargo.toml` `[dependencies]`:

```toml
# LSP
lsp-types = "0.97"

# Code parsing for indexer
tree-sitter = "0.24"
tree-sitter-rust = "0.24"
tree-sitter-python = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-javascript = "0.23"

# File watcher
notify = "8.0"
```

**Step 2: Verify compilation**

```bash
cd /opt/ork-station/ImpForge/src-tauri
cargo check 2>&1 | tail -5
```

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: add lsp-types, tree-sitter grammars, and notify for Phase 2"
```

---

### Task 13: Codebase Indexer Backend (indexer.rs)

**Files:**
- Create: `src-tauri/src/ide/indexer.rs`
- Modify: `src-tauri/src/ide/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Create indexer.rs with Tree-sitter + FastEmbed + LanceDB**

Create `src-tauri/src/ide/indexer.rs`:

```rust
//! Codebase Indexer — Tree-sitter chunking + FastEmbed + LanceDB
//!
//! Parses source files into semantic chunks (functions, classes, structs),
//! embeds them with JinaEmbeddingsV2BaseCode, and stores in LanceDB
//! for offline semantic search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub node_type: String,
    pub name: String,
    pub language: String,
    pub content: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub indexed_files: u32,
    pub total_chunks: u32,
    pub last_indexed: Option<String>,
    pub indexing: bool,
    pub workspace: String,
}

pub struct CodebaseIndexer {
    status: Mutex<IndexStatus>,
}

impl CodebaseIndexer {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(IndexStatus {
                indexed_files: 0,
                total_chunks: 0,
                last_indexed: None,
                indexing: false,
                workspace: String::new(),
            }),
        }
    }
}

/// Extract semantic chunks from source code using tree-sitter
fn extract_chunks(content: &str, file_path: &str, language: &str) -> Vec<CodeChunk> {
    // Fallback: split by functions/blocks using simple heuristics
    // Full tree-sitter integration wired in subsequent task
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return chunks;
    }

    // Simple function-level splitting as initial implementation
    let mut current_chunk_start = 0;
    let mut current_name = file_path.split('/').last().unwrap_or("unknown").to_string();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Detect function/struct/class boundaries
        let is_boundary = match language {
            "rust" => {
                trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("struct ")
                    || trimmed.starts_with("impl ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("pub trait ")
            }
            "python" => {
                trimmed.starts_with("def ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("async def ")
            }
            "typescript" | "javascript" => {
                trimmed.starts_with("function ")
                    || trimmed.starts_with("export function ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("export class ")
                    || trimmed.contains("=> {")
            }
            _ => false,
        };

        if is_boundary && i > current_chunk_start + 2 {
            // Save previous chunk
            let chunk_content = lines[current_chunk_start..i].join("\n");
            if !chunk_content.trim().is_empty() {
                chunks.push(CodeChunk {
                    file_path: file_path.to_string(),
                    start_line: (current_chunk_start + 1) as u32,
                    end_line: i as u32,
                    node_type: "block".to_string(),
                    name: current_name.clone(),
                    language: language.to_string(),
                    content: chunk_content,
                    score: None,
                });
            }

            current_chunk_start = i;
            // Extract name from the boundary line
            current_name = trimmed
                .split('(')
                .next()
                .unwrap_or(trimmed)
                .split('{')
                .next()
                .unwrap_or(trimmed)
                .trim()
                .to_string();
        }
    }

    // Final chunk
    let chunk_content = lines[current_chunk_start..].join("\n");
    if !chunk_content.trim().is_empty() {
        chunks.push(CodeChunk {
            file_path: file_path.to_string(),
            start_line: (current_chunk_start + 1) as u32,
            end_line: lines.len() as u32,
            node_type: "block".to_string(),
            name: current_name,
            language: language.to_string(),
            content: chunk_content,
            score: None,
        });
    }

    chunks
}

fn detect_language(path: &str) -> &str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("svelte") => "typescript",
        Some("go") => "go",
        Some("cs") => "csharp",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        _ => "plaintext",
    }
}

#[tauri::command]
pub async fn index_codebase(
    app: tauri::AppHandle,
    workspace: String,
) -> Result<IndexStatus, String> {
    let indexer = app.state::<CodebaseIndexer>();
    let mut status = indexer.status.lock().await;

    status.indexing = true;
    status.workspace = workspace.clone();

    // Walk directory and collect indexable files
    let mut file_count = 0u32;
    let mut chunk_count = 0u32;

    let walker = walkdir::WalkDir::new(&workspace)
        .max_depth(10)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "__pycache__"
                && name != ".git"
                && name != "dist"
                && name != "build"
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let lang = detect_language(&path.to_string_lossy());
        if lang == "plaintext" {
            continue;
        }

        if let Ok(content) = tokio::fs::read_to_string(path).await {
            let chunks = extract_chunks(&content, &path.to_string_lossy(), lang);
            chunk_count += chunks.len() as u32;
            file_count += 1;

            // TODO: Embed with FastEmbed and store in LanceDB
            // For now, just count chunks
        }
    }

    status.indexed_files = file_count;
    status.total_chunks = chunk_count;
    status.last_indexed = Some(chrono::Utc::now().to_rfc3339());
    status.indexing = false;

    Ok(status.clone())
}

#[tauri::command]
pub async fn search_semantic(
    app: tauri::AppHandle,
    query: String,
    workspace: String,
    limit: Option<u32>,
) -> Result<Vec<CodeChunk>, String> {
    let _limit = limit.unwrap_or(5);

    // TODO: Use FastEmbed to embed query, search LanceDB
    // For now, fall back to text search
    let mut results = Vec::new();

    let walker = walkdir::WalkDir::new(&workspace)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "__pycache__"
        });

    let query_lower = query.to_lowercase();

    for entry in walker.flatten() {
        if !entry.file_type().is_file() || results.len() >= 10 {
            continue;
        }

        let path = entry.path();
        let lang = detect_language(&path.to_string_lossy());
        if lang == "plaintext" {
            continue;
        }

        if let Ok(content) = tokio::fs::read_to_string(path).await {
            let chunks = extract_chunks(&content, &path.to_string_lossy(), lang);
            for chunk in chunks {
                if chunk.content.to_lowercase().contains(&query_lower)
                    || chunk.name.to_lowercase().contains(&query_lower)
                {
                    results.push(chunk);
                }
            }
        }
    }

    results.truncate(_limit as usize);
    Ok(results)
}

#[tauri::command]
pub async fn index_status(app: tauri::AppHandle) -> Result<IndexStatus, String> {
    let indexer = app.state::<CodebaseIndexer>();
    let status = indexer.status.lock().await;
    Ok(status.clone())
}
```

**Step 2: Add walkdir dependency**

In `src-tauri/Cargo.toml`:

```toml
walkdir = "2"
```

**Step 3: Register module and commands**

In `src-tauri/src/ide/mod.rs`, add:
```rust
pub mod indexer;
```

In `src-tauri/src/lib.rs`, add to `invoke_handler`:
```rust
            ide::indexer::index_codebase,
            ide::indexer::search_semantic,
            ide::indexer::index_status,
```

Add to `.setup()`:
```rust
            app.manage(ide::indexer::CodebaseIndexer::new());
```

**Step 4: Verify compilation**

```bash
cd /opt/ork-station/ImpForge/src-tauri
cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add src-tauri/src/ide/indexer.rs src-tauri/src/ide/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat(ide): add codebase indexer with tree-sitter chunking

Initial implementation with heuristic code splitting.
FastEmbed + LanceDB embedding to be wired in follow-up task."
```

---

### Task 14: Git Backend (git.rs)

**Files:**
- Create: `src-tauri/src/ide/git.rs`
- Modify: `src-tauri/src/ide/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Create git.rs with git2 operations**

Create `src-tauri/src/ide/git.rs`:

```rust
//! Git operations for CodeForge IDE
//!
//! Uses git2 for native git operations (status, diff, commit, branch).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String, // "modified", "new", "deleted", "renamed", "untracked"
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusResult {
    pub branch: String,
    pub files: Vec<GitFileStatus>,
    pub ahead: u32,
    pub behind: u32,
    pub clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub time: String,
}

#[tauri::command]
pub async fn git_status(workspace: String) -> Result<GitStatusResult, String> {
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::discover(&workspace)
            .map_err(|e| format!("Not a git repository: {}", e))?;

        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "HEAD".to_string());

        let statuses = repo
            .statuses(Some(
                git2::StatusOptions::new()
                    .include_untracked(true)
                    .recurse_untracked_dirs(true),
            ))
            .map_err(|e| format!("Failed to get status: {}", e))?;

        let mut files = Vec::new();
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let s = entry.status();

            let (status_str, staged) = if s.contains(git2::Status::INDEX_NEW) {
                ("new", true)
            } else if s.contains(git2::Status::INDEX_MODIFIED) {
                ("modified", true)
            } else if s.contains(git2::Status::INDEX_DELETED) {
                ("deleted", true)
            } else if s.contains(git2::Status::WT_MODIFIED) {
                ("modified", false)
            } else if s.contains(git2::Status::WT_NEW) {
                ("untracked", false)
            } else if s.contains(git2::Status::WT_DELETED) {
                ("deleted", false)
            } else {
                continue;
            };

            files.push(GitFileStatus {
                path,
                status: status_str.to_string(),
                staged,
            });
        }

        Ok(GitStatusResult {
            branch,
            clean: files.is_empty(),
            files,
            ahead: 0,
            behind: 0,
        })
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub async fn git_diff(workspace: String, path: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::discover(&workspace)
            .map_err(|e| format!("Not a git repository: {}", e))?;

        let mut opts = git2::DiffOptions::new();
        if let Some(ref p) = path {
            opts.pathspec(p);
        }

        let diff = repo
            .diff_index_to_workdir(None, Some(&mut opts))
            .map_err(|e| format!("Failed to get diff: {}", e))?;

        let mut result = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let prefix = match line.origin() {
                '+' => "+",
                '-' => "-",
                ' ' => " ",
                _ => "",
            };
            result.push_str(prefix);
            result.push_str(&String::from_utf8_lossy(line.content()));
            true
        })
        .map_err(|e| format!("Failed to print diff: {}", e))?;

        Ok(result)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

#[tauri::command]
pub async fn git_log(workspace: String, limit: Option<u32>) -> Result<Vec<CommitInfo>, String> {
    let limit = limit.unwrap_or(20);

    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::discover(&workspace)
            .map_err(|e| format!("Not a git repository: {}", e))?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| format!("Failed to walk commits: {}", e))?;
        revwalk.push_head().map_err(|e| format!("No HEAD: {}", e))?;

        let mut commits = Vec::new();
        for oid in revwalk.take(limit as usize) {
            let oid = oid.map_err(|e| format!("Walk error: {}", e))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| format!("Commit not found: {}", e))?;

            commits.push(CommitInfo {
                id: oid.to_string()[..8].to_string(),
                message: commit.message().unwrap_or("").trim().to_string(),
                author: commit.author().name().unwrap_or("Unknown").to_string(),
                time: chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default(),
            });
        }

        Ok(commits)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
```

**Step 2: Add git2 dependency**

In `src-tauri/Cargo.toml`:
```toml
git2 = "0.20"
```

**Step 3: Register module and commands**

In `src-tauri/src/ide/mod.rs`:
```rust
pub mod git;
```

In `src-tauri/src/lib.rs` invoke_handler:
```rust
            ide::git::git_status,
            ide::git::git_diff,
            ide::git::git_log,
```

**Step 4: Verify and commit**

```bash
cd /opt/ork-station/ImpForge/src-tauri && cargo check 2>&1 | tail -5
git add src-tauri/src/ide/git.rs src-tauri/src/ide/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat(ide): add git backend with status, diff, and log via git2"
```

---

### Task 15: GitPanel Component

**Files:**
- Create: `src/routes/ide/GitPanel.svelte`

**Step 1: Create GitPanel with status, diff preview, and commit**

Create `src/routes/ide/GitPanel.svelte`:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import {
		GitBranch, GitCommit, RefreshCw, Plus, Minus,
		FileEdit, FilePlus, FileX, Loader2, Check
	} from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	interface GitFileStatus {
		path: string;
		status: string;
		staged: boolean;
	}

	interface GitStatusResult {
		branch: string;
		files: GitFileStatus[];
		clean: boolean;
	}

	interface CommitInfo {
		id: string;
		message: string;
		author: string;
		time: string;
	}

	let status = $state<GitStatusResult | null>(null);
	let commits = $state<CommitInfo[]>([]);
	let loading = $state(false);
	let commitMessage = $state('');
	let diffContent = $state('');
	let activeView = $state<'changes' | 'log'>('changes');

	onMount(() => {
		refresh();
	});

	async function refresh() {
		loading = true;
		try {
			status = await invoke<GitStatusResult>('git_status', { workspace: ide.currentDir });
			commits = await invoke<CommitInfo[]>('git_log', { workspace: ide.currentDir, limit: 20 });
		} catch (e) {
			console.error('Git error:', e);
		} finally {
			loading = false;
		}
	}

	async function showDiff(path: string) {
		try {
			diffContent = await invoke<string>('git_diff', { workspace: ide.currentDir, path });
		} catch (e) {
			diffContent = `Error: ${e}`;
		}
	}

	function statusIcon(s: string) {
		if (s === 'modified') return FileEdit;
		if (s === 'new' || s === 'untracked') return FilePlus;
		if (s === 'deleted') return FileX;
		return FileEdit;
	}

	function statusColor(s: string): string {
		if (s === 'modified') return 'text-gx-accent-orange';
		if (s === 'new' || s === 'untracked') return 'text-green-400';
		if (s === 'deleted') return 'text-red-400';
		return 'text-gx-text-muted';
	}
</script>

<div class="flex flex-col h-full bg-gx-bg-secondary overflow-hidden">
	<!-- Header -->
	<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default shrink-0">
		<GitBranch size={14} class="text-gx-neon" />
		<span class="text-xs font-semibold text-gx-text-primary">{status?.branch || 'Git'}</span>
		<div class="flex-1"></div>
		<button onclick={refresh} class="p-0.5 text-gx-text-muted hover:text-gx-neon" disabled={loading}>
			<RefreshCw size={12} class={loading ? 'animate-spin' : ''} />
		</button>
	</div>

	<!-- View tabs -->
	<div class="flex border-b border-gx-border-default shrink-0">
		<button onclick={() => activeView = 'changes'}
			class="flex-1 text-xs py-1 {activeView === 'changes' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted'}">
			Changes {status?.files.length ? `(${status.files.length})` : ''}
		</button>
		<button onclick={() => activeView = 'log'}
			class="flex-1 text-xs py-1 {activeView === 'log' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted'}">
			Log
		</button>
	</div>

	<div class="flex-1 overflow-auto">
		{#if activeView === 'changes'}
			{#if status?.clean}
				<div class="p-4 text-center text-xs text-gx-text-muted">
					<Check size={24} class="mx-auto mb-2 text-green-400 opacity-50" />
					Working tree clean
				</div>
			{:else if status}
				{#each status.files as file}
					<button
						onclick={() => showDiff(file.path)}
						class="flex items-center gap-2 w-full px-3 py-1.5 text-xs hover:bg-gx-bg-hover text-left"
					>
						<svelte:component this={statusIcon(file.status)} size={12} class="{statusColor(file.status)} shrink-0" />
						<span class="truncate text-gx-text-secondary">{file.path}</span>
						{#if file.staged}
							<span class="ml-auto text-[9px] text-green-400 bg-green-400/10 px-1 rounded">staged</span>
						{/if}
					</button>
				{/each}
			{/if}

			{#if diffContent}
				<div class="border-t border-gx-border-default p-2">
					<pre class="text-[11px] font-mono whitespace-pre-wrap max-h-48 overflow-auto">{#each diffContent.split('\n') as line}<span class="{line.startsWith('+') ? 'text-green-400' : line.startsWith('-') ? 'text-red-400' : 'text-gx-text-muted'}">{line}
</span>{/each}</pre>
				</div>
			{/if}
		{:else}
			{#each commits as commit}
				<div class="flex items-start gap-2 px-3 py-1.5 text-xs border-b border-gx-border-default/50 hover:bg-gx-bg-hover">
					<GitCommit size={12} class="text-gx-accent-cyan shrink-0 mt-0.5" />
					<div class="min-w-0">
						<div class="text-gx-text-secondary truncate">{commit.message}</div>
						<div class="text-[10px] text-gx-text-muted">{commit.id} · {commit.author} · {commit.time}</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
```

**Step 2: Commit**

```bash
git add src/routes/ide/GitPanel.svelte
git commit -m "feat(ide): add GitPanel component with status, diff, and log"
```

---

### Task 16: Wire GitPanel into Shell + Add Git Bottom Tab

**Files:**
- Modify: `src/routes/ide/+page.svelte`

**Step 1: Import and add GitPanel**

In `+page.svelte`, add to imports:
```typescript
import GitPanel from './GitPanel.svelte';
```

Add `| 'git'` to the `bottomPanel` type and add a Git tab button next to Problems. Add the GitPanel rendering in the bottom panel area:

```svelte
{:else if bottomPanel === 'git'}
    <GitPanel />
```

**Step 2: Verify build and commit**

```bash
cd /opt/ork-station/ImpForge && pnpm build 2>&1 | tail -5
git add src/routes/ide/+page.svelte
git commit -m "feat(ide): wire GitPanel into IDE shell as bottom tab"
```

---

## Phase 3: Git & Polish (Tasks 17–19) — High Level

### Task 17: ProblemsPanel Component
- Create `src/routes/ide/ProblemsPanel.svelte`
- Display LSP diagnostics (errors, warnings) with clickable navigation
- Wire into bottom panel tabs

### Task 18: Command Palette (Ctrl+P, Ctrl+Shift+P)
- Implement Ctrl+P for quick file open (uses Fuse.js from FileExplorer)
- Implement Ctrl+Shift+P for command palette
- Modal overlay component with fuzzy search

### Task 19: VS Code Theme Import
- Parse VS Code `.json` theme files
- Convert to Monaco `defineTheme()` format
- Settings page: theme picker dropdown

---

## Phase 4: Debug/DAP (Tasks 20–21) — High Level

### Task 20: DAP Backend
- Add `dap-types` crate
- Create `src-tauri/src/ide/debug.rs`
- DAP client that spawns debug adapter processes (codelldb, debugpy)
- Breakpoint state management

### Task 21: Debug UI
- Monaco decoration API for breakpoint gutters
- Call stack panel (Svelte)
- Variables panel with tree view
- Debug controls (continue, step over, step in, step out)

---

## Phase 5: Team Development (Tasks 22–23) — High Level

### Task 22: Automerge CRDT Integration
- Add `automerge` crate (Rust) and `@automerge/automerge` (npm)
- CRDT document sync for collaborative editing
- Relay server (axum + WebSocket)

### Task 23: Live Cursors + Presence
- WebSocket connection to relay server
- Cursor position broadcasting
- Colored cursor decorations in Monaco
- Team member list in sidebar

---

## Phase 6: Subscription & Billing (Tasks 24–25) — High Level

### Task 24: Stripe Integration
- Backend: Stripe API for subscription management
- Frontend: Pricing page, checkout flow
- License tier gating (check tier before enabling features)

### Task 25: Team Management
- Invite system (email, link)
- Role management (owner, admin, member, viewer)
- Usage dashboard (AI completions, API calls, storage)

---

## Verification Checklist

After each phase, verify:

- [ ] `cargo check --workspace` passes
- [ ] `pnpm build` succeeds
- [ ] `cargo test --workspace` passes (existing 292 tests)
- [ ] IDE route loads without console errors
- [ ] Terminal spawns real shell (Phase 1)
- [ ] File explorer shows files and supports fuzzy search (Phase 1)
- [ ] PaneForge panels resize correctly (Phase 1)
- [ ] AI completion shows ghost text (Phase 2)
- [ ] Git status shows file changes (Phase 2)
- [ ] Codebase indexer counts chunks (Phase 2)
