<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		FileEdit, FilePlus, Save, Download, Bold, Italic, Heading,
		List, Link, Sparkles, Languages, Scissors, Expand, CheckCheck,
		Clock, Search, Loader2, Trash2, AlertCircle, X, ChevronDown,
		Eye, EyeOff, RefreshCw, FileText, Type, Code2, Hash
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-writer';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let editorComponent = $derived(styleEngine.getComponentStyle(widgetId, 'editor'));
	let editorStyle = $derived(hasEngineStyle && editorComponent ? componentToCSS(editorComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface DocumentMeta {
		id: string;
		title: string;
		format: string;
		word_count: number;
		updated_at: string;
		tags: string[];
	}

	interface Document {
		id: string;
		title: string;
		content: string;
		format: string;
		word_count: number;
		created_at: string;
		updated_at: string;
		tags: string[];
		auto_saved: boolean;
	}

	interface WordCountStats {
		words: number;
		characters: number;
		sentences: number;
		paragraphs: number;
		reading_time_min: number;
	}

	// ---- State ---------------------------------------------------------------
	let documents = $state<DocumentMeta[]>([]);
	let activeDoc = $state<Document | null>(null);
	let editorContent = $state('');
	let editorTitle = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let sidebarOpen = $state(true);
	let previewMode = $state(false);
	let lastSavedContent = $state('');
	let autoSaveTimer: ReturnType<typeof setInterval> | null = null;
	let saveIndicator = $state<'saved' | 'saving' | 'unsaved' | 'idle'>('idle');

	// AI Assist state
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state('');
	let aiError = $state<string | null>(null);
	let selectedText = $state('');

	// Export dropdown
	let exportMenuOpen = $state(false);

	// New document dialog
	let newDocDialogOpen = $state(false);
	let newDocTitle = $state('');
	let newDocFormat = $state('markdown');

	// Word count
	let wordStats = $state<WordCountStats>({ words: 0, characters: 0, sentences: 0, paragraphs: 0, reading_time_min: 0 });

	// ---- Derived -------------------------------------------------------------
	let filteredDocs = $derived(
		searchQuery.trim()
			? documents.filter(d =>
				d.title.toLowerCase().includes(searchQuery.toLowerCase())
			)
			: documents
	);

	let isDirty = $derived(editorContent !== lastSavedContent);

	let formatLabel = $derived(
		activeDoc?.format === 'markdown' ? 'Markdown' :
		activeDoc?.format === 'html' ? 'HTML' :
		'Plain Text'
	);

	// ---- Data Loading --------------------------------------------------------
	async function loadDocuments() {
		loading = true;
		error = null;
		try {
			documents = await invoke<DocumentMeta[]>('writer_list_documents');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	async function openDocument(id: string) {
		// Save current doc if dirty
		if (activeDoc && isDirty) {
			await saveDocument();
		}

		try {
			const doc = await invoke<Document>('writer_get_document', { id });
			activeDoc = doc;
			editorContent = doc.content;
			editorTitle = doc.title;
			lastSavedContent = doc.content;
			saveIndicator = 'saved';
			previewMode = false;
			aiResult = '';
			aiError = null;
			updateWordCount();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function createDocument() {
		const title = newDocTitle.trim() || 'Untitled Document';
		try {
			const doc = await invoke<Document>('writer_create_document', {
				title,
				format: newDocFormat,
			});
			newDocDialogOpen = false;
			newDocTitle = '';
			await loadDocuments();
			await openDocument(doc.id);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function saveDocument() {
		if (!activeDoc) return;
		saving = true;
		saveIndicator = 'saving';
		try {
			const updated = await invoke<Document>('writer_save_document', {
				id: activeDoc.id,
				title: editorTitle || null,
				content: editorContent,
			});
			activeDoc = updated;
			lastSavedContent = editorContent;
			saveIndicator = 'saved';
			// Update listing
			const idx = documents.findIndex(d => d.id === updated.id);
			if (idx >= 0) {
				documents[idx] = {
					id: updated.id,
					title: updated.title,
					format: updated.format,
					word_count: updated.word_count,
					updated_at: updated.updated_at,
					tags: updated.tags,
				};
				// Re-sort by updated_at descending
				documents = [...documents].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			saveIndicator = 'unsaved';
		} finally {
			saving = false;
		}
	}

	async function deleteDocument(id: string) {
		try {
			await invoke('writer_delete_document', { id });
			if (activeDoc?.id === id) {
				activeDoc = null;
				editorContent = '';
				editorTitle = '';
				lastSavedContent = '';
				saveIndicator = 'idle';
			}
			await loadDocuments();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function exportDocument(format: string) {
		if (!activeDoc) return;
		exportMenuOpen = false;
		try {
			const path = await invoke<string>('writer_export_document', {
				id: activeDoc.id,
				exportFormat: format,
			});
			// Brief success indicator
			error = null;
			// Could show a toast here. For now, log.
			console.log('Exported to:', path);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- AI Assist -----------------------------------------------------------
	async function aiAssist(action: string) {
		if (!activeDoc) return;
		const text = selectedText.trim() || editorContent.trim();
		if (!text) return;

		aiLoading = true;
		aiError = null;
		aiResult = '';
		aiPanelOpen = true;

		try {
			const result = await invoke<string>('writer_ai_assist', {
				documentId: activeDoc.id,
				selection: text,
				action,
			});
			aiResult = result;
		} catch (e) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	function applyAiResult() {
		if (!aiResult) return;

		if (selectedText.trim()) {
			// Replace selection in content
			const idx = editorContent.indexOf(selectedText);
			if (idx >= 0) {
				editorContent =
					editorContent.substring(0, idx) +
					aiResult +
					editorContent.substring(idx + selectedText.length);
			}
		} else {
			// Replace entire content
			editorContent = aiResult;
		}

		aiResult = '';
		aiPanelOpen = false;
		updateWordCount();
	}

	// ---- Word Count ----------------------------------------------------------
	async function updateWordCount() {
		try {
			wordStats = await invoke<WordCountStats>('writer_word_count', {
				content: editorContent,
			});
		} catch {
			// Fallback client-side count
			const words = editorContent.split(/\s+/).filter(Boolean).length;
			wordStats = {
				words,
				characters: editorContent.length,
				sentences: 0,
				paragraphs: 0,
				reading_time_min: Math.round(words / 238 * 100) / 100,
			};
		}
	}

	// ---- Editor Helpers ------------------------------------------------------
	function handleEditorInput() {
		saveIndicator = 'unsaved';
		updateWordCount();
	}

	function handleTextSelection(e: Event) {
		const textarea = e.target as HTMLTextAreaElement;
		const start = textarea.selectionStart;
		const end = textarea.selectionEnd;
		if (start !== end) {
			selectedText = editorContent.substring(start, end);
		} else {
			selectedText = '';
		}
	}

	function insertMarkdown(prefix: string, suffix: string = '') {
		const textarea = document.getElementById('forge-editor') as HTMLTextAreaElement | null;
		if (!textarea) return;
		const start = textarea.selectionStart;
		const end = textarea.selectionEnd;
		const selected = editorContent.substring(start, end);
		const replacement = `${prefix}${selected || 'text'}${suffix || prefix}`;
		editorContent =
			editorContent.substring(0, start) +
			replacement +
			editorContent.substring(end);
		// Re-focus after insertion
		requestAnimationFrame(() => {
			textarea.focus();
			const newCursor = start + prefix.length;
			textarea.setSelectionRange(newCursor, newCursor + (selected.length || 4));
		});
		handleEditorInput();
	}

	function handleEditorKeydown(e: KeyboardEvent) {
		const mod = e.ctrlKey || e.metaKey;
		if (mod && e.key === 's') {
			e.preventDefault();
			saveDocument();
			return;
		}
		if (mod && e.key === 'b') {
			e.preventDefault();
			insertMarkdown('**');
			return;
		}
		if (mod && e.key === 'i') {
			e.preventDefault();
			insertMarkdown('*');
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
			handleEditorInput();
		}
	}

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
			return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
		} catch {
			return dateStr;
		}
	}

	function readingTimeDisplay(mins: number): string {
		if (mins < 1) return '< 1 min read';
		if (mins < 2) return '1 min read';
		return `${Math.ceil(mins)} min read`;
	}

	// Basic markdown preview renderer (client-side, quick)
	function renderPreview(content: string): string {
		let html = content
			// Escape HTML entities
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');

		// Headings
		html = html.replace(/^### (.+)$/gm, '<h3 class="text-base font-semibold text-gx-text-primary mt-4 mb-2">$1</h3>');
		html = html.replace(/^## (.+)$/gm, '<h2 class="text-lg font-semibold text-gx-text-primary mt-5 mb-2">$1</h2>');
		html = html.replace(/^# (.+)$/gm, '<h1 class="text-xl font-bold text-gx-neon mt-6 mb-3">$1</h1>');

		// Bold and italic
		html = html.replace(/\*\*(.+?)\*\*/g, '<strong class="font-semibold text-gx-text-primary">$1</strong>');
		html = html.replace(/\*(.+?)\*/g, '<em class="italic text-gx-text-secondary">$1</em>');

		// Inline code
		html = html.replace(/`(.+?)`/g, '<code class="px-1 py-0.5 rounded bg-gx-bg-elevated text-gx-accent-magenta text-[11px] font-mono">$1</code>');

		// Links
		html = html.replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" class="text-gx-neon underline">$1</a>');

		// Lists
		html = html.replace(/^- (.+)$/gm, '<li class="ml-4 list-disc text-gx-text-secondary text-xs leading-relaxed">$1</li>');

		// Code blocks
		html = html.replace(/```[\s\S]*?```/g, (match) => {
			const code = match.replace(/^```\w*\n?/, '').replace(/\n?```$/, '');
			return `<pre class="bg-gx-bg-elevated rounded-gx p-3 my-2 overflow-x-auto"><code class="text-[11px] font-mono text-gx-text-secondary">${code}</code></pre>`;
		});

		// Paragraphs (double newline)
		html = html.replace(/\n\n/g, '</p><p class="text-xs text-gx-text-secondary leading-relaxed mb-2">');

		// Single newlines to <br>
		html = html.replace(/\n/g, '<br>');

		return `<p class="text-xs text-gx-text-secondary leading-relaxed mb-2">${html}</p>`;
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadDocuments();

		// Auto-save every 30 seconds
		autoSaveTimer = setInterval(() => {
			if (activeDoc && isDirty) {
				saveDocument();
			}
		}, 30_000);
	});

	onDestroy(() => {
		if (autoSaveTimer) clearInterval(autoSaveTimer);
	});
</script>

<div class="flex h-full overflow-hidden" style={containerStyle}>
	<!-- Document Sidebar -->
	{#if sidebarOpen}
		<div class="flex flex-col w-64 border-r border-gx-border-default bg-gx-bg-secondary shrink-0">
			<!-- Sidebar Header -->
			<div class="flex items-center justify-between px-3 py-3 border-b border-gx-border-default">
				<div class="flex items-center gap-2">
					<FileEdit size={14} class="text-gx-neon" />
					<span class="text-xs font-semibold text-gx-text-primary">Documents</span>
				</div>
				<button
					onclick={() => { newDocDialogOpen = true; }}
					class="flex items-center justify-center w-6 h-6 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all"
					title="New document"
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
						placeholder="Search documents..."
						class="w-full pl-7 pr-2 py-1.5 text-[11px] rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
					/>
				</div>
			</div>

			<!-- Document List -->
			<div class="flex-1 overflow-y-auto">
				{#if loading}
					<div class="flex items-center justify-center py-8">
						<Loader2 size={16} class="animate-spin text-gx-text-muted" />
					</div>
				{:else if filteredDocs.length === 0}
					<div class="flex flex-col items-center justify-center py-8 px-3 text-center">
						<FileText size={28} class="text-gx-text-muted/30 mb-2" />
						<p class="text-[11px] text-gx-text-muted">
							{searchQuery ? 'No matching documents' : 'No documents yet'}
						</p>
						{#if !searchQuery}
							<button
								onclick={() => { newDocDialogOpen = true; }}
								class="mt-2 text-[11px] text-gx-neon hover:underline"
							>
								Create your first document
							</button>
						{/if}
					</div>
				{:else}
					{#each filteredDocs as doc (doc.id)}
						<button
							onclick={() => openDocument(doc.id)}
							class="w-full text-left px-3 py-2.5 border-b border-gx-border-default/50 transition-all group
								{activeDoc?.id === doc.id
									? 'bg-gx-bg-elevated border-l-2 border-l-gx-neon'
									: 'hover:bg-gx-bg-hover'}"
						>
							<div class="flex items-start justify-between gap-1">
								<div class="min-w-0 flex-1">
									<p class="text-xs font-medium text-gx-text-primary truncate">
										{doc.title}
									</p>
									<div class="flex items-center gap-2 mt-0.5">
										<span class="text-[10px] text-gx-text-muted">
											{formatDate(doc.updated_at)}
										</span>
										<span class="text-[10px] text-gx-text-muted/60">
											{doc.word_count} words
										</span>
									</div>
								</div>
								<button
									onclick={(e) => { e.stopPropagation(); deleteDocument(doc.id); }}
									class="shrink-0 w-5 h-5 flex items-center justify-center rounded text-gx-text-muted/0 group-hover:text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-all"
									title="Delete document"
								>
									<Trash2 size={11} />
								</button>
							</div>
							{#if doc.tags.length > 0}
								<div class="flex gap-1 mt-1 flex-wrap">
									{#each doc.tags.slice(0, 3) as tag}
										<span class="text-[9px] px-1 py-0 rounded bg-gx-bg-primary text-gx-text-muted border border-gx-border-default">
											{tag}
										</span>
									{/each}
								</div>
							{/if}
						</button>
					{/each}
				{/if}
			</div>

			<!-- Sidebar Footer -->
			<div class="px-3 py-2 border-t border-gx-border-default">
				<span class="text-[10px] text-gx-text-muted">
					{documents.length} document{documents.length !== 1 ? 's' : ''}
				</span>
			</div>
		</div>
	{/if}

	<!-- Main Editor Area -->
	<div class="flex flex-col flex-1 min-w-0">
		{#if activeDoc}
			<!-- Toolbar -->
			<div class="flex items-center gap-1 px-3 py-1.5 border-b border-gx-border-default bg-gx-bg-secondary shrink-0 overflow-x-auto">
				<!-- Sidebar toggle -->
				<button
					onclick={() => sidebarOpen = !sidebarOpen}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all mr-1"
					title={sidebarOpen ? 'Hide sidebar' : 'Show sidebar'}
				>
					<FileEdit size={14} />
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />

				<!-- Formatting buttons (Markdown shortcuts) -->
				<button
					onclick={() => insertMarkdown('**')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="Bold (Ctrl+B)"
				>
					<Bold size={14} />
				</button>
				<button
					onclick={() => insertMarkdown('*')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="Italic (Ctrl+I)"
				>
					<Italic size={14} />
				</button>
				<button
					onclick={() => insertMarkdown('## ', '\n')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="Heading"
				>
					<Heading size={14} />
				</button>
				<button
					onclick={() => insertMarkdown('- ', '\n')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="List"
				>
					<List size={14} />
				</button>
				<button
					onclick={() => insertMarkdown('[', '](url)')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="Link"
				>
					<Link size={14} />
				</button>
				<button
					onclick={() => insertMarkdown('`')}
					class="flex items-center justify-center w-7 h-7 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
					title="Inline code"
				>
					<Code2 size={14} />
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />

				<!-- Preview toggle -->
				<button
					onclick={() => previewMode = !previewMode}
					class="flex items-center gap-1 px-2 py-1 rounded text-[11px] transition-all
						{previewMode
							? 'text-gx-neon bg-gx-neon/10 border border-gx-neon/30'
							: 'text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-transparent'}"
					title="Toggle preview"
				>
					{#if previewMode}
						<EyeOff size={13} />
						<span>Edit</span>
					{:else}
						<Eye size={13} />
						<span>Preview</span>
					{/if}
				</button>

				<div class="flex-1"></div>

				<!-- Save status -->
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

				<!-- Save button -->
				<button
					onclick={saveDocument}
					disabled={saving || !isDirty}
					class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-neon/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
					title="Save (Ctrl+S)"
				>
					<Save size={13} />
					Save
				</button>

				<!-- Export dropdown -->
				<div class="relative">
					<button
						onclick={() => exportMenuOpen = !exportMenuOpen}
						class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-all"
						title="Export"
					>
						<Download size={13} />
						Export
						<ChevronDown size={10} />
					</button>
					{#if exportMenuOpen}
						<div class="absolute right-0 top-full mt-1 w-40 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-lg z-50">
							<button
								onclick={() => exportDocument('md')}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover transition-colors flex items-center gap-2"
							>
								<Hash size={11} />
								Markdown (.md)
							</button>
							<button
								onclick={() => exportDocument('html')}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover transition-colors flex items-center gap-2"
							>
								<Code2 size={11} />
								HTML (.html)
							</button>
							<button
								onclick={() => exportDocument('txt')}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover transition-colors flex items-center gap-2"
							>
								<Type size={11} />
								Plain Text (.txt)
							</button>
							<Separator class="bg-gx-border-default" />
							<div class="px-3 py-1.5 text-[10px] text-gx-text-muted/50 italic">
								PDF coming soon
							</div>
						</div>
					{/if}
				</div>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />

				<!-- AI Assist toggle -->
				<button
					onclick={() => aiPanelOpen = !aiPanelOpen}
					class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded transition-all
						{aiPanelOpen
							? 'text-gx-accent-magenta bg-gx-accent-magenta/10 border border-gx-accent-magenta/30'
							: 'text-gx-text-muted hover:text-gx-accent-magenta hover:bg-gx-accent-magenta/10 border border-transparent'}"
					title="AI Writing Assistant"
				>
					<Sparkles size={13} />
					AI Assist
				</button>
			</div>

			<!-- Title bar -->
			<div class="px-6 pt-4 pb-2 shrink-0">
				<input
					type="text"
					bind:value={editorTitle}
					oninput={() => { saveIndicator = 'unsaved'; }}
					placeholder="Document title..."
					class="w-full text-xl font-semibold bg-transparent text-gx-text-primary placeholder:text-gx-text-muted/30 focus:outline-none border-none"
				/>
				<div class="flex items-center gap-3 mt-1">
					<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-border-default text-gx-text-muted">
						{formatLabel}
					</Badge>
					<span class="text-[10px] text-gx-text-muted/60">
						Created {formatDate(activeDoc.created_at)}
					</span>
				</div>
			</div>

			<!-- Editor + AI Panel -->
			<div class="flex flex-1 min-h-0 overflow-hidden">
				<!-- Editor / Preview -->
				<div class="flex-1 flex flex-col min-w-0 overflow-hidden" style={editorStyle}>
					{#if previewMode}
						<!-- Markdown Preview -->
						<div class="flex-1 overflow-y-auto px-6 py-4">
							<div class="prose-writer max-w-none">
								{@html renderPreview(editorContent)}
							</div>
						</div>
					{:else}
						<!-- Textarea Editor -->
						<textarea
							id="forge-editor"
							bind:value={editorContent}
							oninput={handleEditorInput}
							onselect={handleTextSelection}
							onmouseup={handleTextSelection}
							onkeydown={handleEditorKeydown}
							placeholder="Start writing..."
							spellcheck="true"
							class="flex-1 w-full px-6 py-4 bg-transparent text-sm text-gx-text-secondary
								placeholder:text-gx-text-muted/30 focus:outline-none resize-none
								font-mono leading-relaxed tracking-wide selection:bg-gx-neon/20 selection:text-gx-text-primary"
						></textarea>
					{/if}

					<!-- Footer stats bar -->
					<div class="flex items-center gap-3 px-6 py-1.5 border-t border-gx-border-default bg-gx-bg-secondary/50 shrink-0 text-[10px] text-gx-text-muted">
						<div class="flex items-center gap-1">
							<Type size={10} />
							<span>{wordStats.words} words</span>
						</div>
						<span class="text-gx-border-default">|</span>
						<span>{wordStats.characters} chars</span>
						<span class="text-gx-border-default">|</span>
						<span>{wordStats.sentences} sentences</span>
						<span class="text-gx-border-default">|</span>
						<span>{wordStats.paragraphs} paragraphs</span>
						<div class="flex-1"></div>
						<div class="flex items-center gap-1">
							<Clock size={10} />
							<span>{readingTimeDisplay(wordStats.reading_time_min)}</span>
						</div>
						{#if selectedText}
							<span class="text-gx-accent-magenta">
								{selectedText.split(/\s+/).filter(Boolean).length} selected
							</span>
						{/if}
					</div>
				</div>

				<!-- AI Assist Panel (right sidebar) -->
				{#if aiPanelOpen}
					<div class="w-72 border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 overflow-hidden">
						<!-- Panel header -->
						<div class="flex items-center justify-between px-3 py-2.5 border-b border-gx-border-default shrink-0">
							<div class="flex items-center gap-1.5">
								<Sparkles size={13} class="text-gx-accent-magenta" />
								<span class="text-xs font-medium text-gx-text-primary">AI Writing Assistant</span>
							</div>
							<button
								onclick={() => aiPanelOpen = false}
								class="text-gx-text-muted hover:text-gx-text-primary transition-colors"
							>
								<X size={14} />
							</button>
						</div>

						<!-- Instructions -->
						<div class="px-3 py-2 border-b border-gx-border-default/50 shrink-0">
							<p class="text-[10px] text-gx-text-muted leading-relaxed">
								Select text in the editor, then choose an action. Or use the full document.
							</p>
							{#if selectedText}
								<div class="mt-1.5 px-2 py-1 rounded bg-gx-bg-primary border border-gx-neon/20">
									<span class="text-[10px] text-gx-neon font-medium">
										{selectedText.split(/\s+/).filter(Boolean).length} words selected
									</span>
								</div>
							{/if}
						</div>

						<!-- Action buttons -->
						<div class="px-3 py-3 space-y-1.5 shrink-0 border-b border-gx-border-default/50">
							<button
								onclick={() => aiAssist('improve')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<Sparkles size={12} class="text-gx-accent-magenta shrink-0" />
								Improve Writing
							</button>
							<button
								onclick={() => aiAssist('fix_grammar')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<CheckCheck size={12} class="text-gx-status-success shrink-0" />
								Fix Grammar
							</button>
							<button
								onclick={() => aiAssist('shorten')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<Scissors size={12} class="text-gx-status-warning shrink-0" />
								Shorten
							</button>
							<button
								onclick={() => aiAssist('expand')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<Expand size={12} class="text-gx-accent-blue shrink-0" />
								Expand
							</button>
							<button
								onclick={() => aiAssist('summarize')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<FileText size={12} class="text-gx-accent-purple shrink-0" />
								Summarize
							</button>

							<Separator class="bg-gx-border-default/50" />

							<button
								onclick={() => aiAssist('translate_en')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<Languages size={12} class="text-gx-neon shrink-0" />
								Translate to English
							</button>
							<button
								onclick={() => aiAssist('translate_de')}
								disabled={aiLoading}
								class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
							>
								<Languages size={12} class="text-gx-neon shrink-0" />
								Translate to German
							</button>
						</div>

						<!-- AI Result area -->
						<div class="flex-1 overflow-y-auto px-3 py-3">
							{#if aiLoading}
								<div class="flex flex-col items-center justify-center py-8 gap-2">
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
								<div class="space-y-3">
									<div class="rounded bg-gx-bg-primary border border-gx-border-default p-3">
										<p class="text-[11px] text-gx-text-secondary leading-relaxed whitespace-pre-wrap font-mono">
											{aiResult}
										</p>
									</div>
									<div class="flex gap-2">
										<button
											onclick={applyAiResult}
											class="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
										>
											<CheckCheck size={12} />
											Apply
										</button>
										<button
											onclick={() => { aiResult = ''; }}
											class="px-3 py-1.5 text-[11px] rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
										>
											Discard
										</button>
									</div>
								</div>
							{:else}
								<div class="flex flex-col items-center justify-center py-8 text-center">
									<Sparkles size={24} class="text-gx-text-muted/20 mb-2" />
									<p class="text-[11px] text-gx-text-muted">
										Select text and choose an action to get AI suggestions
									</p>
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>

		{:else}
			<!-- No document open (welcome state) -->
			<div class="flex-1 flex flex-col items-center justify-center px-8 text-center">
				{#if !sidebarOpen}
					<button
						onclick={() => sidebarOpen = true}
						class="absolute top-3 left-3 flex items-center justify-center w-8 h-8 rounded text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all"
						title="Show sidebar"
					>
						<FileEdit size={16} />
					</button>
				{/if}

				<div class="w-16 h-16 rounded-2xl bg-gx-bg-secondary border border-gx-border-default flex items-center justify-center mb-6">
					<FileEdit size={28} class="text-gx-neon/40" />
				</div>
				<h2 class="text-lg font-semibold text-gx-text-primary mb-2">ForgeWriter</h2>
				<p class="text-sm text-gx-text-muted mb-6 max-w-sm">
					A focused writing environment with AI assistance. Create documents, write in Markdown, and let AI help you improve your text.
				</p>
				<div class="flex gap-3">
					<button
						onclick={() => { newDocDialogOpen = true; }}
						class="flex items-center gap-2 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
					>
						<FilePlus size={14} />
						New Document
					</button>
					{#if documents.length > 0}
						<button
							onclick={() => sidebarOpen = true}
							class="flex items-center gap-2 px-4 py-2 text-xs font-medium rounded-gx text-gx-text-secondary border border-gx-border-default hover:border-gx-text-muted/50 hover:bg-gx-bg-hover transition-all"
						>
							<FileText size={14} />
							Open Recent
						</button>
					{/if}
				</div>

				<!-- Keyboard shortcuts -->
				<div class="mt-8 grid grid-cols-2 gap-x-6 gap-y-1.5 text-[10px] text-gx-text-muted">
					<div class="flex items-center gap-2">
						<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Ctrl+S</kbd>
						<span>Save document</span>
					</div>
					<div class="flex items-center gap-2">
						<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Ctrl+B</kbd>
						<span>Bold text</span>
					</div>
					<div class="flex items-center gap-2">
						<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Ctrl+I</kbd>
						<span>Italic text</span>
					</div>
					<div class="flex items-center gap-2">
						<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Tab</kbd>
						<span>Indent</span>
					</div>
				</div>
			</div>
		{/if}
	</div>

	<!-- Close export menu on outside click -->
	{#if exportMenuOpen}
		<button
			class="fixed inset-0 z-40"
			onclick={() => exportMenuOpen = false}
			aria-label="Close export menu"
		></button>
	{/if}
</div>

<!-- New Document Dialog -->
{#if newDocDialogOpen}
	<div class="fixed inset-0 z-50 flex items-center justify-center">
		<!-- Backdrop -->
		<button
			class="absolute inset-0 bg-black/60"
			onclick={() => newDocDialogOpen = false}
			aria-label="Close dialog"
		></button>

		<!-- Dialog -->
		<div class="relative bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg w-96 p-6 space-y-4">
			<div class="flex items-center gap-2">
				<FilePlus size={16} class="text-gx-neon" />
				<h3 class="text-sm font-semibold text-gx-text-primary">New Document</h3>
			</div>

			<div>
				<label for="new-doc-title" class="text-[11px] text-gx-text-muted mb-1 block">Title</label>
				<input
					id="new-doc-title"
					type="text"
					bind:value={newDocTitle}
					placeholder="Untitled Document"
					onkeydown={(e) => { if (e.key === 'Enter') createDocument(); }}
					class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
				/>
			</div>

			<div>
				<label for="new-doc-format" class="text-[11px] text-gx-text-muted mb-1 block">Format</label>
				<select
					id="new-doc-format"
					bind:value={newDocFormat}
					class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
				>
					<option value="markdown">Markdown</option>
					<option value="html">HTML</option>
					<option value="plaintext">Plain Text</option>
				</select>
			</div>

			{#if error}
				<div class="flex items-center gap-2 px-3 py-2 rounded bg-gx-status-error/10 border border-gx-status-error/20">
					<AlertCircle size={13} class="text-gx-status-error shrink-0" />
					<span class="text-[11px] text-gx-status-error">{error}</span>
				</div>
			{/if}

			<div class="flex justify-end gap-2 pt-2">
				<button
					onclick={() => { newDocDialogOpen = false; error = null; }}
					class="px-3 py-1.5 text-xs rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
				>
					Cancel
				</button>
				<button
					onclick={createDocument}
					class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
				>
					<FilePlus size={13} />
					Create
				</button>
			</div>
		</div>
	</div>
{/if}
</page>
