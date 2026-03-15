<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import {
		ArrowLeft, Globe, Server, Terminal, Activity,
		ExternalLink, RefreshCw, Pin, PinOff, Play,
		Circle, Trash2, Clock, Folder, Link
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import { appLauncherStore, type AppEntry, type AppHealthStatus } from '$lib/stores/app-launcher.svelte';

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

	// ─── State ──────────────────────────────────────────────
	let appId = $derived($page.params.id ?? '');
	let app = $derived(appId ? appLauncherStore.getApp(appId) : undefined);
	let health = $derived(appId ? appLauncherStore.getHealth(appId) : undefined);
	let iframeUrl = $state<string | null>(null);
	let processStatus = $state<string>('idle');
	let refreshing = $state(false);

	onMount(async () => {
		// Ensure apps are loaded
		if (appLauncherStore.apps.length === 0) {
			await appLauncherStore.loadApps();
		}

		// If WebView, get the URL
		if (app?.app_type.type === 'WebView') {
			iframeUrl = app.app_type.url;
		}

		// Check health for this app
		if (app) {
			await appLauncherStore.checkHealth(app.id);
		}
	});

	async function handleLaunch() {
		if (!app) return;
		processStatus = 'launching...';
		const result = await appLauncherStore.launchApp(app.id);
		if (result?.url) {
			iframeUrl = result.url;
		}
		processStatus = result?.status ?? 'running';
	}

	async function handleRefreshHealth() {
		if (!app) return;
		refreshing = true;
		await appLauncherStore.checkHealth(app.id);
		refreshing = false;
	}

	function getAppUrl(entry: AppEntry): string | null {
		if (entry.app_type.type === 'WebView') return entry.app_type.url;
		if (entry.app_type.type === 'WebService') return entry.app_type.url;
		return null;
	}
</script>

<div class="flex flex-col h-full overflow-hidden" style={containerStyle}>
	{#if !app}
		<!-- App not found -->
		<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
			<Circle size={48} class="opacity-30" />
			<p class="text-sm">App not found</p>
			<Button
				variant="outline"
				size="sm"
				onclick={() => goto('/apps')}
				class="border-gx-border-default text-gx-text-secondary"
			>
				<ArrowLeft size={14} class="mr-1.5" />
				Back to Library
			</Button>
		</div>
	{:else}
		<!-- Header bar -->
		<div class="flex items-center gap-3 px-4 py-2.5 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
			<Button
				variant="ghost"
				size="sm"
				onclick={() => goto('/apps')}
				class="h-7 px-2 text-gx-text-muted hover:text-gx-text-primary"
			>
				<ArrowLeft size={16} />
			</Button>

			<!-- App icon -->
			<div class="flex items-center justify-center w-7 h-7 rounded bg-gx-bg-tertiary border border-gx-border-default shrink-0">
				{#if app.app_type.type === 'WebView'}
					<Globe size={14} class="text-gx-neon" />
				{:else if app.app_type.type === 'WebService'}
					<Server size={14} class="text-gx-accent-magenta" />
				{:else if app.app_type.type === 'McpServer'}
					<Activity size={14} class="text-gx-accent-purple" />
				{:else}
					<Terminal size={14} class="text-gx-accent-blue" />
				{/if}
			</div>

			<div class="min-w-0">
				<h2 class="text-sm font-medium text-gx-text-primary truncate">{app.name}</h2>
			</div>

			<!-- Health indicator -->
			{#if app.launch_config.monitoring_enabled}
				<div class="flex items-center gap-1.5">
					<span class="w-2 h-2 rounded-full {health?.healthy ? 'bg-gx-status-success' : health ? 'bg-gx-status-error' : 'bg-gx-text-muted'}"></span>
					<span class="text-[11px] text-gx-text-muted">
						{health?.healthy ? 'Healthy' : health ? 'Unhealthy' : 'Unknown'}
					</span>
				</div>
			{/if}

			<div class="flex-1"></div>

			<!-- Actions -->
			<Button
				variant="ghost"
				size="sm"
				onclick={() => appLauncherStore.pinApp(app.id)}
				class="h-7 px-2 text-gx-text-muted hover:text-gx-neon"
			>
				{#if app.pinned}
					<PinOff size={14} />
				{:else}
					<Pin size={14} />
				{/if}
			</Button>

			{#if getAppUrl(app)}
				<Button
					variant="ghost"
					size="sm"
					onclick={() => { const u = getAppUrl(app!); if (u) window.open(u, '_blank'); }}
					class="h-7 px-2 text-gx-text-muted hover:text-gx-neon"
				>
					<ExternalLink size={14} />
				</Button>
			{/if}

			<Button
				variant="ghost"
				size="sm"
				onclick={handleRefreshHealth}
				disabled={refreshing}
				class="h-7 px-2 text-gx-text-muted hover:text-gx-neon"
			>
				<RefreshCw size={14} class={refreshing ? 'animate-spin' : ''} />
			</Button>
		</div>

		<!-- Content area -->
		<div class="flex-1 overflow-hidden">
			{#if app.app_type.type === 'WebView'}
				<!-- WebView: Sandboxed iframe -->
				{@const url = iframeUrl ?? app.app_type.url}
				{#if url}
					<iframe
						src={url}
						title={app.name}
						class="w-full h-full border-0"
						sandbox="allow-scripts allow-same-origin allow-popups allow-forms allow-modals"
						referrerpolicy="no-referrer"
						loading="lazy"
					></iframe>
				{:else}
					<div class="flex items-center justify-center h-full text-gx-text-muted">
						<p class="text-sm">No URL configured</p>
					</div>
				{/if}

			{:else if app.app_type.type === 'WebService'}
				<!-- WebService: Health dashboard + API info -->
				<div class="p-6 space-y-6 overflow-y-auto h-full">
					<!-- Service Info Card -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
						<h3 class="text-sm font-semibold text-gx-text-primary mb-4 flex items-center gap-2">
							<Server size={16} class="text-gx-accent-magenta" />
							Service Information
						</h3>
						<div class="grid grid-cols-2 gap-4">
							<div class="space-y-1">
								<span class="text-[11px] text-gx-text-muted">Base URL</span>
								<p class="text-sm text-gx-text-primary font-mono flex items-center gap-1.5">
									<Link size={12} class="text-gx-text-muted shrink-0" />
									{app.app_type.url}
								</p>
							</div>
							{#if app.app_type.health_endpoint}
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Health Endpoint</span>
									<p class="text-sm text-gx-text-primary font-mono">{app.app_type.health_endpoint}</p>
								</div>
							{/if}
							<div class="space-y-1">
								<span class="text-[11px] text-gx-text-muted">Category</span>
								<p class="text-sm text-gx-text-primary flex items-center gap-1.5">
									<Folder size={12} class="text-gx-text-muted" />
									{app.category}
								</p>
							</div>
							<div class="space-y-1">
								<span class="text-[11px] text-gx-text-muted">Usage</span>
								<p class="text-sm text-gx-text-primary">{app.usage_count} launches</p>
							</div>
						</div>
					</div>

					<!-- Health Status Card -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
						<div class="flex items-center justify-between mb-4">
							<h3 class="text-sm font-semibold text-gx-text-primary flex items-center gap-2">
								<Activity size={16} class="text-gx-neon" />
								Health Status
							</h3>
							<Button
								variant="outline"
								size="sm"
								onclick={handleRefreshHealth}
								disabled={refreshing}
								class="h-7 text-xs border-gx-border-default text-gx-text-secondary"
							>
								<RefreshCw size={12} class="{refreshing ? 'animate-spin' : ''} mr-1" />
								Check Now
							</Button>
						</div>
						{#if health}
							<div class="flex items-center gap-3 p-3 rounded-gx {health.healthy ? 'bg-gx-status-success/10 border border-gx-status-success/30' : 'bg-gx-status-error/10 border border-gx-status-error/30'}">
								<span class="w-3 h-3 rounded-full {health.healthy ? 'bg-gx-status-success' : 'bg-gx-status-error'}"></span>
								<div>
									<p class="text-sm font-medium {health.healthy ? 'text-gx-status-success' : 'text-gx-status-error'}">
										{health.healthy ? 'Service is healthy' : 'Service is unhealthy'}
									</p>
									<p class="text-xs text-gx-text-muted mt-0.5">{health.message}</p>
								</div>
							</div>
						{:else}
							<p class="text-sm text-gx-text-muted">No health data available. Click "Check Now" to run a health check.</p>
						{/if}
					</div>

					<!-- Quick Actions -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
						<h3 class="text-sm font-semibold text-gx-text-primary mb-3 flex items-center gap-2">
							<Play size={16} class="text-gx-accent-blue" />
							Actions
						</h3>
						<div class="flex gap-2">
							<Button
								variant="outline"
								size="sm"
								onclick={() => window.open(app.app_type.type === 'WebService' ? app.app_type.url : '', '_blank')}
								class="border-gx-border-default text-gx-text-secondary hover:text-gx-neon"
							>
								<ExternalLink size={14} class="mr-1.5" />
								Open in Browser
							</Button>
							<Button
								variant="outline"
								size="sm"
								onclick={handleLaunch}
								class="border-gx-border-default text-gx-text-secondary hover:text-gx-neon"
							>
								<Play size={14} class="mr-1.5" />
								Launch
							</Button>
						</div>
					</div>
				</div>

			{:else if app.app_type.type === 'NativeProcess'}
				<!-- Native Process: Status + launch -->
				<div class="p-6 space-y-6 overflow-y-auto h-full">
					<!-- Process Info -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
						<h3 class="text-sm font-semibold text-gx-text-primary mb-4 flex items-center gap-2">
							<Terminal size={16} class="text-gx-accent-blue" />
							Process Details
						</h3>
						<div class="space-y-3">
							<div class="space-y-1">
								<span class="text-[11px] text-gx-text-muted">Command</span>
								<p class="text-sm text-gx-text-primary font-mono bg-gx-bg-tertiary rounded px-2 py-1">
									{app.app_type.command}
									{#if app.app_type.args.length > 0}
										{' '}{app.app_type.args.join(' ')}
									{/if}
								</p>
							</div>
							<div class="grid grid-cols-3 gap-4">
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Status</span>
									<div class="flex items-center gap-1.5">
										<span class="w-2 h-2 rounded-full {processStatus === 'running' ? 'bg-gx-status-success' : 'bg-gx-text-muted'}"></span>
										<span class="text-sm text-gx-text-primary capitalize">{processStatus}</span>
									</div>
								</div>
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Category</span>
									<p class="text-sm text-gx-text-primary">{app.category}</p>
								</div>
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Usage</span>
									<p class="text-sm text-gx-text-primary">{app.usage_count} launches</p>
								</div>
							</div>
						</div>
					</div>

					<!-- Launch Action -->
					<div class="flex gap-2">
						<Button
							size="sm"
							onclick={handleLaunch}
							class="bg-gx-neon/20 text-gx-neon border border-gx-neon/40 hover:bg-gx-neon/30"
						>
							<Play size={14} class="mr-1.5" />
							Launch Process
						</Button>
					</div>
				</div>

			{:else if app.app_type.type === 'McpServer'}
				<!-- MCP Server: Status + config -->
				<div class="p-6 space-y-6 overflow-y-auto h-full">
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
						<h3 class="text-sm font-semibold text-gx-text-primary mb-4 flex items-center gap-2">
							<Activity size={16} class="text-gx-accent-purple" />
							MCP Server Details
						</h3>
						<div class="space-y-3">
							<div class="space-y-1">
								<span class="text-[11px] text-gx-text-muted">Command</span>
								<p class="text-sm text-gx-text-primary font-mono bg-gx-bg-tertiary rounded px-2 py-1">
									{app.app_type.command}
									{#if app.app_type.args.length > 0}
										{' '}{app.app_type.args.join(' ')}
									{/if}
								</p>
							</div>
							<div class="grid grid-cols-3 gap-4">
								{#if app.app_type.port}
									<div class="space-y-1">
										<span class="text-[11px] text-gx-text-muted">Port</span>
										<p class="text-sm text-gx-text-primary font-mono">{app.app_type.port}</p>
									</div>
								{/if}
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Status</span>
									<div class="flex items-center gap-1.5">
										<span class="w-2 h-2 rounded-full {health?.healthy ? 'bg-gx-status-success' : health ? 'bg-gx-status-error' : 'bg-gx-text-muted'}"></span>
										<span class="text-sm text-gx-text-primary">
											{health?.healthy ? 'Running' : health ? 'Stopped' : 'Unknown'}
										</span>
									</div>
								</div>
								<div class="space-y-1">
									<span class="text-[11px] text-gx-text-muted">Usage</span>
									<p class="text-sm text-gx-text-primary">{app.usage_count} launches</p>
								</div>
							</div>
						</div>
					</div>

					<!-- Health Status -->
					{#if health}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
							<h3 class="text-sm font-semibold text-gx-text-primary mb-3 flex items-center gap-2">
								<Activity size={16} class="text-gx-neon" />
								Health
							</h3>
							<div class="flex items-center gap-3 p-3 rounded-gx {health.healthy ? 'bg-gx-status-success/10 border border-gx-status-success/30' : 'bg-gx-status-error/10 border border-gx-status-error/30'}">
								<span class="w-3 h-3 rounded-full {health.healthy ? 'bg-gx-status-success' : 'bg-gx-status-error'}"></span>
								<div>
									<p class="text-sm font-medium {health.healthy ? 'text-gx-status-success' : 'text-gx-status-error'}">
										{health.healthy ? 'Server is running' : 'Server is not responding'}
									</p>
									<p class="text-xs text-gx-text-muted mt-0.5">{health.message}</p>
								</div>
							</div>
						</div>
					{/if}

					<!-- Actions -->
					<div class="flex gap-2">
						<Button
							size="sm"
							onclick={handleLaunch}
							class="bg-gx-neon/20 text-gx-neon border border-gx-neon/40 hover:bg-gx-neon/30"
						>
							<Play size={14} class="mr-1.5" />
							Start Server
						</Button>
						<Button
							variant="outline"
							size="sm"
							onclick={handleRefreshHealth}
							disabled={refreshing}
							class="border-gx-border-default text-gx-text-secondary"
						>
							<RefreshCw size={14} class="{refreshing ? 'animate-spin' : ''} mr-1.5" />
							Check Health
						</Button>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
