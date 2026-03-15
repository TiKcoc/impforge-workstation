<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import {
		Unplug, RefreshCw, Plus, Trash2, Send, CheckCircle2,
		XCircle, Loader2, Globe, ExternalLink, Eye, Clock,
		ToggleLeft, ToggleRight, ChevronDown, ChevronUp,
		Linkedin, Github, Briefcase, Instagram, Facebook,
		Video, Twitter, FileText, AlertCircle, CheckSquare,
		Square, ArrowUpRight
	} from '@lucide/svelte';

	// ========================================================================
	// Types
	// ========================================================================

	interface ConnectedPlatform {
		id: string;
		name: string;
		platform_type: string;
		url: string;
		icon: string;
		enabled: boolean;
		auto_sync_profile: boolean;
		auto_post: boolean;
		last_synced: string | null;
		status: { status: string; message?: string };
		added_at: string;
	}

	interface PublishResult {
		platform_id: string;
		platform_name: string;
		success: boolean;
		script_id: string | null;
		message: string;
	}

	interface AutomationLogEntry {
		id: string;
		timestamp: string;
		platform_id: string;
		platform_name: string;
		action_type: string;
		status: string;
		details: string;
	}

	interface QueuedScript {
		id: string;
		platform_id: string;
		platform_name: string;
		action_type: string;
		steps: unknown[];
		content_preview: string;
		status: string;
		created_at: string;
		executed_at: string | null;
		error: string | null;
	}

	// ========================================================================
	// State
	// ========================================================================

	let platforms = $state<ConnectedPlatform[]>([]);
	let logEntries = $state<AutomationLogEntry[]>([]);
	let queuedScripts = $state<QueuedScript[]>([]);
	let loading = $state(true);
	let publishing = $state(false);

	// Publish panel
	let publishContent = $state('');
	let selectedPlatforms = $state<Set<string>>(new Set());

	// Add platform dialog
	let addDialogOpen = $state(false);
	let customName = $state('');
	let customUrl = $state('https://');
	let customType = $state('custom');

	// Section collapse
	let logExpanded = $state(false);
	let scriptsExpanded = $state(false);

	// Publishing results
	let publishResults = $state<PublishResult[]>([]);
	let showResults = $state(false);

	// ========================================================================
	// Derived
	// ========================================================================

	let enabledPlatforms = $derived(platforms.filter(p => p.enabled));
	let pendingScripts = $derived(queuedScripts.filter(s => s.status === 'pending'));

	// ========================================================================
	// Data loading
	// ========================================================================

	async function loadPlatforms() {
		try {
			platforms = await invoke<ConnectedPlatform[]>('autopub_get_platforms');
		} catch (e) {
			console.error('Failed to load platforms:', e);
		}
	}

	async function loadLog() {
		try {
			logEntries = await invoke<AutomationLogEntry[]>('autopub_get_log');
		} catch (e) {
			console.error('Failed to load log:', e);
		}
	}

	async function loadScripts() {
		try {
			queuedScripts = await invoke<QueuedScript[]>('autopub_get_scripts');
		} catch (e) {
			console.error('Failed to load scripts:', e);
		}
	}

	async function loadAll() {
		loading = true;
		await Promise.all([loadPlatforms(), loadLog(), loadScripts()]);
		loading = false;
	}

	onMount(() => {
		loadAll();
	});

	// ========================================================================
	// Actions
	// ========================================================================

	async function toggleField(id: string, field: string, value: boolean) {
		try {
			await invoke('autopub_toggle_platform', { id, field, value });
			await loadPlatforms();
		} catch (e) {
			console.error('Toggle failed:', e);
		}
	}

	async function removePlatform(id: string) {
		try {
			await invoke('autopub_remove_platform', { id });
			await loadPlatforms();
			await loadLog();
		} catch (e) {
			console.error('Remove failed:', e);
		}
	}

	async function addCustomPlatform() {
		if (!customName.trim()) return;
		try {
			await invoke('autopub_add_platform', {
				name: customName,
				url: customUrl,
				platformType: customType,
			});
			customName = '';
			customUrl = 'https://';
			customType = 'custom';
			addDialogOpen = false;
			await loadPlatforms();
			await loadLog();
		} catch (e) {
			console.error('Add platform failed:', e);
		}
	}

	function togglePlatformSelection(id: string) {
		const next = new Set(selectedPlatforms);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		selectedPlatforms = next;
	}

	async function publishToSelected() {
		if (!publishContent.trim() || selectedPlatforms.size === 0) return;
		publishing = true;
		try {
			publishResults = await invoke<PublishResult[]>('autopub_publish', {
				content: publishContent,
				platformIds: Array.from(selectedPlatforms),
			});
			showResults = true;
			publishContent = '';
			selectedPlatforms = new Set();
			await loadLog();
			await loadScripts();
		} catch (e) {
			console.error('Publish failed:', e);
		} finally {
			publishing = false;
		}
	}

	async function syncProfile(platformIds: string[]) {
		try {
			await invoke('autopub_sync_profile', {
				platformIds,
				profileData: { bio: 'Profile synced via ImpForge Auto-Publisher' },
			});
			await loadPlatforms();
			await loadLog();
			await loadScripts();
		} catch (e) {
			console.error('Sync failed:', e);
		}
	}

	async function updateScript(scriptId: string, newStatus: string) {
		try {
			await invoke('autopub_update_script', { scriptId, newStatus });
			await loadScripts();
			await loadLog();
		} catch (e) {
			console.error('Script update failed:', e);
		}
	}

	// ========================================================================
	// Helpers
	// ========================================================================

	function getPlatformIcon(icon: string) {
		switch (icon) {
			case 'linkedin': return Linkedin;
			case 'github': return Github;
			case 'twitter': return Twitter;
			case 'briefcase': return Briefcase;
			case 'instagram': return Instagram;
			case 'facebook': return Facebook;
			case 'video': return Video;
			default: return Globe;
		}
	}

	function statusColor(status: { status: string }): string {
		switch (status.status) {
			case 'Connected': return 'text-gx-status-success';
			case 'Syncing': return 'text-gx-status-warning';
			case 'Error': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusLabel(status: { status: string; message?: string }): string {
		if (status.status === 'Error' && status.message) return `Error: ${status.message}`;
		return status.status;
	}

	function formatDate(iso: string | null): string {
		if (!iso) return 'Never';
		try {
			return new Date(iso).toLocaleString();
		} catch {
			return iso;
		}
	}

	function typeLabel(t: string): string {
		switch (t) {
			case 'social_media': return 'Social';
			case 'freelancer': return 'Freelancer';
			case 'professional': return 'Professional';
			case 'job_board': return 'Job Board';
			default: return 'Custom';
		}
	}
</script>

<svelte:head>
	<title>Platforms - ImpForge</title>
</svelte:head>

<div class="h-full overflow-y-auto p-6 space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<div class="p-2 rounded-gx bg-gx-accent-blue/10 border border-gx-accent-blue/20">
				<Unplug size={24} class="text-gx-accent-blue" />
			</div>
			<div>
				<h1 class="text-xl font-bold text-gx-text-primary">Connected Platforms</h1>
				<p class="text-sm text-gx-text-muted">
					Auto-publish, sync profiles, and automate actions across your accounts
				</p>
			</div>
		</div>
		<div class="flex items-center gap-2">
			{#if pendingScripts.length > 0}
				<Badge variant="outline" class="border-gx-status-warning text-gx-status-warning">
					{pendingScripts.length} pending
				</Badge>
			{/if}
			<button
				onclick={loadAll}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
			>
				<RefreshCw size={12} class={loading ? 'animate-spin' : ''} />
				Refresh
			</button>
			<button
				onclick={() => addDialogOpen = true}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-colors"
			>
				<Plus size={12} />
				Add Platform
			</button>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<Loader2 size={32} class="animate-spin text-gx-neon" />
		</div>
	{:else}
		<!-- Platform Grid -->
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
			{#each platforms as platform (platform.id)}
				{@const IconComponent = getPlatformIcon(platform.icon)}
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 flex flex-col gap-3 hover:border-gx-border-hover transition-colors {platform.enabled ? 'border-l-2 border-l-gx-neon' : ''}">
					<!-- Platform Header -->
					<div class="flex items-center gap-3">
						<div class="p-1.5 rounded-gx {platform.enabled ? 'bg-gx-neon/10' : 'bg-gx-bg-tertiary'}">
							<IconComponent size={18} class={platform.enabled ? 'text-gx-neon' : 'text-gx-text-muted'} />
						</div>
						<div class="flex-1 min-w-0">
							<h3 class="text-sm font-medium text-gx-text-primary truncate">{platform.name}</h3>
							<div class="flex items-center gap-1.5">
								<span class="w-1.5 h-1.5 rounded-full {statusColor(platform.status)}
									{platform.status.status === 'Connected' ? 'bg-gx-status-success' : platform.status.status === 'Error' ? 'bg-gx-status-error' : 'bg-gx-text-muted'}"></span>
								<span class="text-[10px] {statusColor(platform.status)}">{statusLabel(platform.status)}</span>
							</div>
						</div>
						<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-border-default text-gx-text-muted shrink-0">
							{typeLabel(platform.platform_type)}
						</Badge>
					</div>

					<!-- Toggle Switches -->
					<div class="space-y-1.5">
						<button
							onclick={() => toggleField(platform.id, 'enabled', !platform.enabled)}
							class="flex items-center justify-between w-full text-[11px] py-1 group"
						>
							<span class="text-gx-text-muted group-hover:text-gx-text-secondary transition-colors">Enabled</span>
							{#if platform.enabled}
								<ToggleRight size={18} class="text-gx-neon" />
							{:else}
								<ToggleLeft size={18} class="text-gx-text-muted" />
							{/if}
						</button>
						<button
							onclick={() => toggleField(platform.id, 'auto_sync_profile', !platform.auto_sync_profile)}
							class="flex items-center justify-between w-full text-[11px] py-1 group"
						>
							<span class="text-gx-text-muted group-hover:text-gx-text-secondary transition-colors">Auto-Sync Profile</span>
							{#if platform.auto_sync_profile}
								<ToggleRight size={18} class="text-gx-neon" />
							{:else}
								<ToggleLeft size={18} class="text-gx-text-muted" />
							{/if}
						</button>
						<button
							onclick={() => toggleField(platform.id, 'auto_post', !platform.auto_post)}
							class="flex items-center justify-between w-full text-[11px] py-1 group"
						>
							<span class="text-gx-text-muted group-hover:text-gx-text-secondary transition-colors">Auto-Post</span>
							{#if platform.auto_post}
								<ToggleRight size={18} class="text-gx-neon" />
							{:else}
								<ToggleLeft size={18} class="text-gx-text-muted" />
							{/if}
						</button>
					</div>

					<!-- Footer -->
					<div class="flex items-center justify-between pt-1 border-t border-gx-border-default">
						<span class="text-[10px] text-gx-text-muted flex items-center gap-1">
							<Clock size={10} />
							{formatDate(platform.last_synced)}
						</span>
						<div class="flex items-center gap-1">
							<button
								onclick={() => syncProfile([platform.id])}
								disabled={!platform.enabled}
								class="p-1 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
								title="Sync Now"
							>
								<RefreshCw size={12} />
							</button>
							<a
								href={platform.url}
								target="_blank"
								rel="noopener noreferrer"
								class="p-1 rounded text-gx-text-muted hover:text-gx-accent-blue hover:bg-gx-bg-hover transition-colors"
								title="Open in browser"
							>
								<ExternalLink size={12} />
							</a>
							{#if platform.id.startsWith('custom-')}
								<button
									onclick={() => removePlatform(platform.id)}
									class="p-1 rounded text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-colors"
									title="Remove platform"
								>
									<Trash2 size={12} />
								</button>
							{/if}
						</div>
					</div>
				</div>
			{/each}
		</div>

		<Separator class="bg-gx-border-default" />

		<!-- Publish Panel -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-2 mb-4">
				<Send size={16} class="text-gx-accent-magenta" />
				<h2 class="text-sm font-semibold text-gx-text-primary">Publish to Platforms</h2>
			</div>

			<!-- Content area -->
			<textarea
				bind:value={publishContent}
				placeholder="Write your content here... It will be adapted for each selected platform."
				class="w-full h-32 px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
			></textarea>

			<!-- Platform selection -->
			<div class="mt-3 flex flex-wrap gap-2">
				{#each enabledPlatforms as platform (platform.id)}
					{@const IconComponent = getPlatformIcon(platform.icon)}
					<button
						onclick={() => togglePlatformSelection(platform.id)}
						class="flex items-center gap-1.5 px-2.5 py-1.5 text-xs rounded-gx border transition-colors
							{selectedPlatforms.has(platform.id)
								? 'bg-gx-neon/10 border-gx-neon/50 text-gx-neon'
								: 'border-gx-border-default text-gx-text-muted hover:border-gx-border-hover hover:text-gx-text-secondary'}"
					>
						{#if selectedPlatforms.has(platform.id)}
							<CheckSquare size={12} />
						{:else}
							<Square size={12} />
						{/if}
						<IconComponent size={12} />
						{platform.name}
					</button>
				{/each}
				{#if enabledPlatforms.length === 0}
					<p class="text-xs text-gx-text-muted italic">Enable at least one platform above to publish.</p>
				{/if}
			</div>

			<!-- Publish button -->
			<div class="mt-4 flex items-center justify-between">
				<p class="text-[10px] text-gx-text-muted">
					Scripts are queued for review before execution. Nothing is posted without your approval.
				</p>
				<button
					onclick={publishToSelected}
					disabled={publishing || !publishContent.trim() || selectedPlatforms.size === 0}
					class="flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
				>
					{#if publishing}
						<Loader2 size={12} class="animate-spin" />
						Publishing...
					{:else}
						<ArrowUpRight size={12} />
						Publish to {selectedPlatforms.size} Platform{selectedPlatforms.size !== 1 ? 's' : ''}
					{/if}
				</button>
			</div>
		</div>

		<!-- Publish Results -->
		{#if showResults && publishResults.length > 0}
			<div class="rounded-gx border border-gx-neon/30 bg-gx-neon/5 p-4 space-y-2">
				<div class="flex items-center justify-between">
					<h3 class="text-sm font-medium text-gx-text-primary flex items-center gap-2">
						<CheckCircle2 size={14} class="text-gx-neon" />
						Publish Results
					</h3>
					<button
						onclick={() => showResults = false}
						class="text-gx-text-muted hover:text-gx-text-secondary text-xs"
					>
						Dismiss
					</button>
				</div>
				{#each publishResults as result (result.platform_id)}
					<div class="flex items-center gap-2 text-xs py-1">
						{#if result.success}
							<CheckCircle2 size={12} class="text-gx-status-success" />
						{:else}
							<XCircle size={12} class="text-gx-status-error" />
						{/if}
						<span class="font-medium text-gx-text-secondary">{result.platform_name}</span>
						<span class="text-gx-text-muted">{result.message}</span>
					</div>
				{/each}
			</div>
		{/if}

		<!-- Queued Scripts (Pending Review) -->
		{#if queuedScripts.length > 0}
			<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary">
				<button
					onclick={() => scriptsExpanded = !scriptsExpanded}
					class="flex items-center justify-between w-full p-4 text-left hover:bg-gx-bg-hover transition-colors"
				>
					<div class="flex items-center gap-2">
						<Eye size={16} class="text-gx-accent-blue" />
						<h2 class="text-sm font-semibold text-gx-text-primary">Automation Scripts</h2>
						<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-border-default text-gx-text-muted">
							{queuedScripts.length}
						</Badge>
						{#if pendingScripts.length > 0}
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-status-warning text-gx-status-warning">
								{pendingScripts.length} awaiting review
							</Badge>
						{/if}
					</div>
					{#if scriptsExpanded}
						<ChevronUp size={14} class="text-gx-text-muted" />
					{:else}
						<ChevronDown size={14} class="text-gx-text-muted" />
					{/if}
				</button>

				{#if scriptsExpanded}
					<div class="border-t border-gx-border-default">
						<div class="overflow-x-auto">
							<table class="w-full text-xs">
								<thead>
									<tr class="bg-gx-bg-tertiary text-gx-text-muted">
										<th class="text-left px-3 py-2 font-medium">Platform</th>
										<th class="text-left px-3 py-2 font-medium">Action</th>
										<th class="text-left px-3 py-2 font-medium">Preview</th>
										<th class="text-left px-3 py-2 font-medium">Steps</th>
										<th class="text-left px-3 py-2 font-medium">Status</th>
										<th class="text-left px-3 py-2 font-medium">Created</th>
										<th class="text-right px-3 py-2 font-medium">Actions</th>
									</tr>
								</thead>
								<tbody>
									{#each queuedScripts as script (script.id)}
										<tr class="border-t border-gx-border-default hover:bg-gx-bg-hover transition-colors">
											<td class="px-3 py-2 text-gx-text-secondary font-medium">{script.platform_name}</td>
											<td class="px-3 py-2 text-gx-text-muted capitalize">{script.action_type.replace('_', ' ')}</td>
											<td class="px-3 py-2 text-gx-text-muted max-w-[200px] truncate">{script.content_preview}</td>
											<td class="px-3 py-2 text-gx-text-muted">{script.steps.length}</td>
											<td class="px-3 py-2">
												<Badge variant="outline" class="text-[9px] px-1 py-0 h-4
													{script.status === 'pending' ? 'border-gx-status-warning text-gx-status-warning' :
													 script.status === 'completed' ? 'border-gx-status-success text-gx-status-success' :
													 script.status === 'failed' ? 'border-gx-status-error text-gx-status-error' :
													 script.status === 'cancelled' ? 'border-gx-text-muted text-gx-text-muted' :
													 'border-gx-accent-blue text-gx-accent-blue'}">
													{script.status}
												</Badge>
											</td>
											<td class="px-3 py-2 text-gx-text-muted">{formatDate(script.created_at)}</td>
											<td class="px-3 py-2 text-right">
												{#if script.status === 'pending'}
													<div class="flex items-center gap-1 justify-end">
														<button
															onclick={() => updateScript(script.id, 'approved')}
															class="p-1 rounded text-gx-status-success hover:bg-gx-status-success/10 transition-colors"
															title="Approve"
														>
															<CheckCircle2 size={14} />
														</button>
														<button
															onclick={() => updateScript(script.id, 'cancelled')}
															class="p-1 rounded text-gx-status-error hover:bg-gx-status-error/10 transition-colors"
															title="Cancel"
														>
															<XCircle size={14} />
														</button>
													</div>
												{/if}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					</div>
				{/if}
			</div>
		{/if}

		<!-- Automation Log -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary">
			<button
				onclick={() => logExpanded = !logExpanded}
				class="flex items-center justify-between w-full p-4 text-left hover:bg-gx-bg-hover transition-colors"
			>
				<div class="flex items-center gap-2">
					<FileText size={16} class="text-gx-text-muted" />
					<h2 class="text-sm font-semibold text-gx-text-primary">Automation Log</h2>
					<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-border-default text-gx-text-muted">
						{logEntries.length}
					</Badge>
				</div>
				{#if logExpanded}
					<ChevronUp size={14} class="text-gx-text-muted" />
				{:else}
					<ChevronDown size={14} class="text-gx-text-muted" />
				{/if}
			</button>

			{#if logExpanded}
				<div class="border-t border-gx-border-default">
					{#if logEntries.length === 0}
						<p class="px-4 py-6 text-center text-xs text-gx-text-muted">No automation actions yet.</p>
					{:else}
						<div class="overflow-x-auto">
							<table class="w-full text-xs">
								<thead>
									<tr class="bg-gx-bg-tertiary text-gx-text-muted">
										<th class="text-left px-3 py-2 font-medium">Timestamp</th>
										<th class="text-left px-3 py-2 font-medium">Platform</th>
										<th class="text-left px-3 py-2 font-medium">Action</th>
										<th class="text-left px-3 py-2 font-medium">Status</th>
										<th class="text-left px-3 py-2 font-medium">Details</th>
									</tr>
								</thead>
								<tbody>
									{#each logEntries.slice(0, 50) as entry (entry.id)}
										<tr class="border-t border-gx-border-default hover:bg-gx-bg-hover transition-colors">
											<td class="px-3 py-1.5 text-gx-text-muted whitespace-nowrap">{formatDate(entry.timestamp)}</td>
											<td class="px-3 py-1.5 text-gx-text-secondary">{entry.platform_name}</td>
											<td class="px-3 py-1.5 text-gx-text-muted">{entry.action_type}</td>
											<td class="px-3 py-1.5">
												{#if entry.status === 'success' || entry.status === 'completed'}
													<span class="text-gx-status-success flex items-center gap-1">
														<CheckCircle2 size={10} />
														{entry.status}
													</span>
												{:else if entry.status === 'queued'}
													<span class="text-gx-status-warning flex items-center gap-1">
														<Clock size={10} />
														{entry.status}
													</span>
												{:else if entry.status === 'failed'}
													<span class="text-gx-status-error flex items-center gap-1">
														<AlertCircle size={10} />
														{entry.status}
													</span>
												{:else}
													<span class="text-gx-text-muted">{entry.status}</span>
												{/if}
											</td>
											<td class="px-3 py-1.5 text-gx-text-muted max-w-[300px] truncate">{entry.details}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
</div>

<!-- Add Platform Dialog -->
<Dialog.Root bind:open={addDialogOpen}>
	<Dialog.Content class="bg-gx-bg-elevated border-gx-border-default max-w-md">
		<Dialog.Header>
			<Dialog.Title class="text-gx-text-primary">Add Custom Platform</Dialog.Title>
			<Dialog.Description class="text-gx-text-muted text-sm">
				Connect a new platform for auto-publishing via Browser Agent (CDP).
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-4">
			<!-- Pre-defined platforms quick-add -->
			<div>
				<label class="text-xs font-medium text-gx-text-secondary mb-2 block">Quick Add</label>
				<div class="flex flex-wrap gap-1.5">
					{#each ['Indeed', 'StepStone', 'Reddit', 'Mastodon', 'StackOverflow', 'Dribbble', 'Behance', 'Medium'] as preset}
						<button
							onclick={() => { customName = preset; customUrl = `https://www.${preset.toLowerCase()}.com`; }}
							class="px-2 py-1 text-[11px] rounded-gx border border-gx-border-default text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/30 transition-colors"
						>
							{preset}
						</button>
					{/each}
				</div>
			</div>

			<Separator class="bg-gx-border-default" />

			<div>
				<label for="platform-name" class="text-xs font-medium text-gx-text-secondary mb-1 block">Platform Name</label>
				<input
					id="platform-name"
					bind:value={customName}
					placeholder="e.g. My Portfolio"
					class="w-full px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
				/>
			</div>
			<div>
				<label for="platform-url" class="text-xs font-medium text-gx-text-secondary mb-1 block">URL</label>
				<input
					id="platform-url"
					bind:value={customUrl}
					placeholder="https://example.com"
					class="w-full px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
				/>
			</div>
			<div>
				<label for="platform-type" class="text-xs font-medium text-gx-text-secondary mb-1 block">Type</label>
				<select
					id="platform-type"
					bind:value={customType}
					class="w-full px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-primary focus:outline-none focus:border-gx-neon"
				>
					<option value="social_media">Social Media</option>
					<option value="freelancer">Freelancer</option>
					<option value="professional">Professional</option>
					<option value="job_board">Job Board</option>
					<option value="custom">Custom</option>
				</select>
			</div>
		</div>

		<Dialog.Footer>
			<button
				onclick={() => addDialogOpen = false}
				class="px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
			>
				Cancel
			</button>
			<button
				onclick={addCustomPlatform}
				disabled={!customName.trim()}
				class="px-3 py-1.5 text-xs rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
			>
				Add Platform
			</button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
