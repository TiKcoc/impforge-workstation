<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		Container,
		Play,
		Square,
		RotateCw,
		Trash2,
		ScrollText,
		Download,
		Server,
		RefreshCw,
		ServerOff,
		X,
		HardDrive,
		AlertTriangle,
		Clock,
		Layers
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-docker';
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
	let dockerHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let dockerHeaderStyle = $derived(hasEngineStyle && dockerHeaderComponent ? componentToCSS(dockerHeaderComponent) : '');

	interface ContainerInfo {
		id: string;
		name: string;
		image: string;
		status: string;
		state: string;
		ports: string[];
	}

	let containers = $state<ContainerInfo[]>([]);
	let dockerInfo = $state<Record<string, string>>({});
	let loading = $state(true);
	let dockerAvailable = $state(true);
	let logContent = $state('');
	let logContainer = $state('');
	let showLogs = $state(false);
	let actionLoading = $state<string | null>(null);
	let pullImageName = $state('');
	let pulling = $state(false);
	let pullError = $state<string | null>(null);
	let confirmRemove = $state<string | null>(null);
	let errorMessage = $state<string | null>(null);
	let logRefreshInterval = $state<ReturnType<typeof setInterval> | null>(null);

	let runningCount = $derived(containers.filter((c) => c.state === 'running').length);
	let stoppedCount = $derived(containers.filter((c) => c.state !== 'running').length);

	async function fetchData() {
		loading = true;
		errorMessage = null;
		try {
			const [info, ctrs] = await Promise.all([
				invoke<Record<string, string>>('docker_info'),
				invoke<ContainerInfo[]>('list_containers')
			]);
			dockerInfo = info;
			containers = ctrs;
			dockerAvailable = true;
		} catch {
			dockerAvailable = false;
			containers = [];
			dockerInfo = {};
		} finally {
			loading = false;
		}
	}

	async function handleAction(containerId: string, containerName: string, action: string) {
		if (action === 'Remove' && confirmRemove !== containerId) {
			confirmRemove = containerId;
			return;
		}
		confirmRemove = null;
		actionLoading = containerId + action;
		errorMessage = null;
		try {
			if (action === 'Logs') {
				const logs = await invoke<string>('container_action', { containerId, action });
				logContent = logs;
				logContainer = containerName;
				showLogs = true;
				startLogAutoRefresh(containerId, containerName);
			} else {
				await invoke<string>('container_action', { containerId, action });
				await fetchData();
			}
		} catch (e) {
			errorMessage = `Action "${action}" on "${containerName}" failed: ${e}`;
		} finally {
			actionLoading = null;
		}
	}

	function startLogAutoRefresh(containerId: string, containerName: string) {
		stopLogAutoRefresh();
		logRefreshInterval = setInterval(async () => {
			try {
				const logs = await invoke<string>('container_action', { containerId, action: 'Logs' });
				logContent = logs;
			} catch {
				// silently ignore refresh errors
			}
		}, 5000);
	}

	function stopLogAutoRefresh() {
		if (logRefreshInterval) {
			clearInterval(logRefreshInterval);
			logRefreshInterval = null;
		}
	}

	function closeLogs() {
		showLogs = false;
		logContent = '';
		logContainer = '';
		stopLogAutoRefresh();
	}

	async function pullImage() {
		if (!pullImageName.trim()) return;
		pulling = true;
		pullError = null;
		try {
			await invoke<string>('container_action', {
				containerId: pullImageName.trim(),
				action: 'Pull'
			});
			pullImageName = '';
			await fetchData();
		} catch (e) {
			pullError = `Failed to pull "${pullImageName}": ${e}`;
		} finally {
			pulling = false;
		}
	}

	function stateBadgeClass(state: string): string {
		switch (state) {
			case 'running':
				return 'border-green-500/50 text-green-400 bg-green-500/10';
			case 'exited':
				return 'border-red-500/50 text-red-400 bg-red-500/10';
			case 'paused':
				return 'border-yellow-500/50 text-yellow-400 bg-yellow-500/10';
			case 'created':
				return 'border-blue-500/50 text-blue-400 bg-blue-500/10';
			default:
				return 'border-gx-border-default text-gx-text-muted';
		}
	}

	function formatCreated(status: string): string {
		// Docker status often includes uptime info like "Up 3 hours" or "Exited (0) 2 days ago"
		return status;
	}

	onMount(() => {
		fetchData();
		return () => stopLogAutoRefresh();
	});
</script>

