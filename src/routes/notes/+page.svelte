<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		BookOpen, FilePlus, Save, Search, Loader2, Trash2, AlertCircle, X,
		Pin, PinOff, Archive, Tag, Link2, Sparkles, Brain, Network,
		ChevronRight, CheckCheck, Star, Hash, Eye, ArrowLeft
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-notes';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface NoteMeta {
		id: string;
		title: string;
		tags: string[];
		link_count: number;
		backlink_count: number;
		updated_at: string;
		preview: string;
	}

	interface Note {
		id: string;
		title: string;
		content: string;
		tags: string[];
		links_to: string[];
		linked_from: string[];
		is_pinned: boolean;
		is_archived: boolean;
		word_count: number;
		created_at: string;
		updated_at: string;
	}

	interface TagInfo {
		name: string;
		count: number;
	}

	interface KnowledgeGraphNode {
		id: string;
		title: string;
		tags: string[];
		connections: number;
	}

	interface KnowledgeGraphEdge {
		from: string;
		to: string;
		label: string | null;
	}

	interface KnowledgeGraph {
		nodes: KnowledgeGraphNode[];
		edges: KnowledgeGraphEdge[];
	}

	// ---- State ---------------------------------------------------------------
	let notes = $state<NoteMeta[]>([]);
	let activeNote = $state<Note | null>(null);
	let editorContent = $state('');
	let editorTitle = $state('');
	let tagInput = $state('');
	let editorTags = $state<string[]>([]);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let activeTagFilter = $state<string | null>(null);
	let lastSavedContent = $state('');
	let lastSavedTitle = $state('');
	let saveIndicator = $state<'saved' | 'saving' | 'unsaved' | 'idle'>('idle');
	let autoSaveTimer: ReturnType<typeof setInterval> | null = null;

	// Tags
	let allTags = $state<TagInfo[]>([]);

	// Backlinks
	let backlinks = $state<NoteMeta[]>([]);

	// AI Panel state
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state('');
	let aiError = $state<string | null>(null);
	let aiGenerateTopic = $state('');
	let aiRelatedNotes = $state<NoteMeta[]>([]);
	let aiSummarizeTag = $state('');

	// Graph state
	let showGraph = $state(false);
	let graph = $state<KnowledgeGraph | null>(null);

	// Wiki-link autocomplete
	let showLinkAutocomplete = $state(false);
	let linkQuery = $state('');
	let linkSuggestions = $derived(
		linkQuery.trim()
			? notes.filter(n =>
				n.title.toLowerCase().includes(linkQuery.toLowerCase()) &&
				n.id !== activeNote?.id
			).slice(0, 8)
			: []
	);

	// All known note titles for resolving links in display
	let noteTitleMap = $derived(
		new Map(notes.map(n => [n.id, n.title]))
	);

	// ---- Derived -------------------------------------------------------------
	let filteredNotes = $derived(
		notes.filter(n => {
			if (searchQuery.trim()) {
				const q = searchQuery.toLowerCase();
				return n.title.toLowerCase().includes(q) || n.preview.toLowerCase().includes(q);
			}
			if (activeTagFilter) {
				return n.tags.some(t => t.toLowerCase() === activeTagFilter!.toLowerCase());
			}
			return true;
		})
	);

	let isDirty = $derived(
		editorContent !== lastSavedContent || editorTitle !== lastSavedTitle
	);

	// ---- Data Loading --------------------------------------------------------
	async function loadNotes() {
		loading = true;
		error = null;
		try {
			notes = await invoke<NoteMeta[]>('notes_list', {
				filter: null,
				tag: null,
			});
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	async function loadTags() {
		try {
			allTags = await invoke<TagInfo[]>('notes_get_tags');
		} catch {
			// Non-critical
		}
	}

	async function openNote(id: string) {
		if (activeNote && isDirty) {
			await saveNote();
		}
		try {
			const note = await invoke<Note>('notes_get', { id });
			activeNote = note;
			editorContent = note.content;
			editorTitle = note.title;
			editorTags = [...note.tags];
			lastSavedContent = note.content;
			lastSavedTitle = note.title;
			saveIndicator = 'saved';
			aiResult = '';
			aiError = null;
			aiRelatedNotes = [];
			showGraph = false;
			loadBacklinks(id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function loadBacklinks(id: string) {
		try {
			backlinks = await invoke<NoteMeta[]>('notes_get_backlinks', { id });
		} catch {
			backlinks = [];
		}
	}

	async function createNote() {
		try {
			const note = await invoke<Note>('notes_create', { title: 'Untitled Note' });
			await loadNotes();
			await openNote(note.id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function saveNote() {
		if (!activeNote) return;
		saving = true;
		saveIndicator = 'saving';
		try {
			const updated = await invoke<Note>('notes_save', {
				id: activeNote.id,
				title: editorTitle,
				content: editorContent,
				tags: editorTags,
			});
			activeNote = updated;
			lastSavedContent = editorContent;
			lastSavedTitle = editorTitle;
			saveIndicator = 'saved';
			await loadNotes();
			await loadTags();
			loadBacklinks(updated.id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			saveIndicator = 'unsaved';
		} finally {
			saving = false;
		}
	}

	async function deleteNote(id: string) {
		try {
			await invoke('notes_delete', { id });
			if (activeNote?.id === id) {
				activeNote = null;
				editorContent = '';
				editorTitle = '';
				editorTags = [];
				lastSavedContent = '';
				lastSavedTitle = '';
				saveIndicator = 'idle';
				backlinks = [];
			}
			await loadNotes();
			await loadTags();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function togglePin(id: string, pinned: boolean) {
		try {
			await invoke('notes_pin', { id, pinned });
			if (activeNote?.id === id) {
				activeNote.is_pinned = pinned;
			}
			await loadNotes();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Tag Management ------------------------------------------------------
	function addTag() {
		const t = tagInput.trim();
		if (t && !editorTags.includes(t)) {
			editorTags = [...editorTags, t];
			saveIndicator = 'unsaved';
		}
		tagInput = '';
	}

	function removeTag(tag: string) {
		editorTags = editorTags.filter(t => t !== tag);
		saveIndicator = 'unsaved';
	}

	function handleTagKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			addTag();
		}
	}

	// ---- Wiki-Link Autocomplete ---------------------------------------------
	function handleEditorInput(e: Event) {
		saveIndicator = 'unsaved';
		const textarea = e.target as HTMLTextAreaElement;
		const cursor = textarea.selectionStart;
		const before = editorContent.substring(0, cursor);

		// Check if we are inside [[ ... (no closing ]])
		const lastOpen = before.lastIndexOf('[[');
		const lastClose = before.lastIndexOf(']]');
		if (lastOpen > lastClose) {
			const query = before.substring(lastOpen + 2);
			if (!query.includes('\n')) {
				linkQuery = query;
				showLinkAutocomplete = true;
				return;
			}
		}
		showLinkAutocomplete = false;
		linkQuery = '';
	}

	function insertWikiLink(title: string) {
		const textarea = document.getElementById('notes-editor') as HTMLTextAreaElement | null;
		if (!textarea) return;
		const cursor = textarea.selectionStart;
		const before = editorContent.substring(0, cursor);
		const lastOpen = before.lastIndexOf('[[');
		if (lastOpen >= 0) {
			editorContent =
				editorContent.substring(0, lastOpen) +
				`[[${title}]]` +
				editorContent.substring(cursor);
		}
		showLinkAutocomplete = false;
		linkQuery = '';
		saveIndicator = 'unsaved';
		requestAnimationFrame(() => {
			textarea.focus();
		});
	}

	// ---- AI Features --------------------------------------------------------
	async function aiGenerate() {
		if (!aiGenerateTopic.trim()) return;
		aiLoading = true;
		aiError = null;
		aiResult = '';
		try {
			let context: string | null = null;
			if (activeNote && activeNote.content.trim()) {
				context = `Note: ${activeNote.title}\n${activeNote.content.substring(0, 500)}`;
			}
			aiResult = await invoke<string>('notes_ai_generate', {
				topic: aiGenerateTopic,
				context,
			});
		} catch (e) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	async function aiFindRelated() {
		if (!activeNote) return;
		aiLoading = true;
		aiError = null;
		aiRelatedNotes = [];
		try {
			aiRelatedNotes = await invoke<NoteMeta[]>('notes_ai_connect', {
				id: activeNote.id,
			});
		} catch (e) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	async function aiSummarize() {
		if (!aiSummarizeTag.trim()) return;
		aiLoading = true;
		aiError = null;
		aiResult = '';
		try {
			aiResult = await invoke<string>('notes_ai_summarize_tag', {
				tag: aiSummarizeTag,
			});
		} catch (e) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	function applyAiAsNewNote() {
		if (!aiResult) return;
		editorContent = aiResult;
		saveIndicator = 'unsaved';
		aiResult = '';
	}

	// ---- Graph ---------------------------------------------------------------
	async function loadGraph() {
		try {
			graph = await invoke<KnowledgeGraph>('notes_get_graph');
			showGraph = true;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// Simple force-directed positions (static layout for SVG)
	function computeGraphPositions(g: KnowledgeGraph): { id: string; x: number; y: number; r: number; title: string }[] {
		if (g.nodes.length === 0) return [];
		const W = 500;
		const H = 400;
		const cx = W / 2;
		const cy = H / 2;

		return g.nodes.map((node, i) => {
			const angle = (2 * Math.PI * i) / g.nodes.length;
			const radius = Math.min(W, H) * 0.35;
			const x = cx + radius * Math.cos(angle);
			const y = cy + radius * Math.sin(angle);
			const r = Math.max(8, Math.min(20, 8 + node.connections * 3));
			return { id: node.id, x, y, r, title: node.title };
		});
	}

	let graphPositions = $derived(graph ? computeGraphPositions(graph) : []);

	function getNodePos(id: string): { x: number; y: number } {
		const pos = graphPositions.find(p => p.id === id);
		return pos ? { x: pos.x, y: pos.y } : { x: 250, y: 200 };
	}

	// ---- Editor Keyboard Shortcuts ------------------------------------------
	function handleEditorKeydown(e: KeyboardEvent) {
		const mod = e.ctrlKey || e.metaKey;
		if (mod && e.key === 's') {
			e.preventDefault();
			saveNote();
			return;
		}
		// Tab inserts two spaces
		if (e.key === 'Tab') {
			e.preventDefault();
			const textarea = e.target as HTMLTextAreaElement;
			const start = textarea.selectionStart;
			const end = textarea.selectionEnd;
			editorContent =
				editorContent.substring(0, start) +
				'  ' +
				editorContent.substring(end);
			requestAnimationFrame(() => {
				textarea.setSelectionRange(start + 2, start + 2);
			});
			saveIndicator = 'unsaved';
		}
		// Escape closes autocomplete
		if (e.key === 'Escape' && showLinkAutocomplete) {
			showLinkAutocomplete = false;
		}
	}

	// ---- Utilities -----------------------------------------------------------
	function formatDate(dateStr: string): string {
		try {
			const d = new Date(dateStr);
			const now = new Date();
			const diffMs = now.getTime() - d.getTime();
			const diffMins = Math.floor(diffMs / 60000);
			if (diffMins < 1) return 'Just now';
			if (diffMins < 60) return `${diffMins}m ago`;
			const diffHrs = Math.floor(diffMins / 60);
			if (diffHrs < 24) return `${diffHrs}h ago`;
			const diffDays = Math.floor(diffHrs / 24);
			if (diffDays < 7) return `${diffDays}d ago`;
			return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
		} catch {
			return dateStr;
		}
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadNotes();
		loadTags();
		autoSaveTimer = setInterval(() => {
			if (activeNote && isDirty) {
				saveNote();
			}
		}, 10_000);
	});

	onDestroy(() => {
		if (autoSaveTimer) clearInterval(autoSaveTimer);
	});
</script>

<div class="flex h-full overflow-hidden" style={containerStyle}>
	<!-- ============================================================ -->
	<!-- LEFT SIDEBAR (250px) -->
	<!-- ============================================================ -->
	<div class="flex flex-col w-[250px] border-r border-gx-border-default bg-gx-bg-secondary shrink-0">
		<!-- Header -->
		<div class="flex items-center justify-between px-3 py-3 border-b border-gx-border-default">
			<div class="flex items-center gap-2">
				<BookOpen size={14} class="text-gx-neon" />
				<span class="text-xs font-semibold text-gx-text-primary">ForgeNotes</span>
			</div>
			<button
				onclick={createNote}
				class="flex items-center justify-center w-6 h-6 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all"
				title="New note"
			>
				<FilePlus size={14} />
			</button>
		</div>

		<!-- Search -->
		<div class="px-3 py-2 border-b border-gx-border-default">
			<div class="relative">
				<Search size={12} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
				<input
					type="text"
					bind:value={searchQuery}
					oninput={() => { activeTagFilter = null; }}
					placeholder="Search notes..."
					class="w-full pl-7 pr-2 py-1.5 text-[11px] rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
				/>
			</div>
		</div>

		<!-- Notes List -->
		<div class="flex-1 overflow-y-auto">
			{#if loading}
				<div class="flex items-center justify-center py-8">
					<Loader2 size={16} class="animate-spin text-gx-text-muted" />
				</div>
			{:else if filteredNotes.length === 0}
				<div class="flex flex-col items-center justify-center py-8 px-3 text-center">
					<BookOpen size={28} class="text-gx-text-muted/30 mb-2" />
					<p class="text-[11px] text-gx-text-muted">
						{searchQuery || activeTagFilter ? 'No matching notes' : 'No notes yet'}
					</p>
					{#if !searchQuery && !activeTagFilter}
						<button
							onclick={createNote}
							class="mt-2 text-[11px] text-gx-neon hover:underline"
						>
							Create your first note
						</button>
					{/if}
				</div>
			{:else}
				{#each filteredNotes as note (note.id)}
					<div
						onclick={() => openNote(note.id)}
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') openNote(note.id); }}
						role="button"
						tabindex="0"
						class="w-full text-left px-3 py-2.5 border-b border-gx-border-default/50 transition-all group cursor-pointer
							{activeNote?.id === note.id
								? 'bg-gx-bg-elevated border-l-2 border-l-gx-neon'
								: 'hover:bg-gx-bg-hover'}"
					>
						<div class="flex items-start justify-between gap-1">
							<div class="min-w-0 flex-1">
								<div class="flex items-center gap-1">
									{#if notes.find(n => n.id === note.id) && allTags.length >= 0}
										<!-- Check pinned status from loaded note data -->
									{/if}
									<p class="text-xs font-medium text-gx-text-primary truncate">
										{note.title}
									</p>
								</div>
								<p class="text-[10px] text-gx-text-muted/70 mt-0.5 line-clamp-2 leading-relaxed">
									{note.preview || 'Empty note'}
								</p>
								<div class="flex items-center gap-2 mt-1">
									<span class="text-[10px] text-gx-text-muted">
										{formatDate(note.updated_at)}
									</span>
									{#if note.link_count > 0}
										<span class="text-[10px] text-gx-accent-cyan flex items-center gap-0.5">
											<Link2 size={9} />
											{note.link_count}
										</span>
									{/if}
									{#if note.backlink_count > 0}
										<span class="text-[10px] text-gx-accent-purple flex items-center gap-0.5">
											<ArrowLeft size={9} />
											{note.backlink_count}
										</span>
									{/if}
								</div>
							</div>
							<button
								onclick={(e) => { e.stopPropagation(); deleteNote(note.id); }}
								class="shrink-0 w-5 h-5 flex items-center justify-center rounded text-gx-text-muted/0 group-hover:text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-all"
								title="Delete note"
							>
								<Trash2 size={11} />
							</button>
						</div>
						{#if note.tags.length > 0}
							<div class="flex gap-1 mt-1 flex-wrap">
								{#each note.tags.slice(0, 3) as tag}
									<span class="text-[9px] px-1 py-0 rounded bg-gx-bg-primary text-gx-text-muted border border-gx-border-default">
										#{tag}
									</span>
								{/each}
								{#if note.tags.length > 3}
									<span class="text-[9px] text-gx-text-muted/50">+{note.tags.length - 3}</span>
								{/if}
							</div>
						{/if}
					</div>
				{/each}
			{/if}
		</div>

		<!-- Tags Section -->
		{#if allTags.length > 0}
			<div class="border-t border-gx-border-default px-3 py-2 max-h-36 overflow-y-auto">
				<div class="flex items-center justify-between mb-1.5">
					<span class="text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">Tags</span>
					{#if activeTagFilter}
						<button
							onclick={() => activeTagFilter = null}
							class="text-[9px] text-gx-neon hover:underline"
						>
							Clear
						</button>
					{/if}
				</div>
				<div class="flex flex-wrap gap-1">
					{#each allTags as tag}
						<button
							onclick={() => { activeTagFilter = tag.name; searchQuery = ''; }}
							class="text-[10px] px-1.5 py-0.5 rounded transition-all
								{activeTagFilter === tag.name
									? 'bg-gx-neon/15 text-gx-neon border border-gx-neon/30'
									: 'bg-gx-bg-primary text-gx-text-muted border border-gx-border-default hover:border-gx-neon/30 hover:text-gx-neon'}"
						>
							#{tag.name}
							<span class="text-gx-text-muted/50 ml-0.5">{tag.count}</span>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Sidebar Footer -->
		<div class="flex items-center justify-between px-3 py-2 border-t border-gx-border-default">
			<span class="text-[10px] text-gx-text-muted">
				{notes.length} note{notes.length !== 1 ? 's' : ''}
			</span>
			<button
				onclick={loadGraph}
				class="text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors flex items-center gap-1"
				title="Show Knowledge Graph"
			>
				<Network size={11} />
				Graph
			</button>
		</div>
	</div>

	<!-- ============================================================ -->
	<!-- CENTER - NOTE EDITOR -->
	<!-- ============================================================ -->
	<div class="flex flex-col flex-1 min-w-0">
		{#if showGraph && graph}
			<!-- Knowledge Graph View -->
			<div class="flex flex-col h-full bg-gx-bg-primary">
				<div class="flex items-center gap-2 px-4 py-3 border-b border-gx-border-default shrink-0">
					<Network size={14} class="text-gx-neon" />
					<span class="text-xs font-semibold text-gx-text-primary">Knowledge Graph</span>
					<Badge variant="outline" class="text-[9px] border-gx-border-default text-gx-text-muted">
						{graph.nodes.length} notes, {graph.edges.length} links
					</Badge>
					<div class="flex-1"></div>
					<button
						onclick={() => showGraph = false}
						class="text-gx-text-muted hover:text-gx-text-primary transition-colors"
					>
						<X size={16} />
					</button>
				</div>
				<div class="flex-1 flex items-center justify-center overflow-hidden p-4">
					{#if graph.nodes.length === 0}
						<div class="text-center">
							<Network size={40} class="text-gx-text-muted/20 mx-auto mb-3" />
							<p class="text-sm text-gx-text-muted">No connections yet</p>
							<p class="text-[11px] text-gx-text-muted/60 mt-1">Use [[Wiki Links]] in your notes to build the graph</p>
						</div>
					{:else}
						<svg viewBox="0 0 500 400" class="w-full h-full max-w-[700px] max-h-[500px]">
							<!-- Edges -->
							{#each graph.edges as edge}
								{@const from = getNodePos(edge.from)}
								{@const to = getNodePos(edge.to)}
								<line
									x1={from.x} y1={from.y}
									x2={to.x} y2={to.y}
									stroke="rgba(0, 240, 255, 0.2)"
									stroke-width="1"
								/>
							{/each}
							<!-- Nodes -->
							{#each graphPositions as pos}
								<g
									class="cursor-pointer"
									onclick={() => { showGraph = false; openNote(pos.id); }}
									onkeydown={(e) => { if (e.key === "Enter") { showGraph = false; openNote(pos.id); } }}
									role="button"
									tabindex="-1"
								>
									<circle
										cx={pos.x} cy={pos.y} r={pos.r}
										fill={activeNote?.id === pos.id ? 'rgba(0, 240, 255, 0.4)' : 'rgba(0, 240, 255, 0.15)'}
										stroke={activeNote?.id === pos.id ? '#00f0ff' : 'rgba(0, 240, 255, 0.3)'}
										stroke-width={activeNote?.id === pos.id ? 2 : 1}
										class="transition-all hover:fill-[rgba(0,240,255,0.3)]"
									/>
									<text
										x={pos.x} y={pos.y + pos.r + 12}
										text-anchor="middle"
										fill="rgba(200, 200, 220, 0.7)"
										font-size="9"
										class="pointer-events-none"
									>
										{pos.title.length > 18 ? pos.title.substring(0, 18) + '...' : pos.title}
									</text>
								</g>
							{/each}
						</svg>
					{/if}
				</div>
			</div>
		{:else if activeNote}
			<!-- Note Editor -->
			<!-- Title + Pin + Save toolbar -->
			<div class="flex items-center gap-2 px-4 py-2 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
				<button
					onclick={() => togglePin(activeNote!.id, !activeNote!.is_pinned)}
					class="flex items-center justify-center w-7 h-7 rounded transition-all
						{activeNote.is_pinned
							? 'text-gx-neon bg-gx-neon/10'
							: 'text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover'}"
					title={activeNote.is_pinned ? 'Unpin' : 'Pin'}
				>
					{#if activeNote.is_pinned}
						<Pin size={14} />
					{:else}
						<PinOff size={14} />
					{/if}
				</button>
				<div class="flex-1"></div>

				<!-- Save indicator -->
				<div class="flex items-center gap-1.5 mr-2">
					{#if saveIndicator === 'saving'}
						<Loader2 size={12} class="animate-spin text-gx-status-warning" />
						<span class="text-[10px] text-gx-status-warning">Saving...</span>
					{:else if saveIndicator === 'saved'}
						<CheckCheck size={12} class="text-gx-status-success" />
						<span class="text-[10px] text-gx-status-success">Saved</span>
					{:else if saveIndicator === 'unsaved'}
						<span class="w-1.5 h-1.5 rounded-full bg-gx-status-warning"></span>
						<span class="text-[10px] text-gx-text-muted">Unsaved</span>
					{/if}
				</div>

				<button
					onclick={saveNote}
					disabled={saving || !isDirty}
					class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-neon/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
					title="Save (Ctrl+S)"
				>
					<Save size={13} />
					Save
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />

				<button
					onclick={() => aiPanelOpen = !aiPanelOpen}
					class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded transition-all
						{aiPanelOpen
							? 'text-gx-accent-magenta bg-gx-accent-magenta/10 border border-gx-accent-magenta/30'
							: 'text-gx-text-muted hover:text-gx-accent-magenta hover:bg-gx-accent-magenta/10 border border-transparent'}"
					title="AI & Graph Panel"
				>
					<Brain size={13} />
					AI
				</button>
			</div>

			<!-- Title -->
			<div class="px-6 pt-4 pb-1 shrink-0">
				<input
					type="text"
					bind:value={editorTitle}
					oninput={() => { saveIndicator = 'unsaved'; }}
					placeholder="Note title..."
					class="w-full text-xl font-semibold bg-transparent text-gx-text-primary placeholder:text-gx-text-muted/30 focus:outline-none border-none"
				/>
			</div>

			<!-- Tag editor -->
			<div class="flex items-center gap-1.5 px-6 pb-2 flex-wrap shrink-0">
				{#each editorTags as tag}
					<span class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-gx-bg-elevated text-gx-text-secondary border border-gx-border-default">
						#{tag}
						<button
							onclick={() => removeTag(tag)}
							class="text-gx-text-muted hover:text-gx-status-error transition-colors"
						>
							<X size={9} />
						</button>
					</span>
				{/each}
				<div class="relative">
					<input
						type="text"
						bind:value={tagInput}
						onkeydown={handleTagKeydown}
						placeholder="Add tag..."
						class="w-20 text-[10px] px-1.5 py-0.5 rounded bg-transparent text-gx-text-muted placeholder:text-gx-text-muted/30 focus:outline-none focus:bg-gx-bg-primary border border-transparent focus:border-gx-border-default transition-colors"
					/>
				</div>
			</div>

			<!-- Editor + Right Panel -->
			<div class="flex flex-1 min-h-0 overflow-hidden">
				<!-- Editor area -->
				<div class="flex-1 flex flex-col min-w-0 overflow-hidden">
					<!-- Textarea -->
					<div class="relative flex-1 overflow-hidden">
						<textarea
							id="notes-editor"
							bind:value={editorContent}
							oninput={handleEditorInput}
							onkeydown={handleEditorKeydown}
							placeholder="Start writing... Use [[Wiki Links]] to connect notes."
							spellcheck="true"
							class="w-full h-full px-6 py-3 bg-transparent text-sm text-gx-text-secondary
								placeholder:text-gx-text-muted/30 focus:outline-none resize-none
								font-mono leading-relaxed tracking-wide selection:bg-gx-neon/20 selection:text-gx-text-primary"
						></textarea>

						<!-- Wiki-link autocomplete dropdown -->
						{#if showLinkAutocomplete && linkSuggestions.length > 0}
							<div class="absolute left-6 bottom-4 w-64 bg-gx-bg-elevated border border-gx-neon/30 rounded-gx shadow-gx-glow-sm z-50 max-h-48 overflow-y-auto">
								<div class="px-2 py-1.5 text-[10px] text-gx-text-muted border-b border-gx-border-default">
									Link to note...
								</div>
								{#each linkSuggestions as suggestion}
									<button
										onclick={() => insertWikiLink(suggestion.title)}
										class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors flex items-center gap-2"
									>
										<Link2 size={11} class="shrink-0 text-gx-accent-cyan" />
										<span class="truncate">{suggestion.title}</span>
									</button>
								{/each}
							</div>
						{/if}
					</div>

					<!-- Backlinks section -->
					{#if backlinks.length > 0}
						<div class="border-t border-gx-border-default px-6 py-3 shrink-0 max-h-40 overflow-y-auto bg-gx-bg-secondary/30">
							<div class="flex items-center gap-1.5 mb-2">
								<ArrowLeft size={12} class="text-gx-accent-purple" />
								<span class="text-[11px] font-medium text-gx-text-secondary">
									{backlinks.length} note{backlinks.length !== 1 ? 's' : ''} link to this note
								</span>
							</div>
							<div class="space-y-1">
								{#each backlinks as bl}
									<button
										onclick={() => openNote(bl.id)}
										class="w-full text-left px-2 py-1.5 rounded text-[11px] text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all flex items-center gap-2"
									>
										<ChevronRight size={10} class="shrink-0" />
										<span class="font-medium text-gx-text-secondary">{bl.title}</span>
										{#if bl.preview}
											<span class="text-gx-text-muted/50 truncate">- {bl.preview}</span>
										{/if}
									</button>
								{/each}
							</div>
						</div>
					{/if}

					<!-- Footer stats -->
					<div class="flex items-center gap-3 px-6 py-1.5 border-t border-gx-border-default bg-gx-bg-secondary/50 shrink-0 text-[10px] text-gx-text-muted">
						<span>{activeNote.word_count || editorContent.split(/\s+/).filter(Boolean).length} words</span>
						<span class="text-gx-border-default">|</span>
						<span>{activeNote.links_to.length} outgoing links</span>
						<span class="text-gx-border-default">|</span>
						<span>{activeNote.linked_from.length} backlinks</span>
						<div class="flex-1"></div>
						<span>Created {formatDate(activeNote.created_at)}</span>
					</div>
				</div>

				<!-- ============================================================ -->
				<!-- RIGHT PANEL - AI & Graph (280px, collapsible) -->
				<!-- ============================================================ -->
				{#if aiPanelOpen}
					<div class="w-[280px] border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 overflow-hidden">
						<div class="flex items-center justify-between px-3 py-2.5 border-b border-gx-border-default shrink-0">
							<div class="flex items-center gap-1.5">
								<Brain size={13} class="text-gx-accent-magenta" />
								<span class="text-xs font-medium text-gx-text-primary">AI Assistant</span>
							</div>
							<button
								onclick={() => aiPanelOpen = false}
								class="text-gx-text-muted hover:text-gx-text-primary transition-colors"
							>
								<X size={14} />
							</button>
						</div>

						<div class="flex-1 overflow-y-auto p-3 space-y-4">
							<!-- AI Generate -->
							<div class="space-y-2">
								<div class="flex items-center gap-1.5">
									<Sparkles size={12} class="text-gx-accent-magenta" />
									<span class="text-[11px] font-medium text-gx-text-secondary">Generate Note</span>
								</div>
								<div class="flex gap-1">
									<input
										type="text"
										bind:value={aiGenerateTopic}
										placeholder="Topic..."
										onkeydown={(e) => { if (e.key === 'Enter') aiGenerate(); }}
										class="flex-1 text-[11px] px-2 py-1.5 rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
									<button
										onclick={aiGenerate}
										disabled={aiLoading || !aiGenerateTopic.trim()}
										class="px-2 py-1.5 text-[11px] rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
									>
										Go
									</button>
								</div>
							</div>

							<Separator class="bg-gx-border-default/50" />

							<!-- AI Connect -->
							<div class="space-y-2">
								<div class="flex items-center gap-1.5">
									<Network size={12} class="text-gx-accent-cyan" />
									<span class="text-[11px] font-medium text-gx-text-secondary">Find Related</span>
								</div>
								<button
									onclick={aiFindRelated}
									disabled={aiLoading}
									class="w-full text-left text-[11px] px-2.5 py-1.5 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover border border-gx-border-default transition-all disabled:opacity-30"
								>
									Find notes related to this one
								</button>
								{#if aiRelatedNotes.length > 0}
									<div class="space-y-1">
										{#each aiRelatedNotes as related}
											<button
												onclick={() => openNote(related.id)}
												class="w-full text-left px-2 py-1.5 rounded text-[11px] text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover transition-all flex items-center gap-1.5"
											>
												<Link2 size={10} class="text-gx-accent-cyan shrink-0" />
												<span class="truncate">{related.title}</span>
											</button>
										{/each}
									</div>
								{/if}
							</div>

							<Separator class="bg-gx-border-default/50" />

							<!-- AI Summarize Tag -->
							<div class="space-y-2">
								<div class="flex items-center gap-1.5">
									<Tag size={12} class="text-gx-accent-purple" />
									<span class="text-[11px] font-medium text-gx-text-secondary">Summarize Tag</span>
								</div>
								<div class="flex gap-1">
									<select
										bind:value={aiSummarizeTag}
										class="flex-1 text-[11px] px-2 py-1.5 rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
									>
										<option value="">Select tag...</option>
										{#each allTags as tag}
											<option value={tag.name}>#{tag.name} ({tag.count})</option>
										{/each}
									</select>
									<button
										onclick={aiSummarize}
										disabled={aiLoading || !aiSummarizeTag}
										class="px-2 py-1.5 text-[11px] rounded bg-gx-accent-purple/15 text-gx-accent-purple border border-gx-accent-purple/30 hover:bg-gx-accent-purple/25 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
									>
										Go
									</button>
								</div>
							</div>

							<Separator class="bg-gx-border-default/50" />

							<!-- AI Result -->
							{#if aiLoading}
								<div class="flex flex-col items-center justify-center py-6 gap-2">
									<Loader2 size={18} class="animate-spin text-gx-accent-magenta" />
									<span class="text-[11px] text-gx-text-muted">AI is thinking...</span>
								</div>
							{:else if aiError}
								<div class="rounded bg-gx-status-error/10 border border-gx-status-error/20 p-3">
									<div class="flex items-start gap-2">
										<AlertCircle size={13} class="text-gx-status-error shrink-0 mt-0.5" />
										<p class="text-[11px] text-gx-status-error leading-relaxed">{aiError}</p>
									</div>
								</div>
							{:else if aiResult}
								<div class="space-y-2">
									<div class="rounded bg-gx-bg-primary border border-gx-border-default p-3 max-h-64 overflow-y-auto">
										<p class="text-[11px] text-gx-text-secondary leading-relaxed whitespace-pre-wrap font-mono">
											{aiResult}
										</p>
									</div>
									<button
										onclick={applyAiAsNewNote}
										class="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
									>
										<CheckCheck size={12} />
										Use as note content
									</button>
								</div>
							{/if}

							<!-- Mini graph button -->
							<div class="pt-2">
								<button
									onclick={loadGraph}
									class="w-full flex items-center justify-center gap-1.5 px-3 py-2 text-[11px] font-medium rounded text-gx-text-muted hover:text-gx-neon border border-gx-border-default hover:border-gx-neon/30 hover:bg-gx-bg-hover transition-all"
								>
									<Network size={13} />
									Open Knowledge Graph
								</button>
							</div>
						</div>
					</div>
				{/if}
			</div>
		{:else}
			<!-- Empty State -->
			<div class="flex-1 flex flex-col items-center justify-center px-8 text-center">
				<div class="w-16 h-16 rounded-2xl bg-gx-bg-secondary border border-gx-border-default flex items-center justify-center mb-6">
					<BookOpen size={28} class="text-gx-neon/40" />
				</div>
				<h2 class="text-lg font-semibold text-gx-text-primary mb-2">ForgeNotes</h2>
				<p class="text-sm text-gx-text-muted mb-6 max-w-sm">
					Start your knowledge base. Write notes in Markdown, connect them with [[Wiki Links]], and let AI discover hidden connections.
				</p>
				<div class="flex gap-3">
					<button
						onclick={createNote}
						class="flex items-center gap-2 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
					>
						<FilePlus size={14} />
						Create First Note
					</button>
				</div>

				<!-- Feature highlights -->
				<div class="mt-8 grid grid-cols-3 gap-4 max-w-lg">
					<div class="text-center p-3 rounded-gx border border-gx-border-default/50">
						<Link2 size={18} class="text-gx-accent-cyan mx-auto mb-1.5" />
						<p class="text-[10px] font-medium text-gx-text-secondary">Wiki Links</p>
						<p class="text-[9px] text-gx-text-muted mt-0.5">Type [[ to link notes</p>
					</div>
					<div class="text-center p-3 rounded-gx border border-gx-border-default/50">
						<Network size={18} class="text-gx-accent-purple mx-auto mb-1.5" />
						<p class="text-[10px] font-medium text-gx-text-secondary">Knowledge Graph</p>
						<p class="text-[9px] text-gx-text-muted mt-0.5">Visualize connections</p>
					</div>
					<div class="text-center p-3 rounded-gx border border-gx-border-default/50">
						<Sparkles size={18} class="text-gx-accent-magenta mx-auto mb-1.5" />
						<p class="text-[10px] font-medium text-gx-text-secondary">AI Powered</p>
						<p class="text-[9px] text-gx-text-muted mt-0.5">Generate and connect</p>
					</div>
				</div>

				<div class="mt-6 text-[10px] text-gx-text-muted">
					<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Ctrl+S</kbd>
					Save &nbsp;
					<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">[[</kbd>
					Wiki Link
				</div>
			</div>
		{/if}
	</div>
</div>
