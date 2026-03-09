<script lang="ts">
	/**
	 * StyledText — BenikUI-style configurable text renderer
	 *
	 * Renders text/numbers with:
	 * - Configurable font family, size, weight, color
	 * - Text outline/stroke (Thin/Medium/Thick)
	 * - Text shadow for glow effects
	 * - Number formatting (Raw, Abbreviated, Percent, CurrentMax, etc.)
	 * - Position offset from parent
	 * - Letter spacing, text transform, opacity
	 *
	 * Usage:
	 *   <StyledText style={textStyle} value={42} max={100} />
	 *   <StyledText style={textStyle}>Custom content</StyledText>
	 */

	import {
		formatNumber,
		type TextStyle,
		type FontFamily,
	} from '$lib/stores/style-engine.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		style: TextStyle;
		/** Numeric value for number formatting */
		value?: number | null;
		/** Max value for ratio/percent formatting */
		max?: number | null;
		/** Static text (overrides value formatting) */
		label?: string | null;
		/** Slot content (highest priority) */
		children?: Snippet;
		class?: string;
	}

	let { style, value = null, max = null, label = null, children, class: className = '' }: Props = $props();

	// Font family resolution
	function fontToCSS(ff: FontFamily): string {
		if (ff === 'System') return 'system-ui, sans-serif';
		if (ff === 'Mono') return "'JetBrains Mono', 'Fira Code', monospace";
		if (typeof ff === 'object' && 'Custom' in ff) return ff.Custom;
		return 'system-ui, sans-serif';
	}

	// Outline to CSS
	function outlineCSS(): string {
		switch (style.outline) {
			case 'Thin': return `-webkit-text-stroke: 1px ${style.color}40`;
			case 'Medium': return `-webkit-text-stroke: 2px ${style.color}60`;
			case 'Thick': return `-webkit-text-stroke: 3px ${style.color}80`;
			default: return '';
		}
	}

	// Display content
	let displayText = $derived(
		label != null
			? label
			: value != null
				? formatNumber(value, max ?? null, style.number_format)
				: ''
	);

	// Combined inline style
	let cssStyle = $derived([
		`font-family: ${fontToCSS(style.font_family)}`,
		`font-size: ${style.font_size}px`,
		`font-weight: ${style.font_weight}`,
		`color: ${style.color}`,
		`letter-spacing: ${style.letter_spacing}px`,
		`text-transform: ${style.text_transform}`,
		`opacity: ${style.opacity}`,
		style.shadow ? `text-shadow: ${style.shadow}` : '',
		outlineCSS(),
		(style.offset[0] !== 0 || style.offset[1] !== 0)
			? `transform: translate(${style.offset[0]}px, ${style.offset[1]}px)`
			: '',
	].filter(s => s).join('; '));
</script>

{#if style.visible}
	<span class="styled-text {className}" style={cssStyle}>
		{#if children}
			{@render children()}
		{:else}
			{displayText}
		{/if}
	</span>
{/if}
