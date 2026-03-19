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
	import { invoke } from '@tauri-apps/api/core';
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
		Globe, Pencil, Lock, Grid3x3, LayoutGrid, Share2, FileEdit, Briefcase,
		Unplug, FileText, Table2, PenTool, FolderOpen, Mail, Presentation, Users,
		CalendarDays, Download, BookOpen, Plug, Route, Heart, FlaskConical, ShieldCheck,
		Trophy, Bell, Focus, X, Clock, CheckCircle, AlertTriangle, Sparkles, Swords
	} from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';
	import { themeStore } from '$lib/stores/theme.svelte';
	import { layoutManager } from '$lib/stores/layout-manager.svelte';
	import { appLauncherStore } from '$lib/stores/app-launcher.svelte';
	import { WidgetPalette } from '$lib/components/layout/index';
	import ErrorToast from '$lib/components/ErrorToast.svelte';
	import AchievementToast from '$lib/components/AchievementToast.svelte';
	import { achievementStore } from '$lib/stores/achievements.svelte';
	import InnerThoughtsSuggestion from '$lib/components/InnerThoughtsSuggestion.svelte';
	import ChatSidePanel from '$lib/components/chat/ChatSidePanel.svelte';
	import { getSetting, saveSetting, isLoaded, getVisibleModules } from '$lib/stores/settings.svelte';
	import { isOnboardingComplete } from '$lib/stores/onboarding.svelte';
	import OnboardingWizard from '$lib/components/OnboardingWizard.svelte';
	import GuidedTour from '$lib/components/GuidedTour.svelte';
	import ModuleDiscovery from '$lib/components/ModuleDiscovery.svelte';
	import { tourStore } from '$lib/stores/guided-tour.svelte';
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

	// ── Focus Mode (Zen Mode) ────────────────────────────────────────
	let focusMode = $state(false);

	// ── Notification Center ──────────────────────────────────────────
	let notificationPanelOpen = $state(false);
	let notifications = $state<Array<{
		id: string;
		title: string;
		message: string;
		notification_type: string;
		read: boolean;
		created_at: string;
		action_route: string | null;
	}>>([]);
	let unreadCount = $state(0);

	async function loadNotifications() {
		try {
			notifications = await invoke('notifications_list', { unreadOnly: false }) as typeof notifications;
			unreadCount = notifications.filter(n => !n.read).length;
		} catch { /* notifications module may not be ready yet */ }
	}

	async function markNotificationRead(id: string) {
		try {
			await invoke('notifications_mark_read', { id });
			const n = notifications.find(x => x.id === id);
			if (n) {
				n.read = true;
				unreadCount = notifications.filter(x => !x.read).length;
			}
		} catch { /* ignore */ }
	}

	async function markAllRead() {
		try {
			await invoke('notifications_mark_all_read');
			notifications.forEach(n => n.read = true);
			unreadCount = 0;
		} catch { /* ignore */ }
	}

	// ── Enhanced Command Palette ─────────────────────────────────────
	let paletteQuery = $state('');
	let paletteResults = $state<Array<{
		id: string;
		title: string;
		subtitle: string;
		icon: string;
		action_type: string;
		route: string | null;
		score: number;
	}>>([]);
	let paletteLoading = $state(false);

	let paletteDebounce: ReturnType<typeof setTimeout> | null = null;

	async function searchPalette(query: string) {
		paletteLoading = true;
		try {
			paletteResults = await invoke('palette_search', { query }) as typeof paletteResults;
		} catch {
			paletteResults = [];
		}
		paletteLoading = false;
	}

	function onPaletteInput(query: string) {
		paletteQuery = query;
		if (paletteDebounce) clearTimeout(paletteDebounce);
		paletteDebounce = setTimeout(() => searchPalette(query), 150);
	}

	function handlePaletteSelect(result: typeof paletteResults[0]) {
		commandOpen = false;
		paletteQuery = '';
		paletteResults = [];

		// Record the action for "recent" tracking
		invoke('palette_record_action', {
			title: result.title,
			subtitle: result.subtitle,
			icon: result.icon,
			actionType: result.action_type,
			route: result.route,
		}).catch(() => {});

		// Track in activity log
		invoke('activity_track', {
			action: result.action_type,
			module: 'command_palette',
			title: result.title,
			detail: null,
		}).catch(() => {});

		if (result.route === '__toggle_agent_panel') {
			rightPanelOpen = !rightPanelOpen;
		} else if (result.route === '__toggle_layout') {
			layoutManager.toggleEditMode();
		} else if (result.route) {
			goto(result.route);
		}
	}

	// Load palette results on open (shows recents when query is empty)
	$effect(() => {
		if (commandOpen) {
			searchPalette(paletteQuery);
		}
	});

	// ── Global Drag & Drop State ──────────────────────────────────────
	// Any module can set dragData when a drag starts; the drop target
	// inspects the type field to decide how to handle the payload.
	//
	// Supported types:
	//   "text"  — plain text selection (ForgeWriter, ForgeNotes)
	//   "cells" — cell range (ForgeSheets)
	//   "file"  — file reference from File Hub { path, name, mime }
	//   "slide" — slide content (ForgeSlides)
	interface DragPayload {
		type: 'text' | 'cells' | 'file' | 'slide' | string;
		source: string;
		data: unknown;
	}

	let dragData = $state<DragPayload | null>(null);
	let isDragOver = $state(false);

	function handleGlobalDragOver(e: DragEvent) {
		e.preventDefault();
		isDragOver = true;
		if (e.dataTransfer) {
			e.dataTransfer.dropEffect = 'copy';
		}
	}

	function handleGlobalDragLeave() {
		isDragOver = false;
	}

	function handleGlobalDrop(e: DragEvent) {
		e.preventDefault();
		isDragOver = false;

		// If we have internal drag data, route it to the active module
		if (dragData) {
			const payload = dragData;
			const targetRoute = activeRoute.split('/').filter(Boolean)[0] || 'home';

			// Route file drops to the correct module
			if (payload.type === 'file' && typeof payload.data === 'object' && payload.data !== null) {
				const fileData = payload.data as { name?: string; mime?: string };
				const name = fileData.name ?? '';
				const mime = fileData.mime ?? '';
				if (mime.startsWith('image/') && targetRoute === 'canvas') {
					// Image files opened in ForgeCanvas
					goto(`/canvas`);
				} else if (name.endsWith('.pdf') && targetRoute !== 'pdf') {
					goto(`/pdf`);
				} else if (
					(name.endsWith('.md') || name.endsWith('.txt') || name.endsWith('.docx')) &&
					targetRoute !== 'writer'
				) {
					goto(`/writer`);
				} else if (
					(name.endsWith('.csv') || name.endsWith('.xlsx')) &&
					targetRoute !== 'sheets'
				) {
					goto(`/sheets`);
				}
			}

			// Dispatch a custom event so the active page component can handle it
			window.dispatchEvent(
				new CustomEvent('impforge:drop', {
					detail: { payload, targetRoute },
				})
			);

			dragData = null;
			return;
		}

		// Handle external file drops (from the OS file manager)
		if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
			const file = e.dataTransfer.files[0];
			const name = file.name.toLowerCase();
			if (name.endsWith('.pdf')) {
				goto('/pdf');
			} else if (name.endsWith('.csv') || name.endsWith('.xlsx')) {
				goto('/sheets');
			} else if (name.endsWith('.md') || name.endsWith('.txt') || name.endsWith('.docx')) {
				goto('/writer');
			} else if (name.endsWith('.ics')) {
				goto('/calendar');
			}
		}
	}

	// Expose dragData setter as a global for modules to use
	function setGlobalDrag(payload: DragPayload | null) {
		dragData = payload;
	}

	// Make setGlobalDrag available to child components via window
	$effect(() => {
		(window as any).__impforge_setDrag = setGlobalDrag;
		return () => {
			delete (window as any).__impforge_setDrag;
		};
	});

	const allActivities = [
		{ id: 'home', icon: LayoutDashboard, label: 'Dashboard', href: '/' },
		{ id: 'chat', icon: MessageSquare, label: 'Chat', href: '/chat' },
		{ id: 'github', icon: GitBranch, label: 'GitHub', href: '/github' },
		{ id: 'docker', icon: Container, label: 'Docker', href: '/docker' },
		{ id: 'workflows', icon: Workflow, label: 'ForgeFlow', href: '/workflows' },
		{ id: 'ide', icon: Code2, label: 'CodeForge IDE', href: '/ide' },
		{ id: 'agents', icon: Network, label: 'NeuralSwarm', href: '/agents' },
		{ id: 'evaluation', icon: Shield, label: 'Evaluation', href: '/evaluation' },
		{ id: 'ai', icon: Brain, label: 'AI Models', href: '/ai' },
		{ id: 'ai-lab', icon: FlaskConical, label: 'AI Lab', href: '/ai-lab' },
		{ id: 'router', icon: Route, label: 'Model Router', href: '/router' },
		{ id: 'browser', icon: Globe, label: 'Browser Agent', href: '/browser' },
		{ id: 'news', icon: Newspaper, label: 'AI News', href: '/news' },
		{ id: 'social', icon: Share2, label: 'Social Media', href: '/social' },
		{ id: 'writer', icon: FileEdit, label: 'ForgeWriter', href: '/writer' },
		{ id: 'notes', icon: BookOpen, label: 'ForgeNotes', href: '/notes' },
		{ id: 'freelancer', icon: Briefcase, label: 'Freelancer', href: '/freelancer' },
		{ id: 'sheets', icon: Table2, label: 'ForgeSheets', href: '/sheets' },
		{ id: 'pdf', icon: FileText, label: 'ForgePDF', href: '/pdf' },
		{ id: 'canvas', icon: PenTool, label: 'ForgeCanvas', href: '/canvas' },
		{ id: 'slides', icon: Presentation, label: 'ForgeSlides', href: '/slides' },
		{ id: 'mail', icon: Mail, label: 'ForgeMail', href: '/mail' },
		{ id: 'team', icon: Users, label: 'ForgeTeam', href: '/team' },
		{ id: 'calendar', icon: CalendarDays, label: 'Calendar', href: '/calendar' },
		{ id: 'import', icon: Download, label: 'Import', href: '/import' },
		{ id: 'files', icon: FolderOpen, label: 'File Hub', href: '/files' },
		{ id: 'platforms', icon: Unplug, label: 'Platforms', href: '/platforms' },
		{ id: 'connector', icon: Plug, label: 'Connector', href: '/connector' },
		{ id: 'apps', icon: LayoutGrid, label: 'App Library', href: '/apps' },
		{ id: 'health', icon: Heart, label: 'System Health', href: '/health' },
		{ id: 'healing', icon: ShieldCheck, label: 'Self-Healing', href: '/healing' },
		{ id: 'achievements', icon: Trophy, label: 'Achievements', href: '/achievements' },
		{ id: 'quest', icon: Swords, label: 'ForgeQuest', href: '/quest' },
	];

	// Adaptive Navigation — filter based on user profile (arXiv:2412.16837)
	let activities = $derived(
		allActivities.filter(a => getVisibleModules().includes(a.id))
	);

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
		// Ctrl+Shift+Z — Toggle focus/zen mode
		if (mod && e.shiftKey && e.key === 'Z') {
			e.preventDefault();
			focusMode = !focusMode;
			return;
		}
		// Escape exits focus mode or closes command palette / notification panel
		if (e.key === 'Escape') {
			if (focusMode) {
				focusMode = false;
				return;
			}
			if (notificationPanelOpen) {
				notificationPanelOpen = false;
				return;
			}
			if (commandOpen) {
				commandOpen = false;
				return;
			}
		}
	}

	// Inner Thoughts — track user state for proactive suggestions (arXiv:2501.00383)
	let isUserTyping = $state(false);
	let typingTimeout: ReturnType<typeof setTimeout> | null = null;

	function handleTypingActivity() {
		isUserTyping = true;
		if (typingTimeout) clearTimeout(typingTimeout);
		typingTimeout = setTimeout(() => {
			isUserTyping = false;
		}, 3000);
	}

	// Report module + typing state to the Inner Thoughts Engine
	$effect(() => {
		const currentModule = activeRoute.split('/').filter(Boolean)[0] || 'home';
		invoke('thoughts_update_user_state', {
			module: currentModule,
			isTyping: isUserTyping,
		}).catch(() => {});
		// Track module usage for achievements (fire-and-forget)
		if (currentModule && currentModule !== 'home') {
			achievementStore.trackAction(`module:${currentModule}`);
		}
	});

	onMount(() => {
		system.startPolling();
		themeStore.loadThemes();
		themeStore.loadWidgets();
		appLauncherStore.loadApps();
		achievementStore.load();

		// Auto-scan for services on app start (background, non-blocking)
		invoke('connector_scan').catch(() => {});

		// Load notifications on startup and poll every 30s
		loadNotifications();
		const notifInterval = setInterval(loadNotifications, 30_000);

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
			clearInterval(notifInterval);
			if (saveTimer) clearTimeout(saveTimer);
			unlistenMove.then(fn => fn());
			unlistenResize.then(fn => fn());
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} onkeypress={handleTypingActivity} />

<svelte:head>
	<title>ImpForge — AI Workstation Builder</title>
	<meta name="description" content="Your complete AI stack. One desktop app." />
</svelte:head>

<!-- Skip-to-content (WCAG 2.1 AA — first focusable element for keyboard users) -->
<a href="#main-content" class="skip-to-content">Skip to main content</a>

<div class="flex h-screen w-screen overflow-hidden bg-gx-bg-primary text-gx-text-primary">
	<!-- Activity Bar — hidden in Focus Mode -->
	{#if !focusMode}
	<nav data-tour="sidebar" class="{hasEngineStyle && activityBarComponent ? '' : 'bg-gx-bg-secondary'} flex flex-col w-12 border-r border-gx-border-default shrink-0" style={activityBarStyle} aria-label="Main navigation">
		<!-- Top activities -->
		<div class="flex flex-col items-center gap-1 pt-2" role="list">
			{#each activities as item}
				<div role="listitem">
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger>
							<a
								data-tour="nav-{item.id}"
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

		<!-- Pinned Apps (from App Launcher) -->
		{#if appLauncherStore.pinnedApps.length > 0}
			<div class="flex flex-col items-center gap-1 pt-1 mt-1 border-t border-gx-border-default mx-1">
				{#each appLauncherStore.pinnedApps as app (app.id)}
					<Tooltip.Provider>
						<Tooltip.Root>
							<Tooltip.Trigger>
								<a
									href="/apps/{app.id}"
									aria-label={app.name}
									aria-current={activeRoute === `/apps/${app.id}` ? 'page' : undefined}
									class="flex items-center justify-center w-10 h-10 rounded-gx transition-all duration-200
										{activeRoute === `/apps/${app.id}`
											? 'bg-gx-bg-elevated text-gx-neon border-l-2 border-gx-neon'
											: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
								>
									{#if app.icon}
										<img src={app.icon} alt="" class="w-5 h-5 rounded" />
									{:else if app.app_type.type === 'WebView'}
										<Globe size={18} />
									{:else if app.app_type.type === 'WebService'}
										<Monitor size={18} />
									{:else}
										<LayoutGrid size={18} />
									{/if}
								</a>
							</Tooltip.Trigger>
							<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
								{app.name}
							</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>
				{/each}
			</div>
		{/if}

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

			<!-- Discover Modules button -->
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							onclick={() => tourStore.toggleModuleDiscovery()}
							aria-label="Discover Modules"
							class="flex items-center justify-center w-10 h-10 rounded-gx text-gx-text-muted hover:text-gx-accent-purple hover:bg-gx-accent-purple/10 transition-all duration-200"
						>
							<Grid3x3 size={18} />
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content side="right" class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
						Discover Modules
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>

			{#each bottomActivities as item}
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger>
							<a
								data-tour="nav-{item.id}"
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
	{/if}

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

			<!-- Focus / Zen Mode toggle -->
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							onclick={() => focusMode = !focusMode}
							aria-label={focusMode ? 'Exit Focus Mode (Escape)' : 'Enter Focus Mode (Ctrl+Shift+Z)'}
							aria-pressed={focusMode}
							class="flex items-center justify-center w-7 h-7 rounded-gx transition-all duration-200
								{focusMode
									? 'bg-gx-neon/20 text-gx-neon border border-gx-neon/50'
									: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
						>
							<Focus size={14} />
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
						{focusMode ? 'Exit Focus Mode (Esc)' : 'Focus Mode (Ctrl+Shift+Z)'}
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>

			<!-- Notification Bell -->
			<div class="relative">
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger>
							<button
								onclick={() => notificationPanelOpen = !notificationPanelOpen}
								aria-label="Notifications ({unreadCount} unread)"
								class="flex items-center justify-center w-7 h-7 rounded-gx transition-all duration-200
									text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover"
							>
								<Bell size={14} />
								{#if unreadCount > 0}
									<span class="absolute -top-0.5 -right-0.5 w-4 h-4 flex items-center justify-center text-[9px] font-bold rounded-full bg-gx-neon text-gx-bg-primary">
										{unreadCount > 9 ? '9+' : unreadCount}
									</span>
								{/if}
							</button>
						</Tooltip.Trigger>
						<Tooltip.Content class="bg-gx-bg-elevated text-gx-text-primary border-gx-border-default">
							Notifications ({unreadCount} unread)
						</Tooltip.Content>
					</Tooltip.Root>
				</Tooltip.Provider>

				<!-- Notification Dropdown Panel -->
				{#if notificationPanelOpen}
					<div class="absolute right-0 top-9 w-80 max-h-96 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg z-50 flex flex-col overflow-hidden">
						<div class="flex items-center justify-between px-3 py-2 border-b border-gx-border-default shrink-0">
							<span class="text-xs font-semibold text-gx-text-secondary">Notifications</span>
							<div class="flex items-center gap-1">
								{#if unreadCount > 0}
									<button
										onclick={markAllRead}
										class="text-[10px] text-gx-text-muted hover:text-gx-neon transition-colors px-1"
									>
										Mark all read
									</button>
								{/if}
								<button
									onclick={() => notificationPanelOpen = false}
									class="text-gx-text-muted hover:text-gx-text-secondary transition-colors"
								>
									<X size={12} />
								</button>
							</div>
						</div>
						<div class="flex-1 overflow-y-auto">
							{#if notifications.length === 0}
								<div class="flex flex-col items-center justify-center py-8 text-gx-text-muted">
									<Bell size={24} class="mb-2 opacity-30" />
									<span class="text-xs">No notifications yet</span>
								</div>
							{:else}
								{#each notifications.slice(0, 20) as notif (notif.id)}
									<button
										onclick={() => {
											markNotificationRead(notif.id);
											if (notif.action_route) {
												notificationPanelOpen = false;
												goto(notif.action_route);
											}
										}}
										class="w-full text-left px-3 py-2 border-b border-gx-border-default/50 hover:bg-gx-bg-hover transition-colors
											{notif.read ? 'opacity-60' : ''}"
									>
										<div class="flex items-start gap-2">
											<span class="mt-0.5 shrink-0">
												{#if notif.notification_type === 'achievement'}
													<Trophy size={12} class="text-gx-status-warning" />
												{:else if notif.notification_type === 'ai_suggestion'}
													<Sparkles size={12} class="text-gx-accent-magenta" />
												{:else if notif.notification_type === 'reminder'}
													<Clock size={12} class="text-gx-accent-blue" />
												{:else if notif.notification_type === 'workflow_complete'}
													<CheckCircle size={12} class="text-gx-status-success" />
												{:else if notif.notification_type === 'system_alert'}
													<AlertTriangle size={12} class="text-gx-status-error" />
												{:else}
													<Users size={12} class="text-gx-text-muted" />
												{/if}
											</span>
											<div class="flex-1 min-w-0">
												<div class="flex items-center gap-1">
													<span class="text-[11px] font-medium text-gx-text-secondary truncate">{notif.title}</span>
													{#if !notif.read}
														<span class="w-1.5 h-1.5 rounded-full bg-gx-neon shrink-0"></span>
													{/if}
												</div>
												<p class="text-[10px] text-gx-text-muted truncate">{notif.message}</p>
											</div>
										</div>
									</button>
								{/each}
							{/if}
						</div>
					</div>
				{/if}
			</div>

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
		<main
			id="main-content"
			class="flex-1 overflow-hidden {isDragOver ? 'ring-2 ring-gx-neon/40 ring-inset' : ''}"
			tabindex="-1"
			ondragover={handleGlobalDragOver}
			ondragleave={handleGlobalDragLeave}
			ondrop={handleGlobalDrop}
		>
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

		<!-- Status bar — hidden in Focus Mode -->
		{#if !focusMode}
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
				<span class="text-gx-status-success">●</span>
				<span>ForgeFlow</span>
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
		{/if}

		<!-- Focus Mode indicator bar (minimal, bottom-right) -->
		{#if focusMode}
		<div class="absolute bottom-3 right-3 z-40 flex items-center gap-2 px-3 py-1.5 rounded-gx bg-gx-bg-elevated/90 border border-gx-border-default text-[11px] text-gx-text-muted backdrop-blur-sm">
			<Focus size={12} class="text-gx-neon" />
			<span>Focus Mode</span>
			<button
				onclick={() => focusMode = false}
				class="ml-1 text-gx-text-muted hover:text-gx-neon transition-colors"
				aria-label="Exit Focus Mode"
			>
				<X size={12} />
			</button>
		</div>
		{/if}
	</div>
</div>

<!-- Enhanced Command Palette — AI-powered search (Ctrl+K) -->
<Dialog.Root bind:open={commandOpen}>
	<Dialog.Content class="p-0 {hasEngineStyle && commandPaletteComponent ? '' : 'bg-gx-bg-elevated'} border-gx-border-default max-w-lg shadow-gx-glow-lg" style={commandPaletteStyle} aria-label="Command palette">
		<!-- Search input -->
		<div class="flex items-center gap-2 px-3 py-2.5 border-b border-gx-border-default">
			<Search size={16} class="text-gx-text-muted shrink-0" />
			<input
				type="text"
				placeholder="Search modules, actions, documents..."
				class="flex-1 bg-transparent text-sm text-gx-text-primary outline-none placeholder:text-gx-text-muted"
				bind:value={paletteQuery}
				oninput={(e) => onPaletteInput(e.currentTarget.value)}
			/>
			{#if paletteLoading}
				<div class="w-4 h-4 border-2 border-gx-neon/30 border-t-gx-neon rounded-full animate-spin"></div>
			{/if}
			<kbd class="px-1.5 py-0.5 text-[10px] bg-gx-bg-tertiary rounded border border-gx-border-default text-gx-text-muted">Esc</kbd>
		</div>

		<!-- Results list -->
		<div class="max-h-80 overflow-y-auto">
			{#if paletteResults.length === 0 && paletteQuery.length > 0 && !paletteLoading}
				<div class="flex flex-col items-center justify-center py-8 text-gx-text-muted">
					<Search size={24} class="mb-2 opacity-30" />
					<span class="text-xs">No results for "{paletteQuery}"</span>
				</div>
			{:else if paletteResults.length === 0 && paletteQuery.length === 0 && !paletteLoading}
				<!-- Fallback: show default navigation + actions when backend returns no recents -->
				<div class="py-1">
					<div class="px-3 py-1.5 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">Navigation</div>
					{#each activities.slice(0, 8) as item, i}
						<button
							onclick={() => { commandOpen = false; goto(item.href); }}
							class="w-full flex items-center justify-between px-3 py-1.5 text-sm text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
						>
							<span class="flex items-center gap-2">
								<item.icon size={14} />
								<span>{item.label}</span>
							</span>
							{#if i < 9}
								<kbd class="px-1 py-0.5 text-[9px] bg-gx-bg-tertiary rounded border border-gx-border-default">Ctrl+{i + 1}</kbd>
							{/if}
						</button>
					{/each}
				</div>
			{:else}
				<!-- Group results by action_type -->
				{@const recentResults = paletteResults.filter(r => r.id.startsWith('recent_'))}
				{@const navResults = paletteResults.filter(r => r.action_type === 'navigate' && !r.id.startsWith('recent_'))}
				{@const actionResults = paletteResults.filter(r => (r.action_type === 'create' || r.action_type === 'run_command') && !r.id.startsWith('recent_'))}
				{@const docResults = paletteResults.filter(r => r.action_type === 'open_document')}

				{#if recentResults.length > 0}
					<div class="py-1">
						<div class="px-3 py-1.5 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider flex items-center gap-1">
							<Clock size={10} />
							Recent
						</div>
						{#each recentResults as result (result.id)}
							<button
								onclick={() => handlePaletteSelect(result)}
								class="w-full flex items-center gap-2 px-3 py-1.5 text-sm text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
							>
								<Search size={14} class="shrink-0 text-gx-text-muted" />
								<span class="truncate">{result.title}</span>
								<span class="ml-auto text-[10px] text-gx-text-muted truncate max-w-[120px]">{result.subtitle}</span>
							</button>
						{/each}
					</div>
				{/if}

				{#if navResults.length > 0}
					<div class="py-1 border-t border-gx-border-default/50">
						<div class="px-3 py-1.5 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">Modules</div>
						{#each navResults as result (result.id)}
							<button
								onclick={() => handlePaletteSelect(result)}
								class="w-full flex items-center justify-between px-3 py-1.5 text-sm text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
							>
								<span class="flex items-center gap-2">
									<Search size={14} class="shrink-0" />
									<span>{result.title}</span>
								</span>
								<span class="text-[10px] text-gx-text-muted">{result.subtitle}</span>
							</button>
						{/each}
					</div>
				{/if}

				{#if actionResults.length > 0}
					<div class="py-1 border-t border-gx-border-default/50">
						<div class="px-3 py-1.5 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">Actions</div>
						{#each actionResults as result (result.id)}
							<button
								onclick={() => handlePaletteSelect(result)}
								class="w-full flex items-center gap-2 px-3 py-1.5 text-sm text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
							>
								<Sparkles size={14} class="shrink-0 text-gx-accent-magenta" />
								<span>{result.title}</span>
								<span class="ml-auto text-[10px] text-gx-text-muted">{result.subtitle}</span>
							</button>
						{/each}
					</div>
				{/if}

				{#if docResults.length > 0}
					<div class="py-1 border-t border-gx-border-default/50">
						<div class="px-3 py-1.5 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider flex items-center gap-1">
							<FileText size={10} />
							Documents
						</div>
						{#each docResults as result (result.id)}
							<button
								onclick={() => handlePaletteSelect(result)}
								class="w-full flex items-start gap-2 px-3 py-1.5 text-sm text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-neon transition-colors"
							>
								<FileText size={14} class="shrink-0 mt-0.5 text-gx-text-muted" />
								<div class="flex-1 min-w-0 text-left">
									<div class="truncate">{result.title}</div>
									<div class="text-[10px] text-gx-text-muted truncate">{result.subtitle}</div>
								</div>
							</button>
						{/each}
					</div>
				{/if}
			{/if}
		</div>

		<!-- Footer hint -->
		<div class="flex items-center justify-between px-3 py-1.5 border-t border-gx-border-default text-[10px] text-gx-text-muted">
			<span>Type to search modules, actions, and documents</span>
			<span class="flex items-center gap-1">
				<kbd class="px-1 py-0.5 bg-gx-bg-tertiary rounded border border-gx-border-default">Enter</kbd>
				to select
			</span>
		</div>
	</Dialog.Content>
</Dialog.Root>

<!-- Onboarding Wizard — first-run setup overlay -->
{#if showOnboarding}
	<OnboardingWizard />
{/if}

<!-- Guided Tour — spotlight overlay with progressive disclosure -->
<GuidedTour />

<!-- Module Discovery — explore and activate hidden modules -->
<ModuleDiscovery />

<!-- Inner Thoughts — proactive AI suggestion toast (arXiv:2501.00383) -->
<InnerThoughtsSuggestion />

<!-- Error Toast — global error notification overlay -->
<ErrorToast />

<!-- Achievement Toast — celebration pop-up when an achievement unlocks -->
<AchievementToast />
