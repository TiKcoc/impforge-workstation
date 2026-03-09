<script lang="ts">
	/**
	 * WidgetRenderer — Maps widget IDs to their Svelte components
	 *
	 * This is the central dispatch for the layout system.
	 * Given a widget ID from the registry, it renders the correct component.
	 * Unknown widgets show a placeholder.
	 */

	import SystemStatsWidget from './SystemStatsWidget.svelte';
	import ServiceHealthWidget from './ServiceHealthWidget.svelte';
	import AgentPoolWidget from './AgentPoolWidget.svelte';
	import QuickActionsWidget from './QuickActionsWidget.svelte';
	import ModelStatusWidget from './ModelStatusWidget.svelte';
	import { Package } from '@lucide/svelte';

	interface Props {
		widgetId: string;
	}

	let { widgetId }: Props = $props();
</script>

{#if widgetId === 'system-stats'}
	<SystemStatsWidget />
{:else if widgetId === 'agent-pool'}
	<AgentPoolWidget />
{:else if widgetId === 'model-status'}
	<ModelStatusWidget />
{:else if widgetId === 'quick-chat'}
	<!-- Quick Chat widget (placeholder — full chat uses /chat route) -->
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Quick Chat</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/chat" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open Chat →</a>
		</div>
	</div>
{:else if widgetId === 'docker-overview'}
	<ServiceHealthWidget />
{:else if widgetId === 'github-feed'}
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">GitHub Feed</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/github" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open GitHub →</a>
		</div>
	</div>
{:else if widgetId === 'browser-sessions' || widgetId === 'network-waterfall' || widgetId === 'console-output'}
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">
				{widgetId.replace(/-/g, ' ')}
			</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/browser" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open Browser →</a>
		</div>
	</div>
{:else if widgetId === 'eval-pipeline'}
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Eval Pipeline</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/evaluation" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open Eval →</a>
		</div>
	</div>
{:else if widgetId === 'news-ticker'}
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">News Ticker</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/news" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open News →</a>
		</div>
	</div>
{:else if widgetId === 'workflow-status'}
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Workflow Status</span>
		</div>
		<div class="flex-1 flex items-center justify-center p-4">
			<a href="/n8n" class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors">Open n8n →</a>
		</div>
	</div>
{:else}
	<!-- Unknown widget placeholder -->
	<div class="h-full flex flex-col items-center justify-center bg-gx-bg-secondary border border-gx-border-default rounded-gx p-4">
		<Package size={20} class="text-gx-text-muted mb-2" />
		<span class="text-[10px] text-gx-text-muted font-mono">{widgetId}</span>
	</div>
{/if}
