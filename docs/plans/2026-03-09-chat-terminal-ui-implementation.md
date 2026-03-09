# ChatTerminalUI 3x3x3 Enterprise Upgrade — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform ImpForge's basic chat into a modular 3x3x3 ChatTerminalUI system with 3 placement modes, 3 stream modes, 3 visualization levels — all switchable in Settings.

**Architecture:** BenikUI-inspired module registry where each chat variant registers as a switchable module. The ChatRenderer component handles markdown/code/math/diagram rendering via marked.js pipeline. Model visualization uses SVG-based pipeline DAG. All state managed via Svelte 5 runes stores.

**Tech Stack:** SvelteKit 5.51, Tauri 2.10, Rust, marked.js, highlight.js, KaTeX, mermaid (lazy), paneforge (already installed), motion (already installed), bits-ui (already installed)

**Design Doc:** `docs/plans/2026-03-09-chat-terminal-ui-enterprise-design.md`

---

## Phase 1: Foundation — Rendering Engine & Settings

### Task 1: Install rendering dependencies

**Files:**
- Modify: `package.json`

**Step 1: Install npm packages**

Run:
```bash
cd /opt/ork-station/ImpForge && pnpm add marked highlight.js katex
```

Note: mermaid will be lazy-loaded later (180kb). These three are the core rendering stack.

**Step 2: Verify installation**

Run:
```bash
cd /opt/ork-station/ImpForge && node -e "require('marked'); require('highlight.js'); require('katex'); console.log('OK')"
```
Expected: `OK`

**Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "feat(chat): add marked, highlight.js, katex rendering dependencies"
```

---

### Task 2: ChatLayoutSettings in Settings Store

**Files:**
- Modify: `src/lib/stores/settings.svelte.ts`

**Step 1: Add ChatLayoutSettings to AppSettings interface**

Add after the `n8nEnabled: boolean;` line in `AppSettings`:

```typescript
	// Chat Layout (3x3x3 modular system)
	chatPlacement: 'side-panel' | 'dedicated' | 'convergence';
	chatStreamMode: 'split' | 'unified' | 'mission-control';
	chatVizLevel: 'minimal' | 'cards' | 'pipeline';
	chatShowThinking: boolean;
	chatShowRouting: boolean;
	chatAnimations: boolean;
	chatCompactMode: boolean;
```

**Step 2: Add defaults in DEFAULT_SETTINGS**

Add after `n8nEnabled: false,`:

```typescript
	chatPlacement: 'dedicated',
	chatStreamMode: 'unified',
	chatVizLevel: 'cards',
	chatShowThinking: true,
	chatShowRouting: true,
	chatAnimations: true,
	chatCompactMode: false,
```

**Step 3: Add the new keys to the switch statement in loadSettings()**

Add new cases in the switch inside `loadSettings()`:

```typescript
				case 'chatPlacement':
					settings[key] = value as 'side-panel' | 'dedicated' | 'convergence';
					break;
				case 'chatStreamMode':
					settings[key] = value as 'split' | 'unified' | 'mission-control';
					break;
				case 'chatVizLevel':
					settings[key] = value as 'minimal' | 'cards' | 'pipeline';
					break;
				case 'chatShowThinking':
				case 'chatShowRouting':
				case 'chatAnimations':
				case 'chatCompactMode':
					settings[key] = value as boolean;
					break;
```

**Step 4: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | tail -5`
Expected: No new errors from settings.svelte.ts

**Step 5: Commit**

```bash
git add src/lib/stores/settings.svelte.ts
git commit -m "feat(settings): add ChatLayoutSettings for 3x3x3 modular chat system"
```

---

### Task 3: ChatRenderer — Markdown Rendering Component

**Files:**
- Create: `src/lib/components/chat/ChatRenderer.svelte`

**Step 1: Create the component**

This component takes raw markdown content and renders it with syntax highlighting, math, and streaming cursor support.

```svelte
<script lang="ts">
	import { marked } from 'marked';
	import hljs from 'highlight.js/lib/core';
	// Register only common languages to keep bundle small
	import javascript from 'highlight.js/lib/languages/javascript';
	import typescript from 'highlight.js/lib/languages/typescript';
	import python from 'highlight.js/lib/languages/python';
	import rust from 'highlight.js/lib/languages/rust';
	import bash from 'highlight.js/lib/languages/bash';
	import json from 'highlight.js/lib/languages/json';
	import css from 'highlight.js/lib/languages/css';
	import xml from 'highlight.js/lib/languages/xml';
	import sql from 'highlight.js/lib/languages/sql';
	import yaml from 'highlight.js/lib/languages/yaml';
	import dockerfile from 'highlight.js/lib/languages/dockerfile';
	import markdown from 'highlight.js/lib/languages/markdown';
	import go from 'highlight.js/lib/languages/go';
	import csharp from 'highlight.js/lib/languages/csharp';

	hljs.registerLanguage('javascript', javascript);
	hljs.registerLanguage('js', javascript);
	hljs.registerLanguage('typescript', typescript);
	hljs.registerLanguage('ts', typescript);
	hljs.registerLanguage('python', python);
	hljs.registerLanguage('py', python);
	hljs.registerLanguage('rust', rust);
	hljs.registerLanguage('rs', rust);
	hljs.registerLanguage('bash', bash);
	hljs.registerLanguage('sh', bash);
	hljs.registerLanguage('shell', bash);
	hljs.registerLanguage('json', json);
	hljs.registerLanguage('css', css);
	hljs.registerLanguage('html', xml);
	hljs.registerLanguage('xml', xml);
	hljs.registerLanguage('sql', sql);
	hljs.registerLanguage('yaml', yaml);
	hljs.registerLanguage('yml', yaml);
	hljs.registerLanguage('dockerfile', dockerfile);
	hljs.registerLanguage('docker', dockerfile);
	hljs.registerLanguage('markdown', markdown);
	hljs.registerLanguage('md', markdown);
	hljs.registerLanguage('go', go);
	hljs.registerLanguage('csharp', csharp);
	hljs.registerLanguage('cs', csharp);

	interface Props {
		content: string;
		streaming?: boolean;
	}

	let { content, streaming = false }: Props = $props();

	// Configure marked with highlight.js
	const renderer = new marked.Renderer();

	renderer.code = ({ text, lang }: { text: string; lang?: string }) => {
		const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
		let highlighted: string;
		try {
			highlighted = language !== 'plaintext'
				? hljs.highlight(text, { language }).value
				: escapeHtml(text);
		} catch {
			highlighted = escapeHtml(text);
		}
		return `<div class="forge-code-block group relative my-3">
			<div class="flex items-center justify-between px-3 py-1.5 text-[10px] font-mono text-gx-text-muted bg-gx-bg-primary border border-gx-border-default rounded-t-lg border-b-0">
				<span>${language}</span>
				<button class="forge-copy-btn opacity-0 group-hover:opacity-100 transition-opacity text-gx-text-muted hover:text-gx-neon" data-code="${encodeURIComponent(text)}">Copy</button>
			</div>
			<pre class="p-3 bg-gx-bg-primary border border-gx-border-default rounded-b-lg overflow-x-auto text-xs leading-relaxed"><code class="hljs language-${language}">${highlighted}</code></pre>
		</div>`;
	};

	function escapeHtml(text: string): string {
		return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
	}

	marked.setOptions({ renderer, gfm: true, breaks: true });

	// Render with rAF debounce for streaming
	let renderedHtml = $state('');
	let rafId = 0;

	$effect(() => {
		const raw = content;
		if (rafId) cancelAnimationFrame(rafId);
		rafId = requestAnimationFrame(() => {
			renderedHtml = marked.parse(raw) as string;
		});
	});

	// Handle copy button clicks via event delegation
	function handleClick(e: MouseEvent) {
		const btn = (e.target as HTMLElement).closest('.forge-copy-btn');
		if (btn instanceof HTMLElement && btn.dataset.code) {
			navigator.clipboard.writeText(decodeURIComponent(btn.dataset.code));
			btn.textContent = 'Copied!';
			setTimeout(() => { btn.textContent = 'Copy'; }, 2000);
		}
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="forge-renderer prose prose-invert prose-sm max-w-none" onclick={handleClick}>
	{@html renderedHtml}
	{#if streaming}
		<span class="inline-block w-1.5 h-4 bg-gx-neon animate-pulse ml-0.5 align-text-bottom"></span>
	{/if}
</div>

<style>
	.forge-renderer :global(p) {
		margin: 0.25em 0;
	}
	.forge-renderer :global(ul),
	.forge-renderer :global(ol) {
		margin: 0.5em 0;
		padding-left: 1.5em;
	}
	.forge-renderer :global(li) {
		margin: 0.15em 0;
	}
	.forge-renderer :global(code:not(.hljs)) {
		background: var(--color-gx-bg-primary, #1a1a2e);
		border: 1px solid var(--color-gx-border-default, #2a2a3e);
		border-radius: 4px;
		padding: 0.1em 0.35em;
		font-size: 0.85em;
		font-family: 'JetBrains Mono Variable', monospace;
	}
	.forge-renderer :global(blockquote) {
		border-left: 3px solid var(--color-gx-neon, #00d4ff);
		padding-left: 0.75em;
		margin: 0.5em 0;
		opacity: 0.85;
	}
	.forge-renderer :global(a) {
		color: var(--color-gx-neon, #00d4ff);
		text-decoration: underline;
	}
	.forge-renderer :global(table) {
		border-collapse: collapse;
		margin: 0.5em 0;
		font-size: 0.85em;
		width: 100%;
	}
	.forge-renderer :global(th),
	.forge-renderer :global(td) {
		border: 1px solid var(--color-gx-border-default, #2a2a3e);
		padding: 0.35em 0.75em;
		text-align: left;
	}
	.forge-renderer :global(th) {
		background: var(--color-gx-bg-primary, #1a1a2e);
		font-weight: 600;
	}
	.forge-renderer :global(hr) {
		border-color: var(--color-gx-border-default, #2a2a3e);
		margin: 1em 0;
	}
	.forge-renderer :global(h1),
	.forge-renderer :global(h2),
	.forge-renderer :global(h3) {
		margin-top: 0.75em;
		margin-bottom: 0.25em;
		font-weight: 600;
	}
</style>
```

