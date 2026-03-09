<script lang="ts">
	import { onMount } from 'svelte';
	import { loadSettings, saveSetting, getSettings } from '$lib/stores/settings.svelte';
	import { themeStore, type NexusTheme } from '$lib/stores/theme.svelte';
	import { license } from '$lib/stores/license.svelte';
	import { forgeWatchStore, type DiscoveredPath } from '$lib/stores/forgewatch.svelte';
	import { sunshineStore } from '$lib/stores/sunshine.svelte';
	import { digestStore } from '$lib/stores/digest.svelte';
	import {
		Settings, Key, Palette, Globe, Server, Save, Check,
		AlertCircle, Eye, EyeOff, ExternalLink, RefreshCw,
		Download, Upload, Trash2, Plus, Copy, Paintbrush, Layout, Grid3x3,
		Shield, ShieldCheck, ShieldAlert, Wrench, Crown, Sparkles, Loader2,
		FolderSearch, FolderPlus, FolderMinus, HardDrive, Search,
		Monitor, Play, Square, Wifi, Brain, Sliders
	} from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const seWidgetId = 'page-settings';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(seWidgetId)) {
			styleEngine.loadWidgetStyle(seWidgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(seWidgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(seWidgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let sectionComponent = $derived(styleEngine.getComponentStyle(seWidgetId, 'section'));
	let sectionStyle = $derived(hasEngineStyle && sectionComponent ? componentToCSS(sectionComponent) : '');
	let seHeaderComponent = $derived(styleEngine.getComponentStyle(seWidgetId, 'header'));
	let seHeaderStyle = $derived(hasEngineStyle && seHeaderComponent ? componentToCSS(seHeaderComponent) : '');

	let loaded = $state(false);
	let showApiKey = $state(false);
	let ollamaStatus = $state<'idle' | 'checking' | 'online' | 'offline'>('idle');
	let serviceStatuses = $state<Record<string, 'idle' | 'checking' | 'online' | 'offline'>>({
		n8n: 'idle', langflow: 'idle', openwebui: 'idle',
		grafana: 'idle', searxng: 'idle', comfyui: 'idle',
	});
	let saveFlash = $state(false);
	let fontSize = $state(14);

	const services = [
		{ key: 'n8n', name: 'n8n', defaultUrl: 'http://localhost:5678', description: 'Workflow Automation' },
		{ key: 'langflow', name: 'LangFlow', defaultUrl: 'http://localhost:7860', description: 'AI Flow Builder' },
		{ key: 'openwebui', name: 'Open WebUI', defaultUrl: 'http://localhost:3000', description: 'Chat Interface' },
		{ key: 'grafana', name: 'Grafana', defaultUrl: 'http://localhost:3001', description: 'Monitoring Dashboard' },
		{ key: 'searxng', name: 'SearXNG', defaultUrl: 'http://localhost:8080', description: 'Private Search Engine' },
		{ key: 'comfyui', name: 'ComfyUI', defaultUrl: 'http://localhost:8188', description: 'Image Generation' },
	];

	let serviceUrls = $state<Record<string, string>>({
		n8n: 'http://localhost:5678', langflow: 'http://localhost:7860', openwebui: 'http://localhost:3000',
		grafana: 'http://localhost:3001', searxng: 'http://localhost:8080', comfyui: 'http://localhost:8188',
	});

	let settings = $derived(getSettings());
	let apiKeySet = $derived(settings.openrouterKey.length > 0);

	let newWatchPath = $state('');

	onMount(async () => { await loadSettings(); license.load(); forgeWatchStore.init(); sunshineStore.init(); digestStore.init(); loaded = true; });

	async function handleApiKeyChange(e: Event) {
		await saveSetting('openrouterKey', (e.target as HTMLInputElement).value);
		flashSave();
	}
	async function handleOllamaUrlChange(e: Event) {
		await saveSetting('ollamaUrl', (e.target as HTMLInputElement).value);
		flashSave();
	}
	async function handleProviderChange(e: Event) {
		const value = (e.target as HTMLSelectElement).value;
		if (value === 'auto') { await saveSetting('autoRouting', true); await saveSetting('preferLocalModels', true); }
		else if (value === 'ollama') { await saveSetting('autoRouting', false); await saveSetting('preferLocalModels', true); }
		else { await saveSetting('autoRouting', false); await saveSetting('preferLocalModels', false); }
		flashSave();
	}
	function getCurrentProvider(): string {
		if (settings.autoRouting) return 'auto';
		if (settings.preferLocalModels) return 'ollama';
		return 'openrouter';
	}
	async function testOllamaConnection() {
		ollamaStatus = 'checking';
		try { const res = await fetch(settings.ollamaUrl + '/api/tags', { signal: AbortSignal.timeout(5000) }); ollamaStatus = res.ok ? 'online' : 'offline'; }
		catch { ollamaStatus = 'offline'; }
	}
	async function testServiceConnection(key: string) {
		serviceStatuses[key] = 'checking';
		try { await fetch(serviceUrls[key], { signal: AbortSignal.timeout(5000), mode: 'no-cors' }); serviceStatuses[key] = 'online'; }
		catch { serviceStatuses[key] = 'offline'; }
	}
	function flashSave() { saveFlash = true; setTimeout(() => { saveFlash = false; }, 1500); }
	function statusDot(s: string): string { return s === 'online' ? 'text-gx-status-success' : s === 'offline' ? 'text-gx-status-error' : s === 'checking' ? 'text-gx-status-warning animate-pulse' : 'text-gx-text-muted'; }
	function statusLabel(s: string): string { return s === 'online' ? 'Connected' : s === 'offline' ? 'Unreachable' : s === 'checking' ? 'Checking...' : 'Not tested'; }

	// ── Theme Picker ──────────────────────────────────
	let importString = $state('');
	let exportString = $state('');
	let showImport = $state(false);
	let showExport = $state(false);
	let showCustomCreate = $state(false);
	let customName = $state('');
	let customColors = $state<Record<string, string>>({
		'--color-gx-neon': '#00ff66', '--color-gx-bg-primary': '#0a0a0f',
		'--color-gx-bg-secondary': '#12121a', '--color-gx-bg-tertiary': '#1a1a25',
		'--color-gx-text-primary': '#e8e8f0', '--color-gx-accent-cyan': '#00e5ff',
		'--color-gx-accent-purple': '#b366ff', '--color-gx-accent-magenta': '#ff3399',
		'--color-gx-accent-orange': '#ff9500',
	});
	const themePreviewVars = ['--color-gx-neon', '--color-gx-bg-primary', '--color-gx-accent-cyan', '--color-gx-accent-purple'];
	function getThemeColor(theme: NexusTheme, varName: string): string {
		return theme.variables.find(([k]) => k === varName)?.[1] ?? '#333';
	}
	async function handleThemeSelect(themeId: string) { await themeStore.setTheme(themeId); flashSave(); }
	async function handleExportTheme() {
		if (!themeStore.activeTheme) return;
		const encoded = await themeStore.exportTheme(themeStore.activeTheme.id);
		if (encoded) { exportString = encoded; showExport = true; }
	}
	async function handleImportTheme() {
		if (!importString.trim()) return;
		await themeStore.importTheme(importString.trim());
		importString = ''; showImport = false; flashSave();
	}
	async function handleCreateCustomTheme() {
		if (!customName.trim()) return;
		const theme: NexusTheme = { id: `custom-${Date.now()}`, name: customName.trim(), author: 'User', version: '1.0.0', variables: Object.entries(customColors), is_builtin: false };
		await themeStore.saveCustomTheme(theme); showCustomCreate = false; customName = ''; flashSave();
	}
	async function handleDeleteTheme(themeId: string) { await themeStore.deleteTheme(themeId); flashSave(); }
	function copyToClipboard(text: string) { navigator.clipboard.writeText(text); }

	// ── WCAG Contrast Checker ──────────────────────────
	interface ContrastCheck {
		pair: string;
		foreground: string;
		background: string;
		ratio: number;
		aa_normal: boolean;
		aa_large: boolean;
		aaa_normal: boolean;
	}
	interface ContrastFix {
		pair: string;
		original_fg: string;
		suggested_fg: string;
		original_ratio: number;
		fixed_ratio: number;
		target: string;
	}

	let contrastChecks = $state<ContrastCheck[]>([]);
	let contrastFixes = $state<ContrastFix[]>([]);
	let contrastLoading = $state(false);
	let showContrastPanel = $state(false);

	async function runContrastCheck() {
		if (!themeStore.activeTheme) return;
		contrastLoading = true;
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const [checks, fixes] = await Promise.all([
				invoke<ContrastCheck[]>('theme_validate_contrast', { themeId: themeStore.activeTheme.id }),
				invoke<ContrastFix[]>('theme_suggest_fixes', { themeId: themeStore.activeTheme.id }),
			]);
			contrastChecks = checks;
			contrastFixes = fixes;
			showContrastPanel = true;
		} catch (e) {
			console.error('Contrast check failed:', e);
		} finally {
			contrastLoading = false;
		}
	}

	let contrastPassCount = $derived(contrastChecks.filter(c => c.aa_normal).length);
	let contrastTotalCount = $derived(contrastChecks.length);
	let contrastAllPass = $derived(contrastPassCount === contrastTotalCount && contrastTotalCount > 0);

	// ── License Activation ──────────────────────────
	let licenseKey = $state('');
	let licenseActivating = $state(false);
	let licenseError = $state<string | null>(null);
	let licenseSuccess = $state(false);

	async function handleActivateLicense() {
		if (!licenseKey.trim()) return;
		licenseActivating = true;
		licenseError = null;
		licenseSuccess = false;
		const ok = await license.activate(licenseKey.trim());
		licenseActivating = false;
		if (ok) {
			licenseSuccess = true;
			licenseKey = '';
			setTimeout(() => { licenseSuccess = false; }, 3000);
		} else {
			licenseError = license.error ?? 'Activation failed';
		}
	}

	let tierBadgeClass = $derived(
		license.isEnterprise ? 'bg-amber-500/10 text-amber-400 border-amber-500/30' :
		license.isPro ? 'bg-purple-500/10 text-purple-400 border-purple-500/30' :
		'bg-gx-bg-elevated text-gx-text-muted border-gx-border-default'
	);
	let tierLabel = $derived(
		license.isEnterprise ? 'Enterprise' : license.isPro ? 'Pro' : 'Community'
	);
</script>

<div class="h-full overflow-y-auto" style={containerStyle}>
	<div class="max-w-3xl mx-auto p-6 space-y-6 pb-16">
		<!-- Header -->
		<div class="flex items-center justify-between" style={seHeaderStyle}>
			<div class="flex items-center gap-3">
				<div class="{hasEngineStyle && seHeaderComponent ? '' : 'bg-gx-bg-elevated'} flex items-center justify-center w-10 h-10 rounded-gx-lg">
					<Settings size={22} class="text-gx-neon" />
				</div>
				<div>
					<h1 class="text-xl font-bold text-gx-text-primary">Settings</h1>
					<p class="text-sm text-gx-text-muted">Configure your ImpForge workstation</p>
				</div>
			</div>
			{#if saveFlash}
				<div class="flex items-center gap-1.5 text-xs text-gx-status-success animate-pulse">
					<Check size={14} /><span>Saved</span>
				</div>
			{/if}
		</div>

		{#if !loaded}
			<div class="space-y-6">
				{#each Array(4) as _}
					<div class="h-40 rounded-gx-lg bg-gx-bg-secondary animate-pulse border border-gx-border-default"></div>
				{/each}
			</div>
		{:else}
			<!-- Section 1: API Keys -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-api-keys">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Key size={16} class="text-gx-neon" />
					<h2 id="settings-api-keys" class="text-sm font-semibold text-gx-text-primary">API Keys</h2>
				</div>
				<div class="p-5 space-y-4">
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<label for="api-key" class="text-sm font-medium text-gx-text-secondary">OpenRouter API Key</label>
							{#if apiKeySet}
								<Badge class="bg-gx-neon/10 text-gx-neon border-gx-neon/30 text-[10px]"><Check size={10} class="mr-1" /> Configured</Badge>
							{:else}
								<Badge variant="outline" class="border-gx-status-warning/50 text-gx-status-warning text-[10px]"><AlertCircle size={10} class="mr-1" /> Not set</Badge>
							{/if}
						</div>
						<div class="relative">
							<input id="api-key" type={showApiKey ? 'text' : 'password'} value={settings.openrouterKey} oninput={handleApiKeyChange} placeholder="sk-or-v1-..." class="w-full px-3 py-2 pr-10 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono" />
							<button onclick={() => showApiKey = !showApiKey} class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gx-text-muted hover:text-gx-text-secondary transition-colors rounded" title={showApiKey ? 'Hide key' : 'Show key'}>
								{#if showApiKey}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
							</button>
						</div>
						<div class="flex items-start gap-2 text-xs text-gx-text-muted">
							<AlertCircle size={12} class="mt-0.5 shrink-0" />
							<p>Free models work without an API key. Add a key for higher rate limits and premium models.</p>
						</div>
						<a href="https://openrouter.ai/keys" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1.5 text-xs text-gx-neon hover:text-gx-neon-bright transition-colors">
							Get an API key at openrouter.ai <ExternalLink size={11} />
						</a>
					</div>
				</div>
			</section>

			<!-- Section: License -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-license">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Crown size={16} class="text-gx-neon" />
					<h2 id="settings-license" class="text-sm font-semibold text-gx-text-primary">License</h2>
					<div class="flex-1"></div>
					<Badge class="{tierBadgeClass} text-[10px]">
						{#if license.isEnterprise}
							<Sparkles size={10} class="mr-1" />
						{:else if license.isPro}
							<Crown size={10} class="mr-1" />
						{/if}
						{tierLabel}
					</Badge>
				</div>
				<div class="p-5 space-y-4">
					{#if license.isPro && license.info}
						<div class="space-y-3">
							<div class="flex items-center gap-3 p-3 rounded-gx bg-gx-neon/5 border border-gx-neon/20">
								<div class="flex items-center justify-center w-8 h-8 rounded-full bg-gx-neon/10">
									<Check size={16} class="text-gx-neon" />
								</div>
								<div class="flex-1 min-w-0">
									<p class="text-sm font-medium text-gx-text-primary">ImpForge {tierLabel} Active</p>
									<p class="text-xs text-gx-text-muted truncate">{license.info.email}</p>
								</div>
							</div>
							<div class="grid grid-cols-2 gap-3 text-xs">
								<div class="p-2.5 rounded-gx bg-gx-bg-tertiary">
									<span class="text-gx-text-muted">Devices</span>
									<p class="font-medium text-gx-text-primary mt-0.5">{license.info.devices} / {license.info.devices}</p>
								</div>
								<div class="p-2.5 rounded-gx bg-gx-bg-tertiary">
									<span class="text-gx-text-muted">Expires</span>
									<p class="font-medium text-gx-text-primary mt-0.5">{new Date(license.info.expires).toLocaleDateString()}</p>
								</div>
							</div>
						</div>
					{:else}
						<div class="space-y-3">
							<p class="text-sm text-gx-text-secondary">
								Enter your license key to unlock Pro features like Neural Trust, Brain v2.0, and the Cascade Router.
							</p>
							<div class="space-y-2">
								<div class="flex gap-2">
									<input
										type="text"
										bind:value={licenseKey}
										placeholder="IMPF-XXXX-XXXX-XXXX-XXXX"
										class="flex-1 px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono uppercase tracking-wider"
									/>
									<button
										onclick={handleActivateLicense}
										disabled={licenseActivating || !licenseKey.trim()}
										class="flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-gx transition-all disabled:opacity-50
											bg-gx-neon/20 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/30"
									>
										{#if licenseActivating}
											<Loader2 size={12} class="animate-spin" /> Activating...
										{:else}
											<Key size={12} /> Activate
										{/if}
									</button>
								</div>
								{#if licenseError}
									<div class="flex items-center gap-1.5 text-xs text-gx-status-error">
										<AlertCircle size={12} />
										<span>{licenseError}</span>
									</div>
								{/if}
								{#if licenseSuccess}
									<div class="flex items-center gap-1.5 text-xs text-gx-status-success animate-pulse">
										<Check size={12} />
										<span>License activated successfully!</span>
									</div>
								{/if}
							</div>
							<div class="p-3 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
								<p class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider mb-2">Pro Features Include</p>
								<ul class="space-y-1.5 text-xs text-gx-text-secondary">
									<li class="flex items-center gap-2"><Crown size={11} class="text-purple-400 shrink-0" /> Three-Factor Hebbian Neural Trust</li>
									<li class="flex items-center gap-2"><Sparkles size={11} class="text-purple-400 shrink-0" /> Brain v2.0 (FSRS-5 + CLS Replay)</li>
									<li class="flex items-center gap-2"><Globe size={11} class="text-purple-400 shrink-0" /> 5-Tier Cascade Inference Router</li>
									<li class="flex items-center gap-2"><Shield size={11} class="text-purple-400 shrink-0" /> MAPE-K Self-Healing Health Loop</li>
								</ul>
							</div>
						</div>
					{/if}
				</div>
			</section>

			<!-- Section 2: AI Model Preferences -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-ai-models">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Globe size={16} class="text-gx-neon" />
					<h2 id="settings-ai-models" class="text-sm font-semibold text-gx-text-primary">AI Model Preferences</h2>
				</div>
				<div class="p-5 space-y-5">
					<div class="space-y-2">
						<label for="provider" class="text-sm font-medium text-gx-text-secondary">Default Provider</label>
						<select id="provider" value={getCurrentProvider()} onchange={handleProviderChange} class="w-full px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-neon focus:outline-none appearance-none cursor-pointer">
							<option value="auto">Auto (Intelligent Router)</option>
							<option value="openrouter">OpenRouter</option>
							<option value="ollama">Ollama</option>
						</select>
						<p class="text-xs text-gx-text-muted">Auto mode picks the best model for each task, prioritizing free and local options.</p>
					</div>
					<div class="space-y-2">
						<label for="ollama-url" class="text-sm font-medium text-gx-text-secondary">Ollama URL</label>
						<div class="flex gap-2">
							<input id="ollama-url" type="text" value={settings.ollamaUrl} oninput={handleOllamaUrlChange} placeholder="http://localhost:11434" class="flex-1 px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono" />
							<button onclick={testOllamaConnection} disabled={ollamaStatus === 'checking'} class="flex items-center gap-1.5 px-3 py-2 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all disabled:opacity-50">
								<RefreshCw size={12} class={ollamaStatus === 'checking' ? 'animate-spin' : ''} /> Test
							</button>
						</div>
						{#if ollamaStatus !== 'idle'}
							<div class="flex items-center gap-1.5 text-xs">
								<span class={statusDot(ollamaStatus)}>&#9679;</span>
								<span class={statusDot(ollamaStatus)}>{statusLabel(ollamaStatus)}</span>
							</div>
						{/if}
					</div>
				</div>
			</section>

			<!-- Section: Chat Layout (3x3x3 Modular System) -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-chat-layout">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Layout size={16} class="text-gx-neon" />
					<h2 id="settings-chat-layout" class="text-sm font-semibold text-gx-text-primary">Chat Layout</h2>
					<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-accent-cyan/30 text-gx-accent-cyan">3×3×3</Badge>
				</div>
				<div class="p-5 space-y-5">
					<!-- Chat Placement -->
					<div>
						<label class="block text-xs font-medium text-gx-text-secondary mb-2">Placement Mode</label>
						<div class="grid grid-cols-3 gap-2">
							{#each [
								{ value: 'side-panel' as const, label: 'Side Panel', desc: 'Ctrl+J toggle' },
								{ value: 'dedicated' as const, label: 'Dedicated Page', desc: 'Full page at /chat' },
								{ value: 'convergence' as const, label: 'Convergence', desc: 'IDE+Chat+Terminal' },
							] as opt}
								<button
									onclick={() => saveSetting('chatPlacement', opt.value)}
									class="flex flex-col items-center gap-1 p-3 rounded-gx border transition-all text-center
										{settings.chatPlacement === opt.value
											? 'border-gx-neon bg-gx-neon/10 text-gx-neon'
											: 'border-gx-border-default hover:border-gx-text-muted text-gx-text-muted'}"
								>
									<span class="text-xs font-medium">{opt.label}</span>
									<span class="text-[10px] opacity-70">{opt.desc}</span>
								</button>
							{/each}
						</div>
					</div>

					<!-- Stream Rendering Mode -->
					<div>
						<label class="block text-xs font-medium text-gx-text-secondary mb-2">Stream Mode</label>
						<div class="grid grid-cols-3 gap-2">
							{#each [
								{ value: 'split' as const, label: 'Split Panel', desc: 'User / AI sides' },
								{ value: 'unified' as const, label: 'Unified Stream', desc: 'Single flow' },
								{ value: 'mission-control' as const, label: 'Mission Control', desc: 'Multi-panel' },
							] as opt}
								<button
									onclick={() => saveSetting('chatStreamMode', opt.value)}
									class="flex flex-col items-center gap-1 p-3 rounded-gx border transition-all text-center
										{settings.chatStreamMode === opt.value
											? 'border-gx-accent-cyan bg-gx-accent-cyan/10 text-gx-accent-cyan'
											: 'border-gx-border-default hover:border-gx-text-muted text-gx-text-muted'}"
								>
									<span class="text-xs font-medium">{opt.label}</span>
									<span class="text-[10px] opacity-70">{opt.desc}</span>
								</button>
							{/each}
						</div>
					</div>

					<!-- Visualization Level -->
					<div>
						<label class="block text-xs font-medium text-gx-text-secondary mb-2">Model Visualization</label>
						<div class="grid grid-cols-3 gap-2">
							{#each [
								{ value: 'minimal' as const, label: 'Minimal', desc: 'Status badges' },
								{ value: 'cards' as const, label: 'Activity Cards', desc: 'Progress + tokens' },
								{ value: 'pipeline' as const, label: 'Full Pipeline', desc: 'Animated DAG' },
							] as opt}
								<button
									onclick={() => saveSetting('chatVizLevel', opt.value)}
									class="flex flex-col items-center gap-1 p-3 rounded-gx border transition-all text-center
										{settings.chatVizLevel === opt.value
											? 'border-gx-accent-purple bg-gx-accent-purple/10 text-gx-accent-purple'
											: 'border-gx-border-default hover:border-gx-text-muted text-gx-text-muted'}"
								>
									<span class="text-xs font-medium">{opt.label}</span>
									<span class="text-[10px] opacity-70">{opt.desc}</span>
								</button>
							{/each}
						</div>
					</div>

					<!-- Toggle Switches -->
					<div class="grid grid-cols-2 gap-3">
						{#each [
							{ key: 'chatShowThinking' as const, label: 'Show Thinking Blocks' },
							{ key: 'chatShowRouting' as const, label: 'Show Routing Decisions' },
							{ key: 'chatAnimations' as const, label: 'Enable Animations' },
							{ key: 'chatCompactMode' as const, label: 'Compact Mode' },
						] as toggle}
							<label class="flex items-center gap-2 text-xs text-gx-text-muted cursor-pointer hover:text-gx-text-secondary transition-colors">
								<input
									type="checkbox"
									checked={settings[toggle.key]}
									onchange={() => saveSetting(toggle.key, !settings[toggle.key])}
									class="w-3.5 h-3.5 rounded accent-gx-neon"
								/>
								{toggle.label}
							</label>
						{/each}
					</div>
				</div>
			</section>

			<!-- Section 3: Theme Engine (ElvUI-Style) -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-theme-engine">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Palette size={16} class="text-gx-neon" />
					<h2 id="settings-theme-engine" class="text-sm font-semibold text-gx-text-primary">Theme Engine</h2>
					<div class="flex-1"></div>
					<div class="flex items-center gap-1.5">
						<button onclick={handleExportTheme} class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-text-muted hover:text-gx-accent-cyan border border-gx-border-default hover:border-gx-accent-cyan/30 transition-all">
							<Download size={10} /> Export
						</button>
						<button onclick={() => showImport = !showImport} class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-text-muted hover:text-gx-accent-purple border border-gx-border-default hover:border-gx-accent-purple/30 transition-all">
							<Upload size={10} /> Import
						</button>
						<button onclick={() => showCustomCreate = !showCustomCreate} class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx text-gx-neon hover:bg-gx-neon/10 border border-gx-neon/30 transition-all">
							<Plus size={10} /> Create
						</button>
					</div>
				</div>
				<div class="p-5 space-y-5">
					<div class="space-y-2">
						<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Installed Themes</span>
						<div class="grid grid-cols-1 gap-2">
							{#each themeStore.themes as theme (theme.id)}
								<div
									role="button"
									tabindex="0"
									onclick={() => handleThemeSelect(theme.id)}
									onkeydown={(e) => e.key === 'Enter' && handleThemeSelect(theme.id)}
									class="relative flex items-center gap-3 px-4 py-3 rounded-gx border transition-all text-left cursor-pointer
										{themeStore.activeTheme?.id === theme.id ? 'border-gx-neon bg-gx-neon/5 shadow-[0_0_12px_rgba(0,255,102,0.15)]' : 'border-gx-border-default bg-gx-bg-tertiary hover:border-gx-text-muted'}"
								>
									<div class="flex gap-1 shrink-0">
										{#each themePreviewVars as varName}
											<div class="w-5 h-5 rounded-full border border-white/10" style="background-color: {getThemeColor(theme, varName)}"></div>
										{/each}
									</div>
									<div class="min-w-0 flex-1">
										<div class="flex items-center gap-2">
											<span class="text-sm font-medium text-gx-text-primary">{theme.name}</span>
											{#if theme.is_builtin}
												<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted">Built-in</Badge>
											{:else}
												<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 border-gx-accent-purple/30 text-gx-accent-purple">Custom</Badge>
											{/if}
										</div>
										{#if theme.author}
											<span class="text-[10px] text-gx-text-muted">by {theme.author}</span>
										{/if}
									</div>
									{#if themeStore.activeTheme?.id === theme.id}
										<div class="shrink-0 w-5 h-5 rounded-full bg-gx-neon/20 flex items-center justify-center">
											<Check size={12} class="text-gx-neon" />
										</div>
									{/if}
									{#if !theme.is_builtin}
										<div
											role="button"
											tabindex="0"
											onclick={(e: MouseEvent) => { e.stopPropagation(); handleDeleteTheme(theme.id); }}
											onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') { e.stopPropagation(); handleDeleteTheme(theme.id); } }}
											class="shrink-0 p-1 text-gx-text-muted hover:text-gx-status-error transition-colors rounded cursor-pointer"
										>
											<Trash2 size={12} />
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>

					{#if showImport}
						<div class="space-y-2 p-3 rounded-gx border border-gx-accent-purple/30 bg-gx-accent-purple/5">
							<span class="text-[10px] text-gx-accent-purple font-semibold uppercase tracking-wider">Import Theme Profile</span>
							<textarea bind:value={importString} placeholder="Paste theme profile string here..." rows={3} class="w-full px-3 py-2 text-xs font-mono bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-accent-purple focus:outline-none resize-none"></textarea>
							<div class="flex gap-2">
								<button onclick={handleImportTheme} disabled={!importString.trim()} class="flex-1 flex items-center justify-center gap-1 px-3 py-1.5 text-xs rounded-gx bg-gx-accent-purple/20 text-gx-accent-purple hover:bg-gx-accent-purple/30 border border-gx-accent-purple/30 disabled:opacity-40 transition-all">
									<Upload size={12} /> Import
								</button>
								<button onclick={() => { showImport = false; importString = ''; }} class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary border border-gx-border-default transition-all">Cancel</button>
							</div>
						</div>
					{/if}

					{#if showExport && exportString}
						<div class="space-y-2 p-3 rounded-gx border border-gx-accent-cyan/30 bg-gx-accent-cyan/5">
							<div class="flex items-center justify-between">
								<span class="text-[10px] text-gx-accent-cyan font-semibold uppercase tracking-wider">Theme Profile String</span>
								<button onclick={() => copyToClipboard(exportString)} class="flex items-center gap-1 text-[10px] text-gx-accent-cyan hover:text-gx-accent-cyan/80 transition-colors">
									<Copy size={10} /> Copy
								</button>
							</div>
							<div class="px-3 py-2 text-[10px] font-mono bg-gx-bg-primary rounded-gx border border-gx-border-default text-gx-text-muted break-all max-h-24 overflow-auto">{exportString}</div>
							<p class="text-[9px] text-gx-text-muted">Share this string to let others import your theme — like ElvUI profile strings.</p>
							<button onclick={() => { showExport = false; exportString = ''; }} class="text-[10px] text-gx-text-muted hover:text-gx-text-secondary transition-colors">Close</button>
						</div>
					{/if}

					{#if showCustomCreate}
						<div class="space-y-3 p-3 rounded-gx border border-gx-neon/30 bg-gx-neon/5">
							<span class="text-[10px] text-gx-neon font-semibold uppercase tracking-wider">Create Custom Theme</span>
							<input type="text" bind:value={customName} placeholder="Theme name..." class="w-full px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none" />
							<div class="grid grid-cols-3 gap-2">
								{#each Object.entries(customColors) as [varName, color]}
									<div class="space-y-1">
										<span class="text-[9px] text-gx-text-muted">{varName.replace('--color-gx-', '')}</span>
										<div class="flex items-center gap-1.5">
											<input type="color" bind:value={customColors[varName]} class="w-7 h-7 rounded border-0 cursor-pointer bg-transparent" />
											<span class="text-[9px] font-mono text-gx-text-muted">{color}</span>
										</div>
									</div>
								{/each}
							</div>
							<div class="flex gap-2">
								<button onclick={handleCreateCustomTheme} disabled={!customName.trim()} class="flex-1 flex items-center justify-center gap-1 px-3 py-1.5 text-xs rounded-gx bg-gx-neon/20 text-gx-neon hover:bg-gx-neon/30 border border-gx-neon/30 disabled:opacity-40 transition-all">
									<Paintbrush size={12} /> Create Theme
								</button>
								<button onclick={() => showCustomCreate = false} class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary border border-gx-border-default transition-all">Cancel</button>
							</div>
						</div>
					{/if}

					<!-- WCAG 2.1 AA Contrast Checker -->
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">Accessibility (WCAG 2.1 AA)</span>
							<button
								onclick={runContrastCheck}
								disabled={contrastLoading || !themeStore.activeTheme}
								aria-label="Run WCAG contrast check on current theme"
								class="flex items-center gap-1 px-2 py-1 text-[10px] rounded-gx border transition-all disabled:opacity-40
									{contrastAllPass
										? 'text-gx-status-success border-gx-status-success/30 hover:bg-gx-status-success/10'
										: contrastChecks.length > 0
											? 'text-gx-status-warning border-gx-status-warning/30 hover:bg-gx-status-warning/10'
											: 'text-gx-text-muted border-gx-border-default hover:border-gx-neon hover:text-gx-neon'}"
							>
								{#if contrastLoading}
									<RefreshCw size={10} class="animate-spin" /> Checking...
								{:else if contrastAllPass}
									<ShieldCheck size={10} /> AA Pass ({contrastPassCount}/{contrastTotalCount})
								{:else if contrastChecks.length > 0}
									<ShieldAlert size={10} /> {contrastPassCount}/{contrastTotalCount} Pass
								{:else}
									<Shield size={10} /> Check Contrast
								{/if}
							</button>
						</div>

						{#if showContrastPanel && contrastChecks.length > 0}
							<div class="space-y-2 p-3 rounded-gx border bg-gx-bg-primary
								{contrastAllPass ? 'border-gx-status-success/30' : 'border-gx-status-warning/30'}">
								<div class="space-y-1.5">
									{#each contrastChecks as check (check.pair)}
										<div class="flex items-center gap-2 text-[11px]">
											<span class={check.aa_normal ? 'text-gx-status-success' : 'text-gx-status-error'}>
												{check.aa_normal ? '✓' : '✗'}
											</span>
											<span class="text-gx-text-muted w-36 truncate" title={check.pair}>
												{check.pair}
											</span>
											<div class="flex items-center gap-1">
												<span class="w-3 h-3 rounded-full border border-white/10 shrink-0" style="background-color: {check.foreground}"></span>
												<span class="text-gx-text-muted">on</span>
												<span class="w-3 h-3 rounded-full border border-white/10 shrink-0" style="background-color: {check.background}"></span>
											</div>
											<span class="ml-auto font-mono {check.aa_normal ? 'text-gx-status-success' : 'text-gx-status-error'}">
												{check.ratio.toFixed(1)}:1
											</span>
											<span class="text-[9px] text-gx-text-muted">
												{check.aa_normal ? 'AA' : ''}{check.aaa_normal ? '+AAA' : ''}
											</span>
										</div>
									{/each}
								</div>

								{#if contrastFixes.length > 0}
									<div class="mt-2 pt-2 border-t border-gx-border-default space-y-1.5">
										<div class="flex items-center gap-1 text-[10px] text-gx-accent-orange font-semibold uppercase tracking-wider">
											<Wrench size={10} />
											<span>Suggested Fixes</span>
										</div>
										{#each contrastFixes as fix (fix.pair)}
											<div class="flex items-center gap-2 text-[11px]">
												<span class="text-gx-text-muted w-36 truncate">{fix.pair}</span>
												<div class="flex items-center gap-1">
													<span class="w-3 h-3 rounded-full border border-white/10" style="background-color: {fix.original_fg}"></span>
													<span class="text-gx-text-muted">→</span>
													<span class="w-3 h-3 rounded-full border border-gx-neon/50" style="background-color: {fix.suggested_fg}"></span>
												</div>
												<span class="font-mono text-gx-status-success ml-auto">{fix.fixed_ratio.toFixed(1)}:1</span>
												<span class="font-mono text-[9px] text-gx-text-muted">{fix.suggested_fg}</span>
											</div>
										{/each}
									</div>
								{/if}

								{#if contrastAllPass}
									<p class="text-[10px] text-gx-status-success mt-1">All color pairs pass WCAG 2.1 AA contrast requirements.</p>
								{:else}
									<p class="text-[10px] text-gx-text-muted mt-1">Minimum 4.5:1 for normal text, 3:1 for large text (18pt+).</p>
								{/if}
							</div>
						{/if}
					</div>

					<!-- Font Size -->
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<label for="font-size" class="text-sm font-medium text-gx-text-secondary">Font Size</label>
							<span class="text-xs font-mono text-gx-neon">{fontSize}px</span>
						</div>
						<input id="font-size" type="range" min="12" max="20" step="1" bind:value={fontSize}
							class="w-full h-1.5 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-gx-neon [&::-webkit-slider-thumb]:shadow-gx-glow-sm [&::-webkit-slider-thumb]:cursor-pointer bg-gx-bg-elevated border-0"
							style="background: linear-gradient(to right, var(--color-gx-neon) 0%, var(--color-gx-neon) {((fontSize - 12) / 8) * 100}%, var(--color-gx-bg-elevated) {((fontSize - 12) / 8) * 100}%, var(--color-gx-bg-elevated) 100%);"
						/>
						<div class="flex justify-between text-[10px] text-gx-text-muted">
							<span>12px</span><span>16px</span><span>20px</span>
						</div>
					</div>
				</div>
			</section>

			<!-- Section 4: Services -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden" aria-labelledby="settings-services">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Server size={16} class="text-gx-neon" />
					<h2 id="settings-services" class="text-sm font-semibold text-gx-text-primary">Services</h2>
				</div>
				<div class="divide-y divide-gx-border-default">
					{#each services as service (service.key)}
						<div class="px-5 py-4 space-y-2">
							<div class="flex items-center justify-between">
								<div>
									<span class="text-sm font-medium text-gx-text-primary">{service.name}</span>
									<span class="text-xs text-gx-text-muted ml-2">{service.description}</span>
								</div>
								{#if serviceStatuses[service.key] !== 'idle'}
									<div class="flex items-center gap-1.5 text-xs">
										<span class={statusDot(serviceStatuses[service.key])}>&#9679;</span>
										<span class={statusDot(serviceStatuses[service.key])}>{statusLabel(serviceStatuses[service.key])}</span>
									</div>
								{/if}
							</div>
							<div class="flex gap-2">
								<input type="text" bind:value={serviceUrls[service.key]} placeholder={service.defaultUrl} class="flex-1 px-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono" />
								<button onclick={() => testServiceConnection(service.key)} disabled={serviceStatuses[service.key] === 'checking'} class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all disabled:opacity-50">
									<RefreshCw size={11} class={serviceStatuses[service.key] === 'checking' ? 'animate-spin' : ''} /> Check
								</button>
								<a href={serviceUrls[service.key]} target="_blank" rel="noopener noreferrer" class="flex items-center px-2 py-1.5 text-gx-text-muted hover:text-gx-neon transition-colors rounded-gx border border-gx-border-default hover:border-gx-neon bg-gx-bg-elevated" title="Open in browser">
									<ExternalLink size={12} />
								</a>
							</div>
						</div>
					{/each}
				</div>
			</section>
		{/if}

		<!-- ForgeWatch — Filesystem Monitoring -->
		<section class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-2 mb-4">
				<FolderSearch size={18} class="text-gx-neon" />
				<h2 class="text-base font-semibold text-gx-text-primary">ForgeWatch</h2>
				<span class="text-xs text-gx-text-muted">Filesystem Monitoring & Indexing</span>
				{#if forgeWatchStore.status}
					<Badge variant={forgeWatchStore.status.running ? 'default' : 'secondary'} class="ml-auto text-[10px]">
						{forgeWatchStore.status.running ? 'Active' : 'Inactive'}
					</Badge>
				{/if}
			</div>

			<!-- Status -->
			{#if forgeWatchStore.status}
				<div class="grid grid-cols-3 gap-3 mb-4">
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-lg font-bold text-gx-text-primary">{forgeWatchStore.status.watched_paths}</div>
						<div class="text-[10px] text-gx-text-muted">Watched Paths</div>
					</div>
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-lg font-bold text-gx-text-primary">{forgeWatchStore.status.total_files_indexed}</div>
						<div class="text-[10px] text-gx-text-muted">Files Indexed</div>
					</div>
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-lg font-bold text-gx-text-primary">{forgeWatchStore.status.events_processed}</div>
						<div class="text-[10px] text-gx-text-muted">Events Processed</div>
					</div>
				</div>
			{/if}

			<!-- Add Watch Path -->
			<div class="flex gap-2 mb-3">
				<input
					type="text"
					bind:value={newWatchPath}
					placeholder="/home/user/projects"
					class="flex-1 px-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono"
				/>
				<button
					onclick={async () => { if (newWatchPath) { await forgeWatchStore.addPath(newWatchPath); newWatchPath = ''; } }}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all"
				>
					<FolderPlus size={12} /> Add
				</button>
				<button
					onclick={() => forgeWatchStore.discover('/home')}
					disabled={forgeWatchStore.isDiscovering}
					class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all disabled:opacity-50"
				>
					{#if forgeWatchStore.isDiscovering}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<Search size={12} />
					{/if}
					Auto-Discover
				</button>
			</div>

			<!-- Watched Paths List -->
			{#if forgeWatchStore.watchedPaths.length > 0}
				<div class="space-y-1.5 mb-3">
					{#each forgeWatchStore.watchedPaths as wp}
						<div class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
							<HardDrive size={12} class="text-gx-text-muted shrink-0" />
							<span class="flex-1 text-xs font-mono text-gx-text-primary truncate">{wp.path}</span>
							{#if wp.label}
								<Badge variant="outline" class="text-[10px]">{wp.label}</Badge>
							{/if}
							<Badge variant="secondary" class="text-[10px]">{wp.scan_mode}</Badge>
							<button
								onclick={() => forgeWatchStore.reindex(wp.path)}
								disabled={forgeWatchStore.isReindexing}
								class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors"
								title="Reindex"
							>
								<RefreshCw size={11} class={forgeWatchStore.isReindexing ? 'animate-spin' : ''} />
							</button>
							<button
								onclick={() => forgeWatchStore.removePath(wp.path)}
								class="p-1 text-gx-text-muted hover:text-red-400 transition-colors"
								title="Remove"
							>
								<FolderMinus size={11} />
							</button>
						</div>
					{/each}
				</div>
			{:else}
				<p class="text-xs text-gx-text-muted mb-3">No watched paths configured. Add a path or use Auto-Discover.</p>
			{/if}

			<!-- Discovered Paths -->
			{#if forgeWatchStore.discoveredPaths.length > 0}
				<div class="mt-3">
					<h3 class="text-xs font-semibold text-gx-text-secondary mb-2">Discovered Projects</h3>
					<div class="space-y-1.5 max-h-48 overflow-y-auto">
						{#each forgeWatchStore.discoveredPaths as dp}
							<div class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-bg-tertiary/50 border border-gx-border-default/50">
								<span class="flex-1 text-xs font-mono text-gx-text-secondary truncate">{dp.path}</span>
								<span class="text-[10px] text-gx-text-muted">{dp.reason}</span>
								{#if dp.project_type}
									<Badge variant="outline" class="text-[10px]">{dp.project_type}</Badge>
								{/if}
								<button
									onclick={() => forgeWatchStore.addDiscovered(dp)}
									class="flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium bg-gx-bg-elevated border border-gx-border-default rounded text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all"
								>
									<Plus size={10} /> Watch
								</button>
							</div>
						{/each}
					</div>
				</div>
			{/if}

			<!-- Error Display -->
			{#if forgeWatchStore.error}
				<div class="mt-2 px-3 py-2 rounded-gx bg-red-500/10 border border-red-500/20 text-xs text-red-400">
					{forgeWatchStore.error}
				</div>
			{/if}
		</section>

		<!-- ForgeSunshine — Moonlight Remote Access -->
		<section class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-2 mb-4">
				<Monitor size={18} class="text-gx-accent-cyan" />
				<h2 class="text-base font-semibold text-gx-text-primary">ForgeSunshine</h2>
				<span class="text-xs text-gx-text-muted">Moonlight Remote Access</span>
				{#if sunshineStore.status}
					<Badge variant={sunshineStore.status.running ? 'default' : 'secondary'} class="ml-auto text-[10px]">
						{sunshineStore.status.running ? 'Streaming' : 'Stopped'}
					</Badge>
				{/if}
			</div>

			{#if sunshineStore.info}
				<div class="grid grid-cols-3 gap-3 mb-4">
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-sm font-bold text-gx-text-primary">
							{#if sunshineStore.info.installed}
								<span class="text-gx-status-success">Installed</span>
							{:else}
								<span class="text-gx-status-warning">Not Found</span>
							{/if}
						</div>
						<div class="text-[10px] text-gx-text-muted">Status</div>
					</div>
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-sm font-bold text-gx-text-primary">{sunshineStore.info.version ?? '—'}</div>
						<div class="text-[10px] text-gx-text-muted">Version</div>
					</div>
					<div class="rounded-gx bg-gx-bg-tertiary p-3 text-center">
						<div class="text-sm font-bold text-gx-text-primary truncate">{sunshineStore.info.platform}</div>
						<div class="text-[10px] text-gx-text-muted">Platform</div>
					</div>
				</div>
			{/if}

			{#if sunshineStore.info && !sunshineStore.info.installed}
				<div class="p-3 mb-4 rounded-gx bg-gx-accent-cyan/5 border border-gx-accent-cyan/20">
					<p class="text-xs text-gx-text-secondary mb-2">Sunshine is not installed. Install it to enable Moonlight streaming.</p>
					{#if sunshineStore.installCommand}
						<div class="flex items-center gap-2">
							<code class="flex-1 px-2 py-1 text-[11px] font-mono bg-gx-bg-primary rounded border border-gx-border-default text-gx-text-primary">{sunshineStore.installCommand}</code>
							<button onclick={() => sunshineStore.installCommand && navigator.clipboard.writeText(sunshineStore.installCommand)} class="p-1 text-gx-text-muted hover:text-gx-accent-cyan transition-colors" title="Copy command">
								<Copy size={12} />
							</button>
						</div>
					{/if}
				</div>
			{/if}

			{#if sunshineStore.info?.installed}
				<div class="flex gap-2 mb-4">
					{#if sunshineStore.status?.running}
						<button onclick={() => sunshineStore.stop()} disabled={sunshineStore.isStopping} class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-red-500/10 border border-red-500/30 rounded-gx text-red-400 hover:bg-red-500/20 transition-all disabled:opacity-50">
							{#if sunshineStore.isStopping}<Loader2 size={12} class="animate-spin" />{:else}<Square size={12} />{/if} Stop
						</button>
					{:else}
						<button onclick={() => sunshineStore.start()} disabled={sunshineStore.isStarting} class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-neon/10 border border-gx-neon/30 rounded-gx text-gx-neon hover:bg-gx-neon/20 transition-all disabled:opacity-50">
							{#if sunshineStore.isStarting}<Loader2 size={12} class="animate-spin" />{:else}<Play size={12} />{/if} Start
						</button>
					{/if}
					<button onclick={() => sunshineStore.refreshStatus()} class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all">
						<RefreshCw size={12} /> Refresh
					</button>
					{#if sunshineStore.info?.web_ui_url}
						<a href={sunshineStore.info.web_ui_url} target="_blank" rel="noopener noreferrer" class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-accent-cyan hover:text-gx-accent-cyan transition-all">
							<ExternalLink size={12} /> Web UI
						</a>
					{/if}
				</div>

				{#if sunshineStore.config}
					<div class="space-y-3">
						<h3 class="text-xs font-semibold text-gx-text-secondary">Streaming Configuration</h3>
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-1">
								<label class="text-[10px] text-gx-text-muted">Resolution</label>
								<select value="{sunshineStore.config.resolution_width}x{sunshineStore.config.resolution_height}" onchange={(e) => { if (!sunshineStore.config) return; const [w, h] = (e.target as HTMLSelectElement).value.split('x').map(Number); sunshineStore.saveConfig({ ...sunshineStore.config, resolution_width: w, resolution_height: h }); }} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-neon focus:outline-none">
									<option value="1280x720">1280x720 (720p)</option>
									<option value="1920x1080">1920x1080 (1080p)</option>
									<option value="2560x1440">2560x1440 (1440p)</option>
									<option value="3840x2160">3840x2160 (4K)</option>
								</select>
							</div>
							<div class="space-y-1">
								<label class="text-[10px] text-gx-text-muted">FPS</label>
								<select value={sunshineStore.config.fps} onchange={(e) => { if (!sunshineStore.config) return; sunshineStore.saveConfig({ ...sunshineStore.config, fps: Number((e.target as HTMLSelectElement).value) }); }} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-neon focus:outline-none">
									<option value="30">30 FPS</option>
									<option value="60">60 FPS</option>
									<option value="120">120 FPS</option>
								</select>
							</div>
							<div class="space-y-1">
								<label class="text-[10px] text-gx-text-muted">Encoder</label>
								<select value={sunshineStore.config.encoder} onchange={(e) => { if (!sunshineStore.config) return; sunshineStore.saveConfig({ ...sunshineStore.config, encoder: (e.target as HTMLSelectElement).value as any }); }} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-neon focus:outline-none">
									<option value="Auto">Auto</option>
									<option value="Nvenc">NVENC (NVIDIA)</option>
									<option value="Vaapi">VAAPI (AMD/Intel)</option>
									<option value="Qsv">QuickSync (Intel)</option>
									<option value="Software">Software (x264)</option>
								</select>
							</div>
							<div class="space-y-1">
								<label class="text-[10px] text-gx-text-muted">Audio</label>
								<button onclick={() => { if (!sunshineStore.config) return; sunshineStore.saveConfig({ ...sunshineStore.config, audio_enabled: !sunshineStore.config.audio_enabled }); }} class="w-full px-2 py-1.5 text-xs rounded-gx border transition-all text-left {sunshineStore.config.audio_enabled ? 'bg-gx-neon/10 border-gx-neon/30 text-gx-neon' : 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted'}">
									{sunshineStore.config.audio_enabled ? 'Enabled' : 'Disabled'}
								</button>
							</div>
						</div>
						<div class="flex items-center gap-2 text-[10px] text-gx-text-muted">
							<Wifi size={10} />
							<span>Port: {sunshineStore.config.port} | Web UI: {sunshineStore.config.web_port}</span>
						</div>
					</div>
				{/if}
			{/if}

			{#if sunshineStore.error}
				<div class="mt-2 px-3 py-2 rounded-gx bg-red-500/10 border border-red-500/20 text-xs text-red-400">
					{sunshineStore.error}
				</div>
			{/if}
		</section>

		<!-- Auto-Learn — Universal Input Digest Settings -->
		<section class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-2 mb-4">
				<Brain size={18} class="text-gx-accent-purple" />
				<h2 class="text-base font-semibold text-gx-text-primary">Auto-Learn</h2>
				<span class="text-xs text-gx-text-muted">Universal Input Digest (NLP Pipeline)</span>
				{#if digestStore.totalDigested > 0}
					<Badge variant="default" class="ml-auto text-[10px]">
						{digestStore.totalDigested} learned
					</Badge>
				{/if}
			</div>

			<p class="text-xs text-gx-text-secondary mb-4">
				ForgeMemory automatically extracts knowledge from your workflow — terminal output, file edits, web browsing, and more. 80+ multilingual NLP patterns detect preferences, decisions, errors, and technical facts.
			</p>

			<!-- Source Toggles -->
			<div class="space-y-2 mb-4">
				<h3 class="text-xs font-semibold text-gx-text-secondary">Input Sources</h3>
				<div class="grid grid-cols-2 gap-2">
					<button onclick={() => digestStore.updateConfig('terminal_enabled', !digestStore.config.terminal_enabled)} class="flex items-center gap-2 px-3 py-2 rounded-gx border transition-all text-left text-xs {digestStore.config.terminal_enabled ? 'bg-gx-neon/10 border-gx-neon/30 text-gx-neon' : 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted'}">
						<span class="w-2 h-2 rounded-full {digestStore.config.terminal_enabled ? 'bg-gx-neon' : 'bg-gx-text-muted'}"></span>
						Terminal Output
					</button>
					<button onclick={() => digestStore.updateConfig('editor_enabled', !digestStore.config.editor_enabled)} class="flex items-center gap-2 px-3 py-2 rounded-gx border transition-all text-left text-xs {digestStore.config.editor_enabled ? 'bg-gx-neon/10 border-gx-neon/30 text-gx-neon' : 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted'}">
						<span class="w-2 h-2 rounded-full {digestStore.config.editor_enabled ? 'bg-gx-neon' : 'bg-gx-text-muted'}"></span>
						Editor / File Saves
					</button>
					<button onclick={() => digestStore.updateConfig('clipboard_enabled', !digestStore.config.clipboard_enabled)} class="flex items-center gap-2 px-3 py-2 rounded-gx border transition-all text-left text-xs {digestStore.config.clipboard_enabled ? 'bg-gx-neon/10 border-gx-neon/30 text-gx-neon' : 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted'}">
						<span class="w-2 h-2 rounded-full {digestStore.config.clipboard_enabled ? 'bg-gx-neon' : 'bg-gx-text-muted'}"></span>
						Clipboard
					</button>
					<button onclick={() => digestStore.updateConfig('url_enabled', !digestStore.config.url_enabled)} class="flex items-center gap-2 px-3 py-2 rounded-gx border transition-all text-left text-xs {digestStore.config.url_enabled ? 'bg-gx-neon/10 border-gx-neon/30 text-gx-neon' : 'bg-gx-bg-tertiary border-gx-border-default text-gx-text-muted'}">
						<span class="w-2 h-2 rounded-full {digestStore.config.url_enabled ? 'bg-gx-neon' : 'bg-gx-text-muted'}"></span>
						Web / URLs
					</button>
				</div>
			</div>

			<!-- Sensitivity Slider -->
			<div class="space-y-2 mb-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-1.5">
						<Sliders size={12} class="text-gx-text-muted" />
						<label class="text-xs font-medium text-gx-text-secondary">NLP Sensitivity</label>
					</div>
					<span class="text-xs font-mono text-gx-accent-purple">{(digestStore.config.nlp_threshold * 100).toFixed(0)}%</span>
				</div>
				<input type="range" min="0" max="100" step="5" value={digestStore.config.nlp_threshold * 100} oninput={(e) => digestStore.updateConfig('nlp_threshold', Number((e.target as HTMLInputElement).value) / 100)} class="w-full h-1.5 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-gx-accent-purple [&::-webkit-slider-thumb]:cursor-pointer bg-gx-bg-elevated border-0" style="background: linear-gradient(to right, var(--color-gx-accent-purple) 0%, var(--color-gx-accent-purple) {digestStore.config.nlp_threshold * 100}%, var(--color-gx-bg-elevated) {digestStore.config.nlp_threshold * 100}%, var(--color-gx-bg-elevated) 100%);" />
				<div class="flex justify-between text-[10px] text-gx-text-muted">
					<span>Low (catch more)</span><span>High (only important)</span>
				</div>
			</div>

			<!-- Advanced Settings -->
			<div class="grid grid-cols-2 gap-3 mb-3">
				<div class="space-y-1">
					<label class="text-[10px] text-gx-text-muted">Debounce (ms)</label>
					<input type="number" min="500" max="10000" step="500" value={digestStore.config.debounce_ms} oninput={(e) => digestStore.updateConfig('debounce_ms', Number((e.target as HTMLInputElement).value))} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-accent-purple focus:outline-none font-mono" />
				</div>
				<div class="space-y-1">
					<label class="text-[10px] text-gx-text-muted">Max Lines / Batch</label>
					<input type="number" min="10" max="1000" step="10" value={digestStore.config.max_lines} oninput={(e) => digestStore.updateConfig('max_lines', Number((e.target as HTMLInputElement).value))} class="w-full px-2 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-accent-purple focus:outline-none font-mono" />
				</div>
			</div>

			<!-- Last Result -->
			{#if digestStore.lastResult}
				<div class="p-2.5 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-[10px] text-gx-text-muted">
					<span class="font-medium text-gx-text-secondary">Last digest:</span>
					{digestStore.lastResult.source} — {digestStore.lastResult.lines_processed} lines, {digestStore.lastResult.memories_created} memories, {digestStore.lastResult.entities_extracted} entities
				</div>
			{/if}

			{#if digestStore.error}
				<div class="mt-2 px-3 py-2 rounded-gx bg-red-500/10 border border-red-500/20 text-xs text-red-400">
					{digestStore.error}
				</div>
			{/if}
		</section>
	</div>
</div>
