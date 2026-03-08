<script lang="ts">
	import { onMount } from 'svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import {
		Code2, FolderOpen, FolderClosed, Search,
		Terminal, Bot, X, ChevronRight, ChevronDown,
		Play, Send, Loader2, ArrowUp, RotateCcw
	} from '@lucide/svelte';
	import { ide, type FileEntry } from '$lib/stores/ide.svelte';

	// Panel state
	let bottomPanel = $state<'terminal' | 'agent'>('agent');
	let sidebarWidth = $state(240);
	let bottomPanelHeight = $state(280);
	let searchQuery = $state('');
	let agentInput = $state('');
	let terminalInput = $state('');
	let showSearch = $state(false);

	// Monaco editor references
	let editorContainer = $state<HTMLDivElement>(undefined!);
	let monacoEditor: any = null;
	let monacoModule: any = null;

	onMount(() => {
		// Load initial directory
		ide.loadDirectory('/home');

		// Dynamically import Monaco Editor (not SSR-compatible)
		import('monaco-editor').then((mod) => {
			monacoModule = mod;
			monacoModule.editor.defineTheme('impforge-dark', {
				base: 'vs-dark',
				inherit: true,
				rules: [
					{ token: 'comment', foreground: '5a6a7a', fontStyle: 'italic' },
					{ token: 'keyword', foreground: 'c792ea' },
					{ token: 'string', foreground: 'c3e88d' },
					{ token: 'number', foreground: 'f78c6c' },
					{ token: 'type', foreground: 'ffcb6b' },
					{ token: 'function', foreground: '82aaff' },
					{ token: 'variable', foreground: 'eeffff' },
				],
				colors: {
					'editor.background': '#0a0e14',
					'editor.foreground': '#e0e0e0',
					'editor.lineHighlightBackground': '#141820',
					'editor.selectionBackground': '#1a3a5c',
					'editorCursor.foreground': '#00FF66',
					'editorLineNumber.foreground': '#3a4a5a',
					'editorLineNumber.activeForeground': '#00FF66',
					'editor.selectionHighlightBackground': '#1a3a5c55',
					'editorIndentGuide.background': '#1a1e28',
					'editorIndentGuide.activeBackground': '#2a3a4a',
				},
			});
		}).catch((e) => {
			console.error('Failed to load Monaco:', e);
		});

		return () => {
			if (monacoEditor) monacoEditor.dispose();
		};
	});

	// Create or update Monaco editor when active tab changes
	$effect(() => {
		const tab = ide.activeTab;
		if (tab && editorContainer && monacoModule) {
			if (monacoEditor) {
				const model = monacoEditor.getModel();
				if (model) {
					const currentValue = model.getValue();
					if (currentValue !== tab.content) model.setValue(tab.content);
					monacoModule.editor.setModelLanguage(model, tab.language);
				}
			} else {
				monacoEditor = monacoModule.editor.create(editorContainer, {
					value: tab.content,
					language: tab.language,
					theme: 'impforge-dark',
					fontSize: 13,
					fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
					fontLigatures: true,
					minimap: { enabled: true, maxColumn: 80 },
					scrollBeyondLastLine: false,
					smoothScrolling: true,
					cursorBlinking: 'smooth',
					cursorSmoothCaretAnimation: 'on',
					renderLineHighlight: 'all',
					bracketPairColorization: { enabled: true },
					automaticLayout: true,
					padding: { top: 8 },
					lineNumbers: 'on',
					wordWrap: 'on',
					tabSize: 2,
				});

				monacoEditor.onDidChangeModelContent(() => {
					const newContent = monacoEditor.getModel()?.getValue() || '';
					ide.updateTabContent(ide.activeTabIndex, newContent);
				});

				monacoEditor.addCommand(
					monacoModule.KeyMod.CtrlCmd | monacoModule.KeyCode.KeyS,
					() => ide.saveFile(ide.activeTabIndex)
				);
			}
		}
	});

	function goUp() {
		const parent = ide.currentDir.split('/').slice(0, -1).join('/') || '/';
		ide.loadDirectory(parent);
	}

	async function handleSearch() {
		if (searchQuery.trim()) await ide.searchFiles(searchQuery);
	}

	async function handleTerminalInput(e: KeyboardEvent) {
		if (e.key === 'Enter' && terminalInput.trim()) {
			await ide.executeCommand(terminalInput);
			terminalInput = '';
		}
	}

	async function handleAgentSend() {
		if (agentInput.trim() && !ide.agentLoading) {
			const msg = agentInput;
			agentInput = '';
			await ide.sendAgentMessage(msg);
		}
	}

	function handleAgentKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleAgentSend();
		}
	}

	function getFileIcon(entry: FileEntry): string {
		if (entry.is_dir) return '';
		const ext = entry.extension?.toLowerCase() || '';
		const icons: Record<string, string> = {
			rs: '🦀', py: '🐍', ts: '💎', js: '⚡', svelte: '🔥',
			json: '{}', toml: '⚙️', md: '📝', css: '🎨', html: '🌐',
		};
		return icons[ext] || '📄';
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes}B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
		return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
	}
