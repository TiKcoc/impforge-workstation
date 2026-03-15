<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import {
		Grid3x3, List, Plus, Search, ExternalLink, Trash2, Pin, PinOff,
		Monitor, Globe, Terminal, Server, Play, Heart, Clock,
		LayoutGrid, RefreshCw, ScanSearch, X, Folder, Link,
		Activity, Settings, MoreVertical, Eye, Edit, ChevronRight
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import {
		appLauncherStore,
		APP_CATEGORIES,
		type AppCategory,
		type AppType,
		type AppEntry,
	} from '$lib/stores/app-launcher.svelte';

	// ─── BenikUI style engine integration ───────────────────
	const widgetId = 'page-apps';
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

	// ─── View state ─────────────────────────────────────────
	let viewMode = $state<'grid' | 'list'>('grid');
	let activeCategory = $state<AppCategory>('All');
	let searchQuery = $state('');
	let showAddDialog = $state(false);
	let confirmRemoveId = $state<string | null>(null);

	// ─── Add App form state ─────────────────────────────────
	let newAppName = $state('');
	let newAppType = $state<'NativeProcess' | 'WebView' | 'WebService' | 'McpServer'>('NativeProcess');
	let newAppCommand = $state('');
	let newAppArgs = $state('');
	let newAppUrl = $state('');
	let newAppHealthEndpoint = $state('');
	let newAppPort = $state('');
	let newAppCategory = $state('Custom');
	let newAppPinned = $state(false);
	let newAppAutoStart = $state(false);
	let newAppSidebar = $state(true);
	let newAppMonitoring = $state(false);
	let addingApp = $state(false);

	// ─── Filtered apps ──────────────────────────────────────
	let filteredApps = $derived(appLauncherStore.getFilteredApps(activeCategory, searchQuery));

	// ─── Health check polling ───────────────────────────────
	let healthInterval: ReturnType<typeof setInterval> | null = null;

	onMount(async () => {
		await appLauncherStore.loadApps();
		// Initial health check
		await appLauncherStore.checkAllHealth();
		// Poll health every 30 seconds
		healthInterval = setInterval(() => {
			appLauncherStore.checkAllHealth();
		}, 30_000);
	});

	onDestroy(() => {
		if (healthInterval) {
			clearInterval(healthInterval);
			healthInterval = null;
		}
	});

	// ─── Helpers ────────────────────────────────────────────

	function appTypeIcon(appType: AppType) {
		switch (appType.type) {
			case 'NativeProcess': return Terminal;
			case 'WebView': return Globe;
			case 'WebService': return Server;
			case 'McpServer': return Activity;
		}
	}

	function appTypeLabel(appType: AppType): string {
		switch (appType.type) {
			case 'NativeProcess': return 'Native';
			case 'WebView': return 'Web App';
			case 'WebService': return 'API';
			case 'McpServer': return 'MCP';
		}
	}

	function appTypeBadgeColor(appType: AppType): string {
		switch (appType.type) {
			case 'NativeProcess': return 'border-gx-accent-blue/40 text-gx-accent-blue';
			case 'WebView': return 'border-gx-neon/40 text-gx-neon';
			case 'WebService': return 'border-gx-accent-magenta/40 text-gx-accent-magenta';
			case 'McpServer': return 'border-gx-accent-purple/40 text-gx-accent-purple';
		}
	}

	function formatLastUsed(lastUsed: string | null): string {
		if (!lastUsed) return 'Never';
		const date = new Date(lastUsed);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMin = Math.floor(diffMs / 60_000);
		if (diffMin < 1) return 'Just now';
		if (diffMin < 60) return `${diffMin}m ago`;
		const diffH = Math.floor(diffMin / 60);
		if (diffH < 24) return `${diffH}h ago`;
		const diffD = Math.floor(diffH / 24);
		return `${diffD}d ago`;
	}

	function resetAddForm() {
		newAppName = '';
		newAppType = 'NativeProcess';
		newAppCommand = '';
		newAppArgs = '';
		newAppUrl = '';
		newAppHealthEndpoint = '';
		newAppPort = '';
		newAppCategory = 'Custom';
		newAppPinned = false;
		newAppAutoStart = false;
		newAppSidebar = true;
		newAppMonitoring = false;
	}

	// ─── Actions ────────────────────────────────────────────

	async function handleAddApp() {
		if (!newAppName.trim()) return;
		addingApp = true;

		let appType: AppType;
		switch (newAppType) {
			case 'NativeProcess':
				appType = {
					type: 'NativeProcess',
					command: newAppCommand,
					args: newAppArgs ? newAppArgs.split(' ').filter(Boolean) : [],
				};
				break;
			case 'WebView':
				appType = { type: 'WebView', url: newAppUrl };
				break;
			case 'WebService':
				appType = {
					type: 'WebService',
					url: newAppUrl,
					health_endpoint: newAppHealthEndpoint || null,
				};
				break;
			case 'McpServer':
				appType = {
					type: 'McpServer',
					command: newAppCommand,
					args: newAppArgs ? newAppArgs.split(' ').filter(Boolean) : [],
					port: newAppPort ? parseInt(newAppPort, 10) : null,
				};
				break;
		}

		await appLauncherStore.addApp({
			name: newAppName.trim(),
			app_type: appType,
			icon: null,
			category: newAppCategory,
			pinned: newAppPinned,
			launch_config: {
				auto_start: newAppAutoStart,
				show_in_sidebar: newAppSidebar,
				monitoring_enabled: newAppMonitoring,
			},
		});

		addingApp = false;
		showAddDialog = false;
		resetAddForm();
	}

	async function handleLaunchApp(app: AppEntry) {
		const result = await appLauncherStore.launchApp(app.id);
		if (result?.url && app.app_type.type === 'WebView') {
			goto(`/apps/${app.id}`);
		} else if (app.app_type.type === 'WebService') {
			goto(`/apps/${app.id}`);
		}
	}

	async function handleRemoveApp(id: string) {
		if (confirmRemoveId !== id) {
			confirmRemoveId = id;
			return;
		}
		await appLauncherStore.removeApp(id);
		confirmRemoveId = null;
	}

	async function handleDiscover() {
		await appLauncherStore.discoverInstalled();
	}
</script>

<!-- App Library Page -->
<div class="flex flex-col h-full overflow-hidden p-4 gap-4" style={containerStyle}>
	<!-- Header Row -->
	<div class="flex items-center gap-3 shrink-0">
		<div class="flex items-center gap-2">
			<LayoutGrid size={22} class="text-gx-neon" />
			<h1 class="text-lg font-semibold text-gx-text-primary">App Library</h1>
		</div>

		<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-5 border-gx-border-default text-gx-text-muted">
			{appLauncherStore.apps.length} apps
		</Badge>

		<div class="flex-1"></div>

		<!-- Search -->
		<div class="relative w-64">
			<Search size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
			<Input
				type="text"
				placeholder="Search apps..."
				bind:value={searchQuery}
				class="pl-8 h-8 text-sm bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
			/>
			{#if searchQuery}
				<button
					onclick={() => searchQuery = ''}
					class="absolute right-2 top-1/2 -translate-y-1/2 text-gx-text-muted hover:text-gx-text-primary transition-colors"
				>
					<X size={12} />
				</button>
			{/if}
		</div>

		<!-- View toggle -->
		<div class="flex items-center rounded-gx border border-gx-border-default overflow-hidden">
			<button
				onclick={() => viewMode = 'grid'}
				class="p-1.5 transition-colors {viewMode === 'grid' ? 'bg-gx-bg-elevated text-gx-neon' : 'bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-text-secondary'}"
				aria-label="Grid view"
			>
				<Grid3x3 size={14} />
			</button>
			<button
				onclick={() => viewMode = 'list'}
				class="p-1.5 transition-colors {viewMode === 'list' ? 'bg-gx-bg-elevated text-gx-neon' : 'bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-text-secondary'}"
				aria-label="List view"
			>
				<List size={14} />
			</button>
		</div>

		<!-- Discover button -->
		<Tooltip.Provider>
			<Tooltip.Root>
				<Tooltip.Trigger>
					<Button
						variant="outline"
						size="sm"
						disabled={appLauncherStore.discovering}
						onclick={handleDiscover}
						class="h-8 gap-1.5 border-gx-border-default text-gx-text-secondary hover:text-gx-neon hover:border-gx-neon bg-gx-bg-tertiary"
					>
						<ScanSearch size={14} class={appLauncherStore.discovering ? 'animate-spin' : ''} />
						<span class="text-xs">Discover</span>
					</Button>
				</Tooltip.Trigger>
				<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
					Scan system for installed applications
				</Tooltip.Content>
			</Tooltip.Root>
		</Tooltip.Provider>

		<!-- Add App button -->
		<Button
			variant="outline"
			size="sm"
			onclick={() => showAddDialog = true}
			class="h-8 gap-1.5 border-gx-neon/50 text-gx-neon hover:bg-gx-neon/10 bg-gx-bg-tertiary"
		>
			<Plus size={14} />
			<span class="text-xs">Add App</span>
		</Button>
	</div>

	<!-- Category Tabs -->
	<div class="flex items-center gap-1 shrink-0">
		{#each APP_CATEGORIES as category}
			<button
				onclick={() => activeCategory = category}
				class="px-3 py-1.5 text-xs font-medium rounded-gx transition-all duration-200
					{activeCategory === category
						? 'bg-gx-bg-elevated text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
			>
				{category}
				{#if category === 'Favorites'}
					<span class="ml-1 text-[10px] text-gx-text-muted">({appLauncherStore.favoriteApps.length})</span>
				{/if}
			</button>
		{/each}
	</div>

	<!-- Error message -->
	{#if appLauncherStore.error}
		<div class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-sm text-gx-status-error shrink-0">
			<X size={14} />
			<span>{appLauncherStore.error}</span>
		</div>
	{/if}

	<!-- App Grid / List -->
	<div class="flex-1 overflow-y-auto">
		{#if appLauncherStore.loading}
			<!-- Loading skeleton -->
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
				{#each Array(8) as _}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary animate-pulse h-36"></div>
				{/each}
			</div>
		{:else if filteredApps.length === 0}
			<!-- Empty state -->
			<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
				<LayoutGrid size={48} class="opacity-30" />
				{#if searchQuery}
					<p class="text-sm">No apps matching "{searchQuery}"</p>
				{:else if activeCategory === 'Favorites'}
					<p class="text-sm">No pinned favorites yet</p>
					<p class="text-xs">Pin apps to see them here</p>
				{:else}
					<p class="text-sm">No apps added yet</p>
					<p class="text-xs">Click "Add App" or "Discover" to get started</p>
				{/if}
			</div>
		{:else if viewMode === 'grid'}
			<!-- Grid View -->
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
				{#each filteredApps as app (app.id)}
					{@const health = appLauncherStore.getHealth(app.id)}
					{@const TypeIcon = appTypeIcon(app.app_type)}
					<div
						class="group relative rounded-gx border border-gx-border-default bg-gx-bg-secondary hover:border-gx-neon/40 hover:bg-gx-bg-elevated transition-all duration-200 cursor-pointer"
						style={cardStyle}
					>
						<!-- Health status dot (top right) -->
						{#if app.launch_config.monitoring_enabled}
							<div class="absolute top-2 right-2">
								<span class="w-2 h-2 rounded-full inline-block {health?.healthy ? 'bg-gx-status-success' : health ? 'bg-gx-status-error' : 'bg-gx-text-muted'}"></span>
							</div>
						{/if}

						<!-- Pin indicator -->
						{#if app.pinned}
							<div class="absolute top-2 left-2">
								<Pin size={10} class="text-gx-neon" />
							</div>
						{/if}

						<!-- Card Content (clickable to launch) -->
						<button
							class="w-full p-4 text-left"
							onclick={() => handleLaunchApp(app)}
						>
							<!-- Icon + Name row -->
							<div class="flex items-start gap-3 mb-3">
								<div class="flex items-center justify-center w-10 h-10 rounded-gx bg-gx-bg-tertiary border border-gx-border-default shrink-0">
									{#if app.icon}
										<img src={app.icon} alt="" class="w-6 h-6 rounded" />
									{:else}
										<TypeIcon size={20} class="text-gx-text-muted" />
									{/if}
								</div>
								<div class="min-w-0 flex-1">
									<h3 class="text-sm font-medium text-gx-text-primary truncate">{app.name}</h3>
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 mt-1 {appTypeBadgeColor(app.app_type)}">
										{appTypeLabel(app.app_type)}
									</Badge>
								</div>
							</div>

							<!-- Meta row -->
							<div class="flex items-center gap-3 text-[11px] text-gx-text-muted">
								<span class="flex items-center gap-1">
									<Folder size={10} />
									{app.category}
								</span>
								<span class="flex items-center gap-1">
									<Clock size={10} />
									{formatLastUsed(app.last_used)}
								</span>
								{#if app.usage_count > 0}
									<span class="flex items-center gap-1">
										<Play size={10} />
										{app.usage_count}
									</span>
								{/if}
							</div>
						</button>

						<!-- Context menu button (visible on hover) -->
						<div class="absolute bottom-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
							<DropdownMenu.Root>
								<DropdownMenu.Trigger>
									<button class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-primary transition-colors">
										<MoreVertical size={14} />
									</button>
								</DropdownMenu.Trigger>
								<DropdownMenu.Content class="bg-gx-bg-elevated border-gx-border-default min-w-[160px]">
									<DropdownMenu.Item
										class="text-xs text-gx-text-secondary data-[highlighted]:bg-gx-bg-hover data-[highlighted]:text-gx-neon gap-2"
										onclick={() => handleLaunchApp(app)}
									>
										<Play size={12} />
										Launch
									</DropdownMenu.Item>
									<DropdownMenu.Item
										class="text-xs text-gx-text-secondary data-[highlighted]:bg-gx-bg-hover data-[highlighted]:text-gx-neon gap-2"
										onclick={() => appLauncherStore.pinApp(app.id)}
									>
										{#if app.pinned}
											<PinOff size={12} />
											Unpin from Sidebar
										{:else}
											<Pin size={12} />
											Pin to Sidebar
										{/if}
									</DropdownMenu.Item>
									{#if app.app_type.type === 'WebView' || app.app_type.type === 'WebService'}
										<DropdownMenu.Item
											class="text-xs text-gx-text-secondary data-[highlighted]:bg-gx-bg-hover data-[highlighted]:text-gx-neon gap-2"
											onclick={() => goto(`/apps/${app.id}`)}
										>
											<Eye size={12} />
											Open Panel
										</DropdownMenu.Item>
									{/if}
									<DropdownMenu.Item
										class="text-xs text-gx-text-secondary data-[highlighted]:bg-gx-bg-hover data-[highlighted]:text-gx-neon gap-2"
										onclick={() => appLauncherStore.checkHealth(app.id)}
									>
										<Activity size={12} />
										Health Check
									</DropdownMenu.Item>
									<DropdownMenu.Separator class="bg-gx-border-default" />
									<DropdownMenu.Item
										class="text-xs text-gx-status-error data-[highlighted]:bg-gx-status-error/10 data-[highlighted]:text-gx-status-error gap-2"
										onclick={() => handleRemoveApp(app.id)}
									>
										<Trash2 size={12} />
										{confirmRemoveId === app.id ? 'Confirm Remove' : 'Remove'}
									</DropdownMenu.Item>
								</DropdownMenu.Content>
							</DropdownMenu.Root>
						</div>
					</div>
				{/each}
			</div>
		{:else}
			<!-- List View -->
			<div class="flex flex-col gap-1">
				{#each filteredApps as app (app.id)}
					{@const health = appLauncherStore.getHealth(app.id)}
					{@const TypeIcon = appTypeIcon(app.app_type)}
					<div
						class="group flex items-center gap-3 px-3 py-2 rounded-gx border border-gx-border-default bg-gx-bg-secondary hover:border-gx-neon/40 hover:bg-gx-bg-elevated transition-all duration-200"
						style={cardStyle}
					>
						<!-- Health dot -->
						{#if app.launch_config.monitoring_enabled}
							<span class="w-2 h-2 rounded-full shrink-0 {health?.healthy ? 'bg-gx-status-success' : health ? 'bg-gx-status-error' : 'bg-gx-text-muted'}"></span>
						{:else}
							<span class="w-2 shrink-0"></span>
						{/if}

						<!-- Icon -->
						<div class="flex items-center justify-center w-8 h-8 rounded bg-gx-bg-tertiary border border-gx-border-default shrink-0">
							{#if app.icon}
								<img src={app.icon} alt="" class="w-5 h-5 rounded" />
							{:else}
								<TypeIcon size={16} class="text-gx-text-muted" />
							{/if}
						</div>

						<!-- Name + type -->
						<div class="min-w-0 flex-1">
							<span class="text-sm text-gx-text-primary">{app.name}</span>
						</div>

						<!-- Type badge -->
						<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {appTypeBadgeColor(app.app_type)}">
							{appTypeLabel(app.app_type)}
						</Badge>

						<!-- Category -->
						<span class="text-[11px] text-gx-text-muted w-20 text-right">{app.category}</span>

						<!-- Usage -->
						<span class="text-[11px] text-gx-text-muted w-12 text-right">{app.usage_count}x</span>

						<!-- Last used -->
						<span class="text-[11px] text-gx-text-muted w-16 text-right">{formatLastUsed(app.last_used)}</span>

						<!-- Pin indicator -->
						{#if app.pinned}
							<Pin size={12} class="text-gx-neon shrink-0" />
						{:else}
							<span class="w-3 shrink-0"></span>
						{/if}

						<!-- Actions -->
						<div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
							<Tooltip.Provider>
								<Tooltip.Root>
									<Tooltip.Trigger>
										<button
											onclick={() => handleLaunchApp(app)}
											class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-neon transition-colors"
										>
											<Play size={14} />
										</button>
									</Tooltip.Trigger>
									<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">Launch</Tooltip.Content>
								</Tooltip.Root>
							</Tooltip.Provider>

							<Tooltip.Provider>
								<Tooltip.Root>
									<Tooltip.Trigger>
										<button
											onclick={() => appLauncherStore.pinApp(app.id)}
											class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-neon transition-colors"
										>
											{#if app.pinned}
												<PinOff size={14} />
											{:else}
												<Pin size={14} />
											{/if}
										</button>
									</Tooltip.Trigger>
									<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
										{app.pinned ? 'Unpin' : 'Pin to Sidebar'}
									</Tooltip.Content>
								</Tooltip.Root>
							</Tooltip.Provider>

							<Tooltip.Provider>
								<Tooltip.Root>
									<Tooltip.Trigger>
										<button
											onclick={() => handleRemoveApp(app.id)}
											class="p-1 rounded hover:bg-gx-status-error/20 transition-colors
												{confirmRemoveId === app.id ? 'text-gx-status-error' : 'text-gx-text-muted hover:text-gx-status-error'}"
										>
											<Trash2 size={14} />
										</button>
									</Tooltip.Trigger>
									<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
										{confirmRemoveId === app.id ? 'Click again to confirm' : 'Remove'}
									</Tooltip.Content>
								</Tooltip.Root>
							</Tooltip.Provider>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<!-- ═══════════════════════════════════════════════════════════ -->
<!-- Add App Dialog                                            -->
<!-- ═══════════════════════════════════════════════════════════ -->
<Dialog.Root bind:open={showAddDialog} onOpenChange={(open) => { if (!open) resetAddForm(); }}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-lg">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary flex items-center gap-2">
				<Plus size={18} class="text-gx-neon" />
				Add Application
			</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Add an external program, website, API, or MCP server to your library.
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			<!-- App Name -->
			<div class="space-y-1.5">
				<label for="app-name" class="text-xs font-medium text-gx-text-secondary">Name</label>
				<Input
					id="app-name"
					bind:value={newAppName}
					placeholder="My Application"
					class="h-8 text-sm bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
				/>
			</div>

			<!-- App Type Selector -->
			<fieldset class="space-y-1.5">
				<legend class="text-xs font-medium text-gx-text-secondary">Type</legend>
				<div class="grid grid-cols-4 gap-2" role="radiogroup" aria-label="Application type">
					{#each [
						{ type: 'NativeProcess' as const, icon: Terminal, label: 'Native' },
						{ type: 'WebView' as const, icon: Globe, label: 'Website' },
						{ type: 'WebService' as const, icon: Server, label: 'API' },
						{ type: 'McpServer' as const, icon: Activity, label: 'MCP' },
					] as option}
						<button
							onclick={() => newAppType = option.type}
							class="flex flex-col items-center gap-1.5 p-2.5 rounded-gx border transition-all duration-200
								{newAppType === option.type
									? 'border-gx-neon/50 bg-gx-neon/10 text-gx-neon'
									: 'border-gx-border-default bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-text-secondary hover:border-gx-border-default'}"
						>
							<option.icon size={18} />
							<span class="text-[10px] font-medium">{option.label}</span>
						</button>
					{/each}
				</div>
			</fieldset>

			<!-- Type-specific fields -->
			{#if newAppType === 'NativeProcess'}
				<div class="space-y-3">
					<div class="space-y-1.5">
						<label for="app-command" class="text-xs font-medium text-gx-text-secondary">Executable Path</label>
						<Input
							id="app-command"
							bind:value={newAppCommand}
							placeholder="/usr/bin/my-program"
							class="h-8 text-sm font-mono bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
					<div class="space-y-1.5">
						<label for="app-args" class="text-xs font-medium text-gx-text-secondary">Arguments (space-separated)</label>
						<Input
							id="app-args"
							bind:value={newAppArgs}
							placeholder="--flag value"
							class="h-8 text-sm font-mono bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
				</div>
			{:else if newAppType === 'WebView'}
				<div class="space-y-1.5">
					<label for="app-url" class="text-xs font-medium text-gx-text-secondary">URL</label>
					<Input
						id="app-url"
						bind:value={newAppUrl}
						placeholder="https://example.com"
						class="h-8 text-sm bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
					/>
				</div>
			{:else if newAppType === 'WebService'}
				<div class="space-y-3">
					<div class="space-y-1.5">
						<label for="app-url-svc" class="text-xs font-medium text-gx-text-secondary">Base URL</label>
						<Input
							id="app-url-svc"
							bind:value={newAppUrl}
							placeholder="http://localhost:8080"
							class="h-8 text-sm bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
					<div class="space-y-1.5">
						<label for="app-health" class="text-xs font-medium text-gx-text-secondary">Health Endpoint (optional)</label>
						<Input
							id="app-health"
							bind:value={newAppHealthEndpoint}
							placeholder="/health or /api/status"
							class="h-8 text-sm font-mono bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
				</div>
			{:else if newAppType === 'McpServer'}
				<div class="space-y-3">
					<div class="space-y-1.5">
						<label for="app-mcp-cmd" class="text-xs font-medium text-gx-text-secondary">Command</label>
						<Input
							id="app-mcp-cmd"
							bind:value={newAppCommand}
							placeholder="python -m my_mcp_server"
							class="h-8 text-sm font-mono bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
					<div class="space-y-1.5">
						<label for="app-mcp-args" class="text-xs font-medium text-gx-text-secondary">Arguments (space-separated)</label>
						<Input
							id="app-mcp-args"
							bind:value={newAppArgs}
							placeholder="--port 8080"
							class="h-8 text-sm font-mono bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
					<div class="space-y-1.5">
						<label for="app-mcp-port" class="text-xs font-medium text-gx-text-secondary">Port (optional)</label>
						<Input
							id="app-mcp-port"
							type="number"
							bind:value={newAppPort}
							placeholder="8080"
							class="h-8 text-sm bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon"
						/>
					</div>
				</div>
			{/if}

			<!-- Category -->
			<div class="space-y-1.5">
				<label for="app-category" class="text-xs font-medium text-gx-text-secondary">Category</label>
				<select
					id="app-category"
					bind:value={newAppCategory}
					class="w-full h-8 text-sm rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-primary px-2 focus:outline-none focus:border-gx-neon"
				>
					<option value="Development">Development</option>
					<option value="Web">Web</option>
					<option value="Services">Services</option>
					<option value="Custom">Custom</option>
				</select>
			</div>

			<!-- Options row -->
			<div class="flex flex-wrap gap-4 pt-1">
				<label class="flex items-center gap-2 text-xs text-gx-text-secondary cursor-pointer">
					<input type="checkbox" bind:checked={newAppPinned} class="rounded border-gx-border-default" />
					Pin to sidebar
				</label>
				<label class="flex items-center gap-2 text-xs text-gx-text-secondary cursor-pointer">
					<input type="checkbox" bind:checked={newAppAutoStart} class="rounded border-gx-border-default" />
					Auto-start
				</label>
				<label class="flex items-center gap-2 text-xs text-gx-text-secondary cursor-pointer">
					<input type="checkbox" bind:checked={newAppSidebar} class="rounded border-gx-border-default" />
					Show in sidebar
				</label>
				<label class="flex items-center gap-2 text-xs text-gx-text-secondary cursor-pointer">
					<input type="checkbox" bind:checked={newAppMonitoring} class="rounded border-gx-border-default" />
					Health monitoring
				</label>
			</div>
		</div>

		<Dialog.Footer class="gap-2">
			<Button
				variant="outline"
				size="sm"
				onclick={() => { showAddDialog = false; resetAddForm(); }}
				class="border-gx-border-default text-gx-text-secondary"
			>
				Cancel
			</Button>
			<Button
				size="sm"
				disabled={!newAppName.trim() || addingApp}
				onclick={handleAddApp}
				class="bg-gx-neon/20 text-gx-neon border border-gx-neon/40 hover:bg-gx-neon/30"
			>
				{#if addingApp}
					<RefreshCw size={14} class="animate-spin mr-1" />
				{/if}
				Add App
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
