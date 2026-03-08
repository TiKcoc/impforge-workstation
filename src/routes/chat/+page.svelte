<script lang="ts">
	import { chatStore } from '$lib/stores/chat.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import {
		MessageSquare, Send, Plus, Trash2, Bot, User, Loader2, Copy, Check
	} from '@lucide/svelte';

	let inputValue = $state('');
	let messagesContainer: HTMLDivElement | undefined = $state();
	let sidebarOpen = $state(true);
	let copied = $state<string | null>(null);

	let activeMessages = $derived(chatStore.messages);

	function scrollToBottom() {
		if (messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	$effect(() => {
		// Re-run whenever messages change or streaming content updates
		if (activeMessages.length > 0) {
			scrollToBottom();
		}
	});

	async function handleSend() {
		const msg = inputValue.trim();
		if (!msg || chatStore.isStreaming) return;
		inputValue = '';
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.ctrlKey || !e.shiftKey)) {
			e.preventDefault();
			handleSend();
		}
	}

	function copyToClipboard(text: string, id: string) {
		navigator.clipboard.writeText(text);
		copied = id;
		setTimeout(() => { copied = null; }, 2000);
	}

	function formatTimestamp(date: Date): string {
		return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}

	/**
	 * Simple code block renderer: splits content by triple-backtick fences
	 * and returns segments tagged as code or text.
	 */
	function parseContent(content: string): Array<{ type: 'text' | 'code'; lang?: string; value: string }> {
		const segments: Array<{ type: 'text' | 'code'; lang?: string; value: string }> = [];
		const parts = content.split(/```(\w*)\n?/);
		let inCode = false;
		let lang = '';

		for (let i = 0; i < parts.length; i++) {
			if (i % 2 === 0) {
				// Even parts are either text or code content
				if (inCode) {
					// Strip trailing ``` if present
					const value = parts[i].replace(/\n?$/, '');
					segments.push({ type: 'code', lang: lang || undefined, value });
					inCode = false;
				} else if (parts[i]) {
					segments.push({ type: 'text', value: parts[i] });
				}
			} else {
				// Odd parts are the language identifier after ```
				lang = parts[i];
				inCode = true;
			}
		}
		return segments;
	}
</script>

