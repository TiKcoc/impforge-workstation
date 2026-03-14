<script lang="ts">
	import { tick } from 'svelte';
	import { Search, Terminal, File, FolderOpen, Hash } from '@lucide/svelte';
	import { ide, type FileEntry } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import Fuse from 'fuse.js';

	// BenikUI style engine
	const widgetId = 'ide-command-palette';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let overlayComp = $derived(styleEngine.getComponentStyle(widgetId, 'overlay'));
	let overlayStyle = $derived(hasEngineStyle && overlayComp ? componentToCSS(overlayComp) : '');
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let searchInputComp = $derived(styleEngine.getComponentStyle(widgetId, 'search-input'));
	let searchInputStyle = $derived(hasEngineStyle && searchInputComp ? componentToCSS(searchInputComp) : '');
	let resultItemComp = $derived(styleEngine.getComponentStyle(widgetId, 'result-item'));
	let resultItemStyle = $derived(hasEngineStyle && resultItemComp ? componentToCSS(resultItemComp) : '');

	interface Props {
		open: boolean;
		mode: 'file' | 'command';
		onClose: () => void;
		onSelectFile?: (path: string, name: string) => void;
		onExecuteCommand?: (command: string) => void;
	}

	let { open, mode, onClose, onSelectFile, onExecuteCommand }: Props = $props();

	// Internal state
	let query = $state('');
	let selectedIndex = $state(0);
	let inputEl = $state<HTMLInputElement | null>(null);
	let listEl = $state<HTMLDivElement | null>(null);

	// Command definitions
	interface Command {
		id: string;
		label: string;
		shortcut: string;
		category: string;
	}

	const commands: Command[] = [
		{ id: 'save', label: 'Save File', shortcut: 'Ctrl+S', category: 'File' },
		{ id: 'saveAll', label: 'Save All Files', shortcut: 'Ctrl+Shift+S', category: 'File' },
		{ id: 'toggleTerminal', label: 'Toggle Terminal', shortcut: 'Ctrl+`', category: 'View' },
		{ id: 'toggleAi', label: 'Toggle AI Panel', shortcut: '', category: 'View' },
		{ id: 'toggleExplorer', label: 'Toggle File Explorer', shortcut: 'Ctrl+B', category: 'View' },
		{ id: 'formatDocument', label: 'Format Document', shortcut: 'Shift+Alt+F', category: 'Edit' },
		{ id: 'goToLine', label: 'Go to Line', shortcut: 'Ctrl+G', category: 'Navigate' },
		{ id: 'findInFiles', label: 'Find in Files', shortcut: 'Ctrl+Shift+F', category: 'Search' },
		{ id: 'newFile', label: 'New File', shortcut: '', category: 'File' },
		{ id: 'newTerminal', label: 'New Terminal', shortcut: '', category: 'Terminal' },
		{ id: 'gitStatus', label: 'Git: Show Status', shortcut: '', category: 'Git' },
		{ id: 'gitCommit', label: 'Git: Commit', shortcut: '', category: 'Git' },
		{ id: 'reloadWindow', label: 'Reload Window', shortcut: '', category: 'Developer' },
	];

	// Collect all known files for file mode search
	function collectAllFiles(): FileEntry[] {
		const all: FileEntry[] = [...ide.files];
		for (const [, entries] of ide.subDirFiles) {
			all.push(...entries);
		}
		return all.filter((f) => !f.is_dir);
	}

	// Fuse instances for fuzzy search
	const fileFuse = $derived(
		new Fuse(collectAllFiles(), {
			keys: ['name', 'path'],
			threshold: 0.4,
			includeScore: true,
		})
	);

	const commandFuse = $derived(
		new Fuse(commands, {
			keys: ['label', 'category'],
			threshold: 0.3,
			includeScore: true,
		})
	);

	// Filtered results based on mode and query
	const fileResults = $derived.by(() => {
		const searchTerm = query.trim();
		if (!searchTerm) {
			// Show recently opened or all files when no query
			return collectAllFiles().slice(0, 10);
		}
		return fileFuse.search(searchTerm).map((r) => r.item).slice(0, 10);
	});

	const commandResults = $derived.by(() => {
		const searchTerm = query.trim();
		if (!searchTerm) {
			return commands;
		}
		return commandFuse.search(searchTerm).map((r) => r.item).slice(0, 10);
	});

	const resultCount = $derived(mode === 'file' ? fileResults.length : commandResults.length);

	// File icon based on extension
	function getFileIcon(ext: string | null): string {
		const icons: Record<string, string> = {
			rs: '🦀', py: '🐍', ts: '💎', js: '⚡', svelte: '🔥',
			json: '{}', toml: '⚙️', md: '📝', css: '🎨', html: '🌐',
			yaml: '📋', yml: '📋', sh: '🐚', sql: '🗃️',
		};
		return icons[ext?.toLowerCase() || ''] || '📄';
	}

	// Extract parent directory for display
	function getParentDir(path: string): string {
		const parts = path.split('/');
		if (parts.length >= 2) {
			return parts.slice(-2, -1)[0] || '';
		}
		return '';
	}

	// Reset state when palette opens
	$effect(() => {
		if (open) {
			query = '';
			selectedIndex = 0;
			tick().then(() => {
				inputEl?.focus();
			});
		}
	});

	// Clamp selected index when results change
	$effect(() => {
		if (selectedIndex >= resultCount) {
			selectedIndex = Math.max(0, resultCount - 1);
		}
	});

	// Scroll selected item into view
	$effect(() => {
		if (listEl && resultCount > 0) {
			const items = listEl.querySelectorAll('[data-palette-item]');
			const selected = items[selectedIndex];
			if (selected) {
				selected.scrollIntoView({ block: 'nearest' });
			}
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
			return;
		}

		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = (selectedIndex + 1) % Math.max(1, resultCount);
			return;
		}

		if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = (selectedIndex - 1 + Math.max(1, resultCount)) % Math.max(1, resultCount);
			return;
		}

		if (e.key === 'Enter') {
			e.preventDefault();
			selectCurrent();
			return;
		}
	}

	function selectCurrent() {
		if (mode === 'file') {
			const file = fileResults[selectedIndex];
			if (file) {
				onSelectFile?.(file.path, file.name);
				onClose();
			}
		} else {
			const cmd = commandResults[selectedIndex];
			if (cmd) {
				onExecuteCommand?.(cmd.id);
				onClose();
			}
		}
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}
</script>

