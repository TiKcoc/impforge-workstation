<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Code2, Bot, Terminal, AlertTriangle, GitBranch, X, Bug, Users, Crown, Palette, Search, List, Database, Globe, Sparkles, FileText, Network, Loader2 } from '@lucide/svelte';
	import { Pane, PaneGroup, Handle } from '$lib/components/ui/resizable/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-ide';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let toolbarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'toolbar'));
	let toolbarStyle = $derived(hasEngineStyle && toolbarComponent ? componentToCSS(toolbarComponent) : '');
	let sidebarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'sidebar'));
	let sidebarStyle = $derived(hasEngineStyle && sidebarComponent ? componentToCSS(sidebarComponent) : '');
	let editorComponent = $derived(styleEngine.getComponentStyle(widgetId, 'editor'));
	let editorStyle = $derived(hasEngineStyle && editorComponent ? componentToCSS(editorComponent) : '');
	let terminalComponent = $derived(styleEngine.getComponentStyle(widgetId, 'terminal'));
	let terminalStyle = $derived(hasEngineStyle && terminalComponent ? componentToCSS(terminalComponent) : '');
	import FileExplorer from './FileExplorer.svelte';
	import CodeEditor from './CodeEditor.svelte';
	import IdeTerminal from './IdeTerminal.svelte';
	import GitPanel from './GitPanel.svelte';
	import AiAgent from './AiAgent.svelte';
	import MultiAgentPanel from './MultiAgentPanel.svelte';
	import IdeStatusBar from './IdeStatusBar.svelte';
	import CommandPalette from './CommandPalette.svelte';
	import ProblemsPanel from './ProblemsPanel.svelte';
	import DebugPanel from './DebugPanel.svelte';
	import CollabPanel from './CollabPanel.svelte';
	import PricingPanel from './PricingPanel.svelte';
	import ThemeImporter from './ThemeImporter.svelte';
	import SearchPanel from './SearchPanel.svelte';
	import SymbolOutline from './SymbolOutline.svelte';
	import DbClientPanel from './DbClientPanel.svelte';
	import HttpClientPanel from './HttpClientPanel.svelte';
	import SpecDrivenPanel from './SpecDrivenPanel.svelte';
	import CodeGraphPanel from './CodeGraphPanel.svelte';

	// Component refs
	let codeEditorRef = $state<CodeEditor | undefined>(undefined);

	// Panel visibility
	let showAiPanel = $state(true);
	let showExplorer = $state(true);
	let sidebarView = $state<'explorer' | 'outline'>('explorer');
	let bottomPanel = $state<'terminal' | 'problems' | 'git' | 'debug' | 'search' | 'database' | 'http' | 'graph'>('terminal');
	let rightPanel = $state<'ai' | 'multi-agent' | 'collab' | 'pricing' | 'themes' | 'spec'>('multi-agent');
	let cursorLine = $state(1);
	let cursorCol = $state(1);
	let showGoToLine = $state(false);
	let goToLineInput = $state('');
	let initialLoading = $state(true);

	// Command palette state
	let showCommandPalette = $state(false);
	let paletteMode = $state<'file' | 'command'>('file');

	onMount(async () => {
		await ide.loadDirectory('/home');
		initialLoading = false;
		document.addEventListener('keydown', handleGlobalKeydown);
	});

	onDestroy(() => {
		document.removeEventListener('keydown', handleGlobalKeydown);
	});

	function handleGlobalKeydown(e: KeyboardEvent) {
		// Ctrl+Shift+P: Command palette
		if (e.ctrlKey && e.shiftKey && e.key === 'P') {
			e.preventDefault();
			paletteMode = 'command';
			showCommandPalette = true;
			return;
		}
		// Ctrl+P: File search palette
		if (e.ctrlKey && !e.shiftKey && e.key === 'p') {
			e.preventDefault();
			paletteMode = 'file';
			showCommandPalette = true;
			return;
		}
		// Ctrl+B: Toggle file explorer
		if (e.ctrlKey && !e.shiftKey && e.key === 'b') {
			e.preventDefault();
			showExplorer = !showExplorer;
			return;
		}
		// Ctrl+H: Find & Replace (Monaco built-in)
		if (e.ctrlKey && !e.shiftKey && e.key === 'h') {
			e.preventDefault();
			if (codeEditorRef) {
				const editor = codeEditorRef.getEditor();
				if (editor) editor.getAction('editor.action.startFindReplaceAction')?.run();
			}
			return;
		}
		// Ctrl+F: Find (Monaco built-in)
		if (e.ctrlKey && !e.shiftKey && e.key === 'f') {
			e.preventDefault();
			if (codeEditorRef) {
				const editor = codeEditorRef.getEditor();
				if (editor) editor.getAction('actions.find')?.run();
			}
			return;
		}
		// Ctrl+Shift+F: Format Document
		if (e.ctrlKey && e.shiftKey && e.key === 'F') {
			e.preventDefault();
			if (codeEditorRef) {
				const editor = codeEditorRef.getEditor();
				if (editor) editor.getAction('editor.action.formatDocument')?.run();
			}
			return;
		}
		// Ctrl+G: Go to line
		if (e.ctrlKey && !e.shiftKey && e.key === 'g') {
			e.preventDefault();
			showGoToLine = true;
			goToLineInput = '';
			return;
		}
		// Escape: Close go-to-line dialog
		if (e.key === 'Escape' && showGoToLine) {
			showGoToLine = false;
			return;
		}
	}

	function handlePaletteClose() {
		showCommandPalette = false;
	}

	function handleFileSelect(path: string, name: string) {
		ide.openFile(path, name);
	}

	function handleExecuteCommand(command: string) {
		switch (command) {
			case 'save':
				ide.saveFile(ide.activeTabIndex);
				break;
			case 'saveAll':
				for (let i = 0; i < ide.openTabs.length; i++) {
					if (ide.openTabs[i].modified) ide.saveFile(i);
				}
				break;
			case 'toggleTerminal':
				bottomPanel = bottomPanel === 'terminal' ? 'problems' : 'terminal';
				break;
			case 'toggleAi':
				showAiPanel = !showAiPanel;
				break;
			case 'toggleExplorer':
				showExplorer = !showExplorer;
				break;
			case 'formatDocument':
				// Trigger Monaco's built-in format action (works with LSP formatters if registered)
				if (codeEditorRef) {
					const editor = codeEditorRef.getEditor();
					if (editor) {
						editor.getAction('editor.action.formatDocument')?.run();
					}
				}
				break;
			case 'goToLine':
				showGoToLine = true;
				goToLineInput = '';
				break;
			case 'findInFiles':
				bottomPanel = 'search';
				break;
			case 'newFile':
				{
					const dir = ide.currentDir || '/home';
					const name = `untitled-${Date.now()}.ts`;
					ide.openFile(`${dir}/${name}`, name);
				}
				break;
			case 'newTerminal':
				bottomPanel = 'terminal';
				break;
			case 'gitStatus':
				bottomPanel = 'git';
				break;
			case 'gitCommit':
				bottomPanel = 'git';
				break;
			case 'reloadWindow':
				window.location.reload();
				break;
			case 'toggleDebug':
				bottomPanel = bottomPanel === 'debug' ? 'terminal' : 'debug';
				break;
			case 'toggleCollab':
				rightPanel = 'collab';
				showAiPanel = true;
				break;
			case 'toggleThemes':
				rightPanel = 'themes';
				showAiPanel = true;
				break;
			default:
				break;
		}
	}

	function handleProblemsNavigate(filePath: string, line: number, col: number) {
		// Open the file and later the editor can scroll to line:col
		const name = filePath.split('/').pop() || filePath;
		ide.openFile(filePath, name);
	}

	function handleCursorChange(line: number, col: number) {
		cursorLine = line;
		cursorCol = col;
	}
