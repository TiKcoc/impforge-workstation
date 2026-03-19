<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import ProGate from '$lib/components/ProGate.svelte';
	import {
		Route, Zap, Cloud, Cpu, BarChart3, ArrowUpDown,
		CheckCircle, XCircle, Download, Play, RefreshCw,
		ChevronUp, ChevronDown, Gauge, Timer, DollarSign,
		Brain, Layers, CircleDot, AlertTriangle, Settings2
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-router';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let cardComponent = $derived(styleEngine.getComponentStyle(widgetId, 'card'));
	let cardStyle = $derived(hasEngineStyle && cardComponent ? componentToCSS(cardComponent) : '');
	let routerHeaderComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let routerHeaderStyle = $derived(hasEngineStyle && routerHeaderComponent ? componentToCSS(routerHeaderComponent) : '');

	// --- Types ---

	interface ModelTier {
		name: string;
		model_id: string;
		tier_level: number;
		max_context: number;
		capabilities: string[];
		cost_per_token: number;
		avg_latency_ms: number;
		available: boolean;
	}

	interface RouterStats {
		total_requests: number;
		local_handled: number;
		cloud_fallbacks: number;
		avg_confidence: number;
		cost_saved_usd: number;
		escalations: number;
	}

	interface ClassifyResult {
		complexity: string;
		label: string;
		min_tier: number;
		word_count: number;
	}

	interface RouterResponse {
		text: string;
		model_used: string;
		tier_level: number;
		confidence: number;
		escalated: boolean;
		latency_ms: number;
		tokens: number;
		complexity: string;
	}

	// --- State ---

	let tiers = $state<ModelTier[]>([]);
	let stats = $state<RouterStats>({
		total_requests: 0,
		local_handled: 0,
		cloud_fallbacks: 0,
		avg_confidence: 0,
		cost_saved_usd: 0,
		escalations: 0,
	});
	let loading = $state(true);
	let detecting = $state(false);
	let error = $state<string | null>(null);

	// Classifier demo
	let classifyInput = $state('');
	let classifyResult = $state<ClassifyResult | null>(null);
	let classifying = $state(false);

	// Inference test
	let inferInput = $state('');
	let inferResult = $state<RouterResponse | null>(null);
	let inferring = $state(false);

	// Active tab
	let activeTab = $state('overview');

	// --- Data loading ---

	async function loadTiers() {
		try {
			tiers = await invoke<ModelTier[]>('router_get_tiers');
		} catch (e) {
			error = String(e);
		}
	}

	async function loadStats() {
		try {
			stats = await invoke<RouterStats>('router_get_stats');
		} catch (e) {
			// Stats failure is non-critical
		}
	}

	async function detectModels() {
		detecting = true;
		error = null;
		try {
			tiers = await invoke<ModelTier[]>('router_detect_models');
		} catch (e) {
			error = String(e);
		} finally {
			detecting = false;
		}
	}

	async function classifyPrompt() {
		if (!classifyInput.trim()) return;
		classifying = true;
		classifyResult = null;
		try {
			classifyResult = await invoke<ClassifyResult>('router_classify_task', { prompt: classifyInput });
		} catch (e) {
			error = String(e);
		} finally {
			classifying = false;
		}
	}

	async function runInference() {
		if (!inferInput.trim()) return;
		inferring = true;
		inferResult = null;
		error = null;
		try {
			inferResult = await invoke<RouterResponse>('router_infer', { prompt: inferInput });
			await loadStats();
		} catch (e) {
			error = String(e);
		} finally {
			inferring = false;
		}
	}

	onMount(async () => {
		loading = true;
		await Promise.all([loadTiers(), loadStats()]);
		loading = false;
	});

	// --- Derived values ---

	let availableCount = $derived(tiers.filter(t => t.available).length);
	let localRatio = $derived(
		stats.total_requests > 0
			? Math.round((stats.local_handled / stats.total_requests) * 100)
			: 0
	);
	let cloudRatio = $derived(
		stats.total_requests > 0
			? Math.round((stats.cloud_fallbacks / stats.total_requests) * 100)
			: 0
	);

	// --- Helpers ---

	function tierColor(level: number): string {
		switch (level) {
			case 1: return 'text-gx-status-success';
			case 2: return 'text-gx-accent-blue';
			case 3: return 'text-gx-accent-purple';
			case 4: return 'text-gx-accent-magenta';
			case 5: return 'text-gx-status-warning';
			default: return 'text-gx-text-muted';
		}
	}

	function tierBgColor(level: number): string {
		switch (level) {
			case 1: return 'bg-gx-status-success/10 border-gx-status-success/30';
			case 2: return 'bg-gx-accent-blue/10 border-gx-accent-blue/30';
			case 3: return 'bg-gx-accent-purple/10 border-gx-accent-purple/30';
			case 4: return 'bg-gx-accent-magenta/10 border-gx-accent-magenta/30';
			case 5: return 'bg-gx-status-warning/10 border-gx-status-warning/30';
			default: return 'bg-gx-bg-tertiary border-gx-border-default';
		}
	}

	function complexityColor(c: string): string {
		switch (c) {
			case 'trivial': return 'text-gx-status-success';
			case 'simple': return 'text-gx-accent-blue';
			case 'medium': return 'text-gx-accent-purple';
			case 'complex': return 'text-gx-accent-magenta';
			case 'expert': return 'text-gx-status-warning';
			default: return 'text-gx-text-muted';
		}
	}

	function formatLatency(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		return `${(ms / 1000).toFixed(1)}s`;
	}
</script>

<div class="h-full overflow-auto p-6 space-y-6" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center justify-between" style={routerHeaderStyle}>
		<div class="flex items-center gap-3">
			<div class="flex items-center justify-center w-10 h-10 rounded-gx bg-gx-accent-purple/15 border border-gx-accent-purple/30">
				<Route size={22} class="text-gx-accent-purple" />
			</div>
			<div>
				<h1 class="text-xl font-bold text-gx-text-primary">Model Router</h1>
				<p class="text-sm text-gx-text-muted">Smart tiered inference -- smallest model first, escalate when needed</p>
			</div>
		</div>
		<div class="flex items-center gap-2">
			<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-accent-purple/40 text-gx-accent-purple">
				arXiv:2510.03847
			</Badge>
			<button
				onclick={detectModels}
				disabled={detecting}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx
					bg-gx-accent-purple/15 border border-gx-accent-purple/30 text-gx-accent-purple
					hover:bg-gx-accent-purple/25 transition-all disabled:opacity-50"
			>
				<RefreshCw size={12} class={detecting ? 'animate-spin' : ''} />
				{detecting ? 'Detecting...' : 'Detect Models'}
			</button>
		</div>
	</div>

	<!-- Quick Stats Row -->
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
		<!-- Available Tiers -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3" style={cardStyle}>
			<div class="flex items-center gap-2 mb-1">
				<Layers size={13} class="text-gx-accent-purple" />
				<span class="text-[11px] text-gx-text-muted">Available Tiers</span>
			</div>
			<div class="text-lg font-bold text-gx-text-primary">
				{availableCount}<span class="text-sm text-gx-text-muted font-normal">/{tiers.length}</span>
			</div>
		</div>

		<!-- Local Ratio -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3" style={cardStyle}>
			<div class="flex items-center gap-2 mb-1">
				<Cpu size={13} class="text-gx-status-success" />
				<span class="text-[11px] text-gx-text-muted">Local Handled</span>
			</div>
			<div class="text-lg font-bold text-gx-status-success">
				{localRatio}%
				<span class="text-[11px] text-gx-text-muted font-normal ml-1">({stats.local_handled})</span>
			</div>
		</div>

		<!-- Total Requests -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3" style={cardStyle}>
			<div class="flex items-center gap-2 mb-1">
				<BarChart3 size={13} class="text-gx-accent-blue" />
				<span class="text-[11px] text-gx-text-muted">Total Requests</span>
			</div>
			<div class="text-lg font-bold text-gx-text-primary">
				{stats.total_requests}
			</div>
		</div>

		<!-- Cost Saved -->
		<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-3" style={cardStyle}>
			<div class="flex items-center gap-2 mb-1">
				<DollarSign size={13} class="text-gx-neon" />
				<span class="text-[11px] text-gx-text-muted">Est. Saved</span>
			</div>
			<div class="text-lg font-bold text-gx-neon">
				${stats.cost_saved_usd.toFixed(4)}
			</div>
		</div>
	</div>

	<!-- Error display -->
	{#if error}
		<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/10 p-3 flex items-start gap-2">
			<AlertTriangle size={14} class="text-gx-status-error mt-0.5 shrink-0" />
			<div class="text-sm text-gx-status-error">{error}</div>
			<button onclick={() => error = null} class="ml-auto text-gx-status-error/60 hover:text-gx-status-error">
				<XCircle size={14} />
			</button>
		</div>
	{/if}

	<!-- Tabs -->
	<Tabs.Root bind:value={activeTab}>
		<Tabs.List class="bg-gx-bg-secondary border border-gx-border-default rounded-gx p-1">
			<Tabs.Trigger value="overview" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon text-gx-text-muted text-xs px-3 py-1.5 rounded">
				<Layers size={12} class="mr-1.5" />
				Tier Overview
			</Tabs.Trigger>
			<Tabs.Trigger value="classify" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon text-gx-text-muted text-xs px-3 py-1.5 rounded">
				<Brain size={12} class="mr-1.5" />
				Classifier
			</Tabs.Trigger>
			<Tabs.Trigger value="test" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon text-gx-text-muted text-xs px-3 py-1.5 rounded">
				<Play size={12} class="mr-1.5" />
				Test Inference
			</Tabs.Trigger>
			<Tabs.Trigger value="stats" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon text-gx-text-muted text-xs px-3 py-1.5 rounded">
				<BarChart3 size={12} class="mr-1.5" />
				Statistics
			</Tabs.Trigger>
		</Tabs.List>

		<!-- Tab: Tier Overview -->
		<Tabs.Content value="overview" class="mt-4">
			{#if loading}
				<div class="flex items-center justify-center py-12">
					<RefreshCw size={20} class="animate-spin text-gx-accent-purple mr-2" />
					<span class="text-sm text-gx-text-muted">Loading tier configuration...</span>
				</div>
			{:else}
				<div class="space-y-3">
					{#each tiers as tier (tier.tier_level)}
						<div class="rounded-gx border {tier.available ? tierBgColor(tier.tier_level) : 'border-gx-border-default bg-gx-bg-secondary'} p-4 transition-all">
							<div class="flex items-center justify-between mb-3">
								<div class="flex items-center gap-3">
									<!-- Tier level badge -->
									<div class="flex items-center justify-center w-8 h-8 rounded-full border {tierBgColor(tier.tier_level)}">
										<span class="text-sm font-bold {tierColor(tier.tier_level)}">T{tier.tier_level}</span>
									</div>
									<div>
										<div class="flex items-center gap-2">
											<span class="font-semibold text-gx-text-primary">{tier.name}</span>
											{#if tier.available}
												<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-success/50 text-gx-status-success">
													Available
												</Badge>
											{:else}
												<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-error/50 text-gx-status-error">
													Not Installed
												</Badge>
											{/if}
										</div>
										<div class="text-xs text-gx-text-muted font-mono mt-0.5">{tier.model_id}</div>
									</div>
								</div>

								<div class="flex items-center gap-4 text-[11px] text-gx-text-muted">
									<div class="flex items-center gap-1" title="Max context window">
										<Layers size={11} />
										<span>{(tier.max_context / 1024).toFixed(0)}K ctx</span>
									</div>
									<div class="flex items-center gap-1" title="Average latency">
										<Timer size={11} />
										<span>{formatLatency(tier.avg_latency_ms)}</span>
									</div>
									<div class="flex items-center gap-1" title="Cost per token">
										<DollarSign size={11} />
										<span>{tier.cost_per_token === 0 ? 'Free' : `$${tier.cost_per_token.toFixed(6)}`}</span>
									</div>
								</div>
							</div>

							<!-- Capabilities -->
							<div class="flex flex-wrap gap-1.5">
								{#each tier.capabilities as cap}
									<span class="px-2 py-0.5 text-[10px] rounded-full bg-gx-bg-primary border border-gx-border-default text-gx-text-secondary">
										{cap}
									</span>
								{/each}
							</div>

							<!-- Install hint for unavailable tiers -->
							{#if !tier.available && tier.tier_level < 5}
								<div class="mt-2 flex items-center gap-2 text-[11px] text-gx-text-muted">
									<Download size={11} />
									<code class="px-1.5 py-0.5 rounded bg-gx-bg-primary border border-gx-border-default font-mono">
										ollama pull {tier.model_id}
									</code>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</Tabs.Content>

		<!-- Tab: Classifier Demo -->
		<Tabs.Content value="classify" class="mt-4">
			<div class="space-y-4">
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
					<h3 class="text-sm font-semibold text-gx-text-primary mb-2 flex items-center gap-2">
						<Brain size={14} class="text-gx-accent-purple" />
						Task Complexity Classifier
					</h3>
					<p class="text-xs text-gx-text-muted mb-3">
						Type a prompt to see which tier would handle it. Classification is instant (heuristic, no LLM needed).
					</p>

					<div class="flex gap-2">
						<input
							type="text"
							bind:value={classifyInput}
							placeholder="Type a prompt to classify..."
							onkeydown={(e) => { if (e.key === 'Enter') classifyPrompt(); }}
							class="flex-1 px-3 py-2 text-sm rounded-gx bg-gx-bg-primary border border-gx-border-default
								text-gx-text-primary placeholder:text-gx-text-muted
								focus:border-gx-accent-purple focus:outline-none"
						/>
						<button
							onclick={classifyPrompt}
							disabled={classifying || !classifyInput.trim()}
							class="px-4 py-2 text-xs font-medium rounded-gx bg-gx-accent-purple/15 border border-gx-accent-purple/30
								text-gx-accent-purple hover:bg-gx-accent-purple/25 transition-all disabled:opacity-50"
						>
							{classifying ? 'Analyzing...' : 'Classify'}
						</button>
					</div>
				</div>

				{#if classifyResult}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<h4 class="text-xs font-semibold text-gx-text-muted mb-3 uppercase tracking-wider">Classification Result</h4>
						<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Complexity</div>
								<div class="text-lg font-bold {complexityColor(classifyResult.complexity)}">
									{classifyResult.label}
								</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Minimum Tier</div>
								<div class="text-lg font-bold {tierColor(classifyResult.min_tier)}">
									T{classifyResult.min_tier}
								</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Word Count</div>
								<div class="text-lg font-bold text-gx-text-primary">{classifyResult.word_count}</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Best Model</div>
								<div class="text-sm font-mono text-gx-text-secondary">
									{#each tiers.filter(t => t.available && t.tier_level >= classifyResult.min_tier) as match}
										{#if match === tiers.filter(t => t.available && t.tier_level >= classifyResult.min_tier)[0]}
											{match.model_id}
										{/if}
									{:else}
										<span class="text-gx-text-muted italic">No local model available</span>
									{/each}
								</div>
							</div>
						</div>

						<!-- Visual tier indicator -->
						<div class="mt-4 flex items-center gap-1">
							{#each [1, 2, 3, 4, 5] as lvl}
								<div
									class="flex-1 h-2 rounded-full transition-all {lvl <= classifyResult.min_tier
										? (lvl === classifyResult.min_tier ? 'bg-gx-accent-purple' : 'bg-gx-accent-purple/30')
										: 'bg-gx-bg-tertiary'}"
								></div>
							{/each}
						</div>
						<div class="flex justify-between mt-1 text-[9px] text-gx-text-muted">
							<span>Nano</span>
							<span>Small</span>
							<span>Medium</span>
							<span>Large</span>
							<span>Cloud</span>
						</div>
					</div>

					<!-- Example prompts -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<h4 class="text-xs font-semibold text-gx-text-muted mb-3 uppercase tracking-wider">Try These Examples</h4>
						<div class="grid grid-cols-1 md:grid-cols-2 gap-2">
							{#each [
								{ prompt: 'Is the sky blue?', expected: 'Trivial' },
								{ prompt: 'Summarize this paragraph about AI safety', expected: 'Simple' },
								{ prompt: 'Write a function to sort a list in Rust', expected: 'Medium' },
								{ prompt: 'Explain the trade-offs between microservices and monolithic architectures. Compare latency, deployment, and team coordination aspects.', expected: 'Complex' },
								{ prompt: 'Write a comprehensive literature review on transformer architectures', expected: 'Expert' },
							] as example}
								<button
									onclick={() => { classifyInput = example.prompt; classifyPrompt(); }}
									class="text-left p-2 rounded-gx border border-gx-border-default hover:border-gx-accent-purple/40
										hover:bg-gx-bg-hover transition-all group"
								>
									<div class="text-[11px] text-gx-text-secondary group-hover:text-gx-text-primary truncate">
										{example.prompt}
									</div>
									<div class="text-[9px] text-gx-text-muted mt-0.5">
										Expected: <span class="{complexityColor(example.expected.toLowerCase())}">{example.expected}</span>
									</div>
								</button>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		</Tabs.Content>

		<!-- Tab: Test Inference -->
		<Tabs.Content value="test" class="mt-4">
			<div class="space-y-4">
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
					<h3 class="text-sm font-semibold text-gx-text-primary mb-2 flex items-center gap-2">
						<Play size={14} class="text-gx-neon" />
						Test Smart Routing
					</h3>
					<p class="text-xs text-gx-text-muted mb-3">
						Send a prompt through the router. It will auto-classify and select the smallest capable model.
					</p>

					<div class="space-y-2">
						<textarea
							bind:value={inferInput}
							placeholder="Enter a prompt to route through the model tiers..."
							rows="3"
							class="w-full px-3 py-2 text-sm rounded-gx bg-gx-bg-primary border border-gx-border-default
								text-gx-text-primary placeholder:text-gx-text-muted resize-none
								focus:border-gx-neon focus:outline-none"
						></textarea>
						<div class="flex justify-between items-center">
							<span class="text-[10px] text-gx-text-muted">
								The router will pick the cheapest model that can handle your task.
							</span>
							<button
								onclick={runInference}
								disabled={inferring || !inferInput.trim()}
								class="flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-gx
									bg-gx-neon/15 border border-gx-neon/30 text-gx-neon
									hover:bg-gx-neon/25 transition-all disabled:opacity-50"
							>
								{#if inferring}
									<RefreshCw size={12} class="animate-spin" />
									Routing...
								{:else}
									<Zap size={12} />
									Route & Infer
								{/if}
							</button>
						</div>
					</div>
				</div>

				{#if inferResult}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4 space-y-4" style={cardStyle}>
						<!-- Routing decision info -->
						<div class="grid grid-cols-2 md:grid-cols-5 gap-3">
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Model Used</div>
								<div class="text-xs font-mono font-bold text-gx-text-primary">{inferResult.model_used}</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Tier</div>
								<div class="text-sm font-bold {tierColor(inferResult.tier_level)}">
									T{inferResult.tier_level}
									{#if inferResult.escalated}
										<span class="text-[9px] text-gx-status-warning ml-1">(escalated)</span>
									{/if}
								</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Complexity</div>
								<div class="text-sm font-bold {complexityColor(inferResult.complexity)}">
									{inferResult.complexity}
								</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Latency</div>
								<div class="text-sm font-bold text-gx-text-primary">{formatLatency(inferResult.latency_ms)}</div>
							</div>
							<div>
								<div class="text-[10px] text-gx-text-muted mb-1">Confidence</div>
								<div class="flex items-center gap-2">
									<Progress value={inferResult.confidence * 100} class="h-1.5 flex-1" />
									<span class="text-xs font-mono text-gx-text-secondary">{(inferResult.confidence * 100).toFixed(0)}%</span>
								</div>
							</div>
						</div>

						<!-- Response text -->
						<div>
							<div class="text-[10px] text-gx-text-muted mb-1 uppercase tracking-wider">Response</div>
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-sm text-gx-text-secondary whitespace-pre-wrap max-h-64 overflow-auto">
								{inferResult.text}
							</div>
						</div>

						<!-- Token info -->
						<div class="flex items-center gap-4 text-[11px] text-gx-text-muted">
							<span>Tokens: {inferResult.tokens}</span>
							<span>Local: {inferResult.tier_level < 5 ? 'Yes' : 'No'}</span>
							<span>Cost: {inferResult.tier_level < 5 ? 'Free' : `~$${(inferResult.tokens * 0.000001).toFixed(6)}`}</span>
						</div>
					</div>
				{/if}
			</div>
		</Tabs.Content>

		<!-- Tab: Statistics -->
		<Tabs.Content value="stats" class="mt-4">
			<div class="space-y-4">
				<!-- Stats cards -->
				<div class="grid grid-cols-2 md:grid-cols-3 gap-3">
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<BarChart3 size={14} class="text-gx-accent-blue" />
							<span class="text-xs font-medium text-gx-text-secondary">Total Requests</span>
						</div>
						<div class="text-2xl font-bold text-gx-text-primary">{stats.total_requests}</div>
					</div>

					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<Cpu size={14} class="text-gx-status-success" />
							<span class="text-xs font-medium text-gx-text-secondary">Local Handled</span>
						</div>
						<div class="text-2xl font-bold text-gx-status-success">{stats.local_handled}</div>
						<div class="text-[11px] text-gx-text-muted mt-1">{localRatio}% of total</div>
					</div>

					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<Cloud size={14} class="text-gx-status-warning" />
							<span class="text-xs font-medium text-gx-text-secondary">Cloud Fallbacks</span>
						</div>
						<div class="text-2xl font-bold text-gx-status-warning">{stats.cloud_fallbacks}</div>
						<div class="text-[11px] text-gx-text-muted mt-1">{cloudRatio}% of total</div>
					</div>

					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<Gauge size={14} class="text-gx-accent-purple" />
							<span class="text-xs font-medium text-gx-text-secondary">Avg Confidence</span>
						</div>
						<div class="text-2xl font-bold text-gx-text-primary">{(stats.avg_confidence * 100).toFixed(1)}%</div>
						<Progress value={stats.avg_confidence * 100} class="h-1.5 mt-2" />
					</div>

					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<ArrowUpDown size={14} class="text-gx-accent-magenta" />
							<span class="text-xs font-medium text-gx-text-secondary">Escalations</span>
						</div>
						<div class="text-2xl font-bold text-gx-accent-magenta">{stats.escalations}</div>
						<div class="text-[11px] text-gx-text-muted mt-1">
							{stats.total_requests > 0 ? Math.round((stats.escalations / stats.total_requests) * 100) : 0}% escalation rate
						</div>
					</div>

					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<div class="flex items-center gap-2 mb-2">
							<DollarSign size={14} class="text-gx-neon" />
							<span class="text-xs font-medium text-gx-text-secondary">Est. Cost Saved</span>
						</div>
						<div class="text-2xl font-bold text-gx-neon">${stats.cost_saved_usd.toFixed(4)}</div>
						<div class="text-[11px] text-gx-text-muted mt-1">vs always using cloud</div>
					</div>
				</div>

				<!-- Local vs Cloud ratio bar -->
				{#if stats.total_requests > 0}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
						<h4 class="text-xs font-semibold text-gx-text-muted mb-3 uppercase tracking-wider">Local vs Cloud Ratio</h4>
						<div class="flex rounded-full overflow-hidden h-4 bg-gx-bg-tertiary">
							{#if localRatio > 0}
								<div
									class="bg-gx-status-success/60 transition-all flex items-center justify-center"
									style="width: {localRatio}%"
								>
									{#if localRatio > 10}
										<span class="text-[9px] font-bold text-gx-text-primary">{localRatio}%</span>
									{/if}
								</div>
							{/if}
							{#if cloudRatio > 0}
								<div
									class="bg-gx-status-warning/60 transition-all flex items-center justify-center"
									style="width: {cloudRatio}%"
								>
									{#if cloudRatio > 10}
										<span class="text-[9px] font-bold text-gx-text-primary">{cloudRatio}%</span>
									{/if}
								</div>
							{/if}
						</div>
						<div class="flex justify-between mt-2 text-[10px]">
							<div class="flex items-center gap-1.5">
								<span class="w-2 h-2 rounded-full bg-gx-status-success/60"></span>
								<span class="text-gx-text-muted">Local ({stats.local_handled})</span>
							</div>
							<div class="flex items-center gap-1.5">
								<span class="w-2 h-2 rounded-full bg-gx-status-warning/60"></span>
								<span class="text-gx-text-muted">Cloud ({stats.cloud_fallbacks})</span>
							</div>
						</div>
					</div>
				{:else}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-6 text-center" style={cardStyle}>
						<BarChart3 size={24} class="text-gx-text-muted mx-auto mb-2" />
						<p class="text-sm text-gx-text-muted">No requests yet. Use the Test Inference tab to generate data.</p>
					</div>
				{/if}

				<!-- How it works -->
				<div class="rounded-gx border border-gx-border-default bg-gx-bg-secondary p-4" style={cardStyle}>
					<h4 class="text-xs font-semibold text-gx-text-muted mb-3 uppercase tracking-wider">How the Router Works</h4>
					<div class="space-y-2 text-[11px] text-gx-text-secondary">
						<div class="flex items-start gap-2">
							<CircleDot size={12} class="text-gx-accent-purple mt-0.5 shrink-0" />
							<span><strong>Step 1:</strong> Classify prompt complexity using fast heuristics (&lt;1ms, no LLM needed)</span>
						</div>
						<div class="flex items-start gap-2">
							<CircleDot size={12} class="text-gx-accent-blue mt-0.5 shrink-0" />
							<span><strong>Step 2:</strong> Select the smallest available model that can handle the complexity tier</span>
						</div>
						<div class="flex items-start gap-2">
							<CircleDot size={12} class="text-gx-neon mt-0.5 shrink-0" />
							<span><strong>Step 3:</strong> Run inference. If no local model is available, escalate to the next tier up</span>
						</div>
						<div class="flex items-start gap-2">
							<CircleDot size={12} class="text-gx-status-success mt-0.5 shrink-0" />
							<span><strong>Step 4:</strong> Track statistics: local vs cloud ratio, cost savings, confidence scores</span>
						</div>
					</div>
					<div class="mt-3 pt-3 border-t border-gx-border-default text-[10px] text-gx-text-muted">
						Based on arXiv:2510.03847 -- "Small Language Models for Agentic Systems: A Survey"
					</div>
				</div>
			</div>
		</Tabs.Content>
	</Tabs.Root>
</div>
</script>
