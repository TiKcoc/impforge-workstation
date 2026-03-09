<script lang="ts">
	/**
	 * AgentPoolWidget — NeuralSwarm agent pool status
	 * Shows agent names and their current state (idle/busy/error).
	 */
	import { Bot, Brain } from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { StyledWidget } from '$lib/components/styled';

	const agents = [
		{ name: 'Orchestrator', role: 'coordinator', status: 'idle' },
		{ name: 'Coder', role: 'code-gen', status: 'idle' },
		{ name: 'Researcher', role: 'research', status: 'idle' },
		{ name: 'Debugger', role: 'debug', status: 'idle' },
	];
</script>

<StyledWidget widgetId="agent-pool">
<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
	<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
		<Bot size={12} class="text-gx-accent-magenta" />
		<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Agent Pool</span>
		<Badge variant="outline" class="text-[8px] px-1 py-0 h-3 border-gx-neon/30 text-gx-neon ml-auto">Auto</Badge>
	</div>
	<div class="flex-1 p-2.5 space-y-1 overflow-auto">
		{#each agents as agent}
			<div class="flex items-center gap-2 text-[11px] py-0.5 px-1 rounded hover:bg-gx-bg-hover transition-colors">
				<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success shrink-0"></span>
				<span class="text-gx-text-secondary flex-1">{agent.name}</span>
				<span class="text-[9px] text-gx-text-muted font-mono">{agent.status}</span>
			</div>
		{/each}
	</div>
	<div class="px-2.5 py-1.5 border-t border-gx-border-default">
		<div class="flex items-center gap-1 text-[9px] text-gx-text-muted">
			<Brain size={9} />
			<span>Model: llama-4-scout</span>
		</div>
	</div>
</div>
</StyledWidget>