</script>

<div class="flex flex-col h-full overflow-hidden">
	<!-- IDE Top Bar -->
	<div class="flex items-center h-9 px-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0 gap-2">
		<Code2 size={16} class="text-gx-neon" />
		<span class="text-sm font-semibold text-gx-neon">CodeForge</span>
		<Separator orientation="vertical" class="h-4 bg-gx-border-default" />
		<span class="text-xs text-gx-text-muted truncate">{ide.currentDir}</span>
		<div class="flex-1"></div>
		<button onclick={() => showSearch = !showSearch} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors">
			<Search size={14} />
		</button>
	</div>

	<!-- Search bar (collapsible) -->
	{#if showSearch}
		<div class="flex items-center gap-2 px-2 py-1.5 bg-gx-bg-tertiary border-b border-gx-border-default">
			<Search size={14} class="text-gx-text-muted shrink-0" />
			<input
				type="text"
				bind:value={searchQuery}
				onkeydown={(e) => e.key === 'Enter' && handleSearch()}
				placeholder="Search in files..."
				class="flex-1 bg-transparent text-sm text-gx-text-primary placeholder:text-gx-text-muted outline-none"
			/>
			<button onclick={handleSearch} class="text-xs px-2 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-gx-text-muted hover:text-gx-neon hover:border-gx-neon transition-all">
				Search
			</button>
		</div>

		{#if ide.searchResults.length > 0}
			<div class="max-h-32 overflow-auto bg-gx-bg-tertiary border-b border-gx-border-default">
				{#each ide.searchResults.slice(0, 20) as result}
					<button
						onclick={() => ide.openFile(result.file, result.file.split('/').pop() || '')}
						class="flex items-center gap-2 w-full px-3 py-1 text-xs hover:bg-gx-bg-hover text-left"
					>
						<span class="text-gx-text-muted shrink-0">L{result.line}</span>
						<span class="text-gx-accent-cyan truncate">{result.file.split('/').pop()}</span>
						<span class="text-gx-text-secondary truncate">{result.content}</span>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	<!-- Main IDE area -->
	<div class="flex flex-1 min-h-0 overflow-hidden">
		<!-- File Tree Sidebar -->
		<div class="flex flex-col bg-gx-bg-secondary border-r border-gx-border-default shrink-0 overflow-hidden" style="width: {sidebarWidth}px">
			<div class="flex items-center gap-1 px-2 py-1.5 border-b border-gx-border-default">
				<span class="text-[11px] font-semibold text-gx-text-muted uppercase tracking-wider">Explorer</span>
				<div class="flex-1"></div>
				<button onclick={goUp} class="p-0.5 text-gx-text-muted hover:text-gx-neon"><ArrowUp size={12} /></button>
				<button onclick={() => ide.loadDirectory(ide.currentDir)} class="p-0.5 text-gx-text-muted hover:text-gx-neon"><RotateCcw size={12} /></button>
			</div>

			<div class="flex-1 overflow-auto text-xs">
				{#if ide.loading}
					<div class="flex items-center justify-center py-8"><Loader2 size={16} class="animate-spin text-gx-text-muted" /></div>
				{:else}
					{#each ide.files as entry}
						{@render fileTreeEntry(entry, 0)}
					{/each}
				{/if}
			</div>
		</div>

		<!-- Editor + Bottom Panel -->
		<div class="flex flex-col flex-1 min-w-0">
			<!-- Tab bar -->
			<div class="flex items-center h-8 bg-gx-bg-secondary border-b border-gx-border-default overflow-x-auto shrink-0">
				{#each ide.openTabs as tab, i}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						onclick={() => ide.activeTabIndex = i}
						onkeydown={(e) => e.key === 'Enter' && (ide.activeTabIndex = i)}
						role="tab"
						tabindex="0"
						class="flex items-center gap-1.5 px-3 h-full text-xs border-r border-gx-border-default shrink-0 cursor-pointer
							{i === ide.activeTabIndex
								? 'bg-gx-bg-primary text-gx-text-primary border-t-2 border-t-gx-neon'
								: 'text-gx-text-muted hover:bg-gx-bg-hover'}"
					>
						{#if tab.modified}
							<span class="w-2 h-2 rounded-full bg-gx-accent-orange shrink-0"></span>
						{/if}
						<span class="truncate max-w-[120px]">{tab.name}</span>
						<button
							onclick={(e) => { e.stopPropagation(); ide.closeTab(i); }}
							class="ml-1 p-0.5 rounded hover:bg-gx-bg-elevated text-gx-text-muted hover:text-gx-text-primary"
						>
							<X size={12} />
						</button>
					</div>
				{/each}
			</div>

			<!-- Editor area -->
			<div class="flex-1 min-h-0 relative" style="min-height: 100px">
				{#if ide.openTabs.length > 0}
					<div bind:this={editorContainer} class="absolute inset-0"></div>
				{:else}
					<div class="flex flex-col items-center justify-center h-full text-gx-text-muted gap-4">
						<Code2 size={48} class="opacity-20" />
						<div class="text-center">
							<p class="text-sm font-medium">CodeForge IDE</p>
							<p class="text-xs mt-1">Open a file from the explorer or ask the AI agent</p>
						</div>
						<div class="flex flex-col gap-1 text-xs text-gx-text-muted mt-2">
							<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+S</kbd> Save file</span>
							<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+K</kbd> Command palette</span>
						</div>
					</div>
				{/if}
			</div>

			<!-- Bottom Panel -->
			<div class="shrink-0 border-t border-gx-border-default" style="height: {bottomPanelHeight}px">
				<div class="flex items-center h-7 bg-gx-bg-secondary border-b border-gx-border-default px-1">
					<button
						onclick={() => bottomPanel = 'agent'}
						class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
							{bottomPanel === 'agent' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
					>
						<Bot size={13} />
						AI Agent
					</button>
					<button
						onclick={() => bottomPanel = 'terminal'}
						class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
							{bottomPanel === 'terminal' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
					>
						<Terminal size={13} />
						Terminal
					</button>
				</div>

				<div class="flex flex-col h-[calc(100%-28px)] overflow-hidden">
					{#if bottomPanel === 'agent'}
						<!-- AI Agent Chat -->
						<div class="flex-1 overflow-auto p-2 space-y-2">
							{#if ide.agentMessages.length === 0}
								<div class="text-center py-6">
									<Bot size={32} class="mx-auto text-gx-text-muted opacity-30 mb-2" />
									<p class="text-xs text-gx-text-muted">AI coding agent ready. It can read, write, and search your files.</p>
									<div class="flex flex-wrap gap-1.5 justify-center mt-3">
										{#each ['Read this file', 'Find TODO comments', 'Explain this code', 'Create a test'] as suggestion}
											<button
												onclick={() => { agentInput = suggestion; handleAgentSend(); }}
												class="text-[11px] px-2 py-1 bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/50 transition-all"
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
										<div class="max-w-[80%] px-3 py-1.5 rounded-gx bg-gx-neon/10 border border-gx-neon/20 text-xs text-gx-text-primary">
											{msg.content}
										</div>
									{:else if msg.role === 'tool'}
										<div class="max-w-[90%] px-2 py-1 rounded bg-gx-bg-tertiary border border-gx-border-default text-[11px] font-mono">
											<div class="flex items-center gap-1 text-gx-accent-cyan mb-0.5">
												<Play size={10} />
												{msg.toolCall?.tool}
											</div>
											<pre class="text-gx-text-muted whitespace-pre-wrap max-h-24 overflow-auto">{msg.content.slice(0, 500)}{msg.content.length > 500 ? '...' : ''}</pre>
										</div>
									{:else}
										<div class="max-w-[90%] px-3 py-1.5 rounded-gx bg-gx-bg-elevated border border-gx-border-default text-xs text-gx-text-secondary">
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

						<div class="flex items-end gap-2 px-2 py-1.5 border-t border-gx-border-default bg-gx-bg-secondary">
							<textarea
								bind:value={agentInput}
								onkeydown={handleAgentKeydown}
								placeholder="Ask the AI agent... (Enter to send)"
								rows="1"
								class="flex-1 bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-2 py-1.5 text-xs text-gx-text-primary placeholder:text-gx-text-muted resize-none outline-none focus:border-gx-neon transition-colors"
							></textarea>
							<button
								onclick={handleAgentSend}
								disabled={ide.agentLoading || !agentInput.trim()}
								class="p-1.5 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
							>
								<Send size={14} />
							</button>
						</div>
					{:else}
						<!-- Terminal -->
						<div class="flex-1 overflow-auto p-2 font-mono text-xs">
							<pre class="text-gx-text-secondary whitespace-pre-wrap">{ide.terminalOutput || `CodeForge Terminal\nType commands below. Working directory: ${ide.currentDir}\n`}</pre>
						</div>
						<div class="flex items-center gap-2 px-2 py-1.5 border-t border-gx-border-default bg-gx-bg-secondary">
							<span class="text-xs text-gx-neon font-mono">$</span>
							<input
								type="text"
								bind:value={terminalInput}
								onkeydown={handleTerminalInput}
								placeholder="Enter command..."
								class="flex-1 bg-gx-bg-tertiary border border-gx-border-default rounded px-2 py-1 text-xs font-mono text-gx-text-primary placeholder:text-gx-text-muted outline-none focus:border-gx-neon transition-colors"
							/>
						</div>
					{/if}
				</div>
			</div>
		</div>
	</div>
</div>

<!-- File tree entry snippet (recursive) -->
{#snippet fileTreeEntry(entry: FileEntry, depth: number)}
	<button
		onclick={() => entry.is_dir ? ide.toggleDir(entry) : ide.openFile(entry.path, entry.name)}
		class="flex items-center gap-1.5 w-full px-2 py-1 hover:bg-gx-bg-hover text-left group"
		style="padding-left: {8 + depth * 16}px"
	>
		{#if entry.is_dir}
			{#if ide.expandedDirs.has(entry.path)}
				<ChevronDown size={12} class="text-gx-text-muted shrink-0" />
				<FolderOpen size={14} class="text-gx-accent-orange shrink-0" />
			{:else}
				<ChevronRight size={12} class="text-gx-text-muted shrink-0" />
				<FolderClosed size={14} class="text-gx-accent-orange shrink-0" />
			{/if}
			<span class="text-gx-text-secondary truncate">{entry.name}</span>
		{:else}
			<span class="w-3 shrink-0"></span>
			<span class="text-[10px] shrink-0">{getFileIcon(entry)}</span>
			<span class="text-gx-text-secondary truncate">{entry.name}</span>
			<span class="ml-auto text-[10px] text-gx-text-muted opacity-0 group-hover:opacity-100">{formatSize(entry.size)}</span>
		{/if}
	</button>

	{#if entry.is_dir && ide.expandedDirs.has(entry.path)}
		{#each ide.subDirFiles.get(entry.path) || [] as subEntry}
			{@render fileTreeEntry(subEntry, depth + 1)}
		{/each}
	{/if}
{/snippet}
