<script lang="ts">
	/**
	 * ModelStatusBadge — BenikUI-integrated model status indicator
	 *
	 * Inline badge showing active model name and status with per-model
	 * accent colors. Uses style engine for customizable appearance.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Badge wrapper
	 *   - dot: Status indicator dot
	 */

	import { modelStatus } from '$lib/stores/model-status.svelte';
	import type { ModelState } from '$lib/stores/model-status.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		model?: ModelState;
	}

	let { widgetId = 'chat-model-badge', model }: Props = $props();

	let displayModel = $derived(model ?? modelStatus.activeModel);

	/** Per-model accent color from model name */
	function getModelAccent(name: string): string {
		const n = name.toLowerCase();
		if (n.includes('claude') || n.includes('anthropic')) return '#a855f7';
		if (n.includes('qwen')) return '#06b6d4';
		if (n.includes('hermes')) return '#f59e0b';
		if (n.includes('ollama') || n.includes('local') || n.includes('llama') || n.includes('dolphin')) return '#22c55e';
		return '#06b6d4';
	}

	let accent = $derived(displayModel ? getModelAccent(displayModel.name) : '#606070');

	let dotStyle = $derived(() => {
		if (!displayModel) return '';
		switch (displayModel.status) {
			case 'idle': return `background: #9ca3af;`;
			case 'thinking': return `background: ${accent}; animation: pulse 2s infinite;`;
			case 'generating': return `background: ${accent}; animation: pulse 1.5s infinite;`;
			case 'error': return `background: #ef4444;`;
			default: return `background: #9ca3af;`;
		}
	});

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
</script>

{#if displayModel}
	<div
		class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] {hasEngineStyle ? '' : 'bg-gx-bg-elevated border border-gx-border-default'}"
		style="{containerStyle} border-color: {accent}20;"
	>
		<span class="w-2 h-2 rounded-full shrink-0" style={dotStyle()}></span>
		<span class="font-mono text-gx-text-secondary truncate max-w-[120px]" style="color: {accent}cc;">
			{displayModel.name.split('/').pop()}
		</span>
		{#if displayModel.status === 'generating'}
			<span class="text-gx-text-muted tabular-nums">{displayModel.tokensGenerated}tk</span>
		{/if}
	</div>
{/if}
