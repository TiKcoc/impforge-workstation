<script lang="ts">
	import {
		FolderOpen, FolderClosed, Search, ChevronRight,
		ChevronDown, ArrowUp, RotateCcw, Loader2
	} from '@lucide/svelte';
	import { ide, type FileEntry } from '$lib/stores/ide.svelte';
	import Fuse from 'fuse.js';

	let searchQuery = $state('');
	let searchMode = $state(false);
	let fuseResults = $state<FileEntry[]>([]);

	const fuse = $derived(
		new Fuse(collectAllFiles(), {
			keys: ['name', 'path'],
			threshold: 0.4,
			includeScore: true,
		})
	);

	function collectAllFiles(): FileEntry[] {
		const all: FileEntry[] = [...ide.files];
		for (const [, entries] of ide.subDirFiles) {
			all.push(...entries);
		}
		return all.filter((f) => !f.is_dir);
	}

	function handleSearch() {
		if (!searchQuery.trim()) {
			fuseResults = [];
			return;
		}
		fuseResults = fuse.search(searchQuery).map((r) => r.item).slice(0, 20);
	}

	function goUp() {
		const parent = ide.currentDir.split('/').slice(0, -1).join('/') || '/';
		ide.loadDirectory(parent);
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

<div class="flex flex-col h-full bg-[#0d1117] overflow-hidden">
	<!-- Header -->
	<div class="flex items-center gap-1 px-2 py-1.5 border-b border-white/5 shrink-0">
		<span class="text-[11px] font-semibold text-white/40 uppercase tracking-wider">Explorer</span>
		<div class="flex-1"></div>
		<button onclick={() => searchMode = !searchMode} class="p-0.5 text-white/40 hover:text-[#00FF66]">
			<Search size={12} />
		</button>
		<button onclick={goUp} class="p-0.5 text-white/40 hover:text-[#00FF66]">
			<ArrowUp size={12} />
		</button>
		<button onclick={() => ide.loadDirectory(ide.currentDir)} class="p-0.5 text-white/40 hover:text-[#00FF66]">
			<RotateCcw size={12} />
		</button>
	</div>

	<!-- Fuzzy search -->
	{#if searchMode}
		<div class="px-2 py-1 border-b border-white/5">
			<input
				type="text"
				bind:value={searchQuery}
				oninput={handleSearch}
				placeholder="Search files..."
				class="w-full bg-[#161b22] border border-white/10 rounded px-2 py-1 text-xs text-white/90 placeholder:text-white/30 outline-none focus:border-[#00FF66]"
			/>
		</div>
		{#if fuseResults.length > 0}
			<div class="max-h-40 overflow-auto border-b border-white/5">
				{#each fuseResults as entry}
					<button
						onclick={() => ide.openFile(entry.path, entry.name)}
						class="flex items-center gap-1.5 w-full px-3 py-1 text-xs hover:bg-white/5 text-left"
					>
						<span class="text-[10px] shrink-0">{getFileIcon(entry)}</span>
						<span class="text-[#89ddff] truncate">{entry.name}</span>
						<span class="text-[9px] text-white/30 truncate ml-auto">{entry.path.split('/').slice(-2, -1)[0]}</span>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	<!-- File tree -->
	<div class="flex-1 overflow-auto text-xs">
		{#if ide.loading}
			<div class="flex items-center justify-center py-8">
				<Loader2 size={16} class="animate-spin text-white/30" />
			</div>
		{:else}
			{#each ide.files as entry}
				{@render fileTreeEntry(entry, 0)}
			{/each}
		{/if}
	</div>

	<!-- Current path -->
	<div class="px-2 py-1 border-t border-white/5 shrink-0">
		<span class="text-[10px] text-white/30 truncate block">{ide.currentDir}</span>
	</div>
</div>

{#snippet fileTreeEntry(entry: FileEntry, depth: number)}
	<button
		onclick={() => entry.is_dir ? ide.toggleDir(entry) : ide.openFile(entry.path, entry.name)}
		class="flex items-center gap-1.5 w-full px-2 py-1 hover:bg-white/5 text-left group"
		style="padding-left: {8 + depth * 16}px"
	>
		{#if entry.is_dir}
			{#if ide.expandedDirs.has(entry.path)}
				<ChevronDown size={12} class="text-white/30 shrink-0" />
				<FolderOpen size={14} class="text-orange-400 shrink-0" />
			{:else}
				<ChevronRight size={12} class="text-white/30 shrink-0" />
				<FolderClosed size={14} class="text-orange-400 shrink-0" />
			{/if}
			<span class="text-white/70 truncate">{entry.name}</span>
		{:else}
			<span class="w-3 shrink-0"></span>
			<span class="text-[10px] shrink-0">{getFileIcon(entry)}</span>
			<span class="text-white/70 truncate">{entry.name}</span>
			<span class="ml-auto text-[10px] text-white/30 opacity-0 group-hover:opacity-100">
				{formatSize(entry.size)}
			</span>
		{/if}
	</button>

	{#if entry.is_dir && ide.expandedDirs.has(entry.path)}
		{#each ide.subDirFiles.get(entry.path) || [] as subEntry}
			{@render fileTreeEntry(subEntry, depth + 1)}
		{/each}
	{/if}
{/snippet}
