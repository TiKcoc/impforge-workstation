<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import {
		Workflow, Plus, Play, Trash2, Search, ChevronRight,
		Clock, Hash, Settings2, Zap, Globe, FileText, Brain,
		Terminal, Mail, GitBranch, Bell, Database, Share2,
		Filter, ArrowRight, Loader2, CheckCircle2, XCircle,
		Copy, ToggleLeft, ToggleRight, History, ChevronDown,
		ChevronUp, Timer, Repeat, Merge, LayoutTemplate
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'page-workflows';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// -----------------------------------------------------------------------
	// Types (mirror Rust structs)
	// -----------------------------------------------------------------------

	interface WorkflowMeta {
		id: string;
		name: string;
		description: string;
		enabled: boolean;
		node_count: number;
		run_count: number;
		last_run: string | null;
	}

	interface FlowNode {
		id: string;
		node_type: NodeType;
		label: string;
		config: Record<string, unknown>;
		position: [number, number];
	}

	interface FlowEdge {
		id: string;
		source: string;
		target: string;
		label: string | null;
	}

	interface FullWorkflow {
		id: string;
		name: string;
		description: string;
		nodes: FlowNode[];
		edges: FlowEdge[];
		enabled: boolean;
		run_count: number;
		last_run: string | null;
		created_at: string;
		updated_at: string;
	}

	interface WorkflowRun {
		id: string;
		workflow_id: string;
		status: string;
		started_at: string;
		completed_at: string | null;
		node_results: NodeResult[];
		error: string | null;
	}

	interface NodeResult {
		node_id: string;
		status: string;
		output: unknown;
		duration_ms: number;
		error: string | null;
	}

	interface WorkflowTemplate {
		id: string;
		name: string;
		description: string;
		category: string;
		workflow: FullWorkflow;
	}

	type NodeType = { kind: string; [key: string]: unknown };

	// -----------------------------------------------------------------------
	// State
	// -----------------------------------------------------------------------

	let workflows = $state<WorkflowMeta[]>([]);
	let activeWorkflow = $state<FullWorkflow | null>(null);
	let selectedNodeId = $state<string | null>(null);
	let runs = $state<WorkflowRun[]>([]);
	let templates = $state<WorkflowTemplate[]>([]);
	let loading = $state(false);
	let running = $state(false);
	let searchQuery = $state('');
	let showTemplates = $state(false);
	let showNewDialog = $state(false);
	let showRunHistory = $state(false);
	let newName = $state('');
	let newDescription = $state('');
	let error = $state<string | null>(null);

	// Canvas interaction state
	let canvasRef: HTMLDivElement | undefined = $state(undefined);
	let draggingNodeId = $state<string | null>(null);
	let dragOffset = $state({ x: 0, y: 0 });
	let connectingFrom = $state<string | null>(null);
	let mousePos = $state({ x: 0, y: 0 });
	let canvasScroll = $state({ x: 0, y: 0 });

	// -----------------------------------------------------------------------
	// Derived
	// -----------------------------------------------------------------------

	let filteredWorkflows = $derived(
		workflows.filter(w =>
			!searchQuery || w.name.toLowerCase().includes(searchQuery.toLowerCase())
		)
	);

	let selectedNode = $derived(
		activeWorkflow?.nodes.find(n => n.id === selectedNodeId) ?? null
	);

	// -----------------------------------------------------------------------
	// Node palette -- types available for adding
	// -----------------------------------------------------------------------

	const NODE_PALETTE = [
		{ group: 'Triggers', color: '#22c55e', items: [
			{ kind: 'trigger_manual', label: 'Manual Trigger', icon: Play },
			{ kind: 'trigger_cron', label: 'Cron Schedule', icon: Clock, defaults: { schedule: '0 */5 * * * *' } },
			{ kind: 'trigger_webhook', label: 'Webhook', icon: Globe, defaults: { path: '/hook' } },
			{ kind: 'trigger_file_watch', label: 'File Watch', icon: FileText, defaults: { path: './' } },
			{ kind: 'trigger_app_event', label: 'App Event', icon: Zap, defaults: { event_name: '' } },
		]},
		{ group: 'Actions', color: '#3b82f6', items: [
			{ kind: 'action_http_request', label: 'HTTP Request', icon: Globe, defaults: { method: 'GET', url: '', headers: [], body: null } },
			{ kind: 'action_llm_call', label: 'AI / LLM', icon: Brain, defaults: { prompt: '', model: null } },
			{ kind: 'action_shell_command', label: 'Shell Command', icon: Terminal, defaults: { command: '' } },
			{ kind: 'action_email', label: 'Send Email', icon: Mail, defaults: { to: '', subject: '', body: '' } },
			{ kind: 'action_file_op', label: 'File Operation', icon: FileText, defaults: { operation: 'read', path: '' } },
			{ kind: 'action_json_transform', label: 'JSON Transform', icon: Filter, defaults: { expression: '' } },
			{ kind: 'action_notification', label: 'Notification', icon: Bell, defaults: { title: '', message: '' } },
			{ kind: 'action_git_op', label: 'Git Operation', icon: GitBranch, defaults: { operation: 'status' } },
			{ kind: 'action_social_post', label: 'Social Post', icon: Share2, defaults: { platform: '', content: '' } },
			{ kind: 'action_db_query', label: 'DB Query', icon: Database, defaults: { query: '' } },
		]},
		{ group: 'Control', color: '#a855f7', items: [
			{ kind: 'control_condition', label: 'Condition', icon: Filter, defaults: { expression: '' } },
			{ kind: 'control_loop', label: 'Loop', icon: Repeat, defaults: { count: 3 } },
			{ kind: 'control_delay', label: 'Delay', icon: Timer, defaults: { seconds: 5 } },
			{ kind: 'control_merge', label: 'Merge', icon: Merge, defaults: {} },
		]},
	];

	// -----------------------------------------------------------------------
	// Color helpers
	// -----------------------------------------------------------------------

	function nodeColor(kind: string): string {
		if (kind.startsWith('trigger')) return '#22c55e';
		if (kind.startsWith('action')) return '#3b82f6';
		if (kind.startsWith('control')) return '#a855f7';
		return '#6b7280';
	}

	function nodeIcon(kind: string) {
		for (const group of NODE_PALETTE) {
			for (const item of group.items) {
				if (item.kind === kind) return item.icon;
			}
		}
		return Workflow;
	}

	function kindLabel(kind: string): string {
		for (const group of NODE_PALETTE) {
			for (const item of group.items) {
				if (item.kind === kind) return item.label;
			}
		}
		return kind;
	}

	// -----------------------------------------------------------------------
	// API calls
	// -----------------------------------------------------------------------

	async function loadWorkflows() {
		try {
			workflows = await invoke<WorkflowMeta[]>('flow_list');
		} catch (e) {
			error = `Failed to load workflows: ${e}`;
		}
	}

	async function openWorkflow(id: string) {
		loading = true;
		try {
			activeWorkflow = await invoke<FullWorkflow>('flow_get', { id });
			selectedNodeId = null;
			await loadRuns(id);
		} catch (e) {
			error = `Failed to open workflow: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function createWorkflow() {
		if (!newName.trim()) return;
		try {
			const wf = await invoke<FullWorkflow>('flow_create', {
				name: newName.trim(),
				description: newDescription.trim(),
			});
			showNewDialog = false;
			newName = '';
			newDescription = '';
			await loadWorkflows();
			await openWorkflow(wf.id);
		} catch (e) {
			error = `Failed to create workflow: ${e}`;
		}
	}

	async function saveWorkflow() {
		if (!activeWorkflow) return;
		try {
			await invoke('flow_save', { id: activeWorkflow.id, workflow: activeWorkflow });
			await loadWorkflows();
		} catch (e) {
			error = `Failed to save workflow: ${e}`;
		}
	}

	async function deleteWorkflow(id: string) {
		try {
			await invoke('flow_delete', { id });
			if (activeWorkflow?.id === id) activeWorkflow = null;
			await loadWorkflows();
		} catch (e) {
			error = `Failed to delete workflow: ${e}`;
		}
	}

	async function addNode(kind: string, label: string, defaults: Record<string, unknown> = {}) {
		if (!activeWorkflow) return;
		try {
			const nodeType = { kind, ...defaults };
			const pos: [number, number] = [200 + Math.random() * 300, 150 + Math.random() * 200];
			const node = await invoke<FlowNode>('flow_add_node', {
				workflowId: activeWorkflow.id,
				nodeType: nodeType,
				label,
				config: {},
				position: pos,
			});
			activeWorkflow.nodes = [...activeWorkflow.nodes, node];
			selectedNodeId = node.id;
		} catch (e) {
			error = `Failed to add node: ${e}`;
		}
	}

	async function removeNode(nodeId: string) {
		if (!activeWorkflow) return;
		try {
			await invoke('flow_remove_node', { workflowId: activeWorkflow.id, nodeId });
			activeWorkflow.nodes = activeWorkflow.nodes.filter(n => n.id !== nodeId);
			activeWorkflow.edges = activeWorkflow.edges.filter(e => e.source !== nodeId && e.target !== nodeId);
			if (selectedNodeId === nodeId) selectedNodeId = null;
		} catch (e) {
			error = `Failed to remove node: ${e}`;
		}
	}

	async function connectNodes(sourceId: string, targetId: string) {
		if (!activeWorkflow || sourceId === targetId) return;
		try {
			const edge = await invoke<FlowEdge>('flow_connect', {
				workflowId: activeWorkflow.id,
				sourceId,
				targetId,
			});
			activeWorkflow.edges = [...activeWorkflow.edges, edge];
		} catch (e) {
			error = `Failed to connect: ${e}`;
		}
	}

	async function disconnectEdge(edgeId: string) {
		if (!activeWorkflow) return;
		try {
			await invoke('flow_disconnect', { workflowId: activeWorkflow.id, edgeId });
			activeWorkflow.edges = activeWorkflow.edges.filter(e => e.id !== edgeId);
		} catch (e) {
			error = `Failed to disconnect: ${e}`;
		}
	}

	async function runWorkflow() {
		if (!activeWorkflow) return;
		running = true;
		error = null;
		try {
			const run = await invoke<WorkflowRun>('flow_run', { workflowId: activeWorkflow.id });
			runs = [run, ...runs];
			showRunHistory = true;
			await loadWorkflows();
			// Re-read workflow to get updated run_count
			activeWorkflow = await invoke<FullWorkflow>('flow_get', { id: activeWorkflow.id });
		} catch (e) {
			error = `Workflow execution failed: ${e}`;
		} finally {
			running = false;
		}
	}

	async function loadRuns(workflowId: string) {
		try {
			runs = await invoke<WorkflowRun[]>('flow_get_runs', { workflowId, limit: 20 });
		} catch {
			runs = [];
		}
	}

	async function loadTemplates() {
		try {
			templates = await invoke<WorkflowTemplate[]>('flow_get_templates');
		} catch {
			templates = [];
		}
	}

	async function useTemplate(tpl: WorkflowTemplate) {
		try {
			const wf = await invoke<FullWorkflow>('flow_create', {
				name: tpl.workflow.name,
				description: tpl.workflow.description,
			});
			// Copy nodes and edges from template
			const updated = { ...tpl.workflow, id: wf.id, created_at: wf.created_at, updated_at: wf.updated_at };
			await invoke('flow_save', { id: wf.id, workflow: updated });
			showTemplates = false;
			await loadWorkflows();
			await openWorkflow(wf.id);
		} catch (e) {
			error = `Failed to create from template: ${e}`;
		}
	}

	// -----------------------------------------------------------------------
	// Canvas interaction
	// -----------------------------------------------------------------------

	function handleCanvasMouseMove(e: MouseEvent) {
		if (!canvasRef) return;
		const rect = canvasRef.getBoundingClientRect();
		mousePos = { x: e.clientX - rect.left + canvasScroll.x, y: e.clientY - rect.top + canvasScroll.y };

		if (draggingNodeId && activeWorkflow) {
			const node = activeWorkflow.nodes.find(n => n.id === draggingNodeId);
			if (node) {
				node.position = [mousePos.x - dragOffset.x, mousePos.y - dragOffset.y];
				activeWorkflow.nodes = [...activeWorkflow.nodes];
			}
		}
	}

	function handleCanvasMouseUp() {
		if (draggingNodeId) {
			draggingNodeId = null;
			saveWorkflow();
		}
	}

	function handleNodeMouseDown(e: MouseEvent, nodeId: string) {
		if (!activeWorkflow) return;
		const node = activeWorkflow.nodes.find(n => n.id === nodeId);
		if (!node || !canvasRef) return;
		e.stopPropagation();
		const rect = canvasRef.getBoundingClientRect();
		const mx = e.clientX - rect.left + canvasScroll.x;
		const my = e.clientY - rect.top + canvasScroll.y;
		dragOffset = { x: mx - node.position[0], y: my - node.position[1] };
		draggingNodeId = nodeId;
		selectedNodeId = nodeId;
	}

	function handleOutputClick(e: MouseEvent, nodeId: string) {
		e.stopPropagation();
		if (connectingFrom === null) {
			connectingFrom = nodeId;
		} else {
			if (connectingFrom !== nodeId) {
				connectNodes(connectingFrom, nodeId);
			}
			connectingFrom = null;
		}
	}

	function handleInputClick(e: MouseEvent, nodeId: string) {
		e.stopPropagation();
		if (connectingFrom !== null && connectingFrom !== nodeId) {
			connectNodes(connectingFrom, nodeId);
			connectingFrom = null;
		}
	}

	function handleCanvasScroll(e: Event) {
		const target = e.target as HTMLDivElement;
		canvasScroll = { x: target.scrollLeft, y: target.scrollTop };
	}

	// -----------------------------------------------------------------------
	// Edge SVG path (cubic bezier)
	// -----------------------------------------------------------------------

	function edgePath(source: FlowNode, target: FlowNode): string {
		const sx = source.position[0] + 180; // right side of card
		const sy = source.position[1] + 30;  // vertical center
		const tx = target.position[0];       // left side of card
		const ty = target.position[1] + 30;
		const dx = Math.abs(tx - sx) * 0.5;
		return `M ${sx} ${sy} C ${sx + dx} ${sy}, ${tx - dx} ${ty}, ${tx} ${ty}`;
	}

	// -----------------------------------------------------------------------
	// Config field definitions per node kind
	// -----------------------------------------------------------------------

	function getConfigFields(kind: string): { key: string; label: string; type: 'text' | 'textarea' | 'select'; options?: string[] }[] {
		switch (kind) {
			case 'trigger_cron': return [{ key: 'schedule', label: 'Cron Schedule', type: 'text' }];
			case 'trigger_webhook': return [{ key: 'path', label: 'Webhook Path', type: 'text' }];
			case 'trigger_file_watch': return [{ key: 'path', label: 'Watch Path', type: 'text' }];
			case 'trigger_app_event': return [{ key: 'event_name', label: 'Event Name', type: 'text' }];
			case 'action_http_request': return [
				{ key: 'method', label: 'Method', type: 'select', options: ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'] },
				{ key: 'url', label: 'URL', type: 'text' },
				{ key: 'body', label: 'Body', type: 'textarea' },
			];
			case 'action_llm_call': return [
				{ key: 'prompt', label: 'Prompt', type: 'textarea' },
				{ key: 'model', label: 'Model (optional)', type: 'text' },
			];
			case 'action_shell_command': return [{ key: 'command', label: 'Command', type: 'textarea' }];
			case 'action_email': return [
				{ key: 'to', label: 'To', type: 'text' },
				{ key: 'subject', label: 'Subject', type: 'text' },
				{ key: 'body', label: 'Body', type: 'textarea' },
			];
			case 'action_file_op': return [
				{ key: 'operation', label: 'Operation', type: 'select', options: ['read', 'write', 'exists', 'delete', 'list'] },
				{ key: 'path', label: 'Path', type: 'text' },
			];
			case 'action_json_transform': return [{ key: 'expression', label: 'Expression', type: 'text' }];
			case 'action_notification': return [
				{ key: 'title', label: 'Title', type: 'text' },
				{ key: 'message', label: 'Message', type: 'textarea' },
			];
			case 'action_git_op': return [
				{ key: 'operation', label: 'Operation', type: 'select', options: ['status', 'pull', 'push', 'log'] },
			];
			case 'action_social_post': return [
				{ key: 'platform', label: 'Platform', type: 'select', options: ['linkedin', 'twitter', 'github', 'hackernews'] },
				{ key: 'content', label: 'Content', type: 'textarea' },
			];
			case 'action_db_query': return [{ key: 'query', label: 'SQL Query', type: 'textarea' }];
			case 'control_condition': return [{ key: 'expression', label: 'Condition (e.g. status == 200)', type: 'text' }];
			case 'control_loop': return [{ key: 'count', label: 'Iterations', type: 'text' }];
			case 'control_delay': return [{ key: 'seconds', label: 'Delay (seconds)', type: 'text' }];
			default: return [];
		}
	}

	function updateNodeField(nodeId: string, key: string, value: string) {
		if (!activeWorkflow) return;
		const node = activeWorkflow.nodes.find(n => n.id === nodeId);
		if (!node) return;
		// Update the node_type fields directly
		(node.node_type as Record<string, unknown>)[key] = value;
		activeWorkflow.nodes = [...activeWorkflow.nodes];
	}

	function getNodeFieldValue(node: FlowNode, key: string): string {
		const val = (node.node_type as Record<string, unknown>)[key];
		if (val === null || val === undefined) return '';
		return String(val);
	}

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	onMount(async () => {
		await loadWorkflows();
		await loadTemplates();
	});
</script>

<div class="flex h-full" style={containerStyle}>
	<!-- Left Panel: Workflow List (220px) -->
	<div class="w-[220px] flex flex-col border-r border-gx-border-default bg-gx-bg-secondary shrink-0">
		<!-- Header -->
		<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default">
			<Workflow size={14} class="text-gx-neon" />
			<span class="text-xs font-semibold text-gx-text-secondary">ForgeFlow</span>
			<div class="flex-1"></div>
			<button
				onclick={() => showTemplates = true}
				class="p-1 text-gx-text-muted hover:text-gx-accent-purple transition-colors"
				title="Templates"
			>
				<LayoutTemplate size={13} />
			</button>
			<button
				onclick={() => showNewDialog = true}
				class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
				title="New Workflow"
			>
				<Plus size={14} />
			</button>
		</div>

		<!-- Search -->
		<div class="px-2 py-1.5">
			<div class="flex items-center gap-1.5 px-2 py-1 bg-gx-bg-tertiary rounded border border-gx-border-default">
				<Search size={11} class="text-gx-text-muted" />
				<input
					type="text"
					placeholder="Search..."
					bind:value={searchQuery}
					class="flex-1 bg-transparent text-xs text-gx-text-primary outline-none placeholder:text-gx-text-muted"
				/>
			</div>
		</div>

		<!-- Workflow list -->
		<div class="flex-1 overflow-y-auto px-1">
			{#each filteredWorkflows as wf (wf.id)}
				<button
					onclick={() => openWorkflow(wf.id)}
					class="w-full flex flex-col gap-0.5 px-2 py-2 rounded-gx text-left transition-colors mb-0.5
						{activeWorkflow?.id === wf.id
							? 'bg-gx-neon/10 border border-gx-neon/20'
							: 'hover:bg-gx-bg-hover'}"
				>
					<div class="flex items-center gap-1.5">
						<span class="w-1.5 h-1.5 rounded-full shrink-0" class:bg-gx-status-success={wf.enabled} class:bg-gx-text-muted={!wf.enabled}></span>
						<span class="text-xs font-medium text-gx-text-primary truncate">{wf.name}</span>
					</div>
					<div class="flex items-center gap-2 pl-3 text-[10px] text-gx-text-muted">
						<span>{wf.node_count} nodes</span>
						<span class="flex items-center gap-0.5">
							<Play size={8} /> {wf.run_count}
						</span>
					</div>
				</button>
			{/each}

			{#if filteredWorkflows.length === 0}
				<div class="text-center py-8 text-xs text-gx-text-muted">
					<Workflow size={24} class="mx-auto mb-2 opacity-30" />
					<p>No workflows yet</p>
					<button
						onclick={() => showNewDialog = true}
						class="mt-2 text-gx-neon hover:underline"
					>
						Create one
					</button>
				</div>
			{/if}
		</div>
	</div>

	<!-- Main area -->
	{#if activeWorkflow}
		<div class="flex-1 flex flex-col min-w-0">
			<!-- Toolbar -->
			<div class="flex items-center gap-2 px-3 py-1.5 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
				<span class="text-sm font-semibold text-gx-text-primary truncate">{activeWorkflow.name}</span>
				<Badge variant="outline" class="text-[10px] px-1.5 h-4 border-gx-border-default text-gx-text-muted">
					{activeWorkflow.nodes.length} nodes
				</Badge>

				<div class="flex-1"></div>

				<!-- Add node dropdown groups -->
				{#each NODE_PALETTE as group}
					<div class="relative group">
						<button
							class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary hover:border-gx-text-muted transition-colors"
							style="border-color: {group.color}40; color: {group.color}"
						>
							<Plus size={10} />
							{group.group}
						</button>
						<div class="absolute top-full left-0 mt-1 w-44 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-lg z-50 hidden group-hover:block">
							{#each group.items as item}
								<button
									onclick={() => addNode(item.kind, item.label, item.defaults ?? {})}
									class="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-text-primary transition-colors"
								>
									<item.icon size={12} style="color: {group.color}" />
									{item.label}
								</button>
							{/each}
						</div>
					</div>
				{/each}

				<div class="w-px h-4 bg-gx-border-default"></div>

				<!-- Run button -->
				<button
					onclick={runWorkflow}
					disabled={running || activeWorkflow.nodes.length === 0}
					class="flex items-center gap-1.5 px-3 py-1 text-xs font-medium rounded-gx transition-all
						{running
							? 'bg-gx-status-warning/20 text-gx-status-warning border border-gx-status-warning/30'
							: 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 hover:shadow-gx-glow-sm'}"
				>
					{#if running}
						<Loader2 size={12} class="animate-spin" />
						Running...
					{:else}
						<Play size={12} />
						Run
					{/if}
				</button>

				<!-- History toggle -->
				<button
					onclick={() => showRunHistory = !showRunHistory}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
				>
					<History size={11} />
					{#if showRunHistory}<ChevronDown size={10} />{:else}<ChevronUp size={10} />{/if}
				</button>

				<!-- Delete workflow -->
				<button
					onclick={() => deleteWorkflow(activeWorkflow?.id ?? '')}
					class="p-1 text-gx-text-muted hover:text-gx-status-error transition-colors"
					title="Delete workflow"
				>
					<Trash2 size={13} />
				</button>
			</div>

			<!-- Canvas + right config panel -->
			<div class="flex-1 flex min-h-0">
				<!-- Canvas area -->
				<div
					bind:this={canvasRef}
					onmousemove={handleCanvasMouseMove}
					onmouseup={handleCanvasMouseUp}
					onscroll={handleCanvasScroll}
					role="application"
					aria-label="Workflow canvas"
					class="flex-1 relative overflow-auto bg-gx-bg-primary"
					style="background-image: radial-gradient(circle, var(--gx-border-default) 1px, transparent 1px); background-size: 24px 24px;"
				>
					<!-- SVG layer for edges -->
					<svg class="absolute inset-0 w-full h-full pointer-events-none" style="min-width: 2000px; min-height: 1200px;">
						{#each activeWorkflow.edges as edge (edge.id)}
							{@const srcNode = activeWorkflow.nodes.find(n => n.id === edge.source)}
							{@const tgtNode = activeWorkflow.nodes.find(n => n.id === edge.target)}
							{#if srcNode && tgtNode}
								<!-- svelte-ignore event_directive_deprecated -->
								<g class="pointer-events-auto cursor-pointer" onclick={() => disconnectEdge(edge.id)}>
									<path
										d={edgePath(srcNode, tgtNode)}
										fill="none"
										stroke="var(--gx-border-default)"
										stroke-width="2"
										class="hover:stroke-[var(--gx-neon)] transition-colors"
									/>
									<!-- Hit area for clicking -->
									<path
										d={edgePath(srcNode, tgtNode)}
										fill="none"
										stroke="transparent"
										stroke-width="12"
									/>
									<!-- Arrow marker -->
									{@const tx = tgtNode.position[0]}
									{@const ty = tgtNode.position[1] + 30}
									<polygon
										points="{tx},{ty} {tx-8},{ty-4} {tx-8},{ty+4}"
										fill="var(--gx-border-default)"
									/>
								</g>
							{/if}
						{/each}

						<!-- Connection-in-progress line -->
						{#if connectingFrom}
							{@const fromNode = activeWorkflow.nodes.find(n => n.id === connectingFrom)}
							{#if fromNode}
								<line
									x1={fromNode.position[0] + 180}
									y1={fromNode.position[1] + 30}
									x2={mousePos.x}
									y2={mousePos.y}
									stroke="var(--gx-neon)"
									stroke-width="2"
									stroke-dasharray="6 3"
									opacity="0.6"
								/>
							{/if}
						{/if}
					</svg>

					<!-- Node cards -->
					<div class="relative" style="min-width: 2000px; min-height: 1200px;">
						{#each activeWorkflow.nodes as node (node.id)}
							{@const color = nodeColor(node.node_type.kind)}
							<!-- svelte-ignore event_directive_deprecated -->
							<div
								class="absolute select-none"
								style="left: {node.position[0]}px; top: {node.position[1]}px; width: 180px;"
								onmousedown={(e) => handleNodeMouseDown(e, node.id)}
							>
								<div
									class="rounded-gx border-2 transition-all cursor-grab active:cursor-grabbing
										{selectedNodeId === node.id ? 'shadow-lg' : 'shadow-sm hover:shadow-md'}"
									style="
										border-color: {selectedNodeId === node.id ? color : color + '40'};
										background: var(--gx-bg-elevated);
									"
								>
									<!-- Node header -->
									<div class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-t-gx" style="background: {color}15;">
										{@const Icon = nodeIcon(node.node_type.kind)}
										<Icon size={12} style="color: {color}" />
										<span class="text-[11px] font-medium text-gx-text-primary truncate flex-1">{node.label}</span>
										<button
											onclick={(e) => { e.stopPropagation(); removeNode(node.id); }}
											class="text-gx-text-muted hover:text-gx-status-error transition-colors opacity-0 group-hover:opacity-100"
										>
											<Trash2 size={10} />
										</button>
									</div>
									<!-- Node body -->
									<div class="px-2.5 py-1.5">
										<span class="text-[10px] text-gx-text-muted">{kindLabel(node.node_type.kind)}</span>
									</div>
									<!-- Connection ports -->
									<div class="flex items-center justify-between px-1 pb-1">
										<!-- Input port (left) -->
										{#if !node.node_type.kind.startsWith('trigger')}
											<!-- svelte-ignore event_directive_deprecated -->
											<button
												onclick={(e) => handleInputClick(e, node.id)}
												class="w-3 h-3 rounded-full border-2 transition-colors hover:scale-125"
												style="border-color: {color}; background: {connectingFrom ? color : 'transparent'};"
												title="Input"
											></button>
										{:else}
											<div class="w-3"></div>
										{/if}
										<!-- Output port (right) -->
										<!-- svelte-ignore event_directive_deprecated -->
										<button
											onclick={(e) => handleOutputClick(e, node.id)}
											class="w-3 h-3 rounded-full border-2 transition-colors hover:scale-125"
											style="border-color: {color}; background: {connectingFrom === node.id ? color : 'transparent'};"
											title="Output (click to connect)"
										></button>
									</div>
								</div>
							</div>
						{/each}
					</div>

					<!-- Empty state -->
					{#if activeWorkflow.nodes.length === 0}
						<div class="absolute inset-0 flex items-center justify-center pointer-events-none">
							<div class="text-center">
								<Workflow size={48} class="mx-auto mb-3 text-gx-text-muted opacity-20" />
								<p class="text-sm text-gx-text-muted mb-1">Empty workflow</p>
								<p class="text-xs text-gx-text-muted">Use the toolbar above to add trigger and action nodes</p>
							</div>
						</div>
					{/if}

					<!-- Cancel connection hint -->
					{#if connectingFrom}
						<div class="absolute top-2 left-1/2 -translate-x-1/2 px-3 py-1 bg-gx-neon/10 border border-gx-neon/30 rounded-gx text-xs text-gx-neon z-10">
							Click a target node's input port to connect, or
							<button onclick={() => connectingFrom = null} class="underline">cancel</button>
						</div>
					{/if}
				</div>

				<!-- Right Panel: Node Config (280px) -->
				{#if selectedNode}
					<div class="w-[280px] flex flex-col border-l border-gx-border-default bg-gx-bg-secondary shrink-0 overflow-y-auto">
						<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default">
							<Settings2 size={13} class="text-gx-neon" />
							<span class="text-xs font-semibold text-gx-text-secondary">Node Config</span>
							<div class="flex-1"></div>
							<button onclick={() => selectedNodeId = null} class="text-gx-text-muted hover:text-gx-text-secondary">
								<XCircle size={13} />
							</button>
						</div>

						<div class="p-3 space-y-3">
							<!-- Node identity -->
							<div>
								<label class="text-[10px] text-gx-text-muted uppercase tracking-wider">Label</label>
								<input
									type="text"
									bind:value={selectedNode.label}
									onchange={saveWorkflow}
									class="w-full mt-1 px-2 py-1.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs text-gx-text-primary outline-none focus:border-gx-neon"
								/>
							</div>

							<div>
								<label class="text-[10px] text-gx-text-muted uppercase tracking-wider">Type</label>
								<div class="flex items-center gap-1.5 mt-1">
									<span class="w-2 h-2 rounded-full" style="background: {nodeColor(selectedNode.node_type.kind)}"></span>
									<span class="text-xs text-gx-text-secondary">{kindLabel(selectedNode.node_type.kind)}</span>
								</div>
							</div>

							<div class="border-t border-gx-border-default pt-3">
								<span class="text-[10px] text-gx-text-muted uppercase tracking-wider">Configuration</span>
							</div>

							<!-- Dynamic config fields -->
							{#each getConfigFields(selectedNode.node_type.kind) as field}
								<div>
									<label class="text-[10px] text-gx-text-muted">{field.label}</label>
									{#if field.type === 'select'}
										<select
											value={getNodeFieldValue(selectedNode, field.key)}
											onchange={(e) => { updateNodeField(selectedNode?.id ?? '', field.key, (e.target as HTMLSelectElement).value); saveWorkflow(); }}
											class="w-full mt-1 px-2 py-1.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs text-gx-text-primary outline-none focus:border-gx-neon"
										>
											{#each (field.options ?? []) as opt}
												<option value={opt}>{opt}</option>
											{/each}
										</select>
									{:else if field.type === 'textarea'}
										<textarea
											value={getNodeFieldValue(selectedNode, field.key)}
											onchange={(e) => { updateNodeField(selectedNode?.id ?? '', field.key, (e.target as HTMLTextAreaElement).value); saveWorkflow(); }}
											class="w-full mt-1 px-2 py-1.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs text-gx-text-primary outline-none focus:border-gx-neon resize-y"
											rows="3"
										></textarea>
									{:else}
										<input
											type="text"
											value={getNodeFieldValue(selectedNode, field.key)}
											onchange={(e) => { updateNodeField(selectedNode?.id ?? '', field.key, (e.target as HTMLInputElement).value); saveWorkflow(); }}
											class="w-full mt-1 px-2 py-1.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs text-gx-text-primary outline-none focus:border-gx-neon"
										/>
									{/if}
								</div>
							{/each}

							{#if getConfigFields(selectedNode.node_type.kind).length === 0}
								<p class="text-xs text-gx-text-muted italic">No configuration needed</p>
							{/if}

							<!-- Connections -->
							<div class="border-t border-gx-border-default pt-3">
								<span class="text-[10px] text-gx-text-muted uppercase tracking-wider">Connections</span>
								<div class="mt-2 space-y-1">
									{#each activeWorkflow.edges.filter(e => e.source === selectedNode?.id || e.target === selectedNode?.id) as edge (edge.id)}
										{@const otherNodeId = edge.source === selectedNode?.id ? edge.target : edge.source}
										{@const otherNode = activeWorkflow.nodes.find(n => n.id === otherNodeId)}
										<div class="flex items-center gap-1.5 text-[11px] text-gx-text-secondary">
											{#if edge.source === selectedNode?.id}
												<ArrowRight size={10} class="text-gx-neon" />
												<span class="truncate">{otherNode?.label ?? otherNodeId}</span>
											{:else}
												<span class="truncate">{otherNode?.label ?? otherNodeId}</span>
												<ArrowRight size={10} class="text-gx-neon" />
												<span class="text-gx-text-muted">this</span>
											{/if}
											<button onclick={() => disconnectEdge(edge.id)} class="ml-auto text-gx-text-muted hover:text-gx-status-error">
												<XCircle size={10} />
											</button>
										</div>
									{/each}
								</div>
							</div>

							<!-- Delete node -->
							<div class="border-t border-gx-border-default pt-3">
								<button
									onclick={() => removeNode(selectedNode?.id ?? '')}
									class="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs text-gx-status-error border border-gx-status-error/30 rounded-gx hover:bg-gx-status-error/10 transition-colors"
								>
									<Trash2 size={12} />
									Delete Node
								</button>
							</div>
						</div>
					</div>
				{/if}
			</div>

			<!-- Bottom: Run History (collapsible) -->
			{#if showRunHistory && runs.length > 0}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[200px] overflow-y-auto">
					<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default">
						<History size={12} class="text-gx-text-muted" />
						<span class="text-[11px] font-medium text-gx-text-secondary">Run History</span>
						<Badge variant="outline" class="text-[9px] px-1 h-3.5 border-gx-border-default text-gx-text-muted">
							{runs.length}
						</Badge>
						<div class="flex-1"></div>
						<button onclick={() => showRunHistory = false} class="text-gx-text-muted hover:text-gx-text-secondary">
							<ChevronDown size={12} />
						</button>
					</div>
					<table class="w-full text-[11px]">
						<thead>
							<tr class="text-gx-text-muted border-b border-gx-border-default">
								<th class="text-left px-3 py-1 font-medium">Run ID</th>
								<th class="text-left px-3 py-1 font-medium">Status</th>
								<th class="text-left px-3 py-1 font-medium">Started</th>
								<th class="text-left px-3 py-1 font-medium">Nodes</th>
								<th class="text-left px-3 py-1 font-medium">Error</th>
							</tr>
						</thead>
						<tbody>
							{#each runs as run (run.id)}
								<tr class="border-b border-gx-border-default/50 hover:bg-gx-bg-hover">
									<td class="px-3 py-1 font-mono text-gx-text-muted">{run.id.slice(0, 8)}</td>
									<td class="px-3 py-1">
										{#if run.status === 'completed'}
											<span class="flex items-center gap-1 text-gx-status-success">
												<CheckCircle2 size={10} /> Done
											</span>
										{:else if run.status === 'failed'}
											<span class="flex items-center gap-1 text-gx-status-error">
												<XCircle size={10} /> Failed
											</span>
										{:else}
											<span class="flex items-center gap-1 text-gx-status-warning">
												<Loader2 size={10} class="animate-spin" /> {run.status}
											</span>
										{/if}
									</td>
									<td class="px-3 py-1 text-gx-text-muted">
										{new Date(run.started_at).toLocaleString()}
									</td>
									<td class="px-3 py-1 text-gx-text-muted">
										{run.node_results.length} executed
									</td>
									<td class="px-3 py-1 text-gx-status-error truncate max-w-[200px]">
										{run.error ?? ''}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>
	{:else}
		<!-- No workflow selected -->
		<div class="flex-1 flex items-center justify-center bg-gx-bg-primary">
			<div class="text-center max-w-md">
				<Workflow size={56} class="mx-auto mb-4 text-gx-text-muted opacity-20" />
				<h2 class="text-lg font-semibold text-gx-text-primary mb-2">ForgeFlow</h2>
				<p class="text-sm text-gx-text-muted mb-6">
					Build automated workflows with triggers, AI actions, and integrations.
					Replace n8n, Zapier, and Make.com -- all running locally.
				</p>
				<div class="flex items-center justify-center gap-3">
					<button
						onclick={() => showNewDialog = true}
						class="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 hover:shadow-gx-glow-sm transition-all"
					>
						<Plus size={16} />
						New Workflow
					</button>
					<button
						onclick={() => showTemplates = true}
						class="flex items-center gap-2 px-4 py-2 text-sm font-medium border border-gx-border-default text-gx-text-secondary rounded-gx hover:border-gx-accent-purple hover:text-gx-accent-purple transition-all"
					>
						<LayoutTemplate size={16} />
						From Template
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>

<!-- Error toast -->
{#if error}
	<div class="fixed bottom-4 right-4 z-50 flex items-center gap-2 px-4 py-2.5 bg-gx-status-error/10 border border-gx-status-error/30 rounded-gx text-sm text-gx-status-error shadow-lg max-w-md">
		<XCircle size={16} />
		<span class="flex-1 truncate">{error}</span>
		<button onclick={() => error = null} class="shrink-0 hover:text-gx-text-primary">
			<XCircle size={14} />
		</button>
	</div>
{/if}

<!-- New Workflow Dialog -->
<Dialog.Root bind:open={showNewDialog}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-sm">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary">New Workflow</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Create a new automation workflow.
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-3">
			<div>
				<label class="text-xs text-gx-text-muted" for="wf-name">Name</label>
				<input
					id="wf-name"
					type="text"
					bind:value={newName}
					placeholder="My Workflow"
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-sm text-gx-text-primary outline-none focus:border-gx-neon"
				/>
			</div>
			<div>
				<label class="text-xs text-gx-text-muted" for="wf-desc">Description</label>
				<textarea
					id="wf-desc"
					bind:value={newDescription}
					placeholder="What does this workflow do?"
					rows="2"
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-sm text-gx-text-primary outline-none focus:border-gx-neon resize-none"
				></textarea>
			</div>
		</div>
		<Dialog.Footer>
			<button onclick={() => showNewDialog = false} class="px-3 py-1.5 text-sm text-gx-text-muted hover:text-gx-text-secondary">Cancel</button>
			<button
				onclick={createWorkflow}
				disabled={!newName.trim()}
				class="px-4 py-1.5 text-sm font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 disabled:opacity-40"
			>
				Create
			</button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Templates Dialog -->
<Dialog.Root bind:open={showTemplates}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-2xl">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary">Workflow Templates</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Start with a pre-built workflow and customize it.
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid grid-cols-2 gap-3 py-3 max-h-[400px] overflow-y-auto">
			{#each templates as tpl (tpl.id)}
				<div class="flex flex-col p-3 rounded-gx border border-gx-border-default bg-gx-bg-primary hover:border-gx-neon/30 transition-colors">
					<div class="flex items-center gap-2 mb-1.5">
						<Workflow size={14} class="text-gx-neon" />
						<span class="text-sm font-medium text-gx-text-primary">{tpl.name}</span>
					</div>
					<p class="text-xs text-gx-text-muted mb-2 flex-1">{tpl.description}</p>
					<div class="flex items-center justify-between">
						<Badge variant="outline" class="text-[10px] px-1.5 h-4 border-gx-border-default text-gx-text-muted">
							{tpl.category}
						</Badge>
						<button
							onclick={() => useTemplate(tpl)}
							class="flex items-center gap-1 px-2 py-1 text-[11px] text-gx-neon hover:bg-gx-neon/10 rounded transition-colors"
						>
							<Copy size={10} />
							Use Template
						</button>
					</div>
				</div>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
