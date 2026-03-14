<script lang="ts">
	import '../app.css';
	// Fontsource variable fonts — bundled WOFF2, zero network dependency (SIL OFL)
	// Core UI fonts (always loaded — used by app.css defaults)
	import '@fontsource-variable/inter';
	import '@fontsource-variable/jetbrains-mono';
	import '@fontsource-variable/space-grotesk';
	// Extended fonts: lazy-loaded on demand via $lib/utils/lazy-fonts.ts
	// (Outfit, Fira Code, Orbitron, Exo 2, Geist Mono, Comfortaa, DM Sans,
	//  Nunito, Recursive, Mona Sans, Roboto Flex, Oxanium, Montserrat,
	//  Plus Jakarta Sans, Sora)
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import * as Command from '$lib/components/ui/command/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { PaneGroup, Pane, Handle } from '$lib/components/ui/resizable/index.js';
	import {
		MessageSquare, GitBranch, Container, Workflow,
		Settings, LayoutDashboard, Brain, Newspaper,
		Code2, Search, Cpu, HardDrive,
		ChevronRight, Command as CommandIcon, Monitor,
		PanelRightClose, PanelRightOpen, Bot, Activity, Network, Shield,
		Globe, Pencil, Lock, Grid3x3
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';
	import { themeStore } from '$lib/stores/theme.svelte';
	import { layoutManager } from '$lib/stores/layout-manager.svelte';
	import { WidgetPalette } from '$lib/components/layout/index';
	import ErrorToast from '$lib/components/ErrorToast.svelte';
	import ChatSidePanel from '$lib/components/chat/ChatSidePanel.svelte';
	import { getSetting, saveSetting, isLoaded } from '$lib/stores/settings.svelte';
	import { isOnboardingComplete } from '$lib/stores/onboarding.svelte';
	import OnboardingWizard from '$lib/components/OnboardingWizard.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	let { children } = $props();

	// BenikUI style engine integration
	const widgetId = 'app-layout';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let activityBarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'activity-bar'));
	let activityBarStyle = $derived(hasEngineStyle && activityBarComponent ? componentToCSS(activityBarComponent) : '');
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let headerStyle = $derived(hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : '');
	let statusBarComponent = $derived(styleEngine.getComponentStyle(widgetId, 'status-bar'));
	let statusBarStyle = $derived(hasEngineStyle && statusBarComponent ? componentToCSS(statusBarComponent) : '');
	let agentPanelComponent = $derived(styleEngine.getComponentStyle(widgetId, 'agent-panel'));
	let agentPanelStyle = $derived(hasEngineStyle && agentPanelComponent ? componentToCSS(agentPanelComponent) : '');
	let commandPaletteComponent = $derived(styleEngine.getComponentStyle(widgetId, 'command-palette'));
	let commandPaletteStyle = $derived(hasEngineStyle && commandPaletteComponent ? componentToCSS(commandPaletteComponent) : '');
	let showOnboarding = $derived(isLoaded() && !isOnboardingComplete());
	let commandOpen = $state(false);
	let rightPanelOpen = $state(false);
	let chatPanelOpen = $state(false);
	let chatPlacement = $derived(getSetting('chatPlacement'));

	const activities = [
		{ id: 'home', icon: LayoutDashboard, label: 'Dashboard', href: '/' },
		{ id: 'chat', icon: MessageSquare, label: 'Chat', href: '/chat' },
		{ id: 'github', icon: GitBranch, label: 'GitHub', href: '/github' },
		{ id: 'docker', icon: Container, label: 'Docker', href: '/docker' },
		{ id: 'n8n', icon: Workflow, label: 'n8n & Services', href: '/n8n' },
		{ id: 'ide', icon: Code2, label: 'CodeForge IDE', href: '/ide' },
		{ id: 'agents', icon: Network, label: 'NeuralSwarm', href: '/agents' },
		{ id: 'evaluation', icon: Shield, label: 'Evaluation', href: '/evaluation' },
		{ id: 'ai', icon: Brain, label: 'AI Models', href: '/ai' },
		{ id: 'browser', icon: Globe, label: 'Browser Agent', href: '/browser' },
		{ id: 'news', icon: Newspaper, label: 'AI News', href: '/news' },
	];

	const bottomActivities = [
		{ id: 'settings', icon: Settings, label: 'Settings', href: '/settings' },
	];

	let activeRoute = $derived($page.url.pathname);

	function handleKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'k') {
			e.preventDefault();
			commandOpen = !commandOpen;
		}
		if (mod && e.key === 'b') {
			e.preventDefault();
			rightPanelOpen = !rightPanelOpen;
		}
		if (mod && e.key === 'e') {
			e.preventDefault();
			layoutManager.toggleEditMode();
		}
		if (mod && e.key === ',') {
			e.preventDefault();
			window.location.href = '/settings';
		}
		// Ctrl+J — Toggle chat side panel
		if (mod && e.key === 'j') {
			e.preventDefault();
			chatPanelOpen = !chatPanelOpen;
			return;
		}
		// Ctrl+Shift+F — Toggle convergence mode
		if (mod && e.shiftKey && e.key === 'F') {
			e.preventDefault();
			if ($page.url.pathname === '/convergence') {
				goto('/chat');
			} else {
				goto('/convergence');
			}
			return;
		}
		// Ctrl+1-9 — quick nav to activities by position
		if (mod && e.key >= '1' && e.key <= '9') {
			const idx = parseInt(e.key) - 1;
			const all = [...activities, ...bottomActivities];
			if (idx < all.length) {
				e.preventDefault();
				window.location.href = all[idx].href;
			}
		}
		// Escape closes command palette
		if (e.key === 'Escape' && commandOpen) {
			commandOpen = false;
		}
	}

	onMount(() => {
		system.startPolling();
		themeStore.loadThemes();
		themeStore.loadWidgets();

		// T4.2 — Restore and persist window position/size
		const win = getCurrentWindow();
		let saveTimer: ReturnType<typeof setTimeout> | null = null;

		async function restoreWindowGeometry() {
			try {
				const saved = getSetting('windowGeometry') as { x: number; y: number; w: number; h: number } | undefined;
				if (saved && saved.w > 400 && saved.h > 300) {
					const { PhysicalPosition, PhysicalSize } = await import('@tauri-apps/api/dpi');
					await win.setPosition(new PhysicalPosition(Math.max(0, saved.x), Math.max(0, saved.y)));
					await win.setSize(new PhysicalSize(saved.w, saved.h));
				}
			} catch { /* first run or invalid data — use defaults from tauri.conf.json */ }
		}

		async function saveWindowGeometry() {
			try {
				const pos = await win.outerPosition();
				const size = await win.outerSize();
				saveSetting('windowGeometry', { x: pos.x, y: pos.y, w: size.width, h: size.height });
			} catch { /* window API unavailable during SSR/dev */ }
		}

		function debouncedSave() {
			if (saveTimer) clearTimeout(saveTimer);
			saveTimer = setTimeout(saveWindowGeometry, 500);
		}

		restoreWindowGeometry();

		const unlistenMove = win.onMoved(debouncedSave);
		const unlistenResize = win.onResized(debouncedSave);

		return () => {
			system.stopPolling();
			if (saveTimer) clearTimeout(saveTimer);
			unlistenMove.then(fn => fn());
			unlistenResize.then(fn => fn());
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<svelte:head>
	<title>ImpForge — AI Workstation Builder</title>
	<meta name="description" content="Your complete AI stack. One desktop app." />
</svelte:head>

<!-- Skip-to-content (WCAG 2.1 AA — first focusable element for keyboard users) -->
<a href="#main-content" class="skip-to-content">Skip to main content</a>

<div class="flex h-screen w-screen overflow-hidden bg-gx-bg-primary text-gx-text-primary">
	<!-- Activity Bar (leftmost, 48px) — ARIA: navigation landmark -->
	<nav class="{hasEngineStyle && activityBarComponent ? '' : 'bg-gx-bg-secondary'} flex flex-col w-12 border-r border-gx-border-default shrink-0" style={activityBarStyle} aria-label="Main navigation">
		<!-- Top activities -->
		<div class="flex flex-col items-center gap-1 pt-2" role="list">
			{#each activities as item}
				<div role="listitem">
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger>
							<a
								href={item.href}
								aria-label={item.label}
								aria-current={activeRoute === item.href || (item.href !== '/' && activeRoute.startsWith(item.href)) ? 'page' : undefined}
								class="flex items-center justify-center w-10 h-10 rounded-gx transition-all duration-200
									{activeRoute === item.href || (item.href !== '/' && activeRoute.startsWith(item.href))
										? 'bg-gx-bg-elevated text-gx-neon border-l-2 border-gx-neon'
										: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
							>
								<item.icon size={20} />
							</a>
						</Tooltip.Trigger>
						<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
							{item.label}
						</Tooltip.Content>
					</Tooltip.Root>
				</Tooltip.Provider>
				</div>
			{/each}
		</div>

		<!-- Spacer -->
		<div class="flex-1"></div>

		<!-- Bottom activities -->
		<div class="flex flex-col items-center gap-1 pb-2">
			<!-- Layout edit mode toggle (ElvUI "Toggle Anchors") -->
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							onclick={() => layoutManager.toggleEditMode()}
							aria-label={layoutManager.editMode ? 'Lock layout' : 'Edit layout'}
							aria-pressed={layoutManager.editMode}
							class="flex items-center justify-center w-10 h-10 rounded-gx transition-all duration-200
								{layoutManager.editMode
									? 'bg-gx-accent-purple/20 text-gx-accent-purple border border-gx-accent-purple/50 shadow-[0_0_12px_rgba(153,51,255,0.3)]'
									: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
						>
							{#if layoutManager.editMode}
								<Lock size={18} />
							{:else}
								<Pencil size={18} />
							{/if}
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
						{layoutManager.editMode ? 'Lock Layout (Ctrl+E)' : 'Edit Layout (Ctrl+E)'}
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>

			<!-- Right panel toggle -->
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							onclick={() => rightPanelOpen = !rightPanelOpen}
							aria-label={rightPanelOpen ? 'Close agent panel' : 'Open agent panel'}
							aria-expanded={rightPanelOpen}
							class="flex items-center justify-center w-10 h-10 rounded-gx transition-all duration-200
								{rightPanelOpen
									? 'bg-gx-bg-elevated text-gx-neon'
									: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
						>
							{#if rightPanelOpen}
								<PanelRightClose size={20} />
							{:else}
								<PanelRightOpen size={20} />
							{/if}
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
						Agent Panel (Ctrl+B)
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>

			{#each bottomActivities as item}
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger>
							<a
								href={item.href}
								class="flex items-center justify-center w-10 h-10 rounded-gx transition-all duration-200
									{activeRoute === item.href
										? 'bg-gx-bg-elevated text-gx-neon'
										: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
							>
								<item.icon size={20} />
							</a>
						</Tooltip.Trigger>
						<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
							{item.label}
						</Tooltip.Content>
					</Tooltip.Root>
				</Tooltip.Provider>
			{/each}
		</div>
	</nav>

	<!-- Main content area with PaneForge resizable panels -->
	<div class="flex flex-col flex-1 min-w-0">
		<!-- Top bar with breadcrumb + search — ARIA: banner landmark -->
		<header class="flex items-center h-10 px-3 {hasEngineStyle && headerComponent ? '' : 'bg-gx-bg-secondary'} border-b border-gx-border-default shrink-0 gap-2" style={headerStyle}>
			<!-- Breadcrumb -->
			<nav class="flex items-center gap-1 text-sm text-gx-text-muted" aria-label="Breadcrumb">
				<span class="text-gx-neon font-semibold">ImpForge</span>
				{#if activeRoute !== '/'}
					<ChevronRight size={14} />
					<span class="text-gx-text-secondary capitalize">
						{activeRoute.split('/').filter(Boolean)[0] || 'Dashboard'}
					</span>
				{/if}
			</nav>

			<div class="flex-1"></div>

			<!-- Edit mode indicator -->
			{#if layoutManager.editMode}
				<div class="flex items-center gap-1.5 px-2 py-0.5 rounded-gx bg-gx-accent-purple/15 border border-gx-accent-purple/40 animate-pulse">
					<Grid3x3 size={12} class="text-gx-accent-purple" />
					<span class="text-[10px] font-semibold text-gx-accent-purple uppercase tracking-wider">Layout Edit Mode</span>
					{#if layoutManager.isDirty}
						<span class="w-1.5 h-1.5 rounded-full bg-gx-status-warning"></span>
					{/if}
				</div>
			{/if}

			<!-- Command palette trigger -->
			<button
				onclick={() => commandOpen = true}
				aria-label="Open command palette (Ctrl+K)"
				class="flex items-center gap-2 px-3 py-1 text-xs text-gx-text-muted bg-gx-bg-tertiary border border-gx-border-default rounded-gx hover:border-gx-neon hover:text-gx-text-secondary transition-all"
			>
				<Search size={12} />
				<span>Search...</span>
				<kbd class="px-1 py-0.5 text-[10px] bg-gx-bg-elevated rounded border border-gx-border-default">
					Ctrl+K
				</kbd>
			</button>
		</header>

		<!-- Resizable content area — ARIA: main landmark -->
		<main id="main-content" class="flex-1 overflow-hidden" tabindex="-1">
			{#if rightPanelOpen || layoutManager.editMode}
				<PaneGroup direction="horizontal" class="h-full">
					<Pane defaultSize={layoutManager.editMode ? 70 : 75} minSize={40} class="overflow-auto">
						{@render children()}
					</Pane>
					<Handle withHandle class="bg-gx-border-default hover:bg-gx-neon/30 transition-colors" />
					<Pane defaultSize={layoutManager.editMode ? 30 : 25} minSize={15} maxSize={45} class="overflow-hidden">
						{#if layoutManager.editMode}
							<!-- Widget Palette (edit mode) -->
							<WidgetPalette />
						{:else}
						<!-- Agent / NeuralSwarm Side Panel -->
						<div class="flex flex-col h-full {hasEngineStyle && agentPanelComponent ? '' : 'bg-gx-bg-secondary'} border-l border-gx-border-default" style={agentPanelStyle}>
							<!-- Panel header -->
							<div class="flex items-center gap-2 h-9 px-3 border-b border-gx-border-default shrink-0">
								<Network size={14} class="text-gx-neon" />
								<span class="text-xs font-medium text-gx-text-secondary">Agent Panel</span>
								<div class="flex-1"></div>
								<button
									onclick={() => rightPanelOpen = false}
									class="text-gx-text-muted hover:text-gx-neon transition-colors"
								>
									<PanelRightClose size={14} />
								</button>
							</div>

							<!-- Quick Agent Status -->
							<div class="flex-1 overflow-y-auto p-3 space-y-3">
								<!-- NeuralSwarm Status -->
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<Activity size={13} class="text-gx-neon" />
										<span class="text-xs font-medium text-gx-text-secondary">NeuralSwarm</span>
									</div>
									<div class="space-y-1.5">
										<div class="flex items-center justify-between text-[11px]">
											<span class="text-gx-text-muted">Router</span>
											<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-neon/30 text-gx-neon">Auto</Badge>
										</div>
										<div class="flex items-center justify-between text-[11px]">
											<span class="text-gx-text-muted">Active Model</span>
											<span class="text-gx-text-secondary font-mono">llama-4-scout</span>
										</div>
										<div class="flex items-center justify-between text-[11px]">
											<span class="text-gx-text-muted">Requests</span>
											<span class="text-gx-text-secondary">0 today</span>
										</div>
									</div>
								</div>

								<!-- Agent Pool -->
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<Bot size={13} class="text-gx-accent-magenta" />
										<span class="text-xs font-medium text-gx-text-secondary">Agent Pool</span>
									</div>
									<div class="space-y-1">
										{#each ['Orchestrator', 'Coder', 'Researcher', 'Debugger'] as agent}
											<div class="flex items-center gap-2 text-[11px] py-0.5">
												<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success shrink-0"></span>
												<span class="text-gx-text-muted">{agent}</span>
												<span class="ml-auto text-gx-text-muted">idle</span>
											</div>
										{/each}
									</div>
								</div>

								<!-- Quick Actions -->
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<Brain size={13} class="text-gx-accent-blue" />
										<span class="text-xs font-medium text-gx-text-secondary">Quick Actions</span>
									</div>
									<div class="space-y-1">
										<button class="w-full text-left text-[11px] text-gx-text-muted hover:text-gx-neon py-1 px-2 rounded hover:bg-gx-bg-hover transition-colors">
											Start Agent Task...
										</button>
										<button class="w-full text-left text-[11px] text-gx-text-muted hover:text-gx-neon py-1 px-2 rounded hover:bg-gx-bg-hover transition-colors">
											View Agent Topology
										</button>
										<button class="w-full text-left text-[11px] text-gx-text-muted hover:text-gx-neon py-1 px-2 rounded hover:bg-gx-bg-hover transition-colors">
											NeuralSwarm Metrics
										</button>
									</div>
								</div>
							</div>
						</div>
						{/if}
					</Pane>
				</PaneGroup>
			{:else}
				<div class="h-full flex overflow-hidden">
					<div class="flex-1 overflow-auto">
						{@render children()}
					</div>
					<!-- Chat Side Panel (when placement is 'side-panel') -->
					{#if chatPlacement === 'side-panel'}
						<ChatSidePanel open={chatPanelOpen} onClose={() => chatPanelOpen = false} />
					{/if}
				</div>
			{/if}
		</main>

		<!-- Status bar — live metrics — ARIA: contentinfo landmark -->
		<footer class="flex items-center h-6 px-3 {hasEngineStyle && statusBarComponent ? '' : 'bg-gx-bg-secondary'} border-t border-gx-border-default text-[11px] text-gx-text-muted shrink-0 gap-3" style={statusBarStyle} aria-label="System status">
			<span class="text-gx-neon font-semibold">ImpForge</span>
			<span>v0.6.0</span>
			<Separator orientation="vertical" class="h-3 bg-gx-border-default" />

			<div class="flex items-center gap-1">
				<span class={system.services.ollama === 'online' ? 'text-gx-status-success' : system.services.ollama === 'checking' ? 'text-gx-status-warning' : 'text-gx-status-error'}>●</span>
				<span>Ollama</span>
			</div>
			<div class="flex items-center gap-1">
				<span class={system.services.docker === 'online' ? 'text-gx-status-success' : system.services.docker === 'checking' ? 'text-gx-status-warning' : 'text-gx-status-error'}>●</span>
				<span>Docker</span>
			</div>
			<div class="flex items-center gap-1">
				<span class={system.services.n8n === 'online' ? 'text-gx-status-success' : system.services.n8n === 'checking' ? 'text-gx-status-warning' : 'text-gx-status-error'}>●</span>
				<span>n8n</span>
			</div>
			<div class="flex items-center gap-1">
				<span class={system.services.neuralswarm === 'online' ? 'text-gx-status-success' : system.services.neuralswarm === 'checking' ? 'text-gx-status-warning' : 'text-gx-status-error'}>●</span>
				<span>Swarm</span>
			</div>

			<div class="flex-1"></div>

			{#if system.stats}
				<div class="flex items-center gap-1">
					<Cpu size={11} />
					<span>{system.stats.cpu_percent.toFixed(0)}%</span>
				</div>
				<div class="flex items-center gap-1">
					<HardDrive size={11} />
					<span>{system.stats.ram_used_gb.toFixed(1)}/{system.stats.ram_total_gb.toFixed(0)}G</span>
				</div>
				{#if system.stats.gpu_vram_used_mb != null}
					<div class="flex items-center gap-1">
						<Monitor size={11} class="text-gx-accent-magenta" />
						<span>{(system.stats.gpu_vram_used_mb / 1024).toFixed(1)}/{((system.stats.gpu_vram_total_mb ?? 0) / 1024).toFixed(0)}G</span>
						{#if system.stats.gpu_temp_c != null}
							<span class="text-gx-text-muted">{system.stats.gpu_temp_c.toFixed(0)}°</span>
						{/if}
					</div>
				{/if}
			{:else}
				<div class="flex items-center gap-1 animate-pulse">
					<Cpu size={11} />
					<span>Loading...</span>
				</div>
			{/if}

			{#if layoutManager.editMode}
				<Badge variant="outline" class="text-[10px] px-1 py-0 h-4 border-gx-accent-purple/50 text-gx-accent-purple">
					Editing
				</Badge>
			{/if}
			<Badge variant="outline" class="text-[10px] px-1 py-0 h-4 border-gx-border-default text-gx-text-muted">
				Free Tier
			</Badge>
		</footer>
	</div>
</div>

<!-- Command Palette — ARIA: dialog with search -->
<Dialog.Root bind:open={commandOpen}>
	<Dialog.Content class="p-0 {hasEngineStyle && commandPaletteComponent ? '' : 'bg-gx-bg-elevated'} border-gx-border-default max-w-lg shadow-gx-glow-lg" style={commandPaletteStyle} aria-label="Command palette">
		<Command.Root class="bg-transparent">
			<Command.Input placeholder="Type a command or search..." class="border-b border-gx-border-default bg-transparent text-gx-text-primary" />
			<Command.List class="max-h-80">
				<Command.Empty class="text-gx-text-muted">No results found.</Command.Empty>

				<Command.Group heading="Navigation">
					{#each activities as item, i}
						<Command.Item
							onSelect={() => { commandOpen = false; window.location.href = item.href; }}
							class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon flex items-center justify-between"
						>
							<span class="flex items-center">
								<item.icon size={16} class="mr-2" />
								{item.label}
							</span>
							{#if i < 9}
								<kbd class="px-1 py-0.5 text-[9px] bg-gx-bg-tertiary rounded border border-gx-border-default ml-auto">Ctrl+{i + 1}</kbd>
							{/if}
						</Command.Item>
					{/each}
					<Command.Item
						onSelect={() => { commandOpen = false; window.location.href = '/settings'; }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon flex items-center justify-between"
					>
						<span class="flex items-center">
							<Settings size={16} class="mr-2" />
							Settings
						</span>
						<kbd class="px-1 py-0.5 text-[9px] bg-gx-bg-tertiary rounded border border-gx-border-default ml-auto">Ctrl+,</kbd>
					</Command.Item>
				</Command.Group>

				<Command.Separator class="bg-gx-border-default" />

				<Command.Group heading="Actions">
					<Command.Item
						onSelect={() => { commandOpen = false; window.location.href = '/chat'; }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon"
					>
						<Brain size={16} class="mr-2" />
						New Chat
					</Command.Item>
					<Command.Item
						onSelect={() => { commandOpen = false; window.location.href = '/docker'; }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon"
					>
						<Container size={16} class="mr-2" />
						Start Container
					</Command.Item>
					<Command.Item
						onSelect={() => { commandOpen = false; window.location.href = '/n8n'; }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon"
					>
						<Workflow size={16} class="mr-2" />
						Create Workflow
					</Command.Item>
					<Command.Item
						onSelect={() => { commandOpen = false; rightPanelOpen = !rightPanelOpen; }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon flex items-center justify-between"
					>
						<span class="flex items-center">
							<Network size={16} class="mr-2" />
							Toggle Agent Panel
						</span>
						<kbd class="px-1 py-0.5 text-[9px] bg-gx-bg-tertiary rounded border border-gx-border-default ml-auto">Ctrl+B</kbd>
					</Command.Item>
					<Command.Item
						onSelect={() => { commandOpen = false; layoutManager.toggleEditMode(); }}
						class="text-gx-text-secondary data-[selected]:bg-gx-bg-hover data-[selected]:text-gx-neon flex items-center justify-between"
					>
						<span class="flex items-center">
							<Grid3x3 size={16} class="mr-2" />
							{layoutManager.editMode ? 'Lock Layout' : 'Edit Layout'}
						</span>
						<kbd class="px-1 py-0.5 text-[9px] bg-gx-bg-tertiary rounded border border-gx-border-default ml-auto">Ctrl+E</kbd>
					</Command.Item>
				</Command.Group>
			</Command.List>
		</Command.Root>
	</Dialog.Content>
</Dialog.Root>

<!-- Onboarding Wizard — first-run setup overlay -->
{#if showOnboarding}
	<OnboardingWizard />
{/if}

<!-- Error Toast — global error notification overlay -->
<ErrorToast />
