<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Newspaper, ExternalLink, Clock, Tag, Filter } from '@lucide/svelte';

	interface NewsItem {
		id: number;
		title: string;
		summary: string;
		source: string;
		url: string;
		date: string;
		category: 'Models' | 'Tools' | 'Research' | 'Industry';
		nexusRelevant: boolean;
	}

	const newsItems: NewsItem[] = [
		{
			id: 1,
			title: 'Claude 4.6 Opus Released',
			summary:
				'Anthropic releases Claude 4.6 Opus with improved reasoning, 200K context window, and native tool use. Benchmarks show 15% improvement in code generation tasks over previous version.',
			source: 'Anthropic Blog',
			url: 'https://anthropic.com/blog',
			date: '2026-03-05',
			category: 'Models',
			nexusRelevant: true
		},
		{
			id: 2,
			title: 'Tauri 2.10 Brings WebView Improvements',
			summary:
				'Tauri 2.10 ships with improved WebView2 performance, better IPC throughput, and native window management APIs. Memory usage reduced by 20% compared to 2.9.',
			source: 'Tauri Blog',
			url: 'https://tauri.app/blog',
			date: '2026-03-04',
			category: 'Tools',
			nexusRelevant: true
		},
		{
			id: 3,
			title: 'RouteLLM: 75% Cost Reduction in LLM Serving',
			summary:
				'New paper from Berkeley demonstrates intelligent routing between small and large models, achieving 75% cost reduction with only 3% quality degradation on standard benchmarks.',
			source: 'arXiv 2602.18291',
			url: 'https://arxiv.org',
			date: '2026-03-03',
			category: 'Research',
			nexusRelevant: true
		},
		{
			id: 4,
			title: 'AMD ROCm 7.2 Preview Released',
			summary:
				'ROCm 7.2 preview adds Flash Attention 2.0 support for RDNA 3, improved HIP compiler performance, and better PyTorch 2.6 integration. VRAM management overhauled.',
			source: 'AMD Developer Blog',
			url: 'https://rocm.docs.amd.com',
			date: '2026-03-02',
			category: 'Tools',
			nexusRelevant: true
		},
		{
			id: 5,
			title: 'Svelte 5 Runes Production Ready',
			summary:
				'Svelte 5 runes system declared production-ready with $state, $derived, $effect, and $props. Ecosystem adoption reaches 85% of top packages. Performance benchmarks show 40% faster updates.',
			source: 'Svelte Blog',
			url: 'https://svelte.dev/blog',
			date: '2026-02-28',
			category: 'Tools',
			nexusRelevant: true
		},
		{
			id: 6,
			title: 'Agent-as-a-Judge: Evaluating AI Agent Quality',
			summary:
				'Stanford researchers propose using specialized AI agents as judges for complex multi-step tasks. The method outperforms human evaluation in consistency while reducing evaluation costs by 90%.',
			source: 'arXiv 2602.15544',
			url: 'https://arxiv.org',
			date: '2026-02-26',
			category: 'Research',
			nexusRelevant: false
		},
		{
			id: 7,
			title: 'Docker Desktop 5.0 with AI Assistant',
			summary:
				'Docker Desktop 5.0 integrates an AI assistant for Dockerfile optimization, compose file generation, and container debugging. Supports local model backends including Ollama.',
			source: 'Docker Blog',
			url: 'https://docker.com/blog',
			date: '2026-02-24',
			category: 'Tools',
			nexusRelevant: false
		},
		{
			id: 8,
			title: 'OpenRouter Free Tier Expanded to 28 Models',
			summary:
				'OpenRouter expands free model tier to 28 models including Devstral Small, Llama 4 Scout, Qwen3-30B-A3B, and Phi-4. Rate limits increased to 200 requests per minute for free users.',
			source: 'OpenRouter',
			url: 'https://openrouter.ai',
			date: '2026-02-22',
			category: 'Industry',
			nexusRelevant: true
		},
		{
			id: 9,
			title: 'Qwen3 Released: Open-Source Reasoning Models',
			summary:
				'Alibaba releases Qwen3 family with native thinking mode. The 30B-A3B MoE variant runs efficiently on consumer GPUs while matching GPT-4 on reasoning benchmarks.',
			source: 'Qwen Blog',
			url: 'https://qwen.ai',
			date: '2026-02-20',
			category: 'Models',
			nexusRelevant: true
		},
		{
			id: 10,
			title: 'MCP Protocol Reaches 1.0 Specification',
			summary:
				'The Model Context Protocol (MCP) reaches 1.0 with standardized tool calling, resource management, and server discovery. Over 500 MCP servers now listed in the official registry.',
			source: 'MCP Working Group',
			url: 'https://modelcontextprotocol.io',
			date: '2026-02-18',
			category: 'Industry',
			nexusRelevant: true
		}
	];

	type CategoryFilter = 'All' | 'Models' | 'Tools' | 'Research' | 'Industry';
	let activeFilter = $state<CategoryFilter>('All');

	const filters: CategoryFilter[] = ['All', 'Models', 'Tools', 'Research', 'Industry'];

	let filteredNews = $derived(
		activeFilter === 'All'
			? newsItems
			: newsItems.filter((item) => item.category === activeFilter)
	);

	const categoryStyles: Record<string, string> = {
		Models: 'bg-purple-500/10 text-purple-400 border-purple-500/30',
		Tools: 'bg-blue-500/10 text-blue-400 border-blue-500/30',
		Research: 'bg-amber-500/10 text-amber-400 border-amber-500/30',
		Industry: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30'
	};

	function formatDate(dateStr: string): string {
		const date = new Date(dateStr);
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function openSource(url: string) {
		window.open(url, '_blank');
	}
</script>

<main class="flex flex-col h-screen bg-gx-bg-primary">
	<!-- Header -->
	<header
		class="h-14 border-b border-gx-border-default bg-gx-bg-secondary flex items-center px-4 gap-3 shrink-0"
	>
		<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">&larr;</a>
		<Newspaper class="w-5 h-5 text-gx-neon" />
		<h1 class="text-lg font-semibold text-gx-text-primary">AI News Feed</h1>
		<Badge variant="outline" class="text-xs border-gx-border-default text-gx-text-muted">
			<Clock size={10} class="mr-1" />
			Weekly Digest
		</Badge>
		<div class="flex-1"></div>
		<span class="text-xs text-gx-text-muted">{filteredNews.length} articles</span>
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

	<!-- News Feed -->
	<div class="flex-1 overflow-y-auto p-4">
		<div class="max-w-3xl mx-auto space-y-3">
			{#each filteredNews as item (item.id)}
				<Card
					class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-all group"
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
									{#if item.nexusRelevant}
										<Badge
											variant="outline"
											class="bg-gx-neon/10 text-gx-neon border-gx-neon/30 text-[10px]"
										>
											Relevant to NEXUS
										</Badge>
									{/if}
								</div>
								<CardTitle class="text-sm leading-snug">{item.title}</CardTitle>
							</div>
							<button
								onclick={() => openSource(item.url)}
								class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-tertiary transition-colors shrink-0 opacity-0 group-hover:opacity-100"
								title="Open source"
							>
								<ExternalLink size={14} />
							</button>
						</div>
					</CardHeader>
					<CardContent>
						<p class="text-xs text-gx-text-secondary leading-relaxed line-clamp-3">
							{item.summary}
						</p>
						<div class="flex items-center justify-between mt-3">
							<div class="flex items-center gap-3 text-[11px] text-gx-text-muted">
								<span>via {item.source}</span>
								<span class="flex items-center gap-1">
									<Clock size={10} />
									{formatDate(item.date)}
								</span>
							</div>
							<button
								onclick={() => openSource(item.url)}
								class="text-[11px] text-gx-neon hover:underline flex items-center gap-1"
							>
								Read more
								<ExternalLink size={10} />
							</button>
						</div>
					</CardContent>
				</Card>
			{/each}

			{#if filteredNews.length === 0}
				<div class="flex flex-col items-center justify-center py-16 gap-3 text-gx-text-muted">
					<Newspaper size={36} class="opacity-40" />
					<p>No articles in this category</p>
				</div>
			{/if}
		</div>
	</div>
</main>
