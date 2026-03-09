<script lang="ts">
	import { Bot, Play, Send, Loader2, Sparkles } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

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

<div class="flex flex-col h-full bg-[#0d1117]">
	<div class="flex items-center gap-2 px-3 py-2 border-b border-white/5 shrink-0">
		<Sparkles size={14} class="text-[#00FF66]" />
		<span class="text-xs font-semibold text-white/90">AI Agent</span>
	</div>

	<div class="flex-1 overflow-auto p-2 space-y-2">
		{#if ide.agentMessages.length === 0}
			<div class="text-center py-6">
				<Bot size={32} class="mx-auto text-white/20 mb-2" />
				<p class="text-xs text-white/40">AI coding agent ready.</p>
				<p class="text-[10px] text-white/30 mt-1">Can read, write, search, and execute.</p>
				<div class="flex flex-wrap gap-1.5 justify-center mt-3">
					{#each suggestions as suggestion}
						<button
							onclick={() => { agentInput = suggestion; handleSend(); }}
							class="text-[11px] px-2 py-1 bg-white/5 border border-white/10 rounded text-white/40 hover:text-[#00FF66] hover:border-[#00FF66]/50 transition-all"
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
					<div class="max-w-[85%] px-3 py-1.5 rounded bg-[#00FF66]/10 border border-[#00FF66]/20 text-xs text-white/90">
						{msg.content}
					</div>
				{:else if msg.role === 'tool'}
					<div class="max-w-[95%] px-2 py-1 rounded bg-[#161b22] border border-white/10 text-[11px] font-mono">
						<div class="flex items-center gap-1 text-[#89ddff] mb-0.5">
							<Play size={10} />
							{msg.toolCall?.tool}
						</div>
						<pre class="text-white/40 whitespace-pre-wrap max-h-24 overflow-auto">{msg.content.slice(0, 500)}{msg.content.length > 500 ? '...' : ''}</pre>
					</div>
				{:else}
					<div class="max-w-[95%] px-3 py-1.5 rounded bg-[#161b22] border border-white/10 text-xs text-white/70">
						<pre class="whitespace-pre-wrap font-sans">{msg.content}</pre>
					</div>
				{/if}
			</div>
		{/each}

		{#if ide.agentLoading}
			<div class="flex items-center gap-2 text-xs text-white/40">
				<Loader2 size={12} class="animate-spin" />
				<span>Thinking...</span>
			</div>
		{/if}
	</div>

	<div class="flex items-end gap-2 px-2 py-1.5 border-t border-white/5">
		<textarea
			bind:value={agentInput}
			onkeydown={handleKeydown}
			placeholder="Ask the AI agent..."
			rows="1"
			class="flex-1 bg-[#161b22] border border-white/10 rounded px-2 py-1.5 text-xs text-white/90 placeholder:text-white/30 resize-none outline-none focus:border-[#00FF66] transition-colors"
		></textarea>
		<button
			onclick={handleSend}
			disabled={ide.agentLoading || !agentInput.trim()}
			class="p-1.5 rounded bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30 hover:bg-[#00FF66]/20 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
		>
			<Send size={14} />
		</button>
	</div>
</div>
