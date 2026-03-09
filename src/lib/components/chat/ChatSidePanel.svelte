<script lang="ts">
	/**
	 * ChatSidePanel — BenikUI-integrated collapsible chat sidebar
	 *
	 * Right-side panel with chat messages and model status.
	 * Uses style engine for deep customization of all panel sections.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: The panel wrapper
	 *   - header: Panel header bar
	 *   - messages: Messages scroll area
	 */

	import { chatStore } from '$lib/stores/chat.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from './ChatMessage.svelte';
	import ChatInput from './ChatInput.svelte';
	import ModelStatusBadge from './ModelStatusBadge.svelte';
	import ModelPipelineView from './ModelPipelineView.svelte';
	import ModelAvatar from './ModelAvatar.svelte';
	import './hljs-forge.css';
	import { X, Maximize2 } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		open: boolean;
		onClose: () => void;
	}

	let { widgetId = 'chat-side-panel', open, onClose }: Props = $props();

	let messagesContainer: HTMLDivElement | undefined = $state();
	let vizLevel = $derived(getSetting('chatVizLevel'));
	let activeMessages = $derived(chatStore.messages);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let messagesComponent = $derived(styleEngine.getComponentStyle(widgetId, 'messages'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let headerStyle = $derived(
		hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : ''
	);
	let messagesStyle = $derived(
		hasEngineStyle && messagesComponent ? componentToCSS(messagesComponent) : ''
	);

	function scrollToBottom() {
		if (messagesContainer) {
			requestAnimationFrame(() => {
				messagesContainer!.scrollTop = messagesContainer!.scrollHeight;
			});
		}
	}

	$effect(() => {
		if (activeMessages.length > 0 && open) scrollToBottom();
	});

	async function handleSend(msg: string) {
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}

	function openFullChat() {
		onClose();
		goto('/chat');
	}
</script>

{#if open}
	<div
		class="w-[380px] h-full border-l border-gx-border-default flex flex-col shrink-0 shadow-xl {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
		style={containerStyle}
	>
		<!-- Header -->
		<div
			class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
			style={headerStyle}
		>
			<ModelAvatar size={20} />
			<span class="text-xs font-medium text-gx-text-primary flex-1">Chat</span>
			<ModelStatusBadge />
			<button onclick={openFullChat} class="p-1 rounded text-gx-text-muted hover:text-gx-neon" aria-label="Open full chat">
				<Maximize2 size={12} />
			</button>
			<button onclick={onClose} class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary" aria-label="Close chat">
				<X size={14} />
			</button>
		</div>

		<!-- Messages -->
		<div
			bind:this={messagesContainer}
			class="flex-1 overflow-y-auto px-3 py-3 space-y-2"
			style={messagesStyle}
		>
			{#if activeMessages.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<ModelAvatar size={28} />
					<p class="text-xs text-center">Ask anything — AI routed automatically</p>
				</div>
			{:else}
				{#if vizLevel === 'pipeline' && chatStore.isStreaming}
					<ModelPipelineView />
				{/if}
				{#each activeMessages as msg (msg.id)}
					<ChatMessage message={msg} compact={true} />
				{/each}
			{/if}
		</div>

		<!-- Input -->
		<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} placeholder="Ask anything..." />
	</div>
{/if}
