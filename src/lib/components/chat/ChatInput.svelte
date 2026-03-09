<script lang="ts">
	/**
	 * ChatInput — BenikUI-integrated chat input with @-mentions
	 *
	 * Smart input with context mentions (@file, @codebase, etc.),
	 * auto-resize, and style engine integration for deep customization.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - textarea: The input field
	 *   - send-button: The send button
	 *   - mentions-popup: The @-mention dropdown
	 */

	import { Send, Loader2 } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		onSend: (message: string) => void;
		isStreaming: boolean;
		placeholder?: string;
	}

	let {
		widgetId = 'chat-input',
		onSend,
		isStreaming,
		placeholder = 'Type your message... (Enter to send, Shift+Enter for new line)'
	}: Props = $props();

	let value = $state('');
	let showMentions = $state(false);
	let mentionQuery = $state('');
	let selectedMentionIndex = $state(0);

	const MENTION_SOURCES = [
		{ id: 'file', label: '@file', description: 'Include a file as context', icon: '📄' },
		{ id: 'codebase', label: '@codebase', description: 'Search project code', icon: '🔍' },
		{ id: 'docs', label: '@docs', description: 'Search documentation', icon: '📚' },
		{ id: 'terminal', label: '@terminal', description: 'Recent terminal output', icon: '📟' },
		{ id: 'memory', label: '@memory', description: 'ForgeMemory lookup', icon: '🧠' },
		{ id: 'model', label: '@model', description: 'Force specific model', icon: '🤖' },
		{ id: 'diff', label: '@diff', description: 'Current git diff', icon: '📊' },
		{ id: 'errors', label: '@errors', description: 'Lint/build errors', icon: '❌' },
	];

	let filteredMentions = $derived(
		mentionQuery
			? MENTION_SOURCES.filter(s => s.label.includes(mentionQuery.toLowerCase()) || s.description.toLowerCase().includes(mentionQuery.toLowerCase()))
			: MENTION_SOURCES
	);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let textareaComponent = $derived(styleEngine.getComponentStyle(widgetId, 'textarea'));
	let sendButtonComponent = $derived(styleEngine.getComponentStyle(widgetId, 'send-button'));
	let mentionsComponent = $derived(styleEngine.getComponentStyle(widgetId, 'mentions-popup'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let textareaStyle = $derived(
		hasEngineStyle && textareaComponent ? componentToCSS(textareaComponent) : ''
	);
	let sendButtonStyle = $derived(
		hasEngineStyle && sendButtonComponent ? componentToCSS(sendButtonComponent) : ''
	);
	let mentionsStyle = $derived(
		hasEngineStyle && mentionsComponent ? componentToCSS(mentionsComponent) : ''
	);

	function handleKeydown(e: KeyboardEvent) {
		if (showMentions) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				selectedMentionIndex = (selectedMentionIndex + 1) % filteredMentions.length;
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				selectedMentionIndex = (selectedMentionIndex - 1 + filteredMentions.length) % filteredMentions.length;
				return;
			}
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				insertMention(filteredMentions[selectedMentionIndex]);
				return;
			}
			if (e.key === 'Escape') {
				showMentions = false;
				return;
			}
		}

		if (e.key === 'Enter' && (e.ctrlKey || !e.shiftKey)) {
			e.preventDefault();
			handleSend();
		}
	}

	function handleInput() {
		const lastAt = value.lastIndexOf('@');
		if (lastAt >= 0) {
			const afterAt = value.slice(lastAt + 1);
			if (!afterAt.includes(' ') && (lastAt === 0 || value[lastAt - 1] === ' ' || value[lastAt - 1] === '\n')) {
				mentionQuery = afterAt;
				showMentions = true;
				selectedMentionIndex = 0;
				return;
			}
		}
		showMentions = false;
	}

	function insertMention(source: typeof MENTION_SOURCES[number]) {
		const lastAt = value.lastIndexOf('@');
		value = value.slice(0, lastAt) + source.label + ' ';
		showMentions = false;
	}

	function handleSend() {
		const msg = value.trim();
		if (!msg || isStreaming) return;
		value = '';
		showMentions = false;
		onSend(msg);
	}
</script>

<div
	class="border-t border-gx-border-default p-3 {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
	style={containerStyle}
>
	<!-- @-mention popup -->
	{#if showMentions && filteredMentions.length > 0}
		<div
			class="absolute bottom-full left-0 right-0 mx-3 mb-1 rounded-lg shadow-lg max-h-[200px] overflow-y-auto z-50 {hasEngineStyle ? '' : 'bg-gx-bg-elevated border border-gx-border-default'}"
			style={mentionsStyle}
		>
			{#each filteredMentions as source, i (source.id)}
				<button
					onclick={() => insertMention(source)}
					class="flex items-center gap-3 w-full px-3 py-2 text-sm text-left transition-colors
						{i === selectedMentionIndex ? 'bg-gx-neon/10 text-gx-neon' : 'text-gx-text-secondary hover:bg-gx-bg-hover'}"
				>
					<span class="text-base">{source.icon}</span>
					<div>
						<div class="font-mono text-xs">{source.label}</div>
						<div class="text-[10px] text-gx-text-muted">{source.description}</div>
					</div>
				</button>
			{/each}
		</div>
	{/if}

	<div class="flex gap-2 items-end relative">
		<div class="flex-1 relative">
			<textarea
				bind:value
				onkeydown={handleKeydown}
				oninput={handleInput}
				{placeholder}
				rows={1}
				disabled={isStreaming}
				class="w-full px-4 py-2.5 text-sm rounded-gx-lg resize-none
					focus:outline-none transition-colors disabled:opacity-50
					min-h-[40px] max-h-[160px]
					{hasEngineStyle ? '' : 'bg-gx-bg-tertiary border border-gx-border-default focus:border-gx-neon'}"
				style="{textareaStyle} field-sizing: content;"
			></textarea>
		</div>
		<button
			onclick={handleSend}
			disabled={isStreaming || !value.trim()}
			class="px-4 py-2.5 font-medium text-sm rounded-gx-lg
				hover:brightness-110 transition-all disabled:opacity-30 disabled:cursor-not-allowed
				flex items-center gap-2 shrink-0
				{hasEngineStyle ? '' : 'bg-gx-neon text-gx-bg-primary shadow-gx-glow-sm'}"
			style={sendButtonStyle}
		>
			{#if isStreaming}
				<Loader2 size={16} class="animate-spin" />
			{:else}
				<Send size={16} />
			{/if}
		</button>
	</div>
	<div class="flex items-center gap-3 mt-1.5 text-[10px] text-gx-text-muted">
		<span>Enter to send · Shift+Enter for new line · @ for context</span>
	</div>
</div>
