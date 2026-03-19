<script lang="ts">
	/**
	 * ChatConvergence — BenikUI-integrated three-panel convergence view
	 *
	 * IDE-like layout: Left (Explorer + Model Cards) | Center (Editor + Terminal) | Right (Chat).
	 * All sections are style-engine aware for deep customization.
	 *
	 * HyperChat enhancements (arXiv:2601.01027, arXiv:2504.19056):
	 *   - AgentCharacter in chat header (state indicator)
	 *   - ModelRoutingViz below header (cascade visualization)
	 *   - TokenStreamViz during generation
	 *   - EnhancedChatMessage with reactions, diff view, agent avatars
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root layout wrapper
	 *   - explorer: Left explorer panel
	 *   - editor: Center editor area
	 *   - terminal: Bottom terminal section
	 *   - chat-panel: Right chat panel
	 *   - chat-header: Chat panel header
	 */

	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { chatStore } from '$lib/stores/chat.svelte';
	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { getSetting } from '$lib/stores/settings.svelte';
	import EnhancedChatMessage from './EnhancedChatMessage.svelte';
	import ChatInput from './ChatInput.svelte';
	import ModelPipelineView from './ModelPipelineView.svelte';
	import ModelActivityCard from './ModelActivityCard.svelte';
	import ModelAvatar from './ModelAvatar.svelte';
	import AgentCharacter from './AgentCharacter.svelte';
	import ModelRoutingViz from './ModelRoutingViz.svelte';
	import TokenStreamViz from './TokenStreamViz.svelte';
	import './hljs-forge.css';
	import { FolderTree, Terminal as TerminalIcon, Cpu, Cloud, ChevronDown, Flame } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import { achievementStore } from '$lib/stores/achievements.svelte';

	interface Props {
		widgetId?: string;
	}

	let { widgetId = 'chat-convergence' }: Props = $props();

	let messagesContainer: HTMLDivElement | undefined = $state();
	let activeMessages = $derived(chatStore.messages);
	let vizLevel = $derived(getSetting('chatVizLevel'));

	// ── Model Selector (Ollama + OpenRouter) ──
	interface ModelOption {
		id: string;
		name: string;
		provider: 'ollama' | 'openrouter';
	}

	let ollamaAvailable = $state(false);
	let ollamaModels = $state<ModelOption[]>([]);
	let showModelDropdown = $state(false);

	const cloudModels: ModelOption[] = [
		{ id: 'mistralai/devstral-small:free', name: 'Devstral Small', provider: 'openrouter' },
		{ id: 'meta-llama/llama-4-scout:free', name: 'Llama 4 Scout', provider: 'openrouter' },
		{ id: 'qwen/qwen3-30b-a3b:free', name: 'Qwen3 30B', provider: 'openrouter' },
	];
	let hasApiKey = $derived(!!getSetting('openrouterKey'));

	let selectedModelName = $derived(() => {
		const sel = chatStore.selectedModel;
		if (!sel) return ollamaAvailable ? 'Auto (Local)' : hasApiKey ? 'Auto (Cloud)' : 'No Backend';
		const found = [...ollamaModels, ...cloudModels].find(m => m.id === sel);
		return found?.name ?? sel;
	});

	async function detectOllama() {
		try {
			const status = await invoke<{ available: boolean; models: string[] }>('cmd_ollama_status');
			ollamaAvailable = status.available;
			if (status.available && status.models) {
				ollamaModels = status.models.map(m => ({
					id: `ollama:${m}`,
					name: m.replace(/:latest$/, ''),
					provider: 'ollama' as const,
				}));
				if (!chatStore.selectedModel && !hasApiKey && ollamaModels.length > 0) {
					chatStore.selectedModel = ollamaModels[0].id;
				}
			}
		} catch {
			ollamaAvailable = false;
		}
	}

	function selectModel(id: string) {
		chatStore.selectedModel = id;
		showModelDropdown = false;
	}

	onMount(() => {
		detectOllama();
	});

	// BenikUI style engine — auto-load widget style
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
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

	// --- HyperChat: Agent state derivation ---
	let agentState = $derived((): 'idle' | 'thinking' | 'working' | 'success' | 'error' | 'waiting' | 'sleeping' => {
		if (!chatStore.isStreaming) {
			const lastMsg = activeMessages[activeMessages.length - 1];
			if (lastMsg?.role === 'assistant' && lastMsg.content.startsWith('Error:')) return 'error';
			if (lastMsg?.role === 'assistant' && lastMsg.content) return 'idle';
			return 'idle';
		}
		const lastMsg = activeMessages[activeMessages.length - 1];
		if (lastMsg?.streaming && lastMsg.content === '') return 'thinking';
		return 'working';
	});

	let agentName = $derived(() => {
		const active = modelStatus.activeModel;
		if (!active) return 'AI';
		const n = active.name.toLowerCase();
		if (n.includes('claude')) return 'Claude';
		if (n.includes('qwen')) return 'Qwen';
		if (n.includes('hermes')) return 'Hermes';
		if (n.includes('llama')) return 'Llama';
		if (n.includes('mistral') || n.includes('devstral')) return 'Mistral';
		return active.name.split(/[:/]/).pop()?.replace(/:latest$/, '').slice(0, 12) ?? 'AI';
	});

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
		const ollamaUrl = getSetting('ollamaUrl') || 'http://localhost:11434';
		// Track chat action for achievements (fire-and-forget)
		achievementStore.trackAction('chat');
		await chatStore.sendMessage(msg, key, ollamaUrl);
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
			<!-- HyperChat: AgentCharacter state indicator -->
			<AgentCharacter
				agentName={agentName()}
				state={agentState()}
				model={modelStatus.activeModel?.name}
				size="sm"
			/>
			<span class="text-xs font-medium text-gx-text-primary">
				{chatStore.activeConversation?.title ?? 'Chat'}
			</span>
			<div class="flex-1"></div>
			<!-- Streak badge -->
			{#if achievementStore.progress.current_streak > 0}
				<div class="flex items-center gap-1 px-1.5 py-0.5 rounded bg-orange-500/10 text-[10px]">
					<Flame size={10} class="text-orange-400 {achievementStore.progress.current_streak >= 7 ? 'animate-pulse' : ''}" />
					<span class="font-medium text-orange-400">{achievementStore.progress.current_streak}</span>
				</div>
			{/if}
			<!-- Model Selector -->
			<div class="relative">
				<button
					onclick={() => showModelDropdown = !showModelDropdown}
					class="flex items-center gap-1.5 px-2 py-1 rounded text-[10px] bg-gx-bg-tertiary hover:bg-gx-bg-hover text-gx-text-muted transition-colors"
				>
					{#if chatStore.selectedModel?.startsWith('ollama:')}
						<Cpu size={10} class="text-green-400" />
					{:else}
						<Cloud size={10} class="text-blue-400" />
					{/if}
					<span class="max-w-[100px] truncate">{selectedModelName()}</span>
					<ChevronDown size={10} />
				</button>
				{#if showModelDropdown}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="absolute right-0 top-full mt-1 w-56 bg-gx-bg-secondary border border-gx-border-default rounded shadow-xl z-50 overflow-hidden"
						onclick={(e) => e.stopPropagation()}
					>
						{#if ollamaModels.length > 0}
							<div class="px-2 py-1 text-[9px] font-medium text-gx-text-disabled uppercase tracking-wider bg-gx-bg-tertiary">
								Local (Ollama)
							</div>
							{#each ollamaModels as model (model.id)}
								<button
									onclick={() => selectModel(model.id)}
									class="w-full px-2 py-1.5 text-left text-[11px] hover:bg-gx-bg-hover flex items-center gap-2 {chatStore.selectedModel === model.id ? 'text-gx-neon bg-gx-neon/5' : 'text-gx-text-primary'}"
								>
									<Cpu size={10} class="text-green-400 shrink-0" />
									<span class="truncate">{model.name}</span>
								</button>
							{/each}
						{/if}
						{#if hasApiKey}
							<div class="px-2 py-1 text-[9px] font-medium text-gx-text-disabled uppercase tracking-wider bg-gx-bg-tertiary">
								Cloud (OpenRouter)
							</div>
							{#each cloudModels as model (model.id)}
								<button
									onclick={() => selectModel(model.id)}
									class="w-full px-2 py-1.5 text-left text-[11px] hover:bg-gx-bg-hover flex items-center gap-2 {chatStore.selectedModel === model.id ? 'text-gx-neon bg-gx-neon/5' : 'text-gx-text-primary'}"
								>
									<Cloud size={10} class="text-blue-400 shrink-0" />
									<span class="truncate">{model.name}</span>
								</button>
							{/each}
						{/if}
						{#if ollamaModels.length === 0 && !hasApiKey}
							<div class="px-3 py-3 text-[10px] text-gx-text-muted text-center">
								No backends. Start Ollama or add API key in Settings.
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<!-- HyperChat: Model Routing Cascade Viz -->
		{#if chatStore.isStreaming}
			<div class="px-2 border-b border-gx-border-default">
				<ModelRoutingViz isStreaming={chatStore.isStreaming} />
			</div>
		{/if}

		<!-- Pipeline above messages (original viz level) -->
		{#if vizLevel === 'pipeline' && chatStore.isStreaming}
			<div class="px-3 py-2 border-b border-gx-border-default">
				<ModelPipelineView />
			</div>
		{/if}

		<!-- HyperChat: Token Stream Viz during generation -->
		{#if chatStore.isStreaming}
			<div class="px-2 border-b border-gx-border-default">
				<TokenStreamViz isStreaming={chatStore.isStreaming} />
			</div>
		{/if}

		<div bind:this={messagesContainer} class="flex-1 overflow-y-auto px-3 py-3 space-y-2">
			{#if activeMessages.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<AgentCharacter agentName="AI" state="sleeping" size="lg" />
					<p class="text-xs text-center">Chat + IDE + Terminal — unified workspace</p>
				</div>
			{:else}
				{#each activeMessages as msg (msg.id)}
					<EnhancedChatMessage message={msg} compact={true} />
				{/each}
			{/if}
			<!-- Typing indicator — visible when streaming starts with empty content -->
			{#if chatStore.isStreaming && activeMessages.length > 0 && activeMessages[activeMessages.length - 1]?.content === ''}
				<div class="flex items-center gap-2 px-3 py-2 text-xs text-gx-text-muted">
					<span class="inline-flex gap-0.5">
						<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 0ms"></span>
						<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 150ms"></span>
						<span class="w-1.5 h-1.5 rounded-full bg-gx-neon animate-bounce" style="animation-delay: 300ms"></span>
					</span>
					<span class="italic">ImpForge is thinking...</span>
				</div>
			{/if}
		</div>

		<!-- Quick Actions Bar -->
		<div class="flex items-center gap-1.5 px-3 py-1.5 border-t border-gx-border-default/50">
			<button
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				title="Attach file"
			>
				<span class="text-xs">&#128206;</span>
				<span>Attach</span>
			</button>
			<button
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				title="Take screenshot"
			>
				<span class="text-xs">&#128247;</span>
				<span>Screenshot</span>
			</button>
			<button
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				title="Voice input"
			>
				<span class="text-xs">&#127908;</span>
				<span>Voice</span>
			</button>
			<button
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-gx-text-muted hover:text-gx-neon hover:bg-gx-neon/5 transition-colors"
				title="Mixture of Agents"
			>
				<span class="text-xs">&#10024;</span>
				<span>MoA</span>
			</button>
		</div>

		<ChatInput onSend={handleSend} isStreaming={chatStore.isStreaming} placeholder="Chat with your code..." />
	</div>
</div>
