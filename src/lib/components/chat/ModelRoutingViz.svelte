<script lang="ts">
	/**
	 * ModelRoutingViz -- Horizontal pipeline showing the model routing cascade
	 *
	 * Displays the three-tier routing architecture:
	 *   Tier 0: Local (Ollama) -> Tier 1: Free Cloud (OpenRouter) -> Tier 2: Paid Cloud
	 *
	 * The active tier has a pulsing neon glow with tokens/second counter.
	 * Cascade arrows animate when escalating between tiers.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - tier: Individual tier node
	 *   - arrow: Cascade arrow connector
	 */

	import { Cpu, Cloud, Zap, ArrowRight } from '@lucide/svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		activeTier?: 0 | 1 | 2 | null;
		activeModel?: string;
		tokensPerSecond?: number;
		isStreaming?: boolean;
	}

	let {
		widgetId = 'model-routing-viz',
		activeTier = null,
		activeModel,
		tokensPerSecond = 0,
		isStreaming = false,
	}: Props = $props();

	// Derive active tier from model name if not explicitly set
	let effectiveTier = $derived(() => {
		if (activeTier !== null) return activeTier;
		const active = modelStatus.activeModel;
		if (!active) return null;
		const name = active.name.toLowerCase();
		if (name.includes('ollama:') || name.includes('local')) return 0;
		if (name.includes(':free')) return 1;
		if (name.includes('openrouter:')) return 2;
		return 0;
	});

	let effectiveModel = $derived(() => {
		if (activeModel) return activeModel;
		const active = modelStatus.activeModel;
		if (!active) return '';
		return active.name.replace(/^(ollama|openrouter):/i, '').replace(/:free$/i, '');
	});

	let effectiveTps = $derived(() => {
		if (tokensPerSecond > 0) return tokensPerSecond;
		const active = modelStatus.activeModel;
		if (!active || active.latencyMs === 0) return 0;
		// Rough estimate: tokens generated / elapsed seconds
		if (active.tokensGenerated > 0) {
			const elapsed = (Date.now() - active.lastActive.getTime()) / 1000;
			return elapsed > 0 ? Math.round(active.tokensGenerated / elapsed) : 0;
		}
		return 0;
	});

	interface TierConfig {
		tier: 0 | 1 | 2;
		label: string;
		sublabel: string;
		icon: typeof Cpu;
		color: string;
		cssVar: string;
	}

	const tiers: TierConfig[] = [
		{ tier: 0, label: 'Local', sublabel: 'Ollama', icon: Cpu, color: 'var(--ai-local)', cssVar: '--ai-local' },
		{ tier: 1, label: 'Cloud Free', sublabel: 'OpenRouter', icon: Cloud, color: 'var(--ai-cloud-free)', cssVar: '--ai-cloud-free' },
		{ tier: 2, label: 'Cloud Pro', sublabel: 'Paid', icon: Zap, color: 'var(--ai-cloud-paid)', cssVar: '--ai-cloud-paid' },
	];

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
</script>

<div
	class="flex items-center gap-1 py-2 px-1 overflow-x-auto {hasEngineStyle ? '' : ''}"
	style={containerStyle}
>
	{#each tiers as tier, i (tier.tier)}
		{@const isActive = effectiveTier() === tier.tier && isStreaming}
		{@const isPast = effectiveTier() !== null && tier.tier < (effectiveTier() ?? 0)}

		<!-- Tier node -->
		<div
			class="relative flex flex-col items-center gap-0.5 px-3 py-2 rounded-gx transition-all duration-300 shrink-0"
			style="
				background: {isActive ? `color-mix(in srgb, ${tier.color} 10%, var(--color-gx-bg-primary))` : 'var(--color-gx-bg-secondary)'};
				border: 1px solid {isActive ? `color-mix(in srgb, ${tier.color} 40%, transparent)` : isPast ? `color-mix(in srgb, ${tier.color} 15%, transparent)` : 'var(--color-gx-border-default)'};
				{isActive ? `box-shadow: 0 0 12px color-mix(in srgb, ${tier.color} 25%, transparent), 0 0 24px color-mix(in srgb, ${tier.color} 10%, transparent);` : ''}
				min-width: 80px;
			"
		>
			<!-- Active pulsing ring -->
			{#if isActive}
				<div
					class="absolute inset-0 rounded-gx animate-agent-pulse"
					style="
						box-shadow: 0 0 16px color-mix(in srgb, {tier.color} 30%, transparent);
						border: 1px solid color-mix(in srgb, {tier.color} 20%, transparent);
					"
				></div>
			{/if}

			<!-- Icon -->
			<tier.icon
				size={16}
				style="color: {isActive ? tier.color : isPast ? `color-mix(in srgb, ${tier.color} 50%, var(--color-gx-text-muted))` : 'var(--color-gx-text-muted)'}; transition: color 0.3s;"
			/>

			<!-- Label -->
			<span
				class="text-[10px] font-medium transition-colors duration-300"
				style="color: {isActive ? tier.color : 'var(--color-gx-text-muted)'};"
			>{tier.label}</span>

			<!-- Model name (active tier only) -->
			{#if isActive && effectiveModel()}
				<span
					class="text-[9px] font-mono truncate max-w-[90px]"
					style="color: {tier.color}; opacity: 0.8;"
				>{effectiveModel()}</span>
			{:else}
				<span class="text-[9px] text-gx-text-disabled">{tier.sublabel}</span>
			{/if}

			<!-- Tokens/second (active tier only) -->
			{#if isActive && effectiveTps() > 0}
				<span
					class="text-[8px] font-mono tabular-nums mt-0.5"
					style="color: {tier.color}; opacity: 0.7;"
				>{effectiveTps()} tok/s</span>
			{/if}

			<!-- Status dot -->
			<div
				class="absolute -top-1 -right-1 w-2.5 h-2.5 rounded-full border"
				style="
					background: {isActive ? tier.color : isPast ? `color-mix(in srgb, ${tier.color} 40%, transparent)` : 'var(--color-gx-bg-tertiary)'};
					border-color: {isActive ? tier.color : 'var(--color-gx-border-default)'};
					{isActive ? `box-shadow: 0 0 6px ${tier.color};` : ''}
				"
			></div>
		</div>

		<!-- Cascade arrow between tiers -->
		{#if i < tiers.length - 1}
			{@const isEscalating = effectiveTier() !== null && tier.tier < (effectiveTier() ?? 0)}
			<div class="flex items-center shrink-0 px-0.5">
				<div class="flex items-center gap-px">
					<!-- Dashes -->
					<div
						class="w-3 h-px"
						style="background: {isEscalating ? tiers[i + 1].color : 'var(--color-gx-border-default)'}; opacity: {isEscalating ? 0.6 : 0.3};"
					></div>
					<!-- Arrow -->
					<ArrowRight
						size={10}
						class={isEscalating ? 'animate-cascade-flow' : ''}
						style="color: {isEscalating ? tiers[i + 1].color : 'var(--color-gx-text-disabled)'}; opacity: {isEscalating ? 0.8 : 0.3};"
					/>
				</div>
			</div>
		{/if}
	{/each}
</div>
