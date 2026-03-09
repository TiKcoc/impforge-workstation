<script lang="ts">
	/**
	 * ReasoningBlock — BenikUI-integrated hierarchical CoT display
	 *
	 * Parses <thinking> content into categorized reasoning steps
	 * (analysis/plan/action/observation) with progressive disclosure.
	 * When style engine styles are loaded, all visual properties become
	 * deeply customizable.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer reasoning block
	 *   - header: The expand/collapse button bar
	 *   - step: Individual reasoning step wrapper
	 */

	import ChatRenderer from './ChatRenderer.svelte';
	import { Brain, ChevronDown, ChevronRight, Search, ClipboardList, Zap, AlertCircle } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		content: string;
		modelName?: string;
		elapsedMs?: number;
	}

	let {
		widgetId = 'chat-reasoning',
		content,
		modelName,
		elapsedMs
	}: Props = $props();

	let expanded = $state(false);

	/** Parse thinking content into reasoning steps */
	interface ReasoningStep {
		icon: typeof Search;
		label: string;
		content: string;
		type: 'analysis' | 'plan' | 'action' | 'observation';
	}

	function parseReasoningSteps(raw: string): ReasoningStep[] {
		const lines = raw.split('\n').filter(l => l.trim());
		const steps: ReasoningStep[] = [];
		let currentContent: string[] = [];
		let currentType: ReasoningStep['type'] = 'analysis';

		function flushStep() {
			if (currentContent.length > 0) {
				const text = currentContent.join('\n').trim();
				if (text) {
					const { icon, label } = stepMeta(currentType);
					steps.push({ icon, label, content: text, type: currentType });
				}
				currentContent = [];
			}
		}

		for (const line of lines) {
			const lower = line.toLowerCase();
			if (lower.match(/^(step\s*\d|first|analyzing|looking|reading|checking|examining|understanding)/)) {
				flushStep();
				currentType = 'analysis';
			} else if (lower.match(/^(plan|approach|strategy|i('ll| will| should| need)|let me|my approach)/)) {
				flushStep();
				currentType = 'plan';
			} else if (lower.match(/^(implement|writ|creat|modif|updat|add|remov|fix|apply|execut|now i)/)) {
				flushStep();
				currentType = 'action';
			} else if (lower.match(/^(note|warning|caveat|however|but|issue|problem|error|wait)/)) {
				flushStep();
				currentType = 'observation';
			}
			currentContent.push(line);
		}
		flushStep();

		if (steps.length === 0) {
			return [{ icon: Brain, label: 'Reasoning', content: raw.trim(), type: 'analysis' }];
		}
		return steps;
	}

	function stepMeta(type: ReasoningStep['type']) {
		switch (type) {
			case 'analysis': return { icon: Search, label: 'Analysis' };
			case 'plan': return { icon: ClipboardList, label: 'Planning' };
			case 'action': return { icon: Zap, label: 'Action' };
			case 'observation': return { icon: AlertCircle, label: 'Observation' };
		}
	}

	function getModelColor(name?: string): string {
		if (!name) return '#f59e0b';
		const n = name.toLowerCase();
		if (n.includes('claude')) return '#a855f7';
		if (n.includes('qwen')) return '#06b6d4';
		if (n.includes('hermes')) return '#f59e0b';
		if (n.includes('local') || n.includes('ollama') || n.includes('llama')) return '#22c55e';
		return '#f59e0b';
	}

	let steps = $derived(parseReasoningSteps(content));
	let accentColor = $derived(getModelColor(modelName));
	let summaryText = $derived(
		steps.length === 1
			? steps[0].content.slice(0, 80) + (steps[0].content.length > 80 ? '...' : '')
			: `${steps.length} steps`
	);

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let stepComponent = $derived(styleEngine.getComponentStyle(widgetId, 'step'));

	let containerStyle = $derived(() => {
		if (hasEngineStyle && containerComponent) {
			return componentToCSS(containerComponent);
		}
		return `border-color: ${accentColor}20;`;
	});

	let headerStyle = $derived(() => {
		if (hasEngineStyle && headerComponent) {
			return componentToCSS(headerComponent);
		}
		return `color: ${accentColor}cc;`;
	});

	let stepStyle = $derived(() => {
		if (hasEngineStyle && stepComponent) {
			return componentToCSS(stepComponent);
		}
		return `--gx-glow-color: ${accentColor};`;
	});
</script>

<div
	class="my-2 overflow-hidden {hasEngineStyle ? '' : 'glass-panel-subtle'}"
	style={containerStyle()}
>
	<!-- Header -->
	<button
		onclick={() => expanded = !expanded}
		class="flex items-center gap-2 w-full px-3 py-2 text-[11px] hover:bg-white/5 transition-colors"
		style={headerStyle()}
	>
		<Brain size={13} style="color: {accentColor};" />
		{#if expanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
		<span class="font-medium">Reasoning</span>
		<span class="text-gx-text-muted">({summaryText})</span>
		{#if elapsedMs}
			<span class="ml-auto text-[9px] text-gx-text-muted tabular-nums">{(elapsedMs / 1000).toFixed(1)}s</span>
		{/if}
	</button>

	<!-- Steps (expanded) -->
	{#if expanded}
		<div class="px-3 pb-3 space-y-1.5">
			{#each steps as step, i (i)}
				<div
					class="reasoning-step pl-3 py-1.5"
					style={stepStyle()}
				>
					<div class="flex items-center gap-1.5 mb-1">
						<step.icon size={11} style="color: {accentColor};" />
						<span class="text-[10px] font-medium" style="color: {accentColor}aa;">{step.label}</span>
					</div>
					<div class="text-xs text-gx-text-muted leading-relaxed">
						<ChatRenderer content={step.content} />
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
