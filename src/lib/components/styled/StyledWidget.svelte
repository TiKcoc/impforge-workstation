<script lang="ts">
	/**
	 * StyledWidget — Universal BenikUI wrapper for any widget
	 *
	 * Automatically loads styles from the style engine and wraps
	 * the widget content in a StyledContainer with the "container"
	 * sub-component's style applied. Children can access sub-component
	 * styles via the styleEngine store directly.
	 *
	 * Usage:
	 *   <StyledWidget widgetId="agent-pool">
	 *     <YourWidgetContent />
	 *   </StyledWidget>
	 */

	import { styleEngine, componentToCSS, type ComponentStyle } from '$lib/stores/style-engine.svelte';
	import StyledContainer from './StyledContainer.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		widgetId: string;
		class?: string;
		children: Snippet;
	}

	let { widgetId, class: className = '', children }: Props = $props();

	// Auto-load styles on mount
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});

	let containerStyle = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let hasStyles = $derived(!!containerStyle);
</script>

{#if hasStyles && containerStyle}
	<StyledContainer style={containerStyle} class="h-full {className}">
		{@render children()}
	</StyledContainer>
{:else}
	<!-- Fallback: render without custom styling -->
	<div class="h-full {className}">
		{@render children()}
	</div>
{/if}
