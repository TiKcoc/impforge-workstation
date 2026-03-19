<script lang="ts">
	/**
	 * QuickActionsWidget — Navigation shortcuts for dashboard
	 * Compact grid of action links to key sections.
	 */
	import {
		MessageSquare, GitBranch, Container, Workflow,
		Code2, Brain, Globe, Newspaper
	} from '@lucide/svelte';
	import { Zap } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'widget-quick-actions';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	const actions = [
		{ label: 'Chat', icon: MessageSquare, href: '/chat', color: 'text-gx-neon' },
		{ label: 'GitHub', icon: GitBranch, href: '/github', color: 'text-gx-accent-cyan' },
		{ label: 'Docker', icon: Container, href: '/docker', color: 'text-gx-accent-purple' },
		{ label: 'ForgeFlow', icon: Workflow, href: '/workflows', color: 'text-gx-accent-orange' },
		{ label: 'IDE', icon: Code2, href: '/ide', color: 'text-gx-accent-blue' },
		{ label: 'Models', icon: Brain, href: '/ai', color: 'text-gx-accent-magenta' },
		{ label: 'Browser', icon: Globe, href: '/browser', color: 'text-gx-status-info' },
		{ label: 'News', icon: Newspaper, href: '/news', color: 'text-gx-status-warning' },
	];
</script>

<div class="h-full flex flex-col {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-secondary'} border border-gx-border-default rounded-gx overflow-hidden" style={containerStyle}>
	<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
		<Zap size={12} class="text-gx-neon" />
		<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Quick Actions</span>
	</div>
	<div class="flex-1 p-2 overflow-auto">
		<div class="grid grid-cols-4 gap-1.5">
			{#each actions as action}
				<a
					href={action.href}
					class="flex flex-col items-center gap-1 p-2 rounded-gx border border-transparent hover:border-gx-border-hover hover:bg-gx-bg-hover transition-all text-center"
				>
					<action.icon size={16} class={action.color} />
					<span class="text-[9px] text-gx-text-muted">{action.label}</span>
				</a>
			{/each}
		</div>
	</div>
</div>
