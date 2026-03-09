<script lang="ts">
	/**
	 * ModelActivityCard — BenikUI-integrated model status card
	 *
	 * Displays model activity with per-model accent colors, progress bar,
	 * token metrics, and routing info. Uses StyledBar when style engine
	 * styles are loaded, otherwise falls back to built-in bar.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Card wrapper
	 *   - bar: Progress bar (BarStyle used by StyledBar)
	 *   - header: Model name row
	 *   - metrics: Token/latency text
	 */

	import type { ModelState } from '$lib/stores/model-status.svelte';
	import { Cpu, Zap, Loader2 } from '@lucide/svelte';
	import StyledBar from '$lib/components/styled/StyledBar.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		model: ModelState;
	}

	let { widgetId = 'chat-model-card', model }: Props = $props();

	let progress = $derived(
		model.tokensTotal ? Math.round((model.tokensGenerated / model.tokensTotal) * 100) : null
	);

	let normalizedProgress = $derived(
		model.tokensTotal ? model.tokensGenerated / model.tokensTotal : 0.5
	);

	let statusLabel = $derived({
		idle: 'Standby',
		thinking: 'Thinking...',
		generating: 'Generating',
		error: 'Error',
	}[model.status]);

	/** Per-model accent color from model name */
	function getModelAccent(name: string): string {
		const n = name.toLowerCase();
		if (n.includes('claude') || n.includes('anthropic')) return '#a855f7';
		if (n.includes('qwen')) return '#06b6d4';
		if (n.includes('hermes')) return '#f59e0b';
		if (n.includes('ollama') || n.includes('local') || n.includes('llama') || n.includes('dolphin')) return '#22c55e';
		return '#06b6d4';
	}

	let accent = $derived(getModelAccent(model.name));

	let glowStyle = $derived({
		idle: '',
		thinking: `box-shadow: 0 0 15px ${accent}30;`,
		generating: `box-shadow: 0 0 15px ${accent}30;`,
		error: 'box-shadow: 0 0 15px rgba(239,68,68,0.3);',
	}[model.status]);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let barComponent = $derived(styleEngine.getComponentStyle(widgetId, 'bar'));
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let metricsComponent = $derived(styleEngine.getComponentStyle(widgetId, 'metrics'));

	let containerStyle = $derived(() => {
		if (hasEngineStyle && containerComponent) {
			return `${componentToCSS(containerComponent)} ${glowStyle}`;
		}
		return `${glowStyle} border-color: ${accent}20;`;
	});

	let headerStyle = $derived(
		hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : ''
	);

	let metricsStyle = $derived(
		hasEngineStyle && metricsComponent ? componentToCSS(metricsComponent) : ''
	);
</script>

<div
	class="p-3 rounded-lg transition-shadow duration-500 {hasEngineStyle ? '' : 'bg-gx-bg-elevated border border-gx-border-default'}"
	style={containerStyle()}
>
	<div class="flex items-center gap-2 mb-2" style={headerStyle}>
		{#if model.status === 'generating'}
			<Zap size={14} style="color: {accent};" />
		{:else if model.status === 'thinking'}
			<Loader2 size={14} style="color: {accent};" class="animate-spin" />
		{:else}
			<Cpu size={14} class="text-gx-text-muted" />
		{/if}
		<span class="text-xs font-medium text-gx-text-primary truncate">{model.name.split('/').pop()}</span>
	</div>

	<!-- Progress bar -->
	{#if hasEngineStyle && barComponent?.bar}
		<div class="mb-2">
			<StyledBar
				bar={barComponent.bar}
				text={barComponent.text}
				value={normalizedProgress}
			/>
		</div>
	{:else}
		<div class="w-full h-1.5 bg-gx-bg-primary rounded-full mb-2 overflow-hidden">
			{#if model.status === 'generating'}
				<div
					class="h-full rounded-full transition-all duration-300"
					style="width: {progress ?? 50}%; background: linear-gradient(90deg, {accent}, {accent}cc);"
				></div>
			{:else if model.status === 'thinking'}
				<div class="h-full rounded-full animate-pulse w-full opacity-30" style="background: {accent};"></div>
			{/if}
		</div>
	{/if}

	<div class="flex justify-between text-[10px] text-gx-text-muted" style={metricsStyle}>
		<span>{statusLabel}</span>
		{#if model.currentTask}
			<span style="color: {accent};">{model.currentTask}</span>
		{/if}
	</div>

	{#if model.status === 'generating'}
		<div class="flex justify-between text-[10px] text-gx-text-muted mt-1" style={metricsStyle}>
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
