<script lang="ts">
	import { onMount } from 'svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import {
		MessageSquare, GitBranch, Container, Workflow,
		Brain, Newspaper, Code2, Zap, TrendingUp, ArrowRight,
		Cpu, HardDrive, Monitor, Activity
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';

	// Quick action cards
	const quickActions = [
		{
			title: 'Chat',
			description: 'Talk to AI models',
			icon: MessageSquare,
			href: '/chat',
			color: 'text-gx-neon',
			count: '28 free models',
		},
		{
			title: 'GitHub',
			description: 'Manage repositories',
			icon: GitBranch,
			href: '/github',
			color: 'text-gx-accent-cyan',
			count: 'Browse & manage',
		},
		{
			title: 'Docker',
			description: 'Container management',
			icon: Container,
			href: '/docker',
			color: 'text-gx-accent-purple',
			count: 'Live dashboard',
		},
		{
			title: 'n8n & Services',
			description: 'Workflow automation',
			icon: Workflow,
			href: '/n8n',
			color: 'text-gx-accent-orange',
			count: '6 services',
		},
		{
			title: 'CodeForge',
			description: 'AI-powered IDE',
			icon: Code2,
			href: '/ide',
			color: 'text-gx-accent-purple',
			count: 'Monaco + AI Agent',
		},
		{
			title: 'AI Models',
			description: 'Local & cloud models',
			icon: Brain,
			href: '/ai',
			color: 'text-gx-accent-magenta',
			count: 'Model hub',
		},
		{
			title: 'AI News',
			description: 'Latest AI updates',
			icon: Newspaper,
			href: '/news',
			color: 'text-gx-status-info',
			count: 'Weekly digest',
		},
	];

	let cpuPercent = $derived(system.stats?.cpu_percent ?? 0);
	let ramUsed = $derived(system.stats?.ram_used_gb ?? 0);
	let ramTotal = $derived(system.stats?.ram_total_gb ?? 1);
	let ramPercent = $derived(ramTotal > 0 ? (ramUsed / ramTotal) * 100 : 0);
	let vramUsed = $derived((system.stats?.gpu_vram_used_mb ?? 0) / 1024);
	let vramTotal = $derived((system.stats?.gpu_vram_total_mb ?? 1) / 1024);
	let vramPercent = $derived(vramTotal > 0 ? (vramUsed / vramTotal) * 100 : 0);
	let gpuName = $derived(system.stats?.gpu_name ?? null);
	let gpuTemp = $derived(system.stats?.gpu_temp_c ?? null);

	let ollamaOnline = $derived(system.services.ollama === 'online');
	let dockerOnline = $derived(system.services.docker === 'online');
	let n8nOnline = $derived(system.services.n8n === 'online');
	let servicesOnline = $derived(
		[ollamaOnline, dockerOnline, n8nOnline].filter(Boolean).length
	);
</script>

