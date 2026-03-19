<script lang="ts">
	import { MessageSquare, LayoutGrid } from '@lucide/svelte';

	interface Props {
		mode: 'classic' | 'modern';
		onToggle: () => void;
	}

	let { mode, onToggle }: Props = $props();
</script>

<button
	onclick={onToggle}
	class="flex items-center gap-1.5 px-2 py-1 text-[11px] font-medium rounded-gx
		border transition-all duration-200 select-none
		{mode === 'classic'
			? 'text-gx-accent-cyan border-gx-accent-cyan/30 bg-gx-accent-cyan/10 hover:bg-gx-accent-cyan/20'
			: 'text-gx-neon border-gx-neon/30 bg-gx-neon/10 hover:bg-gx-neon/20'}"
	title={mode === 'classic' ? 'Switch to Modern (chat) mode' : 'Switch to Classic (ribbon) mode'}
>
	{#if mode === 'classic'}
		<MessageSquare size={13} />
		<span>Modern</span>
	{:else}
		<LayoutGrid size={13} />
		<span>Classic</span>
	{/if}
</button>
