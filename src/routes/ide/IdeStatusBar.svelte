<script lang="ts">
	import { GitBranch, Circle, Cpu } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	interface Props {
		lspStatus?: string;
		gitBranch?: string;
		aiModel?: string;
	}

	let { lspStatus = 'disconnected', gitBranch = '', aiModel = 'Ollama' }: Props = $props();

	const cursorInfo = $derived(() => {
		const tab = ide.activeTab;
		if (!tab) return 'Ln 1, Col 1';
		return `Ln -, Col -`;
	});

	const language = $derived(ide.activeTab?.language || 'plaintext');
	const encoding = 'UTF-8';

	const lspColor = $derived(
		lspStatus === 'running' ? 'text-green-400' :
		lspStatus === 'starting' ? 'text-yellow-400' :
		'text-white/30'
	);
</script>

<div class="flex items-center h-6 px-2 bg-[#0d1117] border-t border-white/5 text-[11px] text-white/40 shrink-0 gap-3">
	<div class="flex items-center gap-1">
		<Circle size={8} class="{lspColor} fill-current" />
		<span>LSP</span>
	</div>

	{#if gitBranch}
		<div class="flex items-center gap-1">
			<GitBranch size={12} />
			<span>{gitBranch}</span>
		</div>
	{/if}

	<div class="flex-1"></div>

	<span>{cursorInfo()}</span>
	<span>{encoding}</span>
	<span class="text-[#89ddff]">{language}</span>

	<div class="flex items-center gap-1">
		<Cpu size={10} class="text-[#00FF66]" />
		<span class="text-[#00FF66]">{aiModel}</span>
	</div>
</div>
