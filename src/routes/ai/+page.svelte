<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import { Brain, Download, Play, Square, Cpu, Monitor, Cloud } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-ai';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let cardComponent = $derived(styleEngine.getComponentStyle(widgetId, 'card'));
	let cardStyle = $derived(hasEngineStyle && cardComponent ? componentToCSS(cardComponent) : '');
	let aiHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let aiHeaderStyle = $derived(hasEngineStyle && aiHeaderComponent ? componentToCSS(aiHeaderComponent) : '');

	interface ModelInfo {
		id: string;
		name: string;
		provider: string;
		free: boolean;
	}

	let models = $state<ModelInfo[]>([]);
	let loading = $state(true);

	onMount(async () => {
		try {
			models = await invoke<ModelInfo[]>('get_available_models');
		} catch (e) {
			console.error('Failed to load models:', e);
		} finally {
			loading = false;
		}
	});

	// Curated model recommendations
	const localModels = [
		{ name: 'qwen2.5-coder:7b', size: '4.7 GB', type: 'Code', device: 'GPU', status: 'available' },
		{ name: 'llama3.2:8b', size: '4.9 GB', type: 'Chat', device: 'GPU', status: 'available' },
		{ name: 'nomic-embed-text', size: '274 MB', type: 'Embeddings', device: 'CPU', status: 'required' },
		{ name: 'phi4-mini', size: '2.5 GB', type: 'Fast', device: 'GPU', status: 'available' },
	];

	const freeCloudModels = [
		{ name: 'Devstral Small', provider: 'Mistral', use: 'Code & Workflows', limit: '200/day' },
		{ name: 'Llama 4 Scout', provider: 'Meta', use: 'Chat & Research', limit: '200/day' },
		{ name: 'Qwen3-30B-A3B', provider: 'Qwen', use: 'Multi-Step Reasoning', limit: '200/day' },
		{ name: 'Gemma 3 27B', provider: 'Google', use: 'Instructions', limit: '200/day' },
		{ name: 'Mistral Small 3.1', provider: 'Mistral', use: 'General', limit: '200/day' },
		{ name: 'Llama 3.3 70B', provider: 'Meta', use: 'Complex Tasks', limit: '200/day' },
	];
</script>

<div class="p-6 space-y-4" style={containerStyle}>
	<div class="flex items-center gap-3" style={aiHeaderStyle}>
		<Brain size={24} class="text-gx-accent-magenta" />
		<h1 class="text-xl font-bold">AI Models</h1>
		<Badge class="bg-gx-neon/10 text-gx-neon border-gx-neon/30">
			{models.length} loaded
		</Badge>
	</div>

	<Tabs.Root value="local">
		<Tabs.List class="bg-gx-bg-secondary border border-gx-border-default">
			<Tabs.Trigger value="local" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Monitor size={14} class="mr-1.5" />
				Local Models
			</Tabs.Trigger>
			<Tabs.Trigger value="cloud" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Cloud size={14} class="mr-1.5" />
				Free Cloud Models
			</Tabs.Trigger>
			<Tabs.Trigger value="router" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Brain size={14} class="mr-1.5" />
				Smart Router
			</Tabs.Trigger>
		</Tabs.List>

		<Tabs.Content value="local" class="mt-3 space-y-2">
			{#each localModels as model}
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
					<Card.Content class="flex items-center gap-4 py-3">
						<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated">
							{#if model.device === 'GPU'}
								<Monitor size={16} class="text-gx-accent-purple" />
							{:else}
								<Cpu size={16} class="text-gx-accent-cyan" />
							{/if}
						</div>
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2">
								<span class="font-mono text-sm">{model.name}</span>
								<Badge variant="outline" class="text-[10px] border-gx-border-default">{model.type}</Badge>
							</div>
							<span class="text-xs text-gx-text-muted">{model.size} | {model.device}</span>
						</div>
						<button class="px-3 py-1.5 text-xs bg-gx-bg-elevated border border-gx-border-default rounded-gx hover:border-gx-neon hover:text-gx-neon transition-all">
							<Download size={12} class="inline mr-1" />
							Pull
						</button>
					</Card.Content>
				</Card.Root>
			{/each}
		</Tabs.Content>

		<Tabs.Content value="cloud" class="mt-3 space-y-2">
			{#each freeCloudModels as model}
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
					<Card.Content class="flex items-center gap-4 py-3">
						<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated">
							<Cloud size={16} class="text-gx-status-info" />
						</div>
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2">
								<span class="text-sm font-medium">{model.name}</span>
								<Badge class="text-[10px] bg-gx-neon/10 text-gx-neon border-gx-neon/30">FREE</Badge>
							</div>
							<span class="text-xs text-gx-text-muted">{model.provider} | {model.use} | {model.limit}</span>
						</div>
					</Card.Content>
				</Card.Root>
			{/each}
		</Tabs.Content>

		<Tabs.Content value="router" class="mt-3">
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
				<Card.Header>
					<Card.Title class="text-sm">Intelligent Model Router</Card.Title>
					<Card.Description class="text-xs text-gx-text-muted">
						Automatically selects the best model for each task. Zero API cost for 95%+ of requests.
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3 text-xs">
					<div class="grid grid-cols-2 gap-2">
						<div class="p-2 bg-gx-bg-tertiary rounded-gx">
							<span class="text-gx-text-muted">Code Generation</span>
							<div class="font-mono text-gx-neon mt-1">Devstral Small :free</div>
						</div>
						<div class="p-2 bg-gx-bg-tertiary rounded-gx">
							<span class="text-gx-text-muted">Chat / Questions</span>
							<div class="font-mono text-gx-neon mt-1">Llama 4 Scout :free</div>
						</div>
						<div class="p-2 bg-gx-bg-tertiary rounded-gx">
							<span class="text-gx-text-muted">README Summary</span>
							<div class="font-mono text-gx-neon mt-1">T5-small ONNX (local)</div>
						</div>
						<div class="p-2 bg-gx-bg-tertiary rounded-gx">
							<span class="text-gx-text-muted">Multi-Step Tasks</span>
							<div class="font-mono text-gx-neon mt-1">Qwen3-30B :free</div>
						</div>
					</div>
				</Card.Content>
			</Card.Root>
		</Tabs.Content>
	</Tabs.Root>
</div>