</script>

{#if initialLoading}
<div class="flex items-center justify-center h-full bg-gx-bg-primary">
	<Loader2 class="w-8 h-8 animate-spin text-gx-neon" />
	<span class="ml-3 text-gx-text-muted text-sm">Loading CodeForge...</span>
</div>
{:else}
<div class="flex flex-col h-full overflow-hidden" style={containerStyle}>
	<!-- IDE Top Bar -->
	<div class="{hasEngineStyle && toolbarComponent ? '' : 'bg-gx-bg-primary'} flex items-center h-9 px-2 border-b border-gx-border-default shrink-0 gap-2" style={toolbarStyle}>
		<Code2 size={16} class="text-gx-neon" />
		<span class="text-sm font-semibold text-gx-neon">CodeForge</span>
		<Separator orientation="vertical" class="h-4 bg-gx-border-default" />
		<span class="text-xs text-gx-text-muted truncate">{ide.currentDir}</span>
		<div class="flex-1"></div>
		<button onclick={() => { rightPanel = 'spec'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'spec' && showAiPanel ? 'text-gx-neon' : ''}" title="Spec-Driven Dev"><FileText size={14} /></button>
		<button onclick={() => { rightPanel = 'multi-agent'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'multi-agent' && showAiPanel ? 'text-gx-neon' : ''}" title="Multi-Agent"><Sparkles size={14} /></button>
		<button onclick={() => { rightPanel = 'ai'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'ai' && showAiPanel ? 'text-gx-neon' : ''}" title="AI Agent"><Bot size={14} /></button>
		<button onclick={() => { rightPanel = 'collab'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'collab' && showAiPanel ? 'text-gx-neon' : ''}" title="Collaboration"><Users size={14} /></button>
		<button onclick={() => { rightPanel = 'themes'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'themes' && showAiPanel ? 'text-gx-neon' : ''}" title="Themes"><Palette size={14} /></button>
		<button onclick={() => { rightPanel = 'pricing'; showAiPanel = true; }} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors {rightPanel === 'pricing' && showAiPanel ? 'text-gx-neon' : ''}" title="Subscription"><Crown size={14} /></button>
	</div>

	<!-- Main IDE Layout -->
	<div class="flex-1 min-h-0">
		<PaneGroup direction="horizontal">
			<!-- Sidebar (Explorer / Outline) — toggleable via Ctrl+B -->
			{#if showExplorer}
				<Pane defaultSize={18} minSize={12} maxSize={30}>
					<div class="flex flex-col h-full">
						<!-- Sidebar tab selector -->
						<div class="flex items-center h-7 bg-gx-bg-primary border-b border-gx-border-default px-1 shrink-0">
							<button
								onclick={() => sidebarView = 'explorer'}
								class="flex items-center gap-1 px-2 h-full text-xs transition-colors
									{sidebarView === 'explorer' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
							>
								Explorer
							</button>
							<button
								onclick={() => sidebarView = 'outline'}
								class="flex items-center gap-1 px-2 h-full text-xs transition-colors
									{sidebarView === 'outline' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
							>
								<List size={11} />
								Outline
							</button>
						</div>
						<div class="flex-1 min-h-0">
							{#if sidebarView === 'outline'}
								<SymbolOutline onNavigate={(line) => {
									if (codeEditorRef) {
										const editor = codeEditorRef.getEditor();
										if (editor) {
											editor.revealLineInCenter(line);
											editor.setPosition({ lineNumber: line, column: 1 });
											editor.focus();
										}
									}
								}} />
							{:else}
								<FileExplorer />
							{/if}
						</div>
					</div>
				</Pane>

				<Handle />
			{/if}

			<!-- Editor + Bottom Panel -->
			<Pane defaultSize={showAiPanel ? 57 : 82} minSize={30}>
				<PaneGroup direction="vertical">
					<!-- Editor area -->
					<Pane defaultSize={65} minSize={20}>
						<div class="flex flex-col h-full">
							<!-- Tab bar -->
							<div class="flex items-center h-8 bg-gx-bg-primary border-b border-gx-border-default overflow-x-auto shrink-0">
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
											<span class="w-2 h-2 rounded-full bg-gx-status-warning shrink-0"></span>
										{/if}
										<span class="truncate max-w-[120px]">{tab.name}</span>
										<button
											onclick={(e) => { e.stopPropagation(); ide.closeTab(i); }}
											class="ml-1 p-0.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary"
										>
											<X size={12} />
										</button>
									</div>
								{/each}
							</div>

							<!-- Monaco Editor -->
							<CodeEditor bind:this={codeEditorRef} onCursorChange={handleCursorChange} />
						</div>
					</Pane>

					<Handle />

					<!-- Bottom Panel (Terminal / Problems) -->
					<Pane defaultSize={35} minSize={10} maxSize={60}>
						<div class="flex flex-col h-full">
							<div class="flex items-center h-7 bg-gx-bg-primary border-b border-gx-border-default px-1 shrink-0">
								<button
									onclick={() => bottomPanel = 'terminal'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'terminal' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Terminal size={13} />
									Terminal
								</button>
								<button
									onclick={() => bottomPanel = 'problems'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'problems' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<AlertTriangle size={13} />
									Problems
								</button>
								<button
									onclick={() => bottomPanel = 'git'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'git' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<GitBranch size={13} />
									Git
								</button>
								<button
									onclick={() => bottomPanel = 'debug'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'debug' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Bug size={13} />
									Debug
								</button>
								<button
									onclick={() => bottomPanel = 'search'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'search' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Search size={13} />
									Search
								</button>
								<button
									onclick={() => bottomPanel = 'database'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'database' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Database size={13} />
									DB
								</button>
								<button
									onclick={() => bottomPanel = 'http'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'http' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Globe size={13} />
									HTTP
								</button>
								<button
									onclick={() => bottomPanel = 'graph'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'graph' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
								>
									<Network size={13} />
									Graph
								</button>
							</div>

							<div class="flex-1 min-h-0">
								{#if bottomPanel === 'terminal'}
									<IdeTerminal />
								{:else if bottomPanel === 'git'}
									<GitPanel />
								{:else if bottomPanel === 'debug'}
									<DebugPanel />
								{:else if bottomPanel === 'search'}
									<SearchPanel onNavigate={(filePath, line) => handleProblemsNavigate(filePath, line, 1)} />
								{:else if bottomPanel === 'database'}
									<DbClientPanel />
								{:else if bottomPanel === 'http'}
									<HttpClientPanel />
								{:else if bottomPanel === 'graph'}
									<CodeGraphPanel />
								{:else}
									<ProblemsPanel onNavigate={handleProblemsNavigate} />
								{/if}
							</div>
						</div>
					</Pane>
				</PaneGroup>
			</Pane>

			<!-- Right Panel (collapsible) -->
			{#if showAiPanel}
				<Handle />
				<Pane defaultSize={25} minSize={15} maxSize={40}>
					{#if rightPanel === 'spec'}
						<SpecDrivenPanel />
					{:else if rightPanel === 'multi-agent'}
						<MultiAgentPanel />
					{:else if rightPanel === 'ai'}
						<AiAgent />
					{:else if rightPanel === 'collab'}
						<CollabPanel />
					{:else if rightPanel === 'themes'}
						<ThemeImporter />
					{:else if rightPanel === 'pricing'}
						<PricingPanel />
					{/if}
				</Pane>
			{/if}
		</PaneGroup>
	</div>

	<!-- Status Bar -->
	<IdeStatusBar cursorLine={cursorLine} cursorCol={cursorCol} />
</div>

<!-- Command Palette Overlay -->
<CommandPalette
	open={showCommandPalette}
	mode={paletteMode}
	onClose={handlePaletteClose}
	onSelectFile={handleFileSelect}
	onExecuteCommand={handleExecuteCommand}
/>

<!-- Go to Line Dialog -->
{#if showGoToLine}
	<div class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]" role="dialog" aria-label="Go to line">
		<button class="absolute inset-0 bg-black/30 cursor-default" onclick={() => showGoToLine = false} aria-label="Close dialog"></button>
		<div class="relative bg-gx-bg-secondary border border-gx-border-default rounded-lg shadow-2xl w-72 p-3">
			<label for="goto-line-input" class="text-xs text-gx-text-muted mb-1.5 block">Go to Line</label>
			<input
				id="goto-line-input"
				type="number"
				min="1"
				bind:value={goToLineInput}
				onkeydown={(e) => {
					if (e.key === 'Enter' && goToLineInput) {
						cursorLine = parseInt(goToLineInput) || 1;
						cursorCol = 1;
						showGoToLine = false;
					}
					if (e.key === 'Escape') showGoToLine = false;
				}}
				placeholder="Line number..."
				class="w-full px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono"
			/>
		</div>
	</div>
{/if}
{/if}
