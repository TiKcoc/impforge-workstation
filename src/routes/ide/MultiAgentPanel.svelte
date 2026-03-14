<script lang="ts">
	/**
	 * MultiAgentPanel — Concurrent AI Agent Sessions with BenikUI integration
	 *
	 * Enables running multiple AI agents simultaneously, each with their own
	 * conversation context, role, and tool access. JetBrains AI and Cursor
	 * only support single sessions — this gives ImpForge a competitive edge.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - session-tab: Individual session tab
	 *   - session-list: Session switcher sidebar
	 *   - agent-header: Session header with role + model info
	 */

	import { invoke } from '@tauri-apps/api/core';
	import {
		Bot, Plus, X, Send, Loader2, Sparkles, Code2,
		Bug, FileSearch, Pencil, Zap, ChevronDown
	} from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface AgentMessage {
		role: 'user' | 'assistant' | 'tool' | 'system';
		content: string;
		toolCall?: { tool: string; args: string };
		timestamp: number;
	}

	interface AgentSession {
		id: string;
		name: string;
		role: AgentRole;
		messages: AgentMessage[];
		loading: boolean;
		model: string;
		createdAt: number;
	}

	type AgentRole = 'coder' | 'reviewer' | 'debugger' | 'researcher' | 'architect' | 'custom';

	// BenikUI style engine
	const widgetId = 'ide-multi-agent';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let tabComp = $derived(styleEngine.getComponentStyle(widgetId, 'session-tab'));
	let tabStyle = $derived(hasEngineStyle && tabComp ? componentToCSS(tabComp) : '');
	let headerComp = $derived(styleEngine.getComponentStyle(widgetId, 'agent-header'));
	let headerStyle = $derived(hasEngineStyle && headerComp ? componentToCSS(headerComp) : '');

	// State
	let sessions = $state<AgentSession[]>([]);
	let activeSessionId = $state('');
	let input = $state('');
	let showNewSession = $state(false);
	let newSessionRole = $state<AgentRole>('coder');

	let activeSession = $derived(sessions.find(s => s.id === activeSessionId));

	const roleConfig: Record<AgentRole, { label: string; icon: typeof Bot; color: string; systemPrompt: string }> = {
		coder: {
			label: 'Coder',
			icon: Code2,
			color: 'text-gx-neon',
			systemPrompt: 'You are an expert coding agent. Write clean, efficient code. Use the available tools to read, modify, and test files.',
		},
		reviewer: {
			label: 'Reviewer',
			icon: FileSearch,
			color: 'text-gx-accent-cyan',
			systemPrompt: 'You are a code reviewer. Analyze code for bugs, security issues, performance problems, and style violations. Be thorough but constructive.',
		},
		debugger: {
			label: 'Debugger',
			icon: Bug,
			color: 'text-gx-status-error',
			systemPrompt: 'You are a debugging specialist. Systematically identify root causes of issues. Use tools to inspect files, run tests, and trace execution paths.',
		},
		researcher: {
			label: 'Researcher',
			icon: FileSearch,
			color: 'text-gx-accent-magenta',
			systemPrompt: 'You are a research agent. Find relevant documentation, code patterns, and best practices. Summarize findings concisely.',
		},
		architect: {
			label: 'Architect',
			icon: Zap,
			color: 'text-gx-status-warning',
			systemPrompt: 'You are a software architect. Design systems, plan implementations, evaluate trade-offs. Focus on scalability, maintainability, and correctness.',
		},
		custom: {
			label: 'Custom',
			icon: Sparkles,
			color: 'text-gx-text-secondary',
			systemPrompt: 'You are a helpful AI assistant with access to the codebase.',
		},
	};

	function createSession(role: AgentRole) {
		const id = `session_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
		const config = roleConfig[role];
		const session: AgentSession = {
			id,
			name: `${config.label} ${sessions.filter(s => s.role === role).length + 1}`,
			role,
			messages: [{
				role: 'system',
				content: config.systemPrompt,
				timestamp: Date.now(),
			}],
			loading: false,
			model: 'auto',
			createdAt: Date.now(),
		};
		sessions = [...sessions, session];
		activeSessionId = id;
		showNewSession = false;
	}

	function closeSession(id: string) {
		sessions = sessions.filter(s => s.id !== id);
		if (activeSessionId === id) {
			activeSessionId = sessions[0]?.id || '';
		}
	}

	async function sendMessage() {
		if (!input.trim() || !activeSession) return;
		const msg = input.trim();
		input = '';

		const sessionIdx = sessions.findIndex(s => s.id === activeSessionId);
		if (sessionIdx === -1) return;

		// Add user message
		sessions[sessionIdx].messages = [
			...sessions[sessionIdx].messages,
			{ role: 'user', content: msg, timestamp: Date.now() },
		];
		sessions[sessionIdx].loading = true;
		sessions = [...sessions]; // trigger reactivity

		try {
			// Use the existing IDE agent tool call system
			const result = await invoke<string>('ide_agent_tool_call', {
				input: msg,
				context: sessions[sessionIdx].messages
					.filter(m => m.role !== 'system')
					.slice(-10)
					.map(m => `${m.role}: ${m.content}`)
					.join('\n'),
			});

			sessions[sessionIdx].messages = [
				...sessions[sessionIdx].messages,
				{ role: 'assistant', content: result, timestamp: Date.now() },
			];
		} catch (e) {
			sessions[sessionIdx].messages = [
				...sessions[sessionIdx].messages,
				{ role: 'assistant', content: `Error: ${e}`, timestamp: Date.now() },
			];
		}

		sessions[sessionIdx].loading = false;
		sessions = [...sessions];
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			sendMessage();
		}
	}

	// Auto-create first session
	$effect(() => {
		if (sessions.length === 0) {
			createSession('coder');
		}
	});
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Session tabs -->
	<div class="flex items-center gap-0.5 px-1 py-1 border-b border-gx-border-subtle shrink-0 overflow-x-auto">
		{#each sessions as session}
			{@const config = roleConfig[session.role]}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				onclick={() => { activeSessionId = session.id; }}
				onkeydown={(e) => e.key === 'Enter' && (activeSessionId = session.id)}
				role="tab"
				tabindex="0"
				aria-selected={activeSessionId === session.id}
				class="flex items-center gap-1 px-2 py-0.5 text-[10px] rounded shrink-0 transition-colors cursor-pointer
					{activeSessionId === session.id
						? 'bg-gx-bg-elevated border border-gx-neon/30 text-gx-text-primary'
						: 'text-gx-text-muted hover:bg-gx-bg-hover border border-transparent'}"
				style={tabStyle}
			>
				<config.icon size={10} class={config.color} />
				<span class="truncate max-w-20">{session.name}</span>
				{#if session.loading}
					<Loader2 size={8} class="animate-spin text-gx-neon" />
				{/if}
				<button
					onclick={(e) => { e.stopPropagation(); closeSession(session.id); }}
					class="ml-0.5 p-0.5 rounded hover:bg-gx-bg-secondary text-gx-text-disabled hover:text-gx-text-muted"
				>
					<X size={8} />
				</button>
			</div>
		{/each}

		<!-- New session button -->
		<div class="relative">
			<button
				onclick={() => { showNewSession = !showNewSession; }}
				class="p-1 text-gx-text-muted hover:text-gx-neon rounded hover:bg-gx-bg-hover"
				title="New agent session"
			>
				<Plus size={12} />
			</button>

			{#if showNewSession}
				<div class="absolute top-full left-0 mt-1 bg-gx-bg-elevated border border-gx-border-default rounded shadow-lg z-50 py-1 min-w-32">
					{#each Object.entries(roleConfig) as [role, config]}
						<button
							onclick={() => createSession(role as AgentRole)}
							class="flex items-center gap-2 w-full px-3 py-1.5 text-xs hover:bg-gx-bg-hover text-left"
						>
							<config.icon size={12} class={config.color} />
							<span class="text-gx-text-secondary">{config.label}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	<!-- Active session content -->
	{#if activeSession}
		{@const activeConfig = roleConfig[activeSession.role]}
		<!-- Session header -->
		<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-subtle/50 shrink-0" style={headerStyle}>
			<activeConfig.icon size={12} class={activeConfig.color} />
			<span class="text-xs font-medium text-gx-text-primary">{activeSession.name}</span>
			<span class="text-[9px] px-1.5 py-0.5 rounded bg-gx-bg-secondary text-gx-text-disabled">{activeSession.role}</span>
			<span class="text-[9px] text-gx-text-disabled ml-auto">{activeSession.messages.filter(m => m.role !== 'system').length} msgs</span>
		</div>

		<!-- Messages -->
		<div class="flex-1 overflow-auto p-2 space-y-2">
			{#each activeSession.messages.filter(m => m.role !== 'system') as msg}
				<div class="flex gap-2 {msg.role === 'user' ? 'justify-end' : ''}">
					{#if msg.role === 'user'}
						<div class="max-w-[85%] px-3 py-1.5 rounded bg-gx-neon/10 border border-gx-neon/20 text-xs text-gx-text-primary">
							{msg.content}
						</div>
					{:else if msg.role === 'tool'}
						<div class="max-w-[95%] px-2 py-1 rounded bg-gx-bg-secondary border border-gx-border-default text-[11px] font-mono">
							<pre class="text-gx-text-muted whitespace-pre-wrap max-h-24 overflow-auto">{msg.content.slice(0, 500)}{msg.content.length > 500 ? '...' : ''}</pre>
						</div>
					{:else}
						<div class="max-w-[95%] px-3 py-1.5 rounded bg-gx-bg-secondary border border-gx-border-default text-xs text-gx-text-secondary">
							<pre class="whitespace-pre-wrap font-sans">{msg.content}</pre>
						</div>
					{/if}
				</div>
			{/each}

			{#if activeSession.loading}
				<div class="flex items-center gap-2 text-xs text-gx-text-muted">
					<Loader2 size={12} class="animate-spin" />
					<span>Thinking...</span>
				</div>
			{/if}
		</div>

		<!-- Input -->
		<div class="flex items-end gap-2 px-2 py-1.5 border-t border-gx-border-default shrink-0">
			<textarea
				bind:value={input}
				onkeydown={handleKeydown}
				placeholder="Message {activeSession.name}..."
				rows="1"
				class="flex-1 bg-gx-bg-secondary border border-gx-border-default rounded px-2 py-1.5 text-xs text-gx-text-primary placeholder:text-gx-text-muted resize-none outline-none focus:border-gx-neon transition-colors"
			></textarea>
			<button
				onclick={sendMessage}
				disabled={activeSession.loading || !input.trim()}
				class="p-1.5 rounded bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-30 transition-all"
			>
				<Send size={14} />
			</button>
		</div>
	{:else}
		<div class="flex-1 flex items-center justify-center">
			<div class="text-center text-gx-text-disabled">
				<Bot size={32} class="mx-auto mb-2 opacity-30" />
				<p class="text-xs">No agent sessions</p>
				<button
					onclick={() => createSession('coder')}
					class="mt-2 flex items-center gap-1 mx-auto px-3 py-1 text-xs bg-gx-neon/20 text-gx-neon rounded hover:bg-gx-neon/30"
				>
					<Plus size={12} />
					New Session
				</button>
			</div>
		</div>
	{/if}
</div>
