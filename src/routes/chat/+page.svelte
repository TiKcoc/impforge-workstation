<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { chat } from '$lib/stores/chat.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import {
		Plus, Trash2, Send, Bot, User, Loader2, Sparkles, Copy, Check
	} from '@lucide/svelte';

	let inputValue = $state('');
	let messagesContainer: HTMLDivElement | undefined = $state();
	let copied = $state<string | null>(null);

	onMount(() => {
		if (chat.conversations.length === 0) {
			chat.createConversation();
		}
	});

	async function handleSend(e: Event) {
		e.preventDefault();
		const msg = inputValue.trim();
		if (!msg) return;
		inputValue = '';
		await chat.sendMessage(msg);
		await tick();
		scrollToBottom();
	}

	function scrollToBottom() {
		if (messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	function copyToClipboard(text: string, id: string) {
		navigator.clipboard.writeText(text);
		copied = id;
		setTimeout(() => { copied = null; }, 2000);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend(e);
		}
	}

	let activeMessages = $derived(chat.activeConversation?.messages ?? []);

	$effect(() => {
		if (chat.streamingContent) {
			scrollToBottom();
		}
	});
</script>

<div class="flex h-full">
	<!-- Conversation sidebar -->
	<div class="w-56 bg-gx-bg-secondary border-r border-gx-border-default flex flex-col shrink-0">
		<div class="p-2">
			<button
				onclick={() => chat.createConversation()}
				class="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors"
			>
				<Plus size={14} />
				New Chat
			</button>
		</div>

		<div class="flex-1 overflow-y-auto px-1 space-y-0.5">
			{#each chat.sortedConversations as conv}
				<div
					role="button"
					tabindex="0"
					onclick={() => chat.setActive(conv.id)}
					onkeydown={(e) => e.key === 'Enter' && chat.setActive(conv.id)}
					class="group flex items-center gap-2 px-3 py-2 text-xs rounded-gx cursor-pointer transition-colors
						{chat.activeConversationId === conv.id
							? 'bg-gx-bg-elevated text-gx-text-primary'
							: 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
				>
					<span class="flex-1 truncate">{conv.title}</span>
					{#if chat.conversations.length > 1}
						<button
							onclick={(e) => { e.stopPropagation(); chat.deleteConversation(conv.id); }}
							class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all"
						>
							<Trash2 size={12} />
						</button>
					{/if}
				</div>
			{/each}
		</div>

		<div class="p-2 border-t border-gx-border-default">
			<div class="text-[10px] text-gx-text-muted text-center">
				{chat.conversations.length} conversation{chat.conversations.length !== 1 ? 's' : ''}
			</div>
		</div>
	</div>

	<!-- Main chat area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Messages -->
		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto p-4 space-y-4">
			{#if activeMessages.length === 0 && !chat.streamingContent}
				<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
					<div class="w-16 h-16 rounded-full bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default">
						<Sparkles size={28} class="text-gx-neon" />
					</div>
					<h2 class="text-lg font-medium text-gx-text-secondary">How can I help you?</h2>
					<p class="text-sm max-w-md text-center">
						Ask me anything. I'll route your message to the best free AI model automatically.
					</p>
					<div class="flex gap-2 mt-2">
						{#each ['Write a Python function', 'Create a Dockerfile', 'Explain async/await'] as suggestion}
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
						{#if msg.role === 'assistant'}
							<div class="w-7 h-7 rounded-full bg-gx-neon/10 flex items-center justify-center shrink-0 mt-0.5">
								<Bot size={14} class="text-gx-neon" />
							</div>
						{/if}

						<div class="max-w-[75%] {msg.role === 'user'
							? 'bg-gx-neon/10 border border-gx-neon/20'
							: 'bg-gx-bg-elevated border border-gx-border-default'} rounded-gx-lg px-4 py-3">
							<div class="text-sm whitespace-pre-wrap break-words">{msg.content}</div>
							<div class="flex items-center gap-2 mt-2 text-[10px] text-gx-text-muted">
								<span>{msg.timestamp.toLocaleTimeString()}</span>
								{#if msg.model}
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default">
										{msg.model}
									</Badge>
								{/if}
								{#if msg.role === 'assistant'}
									<button
										onclick={() => copyToClipboard(msg.content, msg.id)}
										class="ml-auto hover:text-gx-neon transition-colors"
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

						{#if msg.role === 'user'}
							<div class="w-7 h-7 rounded-full bg-gx-bg-elevated flex items-center justify-center shrink-0 mt-0.5 border border-gx-border-default">
								<User size={14} class="text-gx-text-muted" />
							</div>
						{/if}
					</div>
				{/each}

				<!-- Streaming indicator -->
				{#if chat.streamingContent}
					<div class="flex gap-3">
						<div class="w-7 h-7 rounded-full bg-gx-neon/10 flex items-center justify-center shrink-0 mt-0.5">
							<Bot size={14} class="text-gx-neon animate-pulse" />
						</div>
						<div class="max-w-[75%] bg-gx-bg-elevated border border-gx-neon/30 rounded-gx-lg px-4 py-3">
							<div class="text-sm whitespace-pre-wrap break-words">{chat.streamingContent}<span class="inline-block w-1.5 h-4 bg-gx-neon animate-pulse ml-0.5 align-text-bottom"></span></div>
						</div>
					</div>
				{/if}
			{/if}
		</div>

		<!-- Input area -->
		<div class="border-t border-gx-border-default bg-gx-bg-secondary p-3">
			<form onsubmit={handleSend} class="flex gap-2">
				<div class="flex-1 relative">
					<textarea
						bind:value={inputValue}
						onkeydown={handleKeydown}
						placeholder="Type your message..."
						rows="1"
						disabled={chat.sending}
						class="w-full px-4 py-2.5 pr-10 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx-lg resize-none focus:border-gx-neon transition-colors disabled:opacity-50"
					></textarea>
				</div>
				<button
					type="submit"
					disabled={chat.sending || !inputValue.trim()}
					class="px-4 py-2.5 bg-gx-neon text-gx-bg-primary font-medium text-sm rounded-gx-lg hover:bg-gx-neon-bright transition-colors disabled:opacity-30 disabled:cursor-not-allowed flex items-center gap-2"
				>
					{#if chat.sending}
						<Loader2 size={16} class="animate-spin" />
					{:else}
						<Send size={16} />
					{/if}
				</button>
			</form>
			<div class="flex items-center gap-2 mt-1.5 text-[10px] text-gx-text-muted">
				<Sparkles size={10} />
				<span>Intelligent Router — auto-selects the best free model</span>
			</div>
		</div>
	</div>
</div>
