<script lang="ts">
	/**
	 * GlassPanel — BenikUI-integrated glass surface wrapper
	 *
	 * When a widgetId is provided and the style engine has styles loaded,
	 * uses StyledContainer for deep customization (background type, border
	 * pattern, glow type, animation, etc.). Otherwise falls back to the
	 * glass-panel CSS classes for zero-config rendering.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: The glass surface itself
	 */

	import StyledWidget from '$lib/components/styled/StyledWidget.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		/** Style engine widget ID for deep customization */
		widgetId?: string;
		variant?: 'default' | 'elevated' | 'subtle';
		/** Per-model accent color (hex) — applied as border/glow tint */
		modelAccent?: string | null;
		class?: string;
		children: Snippet;
	}

	let {
		widgetId,
		variant = 'default',
		modelAccent = null,
		class: className = '',
		children
	}: Props = $props();

	let panelClass = $derived({
		default: 'glass-panel',
		elevated: 'glass-panel-elevated',
		subtle: 'glass-panel-subtle',
	}[variant]);

	let hasEngineStyle = $derived(
		widgetId ? styleEngine.widgetStyles.has(widgetId) : false
	);

	let accentStyle = $derived(
		modelAccent
			? `border-color: ${modelAccent}20; --gx-glow-color: ${modelAccent};`
			: ''
	);
</script>

{#if hasEngineStyle && widgetId}
	<StyledWidget {widgetId} class={className}>
		{@render children()}
	</StyledWidget>
{:else}
	<div
		class="{panelClass} {className}"
		style={accentStyle}
	>
		{@render children()}
	</div>
{/if}
