<script lang="ts">
	import { onMount } from 'svelte';
	import { loadSettings, saveSetting, getSettings } from '$lib/stores/settings.svelte';
	import { themeStore, type NexusTheme } from '$lib/stores/theme.svelte';
	import {
		Settings, Key, Palette, Globe, Server, Save, Check,
		AlertCircle, Eye, EyeOff, ExternalLink, RefreshCw,
		Download, Upload, Trash2, Plus, Copy, Paintbrush, Layout, Grid3x3
	} from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';

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

	onMount(async () => { await loadSettings(); loaded = true; });

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
</script>

<div class="h-full overflow-y-auto">
	<div class="max-w-3xl mx-auto p-6 space-y-6 pb-16">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class="flex items-center justify-center w-10 h-10 rounded-gx-lg bg-gx-bg-elevated">
					<Settings size={22} class="text-gx-neon" />
				</div>
				<div>
					<h1 class="text-xl font-bold text-gx-text-primary">Settings</h1>
					<p class="text-sm text-gx-text-muted">Configure your NEXUS workstation</p>
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
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Key size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">API Keys</h2>
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

			<!-- Section 2: AI Model Preferences -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Globe size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">AI Model Preferences</h2>
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

			<!-- Section 3: Theme Engine (ElvUI-Style) -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Palette size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">Theme Engine</h2>
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
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Server size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">Services</h2>
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
	</div>
</div>
