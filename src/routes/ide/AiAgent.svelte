<script lang="ts">
	import { Bot, Play, Send, Loader2, Sparkles } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'ide-ai-agent';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let headerComp = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let headerStyle = $derived(hasEngineStyle && headerComp ? componentToCSS(headerComp) : '');
	let messageBubbleComp = $derived(styleEngine.getComponentStyle(widgetId, 'message-bubble'));
	let messageBubbleStyle = $derived(hasEngineStyle && messageBubbleComp ? componentToCSS(messageBubbleComp) : '');
	let toolCallComp = $derived(styleEngine.getComponentStyle(widgetId, 'tool-call'));
	let toolCallStyle = $derived(hasEngineStyle && toolCallComp ? componentToCSS(toolCallComp) : '');
	let inputAreaComp = $derived(styleEngine.getComponentStyle(widgetId, 'input-area'));
	let inputAreaStyle = $derived(hasEngineStyle && inputAreaComp ? componentToCSS(inputAreaComp) : '');
	let suggestionsComp = $derived(styleEngine.getComponentStyle(widgetId, 'suggestions'));
	let suggestionsStyle = $derived(hasEngineStyle && suggestionsComp ? componentToCSS(suggestionsComp) : '');

	let agentInput = $state('');

	async function handleSend() {
		if (agentInput.trim() && !ide.agentLoading) {
			const msg = agentInput;
			agentInput = '';
			await ide.sendAgentMessage(msg);
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	const suggestions = [
		'Read this file',
		'Find TODO comments',
		'Explain this code',
		'Write tests for this',
		'Refactor this function',
		'Find similar code',
	];
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default shrink-0" style={headerStyle}>
		<Sparkles size={14} class="text-gx-neon" />
		<span class="text-xs font-semibold text-gx-text-primary">AI Agent</span>
	</div>

	<div class="flex-1 overflow-auto p-2 space-y-2">
		{#if ide.agentMessages.length === 0}
			<div class="text-center py-6">
				<Bot size={32} class="mx-auto text-gx-text-muted mb-2" />
				<p class="text-xs text-gx-text-muted">AI coding agent ready.</p>
				<p class="text-[10px] text-gx-text-muted mt-1">Can read, write, search, and execute.</p>
				<div class="flex flex-wrap gap-1.5 justify-center mt-3" style={suggestionsStyle}>
					{#each suggestions as suggestion}
						<button
							onclick={() => { agentInput = suggestion; handleSend(); }}
							class="text-[11px] px-2 py-1 bg-gx-bg-elevated border border-gx-border-default rounded text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/50 transition-all"
						>
							{suggestion}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		{#each ide.agentMessages as msg}
			<div class="flex gap-2 {msg.role === 'user' ? 'justify-end' : ''}">
				{#if msg.role === 'user'}
					<div class="max-w-[85%] px-3 py-1.5 rounded bg-gx-neon/10 border border-gx-neon/20 text-xs text-gx-text-primary" style={messageBubbleStyle}>
						{msg.content}
					</div>
				{:else if msg.role === 'tool'}
					<div class="max-w-[95%] px-2 py-1 rounded bg-gx-bg-secondary border border-gx-border-default text-[11px] font-mono" style={toolCallStyle}>
						<div class="flex items-center gap-1 text-gx-accent-cyan mb-0.5">
							<Play size={10} />
							{msg.toolCall?.tool}
						</div>
						<pre class="text-gx-text-muted whitespace-pre-wrap max-h-24 overflow-auto">{msg.content.slice(0, 500)}{msg.content.length > 500 ? '...' : ''}</pre>
					</div>
				{:else}
					<div class="max-w-[95%] px-3 py-1.5 rounded bg-gx-bg-secondary border border-gx-border-default text-xs text-gx-text-secondary">
						<pre class="whitespace-pre-wrap font-sans">{msg.content}</pre>
					</div>
				{/if}
			</div>
		{/each}

		{#if ide.agentLoading}
			<div class="flex items-center gap-2 text-xs text-gx-text-muted">
				<Loader2 size={12} class="animate-spin" />
				<span>Thinking...</span>
			</div>
		{/if}
	</div>

	<div class="flex items-end gap-2 px-2 py-1.5 border-t border-gx-border-default" style={inputAreaStyle}>
		<textarea
			bind:value={agentInput}
			onkeydown={handleKeydown}
			placeholder="Ask the AI agent..."
			rows="1"
			class="flex-1 bg-gx-bg-secondary border border-gx-border-default rounded px-2 py-1.5 text-xs text-gx-text-primary placeholder:text-gx-text-muted resize-none outline-none focus:border-gx-neon transition-colors"
		></textarea>
		<button
			onclick={handleSend}
			disabled={ide.agentLoading || !agentInput.trim()}
			class="p-1.5 rounded bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
		>
			<Send size={14} />
		</button>
	</div>
</div>