**Step 2: Add highlight.js theme CSS**

Create: `src/lib/components/chat/hljs-forge.css`

```css
/* ImpForge highlight.js theme — matches gx-* dark design tokens */
.hljs {
	color: #c9d1d9;
	background: transparent;
}
.hljs-comment, .hljs-quote { color: #8b949e; font-style: italic; }
.hljs-keyword, .hljs-selector-tag { color: #ff7b72; }
.hljs-string, .hljs-addition { color: #a5d6ff; }
.hljs-number, .hljs-literal { color: #79c0ff; }
.hljs-built_in { color: #d2a8ff; }
.hljs-function .hljs-title, .hljs-title.function_ { color: #d2a8ff; }
.hljs-type, .hljs-title.class_ { color: #ffa657; }
.hljs-attr, .hljs-attribute { color: #79c0ff; }
.hljs-variable, .hljs-template-variable { color: #ffa657; }
.hljs-regexp { color: #a5d6ff; }
.hljs-symbol { color: #79c0ff; }
.hljs-meta { color: #8b949e; }
.hljs-deletion { color: #ffa198; background: rgba(255,129,130,0.1); }
.hljs-addition { background: rgba(63,185,80,0.1); }
.hljs-section { color: #79c0ff; font-weight: bold; }
.hljs-emphasis { font-style: italic; }
.hljs-strong { font-weight: bold; }
```

**Step 3: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -E "Error|error" | head -5`
Expected: No new errors from ChatRenderer.svelte

**Step 4: Commit**

```bash
git add src/lib/components/chat/
git commit -m "feat(chat): add ChatRenderer with marked.js + highlight.js rendering pipeline"
```

---

### Task 4: Block Type System & ChatMessage Component

**Files:**
- Create: `src/lib/components/chat/types.ts`
- Create: `src/lib/components/chat/ChatMessage.svelte`

**Step 1: Create the block type system**

`src/lib/components/chat/types.ts`:

```typescript
/** Block types for the Unified Stream renderer */
export type BlockType =
	| 'chat'
	| 'code'
	| 'thinking'
	| 'tool'
	| 'terminal'
	| 'diff'
	| 'diagram'
	| 'math'
	| 'image'
	| 'routing'
	| 'error'
	| 'system';

export interface ChatBlock {
	id: string;
	type: BlockType;
	content: string;
	meta?: {
		lang?: string;
		tool?: string;
		model?: string;
		status?: string;
		durationMs?: number;
	};
}

/** Parse raw assistant content into typed blocks */
export function parseBlocks(content: string): ChatBlock[] {
	const blocks: ChatBlock[] = [];
	// Match thinking blocks: <thinking>...</thinking>
	// Match tool blocks: ```tool\n{...}\n```
	// Match code blocks: ```lang\n...\n```
	// Everything else is chat

	const parts = content.split(/(<thinking>[\s\S]*?<\/thinking>)/);

	for (const part of parts) {
		if (!part) continue;

		// Thinking block
		const thinkMatch = part.match(/^<thinking>([\s\S]*)<\/thinking>$/);
		if (thinkMatch) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'thinking',
				content: thinkMatch[1].trim(),
			});
			continue;
		}

		// For non-thinking parts, just treat as chat (marked.js handles code blocks internally)
		if (part.trim()) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'chat',
				content: part,
			});
		}
	}

	return blocks.length > 0 ? blocks : [{ id: crypto.randomUUID(), type: 'chat', content }];
}
```

**Step 2: Create ChatMessage component**

`src/lib/components/chat/ChatMessage.svelte`:

```svelte
<script lang="ts">
	import ChatRenderer from './ChatRenderer.svelte';
	import { parseBlocks } from './types';
	import {
		Bot, User, Loader2, Copy, Check, ChevronDown, ChevronRight,
		Cpu, Cloud, Zap, Layers, Brain
	} from '@lucide/svelte';
	import { license } from '$lib/stores/license.svelte';
	import type { Message } from '$lib/stores/chat.svelte';

	interface Props {
		message: Message;
		compact?: boolean;
	}

	let { message, compact = false }: Props = $props();

	let copied = $state(false);
	let thinkingOpen = $state(false);

	let blocks = $derived(parseBlocks(message.content));

	function getCascadeInfo(model?: string) {
		if (!model) return { tier: 0, tierLabel: 'Tier 0', shortModel: 'Unknown', cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers };
		const m = model.toLowerCase();
		if (m.includes('ollama:') || m.includes('local')) {
			const shortModel = model.replace(/^(Ollama|Local[^:]*):?\s*/i, '').trim() || model;
			return { tier: 0, tierLabel: 'Tier 0 - Local', shortModel, cost: 'Free', colorClass: 'text-emerald-400', icon: Cpu };
		}
		if (m.includes(':free')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').replace(/:free$/i, '').trim();
			return { tier: 1, tierLabel: 'Tier 1 - Cloud Free', shortModel, cost: 'Free', colorClass: 'text-sky-400', icon: Cloud };
		}
		if (m.includes('openrouter:')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').trim();
			return { tier: 2, tierLabel: 'Tier 2 - Cloud Pro', shortModel, cost: 'Paid', colorClass: 'text-amber-400', icon: Zap };
		}
		return { tier: 0, tierLabel: 'Tier 0', shortModel: model, cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers };
	}

	function copyToClipboard() {
		navigator.clipboard.writeText(message.content);
		copied = true;
		setTimeout(() => { copied = false; }, 2000);
	}

	function formatTime(date: Date): string {
		return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}

	let cascade = $derived(getCascadeInfo(message.model));
</script>

