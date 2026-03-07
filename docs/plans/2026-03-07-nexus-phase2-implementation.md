# NEXUS Phase 2 — Full Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform NEXUS from a working skeleton (Dashboard, IDE, AI Models, News) into a production-ready AI Workstation with live metrics, streaming chat, real Docker/GitHub data, resizable panels, embedded WebView browser, and full OpenRouter integration.

**Architecture:** Tauri 2.10 + SvelteKit 5 (Svelte 5 Runes) + Rust backend. Opera GX neon-green theme. PaneForge for resizable panels. All AI routing through Rust keyword classifier → OpenRouter free models / Ollama local. Monitoring via sysinfo + libamdgpu_top. WebView via Tauri's webview API for embedded services (n8n, LangFlow, etc.).

**Tech Stack:** Rust (tauri, bollard, reqwest, sysinfo, libamdgpu_top, tauri-plugin-websocket), SvelteKit 5 (svelte 5 runes, paneforge, monaco-editor, @xterm/xterm), shadcn-svelte, TailwindCSS 4, Opera GX theme.

---

## Overview of Tasks

| # | Task | Priority | Est. Lines |
|---|------|----------|------------|
| 1 | Rust: System Monitoring Commands (sysinfo + AMD GPU) | CRITICAL | ~120 |
| 2 | Frontend: Live Status Bar with Real Metrics | CRITICAL | ~80 |
| 3 | Frontend: PaneForge Resizable Layout | HIGH | ~100 |
| 4 | Rust: Streaming Chat Backend (SSE via Tauri Events) | CRITICAL | ~150 |
| 5 | Frontend: Chat Page with Streaming + Message Types | CRITICAL | ~350 |
| 6 | Frontend: Docker Management Page (Real Data) | HIGH | ~250 |
| 7 | Frontend: GitHub Integration Page (Real Data) | HIGH | ~280 |
| 8 | Rust: Service Health Check Commands | HIGH | ~80 |
| 9 | Frontend: Embedded WebView Browser for n8n | HIGH | ~200 |
| 10 | Frontend: Settings Page with OpenRouter Config | MEDIUM | ~200 |
| 11 | GitHub Repository Setup (Task #104) | HIGH | ~50 |

**Total estimated:** ~1,860 new lines across 11 tasks.

---

### Task 1: Rust — System Monitoring Tauri Commands

The monitoring module (`src-tauri/src/monitoring/mod.rs`) already has complete structs and logic for CPU, RAM, GPU, Disk, Network. But the Tauri commands are NOT registered in `lib.rs`. We need to register them and add a lightweight polling command.

**Files:**
- Modify: `src-tauri/src/lib.rs` (register monitoring commands)
- Modify: `src-tauri/src/monitoring/mod.rs` (add `get_quick_stats` command)

**Step 1: Add a quick-stats command to monitoring/mod.rs**

Add at the end of the file (before the last `}`):

```rust
/// Quick stats for status bar (lightweight, <5ms)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickStats {
    pub cpu_percent: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub gpu_name: Option<String>,
    pub gpu_temp_c: Option<f32>,
    pub gpu_vram_used_mb: Option<u64>,
    pub gpu_vram_total_mb: Option<u64>,
    pub gpu_usage_percent: Option<f32>,
}

#[tauri::command]
pub async fn get_quick_stats() -> Result<QuickStats, String> {
    let mut sys = SYSTEM.lock().map_err(|e| e.to_string())?;
    sys.refresh_cpu_specifics(CpuRefreshKind::new().with_cpu_usage());
    sys.refresh_memory_kind(MemoryRefreshKind::new().with_ram());

    let cpu_percent = sys.global_cpu_usage();
    let ram_used_gb = sys.used_memory() as f32 / 1_073_741_824.0;
    let ram_total_gb = sys.total_memory() as f32 / 1_073_741_824.0;

    // AMD GPU via libamdgpu_top (if available)
    let (gpu_name, gpu_temp_c, gpu_vram_used_mb, gpu_vram_total_mb, gpu_usage_percent) =
        get_amd_gpu_quick().unwrap_or((None, None, None, None, None));

    Ok(QuickStats {
        cpu_percent,
        ram_used_gb,
        ram_total_gb,
        gpu_name,
        gpu_temp_c,
        gpu_vram_used_mb,
        gpu_vram_total_mb,
        gpu_usage_percent,
    })
}

fn get_amd_gpu_quick() -> Result<(Option<String>, Option<f32>, Option<u64>, Option<u64>, Option<f32>), String> {
    // Use libamdgpu_top for AMD GPU stats
    // This reads from /sys/class/drm/card*/device/ sysfs entries
    let name = std::fs::read_to_string("/sys/class/drm/card1/device/product_name")
        .or_else(|_| std::fs::read_to_string("/sys/class/drm/card0/device/product_name"))
        .ok()
        .map(|s| s.trim().to_string());

    let temp = std::fs::read_to_string("/sys/class/drm/card1/device/hwmon/hwmon4/temp1_input")
        .or_else(|_| {
            // Try to find the right hwmon
            for i in 0..10 {
                let path = format!("/sys/class/drm/card1/device/hwmon/hwmon{}/temp1_input", i);
                if let Ok(v) = std::fs::read_to_string(&path) {
                    return Ok(v);
                }
            }
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no hwmon"))
        })
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|t| t / 1000.0); // millidegrees to degrees

    let vram_used = std::fs::read_to_string("/sys/class/drm/card1/device/mem_info_vram_used")
        .or_else(|_| std::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_used"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|b| b / 1_048_576); // bytes to MB

    let vram_total = std::fs::read_to_string("/sys/class/drm/card1/device/mem_info_vram_total")
        .or_else(|_| std::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_total"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|b| b / 1_048_576);

    let usage = std::fs::read_to_string("/sys/class/drm/card1/device/gpu_busy_percent")
        .or_else(|_| std::fs::read_to_string("/sys/class/drm/card0/device/gpu_busy_percent"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());

    Ok((name, temp, vram_used, vram_total, usage))
}
```

**Step 2: Register monitoring commands in lib.rs**

Add to `invoke_handler` in `src-tauri/src/lib.rs`:

```rust
mod monitoring;

// In invoke_handler, add:
// Monitoring commands
monitoring::get_quick_stats,
```

**Step 3: Run cargo check**

Run: `cd /opt/ork-station/Nexus && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: PASS (0 errors)

**Step 4: Commit**

```bash
git add src-tauri/src/monitoring/mod.rs src-tauri/src/lib.rs
git commit -m "feat(monitoring): register get_quick_stats Tauri command for status bar"
```

---

### Task 2: Frontend — Live Status Bar with Real Metrics

Replace the placeholder `---` values in the layout's status bar footer with real data polled from `get_quick_stats` every 2 seconds.

**Files:**
- Create: `src/lib/stores/system.svelte.ts`
- Modify: `src/routes/+layout.svelte` (wire up real metrics)

**Step 1: Create system store**

```typescript
// src/lib/stores/system.svelte.ts
import { invoke } from '@tauri-apps/api/core';

export interface QuickStats {
	cpu_percent: number;
	ram_used_gb: number;
	ram_total_gb: number;
	gpu_name: string | null;
	gpu_temp_c: number | null;
	gpu_vram_used_mb: number | null;
	gpu_vram_total_mb: number | null;
	gpu_usage_percent: number | null;
}

export interface ServiceStatus {
	ollama: 'online' | 'offline' | 'checking';
	docker: 'online' | 'offline' | 'checking';
	n8n: 'online' | 'offline' | 'checking';
}

class SystemStore {
	stats = $state<QuickStats | null>(null);
	services = $state<ServiceStatus>({
		ollama: 'checking',
		docker: 'checking',
		n8n: 'checking',
	});
	private intervalId: ReturnType<typeof setInterval> | null = null;

	async poll() {
		try {
			this.stats = await invoke<QuickStats>('get_quick_stats');
		} catch (e) {
			console.error('Failed to get system stats:', e);
		}
	}

	async checkServices() {
		// Ollama
		try {
			const resp = await fetch('http://localhost:11434/api/tags');
			this.services = { ...this.services, ollama: resp.ok ? 'online' : 'offline' };
		} catch {
			this.services = { ...this.services, ollama: 'offline' };
		}

		// Docker (via Tauri command)
		try {
			await invoke('docker_info');
			this.services = { ...this.services, docker: 'online' };
		} catch {
			this.services = { ...this.services, docker: 'offline' };
		}

		// n8n
		try {
			const resp = await fetch('http://localhost:5678/healthz');
			this.services = { ...this.services, n8n: resp.ok ? 'online' : 'offline' };
		} catch {
			this.services = { ...this.services, n8n: 'offline' };
		}
	}

	startPolling(intervalMs = 2000) {
		this.poll();
		this.checkServices();
		this.intervalId = setInterval(() => this.poll(), intervalMs);
		// Check services every 30s
		setInterval(() => this.checkServices(), 30000);
	}

	stopPolling() {
		if (this.intervalId) clearInterval(this.intervalId);
	}
}

export const system = new SystemStore();
```

**Step 2: Wire up +layout.svelte status bar**

Replace the status bar footer section in `src/routes/+layout.svelte`:

```svelte
<script lang="ts">
	// Add to imports:
	import { system } from '$lib/stores/system.svelte';
	import { onMount } from 'svelte';

	// Add to script:
	onMount(() => {
		system.startPolling();
		return () => system.stopPolling();
	});
</script>

<!-- Replace footer content with: -->
<footer class="flex items-center h-6 px-3 bg-gx-bg-secondary border-t border-gx-border-default text-[11px] text-gx-text-muted shrink-0 gap-3">
	<span class="text-gx-neon font-semibold">NEXUS</span>
	<span>v0.1.0</span>
	<Separator orientation="vertical" class="h-3 bg-gx-border-default" />

	<!-- Service indicators -->
	<div class="flex items-center gap-1">
		<span class={system.services.ollama === 'online' ? 'text-gx-status-success' : 'text-gx-status-error'}>●</span>
		<span>Ollama</span>
	</div>
	<div class="flex items-center gap-1">
		<span class={system.services.docker === 'online' ? 'text-gx-status-success' : 'text-gx-status-error'}>●</span>
		<span>Docker</span>
	</div>
	<div class="flex items-center gap-1">
		<span class={system.services.n8n === 'online' ? 'text-gx-status-success' : 'text-gx-status-error'}>●</span>
		<span>n8n</span>
	</div>

	<div class="flex-1" />

	<!-- Real metrics -->
	{#if system.stats}
		<div class="flex items-center gap-1">
			<Cpu size={11} />
			<span>{system.stats.cpu_percent.toFixed(0)}%</span>
		</div>
		<div class="flex items-center gap-1">
			<HardDrive size={11} />
			<span>{system.stats.ram_used_gb.toFixed(1)} / {system.stats.ram_total_gb.toFixed(0)} GB</span>
		</div>
		{#if system.stats.gpu_vram_used_mb != null}
			<div class="flex items-center gap-1">
				<span class="text-gx-accent-magenta">GPU</span>
				<span>{(system.stats.gpu_vram_used_mb / 1024).toFixed(1)} / {((system.stats.gpu_vram_total_mb ?? 0) / 1024).toFixed(0)} GB</span>
				{#if system.stats.gpu_temp_c != null}
					<span class="text-gx-text-muted">{system.stats.gpu_temp_c.toFixed(0)}°C</span>
				{/if}
			</div>
		{/if}
	{:else}
		<div class="flex items-center gap-1">
			<Cpu size={11} />
			<span>Loading...</span>
		</div>
	{/if}

	<Badge variant="outline" class="text-[10px] px-1 py-0 h-4 border-gx-border-default text-gx-text-muted">
		Free Tier
	</Badge>
</footer>
```

**Step 3: Remove old status variables from layout**

Delete the three `$state` variables: `ollamaStatus`, `dockerStatus`, `n8nStatus`.

**Step 4: Run pnpm check**

Run: `cd /opt/ork-station/Nexus && pnpm check 2>&1 | tail -5`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/stores/system.svelte.ts src/routes/+layout.svelte
git commit -m "feat(status-bar): live CPU/RAM/GPU metrics from sysinfo + AMD sysfs"
```

---

### Task 3: Frontend — PaneForge Resizable Layout

Replace the fixed activity bar + content layout with PaneForge resizable panels. The activity bar stays fixed (48px), but the main content area gets a collapsible sidebar.

**Files:**
- Modify: `src/routes/+layout.svelte` (add PaneForge panels)

**Step 1: Import PaneForge and add resizable sidebar**

The shadcn-svelte resizable component wraps PaneForge. Add a secondary sidebar panel for context-specific content (conversation list on /chat, repo list on /github, container list on /docker):

```svelte
<script lang="ts">
	import * as Resizable from '$lib/components/ui/resizable/index.js';
	// ... existing imports
</script>

<!-- Main content area (replaces the existing flex-col div) -->
<div class="flex flex-col flex-1 min-w-0">
	<!-- Top bar (unchanged) -->
	<header>...</header>

	<!-- Resizable content area -->
	<Resizable.PaneGroup direction="horizontal" class="flex-1">
		<!-- Optional sidebar (only shown on routes that need it) -->
		{#if showSidebar}
			<Resizable.Pane defaultSize={20} minSize={15} maxSize={35}>
				<div class="h-full bg-gx-bg-secondary border-r border-gx-border-default overflow-y-auto">
					{@render children()}  <!-- Sidebar slot -->
				</div>
			</Resizable.Pane>
			<Resizable.Handle class="w-1 bg-gx-border-default hover:bg-gx-neon transition-colors" />
		{/if}

		<!-- Main content -->
		<Resizable.Pane defaultSize={showSidebar ? 80 : 100}>
			<main class="h-full overflow-auto">
				{@render children()}
			</main>
		</Resizable.Pane>
	</Resizable.PaneGroup>

	<!-- Status bar (unchanged) -->
	<footer>...</footer>
</div>
```

The `showSidebar` derived state checks the current route:

```typescript
let showSidebar = $derived(
	['/chat', '/github', '/docker'].includes(activeRoute) ||
	activeRoute.startsWith('/chat') ||
	activeRoute.startsWith('/github') ||
	activeRoute.startsWith('/docker')
);
```

**Note:** For Phase 2, keep it simple — the sidebar is just a wider activity bar area. The route pages themselves render their own sidebar content via the page component. The PaneForge `Pane` wraps the `<main>` area only. Don't over-engineer with nested layouts yet.

**Step 2: Run pnpm check**

Run: `cd /opt/ork-station/Nexus && pnpm check 2>&1 | tail -5`
Expected: PASS

**Step 3: Commit**

```bash
git add src/routes/+layout.svelte
git commit -m "feat(layout): PaneForge resizable panels in main content area"
```

---

### Task 4: Rust — Streaming Chat via Tauri Events

The current `route_message` waits for the full response. Add streaming support by emitting Tauri events as tokens arrive.

**Files:**
- Modify: `src-tauri/src/router/targets.rs` (add streaming functions)
- Modify: `src-tauri/src/lib.rs` (add streaming command)

**Step 1: Add streaming execution to targets.rs**

```rust
use tauri::{AppHandle, Emitter};

/// Stream response tokens via Tauri events
pub async fn execute_streaming(
    &self,
    system_prompt: &str,
    user_prompt: &str,
    app: &AppHandle,
    conversation_id: &str,
) -> Result<String, RouterError> {
    match self {
        Self::OpenRouter { model } => {
            stream_openrouter(model, system_prompt, user_prompt, app, conversation_id).await
        }
        Self::Ollama { model } => {
            stream_ollama(model, system_prompt, user_prompt, app, conversation_id).await
        }
        _ => self.execute(system_prompt, user_prompt).await,
    }
}

async fn stream_openrouter(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app: &AppHandle,
    conversation_id: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| RouterError::MissingApiKey { provider: "OpenRouter".to_string() })?;

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://nexus.dev")
        .header("X-Title", "NEXUS AI Workstation Builder")
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?;

    let mut full_content = String::new();
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| RouterError::RequestFailed(e))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" { break; }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                        full_content.push_str(delta);
                        let _ = app.emit("chat-stream", serde_json::json!({
                            "conversation_id": conversation_id,
                            "delta": delta,
                            "done": false,
                        }));
                    }
                }
            }
        }
    }

    let _ = app.emit("chat-stream", serde_json::json!({
        "conversation_id": conversation_id,
        "delta": "",
        "done": true,
        "full_content": &full_content,
    }));

    Ok(full_content)
}

async fn stream_ollama(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app: &AppHandle,
    conversation_id: &str,
) -> Result<String, RouterError> {
    let client = reqwest::Client::new();

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ]
        }))
        .send()
        .await?;

    let mut full_content = String::new();
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| RouterError::RequestFailed(e))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(content) = json["message"]["content"].as_str() {
                    full_content.push_str(content);
                    let _ = app.emit("chat-stream", serde_json::json!({
                        "conversation_id": conversation_id,
                        "delta": content,
                        "done": json["done"].as_bool().unwrap_or(false),
                    }));
                }
            }
        }
    }

    let _ = app.emit("chat-stream", serde_json::json!({
        "conversation_id": conversation_id,
        "delta": "",
        "done": true,
        "full_content": &full_content,
    }));

    Ok(full_content)
}
```

**Step 2: Add streaming command to lib.rs**

```rust
/// Route a message with streaming response
#[tauri::command]
async fn route_message_stream(
    app: tauri::AppHandle,
    message: RoutedMessage,
) -> Result<String, String> {
    let config = router::RouterConfig::new()
        .with_openrouter_key(std::env::var("OPENROUTER_API_KEY").unwrap_or_default());

    let task_type = router::classify_fast(&message.content);
    let target = router::targets::select_target(task_type, &config);
    let conv_id = message.conversation_id.unwrap_or_else(|| "default".to_string());

    target
        .execute_streaming("You are a helpful assistant.", &message.content, &app, &conv_id)
        .await
        .map_err(|e| e.to_string())
}
```

Register in `invoke_handler`: `route_message_stream,`

**Step 3: Run cargo check**

Run: `cd /opt/ork-station/Nexus && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/router/targets.rs src-tauri/src/lib.rs
git commit -m "feat(chat): streaming response via Tauri events for OpenRouter + Ollama"
```

---

### Task 5: Frontend — Chat Page with Streaming + Rich Messages

Complete rewrite of the chat placeholder into a full-featured chat with streaming, conversation sidebar, model selector, and rich message rendering.

**Files:**
- Modify: `src/lib/stores/chat.svelte.ts` (class-based refactor + streaming)
- Modify: `src/routes/chat/+page.svelte` (complete rewrite)

**Step 1: Refactor chat store to class-based pattern (like IdeStore)**

Rewrite `chat.svelte.ts` to a class with:
- `conversations`, `activeConversationId`, `sending`, `streamingContent`
- `sendMessage()` → calls `route_message_stream` and listens to `chat-stream` events
- `createConversation()`, `deleteConversation()`
- Svelte 5 `$state` fields on class

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ... (interfaces stay the same, add streamingContent)

class ChatStore {
    conversations = $state<Conversation[]>([]);
    activeConversationId = $state<string | null>(null);
    sending = $state(false);
    streamingContent = $state('');

    get activeConversation(): Conversation | undefined {
        return this.conversations.find(c => c.id === this.activeConversationId);
    }

    createConversation(title = 'New Chat'): string { /* ... */ }
    setActive(id: string) { this.activeConversationId = id; }
    deleteConversation(id: string) { /* ... */ }

    async sendMessage(content: string, modelId?: string) {
        if (!this.activeConversationId || this.sending) return;
        this.sending = true;
        this.streamingContent = '';

        // Add user message
        const conv = this.activeConversation;
        if (!conv) return;
        conv.messages = [...conv.messages, {
            id: generateId(), role: 'user', content, timestamp: new Date()
        }];

        // Listen for streaming tokens
        const unlisten = await listen<{delta: string; done: boolean; full_content?: string}>('chat-stream', (event) => {
            if (event.payload.done) {
                conv.messages = [...conv.messages, {
                    id: generateId(), role: 'assistant',
                    content: event.payload.full_content || this.streamingContent,
                    timestamp: new Date(), model: modelId
                }];
                this.streamingContent = '';
                this.sending = false;
                unlisten();
            } else {
                this.streamingContent += event.payload.delta;
            }
        });

        try {
            await invoke('route_message_stream', {
                message: {
                    content,
                    model_id: modelId || null,
                    conversation_id: this.activeConversationId
                }
            });
        } catch (e) {
            conv.messages = [...conv.messages, {
                id: generateId(), role: 'assistant',
                content: `Error: ${e}`, timestamp: new Date()
            }];
            this.sending = false;
            unlisten();
        }
    }
}

export const chat = new ChatStore();
```

**Step 2: Rewrite chat/+page.svelte**

Full chat UI with:
- Left sidebar: conversation list with create/delete
- Main area: message feed with markdown rendering
- Bottom: input with model selector badge + send button
- Streaming: show `chat.streamingContent` as typing indicator
- Message types: code blocks with syntax highlighting, thinking indicators

The page is ~300 lines. Key structure:

```svelte
<div class="flex h-full">
    <!-- Conversation sidebar -->
    <div class="w-64 bg-gx-bg-secondary border-r border-gx-border-default flex flex-col">
        <button onclick={createNew}>+ New Chat</button>
        {#each chat.conversations as conv}
            <button onclick={() => chat.setActive(conv.id)}>
                {conv.title}
            </button>
        {/each}
    </div>

    <!-- Main chat area -->
    <div class="flex-1 flex flex-col">
        <!-- Messages -->
        <div class="flex-1 overflow-y-auto p-4 space-y-4">
            {#each messages as msg}
                <ChatMessage {msg} />
            {/each}
            {#if chat.streamingContent}
                <div class="text-gx-text-secondary animate-pulse">
                    {chat.streamingContent}
                </div>
            {/if}
        </div>

        <!-- Input -->
        <form onsubmit={handleSend} class="p-4 border-t border-gx-border-default">
            <div class="flex gap-2">
                <input bind:value={inputValue} placeholder="Message..." class="flex-1 px-4 py-2" />
                <button type="submit" disabled={chat.sending}>Send</button>
            </div>
        </form>
    </div>
</div>
```

**Step 3: Run pnpm check**

Run: `cd /opt/ork-station/Nexus && pnpm check 2>&1 | tail -5`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib/stores/chat.svelte.ts src/routes/chat/+page.svelte
git commit -m "feat(chat): streaming chat with conversation sidebar and rich messages"
```

---

### Task 6: Frontend — Docker Management Page

Replace the Docker placeholder with a full container management UI using the existing `list_containers`, `container_action`, and `docker_info` Tauri commands.

**Files:**
- Modify: `src/routes/docker/+page.svelte` (complete rewrite)

**Step 1: Implement Docker page**

Features:
- Auto-load containers on mount
- Table view: Name, Image, Status, Ports, Actions
- Action buttons: Start, Stop, Restart, Remove, View Logs
- Docker system info card (version, total containers, images)
- Log viewer modal/panel
- Refresh button

Key structure:

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    // ... shadcn imports

    interface ContainerInfo {
        id: string; name: string; image: string;
        status: string; state: string; ports: string[];
    }

    let containers = $state<ContainerInfo[]>([]);
    let dockerInfo = $state<Record<string, string>>({});
    let loading = $state(true);
    let logs = $state('');
    let showLogs = $state(false);

    async function loadContainers() {
        loading = true;
        try {
            containers = await invoke('list_containers');
            dockerInfo = await invoke('docker_info');
        } catch (e) { console.error(e); }
        loading = false;
    }

    async function performAction(id: string, action: string) {
        await invoke('container_action', { containerId: id, action });
        await loadContainers();
    }

    async function viewLogs(id: string) {
        logs = await invoke('container_action', { containerId: id, action: 'Logs' });
        showLogs = true;
    }

    onMount(loadContainers);
</script>
```

**Step 2: Run pnpm check, commit**

---

### Task 7: Frontend — GitHub Integration Page

Replace the GitHub placeholder with real repository browsing, issues, and PRs.

**Files:**
- Modify: `src/routes/github/+page.svelte` (complete rewrite)

**Step 1: Implement GitHub page**

Features:
- Auto-load repos on mount
- Repo grid: name, description, language badge, stars, last updated
- Click repo → show issues + PRs tabs
- User info card (avatar, login)
- Token configuration hint if not set

Key structure: repo grid → click repo → issues/PRs tabs → detail view.

**Step 2: Run pnpm check, commit**

---

### Task 8: Rust — Service Health Check Commands

Add lightweight health check commands for status indicators.

**Files:**
- Modify: `src-tauri/src/lib.rs` (add check_service_health command)

**Step 1: Add service health check command**

```rust
#[tauri::command]
async fn check_service_health(service: String) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let url = match service.as_str() {
        "ollama" => "http://localhost:11434/api/tags",
        "n8n" => "http://localhost:5678/healthz",
        "langflow" => "http://localhost:7860/health",
        "openwebui" => "http://localhost:3000/health",
        "grafana" => "http://localhost:4000/api/health",
        "searxng" => "http://localhost:8888/healthz",
        _ => return Err(format!("Unknown service: {}", service)),
    };

    match client.get(url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}
```

Register: `check_service_health,`

**Step 2: cargo check, commit**

---

### Task 9: Frontend — Embedded WebView Browser for n8n + Services

Replace the n8n placeholder with an embedded browser using Tauri's multiwebview API OR an iframe-based approach.

**Files:**
- Modify: `src/routes/n8n/+page.svelte` (complete rewrite with webview)

**Step 1: Research approach**

Tauri 2 supports multiwebview via `tauri::WebviewBuilder` on the Rust side. However, for simplicity and cross-platform compatibility, we use an iframe pointing to the local service URL. The iframe approach works because n8n runs on localhost and CSP allows it.

For Tauri native webview (better isolation), use `@tauri-apps/api/webview`:

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
```

**Step 2: Implement n8n page with service detection**

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';

    let serviceOnline = $state(false);
    let loading = $state(true);
    let serviceUrl = 'http://localhost:5678';

    // Tabs for different embedded services
    const services = [
        { id: 'n8n', name: 'n8n', url: 'http://localhost:5678', port: 5678 },
        { id: 'langflow', name: 'LangFlow', url: 'http://localhost:7860', port: 7860 },
        { id: 'openwebui', name: 'Open WebUI', url: 'http://localhost:3000', port: 3000 },
    ];
    let activeService = $state(services[0]);

    async function checkService() {
        loading = true;
        try {
            serviceOnline = await invoke('check_service_health', { service: activeService.id });
        } catch { serviceOnline = false; }
        loading = false;
    }

    onMount(checkService);
</script>

<div class="flex flex-col h-full">
    <!-- Service tabs -->
    <div class="flex items-center gap-2 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default">
        {#each services as svc}
            <button
                onclick={() => { activeService = svc; checkService(); }}
                class="px-3 py-1 text-xs rounded-gx {activeService.id === svc.id ? 'bg-gx-neon text-gx-bg-primary' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
            >
                {svc.name}
            </button>
        {/each}
    </div>

    <!-- Embedded content -->
    {#if loading}
        <div class="flex-1 flex items-center justify-center">
            <span class="text-gx-text-muted">Checking {activeService.name}...</span>
        </div>
    {:else if serviceOnline}
        <iframe
            src={activeService.url}
            class="flex-1 w-full border-none"
            title="{activeService.name} Dashboard"
            sandbox="allow-same-origin allow-scripts allow-forms allow-popups"
        />
    {:else}
        <div class="flex-1 flex flex-col items-center justify-center gap-4">
            <span class="text-gx-status-error text-4xl">●</span>
            <h2 class="text-lg font-semibold">{activeService.name} is not running</h2>
            <p class="text-gx-text-muted text-sm">
                Start it with: <code class="bg-gx-bg-tertiary px-2 py-1 rounded">docker start {activeService.id}</code>
            </p>
        </div>
    {/if}
</div>
```

**Step 3: Update Tauri CSP to allow iframe sources**

In `src-tauri/tauri.conf.json`, add to security.csp:
```json
"csp": "default-src 'self'; frame-src http://localhost:*; connect-src 'self' http://localhost:* https://openrouter.ai https://api.github.com; img-src 'self' data: https:; style-src 'self' 'unsafe-inline'"
```

**Step 4: pnpm check, commit**

---

### Task 10: Frontend — Settings Page with OpenRouter Config

Replace the settings placeholder with API key management, theme options, and service configuration.

**Files:**
- Modify: `src/routes/settings/+page.svelte` (complete rewrite)

**Step 1: Implement Settings page**

Sections:
1. **API Keys**: OpenRouter, GitHub Token — saved to Tauri Store
2. **AI Router**: Default model, prefer free models toggle
3. **Services**: n8n URL, LangFlow URL, etc.
4. **Appearance**: Theme accent color (future)
5. **About**: Version, links

Use `@tauri-apps/plugin-store` for persistence:

```typescript
import { Store } from '@tauri-apps/plugin-store';

const store = await Store.load('settings.json');
await store.set('openrouter_key', apiKey);
await store.save();
```

**Step 2: pnpm check, commit**

---

### Task 11: GitHub Repository Setup (Task #104)

Create the NEXUS GitHub repository with proper README, license, and CI/CD.

**Files:**
- Create: `/opt/ork-station/Nexus/README.md`
- Create: `/opt/ork-station/Nexus/.gitignore` (if not exists)

**Step 1: Initialize git remote and push**

```bash
cd /opt/ork-station/Nexus
gh repo create ork-station/nexus --private --description "NEXUS — AI Workstation Builder" --source=. --remote=origin
git push -u origin main
```

**Step 2: Add collaborator**

```bash
gh api repos/ork-station/nexus/collaborators/Christof-Treitges -X PUT -f permission=push
```

Note: GitHub identifies users by username, not email. We need the GitHub username for Christof.Treitges@outlook.de. The user should be invited via GitHub's UI or we find the username first.

**Step 3: Create public README**

Professional README with: logo, features, screenshots placeholder, installation, tech stack, pricing tiers, license.

**Step 4: Commit and push**

---

## Execution Dependencies

```
Task 1 (Rust monitoring) ──→ Task 2 (Status bar)
Task 4 (Rust streaming) ──→ Task 5 (Chat page)
Task 8 (Health checks) ──→ Task 9 (WebView browser)

Independent:
- Task 3 (PaneForge) - can run anytime
- Task 6 (Docker page) - backend already exists
- Task 7 (GitHub page) - backend already exists
- Task 10 (Settings) - can run anytime
- Task 11 (GitHub repo) - can run anytime
```

## Parallel Execution Groups

**Group A (can run in parallel):** Tasks 1, 4, 8 (all Rust backend)
**Group B (after Group A):** Tasks 2, 5, 9 (frontend consuming new commands)
**Group C (independent):** Tasks 3, 6, 7, 10, 11 (no new backend deps)

## Testing Strategy

After each task:
1. `cargo check --manifest-path src-tauri/Cargo.toml` (Rust)
2. `pnpm check` (Svelte/TypeScript)
3. `cargo test --manifest-path src-tauri/Cargo.toml` (Rust unit tests)
4. Manual smoke test in `pnpm tauri dev` for UI tasks
