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
		ChevronUp, Timer, Repeat, Merge, LayoutTemplate,
		Sparkles, CalendarClock, Variable, BarChart3,
		RefreshCw, Lightbulb, AlertTriangle, TrendingUp,
		Power, PowerOff, Key, GitCommit, Download, Upload,
		Split, Sigma, SortAsc, Layers, Fingerprint, Webhook,
		GitFork, Eye, EyeOff
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
		retry_config: RetryConfig | null;
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
		variables: Record<string, unknown>;
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
		input: unknown;
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

	interface RetryConfig {
		max_retries: number;
		backoff_ms: number;
		on_failure: string | { fallback: string };
	}

	interface WorkflowSchedule {
		workflow_id: string;
		cron_expression: string;
		enabled: boolean;
		next_run: string | null;
		timezone: string;
	}

	interface WorkflowAnalytics {
		total_runs: number;
		success_rate: number;
		avg_duration_ms: number;
		most_failed_node: string | null;
		last_7_days: DailyStats[];
	}

	interface DailyStats {
		date: string;
		runs: number;
		successes: number;
		failures: number;
		avg_duration_ms: number;
	}

	interface NodeSuggestion {
		node_type: NodeType;
		label: string;
		description: string;
		confidence: number;
	}

	interface CredentialMeta {
		id: string;
		name: string;
		credential_type: string;
		created_at: string;
	}

	interface WorkflowVersion {
		version: number;
		snapshot: unknown;
		message: string;
		created_at: string;
	}

	interface WebhookInfoData {
		workflow_id: string;
		path: string;
		method: string;
		url: string;
	}

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

	// AI generation state
	let showAiGenerate = $state(false);
	let aiDescription = $state('');
	let aiGenerating = $state(false);

	// AI suggestions state
	let suggestions = $state<NodeSuggestion[]>([]);
	let loadingSuggestions = $state(false);

	// Schedule state
	let showScheduleDialog = $state(false);
	let scheduleExpression = $state('0 0 9 * * *');
	let schedules = $state<WorkflowSchedule[]>([]);

	// Variables state
	let showVariables = $state(false);
	let variables = $state<Record<string, unknown>>({});
	let newVarKey = $state('');
	let newVarValue = $state('');

	// Analytics state
	let showAnalytics = $state(false);
	let analytics = $state<WorkflowAnalytics | null>(null);

	// Credential vault state
	let showCredentials = $state(false);
	let credentials = $state<CredentialMeta[]>([]);
	let newCredName = $state('');
	let newCredType = $state('api_key');
	let newCredValue = $state('');

	// Version history state
	let showVersions = $state(false);
	let versions = $state<WorkflowVersion[]>([]);
	let versionMessage = $state('');

	// Import/Export state
	let showImport = $state(false);
	let importJson = $state('');

	// Execution detail state
	let expandedRunId = $state<string | null>(null);

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
		{ group: 'Data', color: '#ec4899', items: [
			{ kind: 'action_split', label: 'Split', icon: Split, defaults: { field: '' } },
			{ kind: 'action_filter', label: 'Filter', icon: Filter, defaults: { expression: '' } },
			{ kind: 'action_sort', label: 'Sort', icon: SortAsc, defaults: { field: '', ascending: true } },
			{ kind: 'action_aggregate', label: 'Aggregate', icon: Sigma, defaults: { field: '', operation: 'sum' } },
			{ kind: 'action_map', label: 'Map', icon: Layers, defaults: { expression: '' } },
			{ kind: 'action_unique', label: 'Unique', icon: Fingerprint, defaults: { field: '' } },
			{ kind: 'action_sub_workflow', label: 'Sub-Workflow', icon: Workflow, defaults: { workflow_id: '' } },
		]},
		{ group: 'Control', color: '#a855f7', items: [
			{ kind: 'control_condition', label: 'Condition', icon: Filter, defaults: { expression: '' } },
			{ kind: 'control_loop', label: 'Loop', icon: Repeat, defaults: { count: 3 } },
			{ kind: 'control_delay', label: 'Delay', icon: Timer, defaults: { seconds: 5 } },
			{ kind: 'control_parallel', label: 'Parallel', icon: GitFork, defaults: {} },
			{ kind: 'control_merge', label: 'Merge', icon: Merge, defaults: {} },
		]},
	];

	// -----------------------------------------------------------------------
	// Color helpers
	// -----------------------------------------------------------------------

	function nodeColor(kind: string): string {
		if (kind.startsWith('trigger')) return '#22c55e';
		if (kind.startsWith('action_split') || kind.startsWith('action_filter') ||
			kind.startsWith('action_sort') || kind.startsWith('action_aggregate') ||
			kind.startsWith('action_map') || kind.startsWith('action_unique') ||
			kind.startsWith('action_sub_workflow')) return '#ec4899';
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
			await loadVariables();
			await loadAnalytics();
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
	// AI Generation
	// -----------------------------------------------------------------------

	async function aiGenerateWorkflow() {
		if (!aiDescription.trim()) return;
		aiGenerating = true;
		error = null;
		try {
			const wf = await invoke<FullWorkflow>('flow_ai_generate', {
				description: aiDescription.trim(),
			});
			showAiGenerate = false;
			aiDescription = '';
			await loadWorkflows();
			await openWorkflow(wf.id);
		} catch (e) {
			error = `AI generation failed: ${e}`;
		} finally {
			aiGenerating = false;
		}
	}

	async function loadSuggestions(nodeId: string) {
		if (!activeWorkflow) return;
		loadingSuggestions = true;
		try {
			suggestions = await invoke<NodeSuggestion[]>('flow_ai_suggest_next', {
				workflowId: activeWorkflow.id,
				currentNodeId: nodeId,
			});
		} catch {
			suggestions = [];
		} finally {
			loadingSuggestions = false;
		}
	}

	async function applySuggestion(suggestion: NodeSuggestion) {
		if (!activeWorkflow || !selectedNodeId) return;
		try {
			const lastNode = activeWorkflow.nodes.find(n => n.id === selectedNodeId);
			const posX = (lastNode?.position[0] ?? 200) + 280;
			const posY = lastNode?.position[1] ?? 200;
			const node = await invoke<FlowNode>('flow_add_node', {
				workflowId: activeWorkflow.id,
				nodeType: suggestion.node_type,
				label: suggestion.label,
				config: {},
				position: [posX, posY] as [number, number],
			});
			activeWorkflow.nodes = [...activeWorkflow.nodes, node];
			// Auto-connect from selected node
			await connectNodes(selectedNodeId, node.id);
			selectedNodeId = node.id;
		} catch (e) {
			error = `Failed to add suggested node: ${e}`;
		}
	}

	// Watch for selected node changes to load suggestions
	$effect(() => {
		if (selectedNodeId && activeWorkflow) {
			loadSuggestions(selectedNodeId);
		} else {
			suggestions = [];
		}
	});

	// -----------------------------------------------------------------------
	// Scheduling
	// -----------------------------------------------------------------------

	const CRON_PRESETS = [
		{ label: 'Every hour', cron: '0 0 * * * *' },
		{ label: 'Daily at 9am', cron: '0 0 9 * * *' },
		{ label: 'Daily at midnight', cron: '0 0 0 * * *' },
		{ label: 'Weekly on Monday', cron: '0 0 9 * * MON' },
		{ label: 'Monthly on 1st', cron: '0 0 9 1 * *' },
		{ label: 'Every 30 minutes', cron: '0 */30 * * * *' },
		{ label: 'Weekdays at 10am', cron: '0 0 10 * * MON-FRI' },
	];

	async function scheduleWorkflow() {
		if (!activeWorkflow) return;
		try {
			const sched = await invoke<WorkflowSchedule>('flow_schedule', {
				workflowId: activeWorkflow.id,
				cron: scheduleExpression,
			});
			showScheduleDialog = false;
			// Update local list
			schedules = schedules.filter(s => s.workflow_id !== sched.workflow_id);
			schedules = [sched, ...schedules];
		} catch (e) {
			error = `Failed to schedule: ${e}`;
		}
	}

	async function unscheduleWorkflow() {
		if (!activeWorkflow) return;
		try {
			await invoke('flow_unschedule', { workflowId: activeWorkflow.id });
			schedules = schedules.filter(s => s.workflow_id !== activeWorkflow?.id);
			showScheduleDialog = false;
		} catch (e) {
			error = `Failed to unschedule: ${e}`;
		}
	}

	async function loadSchedules() {
		try {
			schedules = await invoke<WorkflowSchedule[]>('flow_list_scheduled');
		} catch {
			schedules = [];
		}
	}

	function getCurrentSchedule(): WorkflowSchedule | undefined {
		return schedules.find(s => s.workflow_id === activeWorkflow?.id);
	}

	// -----------------------------------------------------------------------
	// Variables
	// -----------------------------------------------------------------------

	async function loadVariables() {
		if (!activeWorkflow) return;
		try {
			variables = await invoke<Record<string, unknown>>('flow_get_variables', {
				workflowId: activeWorkflow.id,
			});
		} catch {
			variables = {};
		}
	}

	async function setVariable() {
		if (!activeWorkflow || !newVarKey.trim()) return;
		try {
			let parsedValue: unknown = newVarValue;
			// Try to parse as JSON
			try { parsedValue = JSON.parse(newVarValue); } catch { /* keep as string */ }
			await invoke('flow_set_variable', {
				workflowId: activeWorkflow.id,
				key: newVarKey.trim(),
				value: parsedValue,
			});
			newVarKey = '';
			newVarValue = '';
			await loadVariables();
		} catch (e) {
			error = `Failed to set variable: ${e}`;
		}
	}

	async function deleteVariable(key: string) {
		if (!activeWorkflow) return;
		// Set to null effectively removes it (or we save the workflow directly)
		try {
			const wf = activeWorkflow;
			const vars = { ...variables };
			delete vars[key];
			wf.variables = vars;
			await invoke('flow_save', { id: wf.id, workflow: wf });
			variables = vars;
		} catch (e) {
			error = `Failed to delete variable: ${e}`;
		}
	}

	// -----------------------------------------------------------------------
	// Analytics
	// -----------------------------------------------------------------------

	async function loadAnalytics() {
		if (!activeWorkflow) return;
		try {
			analytics = await invoke<WorkflowAnalytics>('flow_analytics', {
				workflowId: activeWorkflow.id,
			});
		} catch {
			analytics = null;
		}
	}

	// -----------------------------------------------------------------------
	// Credentials
	// -----------------------------------------------------------------------

	async function loadCredentials() {
		try {
			credentials = await invoke<CredentialMeta[]>('flow_list_credentials');
		} catch {
			credentials = [];
		}
	}

	async function saveCredential() {
		if (!newCredName.trim() || !newCredValue.trim()) return;
		try {
			const data: Record<string, string> = {};
			if (newCredType === 'api_key' || newCredType === 'bearer') {
				data.token = newCredValue;
			} else if (newCredType === 'basic_auth') {
				const parts = newCredValue.split(':');
				data.username = parts[0] ?? '';
				data.password = parts.slice(1).join(':');
			} else {
				data.value = newCredValue;
			}
			await invoke('flow_save_credential', {
				name: newCredName.trim(),
				credentialType: newCredType,
				data,
			});
			newCredName = '';
			newCredValue = '';
			await loadCredentials();
		} catch (e) {
			error = `Failed to save credential: ${e}`;
		}
	}

	async function deleteCredential(id: string) {
		try {
			await invoke('flow_delete_credential', { id });
			await loadCredentials();
		} catch (e) {
			error = `Failed to delete credential: ${e}`;
		}
	}

	// -----------------------------------------------------------------------
	// Versioning
	// -----------------------------------------------------------------------

	async function loadVersions() {
		if (!activeWorkflow) return;
		try {
			versions = await invoke<WorkflowVersion[]>('flow_list_versions', {
				workflowId: activeWorkflow.id,
			});
		} catch {
			versions = [];
		}
	}

	async function saveVersion() {
		if (!activeWorkflow || !versionMessage.trim()) return;
		try {
			await invoke('flow_save_version', {
				workflowId: activeWorkflow.id,
				message: versionMessage.trim(),
			});
			versionMessage = '';
			await loadVersions();
		} catch (e) {
			error = `Failed to save version: ${e}`;
		}
	}

	async function rollbackVersion(version: number) {
		if (!activeWorkflow) return;
		try {
			activeWorkflow = await invoke<FullWorkflow>('flow_rollback', {
				workflowId: activeWorkflow.id,
				version,
			});
			await loadWorkflows();
			await loadVersions();
		} catch (e) {
			error = `Failed to rollback: ${e}`;
		}
	}

	// -----------------------------------------------------------------------
	// Import / Export
	// -----------------------------------------------------------------------

	async function exportWorkflow() {
		if (!activeWorkflow) return;
		try {
			const json = await invoke<string>('flow_export', {
				workflowId: activeWorkflow.id,
			});
			// Download as file
			const blob = new Blob([json], { type: 'application/json' });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `${activeWorkflow.name.replace(/\s+/g, '-').toLowerCase()}.json`;
			a.click();
			URL.revokeObjectURL(url);
		} catch (e) {
			error = `Failed to export: ${e}`;
		}
	}

	async function importWorkflow() {
		if (!importJson.trim()) return;
		try {
			const wf = await invoke<FullWorkflow>('flow_import', {
				json: importJson.trim(),
			});
			showImport = false;
			importJson = '';
			await loadWorkflows();
			await openWorkflow(wf.id);
		} catch (e) {
			error = `Failed to import: ${e}`;
		}
	}

	function handleImportFile(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const reader = new FileReader();
		reader.onload = () => {
			importJson = reader.result as string;
		};
		reader.readAsText(file);
	}

	// -----------------------------------------------------------------------
	// Workflow toggle + duplicate
	// -----------------------------------------------------------------------

	async function toggleWorkflow() {
		if (!activeWorkflow) return;
		try {
			const newEnabled = !activeWorkflow.enabled;
			await invoke('flow_toggle', {
				workflowId: activeWorkflow.id,
				enabled: newEnabled,
			});
			activeWorkflow.enabled = newEnabled;
			await loadWorkflows();
		} catch (e) {
			error = `Failed to toggle: ${e}`;
		}
	}

	async function duplicateWorkflow() {
		if (!activeWorkflow) return;
		try {
			const wf = await invoke<FullWorkflow>('flow_duplicate', {
				workflowId: activeWorkflow.id,
			});
			await loadWorkflows();
			await openWorkflow(wf.id);
		} catch (e) {
			error = `Failed to duplicate: ${e}`;
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
			case 'action_sub_workflow': return [{ key: 'workflow_id', label: 'Sub-Workflow ID', type: 'text' }];
			case 'action_split': return [{ key: 'field', label: 'Field to split', type: 'text' }];
			case 'action_filter': return [{ key: 'expression', label: 'Filter expression', type: 'text' }];
			case 'action_sort': return [
				{ key: 'field', label: 'Sort by field', type: 'text' },
				{ key: 'ascending', label: 'Direction', type: 'select', options: ['true', 'false'] },
			];
			case 'action_aggregate': return [
				{ key: 'field', label: 'Field', type: 'text' },
				{ key: 'operation', label: 'Operation', type: 'select', options: ['sum', 'avg', 'count', 'min', 'max'] },
			];
			case 'action_map': return [{ key: 'expression', label: 'Map expression (field path)', type: 'text' }];
			case 'action_unique': return [{ key: 'field', label: 'Unique by field', type: 'text' }];
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
		await loadSchedules();
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

				<!-- AI Generate -->
				<button
					onclick={() => showAiGenerate = true}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-accent-purple/40 text-gx-accent-purple hover:bg-gx-accent-purple/10 transition-colors"
					title="AI Generate Workflow"
				>
					<Sparkles size={11} />
					AI Generate
				</button>

				<!-- Schedule -->
				<button
					onclick={() => showScheduleDialog = true}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					class:border-gx-status-success={!!getCurrentSchedule()}
					class:text-gx-status-success={!!getCurrentSchedule()}
					title="Schedule"
				>
					<CalendarClock size={11} />
				</button>

				<!-- Variables -->
				<button
					onclick={() => { showVariables = !showVariables; if (showVariables) loadVariables(); }}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Workflow Variables"
				>
					<Variable size={11} />
				</button>

				<!-- Analytics -->
				<button
					onclick={() => { showAnalytics = !showAnalytics; if (showAnalytics) loadAnalytics(); }}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Analytics"
				>
					<BarChart3 size={11} />
				</button>

				<!-- Credentials -->
				<button
					onclick={() => { showCredentials = !showCredentials; if (showCredentials) loadCredentials(); }}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Credential Vault"
				>
					<Key size={11} />
				</button>

				<!-- Version History -->
				<button
					onclick={() => { showVersions = !showVersions; if (showVersions) loadVersions(); }}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Version History"
				>
					<GitCommit size={11} />
				</button>

				<!-- Export -->
				<button
					onclick={exportWorkflow}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Export Workflow"
				>
					<Download size={11} />
				</button>

				<!-- Import -->
				<button
					onclick={() => showImport = true}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Import Workflow"
				>
					<Upload size={11} />
				</button>

				<!-- Toggle enabled -->
				<button
					onclick={toggleWorkflow}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border transition-colors"
					class:border-gx-status-success={activeWorkflow.enabled}
					class:text-gx-status-success={activeWorkflow.enabled}
					class:border-gx-border-default={!activeWorkflow.enabled}
					class:text-gx-text-muted={!activeWorkflow.enabled}
					title={activeWorkflow.enabled ? 'Disable workflow' : 'Enable workflow'}
				>
					{#if activeWorkflow.enabled}
						<Eye size={11} />
					{:else}
						<EyeOff size={11} />
					{/if}
				</button>

				<!-- Duplicate -->
				<button
					onclick={duplicateWorkflow}
					class="flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-colors"
					title="Duplicate Workflow"
				>
					<Copy size={11} />
				</button>

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
								{@const tx = tgtNode.position[0]}
								{@const ty = tgtNode.position[1] + 30}
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
							{@const Icon = nodeIcon(node.node_type.kind)}
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
									<div class="px-2.5 py-1.5 flex items-center gap-1.5">
										<span class="text-[10px] text-gx-text-muted">{kindLabel(node.node_type.kind)}</span>
										{#if node.retry_config}
											<span class="ml-auto flex items-center gap-0.5 px-1 py-0.5 rounded bg-gx-status-warning/10 text-[9px] text-gx-status-warning" title="Retry: {node.retry_config.max_retries}x, backoff {node.retry_config.backoff_ms}ms">
												<RefreshCw size={8} />
												{node.retry_config.max_retries}x
											</span>
										{/if}
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

							<!-- Retry Config -->
							<div class="border-t border-gx-border-default pt-3">
								<span class="text-[10px] text-gx-text-muted uppercase tracking-wider">Error Recovery</span>
								{#if selectedNode?.retry_config}
									<div class="mt-2 space-y-1.5">
										<div class="flex items-center gap-1.5 text-[11px]">
											<RefreshCw size={10} class="text-gx-status-warning" />
											<span class="text-gx-text-secondary">Retries: {selectedNode.retry_config.max_retries}</span>
										</div>
										<div class="flex items-center gap-1.5 text-[11px]">
											<Timer size={10} class="text-gx-text-muted" />
											<span class="text-gx-text-secondary">Backoff: {selectedNode.retry_config.backoff_ms}ms</span>
										</div>
										<div class="flex items-center gap-1.5 text-[11px]">
											<AlertTriangle size={10} class="text-gx-text-muted" />
											<span class="text-gx-text-secondary">On fail: {typeof selectedNode.retry_config.on_failure === 'string' ? selectedNode.retry_config.on_failure : 'fallback'}</span>
										</div>
									</div>
								{:else}
									<p class="mt-1 text-[11px] text-gx-text-muted italic">No retry configured</p>
								{/if}
							</div>

							<!-- AI Suggestions -->
							<div class="border-t border-gx-border-default pt-3">
								<div class="flex items-center gap-1.5 mb-2">
									<Lightbulb size={10} class="text-gx-accent-purple" />
									<span class="text-[10px] text-gx-text-muted uppercase tracking-wider">AI Suggestions</span>
								</div>
								{#if loadingSuggestions}
									<div class="flex items-center gap-1.5 text-[11px] text-gx-text-muted">
										<Loader2 size={10} class="animate-spin" />
										Thinking...
									</div>
								{:else if suggestions.length > 0}
									<div class="space-y-1.5">
										{#each suggestions as suggestion}
											<button
												onclick={() => applySuggestion(suggestion)}
												class="w-full flex flex-col gap-0.5 p-2 rounded border border-gx-border-default hover:border-gx-accent-purple/40 hover:bg-gx-accent-purple/5 transition-colors text-left"
											>
												<div class="flex items-center gap-1.5">
													<Sparkles size={9} class="text-gx-accent-purple" />
													<span class="text-[11px] font-medium text-gx-text-secondary">{suggestion.label}</span>
													<span class="ml-auto text-[9px] text-gx-text-muted">{Math.round(suggestion.confidence * 100)}%</span>
												</div>
												<span class="text-[10px] text-gx-text-muted pl-4">{suggestion.description}</span>
											</button>
										{/each}
									</div>
								{:else}
									<p class="text-[11px] text-gx-text-muted italic">No suggestions available</p>
								{/if}
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

			<!-- Bottom: Run History (collapsible, with expandable node detail) -->
			{#if showRunHistory && runs.length > 0}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[300px] overflow-y-auto">
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
					{#each runs as run (run.id)}
						<div class="border-b border-gx-border-default/50">
							<button
								onclick={() => expandedRunId = expandedRunId === run.id ? null : run.id}
								class="w-full flex items-center gap-3 px-3 py-1.5 text-[11px] hover:bg-gx-bg-hover transition-colors"
							>
								<span class="font-mono text-gx-text-muted w-16 shrink-0">{run.id.slice(0, 8)}</span>
								{#if run.status === 'completed'}
									<span class="flex items-center gap-1 text-gx-status-success w-16 shrink-0">
										<CheckCircle2 size={10} /> Done
									</span>
								{:else if run.status === 'failed'}
									<span class="flex items-center gap-1 text-gx-status-error w-16 shrink-0">
										<XCircle size={10} /> Failed
									</span>
								{:else}
									<span class="flex items-center gap-1 text-gx-status-warning w-16 shrink-0">
										<Loader2 size={10} class="animate-spin" /> {run.status}
									</span>
								{/if}
								<span class="text-gx-text-muted">{new Date(run.started_at).toLocaleString()}</span>
								<span class="text-gx-text-muted">{run.node_results.length} nodes</span>
								{#if run.error}
									<span class="text-gx-status-error truncate flex-1">{run.error}</span>
								{/if}
								<div class="ml-auto">
									{#if expandedRunId === run.id}<ChevronUp size={10} />{:else}<ChevronDown size={10} />{/if}
								</div>
							</button>
							<!-- Expanded node detail -->
							{#if expandedRunId === run.id}
								<div class="px-3 pb-2 space-y-1">
									{#each run.node_results as nr}
										{@const nodeDef = activeWorkflow?.nodes.find(n => n.id === nr.node_id)}
										<div class="rounded border border-gx-border-default/50 bg-gx-bg-primary p-2">
											<div class="flex items-center gap-2 mb-1">
												{#if nr.status === 'completed'}
													<CheckCircle2 size={9} class="text-gx-status-success" />
												{:else}
													<XCircle size={9} class="text-gx-status-error" />
												{/if}
												<span class="text-[10px] font-medium text-gx-text-primary">{nodeDef?.label ?? nr.node_id.slice(0, 8)}</span>
												<span class="text-[9px] text-gx-text-muted">{nr.duration_ms}ms</span>
												{#if nr.error}
													<span class="text-[9px] text-gx-status-error ml-auto">{nr.error}</span>
												{/if}
											</div>
											<div class="grid grid-cols-2 gap-2">
												<div>
													<span class="text-[9px] text-gx-text-muted uppercase tracking-wider">Input</span>
													<pre class="mt-0.5 text-[9px] text-gx-text-secondary bg-gx-bg-tertiary rounded p-1 max-h-[60px] overflow-auto font-mono whitespace-pre-wrap break-all">{JSON.stringify(nr.input, null, 1)}</pre>
												</div>
												<div>
													<span class="text-[9px] text-gx-text-muted uppercase tracking-wider">Output</span>
													<pre class="mt-0.5 text-[9px] text-gx-text-secondary bg-gx-bg-tertiary rounded p-1 max-h-[60px] overflow-auto font-mono whitespace-pre-wrap break-all">{JSON.stringify(nr.output, null, 1)}</pre>
												</div>
											</div>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}

			<!-- Analytics Panel (collapsible) -->
			{#if showAnalytics && analytics}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[200px] overflow-y-auto">
					<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default">
						<BarChart3 size={12} class="text-gx-accent-purple" />
						<span class="text-[11px] font-medium text-gx-text-secondary">Analytics</span>
						<div class="flex-1"></div>
						<button onclick={() => showAnalytics = false} class="text-gx-text-muted hover:text-gx-text-secondary">
							<ChevronDown size={12} />
						</button>
					</div>
					<div class="p-3">
						<!-- Summary stats -->
						<div class="grid grid-cols-4 gap-3 mb-3">
							<div class="text-center">
								<div class="text-lg font-bold text-gx-text-primary">{analytics.total_runs}</div>
								<div class="text-[10px] text-gx-text-muted">Total Runs</div>
							</div>
							<div class="text-center">
								<div class="text-lg font-bold" class:text-gx-status-success={analytics.success_rate >= 0.8} class:text-gx-status-warning={analytics.success_rate >= 0.5 && analytics.success_rate < 0.8} class:text-gx-status-error={analytics.success_rate < 0.5}>
									{Math.round(analytics.success_rate * 100)}%
								</div>
								<div class="text-[10px] text-gx-text-muted">Success Rate</div>
							</div>
							<div class="text-center">
								<div class="text-lg font-bold text-gx-text-primary">
									{analytics.avg_duration_ms < 1000 ? `${analytics.avg_duration_ms}ms` : `${(analytics.avg_duration_ms / 1000).toFixed(1)}s`}
								</div>
								<div class="text-[10px] text-gx-text-muted">Avg Duration</div>
							</div>
							<div class="text-center">
								<div class="text-sm font-bold text-gx-status-error truncate" title={analytics.most_failed_node ?? 'None'}>
									{analytics.most_failed_node ? analytics.most_failed_node.slice(0, 8) : '--'}
								</div>
								<div class="text-[10px] text-gx-text-muted">Most Failed</div>
							</div>
						</div>
						<!-- Last 7 days bar chart -->
						<div class="flex items-end gap-1 h-16">
							{#each analytics.last_7_days as day}
								{@const maxRuns = Math.max(...analytics.last_7_days.map(d => d.runs), 1)}
								{@const height = day.runs > 0 ? Math.max((day.runs / maxRuns) * 100, 8) : 4}
								{@const successPct = day.runs > 0 ? (day.successes / day.runs) * 100 : 0}
								<div class="flex-1 flex flex-col items-center gap-0.5">
									<div
										class="w-full rounded-t transition-all"
										style="height: {height}%; background: linear-gradient(to top, {successPct > 80 ? 'var(--gx-status-success)' : successPct > 50 ? 'var(--gx-status-warning)' : 'var(--gx-status-error)'}, transparent);"
										title="{day.date}: {day.runs} runs ({day.successes}ok, {day.failures}fail)"
									></div>
									<span class="text-[8px] text-gx-text-muted">{day.date.slice(5)}</span>
								</div>
							{/each}
						</div>
					</div>
				</div>
			{/if}

			<!-- Variables Panel (collapsible) -->
			{#if showVariables}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[200px] overflow-y-auto">
					<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default">
						<Variable size={12} class="text-gx-neon" />
						<span class="text-[11px] font-medium text-gx-text-secondary">Variables</span>
						<span class="text-[10px] text-gx-text-muted ml-1">Use as {'{{var.name}}'}</span>
						<div class="flex-1"></div>
						<button onclick={() => showVariables = false} class="text-gx-text-muted hover:text-gx-text-secondary">
							<ChevronDown size={12} />
						</button>
					</div>
					<div class="p-2 space-y-1.5">
						<!-- Existing variables -->
						{#each Object.entries(variables) as [key, val]}
							<div class="flex items-center gap-2 px-2 py-1 bg-gx-bg-tertiary rounded text-[11px]">
								<span class="font-mono text-gx-neon">{key}</span>
								<span class="text-gx-text-muted">=</span>
								<span class="flex-1 truncate text-gx-text-secondary">{typeof val === 'string' ? val : JSON.stringify(val)}</span>
								<button onclick={() => deleteVariable(key)} class="text-gx-text-muted hover:text-gx-status-error shrink-0">
									<XCircle size={10} />
								</button>
							</div>
						{/each}
						<!-- Add new variable -->
						<div class="flex items-center gap-1.5">
							<input
								type="text"
								bind:value={newVarKey}
								placeholder="key"
								class="w-24 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none focus:border-gx-neon font-mono"
							/>
							<span class="text-gx-text-muted text-[11px]">=</span>
							<input
								type="text"
								bind:value={newVarValue}
								placeholder="value"
								class="flex-1 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none focus:border-gx-neon"
							/>
							<button
								onclick={setVariable}
								disabled={!newVarKey.trim()}
								class="px-2 py-1 text-[11px] text-gx-neon border border-gx-neon/30 rounded hover:bg-gx-neon/10 disabled:opacity-40 transition-colors"
							>
								<Plus size={10} />
							</button>
						</div>
					</div>
				</div>
			{/if}

			<!-- Credentials Panel (collapsible) -->
			{#if showCredentials}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[200px] overflow-y-auto">
					<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default">
						<Key size={12} class="text-gx-status-warning" />
						<span class="text-[11px] font-medium text-gx-text-secondary">Credential Vault</span>
						<span class="text-[10px] text-gx-text-muted ml-1">Use as {'{{cred.name}}'}</span>
						<div class="flex-1"></div>
						<button onclick={() => showCredentials = false} class="text-gx-text-muted hover:text-gx-text-secondary">
							<ChevronDown size={12} />
						</button>
					</div>
					<div class="p-2 space-y-1.5">
						{#each credentials as cred (cred.id)}
							<div class="flex items-center gap-2 px-2 py-1 bg-gx-bg-tertiary rounded text-[11px]">
								<Key size={9} class="text-gx-status-warning shrink-0" />
								<span class="font-mono text-gx-status-warning">{cred.name}</span>
								<Badge variant="outline" class="text-[9px] px-1 h-3.5 border-gx-border-default text-gx-text-muted">
									{cred.credential_type}
								</Badge>
								<span class="flex-1 text-[9px] text-gx-text-muted">{new Date(cred.created_at).toLocaleDateString()}</span>
								<button onclick={() => deleteCredential(cred.id)} class="text-gx-text-muted hover:text-gx-status-error shrink-0">
									<XCircle size={10} />
								</button>
							</div>
						{/each}
						{#if credentials.length === 0}
							<p class="text-[11px] text-gx-text-muted italic px-2">No credentials stored yet</p>
						{/if}
						<div class="flex items-center gap-1.5 pt-1">
							<input
								type="text"
								bind:value={newCredName}
								placeholder="name"
								class="w-20 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none focus:border-gx-status-warning font-mono"
							/>
							<select
								bind:value={newCredType}
								class="w-20 px-1 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none"
							>
								<option value="api_key">API Key</option>
								<option value="bearer">Bearer</option>
								<option value="basic_auth">Basic Auth</option>
								<option value="oauth2">OAuth2</option>
							</select>
							<input
								type="password"
								bind:value={newCredValue}
								placeholder="secret value"
								class="flex-1 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none focus:border-gx-status-warning"
							/>
							<button
								onclick={saveCredential}
								disabled={!newCredName.trim() || !newCredValue.trim()}
								class="px-2 py-1 text-[11px] text-gx-status-warning border border-gx-status-warning/30 rounded hover:bg-gx-status-warning/10 disabled:opacity-40 transition-colors"
							>
								<Plus size={10} />
							</button>
						</div>
					</div>
				</div>
			{/if}

			<!-- Version History Panel (collapsible) -->
			{#if showVersions}
				<div class="border-t border-gx-border-default bg-gx-bg-secondary shrink-0 max-h-[200px] overflow-y-auto">
					<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default">
						<GitCommit size={12} class="text-gx-accent-purple" />
						<span class="text-[11px] font-medium text-gx-text-secondary">Version History</span>
						<Badge variant="outline" class="text-[9px] px-1 h-3.5 border-gx-border-default text-gx-text-muted">
							{versions.length}
						</Badge>
						<div class="flex-1"></div>
						<button onclick={() => showVersions = false} class="text-gx-text-muted hover:text-gx-text-secondary">
							<ChevronDown size={12} />
						</button>
					</div>
					<div class="p-2 space-y-1.5">
						<!-- Save new version -->
						<div class="flex items-center gap-1.5">
							<input
								type="text"
								bind:value={versionMessage}
								placeholder="Version message..."
								class="flex-1 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded text-[11px] text-gx-text-primary outline-none focus:border-gx-accent-purple"
							/>
							<button
								onclick={saveVersion}
								disabled={!versionMessage.trim()}
								class="px-2 py-1 text-[11px] text-gx-accent-purple border border-gx-accent-purple/30 rounded hover:bg-gx-accent-purple/10 disabled:opacity-40 transition-colors"
							>
								Save
							</button>
						</div>
						<!-- Version list -->
						{#each versions as ver (ver.version)}
							<div class="flex items-center gap-2 px-2 py-1 bg-gx-bg-tertiary rounded text-[11px]">
								<GitCommit size={9} class="text-gx-accent-purple shrink-0" />
								<span class="font-mono text-gx-accent-purple">v{ver.version}</span>
								<span class="flex-1 truncate text-gx-text-secondary">{ver.message}</span>
								<span class="text-[9px] text-gx-text-muted shrink-0">{new Date(ver.created_at).toLocaleDateString()}</span>
								<button
									onclick={() => rollbackVersion(ver.version)}
									class="text-[10px] text-gx-status-warning hover:underline shrink-0"
									title="Rollback to v{ver.version}"
								>
									Rollback
								</button>
							</div>
						{/each}
						{#if versions.length === 0}
							<p class="text-[11px] text-gx-text-muted italic px-2">No versions saved yet</p>
						{/if}
					</div>
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
				<div class="flex items-center justify-center gap-3 flex-wrap">
					<button
						onclick={() => showNewDialog = true}
						class="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 hover:shadow-gx-glow-sm transition-all"
					>
						<Plus size={16} />
						New Workflow
					</button>
					<button
						onclick={() => showAiGenerate = true}
						class="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-gx-accent-purple/10 text-gx-accent-purple border border-gx-accent-purple/30 rounded-gx hover:bg-gx-accent-purple/20 hover:shadow-gx-glow-sm transition-all"
					>
						<Sparkles size={16} />
						AI Generate
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

<!-- AI Generate Dialog -->
<Dialog.Root bind:open={showAiGenerate}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-lg">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-gx-text-primary">
				<Sparkles size={18} class="text-gx-accent-purple" />
				AI Workflow Generator
			</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Describe what you want automated in plain language. AI will build the workflow for you.
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-3">
			<div>
				<label class="text-xs text-gx-text-muted" for="ai-desc">Describe your workflow</label>
				<textarea
					id="ai-desc"
					bind:value={aiDescription}
					placeholder="Every morning at 9am, fetch HackerNews top stories, summarize them with AI, and email me the digest..."
					rows="4"
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-sm text-gx-text-primary outline-none focus:border-gx-accent-purple resize-none"
				></textarea>
			</div>
			<div class="flex flex-wrap gap-1.5">
				<span class="text-[10px] text-gx-text-muted">Examples:</span>
				{#each [
					'Monitor a price API every 30 min and email me when price drops below $100',
					'Every weekday at 10am, review my git changes with AI and send a report',
					'Watch my downloads folder, extract text from new PDFs, and save summaries to database'
				] as example}
					<button
						onclick={() => aiDescription = example}
						class="px-2 py-0.5 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded text-gx-text-muted hover:text-gx-accent-purple hover:border-gx-accent-purple/30 transition-colors"
					>
						{example.slice(0, 50)}...
					</button>
				{/each}
			</div>
		</div>
		<Dialog.Footer>
			<button onclick={() => showAiGenerate = false} class="px-3 py-1.5 text-sm text-gx-text-muted hover:text-gx-text-secondary">Cancel</button>
			<button
				onclick={aiGenerateWorkflow}
				disabled={!aiDescription.trim() || aiGenerating}
				class="flex items-center gap-2 px-4 py-1.5 text-sm font-medium bg-gx-accent-purple/10 text-gx-accent-purple border border-gx-accent-purple/30 rounded-gx hover:bg-gx-accent-purple/20 disabled:opacity-40 transition-colors"
			>
				{#if aiGenerating}
					<Loader2 size={14} class="animate-spin" />
					Generating...
				{:else}
					<Sparkles size={14} />
					Generate Workflow
				{/if}
			</button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Import Dialog -->
<Dialog.Root bind:open={showImport}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-lg">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-gx-text-primary">
				<Upload size={18} class="text-gx-neon" />
				Import Workflow
			</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Import a workflow from a JSON file or paste JSON directly.
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-3">
			<div>
				<label class="text-xs text-gx-text-muted" for="import-file">Choose file</label>
				<input
					id="import-file"
					type="file"
					accept=".json"
					onchange={handleImportFile}
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-sm text-gx-text-primary outline-none file:mr-3 file:py-1 file:px-3 file:rounded file:border-0 file:text-xs file:bg-gx-neon/10 file:text-gx-neon"
				/>
			</div>
			<div>
				<label class="text-xs text-gx-text-muted" for="import-json">Or paste JSON</label>
				<textarea
					id="import-json"
					bind:value={importJson}
					placeholder={'{"name": "My Workflow", "nodes": [...], "edges": [...]}'}
					rows="6"
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs text-gx-text-primary font-mono outline-none focus:border-gx-neon resize-none"
				></textarea>
			</div>
		</div>
		<Dialog.Footer>
			<button onclick={() => showImport = false} class="px-3 py-1.5 text-sm text-gx-text-muted hover:text-gx-text-secondary">Cancel</button>
			<button
				onclick={importWorkflow}
				disabled={!importJson.trim()}
				class="flex items-center gap-2 px-4 py-1.5 text-sm font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 disabled:opacity-40 transition-colors"
			>
				<Upload size={14} />
				Import
			</button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Schedule Dialog -->
<Dialog.Root bind:open={showScheduleDialog}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-sm">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-gx-text-primary">
				<CalendarClock size={18} class="text-gx-neon" />
				Schedule Workflow
			</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Set a cron schedule to run this workflow automatically.
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-3">
			<!-- Cron presets -->
			<div>
				<label class="text-xs text-gx-text-muted">Quick presets</label>
				<div class="flex flex-wrap gap-1.5 mt-1">
					{#each CRON_PRESETS as preset}
						<button
							onclick={() => scheduleExpression = preset.cron}
							class="px-2 py-1 text-[11px] rounded border transition-colors
								{scheduleExpression === preset.cron
									? 'border-gx-neon/40 text-gx-neon bg-gx-neon/10'
									: 'border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary'}"
						>
							{preset.label}
						</button>
					{/each}
				</div>
			</div>
			<!-- Custom expression -->
			<div>
				<label class="text-xs text-gx-text-muted" for="cron-expr">Cron Expression</label>
				<input
					id="cron-expr"
					type="text"
					bind:value={scheduleExpression}
					class="w-full mt-1 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded text-sm text-gx-text-primary font-mono outline-none focus:border-gx-neon"
				/>
				<p class="mt-1 text-[10px] text-gx-text-muted">Format: second minute hour day month weekday</p>
			</div>
			<!-- Current schedule info -->
			{#if getCurrentSchedule()}
				{@const currentSched = getCurrentSchedule()!}
				<div class="p-2 bg-gx-bg-tertiary rounded border border-gx-border-default">
					<div class="flex items-center gap-1.5 text-[11px] mb-1">
						{#if currentSched.enabled}
							<Power size={10} class="text-gx-status-success" />
							<span class="text-gx-status-success">Active</span>
						{:else}
							<PowerOff size={10} class="text-gx-text-muted" />
							<span class="text-gx-text-muted">Disabled</span>
						{/if}
					</div>
					{#if currentSched.next_run}
						<div class="flex items-center gap-1.5 text-[11px] text-gx-text-secondary">
							<Clock size={10} class="text-gx-text-muted" />
							Next: {new Date(currentSched.next_run).toLocaleString()}
						</div>
					{/if}
				</div>
			{/if}
		</div>
		<Dialog.Footer>
			{#if getCurrentSchedule()}
				<button
					onclick={unscheduleWorkflow}
					class="px-3 py-1.5 text-sm text-gx-status-error hover:bg-gx-status-error/10 rounded transition-colors"
				>
					Remove Schedule
				</button>
			{/if}
			<div class="flex-1"></div>
			<button onclick={() => showScheduleDialog = false} class="px-3 py-1.5 text-sm text-gx-text-muted hover:text-gx-text-secondary">Cancel</button>
			<button
				onclick={scheduleWorkflow}
				class="px-4 py-1.5 text-sm font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20"
			>
				Save Schedule
			</button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
