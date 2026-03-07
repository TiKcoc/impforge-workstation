<script lang="ts">
	import { onMount } from 'svelte';
	import { loadSettings, saveSetting, getSettings } from '$lib/stores/settings.svelte';
	import {
		Settings, Key, Palette, Globe, Server, Save, Check,
		AlertCircle, Eye, EyeOff, ExternalLink, RefreshCw
	} from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';

	// Local UI state
	let loaded = $state(false);
	let showApiKey = $state(false);
	let ollamaStatus = $state<'idle' | 'checking' | 'online' | 'offline'>('idle');
	let serviceStatuses = $state<Record<string, 'idle' | 'checking' | 'online' | 'offline'>>({
		n8n: 'idle',
		langflow: 'idle',
		openwebui: 'idle',
		grafana: 'idle',
		searxng: 'idle',
		comfyui: 'idle',
	});
	let saveFlash = $state(false);
	let fontSize = $state(14);

	// Service definitions
	const services = [
		{ key: 'n8n', name: 'n8n', defaultUrl: 'http://localhost:5678', description: 'Workflow Automation' },
		{ key: 'langflow', name: 'LangFlow', defaultUrl: 'http://localhost:7860', description: 'AI Flow Builder' },
		{ key: 'openwebui', name: 'Open WebUI', defaultUrl: 'http://localhost:3000', description: 'Chat Interface' },
		{ key: 'grafana', name: 'Grafana', defaultUrl: 'http://localhost:3001', description: 'Monitoring Dashboard' },
		{ key: 'searxng', name: 'SearXNG', defaultUrl: 'http://localhost:8080', description: 'Private Search Engine' },
		{ key: 'comfyui', name: 'ComfyUI', defaultUrl: 'http://localhost:8188', description: 'Image Generation' },
	];

	let serviceUrls = $state<Record<string, string>>({
		n8n: 'http://localhost:5678',
		langflow: 'http://localhost:7860',
		openwebui: 'http://localhost:3000',
		grafana: 'http://localhost:3001',
		searxng: 'http://localhost:8080',
		comfyui: 'http://localhost:8188',
	});

	// Derived state
	let settings = $derived(getSettings());
	let apiKeySet = $derived(settings.openrouterKey.length > 0);
	let maskedKey = $derived(
		settings.openrouterKey
			? settings.openrouterKey.slice(0, 6) + '...' + settings.openrouterKey.slice(-4)
			: ''
	);
	let providerLabel = $derived(
		settings.autoRouting ? 'Auto (Intelligent Router)'
		: settings.preferLocalModels ? 'Ollama'
		: 'OpenRouter'
	);

	onMount(async () => {
		await loadSettings();
		loaded = true;
	});

	// Handlers
	async function handleApiKeyChange(e: Event) {
		const target = e.target as HTMLInputElement;
		await saveSetting('openrouterKey', target.value);
		flashSave();
	}

	async function handleOllamaUrlChange(e: Event) {
		const target = e.target as HTMLInputElement;
		await saveSetting('ollamaUrl', target.value);
		flashSave();
	}

	async function handleProviderChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		const value = target.value;
		if (value === 'auto') {
			await saveSetting('autoRouting', true);
			await saveSetting('preferLocalModels', true);
		} else if (value === 'ollama') {
			await saveSetting('autoRouting', false);
			await saveSetting('preferLocalModels', true);
		} else {
			await saveSetting('autoRouting', false);
			await saveSetting('preferLocalModels', false);
		}
		flashSave();
	}

	function getCurrentProvider(): string {
		if (settings.autoRouting) return 'auto';
		if (settings.preferLocalModels) return 'ollama';
		return 'openrouter';
	}

	async function testOllamaConnection() {
		ollamaStatus = 'checking';
		try {
			const res = await fetch(settings.ollamaUrl + '/api/tags', {
				signal: AbortSignal.timeout(5000),
			});
			ollamaStatus = res.ok ? 'online' : 'offline';
		} catch {
			ollamaStatus = 'offline';
		}
	}

	async function testServiceConnection(key: string) {
		serviceStatuses[key] = 'checking';
		try {
			const res = await fetch(serviceUrls[key], {
				signal: AbortSignal.timeout(5000),
				mode: 'no-cors',
			});
			// no-cors returns opaque response, so any non-error means reachable
			serviceStatuses[key] = 'online';
		} catch {
			serviceStatuses[key] = 'offline';
		}
	}

	function flashSave() {
		saveFlash = true;
		setTimeout(() => { saveFlash = false; }, 1500);
	}

	function statusDot(status: 'idle' | 'checking' | 'online' | 'offline'): string {
		switch (status) {
			case 'online': return 'text-gx-status-success';
			case 'offline': return 'text-gx-status-error';
			case 'checking': return 'text-gx-status-warning animate-pulse';
			default: return 'text-gx-text-muted';
		}
	}

	function statusLabel(status: 'idle' | 'checking' | 'online' | 'offline'): string {
		switch (status) {
			case 'online': return 'Connected';
			case 'offline': return 'Unreachable';
			case 'checking': return 'Checking...';
			default: return 'Not tested';
		}
	}
