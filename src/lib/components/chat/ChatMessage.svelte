<script lang="ts">
	/**
	 * ChatMessage — BenikUI-integrated chat message bubble
	 *
	 * Renders user/assistant messages with per-model branded accents,
	 * reasoning blocks, glass panel surfaces, and style engine integration.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Message bubble wrapper
	 *   - footer: Message footer (timestamp, model badge)
	 *   - model-tag: Model name tag in footer
	 */

	import ChatRenderer from './ChatRenderer.svelte';
	import ModelAvatar from './ModelAvatar.svelte';
	import ReasoningBlock from './ReasoningBlock.svelte';
	import { parseBlocks } from './types';
	import {
		User, Copy, Check,
		Cpu, Cloud, Zap, Layers,
		AlertCircle, Wrench, Loader2
	} from '@lucide/svelte';
	import { license } from '$lib/stores/license.svelte';
	import type { Message } from '$lib/stores/chat.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		message: Message;
		compact?: boolean;
	}

	let { widgetId = 'chat-message', message, compact = false }: Props = $props();

	let copied = $state(false);

	let blocks = $derived(parseBlocks(message.content));

	function getCascadeInfo(model?: string) {
		if (!model) return { tier: 0, tierLabel: 'Tier 0', shortModel: 'Unknown', cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers };
		const m = model.toLowerCase();
		if (m.includes('ollama:') || m.includes('local')) {
			const shortModel = model.replace(/^(Ollama|Local[^:]*):?\s*/i, '').trim() || model;
			return { tier: 0, tierLabel: 'Tier 0 - Local', shortModel, cost: 'Free', colorClass: 'text-gx-model-local', icon: Cpu };
		}
		if (m.includes(':free')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').replace(/:free$/i, '').trim();
			return { tier: 1, tierLabel: 'Tier 1 - Cloud Free', shortModel, cost: 'Free', colorClass: 'text-gx-model-qwen', icon: Cloud };
		}
		if (m.includes('openrouter:')) {
			const shortModel = model.replace(/^OpenRouter:\s*/i, '').trim();
			return { tier: 2, tierLabel: 'Tier 2 - Cloud Pro', shortModel, cost: 'Paid', colorClass: 'text-gx-model-hermes', icon: Zap };
		}
		return { tier: 0, tierLabel: 'Tier 0', shortModel: model, cost: 'Free', colorClass: 'text-gx-text-muted', icon: Layers };
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

	function copyToClipboard() {
		navigator.clipboard.writeText(message.content);
		copied = true;
		setTimeout(() => { copied = false; }, 2000);
	}

	function formatTime(date: Date): string {
		return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}

	let cascade = $derived(getCascadeInfo(message.model));
	let accentColor = $derived(getModelAccentColor(message.model));

	// Style engine integration — separate widget IDs for user vs assistant
	let effectiveWidgetId = $derived(
		message.role === 'user' ? `${widgetId}-user` : `${widgetId}-assistant`
	);
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(effectiveWidgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(effectiveWidgetId, 'container'));
	let footerComponent = $derived(styleEngine.getComponentStyle(effectiveWidgetId, 'footer'));
	let modelTagComponent = $derived(styleEngine.getComponentStyle(effectiveWidgetId, 'model-tag'));

	let containerStyle = $derived(() => {
		if (hasEngineStyle && containerComponent) {
			const css = componentToCSS(containerComponent);
			// Still apply model accent as border tint for assistant messages
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

	let modelTagStyle = $derived(() => {
		if (hasEngineStyle && modelTagComponent) {
			return `${componentToCSS(modelTagComponent)} color: ${accentColor}; border-color: ${accentColor}30;`;
		}
		return `color: ${accentColor}; border-color: ${accentColor}30;`;
	});
</script>

<div class="flex gap-3 {message.role === 'user' ? 'justify-end' : ''} {compact ? 'py-1' : 'py-2'}">
	<!-- Assistant avatar — branded per model -->
	{#if message.role === 'assistant'}
		<div class="mt-0.5">
			<ModelAvatar
				modelName={message.model}
				streaming={message.streaming}
				size={compact ? 28 : 32}
			/>
		</div>
	{/if}

	<!-- Message bubble — glass surface or style engine -->
	<div class="max-w-[80%] min-w-0 {hasEngineStyle ? '' : message.role === 'user'
		? 'glass-panel-subtle'
		: 'glass-panel'} px-4 py-3"
		style={containerStyle()}
	>
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
			{#if message.role === 'assistant' && message.model}
				<span
					class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded {hasEngineStyle ? '' : 'glass-panel-subtle'} text-[9px] font-mono"
					style={modelTagStyle()}
				>
					<cascade.icon size={9} />
					{#if license.isPro}
						{cascade.shortModel}
					{:else}
						{cascade.tier === 0 ? 'Local' : cascade.shortModel}
					{/if}
				</span>
				{#if license.isPro}
					<span
						class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded {hasEngineStyle ? '' : 'glass-panel-subtle'} text-[9px]"
						style="color: {accentColor}; border-color: {accentColor}20;"
					>
						{cascade.tierLabel}
					</span>
				{/if}
			{/if}
			{#if message.taskType}
				<span class="px-1.5 py-0.5 rounded {hasEngineStyle ? '' : 'glass-panel-subtle'} text-[9px]" style="color: {accentColor};">{message.taskType}</span>
			{/if}
			{#if message.role === 'assistant' && !message.streaming}
				<button onclick={copyToClipboard} class="ml-auto hover:text-gx-neon transition-colors p-0.5" aria-label="Copy message">
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
