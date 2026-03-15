<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Download, Plus, Loader2, AlertCircle, X, RefreshCw,
		Trash2, Play, Clock, CheckCircle2, XCircle, Eye,
		ChevronDown, Settings, Power, PowerOff, ArrowRight,
		Calendar as CalendarIcon, Table2, FileEdit, FileText,
		FolderOpen, Newspaper, PenTool, Globe, Cloud, Box,
		HardDrive, Rss, MoreHorizontal, History, Zap
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-import';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface ImportSource {
		id: string;
		name: string;
		source_type: string;
		url: string;
		auto_import: boolean;
		import_interval_hours: number;
		last_imported: string | null;
		status: { status: string; message?: string };
		target_module: string;
		created_at: string;
	}

	interface ImportJob {
		id: string;
		source_id: string;
		source_name: string;
		started_at: string;
		completed_at: string | null;
		status: { status: string; message?: string };
		items_imported: number;
		target_module: string;
		automation_steps: CdpStep[];
		error_details: string | null;
	}

	interface CdpStep {
		step_type: string;
		url?: string;
		selector?: string;
		value?: string;
		timeout_ms?: number;
		ms?: number;
		label?: string;
		script?: string;
	}

	interface SourceTypeDef {
		type: string;
		label: string;
		icon: string;
		default_target: string;
	}

	interface TargetModule {
		id: string;
		label: string;
		icon: string;
	}

	// ---- State ---------------------------------------------------------------
	let sources = $state<ImportSource[]>([]);
	let history = $state<ImportJob[]>([]);
	let sourceTypes = $state<SourceTypeDef[]>([]);
	let targetModules = $state<TargetModule[]>([]);
	let loading = $state(true);
	let error = $state('');

	// Add source dialog
	let showAddDialog = $state(false);
	let addForm = $state({
		name: '',
		sourceType: '' as string,
		url: '',
		targetModule: ''
	});
	let addLoading = $state(false);

	// Steps preview dialog
	let showStepsDialog = $state(false);
	let previewSteps = $state<CdpStep[]>([]);
	let previewSourceId = $state('');
	let previewSourceName = $state('');
	let runningImport = $state(false);

	// Active tab
	type TabId = 'sources' | 'history';
	let activeTab = $state<TabId>('sources');

	// ---- Data loading --------------------------------------------------------
	async function loadSources() {
		try {
			sources = await invoke<ImportSource[]>('autoimport_list_sources');
		} catch (e) {
			console.error('Failed to load import sources:', e);
		}
	}

	async function loadHistory() {
		try {
			history = await invoke<ImportJob[]>('autoimport_history', { limit: 50 });
		} catch (e) {
			console.error('Failed to load import history:', e);
		}
	}

	async function loadSourceTypes() {
		try {
			sourceTypes = await invoke<SourceTypeDef[]>('autoimport_source_types');
		} catch (e) {
			console.error('Failed to load source types:', e);
		}
	}

	async function loadTargetModules() {
		try {
			targetModules = await invoke<TargetModule[]>('autoimport_target_modules');
		} catch (e) {
			console.error('Failed to load target modules:', e);
		}
	}

	async function loadAll() {
		loading = true;
		error = '';
		try {
			await Promise.all([
				loadSources(),
				loadHistory(),
				loadSourceTypes(),
				loadTargetModules()
			]);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadAll();
	});

	// ---- Actions -------------------------------------------------------------
	async function addSource() {
		if (!addForm.name || !addForm.sourceType || !addForm.url) return;
		addLoading = true;
		try {
			await invoke('autoimport_add_source', {
				name: addForm.name,
				sourceType: addForm.sourceType,
				url: addForm.url,
				targetModule: addForm.targetModule || null
			});
			showAddDialog = false;
			addForm = { name: '', sourceType: '', url: '', targetModule: '' };
			await loadSources();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			addLoading = false;
		}
	}

	async function removeSource(id: string) {
		try {
			await invoke('autoimport_remove_source', { id });
			await loadSources();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function toggleAutoImport(id: string, enabled: boolean) {
		try {
			await invoke('autoimport_toggle', {
				id,
				autoImport: enabled,
				intervalHours: null
			});
			await loadSources();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function previewImport(source: ImportSource) {
		try {
			previewSteps = await invoke<CdpStep[]>('autoimport_get_steps', {
				sourceType: source.source_type,
				url: source.url
			});
			previewSourceId = source.id;
			previewSourceName = source.name;
			showStepsDialog = true;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function executeImport(sourceId: string) {
		runningImport = true;
		try {
			const job = await invoke<ImportJob>('autoimport_run', { sourceId });
			// Simulate CDP execution completing (in production, the CDP engine
			// would execute steps and call autoimport_complete)
			await invoke('autoimport_complete', {
				jobId: job.id,
				success: true,
				itemsImported: 1,
				errorMessage: null
			});
			showStepsDialog = false;
			await Promise.all([loadSources(), loadHistory()]);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			runningImport = false;
		}
	}

	async function resetStatus(id: string) {
		try {
			await invoke('autoimport_reset_status', { id });
			await loadSources();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Helpers -------------------------------------------------------------
	function statusColor(status: { status: string }): string {
		switch (status.status) {
			case 'Idle': return 'text-gx-text-muted';
			case 'Importing': return 'text-gx-status-warning';
			case 'Success': return 'text-gx-status-success';
			case 'Failed': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusLabel(status: { status: string; message?: string }): string {
		if (status.status === 'Failed' && status.message) {
			return `Failed: ${status.message}`;
		}
		return status.status;
	}

	function sourceTypeIcon(sourceType: string) {
		switch (sourceType) {
			case 'google_calendar': return CalendarIcon;
			case 'outlook_calendar': return CalendarIcon;
			case 'google_drive': return HardDrive;
			case 'onedrive': return Cloud;
			case 'dropbox': return Box;
			case 'google_sheets': return Table2;
			case 'office365': return FileText;
			case 'generic_website': return Globe;
			case 'rss_feed': return Rss;
			case 'custom': return Settings;
			default: return Download;
		}
	}

	function targetModuleIcon(moduleId: string) {
		switch (moduleId) {
			case 'calendar': return CalendarIcon;
			case 'sheets': return Table2;
			case 'writer': return FileEdit;
			case 'pdf': return FileText;
			case 'files': return FolderOpen;
			case 'news': return Newspaper;
			case 'canvas': return PenTool;
			default: return FolderOpen;
		}
	}

	function stepTypeLabel(step: CdpStep): string {
		switch (step.step_type) {
			case 'Navigate': return `Navigate to ${step.url}`;
			case 'WaitFor': return `Wait for element: ${step.selector}`;
			case 'Click': return `Click: ${step.selector}`;
			case 'Fill': return `Fill: ${step.selector}`;
			case 'Screenshot': return `Screenshot: ${step.label}`;
			case 'Wait': return `Wait ${step.ms}ms`;
			case 'ExecuteJs': return 'Execute JavaScript';
			default: return step.step_type;
		}
	}

	function formatDate(iso: string | null): string {
		if (!iso) return 'Never';
		try {
			const d = new Date(iso);
			return d.toLocaleString(undefined, {
				month: 'short', day: 'numeric',
				hour: '2-digit', minute: '2-digit'
			});
		} catch {
			return iso;
		}
	}

	function timeAgo(iso: string): string {
		try {
			const d = new Date(iso);
			const now = new Date();
			const diffMs = now.getTime() - d.getTime();
			const diffMin = Math.floor(diffMs / 60000);
			if (diffMin < 1) return 'just now';
			if (diffMin < 60) return `${diffMin}m ago`;
			const diffHr = Math.floor(diffMin / 60);
			if (diffHr < 24) return `${diffHr}h ago`;
			const diffDays = Math.floor(diffHr / 24);
			return `${diffDays}d ago`;
		} catch {
			return iso;
		}
	}

	// Set default target module when source type changes
	$effect(() => {
		if (addForm.sourceType && !addForm.targetModule) {
			const match = sourceTypes.find(t => {
				const serialized = JSON.stringify(t.type);
				return serialized.includes(addForm.sourceType);
			});
			if (match) {
				addForm.targetModule = match.default_target;
			}
		}
	});
</script>

<div class="h-full overflow-y-auto {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<div class="max-w-6xl mx-auto p-6 space-y-6">

		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class="flex items-center justify-center w-10 h-10 rounded-gx bg-gx-accent-blue/15 border border-gx-accent-blue/30">
					<Download size={20} class="text-gx-accent-blue" />
				</div>
				<div>
					<h1 class="text-xl font-bold text-gx-text-primary">Import Manager</h1>
					<p class="text-xs text-gx-text-muted">Auto-import data from Google Calendar, Outlook, Drive, Sheets, and more</p>
				</div>
			</div>
			<div class="flex items-center gap-2">
				<button
					onclick={() => loadAll()}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				>
					<RefreshCw size={13} class={loading ? 'animate-spin' : ''} />
					Refresh
				</button>
				<button
					onclick={() => { showAddDialog = true; addForm = { name: '', sourceType: '', url: '', targetModule: '' }; }}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx bg-gx-accent-blue text-white hover:bg-gx-accent-blue/80 transition-colors"
				>
					<Plus size={13} />
					Add Source
				</button>
			</div>
		</div>

		<!-- Error Banner -->
		{#if error}
			<div class="flex items-center gap-2 p-3 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-gx-status-error text-sm">
				<AlertCircle size={16} />
				<span class="flex-1">{error}</span>
				<button onclick={() => error = ''}>
					<X size={14} />
				</button>
			</div>
		{/if}

		<!-- Tabs -->
		<div class="flex items-center gap-1 border-b border-gx-border-default">
			<button
				onclick={() => activeTab = 'sources'}
				class="px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px
					{activeTab === 'sources'
						? 'border-gx-neon text-gx-neon'
						: 'border-transparent text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				<span class="flex items-center gap-1.5">
					<Zap size={14} />
					Sources
					{#if sources.length > 0}
						<Badge variant="secondary" class="text-[10px] px-1.5 h-4">{sources.length}</Badge>
					{/if}
				</span>
			</button>
			<button
				onclick={() => activeTab = 'history'}
				class="px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px
					{activeTab === 'history'
						? 'border-gx-neon text-gx-neon'
						: 'border-transparent text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				<span class="flex items-center gap-1.5">
					<History size={14} />
					Import History
					{#if history.length > 0}
						<Badge variant="secondary" class="text-[10px] px-1.5 h-4">{history.length}</Badge>
					{/if}
				</span>
			</button>
		</div>

		<!-- Loading State -->
		{#if loading}
			<div class="flex items-center justify-center py-20">
				<Loader2 size={24} class="animate-spin text-gx-neon" />
				<span class="ml-2 text-sm text-gx-text-muted">Loading import sources...</span>
			</div>

		<!-- Sources Tab -->
		{:else if activeTab === 'sources'}
			{#if sources.length === 0}
				<div class="flex flex-col items-center justify-center py-20 gap-4">
					<div class="w-16 h-16 rounded-full bg-gx-bg-elevated flex items-center justify-center">
						<Download size={28} class="text-gx-text-muted" />
					</div>
					<div class="text-center">
						<p class="text-sm text-gx-text-secondary font-medium">No import sources configured</p>
						<p class="text-xs text-gx-text-muted mt-1">Add a source to start auto-importing data from external services</p>
					</div>
					<button
						onclick={() => { showAddDialog = true; addForm = { name: '', sourceType: '', url: '', targetModule: '' }; }}
						class="flex items-center gap-1.5 px-4 py-2 text-sm rounded-gx bg-gx-accent-blue text-white hover:bg-gx-accent-blue/80 transition-colors"
					>
						<Plus size={14} />
						Add Your First Source
					</button>
				</div>
			{:else}
				<div class="grid gap-3">
					{#each sources as source (source.id)}
						{@const TypeIcon = sourceTypeIcon(source.source_type)}
						{@const TargetIcon = targetModuleIcon(source.target_module)}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 hover:border-gx-border-hover transition-colors">
							<div class="flex items-start gap-3">
								<!-- Source icon -->
								<div class="flex items-center justify-center w-10 h-10 rounded-gx bg-gx-bg-elevated border border-gx-border-default shrink-0">
									<TypeIcon size={18} class="text-gx-accent-blue" />
								</div>

								<!-- Source info -->
								<div class="flex-1 min-w-0">
									<div class="flex items-center gap-2 mb-0.5">
										<span class="text-sm font-semibold text-gx-text-primary truncate">{source.name}</span>
										<Badge variant="outline" class="text-[10px] px-1.5 h-4 border-gx-border-default">
											{source.source_type.replace(/_/g, ' ')}
										</Badge>
									</div>
									<p class="text-xs text-gx-text-muted truncate mb-2">{source.url}</p>

									<div class="flex items-center gap-4 text-[11px]">
										<!-- Status -->
										<span class="flex items-center gap-1 {statusColor(source.status)}">
											{#if source.status.status === 'Importing'}
												<Loader2 size={11} class="animate-spin" />
											{:else if source.status.status === 'Success'}
												<CheckCircle2 size={11} />
											{:else if source.status.status === 'Failed'}
												<XCircle size={11} />
											{:else}
												<Clock size={11} />
											{/if}
											{statusLabel(source.status)}
										</span>

										<Separator orientation="vertical" class="h-3 bg-gx-border-default" />

										<!-- Target module -->
										<span class="flex items-center gap-1 text-gx-text-muted">
											<ArrowRight size={10} />
											<TargetIcon size={11} />
											{source.target_module}
										</span>

										<Separator orientation="vertical" class="h-3 bg-gx-border-default" />

										<!-- Last imported -->
										<span class="text-gx-text-muted">
											Last: {formatDate(source.last_imported)}
										</span>

										{#if source.auto_import}
											<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
											<span class="flex items-center gap-1 text-gx-accent-blue">
												<RefreshCw size={10} />
												Every {source.import_interval_hours}h
											</span>
										{/if}
									</div>
								</div>

								<!-- Actions -->
								<div class="flex items-center gap-1 shrink-0">
									<!-- Auto-import toggle -->
									<button
										onclick={() => toggleAutoImport(source.id, !source.auto_import)}
										title={source.auto_import ? 'Disable auto-import' : 'Enable auto-import'}
										class="flex items-center justify-center w-8 h-8 rounded-gx transition-colors
											{source.auto_import
												? 'bg-gx-accent-blue/15 text-gx-accent-blue hover:bg-gx-accent-blue/25'
												: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
									>
										{#if source.auto_import}
											<Power size={14} />
										{:else}
											<PowerOff size={14} />
										{/if}
									</button>

									<!-- Preview and run -->
									<button
										onclick={() => previewImport(source)}
										disabled={source.status.status === 'Importing'}
										title="Preview and run import"
										class="flex items-center justify-center w-8 h-8 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
									>
										<Play size={14} />
									</button>

									<!-- Reset status (shown when failed) -->
									{#if source.status.status === 'Failed'}
										<button
											onclick={() => resetStatus(source.id)}
											title="Reset status"
											class="flex items-center justify-center w-8 h-8 rounded-gx text-gx-status-warning hover:bg-gx-status-warning/10 transition-colors"
										>
											<RefreshCw size={14} />
										</button>
									{/if}

									<!-- Delete -->
									<button
										onclick={() => removeSource(source.id)}
										title="Remove source"
										class="flex items-center justify-center w-8 h-8 rounded-gx text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-colors"
									>
										<Trash2 size={14} />
									</button>
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/if}

		<!-- History Tab -->
		{:else if activeTab === 'history'}
			{#if history.length === 0}
				<div class="flex flex-col items-center justify-center py-20 gap-3">
					<History size={28} class="text-gx-text-muted" />
					<p class="text-sm text-gx-text-muted">No import history yet</p>
				</div>
			{:else}
				<div class="rounded-gx border border-gx-border-default overflow-hidden">
					<table class="w-full text-xs">
						<thead>
							<tr class="bg-gx-bg-secondary border-b border-gx-border-default">
								<th class="text-left px-3 py-2 font-medium text-gx-text-muted">Source</th>
								<th class="text-left px-3 py-2 font-medium text-gx-text-muted">Target</th>
								<th class="text-left px-3 py-2 font-medium text-gx-text-muted">Started</th>
								<th class="text-left px-3 py-2 font-medium text-gx-text-muted">Duration</th>
								<th class="text-right px-3 py-2 font-medium text-gx-text-muted">Items</th>
								<th class="text-left px-3 py-2 font-medium text-gx-text-muted">Status</th>
							</tr>
						</thead>
						<tbody>
							{#each history as job (job.id)}
								<tr class="border-b border-gx-border-default/50 hover:bg-gx-bg-hover/50 transition-colors">
									<td class="px-3 py-2">
										<span class="text-gx-text-secondary font-medium">{job.source_name}</span>
									</td>
									<td class="px-3 py-2">
										<Badge variant="outline" class="text-[10px] px-1.5 h-4 border-gx-border-default">
											{job.target_module}
										</Badge>
									</td>
									<td class="px-3 py-2 text-gx-text-muted" title={job.started_at}>
										{timeAgo(job.started_at)}
									</td>
									<td class="px-3 py-2 text-gx-text-muted">
										{#if job.completed_at}
											{@const durationMs = new Date(job.completed_at).getTime() - new Date(job.started_at).getTime()}
											{#if durationMs < 1000}
												<1s
											{:else if durationMs < 60000}
												{Math.floor(durationMs / 1000)}s
											{:else}
												{Math.floor(durationMs / 60000)}m {Math.floor((durationMs % 60000) / 1000)}s
											{/if}
										{:else}
											<Loader2 size={11} class="animate-spin" />
										{/if}
									</td>
									<td class="px-3 py-2 text-right text-gx-text-secondary font-mono">
										{job.items_imported}
									</td>
									<td class="px-3 py-2">
										<span class="flex items-center gap-1 {statusColor(job.status)}">
											{#if job.status.status === 'Importing'}
												<Loader2 size={11} class="animate-spin" />
											{:else if job.status.status === 'Success'}
												<CheckCircle2 size={11} />
											{:else if job.status.status === 'Failed'}
												<XCircle size={11} />
											{/if}
											{statusLabel(job.status)}
										</span>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		{/if}
	</div>
</div>

<!-- Add Source Dialog -->
{#if showAddDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
		<div class="w-full max-w-lg rounded-gx border border-gx-border-default bg-gx-bg-elevated shadow-gx-glow-lg p-6 m-4">
			<div class="flex items-center justify-between mb-5">
				<h2 class="text-base font-bold text-gx-text-primary flex items-center gap-2">
					<Plus size={16} class="text-gx-accent-blue" />
					Add Import Source
				</h2>
				<button onclick={() => showAddDialog = false} class="text-gx-text-muted hover:text-gx-text-primary">
					<X size={16} />
				</button>
			</div>

			<div class="space-y-4">
				<!-- Name -->
				<div>
					<label for="import-name" class="block text-xs font-medium text-gx-text-secondary mb-1">Source Name</label>
					<input
						id="import-name"
						type="text"
						bind:value={addForm.name}
						placeholder="My Google Calendar"
						class="w-full px-3 py-2 text-sm rounded-gx border border-gx-border-default bg-gx-bg-primary text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
					/>
				</div>

				<!-- Source Type -->
				<div>
					<label for="import-type" class="block text-xs font-medium text-gx-text-secondary mb-1">Source Type</label>
					<select
						id="import-type"
						bind:value={addForm.sourceType}
						class="w-full px-3 py-2 text-sm rounded-gx border border-gx-border-default bg-gx-bg-primary text-gx-text-primary focus:outline-none focus:border-gx-neon"
					>
						<option value="">Select a source type...</option>
						{#each sourceTypes as st}
							<option value={JSON.parse(JSON.stringify(st.type))}>{st.label}</option>
						{/each}
					</select>
				</div>

				<!-- URL -->
				<div>
					<label for="import-url" class="block text-xs font-medium text-gx-text-secondary mb-1">URL</label>
					<input
						id="import-url"
						type="url"
						bind:value={addForm.url}
						placeholder="https://calendar.google.com/..."
						class="w-full px-3 py-2 text-sm rounded-gx border border-gx-border-default bg-gx-bg-primary text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
					/>
				</div>

				<!-- Target Module -->
				<div>
					<label for="import-target" class="block text-xs font-medium text-gx-text-secondary mb-1">Import Into</label>
					<select
						id="import-target"
						bind:value={addForm.targetModule}
						class="w-full px-3 py-2 text-sm rounded-gx border border-gx-border-default bg-gx-bg-primary text-gx-text-primary focus:outline-none focus:border-gx-neon"
					>
						<option value="">Auto-detect (based on source type)</option>
						{#each targetModules as tm}
							<option value={tm.id}>{tm.label}</option>
						{/each}
					</select>
				</div>
			</div>

			<!-- Dialog Actions -->
			<div class="flex items-center justify-end gap-2 mt-6">
				<button
					onclick={() => showAddDialog = false}
					class="px-4 py-2 text-xs rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={addSource}
					disabled={addLoading || !addForm.name || !addForm.sourceType || !addForm.url}
					class="flex items-center gap-1.5 px-4 py-2 text-xs rounded-gx bg-gx-accent-blue text-white hover:bg-gx-accent-blue/80 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
				>
					{#if addLoading}
						<Loader2 size={13} class="animate-spin" />
					{:else}
						<Plus size={13} />
					{/if}
					Add Source
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Steps Preview & Execute Dialog -->
{#if showStepsDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
		<div class="w-full max-w-2xl rounded-gx border border-gx-border-default bg-gx-bg-elevated shadow-gx-glow-lg p-6 m-4 max-h-[80vh] overflow-y-auto">
			<div class="flex items-center justify-between mb-5">
				<h2 class="text-base font-bold text-gx-text-primary flex items-center gap-2">
					<Eye size={16} class="text-gx-neon" />
					Import Preview: {previewSourceName}
				</h2>
				<button onclick={() => showStepsDialog = false} class="text-gx-text-muted hover:text-gx-text-primary">
					<X size={16} />
				</button>
			</div>

			<p class="text-xs text-gx-text-muted mb-4">
				The following CDP automation steps will be executed in order. Review them before proceeding.
			</p>

			<!-- Steps list -->
			<div class="space-y-2 mb-6">
				{#each previewSteps as step, i}
					<div class="flex items-start gap-3 p-3 rounded-gx bg-gx-bg-primary border border-gx-border-default">
						<div class="flex items-center justify-center w-6 h-6 rounded-full bg-gx-bg-elevated text-[10px] font-bold text-gx-neon shrink-0 border border-gx-border-default">
							{i + 1}
						</div>
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2 mb-0.5">
								<Badge variant="outline" class="text-[10px] px-1.5 h-4 border-gx-accent-blue/30 text-gx-accent-blue">
									{step.step_type}
								</Badge>
							</div>
							<p class="text-xs text-gx-text-secondary break-all">
								{stepTypeLabel(step)}
							</p>
							{#if step.step_type === 'ExecuteJs' && step.script}
								<details class="mt-1">
									<summary class="text-[10px] text-gx-text-muted cursor-pointer hover:text-gx-text-secondary">
										Show script
									</summary>
									<pre class="mt-1 text-[10px] text-gx-text-muted bg-gx-bg-elevated rounded p-2 overflow-x-auto max-h-32">{step.script.trim()}</pre>
								</details>
							{/if}
						</div>
					</div>
				{/each}
			</div>

			<!-- Dialog Actions -->
			<div class="flex items-center justify-between">
				<span class="text-[11px] text-gx-text-muted">
					{previewSteps.length} step{previewSteps.length !== 1 ? 's' : ''} will be executed
				</span>
				<div class="flex items-center gap-2">
					<button
						onclick={() => showStepsDialog = false}
						class="px-4 py-2 text-xs rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
					>
						Cancel
					</button>
					<button
						onclick={() => executeImport(previewSourceId)}
						disabled={runningImport}
						class="flex items-center gap-1.5 px-4 py-2 text-xs rounded-gx bg-gx-neon text-gx-bg-primary font-semibold hover:bg-gx-neon/80 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
					>
						{#if runningImport}
							<Loader2 size={13} class="animate-spin" />
							Running...
						{:else}
							<Play size={13} />
							Execute Import
						{/if}
					</button>
				</div>
			</div>
		</div>
	</div>
{/if}
