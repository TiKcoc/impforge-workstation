<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Code2, Sparkles, Zap, Database } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';

	interface Props {
		onCursorChange?: (line: number, col: number) => void;
	}

	let { onCursorChange }: Props = $props();

	let editorContainer = $state<HTMLDivElement>(undefined!);
	let monacoEditor: any = null;
	let monacoModule: any = null;
	let aiCompletionsEnabled = $state(true);
	let completionDisposable: any = null;

	// AI completion telemetry (displayed in status indicator)
	let lastModel = $state('');
	let lastLatency = $state(0);
	let lastFromCache = $state(false);

	onMount(async () => {
		monacoModule = await import('monaco-editor');

		monacoModule.editor.defineTheme('impforge-dark', {
			base: 'vs-dark',
			inherit: true,
			rules: [
				{ token: 'comment', foreground: '5a6a7a', fontStyle: 'italic' },
				{ token: 'keyword', foreground: 'c792ea' },
				{ token: 'string', foreground: 'c3e88d' },
				{ token: 'number', foreground: 'f78c6c' },
				{ token: 'type', foreground: 'ffcb6b' },
				{ token: 'function', foreground: '82aaff' },
				{ token: 'variable', foreground: 'eeffff' },
				{ token: 'operator', foreground: '89ddff' },
				{ token: 'constant', foreground: 'f78c6c' },
				{ token: 'tag', foreground: 'ff5370' },
				{ token: 'attribute.name', foreground: 'ffcb6b' },
				{ token: 'attribute.value', foreground: 'c3e88d' },
			],
			colors: {
				'editor.background': '#0a0e14',
				'editor.foreground': '#e0e0e0',
				'editor.lineHighlightBackground': '#141820',
				'editor.selectionBackground': '#1a3a5c',
				'editorCursor.foreground': '#00FF66',
				'editorLineNumber.foreground': '#3a4a5a',
				'editorLineNumber.activeForeground': '#00FF66',
				'editor.selectionHighlightBackground': '#1a3a5c55',
				'editorIndentGuide.background': '#1a1e28',
				'editorIndentGuide.activeBackground': '#2a3a4a',
				'editorBracketMatch.background': '#1a3a5c44',
				'editorBracketMatch.border': '#00FF6644',
			},
		});
	});

	$effect(() => {
		const tab = ide.activeTab;
		if (tab && editorContainer && monacoModule) {
			if (monacoEditor) {
				const model = monacoEditor.getModel();
				if (model) {
					const currentValue = model.getValue();
					if (currentValue !== tab.content) model.setValue(tab.content);
					monacoModule.editor.setModelLanguage(model, tab.language);
				}
			} else {
				monacoEditor = monacoModule.editor.create(editorContainer, {
					value: tab.content,
					language: tab.language,
					theme: 'impforge-dark',
					fontSize: 13,
					fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
					fontLigatures: true,
					minimap: { enabled: true, maxColumn: 80 },
					scrollBeyondLastLine: false,
					smoothScrolling: true,
					cursorBlinking: 'smooth',
					cursorSmoothCaretAnimation: 'on',
					renderLineHighlight: 'all',
					bracketPairColorization: { enabled: true },
					automaticLayout: true,
					padding: { top: 8 },
					lineNumbers: 'on',
					wordWrap: 'on',
					tabSize: 2,
					suggest: {
						showMethods: true,
						showFunctions: true,
						showVariables: true,
						showClasses: true,
						preview: true,
					},
				});

				monacoEditor.onDidChangeModelContent(() => {
					const newContent = monacoEditor.getModel()?.getValue() || '';
					ide.updateTabContent(ide.activeTabIndex, newContent);
				});

				monacoEditor.onDidChangeCursorPosition((e: any) => {
					onCursorChange?.(e.position.lineNumber, e.position.column);
				});

				monacoEditor.addCommand(
					monacoModule.KeyMod.CtrlCmd | monacoModule.KeyCode.KeyS,
					() => ide.saveFile(ide.activeTabIndex)
				);

				// Register AI inline completion provider (FIM — Fill-in-the-Middle)
				registerAiCompletionProvider(monacoModule);
			}
		}
	});

	/**
	 * Extract type signatures from other open tabs for cross-file context.
	 * Sends struct/function/interface/class signatures to the completion engine
	 * so it can resolve types referenced in the current file.
	 * (Microsoft Research 2024: +34% completion quality)
	 */
	function extractWorkspaceSymbols(): Array<{ file_path: string; signature: string; kind: string }> {
		const symbols: Array<{ file_path: string; signature: string; kind: string }> = [];
		const activeIndex = ide.activeTabIndex;

		for (let i = 0; i < ide.openTabs.length && symbols.length < 20; i++) {
			if (i === activeIndex) continue;
			const tab = ide.openTabs[i];
			if (!tab.content) continue;

			// Extract top-level signatures (fast regex scan, no AST needed)
			const lines = tab.content.split('\n');
			for (const line of lines) {
				const t = line.trim();
				if (symbols.length >= 20) break;

				// Rust: struct, enum, trait, fn, impl
				if (/^pub\s+(struct|enum|trait|fn|async\s+fn)\s+\w+/.test(t)) {
					symbols.push({ file_path: tab.path, signature: t.replace(/\s*\{.*$/, ''), kind: t.includes('fn ') ? 'function' : 'struct' });
				}
				// TypeScript/JavaScript: interface, type, function, class
				else if (/^export\s+(interface|type|function|class|const)\s+\w+/.test(t)) {
					symbols.push({ file_path: tab.path, signature: t.replace(/\s*\{.*$/, ''), kind: t.includes('function') ? 'function' : 'type' });
				}
				// Python: class, def
				else if (/^(class|def|async\s+def)\s+\w+/.test(t)) {
					symbols.push({ file_path: tab.path, signature: t.replace(/:\s*$/, ''), kind: t.startsWith('class') ? 'class' : 'function' });
				}
			}
		}

		return symbols;
	}

	/**
	 * AI Inline Completion Provider — Speculative Decoding + Multi-Model Cascading
	 *
	 * Sends code context (prefix + suffix + workspace symbols) to the
	 * completion engine. Uses speculative decoding for Medium/Complex
	 * completions: fast draft model → full model verification.
	 */
	function registerAiCompletionProvider(monaco: any) {
		if (completionDisposable) completionDisposable.dispose();

		let debounceTimer: ReturnType<typeof setTimeout> | null = null;

		completionDisposable = monaco.languages.registerInlineCompletionsProvider('*', {
			provideInlineCompletions: async (model: any, position: any, _context: any, token: any) => {
				if (!aiCompletionsEnabled || token.isCancellationRequested) {
					return { items: [] };
				}

				// Debounce: wait 500ms after last keystroke before calling AI
				if (debounceTimer) clearTimeout(debounceTimer);
				return new Promise((resolve) => {
					debounceTimer = setTimeout(async () => {
						if (token.isCancellationRequested) {
							resolve({ items: [] });
							return;
						}

						const textUntilPosition = model.getValueInRange({
							startLineNumber: 1,
							startColumn: 1,
							endLineNumber: position.lineNumber,
							endColumn: position.column,
						});
						const textAfterPosition = model.getValueInRange({
							startLineNumber: position.lineNumber,
							startColumn: position.column,
							endLineNumber: model.getLineCount(),
							endColumn: model.getLineMaxColumn(model.getLineCount()),
						});

						try {
							// Cross-file context: extract type signatures from other open tabs
							const workspaceSymbols = extractWorkspaceSymbols();

							const result = await invoke<{
								completion: string;
								model_used: string;
								latency_ms: number;
								from_cache: boolean;
							}>('ai_complete', {
								request: {
									file_path: ide.activeTab?.path || '',
									language: ide.activeTab?.language || 'plaintext',
									prefix: textUntilPosition.slice(-3000),
									suffix: textAfterPosition.slice(0, 1000),
									line: position.lineNumber,
									column: position.column,
									workspace_symbols: workspaceSymbols,
								}
							});

							// Update telemetry display
							if (result.model_used !== 'none') {
								lastModel = result.model_used;
								lastLatency = result.latency_ms;
								lastFromCache = result.from_cache;
							}

							if (!result.completion || token.isCancellationRequested) {
								resolve({ items: [] });
								return;
							}

							resolve({
								items: [{
									insertText: result.completion,
									range: {
										startLineNumber: position.lineNumber,
										startColumn: position.column,
										endLineNumber: position.lineNumber,
										endColumn: position.column,
									},
								}],
							});
						} catch {
							resolve({ items: [] });
						}
					}, 500);
				});
			},
			freeInlineCompletions: () => {},
		});
	}

	onDestroy(() => {
		if (completionDisposable) completionDisposable.dispose();
		if (monacoEditor) monacoEditor.dispose();
	});

	export function getEditor() {
		return monacoEditor;
	}

	export function getMonaco() {
		return monacoModule;
	}
</script>

<div class="flex-1 min-h-0 relative">
	{#if ide.openTabs.length > 0}
		<div bind:this={editorContainer} class="absolute inset-0"></div>
		<!-- AI Completion Status Bar -->
		<div class="absolute bottom-2 right-2 z-10 flex items-center gap-1.5">
			<!-- Telemetry: model + latency -->
			{#if lastModel && aiCompletionsEnabled}
				<div class="flex items-center gap-1 px-1.5 py-0.5 rounded text-[9px] bg-[#0d1117]/90 border border-white/5 text-white/30"
					title="Last completion: {lastModel} in {lastLatency}ms{lastFromCache ? ' (cached)' : ''}">
					{#if lastFromCache}
						<Database size={8} class="text-cyan-400" />
					{:else}
						<Zap size={8} class="text-amber-400" />
					{/if}
					<span class="text-white/50">{lastModel.length > 15 ? lastModel.slice(0, 15) + '…' : lastModel}</span>
					<span class="text-white/20">{lastLatency}ms</span>
				</div>
			{/if}

			<!-- AI Toggle -->
			<button
				onclick={() => aiCompletionsEnabled = !aiCompletionsEnabled}
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] transition-all
					{aiCompletionsEnabled
						? 'bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30'
						: 'bg-white/5 text-white/30 border border-white/10'}"
				title={aiCompletionsEnabled ? 'AI Completions: ON (Tab to accept)' : 'AI Completions: OFF'}
			>
				<Sparkles size={10} />
				AI {aiCompletionsEnabled ? 'ON' : 'OFF'}
			</button>
		</div>
	{:else}
		<div class="flex flex-col items-center justify-center h-full text-white/30 gap-4">
			<Code2 size={48} class="opacity-20" />
			<div class="text-center">
				<p class="text-sm font-medium">CodeForge IDE</p>
				<p class="text-xs mt-1">Open a file from the explorer or press Ctrl+P</p>
			</div>
			<div class="flex flex-col gap-1 text-xs text-white/30 mt-2">
				<span><kbd class="px-1 py-0.5 bg-white/5 border border-white/10 rounded text-[10px]">Ctrl+S</kbd> Save file</span>
				<span><kbd class="px-1 py-0.5 bg-white/5 border border-white/10 rounded text-[10px]">Ctrl+P</kbd> Quick Open</span>
				<span><kbd class="px-1 py-0.5 bg-white/5 border border-white/10 rounded text-[10px]">Ctrl+`</kbd> Toggle Terminal</span>
			</div>
		</div>
	{/if}
</div>
