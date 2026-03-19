<script lang="ts">
	import { browserAgentStore, type BrowserAgentConfig } from '$lib/stores/browser-agent.svelte';
	import { scraperStore } from '$lib/stores/scraper.svelte';
	import { cdpStore } from '$lib/stores/cdp.svelte';
	import { browserImportStore } from '$lib/stores/browser-import.svelte';
	import { playgroundStore, type NetworkEntry } from '$lib/stores/browser-playground.svelte';
	import { forgeBrowserStore } from '$lib/stores/forge-browser.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import {
		Globe, Play, Loader2, Search, Send, Webhook,
		Trash2, Code2, ChevronRight,
		Zap, Workflow, Bot, Eye, Camera, Terminal,
		X, Plus, Monitor, Import, BookMarked,
		History, Shield, MousePointer, FileText, ArrowDown,
		Activity, Cookie, Gauge, Crosshair, RefreshCw,
		Briefcase, User, BookOpen, Pin, PinOff, Sparkles,
		Languages, Scissors, BookText, LayoutGrid, PanelLeft,
		ChevronDown, Star
	} from '@lucide/svelte';
	import { onMount, onDestroy } from 'svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-browser';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let toolbarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'toolbar'));
	let toolbarStyle = $derived(hasEngineStyle && toolbarComponent ? componentToCSS(toolbarComponent) : '');
	let contentComponent = $derived(styleEngine.getComponentStyle(widgetId, 'content'));
	let contentStyle = $derived(hasEngineStyle && contentComponent ? componentToCSS(contentComponent) : '');

	// ── State ──────────────────────────────────────────────
	let urlInput = $state('');
	let selectorInput = $state('');
	let taskInput = $state('');
	let webhookUrl = $state('');
	let jsInput = $state('');
	let cdpSelector = $state('');
	let cdpFillValue = $state('');
	let activeTab = $state<'browser' | 'playground' | 'agent' | 'import' | 'webhook'>('browser');

	// Agent config
	let agentModel = $state('hermes3:latest');
	let maxSteps = $state(10);

	// ── ForgeBrowser State ─────────────────────────────────
	let showSpaceSidebar = $state(true);
	let showAiPanel = $state(false);
	let showPresetPicker = $state(false);
	let newSpaceName = $state('');
	let newSpaceColor = $state('#00ff66');
	let aiTargetUrl = $state('');
	let aiTranslateLanguage = $state('German');
	let aiClipFormat = $state('markdown');

	// Space icon mapping
	const spaceIconMap: Record<string, typeof Briefcase> = {
		Briefcase, User, BookOpen, Star, Globe
	};

	function getSpaceIcon(iconName: string) {
		return spaceIconMap[iconName] ?? Briefcase;
	}

	async function handleCreateSpace() {
		if (!newSpaceName.trim()) return;
		await forgeBrowserStore.createSpace(newSpaceName.trim(), newSpaceColor, 'Star');
		newSpaceName = '';
	}

	async function handleAiSummarize() {
		const url = aiTargetUrl.trim() || forgeBrowserStore.activeTab?.url || urlInput.trim();
		if (!url) return;
		await forgeBrowserStore.aiSummarize(url);
	}

	async function handleAiTranslate() {
		const url = aiTargetUrl.trim() || forgeBrowserStore.activeTab?.url || urlInput.trim();
		if (!url) return;
		await forgeBrowserStore.aiTranslate(url, aiTranslateLanguage);
	}

	async function handleAiWebClip() {
		const url = aiTargetUrl.trim() || forgeBrowserStore.activeTab?.url || urlInput.trim();
		if (!url) return;
		await forgeBrowserStore.aiWebClip(url, aiClipFormat);
	}

	async function handleAiReaderMode() {
		const url = aiTargetUrl.trim() || forgeBrowserStore.activeTab?.url || urlInput.trim();
		if (!url) return;
		await forgeBrowserStore.aiReaderMode(url);
	}

	// ── Init ───────────────────────────────────────────────
	onMount(() => {
		cdpStore.detectBrowsers();
		browserImportStore.detectProfiles();
		forgeBrowserStore.loadSpaces();
		forgeBrowserStore.loadPresets();
	});

	onDestroy(() => {
		playgroundStore.stopPolling();
	});

	// ── CDP Actions ───────────────────────────────────────
	async function handleCdpNavigate() {
		if (!urlInput.trim()) return;
		const url = urlInput.trim().startsWith('http') ? urlInput.trim() : `https://${urlInput.trim()}`;
		if (cdpStore.pages.length === 0) {
			await cdpStore.openPage(url);
		} else {
			await cdpStore.navigate(url);
		}
	}

	async function handleCdpClick() {
		if (!cdpSelector.trim()) return;
		await cdpStore.click(cdpSelector.trim());
	}

	async function handleCdpFill() {
		if (!cdpSelector.trim() || !cdpFillValue.trim()) return;
		await cdpStore.fill(cdpSelector.trim(), cdpFillValue.trim());
	}

	async function handleCdpScreenshot() {
		await cdpStore.screenshot(true);
	}

	async function handleCdpJs() {
		if (!jsInput.trim()) return;
		await cdpStore.executeJs(jsInput.trim());
	}

	async function handleCdpExtract() {
		if (!cdpSelector.trim()) return;
		const text = await cdpStore.extract(cdpSelector.trim());
		if (text) displayOverride = text;
	}

	// ── Playground Actions ─────────────────────────────────
	async function handleQuickExtract() {
		if (!urlInput.trim()) return;
		const sel = selectorInput.trim() || undefined;
		await browserAgentStore.quickExtract(urlInput.trim(), sel);
	}

	async function handleScrape() {
		if (!urlInput.trim()) return;
		await scraperStore.scrape(urlInput.trim(), {
			selectors: selectorInput.trim() ? [selectorInput.trim()] : undefined
		});
	}

	// ── Agent Actions ──────────────────────────────────────
	async function handleRunAgent() {
		if (!taskInput.trim()) return;
		const config: BrowserAgentConfig = {
			model: agentModel,
			max_steps: maxSteps,
			webhook: webhookUrl ? { url: webhookUrl, method: 'POST', headers: {} } : undefined
		};
		await browserAgentStore.runTask(taskInput.trim(), config);
	}

	// ── Import Actions ─────────────────────────────────────
	async function handleImportAll(profile: typeof browserImportStore.profiles[0]) {
		await browserImportStore.importAll(profile);
	}

	// ── Webhook Actions ────────────────────────────────────
	async function handleSendWebhook() {
		if (!webhookUrl.trim()) return;
		const data = browserAgentStore.currentTask
			? browserAgentStore.currentTask
			: { content: browserAgentStore.playgroundContent, url: browserAgentStore.playgroundUrl };
		await browserAgentStore.sendWebhook(webhookUrl.trim(), data);
	}

	// ── Element Picker ────────────────────────────────────
	async function handleScanElements() {
		await cdpStore.getElements();
	}

	async function handleSelectElement(selector: string) {
		cdpSelector = selector;
		await cdpStore.highlightElement(selector);
	}

	// ── Playground Actions ────────────────────────────────
	async function handleStartDevtools() {
		if (!cdpStore.activePage) return;
		await playgroundStore.enableNetwork(cdpStore.activePage);
		await playgroundStore.enableConsole(cdpStore.activePage);
		playgroundStore.startPolling(cdpStore.activePage);
	}

	async function handleRefreshPerf() {
		if (!cdpStore.activePage) return;
		await playgroundStore.fetchPerf(cdpStore.activePage);
	}

	async function handleRefreshCookies() {
		if (!cdpStore.activePage) return;
		await playgroundStore.fetchCookies(cdpStore.activePage);
	}

	function getStatusColor(status: number | null): string {
		if (!status) return 'text-gx-text-muted';
		if (status < 300) return 'text-gx-status-success';
		if (status < 400) return 'text-gx-accent-blue';
		if (status < 500) return 'text-gx-status-warning';
		return 'text-gx-status-error';
	}

	function formatBytes(bytes: number | null): string {
		if (!bytes) return '—';
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / 1048576).toFixed(1)} MB`;
	}

	function getConsoleColor(level: string): string {
		switch (level) {
			case 'error': return 'text-gx-status-error';
			case 'warn': return 'text-gx-status-warning';
			case 'info': return 'text-gx-status-info';
			case 'debug': return 'text-gx-text-muted';
			default: return 'text-gx-text-primary';
		}
	}

	function getPerfColor(ms: number | null): string {
		if (!ms) return 'text-gx-text-muted';
		if (ms < 1000) return 'text-gx-status-success';
		if (ms < 3000) return 'text-gx-status-warning';
		return 'text-gx-status-error';
	}

	// ── Helpers ────────────────────────────────────────────
	let displayOverride = $state('');

	function copyContent() {
		const content = displayOverride || browserAgentStore.playgroundContent || browserAgentStore.currentTask?.final_result || '';
		navigator.clipboard.writeText(content);
	}

	let isLoading = $derived(browserAgentStore.isRunning || scraperStore.isLoading || cdpStore.isLoading);
	let displayContent = $derived(
		displayOverride ||
		browserAgentStore.playgroundContent ||
		browserAgentStore.currentTask?.final_result ||
		scraperStore.results[0]?.markdown ||
		''
	);

	// Active page info
	let activePageInfo = $derived(
		cdpStore.pages.find(p => p.page_id === cdpStore.activePage)
	);

	// Browser icon mapping
	function getBrowserIcon(type: string): string {
		const icons: Record<string, string> = {
			chrome: 'Chrome', brave: 'Brave', firefox: 'Firefox',
			edge: 'Edge', opera: 'Opera', vivaldi: 'Vivaldi',
			chromium: 'Chromium'
		};
		return icons[type] || type;
	}
</script>

<div class="flex flex-col h-full overflow-hidden {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Header — Opera GX Style -->
	<div class="flex items-center gap-3 px-5 py-3 border-b border-gx-border-default shrink-0 {hasEngineStyle && toolbarComponent ? '' : 'bg-gradient-to-r from-gx-bg-secondary to-gx-bg-primary'}" style={toolbarStyle}>
		<div class="relative">
			<Globe size={22} class="text-gx-neon" />
			<div class="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-gx-neon animate-pulse"></div>
		</div>
		<div>
			<h1 class="text-base font-semibold text-gx-text-primary tracking-tight">ImpForge Browser</h1>
			<p class="text-[10px] text-gx-text-muted">CDP Engine + AI Agent + Automation Pipeline</p>
		</div>
		<div class="flex-1"></div>
		<!-- Layout Preset Picker -->
		<div class="relative">
			<button
				onclick={() => showPresetPicker = !showPresetPicker}
				class="flex items-center gap-1.5 px-2.5 py-1.5 text-[10px] rounded-gx border transition-all
					{showPresetPicker
						? 'bg-gx-accent-cyan/15 text-gx-accent-cyan border-gx-accent-cyan/30'
						: 'bg-gx-bg-tertiary text-gx-text-muted border-gx-border-default hover:text-gx-accent-cyan hover:border-gx-accent-cyan/30'}"
			>
				<LayoutGrid size={12} />
				{forgeBrowserStore.activePreset?.name ?? 'Layout'}
				<ChevronDown size={10} />
			</button>
			{#if showPresetPicker}
				<div class="absolute right-0 top-full mt-1 w-64 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-xl z-50 overflow-hidden">
					<div class="px-3 py-2 border-b border-gx-border-default">
						<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Layout Presets</span>
					</div>
					<div class="max-h-72 overflow-y-auto">
						{#each forgeBrowserStore.presets as preset}
							<button
								onclick={() => { forgeBrowserStore.applyPreset(preset.id); showPresetPicker = false; }}
								class="w-full flex items-start gap-2.5 px-3 py-2 text-left transition-all hover:bg-gx-bg-hover
									{forgeBrowserStore.activePresetId === preset.id ? 'bg-gx-accent-cyan/10 border-l-2 border-gx-accent-cyan' : ''}"
							>
								<LayoutGrid size={14} class="shrink-0 mt-0.5 {forgeBrowserStore.activePresetId === preset.id ? 'text-gx-accent-cyan' : 'text-gx-text-muted'}" />
								<div class="min-w-0">
									<p class="text-xs font-medium {forgeBrowserStore.activePresetId === preset.id ? 'text-gx-accent-cyan' : 'text-gx-text-primary'}">{preset.name}</p>
									<p class="text-[9px] text-gx-text-muted mt-0.5">{preset.description}</p>
									<div class="flex gap-1 mt-1 flex-wrap">
										{#each preset.layout.panels as panel}
											<span class="px-1 py-0 text-[8px] rounded bg-gx-bg-tertiary text-gx-text-muted border border-gx-border-default">{panel.module}</span>
										{/each}
									</div>
								</div>
							</button>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Space Sidebar Toggle -->
		<button
			onclick={() => showSpaceSidebar = !showSpaceSidebar}
			class="flex items-center gap-1 px-2 py-1.5 text-[10px] rounded-gx border transition-all
				{showSpaceSidebar
					? 'bg-gx-neon/15 text-gx-neon border-gx-neon/30'
					: 'bg-gx-bg-tertiary text-gx-text-muted border-gx-border-default hover:text-gx-neon'}"
			title="Toggle Space Sidebar"
		>
			<PanelLeft size={12} />
		</button>

		<!-- AI Tools Toggle -->
		<button
			onclick={() => showAiPanel = !showAiPanel}
			class="flex items-center gap-1 px-2 py-1.5 text-[10px] rounded-gx border transition-all
				{showAiPanel
					? 'bg-gx-accent-purple/15 text-gx-accent-purple border-gx-accent-purple/30'
					: 'bg-gx-bg-tertiary text-gx-text-muted border-gx-border-default hover:text-gx-accent-purple'}"
			title="Toggle AI Tools"
		>
			<Sparkles size={12} />
		</button>

		{#if cdpStore.installedBrowsers.length > 0}
			<Badge variant="outline" class="text-[9px] border-gx-neon/20 text-gx-neon px-1.5 py-0.5">
				CDP: {cdpStore.installedBrowsers[0].name}
			</Badge>
		{:else}
			<Badge variant="outline" class="text-[9px] border-gx-status-warning/30 text-gx-status-warning px-1.5 py-0.5">
				HTTP-Only Mode
			</Badge>
		{/if}
		<Badge variant="outline" class="text-[9px] border-gx-border-default text-gx-text-muted px-1.5 py-0.5">
			{forgeBrowserStore.totalTabCount} tabs
		</Badge>
	</div>

	<!-- Tab Bar — Neon underline style -->
	<div class="flex items-center gap-0.5 px-5 border-b border-gx-border-default shrink-0 bg-gx-bg-secondary/50">
		{#each [
			{ id: 'browser', icon: Monitor, label: 'Browser' },
			{ id: 'playground', icon: Eye, label: 'Playground' },
			{ id: 'agent', icon: Bot, label: 'AI Agent' },
			{ id: 'import', icon: Import, label: 'Import' },
			{ id: 'webhook', icon: Webhook, label: 'Webhooks' },
		] as tab}
			<button
				onclick={() => activeTab = tab.id as typeof activeTab}
				class="relative flex items-center gap-1.5 px-3 py-2.5 text-xs transition-all
					{activeTab === tab.id
						? 'text-gx-neon'
						: 'text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				<tab.icon size={13} />
				{tab.label}
				{#if activeTab === tab.id}
					<div class="absolute bottom-0 left-1 right-1 h-[2px] bg-gx-neon rounded-t-full shadow-[0_0_8px_rgba(0,255,102,0.4)]"></div>
				{/if}
			</button>
		{/each}

		<!-- CDP Page Tabs -->
		{#if activeTab === 'browser' && cdpStore.pages.length > 0}
			<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-1" />
			{#each cdpStore.pages as page}
				<div
					role="tab"
					tabindex="0"
					onclick={() => cdpStore.activePage = page.page_id}
					onkeydown={(e) => e.key === 'Enter' && (cdpStore.activePage = page.page_id)}
					class="flex items-center gap-1 px-2 py-1.5 text-[10px] rounded-t max-w-[140px] cursor-pointer transition-all
						{cdpStore.activePage === page.page_id
							? 'bg-gx-bg-elevated text-gx-text-primary border-t border-x border-gx-neon/30'
							: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
				>
					<Globe size={10} class="shrink-0" />
					<span class="truncate">{page.title || page.url || 'New Tab'}</span>
					<button
						onclick={(e: MouseEvent) => { e.stopPropagation(); cdpStore.closePage(page.page_id); }}
						class="shrink-0 hover:text-gx-status-error transition-colors"
					>
						<X size={10} />
					</button>
				</div>
			{/each}
			<button
				onclick={() => cdpStore.openPage('about:blank')}
				class="flex items-center p-1.5 text-gx-text-muted hover:text-gx-neon transition-colors"
			>
				<Plus size={12} />
			</button>
		{/if}
	</div>

	<!-- Content Area -->
	<div class="flex-1 overflow-hidden flex">
		<!-- Space Sidebar (Arc-style, 48px) -->
		{#if showSpaceSidebar}
			<div class="w-12 shrink-0 border-r border-gx-border-default flex flex-col items-center py-2 gap-1.5 bg-gx-bg-secondary/50">
				{#each forgeBrowserStore.spaces as space}
					{@const SpaceIcon = getSpaceIcon(space.icon)}
					<button
						onclick={() => forgeBrowserStore.switchSpace(space.id)}
						class="relative w-9 h-9 flex items-center justify-center rounded-lg transition-all group
							{forgeBrowserStore.activeSpaceId === space.id
								? 'bg-opacity-25 shadow-sm'
								: 'hover:bg-gx-bg-hover'}"
						style={forgeBrowserStore.activeSpaceId === space.id ? `background-color: ${space.color}20; box-shadow: inset 0 0 0 2px ${space.color}40` : ''}
						title="{space.name} ({space.tabs.length} tabs)"
					>
						<SpaceIcon size={16} style="color: {space.color}" />
						{#if space.tabs.length > 0}
							<span
								class="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 flex items-center justify-center rounded-full text-[7px] font-bold text-white"
								style="background-color: {space.color}"
							>
								{space.tabs.length}
							</span>
						{/if}
					</button>
				{/each}

				<Separator class="bg-gx-border-default w-6 my-1" />

				<!-- Add Space -->
				<div class="relative group">
					<button
						onclick={() => {
							const name = prompt('Space name:');
							if (name) forgeBrowserStore.createSpace(name, '#' + Math.floor(Math.random()*16777215).toString(16).padStart(6, '0'), 'Star');
						}}
						class="w-9 h-9 flex items-center justify-center rounded-lg text-gx-text-muted hover:text-gx-neon hover:bg-gx-neon/10 transition-all border border-dashed border-gx-border-default hover:border-gx-neon/30"
						title="Add Space"
					>
						<Plus size={14} />
					</button>
				</div>
			</div>

			<!-- Space Tabs Column -->
			{#if forgeBrowserStore.activeSpace && forgeBrowserStore.activeTabs.length > 0}
				<div class="w-44 shrink-0 border-r border-gx-border-default flex flex-col bg-gx-bg-secondary/20 overflow-hidden">
					<div class="px-2.5 py-2 border-b border-gx-border-default">
						<span class="text-[10px] font-semibold uppercase tracking-wider" style="color: {forgeBrowserStore.activeSpace.color}">
							{forgeBrowserStore.activeSpace.name}
						</span>
					</div>
					<div class="flex-1 overflow-y-auto">
						<!-- Pinned Tabs -->
						{#if forgeBrowserStore.pinnedTabs.length > 0}
							<div class="px-1.5 py-1">
								<span class="text-[8px] text-gx-text-muted font-semibold uppercase tracking-wider px-1">Pinned</span>
								{#each forgeBrowserStore.pinnedTabs as tab}
									<button
										onclick={() => forgeBrowserStore.activateTab(tab.id)}
										class="w-full flex items-center gap-1.5 px-2 py-1.5 text-left text-[10px] rounded-gx transition-all
											{tab.is_active ? 'bg-gx-bg-elevated text-gx-text-primary' : 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
									>
										<Pin size={9} class="shrink-0 text-gx-accent-orange" />
										<span class="truncate flex-1">{tab.title || 'Untitled'}</span>
									</button>
								{/each}
							</div>
						{/if}
						<!-- Unpinned Tabs -->
						{#each forgeBrowserStore.unpinnedTabs as tab}
							<div class="group flex items-center px-1.5">
								<button
									onclick={() => forgeBrowserStore.activateTab(tab.id)}
									class="flex-1 flex items-center gap-1.5 px-2 py-1.5 text-left text-[10px] rounded-gx transition-all min-w-0
										{tab.is_active ? 'bg-gx-bg-elevated text-gx-text-primary' : 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
								>
									<Globe size={9} class="shrink-0" />
									<span class="truncate">{tab.title || 'Untitled'}</span>
								</button>
								<div class="hidden group-hover:flex items-center gap-px shrink-0">
									<button
										onclick={(e) => { e.stopPropagation(); forgeBrowserStore.pinTab(tab.id, true); }}
										class="p-0.5 text-gx-text-muted hover:text-gx-accent-orange transition-colors"
										title="Pin tab"
									>
										<Pin size={8} />
									</button>
									<button
										onclick={(e) => { e.stopPropagation(); forgeBrowserStore.closeTab(tab.id); }}
										class="p-0.5 text-gx-text-muted hover:text-gx-status-error transition-colors"
										title="Close tab"
									>
										<X size={8} />
									</button>
								</div>
							</div>
						{/each}
					</div>
					<!-- New Tab in Space -->
					<div class="px-2 py-1.5 border-t border-gx-border-default">
						<button
							onclick={() => {
								const url = prompt('URL:');
								if (url) forgeBrowserStore.openTab(url, forgeBrowserStore.activeSpaceId);
							}}
							class="w-full flex items-center justify-center gap-1 py-1 text-[10px] text-gx-text-muted hover:text-gx-neon rounded-gx hover:bg-gx-neon/10 transition-all"
						>
							<Plus size={10} /> New Tab
						</button>
					</div>
				</div>
			{/if}
		{/if}

		<!-- Left Panel -->
		<div class="w-[380px] shrink-0 border-r border-gx-border-default flex flex-col overflow-y-auto bg-gx-bg-secondary/30">

			{#if activeTab === 'browser'}
				<!-- ═══ BROWSER TAB ═══ CDP Controls -->
				<div class="p-4 space-y-3">
					<!-- URL Bar -->
					<div class="flex gap-1.5">
						<div class="flex-1 flex items-center gap-2 px-3 py-2 bg-gx-bg-tertiary border border-gx-border-default rounded-gx focus-within:border-gx-neon transition-colors">
							<Shield size={12} class="text-gx-text-muted shrink-0" />
							<input
								type="text"
								bind:value={urlInput}
								placeholder="Enter URL or search..."
								onkeydown={(e) => e.key === 'Enter' && handleCdpNavigate()}
								class="flex-1 text-sm bg-transparent text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none"
							/>
						</div>
						<button
							onclick={handleCdpNavigate}
							disabled={isLoading || !urlInput.trim()}
							class="px-3 py-2 text-xs font-medium rounded-gx transition-all
								{isLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-neon/20 text-gx-neon hover:bg-gx-neon/30 border border-gx-neon/30'}"
						>
							{#if isLoading}
								<Loader2 size={14} class="animate-spin" />
							{:else}
								Go
							{/if}
						</button>
					</div>

					<!-- CDP Actions -->
					<div class="space-y-2">
						<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">CDP Actions</span>

						<!-- Selector + Action -->
						<div class="space-y-1.5">
							<input
								type="text"
								bind:value={cdpSelector}
								placeholder="CSS Selector: button.submit, #login, .nav-link"
								class="w-full px-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon/50 focus:outline-none"
							/>
							<div class="flex gap-1">
								<button onclick={handleCdpClick} disabled={!cdpSelector.trim()} class="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-accent-cyan hover:border-gx-accent-cyan/30 disabled:opacity-30 transition-all">
									<MousePointer size={10} /> Click
								</button>
								<button onclick={handleCdpExtract} disabled={!cdpSelector.trim()} class="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/30 disabled:opacity-30 transition-all">
									<FileText size={10} /> Extract
								</button>
								<button onclick={handleCdpScreenshot} class="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-accent-purple hover:border-gx-accent-purple/30 transition-all">
									<Camera size={10} /> Screenshot
								</button>
							</div>
						</div>

						<!-- Fill Field -->
						<div class="flex gap-1.5">
							<input
								type="text"
								bind:value={cdpFillValue}
								placeholder="Value to fill..."
								class="flex-1 px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon/50 focus:outline-none"
							/>
							<button onclick={handleCdpFill} disabled={!cdpSelector.trim() || !cdpFillValue.trim()} class="flex items-center gap-1 px-3 py-1.5 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-accent-orange hover:border-gx-accent-orange/30 disabled:opacity-30 transition-all">
								Fill
							</button>
						</div>

						<!-- JS Console -->
						<div class="space-y-1">
							<div class="flex items-center gap-1.5">
								<Terminal size={10} class="text-gx-text-muted" />
								<span class="text-[10px] text-gx-text-muted font-semibold">JavaScript Console</span>
							</div>
							<div class="flex gap-1.5">
								<input
									type="text"
									bind:value={jsInput}
									placeholder="document.title, window.scrollTo(0, 9999), ..."
									onkeydown={(e) => e.key === 'Enter' && handleCdpJs()}
									class="flex-1 px-2 py-1.5 text-xs font-mono bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-accent-cyan placeholder:text-gx-text-muted focus:border-gx-accent-cyan/50 focus:outline-none"
								/>
								<button onclick={handleCdpJs} disabled={!jsInput.trim()} class="px-3 py-1.5 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-accent-cyan hover:border-gx-accent-cyan/30 disabled:opacity-30 transition-all">
									Run
								</button>
							</div>
							{#if cdpStore.jsResult !== null}
								<div class="px-2 py-1.5 text-[10px] font-mono bg-gx-bg-primary rounded-gx border border-gx-border-default text-gx-accent-cyan max-h-20 overflow-auto">
									{JSON.stringify(cdpStore.jsResult, null, 2)}
								</div>
							{/if}
						</div>
					</div>

					<Separator class="bg-gx-border-default" />

					<!-- Scroll Controls -->
					<div class="flex gap-1">
						<button onclick={() => cdpStore.scroll('up')} class="flex-1 px-2 py-1 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-all">Scroll Up</button>
						<button onclick={() => cdpStore.scroll('down')} class="flex-1 px-2 py-1 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-all">Scroll Down</button>
						<button onclick={() => cdpStore.scroll('top')} class="flex-1 px-2 py-1 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-all">Top</button>
						<button onclick={() => cdpStore.scroll('bottom')} class="flex-1 px-2 py-1 text-[10px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary transition-all">Bottom</button>
					</div>

					<!-- Element Picker -->
					<div class="space-y-1.5">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-1.5">
								<Crosshair size={11} class="text-gx-accent-purple" />
								<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Element Picker</span>
							</div>
							<button
								onclick={handleScanElements}
								disabled={!cdpStore.activePage}
								class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-accent-purple hover:bg-gx-accent-purple/10 border border-gx-accent-purple/30 transition-all disabled:opacity-30"
							>
								<Search size={10} /> Scan Page
							</button>
						</div>
						{#if cdpStore.elements.length > 0}
							<div class="max-h-48 overflow-y-auto space-y-px rounded-gx border border-gx-border-default bg-gx-bg-primary">
								{#each cdpStore.elements as el}
									<button
										onclick={() => handleSelectElement(el.selector)}
										onmouseenter={() => cdpStore.highlightElement(el.selector)}
										class="w-full flex items-start gap-2 px-2 py-1.5 text-left text-[10px] hover:bg-gx-accent-purple/10 transition-colors
											{cdpSelector === el.selector ? 'bg-gx-accent-purple/15 border-l-2 border-gx-accent-purple' : ''}"
									>
										<span class="shrink-0 font-mono font-bold text-gx-accent-cyan">&lt;{el.tag}&gt;</span>
										<div class="min-w-0 flex-1">
											{#if el.id}
												<span class="text-gx-accent-orange">#{el.id}</span>
											{/if}
											{#if el.classes.length > 0}
												<span class="text-gx-accent-purple">.{el.classes.slice(0, 3).join('.')}</span>
											{/if}
											{#if el.text_preview}
												<p class="text-gx-text-muted truncate mt-0.5">"{el.text_preview}"</p>
											{/if}
										</div>
										<span class="shrink-0 text-[9px] text-gx-text-muted font-mono truncate max-w-[100px]" title={el.selector}>
											{el.selector}
										</span>
									</button>
								{/each}
							</div>
							<p class="text-[9px] text-gx-text-muted">{cdpStore.elements.length} interactive elements — click to select, hover to highlight</p>
						{:else if cdpStore.activePage}
							<p class="text-[9px] text-gx-text-muted">Click "Scan Page" to discover interactive elements</p>
						{/if}
					</div>

					<Separator class="bg-gx-border-default" />

					<!-- Detected Browsers -->
					{#if cdpStore.installedBrowsers.length > 0}
						<div class="space-y-1.5">
							<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Detected Browsers</span>
							{#each cdpStore.installedBrowsers as browser}
								<div class="flex items-center gap-2 px-2 py-1.5 text-[11px] rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-gx-text-secondary">
									<Monitor size={11} class="text-gx-neon" />
									<span class="font-medium">{browser.name}</span>
									<span class="text-gx-text-muted truncate text-[9px]">{browser.path}</span>
								</div>
							{/each}
						</div>
					{/if}
				</div>

			{:else if activeTab === 'playground'}
				<!-- ═══ PLAYGROUND TAB ═══ DevTools Panel -->
				<div class="flex flex-col h-full">
					<!-- DevTools Sub-Tabs -->
					<div class="flex items-center gap-1 px-3 py-1.5 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
						{#each [
							{ id: 'network', icon: Activity, label: 'Network' },
							{ id: 'console', icon: Terminal, label: 'Console' },
							{ id: 'performance', icon: Gauge, label: 'Performance' },
							{ id: 'cookies', icon: Cookie, label: 'Cookies' },
						] as dtab}
							<button
								onclick={() => { playgroundStore.activeDevtoolsTab = dtab.id as 'network' | 'console' | 'performance' | 'cookies'; }}
								class="flex items-center gap-1.5 px-3 py-1 text-[11px] rounded-gx transition-all
									{playgroundStore.activeDevtoolsTab === dtab.id
										? 'bg-gx-bg-elevated text-gx-neon border border-gx-neon/30'
										: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
							>
								<dtab.icon size={12} />
								{dtab.label}
							</button>
						{/each}
						<div class="flex-1"></div>
						{#if !playgroundStore.isPolling}
							<button
								onclick={handleStartDevtools}
								disabled={!cdpStore.activePage}
								class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-neon hover:bg-gx-neon/10 border border-gx-neon/30 transition-all disabled:opacity-40"
							>
								<Play size={10} /> Start Capture
							</button>
						{:else}
							<button
								onclick={() => playgroundStore.stopPolling()}
								class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-status-error hover:bg-gx-status-error/10 border border-gx-status-error/30 transition-all"
							>
								<X size={10} /> Stop
							</button>
						{/if}
					</div>

					<!-- DevTools Content -->
					<div class="flex-1 overflow-auto p-2">
						{#if playgroundStore.activeDevtoolsTab === 'network'}
							<!-- ── Network Waterfall ── -->
							<div class="flex items-center justify-between mb-2">
								<span class="text-[10px] text-gx-text-muted">{playgroundStore.networkEntries.length} requests</span>
								<button onclick={() => playgroundStore.clearNetwork()} class="text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors">
									<Trash2 size={11} />
								</button>
							</div>
							{#if playgroundStore.networkEntries.length === 0}
								<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
									<Activity size={32} class="mb-2 opacity-30" />
									<p class="text-xs">No network activity captured</p>
									<p class="text-[10px] mt-1">Open a CDP page and click "Start Capture"</p>
								</div>
							{:else}
								<div class="space-y-px">
									<!-- Header -->
									<div class="grid grid-cols-12 gap-1 px-2 py-1 text-[9px] text-gx-text-muted font-semibold uppercase tracking-wider">
										<span class="col-span-1">Method</span>
										<span class="col-span-1">Status</span>
										<span class="col-span-5">URL</span>
										<span class="col-span-2">Type</span>
										<span class="col-span-1">Size</span>
										<span class="col-span-2">Time</span>
									</div>
									{#each playgroundStore.networkEntries as entry}
										<div class="grid grid-cols-12 gap-1 px-2 py-1 text-[10px] rounded-gx hover:bg-gx-bg-hover transition-colors group">
											<span class="col-span-1 font-mono font-semibold text-gx-accent-cyan">{entry.method}</span>
											<span class="col-span-1 font-mono {getStatusColor(entry.status)}">{entry.status ?? '...'}</span>
											<span class="col-span-5 truncate text-gx-text-secondary group-hover:text-gx-text-primary" title={entry.url}>
												{entry.url.replace(/^https?:\/\/[^/]+/, '')}
											</span>
											<span class="col-span-2 text-gx-text-muted">{entry.resource_type}</span>
											<span class="col-span-1 text-gx-text-muted">{formatBytes(entry.size_bytes)}</span>
											<span class="col-span-2 flex items-center gap-1">
												{#if entry.duration_ms}
													<div class="flex-1 h-1.5 bg-gx-bg-tertiary rounded-full overflow-hidden">
														<div
															class="h-full rounded-full {entry.duration_ms < 200 ? 'bg-gx-status-success' : entry.duration_ms < 1000 ? 'bg-gx-status-warning' : 'bg-gx-status-error'}"
															style="width: {Math.min(entry.duration_ms / 20, 100)}%"
														></div>
													</div>
													<span class="text-gx-text-muted text-[9px] shrink-0">{entry.duration_ms.toFixed(0)}ms</span>
												{:else}
													<span class="text-gx-text-muted">pending</span>
												{/if}
											</span>
										</div>
									{/each}
								</div>
							{/if}

						{:else if playgroundStore.activeDevtoolsTab === 'console'}
							<!-- ── Console Output ── -->
							<div class="flex items-center justify-between mb-2">
								<span class="text-[10px] text-gx-text-muted">{playgroundStore.consoleEntries.length} entries</span>
								<button onclick={() => playgroundStore.clearConsole()} class="text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors">
									<Trash2 size={11} />
								</button>
							</div>
							{#if playgroundStore.consoleEntries.length === 0}
								<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
									<Terminal size={32} class="mb-2 opacity-30" />
									<p class="text-xs">Console is empty</p>
									<p class="text-[10px] mt-1">Start capture to intercept console.log/warn/error</p>
								</div>
							{:else}
								<div class="space-y-px font-mono">
									{#each playgroundStore.consoleEntries as entry}
										<div class="flex items-start gap-2 px-2 py-0.5 text-[10px] rounded-gx hover:bg-gx-bg-hover transition-colors
											{entry.level === 'error' ? 'bg-gx-status-error/5' : entry.level === 'warn' ? 'bg-gx-status-warning/5' : ''}">
											<span class="shrink-0 w-10 {getConsoleColor(entry.level)} font-semibold">{entry.level}</span>
											<span class="flex-1 text-gx-text-secondary break-all">{entry.text}</span>
											{#if entry.url}
												<span class="shrink-0 text-gx-text-muted text-[9px]">{entry.url.split('/').pop()}{entry.line_number ? `:${entry.line_number}` : ''}</span>
											{/if}
										</div>
									{/each}
								</div>
							{/if}

						{:else if playgroundStore.activeDevtoolsTab === 'performance'}
							<!-- ── Performance Metrics ── -->
							<div class="flex items-center justify-between mb-3">
								<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Page Performance</span>
								<button
									onclick={handleRefreshPerf}
									disabled={!cdpStore.activePage}
									class="flex items-center gap-1 text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors disabled:opacity-40"
								>
									<RefreshCw size={11} /> Refresh
								</button>
							</div>
							{#if !playgroundStore.perfMetrics}
								<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
									<Gauge size={32} class="mb-2 opacity-30" />
									<p class="text-xs">No performance data</p>
									<p class="text-[10px] mt-1">Navigate to a page then click Refresh</p>
								</div>
							{:else}
								<div class="grid grid-cols-2 gap-2">
									{#each [
										{ label: 'DOM Content Loaded', value: playgroundStore.perfMetrics.dom_content_loaded_ms, unit: 'ms' },
										{ label: 'Load Event', value: playgroundStore.perfMetrics.load_event_ms, unit: 'ms' },
										{ label: 'First Paint', value: playgroundStore.perfMetrics.first_paint_ms, unit: 'ms' },
										{ label: 'First Contentful Paint', value: playgroundStore.perfMetrics.first_contentful_paint_ms, unit: 'ms' },
										{ label: 'DOM Nodes', value: playgroundStore.perfMetrics.dom_nodes, unit: '' },
										{ label: 'JS Heap Size', value: playgroundStore.perfMetrics.js_heap_size_mb, unit: 'MB' },
									] as metric}
										<div class="rounded-gx border border-gx-border-default bg-gx-bg-tertiary p-3">
											<div class="text-[10px] text-gx-text-muted mb-1">{metric.label}</div>
											<div class="text-lg font-mono font-bold {metric.unit === 'ms' ? getPerfColor(metric.value) : 'text-gx-text-primary'}">
												{metric.value != null ? (metric.unit === 'MB' ? metric.value.toFixed(1) : Math.round(metric.value)) : '—'}
												<span class="text-[10px] text-gx-text-muted font-normal">{metric.unit}</span>
											</div>
										</div>
									{/each}
								</div>
							{/if}

						{:else if playgroundStore.activeDevtoolsTab === 'cookies'}
							<!-- ── Cookie Viewer ── -->
							<div class="flex items-center justify-between mb-2">
								<span class="text-[10px] text-gx-text-muted">{playgroundStore.cookies.length} cookies</span>
								<button
									onclick={handleRefreshCookies}
									disabled={!cdpStore.activePage}
									class="flex items-center gap-1 text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors disabled:opacity-40"
								>
									<RefreshCw size={11} /> Refresh
								</button>
							</div>
							{#if playgroundStore.cookies.length === 0}
								<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
									<Cookie size={32} class="mb-2 opacity-30" />
									<p class="text-xs">No cookies found</p>
									<p class="text-[10px] mt-1">Navigate to a page and click Refresh</p>
								</div>
							{:else}
								<div class="space-y-px">
									<div class="grid grid-cols-12 gap-1 px-2 py-1 text-[9px] text-gx-text-muted font-semibold uppercase tracking-wider">
										<span class="col-span-3">Name</span>
										<span class="col-span-4">Value</span>
										<span class="col-span-2">Domain</span>
										<span class="col-span-1">Secure</span>
										<span class="col-span-1">HttpOnly</span>
										<span class="col-span-1"></span>
									</div>
									{#each playgroundStore.cookies as cookie}
										<div class="grid grid-cols-12 gap-1 px-2 py-1 text-[10px] rounded-gx hover:bg-gx-bg-hover transition-colors">
											<span class="col-span-3 font-mono text-gx-accent-cyan truncate" title={cookie.name}>{cookie.name}</span>
											<span class="col-span-4 font-mono text-gx-text-secondary truncate" title={cookie.value}>{cookie.value}</span>
											<span class="col-span-2 text-gx-text-muted truncate">{cookie.domain}</span>
											<span class="col-span-1">{cookie.secure ? '🔒' : '—'}</span>
											<span class="col-span-1">{cookie.http_only ? '✓' : '—'}</span>
											<span class="col-span-1">
												<button
													onclick={() => cdpStore.activePage && playgroundStore.deleteCookie(cdpStore.activePage, cookie.name)}
													class="text-gx-text-muted hover:text-gx-status-error transition-colors"
												>
													<Trash2 size={10} />
												</button>
											</span>
										</div>
									{/each}
								</div>
							{/if}
						{/if}
					</div>
				</div>

			{:else if activeTab === 'agent'}
				<!-- ═══ AI AGENT TAB ═══ -->
				<div class="p-4 space-y-3">
					<div class="space-y-1.5">
						<label for="agent-task" class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Task Description</label>
						<textarea
							id="agent-task"
							bind:value={taskInput}
							placeholder="Find the top 5 AI papers from arxiv this week and extract their abstracts"
							rows={3}
							class="w-full px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none resize-none"
						></textarea>
					</div>
					<div class="grid grid-cols-2 gap-2">
						<div class="space-y-1">
							<label for="agent-model" class="text-[10px] text-gx-text-muted">Model</label>
							<select id="agent-model" bind:value={agentModel} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary">
								<option value="hermes3:latest">Hermes 3</option>
								<option value="dolphin3:latest">Dolphin 3</option>
								<option value="qwen2.5-coder:7b">Qwen Coder</option>
							</select>
						</div>
						<div class="space-y-1">
							<label for="agent-steps" class="text-[10px] text-gx-text-muted">Max Steps</label>
							<input id="agent-steps" type="number" bind:value={maxSteps} min={1} max={50} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary" />
						</div>
					</div>
					<button
						onclick={handleRunAgent}
						disabled={isLoading || !taskInput.trim()}
						class="w-full flex items-center justify-center gap-2 px-4 py-2.5 text-xs font-medium rounded-gx transition-all
							{isLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-neon/20 text-gx-neon hover:bg-gx-neon/30 border border-gx-neon/30'}"
					>
						{#if isLoading}<Loader2 size={13} class="animate-spin" /> Agent Running...{:else}<Bot size={13} /> Run Agent Task{/if}
					</button>

					{#if browserAgentStore.currentTask}
						<Separator class="bg-gx-border-default" />
						<div class="space-y-1.5">
							<div class="flex items-center gap-2">
								<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Steps</span>
								<Badge variant="outline" class="text-[8px] px-1 py-0 h-3 {browserAgentStore.currentTask.success ? 'border-gx-status-success/30 text-gx-status-success' : 'border-gx-status-error/30 text-gx-status-error'}">
									{browserAgentStore.currentTask.success ? 'OK' : 'FAIL'}
								</Badge>
							</div>
							<div class="space-y-1 max-h-52 overflow-y-auto">
								{#each browserAgentStore.currentTask.steps as step}
									<div class="flex items-start gap-1.5 text-[10px] px-2 py-1.5 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
										<span class="shrink-0 w-3.5 h-3.5 flex items-center justify-center rounded-full text-[8px] font-bold mt-0.5
											{step.success ? 'bg-gx-status-success/20 text-gx-status-success' : 'bg-gx-status-error/20 text-gx-status-error'}">
											{step.step_number}
										</span>
										<div class="min-w-0">
											<span class="text-gx-text-secondary">{step.thought}</span>
											<p class="text-gx-text-muted truncate mt-0.5">{step.observation.slice(0, 80)}</p>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</div>

			{:else if activeTab === 'import'}
				<!-- ═══ IMPORT TAB ═══ Browser Data Import -->
				<div class="p-4 space-y-3">
					<div class="flex items-center gap-2 mb-1">
						<Import size={14} class="text-gx-accent-cyan" />
						<span class="text-xs font-medium text-gx-text-primary">Browser Data Import</span>
					</div>
					<p class="text-[10px] text-gx-text-muted">
						Auto-detected browser profiles. Import bookmarks, history, and settings — no file download needed.
					</p>

					{#if browserImportStore.isScanning}
						<div class="flex items-center gap-2 text-xs text-gx-text-muted">
							<Loader2 size={12} class="animate-spin" /> Scanning...
						</div>
					{:else if browserImportStore.profiles.length === 0}
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-tertiary p-3 text-center">
							<p class="text-xs text-gx-text-muted">No browser profiles found</p>
							<button
								onclick={() => browserImportStore.detectProfiles()}
								class="mt-2 px-3 py-1.5 text-[10px] rounded-gx bg-gx-bg-elevated text-gx-text-muted hover:text-gx-neon border border-gx-border-default transition-all"
							>
								Scan Again
							</button>
						</div>
					{:else}
						<div class="space-y-1.5">
							{#each browserImportStore.profiles as profile}
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-tertiary p-2.5 space-y-2">
									<div class="flex items-center gap-2">
										<Monitor size={13} class="text-gx-accent-cyan shrink-0" />
										<div class="min-w-0 flex-1">
											<p class="text-xs font-medium text-gx-text-primary">{profile.browser_name}</p>
											<p class="text-[9px] text-gx-text-muted truncate">{profile.profile_name} — {profile.profile_path}</p>
										</div>
									</div>
									<div class="flex gap-1">
										{#if profile.has_bookmarks}
											<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 border-gx-accent-cyan/20 text-gx-accent-cyan">
												<BookMarked size={8} class="mr-0.5" /> Bookmarks
											</Badge>
										{/if}
										{#if profile.has_history}
											<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 border-gx-accent-purple/20 text-gx-accent-purple">
												<History size={8} class="mr-0.5" /> History
											</Badge>
										{/if}
										{#if profile.has_passwords}
											<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 border-gx-status-warning/20 text-gx-status-warning">
												<Shield size={8} class="mr-0.5" /> Passwords
											</Badge>
										{/if}
									</div>
									<div class="flex gap-1.5">
										<button
											onclick={() => handleImportAll(profile)}
											disabled={browserImportStore.isImporting}
											class="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-[10px] font-medium rounded-gx transition-all
												{browserImportStore.isImporting ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-accent-cyan/15 text-gx-accent-cyan hover:bg-gx-accent-cyan/25 border border-gx-accent-cyan/30'}"
										>
											{#if browserImportStore.isImporting}
												<Loader2 size={10} class="animate-spin" />
											{:else}
												<ArrowDown size={10} />
											{/if}
											Import All
										</button>
									</div>
								</div>
							{/each}
						</div>
					{/if}

					<!-- Import Result -->
					{#if browserImportStore.lastImportResult}
						<Separator class="bg-gx-border-default" />
						<div class="rounded-gx border border-gx-status-success/20 bg-gx-status-success/5 p-3 space-y-1.5">
							<p class="text-xs font-medium text-gx-status-success">Import Complete</p>
							<div class="text-[10px] text-gx-text-secondary space-y-0.5">
								<p>Bookmarks: <span class="text-gx-neon font-mono">{browserImportStore.lastImportResult.bookmarks_imported}</span></p>
								<p>History: <span class="text-gx-neon font-mono">{browserImportStore.lastImportResult.history_imported}</span></p>
								{#each browserImportStore.lastImportResult.errors as err}
									<p class="text-gx-status-error text-[9px]">{err}</p>
								{/each}
							</div>
						</div>
					{/if}
				</div>

			{:else if activeTab === 'webhook'}
				<!-- ═══ WEBHOOK TAB ═══ n8n / Zapier -->
				<div class="p-4 space-y-3">
					<div class="space-y-1.5">
						<label for="webhook-url" class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Webhook URL</label>
						<input
							id="webhook-url"
							type="url"
							bind:value={webhookUrl}
							placeholder="https://your-n8n.example.com/webhook/..."
							class="w-full px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none"
						/>
					</div>
					<div class="space-y-1.5">
						<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Quick Connect</span>
						<div class="space-y-1">
							<button
								onclick={() => webhookUrl = 'http://localhost:5678/webhook/'}
								class="w-full flex items-center gap-2 text-left text-[11px] px-3 py-2 rounded-gx bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-accent-orange hover:border-gx-accent-orange/30 border border-gx-border-default transition-all"
							>
								<Workflow size={13} class="text-gx-accent-orange" /> n8n Local (localhost:5678)
							</button>
							<button
								onclick={() => webhookUrl = 'https://hooks.zapier.com/hooks/catch/'}
								class="w-full flex items-center gap-2 text-left text-[11px] px-3 py-2 rounded-gx bg-gx-bg-tertiary text-gx-text-muted hover:text-gx-accent-magenta hover:border-gx-accent-magenta/30 border border-gx-border-default transition-all"
							>
								<Zap size={13} class="text-gx-accent-magenta" /> Zapier Webhook
							</button>
						</div>
					</div>
					<button
						onclick={handleSendWebhook}
						disabled={!webhookUrl.trim() || !displayContent}
						class="w-full flex items-center justify-center gap-2 px-4 py-2 text-xs font-medium rounded-gx transition-all
							{!webhookUrl.trim() || !displayContent
								? 'bg-gx-bg-elevated text-gx-text-muted cursor-not-allowed'
								: 'bg-gx-accent-magenta/20 text-gx-accent-magenta hover:bg-gx-accent-magenta/30 border border-gx-accent-magenta/30'}"
					>
						<Send size={13} /> Send to Webhook
					</button>

					<Separator class="bg-gx-border-default" />
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3 space-y-1.5">
						<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Pipeline Flow</span>
						<div class="space-y-1 text-[10px] text-gx-text-muted">
							<p><span class="text-gx-neon">1.</span> Extract via Browser, Playground, or AI Agent</p>
							<p><span class="text-gx-neon">2.</span> Set your webhook URL (n8n / Zapier)</p>
							<p><span class="text-gx-neon">3.</span> Send — AI models in n8n process the data</p>
							<p class="text-gx-text-muted/50 mt-1 text-[9px]">
								Ollama models running in ImpForge can be connected to n8n via the AI Models page.
							</p>
						</div>
					</div>
				</div>
			{/if}
		</div>

		<!-- Right: Content Display -->
		<div class="flex-1 flex flex-col overflow-hidden">
			<!-- Content toolbar -->
			<div class="flex items-center gap-2 px-4 py-2 border-b border-gx-border-default shrink-0 bg-gx-bg-secondary/30">
				<span class="text-[10px] text-gx-text-muted">
					{#if activePageInfo}
						<span class="text-gx-text-secondary">{activePageInfo.title || activePageInfo.url}</span>
					{:else if browserAgentStore.playgroundUrl}
						<span class="text-gx-text-secondary">{browserAgentStore.playgroundUrl}</span>
					{:else}
						Output
					{/if}
				</span>
				<div class="flex-1"></div>
				{#if displayContent || cdpStore.lastScreenshot}
					<button
						onclick={copyContent}
						class="flex items-center gap-1 px-2 py-1 text-[10px] text-gx-text-muted hover:text-gx-neon rounded-gx hover:bg-gx-bg-hover transition-all"
					>
						<Code2 size={11} /> Copy
					</button>
					<button
						onclick={() => { displayOverride = ''; browserAgentStore.clearResults(); }}
						class="flex items-center gap-1 px-2 py-1 text-[10px] text-gx-text-muted hover:text-gx-status-error rounded-gx hover:bg-gx-bg-hover transition-all"
					>
						<Trash2 size={11} /> Clear
					</button>
				{/if}
			</div>

			<!-- Content area -->
			<div class="flex-1 overflow-auto p-4">
				{#if isLoading}
					<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
						<div class="relative">
							<Loader2 size={28} class="animate-spin text-gx-neon" />
							<div class="absolute inset-0 rounded-full blur-md bg-gx-neon/20"></div>
						</div>
						<span class="text-xs">Processing...</span>
					</div>
				{:else if cdpStore.lastScreenshot}
					<!-- Screenshot Preview -->
					<div class="space-y-2">
						<div class="flex items-center gap-2">
							<Camera size={12} class="text-gx-accent-purple" />
							<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Screenshot</span>
						</div>
						<img
							src="data:image/png;base64,{cdpStore.lastScreenshot}"
							alt="Page screenshot"
							class="w-full rounded-gx border border-gx-border-default shadow-lg"
						/>
					</div>
				{:else if browserAgentStore.error || scraperStore.error || cdpStore.error}
					<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/5 p-4">
						<p class="text-xs text-gx-status-error">{browserAgentStore.error || scraperStore.error || cdpStore.error}</p>
					</div>
				{:else if displayContent}
					<div class="prose prose-invert prose-sm max-w-none text-gx-text-secondary font-mono text-[11px] leading-relaxed whitespace-pre-wrap">
						{displayContent}
					</div>
				{:else if forgeBrowserStore.aiResult}
					<!-- AI Result Display -->
					<div class="space-y-3">
						<div class="flex items-center gap-2">
							<Sparkles size={14} class="text-gx-accent-purple" />
							<span class="text-xs font-medium text-gx-text-primary">AI {forgeBrowserStore.aiResult.operation}</span>
							<Badge variant="outline" class="text-[8px] border-gx-accent-purple/20 text-gx-accent-purple px-1 py-0">
								{forgeBrowserStore.aiResult.model_used}
							</Badge>
							<div class="flex-1"></div>
							<button
								onclick={() => forgeBrowserStore.clearAiResult()}
								class="text-[10px] text-gx-text-muted hover:text-gx-status-error transition-colors"
							>
								<X size={12} />
							</button>
						</div>
						<div class="text-[9px] text-gx-text-muted truncate">{forgeBrowserStore.aiResult.url}</div>
						<div class="prose prose-invert prose-sm max-w-none text-gx-text-secondary text-[11px] leading-relaxed whitespace-pre-wrap">
							{forgeBrowserStore.aiResult.content}
						</div>
					</div>
				{:else if forgeBrowserStore.aiError}
					<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/5 p-4">
						<div class="flex items-center gap-2 mb-1">
							<Sparkles size={12} class="text-gx-status-error" />
							<span class="text-xs font-medium text-gx-status-error">AI Error</span>
						</div>
						<p class="text-[10px] text-gx-status-error/80">{forgeBrowserStore.aiError}</p>
					</div>
				{:else}
					<!-- Empty State -->
					<div class="flex flex-col items-center justify-center h-full gap-4 text-gx-text-muted">
						<div class="relative">
							<Globe size={44} class="text-gx-border-default" />
							<div class="absolute -inset-2 rounded-full bg-gx-neon/5 blur-xl"></div>
						</div>
						<div class="text-center space-y-1.5">
							<p class="text-sm font-medium text-gx-text-secondary">ImpForge Browser Ready</p>
							<p class="text-[11px] text-gx-text-muted max-w-sm">
								{#if cdpStore.installedBrowsers.length > 0}
									Full CDP browser control active. Navigate, click, fill forms, take screenshots, and execute JavaScript.
								{:else}
									HTTP extraction mode. Install Chrome or Brave for full CDP browser automation.
								{/if}
							</p>
						</div>
						<div class="flex flex-wrap gap-1.5 mt-2">
							<Badge variant="outline" class="text-[9px] border-gx-neon/20 text-gx-neon">Spaces</Badge>
							<Badge variant="outline" class="text-[9px] border-gx-accent-cyan/20 text-gx-accent-cyan">AI Agent</Badge>
							<Badge variant="outline" class="text-[9px] border-gx-accent-purple/20 text-gx-accent-purple">AI Tools</Badge>
							<Badge variant="outline" class="text-[9px] border-gx-accent-magenta/20 text-gx-accent-magenta">Webhooks</Badge>
							<Badge variant="outline" class="text-[9px] border-gx-accent-orange/20 text-gx-accent-orange">Data Import</Badge>
							<Badge variant="outline" class="text-[9px] border-gx-border-default text-gx-text-muted">MIT Licensed</Badge>
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- AI Tools Panel (right, collapsible) -->
		{#if showAiPanel}
			<div class="w-64 shrink-0 border-l border-gx-border-default flex flex-col bg-gx-bg-secondary/30 overflow-y-auto">
				<div class="flex items-center gap-2 px-3 py-2.5 border-b border-gx-border-default">
					<Sparkles size={14} class="text-gx-accent-purple" />
					<span class="text-xs font-semibold text-gx-text-primary">AI Tools</span>
					<div class="flex-1"></div>
					<button onclick={() => showAiPanel = false} class="text-gx-text-muted hover:text-gx-text-secondary transition-colors">
						<X size={12} />
					</button>
				</div>

				<div class="p-3 space-y-3">
					<!-- Target URL -->
					<div class="space-y-1">
						<label for="ai-url" class="text-[9px] text-gx-text-muted font-semibold uppercase tracking-wider">Target URL</label>
						<input
							id="ai-url"
							type="text"
							bind:value={aiTargetUrl}
							placeholder={forgeBrowserStore.activeTab?.url || urlInput || 'https://...'}
							class="w-full px-2 py-1.5 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-accent-purple/50 focus:outline-none"
						/>
						<p class="text-[8px] text-gx-text-muted">Leave empty to use current tab URL</p>
					</div>

					<Separator class="bg-gx-border-default" />

					<!-- Summarize -->
					<button
						onclick={handleAiSummarize}
						disabled={forgeBrowserStore.aiLoading}
						class="w-full flex items-center gap-2 px-3 py-2 text-[11px] rounded-gx transition-all
							{forgeBrowserStore.aiLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-accent-purple/10 text-gx-accent-purple hover:bg-gx-accent-purple/20 border border-gx-accent-purple/20'}"
					>
						{#if forgeBrowserStore.aiLoading}
							<Loader2 size={13} class="animate-spin" />
						{:else}
							<Sparkles size={13} />
						{/if}
						Summarize Page
					</button>

					<!-- Translate -->
					<div class="space-y-1.5">
						<div class="flex gap-1.5">
							<select
								bind:value={aiTranslateLanguage}
								class="flex-1 px-2 py-1.5 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary"
							>
								<option value="German">German</option>
								<option value="English">English</option>
								<option value="French">French</option>
								<option value="Spanish">Spanish</option>
								<option value="Japanese">Japanese</option>
								<option value="Chinese">Chinese</option>
								<option value="Korean">Korean</option>
								<option value="Portuguese">Portuguese</option>
								<option value="Russian">Russian</option>
							</select>
							<button
								onclick={handleAiTranslate}
								disabled={forgeBrowserStore.aiLoading}
								class="flex items-center gap-1 px-2.5 py-1.5 text-[10px] rounded-gx transition-all
									{forgeBrowserStore.aiLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-accent-cyan/10 text-gx-accent-cyan hover:bg-gx-accent-cyan/20 border border-gx-accent-cyan/20'}"
							>
								<Languages size={11} /> Translate
							</button>
						</div>
					</div>

					<!-- Web Clip -->
					<div class="space-y-1.5">
						<div class="flex gap-1.5">
							<select
								bind:value={aiClipFormat}
								class="flex-1 px-2 py-1.5 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary"
							>
								<option value="markdown">Markdown</option>
								<option value="summary">Summary</option>
								<option value="outline">Outline</option>
							</select>
							<button
								onclick={handleAiWebClip}
								disabled={forgeBrowserStore.aiLoading}
								class="flex items-center gap-1 px-2.5 py-1.5 text-[10px] rounded-gx transition-all
									{forgeBrowserStore.aiLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-accent-orange/10 text-gx-accent-orange hover:bg-gx-accent-orange/20 border border-gx-accent-orange/20'}"
							>
								<Scissors size={11} /> Clip
							</button>
						</div>
					</div>

					<Separator class="bg-gx-border-default" />

					<!-- Reader Mode -->
					<button
						onclick={handleAiReaderMode}
						disabled={forgeBrowserStore.aiLoading}
						class="w-full flex items-center gap-2 px-3 py-2 text-[11px] rounded-gx transition-all
							{forgeBrowserStore.aiLoading ? 'bg-gx-bg-elevated text-gx-text-muted' : 'bg-gx-bg-tertiary text-gx-text-secondary hover:text-gx-neon hover:bg-gx-neon/10 border border-gx-border-default hover:border-gx-neon/20'}"
					>
						<BookText size={13} /> Reader Mode
					</button>

					{#if forgeBrowserStore.aiResult}
						<Separator class="bg-gx-border-default" />
						<div class="rounded-gx border border-gx-accent-purple/20 bg-gx-accent-purple/5 p-2 space-y-1">
							<div class="flex items-center gap-1">
								<Sparkles size={10} class="text-gx-accent-purple" />
								<span class="text-[9px] text-gx-accent-purple font-semibold">{forgeBrowserStore.aiResult.operation}</span>
							</div>
							<p class="text-[9px] text-gx-text-muted truncate">{forgeBrowserStore.aiResult.url}</p>
							<p class="text-[10px] text-gx-text-secondary line-clamp-3">{forgeBrowserStore.aiResult.content.slice(0, 200)}...</p>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</div>
</div>