<div class="flex gap-3 {message.role === 'user' ? 'justify-end' : ''} {compact ? 'py-1' : 'py-2'}">
	<!-- Assistant avatar -->
	{#if message.role === 'assistant'}
		<div class="w-8 h-8 rounded-full bg-gx-neon/10 flex items-center justify-center shrink-0 mt-0.5 border border-gx-neon/20">
			{#if message.streaming}
				<Loader2 size={14} class="text-gx-neon animate-spin" />
			{:else}
				<Bot size={14} class="text-gx-neon" />
			{/if}
		</div>
	{/if}

	<!-- Message bubble -->
	<div class="max-w-[80%] min-w-0 {message.role === 'user'
		? 'bg-gx-neon/10 border border-gx-neon/20'
		: 'bg-gx-bg-elevated border border-gx-border-default'} rounded-gx-lg px-4 py-3">

		<!-- Blocks rendering -->
		<div class="text-sm leading-relaxed">
			{#each blocks as block (block.id)}
				{#if block.type === 'thinking'}
					<div class="my-2 border border-amber-400/20 rounded-lg overflow-hidden">
						<button
							onclick={() => thinkingOpen = !thinkingOpen}
							class="flex items-center gap-2 w-full px-3 py-1.5 text-[11px] text-amber-400/70 bg-amber-400/5 hover:bg-amber-400/10 transition-colors"
						>
							<Brain size={12} />
							{#if thinkingOpen}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
							<span>Thinking</span>
						</button>
						{#if thinkingOpen}
							<div class="px-3 py-2 text-xs text-gx-text-muted opacity-70">
								<ChatRenderer content={block.content} />
							</div>
						{/if}
					</div>
				{:else if block.type === 'chat'}
					<ChatRenderer content={block.content} streaming={message.streaming && block === blocks[blocks.length - 1]} />
				{/if}
			{/each}

			{#if message.streaming && message.content === ''}
				<span class="inline-flex gap-1 items-center text-gx-text-muted">
					<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 0ms"></span>
					<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 150ms"></span>
					<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 300ms"></span>
				</span>
			{/if}
		</div>

		<!-- Message footer -->
		<div class="flex items-center gap-2 mt-2 text-[10px] text-gx-text-muted">
			<span>{formatTime(message.timestamp)}</span>
			{#if message.role === 'assistant' && message.model}
				<span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-gx-bg-primary border border-gx-border-default text-[9px] font-mono {cascade.colorClass}">
					<cascade.icon size={9} />
					{#if license.isPro}
						{cascade.shortModel}
					{:else}
						{cascade.tier === 0 ? 'Local' : cascade.shortModel}
					{/if}
				</span>
				{#if license.isPro}
					<span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[9px] {cascade.tier === 0
						? 'bg-emerald-400/10 border-emerald-400/20 text-emerald-400'
						: cascade.tier === 1
							? 'bg-sky-400/10 border-sky-400/20 text-sky-400'
							: 'bg-amber-400/10 border-amber-400/20 text-amber-400'}">
						{cascade.tierLabel}
					</span>
				{/if}
			{/if}
			{#if message.taskType}
				<span class="px-1.5 py-0.5 rounded bg-gx-neon/10 text-gx-neon text-[9px]">{message.taskType}</span>
			{/if}
			{#if message.role === 'assistant' && !message.streaming}
				<button onclick={copyToClipboard} class="ml-auto hover:text-gx-neon transition-colors p-0.5" aria-label="Copy">
					{#if copied}<Check size={11} class="text-gx-status-success" />{:else}<Copy size={11} />{/if}
				</button>
			{/if}
		</div>
	</div>

	<!-- User avatar -->
	{#if message.role === 'user'}
		<div class="w-8 h-8 rounded-full bg-gx-bg-elevated flex items-center justify-center shrink-0 mt-0.5 border border-gx-border-default">
			<User size={14} class="text-gx-text-muted" />
		</div>
	{/if}
</div>
```

**Step 3: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -c "Error"`
Expected: Same count as before (3 pre-existing errors in ThemeImporter.svelte)

**Step 4: Commit**

```bash
git add src/lib/components/chat/
git commit -m "feat(chat): add block type system and ChatMessage component with thinking blocks"
```

---

## Phase 2: Three Stream Rendering Modes

### Task 5: ChatInput Component with @-Mention Autocomplete

**Files:**
- Create: `src/lib/components/chat/ChatInput.svelte`

**Step 1: Create the input component**

```svelte
<script lang="ts">
	import { Send, Loader2, AtSign } from '@lucide/svelte';

	interface Props {
		onSend: (message: string) => void;
		isStreaming: boolean;
		placeholder?: string;
	}

	let { onSend, isStreaming, placeholder = 'Type your message... (Enter to send, Shift+Enter for new line)' }: Props = $props();

	let value = $state('');
	let showMentions = $state(false);
	let mentionQuery = $state('');
	let selectedMentionIndex = $state(0);

	const MENTION_SOURCES = [
		{ id: 'file', label: '@file', description: 'Include a file as context', icon: '📄' },
		{ id: 'codebase', label: '@codebase', description: 'Search project code', icon: '🔍' },
		{ id: 'docs', label: '@docs', description: 'Search documentation', icon: '📚' },
		{ id: 'terminal', label: '@terminal', description: 'Recent terminal output', icon: '📟' },
		{ id: 'memory', label: '@memory', description: 'ForgeMemory lookup', icon: '🧠' },
		{ id: 'model', label: '@model', description: 'Force specific model', icon: '🤖' },
		{ id: 'diff', label: '@diff', description: 'Current git diff', icon: '📊' },
		{ id: 'errors', label: '@errors', description: 'Lint/build errors', icon: '❌' },
	];

	let filteredMentions = $derived(
		mentionQuery
			? MENTION_SOURCES.filter(s => s.label.includes(mentionQuery.toLowerCase()) || s.description.toLowerCase().includes(mentionQuery.toLowerCase()))
			: MENTION_SOURCES
	);

	function handleKeydown(e: KeyboardEvent) {
		if (showMentions) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				selectedMentionIndex = (selectedMentionIndex + 1) % filteredMentions.length;
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				selectedMentionIndex = (selectedMentionIndex - 1 + filteredMentions.length) % filteredMentions.length;
				return;
			}
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				insertMention(filteredMentions[selectedMentionIndex]);
				return;
			}
			if (e.key === 'Escape') {
				showMentions = false;
				return;
			}
		}

		if (e.key === 'Enter' && (e.ctrlKey || !e.shiftKey)) {
			e.preventDefault();
			handleSend();
		}
	}

	function handleInput() {
		// Detect @ at cursor position
		const lastAt = value.lastIndexOf('@');
		if (lastAt >= 0) {
			const afterAt = value.slice(lastAt + 1);
			// Only show if @ is at word boundary and no space after query
			if (!afterAt.includes(' ') && (lastAt === 0 || value[lastAt - 1] === ' ' || value[lastAt - 1] === '\n')) {
				mentionQuery = afterAt;
				showMentions = true;
				selectedMentionIndex = 0;
				return;
			}
		}
		showMentions = false;
	}

	function insertMention(source: typeof MENTION_SOURCES[number]) {
		const lastAt = value.lastIndexOf('@');
		value = value.slice(0, lastAt) + source.label + ' ';
		showMentions = false;
	}

	function handleSend() {
		const msg = value.trim();
		if (!msg || isStreaming) return;
		value = '';
		showMentions = false;
		onSend(msg);
	}
</script>

<div class="border-t border-gx-border-default bg-gx-bg-secondary p-3">
	<!-- @-mention popup -->
	{#if showMentions && filteredMentions.length > 0}
		<div class="absolute bottom-full left-0 right-0 mx-3 mb-1 bg-gx-bg-elevated border border-gx-border-default rounded-lg shadow-lg max-h-[200px] overflow-y-auto z-50">
			{#each filteredMentions as source, i (source.id)}
				<button
					onclick={() => insertMention(source)}
					class="flex items-center gap-3 w-full px-3 py-2 text-sm text-left transition-colors
						{i === selectedMentionIndex ? 'bg-gx-neon/10 text-gx-neon' : 'text-gx-text-secondary hover:bg-gx-bg-hover'}"
				>
					<span class="text-base">{source.icon}</span>
					<div>
						<div class="font-mono text-xs">{source.label}</div>
						<div class="text-[10px] text-gx-text-muted">{source.description}</div>
					</div>
				</button>
			{/each}
		</div>
	{/if}

	<div class="flex gap-2 items-end relative">
		<div class="flex-1 relative">
			<textarea
				bind:value
				onkeydown={handleKeydown}
				oninput={handleInput}
				{placeholder}
				rows={1}
				disabled={isStreaming}
				class="w-full px-4 py-2.5 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx-lg resize-none
					focus:border-gx-neon focus:outline-none transition-colors disabled:opacity-50
					min-h-[40px] max-h-[160px]"
				style="field-sizing: content;"
			></textarea>
		</div>
		<button
			onclick={handleSend}
			disabled={isStreaming || !value.trim()}
			class="px-4 py-2.5 bg-gx-neon text-gx-bg-primary font-medium text-sm rounded-gx-lg
				hover:brightness-110 transition-all disabled:opacity-30 disabled:cursor-not-allowed
				flex items-center gap-2 shadow-gx-glow-sm shrink-0"
		>
			{#if isStreaming}
				<Loader2 size={16} class="animate-spin" />
			{:else}
				<Send size={16} />
			{/if}
		</button>
	</div>
	<div class="flex items-center gap-3 mt-1.5 text-[10px] text-gx-text-muted">
		<span>Enter to send · Shift+Enter for new line · @ for context</span>
	</div>
</div>
```

**Step 2: Commit**

```bash
git add src/lib/components/chat/ChatInput.svelte
git commit -m "feat(chat): add ChatInput component with @-mention autocomplete"
```

---

### Task 6: ChatSidebar Component (shared across modes)

**Files:**
- Create: `src/lib/components/chat/ChatSidebar.svelte`

**Step 1: Extract sidebar from chat/+page.svelte into reusable component**

```svelte
<script lang="ts">
	import { chatStore } from '$lib/stores/chat.svelte';
	import { MessageSquare, Plus, Trash2 } from '@lucide/svelte';

	interface Props {
		collapsed?: boolean;
	}

	let { collapsed = false }: Props = $props();
</script>

{#if !collapsed}
	<div class="w-[250px] bg-gx-bg-secondary border-r border-gx-border-default flex flex-col shrink-0">
		<div class="p-3 border-b border-gx-border-default">
			<button
				onclick={() => chatStore.newConversation()}
				class="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors shadow-gx-glow-sm"
			>
				<Plus size={14} />
				New Chat
			</button>
		</div>

		<div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5">
			{#each chatStore.conversations as conv (conv.id)}
				<div
					role="button"
					tabindex="0"
					onclick={() => chatStore.setActive(conv.id)}
					onkeydown={(e) => e.key === 'Enter' && chatStore.setActive(conv.id)}
					class="group flex items-center gap-2 px-3 py-2.5 text-xs rounded-gx cursor-pointer transition-colors
						{chatStore.activeConversationId === conv.id
							? 'bg-gx-bg-elevated text-gx-text-primary border-l-2 border-gx-neon'
							: 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
				>
					<MessageSquare size={12} class="shrink-0 {chatStore.activeConversationId === conv.id ? 'text-gx-neon' : ''}" />
					<span class="flex-1 truncate">{conv.title}</span>
					<button
						onclick={(e) => { e.stopPropagation(); chatStore.deleteConversation(conv.id); }}
						class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all p-0.5 rounded"
						aria-label="Delete conversation"
					>
						<Trash2 size={12} />
					</button>
				</div>
			{/each}

			{#if chatStore.conversations.length === 0}
				<div class="text-[11px] text-gx-text-muted text-center py-8 px-4">
					No conversations yet. Start a new chat!
				</div>
			{/if}
		</div>

		<div class="p-3 border-t border-gx-border-default">
			<div class="text-[10px] text-gx-text-muted text-center">
				{chatStore.conversations.length} conversation{chatStore.conversations.length !== 1 ? 's' : ''}
			</div>
		</div>
	</div>
{/if}
```

**Step 2: Commit**

```bash
git add src/lib/components/chat/ChatSidebar.svelte
git commit -m "feat(chat): extract ChatSidebar as reusable component"
```

---

### Task 7: Model Visualization Components (3 levels)

**Files:**
- Create: `src/lib/stores/model-status.svelte.ts`
- Create: `src/lib/components/chat/ModelStatusBadge.svelte`
- Create: `src/lib/components/chat/ModelActivityCard.svelte`
- Create: `src/lib/components/chat/ModelPipelineView.svelte`

**Step 1: Create ModelStatus store**

`src/lib/stores/model-status.svelte.ts`:

```typescript
/**
 * Model Status Store — tracks which AI models are active, their state, and pipeline flow.
 * Feeds all three visualization levels: Badges, Activity Cards, and Pipeline DAG.
 */

export interface ModelState {
	id: string;
	name: string;
	status: 'idle' | 'thinking' | 'generating' | 'error';
	currentTask: string | null;
	tokensGenerated: number;
	tokensTotal: number | null;
	latencyMs: number;
	routingReason: string | null;
	lastActive: Date;
}

export interface PipelineNode {
	id: string;
	type: 'input' | 'classifier' | 'model' | 'memory' | 'output';
	label: string;
	status: 'idle' | 'active' | 'completed' | 'error';
	x: number;
	y: number;
	metrics?: { tokens: number; latencyMs: number };
}

export interface PipelineEdge {
	from: string;
	to: string;
	active: boolean;
}

class ModelStatusStore {
	models = $state<ModelState[]>([]);
	pipeline = $state<PipelineNode[]>([]);
	edges = $state<PipelineEdge[]>([]);
	lastRouting = $state<{ taskType: string; model: string; reason: string } | null>(null);

	get activeModel() {
		return this.models.find((m) => m.status === 'generating' || m.status === 'thinking') ?? null;
	}

	/** Called when chat_stream sends a Started event */
	onStarted(model: string, taskType: string) {
		// Upsert model state
		const existing = this.models.find((m) => m.name === model);
		if (existing) {
			existing.status = 'generating';
			existing.currentTask = taskType;
			existing.tokensGenerated = 0;
			existing.lastActive = new Date();
		} else {
			this.models.push({
				id: crypto.randomUUID(),
				name: model,
				status: 'generating',
				currentTask: taskType,
				tokensGenerated: 0,
				tokensTotal: null,
				latencyMs: 0,
				routingReason: null,
				lastActive: new Date(),
			});
		}

		// Update pipeline
		this.pipeline = [
			{ id: 'input', type: 'input', label: 'User Input', status: 'completed', x: 0, y: 50 },
			{ id: 'classifier', type: 'classifier', label: `Classifier → ${taskType}`, status: 'completed', x: 150, y: 50 },
			{ id: 'model', type: 'model', label: model.split('/').pop() || model, status: 'active', x: 350, y: 50 },
			{ id: 'memory', type: 'memory', label: 'ForgeMemory', status: 'active', x: 350, y: 120 },
			{ id: 'output', type: 'output', label: 'Output', status: 'idle', x: 550, y: 50 },
		];
		this.edges = [
			{ from: 'input', to: 'classifier', active: false },
			{ from: 'classifier', to: 'model', active: true },
			{ from: 'memory', to: 'model', active: true },
			{ from: 'model', to: 'output', active: false },
		];

		this.lastRouting = { taskType, model, reason: `Task classified as ${taskType}` };
	}

	/** Called on each Delta event */
	onDelta() {
		const active = this.activeModel;
		if (active) {
			active.tokensGenerated += 1;
		}
	}

	/** Called when streaming finishes */
	onFinished(totalTokens: number) {
		const active = this.activeModel;
		if (active) {
			active.status = 'idle';
			active.tokensGenerated = totalTokens;
			active.tokensTotal = totalTokens;
		}

		// Update pipeline
		const modelNode = this.pipeline.find((n) => n.id === 'model');
		const outputNode = this.pipeline.find((n) => n.id === 'output');
		if (modelNode) modelNode.status = 'completed';
		if (outputNode) outputNode.status = 'completed';
		this.edges = this.edges.map((e) => ({ ...e, active: false }));
	}

	/** Called on error */
	onError() {
		const active = this.activeModel;
		if (active) active.status = 'error';
	}

	/** Reset pipeline state */
	reset() {
		this.pipeline = [];
		this.edges = [];
	}
}

export const modelStatus = new ModelStatusStore();
```

**Step 2: Create ModelStatusBadge (Minimal visualization)**

`src/lib/components/chat/ModelStatusBadge.svelte`:

```svelte
<script lang="ts">
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import type { ModelState } from '$lib/stores/model-status.svelte';

	interface Props {
		model?: ModelState;
	}

	let { model }: Props = $props();

	let displayModel = $derived(model ?? modelStatus.activeModel);

	const statusColors: Record<string, string> = {
		idle: 'bg-gray-400',
		thinking: 'bg-amber-400 animate-pulse',
		generating: 'bg-cyan-400 animate-pulse',
		error: 'bg-red-400',
	};
</script>

{#if displayModel}
	<div class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-gx-bg-elevated border border-gx-border-default text-[10px]">
		<span class="w-2 h-2 rounded-full {statusColors[displayModel.status] ?? 'bg-gray-400'}"></span>
		<span class="font-mono text-gx-text-secondary truncate max-w-[120px]">{displayModel.name.split('/').pop()}</span>
		{#if displayModel.status === 'generating'}
			<span class="text-gx-text-muted">{displayModel.tokensGenerated}tk</span>
		{/if}
	</div>
{/if}
```

**Step 3: Create ModelActivityCard (Medium visualization)**

`src/lib/components/chat/ModelActivityCard.svelte`:

```svelte
<script lang="ts">
	import type { ModelState } from '$lib/stores/model-status.svelte';
	import { Cpu, Cloud, Zap, Loader2 } from '@lucide/svelte';

	interface Props {
		model: ModelState;
	}

	let { model }: Props = $props();

	let progress = $derived(
		model.tokensTotal ? Math.round((model.tokensGenerated / model.tokensTotal) * 100) : null
	);

	let statusLabel = $derived({
		idle: 'Standby',
		thinking: 'Thinking...',
		generating: 'Generating',
		error: 'Error',
	}[model.status]);

	let glowClass = $derived({
		idle: '',
		thinking: 'shadow-[0_0_15px_rgba(245,158,11,0.3)]',
		generating: 'shadow-[0_0_15px_rgba(0,212,255,0.3)]',
		error: 'shadow-[0_0_15px_rgba(239,68,68,0.3)]',
	}[model.status]);
</script>

<div class="p-3 rounded-lg bg-gx-bg-elevated border border-gx-border-default {glowClass} transition-shadow duration-500">
	<div class="flex items-center gap-2 mb-2">
		{#if model.status === 'generating'}
			<Zap size={14} class="text-cyan-400" />
		{:else if model.status === 'thinking'}
			<Loader2 size={14} class="text-amber-400 animate-spin" />
		{:else}
			<Cpu size={14} class="text-gx-text-muted" />
		{/if}
		<span class="text-xs font-medium text-gx-text-primary truncate">{model.name.split('/').pop()}</span>
	</div>

	<!-- Progress bar -->
	<div class="w-full h-1.5 bg-gx-bg-primary rounded-full mb-2 overflow-hidden">
		{#if model.status === 'generating'}
			<div
				class="h-full bg-gradient-to-r from-cyan-400 to-gx-neon rounded-full transition-all duration-300"
				style="width: {progress ?? 50}%"
			></div>
		{:else if model.status === 'thinking'}
			<div class="h-full bg-amber-400 rounded-full animate-pulse w-full opacity-30"></div>
		{/if}
	</div>

	<div class="flex justify-between text-[10px] text-gx-text-muted">
		<span>{statusLabel}</span>
		{#if model.currentTask}
			<span class="text-gx-neon">{model.currentTask}</span>
		{/if}
	</div>

	{#if model.status === 'generating'}
		<div class="flex justify-between text-[10px] text-gx-text-muted mt-1">
			<span>{model.tokensGenerated} tokens</span>
			{#if model.latencyMs > 0}
				<span>{model.latencyMs.toFixed(0)}ms/tk</span>
			{/if}
		</div>
	{/if}

	{#if model.routingReason}
		<div class="mt-1.5 text-[9px] text-gx-text-muted italic truncate">
			{model.routingReason}
		</div>
	{/if}
</div>
```

**Step 4: Create ModelPipelineView (Full DAG visualization)**

`src/lib/components/chat/ModelPipelineView.svelte`:

```svelte
<script lang="ts">
	import { modelStatus } from '$lib/stores/model-status.svelte';

	const NODE_RADIUS = 24;
	const SVG_WIDTH = 650;
	const SVG_HEIGHT = 180;

	const statusColors: Record<string, string> = {
		idle: '#6b7280',
		active: '#00d4ff',
		completed: '#22c55e',
		error: '#ef4444',
	};

	const typeIcons: Record<string, string> = {
		input: 'I',
		classifier: 'C',
		model: 'M',
		memory: 'K',
		output: 'O',
	};
</script>

{#if modelStatus.pipeline.length > 0}
	<div class="rounded-lg border border-gx-border-default bg-gx-bg-primary/50 p-2 overflow-x-auto">
		<svg width={SVG_WIDTH} height={SVG_HEIGHT} viewBox="0 0 {SVG_WIDTH} {SVG_HEIGHT}" class="w-full">
			<defs>
				<filter id="glow">
					<feGaussianBlur stdDeviation="3" result="coloredBlur" />
					<feMerge>
						<feMergeNode in="coloredBlur" />
						<feMergeNode in="SourceGraphic" />
					</feMerge>
				</filter>
				<!-- Animated dash for active edges -->
				<marker id="arrow" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
					<path d="M0,0 L8,3 L0,6" fill="#00d4ff" opacity="0.6" />
				</marker>
			</defs>

			<!-- Edges -->
			{#each modelStatus.edges as edge}
				{@const fromNode = modelStatus.pipeline.find((n) => n.id === edge.from)}
				{@const toNode = modelStatus.pipeline.find((n) => n.id === edge.to)}
				{#if fromNode && toNode}
					<line
						x1={fromNode.x + NODE_RADIUS + 20}
						y1={fromNode.y}
						x2={toNode.x - NODE_RADIUS + 20}
						y2={toNode.y}
						stroke={edge.active ? '#00d4ff' : '#374151'}
						stroke-width={edge.active ? 2 : 1}
						stroke-dasharray={edge.active ? '8 4' : 'none'}
						marker-end="url(#arrow)"
					>
						{#if edge.active}
							<animate attributeName="stroke-dashoffset" from="12" to="0" dur="0.8s" repeatCount="indefinite" />
						{/if}
					</line>
				{/if}
			{/each}

			<!-- Nodes -->
			{#each modelStatus.pipeline as node}
				<g transform="translate({node.x + 20}, {node.y})">
					<!-- Glow for active nodes -->
					{#if node.status === 'active'}
						<circle r={NODE_RADIUS + 4} fill="none" stroke="#00d4ff" stroke-width="1" opacity="0.3" filter="url(#glow)">
							<animate attributeName="r" values="{NODE_RADIUS + 2};{NODE_RADIUS + 6};{NODE_RADIUS + 2}" dur="2s" repeatCount="indefinite" />
							<animate attributeName="opacity" values="0.3;0.6;0.3" dur="2s" repeatCount="indefinite" />
						</circle>
					{/if}

					<!-- Node circle -->
					<circle
						r={NODE_RADIUS}
						fill="#1a1a2e"
						stroke={statusColors[node.status]}
						stroke-width="2"
					/>

					<!-- Type icon -->
					<text
						text-anchor="middle"
						dominant-baseline="central"
						fill={statusColors[node.status]}
						font-size="14"
						font-weight="bold"
						font-family="monospace"
					>{typeIcons[node.type] ?? '?'}</text>

					<!-- Label below -->
					<text
						text-anchor="middle"
						y={NODE_RADIUS + 14}
						fill="#9ca3af"
						font-size="9"
						font-family="sans-serif"
					>{node.label.length > 18 ? node.label.slice(0, 18) + '...' : node.label}</text>

					<!-- Metrics above (if present) -->
					{#if node.metrics}
						<text
							text-anchor="middle"
							y={-NODE_RADIUS - 6}
							fill="#00d4ff"
							font-size="8"
							font-family="monospace"
						>{node.metrics.tokens}tk · {node.metrics.latencyMs}ms</text>
					{/if}
				</g>
			{/each}

			<!-- Routing label -->
			{#if modelStatus.lastRouting}
				<text x={SVG_WIDTH / 2} y={SVG_HEIGHT - 8} text-anchor="middle" fill="#6b7280" font-size="9" font-family="sans-serif">
					Route: {modelStatus.lastRouting.reason}
				</text>
			{/if}
		</svg>
	</div>
{/if}
```

**Step 5: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -c "Error"`
Expected: Same count as before (3 pre-existing)

**Step 6: Commit**

```bash
git add src/lib/stores/model-status.svelte.ts src/lib/components/chat/ModelStatusBadge.svelte src/lib/components/chat/ModelActivityCard.svelte src/lib/components/chat/ModelPipelineView.svelte
git commit -m "feat(chat): add 3-level model visualization — Badge, ActivityCard, PipelineView"
```

---

### Task 8: Upgraded Dedicated Chat Page (Stream Mode: Unified)

**Files:**
- Modify: `src/routes/chat/+page.svelte` (full rewrite)

**Step 1: Rewrite chat page using new components**

Replace the entire content of `src/routes/chat/+page.svelte`:

```svelte
<script lang="ts">
	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from '$lib/components/chat/ChatMessage.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import ChatSidebar from '$lib/components/chat/ChatSidebar.svelte';
	import ModelStatusBadge from '$lib/components/chat/ModelStatusBadge.svelte';
	import ModelActivityCard from '$lib/components/chat/ModelActivityCard.svelte';
	import ModelPipelineView from '$lib/components/chat/ModelPipelineView.svelte';
	import '$lib/components/chat/hljs-forge.css';
	import { Bot, Loader2, PanelLeftClose, PanelLeft, LayoutDashboard } from '@lucide/svelte';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let sidebarOpen = $state(true);

	let streamMode = $derived(getSetting('chatStreamMode'));
	let vizLevel = $derived(getSetting('chatVizLevel'));
	let compact = $derived(getSetting('chatCompactMode'));
	let activeMessages = $derived(chatStore.messages);

	function scrollToBottom() {
		if (messagesContainer) {
			requestAnimationFrame(() => {
				messagesContainer!.scrollTop = messagesContainer!.scrollHeight;
			});
		}
	}

	$effect(() => {
		if (activeMessages.length > 0) scrollToBottom();
	});

	async function handleSend(msg: string) {
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}
</script>

<div class="flex h-full">
	<!-- Sidebar -->
	<ChatSidebar collapsed={!sidebarOpen} />

	<!-- Main area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Header -->
		<div class="flex items-center gap-3 px-4 py-2 border-b border-gx-border-default bg-gx-bg-secondary">
			<button
				onclick={() => sidebarOpen = !sidebarOpen}
				class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
				aria-label="Toggle sidebar"
			>
				{#if sidebarOpen}<PanelLeftClose size={16} />{:else}<PanelLeft size={16} />{/if}
			</button>
			<div class="flex-1 min-w-0">
				<h1 class="text-sm font-medium text-gx-text-primary truncate">
					{chatStore.activeConversation?.title ?? 'ImpForge Chat'}
				</h1>
			</div>

			<!-- Model Status Badge (minimal viz) -->
			{#if vizLevel === 'minimal'}
				<ModelStatusBadge />
			{/if}

			{#if chatStore.isStreaming}
				<div class="flex items-center gap-1.5 text-[11px] text-gx-neon">
					<Loader2 size={12} class="animate-spin" />
					<span>Streaming</span>
				</div>
			{/if}
		</div>

		<!-- Content area: depends on stream mode -->
		<div class="flex-1 flex min-h-0">
			<!-- Chat messages -->
			<div class="flex-1 flex flex-col min-w-0">
				<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-4 py-4 space-y-2">
					{#if activeMessages.length === 0}
						<!-- Empty state -->
						<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
							<div class="w-20 h-20 rounded-full bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shadow-gx-glow-md">
								<Bot size={36} class="text-gx-neon" />
							</div>
							<h2 class="text-lg font-medium text-gx-text-secondary">Start a new conversation</h2>
							<p class="text-sm max-w-md text-center leading-relaxed">
								Ask me anything. Your message will be routed to the best AI model automatically.
							</p>
							<div class="flex flex-wrap justify-center gap-2 mt-2 max-w-lg">
								{#each ['Write a Python function', 'Create a Dockerfile', 'Explain async/await', 'Debug my code', 'Design a system', 'Research a topic'] as suggestion}
									<button
										onclick={() => handleSend(suggestion)}
										class="px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-muted hover:border-gx-neon hover:text-gx-neon transition-colors"
									>
										{suggestion}
									</button>
								{/each}
							</div>
						</div>
					{:else}
						<!-- Pipeline visualization (if enabled, shown above messages) -->
						{#if vizLevel === 'pipeline' && chatStore.isStreaming}
							<div class="mb-4">
								<ModelPipelineView />
							</div>
						{/if}

						{#each activeMessages as msg (msg.id)}
							<ChatMessage message={msg} compact={compact} />
						{/each}
					{/if}
				</div>

				<!-- Input -->
				<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} />
			</div>

			<!-- Right panel: Mission Control or Activity Cards -->
			{#if streamMode === 'mission-control' || vizLevel === 'cards'}
				<div class="w-[280px] border-l border-gx-border-default bg-gx-bg-secondary p-3 overflow-y-auto shrink-0">
					{#if vizLevel === 'pipeline' || vizLevel === 'cards'}
						<div class="text-[11px] font-medium text-gx-text-muted uppercase tracking-wider mb-3">
							<LayoutDashboard size={12} class="inline mr-1" />
							Model Status
						</div>
						<div class="space-y-2">
							{#each modelStatus.models as model (model.id)}
								<ModelActivityCard {model} />
							{/each}
							{#if modelStatus.models.length === 0}
								<div class="text-[11px] text-gx-text-muted text-center py-4">
									No models active. Send a message to start.
								</div>
							{/if}
						</div>
					{/if}

					{#if streamMode === 'mission-control'}
						<!-- Pipeline always visible in mission control -->
						<div class="mt-4">
							<div class="text-[11px] font-medium text-gx-text-muted uppercase tracking-wider mb-2">
								Pipeline
							</div>
							<ModelPipelineView />
						</div>

						{#if modelStatus.lastRouting}
							<div class="mt-4 p-2 rounded bg-gx-bg-primary border border-gx-border-default">
								<div class="text-[10px] text-gx-text-muted mb-1">Last Routing Decision</div>
								<div class="text-[11px] text-gx-text-secondary">
									<span class="text-gx-neon">{modelStatus.lastRouting.taskType}</span> →
									<span class="font-mono">{modelStatus.lastRouting.model.split('/').pop()}</span>
								</div>
							</div>
						{/if}
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
```

**Step 2: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -c "Error"`
Expected: Same count as before (3 pre-existing)

**Step 3: Commit**

```bash
git add src/routes/chat/+page.svelte
git commit -m "feat(chat): upgrade dedicated chat page with ChatRenderer, 3 viz levels, mission control"
```

---

### Task 9: Wire ModelStatus Store to Chat Streaming

**Files:**
- Modify: `src/lib/stores/chat.svelte.ts`

**Step 1: Import and wire modelStatus events**

Add import at line 2:

```typescript
import { modelStatus } from './model-status.svelte';
```

Update the `channel.onmessage` handler inside `sendMessage()` to call modelStatus:

Replace the switch statement body (lines 103-124) with:

```typescript
		channel.onmessage = (event: ChatEvent) => {
			switch (event.event) {
				case 'Started':
					assistantMsg.model = event.data.model;
					assistantMsg.taskType = event.data.task_type;
					modelStatus.onStarted(event.data.model, event.data.task_type);
					break;
				case 'Delta':
					assistantMsg.content += event.data.content;
					modelStatus.onDelta();
					break;
				case 'Finished':
					assistantMsg.streaming = false;
					this.isStreaming = false;
					modelStatus.onFinished(event.data.total_tokens);
					if (conv.title === 'New Chat' && conv.messages.length >= 2) {
						conv.title = conv.messages[0].content.slice(0, 50);
					}
					break;
				case 'Error':
					assistantMsg.content = `Error: ${event.data.message}`;
					assistantMsg.streaming = false;
					this.isStreaming = false;
					modelStatus.onError();
					break;
			}
		};
```

**Step 2: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -c "Error"`
Expected: Same count as before

**Step 3: Commit**

```bash
git add src/lib/stores/chat.svelte.ts
git commit -m "feat(chat): wire ModelStatus store to chat streaming events"
```

---

## Phase 3: Chat Placement Modes

### Task 10: Side-Panel Chat Component

**Files:**
- Create: `src/lib/components/chat/ChatSidePanel.svelte`

**Step 1: Create the side-panel wrapper**

This component wraps the chat in a slide-out panel that can appear on any page.

```svelte
<script lang="ts">
	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from './ChatMessage.svelte';
	import ChatInput from './ChatInput.svelte';
	import ModelStatusBadge from './ModelStatusBadge.svelte';
	import ModelPipelineView from './ModelPipelineView.svelte';
	import './hljs-forge.css';
	import { Bot, X, Maximize2 } from '@lucide/svelte';
	import { goto } from '$app/navigation';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	let messagesContainer: HTMLDivElement | undefined = $state();
	let vizLevel = $derived(getSetting('chatVizLevel'));
	let activeMessages = $derived(chatStore.messages);

	function scrollToBottom() {
		if (messagesContainer) {
			requestAnimationFrame(() => {
				messagesContainer!.scrollTop = messagesContainer!.scrollHeight;
			});
		}
	}

	$effect(() => {
		if (activeMessages.length > 0 && open) scrollToBottom();
	});

	async function handleSend(msg: string) {
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}

	function openFullChat() {
		onClose();
		goto('/chat');
	}
</script>

{#if open}
	<div class="w-[380px] h-full border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 shadow-xl">
		<!-- Header -->
		<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default">
			<Bot size={16} class="text-gx-neon" />
			<span class="text-xs font-medium text-gx-text-primary flex-1">Chat</span>
			<ModelStatusBadge />
			<button onclick={openFullChat} class="p-1 rounded text-gx-text-muted hover:text-gx-neon" aria-label="Open full chat">
				<Maximize2 size={12} />
			</button>
			<button onclick={onClose} class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary" aria-label="Close chat">
				<X size={14} />
			</button>
		</div>

		<!-- Messages -->
		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-3 py-3 space-y-2">
			{#if activeMessages.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<Bot size={28} class="text-gx-neon opacity-50" />
					<p class="text-xs text-center">Ask anything — AI routed automatically</p>
				</div>
			{:else}
				{#if vizLevel === 'pipeline' && chatStore.isStreaming}
					<ModelPipelineView />
				{/if}
				{#each activeMessages as msg (msg.id)}
					<ChatMessage message={msg} compact={true} />
				{/each}
			{/if}
		</div>

		<!-- Input -->
		<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} placeholder="Ask anything..." />
	</div>
{/if}
```

**Step 2: Commit**

```bash
git add src/lib/components/chat/ChatSidePanel.svelte
git commit -m "feat(chat): add ChatSidePanel for persistent side-panel mode"
```

---

### Task 11: Integrate Side-Panel into Main Layout

**Files:**
- Modify: `src/routes/+layout.svelte`

**Step 1: Add side-panel chat to the layout**

In the `<script>` section of `+layout.svelte`, add these imports and state:

```typescript
import ChatSidePanel from '$lib/components/chat/ChatSidePanel.svelte';
import { getSetting } from '$lib/stores/settings.svelte';

let chatPanelOpen = $state(false);
let chatPlacement = $derived(getSetting('chatPlacement'));
```

**Step 2: Add keyboard shortcut Ctrl+J**

In the existing `handleKeydown` function, add this case:

```typescript
// Ctrl+J — Toggle chat side panel
if (e.ctrlKey && e.key === 'j') {
    e.preventDefault();
    chatPanelOpen = !chatPanelOpen;
    return;
}
```

**Step 3: Add the ChatSidePanel to the layout template**

Right before the closing `</div>` of the main content area, add:

```svelte
<!-- Chat Side Panel (when placement is 'side-panel') -->
{#if chatPlacement === 'side-panel'}
    <ChatSidePanel open={chatPanelOpen} onClose={() => chatPanelOpen = false} />
{/if}
```

**Step 4: Run type check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | grep -c "Error"`
Expected: Same count as before

**Step 5: Commit**

```bash
git add src/routes/+layout.svelte
git commit -m "feat(layout): integrate ChatSidePanel with Ctrl+J toggle"
```

---

### Task 12: Chat Settings UI Section

**Files:**
- Modify: `src/routes/settings/+page.svelte`

**Step 1: Read the current settings page to understand its structure**

Run: `cat src/routes/settings/+page.svelte | head -50`

**Step 2: Add a "Chat Layout" settings section**

Add a new settings group after the existing ones. The section should include:
- Chat Placement: radio group (Side-Panel / Dedicated / Convergence)
- Stream Mode: radio group (Split / Unified / Mission Control)
- Visualization Level: radio group (Minimal / Activity Cards / Pipeline)
- Toggle switches: Show Thinking, Show Routing, Animations, Compact Mode

Each radio/toggle should call `saveSetting(key, value)` onChange.

Example structure for one radio group:

```svelte
<div class="space-y-3">
    <h3 class="text-sm font-medium text-gx-text-primary">Chat Placement</h3>
    <p class="text-[11px] text-gx-text-muted">Where the chat interface lives in the app</p>
    {#each [
        { value: 'side-panel', label: 'Side Panel', desc: 'Slide-out panel on every page (Ctrl+J)' },
        { value: 'dedicated', label: 'Dedicated Page', desc: 'Full chat page with sidebar' },
        { value: 'convergence', label: 'Full Convergence', desc: 'Chat + IDE + Terminal unified' },
    ] as option}
        <label class="flex items-start gap-3 p-2 rounded-lg cursor-pointer hover:bg-gx-bg-hover transition-colors {getSetting('chatPlacement') === option.value ? 'bg-gx-neon/5 border border-gx-neon/20' : ''}">
            <input
                type="radio"
                name="chatPlacement"
                value={option.value}
                checked={getSetting('chatPlacement') === option.value}
                onchange={() => saveSetting('chatPlacement', option.value)}
                class="mt-0.5"
            />
            <div>
                <div class="text-xs font-medium">{option.label}</div>
                <div class="text-[10px] text-gx-text-muted">{option.desc}</div>
            </div>
        </label>
    {/each}
</div>
```

Repeat pattern for streamMode and vizLevel.

**Step 3: Commit**

```bash
git add src/routes/settings/+page.svelte
git commit -m "feat(settings): add Chat Layout settings section for 3x3x3 configuration"
```

---

## Phase 4: Rust Backend Extensions

### Task 13: Extended ChatEvent with Routing Info

**Files:**
- Modify: `src-tauri/src/chat.rs`

**Step 1: Add Routing variant to ChatEvent**

Replace the `ChatEvent` enum with:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum ChatEvent {
    Started { model: String, task_type: String },
    Delta { content: String },
    Finished { total_tokens: u32 },
    Error { message: String },
    Routing {
        task_type: String,
        selected_model: String,
        reason: String,
        classification_ms: f64,
    },
}
```

**Step 2: Send Routing event before Started**

In `chat_stream`, right after `classify_fast` and model selection, add timing and the Routing event:

```rust
    let classify_start = std::time::Instant::now();
    let task_type = crate::router::classify_fast(&message);
    let classification_ms = classify_start.elapsed().as_secs_f64() * 1000.0;
    let task_type_str = format!("{:?}", task_type);

    let model = model_id.unwrap_or_else(|| {
        match task_type {
            crate::router::TaskType::CodeGeneration
            | crate::router::TaskType::DockerfileGen => {
                "mistralai/devstral-small:free".to_string()
            }
            crate::router::TaskType::MultiStepReasoning => {
                "qwen/qwen3-30b-a3b:free".to_string()
            }
            _ => "meta-llama/llama-4-scout:free".to_string(),
        }
    });

    // Send routing decision to frontend for pipeline visualization
    let _ = on_event.send(ChatEvent::Routing {
        task_type: task_type_str.clone(),
        selected_model: model.clone(),
        reason: format!("Classified as {} in {:.1}ms", task_type.description(), classification_ms),
        classification_ms,
    });
```

**Step 3: Update ChatEvent type in frontend**

In `src/lib/stores/chat.svelte.ts`, update the `ChatEvent` type:

```typescript
type ChatEvent =
	| { event: 'Started'; data: { model: string; task_type: string } }
	| { event: 'Delta'; data: { content: string } }
	| { event: 'Finished'; data: { total_tokens: number } }
	| { event: 'Error'; data: { message: string } }
	| { event: 'Routing'; data: { task_type: string; selected_model: string; reason: string; classification_ms: number } };
```

And add to the switch statement:

```typescript
			case 'Routing':
				modelStatus.lastRouting = {
					taskType: event.data.task_type,
					model: event.data.selected_model,
					reason: event.data.reason,
				};
				break;
```

**Step 4: Build and test**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo build 2>&1 | tail -5`
Expected: Build succeeds

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test 2>&1 | tail -5`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src-tauri/src/chat.rs src/lib/stores/chat.svelte.ts
git commit -m "feat(chat): add Routing event to ChatEvent for pipeline visualization"
```

---

## Phase 5: Convergence Mode

### Task 14: ChatConvergence Component

**Files:**
- Create: `src/lib/components/chat/ChatConvergence.svelte`

**Step 1: Create the convergence layout**

This component merges Chat + IDE + Terminal into one unified view using paneforge (already installed).

```svelte
<script lang="ts">
	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from './ChatMessage.svelte';
	import ChatInput from './ChatInput.svelte';
	import ModelPipelineView from './ModelPipelineView.svelte';
	import ModelActivityCard from './ModelActivityCard.svelte';
	import './hljs-forge.css';
	import { Bot, FolderTree, Terminal as TerminalIcon } from '@lucide/svelte';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let activeMessages = $derived(chatStore.messages);
	let vizLevel = $derived(getSetting('chatVizLevel'));

	function scrollToBottom() {
		if (messagesContainer) {
			requestAnimationFrame(() => {
				messagesContainer!.scrollTop = messagesContainer!.scrollHeight;
			});
		}
	}

	$effect(() => {
		if (activeMessages.length > 0) scrollToBottom();
	});

	async function handleSend(msg: string) {
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}
</script>

<div class="flex h-full">
	<!-- Left: File Explorer + Model Status -->
	<div class="w-[200px] border-r border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0">
		<div class="p-2 border-b border-gx-border-default">
			<div class="flex items-center gap-2 text-[11px] font-medium text-gx-text-muted uppercase tracking-wider">
				<FolderTree size={12} />
				Explorer
			</div>
		</div>
		<div class="flex-1 overflow-y-auto p-2 text-[11px] text-gx-text-muted">
			<p class="italic">Open from IDE to browse files</p>
		</div>

		<!-- Model Cards -->
		{#if vizLevel === 'cards' || vizLevel === 'pipeline'}
			<div class="border-t border-gx-border-default p-2 space-y-2">
				<div class="text-[10px] font-medium text-gx-text-muted uppercase">Models</div>
				{#each modelStatus.models as model (model.id)}
					<ModelActivityCard {model} />
				{/each}
			</div>
		{/if}
	</div>

	<!-- Center: Editor placeholder + Terminal -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Editor area (placeholder — connects to IDE store when files are open) -->
		<div class="flex-1 bg-gx-bg-primary flex items-center justify-center text-gx-text-muted text-sm border-b border-gx-border-default">
			<div class="text-center">
				<p class="text-xs">Editor — Connected to CodeForge IDE</p>
				<p class="text-[10px] mt-1">Open files from Explorer or use @file in chat</p>
			</div>
		</div>

		<!-- Terminal area -->
		<div class="h-[150px] bg-gx-bg-primary border-t border-gx-border-default p-2 overflow-y-auto">
			<div class="flex items-center gap-2 text-[11px] text-gx-text-muted mb-1">
				<TerminalIcon size={12} />
				Terminal
			</div>
			<div class="font-mono text-[11px] text-green-400/70">
				$ Ready for commands...
			</div>
		</div>
	</div>

	<!-- Right: Chat Stream -->
	<div class="w-[380px] border-l border-gx-border-default flex flex-col shrink-0">
		<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default bg-gx-bg-secondary">
			<Bot size={14} class="text-gx-neon" />
			<span class="text-xs font-medium text-gx-text-primary flex-1">
				{chatStore.activeConversation?.title ?? 'Chat'}
			</span>
		</div>

		<!-- Pipeline above messages -->
		{#if vizLevel === 'pipeline' && chatStore.isStreaming}
			<div class="px-3 py-2 border-b border-gx-border-default">
				<ModelPipelineView />
			</div>
		{/if}

		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-3 py-3 space-y-2">
			{#if activeMessages.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<Bot size={28} class="text-gx-neon opacity-50" />
					<p class="text-xs text-center">Chat + IDE + Terminal — unified workspace</p>
				</div>
			{:else}
				{#each activeMessages as msg (msg.id)}
					<ChatMessage message={msg} compact={true} />
				{/each}
			{/if}
		</div>

		<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} placeholder="Chat with your code..." />
	</div>
</div>
```

**Step 2: Add convergence route**

Create `src/routes/convergence/+page.svelte`:

```svelte
<script lang="ts">
	import ChatConvergence from '$lib/components/chat/ChatConvergence.svelte';
</script>

<ChatConvergence />
```

**Step 3: Commit**

```bash
git add src/lib/components/chat/ChatConvergence.svelte src/routes/convergence/+page.svelte
git commit -m "feat(chat): add Full Convergence mode — Chat + IDE + Terminal unified"
```

---

### Task 15: Add Convergence to Activity Bar

**Files:**
- Modify: `src/routes/+layout.svelte`

**Step 1: Add Convergence mode keyboard shortcut**

In the `handleKeydown` function, add:

```typescript
// Ctrl+Shift+F — Toggle convergence mode
if (e.ctrlKey && e.shiftKey && e.key === 'F') {
    e.preventDefault();
    goto('/convergence');
    return;
}
```

**Step 2: Add to activity bar if using convergence placement**

Find the activities array in the layout and add a Convergence entry when chatPlacement is 'convergence', or ensure navigation to `/convergence` works from the chat activity item.

**Step 3: Commit**

```bash
git add src/routes/+layout.svelte
git commit -m "feat(layout): add Ctrl+Shift+F shortcut for convergence mode"
```

---

## Phase 6: Component Index & Final Wiring

### Task 16: Chat Component Index

**Files:**
- Create: `src/lib/components/chat/index.ts`

**Step 1: Create barrel export**

```typescript
export { default as ChatRenderer } from './ChatRenderer.svelte';
export { default as ChatMessage } from './ChatMessage.svelte';
export { default as ChatInput } from './ChatInput.svelte';
export { default as ChatSidebar } from './ChatSidebar.svelte';
export { default as ChatSidePanel } from './ChatSidePanel.svelte';
export { default as ChatConvergence } from './ChatConvergence.svelte';
export { default as ModelStatusBadge } from './ModelStatusBadge.svelte';
export { default as ModelActivityCard } from './ModelActivityCard.svelte';
export { default as ModelPipelineView } from './ModelPipelineView.svelte';
export * from './types';
```

**Step 2: Commit**

```bash
git add src/lib/components/chat/index.ts
git commit -m "feat(chat): add component barrel export index"
```

---

### Task 17: Full Build & Test Verification

**Step 1: Run Rust tests**

Run: `cd /opt/ork-station/ImpForge/src-tauri && cargo test 2>&1 | tail -10`
Expected: All tests pass (841+)

**Step 2: Run svelte-check**

Run: `cd /opt/ork-station/ImpForge && pnpm exec svelte-check --threshold warning 2>&1 | tail -10`
Expected: No new errors (only 3 pre-existing in ThemeImporter.svelte)

**Step 3: Run frontend build**

Run: `cd /opt/ork-station/ImpForge && pnpm build 2>&1 | tail -10`
Expected: Build succeeds

**Step 4: Run Tauri dev to verify UI**

Run: `cd /opt/ork-station/ImpForge && pnpm tauri dev 2>&1 | head -20`
Expected: App launches, chat page loads with new components

**Step 5: Commit any remaining fixes**

```bash
git add -A
git commit -m "feat(chat): ChatTerminalUI 3x3x3 enterprise upgrade — complete"
```

---

## Summary

| Phase | Tasks | Components Created |
|-------|-------|--------------------|
| 1. Foundation | 1-4 | ChatRenderer, ChatMessage, types.ts, hljs-forge.css, settings extensions |
| 2. Stream Modes | 5-9 | ChatInput, ChatSidebar, ModelStatusBadge, ModelActivityCard, ModelPipelineView, upgraded /chat page, model-status store |
| 3. Placement Modes | 10-12 | ChatSidePanel, layout integration, settings UI |
| 4. Rust Backend | 13 | Extended ChatEvent with Routing |
| 5. Convergence | 14-15 | ChatConvergence, /convergence route |
| 6. Final | 16-17 | Index, build verification |

**Total new files:** ~18
**Total new LoC:** ~2,500-3,000
**Dependencies added:** marked, highlight.js, katex
**Existing code modified:** chat.svelte.ts, settings.svelte.ts, +layout.svelte, chat.rs, chat/+page.svelte
