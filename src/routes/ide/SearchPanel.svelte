<script lang="ts">
	/**
	 * SearchPanel — Find in Files with BenikUI integration
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - search-input: Query input area
	 *   - results-list: Search results
	 *   - result-item: Individual result row
	 */

	import { invoke } from '@tauri-apps/api/core';
	import { Search, FileText, X, Loader2, FolderOpen } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface SearchMatch {
		file: string;
		line: number;
		content: string;
	}

	interface Props {
		onNavigate?: (filePath: string, line: number) => void;
	}

	let { onNavigate }: Props = $props();

	// BenikUI style engine
	const widgetId = 'ide-search-panel';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let inputComp = $derived(styleEngine.getComponentStyle(widgetId, 'search-input'));
	let inputStyle = $derived(hasEngineStyle && inputComp ? componentToCSS(inputComp) : '');
	let resultsComp = $derived(styleEngine.getComponentStyle(widgetId, 'results-list'));
	let resultsStyle = $derived(hasEngineStyle && resultsComp ? componentToCSS(resultsComp) : '');

	let query = $state('');
	let results = $state<SearchMatch[]>([]);
	let searching = $state(false);
	let searchDone = $state(false);
	let debounceTimer: ReturnType<typeof setTimeout> | null = null;

	// Group results by file
	let groupedResults = $derived(() => {
		const groups = new Map<string, SearchMatch[]>();
		for (const r of results) {
			const existing = groups.get(r.file) || [];
			existing.push(r);
			groups.set(r.file, existing);
		}
		return groups;
	});

	function handleInput() {
		if (debounceTimer) clearTimeout(debounceTimer);
		if (!query.trim()) {
			results = [];
			searchDone = false;
			return;
		}
		debounceTimer = setTimeout(doSearch, 300);
	}

	async function doSearch() {
		if (!query.trim() || !ide.currentDir) return;
		searching = true;
		searchDone = false;
		try {
			results = await invoke<SearchMatch[]>('ide_search_files', {
				pattern: query,
				searchPath: ide.currentDir,
				maxResults: 100,
			});
		} catch (e) {
			console.error('Search failed:', e);
			results = [];
		}
		searching = false;
		searchDone = true;
	}

	function handleResultClick(match: SearchMatch) {
		const name = match.file.split('/').pop() || match.file;
		ide.openFile(match.file, name);
		onNavigate?.(match.file, match.line);
	}

	function shortPath(fullPath: string): string {
		if (!ide.currentDir) return fullPath;
		return fullPath.replace(ide.currentDir + '/', '');
	}

	function highlightMatch(content: string): string {
		if (!query.trim()) return content;
		const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		return content.replace(new RegExp(`(${escaped})`, 'gi'), '<mark class="bg-gx-neon/30 text-gx-neon rounded px-0.5">$1</mark>');
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Search input -->
	<div class="px-2 py-2 border-b border-gx-border-subtle shrink-0" style={inputStyle}>
		<div class="flex items-center gap-1.5 px-2 py-1.5 bg-gx-bg-secondary border border-gx-border-default rounded focus-within:border-gx-neon transition-colors">
			<Search size={12} class="text-gx-text-muted shrink-0" />
			<input
				bind:value={query}
				oninput={handleInput}
				onkeydown={(e) => { if (e.key === 'Enter') doSearch(); }}
				placeholder="Search in files..."
				class="flex-1 bg-transparent text-xs text-gx-text-primary placeholder:text-gx-text-disabled outline-none"
			/>
			{#if searching}
				<Loader2 size={12} class="text-gx-neon animate-spin shrink-0" />
			{/if}
			{#if query}
				<button onclick={() => { query = ''; results = []; searchDone = false; }} class="p-0.5 text-gx-text-muted hover:text-gx-text-secondary">
					<X size={10} />
				</button>
			{/if}
		</div>
		{#if searchDone}
			<p class="text-[10px] text-gx-text-disabled mt-1 px-1">
				{results.length} result{results.length !== 1 ? 's' : ''} in {groupedResults().size} file{groupedResults().size !== 1 ? 's' : ''}
			</p>
		{/if}
	</div>

	<!-- Results -->
	<div class="flex-1 overflow-auto text-xs" style={resultsStyle}>
		{#if results.length > 0}
			{#each [...groupedResults().entries()] as [file, matches]}
				<div class="border-b border-gx-border-subtle">
					<div class="flex items-center gap-1.5 px-2 py-1 bg-gx-bg-secondary/50 sticky top-0">
						<FileText size={11} class="text-gx-accent-cyan shrink-0" />
						<span class="text-gx-text-secondary font-medium truncate" title={file}>{shortPath(file)}</span>
						<span class="text-gx-text-disabled ml-auto shrink-0">{matches.length}</span>
					</div>
					{#each matches as match}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							onclick={() => handleResultClick(match)}
							onkeydown={(e) => e.key === 'Enter' && handleResultClick(match)}
							role="button"
							tabindex="0"
							class="flex items-start gap-2 px-4 py-1 hover:bg-gx-bg-hover cursor-pointer"
						>
							<span class="text-gx-text-disabled font-mono w-8 text-right shrink-0">{match.line}</span>
							<span class="text-gx-text-muted truncate font-mono">{@html highlightMatch(match.content)}</span>
						</div>
					{/each}
				</div>
			{/each}
		{:else if searchDone && query}
			<div class="p-4 text-center text-gx-text-disabled">
				<Search size={24} class="mx-auto mb-2 opacity-30" />
				<p>No results for "{query}"</p>
			</div>
		{:else if !query}
			<div class="p-4 text-center text-gx-text-disabled">
				<FolderOpen size={24} class="mx-auto mb-2 opacity-30" />
				<p>Type to search across files</p>
				<p class="text-[10px] mt-1">Searches in {ide.currentDir || '~'}</p>
			</div>
		{/if}
	</div>
</div>
