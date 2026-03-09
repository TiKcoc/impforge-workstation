<script lang="ts">
	import { onMount } from 'svelte';
	import { Code2, Bot, Terminal, AlertTriangle, X } from '@lucide/svelte';
	import { Pane, PaneGroup, Handle } from '$lib/components/ui/resizable/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { ide } from '$lib/stores/ide.svelte';
	import FileExplorer from './FileExplorer.svelte';
	import CodeEditor from './CodeEditor.svelte';
	import IdeTerminal from './IdeTerminal.svelte';
	import AiAgent from './AiAgent.svelte';
	import IdeStatusBar from './IdeStatusBar.svelte';

	// Panel visibility
	let showAiPanel = $state(true);
	let bottomPanel = $state<'terminal' | 'problems'>('terminal');
	let cursorLine = $state(1);
	let cursorCol = $state(1);

	onMount(() => {
		ide.loadDirectory('/home');
	});

	function handleCursorChange(line: number, col: number) {
		cursorLine = line;
		cursorCol = col;
	}
</script>

<div class="flex flex-col h-full overflow-hidden">
	<!-- IDE Top Bar -->
	<div class="flex items-center h-9 px-2 bg-[#0d1117] border-b border-white/5 shrink-0 gap-2">
		<Code2 size={16} class="text-[#00FF66]" />
		<span class="text-sm font-semibold text-[#00FF66]">CodeForge</span>
		<Separator orientation="vertical" class="h-4 bg-white/10" />
		<span class="text-xs text-white/40 truncate">{ide.currentDir}</span>
		<div class="flex-1"></div>
		<button
			onclick={() => showAiPanel = !showAiPanel}
			class="p-1 text-white/40 hover:text-[#00FF66] transition-colors"
			title="Toggle AI Panel"
		>
			<Bot size={14} />
		</button>
	</div>

	<!-- Main IDE Layout -->
	<div class="flex-1 min-h-0">
		<PaneGroup direction="horizontal">
			<!-- File Explorer -->
			<Pane defaultSize={18} minSize={12} maxSize={30}>
				<FileExplorer />
			</Pane>

			<Handle />

			<!-- Editor + Bottom Panel -->
			<Pane defaultSize={showAiPanel ? 57 : 82} minSize={30}>
				<PaneGroup direction="vertical">
					<!-- Editor area -->
					<Pane defaultSize={65} minSize={20}>
						<div class="flex flex-col h-full">
							<!-- Tab bar -->
							<div class="flex items-center h-8 bg-[#0d1117] border-b border-white/5 overflow-x-auto shrink-0">
								{#each ide.openTabs as tab, i}
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<div
										onclick={() => ide.activeTabIndex = i}
										onkeydown={(e) => e.key === 'Enter' && (ide.activeTabIndex = i)}
										role="tab"
										tabindex="0"
										class="flex items-center gap-1.5 px-3 h-full text-xs border-r border-white/5 shrink-0 cursor-pointer
											{i === ide.activeTabIndex
												? 'bg-[#0a0e14] text-white/90 border-t-2 border-t-[#00FF66]'
												: 'text-white/40 hover:bg-white/5'}"
									>
										{#if tab.modified}
											<span class="w-2 h-2 rounded-full bg-orange-400 shrink-0"></span>
										{/if}
										<span class="truncate max-w-[120px]">{tab.name}</span>
										<button
											onclick={(e) => { e.stopPropagation(); ide.closeTab(i); }}
											class="ml-1 p-0.5 rounded hover:bg-white/10 text-white/30 hover:text-white/70"
										>
											<X size={12} />
										</button>
									</div>
								{/each}
							</div>

							<!-- Monaco Editor -->
							<CodeEditor onCursorChange={handleCursorChange} />
						</div>
					</Pane>

					<Handle />

					<!-- Bottom Panel (Terminal / Problems) -->
					<Pane defaultSize={35} minSize={10} maxSize={60}>
						<div class="flex flex-col h-full">
							<div class="flex items-center h-7 bg-[#0d1117] border-b border-white/5 px-1 shrink-0">
								<button
									onclick={() => bottomPanel = 'terminal'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'terminal' ? 'text-[#00FF66] border-b border-[#00FF66]' : 'text-white/40 hover:text-white/60'}"
								>
									<Terminal size={13} />
									Terminal
								</button>
								<button
									onclick={() => bottomPanel = 'problems'}
									class="flex items-center gap-1.5 px-2.5 h-full text-xs transition-colors
										{bottomPanel === 'problems' ? 'text-[#00FF66] border-b border-[#00FF66]' : 'text-white/40 hover:text-white/60'}"
								>
									<AlertTriangle size={13} />
									Problems
								</button>
							</div>

							<div class="flex-1 min-h-0">
								{#if bottomPanel === 'terminal'}
									<IdeTerminal />
								{:else}
									<div class="p-3 text-xs text-white/40">
										No problems detected.
									</div>
								{/if}
							</div>
						</div>
					</Pane>
				</PaneGroup>
			</Pane>

			<!-- AI Agent Panel (collapsible) -->
			{#if showAiPanel}
				<Handle />
				<Pane defaultSize={25} minSize={15} maxSize={40}>
					<AiAgent />
				</Pane>
			{/if}
		</PaneGroup>
	</div>

	<!-- Status Bar -->
	<IdeStatusBar
		lspStatus="disconnected"
		gitBranch=""
		aiModel="Ollama"
	/>
</div>
