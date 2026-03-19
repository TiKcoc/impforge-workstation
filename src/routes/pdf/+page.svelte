<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		FileText, Upload, Download, Sparkles, MessageSquare, Search,
		Trash2, Loader2, AlertCircle, X,
		Eye, BookOpen, Hash, HardDrive, Calendar, PanelRightClose,
		PanelRightOpen, Send, Copy, Check, Type
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-pdf';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface PdfMeta {
		id: string;
		title: string;
		file_size: number;
		page_count: number;
		imported_at: string;
	}

	interface PdfDocument {
		id: string;
		title: string;
		path: string;
		file_size: number;
		page_count: number;
		text_preview: string;
		created_at: string;
		imported_at: string;
	}

	interface PdfAiResult {
		document_id: string;
		action: string;
		result: string;
		model: string;
	}

	interface PdfConvertResult {
		document_id: string;
		output_path: string;
		format: string;
		char_count: number;
	}

	// ---- State ---------------------------------------------------------------
	let documents = $state<PdfMeta[]>([]);
	let activeDoc = $state<PdfDocument | null>(null);
	let extractedText = $state('');
	let loading = $state(true);
	let importing = $state(false);
	let extracting = $state(false);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let sidebarOpen = $state(true);

	// AI Panel state
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state('');
	let aiError = $state<string | null>(null);
	let aiQuestion = $state('');
	let aiCopied = $state(false);

	// Import dialog state
	let importDialogOpen = $state(false);
	let importPath = $state('');

	// Hidden file input ref
	let fileInputEl: HTMLInputElement | undefined = $state();

	// Convert state
	let converting = $state(false);
	let convertResult = $state<string | null>(null);

	// ---- Derived -------------------------------------------------------------
	let filteredDocs = $derived(
		searchQuery.trim()
			? documents.filter(d =>
				d.title.toLowerCase().includes(searchQuery.toLowerCase())
			)
			: documents
	);

	// ---- Data Loading --------------------------------------------------------
	async function loadDocuments() {
		loading = true;
		error = null;
		try {
			documents = await invoke<PdfMeta[]>('pdf_list');
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	async function openDocument(id: string) {
		error = null;
		extractedText = '';
		aiResult = '';
		aiError = null;
		convertResult = null;
		try {
			activeDoc = await invoke<PdfDocument>('pdf_get_info', { id });
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function extractText() {
		if (!activeDoc) return;
		extracting = true;
		error = null;
		try {
			extractedText = await invoke<string>('pdf_get_text', { id: activeDoc.id });
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			extracting = false;
		}
	}

	function triggerFileInput() {
		fileInputEl?.click();
	}

	async function handleFileSelected(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		importing = true;
		error = null;
		try {
			// Tauri WebView provides the real filesystem path via webkitRelativePath
			// or the file name. For drag-and-drop or HTML file inputs in Tauri 2.x,
			// we can read the path from the File object's name property and use the
			// Tauri fs plugin to resolve it. However, the most reliable approach is
			// to use the path directly if available.
			//
			// In Tauri 2.x, HTML <input type="file"> provides the real absolute path
			// through the File's path property (non-standard but supported by WebView).
			const filePath = (file as File & { path?: string }).path || file.name;

			if (!filePath || filePath === file.name) {
				// Fallback: show the manual path input
				importDialogOpen = true;
				importing = false;
				return;
			}

			const doc = await invoke<PdfDocument>('pdf_import', { filePath });
			await loadDocuments();
			activeDoc = doc;
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			importing = false;
			// Reset input so the same file can be re-selected
			input.value = '';
		}
	}

	async function importFromPath() {
		if (!importPath.trim()) return;
		importing = true;
		error = null;
		importDialogOpen = false;
		try {
			const doc = await invoke<PdfDocument>('pdf_import', { filePath: importPath.trim() });
			await loadDocuments();
			activeDoc = doc;
			importPath = '';
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			importing = false;
		}
	}

	function importPdf() {
		triggerFileInput();
	}

	async function deleteDocument(id: string) {
		error = null;
		try {
			await invoke('pdf_delete', { id });
			if (activeDoc?.id === id) {
				activeDoc = null;
				extractedText = '';
			}
			await loadDocuments();
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- AI Operations -------------------------------------------------------
	async function aiSummarize() {
		if (!activeDoc) return;
		aiLoading = true;
		aiError = null;
		aiResult = '';
		try {
			const result = await invoke<PdfAiResult>('pdf_ai_summarize', {
				id: activeDoc.id,
				model: null,
			});
			aiResult = result.result;
		} catch (e: unknown) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	async function aiAsk() {
		if (!activeDoc || !aiQuestion.trim()) return;
		aiLoading = true;
		aiError = null;
		aiResult = '';
		try {
			const result = await invoke<PdfAiResult>('pdf_ai_ask', {
				id: activeDoc.id,
				question: aiQuestion,
				model: null,
			});
			aiResult = result.result;
		} catch (e: unknown) {
			aiError = e instanceof Error ? e.message : String(e);
		} finally {
			aiLoading = false;
		}
	}

	async function copyAiResult() {
		if (!aiResult) return;
		try {
			await navigator.clipboard.writeText(aiResult);
			aiCopied = true;
			setTimeout(() => { aiCopied = false; }, 2000);
		} catch {
			// Clipboard API may not be available in all WebView contexts
		}
	}

	// ---- Convert Operations --------------------------------------------------
	async function convertToText() {
		if (!activeDoc) return;
		converting = true;
		convertResult = null;
		error = null;
		try {
			const result = await invoke<PdfConvertResult>('pdf_convert_to_text', {
				id: activeDoc.id,
			});
			convertResult = `Saved to: ${result.output_path} (${result.char_count.toLocaleString()} chars)`;
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			converting = false;
		}
	}

	async function convertToMarkdown() {
		if (!activeDoc) return;
		converting = true;
		convertResult = null;
		error = null;
		try {
			const result = await invoke<PdfConvertResult>('pdf_convert_to_markdown', {
				id: activeDoc.id,
			});
			convertResult = `Saved to: ${result.output_path} (${result.char_count.toLocaleString()} chars)`;
		} catch (e: unknown) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			converting = false;
		}
	}

	// ---- Helpers -------------------------------------------------------------
	function formatFileSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function formatDate(iso: string): string {
		try {
			return new Date(iso).toLocaleDateString(undefined, {
				year: 'numeric',
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit',
			});
		} catch {
			return iso;
		}
	}

	function handleImportKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') importFromPath();
		if (e.key === 'Escape') importDialogOpen = false;
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadDocuments();
	});
</script>

<div class="flex h-full {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Sidebar: PDF List -->
	{#if sidebarOpen}
		<div class="flex flex-col w-72 border-r border-gx-border-default bg-gx-bg-secondary shrink-0">
			<!-- Sidebar Header -->
			<div class="flex items-center gap-2 h-10 px-3 border-b border-gx-border-default shrink-0">
				<FileText size={16} class="text-gx-neon" />
				<span class="text-sm font-semibold text-gx-text-primary">ForgePDF</span>
				<div class="flex-1"></div>
				<button
					onclick={importPdf}
					disabled={importing}
					class="flex items-center gap-1 px-2 py-1 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors disabled:opacity-50"
				>
					{#if importing}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<Upload size={12} />
					{/if}
					Import
				</button>
			</div>

			<!-- Search -->
			<div class="px-3 py-2">
				<div class="relative">
					<Search size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
					<input
						type="text"
						placeholder="Search PDFs..."
						bind:value={searchQuery}
						class="w-full pl-8 pr-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon transition-colors"
					/>
				</div>
			</div>

			<!-- Document List -->
			<div class="flex-1 overflow-y-auto">
				{#if loading}
					<div class="flex items-center justify-center py-8">
						<Loader2 size={20} class="animate-spin text-gx-text-muted" />
					</div>
				{:else if filteredDocs.length === 0}
					<div class="flex flex-col items-center gap-2 py-8 px-4 text-center">
						<FileText size={32} class="text-gx-text-muted/30" />
						<p class="text-xs text-gx-text-muted">
							{searchQuery ? 'No PDFs match your search' : 'No PDFs imported yet'}
						</p>
						{#if !searchQuery}
							<button
								onclick={importPdf}
								class="text-xs text-gx-neon hover:underline"
							>
								Import your first PDF
							</button>
						{/if}
					</div>
				{:else}
					{#each filteredDocs as doc (doc.id)}
						<div
							onclick={() => openDocument(doc.id)}
							onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') openDocument(doc.id); }}
							role="button"
							tabindex="0"
							class="w-full text-left px-3 py-2.5 border-b border-gx-border-default/50 hover:bg-gx-bg-hover transition-colors group cursor-pointer
								{activeDoc?.id === doc.id ? 'bg-gx-bg-elevated border-l-2 border-l-gx-neon' : ''}"
						>
							<div class="flex items-start gap-2">
								<FileText size={14} class="mt-0.5 shrink-0 {activeDoc?.id === doc.id ? 'text-gx-neon' : 'text-gx-text-muted'}" />
								<div class="flex-1 min-w-0">
									<p class="text-xs font-medium text-gx-text-primary truncate">{doc.title}</p>
									<div class="flex items-center gap-2 mt-1 text-[10px] text-gx-text-muted">
										<span>{doc.page_count} {doc.page_count === 1 ? 'page' : 'pages'}</span>
										<span class="text-gx-border-default">|</span>
										<span>{formatFileSize(doc.file_size)}</span>
									</div>
								</div>
								<button
									onclick={(e) => { e.stopPropagation(); deleteDocument(doc.id); }}
									class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-gx-status-error/20 text-gx-text-muted hover:text-gx-status-error transition-all"
									aria-label="Delete PDF"
								>
									<Trash2 size={12} />
								</button>
							</div>
						</div>
					{/each}
				{/if}
			</div>

			<!-- Sidebar Footer -->
			<div class="px-3 py-2 border-t border-gx-border-default text-[10px] text-gx-text-muted">
				{documents.length} {documents.length === 1 ? 'document' : 'documents'}
			</div>
		</div>
	{/if}

	<!-- Main Content Area -->
	<div class="flex flex-col flex-1 min-w-0">
		<!-- Toolbar -->
		<div class="flex items-center gap-2 h-10 px-3 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
			<button
				onclick={() => sidebarOpen = !sidebarOpen}
				class="p-1 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
				aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
			>
				{#if sidebarOpen}
					<PanelRightClose size={16} />
				{:else}
					<PanelRightOpen size={16} />
				{/if}
			</button>

			{#if activeDoc}
				<Separator orientation="vertical" class="h-4 bg-gx-border-default" />
				<FileText size={14} class="text-gx-neon shrink-0" />
				<span class="text-sm font-medium text-gx-text-primary truncate">{activeDoc.title}</span>
				<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-border-default text-gx-text-muted shrink-0">
					{activeDoc.page_count} {activeDoc.page_count === 1 ? 'page' : 'pages'}
				</Badge>
				<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-border-default text-gx-text-muted shrink-0">
					{formatFileSize(activeDoc.file_size)}
				</Badge>
			{/if}

			<div class="flex-1"></div>

			{#if activeDoc}
				<!-- Extract Text -->
				<button
					onclick={extractText}
					disabled={extracting}
					class="flex items-center gap-1 px-2 py-1 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors disabled:opacity-50"
				>
					{#if extracting}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<Eye size={12} />
					{/if}
					Extract Text
				</button>

				<Separator orientation="vertical" class="h-4 bg-gx-border-default" />

				<!-- Convert buttons -->
				<button
					onclick={convertToText}
					disabled={converting}
					class="flex items-center gap-1 px-2 py-1 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors disabled:opacity-50"
				>
					<Type size={12} />
					.txt
				</button>
				<button
					onclick={convertToMarkdown}
					disabled={converting}
					class="flex items-center gap-1 px-2 py-1 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors disabled:opacity-50"
				>
					<Hash size={12} />
					.md
				</button>

				<Separator orientation="vertical" class="h-4 bg-gx-border-default" />

				<!-- AI Panel toggle -->
				<button
					onclick={() => aiPanelOpen = !aiPanelOpen}
					class="flex items-center gap-1 px-2 py-1 text-xs rounded-gx transition-colors
						{aiPanelOpen ? 'bg-gx-accent-purple/20 text-gx-accent-purple' : 'text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover'}"
				>
					<Sparkles size={12} />
					AI
				</button>
			{/if}
		</div>

		<!-- Error Banner -->
		{#if error}
			<div class="flex items-center gap-2 px-4 py-2 bg-gx-status-error/10 border-b border-gx-status-error/30 text-xs text-gx-status-error">
				<AlertCircle size={14} />
				<span class="flex-1">{error}</span>
				<button onclick={() => error = null} class="p-0.5 rounded hover:bg-gx-status-error/20">
					<X size={12} />
				</button>
			</div>
		{/if}

		<!-- Convert Result Banner -->
		{#if convertResult}
			<div class="flex items-center gap-2 px-4 py-2 bg-gx-status-success/10 border-b border-gx-status-success/30 text-xs text-gx-status-success">
				<Check size={14} />
				<span class="flex-1">{convertResult}</span>
				<button onclick={() => convertResult = null} class="p-0.5 rounded hover:bg-gx-status-success/20">
					<X size={12} />
				</button>
			</div>
		{/if}

		<!-- Main Body -->
		<div class="flex flex-1 overflow-hidden">
			<!-- Document Viewer / Text Area -->
			<div class="flex-1 overflow-y-auto">
				{#if !activeDoc}
					<!-- Empty State -->
					<div class="flex flex-col items-center justify-center h-full gap-4 text-center px-8">
						<div class="w-20 h-20 rounded-2xl bg-gx-bg-elevated flex items-center justify-center">
							<FileText size={40} class="text-gx-text-muted/30" />
						</div>
						<div>
							<h2 class="text-lg font-semibold text-gx-text-primary mb-1">ForgePDF</h2>
							<p class="text-sm text-gx-text-muted max-w-md">
								Import, view, and analyze PDF documents with AI-powered tools.
								Extract text, summarize content, ask questions, and convert to other formats.
							</p>
						</div>
						<button
							onclick={importPdf}
							disabled={importing}
							class="flex items-center gap-2 px-4 py-2 rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 border border-gx-neon/30 transition-colors disabled:opacity-50"
						>
							{#if importing}
								<Loader2 size={16} class="animate-spin" />
							{:else}
								<Upload size={16} />
							{/if}
							Import PDF
						</button>
						<div class="flex items-center gap-4 text-[11px] text-gx-text-muted mt-2">
							<span class="flex items-center gap-1"><Eye size={11} /> View</span>
							<span class="flex items-center gap-1"><Sparkles size={11} /> Summarize</span>
							<span class="flex items-center gap-1"><MessageSquare size={11} /> Ask AI</span>
							<span class="flex items-center gap-1"><Download size={11} /> Convert</span>
						</div>
					</div>
				{:else}
					<div class="p-6 max-w-4xl mx-auto">
						<!-- Document Info Card -->
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 mb-4">
							<div class="flex items-start gap-3">
								<div class="w-12 h-12 rounded-lg bg-gx-status-error/10 flex items-center justify-center shrink-0">
									<FileText size={24} class="text-gx-status-error" />
								</div>
								<div class="flex-1 min-w-0">
									<h2 class="text-base font-semibold text-gx-text-primary truncate">{activeDoc.title}</h2>
									<div class="flex flex-wrap items-center gap-3 mt-1.5 text-xs text-gx-text-muted">
										<span class="flex items-center gap-1">
											<BookOpen size={11} />
											{activeDoc.page_count} {activeDoc.page_count === 1 ? 'page' : 'pages'}
										</span>
										<span class="flex items-center gap-1">
											<HardDrive size={11} />
											{formatFileSize(activeDoc.file_size)}
										</span>
										<span class="flex items-center gap-1">
											<Calendar size={11} />
											{formatDate(activeDoc.imported_at)}
										</span>
									</div>
								</div>
							</div>

							<!-- Text Preview -->
							{#if activeDoc.text_preview}
								<div class="mt-3 pt-3 border-t border-gx-border-default">
									<p class="text-[11px] font-medium text-gx-text-muted mb-1">Preview</p>
									<p class="text-xs text-gx-text-secondary leading-relaxed line-clamp-3">
										{activeDoc.text_preview}
									</p>
								</div>
							{/if}
						</div>

						<!-- Extracted Text -->
						{#if extracting}
							<div class="flex items-center justify-center py-12">
								<Loader2 size={24} class="animate-spin text-gx-neon" />
								<span class="ml-2 text-sm text-gx-text-muted">Extracting text...</span>
							</div>
						{:else if extractedText}
							<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary">
								<div class="flex items-center gap-2 px-4 py-2 border-b border-gx-border-default">
									<Eye size={14} class="text-gx-neon" />
									<span class="text-xs font-medium text-gx-text-secondary">Extracted Text</span>
									<div class="flex-1"></div>
									<span class="text-[10px] text-gx-text-muted">
										{extractedText.length.toLocaleString()} chars
									</span>
								</div>
								<div class="p-4 max-h-[60vh] overflow-y-auto">
									<pre class="text-xs text-gx-text-secondary whitespace-pre-wrap font-mono leading-relaxed">{extractedText}</pre>
								</div>
							</div>
						{:else}
							<!-- Prompt to extract -->
							<div class="flex flex-col items-center gap-3 py-12 text-center">
								<Eye size={32} class="text-gx-text-muted/30" />
								<p class="text-sm text-gx-text-muted">Click "Extract Text" to view the document content</p>
								<div class="flex items-center gap-2">
									<button
										onclick={extractText}
										class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx bg-gx-bg-elevated text-gx-text-secondary hover:text-gx-neon hover:border-gx-neon border border-gx-border-default transition-colors"
									>
										<Eye size={12} />
										Extract Text
									</button>
									<button
										onclick={aiSummarize}
										disabled={aiLoading}
										class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx bg-gx-accent-purple/10 text-gx-accent-purple hover:bg-gx-accent-purple/20 border border-gx-accent-purple/30 transition-colors disabled:opacity-50"
									>
										{#if aiLoading}
											<Loader2 size={12} class="animate-spin" />
										{:else}
											<Sparkles size={12} />
										{/if}
										AI Summarize
									</button>
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</div>

			<!-- AI Panel (Right, Collapsible) -->
			{#if aiPanelOpen && activeDoc}
				<div class="w-80 border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0">
					<!-- AI Panel Header -->
					<div class="flex items-center gap-2 h-10 px-3 border-b border-gx-border-default shrink-0">
						<Sparkles size={14} class="text-gx-accent-purple" />
						<span class="text-xs font-semibold text-gx-text-primary">AI Analysis</span>
						<div class="flex-1"></div>
						<button
							onclick={() => aiPanelOpen = false}
							class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary transition-colors"
						>
							<X size={14} />
						</button>
					</div>

					<!-- AI Actions -->
					<div class="p-3 space-y-2 border-b border-gx-border-default">
						<button
							onclick={aiSummarize}
							disabled={aiLoading}
							class="w-full flex items-center gap-2 px-3 py-2 text-xs rounded-gx bg-gx-accent-purple/10 text-gx-accent-purple hover:bg-gx-accent-purple/20 border border-gx-accent-purple/30 transition-colors disabled:opacity-50"
						>
							{#if aiLoading}
								<Loader2 size={14} class="animate-spin" />
							{:else}
								<BookOpen size={14} />
							{/if}
							Summarize Document
						</button>

						<!-- Ask Question -->
						<div class="flex gap-1.5">
							<input
								type="text"
								placeholder="Ask a question..."
								bind:value={aiQuestion}
								onkeydown={(e) => { if (e.key === 'Enter') aiAsk(); }}
								class="flex-1 px-2.5 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-accent-purple transition-colors"
							/>
							<button
								onclick={aiAsk}
								disabled={aiLoading || !aiQuestion.trim()}
								class="px-2 py-1.5 rounded-gx bg-gx-accent-purple/10 text-gx-accent-purple hover:bg-gx-accent-purple/20 transition-colors disabled:opacity-50"
							>
								<Send size={12} />
							</button>
						</div>
					</div>

					<!-- AI Result -->
					<div class="flex-1 overflow-y-auto p-3">
						{#if aiLoading}
							<div class="flex flex-col items-center gap-2 py-8">
								<Loader2 size={20} class="animate-spin text-gx-accent-purple" />
								<span class="text-xs text-gx-text-muted">Analyzing document...</span>
							</div>
						{:else if aiError}
							<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/10 p-3">
								<div class="flex items-start gap-2">
									<AlertCircle size={14} class="text-gx-status-error mt-0.5 shrink-0" />
									<p class="text-xs text-gx-status-error">{aiError}</p>
								</div>
							</div>
						{:else if aiResult}
							<div class="space-y-2">
								<div class="flex items-center justify-between">
									<span class="text-[10px] font-medium text-gx-text-muted uppercase tracking-wider">Result</span>
									<button
										onclick={copyAiResult}
										class="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded text-gx-text-muted hover:text-gx-text-primary transition-colors"
									>
										{#if aiCopied}
											<Check size={10} class="text-gx-status-success" />
											Copied
										{:else}
											<Copy size={10} />
											Copy
										{/if}
									</button>
								</div>
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<p class="text-xs text-gx-text-secondary leading-relaxed whitespace-pre-wrap">{aiResult}</p>
								</div>
							</div>
						{:else}
							<div class="flex flex-col items-center gap-2 py-8 text-center">
								<Sparkles size={24} class="text-gx-text-muted/30" />
								<p class="text-xs text-gx-text-muted">
									Summarize the document or ask a question to get AI-powered insights.
								</p>
							</div>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<!-- Hidden file input for PDF import -->
<input
	bind:this={fileInputEl}
	type="file"
	accept=".pdf,application/pdf"
	class="hidden"
	onchange={handleFileSelected}
/>

<!-- Manual path import dialog (fallback when WebView does not expose file paths) -->
{#if importDialogOpen}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="dialog" aria-modal="true">
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-lg shadow-gx-glow-lg w-[420px] p-4">
			<div class="flex items-center gap-2 mb-3">
				<Upload size={16} class="text-gx-neon" />
				<h3 class="text-sm font-semibold text-gx-text-primary">Import PDF</h3>
				<div class="flex-1"></div>
				<button onclick={() => importDialogOpen = false} class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary">
					<X size={14} />
				</button>
			</div>
			<p class="text-xs text-gx-text-muted mb-3">Enter the full path to the PDF file:</p>
			<input
				type="text"
				placeholder="/home/user/documents/file.pdf"
				bind:value={importPath}
				onkeydown={handleImportKeydown}
				class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon transition-colors font-mono"
			/>
			<div class="flex justify-end gap-2 mt-3">
				<button
					onclick={() => importDialogOpen = false}
					class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={importFromPath}
					disabled={!importPath.trim() || importing}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 border border-gx-neon/30 transition-colors disabled:opacity-50"
				>
					{#if importing}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<Upload size={12} />
					{/if}
					Import
				</button>
			</div>
		</div>
	</div>
{/if}
