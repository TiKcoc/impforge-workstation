<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import {
		PenTool, Plus, Trash2, FileUp, ChevronDown, ChevronRight,
		Link2, Eye, Send, Sparkles, Download, X, File, FileText,
		Table2, Code2, FileJson, Mail, Palette,
		CheckCircle2, ArrowRight, Loader2, Info,
		Hash, ExternalLink
	} from '@lucide/svelte';

	// BenikUI style engine integration
	const widgetId = 'page-canvas';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));

	// ────────────────────────────────────────────────────────────
	// Types
	// ────────────────────────────────────────────────────────────

	interface SourceChunk {
		id: string;
		text: string;
		line_start: number;
		line_end: number;
		source_id: string;
		used_in_output: boolean;
		relevance_score: number;
	}

	interface SourceEntry {
		id: string;
		file_path: string | null;
		file_name: string;
		content: string;
		chunks: SourceChunk[];
		file_type: string;
	}

	interface OutputSourceLink {
		output_section_idx: number;
		chunk_ids: string[];
		confidence: number;
	}

	interface ChatMessage {
		id: string;
		role: string;
		content: string;
		referenced_chunks: string[];
		timestamp: string;
	}

	interface CanvasProject {
		id: string;
		name: string;
		sources: SourceEntry[];
		output_content: string;
		output_type: string;
		template: string | null;
		background: string | null;
		source_links: OutputSourceLink[];
		chat_history: ChatMessage[];
		created_at: string;
		updated_at: string;
	}

	interface CanvasProjectMeta {
		id: string;
		name: string;
		output_type: string;
		source_count: number;
		template: string | null;
		updated_at: string;
	}

	interface TemplateInfo {
		id: string;
		name: string;
		sections: string[];
		has_background: boolean;
		description: string;
	}

	interface ChatResponse {
		message: string;
		referenced_chunks: string[];
		updated_output: string | null;
	}

	// ────────────────────────────────────────────────────────────
	// State
	// ────────────────────────────────────────────────────────────

	let projects = $state<CanvasProjectMeta[]>([]);
	let currentProject = $state<CanvasProject | null>(null);
	let templates = $state<TemplateInfo[]>([]);
	let selectedChunks = $state<Set<string>>(new Set());
	let expandedSources = $state<Set<string>>(new Set());

	// File input ref
	let fileInputEl: HTMLInputElement | undefined = $state();

	// UI state
	let showProjectList = $state(true);
	let newProjectName = $state('');
	let newProjectType = $state('custom');
	let chatInput = $state('');
	let isGenerating = $state(false);
	let isChatting = $state(false);
	let isAddingSource = $state(false);
	let editMode = $state(false);
	let hoveredSection = $state<number | null>(null);
	let errorMessage = $state('');
	let successMessage = $state('');

	// Background & template
	let selectedBackground = $state('dark');
	let selectedTemplate = $state('');

	const backgrounds: Record<string, { label: string; css: string }> = {
		dark:      { label: 'Dark',      css: 'bg-[#0d1117]' },
		light:     { label: 'Light',     css: 'bg-[#f8f9fa] text-gray-900' },
		paper:     { label: 'Paper',     css: 'bg-[#fdf6e3] text-[#657b83]' },
		parchment: { label: 'Parchment', css: 'bg-[#f5e6c8] text-[#5c4b37]' },
		'gradient-blue':   { label: 'Blue-Purple',   css: 'bg-gradient-to-br from-blue-950 via-indigo-950 to-purple-950' },
		'gradient-green':  { label: 'Green-Teal',    css: 'bg-gradient-to-br from-green-950 via-emerald-950 to-teal-950' },
		'gradient-sunset': { label: 'Sunset',        css: 'bg-gradient-to-br from-orange-950 via-red-950 to-pink-950' },
		'pattern-dots':    { label: 'Dots',          css: 'bg-[#0d1117] bg-[radial-gradient(circle,rgba(255,255,255,0.03)_1px,transparent_1px)] bg-[length:20px_20px]' },
		'pattern-lines':   { label: 'Lines',         css: 'bg-[#0d1117] bg-[repeating-linear-gradient(0deg,rgba(255,255,255,0.02),rgba(255,255,255,0.02)_1px,transparent_1px,transparent_20px)]' },
		'pattern-grid':    { label: 'Grid',          css: 'bg-[#0d1117] bg-[linear-gradient(rgba(255,255,255,0.02)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.02)_1px,transparent_1px)] bg-[length:24px_24px]' },
	};

	// Computed
	let selectedChunkCount = $derived(selectedChunks.size);
	let selectedChunkLabels = $derived(() => {
		if (!currentProject || selectedChunks.size === 0) return '';
		const labels: string[] = [];
		for (const source of currentProject.sources) {
			for (const chunk of source.chunks) {
				if (selectedChunks.has(chunk.id)) {
					labels.push(`${source.file_name} Z.${chunk.line_start}-${chunk.line_end}`);
				}
			}
		}
		return labels.join(' + ');
	});

	// File type icon mapping
	function fileTypeIcon(fileType: string) {
		switch (fileType) {
			case 'markdown': return FileText;
			case 'csv':
			case 'spreadsheet': return Table2;
			case 'code': return Code2;
			case 'json':
			case 'config': return FileJson;
			case 'email': return Mail;
			case 'pdf': return FileText;
			default: return File;
		}
	}

	// ────────────────────────────────────────────────────────────
	// Data loading
	// ────────────────────────────────────────────────────────────

	async function loadProjects() {
		try {
			projects = await invoke<CanvasProjectMeta[]>('canvas_list');
		} catch (e: any) {
			showError(e);
		}
	}

	async function loadTemplates() {
		try {
			templates = await invoke<TemplateInfo[]>('canvas_get_templates');
		} catch (e: any) {
			showError(e);
		}
	}

	async function openProject(id: string) {
		try {
			currentProject = await invoke<CanvasProject>('canvas_open', { id });
			showProjectList = false;
			selectedChunks = new Set();
			expandedSources = new Set();
			errorMessage = '';
		} catch (e: any) {
			showError(e);
		}
	}

	async function createProject() {
		if (!newProjectName.trim()) return;
		try {
			const project = await invoke<CanvasProject>('canvas_create', {
				name: newProjectName.trim(),
				outputType: newProjectType,
			});
			currentProject = project;
			showProjectList = false;
			newProjectName = '';
			await loadProjects();
		} catch (e: any) {
			showError(e);
		}
	}

	async function deleteProject(id: string) {
		try {
			await invoke('canvas_delete', { id });
			if (currentProject?.id === id) {
				currentProject = null;
				showProjectList = true;
			}
			await loadProjects();
			showSuccess('Project deleted');
		} catch (e: any) {
			showError(e);
		}
	}

	async function saveProject() {
		if (!currentProject) return;
		try {
			await invoke('canvas_save', { project: currentProject });
			showSuccess('Project saved');
		} catch (e: any) {
			showError(e);
		}
	}

	// ────────────────────────────────────────────────────────────
	// Source management
	// ────────────────────────────────────────────────────────────

	function addSource() {
		if (!currentProject) return;
		fileInputEl?.click();
	}

	async function handleFileSelected(e: Event) {
		if (!currentProject) return;
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		isAddingSource = true;
		try {
			// Read file content, write to temp dir via Tauri FS, then pass path to backend
			const buffer = await file.arrayBuffer();
			const bytes = new Uint8Array(buffer);

			const { appDataDir } = await import('@tauri-apps/api/path');
			const { writeFile, mkdir, exists } = await import('@tauri-apps/plugin-fs');
			const appData = await appDataDir();
			const importDir = `${appData}/canvas_import`;

			if (!(await exists(importDir))) {
				await mkdir(importDir, { recursive: true });
			}

			const tempPath = `${importDir}/${file.name}`;
			await writeFile(tempPath, bytes);

			const source = await invoke<SourceEntry>('canvas_add_source', {
				projectId: currentProject.id,
				filePath: tempPath,
			});
			currentProject.sources = [...currentProject.sources, source];
			expandedSources = new Set([...expandedSources, source.id]);
			showSuccess(`Added ${source.file_name} (${source.chunks.length} chunks)`);
		} catch (e: any) {
			showError(e);
		} finally {
			isAddingSource = false;
			if (input) input.value = '';
		}
	}

	async function removeSource(sourceId: string) {
		if (!currentProject) return;
		try {
			await invoke('canvas_remove_source', {
				projectId: currentProject.id,
				sourceId,
			});
			currentProject.sources = currentProject.sources.filter(s => s.id !== sourceId);
			// Remove chunks of this source from selection
			const newSelection = new Set(selectedChunks);
			for (const chunk of [...newSelection]) {
				const belongsToRemoved = currentProject.sources.every(
					s => !s.chunks.some(c => c.id === chunk)
				);
				if (belongsToRemoved) newSelection.delete(chunk);
			}
			selectedChunks = newSelection;
			showSuccess('Source removed');
		} catch (e: any) {
			showError(e);
		}
	}

	// ────────────────────────────────────────────────────────────
	// Chunk selection
	// ────────────────────────────────────────────────────────────

	function toggleChunk(chunkId: string, event?: MouseEvent) {
		const newSelection = new Set(selectedChunks);
		if (event?.ctrlKey || event?.metaKey) {
			// Toggle individual chunk
			if (newSelection.has(chunkId)) {
				newSelection.delete(chunkId);
			} else {
				newSelection.add(chunkId);
			}
		} else {
			// Single select (replace)
			if (newSelection.has(chunkId) && newSelection.size === 1) {
				newSelection.delete(chunkId);
			} else {
				newSelection.clear();
				newSelection.add(chunkId);
			}
		}
		selectedChunks = newSelection;
	}

	function selectAllChunks(sourceId: string) {
		if (!currentProject) return;
		const source = currentProject.sources.find(s => s.id === sourceId);
		if (!source) return;
		const newSelection = new Set(selectedChunks);
		for (const chunk of source.chunks) {
			newSelection.add(chunk.id);
		}
		selectedChunks = newSelection;
	}

	function clearSelection() {
		selectedChunks = new Set();
	}

	function toggleSourceExpanded(sourceId: string) {
		const newExpanded = new Set(expandedSources);
		if (newExpanded.has(sourceId)) {
			newExpanded.delete(sourceId);
		} else {
			newExpanded.add(sourceId);
		}
		expandedSources = newExpanded;
	}

	// ────────────────────────────────────────────────────────────
	// AI generation & chat
	// ────────────────────────────────────────────────────────────

	async function generateDocument() {
		if (!currentProject) return;
		isGenerating = true;
		errorMessage = '';
		try {
			const instruction = selectedTemplate
				? `Generate using the "${selectedTemplate}" template.`
				: 'Generate a professional document from the source data.';

			const output = await invoke<string>('canvas_generate', {
				projectId: currentProject.id,
				instruction,
				selectedChunks: [...selectedChunks],
			});
			currentProject.output_content = output;
			// Reload project to get updated source_links
			currentProject = await invoke<CanvasProject>('canvas_open', { id: currentProject.id });
			showSuccess('Document generated');
		} catch (e: any) {
			showError(e);
		} finally {
			isGenerating = false;
		}
	}

	async function sendChatMessage() {
		if (!currentProject || !chatInput.trim()) return;
		isChatting = true;
		const message = chatInput.trim();
		chatInput = '';
		try {
			const response = await invoke<ChatResponse>('canvas_chat', {
				projectId: currentProject.id,
				message,
				selectedChunks: [...selectedChunks],
			});

			if (response.updated_output) {
				currentProject.output_content = response.updated_output;
			}

			// Reload to get updated chat history
			currentProject = await invoke<CanvasProject>('canvas_open', { id: currentProject.id });
		} catch (e: any) {
			showError(e);
		} finally {
			isChatting = false;
		}
	}

	function handleChatKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			sendChatMessage();
		}
	}

	// ────────────────────────────────────────────────────────────
	// Export
	// ────────────────────────────────────────────────────────────

	function exportMarkdown() {
		if (!currentProject?.output_content) return;
		const blob = new Blob([currentProject.output_content], { type: 'text/markdown' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `${currentProject.name.replace(/[^a-zA-Z0-9_-]/g, '_')}.md`;
		a.click();
		URL.revokeObjectURL(url);
	}

	function exportHtml() {
		if (!currentProject?.output_content) return;
		const htmlContent = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>${currentProject.name}</title>
  <style>
    body { font-family: 'Inter', system-ui, sans-serif; max-width: 800px; margin: 2rem auto; padding: 0 1.5rem; line-height: 1.8; color: #e0e0e0; background: #0d1117; }
    h1,h2,h3 { color: #00f0ff; }
    table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
    th, td { border: 1px solid #30363d; padding: 0.5rem 1rem; text-align: left; }
    th { background: #161b22; }
    pre { background: #161b22; padding: 1rem; border-radius: 6px; overflow-x: auto; }
    code { font-family: 'JetBrains Mono', monospace; }
  </style>
</head>
<body>
${currentProject.output_content}
</body>
</html>`;
		const blob = new Blob([htmlContent], { type: 'text/html' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `${currentProject.name.replace(/[^a-zA-Z0-9_-]/g, '_')}.html`;
		a.click();
		URL.revokeObjectURL(url);
	}

	// ────────────────────────────────────────────────────────────
	// Helpers
	// ────────────────────────────────────────────────────────────

	function showError(e: any) {
		const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
		errorMessage = msg;
		successMessage = '';
		setTimeout(() => { errorMessage = ''; }, 8000);
	}

	function showSuccess(msg: string) {
		successMessage = msg;
		errorMessage = '';
		setTimeout(() => { successMessage = ''; }, 3000);
	}

	function truncate(text: string, len: number): string {
		if (text.length <= len) return text;
		return text.slice(0, len) + '...';
	}

	function formatDate(iso: string): string {
		try {
			return new Date(iso).toLocaleDateString(undefined, {
				month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
			});
		} catch {
			return iso;
		}
	}

	function getLinksForSection(sectionIdx: number): OutputSourceLink | undefined {
		return currentProject?.source_links.find(l => l.output_section_idx === sectionIdx);
	}

	function getChunkById(chunkId: string): SourceChunk | undefined {
		if (!currentProject) return undefined;
		for (const source of currentProject.sources) {
			const chunk = source.chunks.find(c => c.id === chunkId);
			if (chunk) return chunk;
		}
		return undefined;
	}

	function getSourceForChunk(chunkId: string): SourceEntry | undefined {
		if (!currentProject) return undefined;
		return currentProject.sources.find(s => s.chunks.some(c => c.id === chunkId));
	}

	// ────────────────────────────────────────────────────────────
	// Lifecycle
	// ────────────────────────────────────────────────────────────

	onMount(() => {
		loadProjects();
		loadTemplates();
	});
</script>

{#if showProjectList || !currentProject}
	<!-- ═══════════════════════════════════════════════════════════ -->
	<!-- PROJECT LIST VIEW                                          -->
	<!-- ═══════════════════════════════════════════════════════════ -->
	<div class="p-6 space-y-6">
		<!-- Header -->
		<div class="flex items-center gap-3">
			<div class="w-10 h-10 rounded-gx-lg flex items-center justify-center bg-gx-bg-elevated border border-gx-accent-purple shadow-[0_0_12px_rgba(153,51,255,0.2)]">
				<PenTool size={20} class="text-gx-accent-purple" />
			</div>
			<div>
				<h1 class="text-xl font-bold">ForgeCanvas</h1>
				<p class="text-xs text-gx-text-muted">3-Panel AI Document Workspace</p>
			</div>
		</div>

		<!-- Create new project -->
		<Card.Root class="bg-gx-bg-secondary border-gx-border-default">
			<Card.Header class="pb-2">
				<Card.Title class="text-sm font-medium flex items-center gap-2">
					<Plus size={14} class="text-gx-neon" />
					New Project
				</Card.Title>
			</Card.Header>
			<Card.Content>
				<div class="flex gap-2">
					<input
						type="text"
						bind:value={newProjectName}
						placeholder="Project name..."
						class="flex-1 h-8 px-3 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx
							focus:border-gx-neon focus:outline-none focus:ring-1 focus:ring-gx-neon/30
							text-gx-text-primary placeholder:text-gx-text-muted"
						onkeydown={(e) => { if (e.key === 'Enter') createProject(); }}
					/>
					<select
						bind:value={newProjectType}
						class="h-8 px-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx
							text-gx-text-secondary focus:border-gx-neon focus:outline-none"
					>
						<option value="custom">Custom</option>
						<option value="business_report">Business Report</option>
						<option value="quarterly_report">Quarterly Report</option>
						<option value="restaurant_menu">Restaurant Menu</option>
						<option value="action_card">Action Card</option>
						<option value="business_plan">Business Plan</option>
						<option value="summary">Summary</option>
						<option value="presentation">Presentation</option>
						<option value="letter">Cover Letter</option>
						<option value="invoice">Invoice</option>
					</select>
					<Button size="sm" class="bg-gx-neon/20 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/30" onclick={createProject}>
						<Plus size={14} class="mr-1" />
						Create
					</Button>
				</div>
			</Card.Content>
		</Card.Root>

		<!-- Project list -->
		{#if projects.length > 0}
			<div class="grid grid-cols-2 gap-3">
				{#each projects as project (project.id)}
					<Card.Root class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-accent-purple/50 transition-all cursor-pointer group"
						onclick={() => openProject(project.id)}
						role="button" tabindex={0}
						onkeydown={(e) => { if (e.key === 'Enter') openProject(project.id); }}
					>
						<Card.Content class="p-4">
							<div class="flex items-start justify-between">
								<div>
									<h3 class="font-medium text-sm text-gx-text-primary group-hover:text-gx-accent-purple transition-colors">
										{project.name}
									</h3>
									<p class="text-xs text-gx-text-muted mt-0.5">
										{project.output_type.replace('_', ' ')} - {project.source_count} sources
									</p>
								</div>
								<button
									class="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-gx-text-muted hover:text-gx-status-error"
									onclick={(e) => { e.stopPropagation(); deleteProject(project.id); }}
								>
									<Trash2 size={14} />
								</button>
							</div>
							<div class="flex items-center gap-2 mt-2">
								<Badge variant="outline" class="text-[10px] border-gx-border-default text-gx-text-muted">
									{formatDate(project.updated_at)}
								</Badge>
								{#if project.template}
									<Badge variant="outline" class="text-[10px] border-gx-accent-purple/30 text-gx-accent-purple">
										{project.template}
									</Badge>
								{/if}
							</div>
						</Card.Content>
					</Card.Root>
				{/each}
			</div>
		{:else}
			<div class="text-center py-12 text-gx-text-muted">
				<PenTool size={48} class="mx-auto mb-3 opacity-30" />
				<p class="text-sm">No canvas projects yet</p>
				<p class="text-xs mt-1">Create your first project above</p>
			</div>
		{/if}
	</div>

{:else}
	<!-- ═══════════════════════════════════════════════════════════ -->
	<!-- CANVAS WORKSPACE (3-Panel + Chat)                          -->
	<!-- ═══════════════════════════════════════════════════════════ -->
	<div class="flex flex-col h-full">
		<!-- Toolbar -->
		<div class="flex items-center gap-2 h-10 px-3 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
			<button
				class="text-xs text-gx-text-muted hover:text-gx-neon transition-colors flex items-center gap-1"
				onclick={() => { showProjectList = true; currentProject = null; loadProjects(); }}
			>
				<ArrowRight size={12} class="rotate-180" />
				Projects
			</button>
			<Separator orientation="vertical" class="h-5 bg-gx-border-default" />
			<span class="text-sm font-medium text-gx-text-primary">{currentProject.name}</span>
			<Badge variant="outline" class="text-[10px] border-gx-border-default text-gx-text-muted">
				{currentProject.output_type.replace('_', ' ')}
			</Badge>
			<div class="flex-1"></div>

			<!-- Template selector -->
			<select
				bind:value={selectedTemplate}
				class="h-6 px-2 text-[11px] bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-secondary"
			>
				<option value="">No template</option>
				{#each templates as tmpl (tmpl.id)}
					<option value={tmpl.id}>{tmpl.name}</option>
				{/each}
			</select>

			<!-- Background picker -->
			<select
				bind:value={selectedBackground}
				class="h-6 px-2 text-[11px] bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-secondary"
			>
				{#each Object.entries(backgrounds) as [key, bg]}
					<option value={key}>{bg.label}</option>
				{/each}
			</select>

			<Separator orientation="vertical" class="h-5 bg-gx-border-default" />

			<!-- Actions -->
			<button class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors" onclick={saveProject} title="Save project">
				<CheckCircle2 size={14} />
			</button>
			<button class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors" onclick={exportMarkdown} title="Export .md">
				<Download size={14} />
			</button>
			<button class="p-1 text-gx-text-muted hover:text-gx-accent-cyan transition-colors" onclick={exportHtml} title="Export .html">
				<ExternalLink size={14} />
			</button>
		</div>

		<!-- Status messages -->
		{#if errorMessage}
			<div class="px-3 py-1.5 bg-gx-status-error/10 border-b border-gx-status-error/30 text-xs text-gx-status-error flex items-center gap-2">
				<Info size={12} />
				{errorMessage}
				<button class="ml-auto" onclick={() => errorMessage = ''}><X size={12} /></button>
			</div>
		{/if}
		{#if successMessage}
			<div class="px-3 py-1.5 bg-gx-status-success/10 border-b border-gx-status-success/30 text-xs text-gx-status-success flex items-center gap-2">
				<CheckCircle2 size={12} />
				{successMessage}
			</div>
		{/if}

		<!-- Three-panel layout -->
		<div class="flex flex-1 min-h-0 overflow-hidden">

			<!-- ══════════ LEFT PANEL: SOURCES (250px) ══════════ -->
			<div class="w-[250px] shrink-0 flex flex-col border-r border-gx-border-default bg-gx-bg-secondary">
				<!-- Drop zone header -->
				<div class="p-2 border-b border-gx-border-default">
					<button
						onclick={addSource}
						disabled={isAddingSource}
						class="w-full flex items-center justify-center gap-2 h-16 rounded-gx border-2 border-dashed
							border-gx-border-default hover:border-gx-accent-purple/50 text-gx-text-muted
							hover:text-gx-accent-purple hover:bg-gx-accent-purple/5 transition-all text-xs"
					>
						{#if isAddingSource}
							<Loader2 size={16} class="animate-spin" />
							<span>Importing...</span>
						{:else}
							<FileUp size={16} />
							<span>Add Source File</span>
						{/if}
					</button>
				</div>

				<!-- Sources list -->
				<div class="flex-1 overflow-y-auto p-2 space-y-1">
					{#each currentProject.sources as source (source.id)}
						{@const Icon = fileTypeIcon(source.file_type)}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary">
							<!-- Source header -->
							<div class="flex items-center gap-1.5 px-2 py-1.5 cursor-pointer hover:bg-gx-bg-hover transition-colors"
								role="button" tabindex="0"
								onclick={() => toggleSourceExpanded(source.id)}
								onkeydown={(e) => { if (e.key === 'Enter') toggleSourceExpanded(source.id); }}
							>
								{#if expandedSources.has(source.id)}
									<ChevronDown size={12} class="text-gx-text-muted shrink-0" />
								{:else}
									<ChevronRight size={12} class="text-gx-text-muted shrink-0" />
								{/if}
								<Icon size={13} class="text-gx-accent-purple shrink-0" />
								<span class="text-[11px] font-medium text-gx-text-secondary truncate flex-1">
									{source.file_name}
								</span>
								<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted shrink-0">
									{source.chunks.length}
								</Badge>
								<button
									class="p-0.5 text-gx-text-muted hover:text-gx-status-error transition-colors shrink-0"
									onclick={(e) => { e.stopPropagation(); removeSource(source.id); }}
									title="Remove source"
								>
									<X size={11} />
								</button>
							</div>

							<!-- Chunks (expanded) -->
							{#if expandedSources.has(source.id)}
								<div class="border-t border-gx-border-default">
									<button
										class="w-full text-left px-2 py-1 text-[10px] text-gx-accent-cyan hover:bg-gx-bg-hover transition-colors"
										onclick={() => selectAllChunks(source.id)}
									>
										Select all {source.chunks.length} chunks
									</button>
									{#each source.chunks as chunk (chunk.id)}
										<button
											class="w-full text-left px-2 py-1.5 text-[10px] border-t border-gx-border-default/50
												hover:bg-gx-bg-hover transition-all cursor-pointer
												{selectedChunks.has(chunk.id) ? 'bg-gx-accent-purple/10 border-l-2 border-l-gx-accent-purple shadow-[inset_0_0_12px_rgba(153,51,255,0.08)]' : ''}"
											onclick={(e) => toggleChunk(chunk.id, e)}
										>
											<div class="flex items-center gap-1 mb-0.5">
												<Hash size={9} class="text-gx-text-muted" />
												<span class="text-gx-text-muted font-mono">
													L{chunk.line_start}-{chunk.line_end}
												</span>
												{#if chunk.used_in_output}
													<Link2 size={9} class="text-gx-neon ml-auto" />
												{/if}
											</div>
											<p class="text-gx-text-secondary leading-tight line-clamp-2">
												{truncate(chunk.text, 120)}
											</p>
										</button>
									{/each}
								</div>
							{/if}
						</div>
					{/each}

					{#if currentProject.sources.length === 0}
						<div class="text-center py-8 text-gx-text-muted">
							<FileUp size={24} class="mx-auto mb-2 opacity-30" />
							<p class="text-[11px]">No sources yet</p>
							<p class="text-[10px] mt-0.5">Click above to add files</p>
						</div>
					{/if}
				</div>

				<!-- Selection info -->
				{#if selectedChunkCount > 0}
					<div class="px-2 py-1.5 border-t border-gx-border-default bg-gx-accent-purple/5">
						<div class="flex items-center justify-between">
							<Badge class="text-[10px] bg-gx-accent-purple/20 text-gx-accent-purple border-gx-accent-purple/30">
								{selectedChunkCount} chunk{selectedChunkCount !== 1 ? 's' : ''} selected
							</Badge>
							<button class="text-[10px] text-gx-text-muted hover:text-gx-neon" onclick={clearSelection}>
								Clear
							</button>
						</div>
					</div>
				{/if}
			</div>

			<!-- ══════════ CENTER PANEL: LIVE DOCUMENT ══════════ -->
			<div class="flex-1 flex flex-col min-w-0">
				<!-- Generate button bar -->
				<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default bg-gx-bg-secondary/50">
					<Button
						size="sm"
						class="bg-gx-accent-purple/20 text-gx-accent-purple border border-gx-accent-purple/30
							hover:bg-gx-accent-purple/30 shadow-[0_0_8px_rgba(153,51,255,0.15)]"
						disabled={isGenerating || currentProject.sources.length === 0}
						onclick={generateDocument}
					>
						{#if isGenerating}
							<Loader2 size={13} class="mr-1.5 animate-spin" />
							Generating...
						{:else}
							<Sparkles size={13} class="mr-1.5" />
							Generate Document
						{/if}
					</Button>
					{#if selectedChunkCount > 0}
						<span class="text-[10px] text-gx-text-muted">
							from {selectedChunkCount} selected chunk{selectedChunkCount !== 1 ? 's' : ''}
						</span>
					{:else if currentProject.sources.length > 0}
						<span class="text-[10px] text-gx-text-muted">
							from all {currentProject.sources.reduce((n, s) => n + s.chunks.length, 0)} chunks
						</span>
					{/if}
					<div class="flex-1"></div>
					<button
						class="text-[10px] px-2 py-0.5 rounded text-gx-text-muted hover:text-gx-text-secondary
							{editMode ? 'bg-gx-accent-cyan/10 text-gx-accent-cyan border border-gx-accent-cyan/30' : 'hover:bg-gx-bg-hover'}"
						onclick={() => editMode = !editMode}
					>
						{editMode ? 'Preview' : 'Edit'}
					</button>
				</div>

				<!-- Document output area -->
				<div class="flex-1 overflow-y-auto {backgrounds[selectedBackground]?.css ?? 'bg-[#0d1117]'}">
					{#if currentProject.output_content}
						{#if editMode}
							<textarea
								bind:value={currentProject.output_content}
								class="w-full h-full p-6 bg-transparent border-none outline-none resize-none
									text-sm font-mono leading-relaxed text-gx-text-primary"
								spellcheck="false"
							></textarea>
						{:else}
							<div class="max-w-3xl mx-auto p-8">
								<article class="prose prose-invert prose-sm max-w-none
									prose-headings:text-gx-neon prose-headings:font-semibold
									prose-p:text-gx-text-secondary prose-p:leading-relaxed
									prose-strong:text-gx-text-primary
									prose-a:text-gx-accent-cyan prose-a:no-underline hover:prose-a:underline
									prose-code:text-gx-accent-magenta prose-code:bg-gx-bg-elevated prose-code:px-1 prose-code:rounded
									prose-pre:bg-gx-bg-elevated prose-pre:border prose-pre:border-gx-border-default
									prose-th:text-gx-text-primary prose-td:text-gx-text-secondary
									prose-table:border-gx-border-default
									prose-li:text-gx-text-secondary prose-li:marker:text-gx-neon">
									{#each currentProject.output_content.split('\n') as line, idx}
										{@const sectionLink = getLinksForSection(idx)}
										<div
											class="relative group {hoveredSection === idx ? 'bg-gx-accent-purple/5 rounded' : ''}"
											onmouseenter={() => hoveredSection = idx}
											onmouseleave={() => hoveredSection = null}
										>
											{#if line.startsWith('# ')}
												<h1>{line.slice(2)}</h1>
											{:else if line.startsWith('## ')}
												<h2>{line.slice(3)}</h2>
											{:else if line.startsWith('### ')}
												<h3>{line.slice(4)}</h3>
											{:else if line.startsWith('- ')}
												<li>{line.slice(2)}</li>
											{:else if line.startsWith('|')}
												<pre class="text-[11px] !p-1 !bg-transparent !border-0">{line}</pre>
											{:else if line.startsWith('<!--')}
												<!-- Hidden source link comment -->
											{:else if line.trim() === ''}
												<br />
											{:else}
												<p>{line}</p>
											{/if}
											{#if sectionLink && hoveredSection === idx}
												<div class="absolute -right-2 top-0 opacity-0 group-hover:opacity-100 transition-opacity">
													<button
														class="p-1 bg-gx-bg-elevated border border-gx-border-default rounded shadow-md"
														title="View source chunks"
													>
														<Link2 size={10} class="text-gx-accent-purple" />
													</button>
												</div>
											{/if}
										</div>
									{/each}
								</article>
							</div>
						{/if}
					{:else}
						<div class="flex items-center justify-center h-full text-gx-text-muted">
							<div class="text-center">
								<Sparkles size={48} class="mx-auto mb-3 opacity-20" />
								<p class="text-sm">No document generated yet</p>
								<p class="text-xs mt-1">Add source files and click "Generate Document"</p>
							</div>
						</div>
					{/if}
				</div>
			</div>

			<!-- ══════════ RIGHT PANEL: AI CONTEXT INSPECTOR (280px) ══════════ -->
			<div class="w-[280px] shrink-0 flex flex-col border-l border-gx-border-default bg-gx-bg-secondary">
				<div class="flex items-center gap-2 h-8 px-3 border-b border-gx-border-default shrink-0">
					<Eye size={13} class="text-gx-accent-cyan" />
					<span class="text-[11px] font-medium text-gx-text-secondary">AI Context Inspector</span>
				</div>

				<div class="flex-1 overflow-y-auto p-2 space-y-2">
					<!-- Source Links section -->
					{#if currentProject.source_links.length > 0}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-2">
							<div class="flex items-center gap-1.5 mb-2">
								<Link2 size={11} class="text-gx-accent-purple" />
								<span class="text-[11px] font-medium text-gx-text-secondary">Source Links</span>
							</div>
							<div class="space-y-1.5">
								{#each currentProject.source_links as link, i}
									<div class="rounded border border-gx-border-default/50 p-1.5 bg-gx-bg-secondary/50">
										<div class="text-[10px] text-gx-text-muted mb-1">Section {link.output_section_idx}</div>
										{#each link.chunk_ids as chunkId}
											{@const chunk = getChunkById(chunkId)}
											{@const source = getSourceForChunk(chunkId)}
											{#if chunk && source}
												<button
													class="w-full text-left flex items-center gap-1 text-[10px] py-0.5 px-1 rounded
														hover:bg-gx-bg-hover transition-colors text-gx-accent-cyan"
													onclick={() => {
														toggleChunk(chunkId);
														expandedSources = new Set([...expandedSources, source.id]);
													}}
												>
													<ArrowRight size={8} />
													<span class="truncate">{source.file_name} L{chunk.line_start}-{chunk.line_end}</span>
												</button>
											{/if}
										{/each}
										<div class="flex items-center gap-1 mt-1">
											<div class="flex-1 h-1 rounded-full bg-gx-bg-tertiary overflow-hidden">
												<div class="h-full rounded-full bg-gx-accent-purple/60" style="width: {link.confidence * 100}%"></div>
											</div>
											<span class="text-[9px] text-gx-text-muted">{(link.confidence * 100).toFixed(0)}%</span>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}

					<!-- Rechenweg / Formula Inspector -->
					{#if currentProject.output_content && currentProject.output_content.match(/\d+[.,]\d+/)}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-2">
							<div class="flex items-center gap-1.5 mb-2">
								<Hash size={11} class="text-gx-accent-orange" />
								<span class="text-[11px] font-medium text-gx-text-secondary">Formula Inspector</span>
							</div>
							<p class="text-[10px] text-gx-text-muted leading-relaxed">
								Hover over numbers in the document to see their source data and calculation breakdown.
							</p>
							{#if hoveredSection !== null}
								{@const link = getLinksForSection(hoveredSection)}
								{#if link}
									<div class="mt-2 p-1.5 rounded border border-gx-accent-orange/20 bg-gx-accent-orange/5">
										<div class="text-[10px] text-gx-accent-orange font-medium mb-1">Rechenweg</div>
										<div class="text-[10px] text-gx-text-secondary font-mono leading-relaxed">
											{#each link.chunk_ids as cid}
												{@const ch = getChunkById(cid)}
												{#if ch}
													<div class="truncate">Source L{ch.line_start}: {truncate(ch.text, 60)}</div>
												{/if}
											{/each}
										</div>
									</div>
								{/if}
							{/if}
						</div>
					{/if}

					<!-- Template info -->
					{#if selectedTemplate}
						{@const tmpl = templates.find(t => t.id === selectedTemplate)}
						{#if tmpl}
							<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-2">
								<div class="flex items-center gap-1.5 mb-2">
									<Palette size={11} class="text-gx-accent-magenta" />
									<span class="text-[11px] font-medium text-gx-text-secondary">Template</span>
								</div>
								<p class="text-[10px] text-gx-text-muted mb-1.5">{tmpl.description}</p>
								<div class="space-y-0.5">
									{#each tmpl.sections as section, i}
										<div class="flex items-center gap-1.5 text-[10px] text-gx-text-secondary">
											<span class="w-4 h-4 flex items-center justify-center rounded bg-gx-bg-tertiary text-[9px] font-mono text-gx-text-muted">
												{i + 1}
											</span>
											{section}
										</div>
									{/each}
								</div>
							</div>
						{/if}
					{/if}

					<!-- Sources overview -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-2">
						<div class="flex items-center gap-1.5 mb-2">
							<Info size={11} class="text-gx-text-muted" />
							<span class="text-[11px] font-medium text-gx-text-secondary">Project Info</span>
						</div>
						<div class="space-y-1 text-[10px]">
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Sources</span>
								<span class="text-gx-text-secondary">{currentProject.sources.length}</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Total Chunks</span>
								<span class="text-gx-text-secondary">{currentProject.sources.reduce((n, s) => n + s.chunks.length, 0)}</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Selected</span>
								<span class="text-gx-accent-purple">{selectedChunkCount}</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Output</span>
								<span class="text-gx-text-secondary">{currentProject.output_content ? `${currentProject.output_content.split('\n').length} lines` : 'Empty'}</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Links</span>
								<span class="text-gx-text-secondary">{currentProject.source_links.length}</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Chat</span>
								<span class="text-gx-text-secondary">{currentProject.chat_history.length} messages</span>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>

		<!-- ══════════ BOTTOM: CONTEXT CHAT (150px) ══════════ -->
		<div class="h-[150px] shrink-0 border-t border-gx-border-default bg-gx-bg-secondary flex flex-col">
			<!-- Context indicator -->
			<div class="flex items-center gap-2 px-3 py-1 border-b border-gx-border-default/50 shrink-0">
				<span class="text-[10px] text-gx-text-muted">Context:</span>
				{#if selectedChunkCount > 0}
					<span class="text-[10px] text-gx-accent-purple font-mono truncate">
						{selectedChunkLabels()}
					</span>
				{:else}
					<span class="text-[10px] text-gx-text-muted italic">No chunks selected (chat uses full document)</span>
				{/if}
			</div>

			<!-- Chat messages (scrollable) -->
			<div class="flex-1 overflow-y-auto px-3 py-1 space-y-1">
				{#each currentProject.chat_history.slice(-6) as msg (msg.id)}
					<div class="flex gap-2 text-[11px] {msg.role === 'user' ? 'justify-end' : ''}">
						<div class="max-w-[80%] px-2 py-1 rounded-gx {msg.role === 'user'
							? 'bg-gx-accent-purple/15 text-gx-text-primary border border-gx-accent-purple/20'
							: 'bg-gx-bg-primary text-gx-text-secondary border border-gx-border-default'}">
							{msg.content}
						</div>
					</div>
				{/each}
				{#if isChatting}
					<div class="flex gap-2 text-[11px]">
						<div class="px-2 py-1 rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-muted">
							<Loader2 size={12} class="animate-spin inline-block mr-1" />
							Thinking...
						</div>
					</div>
				{/if}
			</div>

			<!-- Chat input -->
			<div class="flex items-center gap-2 px-3 py-1.5 border-t border-gx-border-default/50 shrink-0">
				<input
					type="text"
					bind:value={chatInput}
					placeholder={selectedChunkCount > 0
						? `Ask about ${selectedChunkCount} selected chunk${selectedChunkCount !== 1 ? 's' : ''}...`
						: 'Ask about the document...'}
					class="flex-1 h-7 px-3 text-xs bg-gx-bg-primary border border-gx-border-default rounded-gx
						focus:border-gx-accent-purple focus:outline-none focus:ring-1 focus:ring-gx-accent-purple/30
						text-gx-text-primary placeholder:text-gx-text-muted"
					onkeydown={handleChatKeydown}
					disabled={isChatting}
				/>
				<Button
					size="sm"
					class="h-7 px-2 bg-gx-accent-purple/20 text-gx-accent-purple border border-gx-accent-purple/30 hover:bg-gx-accent-purple/30"
					disabled={isChatting || !chatInput.trim()}
					onclick={sendChatMessage}
				>
					<Send size={12} />
				</Button>
			</div>
		</div>
	</div>
{/if}

<!-- Hidden file input for source import -->
<input
	type="file"
	accept=".txt,.md,.csv,.json,.html,.xml,.yaml,.yml,.toml,.rs,.py,.js,.ts,.go,.java,.c,.cpp,.h,.eml,.pdf,.docx,.xlsx"
	class="hidden"
	bind:this={fileInputEl}
	onchange={handleFileSelected}
/>
