<script lang="ts">
	/**
	 * ChatConvergence — BenikUI-integrated three-panel convergence view
	 *
	 * IDE-like layout: Left (Explorer + Model Cards) | Center (Editor + Terminal) | Right (Chat).
	 * All sections are style-engine aware for deep customization.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root layout wrapper
	 *   - explorer: Left explorer panel
	 *   - editor: Center editor area
	 *   - terminal: Bottom terminal section
	 *   - chat-panel: Right chat panel
	 *   - chat-header: Chat panel header
	 */

	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from './ChatMessage.svelte';
	import ChatInput from './ChatInput.svelte';
	import ModelPipelineView from './ModelPipelineView.svelte';
	import ModelActivityCard from './ModelActivityCard.svelte';
	import ModelAvatar from './ModelAvatar.svelte';
	import './hljs-forge.css';
	import { FolderTree, Terminal as TerminalIcon } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
	}

	let { widgetId = 'chat-convergence' }: Props = $props();

	let messagesContainer: HTMLDivElement | undefined = $state();
	let activeMessages = $derived(chatStore.messages);
	let vizLevel = $derived(getSetting('chatVizLevel'));

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let explorerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'explorer'));
	let editorComponent = $derived(styleEngine.getComponentStyle(widgetId, 'editor'));
	let terminalComponent = $derived(styleEngine.getComponentStyle(widgetId, 'terminal'));
	let chatPanelComponent = $derived(styleEngine.getComponentStyle(widgetId, 'chat-panel'));
	let chatHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'chat-header'));

	let explorerStyle = $derived(
		hasEngineStyle && explorerComponent ? componentToCSS(explorerComponent) : ''
	);
	let editorStyle = $derived(
		hasEngineStyle && editorComponent ? componentToCSS(editorComponent) : ''
	);
	let terminalStyle = $derived(
		hasEngineStyle && terminalComponent ? componentToCSS(terminalComponent) : ''
	);
	let chatPanelStyle = $derived(
		hasEngineStyle && chatPanelComponent ? componentToCSS(chatPanelComponent) : ''
	);
	let chatHeaderStyle = $derived(
		hasEngineStyle && chatHeaderComponent ? componentToCSS(chatHeaderComponent) : ''
	);

	function scrollToBottom() {
		if (messagesContainer) {
			requestAnimationFrame(() => {
				messagesContainer!.scrollTop = messagesContainer!.scrollHeight;
			});
		}
	}

	$effect(() => {
		if (activeMessages.length > 0) scrollToBottom();
	});

	async function handleSend(msg: string) {
		const key = getSetting('openrouterKey');
		await chatStore.sendMessage(msg, key);
	}
</script>

<div class="flex h-full">
	<!-- Left: File Explorer + Model Status -->
	<div
		class="w-[200px] border-r border-gx-border-default flex flex-col shrink-0 {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
		style={explorerStyle}
	>
		<div class="p-2 border-b border-gx-border-default">
			<div class="flex items-center gap-2 text-[11px] font-medium text-gx-text-muted uppercase tracking-wider">
				<FolderTree size={12} />
				Explorer
			</div>
		</div>
		<div class="flex-1 overflow-y-auto p-2 text-[11px] text-gx-text-muted">
			<p class="italic">Open from IDE to browse files</p>
		</div>

		<!-- Model Cards -->
		{#if vizLevel === 'cards' || vizLevel === 'pipeline'}
			<div class="border-t border-gx-border-default p-2 space-y-2">
				<div class="text-[10px] font-medium text-gx-text-muted uppercase">Models</div>
				{#each modelStatus.models as model (model.id)}
					<ModelActivityCard {model} />
				{/each}
			</div>
		{/if}
	</div>

	<!-- Center: Editor placeholder + Terminal -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Editor area -->
		<div
			class="flex-1 flex items-center justify-center text-gx-text-muted text-sm border-b border-gx-border-default {hasEngineStyle ? '' : 'bg-gx-bg-primary'}"
			style={editorStyle}
		>
			<div class="text-center">
				<p class="text-xs">Editor — Connected to CodeForge IDE</p>
				<p class="text-[10px] mt-1">Open files from Explorer or use @file in chat</p>
			</div>
		</div>

		<!-- Terminal area -->
		<div
			class="h-[150px] border-t border-gx-border-default p-2 overflow-y-auto {hasEngineStyle ? '' : 'bg-gx-bg-primary'}"
			style={terminalStyle}
		>
			<div class="flex items-center gap-2 text-[11px] text-gx-text-muted mb-1">
				<TerminalIcon size={12} />
				Terminal
			</div>
			<div class="font-mono text-[11px] text-green-400/70">
				$ Ready for commands...
			</div>
		</div>
	</div>

	<!-- Right: Chat Stream -->
	<div
		class="w-[380px] border-l border-gx-border-default flex flex-col shrink-0"
		style={chatPanelStyle}
	>
		<div
			class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-default {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
			style={chatHeaderStyle}
		>
			<ModelAvatar size={16} />
			<span class="text-xs font-medium text-gx-text-primary flex-1">
				{chatStore.activeConversation?.title ?? 'Chat'}
			</span>
		</div>

		<!-- Pipeline above messages -->
		{#if vizLevel === 'pipeline' && chatStore.isStreaming}
			<div class="px-3 py-2 border-b border-gx-border-default">
				<ModelPipelineView />
			</div>
		{/if}

		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-3 py-3 space-y-2">
			{#if activeMessages.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<ModelAvatar size={28} />
					<p class="text-xs text-center">Chat + IDE + Terminal — unified workspace</p>
				</div>
			{:else}
				{#each activeMessages as msg (msg.id)}
					<ChatMessage message={msg} compact={true} />
				{/each}
			{/if}
		</div>

		<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} placeholder="Chat with your code..." />
	</div>
</div>
