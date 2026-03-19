<script lang="ts">
	/**
	 * EnhancedChatMessage -- HyperChat upgraded message component
	 *
	 * Extends the base ChatMessage with:
	 * - AgentCharacter avatar next to AI messages
	 * - Model badge with cascade tier info
	 * - Collapsible reasoning blocks
	 * - Code blocks with syntax highlighting + copy button (via ChatRenderer)
	 * - Inline diff view (green=added, red=removed)
	 * - Timestamp + token count footer
	 * - Reaction buttons
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Message bubble wrapper
	 *   - footer: Footer area
	 *   - reactions: Reaction button row
	 *   - diff-added: Added diff line
	 *   - diff-removed: Removed diff line
	 */

	import ChatRenderer from './ChatRenderer.svelte';
	import ReasoningBlock from './ReasoningBlock.svelte';
	import AgentCharacter from './AgentCharacter.svelte';
	import { parseBlocks } from './types';
	import {
		User, Copy, Check, ThumbsUp, ThumbsDown,
		RotateCcw, Cpu, Cloud, Zap, Layers,
		AlertCircle, Wrench, Loader2
	} from '@lucide/svelte';
	import { license } from '$lib/stores/license.svelte';
	import type { Message } from '$lib/stores/chat.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		message: Message;
		compact?: boolean;
		showReactions?: boolean;
		agentState?: 'idle' | 'thinking' | 'working' | 'success' | 'error' | 'waiting' | 'sleeping';
	}

	let {
		widgetId = 'enhanced-chat-message',
		message,
		compact = false,
		showReactions = true,
		agentState = 'idle',
	}: Props = $props();

	let copied = $state(false);
	let reaction = $state<'up' | 'down' | null>(null);

	let blocks = $derived(parseBlocks(message.content));

	// --- Cascade tier resolution ---
	function getCascadeInfo(model?: string) {
		if (!model) return { tier: 0, tierLabel: 'Tier 0', shortModel: 'Unknown', cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers, color: '#606070' };
		const m = model.toLowerCase();
		if (m.includes('ollama:') || m.includes('local')) {
			const shortModel = model.replace(/^(Ollama|Local[^:]*):?\s*/i, '').trim() || model;
			return { tier: 0, tierLabel: 'Local', shortModel, cost: 'Free', colorClass: 'text-gx-model-local', icon: Cpu, color: 'var(--ai-local)' };
		}
		if (m.includes(':free')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').replace(/:free$/i, '').trim();
			return { tier: 1, tierLabel: 'Cloud Free', shortModel, cost: 'Free', colorClass: 'text-gx-model-qwen', icon: Cloud, color: 'var(--ai-cloud-free)' };
		}
		if (m.includes('openrouter:')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').trim();
			return { tier: 2, tierLabel: 'Cloud Pro', shortModel, cost: 'Paid', colorClass: 'text-gx-model-hermes', icon: Zap, color: 'var(--ai-cloud-paid)' };
		}
		return { tier: 0, tierLabel: 'Tier 0', shortModel: model, cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers, color: '#606070' };
	}

	function getModelAccentColor(model?: string): string {
		if (!model) return '#606070';
		const m = model.toLowerCase();
		if (m.includes('claude') || m.includes('anthropic')) return '#a855f7';
		if (m.includes('qwen')) return '#06b6d4';
		if (m.includes('hermes')) return '#f59e0b';
		if (m.includes('ollama') || m.includes('local') || m.includes('llama') || m.includes('dolphin')) return '#22c55e';
		return '#06b6d4';
	}

	function getAgentName(model?: string): string {
		if (!model) return 'AI';
		const m = model.toLowerCase();
		if (m.includes('claude')) return 'Claude';
		if (m.includes('qwen')) return 'Qwen';
		if (m.includes('hermes')) return 'Hermes';
		if (m.includes('llama')) return 'Llama';
		if (m.includes('mistral') || m.includes('devstral')) return 'Mistral';
		if (m.includes('dolphin')) return 'Dolphin';
		// Extract short name from model ID
		const parts = model.split(/[:/]/);
		return parts[parts.length - 1].replace(/:latest$/, '').slice(0, 12);
	}

	function copyToClipboard() {
		navigator.clipboard.writeText(message.content);
		copied = true;
		setTimeout(() => { copied = false; }, 2000);
	}

	function formatTime(date: Date): string {
		return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}

	// --- Diff parsing ---
	interface DiffLine {
		type: 'added' | 'removed' | 'context';
		content: string;
	}

	function parseDiffLines(content: string): DiffLine[] | null {
		const lines = content.split('\n');
		// Only parse as diff if it looks like a unified diff
		const hasDiffMarkers = lines.some(l => l.startsWith('@@') || l.startsWith('---') || l.startsWith('+++'));
		if (!hasDiffMarkers) return null;

		return lines
			.filter(l => !l.startsWith('---') && !l.startsWith('+++') && !l.startsWith('@@'))
			.map(line => {
				if (line.startsWith('+')) return { type: 'added' as const, content: line.slice(1) };
				if (line.startsWith('-')) return { type: 'removed' as const, content: line.slice(1) };
				return { type: 'context' as const, content: line.startsWith(' ') ? line.slice(1) : line };
			});
	}

	let cascade = $derived(getCascadeInfo(message.model));
	let accentColor = $derived(getModelAccentColor(message.model));
	let agentName = $derived(getAgentName(message.model));

	// Derive effective agent state from message streaming state
	type AgentStateValue = 'idle' | 'thinking' | 'working' | 'success' | 'error' | 'waiting' | 'sleeping';
	let effectiveAgentState = $derived((): AgentStateValue => {
		if (message.streaming && message.content === '') return 'thinking';
		if (message.streaming) return 'working';
		if (message.content.startsWith('Error:')) return 'error';
		if (!message.streaming && message.role === 'assistant' && message.content) return 'success';
		return agentState;
	});

	// Token count estimate (rough: ~4 chars per token)
	let estimatedTokens = $derived(Math.ceil(message.content.length / 4));

	// Style engine integration
	let effectiveWidgetId = $derived(
		message.role === 'user' ? `${widgetId}-user` : `${widgetId}-assistant`
	);
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(effectiveWidgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(effectiveWidgetId, 'container'));
	let footerComponent = $derived(styleEngine.getComponentStyle(effectiveWidgetId, 'footer'));

	let containerStyle = $derived(() => {
		if (hasEngineStyle && containerComponent) {
			const css = componentToCSS(containerComponent);
			if (message.role === 'assistant' && message.model) {
				return `${css} border-color: ${accentColor}15;`;
			}
			return css;
		}
		if (message.role === 'assistant' && message.model) {
			return `border-color: ${accentColor}15;`;
		}
		return '';
	});

	let footerStyle = $derived(
		hasEngineStyle && footerComponent ? componentToCSS(footerComponent) : ''
	);
