<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		FolderOpen, Upload, Download, Sparkles, Search, Trash2,
		Loader2, AlertCircle, X, Eye, FileText, Table2, Code2,
		Image, Video, Music, Archive, Mail, Type, Box,
		ArrowRight, Check, Copy, RefreshCw, ChevronDown, Filter,
		FileEdit, Layers, Zap, Brain, Clock, HardDrive,
		FileSpreadsheet, Presentation, Globe
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-files';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface FileInfo {
		path: string;
		name: string;
		extension: string;
		category: string;
		size_bytes: number;
		mime_type: string;
		can_preview: boolean;
		can_edit: boolean;
		can_convert_to: string[];
		recommended_module: string;
		metadata: Record<string, unknown>;
	}

	interface ConversionResult {
		success: boolean;
		output_path: string;
		source_format: string;
		target_format: string;
		message: string;
	}

	interface FormatInfo {
		extension: string;
		category: string;
		mime_type: string;
		can_preview: boolean;
		can_edit: boolean;
		can_convert_to: string[];
		description: string;
	}

	// ---- State ---------------------------------------------------------------
	let recentFiles = $state<FileInfo[]>([]);
	let supportedFormats = $state<FormatInfo[]>([]);
	let selectedFile = $state<FileInfo | null>(null);
	let previewContent = $state('');
	let loading = $state(true);
	let converting = $state(false);
	let previewing = $state(false);
	let digesting = $state(false);
	let error = $state<string | null>(null);
	let successMessage = $state<string | null>(null);
	let searchQuery = $state('');
	let filterCategory = $state('all');

	// Converter state
	let convertTargetFormat = $state('');
	let conversionResults = $state<ConversionResult[]>([]);
	let batchMode = $state(false);
	let batchFiles = $state<FileInfo[]>([]);

	// AI Digest state
	let digestResult = $state('');
	let digestCopied = $state(false);

	// Drop zone state
	let dragOver = $state(false);

	// File picker dialog state
	let filePickerOpen = $state(false);
	let filePickerPath = $state('');

	// Tabs
	let activeTab = $state<'recent' | 'convert' | 'formats' | 'digest'>('recent');

	// ---- Derived -------------------------------------------------------------
	let filteredRecent = $derived(
		recentFiles.filter(f => {
			const matchesSearch = !searchQuery ||
				f.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				f.extension.toLowerCase().includes(searchQuery.toLowerCase());
			const matchesCategory = filterCategory === 'all' || f.category === filterCategory;
			return matchesSearch && matchesCategory;
		})
	);

	let categories = $derived(
		[...new Set(recentFiles.map(f => f.category))].sort()
	);

	let groupedFormats = $derived(() => {
		const groups: Record<string, FormatInfo[]> = {};
		for (const f of supportedFormats) {
			if (!groups[f.category]) groups[f.category] = [];
			groups[f.category].push(f);
		}
		return groups;
	});

	// ---- Functions -----------------------------------------------------------
	async function loadRecentFiles() {
		try {
			recentFiles = await invoke<FileInfo[]>('file_recent');
		} catch (e) {
			console.error('Failed to load recent files:', e);
		}
	}

	async function loadSupportedFormats() {
		try {
			supportedFormats = await invoke<FormatInfo[]>('file_supported_formats');
		} catch (e) {
			console.error('Failed to load supported formats:', e);
		}
	}

	async function detectFile(path: string) {
		error = null;
		try {
			const info = await invoke<FileInfo>('file_detect', { path });
			selectedFile = info;
			convertTargetFormat = info.can_convert_to[0] ?? '';
			await loadRecentFiles();
			return info;
		} catch (e) {
			error = parseError(e);
			return null;
		}
	}

	async function previewFile(path: string) {
		previewing = true;
		previewContent = '';
		error = null;
		try {
			previewContent = await invoke<string>('file_preview', { path });
		} catch (e) {
			error = parseError(e);
		} finally {
			previewing = false;
		}
	}

	async function openInModule(path: string) {
		try {
			const route = await invoke<string>('file_open_in_module', { path });
			goto(route);
		} catch (e) {
			error = parseError(e);
		}
	}

	async function convertFile() {
		if (!selectedFile || !convertTargetFormat) return;
		converting = true;
		error = null;
		conversionResults = [];
		try {
			if (batchMode && batchFiles.length > 0) {
				const paths = batchFiles.map(f => f.path);
				conversionResults = await invoke<ConversionResult[]>('file_batch_convert', {
					paths,
					targetFormat: convertTargetFormat,
				});
			} else {
				const result = await invoke<ConversionResult>('file_convert', {
					path: selectedFile.path,
					targetFormat: convertTargetFormat,
				});
				conversionResults = [result];
			}
			successMessage = `Converted ${conversionResults.filter(r => r.success).length} file(s) successfully`;
			setTimeout(() => { successMessage = null; }, 4000);
		} catch (e) {
			error = parseError(e);
		} finally {
			converting = false;
		}
	}

	async function aiDigest(path: string) {
		digesting = true;
		digestResult = '';
		error = null;
		try {
			digestResult = await invoke<string>('file_ai_digest', { path });
		} catch (e) {
			error = parseError(e);
		} finally {
			digesting = false;
		}
	}

	function pickFile() {
		filePickerPath = '';
		filePickerOpen = true;
	}

	async function submitPickedFile() {
		const path = filePickerPath.trim();
		if (!path) return;
		filePickerOpen = false;

		if (batchMode) {
			const info = await detectFile(path);
			if (info) {
				batchFiles = [...batchFiles, info];
				selectedFile = batchFiles[0];
			}
		} else {
			await detectFile(path);
			if (selectedFile) {
				await previewFile(path);
			}
		}
	}

	function handlePickerKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') submitPickedFile();
		if (e.key === 'Escape') filePickerOpen = false;
	}

	function handleDrop(event: DragEvent) {
		event.preventDefault();
		dragOver = false;

		const files = event.dataTransfer?.files;
		if (!files || files.length === 0) return;

		// Tauri file drop provides the path via dataTransfer
		const items = event.dataTransfer?.items;
		if (items) {
			for (let i = 0; i < items.length; i++) {
				const item = items[i];
				if (item.kind === 'file') {
					const file = item.getAsFile();
					if (file && (file as any).path) {
						detectFile((file as any).path).then(info => {
							if (info) previewFile(info.path);
						});
						return;
					}
				}
			}
		}

		// Fallback: use first file name
		const file = files[0];
		if (file && file.name) {
			// In Tauri desktop, dropped files have a path property
			const path = (file as any).path || file.name;
			detectFile(path).then(info => {
				if (info) previewFile(info.path);
			});
		}
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		dragOver = true;
	}

	function handleDragLeave() {
		dragOver = false;
	}

	function copyDigest() {
		navigator.clipboard.writeText(digestResult);
		digestCopied = true;
		setTimeout(() => { digestCopied = false; }, 2000);
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
	}

	function categoryIcon(category: string) {
		switch (category) {
			case 'document': return FileEdit;
			case 'spreadsheet': return Table2;
			case 'presentation': return Presentation;
			case 'pdf': return FileText;
			case 'image': return Image;
			case 'video': return Video;
			case 'audio': return Music;
			case 'archive': return Archive;
			case 'code': return Code2;
			case 'data': return Layers;
			case 'email': return Mail;
			case 'font': return Type;
			case 'three_d': return Box;
			default: return FolderOpen;
		}
	}

	function categoryColor(category: string): string {
		switch (category) {
			case 'document': return 'text-blue-400';
			case 'spreadsheet': return 'text-green-400';
			case 'presentation': return 'text-orange-400';
			case 'pdf': return 'text-red-400';
			case 'image': return 'text-purple-400';
			case 'video': return 'text-pink-400';
			case 'audio': return 'text-yellow-400';
			case 'archive': return 'text-gray-400';
			case 'code': return 'text-cyan-400';
			case 'data': return 'text-teal-400';
			case 'email': return 'text-indigo-400';
			case 'font': return 'text-violet-400';
			case 'three_d': return 'text-amber-400';
			default: return 'text-gx-text-muted';
		}
	}

	function parseError(e: unknown): string {
		if (typeof e === 'string') {
			try {
				const parsed = JSON.parse(e);
				return parsed.message || e;
			} catch { return e; }
		}
		return String(e);
	}

	function removeBatchFile(index: number) {
		batchFiles = batchFiles.filter((_, i) => i !== index);
		if (batchFiles.length === 0) selectedFile = null;
	}

	onMount(async () => {
		loading = true;
		await Promise.all([loadRecentFiles(), loadSupportedFormats()]);
		loading = false;

		// Check for ?preview= query param (auto-open from file_open_in_module)
		const previewPath = $page.url.searchParams.get('preview');
		if (previewPath) {
			const decoded = decodeURIComponent(previewPath);
			await detectFile(decoded);
			if (selectedFile) await previewFile(decoded);
		}
	});
