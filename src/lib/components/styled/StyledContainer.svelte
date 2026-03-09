<script lang="ts">
	/**
	 * StyledContainer — BenikUI-style styled wrapper
	 *
	 * Applies ComponentStyle (background, border, glow, animation, padding)
	 * to a container element. Children render inside with full style applied.
	 *
	 * Usage:
	 *   <StyledContainer style={componentStyle}>
	 *     <YourContent />
	 *   </StyledContainer>
	 */

	import { componentToCSS, type ComponentStyle } from '$lib/stores/style-engine.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		style: ComponentStyle | undefined;
		class?: string;
		children: Snippet;
		/** Optional: open style editor on double-click */
		onStyleEdit?: () => void;
	}

	let { style, class: className = '', children, onStyleEdit }: Props = $props();

	let cssString = $derived(style ? componentToCSS(style) : '');

	// Glow pulse animation (when enabled)
	let glowClass = $derived(
		style?.glow.animated && style.glow.glow_type !== 'None'
			? 'gx-glow-pulse-active'
			: ''
	);

	// Background animation class
	let bgAnimClass = $derived(
		style?.background.animated ? 'gx-bg-animated' : ''
	);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="styled-container relative {glowClass} {bgAnimClass} {className}"
	role="group"
	style={cssString}
	style:--gx-pulse-duration="{style?.glow.pulse_duration_ms ?? 2000}ms"
	style:--gx-gradient-duration="{style?.background.animation_duration_ms ?? 5000}ms"
	ondblclick={onStyleEdit}
>
	{@render children()}
</div>

<style>
	.gx-glow-pulse-active {
		animation: gx-glow-pulse var(--gx-pulse-duration, 2s) ease-in-out infinite;
	}

	.gx-bg-animated {
		background-size: 400% 400%;
		animation: gx-gradient-shift var(--gx-gradient-duration, 5s) ease infinite;
	}

	@keyframes gx-glow-pulse {
		0%, 100% { filter: brightness(1); }
		50% { filter: brightness(1.15); }
	}

	@keyframes gx-gradient-shift {
		0% { background-position: 0% 50%; }
		50% { background-position: 100% 50%; }
		100% { background-position: 0% 50%; }
	}

	@media (prefers-reduced-motion: reduce) {
		.gx-glow-pulse-active,
		.gx-bg-animated {
			animation: none !important;
		}
	}
</style>
