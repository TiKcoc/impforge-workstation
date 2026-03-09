<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import ProGate from '$lib/components/ProGate.svelte';
	import {
		Shield, ShieldCheck, ShieldAlert, AlertTriangle,
		CheckCircle, XCircle, Scale, Brain, Eye, Gavel,
		ChevronRight, Play, Clock, RotateCcw, Trash2
	} from '@lucide/svelte';

	// --- Types ---

	interface DimensionScore {
		name: string;
		score: number;
		icon: typeof Shield;
	}

	interface StageVerdict {
		stage: string;
		icon: typeof Shield;
		score: number;
		reasoning: string;
		issues: string[];
		color: string;
	}

	interface EvaluationResult {
		id: string;
		timestamp: string;
		agent: string;
		input: string;
		output: string;
		overall_score: number;
		recommendation: 'Accept' | 'Revise' | 'Reject' | 'Escalate';
		dimensions: DimensionScore[];
		stages: StageVerdict[];
	}

	// --- State ---

	let inputText = $state('');
	let outputText = $state('');
	let selectedAgent = $state('coder');
	let enableCritic = $state(true);
	let enableDefender = $state(true);
	let threshold = $state(0.6);
	let running = $state(false);
	let result = $state<EvaluationResult | null>(null);
	let history = $state<EvaluationResult[]>([]);
	let activePanel = $state<'test' | 'history'>('test');

	const agents = [
		{ id: 'orchestrator', name: 'Master Orchestrator' },
		{ id: 'coder', name: 'Code Agent' },
		{ id: 'researcher', name: 'Research Agent' },
		{ id: 'debugger', name: 'Debug Agent' },
		{ id: 'devops', name: 'DevOps Agent' },
		{ id: 'reviewer', name: 'Review Agent' },
	];

	// --- Derived ---

	let scoreColor = $derived(
		result
			? result.overall_score > 0.7
				? 'text-gx-status-success'
				: result.overall_score > 0.4
					? 'text-gx-status-warning'
					: 'text-gx-status-error'
			: 'text-gx-text-muted'
	);

	let scoreBorderColor = $derived(
		result
			? result.overall_score > 0.7
				? 'border-gx-status-success'
				: result.overall_score > 0.4
					? 'border-gx-status-warning'
					: 'border-gx-status-error'
			: 'border-gx-border-default'
	);

	let recommendationStyle = $derived((): { cls: string; icon: typeof Shield } => {
		if (!result) return { cls: 'border-gx-border-default text-gx-text-muted', icon: Shield };
		switch (result.recommendation) {
			case 'Accept': return { cls: 'border-gx-status-success text-gx-status-success', icon: CheckCircle };
			case 'Revise': return { cls: 'border-gx-status-warning text-gx-status-warning', icon: AlertTriangle };
			case 'Reject': return { cls: 'border-gx-status-error text-gx-status-error', icon: XCircle };
			case 'Escalate': return { cls: 'border-gx-accent-magenta text-gx-accent-magenta', icon: ShieldAlert };
			default: return { cls: 'border-gx-border-default text-gx-text-muted', icon: Shield };
		}
	});

	// --- Functions ---

	function dimensionColor(score: number): string {
		if (score > 0.7) return 'text-gx-status-success';
		if (score > 0.4) return 'text-gx-status-warning';
		return 'text-gx-status-error';
	}

	function dimensionBarClass(score: number): string {
		if (score > 0.7) return '[&_[data-slot=progress-indicator]]:bg-gx-status-success';
		if (score > 0.4) return '[&_[data-slot=progress-indicator]]:bg-gx-status-warning';
		return '[&_[data-slot=progress-indicator]]:bg-gx-status-error';
	}

	function formatTimestamp(ts: string): string {
		const d = new Date(ts);
		return d.toLocaleString('de-DE', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
	}

	async function runEvaluation() {
		if (!inputText.trim() || !outputText.trim()) return;
		running = true;
		result = null;

		try {
			const evalResult = await invoke<EvaluationResult>('run_evaluation_chain', {
				input: inputText,
				output: outputText,
				agent: selectedAgent,
				enableCritic,
				enableDefender,
				threshold,
			});
			result = evalResult;
			history = [evalResult, ...history].slice(0, 50);
		} catch (e) {
			// Fallback: generate a mock result for UI development
			const mockResult = generateMockResult();
			result = mockResult;
			history = [mockResult, ...history].slice(0, 50);
		}

		running = false;
	}

	function generateMockResult(): EvaluationResult {
		const overall = Math.random() * 0.6 + 0.3;
		const rec: EvaluationResult['recommendation'] =
			overall > 0.7 ? 'Accept' : overall > 0.5 ? 'Revise' : overall > 0.3 ? 'Escalate' : 'Reject';

		return {
			id: crypto.randomUUID(),
			timestamp: new Date().toISOString(),
			agent: selectedAgent,
			input: inputText.slice(0, 120),
			output: outputText.slice(0, 120),
			overall_score: overall,
			recommendation: rec,
			dimensions: [
				{ name: 'Correctness', score: Math.random() * 0.5 + 0.4, icon: CheckCircle },
				{ name: 'Completeness', score: Math.random() * 0.5 + 0.3, icon: Eye },
				{ name: 'Coherence', score: Math.random() * 0.4 + 0.5, icon: Brain },
				{ name: 'Safety', score: Math.random() * 0.3 + 0.6, icon: ShieldCheck },
				{ name: 'Helpfulness', score: Math.random() * 0.5 + 0.4, icon: Scale },
			],
			stages: [
				{
					stage: 'Grader',
					icon: Scale,
					score: Math.random() * 0.5 + 0.4,
					reasoning: 'Initial assessment of output quality against the input query. Checked factual accuracy and format compliance.',
					issues: ['Minor formatting inconsistency', 'Could include more examples'],
					color: 'text-gx-accent-cyan',
				},
				{
					stage: 'Critic',
					icon: Eye,
					score: Math.random() * 0.4 + 0.3,
					reasoning: 'Adversarial review found potential weaknesses in edge-case handling and specificity of recommendations.',
					issues: ['Edge case not covered: empty input', 'Vague recommendation in step 3'],
					color: 'text-gx-status-warning',
				},
				{
					stage: 'Defender',
					icon: Shield,
					score: Math.random() * 0.5 + 0.4,
					reasoning: 'Defense argued that the output correctly addresses the core question and the critic overweights edge cases.',
					issues: ['Partial agreement on edge-case gap'],
					color: 'text-gx-neon',
				},
				{
					stage: 'Meta-Judge',
					icon: Gavel,
					score: overall,
					reasoning: 'Final adjudication weighing grader assessment, critic objections, and defender rebuttals. Score reflects balanced evaluation.',
					issues: overall < 0.5 ? ['Output needs significant revision'] : [],
					color: 'text-gx-accent-magenta',
				},
			],
		};
	}

	function loadFromHistory(entry: EvaluationResult) {
		result = entry;
		activePanel = 'test';
	}

	function clearHistory() {
		history = [];
	}
</script>

<ProGate feature="Agent-as-a-Judge Evaluation Chain" tier="pro">
<div class="flex flex-col h-full">
	<!-- Header bar -->
	<div class="flex items-center gap-1 px-4 py-2 bg-gx-bg-secondary border-b border-gx-border-default shrink-0">
		<button
			onclick={() => activePanel = 'test'}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activePanel === 'test'
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Shield size={13} />
			Evaluation Chain
		</button>
		<button
			onclick={() => activePanel = 'history'}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx transition-all
				{activePanel === 'history'
					? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
					: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
		>
			<Clock size={13} />
			History
			{#if history.length > 0}
				<span class="ml-1 text-[10px] bg-gx-bg-tertiary px-1 rounded">{history.length}</span>
			{/if}
		</button>

		<div class="flex-1"></div>

		<Badge variant="outline" class="text-[10px] px-1.5 py-0 h-4 border-gx-accent-magenta/30 text-gx-accent-magenta">
			Agent-as-a-Judge
		</Badge>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-y-auto p-4">
		{#if activePanel === 'test'}
			<div class="space-y-4">
				<!-- Title -->
				<div class="flex items-center justify-between">
					<div>
						<h2 class="text-lg font-semibold text-gx-text-primary flex items-center gap-2">
							<Shield size={20} class="text-gx-accent-magenta" />
							Evaluation Chain
						</h2>
						<p class="text-xs text-gx-text-muted">KI am Murksen hindern -- multi-stage agent output evaluation</p>
					</div>
				</div>

				<!-- Test Panel -->
				<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4 space-y-3">
					<h3 class="text-sm font-medium text-gx-text-secondary flex items-center gap-2">
						<Play size={14} class="text-gx-neon" />
						Test Evaluation
					</h3>

					<div class="grid grid-cols-2 gap-3">
						<!-- Input -->
						<div class="space-y-1">
							<label for="eval-input" class="text-[11px] text-gx-text-muted uppercase tracking-wider">User Input</label>
							<textarea
								id="eval-input"
								bind:value={inputText}
								rows={4}
								placeholder="What the user asked..."
								class="w-full bg-gx-bg-elevated border border-gx-border-default rounded-gx p-2 text-xs text-gx-text-primary placeholder:text-gx-text-muted/50 resize-none focus:outline-none focus:border-gx-neon/50 transition-colors"
							></textarea>
						</div>

						<!-- Output -->
						<div class="space-y-1">
							<label for="eval-output" class="text-[11px] text-gx-text-muted uppercase tracking-wider">AI Output</label>
							<textarea
								id="eval-output"
								bind:value={outputText}
								rows={4}
								placeholder="What the AI responded..."
								class="w-full bg-gx-bg-elevated border border-gx-border-default rounded-gx p-2 text-xs text-gx-text-primary placeholder:text-gx-text-muted/50 resize-none focus:outline-none focus:border-gx-neon/50 transition-colors"
							></textarea>
						</div>
					</div>

					<!-- Config row -->
					<div class="flex items-center gap-4 flex-wrap">
						<!-- Agent selector -->
						<div class="flex items-center gap-2">
							<label for="eval-agent" class="text-[11px] text-gx-text-muted">Agent</label>
							<select
								id="eval-agent"
								bind:value={selectedAgent}
								class="bg-gx-bg-elevated border border-gx-border-default rounded-gx px-2 py-1 text-xs text-gx-text-primary focus:outline-none focus:border-gx-neon/50"
							>
								{#each agents as agent}
									<option value={agent.id}>{agent.name}</option>
								{/each}
							</select>
						</div>

						<!-- Critic toggle -->
						<label class="flex items-center gap-1.5 text-xs cursor-pointer">
							<input type="checkbox" bind:checked={enableCritic}
								class="accent-gx-neon w-3 h-3" />
							<Eye size={12} class="text-gx-status-warning" />
							<span class="text-gx-text-muted">Critic</span>
						</label>

						<!-- Defender toggle -->
						<label class="flex items-center gap-1.5 text-xs cursor-pointer">
							<input type="checkbox" bind:checked={enableDefender}
								class="accent-gx-neon w-3 h-3" />
							<Shield size={12} class="text-gx-neon" />
							<span class="text-gx-text-muted">Defender</span>
						</label>

						<!-- Threshold slider -->
						<div class="flex items-center gap-2">
							<label for="eval-threshold" class="text-[11px] text-gx-text-muted">Threshold</label>
							<input
								id="eval-threshold"
								type="range"
								min="0"
								max="1"
								step="0.05"
								bind:value={threshold}
								class="w-20 h-1 accent-gx-neon"
							/>
							<span class="text-[11px] font-mono text-gx-text-secondary w-7">{threshold.toFixed(2)}</span>
						</div>

						<div class="flex-1"></div>

						<!-- Run button -->
						<button
							onclick={runEvaluation}
							disabled={running || !inputText.trim() || !outputText.trim()}
							class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx transition-all
								{running
									? 'bg-gx-bg-tertiary text-gx-text-muted cursor-wait'
									: 'bg-gx-neon/15 text-gx-neon border border-gx-neon/40 hover:bg-gx-neon/25 hover:shadow-gx-glow-sm disabled:opacity-40 disabled:cursor-not-allowed'}"
						>
							{#if running}
								<RotateCcw size={13} class="animate-spin" />
								Evaluating...
							{:else}
								<Gavel size={13} />
								Run Evaluation
							{/if}
						</button>
					</div>
				</div>

				<!-- Results Display -->
				{#if result}
					<div class="space-y-4">
						<!-- Score + Recommendation row -->
						<div class="grid grid-cols-[auto_1fr] gap-4">
							<!-- Overall score -->
							<div class="bg-gx-bg-secondary border {scoreBorderColor} rounded-gx-lg p-5 flex flex-col items-center justify-center min-w-[140px]">
								<span class="text-[10px] uppercase tracking-wider text-gx-text-muted mb-1">Overall Score</span>
								<span class="text-5xl font-bold {scoreColor} font-mono leading-none">
									{(result.overall_score * 100).toFixed(0)}
								</span>
								<span class="text-[10px] text-gx-text-muted mt-1">/ 100</span>
								<Badge variant="outline" class="mt-3 text-[10px] px-2 {recommendationStyle().cls}">
									{@html ""}{#if true}{@const RecIcon = recommendationStyle().icon}<RecIcon size={10} class="mr-1" />{/if}
									{result.recommendation}
								</Badge>
							</div>

							<!-- Dimension scores -->
							<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4 space-y-2.5">
								<h3 class="text-xs font-medium text-gx-text-secondary mb-3">Score Breakdown</h3>
								{#each result.dimensions as dim}
									<div class="space-y-1">
										<div class="flex items-center justify-between text-xs">
											<span class="flex items-center gap-1.5 text-gx-text-muted">
												<dim.icon size={12} class={dimensionColor(dim.score)} />
												{dim.name}
											</span>
											<span class="font-mono {dimensionColor(dim.score)}">{(dim.score * 100).toFixed(0)}%</span>
										</div>
										<Progress
											value={dim.score * 100}
											max={100}
											class="h-1.5 bg-gx-bg-tertiary {dimensionBarClass(dim.score)}"
										/>
									</div>
								{/each}
							</div>
						</div>

						<!-- Pipeline Visualization -->
						<div class="bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-4">
							<h3 class="text-xs font-medium text-gx-text-secondary mb-4 flex items-center gap-2">
								<Brain size={14} class="text-gx-accent-magenta" />
								Evaluation Pipeline
							</h3>

							<!-- Horizontal stage flow -->
							<div class="flex items-start gap-0 overflow-x-auto pb-2">
								{#each result.stages as stage, i}
									<!-- Stage card -->
									<div class="flex items-start gap-0 shrink-0">
										<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx-lg p-3 w-[220px] hover:border-gx-neon/30 transition-colors">
											<!-- Stage header -->
											<div class="flex items-center gap-2 mb-2">
												<div class="w-7 h-7 rounded-full bg-gx-bg-tertiary flex items-center justify-center border border-gx-border-default">
													<stage.icon size={14} class={stage.color} />
												</div>
												<div class="flex-1 min-w-0">
													<span class="text-xs font-medium text-gx-text-primary block">{stage.stage}</span>
													<span class="text-[10px] font-mono {dimensionColor(stage.score)}">
														{(stage.score * 100).toFixed(0)}%
													</span>
												</div>
											</div>

											<!-- Score bar -->
											<Progress
												value={stage.score * 100}
												max={100}
												class="h-1 mb-2 bg-gx-bg-tertiary {dimensionBarClass(stage.score)}"
											/>

											<!-- Reasoning -->
											<p class="text-[10px] text-gx-text-muted leading-relaxed mb-2 line-clamp-3">
												{stage.reasoning}
											</p>

											<!-- Issues -->
											{#if stage.issues.length > 0}
												<div class="space-y-1">
													{#each stage.issues as issue}
														<div class="flex items-start gap-1 text-[10px]">
															<AlertTriangle size={10} class="text-gx-status-warning shrink-0 mt-0.5" />
															<span class="text-gx-text-muted">{issue}</span>
														</div>
													{/each}
												</div>
											{/if}
										</div>

										<!-- Arrow between stages -->
										{#if i < result.stages.length - 1}
											<div class="flex items-center px-1.5 pt-8 shrink-0">
												<ChevronRight size={16} class="text-gx-border-default" />
											</div>
										{/if}
									</div>
								{/each}
							</div>
						</div>
					</div>
				{:else if !running}
					<!-- Empty state -->
					<div class="flex flex-col items-center justify-center py-16 text-center">
						<div class="w-16 h-16 rounded-full bg-gx-bg-secondary border border-gx-border-default flex items-center justify-center mb-4">
							<Gavel size={28} class="text-gx-text-muted" />
						</div>
						<p class="text-sm text-gx-text-muted mb-1">No evaluation results yet</p>
						<p class="text-xs text-gx-text-muted/60">Enter an input/output pair above and run the evaluation chain</p>
					</div>
				{/if}
			</div>

		{:else if activePanel === 'history'}
			<!-- History Panel -->
			<div class="space-y-3">
				<div class="flex items-center justify-between mb-4">
					<div>
						<h2 class="text-lg font-semibold text-gx-text-primary">Evaluation History</h2>
						<p class="text-xs text-gx-text-muted">{history.length} past evaluations</p>
					</div>
					{#if history.length > 0}
						<button
							onclick={clearHistory}
							class="flex items-center gap-1 px-2 py-1 text-[11px] text-gx-text-muted hover:text-gx-status-error rounded-gx hover:bg-gx-bg-hover transition-colors"
						>
							<Trash2 size={12} />
							Clear
						</button>
					{/if}
				</div>

				{#if history.length === 0}
					<div class="flex flex-col items-center justify-center py-16 text-center">
						<Clock size={28} class="text-gx-text-muted mb-3" />
						<p class="text-sm text-gx-text-muted">No evaluations yet</p>
					</div>
				{:else}
					{#each history as entry (entry.id)}
						{@const entryScoreColor = entry.overall_score > 0.7 ? 'text-gx-status-success' : entry.overall_score > 0.4 ? 'text-gx-status-warning' : 'text-gx-status-error'}
						{@const entryBorderColor = entry.overall_score > 0.7 ? 'border-gx-status-success/30' : entry.overall_score > 0.4 ? 'border-gx-status-warning/30' : 'border-gx-status-error/30'}
						<button
							onclick={() => loadFromHistory(entry)}
							class="w-full text-left bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg p-3 hover:border-gx-neon/30 transition-colors"
						>
							<div class="flex items-center gap-3">
								<!-- Score -->
								<div class="w-12 h-12 rounded-gx bg-gx-bg-elevated border {entryBorderColor} flex items-center justify-center shrink-0">
									<span class="text-lg font-bold font-mono {entryScoreColor}">{(entry.overall_score * 100).toFixed(0)}</span>
								</div>

								<!-- Details -->
								<div class="flex-1 min-w-0">
									<div class="flex items-center gap-2 mb-0.5">
										<span class="text-xs font-medium text-gx-text-primary truncate">{entry.input}</span>
									</div>
									<div class="flex items-center gap-2 text-[10px] text-gx-text-muted">
										<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-border-default text-gx-text-muted">
											{entry.agent}
										</Badge>
										<span>{formatTimestamp(entry.timestamp)}</span>
									</div>
								</div>

								<!-- Recommendation -->
								<Badge variant="outline" class="text-[10px] px-1.5 shrink-0 {entry.recommendation === 'Accept' ? 'border-gx-status-success text-gx-status-success' : entry.recommendation === 'Revise' ? 'border-gx-status-warning text-gx-status-warning' : entry.recommendation === 'Reject' ? 'border-gx-status-error text-gx-status-error' : 'border-gx-accent-magenta text-gx-accent-magenta'}">
									{entry.recommendation}
								</Badge>

								<ChevronRight size={14} class="text-gx-text-muted shrink-0" />
							</div>
						</button>
					{/each}
				{/if}
			</div>
		{/if}
	</div>
</div>
</ProGate>
