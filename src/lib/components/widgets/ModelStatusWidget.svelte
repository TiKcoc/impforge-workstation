<script lang="ts">
	/**
	 * ModelStatusWidget — Local/cloud model availability
	 * Shows Ollama model status and OpenRouter health.
	 */
	import { Brain, Cloud, Server } from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { system } from '$lib/stores/system.svelte';
	import { StyledWidget } from '$lib/components/styled';

	let ollamaOnline = $derived(system.services.ollama === 'online');

	const models = [
		{ name: 'Dolphin 3 8B', provider: 'Ollama', free: true },
		{ name: 'Qwen2.5-Coder 7B', provider: 'Ollama', free: true },
		{ name: 'Devstral Small', provider: 'OpenRouter', free: true },
		{ name: 'Llama 4 Scout', provider: 'OpenRouter', free: true },
	];
</script>

<StyledWidget widgetId="model-status">
<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
	<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
		<Brain size={12} class="text-gx-accent-magenta" />
		<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Models</span>
		<Badge variant="outline" class="text-[8px] px-1 py-0 h-3 border-gx-neon/30 text-gx-neon ml-auto">
			{ollamaOnline ? 'Online' : 'Offline'}
		</Badge>
	</div>
	<div class="flex-1 p-2.5 space-y-1 overflow-auto">
		{#each models as model}
			<div class="flex items-center gap-2 text-[11px] py-0.5">
				{#if model.provider === 'Ollama'}
					<Server size={9} class="text-gx-accent-cyan shrink-0" />
				{:else}
					<Cloud size={9} class="text-gx-accent-purple shrink-0" />
				{/if}
				<span class="text-gx-text-secondary flex-1 truncate">{model.name}</span>
				{#if model.free}
					<span class="text-[8px] text-gx-neon font-mono">FREE</span>
				{/if}
			</div>
		{/each}
	</div>
</div>
</StyledWidget>