<div class="p-6 space-y-6">
	<!-- Hero section -->
	<div class="flex items-center gap-4">
		<div class="w-14 h-14 bg-gx-bg-elevated rounded-gx-lg flex items-center justify-center border border-gx-neon shadow-gx-glow-sm">
			<span class="text-2xl font-bold text-gx-neon">N</span>
		</div>
		<div>
			<h1 class="text-2xl font-bold">Welcome to NEXUS</h1>
			<p class="text-gx-text-muted text-sm">Your complete AI stack. One desktop app.</p>
		</div>
		<div class="flex-1"></div>
		<Badge class="bg-gx-neon/10 text-gx-neon border-gx-neon/30">
			<Zap size={12} class="mr-1" />
			Free Tier Active
		</Badge>
	</div>

	<!-- Quick Actions Grid -->
	<div class="grid grid-cols-3 gap-3">
		{#each quickActions as action}
			<a href={action.href} class="group">
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/50 hover:bg-gx-bg-tertiary transition-all duration-200 cursor-pointer group-hover:shadow-gx-glow-sm">
					<Card.Header class="pb-2">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-2">
								<action.icon size={18} class={action.color} />
								<Card.Title class="text-sm font-medium">{action.title}</Card.Title>
							</div>
							<ArrowRight size={14} class="text-gx-text-muted opacity-0 group-hover:opacity-100 transition-opacity" />
						</div>
					</Card.Header>
					<Card.Content>
						<p class="text-xs text-gx-text-muted">{action.description}</p>
						<Badge variant="outline" class="mt-2 text-[10px] border-gx-border-default text-gx-text-muted">
							{action.count}
						</Badge>
					</Card.Content>
				</Card.Root>
			</a>
		{/each}
	</div>

	<!-- System Overview -->
	<div class="grid grid-cols-2 gap-3">
		<!-- Resource Usage — LIVE -->
		<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
			<Card.Header class="pb-2">
				<Card.Title class="text-sm font-medium flex items-center gap-2">
					<TrendingUp size={16} class="text-gx-neon" />
					System Resources
					{#if system.stats}
						<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success animate-pulse ml-1"></span>
					{/if}
				</Card.Title>
			</Card.Header>
			<Card.Content class="space-y-3">
				<div>
					<div class="flex justify-between text-xs mb-1">
						<span class="text-gx-text-muted flex items-center gap-1">
							<Cpu size={11} />
							CPU
						</span>
						<span class="text-gx-text-secondary font-mono">{cpuPercent.toFixed(0)}%</span>
					</div>
					<Progress value={cpuPercent} class="h-1.5" />
				</div>
				<div>
					<div class="flex justify-between text-xs mb-1">
						<span class="text-gx-text-muted flex items-center gap-1">
							<HardDrive size={11} />
							RAM
						</span>
						<span class="text-gx-text-secondary font-mono">{ramUsed.toFixed(1)} / {ramTotal.toFixed(0)} GB</span>
					</div>
					<Progress value={ramPercent} class="h-1.5" />
				</div>
				{#if system.stats?.gpu_vram_used_mb != null}
					<div>
						<div class="flex justify-between text-xs mb-1">
							<span class="text-gx-text-muted flex items-center gap-1">
								<Monitor size={11} class="text-gx-accent-magenta" />
								GPU VRAM
							</span>
							<span class="text-gx-text-secondary font-mono">{vramUsed.toFixed(1)} / {vramTotal.toFixed(0)} GB</span>
						</div>
						<Progress value={vramPercent} class="h-1.5" />
					</div>
					{#if gpuName}
						<div class="flex justify-between text-xs">
							<span class="text-gx-text-muted">GPU</span>
							<span class="text-gx-text-secondary">{gpuName}{#if gpuTemp} · {gpuTemp.toFixed(0)}°C{/if}</span>
						</div>
					{/if}
				{:else}
					<div>
						<div class="flex justify-between text-xs mb-1">
							<span class="text-gx-text-muted flex items-center gap-1">
								<Monitor size={11} />
								GPU VRAM
							</span>
							<span class="text-gx-text-muted">Not detected</span>
						</div>
						<Progress value={0} class="h-1.5" />
					</div>
				{/if}
			</Card.Content>
		</Card.Root>

		<!-- Intelligent Router Status -->
		<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
			<Card.Header class="pb-2">
				<Card.Title class="text-sm font-medium flex items-center gap-2">
					<Brain size={16} class="text-gx-accent-magenta" />
					Intelligent Router
				</Card.Title>
			</Card.Header>
			<Card.Content class="space-y-2 text-xs">
				<div class="flex justify-between">
					<span class="text-gx-text-muted">Active Provider</span>
					<Badge variant="outline" class="text-[10px] border-gx-neon/30 text-gx-neon">Auto — Free</Badge>
				</div>
				<div class="flex justify-between">
					<span class="text-gx-text-muted">API Cost</span>
					<span class="text-gx-neon font-medium font-mono">$0.00</span>
				</div>
				<div class="flex justify-between">
					<span class="text-gx-text-muted">Services Online</span>
					<span class="text-gx-text-secondary">{servicesOnline} / 3</span>
				</div>
				<div class="flex justify-between">
					<span class="text-gx-text-muted">Ollama</span>
					<span class={ollamaOnline ? 'text-gx-status-success' : 'text-gx-status-error'}>
						{ollamaOnline ? 'Connected' : 'Offline'}
					</span>
				</div>
				<div class="flex justify-between">
					<span class="text-gx-text-muted">Docker</span>
					<span class={dockerOnline ? 'text-gx-status-success' : 'text-gx-status-error'}>
						{dockerOnline ? 'Connected' : 'Offline'}
					</span>
				</div>
			</Card.Content>
		</Card.Root>
	</div>
</div>
