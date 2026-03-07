<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import {
		Bot, Network, Activity, Brain, Cpu, Zap,
		Play, Pause, RefreshCw, ChevronRight, Circle
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';

	interface AgentDef {
		id: string;
		name: string;
		role: string;
		status: string;
		description: string;
		capabilities: string[];
	}

	let agents = $state<AgentDef[]>([]);
	let loading = $state(true);
	let activeTab = $state<'pool' | 'topology' | 'metrics'>('pool');

	// Built-in agent definitions for the NeuralSwarm
	const builtInAgents: AgentDef[] = [
		{
			id: 'orchestrator',
			name: 'Master Orchestrator',
			role: 'orchestrator',
			status: 'idle',
			description: 'Coordinates multi-agent workflows and decomposes objectives into tasks',
			capabilities: ['task-decomposition', 'agent-routing', 'priority-management'],
		},
		{
			id: 'coder',
			name: 'Code Agent',
			role: 'coder',
			status: 'idle',
			description: 'Generates, reviews, and refactors code across languages',
			capabilities: ['code-generation', 'refactoring', 'testing', 'debugging'],
		},
		{
			id: 'researcher',
			name: 'Research Agent',
			role: 'researcher',
			status: 'idle',
			description: 'Searches documentation, papers, and web for relevant information',
			capabilities: ['web-search', 'paper-analysis', 'documentation', 'summarization'],
		},
		{
			id: 'debugger',
			name: 'Debug Agent',
			role: 'debugger',
			status: 'idle',
			description: 'Analyzes errors, traces root causes, and suggests fixes',
			capabilities: ['error-analysis', 'stack-trace', 'root-cause', 'fix-suggestion'],
		},
		{
			id: 'devops',
			name: 'DevOps Agent',
			role: 'devops',
			status: 'idle',
			description: 'Manages Docker, CI/CD, deployment, and infrastructure',
			capabilities: ['docker', 'ci-cd', 'deployment', 'monitoring'],
		},
		{
			id: 'reviewer',
			name: 'Review Agent',
			role: 'reviewer',
			status: 'idle',
			description: 'Reviews code quality, security, and best practices compliance',
			capabilities: ['code-review', 'security-audit', 'best-practices', 'documentation'],
		},
	];

	onMount(async () => {
		try {
			const serverAgents = await invoke<AgentDef[]>('list_agents');
			agents = serverAgents.length > 0 ? serverAgents : builtInAgents;
		} catch {
			agents = builtInAgents;
		}
		loading = false;
	});

	function statusColor(status: string): string {
		switch (status) {
			case 'active': return 'text-gx-status-success';
			case 'busy': return 'text-gx-status-warning';
			case 'error': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusBadge(status: string): { cls: string; label: string } {
		switch (status) {
			case 'active': return { cls: 'border-gx-status-success text-gx-status-success', label: 'Active' };
			case 'busy': return { cls: 'border-gx-status-warning text-gx-status-warning', label: 'Busy' };
			case 'error': return { cls: 'border-gx-status-error text-gx-status-error', label: 'Error' };
			default: return { cls: 'border-gx-border-default text-gx-text-muted', label: 'Idle' };
		}
	}
</script>

<div class="flex flex-col h-full">
	<!-- Tab bar -->
	<div class="flex items-center gap-1 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
		<button
			onclick={() => activeTab = 'pool'}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'pool'
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Bot size={13} />
			Agent Pool
		</button>
		<button
			onclick={() => activeTab = 'topology'}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'topology'
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Network size={13} />
			Topology
		</button>
		<button
			onclick={() => activeTab = 'metrics'}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'metrics'
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Activity size={13} />
			Metrics
		</button>

		<div class="flex-1"></div>

		<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-neon/30 text-gx-neon">
			NeuralSwarm
		</Badge>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-y-auto p-4">
		{#if activeTab === 'pool'}
			<!-- Agent Pool -->
			<div class="space-y-3">
				<div class="flex items-center justify-between mb-4">
					<div>
						<h2 class="text-lg font-semibold text-gx-text-primary">Agent Pool</h2>
						<p class="text-xs text-gx-text-muted">Manage your NeuralSwarm agents</p>
					</div>
					<div class="flex items-center gap-2 text-xs text-gx-text-muted">
						<span class="flex items-center gap-1">
							<Circle size={8} class="text-gx-status-success fill-gx-status-success" />
							{agents.filter(a => a.status === 'active').length} active
						</span>
						<span class="flex items-center gap-1">
							<Circle size={8} class="text-gx-text-muted fill-gx-text-muted" />
							{agents.filter(a => a.status === 'idle').length} idle
						</span>
					</div>
				</div>

				{#each agents as agent (agent.id)}
					<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4 hover:border-gx-neon/30 transition-colors">
						<div class="flex items-start gap-3">
							<div class="w-10 h-10 rounded-gx bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shrink-0">
								<Bot size={18} class={statusColor(agent.status)} />
							</div>
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2 mb-1">
									<span class="text-sm font-medium text-gx-text-primary">{agent.name}</span>
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {statusBadge(agent.status).cls}">
										{statusBadge(agent.status).label}
									</Badge>
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted">
										{agent.role}
									</Badge>
								</div>
								<p class="text-xs text-gx-text-muted mb-2">{agent.description}</p>
								<div class="flex flex-wrap gap-1">
									{#each agent.capabilities as cap}
										<span class="text-[10px] px-1.5 py-0.5 rounded bg-gx-bg-tertiary text-gx-text-muted border border-gx-border-default">
											{cap}
										</span>
									{/each}
								</div>
							</div>
							<div class="flex items-center gap-1 shrink-0">
								<button class="p-1.5 text-gx-text-muted hover:text-gx-neon transition-colors rounded-gx hover:bg-gx-bg-hover" title="Start agent">
									<Play size={14} />
								</button>
								<button class="p-1.5 text-gx-text-muted hover:text-gx-status-warning transition-colors rounded-gx hover:bg-gx-bg-hover" title="Pause agent">
									<Pause size={14} />
								</button>
							</div>
						</div>
					</div>
				{/each}
			</div>

		{:else if activeTab === 'topology'}
			<!-- Agent Topology Graph -->
			<div class="flex flex-col items-center justify-center h-full gap-6">
				<div class="relative">
					<!-- Central orchestrator node -->
					<div class="w-24 h-24 rounded-full bg-gx-neon/10 border-2 border-gx-neon flex items-center justify-center shadow-gx-glow-sm">
						<div class="text-center">
							<Brain size={24} class="text-gx-neon mx-auto" />
							<span class="text-[10px] text-gx-neon mt-1 block">Orchestrator</span>
						</div>
					</div>

					<!-- Agent nodes positioned around center -->
					{#each agents.filter(a => a.role !== 'orchestrator').slice(0, 5) as agent, i}
						{@const angle = (i / 5) * 2 * Math.PI - Math.PI / 2}
						{@const x = Math.cos(angle) * 140}
						{@const y = Math.sin(angle) * 140}
						<div
							class="absolute w-16 h-16 rounded-full bg-gx-bg-elevated border border-gx-border-default flex items-center justify-center"
							style="left: calc(50% + {x}px - 32px); top: calc(50% + {y}px - 32px);"
						>
							<div class="text-center">
								<Bot size={16} class={statusColor(agent.status)} />
								<span class="text-[8px] text-gx-text-muted block mt-0.5 truncate w-14">{agent.name.split(' ')[0]}</span>
							</div>
						</div>
						<!-- Connection line -->
						<svg class="absolute inset-0 pointer-events-none" style="left: calc(50% - 12px); top: calc(50% - 12px);">
							<line x1="12" y1="12" x2={x + 12} y2={y + 12}
								stroke="var(--color-gx-border-default)" stroke-width="1" stroke-dasharray="4 4" />
						</svg>
					{/each}
				</div>

				<p class="text-sm text-gx-text-muted mt-16">
					Agent topology showing {agents.length} agents connected to the Orchestrator
				</p>
			</div>

		{:else if activeTab === 'metrics'}
			<!-- NeuralSwarm Metrics -->
			<div class="space-y-4">
				<h2 class="text-lg font-semibold text-gx-text-primary">NeuralSwarm Metrics</h2>

				<div class="grid grid-cols-3 gap-3">
					<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4">
						<div class="text-xs text-gx-text-muted mb-1">Total Agents</div>
						<div class="text-2xl font-bold text-gx-neon">{agents.length}</div>
					</div>
					<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4">
						<div class="text-xs text-gx-text-muted mb-1">Tasks Completed</div>
						<div class="text-2xl font-bold text-gx-text-primary">0</div>
					</div>
					<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4">
						<div class="text-xs text-gx-text-muted mb-1">Avg Response</div>
						<div class="text-2xl font-bold text-gx-text-primary">--ms</div>
					</div>
				</div>

				<!-- System Resources -->
				{#if system.stats}
					<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4 space-y-3">
						<h3 class="text-sm font-medium text-gx-text-secondary flex items-center gap-2">
							<Cpu size={14} class="text-gx-neon" />
							Hardware Utilization
						</h3>
						<div class="grid grid-cols-2 gap-4 text-xs">
							<div class="flex justify-between">
								<span class="text-gx-text-muted">CPU</span>
								<span class="text-gx-text-secondary font-mono">{system.stats.cpu_percent.toFixed(0)}%</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">RAM</span>
								<span class="text-gx-text-secondary font-mono">{system.stats.ram_used_gb.toFixed(1)}/{system.stats.ram_total_gb.toFixed(0)}G</span>
							</div>
							{#if system.stats.gpu_vram_used_mb != null}
								<div class="flex justify-between">
									<span class="text-gx-text-muted">GPU VRAM</span>
									<span class="text-gx-text-secondary font-mono">{(system.stats.gpu_vram_used_mb / 1024).toFixed(1)}/{((system.stats.gpu_vram_total_mb ?? 0) / 1024).toFixed(0)}G</span>
								</div>
								{#if system.stats.gpu_temp_c != null}
									<div class="flex justify-between">
										<span class="text-gx-text-muted">GPU Temp</span>
										<span class="text-gx-text-secondary font-mono">{system.stats.gpu_temp_c.toFixed(0)}°C</span>
									</div>
								{/if}
							{/if}
						</div>
					</div>
				{/if}

				<!-- Router Stats -->
				<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4 space-y-3">
					<h3 class="text-sm font-medium text-gx-text-secondary flex items-center gap-2">
						<Zap size={14} class="text-gx-accent-magenta" />
						Router Statistics
					</h3>
					<div class="grid grid-cols-2 gap-4 text-xs">
						<div class="flex justify-between">
							<span class="text-gx-text-muted">Mode</span>
							<span class="text-gx-neon">Intelligent Auto</span>
						</div>
						<div class="flex justify-between">
							<span class="text-gx-text-muted">API Cost</span>
							<span class="text-gx-neon font-mono">$0.00</span>
						</div>
						<div class="flex justify-between">
							<span class="text-gx-text-muted">Free Models Used</span>
							<span class="text-gx-text-secondary">0</span>
						</div>
						<div class="flex justify-between">
							<span class="text-gx-text-muted">Local Models Used</span>
							<span class="text-gx-text-secondary">0</span>
						</div>
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>
