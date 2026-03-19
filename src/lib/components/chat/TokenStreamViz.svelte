<script lang="ts">
	/**
	 * TokenStreamViz -- Real-time token streaming visualization
	 *
	 * Displays streaming progress: tokens/second, total generated,
	 * expanding progress bar, and an optional token-level view showing
	 * individual tokens with probability-based opacity.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - bar: Progress bar
	 *   - counter: Token counter area
	 */

	import { Activity, Zap } from '@lucide/svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		isStreaming?: boolean;
		tokensGenerated?: number;
		tokensPerSecond?: number;
		maxTokens?: number;
		recentTokens?: string[];
	}

	let {
		widgetId = 'token-stream-viz',
		isStreaming = false,
		tokensGenerated,
		tokensPerSecond,
		maxTokens = 4096,
		recentTokens = [],
	}: Props = $props();

	// Derive from model status store if not explicitly provided
	let effectiveTokens = $derived(() => {
		if (tokensGenerated !== undefined) return tokensGenerated;
		const active = modelStatus.activeModel;
		return active?.tokensGenerated ?? 0;
	});

	let effectiveTps = $derived(() => {
		if (tokensPerSecond !== undefined) return tokensPerSecond;
		const active = modelStatus.activeModel;
		if (!active || active.tokensGenerated === 0) return 0;
		const elapsed = (Date.now() - active.lastActive.getTime()) / 1000;
		return elapsed > 0.5 ? Math.round(active.tokensGenerated / elapsed) : 0;
	});

	let progressPercent = $derived(() => {
		const tokens = effectiveTokens();
		if (tokens <= 0 || maxTokens <= 0) return 0;
		return Math.min(100, (tokens / maxTokens) * 100);
	});

	// Progress bar color based on usage
	let barColor = $derived(() => {
		const pct = progressPercent();
		if (pct > 80) return 'var(--agent-error)';
		if (pct > 60) return 'var(--agent-waiting)';
		return 'var(--agent-working)';
	});

	// Elapsed time tracking
	let startTime = $state(Date.now());
	let elapsed = $state(0);
	let timerInterval: ReturnType<typeof setInterval> | undefined;

	$effect(() => {
		if (isStreaming) {
			startTime = Date.now();
			timerInterval = setInterval(() => {
				elapsed = Math.round((Date.now() - startTime) / 100) / 10;
			}, 100);
		} else {
			if (timerInterval) clearInterval(timerInterval);
		}
		return () => {
			if (timerInterval) clearInterval(timerInterval);
		};
	});

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let barComponent = $derived(styleEngine.getComponentStyle(widgetId, 'bar'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let barStyle = $derived(
		hasEngineStyle && barComponent ? componentToCSS(barComponent) : ''
	);
</script>

{#if isStreaming || effectiveTokens() > 0}
	<div
		class="flex flex-col gap-1.5 px-3 py-2 {hasEngineStyle ? '' : 'glass-panel-subtle'}"
		style={containerStyle}
	>
		<!-- Metrics row -->
		<div class="flex items-center gap-3 text-[10px]">
			<!-- Tokens generated -->
			<div class="flex items-center gap-1">
				<Activity size={10} style="color: {barColor()};" />
				<span class="tabular-nums font-mono" style="color: {barColor()};">
					{effectiveTokens().toLocaleString()}
				</span>
				<span class="text-gx-text-disabled">tokens</span>
			</div>

			<!-- Tokens per second -->
			{#if effectiveTps() > 0}
				<div class="flex items-center gap-1">
					<Zap size={9} class="text-gx-text-muted" />
					<span class="tabular-nums font-mono text-gx-text-secondary">
						{effectiveTps()}
					</span>
					<span class="text-gx-text-disabled">tok/s</span>
				</div>
			{/if}

			<!-- Elapsed time -->
			{#if isStreaming && elapsed > 0}
				<span class="text-gx-text-disabled tabular-nums font-mono ml-auto">
					{elapsed.toFixed(1)}s
				</span>
			{/if}
		</div>

		<!-- Progress bar -->
		<div class="relative w-full h-1.5 rounded-full overflow-hidden bg-gx-bg-tertiary" style={barStyle}>
			<div
				class="h-full rounded-full transition-all duration-300 relative"
				style="
					width: {progressPercent()}%;
					background: {barColor()};
					{isStreaming ? 'transform-origin: left;' : ''}
				"
			>
				<!-- Shimmer effect on active bar -->
				{#if isStreaming}
					<div
						class="absolute inset-0 gx-bar-spark rounded-full"
						style="opacity: 0.5;"
					></div>
				{/if}
			</div>
		</div>

		<!-- Recent tokens visualization (optional) -->
		{#if recentTokens.length > 0}
			<div class="flex flex-wrap gap-px mt-0.5 max-h-[40px] overflow-hidden">
				{#each recentTokens.slice(-30) as token, i (i)}
					{@const opacity = 0.3 + Math.random() * 0.7}
					<span
						class="text-[9px] font-mono px-0.5 rounded animate-token-fade-in"
						style="
							color: var(--agent-working);
							opacity: {opacity};
							background: color-mix(in srgb, var(--agent-working) {Math.round(opacity * 8)}%, transparent);
						"
					>{token}</span>
				{/each}
			</div>
		{/if}
	</div>
{/if}
