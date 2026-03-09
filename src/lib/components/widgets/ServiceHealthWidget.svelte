<script lang="ts">
	/**
	 * ServiceHealthWidget — Ollama/Docker/n8n/NeuralSwarm status
	 * Shows connection status for all integrated services.
	 */
	import { Activity } from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';

	const services = [
		{ key: 'ollama' as const, label: 'Ollama' },
		{ key: 'docker' as const, label: 'Docker' },
		{ key: 'n8n' as const, label: 'n8n' },
		{ key: 'neuralswarm' as const, label: 'Swarm' },
	];

	let onlineCount = $derived(
		services.filter(s => system.services[s.key] === 'online').length
	);
</script>

<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
	<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
		<Activity size={12} class="text-gx-neon" />
		<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">Services</span>
		<span class="text-[9px] text-gx-text-muted ml-auto font-mono">{onlineCount}/{services.length}</span>
	</div>
	<div class="flex-1 p-2.5 space-y-1.5 overflow-auto">
		{#each services as svc}
			{@const status = system.services[svc.key]}
			<div class="flex items-center justify-between text-[11px] py-0.5">
				<div class="flex items-center gap-1.5">
					<span class="w-1.5 h-1.5 rounded-full shrink-0
						{status === 'online' ? 'bg-gx-status-success' : status === 'checking' ? 'bg-gx-status-warning animate-pulse' : 'bg-gx-status-error'}"></span>
					<span class="text-gx-text-secondary">{svc.label}</span>
				</div>
				<span class="text-gx-text-muted font-mono text-[9px]">
					{status === 'online' ? 'OK' : status === 'checking' ? '...' : 'OFF'}
				</span>
			</div>
		{/each}
	</div>
</div>
