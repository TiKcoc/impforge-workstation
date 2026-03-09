<script lang="ts">
	/**
	 * Chat Page — BenikUI-integrated full chat view
	 *
	 * Three-panel layout: Sidebar | Chat Messages | Mission Control.
	 * All panels are style-engine aware for deep customization.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Page root
	 *   - header: Chat header bar
	 *   - empty-state: Empty chat illustration area
	 *   - right-panel: Mission control / model status panel
	 */

	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import ChatMessage from '$lib/components/chat/ChatMessage.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import ChatSidebar from '$lib/components/chat/ChatSidebar.svelte';
	import ModelStatusBadge from '$lib/components/chat/ModelStatusBadge.svelte';
	import ModelActivityCard from '$lib/components/chat/ModelActivityCard.svelte';
	import ModelPipelineView from '$lib/components/chat/ModelPipelineView.svelte';
	import ModelAvatar from '$lib/components/chat/ModelAvatar.svelte';
	import '$lib/components/chat/hljs-forge.css';
	import { Loader2, PanelLeftClose, PanelLeft, LayoutDashboard } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	const widgetId = 'chat-page';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let sidebarOpen = $state(true);

	let streamMode = $derived(getSetting('chatStreamMode'));
	let vizLevel = $derived(getSetting('chatVizLevel'));
	let compact = $derived(getSetting('chatCompactMode'));
	let activeMessages = $derived(chatStore.messages);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let emptyStateComponent = $derived(styleEngine.getComponentStyle(widgetId, 'empty-state'));
	let rightPanelComponent = $derived(styleEngine.getComponentStyle(widgetId, 'right-panel'));

	let headerStyle = $derived(
		hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : ''
	);
	let emptyStateStyle = $derived(
		hasEngineStyle && emptyStateComponent ? componentToCSS(emptyStateComponent) : ''
	);
	let rightPanelStyle = $derived(
		hasEngineStyle && rightPanelComponent ? componentToCSS(rightPanelComponent) : ''
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
	<!-- Sidebar -->
	<ChatSidebar collapsed={!sidebarOpen} />

	<!-- Main area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Header -->
		<div
			class="flex items-center gap-3 px-4 py-2 border-b border-gx-border-default {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
			style={headerStyle}
		>
			<button
				onclick={() => sidebarOpen = !sidebarOpen}
				class="p-1.5 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
				aria-label="Toggle sidebar"
			>
				{#if sidebarOpen}<PanelLeftClose size={16} />{:else}<PanelLeft size={16} />{/if}
			</button>
			<div class="flex-1 min-w-0">
				<h1 class="text-sm font-medium text-gx-text-primary truncate">
					{chatStore.activeConversation?.title ?? 'ImpForge Chat'}
				</h1>
			</div>

			<!-- Model Status Badge -->
			{#if vizLevel === 'minimal'}
				<ModelStatusBadge />
			{/if}

			{#if chatStore.isStreaming}
				<div class="flex items-center gap-1.5 text-[11px] text-gx-neon">
					<Loader2 size={12} class="animate-spin" />
					<span>Streaming</span>
				</div>
			{/if}
		</div>

		<!-- Content area -->
		<div class="flex-1 flex min-h-0">
			<!-- Chat messages -->
			<div class="flex-1 flex flex-col min-w-0">
				<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-4 py-4 space-y-2">
					{#if activeMessages.length === 0}
						<!-- Empty state -->
						<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted" style={emptyStateStyle}>
							<div class="w-20 h-20 rounded-full bg-gx-bg-elevated flex items-center justify-center border border-gx-border-default shadow-gx-glow-md">
								<ModelAvatar size={36} />
							</div>
							<h2 class="text-lg font-medium text-gx-text-secondary">Start a new conversation</h2>
							<p class="text-sm max-w-md text-center leading-relaxed">
								Ask me anything. Your message will be routed to the best AI model automatically.
							</p>
							<div class="flex flex-wrap justify-center gap-2 mt-2 max-w-lg">
								{#each ['Write a Python function', 'Create a Dockerfile', 'Explain async/await', 'Debug my code', 'Design a system', 'Research a topic'] as suggestion}
									<button
										onclick={() => handleSend(suggestion)}
										class="px-3 py-1.5 text-xs rounded-gx border border-gx-border-default text-gx-text-muted hover:border-gx-neon hover:text-gx-neon transition-colors"
									>
										{suggestion}
									</button>
								{/each}
							</div>
						</div>
					{:else}
						<!-- Pipeline visualization -->
						{#if vizLevel === 'pipeline' && chatStore.isStreaming}
							<div class="mb-4">
								<ModelPipelineView />
							</div>
						{/if}

						{#each activeMessages as msg (msg.id)}
							<ChatMessage message={msg} compact={compact} />
						{/each}
					{/if}
				</div>

				<!-- Input -->
				<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} />
			</div>

			<!-- Right panel: Mission Control or Activity Cards -->
			{#if streamMode === 'mission-control' || vizLevel === 'cards'}
				<div
					class="w-[280px] border-l border-gx-border-default p-3 overflow-y-auto shrink-0 {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
					style={rightPanelStyle}
				>
					{#if vizLevel === 'pipeline' || vizLevel === 'cards'}
						<div class="text-[11px] font-medium text-gx-text-muted uppercase tracking-wider mb-3">
							<LayoutDashboard size={12} class="inline mr-1" />
							Model Status
						</div>
						<div class="space-y-2">
							{#each modelStatus.models as model (model.id)}
								<ModelActivityCard {model} />
							{/each}
							{#if modelStatus.models.length === 0}
								<div class="text-[11px] text-gx-text-muted text-center py-4">
									No models active. Send a message to start.
								</div>
							{/if}
						</div>
					{/if}

					{#if streamMode === 'mission-control'}
						<!-- Pipeline always visible in mission control -->
						<div class="mt-4">
							<div class="text-[11px] font-medium text-gx-text-muted uppercase tracking-wider mb-2">
								Pipeline
							</div>
							<ModelPipelineView />
						</div>

						{#if modelStatus.lastRouting}
							<div class="mt-4 p-2 rounded glass-panel-subtle">
								<div class="text-[10px] text-gx-text-muted mb-1">Last Routing Decision</div>
								<div class="text-[11px] text-gx-text-secondary">
									<span class="text-gx-neon">{modelStatus.lastRouting.taskType}</span> →
									<span class="font-mono">{modelStatus.lastRouting.model.split('/').pop()}</span>
								</div>
							</div>
						{/if}
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
