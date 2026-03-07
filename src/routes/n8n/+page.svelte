<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import {
		Workflow, Globe, RefreshCw, ExternalLink, Loader2,
		AlertCircle, Boxes, Brain, BarChart3, Search
	} from '@lucide/svelte';

	interface ServiceDef {
		id: string;
		name: string;
		icon: typeof Workflow;
		url: string;
		description: string;
		startHint: string;
	}

	const services: ServiceDef[] = [
		{
			id: 'n8n', name: 'n8n', icon: Workflow,
			url: 'http://localhost:5678',
			description: 'Visual workflow automation with 400+ integrations',
			startHint: 'docker run -d --name n8n -p 5678:5678 n8nio/n8n',
		},
		{
			id: 'langflow', name: 'LangFlow', icon: Boxes,
			url: 'http://localhost:7860',
			description: 'Visual AI agent builder with drag-and-drop',
			startHint: 'docker run -d --name langflow -p 7860:7860 langflowai/langflow',
		},
		{
			id: 'openwebui', name: 'Open WebUI', icon: Brain,
			url: 'http://localhost:3000',
			description: 'ChatGPT-style interface for local models',
			startHint: 'docker run -d --name open-webui -p 3000:8080 ghcr.io/open-webui/open-webui',
		},
		{
			id: 'grafana', name: 'Grafana', icon: BarChart3,
			url: 'http://localhost:4000',
			description: 'Observability and monitoring dashboards',
			startHint: 'docker run -d --name grafana -p 4000:3000 grafana/grafana',
		},
		{
			id: 'searxng', name: 'SearXNG', icon: Search,
			url: 'http://localhost:8888',
			description: 'Privacy-respecting meta search engine',
			startHint: 'docker run -d --name searxng -p 8888:8080 searxng/searxng',
		},
		{
			id: 'comfyui', name: 'ComfyUI', icon: Globe,
			url: 'http://localhost:8188',
			description: 'Node-based image generation workflow',
			startHint: 'python main.py --listen',
		},
	];

	let activeService = $state<ServiceDef>(services[0]);
	let serviceOnline = $state(false);
	let loading = $state(true);
	let iframeKey = $state(0);

	async function checkService() {
		loading = true;
		serviceOnline = false;
		try {
			serviceOnline = await invoke<boolean>('cmd_check_service_health', {
				service: activeService.id,
			});
		} catch {
			serviceOnline = false;
		}
		loading = false;
	}

	function switchService(svc: ServiceDef) {
		activeService = svc;
		checkService();
	}

	function refreshIframe() {
		iframeKey++;
	}

	function openExternal() {
		window.open(activeService.url, '_blank');
	}

	onMount(() => {
		checkService();
	});
</script>

<div class="flex flex-col h-full">
	<!-- Service tabs bar -->
	<div class="flex items-center gap-1 px-3 py-1.5 bg-gx-bg-secondary border-b border-gx-border-default shrink-0 overflow-x-auto">
		{#each services as svc}
			<button
				onclick={() => switchService(svc)}
				class="flex items-center gap-1.5 px-3 py-1 text-xs rounded-gx whitespace-nowrap transition-all
					{activeService.id === svc.id
						? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
			>
				<svc.icon size={13} />
				{svc.name}
			</button>
		{/each}

		<div class="flex-1" />

		{#if serviceOnline}
			<button
				onclick={refreshIframe}
				class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
				title="Refresh"
			>
				<RefreshCw size={14} />
			</button>
			<button
				onclick={openExternal}
				class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
				title="Open in browser"
			>
				<ExternalLink size={14} />
			</button>
		{/if}

		<Badge
			variant="outline"
			class="text-[10px] px-1.5 py-0 h-4 {serviceOnline ? 'border-gx-status-success text-gx-status-success' : 'border-gx-status-error text-gx-status-error'}"
		>
			{serviceOnline ? 'Online' : 'Offline'}
		</Badge>
	</div>

	<!-- Content area -->
	{#if loading}
		<div class="flex-1 flex flex-col items-center justify-center gap-3">
			<Loader2 size={32} class="text-gx-neon animate-spin" />
			<span class="text-sm text-gx-text-muted">Checking {activeService.name}...</span>
		</div>
	{:else if serviceOnline}
		{#key iframeKey}
			<iframe
				src={activeService.url}
				class="flex-1 w-full border-none bg-gx-bg-primary"
				title="{activeService.name} Dashboard"
			></iframe>
		{/key}
	{:else}
		<div class="flex-1 flex flex-col items-center justify-center gap-6">
			<div class="w-20 h-20 rounded-full bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default">
				<AlertCircle size={36} class="text-gx-status-error" />
			</div>

			<div class="text-center max-w-md">
				<h2 class="text-lg font-semibold text-gx-text-primary mb-1">
					{activeService.name} is not running
				</h2>
				<p class="text-sm text-gx-text-muted mb-4">
					{activeService.description}
				</p>

				<div class="bg-gx-bg-tertiary border border-gx-border-default rounded-gx-lg p-4 text-left">
					<p class="text-xs text-gx-text-muted mb-2">Start with:</p>
					<code class="text-xs text-gx-neon font-mono break-all">
						{activeService.startHint}
					</code>
				</div>
			</div>

			<button
				onclick={checkService}
				class="flex items-center gap-2 px-4 py-2 text-sm rounded-gx border border-gx-border-default text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-colors"
			>
				<RefreshCw size={14} />
				Retry
			</button>
		</div>
	{/if}
</div>
