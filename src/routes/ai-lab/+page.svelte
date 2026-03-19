<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Tabs from '$lib/components/ui/tabs/index.js';
	import {
		FlaskConical, Layers, ShieldCheck, Brain, Braces,
		Play, RotateCcw, BarChart3, Loader2, AlertTriangle,
		ChevronDown, ChevronUp, Clock, Cpu, Cloud, Sparkles
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'page-ai-lab';
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

	// --- Types ---

	interface MoaResult {
		final_response: string;
		individual_responses: string[];
		synthesis_reasoning: string;
		quality_improvement: number;
		total_ms: number;
		model: string;
	}

	interface ConfidenceResult {
		response: string;
		confidence: number;
		model_used: string;
		escalated: boolean;
		escalation_reason: string | null;
		latency_ms: number;
	}

	interface ConfidenceStats {
		total_requests: number;
		local_count: number;
		cloud_count: number;
		avg_confidence: number;
		local_ratio: number;
		estimated_savings_usd: number;
	}

	interface AwareState {
		assess: string;
		weigh: string;
		act: string;
		reflect: string;
		enrich: string;
		total_ms: number;
		model: string;
	}

	// --- State ---

	let activeTab = $state('moa');

	// MoA state
	let moaPrompt = $state('');
	let moaNumResponses = $state(3);
	let moaResult = $state<MoaResult | null>(null);
	let moaLoading = $state(false);
	let moaError = $state<string | null>(null);
	let moaShowIndividual = $state(false);

	// Confidence state
	let confPrompt = $state('');
	let confThreshold = $state(8.0);
	let confResult = $state<ConfidenceResult | null>(null);
	let confStats = $state<ConfidenceStats | null>(null);
	let confLoading = $state(false);
	let confError = $state<string | null>(null);

	// AWARE state
	let awareTask = $state('');
	let awareContext = $state('');
	let awareResult = $state<AwareState | null>(null);
	let awareLoading = $state(false);
	let awareError = $state<string | null>(null);
	let awareExpandedStage = $state<string | null>(null);

	// Structured state
	let structPrompt = $state('');
	let structSchema = $state('{\n  "type": "object",\n  "required": ["name", "description"],\n  "properties": {\n    "name": { "type": "string" },\n    "description": { "type": "string" },\n    "tags": {\n      "type": "array",\n      "items": { "type": "string" }\n    }\n  }\n}');
	let structResult = $state<unknown>(null);
	let structLoading = $state(false);
	let structError = $state<string | null>(null);

	// --- MoA Actions ---

	async function runMoa() {
		if (!moaPrompt.trim()) return;
		moaLoading = true;
		moaError = null;
		moaResult = null;
		try {
			moaResult = await invoke<MoaResult>('ai_moa_generate', {
				prompt: moaPrompt,
				config: {
					num_responses: moaNumResponses,
					temperature_range: [0.3, 0.9],
					synthesis_model: null,
				},
			});
		} catch (e) {
			moaError = parseError(e);
		} finally {
			moaLoading = false;
		}
	}

	// --- Confidence Actions ---

	async function runConfidence() {
		if (!confPrompt.trim()) return;
		confLoading = true;
		confError = null;
		confResult = null;
		try {
			confResult = await invoke<ConfidenceResult>('ai_confident_generate', {
				prompt: confPrompt,
				threshold: confThreshold,
			});
			await loadConfidenceStats();
		} catch (e) {
			confError = parseError(e);
		} finally {
			confLoading = false;
		}
	}

	async function loadConfidenceStats() {
		try {
			confStats = await invoke<ConfidenceStats>('ai_get_confidence_stats');
		} catch { /* stats unavailable */ }
	}

	async function resetStats() {
		try {
			await invoke('ai_reset_confidence_stats');
			confStats = null;
			await loadConfidenceStats();
		} catch { /* ignore */ }
	}

	// --- AWARE Actions ---

	async function runAware() {
		if (!awareTask.trim()) return;
		awareLoading = true;
		awareError = null;
		awareResult = null;
		try {
			awareResult = await invoke<AwareState>('ai_aware_loop', {
				task: awareTask,
				context: awareContext || null,
			});
		} catch (e) {
			awareError = parseError(e);
		} finally {
			awareLoading = false;
		}
	}

	function toggleAwareStage(stage: string) {
		awareExpandedStage = awareExpandedStage === stage ? null : stage;
	}

	// --- Structured Actions ---

	async function runStructured() {
		if (!structPrompt.trim()) return;
		structLoading = true;
		structError = null;
		structResult = null;
		try {
			const parsedSchema = JSON.parse(structSchema);
			structResult = await invoke('ai_structured_generate', {
				prompt: structPrompt,
				schema: parsedSchema,
			});
		} catch (e) {
			if (e instanceof SyntaxError) {
				structError = `Invalid JSON Schema: ${e.message}`;
			} else {
				structError = parseError(e);
			}
		} finally {
			structLoading = false;
		}
	}

	// --- Helpers ---

	function parseError(e: unknown): string {
		if (typeof e === 'string') {
			try {
				const parsed = JSON.parse(e);
				return parsed.message || e;
			} catch { return e; }
		}
		return String(e);
	}

	function formatMs(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		return `${(ms / 1000).toFixed(1)}s`;
	}

	onMount(() => {
		loadConfidenceStats();
	});
</script>

<div class="p-6 space-y-4 overflow-auto max-h-[calc(100vh-6rem)]" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-3">
		<FlaskConical size={24} class="text-gx-accent-purple" />
		<h1 class="text-xl font-bold">AI Lab</h1>
		<Badge class="bg-gx-accent-purple/10 text-gx-accent-purple border-gx-accent-purple/30">
			Experimental
		</Badge>
		<div class="flex-1"></div>
		<span class="text-xs text-gx-text-muted">Research-backed advanced AI features</span>
	</div>

	<!-- Tabs -->
	<Tabs.Root bind:value={activeTab}>
		<Tabs.List class="bg-gx-bg-secondary border border-gx-border-default">
			<Tabs.Trigger value="moa" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Layers size={14} class="mr-1.5" />MoA
			</Tabs.Trigger>
			<Tabs.Trigger value="confidence" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<ShieldCheck size={14} class="mr-1.5" />Confidence
			</Tabs.Trigger>
			<Tabs.Trigger value="aware" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Brain size={14} class="mr-1.5" />AWARE
			</Tabs.Trigger>
			<Tabs.Trigger value="structured" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<Braces size={14} class="mr-1.5" />Structured
			</Tabs.Trigger>
			<Tabs.Trigger value="stats" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon">
				<BarChart3 size={14} class="mr-1.5" />Stats
			</Tabs.Trigger>
		</Tabs.List>

		<!-- ═══════════════════ MoA Tab ═══════════════════ -->
		<Tabs.Content value="moa" class="space-y-4 mt-4">
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
				<Card.Header class="pb-3">
					<Card.Title class="flex items-center gap-2 text-sm">
						<Layers size={16} class="text-gx-accent-blue" />
						Mixture-of-Agents (MoA)
						<Badge variant="outline" class="text-[10px] border-gx-accent-blue/30 text-gx-accent-blue">arXiv:2601.16596</Badge>
					</Card.Title>
					<Card.Description class="text-xs text-gx-text-muted">
						Generate multiple responses at different temperatures, then synthesize the best answer. +6.6% quality improvement.
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3">
					<textarea
						bind:value={moaPrompt}
						placeholder="Enter your prompt..."
						rows={3}
						class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
					></textarea>
					<div class="flex items-center gap-3">
						<label class="text-xs text-gx-text-muted">Responses:</label>
						<select bind:value={moaNumResponses} class="bg-gx-bg-primary border border-gx-border-default rounded px-2 py-1 text-xs text-gx-text-primary">
							<option value={2}>2</option>
							<option value={3}>3</option>
							<option value={4}>4</option>
							<option value={5}>5</option>
						</select>
						<div class="flex-1"></div>
						<button
							onclick={runMoa}
							disabled={moaLoading || !moaPrompt.trim()}
							class="flex items-center gap-1.5 px-4 py-1.5 rounded-gx text-xs font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
						>
							{#if moaLoading}
								<Loader2 size={12} class="animate-spin" />Generating...
							{:else}
								<Play size={12} />Generate
							{/if}
						</button>
					</div>

					{#if moaError}
						<div class="flex items-center gap-2 p-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-xs text-gx-status-error">
							<AlertTriangle size={12} />{moaError}
						</div>
					{/if}

					{#if moaResult}
						<div class="space-y-3 pt-2 border-t border-gx-border-default">
							<!-- Quality stats -->
							<div class="flex items-center gap-3 text-xs">
								<Badge class="bg-gx-status-success/10 text-gx-status-success border-gx-status-success/30">
									+{moaResult.quality_improvement.toFixed(1)}% quality
								</Badge>
								<span class="text-gx-text-muted flex items-center gap-1"><Clock size={10} />{formatMs(moaResult.total_ms)}</span>
								<span class="text-gx-text-muted flex items-center gap-1"><Cpu size={10} />{moaResult.model}</span>
								<span class="text-gx-text-muted">{moaResult.individual_responses.length} responses synthesized</span>
							</div>

							<!-- Synthesized answer -->
							<div class="rounded-gx bg-gx-bg-primary border border-gx-neon/20 p-3">
								<div class="text-[10px] uppercase tracking-wider text-gx-neon mb-1.5 font-semibold">Synthesized Answer</div>
								<p class="text-sm text-gx-text-primary whitespace-pre-wrap">{moaResult.final_response}</p>
							</div>

							<!-- Reasoning -->
							{#if moaResult.synthesis_reasoning}
								<div class="rounded-gx bg-gx-bg-tertiary border border-gx-border-default p-3">
									<div class="text-[10px] uppercase tracking-wider text-gx-text-muted mb-1.5">Synthesis Reasoning</div>
									<p class="text-xs text-gx-text-secondary whitespace-pre-wrap">{moaResult.synthesis_reasoning}</p>
								</div>
							{/if}

							<!-- Individual responses toggle -->
							<button
								onclick={() => moaShowIndividual = !moaShowIndividual}
								class="flex items-center gap-1 text-xs text-gx-text-muted hover:text-gx-neon transition-colors"
							>
								{#if moaShowIndividual}<ChevronUp size={12} />{:else}<ChevronDown size={12} />{/if}
								{moaShowIndividual ? 'Hide' : 'Show'} individual responses
							</button>

							{#if moaShowIndividual}
								<div class="space-y-2">
									{#each moaResult.individual_responses as resp, i}
										<div class="rounded-gx bg-gx-bg-tertiary border border-gx-border-default p-3">
											<div class="text-[10px] uppercase tracking-wider text-gx-text-muted mb-1">Response {i + 1} (temp {(0.3 + (0.6 / Math.max(1, moaResult.individual_responses.length - 1)) * i).toFixed(2)})</div>
											<p class="text-xs text-gx-text-secondary whitespace-pre-wrap">{resp}</p>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<!-- ═══════════════════ Confidence Tab ═══════════════════ -->
		<Tabs.Content value="confidence" class="space-y-4 mt-4">
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
				<Card.Header class="pb-3">
					<Card.Title class="flex items-center gap-2 text-sm">
						<ShieldCheck size={16} class="text-gx-status-success" />
						Confidence Calibration
						<Badge variant="outline" class="text-[10px] border-gx-status-success/30 text-gx-status-success">60% cost savings</Badge>
					</Card.Title>
					<Card.Description class="text-xs text-gx-text-muted">
						Local model self-rates confidence. Below threshold, automatically escalates to cloud. Saves money by routing only hard queries.
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3">
					<textarea
						bind:value={confPrompt}
						placeholder="Enter your prompt..."
						rows={3}
						class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
					></textarea>
					<div class="flex items-center gap-3">
						<label class="text-xs text-gx-text-muted">Threshold:</label>
						<input
							type="range"
							min={1}
							max={10}
							step={0.5}
							bind:value={confThreshold}
							class="w-32 accent-gx-neon"
						/>
						<span class="text-xs text-gx-text-secondary font-mono w-8">{confThreshold.toFixed(1)}</span>
						<div class="flex-1"></div>
						<button
							onclick={runConfidence}
							disabled={confLoading || !confPrompt.trim()}
							class="flex items-center gap-1.5 px-4 py-1.5 rounded-gx text-xs font-medium bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
						>
							{#if confLoading}
								<Loader2 size={12} class="animate-spin" />Analyzing...
							{:else}
								<Play size={12} />Generate
							{/if}
						</button>
					</div>

					{#if confError}
						<div class="flex items-center gap-2 p-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-xs text-gx-status-error">
							<AlertTriangle size={12} />{confError}
						</div>
					{/if}

					{#if confResult}
						<div class="space-y-3 pt-2 border-t border-gx-border-default">
							<div class="flex items-center gap-3 text-xs flex-wrap">
								<Badge class="{confResult.escalated ? 'bg-gx-accent-blue/10 text-gx-accent-blue border-gx-accent-blue/30' : 'bg-gx-status-success/10 text-gx-status-success border-gx-status-success/30'}">
									{#if confResult.escalated}
										<Cloud size={10} class="mr-1" />Cloud
									{:else}
										<Cpu size={10} class="mr-1" />Local
									{/if}
								</Badge>
								<span class="text-gx-text-muted">Confidence: <span class="font-mono text-gx-text-secondary">{confResult.confidence.toFixed(1)}/10</span></span>
								<span class="text-gx-text-muted flex items-center gap-1"><Clock size={10} />{formatMs(confResult.latency_ms)}</span>
								<span class="text-gx-text-muted">{confResult.model_used}</span>
							</div>
							{#if confResult.escalation_reason}
								<div class="text-xs text-gx-accent-blue bg-gx-accent-blue/5 rounded-gx px-2 py-1 border border-gx-accent-blue/20">
									{confResult.escalation_reason}
								</div>
							{/if}
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3">
								<p class="text-sm text-gx-text-primary whitespace-pre-wrap">{confResult.response}</p>
							</div>
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<!-- ═══════════════════ AWARE Tab ═══════════════════ -->
		<Tabs.Content value="aware" class="space-y-4 mt-4">
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
				<Card.Header class="pb-3">
					<Card.Title class="flex items-center gap-2 text-sm">
						<Brain size={16} class="text-gx-accent-magenta" />
						AWARE Framework
						<Badge variant="outline" class="text-[10px] border-gx-accent-magenta/30 text-gx-accent-magenta">5-stage loop</Badge>
					</Card.Title>
					<Card.Description class="text-xs text-gx-text-muted">
						Assess - Weigh - Act - Reflect - Enrich. A structured reasoning loop that stores learned insights.
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3">
					<textarea
						bind:value={awareTask}
						placeholder="Describe the task to analyze..."
						rows={3}
						class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
					></textarea>
					<textarea
						bind:value={awareContext}
						placeholder="Additional context (optional)..."
						rows={2}
						class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-xs text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
					></textarea>
					<div class="flex items-center gap-3">
						<div class="flex-1"></div>
						<button
							onclick={runAware}
							disabled={awareLoading || !awareTask.trim()}
							class="flex items-center gap-1.5 px-4 py-1.5 rounded-gx text-xs font-medium bg-gx-accent-magenta/10 text-gx-accent-magenta border border-gx-accent-magenta/30 hover:bg-gx-accent-magenta/20 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
						>
							{#if awareLoading}
								<Loader2 size={12} class="animate-spin" />Running loop...
							{:else}
								<Sparkles size={12} />Run AWARE Loop
							{/if}
						</button>
					</div>

					{#if awareError}
						<div class="flex items-center gap-2 p-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-xs text-gx-status-error">
							<AlertTriangle size={12} />{awareError}
						</div>
					{/if}

					{#if awareResult}
						{@const stages = [
							{ key: 'assess', label: 'Assess', desc: 'Situation analysis', color: 'text-gx-accent-blue', bg: 'bg-gx-accent-blue' },
							{ key: 'weigh', label: 'Weigh', desc: 'Options & trade-offs', color: 'text-gx-accent-purple', bg: 'bg-gx-accent-purple' },
							{ key: 'act', label: 'Act', desc: 'Execution plan', color: 'text-gx-neon', bg: 'bg-gx-neon' },
							{ key: 'reflect', label: 'Reflect', desc: 'Outcome evaluation', color: 'text-gx-status-warning', bg: 'bg-gx-status-warning' },
							{ key: 'enrich', label: 'Enrich', desc: 'Learned insights', color: 'text-gx-accent-magenta', bg: 'bg-gx-accent-magenta' },
						]}
						<div class="space-y-2 pt-2 border-t border-gx-border-default">
							<div class="flex items-center gap-3 text-xs">
								<span class="text-gx-text-muted flex items-center gap-1"><Clock size={10} />{formatMs(awareResult.total_ms)}</span>
								<span class="text-gx-text-muted flex items-center gap-1"><Cpu size={10} />{awareResult.model}</span>
							</div>
							{#each stages as stage}
								{@const content = awareResult[stage.key as keyof AwareState]}
								{@const isExpanded = awareExpandedStage === stage.key}
								<button
									onclick={() => toggleAwareStage(stage.key)}
									class="w-full text-left rounded-gx border border-gx-border-default bg-gx-bg-primary p-3 hover:border-gx-neon/30 transition-colors"
								>
									<div class="flex items-center gap-2">
										<div class="w-2 h-2 rounded-full {stage.bg}/60"></div>
										<span class="text-xs font-semibold {stage.color}">{stage.label}</span>
										<span class="text-[10px] text-gx-text-muted">{stage.desc}</span>
										<div class="flex-1"></div>
										{#if isExpanded}<ChevronUp size={12} class="text-gx-text-muted" />{:else}<ChevronDown size={12} class="text-gx-text-muted" />{/if}
									</div>
									{#if isExpanded && typeof content === 'string'}
										<p class="mt-2 text-xs text-gx-text-secondary whitespace-pre-wrap border-t border-gx-border-default pt-2">{content}</p>
									{/if}
								</button>
							{/each}
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<!-- ═══════════════════ Structured Tab ═══════════════════ -->
		<Tabs.Content value="structured" class="space-y-4 mt-4">
			<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
				<Card.Header class="pb-3">
					<Card.Title class="flex items-center gap-2 text-sm">
						<Braces size={16} class="text-gx-accent-yellow" />
						Structured JSON Generation
						<Badge variant="outline" class="text-[10px] border-gx-accent-yellow/30 text-gx-accent-yellow">XGrammar</Badge>
					</Card.Title>
					<Card.Description class="text-xs text-gx-text-muted">
						Generate valid JSON matching a schema. Schema-guided prompting with validation and retry (MLSys 2025).
					</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-3">
					<textarea
						bind:value={structPrompt}
						placeholder="Describe what data to generate..."
						rows={2}
						class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-none"
					></textarea>
					<div>
						<label class="text-[10px] uppercase tracking-wider text-gx-text-muted mb-1 block">JSON Schema</label>
						<textarea
							bind:value={structSchema}
							rows={8}
							class="w-full rounded-gx bg-gx-bg-primary border border-gx-border-default px-3 py-2 text-xs font-mono text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon resize-y"
						></textarea>
					</div>
					<div class="flex items-center gap-3">
						<div class="flex-1"></div>
						<button
							onclick={runStructured}
							disabled={structLoading || !structPrompt.trim()}
							class="flex items-center gap-1.5 px-4 py-1.5 rounded-gx text-xs font-medium bg-gx-accent-yellow/10 text-gx-accent-yellow border border-gx-accent-yellow/30 hover:bg-gx-accent-yellow/20 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
						>
							{#if structLoading}
								<Loader2 size={12} class="animate-spin" />Generating...
							{:else}
								<Braces size={12} />Generate JSON
							{/if}
						</button>
					</div>

					{#if structError}
						<div class="flex items-center gap-2 p-2 rounded-gx bg-gx-status-error/10 border border-gx-status-error/30 text-xs text-gx-status-error">
							<AlertTriangle size={12} />{structError}
						</div>
					{/if}

					{#if structResult}
						<div class="space-y-2 pt-2 border-t border-gx-border-default">
							<div class="flex items-center gap-2">
								<Badge class="bg-gx-status-success/10 text-gx-status-success border-gx-status-success/30">Valid JSON</Badge>
							</div>
							<pre class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-xs font-mono text-gx-text-primary overflow-x-auto">{JSON.stringify(structResult, null, 2)}</pre>
						</div>
					{/if}
				</Card.Content>
			</Card.Root>
		</Tabs.Content>

		<!-- ═══════════════════ Stats Tab ═══════════════════ -->
		<Tabs.Content value="stats" class="space-y-4 mt-4">
			<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
				<!-- Confidence Stats -->
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
					<Card.Header class="pb-2">
						<Card.Title class="text-sm flex items-center gap-2">
							<ShieldCheck size={14} class="text-gx-status-success" />
							Confidence Routing
						</Card.Title>
					</Card.Header>
					<Card.Content class="space-y-2">
						{#if confStats && confStats.total_requests > 0}
							<div class="space-y-1.5">
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted">Total Requests</span>
									<span class="text-gx-text-primary font-mono">{confStats.total_requests}</span>
								</div>
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted flex items-center gap-1"><Cpu size={10} />Local</span>
									<span class="text-gx-status-success font-mono">{confStats.local_count}</span>
								</div>
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted flex items-center gap-1"><Cloud size={10} />Cloud</span>
									<span class="text-gx-accent-blue font-mono">{confStats.cloud_count}</span>
								</div>
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted">Avg Confidence</span>
									<span class="text-gx-text-primary font-mono">{confStats.avg_confidence.toFixed(1)}/10</span>
								</div>
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted">Local Ratio</span>
									<span class="text-gx-status-success font-mono">{(confStats.local_ratio * 100).toFixed(0)}%</span>
								</div>
								<div class="flex justify-between text-xs">
									<span class="text-gx-text-muted">Est. Savings</span>
									<span class="text-gx-neon font-mono">${confStats.estimated_savings_usd.toFixed(4)}</span>
								</div>
							</div>
							<button
								onclick={resetStats}
								class="flex items-center gap-1 text-[10px] text-gx-text-muted hover:text-gx-status-error transition-colors mt-2"
							>
								<RotateCcw size={10} />Reset stats
							</button>
						{:else}
							<p class="text-xs text-gx-text-muted">No confidence data yet. Generate a response in the Confidence tab.</p>
						{/if}
					</Card.Content>
				</Card.Root>

				<!-- MoA Info -->
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
					<Card.Header class="pb-2">
						<Card.Title class="text-sm flex items-center gap-2">
							<Layers size={14} class="text-gx-accent-blue" />
							Mixture-of-Agents
						</Card.Title>
					</Card.Header>
					<Card.Content class="space-y-2">
						<div class="space-y-1.5 text-xs">
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Quality Gain</span>
								<span class="text-gx-status-success font-mono">+6.6%</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Paper</span>
								<span class="text-gx-accent-blue">arXiv:2601.16596</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Method</span>
								<span class="text-gx-text-secondary">Multi-temp synthesis</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Optimal N</span>
								<span class="text-gx-text-secondary font-mono">3-5 responses</span>
							</div>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- AWARE Info -->
				<Card.Root class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
					<Card.Header class="pb-2">
						<Card.Title class="text-sm flex items-center gap-2">
							<Brain size={14} class="text-gx-accent-magenta" />
							AWARE Framework
						</Card.Title>
					</Card.Header>
					<Card.Content class="space-y-2">
						<div class="space-y-1.5 text-xs">
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Stages</span>
								<span class="text-gx-text-secondary font-mono">5 (A-W-A-R-E)</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Memory</span>
								<span class="text-gx-status-success">Enrich stage saved</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Pattern</span>
								<span class="text-gx-text-secondary">Feedback loop</span>
							</div>
							<div class="flex justify-between">
								<span class="text-gx-text-muted">Use Case</span>
								<span class="text-gx-text-secondary">Strategic decisions</span>
							</div>
						</div>
					</Card.Content>
				</Card.Root>
			</div>
		</Tabs.Content>
	</Tabs.Root>
</div>
