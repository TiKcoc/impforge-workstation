<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import {
		Activity, Server, HardDrive, Database, Brain,
		RefreshCw, CheckCircle2, AlertTriangle, XCircle,
		Clock, MemoryStick, FolderOpen, FileText, Table2,
		BookOpen, Presentation, CalendarDays, Mail, Workflow,
		PenTool, Loader2, Heart
	} from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-health';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// --- Types ---
	interface ModuleHealth {
		name: string;
		status: 'healthy' | 'degraded' | 'error' | 'unknown';
		items_count: number;
		last_used: string | null;
	}

	interface SystemHealth {
		overall: 'healthy' | 'degraded' | 'error' | 'unknown';
		modules: ModuleHealth[];
		uptime_seconds: number;
		memory_usage_mb: number;
		storage_used_mb: number;
		ollama_status: 'healthy' | 'degraded' | 'error' | 'unknown';
		documents_count: number;
		total_memory_entries: number;
	}

	// --- State ---
	let health = $state<SystemHealth | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let lastChecked = $state<Date | null>(null);
	let refreshTimer: ReturnType<typeof setInterval> | null = null;

	// --- Computed ---
	let uptimeFormatted = $derived(() => {
		if (!health) return '--';
		const secs = health.uptime_seconds;
		const h = Math.floor(secs / 3600);
		const m = Math.floor((secs % 3600) / 60);
		const s = secs % 60;
		if (h > 0) return `${h}h ${m}m ${s}s`;
		if (m > 0) return `${m}m ${s}s`;
		return `${s}s`;
	});

	let lastCheckedFormatted = $derived(() => {
		if (!lastChecked) return 'Never';
		return lastChecked.toLocaleTimeString();
	});

	// --- Module icon mapping ---
	const moduleIcons: Record<string, typeof FileText> = {
		'ForgeWriter': FileText,
		'ForgeSheets': Table2,
		'ForgeNotes': BookOpen,
		'ForgeSlides': Presentation,
		'ForgePDF': FileText,
		'Calendar': CalendarDays,
		'ForgeMail': Mail,
		'ForgeFlow': Workflow,
		'ForgeCanvas': PenTool,
	};

	// --- Actions ---
	async function fetchHealth() {
		loading = true;
		error = null;
		try {
			health = await invoke<SystemHealth>('health_check');
			lastChecked = new Date();
		} catch (e: unknown) {
			const msg = typeof e === 'string' ? e : (e as { message?: string })?.message ?? 'Health check failed';
			error = msg;
		} finally {
			loading = false;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'healthy': return 'text-gx-status-success';
			case 'degraded': return 'text-gx-status-warning';
			case 'error': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusBg(status: string): string {
		switch (status) {
			case 'healthy': return 'bg-gx-status-success/10 border-gx-status-success/30';
			case 'degraded': return 'bg-gx-status-warning/10 border-gx-status-warning/30';
			case 'error': return 'bg-gx-status-error/10 border-gx-status-error/30';
			default: return 'bg-gx-bg-tertiary border-gx-border-default';
		}
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'healthy': return 'Healthy';
			case 'degraded': return 'Degraded';
			case 'error': return 'Error';
			default: return 'Unknown';
		}
	}

	function formatLastUsed(iso: string | null): string {
		if (!iso) return 'Never';
		try {
			const date = new Date(iso);
			const now = new Date();
			const diffMs = now.getTime() - date.getTime();
			const diffMins = Math.floor(diffMs / 60000);
			if (diffMins < 1) return 'Just now';
			if (diffMins < 60) return `${diffMins}m ago`;
			const diffHours = Math.floor(diffMins / 60);
			if (diffHours < 24) return `${diffHours}h ago`;
			const diffDays = Math.floor(diffHours / 24);
			return `${diffDays}d ago`;
		} catch {
			return 'Unknown';
		}
	}

	onMount(() => {
		fetchHealth();
		// Auto-refresh every 30 seconds
		refreshTimer = setInterval(fetchHealth, 30_000);
	});

	onDestroy(() => {
		if (refreshTimer) clearInterval(refreshTimer);
	});
</script>

<div class="p-6 space-y-6 max-w-5xl mx-auto" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<Heart size={24} class="text-gx-neon" />
			<div>
				<h1 class="text-xl font-bold text-gx-text-primary">System Health</h1>
				<p class="text-sm text-gx-text-muted">Real-time status of all ImpForge subsystems</p>
			</div>
		</div>
		<div class="flex items-center gap-3">
			<span class="text-xs text-gx-text-muted">
				Last checked: {lastCheckedFormatted()}
			</span>
			<button
				onclick={fetchHealth}
				disabled={loading}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border border-gx-border-default
					bg-gx-bg-tertiary hover:bg-gx-bg-hover text-gx-text-secondary transition-all
					disabled:opacity-50 disabled:cursor-not-allowed"
			>
				{#if loading}
					<Loader2 size={14} class="animate-spin" />
				{:else}
					<RefreshCw size={14} />
				{/if}
				Refresh
			</button>
		</div>
	</div>

	{#if error}
		<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/10 p-4 flex items-start gap-3">
			<XCircle size={18} class="text-gx-status-error shrink-0 mt-0.5" />
			<div>
				<p class="text-sm font-medium text-gx-status-error">Health check failed</p>
				<p class="text-xs text-gx-text-muted mt-1">{error}</p>
			</div>
		</div>
	{/if}

	{#if health}
		<!-- Overall Status Banner -->
		<div class="rounded-gx border {statusBg(health.overall)} p-4 flex items-center gap-4">
			{#if health.overall === 'healthy'}
				<CheckCircle2 size={28} class="text-gx-status-success" />
			{:else if health.overall === 'degraded'}
				<AlertTriangle size={28} class="text-gx-status-warning" />
			{:else}
				<XCircle size={28} class="text-gx-status-error" />
			{/if}
			<div>
				<p class="text-lg font-bold {statusColor(health.overall)}">
					System {statusLabel(health.overall)}
				</p>
				<p class="text-xs text-gx-text-muted">
					{health.documents_count} documents across {health.modules.length} modules
				</p>
			</div>
			<div class="ml-auto flex items-center gap-1.5">
				<Badge variant="outline" class="text-[10px] border-gx-border-default text-gx-text-muted">
					Auto-refresh: 30s
				</Badge>
			</div>
		</div>

		<!-- Key Metrics Row -->
		<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
			<!-- Uptime -->
			<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
				<div class="flex items-center gap-2 mb-2">
					<Clock size={14} class="text-gx-accent-blue" />
					<span class="text-xs font-medium text-gx-text-muted uppercase tracking-wider">Uptime</span>
				</div>
				<p class="text-lg font-bold text-gx-text-primary font-mono">{uptimeFormatted()}</p>
			</div>

			<!-- Memory -->
			<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
				<div class="flex items-center gap-2 mb-2">
					<MemoryStick size={14} class="text-gx-accent-magenta" />
					<span class="text-xs font-medium text-gx-text-muted uppercase tracking-wider">Memory</span>
				</div>
				<p class="text-lg font-bold text-gx-text-primary font-mono">
					{health.memory_usage_mb.toFixed(1)} MB
				</p>
			</div>

			<!-- Storage -->
			<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
				<div class="flex items-center gap-2 mb-2">
					<HardDrive size={14} class="text-gx-accent-green" />
					<span class="text-xs font-medium text-gx-text-muted uppercase tracking-wider">Storage</span>
				</div>
				<p class="text-lg font-bold text-gx-text-primary font-mono">
					{health.storage_used_mb.toFixed(1)} MB
				</p>
			</div>

			<!-- ForgeMemory Entries -->
			<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
				<div class="flex items-center gap-2 mb-2">
					<Database size={14} class="text-gx-accent-purple" />
					<span class="text-xs font-medium text-gx-text-muted uppercase tracking-wider">Memory</span>
				</div>
				<p class="text-lg font-bold text-gx-text-primary font-mono">
					{health.total_memory_entries} entries
				</p>
			</div>
		</div>

		<!-- Ollama Status -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
			<div class="flex items-center gap-3">
				<Brain size={18} class={statusColor(health.ollama_status)} />
				<div class="flex-1">
					<div class="flex items-center gap-2">
						<span class="text-sm font-medium text-gx-text-primary">Ollama AI Runtime</span>
						<Badge variant="outline" class="text-[10px] {statusColor(health.ollama_status)} border-current">
							{statusLabel(health.ollama_status)}
						</Badge>
					</div>
					<p class="text-xs text-gx-text-muted mt-0.5">
						{#if health.ollama_status === 'healthy'}
							Local AI inference is available. Models can be used for chat, analysis, and generation.
						{:else if health.ollama_status === 'degraded'}
							Ollama is responding but may have issues. Check model availability.
						{:else if health.ollama_status === 'error'}
							Cannot connect to Ollama. Start it with: ollama serve
						{:else}
							Ollama status could not be determined.
						{/if}
					</p>
				</div>
				<div class="shrink-0">
					{#if health.ollama_status === 'healthy'}
						<CheckCircle2 size={20} class="text-gx-status-success" />
					{:else if health.ollama_status === 'degraded'}
						<AlertTriangle size={20} class="text-gx-status-warning" />
					{:else if health.ollama_status === 'error'}
						<XCircle size={20} class="text-gx-status-error" />
					{:else}
						<Activity size={20} class="text-gx-text-muted" />
					{/if}
				</div>
			</div>
		</div>

		<!-- Storage Usage Bar -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
			<div class="flex items-center justify-between mb-2">
				<div class="flex items-center gap-2">
					<FolderOpen size={14} class="text-gx-accent-green" />
					<span class="text-sm font-medium text-gx-text-primary">Storage Usage</span>
				</div>
				<span class="text-xs text-gx-text-muted font-mono">
					{health.storage_used_mb.toFixed(1)} MB used
				</span>
			</div>
			<div class="w-full h-3 rounded-full bg-gx-bg-tertiary overflow-hidden">
				{@const pct = Math.min(health.storage_used_mb / 1024, 1)}
				<div
					class="h-full rounded-full transition-all duration-500 {pct > 0.8 ? 'bg-gx-status-error' : pct > 0.5 ? 'bg-gx-status-warning' : 'bg-gx-neon'}"
					style="width: {Math.max(pct * 100, 1)}%"
				></div>
			</div>
			<p class="text-[10px] text-gx-text-muted mt-1">
				Based on ImpForge data directory. 1 GB reference bar.
			</p>
		</div>

		<!-- Module Status Grid -->
		<div>
			<div class="flex items-center gap-2 mb-3">
				<Server size={16} class="text-gx-text-secondary" />
				<h2 class="text-sm font-semibold text-gx-text-primary">Module Status</h2>
			</div>
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
				{#each health.modules as mod (mod.name)}
					{@const IconComponent = moduleIcons[mod.name] || FileText}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3 flex items-start gap-3 hover:border-gx-neon/30 transition-colors">
						<div class="shrink-0 w-8 h-8 rounded-gx flex items-center justify-center {statusBg(mod.status)}">
							<IconComponent size={16} class={statusColor(mod.status)} />
						</div>
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2">
								<span class="text-sm font-medium text-gx-text-primary truncate">{mod.name}</span>
								{#if mod.status === 'healthy'}
									<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success shrink-0"></span>
								{:else if mod.status === 'degraded'}
									<span class="w-1.5 h-1.5 rounded-full bg-gx-status-warning shrink-0"></span>
								{:else if mod.status === 'error'}
									<span class="w-1.5 h-1.5 rounded-full bg-gx-status-error shrink-0"></span>
								{:else}
									<span class="w-1.5 h-1.5 rounded-full bg-gx-text-muted shrink-0"></span>
								{/if}
							</div>
							<div class="flex items-center justify-between mt-1">
								<span class="text-[11px] text-gx-text-muted">{mod.items_count} items</span>
								<span class="text-[10px] text-gx-text-muted">{formatLastUsed(mod.last_used)}</span>
							</div>
						</div>
					</div>
				{/each}
			</div>
		</div>
	{:else if !error}
		<!-- Loading skeleton -->
		<div class="space-y-4">
			{#each Array(4) as _, i (i)}
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 animate-pulse">
					<div class="h-4 bg-gx-bg-tertiary rounded w-1/3 mb-2"></div>
					<div class="h-6 bg-gx-bg-tertiary rounded w-1/4"></div>
				</div>
			{/each}
		</div>
	{/if}
</div>