</script>

<div class="flex gap-3 {message.role === 'user' ? 'justify-end' : ''} {compact ? 'py-1' : 'py-2'}">
	<!-- Agent character avatar (assistant only) -->
	{#if message.role === 'assistant'}
		<div class="mt-0.5 shrink-0">
			<AgentCharacter
				agentName={agentName}
				state={effectiveAgentState()}
				model={message.model}
				progress={message.streaming ? undefined : 100}
				size={compact ? 'sm' : 'md'}
			/>
		</div>
	{/if}

	<!-- Message bubble -->
	<div
		class="max-w-[80%] min-w-0 {hasEngineStyle ? '' : message.role === 'user'
			? 'glass-panel-subtle'
			: 'glass-panel'} px-4 py-3"
		style={containerStyle()}
	>
		<!-- Model badge header (assistant only) -->
		{#if message.role === 'assistant' && message.model && !compact}
			<div class="flex items-center gap-2 mb-2 pb-1.5 border-b border-gx-border-default/30">
				<span
					class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded glass-panel-subtle text-[9px] font-mono"
					style="color: {accentColor}; border-color: {accentColor}30;"
				>
					<cascade.icon size={9} />
					{cascade.shortModel}
				</span>
				{#if license.isPro}
					<span
						class="text-[8px] px-1 py-0.5 rounded glass-panel-subtle"
						style="color: {cascade.color};"
					>{cascade.tierLabel}</span>
				{/if}
				{#if message.taskType}
					<span class="text-[8px] px-1 py-0.5 rounded glass-panel-subtle" style="color: {accentColor};">{message.taskType}</span>
				{/if}
			</div>
		{/if}

		<!-- Blocks rendering -->
		<div class="text-sm leading-relaxed">
			{#each blocks as block (block.id)}
				{#if block.type === 'thinking'}
					<ReasoningBlock
						content={block.content}
						modelName={message.model}
					/>
				{:else if block.type === 'tool'}
					<div class="my-2 p-2.5 rounded-gx bg-gx-bg-primary border border-gx-border-default">
						<div class="flex items-center gap-2 text-xs text-gx-text-muted mb-1">
							{#if block.meta?.status === 'running'}
								<Loader2 size={12} class="animate-spin text-gx-neon" />
							{:else}
								<Wrench size={12} class="text-gx-accent" />
							{/if}
							<span class="font-mono text-gx-accent">{block.meta?.tool ?? 'tool'}</span>
							{#if block.meta?.status}
								<span class="text-[10px] text-gx-text-muted">({block.meta.status})</span>
							{/if}
						</div>
						<pre class="text-[11px] text-gx-text-secondary overflow-x-auto whitespace-pre-wrap">{block.content}</pre>
					</div>
				{:else if block.type === 'error'}
					<div class="my-2 p-2.5 rounded-gx bg-gx-status-error/5 border border-gx-status-error/20">
						<div class="flex items-center gap-2 text-xs text-gx-status-error mb-1">
							<AlertCircle size={12} />
							<span class="font-medium">Error</span>
						</div>
						<p class="text-xs text-gx-status-error/80">{block.content}</p>
					</div>
				{:else if block.type === 'system'}
					<div class="my-1 text-[11px] text-gx-text-muted italic">{block.content}</div>
				{:else if block.type === 'diff'}
					<!-- Inline diff view -->
					{@const diffLines = parseDiffLines(block.content)}
					{#if diffLines}
						<div class="my-2 rounded-gx overflow-hidden border border-gx-border-default font-mono text-[11px]">
							{#each diffLines as line (line)}
								<div
									class="px-3 py-0.5"
									style="
										background: {line.type === 'added' ? 'rgba(34, 197, 94, 0.08)' : line.type === 'removed' ? 'rgba(239, 68, 68, 0.08)' : 'transparent'};
										color: {line.type === 'added' ? '#4ade80' : line.type === 'removed' ? '#f87171' : 'var(--color-gx-text-secondary)'};
										border-left: 3px solid {line.type === 'added' ? '#22c55e' : line.type === 'removed' ? '#ef4444' : 'transparent'};
									"
								>
									<span class="text-gx-text-disabled select-none mr-2">{line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' '}</span>{line.content}
								</div>
							{/each}
						</div>
					{:else}
						<ChatRenderer content={block.content} streaming={message.streaming && block === blocks[blocks.length - 1]} />
					{/if}
				{:else if block.type === 'chat'}
					<ChatRenderer content={block.content} streaming={message.streaming && block === blocks[blocks.length - 1]} />
				{/if}
			{/each}

			{#if message.streaming && message.content === ''}
				<span class="inline-flex gap-1 items-center text-gx-text-muted">
					<span class="w-1.5 h-1.5 rounded-full animate-bounce" style="background: {accentColor}; animation-delay: 0ms"></span>
					<span class="w-1.5 h-1.5 rounded-full animate-bounce" style="background: {accentColor}; animation-delay: 150ms"></span>
					<span class="w-1.5 h-1.5 rounded-full animate-bounce" style="background: {accentColor}; animation-delay: 300ms"></span>
				</span>
			{/if}
		</div>

		<!-- Message footer -->
		<div class="flex items-center gap-2 mt-2 text-[10px] text-gx-text-muted" style={footerStyle}>
			<span>{formatTime(message.timestamp)}</span>

			<!-- Token count for assistant messages -->
			{#if message.role === 'assistant' && !message.streaming && estimatedTokens > 0}
				<span class="tabular-nums font-mono text-gx-text-disabled">~{estimatedTokens} tokens</span>
			{/if}

			{#if message.role === 'assistant' && message.model && compact}
				<span
					class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded glass-panel-subtle text-[9px] font-mono"
					style="color: {accentColor}; border-color: {accentColor}30;"
				>
					<cascade.icon size={9} />
					{cascade.shortModel}
				</span>
			{/if}

			<!-- Action buttons -->
			{#if message.role === 'assistant' && !message.streaming}
				<div class="ml-auto flex items-center gap-1">
					<!-- Reaction buttons -->
					{#if showReactions}
						<button
							onclick={() => reaction = reaction === 'up' ? null : 'up'}
							class="p-0.5 hover:text-gx-neon transition-colors"
							aria-label="Thumbs up"
							style="color: {reaction === 'up' ? 'var(--agent-success)' : ''};"
						>
							<ThumbsUp size={11} />
						</button>
						<button
							onclick={() => reaction = reaction === 'down' ? null : 'down'}
							class="p-0.5 hover:text-gx-status-error transition-colors"
							aria-label="Thumbs down"
							style="color: {reaction === 'down' ? 'var(--agent-error)' : ''};"
						>
							<ThumbsDown size={11} />
						</button>
					{/if}

					<!-- Copy button -->
					<button onclick={copyToClipboard} class="p-0.5 hover:text-gx-neon transition-colors" aria-label="Copy message">
						{#if copied}<Check size={11} class="text-gx-status-success" />{:else}<Copy size={11} />{/if}
					</button>

					<!-- Regenerate (placeholder) -->
					<button class="p-0.5 hover:text-gx-neon transition-colors" aria-label="Regenerate response">
						<RotateCcw size={11} />
					</button>
				</div>
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