<main class="flex flex-col h-screen {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<header
		class="h-14 border-b border-gx-border-default {hasEngineStyle && dockerHeaderComponent ? '' : 'bg-gx-bg-secondary'} flex items-center justify-between px-4 shrink-0" style={dockerHeaderStyle}
	>
		<div class="flex items-center gap-3">
			<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">&larr;</a>
			<Container class="w-5 h-5 text-gx-neon" />
			<h1 class="text-lg font-semibold text-gx-text-primary">Docker Management</h1>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={fetchData} disabled={loading}>
				<RefreshCw class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
		</div>
	</header>

	<div class="flex-1 overflow-y-auto p-4 space-y-4">
		{#if loading && containers.length === 0}
			<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
				{#each Array(4) as _}
					<div class="h-24 rounded-gx bg-gx-bg-tertiary animate-pulse"></div>
				{/each}
			</div>
			<div class="space-y-2">
				{#each Array(4) as _}
					<div class="h-16 rounded-gx bg-gx-bg-tertiary animate-pulse"></div>
				{/each}
			</div>
		{:else if !dockerAvailable}
			<div class="flex flex-col items-center justify-center h-96 gap-4">
				<ServerOff class="w-16 h-16 text-gx-text-muted" />
				<h2 class="text-xl font-semibold text-gx-text-primary">Docker is not available</h2>
				<p class="text-gx-text-muted text-center max-w-md">
					Make sure the Docker daemon is running. You can start it with
					<code class="bg-gx-bg-tertiary px-2 py-0.5 rounded text-gx-neon"
						>sudo systemctl start docker</code
					>
				</p>
				<Button variant="outline" onclick={fetchData}>
					<RefreshCw class="w-4 h-4" /> Try Again
				</Button>
			</div>
		{:else}
			<!-- Error Banner -->
			{#if errorMessage}
				<div
					class="flex items-center gap-3 p-3 rounded-gx bg-red-500/10 border border-red-500/30 text-red-400 text-sm"
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

			<!-- System Info Cards -->
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
				<Card class="bg-gx-bg-secondary border-gx-border-default">
					<CardHeader class="pb-2">
						<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
							<Server class="w-3.5 h-3.5" />
							Docker Version
						</CardTitle>
					</CardHeader>
					<CardContent>
						<p class="text-2xl font-bold text-gx-text-primary">
							{dockerInfo.server_version ?? '--'}
						</p>
						<p class="text-xs text-gx-text-muted mt-1">{dockerInfo.os ?? ''}</p>
					</CardContent>
				</Card>
				<Card class="bg-gx-bg-secondary border-gx-border-default">
					<CardHeader class="pb-2">
						<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
							<Container class="w-3.5 h-3.5" />
							Containers
						</CardTitle>
					</CardHeader>
					<CardContent>
						<p class="text-2xl font-bold text-gx-text-primary">
							{dockerInfo.containers ?? '0'}
						</p>
						<p class="text-xs mt-1">
							<span class="text-green-400">{runningCount} running</span>
							<span class="text-gx-text-muted mx-1">/</span>
							<span class="text-red-400">{stoppedCount} stopped</span>
						</p>
					</CardContent>
				</Card>
				<Card class="bg-gx-bg-secondary border-gx-border-default">
					<CardHeader class="pb-2">
						<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
							<Layers class="w-3.5 h-3.5" />
							Images
						</CardTitle>
					</CardHeader>
					<CardContent>
						<p class="text-2xl font-bold text-gx-text-primary">
							{dockerInfo.images ?? '0'}
						</p>
						<p class="text-xs text-gx-text-muted mt-1">{dockerInfo.name ?? ''}</p>
					</CardContent>
				</Card>
				<Card class="bg-gx-bg-secondary border-gx-border-default">
					<CardHeader class="pb-2">
						<CardTitle class="text-xs text-gx-text-muted flex items-center gap-1.5">
							<Download class="w-3.5 h-3.5" />
							Pull Image
						</CardTitle>
					</CardHeader>
					<CardContent>
						<div class="flex gap-2">
							<Input
								bind:value={pullImageName}
								placeholder="e.g. nginx:latest"
								class="h-8 text-xs bg-gx-bg-tertiary border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted"
							/>
							<Button
								variant="outline"
								size="sm"
								onclick={pullImage}
								disabled={pulling || !pullImageName.trim()}
								class="shrink-0 h-8"
							>
								{#if pulling}
									<RefreshCw class="w-3.5 h-3.5 animate-spin" />
								{:else}
									<Download class="w-3.5 h-3.5" />
								{/if}
							</Button>
						</div>
						{#if pullError}
							<p class="text-[10px] text-red-400 mt-1 truncate" title={pullError}>
								{pullError}
							</p>
						{/if}
					</CardContent>
				</Card>
			</div>

			<!-- Container Table -->
			{#if containers.length === 0}
				<div class="flex flex-col items-center justify-center h-48 gap-2">
					<HardDrive class="w-10 h-10 text-gx-text-muted" />
					<p class="text-gx-text-muted">No containers found</p>
				</div>
			{:else}
				<Card class="bg-gx-bg-secondary border-gx-border-default overflow-hidden">
					<CardContent class="p-0">
						<div class="overflow-x-auto">
							<table class="w-full text-sm">
								<thead>
									<tr class="border-b border-gx-border-default text-gx-text-muted">
										<th class="text-left p-3 font-medium">Name</th>
										<th class="text-left p-3 font-medium">Image</th>
										<th class="text-left p-3 font-medium">Status</th>
										<th class="text-left p-3 font-medium">Ports</th>
										<th class="text-left p-3 font-medium">
											<Clock class="w-3.5 h-3.5 inline-block" />
											Info
										</th>
										<th class="text-right p-3 font-medium">Actions</th>
									</tr>
								</thead>
								<tbody>
									{#each containers as container (container.id)}
										<tr
											class="border-b border-gx-border-default/50 hover:bg-gx-bg-tertiary/50 transition-colors"
										>
											<td class="p-3 text-gx-text-primary font-mono text-xs">
												{container.name}
											</td>
											<td
												class="p-3 text-gx-text-secondary text-xs truncate max-w-48"
												title={container.image}
											>
												{container.image}
											</td>
											<td class="p-3">
												<Badge
													variant="outline"
													class={stateBadgeClass(container.state)}
												>
													{container.state}
												</Badge>
											</td>
											<td class="p-3 text-gx-text-muted text-xs font-mono">
												{container.ports.length > 0
													? container.ports.join(', ')
													: '--'}
											</td>
											<td class="p-3 text-gx-text-muted text-xs">
												{formatCreated(container.status)}
											</td>
											<td class="p-3 text-right">
												<div class="flex items-center justify-end gap-1">
													{#if container.state !== 'running'}
														<button
															class="p-1.5 rounded hover:bg-green-500/20 text-gx-text-muted hover:text-green-400 transition-colors disabled:opacity-30"
															onclick={() =>
																handleAction(
																	container.id,
																	container.name,
																	'Start'
																)}
															disabled={actionLoading ===
																container.id + 'Start'}
															title="Start"
														>
															<Play class="w-3.5 h-3.5" />
														</button>
													{/if}
													{#if container.state === 'running'}
														<button
															class="p-1.5 rounded hover:bg-yellow-500/20 text-gx-text-muted hover:text-yellow-400 transition-colors disabled:opacity-30"
															onclick={() =>
																handleAction(
																	container.id,
																	container.name,
																	'Stop'
																)}
															disabled={actionLoading ===
																container.id + 'Stop'}
															title="Stop"
														>
															<Square class="w-3.5 h-3.5" />
														</button>
													{/if}
													<button
														class="p-1.5 rounded hover:bg-blue-500/20 text-gx-text-muted hover:text-blue-400 transition-colors disabled:opacity-30"
														onclick={() =>
															handleAction(
																container.id,
																container.name,
																'Restart'
															)}
														disabled={actionLoading ===
															container.id + 'Restart'}
														title="Restart"
													>
														<RotateCw class="w-3.5 h-3.5" />
													</button>
													<button
														class="p-1.5 rounded hover:bg-gx-neon/20 text-gx-text-muted hover:text-gx-neon transition-colors disabled:opacity-30"
														onclick={() =>
															handleAction(
																container.id,
																container.name,
																'Logs'
															)}
														disabled={actionLoading ===
															container.id + 'Logs'}
														title="View Logs"
													>
														<ScrollText class="w-3.5 h-3.5" />
													</button>
													{#if confirmRemove === container.id}
														<button
															class="px-2 py-1 rounded text-[10px] font-medium bg-red-500/20 text-red-400 border border-red-500/40 hover:bg-red-500/30 transition-colors"
															onclick={() =>
																handleAction(
																	container.id,
																	container.name,
																	'Remove'
																)}
														>
															Confirm
														</button>
														<button
															class="p-1.5 rounded hover:bg-gx-bg-tertiary text-gx-text-muted transition-colors"
															onclick={() =>
																(confirmRemove = null)}
															title="Cancel"
														>
															<X class="w-3.5 h-3.5" />
														</button>
													{:else}
														<button
															class="p-1.5 rounded hover:bg-red-500/20 text-gx-text-muted hover:text-red-400 transition-colors disabled:opacity-30"
															onclick={() =>
																handleAction(
																	container.id,
																	container.name,
																	'Remove'
																)}
															disabled={actionLoading ===
																container.id + 'Remove'}
															title="Remove"
														>
															<Trash2 class="w-3.5 h-3.5" />
														</button>
													{/if}
												</div>
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					</CardContent>
				</Card>
			{/if}
		{/if}
	</div>

	<!-- Logs Panel -->
	{#if showLogs}
		<div
			class="shrink-0 h-72 border-t border-gx-border-default bg-gx-bg-secondary flex flex-col"
		>
			<div
				class="flex items-center justify-between px-4 py-2 border-b border-gx-border-default"
			>
				<span class="text-sm text-gx-text-primary font-medium flex items-center gap-2">
					<ScrollText class="w-4 h-4 text-gx-neon" />
					Logs: <span class="font-mono text-gx-neon">{logContainer}</span>
				</span>
				<div class="flex items-center gap-2">
					<Badge
						variant="outline"
						class="text-[10px] border-gx-neon/30 text-gx-neon px-1.5 py-0"
					>
						Auto-refresh 5s
					</Badge>
					<button
						class="p-1 rounded hover:bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-text-primary transition-colors"
						onclick={closeLogs}
					>
						<X class="w-4 h-4" />
					</button>
				</div>
			</div>
			<pre
				class="flex-1 overflow-auto p-3 text-xs font-mono text-gx-text-secondary whitespace-pre-wrap leading-relaxed">{logContent ||
					'No logs available.'}</pre>
		</div>
	{/if}
</main>
