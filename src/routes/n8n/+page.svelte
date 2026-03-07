<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import {
		Workflow, Globe, RefreshCw, ExternalLink, Loader2,
		AlertCircle, Boxes, Brain, BarChart3, Search,
		Circle, Play, Terminal
	} from '@lucide/svelte';

	interface ServiceDef {
		id: string;
		name: string;
		icon: typeof Workflow;
		url: string;
		description: string;
		startHint: string;
		license: string;
	}

	const services: ServiceDef[] = [
		{
			id: 'n8n', name: 'n8n', icon: Workflow,
			url: 'http://localhost:5678',
			description: 'Visual workflow automation with 400+ integrations',
			startHint: 'docker run -d --name n8n -p 5678:5678 n8nio/n8n',
			license: 'Sustainable Use License',
		},
		{
			id: 'langflow', name: 'LangFlow', icon: Boxes,
			url: 'http://localhost:7860',
			description: 'Visual AI agent builder with drag-and-drop',
			startHint: 'docker run -d --name langflow -p 7860:7860 langflowai/langflow',
			license: 'MIT',
		},
		{
			id: 'openwebui', name: 'Open WebUI', icon: Brain,
			url: 'http://localhost:3000',
			description: 'ChatGPT-style interface for local models',
			startHint: 'docker run -d --name open-webui -p 3000:8080 ghcr.io/open-webui/open-webui',
			license: 'MIT',
		},
		{
			id: 'grafana', name: 'Grafana', icon: BarChart3,
			url: 'http://localhost:4000',
			description: 'Observability and monitoring dashboards',
			startHint: 'docker run -d --name grafana -p 4000:3000 grafana/grafana',
			license: 'AGPL-3.0',
		},
		{
			id: 'searxng', name: 'SearXNG', icon: Search,
			url: 'http://localhost:8888',
			description: 'Privacy-respecting meta search engine',
			startHint: 'docker run -d --name searxng -p 8888:8080 searxng/searxng',
			license: 'AGPL-3.0',
		},
		{
			id: 'comfyui', name: 'ComfyUI', icon: Globe,
			url: 'http://localhost:8188',
			description: 'Node-based image generation workflow',
			startHint: 'python main.py --listen',
			license: 'GPL-3.0',
		},
	];

	let activeService = $state<ServiceDef>(services[0]);
	let serviceStatuses = $state<Record<string, boolean>>({});
	let loading = $state(true);

	async function checkService(svc: ServiceDef) {
		try {
			const online = await invoke<boolean>('cmd_check_service_health', {
				service: svc.id,
			});
			serviceStatuses[svc.id] = online;
		} catch {
			serviceStatuses[svc.id] = false;
		}
	}

	async function checkAllServices() {
		loading = true;
		await Promise.all(services.map(checkService));
		loading = false;
	}

	function switchService(svc: ServiceDef) {
		activeService = svc;
	}

	function openInBrowser(svc: ServiceDef) {
		window.open(svc.url, '_blank');
	}

	onMount(() => {
		checkAllServices();
	});

	let isOnline = $derived(serviceStatuses[activeService.id] ?? false);
	let onlineCount = $derived(Object.values(serviceStatuses).filter(Boolean).length);
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
				<Circle size={6} class="{serviceStatuses[svc.id] ? 'text-gx-status-success fill-gx-status-success' : 'text-gx-text-muted fill-gx-text-muted'}" />
			</button>
		{/each}

		<div class="flex-1"></div>

		<button
			onclick={checkAllServices}
			class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
			title="Refresh all"
		>
			<RefreshCw size={14} class="{loading ? 'animate-spin' : ''}" />
		</button>

		<Badge
			variant="outline"
			class="text-[10px] px-1.5 py-0 h-4 border-gx-border-default text-gx-text-muted"
		>
			{onlineCount}/{services.length} online
		</Badge>
	</div>

	<!-- Service detail -->
	<div class="flex-1 overflow-y-auto p-6">
		<div class="max-w-2xl mx-auto space-y-6">
			<!-- Service header -->
			<div class="flex items-start gap-4">
				<div class="w-16 h-16 rounded-gx-lg bg-gx-bg-elevated border border-gx-border-default flex items-center justify-center shrink-0">
					<activeService.icon size={28} class="{isOnline ? 'text-gx-neon' : 'text-gx-text-muted'}" />
				</div>
				<div class="flex-1">
					<div class="flex items-center gap-2 mb-1">
						<h2 class="text-xl font-bold text-gx-text-primary">{activeService.name}</h2>
						<Badge variant="outline" class="text-[10px] px-1.5 {isOnline ? 'border-gx-status-success text-gx-status-success' : 'border-gx-status-error text-gx-status-error'}">
							{isOnline ? 'Online' : 'Offline'}
						</Badge>
						<Badge variant="outline" class="text-[10px] px-1.5 border-gx-border-default text-gx-text-muted">
							{activeService.license}
						</Badge>
					</div>
					<p class="text-sm text-gx-text-muted">{activeService.description}</p>
					<p class="text-xs text-gx-text-muted mt-1 font-mono">{activeService.url}</p>
				</div>
			</div>

			<!-- Action buttons -->
			<div class="flex gap-3">
				<button
					onclick={() => openInBrowser(activeService)}
					disabled={!isOnline}
					class="flex items-center gap-2 px-5 py-2.5 text-sm font-medium rounded-gx transition-all
						{isOnline
							? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 hover:shadow-gx-glow-sm'
							: 'bg-gx-bg-tertiary text-gx-text-muted border border-gx-border-default cursor-not-allowed'}"
				>
					<ExternalLink size={16} />
					Open in Browser
				</button>
				<button
					onclick={() => checkService(activeService)}
					class="flex items-center gap-2 px-4 py-2.5 text-sm rounded-gx border border-gx-border-default text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-colors"
				>
					<RefreshCw size={14} />
					Check Status
				</button>
			</div>

			<!-- Start instructions (when offline) -->
			{#if !isOnline}
				<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-5">
					<h3 class="text-sm font-medium text-gx-text-secondary mb-3 flex items-center gap-2">
						<Terminal size={14} class="text-gx-neon" />
						How to start {activeService.name}
					</h3>
					<div class="bg-gx-bg-tertiary rounded-gx p-3 font-mono text-xs text-gx-neon border border-gx-border-default">
						{activeService.startHint}
					</div>
					<p class="text-xs text-gx-text-muted mt-3">
						Run this command in your terminal, then click "Check Status" above.
					</p>
				</div>
			{/if}

			<!-- All services overview -->
			<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4">
				<h3 class="text-sm font-medium text-gx-text-secondary mb-3">All Services</h3>
				<div class="space-y-2">
					{#each services as svc}
						<button
							onclick={() => switchService(svc)}
							class="w-full flex items-center gap-3 px-3 py-2 rounded-gx text-left transition-colors
								{activeService.id === svc.id ? 'bg-gx-bg-hover' : 'hover:bg-gx-bg-hover'}"
						>
							<Circle size={8} class="{serviceStatuses[svc.id] ? 'text-gx-status-success fill-gx-status-success' : 'text-gx-text-muted fill-gx-text-muted'}" />
							<svc.icon size={14} class="text-gx-text-muted shrink-0" />
							<span class="text-xs text-gx-text-primary flex-1">{svc.name}</span>
							<span class="text-[10px] text-gx-text-muted font-mono">{svc.url}</span>
							{#if serviceStatuses[svc.id]}
								<span class="text-[10px] text-gx-status-success">Ready</span>
							{/if}
						</button>
					{/each}
				</div>
			</div>
		</div>
	</div>
</div>
