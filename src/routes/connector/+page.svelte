<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Separator } from '$lib/components/ui/separator';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import {
		Plug, RefreshCw, Loader2, Plus, Trash2, ExternalLink,
		Circle, Wifi, WifiOff, Database, Container, GitBranch,
		Brain, Workflow, Globe, Server, Shield, Activity, Clock,
		ChevronDown, ChevronRight, Zap, AlertTriangle, X, Check,
		Settings2, Radio, Cable, Search, AppWindow, Monitor,
		Palette, MessageCircle, Wrench, FileText, Image, Code,
		Package, Terminal
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-connector';
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

	// -------------------------------------------------------------------
	// Types (mirror Rust structs)
	// -------------------------------------------------------------------

	interface DiscoveredService {
		id: string;
		name: string;
		service_type: string;
		url: string;
		port: number;
		status: string;
		capabilities: string[];
		auto_connected: boolean;
		last_check: string;
		response_time_ms: number | null;
		metadata: Record<string, unknown>;
	}

	interface ScanResult {
		services: DiscoveredService[];
		scan_duration_ms: number;
		timestamp: string;
	}

	interface ConnectorConfig {
		auto_scan_on_start: boolean;
		scan_interval_seconds: number;
		custom_endpoints: CustomEndpoint[];
		notification_on_change: boolean;
	}

	interface CustomEndpoint {
		name: string;
		url: string;
		health_path: string | null;
		expected_status: number;
	}

	interface InstalledProgram {
		name: string;
		executable: string;
		icon: string | null;
		category: string;
		version: string | null;
		installed_path: string | null;
	}

	// -------------------------------------------------------------------
	// State
	// -------------------------------------------------------------------

	let activeTab = $state<'services' | 'programs'>('services');
	let programs = $state<InstalledProgram[]>([]);
	let scanningPrograms = $state(false);
	let programFilter = $state('');
	let selectedProgramCategory = $state('all');

	let services = $state<DiscoveredService[]>([]);
	let scanDuration = $state(0);
	let scanTimestamp = $state('');
	let scanning = $state(false);
	let config = $state<ConnectorConfig>({
		auto_scan_on_start: true,
		scan_interval_seconds: 0,
		custom_endpoints: [],
		notification_on_change: true,
	});
	let scanHistory = $state<ScanResult[]>([]);
	let expandedService = $state<string | null>(null);
	let checkingService = $state<string | null>(null);
	let filterText = $state('');
	let selectedCategory = $state('all');

	// Add Custom dialog
	let showAddDialog = $state(false);
	let customName = $state('');
	let customUrl = $state('');
	let customHealthPath = $state('');
	let addingCustom = $state(false);
	let testResult = $state<string | null>(null);

	// Config dialog
	let showConfigDialog = $state(false);
	let configAutoScan = $state(true);
	let configInterval = $state(0);
	let configNotify = $state(true);

	// -------------------------------------------------------------------
	// Computed
	// -------------------------------------------------------------------

	let onlineCount = $derived(services.filter(s => s.status === 'online').length);
	let offlineCount = $derived(services.filter(s => s.status === 'offline').length);
	let totalCount = $derived(services.length);

	const categories = [
		{ id: 'all', label: 'All' },
		{ id: 'ai', label: 'AI Services' },
		{ id: 'database', label: 'Databases' },
		{ id: 'devtools', label: 'Dev Tools' },
		{ id: 'automation', label: 'Automation' },
		{ id: 'custom', label: 'Custom' },
	];

	function serviceCategory(svc: DiscoveredService): string {
		switch (svc.service_type) {
			case 'ollama':
			case 'mcp_server':
			case 'claude_code':
				return 'ai';
			case 'postgre_sql':
			case 'redis':
				return 'database';
			case 'docker':
			case 'git':
				return 'devtools';
			case 'n8n':
			case 'web_socket':
				return 'automation';
			case 'custom_api':
				return 'custom';
			default:
				return 'automation';
		}
	}

	let filteredServices = $derived(
		services.filter(s => {
			const matchesCategory = selectedCategory === 'all' || serviceCategory(s) === selectedCategory;
			const matchesFilter = !filterText || s.name.toLowerCase().includes(filterText.toLowerCase()) || s.url.toLowerCase().includes(filterText.toLowerCase());
			return matchesCategory && matchesFilter;
		})
	);

	// -------------------------------------------------------------------
	// Installed Programs helpers
	// -------------------------------------------------------------------

	const programCategories = [
		{ id: 'all', label: 'All' },
		{ id: 'ai_llm', label: 'AI / LLM' },
		{ id: 'ide', label: 'IDEs' },
		{ id: 'browser', label: 'Browsers' },
		{ id: 'dev_tool', label: 'Dev Tools' },
		{ id: 'creative', label: 'Creative' },
		{ id: 'office', label: 'Office' },
		{ id: 'adobe', label: 'Adobe' },
		{ id: 'communication', label: 'Comms' },
		{ id: 'system', label: 'System' },
		{ id: 'other', label: 'Other' },
	];

	let filteredPrograms = $derived(
		programs.filter(p => {
			const matchesCat = selectedProgramCategory === 'all' || p.category === selectedProgramCategory;
			const matchesFilter = !programFilter ||
				p.name.toLowerCase().includes(programFilter.toLowerCase()) ||
				p.executable.toLowerCase().includes(programFilter.toLowerCase());
			return matchesCat && matchesFilter;
		})
	);

	let programCategoryCounts = $derived(
		programCategories.map(cat => ({
			...cat,
			count: cat.id === 'all'
				? programs.length
				: programs.filter(p => p.category === cat.id).length,
		})).filter(cat => cat.id === 'all' || cat.count > 0)
	);

	function programCategoryIcon(cat: string) {
		switch (cat) {
			case 'ai_llm': return Brain;
			case 'ide': return Code;
			case 'browser': return Globe;
			case 'dev_tool': return Terminal;
			case 'creative': return Palette;
			case 'office': return FileText;
			case 'adobe': return Image;
			case 'communication': return MessageCircle;
			case 'system': return Monitor;
			default: return Package;
		}
	}

	function programCategoryLabel(cat: string): string {
		switch (cat) {
			case 'ai_llm': return 'AI / LLM';
			case 'ide': return 'IDE & Editor';
			case 'browser': return 'Browser';
			case 'dev_tool': return 'Dev Tool';
			case 'creative': return 'Creative / 3D';
			case 'office': return 'Office';
			case 'adobe': return 'Adobe';
			case 'communication': return 'Communication';
			case 'system': return 'System';
			default: return 'Other';
		}
	}

	// -------------------------------------------------------------------
	// Service display helpers
	// -------------------------------------------------------------------

	function serviceIcon(type: string) {
		switch (type) {
			case 'ollama': return Brain;
			case 'postgre_sql': return Database;
			case 'redis': return Database;
			case 'docker': return Container;
			case 'git': return GitBranch;
			case 'mcp_server': return Cable;
			case 'claude_code': return Brain;
			case 'n8n': return Workflow;
			case 'http_service': return Globe;
			case 'web_socket': return Radio;
			case 'custom_api': return Server;
			default: return Globe;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'online': return 'text-gx-status-success';
			case 'offline': return 'text-gx-status-error';
			case 'checking': return 'text-gx-status-warning';
			case 'auth_required': return 'text-gx-accent-orange';
			case 'error': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'online': return 'Online';
			case 'offline': return 'Offline';
			case 'checking': return 'Checking';
			case 'auth_required': return 'Auth Required';
			case 'error': return 'Error';
			default: return status;
		}
	}

	function statusBadgeVariant(status: string): 'default' | 'secondary' | 'destructive' | 'outline' {
		switch (status) {
			case 'online': return 'default';
			case 'offline': return 'destructive';
			case 'auth_required': return 'secondary';
			case 'error': return 'destructive';
			default: return 'outline';
		}
	}

	function serviceTypeLabel(type: string): string {
		switch (type) {
			case 'ollama': return 'Ollama';
			case 'postgre_sql': return 'PostgreSQL';
			case 'redis': return 'Redis';
			case 'docker': return 'Docker';
			case 'git': return 'Git';
			case 'mcp_server': return 'MCP Server';
			case 'claude_code': return 'Claude Code';
			case 'n8n': return 'n8n';
			case 'http_service': return 'HTTP Service';
			case 'web_socket': return 'WebSocket';
			case 'custom_api': return 'Custom API';
			default: return type;
		}
	}

	function formatTimestamp(ts: string): string {
		if (!ts) return '';
		try {
			const d = new Date(ts);
			return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
		} catch {
			return ts;
		}
	}

	function formatDuration(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		return `${(ms / 1000).toFixed(1)}s`;
	}

	// -------------------------------------------------------------------
	// Actions
	// -------------------------------------------------------------------

	async function loadPrograms() {
		scanningPrograms = true;
		try {
			programs = await invoke('connector_installed_programs');
		} catch (e) {
			console.error('Program detection failed:', e);
			programs = [];
		} finally {
			scanningPrograms = false;
		}
	}

	async function runScan() {
		scanning = true;
		try {
			const result: ScanResult = await invoke('connector_scan');
			services = result.services;
			scanDuration = result.scan_duration_ms;
			scanTimestamp = result.timestamp;
		} catch (e) {
			console.error('Scan failed:', e);
		} finally {
			scanning = false;
		}
		await loadHistory();
	}

	async function loadServices() {
		try {
			services = await invoke('connector_get_services');
		} catch {
			services = [];
		}
	}

	async function loadConfig() {
		try {
			config = await invoke('connector_get_config');
			configAutoScan = config.auto_scan_on_start;
			configInterval = config.scan_interval_seconds;
			configNotify = config.notification_on_change;
		} catch { /* use defaults */ }
	}

	async function loadHistory() {
		try {
			scanHistory = await invoke('connector_scan_history');
		} catch {
			scanHistory = [];
		}
	}

	async function checkService(id: string) {
		checkingService = id;
		try {
			const updated: DiscoveredService = await invoke('connector_check_service', { id });
			services = services.map(s => s.id === id ? updated : s);
		} catch (e) {
			console.error('Check failed:', e);
		} finally {
			checkingService = null;
		}
	}

	async function addCustomEndpoint() {
		if (!customName.trim() || !customUrl.trim()) return;
		addingCustom = true;
		testResult = null;
		try {
			const svc: DiscoveredService = await invoke('connector_add_custom', {
				name: customName.trim(),
				url: customUrl.trim(),
				healthPath: customHealthPath.trim() || null,
			});
			services = [...services, svc];
			testResult = svc.status === 'online' ? 'Connected successfully' : `Status: ${statusLabel(svc.status)}`;
			customName = '';
			customUrl = '';
			customHealthPath = '';
			showAddDialog = false;
			await loadConfig();
		} catch (e) {
			testResult = `Error: ${e}`;
		} finally {
			addingCustom = false;
		}
	}

	async function removeCustom(id: string) {
		try {
			await invoke('connector_remove_custom', { id });
			services = services.filter(s => s.id !== id);
			await loadConfig();
		} catch (e) {
			console.error('Remove failed:', e);
		}
	}

	async function saveConfig() {
		const updated: ConnectorConfig = {
			...config,
			auto_scan_on_start: configAutoScan,
			scan_interval_seconds: configInterval,
			notification_on_change: configNotify,
		};
		try {
			await invoke('connector_save_config', { config: updated });
			config = updated;
			showConfigDialog = false;
		} catch (e) {
			console.error('Save config failed:', e);
		}
	}

	async function autoConnect(id: string) {
		try {
			const ok: boolean = await invoke('connector_auto_connect', { serviceId: id });
			if (ok) {
				services = services.map(s => s.id === id ? { ...s, auto_connected: true } : s);
			}
		} catch (e) {
			console.error('Auto-connect failed:', e);
		}
	}

	function toggleExpand(id: string) {
		expandedService = expandedService === id ? null : id;
	}

	// -------------------------------------------------------------------
	// Lifecycle
	// -------------------------------------------------------------------

	onMount(async () => {
		await loadConfig();
		const cached = await invoke('connector_get_services').catch(() => []) as DiscoveredService[];
		if (cached.length > 0) {
			services = cached;
		}
		await loadHistory();
		// Run service scan and program detection in parallel
		await Promise.all([runScan(), loadPrograms()]);
	});
</script>

<div class="flex flex-col h-full overflow-hidden {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-3 px-6 py-4 border-b border-gx-border-default shrink-0">
		<div class="flex items-center gap-2">
			<div class="w-8 h-8 rounded-gx bg-gx-neon/10 flex items-center justify-center">
				<Plug size={18} class="text-gx-neon" />
			</div>
			<div>
				<h1 class="text-lg font-semibold text-gx-text-primary">Universal Connector</h1>
				<p class="text-xs text-gx-text-muted">Zero-config auto-discovery for all local services</p>
			</div>
		</div>

		<div class="flex-1" />

		<!-- Stats badges -->
		<div class="flex items-center gap-2 text-xs">
			{#if activeTab === 'services'}
				<div class="flex items-center gap-1 px-2 py-1 rounded-gx bg-gx-status-success/10 border border-gx-status-success/20">
					<Wifi size={12} class="text-gx-status-success" />
					<span class="text-gx-status-success font-medium">{onlineCount}</span>
				</div>
				<div class="flex items-center gap-1 px-2 py-1 rounded-gx bg-gx-status-error/10 border border-gx-status-error/20">
					<WifiOff size={12} class="text-gx-status-error" />
					<span class="text-gx-status-error font-medium">{offlineCount}</span>
				</div>
				{#if scanTimestamp}
					<div class="flex items-center gap-1 px-2 py-1 rounded-gx bg-gx-bg-tertiary text-gx-text-muted">
						<Clock size={12} />
						<span>{formatTimestamp(scanTimestamp)}</span>
						<span class="text-gx-text-muted">({formatDuration(scanDuration)})</span>
					</div>
				{/if}
			{:else}
				<div class="flex items-center gap-1 px-2 py-1 rounded-gx bg-gx-neon/10 border border-gx-neon/20">
					<AppWindow size={12} class="text-gx-neon" />
					<span class="text-gx-neon font-medium">{programs.length} apps</span>
				</div>
			{/if}
		</div>

		<!-- Action buttons -->
		{#if activeTab === 'services'}
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 border-gx-border-default text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon"
				onclick={() => showConfigDialog = true}
			>
				<Settings2 size={14} />
				Config
			</Button>

			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 border-gx-border-default text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon"
				onclick={() => showAddDialog = true}
			>
				<Plus size={14} />
				Add Custom
			</Button>
		{/if}

		<Button
			variant="default"
			size="sm"
			class="gap-1.5 bg-gx-neon text-gx-bg-primary hover:bg-gx-neon/80"
			onclick={() => activeTab === 'services' ? runScan() : loadPrograms()}
			disabled={scanning || scanningPrograms}
		>
			{#if scanning || scanningPrograms}
				<Loader2 size={14} class="animate-spin" />
				Scanning...
			{:else}
				<RefreshCw size={14} />
				{activeTab === 'services' ? 'Scan Now' : 'Re-detect'}
			{/if}
		</Button>
	</div>

	<!-- Tab bar: Services / Installed Apps -->
	<div class="flex items-center gap-1 px-6 py-1.5 border-b border-gx-border-default shrink-0">
		<button
			class="px-3 py-1.5 text-xs font-medium rounded-gx transition-all duration-150 flex items-center gap-1.5
				{activeTab === 'services'
					? 'bg-gx-neon/15 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
			onclick={() => activeTab = 'services'}
		>
			<Cable size={13} />
			Services
			<span class="ml-0.5 text-[10px] opacity-60">{totalCount}</span>
		</button>
		<button
			class="px-3 py-1.5 text-xs font-medium rounded-gx transition-all duration-150 flex items-center gap-1.5
				{activeTab === 'programs'
					? 'bg-gx-neon/15 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
			onclick={() => activeTab = 'programs'}
		>
			<AppWindow size={13} />
			Installed Apps
			<span class="ml-0.5 text-[10px] opacity-60">{programs.length}</span>
		</button>
	</div>

	{#if activeTab === 'services'}
	<!-- Filter bar -->
	<div class="flex items-center gap-3 px-6 py-2 border-b border-gx-border-default shrink-0">
		<div class="flex items-center gap-1">
			{#each categories as cat}
				<button
					class="px-2.5 py-1 text-xs rounded-gx transition-all duration-150
						{selectedCategory === cat.id
							? 'bg-gx-neon/15 text-gx-neon border border-gx-neon/30'
							: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
					onclick={() => selectedCategory = cat.id}
				>
					{cat.label}
					{#if cat.id !== 'all'}
						<span class="ml-1 text-[10px] opacity-60">
							{services.filter(s => cat.id === 'all' || serviceCategory(s) === cat.id).length}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		<div class="flex-1" />

		<div class="relative">
			<Search size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
			<input
				type="text"
				placeholder="Filter services..."
				bind:value={filterText}
				class="pl-8 pr-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx
					text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon/50 focus:outline-none w-48"
			/>
		</div>
	</div>

	<!-- Service grid -->
	<div class="flex-1 overflow-y-auto p-6">
		{#if scanning && services.length === 0}
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<Loader2 size={32} class="animate-spin text-gx-neon" />
				<p class="text-sm text-gx-text-muted">Scanning local network for services...</p>
				<p class="text-xs text-gx-text-muted">Checking ports: 11434, 5432, 5433, 6379, 5678, 8001-8015, ...</p>
			</div>
		{:else if filteredServices.length === 0 && !scanning}
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<WifiOff size={48} class="text-gx-text-muted opacity-30" />
				<p class="text-sm text-gx-text-muted">
					{services.length === 0
						? 'No services discovered yet. Click "Scan Now" to begin.'
						: 'No services match your filter.'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
				{#each filteredServices as svc (svc.id)}
					{@const IconComponent = serviceIcon(svc.service_type)}
					{@const isExpanded = expandedService === svc.id}
					{@const isChecking = checkingService === svc.id}

					<div
						class="rounded-gx border transition-all duration-200 {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'}
							{svc.status === 'online'
								? 'border-gx-status-success/20 hover:border-gx-status-success/40'
								: svc.status === 'auth_required'
									? 'border-gx-accent-orange/20 hover:border-gx-accent-orange/40'
									: 'border-gx-border-default hover:border-gx-text-muted/30'}
							{isExpanded ? 'ring-1 ring-gx-neon/20' : ''}"
						style={cardStyle}
					>
						<!-- Service header -->
						<button
							class="w-full flex items-center gap-3 p-3 text-left"
							onclick={() => toggleExpand(svc.id)}
						>
							<!-- Icon with status ring -->
							<div class="relative shrink-0">
								<div class="w-9 h-9 rounded-gx flex items-center justify-center
									{svc.status === 'online'
										? 'bg-gx-status-success/10'
										: svc.status === 'auth_required'
											? 'bg-gx-accent-orange/10'
											: 'bg-gx-bg-tertiary'}"
								>
									<IconComponent size={18} class="{svc.status === 'online' ? 'text-gx-status-success' : svc.status === 'auth_required' ? 'text-gx-accent-orange' : 'text-gx-text-muted'}" />
								</div>
								<!-- Pulse dot for online services -->
								{#if svc.status === 'online'}
									<span class="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full bg-gx-status-success border-2 border-gx-bg-secondary">
										<span class="absolute inset-0 rounded-full bg-gx-status-success animate-ping opacity-30"></span>
									</span>
								{/if}
							</div>

							<!-- Name and URL -->
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2">
									<span class="text-sm font-medium text-gx-text-primary truncate">{svc.name}</span>
									{#if svc.auto_connected}
										<Zap size={11} class="text-gx-neon shrink-0" />
									{/if}
								</div>
								<div class="flex items-center gap-2 mt-0.5">
									<span class="text-[11px] text-gx-text-muted truncate font-mono">{svc.url}</span>
									{#if svc.response_time_ms != null && svc.response_time_ms > 0}
										<span class="text-[10px] text-gx-text-muted shrink-0">{svc.response_time_ms}ms</span>
									{/if}
								</div>
							</div>

							<!-- Status badge -->
							<Badge variant={statusBadgeVariant(svc.status)} class="text-[10px] px-1.5 py-0 h-5 shrink-0">
								{statusLabel(svc.status)}
							</Badge>

							<!-- Expand arrow -->
							<div class="shrink-0 text-gx-text-muted">
								{#if isExpanded}
									<ChevronDown size={14} />
								{:else}
									<ChevronRight size={14} />
								{/if}
							</div>
						</button>

						<!-- Expanded details -->
						{#if isExpanded}
							<div class="px-3 pb-3 border-t border-gx-border-default mt-0 pt-3 space-y-3">
								<!-- Capabilities -->
								{#if svc.capabilities.length > 0}
									<div>
										<span class="text-[10px] text-gx-text-muted uppercase tracking-wider font-medium">Capabilities</span>
										<div class="flex flex-wrap gap-1 mt-1">
											{#each svc.capabilities as cap}
												<span class="px-1.5 py-0.5 text-[10px] rounded bg-gx-bg-tertiary text-gx-text-secondary border border-gx-border-default">
													{cap}
												</span>
											{/each}
										</div>
									</div>
								{/if}

								<!-- Metadata (show non-empty fields) -->
								{#if Object.keys(svc.metadata).length > 0}
									<div>
										<span class="text-[10px] text-gx-text-muted uppercase tracking-wider font-medium">Details</span>
										<div class="mt-1 space-y-1">
											{#each Object.entries(svc.metadata) as [key, value]}
												{#if value !== null && value !== ''}
													<div class="flex items-start gap-2 text-[11px]">
														<span class="text-gx-text-muted shrink-0 w-16">{key}:</span>
														<span class="text-gx-text-secondary font-mono break-all">
															{typeof value === 'object' ? JSON.stringify(value) : String(value)}
														</span>
													</div>
												{/if}
											{/each}
										</div>
									</div>
								{/if}

								<!-- Info -->
								<div class="flex items-center gap-2 text-[10px] text-gx-text-muted">
									<span>Type: {serviceTypeLabel(svc.service_type)}</span>
									<span>|</span>
									<span>Port: {svc.port || 'N/A'}</span>
									<span>|</span>
									<span>Last check: {formatTimestamp(svc.last_check)}</span>
								</div>

								<!-- Actions -->
								<div class="flex items-center gap-2 pt-1">
									<Button
										variant="outline"
										size="sm"
										class="h-7 text-xs gap-1 border-gx-border-default text-gx-text-secondary hover:border-gx-neon"
										onclick={() => checkService(svc.id)}
										disabled={isChecking}
									>
										{#if isChecking}
											<Loader2 size={12} class="animate-spin" />
										{:else}
											<RefreshCw size={12} />
										{/if}
										Re-check
									</Button>

									{#if svc.status === 'online' && !svc.auto_connected}
										<Button
											variant="outline"
											size="sm"
											class="h-7 text-xs gap-1 border-gx-neon/30 text-gx-neon hover:bg-gx-neon/10"
											onclick={() => autoConnect(svc.id)}
										>
											<Zap size={12} />
											Auto-connect
										</Button>
									{/if}

									{#if svc.service_type === 'custom_api'}
										<Button
											variant="outline"
											size="sm"
											class="h-7 text-xs gap-1 border-gx-status-error/30 text-gx-status-error hover:bg-gx-status-error/10 ml-auto"
											onclick={() => removeCustom(svc.id)}
										>
											<Trash2 size={12} />
											Remove
										</Button>
									{/if}

									{#if svc.url && svc.url.startsWith('http')}
										<a
											href={svc.url}
											target="_blank"
											rel="noopener noreferrer"
											class="ml-auto flex items-center gap-1 text-[11px] text-gx-text-muted hover:text-gx-neon transition-colors"
										>
											<ExternalLink size={11} />
											Open
										</a>
									{/if}
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}

		<!-- Scan history timeline -->
		{#if scanHistory.length > 1}
			<div class="mt-8 mb-4">
				<h2 class="text-sm font-medium text-gx-text-secondary mb-3 flex items-center gap-2">
					<Activity size={14} class="text-gx-neon" />
					Scan History
				</h2>
				<div class="space-y-1">
					{#each scanHistory.slice().reverse().slice(0, 5) as scan, i}
						<div class="flex items-center gap-3 text-[11px] py-1.5 px-3 rounded-gx hover:bg-gx-bg-hover transition-colors">
							<div class="w-1.5 h-1.5 rounded-full {i === 0 ? 'bg-gx-neon' : 'bg-gx-text-muted/30'}" />
							<span class="text-gx-text-muted w-20">{formatTimestamp(scan.timestamp)}</span>
							<span class="text-gx-text-secondary">
								{scan.services.filter(s => s.status === 'online').length} online
							</span>
							<span class="text-gx-text-muted">
								/ {scan.services.length} total
							</span>
							<span class="text-gx-text-muted ml-auto">{formatDuration(scan.scan_duration_ms)}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>
	{:else}
	<!-- ============================================================= -->
	<!-- Installed Apps tab                                             -->
	<!-- ============================================================= -->

	<!-- Program category filter bar -->
	<div class="flex items-center gap-3 px-6 py-2 border-b border-gx-border-default shrink-0">
		<div class="flex items-center gap-1 flex-wrap">
			{#each programCategoryCounts as cat}
				<button
					class="px-2.5 py-1 text-xs rounded-gx transition-all duration-150
						{selectedProgramCategory === cat.id
							? 'bg-gx-neon/15 text-gx-neon border border-gx-neon/30'
							: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
					onclick={() => selectedProgramCategory = cat.id}
				>
					{cat.label}
					{#if cat.id !== 'all'}
						<span class="ml-1 text-[10px] opacity-60">{cat.count}</span>
					{/if}
				</button>
			{/each}
		</div>

		<div class="flex-1" />

		<div class="relative">
			<Search size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
			<input
				type="text"
				placeholder="Filter apps..."
				bind:value={programFilter}
				class="pl-8 pr-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx
					text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon/50 focus:outline-none w-48"
			/>
		</div>
	</div>

	<!-- Program grid -->
	<div class="flex-1 overflow-y-auto p-6">
		{#if scanningPrograms && programs.length === 0}
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<Loader2 size={32} class="animate-spin text-gx-neon" />
				<p class="text-sm text-gx-text-muted">Detecting installed programs...</p>
				<p class="text-xs text-gx-text-muted">Scanning PATH, .desktop files, and known install locations</p>
			</div>
		{:else if filteredPrograms.length === 0 && !scanningPrograms}
			<div class="flex flex-col items-center justify-center h-full gap-4">
				<AppWindow size={48} class="text-gx-text-muted opacity-30" />
				<p class="text-sm text-gx-text-muted">
					{programs.length === 0
						? 'No programs detected yet. Click "Re-detect" to scan.'
						: 'No programs match your filter.'}
				</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-3">
				{#each filteredPrograms as prog (prog.executable + prog.name)}
					{@const CatIcon = programCategoryIcon(prog.category)}
					<div
						class="rounded-gx border border-gx-border-default hover:border-gx-neon/30 transition-all duration-200
							{hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} p-3"
						style={cardStyle}
					>
						<div class="flex items-start gap-3">
							<!-- Category icon -->
							<div class="w-9 h-9 rounded-gx bg-gx-neon/10 flex items-center justify-center shrink-0">
								<CatIcon size={18} class="text-gx-neon" />
							</div>

							<!-- Info -->
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2">
									<span class="text-sm font-medium text-gx-text-primary truncate">{prog.name}</span>
								</div>
								<div class="text-[11px] text-gx-text-muted font-mono truncate mt-0.5">
									{prog.executable}
								</div>
								{#if prog.version}
									<div class="text-[10px] text-gx-text-muted mt-1 truncate" title={prog.version}>
										{prog.version}
									</div>
								{/if}
							</div>

							<!-- Category badge -->
							<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-5 shrink-0 border-gx-border-default text-gx-text-muted">
								{programCategoryLabel(prog.category)}
							</Badge>
						</div>

						{#if prog.installed_path}
							<div class="mt-2 pt-2 border-t border-gx-border-default">
								<div class="text-[10px] text-gx-text-muted font-mono truncate" title={prog.installed_path}>
									{prog.installed_path}
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
	{/if}
</div>

<!-- Add Custom Service Dialog -->
<Dialog.Root bind:open={showAddDialog}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-md">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary flex items-center gap-2">
				<Plus size={18} class="text-gx-neon" />
				Add Custom Service
			</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-xs">
				Add any HTTP service endpoint for monitoring.
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<div>
				<label for="custom-name" class="text-xs text-gx-text-secondary mb-1 block">Name</label>
				<Input
					id="custom-name"
					bind:value={customName}
					placeholder="My API Server"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted"
				/>
			</div>
			<div>
				<label for="custom-url" class="text-xs text-gx-text-secondary mb-1 block">URL</label>
				<Input
					id="custom-url"
					bind:value={customUrl}
					placeholder="http://localhost:9000"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted"
				/>
			</div>
			<div>
				<label for="custom-health" class="text-xs text-gx-text-secondary mb-1 block">Health Path (optional)</label>
				<Input
					id="custom-health"
					bind:value={customHealthPath}
					placeholder="/health or /api/status"
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted"
				/>
			</div>

			{#if testResult}
				<div class="flex items-center gap-2 text-xs px-3 py-2 rounded-gx
					{testResult.startsWith('Error') ? 'bg-gx-status-error/10 text-gx-status-error' : 'bg-gx-status-success/10 text-gx-status-success'}">
					{#if testResult.startsWith('Error')}
						<AlertTriangle size={14} />
					{:else}
						<Check size={14} />
					{/if}
					{testResult}
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button
				variant="outline"
				class="border-gx-border-default text-gx-text-secondary"
				onclick={() => showAddDialog = false}
			>
				Cancel
			</Button>
			<Button
				class="bg-gx-neon text-gx-bg-primary hover:bg-gx-neon/80 gap-1.5"
				onclick={addCustomEndpoint}
				disabled={addingCustom || !customName.trim() || !customUrl.trim()}
			>
				{#if addingCustom}
					<Loader2 size={14} class="animate-spin" />
				{:else}
					<Plus size={14} />
				{/if}
				Add & Test
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Config Dialog -->
<Dialog.Root bind:open={showConfigDialog}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-md">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary flex items-center gap-2">
				<Settings2 size={18} class="text-gx-neon" />
				Connector Settings
			</Dialog.Title>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<label class="flex items-center justify-between">
				<div>
					<span class="text-sm text-gx-text-primary">Auto-scan on start</span>
					<p class="text-xs text-gx-text-muted">Automatically discover services when ImpForge starts</p>
				</div>
				<input
					type="checkbox"
					bind:checked={configAutoScan}
					class="w-4 h-4 rounded border-gx-border-default accent-gx-neon"
				/>
			</label>

			<div>
				<label for="scan-interval" class="text-sm text-gx-text-primary block mb-1">Re-scan interval (seconds)</label>
				<p class="text-xs text-gx-text-muted mb-2">0 = manual only</p>
				<Input
					id="scan-interval"
					type="number"
					bind:value={configInterval}
					min={0}
					max={3600}
					class="bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary w-32"
				/>
			</div>

			<label class="flex items-center justify-between">
				<div>
					<span class="text-sm text-gx-text-primary">Notifications on change</span>
					<p class="text-xs text-gx-text-muted">Notify when a service comes online or goes offline</p>
				</div>
				<input
					type="checkbox"
					bind:checked={configNotify}
					class="w-4 h-4 rounded border-gx-border-default accent-gx-neon"
				/>
			</label>

			{#if config.custom_endpoints.length > 0}
				<div>
					<span class="text-xs text-gx-text-muted uppercase tracking-wider font-medium">Custom Endpoints ({config.custom_endpoints.length})</span>
					<div class="mt-1 space-y-1">
						{#each config.custom_endpoints as ep}
							<div class="flex items-center gap-2 text-xs py-1">
								<Server size={12} class="text-gx-text-muted" />
								<span class="text-gx-text-secondary">{ep.name}</span>
								<span class="text-gx-text-muted font-mono text-[10px]">{ep.url}</span>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button
				variant="outline"
				class="border-gx-border-default text-gx-text-secondary"
				onclick={() => showConfigDialog = false}
			>
				Cancel
			</Button>
			<Button
				class="bg-gx-neon text-gx-bg-primary hover:bg-gx-neon/80"
				onclick={saveConfig}
			>
				Save Settings
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
