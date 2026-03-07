<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogFooter,
		DialogDescription
	} from '$lib/components/ui/dialog';
	import {
		Brain,
		Network,
		Zap,
		Bug,
		Search,
		PenTool,
		Shield,
		Workflow,
		Plus,
		Edit,
		Trash2,
		Bot,
		Activity,
		Circle,
		RefreshCw,
		X,
		AlertTriangle,
		Code2,
		ArrowLeft,
		BookOpen
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';

	type AgentRole = 'orchestrator' | 'coder' | 'debugger' | 'researcher' | 'writer' | 'reviewer' | 'architect' | string;

	interface AgentConfig {
		id: string;
		name: string;
		role: AgentRole;
		model_id: string;
		system_prompt: string;
		enabled: boolean;
		temperature: number;
		max_tokens: number;
	}

	interface AgentView {
		id: string;
		name: string;
		role: AgentRole;
		status: 'idle' | 'active' | 'error';
		model: string;
		description: string;
		capabilities: string[];
	}

	const ROLE_PRESETS: Record<string, { description: string; capabilities: string[]; icon: typeof Brain; defaultModel: string }> = {
		orchestrator: {
			description: 'Coordinates multi-agent workflows and decomposes objectives into tasks',
			capabilities: ['task-decomposition', 'agent-routing', 'priority-management', 'workflow-coordination'],
			icon: Brain,
			defaultModel: 'hermes-3:8b'
		},
		coder: {
			description: 'Generates, reviews, and refactors code across languages',
			capabilities: ['code-generation', 'refactoring', 'testing', 'debugging'],
			icon: Code2,
			defaultModel: 'qwen2.5-coder:7b'
		},
		debugger: {
			description: 'Analyzes errors, traces root causes, and suggests fixes',
			capabilities: ['error-analysis', 'stack-trace', 'root-cause', 'fix-suggestion'],
			icon: Bug,
			defaultModel: 'mistralai/devstral-small:free'
		},
		researcher: {
			description: 'Searches documentation, papers, and web for relevant information',
			capabilities: ['web-search', 'paper-analysis', 'documentation', 'summarization'],
			icon: Search,
			defaultModel: 'meta-llama/llama-4-scout:free'
		},
		writer: {
			description: 'Creates technical documentation, guides, and content',
			capabilities: ['technical-writing', 'documentation', 'tutorials', 'api-docs'],
			icon: PenTool,
			defaultModel: 'hermes-3:8b'
		},
		reviewer: {
			description: 'Reviews code quality, security, and best practices compliance',
			capabilities: ['code-review', 'security-audit', 'best-practices', 'performance-review'],
			icon: Shield,
			defaultModel: 'qwen2.5-coder:7b'
		},
		architect: {
			description: 'Designs system architecture, API contracts, and data models',
			capabilities: ['system-design', 'api-design', 'data-modeling', 'scalability'],
			icon: Workflow,
			defaultModel: 'hermes-3:8b'
		}
	};

	const ROLE_LIST = Object.keys(ROLE_PRESETS) as AgentRole[];

	// Built-in defaults
	const builtInAgents: AgentView[] = [
		{ id: 'orchestrator', name: 'Master Orchestrator', role: 'orchestrator', status: 'idle', model: 'hermes-3:8b', description: ROLE_PRESETS.orchestrator.description, capabilities: ROLE_PRESETS.orchestrator.capabilities },
		{ id: 'coder', name: 'Code Agent', role: 'coder', status: 'idle', model: 'qwen2.5-coder:7b', description: ROLE_PRESETS.coder.description, capabilities: ROLE_PRESETS.coder.capabilities },
		{ id: 'researcher', name: 'Research Agent', role: 'researcher', status: 'idle', model: 'meta-llama/llama-4-scout:free', description: ROLE_PRESETS.researcher.description, capabilities: ROLE_PRESETS.researcher.capabilities },
		{ id: 'debugger', name: 'Debug Agent', role: 'debugger', status: 'idle', model: 'mistralai/devstral-small:free', description: ROLE_PRESETS.debugger.description, capabilities: ROLE_PRESETS.debugger.capabilities },
		{ id: 'reviewer', name: 'Review Agent', role: 'reviewer', status: 'idle', model: 'qwen2.5-coder:7b', description: ROLE_PRESETS.reviewer.description, capabilities: ROLE_PRESETS.reviewer.capabilities },
		{ id: 'architect', name: 'Architect Agent', role: 'architect', status: 'idle', model: 'hermes-3:8b', description: ROLE_PRESETS.architect.description, capabilities: ROLE_PRESETS.architect.capabilities },
	];

	let agents = $state<AgentView[]>([]);
	let loading = $state(true);
	let activeTab = $state<'pool' | 'topology'>('pool');
	let errorMessage = $state<string | null>(null);

	// Dialog state
	let showCreateDialog = $state(false);
	let showEditDialog = $state(false);
	let editingAgent = $state<AgentView | null>(null);
	let confirmDelete = $state<string | null>(null);

	// Form state
	let formName = $state('');
	let formRole = $state<AgentRole>('coder');
	let formModel = $state('');
	let formDescription = $state('');

	let activeCount = $derived(agents.filter((a) => a.status === 'active').length);
	let idleCount = $derived(agents.filter((a) => a.status === 'idle').length);
	let errorCount = $derived(agents.filter((a) => a.status === 'error').length);

	let roleDistribution = $derived(() => {
		const dist: Record<string, number> = {};
		for (const a of agents) {
			dist[a.role] = (dist[a.role] ?? 0) + 1;
		}
		return dist;
	});

	function configToView(cfg: AgentConfig): AgentView {
		const preset = ROLE_PRESETS[typeof cfg.role === 'string' ? cfg.role : ''];
		return {
			id: cfg.id,
			name: cfg.name,
			role: typeof cfg.role === 'string' ? cfg.role : 'coder',
			status: cfg.enabled ? 'idle' : 'error',
			model: cfg.model_id,
			description: preset?.description ?? cfg.system_prompt.slice(0, 100),
			capabilities: preset?.capabilities ?? []
		};
	}

	async function loadAgents() {
		loading = true;
		try {
			const serverAgents = await invoke<AgentConfig[]>('list_agents');
			if (serverAgents.length > 0) {
				agents = serverAgents.map(configToView);
			} else {
				agents = builtInAgents;
			}
		} catch {
			agents = builtInAgents;
		}
		loading = false;
	}

	function openCreateDialog() {
		formName = '';
		formRole = 'coder';
		formModel = ROLE_PRESETS.coder.defaultModel;
		formDescription = '';
		showCreateDialog = true;
	}

	function openEditDialog(agent: AgentView) {
		editingAgent = agent;
		formName = agent.name;
		formRole = agent.role;
		formModel = agent.model;
		formDescription = agent.description;
		showEditDialog = true;
	}

	function onRoleChange(role: AgentRole) {
		formRole = role;
		const preset = ROLE_PRESETS[role];
		if (preset) {
			formModel = preset.defaultModel;
			if (!formDescription || Object.values(ROLE_PRESETS).some((p) => p.description === formDescription)) {
				formDescription = preset.description;
			}
		}
	}

	async function createAgent() {
		if (!formName.trim()) return;
		errorMessage = null;
		const id = formName.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
		try {
			await invoke('create_agent', {
				id,
				name: formName.trim(),
				role: formRole,
				model_id: formModel || ROLE_PRESETS[formRole]?.defaultModel || 'hermes-3:8b',
				system_prompt: formDescription || ROLE_PRESETS[formRole]?.description || ''
			});
			showCreateDialog = false;
			await loadAgents();
		} catch (e) {
			// If backend doesn't support it, add locally
			const preset = ROLE_PRESETS[formRole];
			agents = [
				...agents,
				{
					id,
					name: formName.trim(),
					role: formRole,
					status: 'idle',
					model: formModel || preset?.defaultModel || 'hermes-3:8b',
					description: formDescription || preset?.description || '',
					capabilities: preset?.capabilities ?? []
				}
			];
			showCreateDialog = false;
		}
	}

	async function saveEditAgent() {
		if (!editingAgent || !formName.trim()) return;
		errorMessage = null;
		try {
			await invoke('update_agent', {
				id: editingAgent.id,
				name: formName.trim(),
				model_id: formModel,
				system_prompt: formDescription
			});
			showEditDialog = false;
			editingAgent = null;
			await loadAgents();
		} catch {
			// Update locally
			agents = agents.map((a) =>
				a.id === editingAgent?.id
					? {
							...a,
							name: formName.trim(),
							role: formRole,
							model: formModel,
							description: formDescription,
							capabilities: ROLE_PRESETS[formRole]?.capabilities ?? a.capabilities
						}
					: a
			);
			showEditDialog = false;
			editingAgent = null;
		}
	}

	async function deleteAgent(id: string) {
		if (confirmDelete !== id) {
			confirmDelete = id;
			return;
		}
		confirmDelete = null;
		errorMessage = null;
		try {
			await invoke('delete_agent', { id });
		} catch {
			// Remove locally if backend fails
		}
		agents = agents.filter((a) => a.id !== id);
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'active':
				return 'text-gx-status-success';
			case 'error':
				return 'text-gx-status-error';
			default:
				return 'text-gx-text-muted';
		}
	}

	function statusBadge(status: string): { cls: string; label: string } {
		switch (status) {
			case 'active':
				return { cls: 'border-green-500/50 text-green-400 bg-green-500/10', label: 'Active' };
			case 'error':
				return { cls: 'border-red-500/50 text-red-400 bg-red-500/10', label: 'Error' };
			default:
				return { cls: 'border-gx-border-default text-gx-text-muted', label: 'Idle' };
		}
	}

	function getRoleIcon(role: string): typeof Brain {
		return ROLE_PRESETS[role]?.icon ?? Bot;
	}

	onMount(loadAgents);
