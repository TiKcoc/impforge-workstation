<script lang="ts">
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { Newspaper, ExternalLink, Clock, TrendingUp } from '@lucide/svelte';

	// Placeholder news items - will be populated by curated feed
	const newsItems = [
		{
			title: 'NEXUS AI News Feed',
			summary: 'Your curated AI news will appear here. Weekly digests from arXiv, HuggingFace, OpenRouter, and more.',
			source: 'NEXUS',
			date: 'Coming soon',
			category: 'Announcement',
			relevance: 'high',
		},
		{
			title: 'Free OpenRouter Models',
			summary: '28+ free models available through OpenRouter. Devstral Small, Llama 4 Scout, Qwen3-30B and more — all at zero cost.',
			source: 'OpenRouter',
			date: 'Always updated',
			category: 'Models',
			relevance: 'high',
		},
		{
			title: 'Local AI is Getting Better',
			summary: 'Quantized models like Qwen2.5-Coder 7B run on consumer GPUs with near-cloud quality. 4-bit models need only 4-5GB VRAM.',
			source: 'Research',
			date: 'Trend',
			category: 'Hardware',
			relevance: 'medium',
		},
	];

	const categoryColors: Record<string, string> = {
		'Announcement': 'bg-gx-neon/10 text-gx-neon border-gx-neon/30',
		'Models': 'bg-gx-accent-magenta/10 text-gx-accent-magenta border-gx-accent-magenta/30',
		'Hardware': 'bg-gx-accent-cyan/10 text-gx-accent-cyan border-gx-accent-cyan/30',
		'Research': 'bg-gx-accent-purple/10 text-gx-accent-purple border-gx-accent-purple/30',
	};
</script>

<div class="p-6 space-y-4">
	<div class="flex items-center gap-3">
		<Newspaper size={24} class="text-gx-status-info" />
		<h1 class="text-xl font-bold">AI News</h1>
		<Badge variant="outline" class="text-xs border-gx-border-default text-gx-text-muted">
			<Clock size={10} class="mr-1" />
			Weekly Digest
		</Badge>
	</div>

	<p class="text-sm text-gx-text-muted">
		Curated AI news from arXiv, HuggingFace, OpenRouter, LangChain, and n8n — delivered weekly.
	</p>

	<div class="space-y-3">
		{#each newsItems as item}
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/30 transition-all cursor-pointer">
				<Card.Header class="pb-2">
					<div class="flex items-start justify-between">
						<div class="flex-1">
							<div class="flex items-center gap-2 mb-1">
								<Badge class={categoryColors[item.category] || 'bg-gx-bg-elevated text-gx-text-muted'} variant="outline">
									{item.category}
								</Badge>
								{#if item.relevance === 'high'}
									<Badge class="bg-gx-neon/10 text-gx-neon border-gx-neon/30 text-[10px]">
										<TrendingUp size={10} class="mr-0.5" />
										Relevant
									</Badge>
								{/if}
							</div>
							<Card.Title class="text-sm">{item.title}</Card.Title>
						</div>
						<span class="text-[10px] text-gx-text-muted shrink-0 ml-2">{item.date}</span>
					</div>
				</Card.Header>
				<Card.Content>
					<p class="text-xs text-gx-text-secondary">{item.summary}</p>
					<div class="flex items-center gap-2 mt-2">
						<span class="text-[10px] text-gx-text-muted">via {item.source}</span>
					</div>
				</Card.Content>
			</Card.Root>
		{/each}
	</div>
</div>