</script>

<div class="h-full overflow-y-auto">
	<div class="max-w-3xl mx-auto p-6 space-y-6 pb-16">

		<!-- Page Header -->
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
					<Check size={14} />
					<span>Saved</span>
				</div>
			{/if}
		</div>

		{#if !loaded}
			<!-- Loading skeleton -->
			<div class="space-y-6">
				{#each Array(4) as _}
					<div class="h-40 rounded-gx-lg bg-gx-bg-secondary animate-pulse border border-gx-border-default"></div>
				{/each}
			</div>
		{:else}

			<!-- ============================================ -->
			<!-- Section 1: API Keys                          -->
			<!-- ============================================ -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Key size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">API Keys</h2>
				</div>

				<div class="p-5 space-y-4">
					<!-- OpenRouter API Key -->
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<label for="api-key" class="text-sm font-medium text-gx-text-secondary">
								OpenRouter API Key
							</label>
							<div class="flex items-center gap-2">
								{#if apiKeySet}
									<Badge class="bg-gx-neon/10 text-gx-neon border-gx-neon/30 text-[10px]">
										<Check size={10} class="mr-1" />
										Configured
									</Badge>
								{:else}
									<Badge variant="outline" class="border-gx-status-warning/50 text-gx-status-warning text-[10px]">
										<AlertCircle size={10} class="mr-1" />
										Not set
									</Badge>
								{/if}
							</div>
						</div>

						<div class="relative">
							<input
								id="api-key"
								type={showApiKey ? 'text' : 'password'}
								value={settings.openrouterKey}
								oninput={handleApiKeyChange}
								placeholder="sk-or-v1-..."
								class="w-full px-3 py-2 pr-10 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono"
							/>
							<button
								onclick={() => showApiKey = !showApiKey}
								class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gx-text-muted hover:text-gx-text-secondary transition-colors rounded"
								title={showApiKey ? 'Hide key' : 'Show key'}
							>
								{#if showApiKey}
									<EyeOff size={16} />
								{:else}
									<Eye size={16} />
								{/if}
							</button>
						</div>

						<div class="flex items-start gap-2 text-xs text-gx-text-muted">
							<AlertCircle size={12} class="mt-0.5 shrink-0" />
							<p>
								Free models work without an API key. Add a key for higher rate limits and access to premium models.
							</p>
						</div>

						<a
							href="https://openrouter.ai/keys"
							target="_blank"
							rel="noopener noreferrer"
							class="inline-flex items-center gap-1.5 text-xs text-gx-neon hover:text-gx-neon-bright transition-colors"
						>
							Get an API key at openrouter.ai
							<ExternalLink size={11} />
						</a>
					</div>
				</div>
			</section>

			<!-- ============================================ -->
			<!-- Section 2: AI Model Preferences              -->
			<!-- ============================================ -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Globe size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">AI Model Preferences</h2>
				</div>

				<div class="p-5 space-y-5">
					<!-- Default Provider -->
					<div class="space-y-2">
						<label for="provider" class="text-sm font-medium text-gx-text-secondary">
							Default Provider
						</label>
						<select
							id="provider"
							value={getCurrentProvider()}
							onchange={handleProviderChange}
							class="w-full px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary focus:border-gx-neon focus:outline-none appearance-none cursor-pointer"
						>
							<option value="auto">Auto (Intelligent Router)</option>
							<option value="openrouter">OpenRouter</option>
							<option value="ollama">Ollama</option>
						</select>
						<p class="text-xs text-gx-text-muted">
							Auto mode picks the best model for each task, prioritizing free and local options.
						</p>
					</div>

					<!-- Ollama URL -->
					<div class="space-y-2">
						<label for="ollama-url" class="text-sm font-medium text-gx-text-secondary">
							Ollama URL
						</label>
						<div class="flex gap-2">
							<input
								id="ollama-url"
								type="text"
								value={settings.ollamaUrl}
								oninput={handleOllamaUrlChange}
								placeholder="http://localhost:11434"
								class="flex-1 px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono"
							/>
							<button
								onclick={testOllamaConnection}
								disabled={ollamaStatus === 'checking'}
								class="flex items-center gap-1.5 px-3 py-2 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all disabled:opacity-50"
							>
								<RefreshCw size={12} class={ollamaStatus === 'checking' ? 'animate-spin' : ''} />
								Test
							</button>
						</div>
						{#if ollamaStatus !== 'idle'}
							<div class="flex items-center gap-1.5 text-xs">
								<span class={statusDot(ollamaStatus)}>&#9679;</span>
								<span class={
									ollamaStatus === 'online' ? 'text-gx-status-success'
									: ollamaStatus === 'offline' ? 'text-gx-status-error'
									: 'text-gx-status-warning'
								}>
									{statusLabel(ollamaStatus)}
								</span>
							</div>
						{/if}
					</div>
				</div>
			</section>

			<!-- ============================================ -->
			<!-- Section 3: Appearance                        -->
			<!-- ============================================ -->
			<section class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg overflow-hidden">
				<div class="flex items-center gap-2.5 px-5 py-3.5 border-b border-gx-border-default bg-gx-bg-tertiary">
					<Palette size={16} class="text-gx-neon" />
					<h2 class="text-sm font-semibold text-gx-text-primary">Appearance</h2>
				</div>

				<div class="p-5 space-y-5">
					<!-- Theme -->
					<div class="space-y-2">
						<label for="theme" class="text-sm font-medium text-gx-text-secondary">
							Theme
						</label>
						<div class="flex items-center gap-3">
							<select
								id="theme"
								disabled
								class="flex-1 px-3 py-2 text-sm bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-muted cursor-not-allowed"
							>
								<option>Opera GX Dark</option>
							</select>
							<Badge variant="outline" class="border-gx-border-default text-gx-text-muted text-[10px] whitespace-nowrap">
								More themes coming soon
							</Badge>
						</div>
					</div>

					<!-- Font Size -->
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<label for="font-size" class="text-sm font-medium text-gx-text-secondary">
								Font Size
							</label>
							<span class="text-xs font-mono text-gx-neon">{fontSize}px</span>
						</div>
						<input
							id="font-size"
							type="range"
							min="12"
							max="20"
							step="1"
							bind:value={fontSize}
							class="w-full h-1.5 rounded-full appearance-none cursor-pointer
								[&::-webkit-slider-thumb]:appearance-none
								[&::-webkit-slider-thumb]:w-4
								[&::-webkit-slider-thumb]:h-4
								[&::-webkit-slider-thumb]:rounded-full
								[&::-webkit-slider-thumb]:bg-gx-neon
								[&::-webkit-slider-thumb]:shadow-gx-glow-sm
								[&::-webkit-slider-thumb]:cursor-pointer
								bg-gx-bg-elevated border-0"
							style="background: linear-gradient(to right, var(--color-gx-neon) 0%, var(--color-gx-neon) {((fontSize - 12) / 8) * 100}%, var(--color-gx-bg-elevated) {((fontSize - 12) / 8) * 100}%, var(--color-gx-bg-elevated) 100%);"
						/>
						<div class="flex justify-between text-[10px] text-gx-text-muted">
							<span>12px</span>
							<span>16px</span>
							<span>20px</span>
						</div>
					</div>
				</div>
			</section>

			<!-- ============================================ -->
			<!-- Section 4: Services                          -->
			<!-- ============================================ -->
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
								<div class="flex items-center gap-2">
									{#if serviceStatuses[service.key] !== 'idle'}
										<div class="flex items-center gap-1.5 text-xs">
											<span class={statusDot(serviceStatuses[service.key])}>&#9679;</span>
											<span class={
												serviceStatuses[service.key] === 'online' ? 'text-gx-status-success'
												: serviceStatuses[service.key] === 'offline' ? 'text-gx-status-error'
												: 'text-gx-status-warning'
											}>
												{statusLabel(serviceStatuses[service.key])}
											</span>
										</div>
									{/if}
								</div>
							</div>
							<div class="flex gap-2">
								<input
									type="text"
									bind:value={serviceUrls[service.key]}
									placeholder={service.defaultUrl}
									class="flex-1 px-3 py-1.5 text-xs bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono"
								/>
								<button
									onclick={() => testServiceConnection(service.key)}
									disabled={serviceStatuses[service.key] === 'checking'}
									class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-gx-bg-elevated border border-gx-border-default rounded-gx text-gx-text-secondary hover:border-gx-neon hover:text-gx-neon transition-all disabled:opacity-50"
								>
									<RefreshCw size={11} class={serviceStatuses[service.key] === 'checking' ? 'animate-spin' : ''} />
									Check
								</button>
								<a
									href={serviceUrls[service.key]}
									target="_blank"
									rel="noopener noreferrer"
									class="flex items-center px-2 py-1.5 text-gx-text-muted hover:text-gx-neon transition-colors rounded-gx border border-gx-border-default hover:border-gx-neon bg-gx-bg-elevated"
									title="Open in browser"
								>
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
