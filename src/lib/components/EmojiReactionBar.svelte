<script lang="ts">
	/**
	 * EmojiReactionBar -- reusable reaction strip for any content item.
	 * Renders existing reactions with counts and a quick-add row.
	 * Follows Svelte 5 runes pattern + Opera GX dark theme classes.
	 */

	interface Reaction {
		emoji: string;
		count: number;
		reacted: boolean;
	}

	interface Props {
		reactions: Reaction[];
		onReact: (emoji: string) => void;
		compact?: boolean;
	}

	let { reactions, onReact, compact = false }: Props = $props();

	const quickEmojis = ['\u{1F44D}', '\u{1F44E}', '\u{2764}\u{FE0F}', '\u{1F389}', '\u{1F680}', '\u{1F914}', '\u{1F440}', '\u{1F4AF}'];

	let showPicker = $state(false);

	// Emojis already used as reactions (avoid duplicates in the picker)
	let usedEmojis = $derived(new Set(reactions.map((r) => r.emoji)));
	let availableQuick = $derived(quickEmojis.filter((e) => !usedEmojis.has(e)));
</script>

<div class="flex flex-wrap items-center gap-1" role="group" aria-label="Reactions">
	{#each reactions as r (r.emoji)}
		<button
			onclick={() => onReact(r.emoji)}
			aria-label="{r.emoji} {r.count} reaction{r.count !== 1 ? 's' : ''}{r.reacted ? ', you reacted' : ''}"
			aria-pressed={r.reacted}
			class="inline-flex items-center gap-0.5 rounded-full border transition-all duration-150
				{compact ? 'px-1.5 py-0.5 text-[11px]' : 'px-2 py-1 text-xs'}
				{r.reacted
					? 'bg-gx-neon/15 border-gx-neon/40 text-gx-neon'
					: 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted hover:border-gx-neon/30 hover:text-gx-text-secondary'}"
		>
			<span>{r.emoji}</span>
			<span class="font-mono tabular-nums">{r.count}</span>
		</button>
	{/each}

	<!-- Add reaction toggle -->
	<div class="relative">
		<button
			onclick={() => (showPicker = !showPicker)}
			aria-label="Add reaction"
			aria-expanded={showPicker}
			class="inline-flex items-center justify-center rounded-full border border-dashed transition-all duration-150
				{compact ? 'w-6 h-6 text-[11px]' : 'w-7 h-7 text-xs'}
				border-gx-border-default text-gx-text-muted hover:border-gx-neon/40 hover:text-gx-neon hover:bg-gx-neon/5"
		>
			+
		</button>

		{#if showPicker}
			<!-- Emoji quick-picker popover -->
			<div
				class="absolute bottom-full left-0 mb-1 flex flex-wrap gap-1 p-1.5 rounded-gx
					bg-gx-bg-elevated border border-gx-border-default shadow-gx-glow-sm z-50 min-w-[160px]"
				role="listbox"
				aria-label="Pick an emoji"
			>
				{#each availableQuick as emoji (emoji)}
					<button
						onclick={() => {
							onReact(emoji);
							showPicker = false;
						}}
						role="option"
						aria-selected={false}
						class="w-7 h-7 flex items-center justify-center rounded hover:bg-gx-bg-hover
							text-base transition-transform hover:scale-125"
					>
						{emoji}
					</button>
				{/each}
				{#if availableQuick.length === 0}
					<span class="text-[10px] text-gx-text-muted px-1">All emojis used</span>
				{/if}
			</div>
		{/if}
	</div>
</div>

<!-- Close picker on outside click (always bound, guards internally) -->
<svelte:window
	onclick={(e) => {
		if (!showPicker) return;
		const target = e.target as HTMLElement;
		if (!target.closest('[role="listbox"]') && !target.closest('[aria-label="Add reaction"]')) {
			showPicker = false;
		}
	}}
/>
