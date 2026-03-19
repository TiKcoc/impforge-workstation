<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import {
		ShieldCheck, RefreshCw, Wrench, CheckCircle2, AlertTriangle,
		XCircle, HelpCircle, HardDrive, Server, Cpu, Database,
		Settings2, Lock, Loader2, History, ChevronDown, ChevronUp,
		Zap, TrendingUp, Play, ToggleLeft, ToggleRight
	} from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-healing';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// --- Types ---
	interface HealthCheck {
		id: string;
		name: string;
		category: 'storage' | 'services' | 'performance' | 'data' | 'configuration' | 'security';
		status: 'healthy' | 'warning' | 'critical' | 'unknown';
		message: string;
		auto_fixable: boolean;
		fix_description: string | null;
	}

	interface RepairAction {
		id: string;
		check_id: string;
		action_type: string;
		description: string;
		status: 'pending' | 'in_progress' | 'success' | 'failed';
		timestamp: string;
		result: string | null;
	}

	interface HealthSnapshot {
		timestamp: string;
		overall_score: number;
		checks: HealthCheck[];
	}

	interface SelfHealingConfig {
		enabled: boolean;
		check_interval_seconds: number;
		auto_repair: boolean;
		max_history: number;
	}

	// --- State ---
	let checks = $state<HealthCheck[]>([]);
	let repairs = $state<RepairAction[]>([]);
	let history = $state<HealthSnapshot[]>([]);
	let config = $state<SelfHealingConfig>({ enabled: true, check_interval_seconds: 60, auto_repair: false, max_history: 168 });
	let score = $state(0);
	let loading = $state(false);
	let repairing = $state<string | null>(null);
	let repairingAll = $state(false);
	let error = $state<string | null>(null);
	let lastChecked = $state<Date | null>(null);
	let historyExpanded = $state(false);
	let repairHistoryExpanded = $state(false);
	let refreshTimer: ReturnType<typeof setInterval> | null = null;

	// --- Derived ---
	let healthyCount = $derived(checks.filter(c => c.status === 'healthy').length);
	let warningCount = $derived(checks.filter(c => c.status === 'warning').length);
	let criticalCount = $derived(checks.filter(c => c.status === 'critical').length);
	let unknownCount = $derived(checks.filter(c => c.status === 'unknown').length);
	let fixableChecks = $derived(checks.filter(c => c.auto_fixable && c.status !== 'healthy'));

	let scoreColor = $derived(
		score >= 90 ? 'text-gx-status-success' :
		score >= 70 ? 'text-gx-status-warning' :
		'text-gx-status-error'
	);

	let scoreBorder = $derived(
		score >= 90 ? 'border-gx-status-success/30' :
		score >= 70 ? 'border-gx-status-warning/30' :
		'border-gx-status-error/30'
	);

	let scoreGlow = $derived(
		score >= 90 ? 'shadow-[0_0_24px_rgba(34,197,94,0.15)]' :
		score >= 70 ? 'shadow-[0_0_24px_rgba(234,179,8,0.15)]' :
		'shadow-[0_0_24px_rgba(239,68,68,0.15)]'
	);

	let lastCheckedFormatted = $derived(() => {
		if (!lastChecked) return 'Never';
		return lastChecked.toLocaleTimeString();
	});

	// --- Category grouping ---
	const categoryOrder: HealthCheck['category'][] = ['storage', 'services', 'performance', 'data', 'configuration', 'security'];
	const categoryLabels: Record<string, string> = {
		storage: 'Storage',
		services: 'Services',
		performance: 'Performance',
		data: 'Data Integrity',
		configuration: 'Configuration',
		security: 'Security',
	};
	const categoryIcons: Record<string, typeof HardDrive> = {
		storage: HardDrive,
		services: Server,
		performance: Cpu,
		data: Database,
		configuration: Settings2,
		security: Lock,
	};

	let groupedChecks = $derived(() => {
		const groups: { category: HealthCheck['category']; label: string; checks: HealthCheck[] }[] = [];
		for (const cat of categoryOrder) {
			const catChecks = checks.filter(c => c.category === cat);
			if (catChecks.length > 0) {
				groups.push({ category: cat, label: categoryLabels[cat] ?? cat, checks: catChecks });
			}
		}
		return groups;
	});

	// --- Actions ---
	async function runChecks() {
		loading = true;
		error = null;
		try {
			checks = await invoke<HealthCheck[]>('healing_run_checks');
			score = await invoke<number>('healing_get_score');
			lastChecked = new Date();
		} catch (e: unknown) {
			const msg = typeof e === 'string' ? e : (e as { message?: string })?.message ?? 'Health check failed';
			error = msg;
		} finally {
			loading = false;
		}
	}

	async function repairOne(checkId: string) {
		repairing = checkId;
		try {
			const result = await invoke<RepairAction>('healing_auto_repair', { checkId });
			repairs = [result, ...repairs];
			// Re-run checks after repair
			await runChecks();
		} catch (e: unknown) {
			const msg = typeof e === 'string' ? e : (e as { message?: string })?.message ?? 'Repair failed';
			error = msg;
		} finally {
			repairing = null;
		}
	}

	async function repairAll() {
		repairingAll = true;
		try {
			const results = await invoke<RepairAction[]>('healing_repair_all');
			repairs = [...results, ...repairs];
			await runChecks();
		} catch (e: unknown) {
			const msg = typeof e === 'string' ? e : (e as { message?: string })?.message ?? 'Repair all failed';
			error = msg;
		} finally {
			repairingAll = false;
		}
	}

	async function loadHistory() {
		try {
			history = await invoke<HealthSnapshot[]>('healing_get_history', { limit: 168 });
		} catch {
			// Non-critical
		}
	}

	async function loadRepairs() {
		try {
			repairs = await invoke<RepairAction[]>('healing_get_repairs');
		} catch {
			// Non-critical
		}
	}

	async function loadConfig() {
		try {
			config = await invoke<SelfHealingConfig>('healing_get_config');
		} catch {
			// Use defaults
		}
	}

	async function toggleAutoRepair() {
		const newConfig = { ...config, auto_repair: !config.auto_repair };
		try {
			await invoke('healing_save_config', { config: newConfig });
			config = newConfig;
		} catch {
			// Revert on error
		}
	}

	// --- Helpers ---
	function statusIcon(status: string) {
		switch (status) {
			case 'healthy': return CheckCircle2;
			case 'warning': return AlertTriangle;
			case 'critical': return XCircle;
			default: return HelpCircle;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'healthy': return 'text-gx-status-success';
			case 'warning': return 'text-gx-status-warning';
			case 'critical': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusBg(status: string): string {
		switch (status) {
			case 'healthy': return 'bg-gx-status-success/10 border-gx-status-success/30';
			case 'warning': return 'bg-gx-status-warning/10 border-gx-status-warning/30';
			case 'critical': return 'bg-gx-status-error/10 border-gx-status-error/30';
			default: return 'bg-gx-bg-tertiary border-gx-border-default';
		}
	}

	function repairStatusColor(status: string): string {
		switch (status) {
			case 'success': return 'text-gx-status-success';
			case 'failed': return 'text-gx-status-error';
			case 'in_progress': return 'text-gx-status-warning';
			default: return 'text-gx-text-muted';
		}
	}

	function formatTimestamp(iso: string): string {
		try {
			const date = new Date(iso);
			return date.toLocaleString();
		} catch {
			return iso;
		}
	}

	function formatTimeAgo(iso: string): string {
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

	// Mini sparkline from history scores
	let sparklinePoints = $derived(() => {
		if (history.length < 2) return '';
		const recent = history.slice(-24); // Last 24 snapshots
		const maxScore = 100;
		const w = 200;
		const h = 40;
		return recent
			.map((s, i) => {
				const x = (i / (recent.length - 1)) * w;
				const y = h - (s.overall_score / maxScore) * h;
				return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
			})
			.join(' ');
	});

	onMount(async () => {
		await loadConfig();
		await runChecks();
		await loadHistory();
		await loadRepairs();
		// Auto-refresh every 60 seconds
		refreshTimer = setInterval(async () => {
			await runChecks();
			await loadHistory();
		}, 60_000);
	});

	onDestroy(() => {
		if (refreshTimer) clearInterval(refreshTimer);
	});
</script>

<div class="p-6 space-y-6 max-w-6xl mx-auto" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<ShieldCheck size={24} class="text-gx-neon" />
			<div>
				<h1 class="text-xl font-bold text-gx-text-primary">Self-Healing System</h1>
				<p class="text-sm text-gx-text-muted">
					MAPE-K autonomous health management
					<span class="text-[10px] ml-1 text-gx-text-muted">(arXiv:2504.20093)</span>
				</p>
			</div>
		</div>
		<div class="flex items-center gap-2">
			<span class="text-xs text-gx-text-muted">
				Last: {lastCheckedFormatted()}
			</span>
			<button
				onclick={runChecks}
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
				Run Checks
			</button>
			{#if fixableChecks.length > 0}
				<button
					onclick={repairAll}
					disabled={repairingAll}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border
						border-gx-neon/40 bg-gx-neon/10 hover:bg-gx-neon/20
						text-gx-neon transition-all disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{#if repairingAll}
						<Loader2 size={14} class="animate-spin" />
					{:else}
						<Zap size={14} />
					{/if}
					Fix All ({fixableChecks.length})
				</button>
			{/if}
		</div>
	</div>

	{#if error}
		<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/10 p-4 flex items-start gap-3">
			<XCircle size={18} class="text-gx-status-error shrink-0 mt-0.5" />
			<div>
				<p class="text-sm font-medium text-gx-status-error">Error</p>
				<p class="text-xs text-gx-text-muted mt-1">{error}</p>
			</div>
		</div>
	{/if}

	<!-- Health Score + Summary Row -->
	<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
		<!-- Big Score Card -->
		<div class="rounded-gx border {scoreBorder} bg-gx-bg-secondary p-6 flex flex-col items-center justify-center {scoreGlow}">
			<p class="text-5xl font-bold font-mono {scoreColor} tabular-nums">
				{Math.round(score)}
			</p>
			<p class="text-xs text-gx-text-muted mt-1 uppercase tracking-wider">Health Score</p>
			<!-- Mini trend sparkline -->
			{#if sparklinePoints().length > 0}
				<svg viewBox="0 0 200 40" class="w-full h-8 mt-3" preserveAspectRatio="none">
					<path d={sparklinePoints()} fill="none" stroke="currentColor" stroke-width="2"
						class="{scoreColor} opacity-60" />
				</svg>
				<p class="text-[10px] text-gx-text-muted mt-0.5">Last {Math.min(history.length, 24)} checks</p>
			{/if}
		</div>

		<!-- Check Summary -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 flex flex-col justify-center">
			<p class="text-xs text-gx-text-muted uppercase tracking-wider mb-3 font-medium">Check Summary</p>
			<div class="space-y-2">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<span class="w-2 h-2 rounded-full bg-gx-status-success"></span>
						<span class="text-sm text-gx-text-secondary">Healthy</span>
					</div>
					<span class="text-sm font-mono font-bold text-gx-text-primary">{healthyCount}</span>
				</div>
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<span class="w-2 h-2 rounded-full bg-gx-status-warning"></span>
						<span class="text-sm text-gx-text-secondary">Warning</span>
					</div>
					<span class="text-sm font-mono font-bold text-gx-text-primary">{warningCount}</span>
				</div>
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<span class="w-2 h-2 rounded-full bg-gx-status-error"></span>
						<span class="text-sm text-gx-text-secondary">Critical</span>
					</div>
					<span class="text-sm font-mono font-bold text-gx-text-primary">{criticalCount}</span>
				</div>
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<span class="w-2 h-2 rounded-full bg-gx-text-muted"></span>
						<span class="text-sm text-gx-text-secondary">Unknown</span>
					</div>
					<span class="text-sm font-mono font-bold text-gx-text-primary">{unknownCount}</span>
				</div>
			</div>
		</div>

		<!-- Auto-Repair Config -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 flex flex-col justify-center">
			<p class="text-xs text-gx-text-muted uppercase tracking-wider mb-3 font-medium">Auto-Repair</p>
			<button
				onclick={toggleAutoRepair}
				class="flex items-center gap-3 p-2 rounded-gx hover:bg-gx-bg-hover transition-colors w-full text-left"
			>
				{#if config.auto_repair}
					<ToggleRight size={28} class="text-gx-neon shrink-0" />
					<div>
						<p class="text-sm font-medium text-gx-neon">Enabled</p>
						<p class="text-[11px] text-gx-text-muted">Auto-fix issues when detected</p>
					</div>
				{:else}
					<ToggleLeft size={28} class="text-gx-text-muted shrink-0" />
					<div>
						<p class="text-sm font-medium text-gx-text-secondary">Disabled</p>
						<p class="text-[11px] text-gx-text-muted">Manual repair only</p>
					</div>
				{/if}
			</button>
			<div class="mt-3 pt-3 border-t border-gx-border-default">
				<div class="flex items-center justify-between text-[11px]">
					<span class="text-gx-text-muted">Check interval</span>
					<span class="text-gx-text-secondary font-mono">{config.check_interval_seconds}s</span>
				</div>
				<div class="flex items-center justify-between text-[11px] mt-1">
					<span class="text-gx-text-muted">Auto-fixable issues</span>
					<span class="text-gx-text-secondary font-mono">{fixableChecks.length}</span>
				</div>
				<div class="flex items-center justify-between text-[11px] mt-1">
					<span class="text-gx-text-muted">History snapshots</span>
					<span class="text-gx-text-secondary font-mono">{history.length}</span>
				</div>
			</div>
		</div>
	</div>

	<!-- Health Checks Grid (grouped by category) -->
	{#if checks.length > 0}
		<div class="space-y-4">
			{#each groupedChecks() as group (group.category)}
				{@const CatIcon = categoryIcons[group.category] || Server}
				<div>
					<div class="flex items-center gap-2 mb-2">
						<CatIcon size={14} class="text-gx-text-muted" />
						<h2 class="text-xs font-semibold text-gx-text-muted uppercase tracking-wider">{group.label}</h2>
						<div class="flex-1 h-px bg-gx-border-default"></div>
					</div>
					<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
						{#each group.checks as check (check.id)}
							{@const Icon = statusIcon(check.status)}
							<div class="rounded-gx border {statusBg(check.status)} p-3 flex flex-col gap-2
								hover:border-gx-neon/30 transition-colors">
								<div class="flex items-start gap-2">
									<div class="shrink-0 mt-0.5">
										<Icon size={16} class={statusColor(check.status)} />
									</div>
									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2">
											<span class="text-sm font-medium text-gx-text-primary truncate">{check.name}</span>
											<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {statusColor(check.status)} border-current shrink-0">
												{check.status}
											</Badge>
										</div>
										<p class="text-[11px] text-gx-text-muted mt-0.5 line-clamp-2">{check.message}</p>
									</div>
								</div>
								{#if check.auto_fixable && check.status !== 'healthy'}
									<button
										onclick={() => repairOne(check.id)}
										disabled={repairing === check.id}
										class="flex items-center gap-1.5 px-2 py-1 text-[11px] rounded border
											border-gx-neon/30 bg-gx-neon/5 hover:bg-gx-neon/15
											text-gx-neon transition-all self-end
											disabled:opacity-50 disabled:cursor-not-allowed"
									>
										{#if repairing === check.id}
											<Loader2 size={12} class="animate-spin" />
											Repairing...
										{:else}
											<Wrench size={12} />
											{check.fix_description ?? 'Auto-Fix'}
										{/if}
									</button>
								{/if}
							</div>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{:else if !loading && !error}
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-8 text-center">
			<Play size={32} class="text-gx-text-muted mx-auto mb-2" />
			<p class="text-sm text-gx-text-muted">Click "Run Checks" to analyze system health</p>
		</div>
	{:else if loading}
		<div class="space-y-3">
			{#each Array(5) as _, i (i)}
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 animate-pulse">
					<div class="h-4 bg-gx-bg-tertiary rounded w-1/3 mb-2"></div>
					<div class="h-3 bg-gx-bg-tertiary rounded w-2/3"></div>
				</div>
			{/each}
		</div>
	{/if}

	<!-- Repair History (collapsible) -->
	{#if repairs.length > 0}
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary">
			<button
				onclick={() => repairHistoryExpanded = !repairHistoryExpanded}
				class="flex items-center justify-between w-full p-4 text-left hover:bg-gx-bg-hover transition-colors rounded-gx"
			>
				<div class="flex items-center gap-2">
					<Wrench size={16} class="text-gx-accent-purple" />
					<span class="text-sm font-semibold text-gx-text-primary">Repair History</span>
					<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-border-default text-gx-text-muted">
						{repairs.length}
					</Badge>
				</div>
				{#if repairHistoryExpanded}
					<ChevronUp size={16} class="text-gx-text-muted" />
				{:else}
					<ChevronDown size={16} class="text-gx-text-muted" />
				{/if}
			</button>
			{#if repairHistoryExpanded}
				<div class="border-t border-gx-border-default p-4 space-y-2 max-h-64 overflow-y-auto">
					{#each repairs as repair (repair.id)}
						<div class="flex items-start gap-3 p-2 rounded-gx bg-gx-bg-primary border border-gx-border-default">
							<div class="shrink-0 mt-0.5">
								{#if repair.status === 'success'}
									<CheckCircle2 size={14} class="text-gx-status-success" />
								{:else if repair.status === 'failed'}
									<XCircle size={14} class="text-gx-status-error" />
								{:else}
									<Loader2 size={14} class="text-gx-status-warning animate-spin" />
								{/if}
							</div>
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2">
									<span class="text-xs font-medium text-gx-text-primary">{repair.description}</span>
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {repairStatusColor(repair.status)} border-current">
										{repair.status}
									</Badge>
								</div>
								{#if repair.result}
									<p class="text-[10px] text-gx-text-muted mt-0.5">{repair.result}</p>
								{/if}
								<p class="text-[10px] text-gx-text-muted mt-0.5">{formatTimeAgo(repair.timestamp)}</p>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}

	<!-- Health History (collapsible) -->
	{#if history.length > 0}
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary">
			<button
				onclick={() => historyExpanded = !historyExpanded}
				class="flex items-center justify-between w-full p-4 text-left hover:bg-gx-bg-hover transition-colors rounded-gx"
			>
				<div class="flex items-center gap-2">
					<History size={16} class="text-gx-accent-blue" />
					<span class="text-sm font-semibold text-gx-text-primary">Health Trend</span>
					<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-border-default text-gx-text-muted">
						{history.length} snapshots
					</Badge>
				</div>
				{#if historyExpanded}
					<ChevronUp size={16} class="text-gx-text-muted" />
				{:else}
					<ChevronDown size={16} class="text-gx-text-muted" />
				{/if}
			</button>
			{#if historyExpanded}
				<div class="border-t border-gx-border-default p-4">
					<!-- SVG Line Chart -->
					{#if history.length >= 2}
						<div class="mb-4">
							<svg viewBox="0 0 600 120" class="w-full h-28" preserveAspectRatio="none">
								<!-- Grid lines -->
								{#each [100, 75, 50, 25] as line}
									<line x1="0" y1={120 - line * 1.1} x2="600" y2={120 - line * 1.1}
										stroke="currentColor" stroke-width="0.5" class="text-gx-border-default" stroke-dasharray="4,4" />
									<text x="4" y={120 - line * 1.1 - 2} fill="currentColor" font-size="8"
										class="text-gx-text-muted">{line}</text>
								{/each}
								<!-- Score line -->
								{#if true}
									{@const recent = history.slice(-48)}
									{@const points = recent.map((s, i) => {
										const x = (i / Math.max(recent.length - 1, 1)) * 580 + 20;
										const y = 115 - (s.overall_score / 100) * 105;
										return `${x.toFixed(1)},${y.toFixed(1)}`;
									}).join(' ')}
									<polyline
										points={points}
										fill="none"
										stroke="currentColor"
										stroke-width="2"
										stroke-linejoin="round"
										class="text-gx-neon"
									/>
									<!-- Area fill -->
									{@const areaPoints = `20,115 ${points} ${(580 + 20).toFixed(1)},115`}
									<polygon
										points={areaPoints}
										class="fill-gx-neon/10"
									/>
									<!-- Current score dot -->
									{@const lastX = (recent.length - 1) / Math.max(recent.length - 1, 1) * 580 + 20}
									{@const lastY = 115 - (recent[recent.length - 1].overall_score / 100) * 105}
									<circle cx={lastX} cy={lastY} r="4" class="fill-gx-neon" />
								{/if}
							</svg>
							<div class="flex items-center justify-between text-[10px] text-gx-text-muted mt-1">
								<span>{formatTimeAgo(history[Math.max(0, history.length - 48)].timestamp)}</span>
								<TrendingUp size={10} class="text-gx-text-muted" />
								<span>Now</span>
							</div>
						</div>
					{/if}
					<!-- History table (last 10) -->
					<div class="space-y-1 max-h-48 overflow-y-auto">
						{#each history.slice(-10).reverse() as snap, i (i)}
							<div class="flex items-center justify-between p-1.5 rounded text-[11px]
								{i === 0 ? 'bg-gx-bg-hover' : ''}">
								<span class="text-gx-text-muted font-mono">{formatTimestamp(snap.timestamp)}</span>
								<div class="flex items-center gap-2">
									<span class="font-mono font-bold {snap.overall_score >= 90 ? 'text-gx-status-success' : snap.overall_score >= 70 ? 'text-gx-status-warning' : 'text-gx-status-error'}">
										{Math.round(snap.overall_score)}
									</span>
									<span class="text-gx-text-muted">
										({snap.checks.filter(c => c.status === 'healthy').length}/{snap.checks.length} OK)
									</span>
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
