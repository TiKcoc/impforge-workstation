<script lang="ts">
	/**
	 * TokenBudgetBar — BenikUI-integrated context budget visualization
	 *
	 * Uses StyledBar when style engine has a bar definition for this widget,
	 * otherwise falls back to the built-in segmented/simple bar.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - bar: The progress bar (BarStyle used by StyledBar)
	 *   - label: Token count text
	 */

	import { Gauge } from '@lucide/svelte';
	import StyledWidget from '$lib/components/styled/StyledWidget.svelte';
	import StyledBar from '$lib/components/styled/StyledBar.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		usedTokens?: number;
		maxTokens?: number;
		breakdown?: { label: string; tokens: number; color: string }[];
	}

	let {
		widgetId = 'chat-token-budget',
		usedTokens = 0,
		maxTokens = 128000,
		breakdown = []
	}: Props = $props();

	let percentage = $derived(maxTokens > 0 ? Math.round((usedTokens / maxTokens) * 100) : 0);
	let normalizedValue = $derived(maxTokens > 0 ? usedTokens / maxTokens : 0);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let barComponent = $derived(styleEngine.getComponentStyle(widgetId, 'bar'));
	let labelComponent = $derived(styleEngine.getComponentStyle(widgetId, 'label'));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));

	let barColorClass = $derived(
		percentage > 85 ? 'token-budget-red'
		: percentage > 60 ? 'token-budget-amber'
		: 'token-budget-green'
	);

	function formatTokens(n: number): string {
		if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
		return `${n}`;
	}

	let showBreakdown = $state(false);

	let containerStyle = $derived(
		containerComponent ? componentToCSS(containerComponent) : ''
	);
	let labelStyle = $derived(
		labelComponent?.text
			? `color: ${labelComponent.text.color}; font-size: ${labelComponent.text.font_size}px;`
			: ''
	);
</script>

{#if usedTokens > 0}
	<div
		class="flex items-center gap-2 px-3 py-1.5 {hasEngineStyle ? '' : 'glass-panel-subtle'}"
		style={containerStyle}
		role="meter"
		aria-valuenow={usedTokens}
		aria-valuemin={0}
		aria-valuemax={maxTokens}
		aria-label="Token budget usage"
	>
		<Gauge size={12} class="text-gx-text-muted shrink-0" />

		<!-- Bar -->
		{#if hasEngineStyle && barComponent?.bar}
			<!-- StyledBar from style engine -->
			<div class="flex-1">
				<StyledBar
					bar={barComponent.bar}
					text={barComponent.text}
					value={normalizedValue}
				/>
			</div>
		{:else}
			<!-- Fallback: built-in bar -->
			<button
				class="flex-1 h-2 bg-gx-bg-primary rounded-full overflow-hidden relative cursor-pointer"
				onclick={() => showBreakdown = !showBreakdown}
				aria-label="Toggle token breakdown"
			>
				{#if breakdown.length > 0}
					<div class="absolute inset-0 flex">
						{#each breakdown as seg (seg.label)}
							{@const segPct = maxTokens > 0 ? (seg.tokens / maxTokens) * 100 : 0}
							<div
								class="h-full transition-all duration-300"
								style="width: {segPct}%; background: {seg.color};"
								title="{seg.label}: {formatTokens(seg.tokens)}"
							></div>
						{/each}
					</div>
				{:else}
					<div
						class="h-full {barColorClass} rounded-full transition-all duration-500 relative gx-bar-spark"
						style="width: {percentage}%;"
					></div>
				{/if}
			</button>
		{/if}

		<!-- Label -->
		<span
			class="text-[10px] text-gx-text-muted tabular-nums whitespace-nowrap shrink-0"
			style={labelStyle}
		>
			{percentage}% · {formatTokens(usedTokens)} / {formatTokens(maxTokens)}
		</span>
	</div>

	<!-- Breakdown tooltip -->
	{#if showBreakdown && breakdown.length > 0}
		<div class="mx-3 mt-1 mb-2 glass-panel-subtle p-2 space-y-1">
			{#each breakdown as seg (seg.label)}
				<div class="flex items-center justify-between text-[10px]">
					<div class="flex items-center gap-1.5">
						<span class="w-2 h-2 rounded-full shrink-0" style="background: {seg.color};"></span>
						<span class="text-gx-text-secondary">{seg.label}</span>
					</div>
					<span class="text-gx-text-muted tabular-nums">{formatTokens(seg.tokens)}</span>
				</div>
			{/each}
		</div>
	{/if}
{/if}
