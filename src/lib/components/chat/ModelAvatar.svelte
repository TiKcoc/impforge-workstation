<script lang="ts">
	/**
	 * ModelAvatar — BenikUI-integrated per-model branded avatar
	 *
	 * Distinct animations per model brand (orbital for Claude, breathe for Qwen,
	 * flicker for Hermes, emanate for Local). When style engine styles are loaded,
	 * the avatar circle and ring become deeply customizable.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - ring: The glow/streaming ring
	 *   - circle: The avatar circle
	 */

	import { Bot, Loader2 } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		modelName?: string;
		streaming?: boolean;
		size?: number;
	}

	let {
		widgetId = 'chat-model-avatar',
		modelName,
		streaming = false,
		size = 32
	}: Props = $props();

	/** Derive model brand from model name string */
	function getModelBrand(name?: string): { color: string; cssVar: string; animation: string; label: string } {
		if (!name) return { color: '#606070', cssVar: '--model-system', animation: '', label: 'AI' };
		const n = name.toLowerCase();
		if (n.includes('claude') || n.includes('anthropic'))
			return { color: '#a855f7', cssVar: '--model-claude', animation: 'animate-model-orbital', label: 'C' };
		if (n.includes('qwen'))
			return { color: '#06b6d4', cssVar: '--model-qwen', animation: 'animate-model-breathe', label: 'Q' };
		if (n.includes('hermes'))
			return { color: '#f59e0b', cssVar: '--model-hermes', animation: 'animate-model-flicker', label: 'H' };
		if (n.includes('ollama') || n.includes('local') || n.includes('llama') || n.includes('mistral') || n.includes('dolphin') || n.includes('phi'))
			return { color: '#22c55e', cssVar: '--model-local', animation: 'animate-model-emanate', label: 'L' };
		if (n.includes('gpt') || n.includes('openai'))
			return { color: '#10b981', cssVar: '--model-local', animation: 'animate-model-breathe', label: 'G' };
		if (n.includes('gemini') || n.includes('google'))
			return { color: '#3b82f6', cssVar: '--model-system', animation: 'animate-model-breathe', label: 'G' };
		return { color: '#06b6d4', cssVar: '--model-qwen', animation: 'animate-model-breathe', label: 'M' };
	}

	let brand = $derived(getModelBrand(modelName));
	let iconSize = $derived(Math.round(size * 0.44));

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let ringComponent = $derived(styleEngine.getComponentStyle(widgetId, 'ring'));
	let circleComponent = $derived(styleEngine.getComponentStyle(widgetId, 'circle'));

	// Merge style engine with brand colors — brand color overrides theme color
	let ringStyle = $derived(() => {
		if (hasEngineStyle && ringComponent) {
			const css = componentToCSS(ringComponent);
			return `${css} --gx-glow-color: ${brand.color};`;
		}
		return `box-shadow: 0 0 12px ${brand.color}40, 0 0 24px ${brand.color}20; border: 1.5px solid ${brand.color}60;`;
	});

	let circleStyle = $derived(() => {
		if (hasEngineStyle && circleComponent) {
			const css = componentToCSS(circleComponent);
			return `${css} color: ${brand.color}; border-color: ${streaming ? brand.color + '80' : brand.color + '30'};`;
		}
		return `background: ${brand.color}15; border: 1.5px solid ${streaming ? brand.color + '80' : brand.color + '30'}; color: ${brand.color}; transition: border-color 0.3s, box-shadow 0.3s; ${streaming ? `box-shadow: 0 0 8px ${brand.color}30;` : ''}`;
	});
</script>

<div
	class="relative flex items-center justify-center shrink-0"
	style="width: {size}px; height: {size}px;"
>
	<!-- Outer glow ring (visible during streaming) -->
	{#if streaming}
		<div
			class="absolute inset-0 rounded-full {brand.animation}"
			style={ringStyle()}
		></div>
		<!-- Orbital ring for Claude specifically -->
		{#if brand.label === 'C'}
			<div
				class="absolute inset-[-4px] rounded-full animate-model-orbital"
				style="border: 1px dashed {brand.color}40; border-top-color: {brand.color};"
			></div>
		{/if}
	{/if}

	<!-- Avatar circle -->
	<div
		class="w-full h-full rounded-full flex items-center justify-center text-[10px] font-bold"
		style={circleStyle()}
	>
		{#if streaming}
			<Loader2 size={iconSize} class="animate-spin" style="color: {brand.color};" />
		{:else}
			<Bot size={iconSize} style="color: {brand.color};" />
		{/if}
	</div>
</div>
