<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Presentation, Plus, Save, Download, Trash2, Play, ChevronLeft, ChevronRight,
		Sparkles, Wand2, Palette, StickyNote, LayoutGrid, Loader2, AlertCircle,
		GripVertical, Copy, MoreVertical, Maximize2, Minimize2, X, Search,
		FileText, Type, Columns2, Image, Quote, Square, RefreshCw, ChevronDown
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-slides';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface SlideTheme {
		name: string;
		primary_color: string;
		secondary_color: string;
		bg_color: string;
		font_family: string;
		heading_font: string;
	}

	interface Slide {
		id: string;
		title: string;
		content: string;
		layout: string;
		notes: string | null;
		background: string | null;
	}

	interface PresentationData {
		id: string;
		title: string;
		slides: Slide[];
		theme: SlideTheme;
		created_at: string;
		updated_at: string;
	}

	interface PresentationMeta {
		id: string;
		title: string;
		slide_count: number;
		theme_name: string;
		updated_at: string;
	}

	// ---- State ---------------------------------------------------------------
	let presentations = $state<PresentationMeta[]>([]);
	let activePres = $state<PresentationData | null>(null);
	let activeSlideIdx = $state(0);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state<string | null>(null);
	let searchQuery = $state('');

	// Editor state
	let editingTitle = $state('');
	let editingContent = $state('');
	let editingNotes = $state('');
	let hasUnsavedChanges = $state(false);

	// AI state
	let aiLoading = $state(false);
	let aiTopic = $state('');
	let aiNumSlides = $state(8);
	let aiStyle = $state('professional and clear');
	let aiImproveInstruction = $state('');
	let aiError = $state<string | null>(null);

	// Theme state
	let themes = $state<SlideTheme[]>([]);
	let showThemePicker = $state(false);

	// Layout picker
	let showLayoutPicker = $state(false);

	// Present mode
	let presentMode = $state(false);
	let presentSlideIdx = $state(0);

	// Context menu
	let contextMenuSlideIdx = $state<number | null>(null);
	let contextMenuPos = $state({ x: 0, y: 0 });

	// Drag state
	let dragIdx = $state<number | null>(null);
	let dragOverIdx = $state<number | null>(null);

	// ---- Derived -------------------------------------------------------------
	let activeSlide = $derived(activePres?.slides[activeSlideIdx] ?? null);
	let filteredPresentations = $derived(
		presentations.filter(p =>
			p.title.toLowerCase().includes(searchQuery.toLowerCase())
		)
	);

	const layoutOptions = [
		{ value: 'title_slide', label: 'Title Slide', icon: Type },
		{ value: 'content_slide', label: 'Content', icon: FileText },
		{ value: 'two_column', label: 'Two Column', icon: Columns2 },
		{ value: 'image_and_text', label: 'Image & Text', icon: Image },
		{ value: 'quote_slide', label: 'Quote', icon: Quote },
		{ value: 'blank_slide', label: 'Blank', icon: Square },
	];

	// ---- Effects -------------------------------------------------------------
	$effect(() => {
		if (activeSlide) {
			editingTitle = activeSlide.title;
			editingContent = activeSlide.content;
			editingNotes = activeSlide.notes ?? '';
		}
	});

	// ---- Data Loading --------------------------------------------------------
	async function loadPresentations() {
		try {
			loading = true;
			error = null;
			presentations = await invoke<PresentationMeta[]>('slides_list');
		} catch (e: any) {
			error = parseError(e);
		} finally {
			loading = false;
		}
	}

	async function loadThemes() {
		try {
			themes = await invoke<SlideTheme[]>('slides_get_themes');
		} catch { /* themes are non-critical */ }
	}

	async function openPresentation(id: string) {
		try {
			loading = true;
			error = null;
			activePres = await invoke<PresentationData>('slides_open', { id });
			activeSlideIdx = 0;
		} catch (e: any) {
			error = parseError(e);
		} finally {
			loading = false;
		}
	}

	// ---- CRUD Operations -----------------------------------------------------
	async function createPresentation() {
		try {
			const pres = await invoke<PresentationData>('slides_create', {
				title: 'Untitled Presentation',
				themeName: null,
			});
			activePres = pres;
			activeSlideIdx = 0;
			await loadPresentations();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function savePresentation() {
		if (!activePres) return;
		try {
			saving = true;
			// Apply current edits to the active slide before saving
			applyEdits();
			await invoke('slides_save', { id: activePres.id, data: activePres });
			hasUnsavedChanges = false;
			await loadPresentations();
		} catch (e: any) {
			error = parseError(e);
		} finally {
			saving = false;
		}
	}

	async function deletePresentation(id: string) {
		try {
			await invoke('slides_delete', { id });
			if (activePres?.id === id) {
				activePres = null;
				activeSlideIdx = 0;
			}
			await loadPresentations();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	// ---- Slide Operations ----------------------------------------------------
	async function addSlide(layout: string, afterIndex?: number) {
		if (!activePres) return;
		try {
			const slide = await invoke<Slide>('slides_add_slide', {
				id: activePres.id,
				layout,
				afterIndex: afterIndex ?? (activePres.slides.length - 1),
			});
			activePres = await invoke<PresentationData>('slides_open', { id: activePres.id });
			activeSlideIdx = afterIndex != null ? afterIndex + 1 : activePres.slides.length - 1;
			showLayoutPicker = false;
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function removeSlide(index: number) {
		if (!activePres) return;
		try {
			await invoke('slides_remove_slide', { id: activePres.id, slideIndex: index });
			activePres = await invoke<PresentationData>('slides_open', { id: activePres.id });
			if (activeSlideIdx >= activePres.slides.length) {
				activeSlideIdx = Math.max(0, activePres.slides.length - 1);
			}
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function duplicateSlide(index: number) {
		if (!activePres) return;
		const slide = activePres.slides[index];
		if (!slide) return;
		try {
			await invoke<Slide>('slides_add_slide', {
				id: activePres.id,
				layout: slide.layout,
				afterIndex: index,
			});
			// Reload and update the duplicated slide content
			activePres = await invoke<PresentationData>('slides_open', { id: activePres.id });
			// Copy content to the new slide
			const newSlide = activePres.slides[index + 1];
			if (newSlide) {
				newSlide.title = slide.title;
				newSlide.content = slide.content;
				newSlide.notes = slide.notes;
				newSlide.background = slide.background;
				await invoke('slides_save', { id: activePres.id, data: activePres });
			}
			activeSlideIdx = index + 1;
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function reorderSlide(from: number, to: number) {
		if (!activePres || from === to) return;
		try {
			await invoke('slides_reorder', { id: activePres.id, fromIndex: from, toIndex: to });
			activePres = await invoke<PresentationData>('slides_open', { id: activePres.id });
			activeSlideIdx = to;
		} catch (e: any) {
			error = parseError(e);
		}
	}

	// ---- AI Operations -------------------------------------------------------
	async function aiGenerate() {
		if (!aiTopic.trim()) return;
		try {
			aiLoading = true;
			aiError = null;
			const pres = await invoke<PresentationData>('slides_ai_generate', {
				topic: aiTopic,
				numSlides: aiNumSlides,
				style: aiStyle,
				themeName: null,
			});
			activePres = pres;
			activeSlideIdx = 0;
			aiTopic = '';
			await loadPresentations();
		} catch (e: any) {
			aiError = parseError(e);
		} finally {
			aiLoading = false;
		}
	}

	async function aiImproveSlide() {
		if (!activePres || !aiImproveInstruction.trim()) return;
		try {
			aiLoading = true;
			aiError = null;
			const improved = await invoke<Slide>('slides_ai_improve_slide', {
				id: activePres.id,
				slideIndex: activeSlideIdx,
				instruction: aiImproveInstruction,
			});
			activePres.slides[activeSlideIdx] = improved;
			editingTitle = improved.title;
			editingContent = improved.content;
			editingNotes = improved.notes ?? '';
			aiImproveInstruction = '';
		} catch (e: any) {
			aiError = parseError(e);
		} finally {
			aiLoading = false;
		}
	}

	// ---- Theme Operations ----------------------------------------------------
	async function applyTheme(theme: SlideTheme) {
		if (!activePres) return;
		activePres.theme = theme;
		hasUnsavedChanges = true;
		showThemePicker = false;
		await savePresentation();
	}

	// ---- Export ---------------------------------------------------------------
	async function exportHtml() {
		if (!activePres) return;
		try {
			applyEdits();
			await invoke('slides_save', { id: activePres.id, data: activePres });
			const path = await invoke<string>('slides_export_html', { id: activePres.id });
			error = null;
			alert(`Exported to: ${path}`);
		} catch (e: any) {
			error = parseError(e);
		}
	}

	// ---- Editor Helpers ------------------------------------------------------
	function applyEdits() {
		if (!activePres || !activeSlide) return;
		activePres.slides[activeSlideIdx] = {
			...activeSlide,
			title: editingTitle,
			content: editingContent,
			notes: editingNotes || null,
		};
	}

	function onEditorInput() {
		hasUnsavedChanges = true;
		applyEdits();
	}

	function selectSlide(index: number) {
		if (activePres && hasUnsavedChanges) {
			applyEdits();
		}
		activeSlideIdx = index;
	}

	// ---- Present Mode --------------------------------------------------------
	function enterPresentMode() {
		if (!activePres) return;
		applyEdits();
		presentSlideIdx = activeSlideIdx;
		presentMode = true;
	}

	function exitPresentMode() {
		presentMode = false;
	}

	function presentNext() {
		if (!activePres) return;
		if (presentSlideIdx < activePres.slides.length - 1) {
			presentSlideIdx++;
		}
	}

	function presentPrev() {
		if (presentSlideIdx > 0) {
			presentSlideIdx--;
		}
	}

	function handlePresentKeydown(e: KeyboardEvent) {
		if (!presentMode) return;
		if (e.key === 'ArrowRight' || e.key === ' ') {
			e.preventDefault();
			presentNext();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			presentPrev();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			exitPresentMode();
		}
	}

	// ---- Context Menu --------------------------------------------------------
	function showContextMenu(e: MouseEvent, index: number) {
		e.preventDefault();
		contextMenuSlideIdx = index;
		contextMenuPos = { x: e.clientX, y: e.clientY };
	}

	function closeContextMenu() {
		contextMenuSlideIdx = null;
	}

	// ---- Drag & Drop ---------------------------------------------------------
	function onDragStart(index: number) {
		dragIdx = index;
	}

	function onDragOver(e: DragEvent, index: number) {
		e.preventDefault();
		dragOverIdx = index;
	}

	function onDragEnd() {
		if (dragIdx !== null && dragOverIdx !== null && dragIdx !== dragOverIdx) {
			reorderSlide(dragIdx, dragOverIdx);
		}
		dragIdx = null;
		dragOverIdx = null;
	}

	// ---- Keyboard Shortcuts --------------------------------------------------
	function handleKeydown(e: KeyboardEvent) {
		if (presentMode) {
			handlePresentKeydown(e);
			return;
		}
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 's') {
			e.preventDefault();
			savePresentation();
		}
		if (mod && e.key === 'n') {
			e.preventDefault();
			if (activePres) {
				showLayoutPicker = true;
			} else {
				createPresentation();
			}
		}
		if (mod && e.key === 'p') {
			e.preventDefault();
			enterPresentMode();
		}
	}

	// ---- Simple Markdown Preview ---------------------------------------------
	function renderMarkdown(md: string): string {
		let html = md
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');

		// Bold
		html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
		// Italic
		html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
		// Inline code
		html = html.replace(/`(.+?)`/g, '<code class="bg-gx-bg-elevated px-1 rounded text-xs">$1</code>');
		// Bullet lists
		html = html.replace(/^- (.+)$/gm, '<li>$1</li>');
		html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul class="list-disc pl-4 space-y-1">$&</ul>');
		// Blockquotes
		html = html.replace(/^&gt; (.+)$/gm, '<blockquote class="border-l-2 border-current pl-3 italic opacity-80">$1</blockquote>');
		// Paragraphs for remaining lines
		html = html.replace(/^(?!<[uolbh])(.*\S.*)$/gm, '<p>$1</p>');

		return html;
	}

	// ---- Error Parsing -------------------------------------------------------
	function parseError(e: any): string {
		if (typeof e === 'string') {
			try {
				const parsed = JSON.parse(e);
				return parsed.message ?? e;
			} catch { return e; }
		}
		return e?.message ?? String(e);
	}

	// ---- Layout label helper -------------------------------------------------
	function layoutLabel(layout: string): string {
		const opt = layoutOptions.find(l => l.value === layout);
		return opt?.label ?? layout;
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadPresentations();
		loadThemes();
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- ===== PRESENT MODE (Full-screen overlay) ===== -->
{#if presentMode && activePres}
	<div
		class="fixed inset-0 z-[9999] flex items-center justify-center"
		style="background: {activePres.theme.bg_color};"
		role="dialog"
		aria-label="Presentation mode"
	>
		{#each activePres.slides as slide, i}
			{#if i === presentSlideIdx}
				<div
					class="w-full h-full flex items-center justify-center p-16"
					style="background: {slide.background ?? activePres.theme.bg_color};"
				>
					<div class="max-w-[80%] w-full" style="font-family: '{activePres.theme.font_family}', sans-serif;">
						{#if slide.title}
							<h1
								class="mb-6 font-bold leading-tight"
								style="font-family: '{activePres.theme.heading_font}', sans-serif; color: {activePres.theme.primary_color}; font-size: {slide.layout === 'title_slide' ? '4rem' : '3rem'};"
							>
								{slide.title}
							</h1>
						{/if}
						<div
							class="text-2xl leading-relaxed prose-invert"
							style="color: {isLight(activePres.theme.bg_color) ? '#1e293b' : '#e2e8f0'};"
						>
							{@html renderMarkdown(slide.content)}
						</div>
					</div>
				</div>
			{/if}
		{/each}

		<!-- Present mode controls -->
		<div class="fixed bottom-6 right-6 flex items-center gap-2">
			<button onclick={presentPrev} class="w-10 h-10 rounded-lg bg-black/30 text-white border border-white/20 hover:bg-white/20 flex items-center justify-center">
				<ChevronLeft size={20} />
			</button>
			<button onclick={presentNext} class="w-10 h-10 rounded-lg bg-black/30 text-white border border-white/20 hover:bg-white/20 flex items-center justify-center">
				<ChevronRight size={20} />
			</button>
			<button onclick={exitPresentMode} class="w-10 h-10 rounded-lg bg-black/30 text-white border border-white/20 hover:bg-red-500/50 flex items-center justify-center">
				<X size={20} />
			</button>
		</div>
		<div class="fixed bottom-6 left-6 text-sm opacity-60" style="color: {isLight(activePres.theme.bg_color) ? '#1e293b' : '#e2e8f0'};">
			{presentSlideIdx + 1} / {activePres.slides.length}
		</div>
	</div>
{/if}

<!-- ===== MAIN EDITOR ===== -->
<div class="flex flex-col h-full overflow-hidden" style={containerStyle}>
	<!-- Top toolbar -->
	<header class="flex items-center gap-2 h-11 px-3 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
		<Presentation size={18} class="text-gx-neon" />
		<h1 class="text-sm font-semibold text-gx-text-primary">ForgeSlides</h1>

		{#if activePres}
			<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />
			<input
				type="text"
				bind:value={activePres.title}
				oninput={() => hasUnsavedChanges = true}
				class="bg-transparent border-none text-sm text-gx-text-secondary font-medium focus:outline-none focus:text-gx-text-primary w-48"
				placeholder="Presentation title..."
			/>
		{/if}

		<div class="flex-1"></div>

		{#if activePres}
			{#if hasUnsavedChanges}
				<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-warning/50 text-gx-status-warning">Unsaved</Badge>
			{/if}

			<button
				onclick={savePresentation}
				disabled={saving}
				class="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-gx bg-gx-bg-tertiary text-gx-text-secondary hover:text-gx-neon hover:border-gx-neon border border-gx-border-default transition-all"
			>
				{#if saving}
					<Loader2 size={12} class="animate-spin" />
				{:else}
					<Save size={12} />
				{/if}
				Save
			</button>

			<button
				onclick={enterPresentMode}
				class="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 border border-gx-neon/30 transition-all"
			>
				<Play size={12} />
				Present
			</button>

			<button
				onclick={exportHtml}
				class="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-gx bg-gx-bg-tertiary text-gx-text-secondary hover:text-gx-text-primary border border-gx-border-default transition-all"
			>
				<Download size={12} />
				Export
			</button>
		{/if}

		<button
			onclick={createPresentation}
			class="flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 border border-gx-neon/30 transition-all"
		>
			<Plus size={12} />
			New
		</button>
	</header>

	<!-- Error banner -->
	{#if error}
		<div class="flex items-center gap-2 px-3 py-1.5 bg-gx-status-error/10 border-b border-gx-status-error/30 text-xs text-gx-status-error">
			<AlertCircle size={14} />
			<span class="flex-1">{error}</span>
			<button onclick={() => error = null} class="hover:text-gx-text-primary"><X size={12} /></button>
		</div>
	{/if}

	<div class="flex flex-1 overflow-hidden">
		{#if !activePres}
			<!-- ===== PRESENTATION LIST (no active presentation) ===== -->
			<div class="flex-1 overflow-auto p-6">
				<div class="max-w-4xl mx-auto">
					<!-- AI Quick Generate -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5 mb-6">
						<div class="flex items-center gap-2 mb-3">
							<Sparkles size={18} class="text-gx-accent-magenta" />
							<h2 class="text-sm font-semibold text-gx-text-primary">AI Generate Presentation</h2>
						</div>
						<div class="flex gap-2">
							<input
								type="text"
								bind:value={aiTopic}
								placeholder="e.g., 10-slide pitch deck for a SaaS startup..."
								class="flex-1 px-3 py-2 text-sm bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
								onkeydown={(e) => e.key === 'Enter' && aiGenerate()}
							/>
							<select
								bind:value={aiNumSlides}
								class="px-2 py-2 text-xs bg-gx-bg-primary border border-gx-border-default rounded-gx text-gx-text-secondary focus:outline-none"
							>
								{#each [5, 8, 10, 12, 15, 20] as n}
									<option value={n}>{n} slides</option>
								{/each}
							</select>
							<button
								onclick={aiGenerate}
								disabled={aiLoading || !aiTopic.trim()}
								class="flex items-center gap-1.5 px-4 py-2 text-xs rounded-gx bg-gx-accent-magenta/20 text-gx-accent-magenta hover:bg-gx-accent-magenta/30 border border-gx-accent-magenta/30 transition-all disabled:opacity-40"
							>
								{#if aiLoading}
									<Loader2 size={14} class="animate-spin" />
									Generating...
								{:else}
									<Sparkles size={14} />
									Generate
								{/if}
							</button>
						</div>
						{#if aiError}
							<p class="text-xs text-gx-status-error mt-2">{aiError}</p>
						{/if}
					</div>

					<!-- Search -->
					<div class="relative mb-4">
						<Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-gx-text-muted" />
						<input
							type="text"
							bind:value={searchQuery}
							placeholder="Search presentations..."
							class="w-full pl-9 pr-3 py-2 text-sm bg-gx-bg-secondary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon"
						/>
					</div>

					<!-- Presentation list -->
					{#if loading}
						<div class="flex items-center justify-center gap-2 py-12 text-gx-text-muted">
							<Loader2 size={18} class="animate-spin" />
							<span class="text-sm">Loading presentations...</span>
						</div>
					{:else if filteredPresentations.length === 0}
						<div class="text-center py-12">
							<Presentation size={48} class="mx-auto text-gx-text-muted/30 mb-3" />
							<p class="text-sm text-gx-text-muted">No presentations yet</p>
							<p class="text-xs text-gx-text-muted/60 mt-1">Create one or let AI generate a deck</p>
						</div>
					{:else}
						<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
							{#each filteredPresentations as pres (pres.id)}
								<div
									role="button"
									tabindex="0"
									onclick={() => openPresentation(pres.id)}
									onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openPresentation(pres.id); } }}
									class="group text-left rounded-gx border border-gx-border-default bg-gx-bg-secondary hover:border-gx-neon/50 hover:bg-gx-bg-elevated transition-all p-4 cursor-pointer"
								>
									<div class="flex items-start justify-between mb-2">
										<h3 class="text-sm font-medium text-gx-text-primary group-hover:text-gx-neon line-clamp-2">{pres.title}</h3>
										<button
											onclick={(e) => { e.stopPropagation(); deletePresentation(pres.id); }}
											class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all p-1 -m-1"
										>
											<Trash2 size={12} />
										</button>
									</div>
									<div class="flex items-center gap-2 text-[11px] text-gx-text-muted">
										<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default">{pres.slide_count} slides</Badge>
										<span>{pres.theme_name}</span>
									</div>
									<p class="text-[10px] text-gx-text-muted/60 mt-2">
										{new Date(pres.updated_at).toLocaleDateString()}
									</p>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{:else}
			<!-- ===== SLIDE EDITOR (active presentation) ===== -->

			<!-- Left Panel: Slide Navigator -->
			<div class="w-[200px] shrink-0 flex flex-col border-r border-gx-border-default bg-gx-bg-secondary overflow-hidden">
				<div class="flex items-center justify-between px-2 py-1.5 border-b border-gx-border-default">
					<button
						onclick={() => { activePres = null; activeSlideIdx = 0; }}
						class="text-[11px] text-gx-text-muted hover:text-gx-neon flex items-center gap-1 transition-colors"
					>
						<ChevronLeft size={12} />
						Back
					</button>
					<button
						onclick={() => showLayoutPicker = !showLayoutPicker}
						class="text-gx-text-muted hover:text-gx-neon transition-colors p-1"
						title="Add slide"
					>
						<Plus size={14} />
					</button>
				</div>

				<!-- Layout picker dropdown -->
				{#if showLayoutPicker}
					<div class="border-b border-gx-border-default bg-gx-bg-elevated p-2 space-y-0.5">
						{#each layoutOptions as opt}
							<button
								onclick={() => addSlide(opt.value)}
								class="w-full flex items-center gap-2 px-2 py-1.5 text-[11px] text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover rounded transition-colors"
							>
								<opt.icon size={12} />
								{opt.label}
							</button>
						{/each}
					</div>
				{/if}

				<!-- Slide thumbnails -->
				<div class="flex-1 overflow-y-auto p-1.5 space-y-1.5">
					{#each activePres.slides as slide, i (slide.id)}
						<button
							onclick={() => selectSlide(i)}
							oncontextmenu={(e) => showContextMenu(e, i)}
							draggable="true"
							ondragstart={() => onDragStart(i)}
							ondragover={(e) => onDragOver(e, i)}
							ondragend={onDragEnd}
							class="w-full rounded border transition-all text-left p-1.5 group relative
								{i === activeSlideIdx
									? 'border-gx-neon bg-gx-neon/10'
									: 'border-gx-border-default bg-gx-bg-primary hover:border-gx-border-hover'}
								{dragOverIdx === i ? 'border-t-2 border-t-gx-accent-magenta' : ''}"
						>
							<!-- Slide number -->
							<div class="flex items-center gap-1 mb-1">
								<GripVertical size={10} class="text-gx-text-muted/40 opacity-0 group-hover:opacity-100 cursor-grab" />
								<span class="text-[9px] text-gx-text-muted font-mono">{i + 1}</span>
								<Badge variant="outline" class="text-[8px] px-0.5 py-0 h-3 border-gx-border-default text-gx-text-muted ml-auto">
									{layoutLabel(slide.layout)}
								</Badge>
							</div>
							<!-- Mini preview -->
							<div class="aspect-video bg-gx-bg-tertiary rounded-sm p-1.5 overflow-hidden">
								{#if slide.title}
									<p class="text-[8px] font-semibold text-gx-text-secondary truncate leading-tight">{slide.title}</p>
								{/if}
								<p class="text-[7px] text-gx-text-muted line-clamp-3 mt-0.5 leading-tight">
									{slide.content.replace(/[#*>\-`\[\]()]/g, '').slice(0, 80)}
								</p>
							</div>
						</button>
					{/each}
				</div>
			</div>

			<!-- Center: Slide Editor -->
			<div class="flex-1 flex flex-col overflow-hidden bg-gx-bg-primary">
				{#if activeSlide}
					<!-- Slide preview (16:9 aspect ratio) -->
					<div class="flex-1 flex items-center justify-center p-4 overflow-auto bg-gx-bg-tertiary/50">
						<div
							class="w-full max-w-[800px] aspect-video rounded-lg shadow-gx-glow-sm border border-gx-border-default overflow-hidden flex items-center justify-center p-8"
							style="background: {activeSlide.background ?? activePres.theme.bg_color};"
						>
							<div class="w-full max-w-[85%]" style="font-family: '{activePres.theme.font_family}', sans-serif;">
								{#if editingTitle}
									<h2
										class="font-bold leading-tight mb-3"
										style="font-family: '{activePres.theme.heading_font}', sans-serif; color: {activePres.theme.primary_color}; font-size: {activeSlide.layout === 'title_slide' ? '2rem' : '1.5rem'}; text-align: {activeSlide.layout === 'title_slide' ? 'center' : 'left'};"
									>
										{editingTitle}
									</h2>
								{/if}
								<div
									class="text-sm leading-relaxed prose-sm"
									style="color: {isLight(activePres.theme.bg_color) ? '#1e293b' : '#e2e8f0'}; text-align: {activeSlide.layout === 'title_slide' || activeSlide.layout === 'quote_slide' ? 'center' : 'left'};"
								>
									{@html renderMarkdown(editingContent)}
								</div>
							</div>
						</div>
					</div>

					<!-- Editor below the preview -->
					<div class="h-[240px] shrink-0 border-t border-gx-border-default bg-gx-bg-secondary flex flex-col overflow-hidden">
						<!-- Editor toolbar -->
						<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-default text-[11px]">
							<span class="text-gx-text-muted">Slide {activeSlideIdx + 1}/{activePres.slides.length}</span>
							<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted">
								{layoutLabel(activeSlide.layout)}
							</Badge>
							<div class="flex-1"></div>
							<span class="text-gx-text-muted/60">{editingContent.length} chars</span>
						</div>

						<div class="flex flex-1 overflow-hidden">
							<!-- Title + Content editor -->
							<div class="flex-1 flex flex-col overflow-hidden">
								<input
									type="text"
									bind:value={editingTitle}
									oninput={onEditorInput}
									placeholder="Slide title..."
									class="px-3 py-1.5 bg-transparent border-b border-gx-border-default/50 text-sm font-semibold text-gx-text-primary placeholder:text-gx-text-muted/40 focus:outline-none"
								/>
								<textarea
									bind:value={editingContent}
									oninput={onEditorInput}
									placeholder="Markdown content... (supports **bold**, *italic*, - lists, > quotes, `code`)"
									class="flex-1 px-3 py-2 bg-transparent text-sm text-gx-text-secondary placeholder:text-gx-text-muted/40 focus:outline-none resize-none font-mono leading-relaxed"
								></textarea>
							</div>

							<!-- Background color picker -->
							<div class="w-10 shrink-0 border-l border-gx-border-default flex flex-col items-center gap-1 py-2">
								<label class="text-[8px] text-gx-text-muted/60 mb-0.5" title="Background color">BG</label>
								<input
									type="color"
									value={activeSlide.background ?? activePres.theme.bg_color}
									oninput={(e) => {
										if (activePres && activeSlide) {
											activePres.slides[activeSlideIdx] = {
												...activeSlide,
												background: (e.target as HTMLInputElement).value,
											};
											hasUnsavedChanges = true;
										}
									}}
									class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
									title="Slide background color"
								/>
								<button
									onclick={() => {
										if (activePres && activeSlide) {
											activePres.slides[activeSlideIdx] = { ...activeSlide, background: null };
											hasUnsavedChanges = true;
										}
									}}
									class="text-[8px] text-gx-text-muted hover:text-gx-neon"
									title="Reset to theme default"
								>
									<RefreshCw size={10} />
								</button>
							</div>
						</div>
					</div>
				{/if}
			</div>

			<!-- Right Panel: AI & Properties -->
			<div class="w-[250px] shrink-0 flex flex-col border-l border-gx-border-default bg-gx-bg-secondary overflow-hidden">
				<!-- AI Generator -->
				<div class="p-3 border-b border-gx-border-default">
					<div class="flex items-center gap-1.5 mb-2">
						<Sparkles size={13} class="text-gx-accent-magenta" />
						<span class="text-[11px] font-semibold text-gx-text-secondary">AI Generator</span>
					</div>
					<input
						type="text"
						bind:value={aiTopic}
						placeholder="Topic for new presentation..."
						class="w-full px-2 py-1.5 text-xs bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon mb-1.5"
						onkeydown={(e) => e.key === 'Enter' && aiGenerate()}
					/>
					<div class="flex gap-1.5 mb-1.5">
						<select
							bind:value={aiNumSlides}
							class="flex-1 px-1 py-1 text-[10px] bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-secondary"
						>
							{#each [5, 8, 10, 12, 15] as n}
								<option value={n}>{n} slides</option>
							{/each}
						</select>
						<select
							bind:value={aiStyle}
							class="flex-1 px-1 py-1 text-[10px] bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-secondary"
						>
							<option value="professional and clear">Professional</option>
							<option value="casual and engaging">Casual</option>
							<option value="technical and detailed">Technical</option>
							<option value="creative and inspiring">Creative</option>
							<option value="minimal and concise">Minimal</option>
						</select>
					</div>
					<button
						onclick={aiGenerate}
						disabled={aiLoading || !aiTopic.trim()}
						class="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 text-[11px] rounded bg-gx-accent-magenta/20 text-gx-accent-magenta hover:bg-gx-accent-magenta/30 border border-gx-accent-magenta/30 disabled:opacity-40 transition-all"
					>
						{#if aiLoading}
							<Loader2 size={12} class="animate-spin" />
							Generating...
						{:else}
							<Sparkles size={12} />
							Generate Deck
						{/if}
					</button>
				</div>

				<!-- AI Improve Slide -->
				{#if activeSlide}
					<div class="p-3 border-b border-gx-border-default">
						<div class="flex items-center gap-1.5 mb-2">
							<Wand2 size={13} class="text-gx-accent-blue" />
							<span class="text-[11px] font-semibold text-gx-text-secondary">AI Improve Slide</span>
						</div>
						<input
							type="text"
							bind:value={aiImproveInstruction}
							placeholder="Make more concise, add examples..."
							class="w-full px-2 py-1.5 text-xs bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon mb-1.5"
							onkeydown={(e) => e.key === 'Enter' && aiImproveSlide()}
						/>
						<div class="flex flex-wrap gap-1 mb-1.5">
							{#each ['Make concise', 'Add examples', 'Add data points', 'Translate to English', 'Simplify'] as quick}
								<button
									onclick={() => { aiImproveInstruction = quick; aiImproveSlide(); }}
									class="px-1.5 py-0.5 text-[9px] rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-muted hover:text-gx-accent-blue hover:border-gx-accent-blue/30 transition-colors"
								>
									{quick}
								</button>
							{/each}
						</div>
						<button
							onclick={aiImproveSlide}
							disabled={aiLoading || !aiImproveInstruction.trim()}
							class="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 text-[11px] rounded bg-gx-accent-blue/20 text-gx-accent-blue hover:bg-gx-accent-blue/30 border border-gx-accent-blue/30 disabled:opacity-40 transition-all"
						>
							{#if aiLoading}
								<Loader2 size={12} class="animate-spin" />
							{:else}
								<Wand2 size={12} />
							{/if}
							Improve
						</button>
						{#if aiError}
							<p class="text-[10px] text-gx-status-error mt-1.5">{aiError}</p>
						{/if}
					</div>
				{/if}

				<!-- Theme Picker -->
				<div class="p-3 border-b border-gx-border-default">
					<button
						onclick={() => showThemePicker = !showThemePicker}
						class="w-full flex items-center gap-1.5 text-[11px] font-semibold text-gx-text-secondary hover:text-gx-neon transition-colors"
					>
						<Palette size={13} />
						<span>Theme: {activePres?.theme.name ?? 'Default'}</span>
						<ChevronDown size={12} class="ml-auto transition-transform {showThemePicker ? 'rotate-180' : ''}" />
					</button>

					{#if showThemePicker}
						<div class="mt-2 space-y-1">
							{#each themes as theme}
								<button
									onclick={() => applyTheme(theme)}
									class="w-full flex items-center gap-2 px-2 py-1.5 rounded text-[11px] hover:bg-gx-bg-hover transition-colors
										{activePres?.theme.name === theme.name ? 'bg-gx-bg-elevated text-gx-neon' : 'text-gx-text-secondary'}"
								>
									<div class="flex gap-0.5">
										<span class="w-3 h-3 rounded-sm border border-gx-border-default" style="background: {theme.bg_color}"></span>
										<span class="w-3 h-3 rounded-sm border border-gx-border-default" style="background: {theme.primary_color}"></span>
										<span class="w-3 h-3 rounded-sm border border-gx-border-default" style="background: {theme.secondary_color}"></span>
									</div>
									{theme.name}
								</button>
							{/each}
						</div>
					{/if}
				</div>

				<!-- Speaker Notes -->
				{#if activeSlide}
					<div class="p-3 border-b border-gx-border-default flex-1 flex flex-col overflow-hidden">
						<div class="flex items-center gap-1.5 mb-2">
							<StickyNote size={13} class="text-gx-accent-amber" />
							<span class="text-[11px] font-semibold text-gx-text-secondary">Speaker Notes</span>
						</div>
						<textarea
							bind:value={editingNotes}
							oninput={onEditorInput}
							placeholder="Add speaker notes for this slide..."
							class="flex-1 w-full px-2 py-1.5 text-xs bg-gx-bg-primary border border-gx-border-default rounded text-gx-text-secondary placeholder:text-gx-text-muted/40 focus:outline-none focus:border-gx-neon resize-none"
						></textarea>
					</div>

					<!-- Slide Properties -->
					<div class="p-3">
						<div class="flex items-center gap-1.5 mb-2">
							<LayoutGrid size={13} class="text-gx-text-muted" />
							<span class="text-[11px] font-semibold text-gx-text-secondary">Slide Layout</span>
						</div>
						<div class="grid grid-cols-2 gap-1">
							{#each layoutOptions as opt}
								<button
									onclick={() => {
										if (activePres && activeSlide) {
											activePres.slides[activeSlideIdx] = { ...activeSlide, layout: opt.value };
											hasUnsavedChanges = true;
										}
									}}
									class="flex items-center gap-1 px-1.5 py-1 text-[10px] rounded border transition-colors
										{activeSlide.layout === opt.value
											? 'border-gx-neon bg-gx-neon/10 text-gx-neon'
											: 'border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary hover:border-gx-border-hover'}"
								>
									<opt.icon size={10} />
									{opt.label}
								</button>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<!-- Bottom status bar -->
	{#if activePres}
		<footer class="flex items-center h-6 px-3 bg-gx-bg-secondary border-t border-gx-border-default text-[10px] text-gx-text-muted shrink-0 gap-3">
			<span>Slide {activeSlideIdx + 1} of {activePres.slides.length}</span>
			<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
			<span>{activePres.theme.name}</span>
			<div class="flex-1"></div>
			<span class="opacity-60">Ctrl+S save | Ctrl+P present | Ctrl+N new slide</span>
		</footer>
	{/if}
</div>

<!-- Context menu -->
{#if contextMenuSlideIdx !== null}
	<div
		class="fixed z-50 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-sm py-1 min-w-[140px]"
		style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px;"
	>
		<button
			onclick={() => { addSlide('content_slide', contextMenuSlideIdx ?? undefined); closeContextMenu(); }}
			class="w-full text-left px-3 py-1.5 text-xs text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
		>
			<Plus size={12} class="inline mr-1.5" />
			Add after
		</button>
		<button
			onclick={() => { if (contextMenuSlideIdx !== null) duplicateSlide(contextMenuSlideIdx); closeContextMenu(); }}
			class="w-full text-left px-3 py-1.5 text-xs text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
		>
			<Copy size={12} class="inline mr-1.5" />
			Duplicate
		</button>
		<div class="h-px bg-gx-border-default my-1"></div>
		<button
			onclick={() => { if (contextMenuSlideIdx !== null) removeSlide(contextMenuSlideIdx); closeContextMenu(); }}
			class="w-full text-left px-3 py-1.5 text-xs text-gx-status-error hover:bg-gx-status-error/10 transition-colors"
		>
			<Trash2 size={12} class="inline mr-1.5" />
			Delete
		</button>
	</div>
	<!-- Backdrop to close context menu -->
	<button class="fixed inset-0 z-40" onclick={closeContextMenu} aria-label="Close menu"></button>
{/if}

<script lang="ts" module>
	/** Check if a hex color is light (for text contrast). */
	function isLight(hex: string): boolean {
		const h = hex.replace('#', '');
		if (h.length < 6) return false;
		const r = parseInt(h.slice(0, 2), 16);
		const g = parseInt(h.slice(2, 4), 16);
		const b = parseInt(h.slice(4, 6), 16);
		const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
		return lum >= 128;
	}
</script>