<div class="flex h-full">
	<!-- Sidebar -->
	{#if sidebarOpen}
		<div class="w-[250px] bg-gx-bg-secondary border-r border-gx-border-default flex flex-col shrink-0">
			<!-- Sidebar header -->
			<div class="p-3 border-b border-gx-border-default">
				<button
					onclick={() => chatStore.newConversation()}
					class="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors shadow-gx-glow-sm"
				>
					<Plus size={14} />
					New Chat
				</button>
			</div>

			<!-- Conversation list -->
			<div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5">
				{#each chatStore.conversations as conv (conv.id)}
					<div
						role="button"
						tabindex="0"
						onclick={() => chatStore.setActive(conv.id)}
						onkeydown={(e) => e.key === 'Enter' && chatStore.setActive(conv.id)}
						class="group flex items-center gap-2 px-3 py-2.5 text-xs rounded-gx cursor-pointer transition-colors
							{chatStore.activeConversationId === conv.id
								? 'bg-gx-bg-elevated text-gx-text-primary border-l-2 border-gx-neon'
								: 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
					>
						<MessageSquare size={12} class="shrink-0 {chatStore.activeConversationId === conv.id ? 'text-gx-neon' : ''}" />
						<span class="flex-1 truncate">{conv.title}</span>
						<button
							onclick={(e) => { e.stopPropagation(); chatStore.deleteConversation(conv.id); }}
							class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all p-0.5 rounded"
							aria-label="Delete conversation"
						>
							<Trash2 size={12} />
						</button>
					</div>
				{/each}

				{#if chatStore.conversations.length === 0}
					<div class="text-[11px] text-gx-text-muted text-center py-8 px-4">
						No conversations yet. Start a new chat!
					</div>
				{/if}
			</div>

			<!-- Sidebar footer -->
			<div class="p-3 border-t border-gx-border-default">
				<div class="text-[10px] text-gx-text-muted text-center">
					{chatStore.conversations.length} conversation{chatStore.conversations.length !== 1 ? 's' : ''}
				</div>
			</div>
		</div>
	{/if}

	<!-- Main chat area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Chat header -->
		<div class="flex items-center gap-3 px-4 py-2.5 border-b border-gx-border-default bg-gx-bg-secondary">
			<button
				onclick={() => sidebarOpen = !sidebarOpen}
				class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
				aria-label="Toggle sidebar"
			>
				<MessageSquare size={16} />
			</button>
			<div class="flex-1 min-w-0">
				<h1 class="text-sm font-medium text-gx-text-primary truncate">
					{chatStore.activeConversation?.title ?? 'ImpForge Chat'}
				</h1>
			</div>
			{#if chatStore.isStreaming}
				<div class="flex items-center gap-1.5 text-[11px] text-gx-neon">
					<Loader2 size={12} class="animate-spin" />
					<span>Streaming</span>
				</div>
			{/if}
		</div>

		<!-- Messages area -->
		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-4 py-4 space-y-4">
			{#if activeMessages.length === 0}
				<!-- Empty state -->
				<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
					<div class="w-20 h-20 rounded-full bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shadow-gx-glow-md">
						<Bot size={36} class="text-gx-neon" />
					</div>
					<h2 class="text-lg font-medium text-gx-text-secondary">Start a new conversation</h2>
					<p class="text-sm max-w-md text-center leading-relaxed">
						Ask me anything. Your message will be routed to the best free AI model automatically via OpenRouter.
					</p>
					<div class="flex flex-wrap justify-center gap-2 mt-2 max-w-lg">
						{#each ['Write a Python function', 'Create a Dockerfile', 'Explain async/await', 'Debug my code'] as suggestion}
							<button
								onclick={() => { inputValue = suggestion; }}
								class="px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-muted hover:border-gx-neon hover:text-gx-neon transition-colors"
							>
								{suggestion}
							</button>
						{/each}
					</div>
				</div>
			{:else}
				{#each activeMessages as msg (msg.id)}
					<div class="flex gap-3 {msg.role === 'user' ? 'justify-end' : ''}">
						<!-- Assistant avatar -->
						{#if msg.role === 'assistant'}
							<div class="w-8 h-8 rounded-full bg-gx-neon/10 flex items-center justify-center shrink-0 mt-0.5 border border-gx-neon/20">
								{#if msg.streaming}
									<Loader2 size={14} class="text-gx-neon animate-spin" />
								{:else}
									<Bot size={14} class="text-gx-neon" />
								{/if}
							</div>
						{/if}

						<!-- Message bubble -->
						<div class="max-w-[75%] {msg.role === 'user'
							? 'bg-gx-neon/10 border border-gx-neon/20'
							: 'bg-gx-bg-elevated border border-gx-border-default'} rounded-gx-lg px-4 py-3">

							<!-- Content with code block support -->
							<div class="text-sm leading-relaxed">
								{#each parseContent(msg.content) as segment}
									{#if segment.type === 'code'}
										<pre class="my-2 p-3 rounded-gx bg-gx-bg-primary border border-gx-border-default overflow-x-auto text-xs"><code>{segment.value}</code></pre>
									{:else}
										<span class="whitespace-pre-wrap break-words">{segment.value}</span>
									{/if}
								{/each}
								{#if msg.streaming && msg.content === ''}
									<span class="inline-flex gap-1 items-center text-gx-text-muted">
										<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 0ms"></span>
										<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 150ms"></span>
										<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 300ms"></span>
									</span>
								{:else if msg.streaming}
									<span class="inline-block w-1.5 h-4 bg-gx-neon animate-pulse ml-0.5 align-text-bottom"></span>
								{/if}
							</div>

							<!-- Message footer -->
							<div class="flex items-center gap-2 mt-2 text-[10px] text-gx-text-muted">
								<span>{formatTimestamp(msg.timestamp)}</span>
								{#if msg.model}
									<span class="px-1.5 py-0.5 rounded bg-gx-bg-primary border border-gx-border-default text-[9px] font-mono">
										{msg.model}
									</span>
								{/if}
								{#if msg.taskType}
									<span class="px-1.5 py-0.5 rounded bg-gx-neon/10 text-gx-neon text-[9px]">
										{msg.taskType}
									</span>
								{/if}
								{#if msg.role === 'assistant' && !msg.streaming}
									<button
										onclick={() => copyToClipboard(msg.content, msg.id)}
										class="ml-auto hover:text-gx-neon transition-colors p-0.5"
										aria-label="Copy message"
									>
										{#if copied === msg.id}
											<Check size={11} class="text-gx-status-success" />
										{:else}
											<Copy size={11} />
										{/if}
									</button>
								{/if}
							</div>
						</div>

						<!-- User avatar -->
						{#if msg.role === 'user'}
							<div class="w-8 h-8 rounded-full bg-gx-bg-elevated flex items-center justify-center shrink-0 mt-0.5 border border-gx-border-default">
								<User size={14} class="text-gx-text-muted" />
							</div>
						{/if}
					</div>
				{/each}
			{/if}
		</div>

		<!-- Input area -->
		<div class="border-t border-gx-border-default bg-gx-bg-secondary p-3">
			<div class="flex gap-2 items-end">
				<div class="flex-1 relative">
					<textarea
						bind:value={inputValue}
						onkeydown={handleKeydown}
						placeholder="Type your message... (Enter to send, Shift+Enter for new line)"
						rows={1}
						disabled={chatStore.isStreaming}
						class="w-full px-4 py-2.5 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx-lg resize-none
							focus:border-gx-neon focus:outline-none transition-colors disabled:opacity-50
							min-h-[40px] max-h-[160px]"
						style="field-sizing: content;"
					></textarea>
				</div>
				<button
					onclick={handleSend}
					disabled={chatStore.isStreaming || !inputValue.trim()}
					class="px-4 py-2.5 bg-gx-neon text-gx-bg-primary font-medium text-sm rounded-gx-lg
						hover:brightness-110 transition-all disabled:opacity-30 disabled:cursor-not-allowed
						flex items-center gap-2 shadow-gx-glow-sm shrink-0"
				>
					{#if chatStore.isStreaming}
						<Loader2 size={16} class="animate-spin" />
					{:else}
						<Send size={16} />
					{/if}
				</button>
			</div>
			<div class="flex items-center gap-3 mt-1.5 text-[10px] text-gx-text-muted">
				<span>Ctrl+Enter or Enter to send</span>
				<span class="text-gx-border-default">|</span>
				<span>Intelligent Router - auto-selects the best free model</span>
			</div>
		</div>
	</div>
</div>
