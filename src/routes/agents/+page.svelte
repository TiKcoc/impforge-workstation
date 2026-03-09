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
		BookOpen,
		Play,
		Square,
		RotateCw,
		ScrollText,
		Clock,
		Heart,
		Timer,
		TrendingUp,
		CheckCircle,
		XCircle,
		Gauge,
		Server
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-agents';
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
	let agentsHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let agentsHeaderStyle = $derived(hasEngineStyle && agentsHeaderComponent ? componentToCSS(agentsHeaderComponent) : '');

	// =====================================================================
	// TYPES
	// =====================================================================

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

	interface OrchestratorStatus {
		running: boolean;
		task_count: number;
		tasks_ok: number;
		tasks_fail: number;
		uptime_seconds: number;
		avg_trust: number;
	}

	interface TaskStatus {
		name: string;
		status: string;
		duration_ms: number | null;
		trust: number;
		last_run: string | null;
		pool: string;
	}

	interface ServiceHealth {
		name: string;
		status: string;
		endpoint: string | null;
	}

	interface NeuralSwarmSnapshot {
		status: OrchestratorStatus;
		tasks: TaskStatus[];
		services: ServiceHealth[];
	}

	// =====================================================================
	// AGENT PRESETS
	// =====================================================================

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

	const builtInAgents: AgentView[] = [
		{ id: 'orchestrator', name: 'Master Orchestrator', role: 'orchestrator', status: 'idle', model: 'hermes-3:8b', description: ROLE_PRESETS.orchestrator.description, capabilities: ROLE_PRESETS.orchestrator.capabilities },
		{ id: 'coder', name: 'Code Agent', role: 'coder', status: 'idle', model: 'qwen2.5-coder:7b', description: ROLE_PRESETS.coder.description, capabilities: ROLE_PRESETS.coder.capabilities },
		{ id: 'researcher', name: 'Research Agent', role: 'researcher', status: 'idle', model: 'meta-llama/llama-4-scout:free', description: ROLE_PRESETS.researcher.description, capabilities: ROLE_PRESETS.researcher.capabilities },
		{ id: 'debugger', name: 'Debug Agent', role: 'debugger', status: 'idle', model: 'mistralai/devstral-small:free', description: ROLE_PRESETS.debugger.description, capabilities: ROLE_PRESETS.debugger.capabilities },
		{ id: 'reviewer', name: 'Review Agent', role: 'reviewer', status: 'idle', model: 'qwen2.5-coder:7b', description: ROLE_PRESETS.reviewer.description, capabilities: ROLE_PRESETS.reviewer.capabilities },
		{ id: 'architect', name: 'Architect Agent', role: 'architect', status: 'idle', model: 'hermes-3:8b', description: ROLE_PRESETS.architect.description, capabilities: ROLE_PRESETS.architect.capabilities },
	];

	// =====================================================================
	// STATE
	// =====================================================================

	// Tabs
	let activeTab = $state<'orchestrator' | 'pool' | 'topology'>('orchestrator');

	// Agent Pool state
	let agents = $state<AgentView[]>([]);
	let loading = $state(true);
	let errorMessage = $state<string | null>(null);

	// Agent dialogs
	let showCreateDialog = $state(false);
	let showEditDialog = $state(false);
	let editingAgent = $state<AgentView | null>(null);
	let confirmDelete = $state<string | null>(null);
	let formName = $state('');
	let formRole = $state<AgentRole>('coder');
	let formModel = $state('');
	let formDescription = $state('');

	// Orchestrator state
	let snapshot = $state<NeuralSwarmSnapshot | null>(null);
	let orchLoading = $state(true);
	let orchError = $state<string | null>(null);
	let showLogs = $state(false);
	let logContent = $state('');
	let orchActionLoading = $state<string | null>(null);
	let orchPollInterval = $state<ReturnType<typeof setInterval> | null>(null);

	// Derived
	let activeCount = $derived(agents.filter((a) => a.status === 'active').length);
	let idleCount = $derived(agents.filter((a) => a.status === 'idle').length);
	let errorCount = $derived(agents.filter((a) => a.status === 'error').length);

	let orchIsActive = $derived(snapshot?.status.running ?? false);
	let taskOkCount = $derived(snapshot?.status.tasks_ok ?? 0);
	let taskFailCount = $derived(snapshot?.status.tasks_fail ?? 0);
	let avgTrust = $derived(() => {
		return snapshot?.status.avg_trust ?? 0;
	});

	// =====================================================================
	// ORCHESTRATOR FUNCTIONS
	// =====================================================================

	async function fetchSnapshot() {
		orchLoading = true;
		orchError = null;
		try {
			snapshot = await invoke<NeuralSwarmSnapshot>('neuralswarm_snapshot');
		} catch (e) {
			orchError = `Failed to fetch orchestrator data: ${e}`;
		}
		orchLoading = false;
	}

	async function fetchLogs() {
		try {
			logContent = await invoke<string>('neuralswarm_logs', { lines: 200 });
			showLogs = true;
		} catch (e) {
			orchError = `Failed to fetch logs: ${e}`;
		}
	}

	async function orchAction(action: string) {
		orchActionLoading = action;
		orchError = null;
		try {
			await invoke<string>('neuralswarm_action', { action });
			// Wait a moment for service state to change
			await new Promise(r => setTimeout(r, 1500));
			await fetchSnapshot();
		} catch (e) {
			orchError = `Action "${action}" failed: ${e}`;
		}
		orchActionLoading = null;
	}

	function startOrchPolling() {
		fetchSnapshot();
		orchPollInterval = setInterval(fetchSnapshot, 10000);
	}

	function stopOrchPolling() {
		if (orchPollInterval) {
			clearInterval(orchPollInterval);
			orchPollInterval = null;
		}
	}

	function formatUptime(seconds: number | null | undefined): string {
		if (!seconds) return '--';
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		if (h > 0) return `${h}h ${m}m`;
		return `${m}m`;
	}

	function trustColor(trust: number | null): string {
		if (trust == null) return 'text-gx-text-muted';
		if (trust >= 0.8) return 'text-green-400';
		if (trust >= 0.5) return 'text-gx-neon';
		if (trust >= 0.3) return 'text-yellow-400';
		return 'text-red-400';
	}

	function taskStatusBadge(status: string): { cls: string; label: string } {
		switch (status) {
			case 'OK':
				return { cls: 'border-green-500/50 text-green-400 bg-green-500/10', label: 'OK' };
			case 'FAIL':
				return { cls: 'border-red-500/50 text-red-400 bg-red-500/10', label: 'FAIL' };
			case 'SKIP':
				return { cls: 'border-yellow-500/50 text-yellow-400 bg-yellow-500/10', label: 'SKIP' };
			default:
				return { cls: 'border-gx-border-default text-gx-text-muted', label: status };
		}
	}

	// =====================================================================
	// AGENT POOL FUNCTIONS
	// =====================================================================

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
			agents = serverAgents.length > 0 ? serverAgents.map(configToView) : builtInAgents;
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
		} catch {
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
		} catch { /* Remove locally */ }
		agents = agents.filter((a) => a.id !== id);
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'active': return 'text-gx-status-success';
			case 'error': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusBadge(status: string): { cls: string; label: string } {
		switch (status) {
			case 'active': return { cls: 'border-green-500/50 text-green-400 bg-green-500/10', label: 'Active' };
			case 'error': return { cls: 'border-red-500/50 text-red-400 bg-red-500/10', label: 'Error' };
			default: return { cls: 'border-gx-border-default text-gx-text-muted', label: 'Idle' };
		}
	}

	function getRoleIcon(role: string): typeof Brain {
		return ROLE_PRESETS[role]?.icon ?? Bot;
	}

	// =====================================================================
	// LIFECYCLE
	// =====================================================================

	onMount(() => {
		loadAgents();
		startOrchPolling();
		return () => stopOrchPolling();
	});
