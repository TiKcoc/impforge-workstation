<script lang="ts">
	/**
	 * IdeStatusBar — BenikUI-integrated status bar with live data
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Status bar root
	 *   - lsp-indicator: LSP connection status
	 *   - git-info: Branch name + status
	 *   - cursor-info: Line/column display
	 *   - language: File language badge
	 *   - ai-model: Active AI model indicator
	 */

	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { GitBranch, Circle, Cpu, FileCode, Lightbulb } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import { editPredictor } from '$lib/stores/edit-predictor.svelte';

	interface Props {
		cursorLine?: number;
		cursorCol?: number;
	}

	let { cursorLine = 1, cursorCol = 1 }: Props = $props();

	// Live data state
	let lspServers = $state<Array<{ language: string; running: boolean }>>([]);
	let gitBranch = $state('');
	let gitClean = $state(true);
	let gitAhead = $state(0);
	let pollInterval: ReturnType<typeof setInterval> | undefined;

	// BenikUI style engine
	const widgetId = 'ide-status-bar';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let lspComp = $derived(styleEngine.getComponentStyle(widgetId, 'lsp-indicator'));
	let lspStyle = $derived(hasEngineStyle && lspComp ? componentToCSS(lspComp) : '');
	let gitComp = $derived(styleEngine.getComponentStyle(widgetId, 'git-info'));
	let gitStyle = $derived(hasEngineStyle && gitComp ? componentToCSS(gitComp) : '');
	let cursorComp = $derived(styleEngine.getComponentStyle(widgetId, 'cursor-info'));
	let cursorStyle = $derived(hasEngineStyle && cursorComp ? componentToCSS(cursorComp) : '');
	let langComp = $derived(styleEngine.getComponentStyle(widgetId, 'language'));
	let langStyle = $derived(hasEngineStyle && langComp ? componentToCSS(langComp) : '');
	let aiComp = $derived(styleEngine.getComponentStyle(widgetId, 'ai-model'));
	let aiStyle = $derived(hasEngineStyle && aiComp ? componentToCSS(aiComp) : '');

	// Derived data
	let language = $derived(ide.activeTab?.language || 'plaintext');
	let runningLspCount = $derived(lspServers.filter(s => s.running).length);
	let lspColor = $derived(
		runningLspCount > 0 ? 'text-gx-status-success' :
		lspServers.length > 0 ? 'text-gx-status-warning' :
		'text-gx-text-disabled'
	);
	let lspLabel = $derived(
		runningLspCount > 0 ? `LSP (${runningLspCount})` : 'LSP'
	);

	async function pollStatus() {
		try {
			const [lspResult, gitResult] = await Promise.allSettled([
				invoke<Array<{ language: string; running: boolean; pid: number | null; server_binary: string; workspace_path: string }>>('lsp_status'),
				ide.currentDir ? invoke<{ branch: string; files: unknown[]; clean: boolean; ahead: number; behind: number }>('git_status', { workspacePath: ide.currentDir }) : Promise.resolve(null),
			]);

			if (lspResult.status === 'fulfilled') {
				lspServers = lspResult.value;
			}
			if (gitResult.status === 'fulfilled' && gitResult.value) {
				gitBranch = gitResult.value.branch;
				gitClean = gitResult.value.clean;
				gitAhead = gitResult.value.ahead;
			}
		} catch { /* silent — status bar is non-critical */ }
	}

	onMount(() => {
		pollStatus();
		pollInterval = setInterval(pollStatus, 5000);
	});

	onDestroy(() => {
		if (pollInterval) clearInterval(pollInterval);
	});
</script>

<div
	class="flex items-center h-6 px-2 border-t border-gx-border-subtle text-[11px] text-gx-text-muted shrink-0 gap-3 {hasEngineStyle ? '' : 'bg-gx-bg-primary'}"
	style={containerStyle}
>
	<!-- LSP indicator -->
	<div class="flex items-center gap-1" style={lspStyle}>
		<Circle size={8} class={`${lspColor} fill-current`} />
		<span>{lspLabel}</span>
	</div>

	<!-- Git info -->
	{#if gitBranch}
		<div class="flex items-center gap-1" style={gitStyle}>
			<GitBranch size={12} />
			<span>{gitBranch}</span>
			{#if !gitClean}
				<span class="text-gx-status-warning">*</span>
			{/if}
			{#if gitAhead > 0}
				<span class="text-gx-accent-cyan text-[10px]">↑{gitAhead}</span>
			{/if}
		</div>
	{/if}

	<div class="flex-1"></div>

	<!-- Edit Predictions (Neuroscience-Inspired v2) -->
	{#if editPredictor.enabled && editPredictor.predictions.length > 0}
		{@const topPred = editPredictor.predictions[0]}
		{@const accuracy = editPredictor.getAccuracy()}
		<button
			onclick={() => {
				const event = new CustomEvent('goto-line', { detail: topPred.line, bubbles: true });
				document.dispatchEvent(event);
			}}
			class="flex items-center gap-1 text-gx-neon/70 hover:text-gx-neon transition-colors"
			title="Next edit prediction: Ln {topPred.line} — {topPred.reason} ({Math.round(topPred.confidence * 100)}%)\nAccuracy: {accuracy}% | Associations: {editPredictor.getAssociationCount()} | Alt+↓ to jump"
		>
			<Lightbulb size={10} />
			<span class="text-[10px]">→Ln {topPred.line}</span>
			{#if topPred.kind === 'hebbian'}
				<span class="text-[8px] text-gx-accent-magenta">🧠</span>
			{/if}
			{#if accuracy > 0}
				<span class="text-[8px] text-gx-text-disabled">{accuracy}%</span>
			{/if}
		</button>
	{/if}

	<!-- Cursor position -->
	<span style={cursorStyle}>Ln {cursorLine}, Col {cursorCol}</span>

	<!-- Encoding -->
	<span>UTF-8</span>

	<!-- Language -->
	<span class="{hasEngineStyle ? '' : 'text-gx-accent-cyan'}" style={langStyle}>
		<FileCode size={10} class="inline mr-0.5" />{language}
	</span>

	<!-- AI model -->
	<div class="flex items-center gap-1" style={aiStyle}>
		<Cpu size={10} class={hasEngineStyle ? '' : 'text-gx-neon'} />
		<span class="{hasEngineStyle ? '' : 'text-gx-neon'}">Ollama</span>
	</div>
</div>
