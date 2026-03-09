<script lang="ts">
	/**
	 * StyledBar — BenikUI-style configurable progress/health bar
	 *
	 * Renders a bar with:
	 * - Configurable fill direction, texture, color thresholds
	 * - Gradient fills, spark effects, striped/glossy textures
	 * - Animated width transitions
	 * - Optional label overlay with number formatting
	 *
	 * Usage:
	 *   <StyledBar bar={barStyle} text={textStyle} value={0.75} max={100} />
	 */

	import {
		barToCSS,
		formatNumber,
		type BarStyle,
		type TextStyle,
	} from '$lib/stores/style-engine.svelte';

	interface Props {
		bar: BarStyle;
		text?: TextStyle | null;
		/** Normalized value 0-1 (or raw value if max is provided) */
		value: number;
		/** Max value for number formatting. If provided, value is treated as absolute. */
		max?: number | null;
		class?: string;
	}

	let { bar, text = null, value, max = null, class: className = '' }: Props = $props();

	// Normalize to 0-1 for bar fill
	let normalizedValue = $derived(max != null && max > 0 ? value / max : value);

	let barCSS = $derived(barToCSS(bar, normalizedValue));

	// Texture class
	let textureClass = $derived(
		bar.texture === 'Striped' ? 'gx-bar-striped' :
		bar.texture === 'Glossy' ? 'gx-bar-glossy' :
		bar.texture === 'Minimalist' ? 'gx-bar-minimal' : ''
	);

	// Spark effect
	let sparkClass = $derived(bar.spark_effect ? 'gx-bar-spark' : '');

	// Fill direction transform
	let fillDirection = $derived(() => {
		switch (bar.fill_direction) {
			case 'RightToLeft': return 'scaleX(-1)';
			case 'BottomToTop': return 'rotate(-90deg)';
			case 'TopToBottom': return 'rotate(90deg)';
			default: return 'none';
		}
	});

	// Formatted label
	let displayText = $derived(
		text && text.visible
			? formatNumber(max != null ? value : Math.round(normalizedValue * 100), max, text.number_format)
			: ''
	);

	// Text color from text style
	let textCSS = $derived(
		text ? `color: ${text.color}; font-size: ${text.font_size}px; font-weight: ${text.font_weight}; letter-spacing: ${text.letter_spacing}px;` : ''
	);
</script>

{#if bar.visible}
	<div
		class="styled-bar relative overflow-hidden {className}"
		style="{barCSS.containerStyle} transform: {fillDirection()};"
		role="progressbar"
		aria-valuenow={Math.round(normalizedValue * 100)}
		aria-valuemin={0}
		aria-valuemax={100}
		aria-label="{displayText || `${Math.round(normalizedValue * 100)}%`}"
	>
		<!-- Fill -->
		<div
			class="absolute inset-y-0 left-0 {textureClass} {sparkClass}"
			style="{barCSS.fillStyle} width: {barCSS.fillWidth};"
		></div>

		<!-- Label overlay -->
		{#if displayText}
			<span
				class="relative z-10 flex items-center justify-center h-full text-center"
				style={textCSS}
			>
				{displayText}
			</span>
		{/if}
	</div>
{/if}

<style>
	.gx-bar-striped::after {
		content: '';
		position: absolute;
		inset: 0;
		background: repeating-linear-gradient(
			45deg,
			transparent,
			transparent 4px,
			rgba(255, 255, 255, 0.08) 4px,
			rgba(255, 255, 255, 0.08) 8px
		);
		pointer-events: none;
	}

	.gx-bar-glossy::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(
			180deg,
			rgba(255, 255, 255, 0.2) 0%,
			rgba(255, 255, 255, 0.05) 50%,
			transparent 50%,
			rgba(0, 0, 0, 0.1) 100%
		);
		pointer-events: none;
	}

	.gx-bar-minimal {
		opacity: 0.8;
	}

	.gx-bar-spark {
		animation: bar-spark 1.5s ease-in-out infinite;
	}

	@keyframes bar-spark {
		0%, 100% { filter: brightness(1); }
		50% { filter: brightness(1.3); }
	}

	@media (prefers-reduced-motion: reduce) {
		.gx-bar-spark {
			animation: none;
		}
	}
</style>