{#if open}
	<!-- Backdrop -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/50 backdrop-blur-sm"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
		style={overlayStyle}
	>
		<!-- Palette container -->
		<div class="w-[500px] max-h-[400px] flex flex-col {hasEngineStyle ? '' : 'bg-gx-bg-primary'} border border-gx-border-default rounded-lg shadow-2xl shadow-black/50 overflow-hidden" style={containerStyle}>
			<!-- Input row -->
			<div class="flex items-center gap-2 px-3 py-2.5 border-b border-gx-border-default" style={searchInputStyle}>
				{#if mode === 'command'}
					<Terminal size={16} class="text-gx-neon shrink-0" />
				{:else}
					<Search size={16} class="text-gx-text-muted shrink-0" />
				{/if}
				<input
					bind:this={inputEl}
					bind:value={query}
					type="text"
					placeholder={mode === 'command' ? 'Type a command...' : 'Search files by name...'}
					class="flex-1 bg-transparent text-sm text-gx-text-primary placeholder:text-gx-text-muted outline-none"
				/>
				{#if mode === 'command'}
					<span class="text-[10px] text-gx-text-muted shrink-0 select-none">commands</span>
				{:else}
					<span class="text-[10px] text-gx-text-muted shrink-0 select-none">files</span>
				{/if}
			</div>

			<!-- Results list -->
			<div bind:this={listEl} class="flex-1 overflow-auto py-1">
				{#if mode === 'file'}
					{#if fileResults.length === 0}
						<div class="px-3 py-6 text-center text-xs text-gx-text-muted">
							No files found
						</div>
					{:else}
						{#each fileResults as file, i}
							<button
								data-palette-item
								onclick={() => { selectedIndex = i; selectCurrent(); }}
								onmouseenter={() => selectedIndex = i}
								class="flex w-full items-center gap-2 px-3 py-1.5 mx-1 rounded cursor-pointer transition-colors text-left
									{i === selectedIndex
										? 'bg-gx-neon/10 border-l-2 border-l-gx-neon'
										: 'border-l-2 border-l-transparent hover:bg-gx-bg-hover'}"
							>
								<span class="text-xs shrink-0">{getFileIcon(file.extension)}</span>
								<span class="text-sm text-gx-text-primary truncate">{file.name}</span>
								<span class="text-[11px] text-gx-text-muted truncate ml-auto">{getParentDir(file.path)}</span>
							</button>
						{/each}
					{/if}
				{:else}
					{#if commandResults.length === 0}
						<div class="px-3 py-6 text-center text-xs text-gx-text-muted">
							No commands found
						</div>
					{:else}
						{@const groupedCommands = groupByCategory(commandResults)}
						{#each Object.entries(groupedCommands) as [category, cmds]}
							<div class="px-3 pt-2 pb-0.5">
								<span class="text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">{category}</span>
							</div>
							{#each cmds as cmd}
								{@const globalIdx = commandResults.indexOf(cmd)}
								<button
									data-palette-item
									onclick={() => { selectedIndex = globalIdx; selectCurrent(); }}
									onmouseenter={() => selectedIndex = globalIdx}
									class="flex w-full items-center gap-2 px-3 py-1.5 mx-1 rounded cursor-pointer transition-colors text-left
										{globalIdx === selectedIndex
											? 'bg-gx-neon/10 border-l-2 border-l-gx-neon'
											: 'border-l-2 border-l-transparent hover:bg-gx-bg-hover'}"
								>
									<Hash size={12} class="text-gx-text-muted shrink-0" />
									<span class="text-sm text-gx-text-primary">{cmd.label}</span>
									{#if cmd.shortcut}
										<span class="ml-auto bg-gx-bg-elevated text-gx-text-muted text-[10px] px-1.5 py-0.5 rounded font-mono">{cmd.shortcut}</span>
									{/if}
								</button>
							{/each}
						{/each}
					{/if}
				{/if}
			</div>

			<!-- Footer hints -->
			<div class="flex items-center gap-3 px-3 py-1.5 border-t border-gx-border-default text-[10px] text-gx-text-muted select-none shrink-0">
				<span><kbd class="bg-gx-bg-elevated px-1 rounded">↑↓</kbd> navigate</span>
				<span><kbd class="bg-gx-bg-elevated px-1 rounded">↵</kbd> select</span>
				<span><kbd class="bg-gx-bg-elevated px-1 rounded">esc</kbd> close</span>
			</div>
		</div>
	</div>
{/if}

<script lang="ts" module>
	/** Group commands by category while preserving array order */
	function groupByCategory(cmds: Array<{ id: string; label: string; shortcut: string; category: string }>): Record<string, typeof cmds> {
		const groups: Record<string, typeof cmds> = {};
		for (const cmd of cmds) {
			if (!groups[cmd.category]) {
				groups[cmd.category] = [];
			}
			groups[cmd.category].push(cmd);
		}
		return groups;
	}
</script>