</script>

<div
	class="h-full overflow-y-auto p-6 space-y-6"
	style={containerStyle}
	ondrop={handleDrop}
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
>
	<!-- Header -->
	<div class="flex items-center gap-3">
		<div class="w-10 h-10 rounded-gx bg-gx-neon/10 flex items-center justify-center">
			<FolderOpen size={22} class="text-gx-neon" />
		</div>
		<div>
			<h1 class="text-xl font-bold text-gx-text-primary">File Hub</h1>
			<p class="text-sm text-gx-text-muted">Detect, preview, convert, and route any file format</p>
		</div>
		<div class="flex-1"></div>
		<button
			onclick={pickFile}
			class="flex items-center gap-2 px-4 py-2 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-all text-sm font-medium"
		>
			<Upload size={16} />
			Open File
		</button>
	</div>

	<!-- Error/Success Banner -->
	{#if error}
		<div class="flex items-center gap-2 p-3 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-gx-status-error text-sm">
			<AlertCircle size={16} />
			<span class="flex-1">{error}</span>
			<button onclick={() => error = null}><X size={14} /></button>
		</div>
	{/if}

	{#if successMessage}
		<div class="flex items-center gap-2 p-3 rounded-gx bg-gx-status-success/10 border border-gx-status-success/30 text-gx-status-success text-sm">
			<Check size={16} />
			<span>{successMessage}</span>
		</div>
	{/if}

	<!-- Drop Zone (prominent when no file selected) -->
	{#if !selectedFile}
		<div
			class="relative border-2 border-dashed rounded-gx p-12 text-center transition-all
				{dragOver
					? 'border-gx-neon bg-gx-neon/5 scale-[1.01]'
					: 'border-gx-border-default hover:border-gx-neon/50 bg-gx-bg-secondary/50'}"
		>
			<FolderOpen size={48} class="mx-auto mb-4 {dragOver ? 'text-gx-neon' : 'text-gx-text-muted'} transition-colors" />
			<p class="text-lg font-medium {dragOver ? 'text-gx-neon' : 'text-gx-text-secondary'}">
				{dragOver ? 'Release to analyze file' : 'Drop any file here'}
			</p>
			<p class="text-sm text-gx-text-muted mt-2">
				ImpForge recognizes 100+ formats -- documents, spreadsheets, code, images, video, 3D models, and more
			</p>
			<button
				onclick={pickFile}
				class="mt-4 px-6 py-2 rounded-gx bg-gx-neon text-gx-bg-primary font-medium hover:opacity-90 transition-opacity"
			>
				Browse Files
			</button>
		</div>
	{/if}

	<!-- Selected File Info Card -->
	{#if selectedFile}
		{@const IconComp = categoryIcon(selectedFile.category)}
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
			<div class="flex items-start gap-4">
				<!-- Category Icon -->
				<div class="w-12 h-12 rounded-gx bg-gx-bg-elevated flex items-center justify-center shrink-0">
					<IconComp size={24} class={categoryColor(selectedFile.category)} />
				</div>

				<!-- File Details -->
				<div class="flex-1 min-w-0">
					<h2 class="text-lg font-semibold text-gx-text-primary truncate">{selectedFile.name}</h2>
					<div class="flex items-center gap-3 mt-1 text-sm text-gx-text-muted flex-wrap">
						<span class="flex items-center gap-1">
							<HardDrive size={12} />
							{formatSize(selectedFile.size_bytes)}
						</span>
						<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-neon/30 text-gx-neon capitalize">
							{selectedFile.category.replace('_', ' ')}
						</Badge>
						<span class="font-mono text-xs">.{selectedFile.extension}</span>
						<span class="text-gx-text-muted">{selectedFile.mime_type}</span>
					</div>
					<div class="flex items-center gap-2 mt-2 flex-wrap">
						{#if selectedFile.can_preview}
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-status-success/30 text-gx-status-success">Preview</Badge>
						{/if}
						{#if selectedFile.can_edit}
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-blue-400/30 text-blue-400">Editable</Badge>
						{/if}
						{#if selectedFile.can_convert_to.length > 0}
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-purple-400/30 text-purple-400">
								Convert to: {selectedFile.can_convert_to.join(', ')}
							</Badge>
						{/if}
					</div>
				</div>

				<!-- Action Buttons -->
				<div class="flex flex-col gap-2 shrink-0">
					<button
						onclick={() => openInModule(selectedFile!.path)}
						class="flex items-center gap-2 px-3 py-1.5 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-all text-xs font-medium"
					>
						<ArrowRight size={14} />
						Open in {selectedFile.recommended_module === 'files' ? 'Preview' : selectedFile.recommended_module.charAt(0).toUpperCase() + selectedFile.recommended_module.slice(1)}
					</button>
					{#if selectedFile.can_preview}
						<button
							onclick={() => previewFile(selectedFile!.path)}
							disabled={previewing}
							class="flex items-center gap-2 px-3 py-1.5 rounded-gx bg-gx-bg-elevated text-gx-text-secondary border border-gx-border-default hover:border-gx-neon/30 transition-all text-xs"
						>
							{#if previewing}
								<Loader2 size={14} class="animate-spin" />
							{:else}
								<Eye size={14} />
							{/if}
							Preview
						</button>
					{/if}
					<button
						onclick={() => { selectedFile = null; previewContent = ''; digestResult = ''; }}
						class="flex items-center gap-2 px-3 py-1.5 rounded-gx text-gx-text-muted hover:text-gx-status-error transition-all text-xs"
					>
						<X size={14} />
						Clear
					</button>
				</div>
			</div>

			<!-- Preview area (collapsible) -->
			{#if previewContent}
				<div class="mt-4 border-t border-gx-border-default pt-4">
					<div class="flex items-center gap-2 mb-2">
						<Eye size={14} class="text-gx-text-muted" />
						<span class="text-xs font-medium text-gx-text-secondary">Preview</span>
					</div>
					<pre class="text-xs font-mono text-gx-text-secondary bg-gx-bg-primary rounded-gx p-3 max-h-64 overflow-auto whitespace-pre-wrap break-words border border-gx-border-default">{previewContent}</pre>
				</div>
			{/if}
		</div>

		<!-- Compact drop zone when file is selected -->
		<div
			class="border border-dashed rounded-gx p-4 text-center transition-all
				{dragOver
					? 'border-gx-neon bg-gx-neon/5'
					: 'border-gx-border-default hover:border-gx-neon/30 bg-gx-bg-secondary/30'}"
		>
			<p class="text-sm text-gx-text-muted">
				<FolderOpen size={14} class="inline mr-1" />
				Drop another file here or <button onclick={pickFile} class="text-gx-neon hover:underline">browse</button>
			</p>
		</div>
	{/if}

	<!-- Tabs -->
	<div class="flex items-center gap-1 border-b border-gx-border-default">
		{#each [
			{ id: 'recent' as const, label: 'Recent Files', icon: Clock },
			{ id: 'convert' as const, label: 'Converter', icon: RefreshCw },
			{ id: 'digest' as const, label: 'AI Digest', icon: Brain },
			{ id: 'formats' as const, label: 'Supported Formats', icon: Layers },
		] as tab}
			<button
				onclick={() => activeTab = tab.id}
				class="flex items-center gap-1.5 px-3 py-2 text-sm font-medium border-b-2 transition-all
					{activeTab === tab.id
						? 'border-gx-neon text-gx-neon'
						: 'border-transparent text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				<tab.icon size={14} />
				{tab.label}
			</button>
		{/each}
	</div>

	<!-- Tab Content -->
	{#if activeTab === 'recent'}
		<!-- Recent Files -->
		<div class="space-y-3">
			<!-- Search & Filter -->
			<div class="flex items-center gap-3">
				<div class="relative flex-1">
					<Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-gx-text-muted" />
					<input
						type="text"
						placeholder="Search recent files..."
						bind:value={searchQuery}
						class="w-full pl-9 pr-3 py-2 text-sm bg-gx-bg-secondary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon/50"
					/>
				</div>
				<div class="flex items-center gap-1">
					<Filter size={14} class="text-gx-text-muted" />
					<select
						bind:value={filterCategory}
						class="text-xs bg-gx-bg-secondary border border-gx-border-default rounded-gx px-2 py-1.5 text-gx-text-secondary focus:outline-none focus:border-gx-neon/50"
					>
						<option value="all">All Categories</option>
						{#each categories as cat}
							<option value={cat}>{cat.replace('_', ' ')}</option>
						{/each}
					</select>
				</div>
			</div>

			{#if loading}
				<div class="flex items-center justify-center py-12 text-gx-text-muted">
					<Loader2 size={24} class="animate-spin mr-2" />
					Loading...
				</div>
			{:else if filteredRecent.length === 0}
				<div class="text-center py-12 text-gx-text-muted">
					<Clock size={32} class="mx-auto mb-3 opacity-30" />
					<p>No recent files</p>
					<p class="text-xs mt-1">Open or drop a file to get started</p>
				</div>
			{:else}
				<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
					{#each filteredRecent as file}
						{@const IconComp = categoryIcon(file.category)}
						<button
							onclick={() => { detectFile(file.path); previewFile(file.path); }}
							class="flex items-center gap-3 p-3 rounded-gx border border-gx-border-default bg-gx-bg-secondary hover:border-gx-neon/30 hover:bg-gx-bg-elevated transition-all text-left group"
						>
							<div class="w-8 h-8 rounded bg-gx-bg-primary flex items-center justify-center shrink-0">
								<IconComp size={16} class={categoryColor(file.category)} />
							</div>
							<div class="min-w-0 flex-1">
								<p class="text-sm font-medium text-gx-text-primary truncate group-hover:text-gx-neon transition-colors">
									{file.name}
								</p>
								<p class="text-[11px] text-gx-text-muted">
									{formatSize(file.size_bytes)} -- .{file.extension}
								</p>
							</div>
							<ArrowRight size={14} class="text-gx-text-muted opacity-0 group-hover:opacity-100 transition-opacity shrink-0" />
						</button>
					{/each}
				</div>
			{/if}
		</div>

	{:else if activeTab === 'convert'}
		<!-- File Converter -->
		<div class="space-y-4">
			<!-- Batch mode toggle -->
			<div class="flex items-center gap-3">
				<label class="flex items-center gap-2 text-sm text-gx-text-secondary cursor-pointer">
					<input
						type="checkbox"
						bind:checked={batchMode}
						class="rounded border-gx-border-default bg-gx-bg-secondary accent-gx-neon"
					/>
					Batch mode (convert multiple files)
				</label>
			</div>

			{#if !selectedFile && batchFiles.length === 0}
				<div class="text-center py-8 text-gx-text-muted">
					<RefreshCw size={32} class="mx-auto mb-3 opacity-30" />
					<p>Select a file to convert</p>
					<button
						onclick={pickFile}
						class="mt-3 px-4 py-2 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-all text-sm"
					>
						Choose File{batchMode ? 's' : ''}
					</button>
				</div>
			{:else}
				<!-- Batch file list -->
				{#if batchMode && batchFiles.length > 0}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3 space-y-2">
						<span class="text-xs font-medium text-gx-text-muted">{batchFiles.length} file(s) selected</span>
						{#each batchFiles as file, i}
							<div class="flex items-center gap-2 text-sm">
								<span class="text-gx-text-secondary truncate flex-1">{file.name}</span>
								<span class="text-gx-text-muted text-xs">.{file.extension}</span>
								<button onclick={() => removeBatchFile(i)} class="text-gx-text-muted hover:text-gx-status-error">
									<X size={12} />
								</button>
							</div>
						{/each}
						<button onclick={pickFile} class="text-xs text-gx-neon hover:underline">+ Add more files</button>
					</div>
				{/if}

				<!-- Conversion controls -->
				{#if selectedFile && selectedFile.can_convert_to.length > 0}
					<div class="flex items-center gap-3 flex-wrap">
						<span class="text-sm text-gx-text-secondary">Convert to:</span>
						<div class="flex items-center gap-2">
							{#each selectedFile.can_convert_to as fmt}
								<button
									onclick={() => convertTargetFormat = fmt}
									class="px-3 py-1 rounded-gx text-xs font-mono transition-all
										{convertTargetFormat === fmt
											? 'bg-gx-neon/20 text-gx-neon border border-gx-neon/50'
											: 'bg-gx-bg-secondary text-gx-text-muted border border-gx-border-default hover:border-gx-neon/30'}"
								>
									.{fmt}
								</button>
							{/each}
						</div>
						<button
							onclick={convertFile}
							disabled={converting || !convertTargetFormat}
							class="flex items-center gap-2 px-4 py-2 rounded-gx bg-gx-neon text-gx-bg-primary font-medium hover:opacity-90 transition-opacity text-sm disabled:opacity-50 disabled:cursor-not-allowed"
						>
							{#if converting}
								<Loader2 size={14} class="animate-spin" />
								Converting...
							{:else}
								<RefreshCw size={14} />
								Convert
							{/if}
						</button>
					</div>
				{:else if selectedFile}
					<div class="p-3 rounded-gx bg-gx-bg-secondary border border-gx-border-default text-sm text-gx-text-muted">
						No conversion targets available for .{selectedFile.extension} files.
					</div>
				{/if}

				<!-- Conversion Results -->
				{#if conversionResults.length > 0}
					<div class="space-y-2">
						<span class="text-xs font-medium text-gx-text-muted">Results</span>
						{#each conversionResults as result}
							<div class="flex items-center gap-3 p-3 rounded-gx border {result.success ? 'border-gx-status-success/30 bg-gx-status-success/5' : 'border-gx-status-error/30 bg-gx-status-error/5'}">
								{#if result.success}
									<Check size={16} class="text-gx-status-success shrink-0" />
								{:else}
									<AlertCircle size={16} class="text-gx-status-error shrink-0" />
								{/if}
								<div class="flex-1 min-w-0">
									<p class="text-sm {result.success ? 'text-gx-status-success' : 'text-gx-status-error'}">
										{result.message}
									</p>
									{#if result.success}
										<p class="text-[11px] text-gx-text-muted truncate mt-0.5">{result.output_path}</p>
									{/if}
								</div>
								{#if result.success}
									<button
										onclick={() => detectFile(result.output_path)}
										class="text-xs text-gx-neon hover:underline shrink-0"
									>
										Open result
									</button>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			{/if}
		</div>

	{:else if activeTab === 'digest'}
		<!-- AI Digest -->
		<div class="space-y-4">
			{#if !selectedFile}
				<div class="text-center py-8 text-gx-text-muted">
					<Brain size={32} class="mx-auto mb-3 opacity-30" />
					<p>Select a file to generate an AI-powered digest</p>
					<p class="text-xs mt-1">Works with documents, spreadsheets, code, and any text-based file</p>
					<button
						onclick={pickFile}
						class="mt-3 px-4 py-2 rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 transition-all text-sm"
					>
						Choose File
					</button>
				</div>
			{:else}
				<div class="flex items-center gap-3">
					<p class="text-sm text-gx-text-secondary">
						Generate AI digest for <span class="font-medium text-gx-text-primary">{selectedFile.name}</span>
					</p>
					<button
						onclick={() => aiDigest(selectedFile!.path)}
						disabled={digesting}
						class="flex items-center gap-2 px-4 py-2 rounded-gx bg-gx-accent-magenta/10 text-gx-accent-magenta border border-gx-accent-magenta/30 hover:bg-gx-accent-magenta/20 transition-all text-sm font-medium disabled:opacity-50"
					>
						{#if digesting}
							<Loader2 size={14} class="animate-spin" />
							Analyzing...
						{:else}
							<Sparkles size={14} />
							Generate Digest
						{/if}
					</button>
				</div>

				{#if digestResult}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4">
						<div class="flex items-center gap-2 mb-3">
							<Sparkles size={14} class="text-gx-accent-magenta" />
							<span class="text-xs font-medium text-gx-text-secondary">AI Document Digest</span>
							<div class="flex-1"></div>
							<button
								onclick={copyDigest}
								class="flex items-center gap-1 text-xs text-gx-text-muted hover:text-gx-neon transition-colors"
							>
								{#if digestCopied}
									<Check size={12} class="text-gx-status-success" />
									Copied
								{:else}
									<Copy size={12} />
									Copy
								{/if}
							</button>
						</div>
						<div class="prose prose-sm prose-invert max-w-none text-gx-text-secondary text-sm whitespace-pre-wrap">
							{digestResult}
						</div>
					</div>
				{/if}
			{/if}
		</div>

	{:else if activeTab === 'formats'}
		<!-- Supported Formats Table -->
		<div class="space-y-4">
			<p class="text-sm text-gx-text-muted">
				ImpForge supports {supportedFormats.length} file formats across {Object.keys(groupedFormats()).length} categories.
			</p>

			{#each Object.entries(groupedFormats()) as [category, formats]}
				{@const CatIcon = categoryIcon(category)}
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary overflow-hidden">
					<div class="flex items-center gap-2 px-4 py-2 bg-gx-bg-elevated border-b border-gx-border-default">
						<CatIcon size={14} class={categoryColor(category)} />
						<span class="text-sm font-medium text-gx-text-primary capitalize">{category.replace('_', ' ')}</span>
						<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted ml-auto">
							{formats.length}
						</Badge>
					</div>
					<div class="divide-y divide-gx-border-default">
						{#each formats as fmt}
							<div class="flex items-center gap-4 px-4 py-2 text-sm hover:bg-gx-bg-hover/30 transition-colors">
								<span class="font-mono text-gx-neon w-12">.{fmt.extension}</span>
								<span class="text-gx-text-secondary flex-1">{fmt.description}</span>
								<div class="flex items-center gap-2">
									{#if fmt.can_preview}
										<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-status-success/30 text-gx-status-success">Preview</Badge>
									{/if}
									{#if fmt.can_edit}
										<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-blue-400/30 text-blue-400">Edit</Badge>
									{/if}
									{#if fmt.can_convert_to.length > 0}
										<span class="text-[10px] text-gx-text-muted">
											<ArrowRight size={10} class="inline" />
											{fmt.can_convert_to.map(t => `.${t}`).join(', ')}
										</span>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- File Picker Dialog (manual path input — Tauri WebView compatible) -->
{#if filePickerOpen}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="dialog" aria-modal="true">
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-lg shadow-gx-glow-lg w-[480px] p-4">
			<div class="flex items-center gap-2 mb-3">
				<FolderOpen size={16} class="text-gx-neon" />
				<h3 class="text-sm font-semibold text-gx-text-primary">
					{batchMode ? 'Add File to Batch' : 'Open File'}
				</h3>
				<div class="flex-1"></div>
				<button onclick={() => filePickerOpen = false} class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary">
					<X size={14} />
				</button>
			</div>
			<p class="text-xs text-gx-text-muted mb-3">Enter the full path to the file:</p>
			<input
				type="text"
				placeholder="/home/user/documents/file.docx"
				bind:value={filePickerPath}
				onkeydown={handlePickerKeydown}
				class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon transition-colors font-mono"
			/>
			<div class="flex justify-end gap-2 mt-3">
				<button
					onclick={() => filePickerOpen = false}
					class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary"
				>
					Cancel
				</button>
				<button
					onclick={submitPickedFile}
					disabled={!filePickerPath.trim()}
					class="px-4 py-1.5 text-xs rounded-gx bg-gx-neon text-gx-bg-primary font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
				>
					Open
				</button>
			</div>
		</div>
	</div>
{/if}
