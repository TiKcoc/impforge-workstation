<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Share2, Send, Calendar, BarChart3, Clock, Eye, Plus, RefreshCw,
		Loader2, Copy, Check, X, Sparkles, FileText, Megaphone,
		Github, Linkedin, Twitter, MessageCircle, Hash, Globe,
		ChevronDown, AlertCircle, Trash2, CheckCircle2
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ─── BenikUI Style Engine ────────────────────────────────────
	const widgetId = 'page-social';
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

	// ─── Types ───────────────────────────────────────────────────
	interface PlatformEngagement {
		views: number;
		likes: number;
		comments: number;
		shares: number;
	}

	interface GoldenHourInfo {
		best_days: string[];
		best_hour_utc: number;
	}

	interface PlatformStatus {
		id: string;
		name: string;
		connected: boolean;
		last_post_date: string | null;
		post_count: number;
		engagement: PlatformEngagement;
		golden_hour: GoldenHourInfo | null;
	}

	interface QueuedPost {
		id: string;
		platform: string;
		content_type: string;
		title: string | null;
		body: string;
		tags: string[];
		status: string;
		scheduled_at: string | null;
		created_at: string;
		published_at: string | null;
	}

	interface ContentTemplate {
		id: string;
		name: string;
		description: string;
		content_type: string;
		platforms: string[];
		template_body: string;
		hook_archetype: string;
	}

	// ─── State ───────────────────────────────────────────────────
	let platforms = $state<PlatformStatus[]>([]);
	let queue = $state<QueuedPost[]>([]);
	let templates = $state<ContentTemplate[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Composer state
	let composerOpen = $state(false);
	let composeContent = $state('');
	let composeTitle = $state('');
	let composeTags = $state('');
	let composeContentType = $state('project_update');
	let selectedPlatforms = $state<Set<string>>(new Set());
	let scheduleEnabled = $state(false);
	let scheduleDate = $state('');
	let scheduleTime = $state('');
	let composing = $state(false);
	let composeError = $state<string | null>(null);

	// AI generation
	let aiGenerating = $state(false);
	let aiTopic = $state('');
	let aiStyle = $state('problem_solution');
	let aiPlatform = $state('twitter');

	// Clipboard feedback
	let copiedId = $state<string | null>(null);

	// Active tab
	type TabId = 'platforms' | 'compose' | 'queue' | 'templates' | 'analytics';
	let activeTab = $state<TabId>('platforms');

	// ─── Platform icon mapping ───────────────────────────────────
	const platformIcons: Record<string, typeof Github> = {
		github: Github,
		linkedin: Linkedin,
		twitter: Twitter,
		hackernews: Hash,
		mastodon: Globe,
		discord: MessageCircle,
	};

	const platformColors: Record<string, string> = {
		github: 'text-gx-text-primary',
		linkedin: 'text-blue-400',
		twitter: 'text-sky-400',
		hackernews: 'text-orange-400',
		mastodon: 'text-indigo-400',
		discord: 'text-violet-400',
	};

	const statusColors: Record<string, string> = {
		draft: 'bg-gx-text-muted/20 text-gx-text-muted border-gx-text-muted/30',
		queued: 'bg-gx-status-warning/15 text-gx-status-warning border-gx-status-warning/30',
		scheduled: 'bg-gx-accent-blue/15 text-gx-accent-blue border-gx-accent-blue/30',
		published: 'bg-gx-status-success/15 text-gx-status-success border-gx-status-success/30',
		failed: 'bg-gx-status-error/15 text-gx-status-error border-gx-status-error/30',
		cancelled: 'bg-gx-text-muted/10 text-gx-text-muted border-gx-text-muted/20',
	};

	const contentTypeLabels: Record<string, string> = {
		release_note: 'Release Note',
		technical_article: 'Technical Article',
		project_update: 'Project Update',
		show_hn: 'Show HN',
		thread_series: 'Thread Series',
		engagement_reply: 'Engagement Reply',
		profile_update: 'Profile Update',
	};

	const hookStyles = [
		{ id: 'problem_solution', label: 'Problem / Solution' },
		{ id: 'contrarian', label: 'Contrarian Take' },
		{ id: 'data_driven', label: 'Data-Driven' },
		{ id: 'personal_story', label: 'Personal Story' },
		{ id: 'tutorial', label: 'Tutorial / How-To' },
		{ id: 'comparison', label: 'Comparison' },
		{ id: 'prediction', label: 'Prediction' },
	];

	// ─── Golden Hour heatmap data ────────────────────────────────
	const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

	function goldenHourHeatmap(plist: PlatformStatus[]): boolean[][] {
		// 7 days x 24 hours: true if any platform has a golden hour there
		const grid: boolean[][] = Array.from({ length: 7 }, () => Array(24).fill(false));
		for (const p of plist) {
			if (!p.golden_hour) continue;
			for (const day of p.golden_hour.best_days) {
				const dayIdx = days.indexOf(day);
				if (dayIdx >= 0) {
					grid[dayIdx][p.golden_hour.best_hour_utc] = true;
				}
			}
		}
		return grid;
	}

	let heatmap = $derived(goldenHourHeatmap(platforms));

	// ─── Data Loading ────────────────────────────────────────────
	async function loadAll() {
		loading = true;
		error = null;
		try {
			const [p, q, t] = await Promise.all([
				invoke<PlatformStatus[]>('social_get_platforms'),
				invoke<QueuedPost[]>('social_get_queue'),
				invoke<ContentTemplate[]>('social_get_templates'),
			]);
			platforms = p;
			queue = q;
			templates = t;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	async function refreshQueue() {
		try {
			queue = await invoke<QueuedPost[]>('social_get_queue');
		} catch (e) {
			console.error('Failed to refresh queue:', e);
		}
	}

	// ─── Composer Actions ────────────────────────────────────────
	function togglePlatform(id: string) {
		const next = new Set(selectedPlatforms);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		selectedPlatforms = next;
	}

	async function submitPost() {
		if (!composeContent.trim()) {
			composeError = 'Post content is required';
			return;
		}
		if (selectedPlatforms.size === 0) {
			composeError = 'Select at least one platform';
			return;
		}

		composing = true;
		composeError = null;

		try {
			let schedule: string | undefined = undefined;
			if (scheduleEnabled && scheduleDate && scheduleTime) {
				const dt = new Date(`${scheduleDate}T${scheduleTime}:00`);
				schedule = dt.toISOString();
			}

			await invoke('social_compose_post', {
				content: composeContent,
				platforms: Array.from(selectedPlatforms),
				contentType: composeContentType,
				title: composeTitle || null,
				tags: composeTags ? composeTags.split(',').map((t: string) => t.trim()).filter(Boolean) : null,
				schedule: schedule || null,
			});

			// Reset composer
			composeContent = '';
			composeTitle = '';
			composeTags = '';
			selectedPlatforms = new Set();
			scheduleEnabled = false;
			scheduleDate = '';
			scheduleTime = '';
			composerOpen = false;

			// Refresh queue
			await refreshQueue();
			activeTab = 'queue';
		} catch (e) {
			composeError = e instanceof Error ? e.message : String(e);
		} finally {
			composing = false;
		}
	}

	async function generateAiContent() {
		if (!aiTopic.trim()) return;

		aiGenerating = true;
		try {
			const generated = await invoke<string>('social_ai_generate', {
				topic: aiTopic,
				platform: aiPlatform,
				style: aiStyle,
			});
			composeContent = generated;
			aiTopic = '';
		} catch (e) {
			composeError = e instanceof Error ? e.message : String(e);
		} finally {
			aiGenerating = false;
		}
	}

	async function cancelPost(postId: string) {
		try {
			await invoke('social_cancel_post', { postId });
			await refreshQueue();
		} catch (e) {
			console.error('Failed to cancel post:', e);
		}
	}

	async function markPublished(postId: string) {
		try {
			await invoke('social_mark_published', { postId });
			await refreshQueue();
		} catch (e) {
			console.error('Failed to mark published:', e);
		}
	}

	function copyToClipboard(text: string, postId: string) {
		navigator.clipboard.writeText(text).then(() => {
			copiedId = postId;
			setTimeout(() => {
				if (copiedId === postId) copiedId = null;
			}, 2000);
		});
	}

	function applyTemplate(template: ContentTemplate) {
		composeContent = template.template_body;
		composeContentType = template.content_type;
		selectedPlatforms = new Set(template.platforms);
		composerOpen = true;
		activeTab = 'compose';
	}

	function formatDate(dateStr: string): string {
		try {
			return new Date(dateStr).toLocaleDateString('en-US', {
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit',
			});
		} catch {
			return dateStr;
		}
	}

	// ─── Derived Stats ───────────────────────────────────────────
	let totalPosts = $derived(queue.length);
	let draftCount = $derived(queue.filter(p => p.status === 'draft').length);
	let queuedCount = $derived(queue.filter(p => p.status === 'queued').length);
	let scheduledCount = $derived(queue.filter(p => p.status === 'scheduled').length);
	let publishedCount = $derived(queue.filter(p => p.status === 'published').length);

	onMount(() => {
		loadAll();
	});
</script>

<div class="flex flex-col h-full overflow-hidden" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center justify-between px-6 py-4 border-b border-gx-border-default shrink-0">
		<div class="flex items-center gap-3">
			<div class="flex items-center justify-center w-9 h-9 rounded-gx bg-gx-accent-magenta/15">
				<Share2 size={18} class="text-gx-accent-magenta" />
			</div>
			<div>
				<h1 class="text-lg font-semibold text-gx-text-primary">Social Media Hub</h1>
				<p class="text-xs text-gx-text-muted">Create, schedule, and manage content across platforms</p>
			</div>
		</div>

		<div class="flex items-center gap-2">
			<!-- Stats badges -->
			<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-border-default text-gx-text-muted">
				{totalPosts} posts
			</Badge>
			{#if queuedCount > 0}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-status-warning/30 text-gx-status-warning">
					{queuedCount} queued
				</Badge>
			{/if}
			{#if scheduledCount > 0}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-accent-blue/30 text-gx-accent-blue">
					{scheduledCount} scheduled
				</Badge>
			{/if}

			<button
				onclick={() => { composerOpen = true; activeTab = 'compose'; }}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
			>
				<Plus size={14} />
				New Post
			</button>

			<button
				onclick={() => loadAll()}
				disabled={loading}
				class="flex items-center justify-center w-8 h-8 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all"
			>
				<RefreshCw size={14} class={loading ? 'animate-spin' : ''} />
			</button>
		</div>
	</div>

	<!-- Tab Navigation -->
	<div class="flex items-center gap-1 px-6 py-2 border-b border-gx-border-default shrink-0 overflow-x-auto">
		{#each [
			{ id: 'platforms' as TabId, label: 'Platforms', icon: Globe },
			{ id: 'compose' as TabId, label: 'Compose', icon: Megaphone },
			{ id: 'queue' as TabId, label: 'Queue', icon: FileText },
			{ id: 'templates' as TabId, label: 'Templates', icon: Sparkles },
			{ id: 'analytics' as TabId, label: 'Analytics', icon: BarChart3 },
		] as tab}
			<button
				onclick={() => activeTab = tab.id}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx transition-all whitespace-nowrap
					{activeTab === tab.id
						? 'bg-gx-bg-elevated text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
			>
				<tab.icon size={13} />
				{tab.label}
				{#if tab.id === 'queue' && queuedCount > 0}
					<span class="inline-flex items-center justify-center w-4 h-4 text-[9px] font-bold rounded-full bg-gx-status-warning/20 text-gx-status-warning">
						{queuedCount}
					</span>
				{/if}
			</button>
		{/each}
	</div>

	<!-- Content Area -->
	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex items-center justify-center h-64">
				<div class="flex items-center gap-3 text-gx-text-muted">
					<Loader2 size={20} class="animate-spin" />
					<span class="text-sm">Loading social media data...</span>
				</div>
			</div>
		{:else if error}
			<div class="flex items-center justify-center h-64">
				<div class="flex flex-col items-center gap-3 text-center">
					<AlertCircle size={32} class="text-gx-status-error" />
					<p class="text-sm text-gx-status-error">{error}</p>
					<button
						onclick={() => loadAll()}
						class="text-xs text-gx-neon hover:underline"
					>
						Try again
					</button>
				</div>
			</div>
		{:else}

			<!-- ═══════ PLATFORMS TAB ═══════ -->
			{#if activeTab === 'platforms'}
				<div class="p-6 space-y-6">
					<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
						{#each platforms as platform}
							{@const IconComp = platformIcons[platform.id] ?? Globe}
							{@const colorClass = platformColors[platform.id] ?? 'text-gx-text-muted'}
							<Card class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-all" style={cardStyle}>
								<CardHeader class="pb-2">
									<div class="flex items-center justify-between">
										<div class="flex items-center gap-2.5">
											<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated">
												<IconComp size={16} class={colorClass} />
											</div>
											<div>
												<CardTitle class="text-sm font-medium text-gx-text-primary">{platform.name}</CardTitle>
												<span class="text-[10px] text-gx-text-muted">
													{#if platform.connected}
														Connected
													{:else}
														Not connected
													{/if}
												</span>
											</div>
										</div>
										<div class="flex items-center gap-1.5">
											<span class="w-2 h-2 rounded-full {platform.connected ? 'bg-gx-status-success' : 'bg-gx-text-muted/40'}"></span>
										</div>
									</div>
								</CardHeader>
								<CardContent class="space-y-3">
									<!-- Metrics -->
									<div class="grid grid-cols-2 gap-2">
										<div class="rounded bg-gx-bg-primary px-2 py-1.5">
											<div class="text-[10px] text-gx-text-muted">Posts</div>
											<div class="text-sm font-semibold text-gx-text-primary">{platform.post_count}</div>
										</div>
										<div class="rounded bg-gx-bg-primary px-2 py-1.5">
											<div class="text-[10px] text-gx-text-muted">Engagement</div>
											<div class="text-sm font-semibold text-gx-text-primary">
												{platform.engagement.likes + platform.engagement.comments + platform.engagement.shares}
											</div>
										</div>
									</div>

									<!-- Golden Hour -->
									{#if platform.golden_hour}
										<div class="flex items-center gap-2 px-2 py-1.5 rounded bg-gx-status-warning/5 border border-gx-status-warning/15">
											<Clock size={12} class="text-gx-status-warning shrink-0" />
											<div class="text-[10px] text-gx-text-muted">
												<span class="text-gx-status-warning font-medium">Best time:</span>
												{platform.golden_hour.best_days.join(', ')} at {platform.golden_hour.best_hour_utc}:00 UTC
											</div>
										</div>
									{/if}

									<!-- Last post -->
									{#if platform.last_post_date}
										<div class="text-[10px] text-gx-text-muted">
											Last post: {formatDate(platform.last_post_date)}
										</div>
									{:else}
										<div class="text-[10px] text-gx-text-muted italic">No posts yet</div>
									{/if}

									<!-- Quick post button -->
									<button
										onclick={() => { selectedPlatforms = new Set([platform.id]); composerOpen = true; activeTab = 'compose'; }}
										class="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-bg-elevated text-gx-text-secondary hover:text-gx-neon hover:bg-gx-neon/10 border border-gx-border-default hover:border-gx-neon/30 transition-all"
									>
										<Send size={12} />
										Quick Post
									</button>
								</CardContent>
							</Card>
						{/each}
					</div>
				</div>

			<!-- ═══════ COMPOSE TAB ═══════ -->
			{:else if activeTab === 'compose'}
				<div class="p-6 max-w-4xl mx-auto space-y-6">
					<!-- AI Content Generator -->
					<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<Sparkles size={14} class="text-gx-accent-magenta" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">AI Content Generator</CardTitle>
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-accent-magenta/30 text-gx-accent-magenta">StoryBrand</Badge>
							</div>
						</CardHeader>
						<CardContent class="space-y-3">
							<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
								<div class="md:col-span-1">
									<label for="ai-topic" class="text-[11px] text-gx-text-muted mb-1 block">Topic</label>
									<input
										id="ai-topic"
										type="text"
										bind:value={aiTopic}
										placeholder="e.g., Announcing ImpForge v2.0"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
								</div>
								<div>
									<label for="ai-platform" class="text-[11px] text-gx-text-muted mb-1 block">Platform</label>
									<select
										id="ai-platform"
										bind:value={aiPlatform}
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
									>
										{#each platforms as p}
											<option value={p.id}>{p.name}</option>
										{/each}
									</select>
								</div>
								<div>
									<label for="ai-style" class="text-[11px] text-gx-text-muted mb-1 block">Hook Style</label>
									<select
										id="ai-style"
										bind:value={aiStyle}
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
									>
										{#each hookStyles as h}
											<option value={h.id}>{h.label}</option>
										{/each}
									</select>
								</div>
							</div>
							<button
								onclick={generateAiContent}
								disabled={aiGenerating || !aiTopic.trim()}
								class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-accent-magenta/15 text-gx-accent-magenta border border-gx-accent-magenta/30 hover:bg-gx-accent-magenta/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
							>
								{#if aiGenerating}
									<Loader2 size={13} class="animate-spin" />
									Generating...
								{:else}
									<Sparkles size={13} />
									Generate Content
								{/if}
							</button>
						</CardContent>
					</Card>

					<!-- Post Composer -->
					<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<Megaphone size={14} class="text-gx-neon" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">Compose Post</CardTitle>
							</div>
						</CardHeader>
						<CardContent class="space-y-4">
							<!-- Title (optional) -->
							<div>
								<label for="post-title" class="text-[11px] text-gx-text-muted mb-1 block">Title (optional)</label>
								<input
									id="post-title"
									type="text"
									bind:value={composeTitle}
									placeholder="Post title..."
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>

							<!-- Content -->
							<div>
								<label for="post-content" class="text-[11px] text-gx-text-muted mb-1 block">Content</label>
								<textarea
									id="post-content"
									bind:value={composeContent}
									placeholder="Write your post content here..."
									rows="8"
									class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors resize-y font-mono leading-relaxed"
								></textarea>
								<div class="flex items-center justify-between mt-1">
									<span class="text-[10px] text-gx-text-muted">
										{composeContent.length} characters
									</span>
									{#if composeContent.length > 280}
										<span class="text-[10px] text-gx-status-warning">
											Exceeds Twitter 280 char limit
										</span>
									{/if}
								</div>
							</div>

							<!-- Content Type -->
							<div>
								<label for="content-type" class="text-[11px] text-gx-text-muted mb-1 block">Content Type</label>
								<select
									id="content-type"
									bind:value={composeContentType}
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
								>
									{#each Object.entries(contentTypeLabels) as [value, label]}
										<option {value}>{label}</option>
									{/each}
								</select>
							</div>

							<!-- Platform Selection -->
							<div>
								<span class="text-[11px] text-gx-text-muted mb-2 block">Platforms</span>
								<div class="flex flex-wrap gap-2">
									{#each platforms as p}
										{@const IconComp = platformIcons[p.id] ?? Globe}
										{@const selected = selectedPlatforms.has(p.id)}
										<button
											onclick={() => togglePlatform(p.id)}
											class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border transition-all
												{selected
													? 'bg-gx-neon/15 text-gx-neon border-gx-neon/40'
													: 'bg-gx-bg-primary text-gx-text-muted border-gx-border-default hover:border-gx-text-muted/50'}"
										>
											<IconComp size={12} />
											{p.name}
											{#if selected}
												<Check size={10} />
											{/if}
										</button>
									{/each}
								</div>
							</div>

							<!-- Tags -->
							<div>
								<label for="post-tags" class="text-[11px] text-gx-text-muted mb-1 block">Tags (comma-separated)</label>
								<input
									id="post-tags"
									type="text"
									bind:value={composeTags}
									placeholder="ai, developer-tools, open-source"
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>

							<!-- Schedule -->
							<div class="space-y-2">
								<label class="flex items-center gap-2 cursor-pointer">
									<input
										type="checkbox"
										bind:checked={scheduleEnabled}
										class="w-3.5 h-3.5 rounded border-gx-border-default bg-gx-bg-primary accent-gx-neon"
									/>
									<span class="text-[11px] text-gx-text-muted">Schedule for later</span>
								</label>
								{#if scheduleEnabled}
									<div class="flex gap-2">
										<input
											type="date"
											bind:value={scheduleDate}
											class="px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
										/>
										<input
											type="time"
											bind:value={scheduleTime}
											class="px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
										/>
									</div>
								{/if}
							</div>

							<!-- Error -->
							{#if composeError}
								<div class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/20">
									<AlertCircle size={13} class="text-gx-status-error shrink-0" />
									<span class="text-xs text-gx-status-error">{composeError}</span>
								</div>
							{/if}

							<!-- Actions -->
							<div class="flex items-center gap-2 pt-2">
								<button
									onclick={submitPost}
									disabled={composing || !composeContent.trim() || selectedPlatforms.size === 0}
									class="flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
								>
									{#if composing}
										<Loader2 size={13} class="animate-spin" />
										Posting...
									{:else if scheduleEnabled}
										<Calendar size={13} />
										Schedule Post
									{:else}
										<Send size={13} />
										Queue Post
									{/if}
								</button>
								<span class="text-[10px] text-gx-text-muted">
									to {selectedPlatforms.size} platform{selectedPlatforms.size !== 1 ? 's' : ''}
								</span>
							</div>
						</CardContent>
					</Card>
				</div>

			<!-- ═══════ QUEUE TAB ═══════ -->
			{:else if activeTab === 'queue'}
				<div class="p-6 space-y-4">
					{#if queue.length === 0}
						<div class="flex flex-col items-center justify-center py-16 text-center">
							<FileText size={40} class="text-gx-text-muted/30 mb-3" />
							<p class="text-sm text-gx-text-muted">No posts in the queue yet</p>
							<button
								onclick={() => { composerOpen = true; activeTab = 'compose'; }}
								class="mt-3 text-xs text-gx-neon hover:underline"
							>
								Create your first post
							</button>
						</div>
					{:else}
						<!-- Queue filter stats -->
						<div class="flex items-center gap-2 text-[11px]">
							<span class="text-gx-text-muted">Showing {queue.length} posts:</span>
							{#if draftCount > 0}
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors.draft}">{draftCount} draft</Badge>
							{/if}
							{#if queuedCount > 0}
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors.queued}">{queuedCount} queued</Badge>
							{/if}
							{#if scheduledCount > 0}
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors.scheduled}">{scheduledCount} scheduled</Badge>
							{/if}
							{#if publishedCount > 0}
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors.published}">{publishedCount} published</Badge>
							{/if}
						</div>

						<div class="space-y-3">
							{#each queue as post (post.id)}
								{@const IconComp = platformIcons[post.platform] ?? Globe}
								{@const colorClass = platformColors[post.platform] ?? 'text-gx-text-muted'}
								<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
									<CardContent class="p-4">
										<div class="flex items-start gap-3">
											<!-- Platform icon -->
											<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated shrink-0 mt-0.5">
												<IconComp size={14} class={colorClass} />
											</div>

											<!-- Content -->
											<div class="flex-1 min-w-0 space-y-2">
												<div class="flex items-center gap-2 flex-wrap">
													{#if post.title}
														<span class="text-xs font-medium text-gx-text-primary">{post.title}</span>
													{/if}
													<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors[post.status] ?? statusColors.draft}">
														{post.status}
													</Badge>
													<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-border-default text-gx-text-muted">
														{contentTypeLabels[post.content_type] ?? post.content_type}
													</Badge>
												</div>

												<div class="text-xs text-gx-text-secondary whitespace-pre-wrap line-clamp-4 font-mono leading-relaxed">
													{post.body}
												</div>

												<!-- Tags -->
												{#if post.tags.length > 0}
													<div class="flex flex-wrap gap-1">
														{#each post.tags as tag}
															<span class="text-[9px] px-1.5 py-0.5 rounded bg-gx-bg-elevated text-gx-text-muted border border-gx-border-default">
																{tag}
															</span>
														{/each}
													</div>
												{/if}

												<!-- Metadata -->
												<div class="flex items-center gap-3 text-[10px] text-gx-text-muted">
													<span>Created {formatDate(post.created_at)}</span>
													{#if post.scheduled_at}
														<span class="flex items-center gap-1 text-gx-accent-blue">
															<Calendar size={10} />
															Scheduled for {formatDate(post.scheduled_at)}
														</span>
													{/if}
													{#if post.published_at}
														<span class="flex items-center gap-1 text-gx-status-success">
															<CheckCircle2 size={10} />
															Published {formatDate(post.published_at)}
														</span>
													{/if}
												</div>
											</div>

											<!-- Actions -->
											<div class="flex items-center gap-1 shrink-0">
												<button
													onclick={() => copyToClipboard(post.body, post.id)}
													title="Copy to clipboard"
													class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all"
												>
													{#if copiedId === post.id}
														<Check size={13} class="text-gx-status-success" />
													{:else}
														<Copy size={13} />
													{/if}
												</button>

												{#if post.status === 'queued' || post.status === 'scheduled'}
													<button
														onclick={() => markPublished(post.id)}
														title="Mark as published"
														class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-success hover:bg-gx-status-success/10 transition-all"
													>
														<CheckCircle2 size={13} />
													</button>
												{/if}

												{#if post.status !== 'published' && post.status !== 'cancelled'}
													<button
														onclick={() => cancelPost(post.id)}
														title="Cancel post"
														class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-all"
													>
														<Trash2 size={13} />
													</button>
												{/if}
											</div>
										</div>
									</CardContent>
								</Card>
							{/each}
						</div>
					{/if}
				</div>

			<!-- ═══════ TEMPLATES TAB ═══════ -->
			{:else if activeTab === 'templates'}
				<div class="p-6">
					<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
						{#each templates as template (template.id)}
							<Card class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-accent-magenta/30 transition-all group cursor-pointer" style={cardStyle}>
								<CardContent class="p-4 space-y-3" onclick={() => applyTemplate(template)}>
									<div class="flex items-center gap-2">
										<Sparkles size={13} class="text-gx-accent-magenta" />
										<span class="text-xs font-medium text-gx-text-primary">{template.name}</span>
									</div>

									<p class="text-[11px] text-gx-text-muted leading-relaxed">{template.description}</p>

									<!-- Platforms -->
									<div class="flex items-center gap-1.5">
										{#each template.platforms as pid}
											{@const TplIcon = platformIcons[pid] ?? Globe}
											<div class="flex items-center justify-center w-5 h-5 rounded bg-gx-bg-elevated" title={pid}>
												<TplIcon size={10} class={platformColors[pid] ?? 'text-gx-text-muted'} />
											</div>
										{/each}
									</div>

									<!-- Hook archetype badge -->
									<div class="flex items-center gap-2">
										<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-accent-magenta/30 text-gx-accent-magenta">
											{hookStyles.find(h => h.id === template.hook_archetype)?.label ?? template.hook_archetype}
										</Badge>
										<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-border-default text-gx-text-muted">
											{contentTypeLabels[template.content_type] ?? template.content_type}
										</Badge>
									</div>

									<!-- Preview snippet -->
									<div class="text-[10px] text-gx-text-muted/60 font-mono line-clamp-3 leading-relaxed bg-gx-bg-primary rounded p-2">
										{template.template_body}
									</div>

									<button
										onclick={() => applyTemplate(template)}
										class="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-bg-elevated text-gx-text-secondary hover:text-gx-accent-magenta hover:bg-gx-accent-magenta/10 border border-gx-border-default hover:border-gx-accent-magenta/30 transition-all opacity-0 group-hover:opacity-100"
									>
										<Megaphone size={12} />
										Use Template
									</button>
								</CardContent>
							</Card>
						{/each}
					</div>
				</div>

			<!-- ═══════ ANALYTICS TAB ═══════ -->
			{:else if activeTab === 'analytics'}
				<div class="p-6 space-y-6">
					<!-- Overview Cards -->
					<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
						<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
							<CardContent class="p-4 text-center">
								<div class="text-2xl font-bold text-gx-text-primary">{totalPosts}</div>
								<div class="text-[11px] text-gx-text-muted mt-1">Total Posts</div>
							</CardContent>
						</Card>
						<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
							<CardContent class="p-4 text-center">
								<div class="text-2xl font-bold text-gx-status-success">{publishedCount}</div>
								<div class="text-[11px] text-gx-text-muted mt-1">Published</div>
							</CardContent>
						</Card>
						<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
							<CardContent class="p-4 text-center">
								<div class="text-2xl font-bold text-gx-status-warning">{queuedCount + scheduledCount}</div>
								<div class="text-[11px] text-gx-text-muted mt-1">Pending</div>
							</CardContent>
						</Card>
						<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
							<CardContent class="p-4 text-center">
								<div class="text-2xl font-bold text-gx-accent-magenta">{platforms.filter(p => p.post_count > 0).length}</div>
								<div class="text-[11px] text-gx-text-muted mt-1">Active Platforms</div>
							</CardContent>
						</Card>
					</div>

					<!-- Posts per Platform -->
					<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<BarChart3 size={14} class="text-gx-accent-blue" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">Posts per Platform</CardTitle>
							</div>
						</CardHeader>
						<CardContent>
							<div class="space-y-2">
								{#each platforms as p}
									{@const IconComp = platformIcons[p.id] ?? Globe}
									{@const maxCount = Math.max(...platforms.map(pp => pp.post_count), 1)}
									{@const pct = (p.post_count / maxCount) * 100}
									<div class="flex items-center gap-3">
										<div class="flex items-center gap-1.5 w-28 shrink-0">
											<IconComp size={12} class={platformColors[p.id] ?? 'text-gx-text-muted'} />
											<span class="text-[11px] text-gx-text-muted">{p.name}</span>
										</div>
										<div class="flex-1 h-4 bg-gx-bg-primary rounded-gx overflow-hidden">
											<div
												class="h-full rounded-gx bg-gx-neon/30 transition-all duration-500"
												style="width: {pct}%"
											></div>
										</div>
										<span class="text-[11px] text-gx-text-secondary font-mono w-8 text-right">{p.post_count}</span>
									</div>
								{/each}
							</div>
						</CardContent>
					</Card>

					<!-- Golden Hour Heatmap -->
					<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<Clock size={14} class="text-gx-status-warning" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">Best Posting Times (UTC)</CardTitle>
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-status-warning/30 text-gx-status-warning">Golden Hour</Badge>
							</div>
						</CardHeader>
						<CardContent>
							<div class="overflow-x-auto">
								<div class="min-w-[600px]">
									<!-- Hour labels -->
									<div class="flex items-center gap-0">
										<div class="w-10 shrink-0"></div>
										{#each Array(24) as _, h}
											<div class="flex-1 text-center text-[8px] text-gx-text-muted/60">{h}</div>
										{/each}
									</div>
									<!-- Day rows -->
									{#each days as day, dayIdx}
										<div class="flex items-center gap-0 mt-0.5">
											<div class="w-10 shrink-0 text-[10px] text-gx-text-muted">{day}</div>
											{#each Array(24) as _, h}
												<div
													class="flex-1 h-4 border border-gx-bg-primary/50 rounded-sm transition-colors
														{heatmap[dayIdx][h]
															? 'bg-gx-status-warning/40 border-gx-status-warning/30'
															: 'bg-gx-bg-primary/30'}"
													title="{day} {h}:00 UTC{heatmap[dayIdx][h] ? ' (Golden Hour)' : ''}"
												></div>
											{/each}
										</div>
									{/each}
								</div>
							</div>
							<div class="flex items-center gap-4 mt-3 text-[10px] text-gx-text-muted">
								<div class="flex items-center gap-1.5">
									<div class="w-3 h-3 rounded-sm bg-gx-status-warning/40 border border-gx-status-warning/30"></div>
									Optimal posting time
								</div>
								<div class="flex items-center gap-1.5">
									<div class="w-3 h-3 rounded-sm bg-gx-bg-primary/30"></div>
									Standard hours
								</div>
							</div>
						</CardContent>
					</Card>

					<!-- Platform Engagement -->
					<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<Eye size={14} class="text-gx-accent-purple" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">Engagement Overview</CardTitle>
							</div>
						</CardHeader>
						<CardContent>
							<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
								{#each platforms as p}
									{@const IconComp = platformIcons[p.id] ?? Globe}
									{@const total = p.engagement.views + p.engagement.likes + p.engagement.comments + p.engagement.shares}
									<div class="rounded-gx bg-gx-bg-primary p-3 text-center">
										<IconComp size={16} class="{platformColors[p.id] ?? 'text-gx-text-muted'} mx-auto mb-1.5" />
										<div class="text-sm font-semibold text-gx-text-primary">{total}</div>
										<div class="text-[9px] text-gx-text-muted mt-0.5">{p.name}</div>
										{#if total > 0}
											<div class="mt-1.5 space-y-0.5 text-[9px] text-gx-text-muted">
												<div class="flex justify-between">
													<span>Views</span>
													<span class="text-gx-text-secondary">{p.engagement.views}</span>
												</div>
												<div class="flex justify-between">
													<span>Likes</span>
													<span class="text-gx-text-secondary">{p.engagement.likes}</span>
												</div>
											</div>
										{:else}
											<div class="text-[9px] text-gx-text-muted/40 mt-1 italic">No data</div>
										{/if}
									</div>
								{/each}
							</div>
						</CardContent>
					</Card>
				</div>
			{/if}
		{/if}
	</div>
</div>