</script>

<main class="flex flex-col h-screen bg-gx-bg-primary">
	<!-- Header -->
	<header
		class="h-14 border-b border-gx-border-default bg-gx-bg-secondary flex items-center px-4 gap-3 shrink-0"
	>
		<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">
			<ArrowLeft size={18} />
		</a>
		<Network class="w-5 h-5 text-gx-neon" />
		<h1 class="text-lg font-semibold text-gx-text-primary">NeuralSwarm Agents</h1>
		<Badge variant="outline" class="text-[10px] px-1.5 py-0 border-gx-neon/30 text-gx-neon">
			NeuralSwarm
		</Badge>
		<div class="flex-1"></div>
		<Button variant="outline" size="sm" onclick={openCreateDialog} class="text-xs h-7">
			<Plus class="w-3.5 h-3.5" />
			Create Agent
		</Button>
		<button
			onclick={loadAgents}
			class="text-gx-text-muted hover:text-gx-neon transition-colors p-1.5 rounded-gx hover:bg-gx-bg-tertiary"
			title="Refresh"
		>
			<RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
		</button>
	</header>

	<!-- Tab Bar -->
	<div
		class="flex items-center gap-1 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0"
	>
		<button
			onclick={() => (activeTab = 'pool')}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'pool'
				? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
				: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Bot size={13} />
			Agent Pool
		</button>
		<button
			onclick={() => (activeTab = 'topology')}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'topology'
				? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
				: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Network size={13} />
			Topology
		</button>

		<div class="flex-1"></div>

		<!-- Stats Summary -->
		<div class="flex items-center gap-3 text-xs text-gx-text-muted">
			<span class="flex items-center gap-1">
				<Bot size={12} />
				{agents.length} total
			</span>
			<span class="flex items-center gap-1">
				<Circle size={8} class="text-green-400 fill-green-400" />
				{activeCount} active
			</span>
			<span class="flex items-center gap-1">
				<Circle size={8} class="text-gx-text-muted fill-gx-text-muted" />
				{idleCount} idle
			</span>
			{#if errorCount > 0}
				<span class="flex items-center gap-1">
					<Circle size={8} class="text-red-400 fill-red-400" />
					{errorCount} error
				</span>
			{/if}
		</div>
	</div>

	<!-- Error Banner -->
	{#if errorMessage}
		<div
			class="mx-4 mt-2 flex items-center gap-3 p-3 rounded-gx bg-red-500/10 border border-red-500/30 text-red-400 text-sm"
		>
			<AlertTriangle class="w-4 h-4 shrink-0" />
			<span class="flex-1">{errorMessage}</span>
			<button
				onclick={() => (errorMessage = null)}
				class="p-1 hover:bg-red-500/20 rounded transition-colors"
			>
				<X class="w-3 h-3" />
			</button>
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 overflow-y-auto p-4">
		{#if loading}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
				{#each Array(6) as _}
					<div class="h-40 rounded-gx-lg bg-gx-bg-tertiary animate-pulse"></div>
				{/each}
			</div>
		{:else if activeTab === 'pool'}
			<!-- Agent Pool Grid -->
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
				{#each agents as agent (agent.id)}
					{@const RoleIcon = getRoleIcon(agent.role)}
					<Card
						class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-colors"
					>
						<CardHeader class="pb-2">
							<div class="flex items-start justify-between">
								<div class="flex items-center gap-2.5">
									<div
										class="w-9 h-9 rounded-gx bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shrink-0"
									>
										<RoleIcon size={16} class={statusColor(agent.status)} />
									</div>
									<div class="min-w-0">
										<CardTitle class="text-sm truncate">{agent.name}</CardTitle>
										<div class="flex items-center gap-1.5 mt-0.5">
											<Badge
												variant="outline"
												class="text-[9px] px-1 py-0 h-3.5 {statusBadge(agent.status).cls}"
											>
												{statusBadge(agent.status).label}
											</Badge>
											<Badge
												variant="outline"
												class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted"
											>
												{agent.role}
											</Badge>
										</div>
									</div>
								</div>
								<div class="flex items-center gap-0.5 shrink-0">
									<button
										class="p-1.5 text-gx-text-muted hover:text-gx-neon transition-colors rounded-gx hover:bg-gx-bg-hover"
										title="Edit agent"
										onclick={() => openEditDialog(agent)}
									>
										<Edit size={13} />
									</button>
									{#if confirmDelete === agent.id}
										<button
											class="px-2 py-1 rounded text-[10px] font-medium bg-red-500/20 text-red-400 border border-red-500/40 hover:bg-red-500/30 transition-colors"
											onclick={() => deleteAgent(agent.id)}
										>
											Confirm
										</button>
										<button
											class="p-1 rounded hover:bg-gx-bg-tertiary text-gx-text-muted transition-colors"
											onclick={() => (confirmDelete = null)}
										>
											<X size={12} />
										</button>
									{:else}
										<button
											class="p-1.5 text-gx-text-muted hover:text-red-400 transition-colors rounded-gx hover:bg-red-500/10"
											title="Delete agent"
											onclick={() => deleteAgent(agent.id)}
										>
											<Trash2 size={13} />
										</button>
									{/if}
								</div>
							</div>
						</CardHeader>
						<CardContent>
							<p class="text-xs text-gx-text-muted mb-2.5 line-clamp-2">
								{agent.description}
							</p>
							<div class="flex items-center gap-1.5 mb-2.5 text-xs text-gx-text-muted">
								<Zap size={11} class="text-gx-neon" />
								<span class="font-mono text-[11px] text-gx-text-secondary truncate">{agent.model}</span>
							</div>
							<div class="flex flex-wrap gap-1">
								{#each agent.capabilities as cap}
									<span
										class="text-[10px] px-1.5 py-0.5 rounded bg-gx-bg-tertiary text-gx-text-muted border border-gx-border-default"
									>
										{cap}
									</span>
								{/each}
							</div>
						</CardContent>
					</Card>
				{/each}
			</div>
		{:else if activeTab === 'topology'}
			<!-- Topology Visualization -->
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<svg
					width="500"
					height="500"
					viewBox="-250 -250 500 500"
					class="max-w-full max-h-full"
				>
					<!-- Connection lines from center to each agent -->
					{#each agents.filter((a) => a.role !== 'orchestrator') as agent, i}
						{@const count = agents.filter((a) => a.role !== 'orchestrator').length}
						{@const angle = (i / count) * 2 * Math.PI - Math.PI / 2}
						{@const x = Math.cos(angle) * 160}
						{@const y = Math.sin(angle) * 160}
						<line
							x1="0"
							y1="0"
							x2={x}
							y2={y}
							stroke="var(--color-gx-border-default, #2a2a35)"
							stroke-width="1.5"
							stroke-dasharray="6 4"
							opacity="0.6"
						/>
						<!-- Animated dot on the line -->
						<circle r="2" fill="var(--color-gx-neon, #00FF66)" opacity="0.5">
							<animateMotion
								dur="{3 + i * 0.5}s"
								repeatCount="indefinite"
								path="M0,0 L{x},{y}"
							/>
						</circle>
					{/each}

					<!-- Central orchestrator node -->
					<circle
						cx="0"
						cy="0"
						r="45"
						fill="none"
						stroke="var(--color-gx-neon, #00FF66)"
						stroke-width="2"
						opacity="0.8"
					/>
					<circle
						cx="0"
						cy="0"
						r="45"
						fill="var(--color-gx-neon, #00FF66)"
						opacity="0.08"
					/>
					<!-- Glow ring -->
					<circle
						cx="0"
						cy="0"
						r="50"
						fill="none"
						stroke="var(--color-gx-neon, #00FF66)"
						stroke-width="1"
						opacity="0.2"
					/>

					<!-- Orchestrator label -->
					<text
						x="0"
						y="-8"
						text-anchor="middle"
						fill="var(--color-gx-neon, #00FF66)"
						font-size="12"
						font-weight="600">Orchestrator</text
					>
					<text
						x="0"
						y="8"
						text-anchor="middle"
						fill="var(--color-gx-text-muted, #666)"
						font-size="9"
					>
						{agents.find((a) => a.role === 'orchestrator')?.model ?? 'hermes-3:8b'}
					</text>

					<!-- Agent nodes -->
					{#each agents.filter((a) => a.role !== 'orchestrator') as agent, i}
						{@const count = agents.filter((a) => a.role !== 'orchestrator').length}
						{@const angle = (i / count) * 2 * Math.PI - Math.PI / 2}
						{@const x = Math.cos(angle) * 160}
						{@const y = Math.sin(angle) * 160}
						{@const nodeColor =
							agent.status === 'active'
								? 'var(--color-gx-status-success, #22c55e)'
								: agent.status === 'error'
									? 'var(--color-gx-status-error, #ef4444)'
									: 'var(--color-gx-border-default, #2a2a35)'}

						<circle
							cx={x}
							cy={y}
							r="30"
							fill="var(--color-gx-bg-elevated, #1a1a22)"
							stroke={nodeColor}
							stroke-width="1.5"
						/>
						<text
							x={x}
							y={y - 5}
							text-anchor="middle"
							fill="var(--color-gx-text-primary, #e0e0e0)"
							font-size="9"
							font-weight="500"
						>
							{agent.name.split(' ')[0]}
						</text>
						<text
							x={x}
							y={y + 7}
							text-anchor="middle"
							fill="var(--color-gx-text-muted, #666)"
							font-size="7"
						>
							{agent.role}
						</text>
						<!-- Status indicator dot -->
						<circle
							cx={x + 22}
							cy={y - 22}
							r="4"
							fill={nodeColor}
						/>
					{/each}
				</svg>

				<p class="text-sm text-gx-text-muted">
					Agent topology: {agents.length} agents connected to the central Orchestrator
				</p>
			</div>
		{/if}
	</div>
</main>

<!-- Create Agent Dialog -->
<Dialog bind:open={showCreateDialog}>
	<DialogContent class="bg-gx-bg-secondary border-gx-border-default max-w-md">
		<DialogHeader>
			<DialogTitle class="text-gx-text-primary">Create New Agent</DialogTitle>
			<DialogDescription class="text-gx-text-muted text-sm">
				Configure a new agent for your NeuralSwarm.
			</DialogDescription>
		</DialogHeader>
		<div class="space-y-4 py-2">
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="agent-name">Name</label>
				<Input
					id="agent-name"
					bind:value={formName}
					placeholder="e.g. Security Auditor"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted"
				/>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="agent-role">Role</label>
				<div class="grid grid-cols-4 gap-1.5">
					{#each ROLE_LIST as role}
						{@const RIcon = ROLE_PRESETS[role].icon}
						<button
							onclick={() => onRoleChange(role)}
							class="flex flex-col items-center gap-1 p-2 rounded-gx border text-xs transition-all
								{formRole === role
								? 'border-gx-neon/50 bg-gx-neon/10 text-gx-neon'
								: 'border-gx-border-default text-gx-text-muted hover:border-gx-border-hover hover:bg-gx-bg-hover'}"
						>
							<RIcon size={14} />
							<span class="text-[10px] capitalize">{role}</span>
						</button>
					{/each}
				</div>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="agent-model">Model</label>
				<Input
					id="agent-model"
					bind:value={formModel}
					placeholder="e.g. hermes-3:8b"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted font-mono text-sm"
				/>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="agent-desc">Description</label>
				<textarea
					id="agent-desc"
					bind:value={formDescription}
					placeholder="Describe the agent's purpose..."
					rows="3"
					class="w-full rounded-gx border border-gx-border-default bg-gx-bg-tertiary text-gx-text-primary text-sm p-2 placeholder:text-gx-text-muted resize-none focus:outline-none focus:border-gx-neon/50"
				></textarea>
			</div>
		</div>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showCreateDialog = false)}>Cancel</Button>
			<Button onclick={createAgent} disabled={!formName.trim()}>
				<Plus class="w-3.5 h-3.5" />
				Create Agent
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Edit Agent Dialog -->
<Dialog bind:open={showEditDialog}>
	<DialogContent class="bg-gx-bg-secondary border-gx-border-default max-w-md">
		<DialogHeader>
			<DialogTitle class="text-gx-text-primary">Edit Agent</DialogTitle>
			<DialogDescription class="text-gx-text-muted text-sm">
				Modify the agent configuration.
			</DialogDescription>
		</DialogHeader>
		<div class="space-y-4 py-2">
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="edit-name">Name</label>
				<Input
					id="edit-name"
					bind:value={formName}
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary"
				/>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="edit-role">Role</label>
				<div class="grid grid-cols-4 gap-1.5">
					{#each ROLE_LIST as role}
						{@const RIcon = ROLE_PRESETS[role].icon}
						<button
							onclick={() => onRoleChange(role)}
							class="flex flex-col items-center gap-1 p-2 rounded-gx border text-xs transition-all
								{formRole === role
								? 'border-gx-neon/50 bg-gx-neon/10 text-gx-neon'
								: 'border-gx-border-default text-gx-text-muted hover:border-gx-border-hover hover:bg-gx-bg-hover'}"
						>
							<RIcon size={14} />
							<span class="text-[10px] capitalize">{role}</span>
						</button>
					{/each}
				</div>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="edit-model">Model</label>
				<Input
					id="edit-model"
					bind:value={formModel}
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary font-mono text-sm"
				/>
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="edit-desc">Description</label>
				<textarea
					id="edit-desc"
					bind:value={formDescription}
					rows="3"
					class="w-full rounded-gx border border-gx-border-default bg-gx-bg-tertiary text-gx-text-primary text-sm p-2 placeholder:text-gx-text-muted resize-none focus:outline-none focus:border-gx-neon/50"
				></textarea>
			</div>
		</div>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showEditDialog = false)}>Cancel</Button>
			<Button onclick={saveEditAgent} disabled={!formName.trim()}>
				<Edit class="w-3.5 h-3.5" />
				Save Changes
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
