<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import {
		Newspaper,
		ExternalLink,
		Clock,
		Tag,
		Filter,
		RefreshCw,
		Loader2,
		WifiOff,
		AlertCircle,
		Rss
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-news';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let cardComponent = $derived(styleEngine.getComponentStyle(widgetId, 'card'));
	let cardStyle = $derived(hasEngineStyle && cardComponent ? componentToCSS(cardComponent) : '');
	let newsHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let newsHeaderStyle = $derived(
		hasEngineStyle && newsHeaderComponent ? componentToCSS(newsHeaderComponent) : ''
	);

	interface NewsItem {
		id: string;
		title: string;
		summary: string;
		source: string;
		url: string;
		date: string;
		category: string;
		is_sample: boolean;
	}

	interface NewsFetchResult {
		items: NewsItem[];
		sources_checked: number;
		sources_failed: number;
		is_cached: boolean;
		error: string | null;
	}

	let newsItems = $state<NewsItem[]>([]);
	let loading = $state(true);
	let refreshing = $state(false);
	let error = $state<string | null>(null);
	let sourcesChecked = $state(0);
	let sourcesFailed = $state(0);
	let isCached = $state(false);

	type CategoryFilter = 'All' | 'Models' | 'Tools' | 'Research' | 'Industry';
	let activeFilter = $state<CategoryFilter>('All');

	const filters: CategoryFilter[] = ['All', 'Models', 'Tools', 'Research', 'Industry'];

	let filteredNews = $derived(
		activeFilter === 'All'
			? newsItems
			: newsItems.filter((item) => item.category === activeFilter)
	);

	const categoryStyles: Record<string, string> = {
		Models: 'bg-gx-accent-purple/10 text-gx-accent-purple border-gx-accent-purple/30',
		Tools: 'bg-gx-accent-blue/10 text-gx-accent-blue border-gx-accent-blue/30',
		Research: 'bg-gx-status-warning/10 text-gx-status-warning border-gx-status-warning/30',
		Industry: 'bg-gx-accent-cyan/10 text-gx-accent-cyan border-gx-accent-cyan/30'
	};

	async function fetchNews(isRefresh = false) {
		if (isRefresh) {
			refreshing = true;
		} else {
			loading = true;
		}
		error = null;

		try {
			const result = await invoke<NewsFetchResult>('news_fetch');
			newsItems = result.items;
			sourcesChecked = result.sources_checked;
			sourcesFailed = result.sources_failed;
			isCached = result.is_cached;
			if (result.error) {
				error = result.error;
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			newsItems = [];
		} finally {
			loading = false;
			refreshing = false;
		}
	}

	function formatDate(dateStr: string): string {
		if (!dateStr) return '';
		const date = new Date(dateStr);
		if (isNaN(date.getTime())) return dateStr;
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function openSource(url: string) {
		if (url) window.open(url, '_blank');
	}

	onMount(() => {
		fetchNews();
	});
</script>

<main
	class="flex flex-col h-screen {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}"
	style={containerStyle}
>
	<!-- Header -->
	<header
		class="h-14 border-b border-gx-border-default {hasEngineStyle && newsHeaderComponent ? '' : 'bg-gx-bg-secondary'} flex items-center px-4 gap-3 shrink-0"
		style={newsHeaderStyle}
	>
		<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">&larr;</a>
		<Newspaper class="w-5 h-5 text-gx-neon" />
		<h1 class="text-lg font-semibold text-gx-text-primary">AI News Feed</h1>

		{#if !loading}
			<Badge variant="outline" class="text-xs border-gx-border-default text-gx-text-muted">
				<Rss size={10} class="mr-1" />
				{sourcesChecked} sources
				{#if sourcesFailed > 0}
					<span class="text-gx-status-warning ml-1">({sourcesFailed} offline)</span>
				{/if}
			</Badge>
		{/if}

		<div class="flex-1"></div>

		{#if isCached}
			<Badge variant="outline" class="text-xs border-gx-status-warning/30 text-gx-status-warning">
				<WifiOff size={10} class="mr-1" />
				Offline
			</Badge>
		{/if}

		<span class="text-xs text-gx-text-muted">{filteredNews.length} articles</span>

		<button
			onclick={() => fetchNews(true)}
			disabled={refreshing}
			class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-colors disabled:opacity-50"
			title="Refresh feeds"
		>
			<RefreshCw size={14} class={refreshing ? 'animate-spin' : ''} />
		</button>
	</header>

	<!-- Filter Tabs -->
	<div
		class="flex items-center gap-1 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0"
	>
		<Filter size={14} class="text-gx-text-muted mr-1" />
		{#each filters as filter}
			<button
				onclick={() => (activeFilter = filter)}
				class="px-3 py-1.5 text-xs rounded-gx transition-all {activeFilter === filter
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
			>
				{filter}
			</button>
		{/each}
	</div>

	<!-- Content Area -->
	<div class="flex-1 overflow-y-auto p-4">
		<div class="max-w-3xl mx-auto space-y-3">
			<!-- Loading State -->
			{#if loading}
				<div class="flex flex-col items-center justify-center py-24 gap-4 text-gx-text-muted">
					<Loader2 size={32} class="animate-spin text-gx-neon" />
					<p class="text-sm">Fetching news from {sourcesChecked || 6} feed sources...</p>
				</div>

				<!-- Error State -->
			{:else if error && newsItems.length === 0}
				<div class="flex flex-col items-center justify-center py-24 gap-4 text-gx-text-muted">
					<AlertCircle size={36} class="text-gx-status-error opacity-60" />
					<p class="text-sm font-medium text-gx-text-secondary">Failed to fetch news</p>
					<p class="text-xs text-gx-text-disabled max-w-md text-center">{error}</p>
					<button
						onclick={() => fetchNews()}
						class="mt-2 px-4 py-2 text-xs rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-colors"
					>
						Try Again
					</button>
				</div>

				<!-- News Feed -->
			{:else}
				<!-- Partial error banner -->
				{#if error}
					<div
						class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-status-warning/10 border border-gx-status-warning/20 text-xs text-gx-status-warning mb-3"
					>
						<AlertCircle size={12} />
						<span>{error}</span>
					</div>
				{/if}

				{#each filteredNews as item (item.id)}
					<Card
						class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-all group {item.is_sample ? 'border-dashed' : ''}"
					>
						<CardHeader class="pb-2">
							<div class="flex items-start justify-between gap-3">
								<div class="flex-1 min-w-0">
									<div class="flex items-center gap-2 mb-2 flex-wrap">
										<Badge
											variant="outline"
											class={categoryStyles[item.category] ??
												'bg-gx-bg-elevated text-gx-text-muted'}
										>
											<Tag size={10} class="mr-1" />
											{item.category}
										</Badge>
										{#if item.is_sample}
											<Badge
												variant="outline"
												class="bg-gx-bg-elevated text-gx-text-disabled border-gx-border-subtle text-[10px]"
											>
												Sample
											</Badge>
										{/if}
									</div>
									<CardTitle class="text-sm leading-snug">{item.title}</CardTitle>
								</div>
								{#if item.url}
									<button
										onclick={() => openSource(item.url)}
										class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-tertiary transition-colors shrink-0 opacity-0 group-hover:opacity-100"
										title="Open source"
									>
										<ExternalLink size={14} />
									</button>
								{/if}
							</div>
						</CardHeader>
						<CardContent>
							<p class="text-xs text-gx-text-secondary leading-relaxed line-clamp-3">
								{item.summary}
							</p>
							<div class="flex items-center justify-between mt-3">
								<div class="flex items-center gap-3 text-[11px] text-gx-text-muted">
									<span>via {item.source}</span>
									{#if item.date}
										<span class="flex items-center gap-1">
											<Clock size={10} />
											{formatDate(item.date)}
										</span>
									{/if}
								</div>
								{#if item.url}
									<button
										onclick={() => openSource(item.url)}
										class="text-[11px] text-gx-neon hover:underline flex items-center gap-1"
									>
										Read more
										<ExternalLink size={10} />
									</button>
								{/if}
							</div>
						</CardContent>
					</Card>
				{/each}

				<!-- Empty filter state -->
				{#if filteredNews.length === 0}
					<div
						class="flex flex-col items-center justify-center py-16 gap-3 text-gx-text-muted"
					>
						<Newspaper size={36} class="opacity-40" />
						<p>No articles in this category</p>
					</div>
				{/if}
			{/if}
		</div>
	</div>
</main>