</script>

<main class="flex flex-col h-screen {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Header -->
	<header class="h-14 border-b border-gx-border-default {hasEngineStyle && agentsHeaderComponent ? '' : 'bg-gx-bg-secondary'} flex items-center px-4 gap-3 shrink-0" style={agentsHeaderStyle}>
		<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">
			<ArrowLeft size={18} />
		</a>
		<Network class="w-5 h-5 text-gx-neon" />
		<h1 class="text-lg font-semibold text-gx-text-primary">NeuralSwarm</h1>
		<Badge variant="outline" class="text-[10px] px-1.5 py-0 border-gx-neon/30 text-gx-neon">
			{orchIsActive ? 'Active' : 'Offline'}
		</Badge>
		{#if snapshot?.status.task_count}
			<Badge variant="outline" class="text-[10px] px-1.5 py-0 border-gx-border-default text-gx-text-muted">
				{snapshot.status.task_count} tasks
			</Badge>
		{/if}
		<div class="flex-1"></div>
		{#if activeTab === 'pool'}
			<Button variant="outline" size="sm" onclick={openCreateDialog} class="text-xs h-7">
				<Plus class="w-3.5 h-3.5" />
				Create Agent
			</Button>
		{/if}
		<button
			onclick={() => { if (activeTab === 'orchestrator') fetchSnapshot(); else loadAgents(); }}
			class="text-gx-text-muted hover:text-gx-neon transition-colors p-1.5 rounded-gx hover:bg-gx-bg-tertiary"
			title="Refresh"
		>
			<RefreshCw size={16} class={(orchLoading || loading) ? 'animate-spin' : ''} />
		</button>
	</header>

	<!-- Tab Bar -->
	<div class="flex items-center gap-1 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
		<button
			onclick={() => (activeTab = 'orchestrator')}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activeTab === 'orchestrator'
				? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
				: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Activity size={13} />
			Orchestrator
		</button>
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

		{#if activeTab === 'orchestrator'}
			<div class="flex items-center gap-3 text-xs text-gx-text-muted">
				<span class="flex items-center gap-1">
					<CheckCircle size={12} class="text-green-400" />
					{taskOkCount} OK
				</span>
				{#if taskFailCount > 0}
					<span class="flex items-center gap-1">
						<XCircle size={12} class="text-red-400" />
						{taskFailCount} FAIL
					</span>
				{/if}
				<span class="flex items-center gap-1">
					<TrendingUp size={12} class="text-gx-neon" />
					Trust {(avgTrust() * 100).toFixed(0)}%
				</span>
			</div>
		{:else}
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
			</div>
		{/if}
	</div>

	<!-- Error Banner -->
	{#if errorMessage || orchError}
		<div class="mx-4 mt-2 flex items-center gap-3 p-3 rounded-gx bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
			<AlertTriangle class="w-4 h-4 shrink-0" />
			<span class="flex-1">{errorMessage || orchError}</span>
			<button
				onclick={() => { errorMessage = null; orchError = null; }}
				class="p-1 hover:bg-red-500/20 rounded transition-colors"
			>
				<X class="w-3 h-3" />
			</button>
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 overflow-y-auto p-4">
		{#if activeTab === 'orchestrator'}
			<!-- ============================================================= -->
			<!-- ORCHESTRATOR DASHBOARD -->
			<!-- ============================================================= -->
			{#if orchLoading && !snapshot}
				<div class="grid grid-cols-1 md:grid-cols-4 gap-3">
					{#each Array(4) as _}
						<div class="h-24 rounded-gx-lg bg-gx-bg-tertiary animate-pulse"></div>
					{/each}
				</div>
				<div class="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-3 mt-3">
					{#each Array(12) as _}
						<div class="h-20 rounded-gx bg-gx-bg-tertiary animate-pulse"></div>
					{/each}
				</div>
			{:else if snapshot}
				<!-- Status Cards -->
				<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 mb-4">
					<!-- Orchestrator Status -->
					<Card class="bg-gx-bg-secondary border-gx-border-default">
						<CardHeader class="pb-2">
							<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
								<Server size={14} />
								Orchestrator
							</CardTitle>
						</CardHeader>
						<CardContent>
							<div class="flex items-center gap-2">
								<span class="w-2.5 h-2.5 rounded-full {orchIsActive ? 'bg-green-400 animate-pulse' : 'bg-gx-text-muted'}"></span>
								<span class="text-lg font-bold {orchIsActive ? 'text-green-400' : 'text-gx-text-muted'}">
									{orchIsActive ? 'Running' : 'Stopped'}
								</span>
							</div>
							<p class="text-xs text-gx-text-muted mt-1.5">Standalone AI engine</p>
						</CardContent>
					</Card>

					<!-- Uptime -->
					<Card class="bg-gx-bg-secondary border-gx-border-default">
						<CardHeader class="pb-2">
							<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
								<Clock size={14} />
								Uptime
							</CardTitle>
						</CardHeader>
						<CardContent>
							<p class="text-lg font-bold text-gx-text-primary">
								{formatUptime(snapshot.status.uptime_seconds)}
							</p>
						</CardContent>
					</Card>

					<!-- Tasks Summary -->
					<Card class="bg-gx-bg-secondary border-gx-border-default">
						<CardHeader class="pb-2">
							<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
								<Gauge size={14} />
								Tasks
							</CardTitle>
						</CardHeader>
						<CardContent>
							<p class="text-lg font-bold text-gx-text-primary">
								{snapshot.status.task_count} <span class="text-sm font-normal text-gx-text-muted">registered</span>
							</p>
							<div class="flex items-center gap-3 mt-1.5 text-xs">
								<span class="text-green-400">{taskOkCount} OK</span>
								{#if taskFailCount > 0}
									<span class="text-red-400">{taskFailCount} FAIL</span>
								{/if}
							</div>
						</CardContent>
					</Card>

					<!-- Trust Score -->
					<Card class="bg-gx-bg-secondary border-gx-border-default">
						<CardHeader class="pb-2">
							<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
								<TrendingUp size={14} />
								Avg Trust
							</CardTitle>
						</CardHeader>
						<CardContent>
							<p class="text-lg font-bold {trustColor(avgTrust())}">
								{(avgTrust() * 100).toFixed(1)}%
							</p>
							<div class="w-full h-1.5 bg-gx-bg-tertiary rounded-full mt-2 overflow-hidden">
								<div
									class="h-full rounded-full transition-all duration-500 {avgTrust() >= 0.5 ? 'bg-gx-neon' : 'bg-yellow-400'}"
									style="width: {avgTrust() * 100}%"
								></div>
							</div>
						</CardContent>
					</Card>
				</div>

				<!-- Controls -->
				<div class="flex items-center gap-2 mb-4">
					{#if orchIsActive}
						<Button
							variant="outline"
							size="sm"
							class="text-xs h-7"
							onclick={() => orchAction('restart')}
							disabled={orchActionLoading != null}
						>
							{#if orchActionLoading === 'restart'}
								<RefreshCw size={13} class="animate-spin" />
							{:else}
								<RotateCw size={13} />
							{/if}
							Restart
						</Button>
						<Button
							variant="outline"
							size="sm"
							class="text-xs h-7 text-red-400 hover:text-red-300 border-red-500/30 hover:border-red-500/50"
							onclick={() => orchAction('stop')}
							disabled={orchActionLoading != null}
						>
							{#if orchActionLoading === 'stop'}
								<RefreshCw size={13} class="animate-spin" />
							{:else}
								<Square size={13} />
							{/if}
							Stop
						</Button>
					{:else}
						<Button
							variant="outline"
							size="sm"
							class="text-xs h-7 text-green-400 hover:text-green-300 border-green-500/30 hover:border-green-500/50"
							onclick={() => orchAction('start')}
							disabled={orchActionLoading != null}
						>
							{#if orchActionLoading === 'start'}
								<RefreshCw size={13} class="animate-spin" />
							{:else}
								<Play size={13} />
							{/if}
							Start
						</Button>
					{/if}

					<Button
						variant="outline"
						size="sm"
						class="text-xs h-7"
						onclick={fetchLogs}
					>
						<ScrollText size={13} />
						Journal
					</Button>

					<div class="flex-1"></div>

					{#if orchLoading}
						<Badge variant="outline" class="text-[10px] px-1.5 py-0 border-gx-neon/30 text-gx-neon animate-pulse">
							Polling...
						</Badge>
					{/if}
				</div>

				<!-- Local Services -->
				{#if snapshot.services.length > 0}
					<div class="mb-4">
						<h3 class="text-xs font-medium text-gx-text-muted mb-2 flex items-center gap-1.5">
							<Heart size={12} />
							Local Services
						</h3>
						<div class="flex flex-wrap gap-2">
							{#each snapshot.services as svc}
								<div class="flex items-center gap-1.5 px-2.5 py-1 rounded-gx border border-gx-border-default bg-gx-bg-secondary text-xs">
									<span class="w-1.5 h-1.5 rounded-full {svc.status === 'active' ? 'bg-green-400' : 'bg-red-400'}"></span>
									<span class="text-gx-text-secondary">{svc.name}</span>
									{#if svc.endpoint}
										<span class="text-gx-text-muted/50 font-mono text-[9px]">{svc.endpoint}</span>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Task Grid -->
				<h3 class="text-xs font-medium text-gx-text-muted mb-2 flex items-center gap-1.5">
					<Activity size={12} />
					Task Workers
				</h3>
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-2">
					{#each snapshot.tasks as task (task.name)}
						{@const badge = taskStatusBadge(task.status)}
						<div class="flex items-center gap-2 p-2.5 rounded-gx border border-gx-border-default bg-gx-bg-secondary hover:border-gx-neon/20 transition-colors">
							<!-- Status dot -->
							<span class="w-2 h-2 rounded-full shrink-0 {task.status === 'OK' ? 'bg-green-400' : task.status === 'FAIL' ? 'bg-red-400' : 'bg-yellow-400'}"></span>

							<!-- Task info -->
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-1.5">
									<span class="text-xs font-medium text-gx-text-primary truncate">
										{task.name.replace(/_/g, ' ')}
									</span>
									<Badge variant="outline" class="text-[8px] px-1 py-0 h-3 shrink-0 {badge.cls}">
										{badge.label}
									</Badge>
								</div>
								<div class="flex items-center gap-2 mt-0.5 text-[10px] text-gx-text-muted">
									{#if task.duration_ms != null}
										<span class="flex items-center gap-0.5">
											<Timer size={8} />
											{(task.duration_ms / 1000).toFixed(1)}s
										</span>
									{/if}
									{#if task.trust > 0}
										<span class="flex items-center gap-0.5 {trustColor(task.trust)}">
											<TrendingUp size={8} />
											{(task.trust * 100).toFixed(0)}%
										</span>
									{/if}
									<span class="text-gx-text-muted/50">{task.pool}</span>
								</div>
							</div>
						</div>
					{/each}
				</div>

				{#if !orchIsActive && snapshot.tasks.length === 0}
					<div class="mt-4 p-4 rounded-gx border border-gx-border-default bg-gx-bg-secondary text-center">
						<Brain size={32} class="mx-auto text-gx-text-muted mb-2" />
						<p class="text-sm text-gx-text-secondary mb-1">No tasks configured yet</p>
						<p class="text-xs text-gx-text-muted">Use the Setup Wizard to configure your local AI stack and orchestrator tasks.</p>
					</div>
				{/if}
			{:else}
				<div class="flex flex-col items-center justify-center h-96 gap-4">
					<Brain class="w-16 h-16 text-gx-text-muted" />
					<h2 class="text-xl font-semibold text-gx-text-primary">ImpForge AI Orchestrator</h2>
					<p class="text-gx-text-muted text-center max-w-md">
						Configure your local AI stack to get started. ImpForge will set up Ollama, download models, and start the orchestrator automatically.
					</p>
					<Button variant="outline" onclick={() => orchAction('start')}>
						<Play size={14} /> Setup Wizard
					</Button>
				</div>
			{/if}

		{:else if activeTab === 'pool'}
			<!-- ============================================================= -->
			<!-- AGENT POOL (existing) -->
			<!-- ============================================================= -->
			{#if loading}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
					{#each Array(6) as _}
						<div class="h-40 rounded-gx-lg bg-gx-bg-tertiary animate-pulse"></div>
					{/each}
				</div>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
					{#each agents as agent (agent.id)}
						{@const RoleIcon = getRoleIcon(agent.role)}
						<Card class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-colors">
							<CardHeader class="pb-2">
								<div class="flex items-start justify-between">
									<div class="flex items-center gap-2.5">
										<div class="w-9 h-9 rounded-gx bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shrink-0">
											<RoleIcon size={16} class={statusColor(agent.status)} />
										</div>
										<div class="min-w-0">
											<CardTitle class="text-sm truncate">{agent.name}</CardTitle>
											<div class="flex items-center gap-1.5 mt-0.5">
												<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {statusBadge(agent.status).cls}">
													{statusBadge(agent.status).label}
												</Badge>
												<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted">
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
								<p class="text-xs text-gx-text-muted mb-2.5 line-clamp-2">{agent.description}</p>
								<div class="flex items-center gap-1.5 mb-2.5 text-xs text-gx-text-muted">
									<Zap size={11} class="text-gx-neon" />
									<span class="font-mono text-[11px] text-gx-text-secondary truncate">{agent.model}</span>
								</div>
								<div class="flex flex-wrap gap-1">
									{#each agent.capabilities as cap}
										<span class="text-[10px] px-1.5 py-0.5 rounded bg-gx-bg-tertiary text-gx-text-muted border border-gx-border-default">
											{cap}
										</span>
									{/each}
								</div>
							</CardContent>
						</Card>
					{/each}
				</div>
			{/if}

		{:else if activeTab === 'topology'}
			<!-- ============================================================= -->
			<!-- TOPOLOGY (existing) -->
			<!-- ============================================================= -->
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<svg width="500" height="500" viewBox="-250 -250 500 500" class="max-w-full max-h-full">
					{#each agents.filter((a) => a.role !== 'orchestrator') as agent, i}
						{@const count = agents.filter((a) => a.role !== 'orchestrator').length}
						{@const angle = (i / count) * 2 * Math.PI - Math.PI / 2}
						{@const x = Math.cos(angle) * 160}
						{@const y = Math.sin(angle) * 160}
						<line x1="0" y1="0" x2={x} y2={y}
							stroke="var(--color-gx-border-default, #2a2a35)"
							stroke-width="1.5" stroke-dasharray="6 4" opacity="0.6" />
						<circle r="2" fill="var(--color-gx-neon, #00FF66)" opacity="0.5">
							<animateMotion dur="{3 + i * 0.5}s" repeatCount="indefinite" path="M0,0 L{x},{y}" />
						</circle>
					{/each}

					<circle cx="0" cy="0" r="45" fill="none" stroke="var(--color-gx-neon, #00FF66)" stroke-width="2" opacity="0.8" />
					<circle cx="0" cy="0" r="45" fill="var(--color-gx-neon, #00FF66)" opacity="0.08" />
					<circle cx="0" cy="0" r="50" fill="none" stroke="var(--color-gx-neon, #00FF66)" stroke-width="1" opacity="0.2" />

					<text x="0" y="-8" text-anchor="middle" fill="var(--color-gx-neon, #00FF66)" font-size="12" font-weight="600">Orchestrator</text>
					<text x="0" y="8" text-anchor="middle" fill="var(--color-gx-text-muted, #666)" font-size="9">
						{agents.find((a) => a.role === 'orchestrator')?.model ?? 'hermes-3:8b'}
					</text>

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

						<circle cx={x} cy={y} r="30" fill="var(--color-gx-bg-elevated, #1a1a22)" stroke={nodeColor} stroke-width="1.5" />
						<text x={x} y={y - 5} text-anchor="middle" fill="var(--color-gx-text-primary, #e0e0e0)" font-size="9" font-weight="500">
							{agent.name.split(' ')[0]}
						</text>
						<text x={x} y={y + 7} text-anchor="middle" fill="var(--color-gx-text-muted, #666)" font-size="7">
							{agent.role}
						</text>
						<circle cx={x + 22} cy={y - 22} r="4" fill={nodeColor} />
					{/each}
				</svg>
				<p class="text-sm text-gx-text-muted">
					Agent topology: {agents.length} agents connected to the central Orchestrator
				</p>
			</div>
		{/if}
	</div>

	<!-- Journal Log Panel -->
	{#if showLogs}
		<div class="shrink-0 h-72 border-t border-gx-border-default bg-gx-bg-secondary flex flex-col">
			<div class="flex items-center justify-between px-4 py-2 border-b border-gx-border-default">
				<span class="text-sm text-gx-text-primary font-medium flex items-center gap-2">
					<ScrollText size={14} class="text-gx-neon" />
					Journal: <span class="font-mono text-gx-neon">neuralswarm-orchestrator</span>
				</span>
				<div class="flex items-center gap-2">
					<Button variant="outline" size="sm" class="text-xs h-6" onclick={fetchLogs}>
						<RefreshCw size={12} />
					</Button>
					<button
						class="p-1 rounded hover:bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-text-primary transition-colors"
						onclick={() => showLogs = false}
					>
						<X size={14} />
					</button>
				</div>
			</div>
			<pre class="flex-1 overflow-auto p-3 text-xs font-mono text-gx-text-secondary whitespace-pre-wrap leading-relaxed">{logContent || 'No logs available.'}</pre>
		</div>
	{/if}
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
				<Input id="agent-name" bind:value={formName} placeholder="e.g. Security Auditor"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted" />
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
				<Input id="agent-model" bind:value={formModel} placeholder="e.g. hermes-3:8b"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted font-mono text-sm" />
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="agent-desc">Description</label>
				<textarea id="agent-desc" bind:value={formDescription} placeholder="Describe the agent's purpose..." rows="3"
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
				<Input id="edit-name" bind:value={formName}
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary" />
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
				<Input id="edit-model" bind:value={formModel}
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary font-mono text-sm" />
			</div>
			<div class="space-y-1.5">
				<label class="text-xs text-gx-text-secondary font-medium" for="edit-desc">Description</label>
				<textarea id="edit-desc" bind:value={formDescription} rows="3"
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
