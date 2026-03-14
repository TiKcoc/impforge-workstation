<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Code2, Sparkles, Zap, Database } from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';
	import { editPredictor } from '$lib/stores/edit-predictor.svelte';

	// BenikUI style engine
	const widgetId = 'ide-code-editor';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let editorAreaComp = $derived(styleEngine.getComponentStyle(widgetId, 'editor-area'));
	let editorAreaStyle = $derived(hasEngineStyle && editorAreaComp ? componentToCSS(editorAreaComp) : '');
	let minimapComp = $derived(styleEngine.getComponentStyle(widgetId, 'minimap'));
	let minimapStyle = $derived(hasEngineStyle && minimapComp ? componentToCSS(minimapComp) : '');

	interface Props {
		onCursorChange?: (line: number, col: number) => void;
	}

	let { onCursorChange }: Props = $props();

	let editorContainer = $state<HTMLDivElement>(undefined!);
	let monacoEditor: any = null;
	let monacoModule: any = null;
	let aiCompletionsEnabled = $state(true);
	let completionDisposable: any = null;
	let breakpoints = $state<Set<number>>(new Set());
	let gutterDecorations: string[] = [];
	let predictionDecorations: string[] = [];

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

					// Feed edit predictor with cursor position + file content
					const pos = monacoEditor.getPosition();
					if (pos && ide.activeTab) {
						editPredictor.recordEdit(pos.lineNumber, newContent, ide.activeTab.path);
					}
				});

				monacoEditor.onDidChangeCursorPosition((e: any) => {
					onCursorChange?.(e.position.lineNumber, e.position.column);
				});

				monacoEditor.addCommand(
					monacoModule.KeyMod.CtrlCmd | monacoModule.KeyCode.KeyS,
					() => ide.saveFile(ide.activeTabIndex)
				);

				// Alt+Down: Jump to top prediction (neuroscience-inspired navigation)
				monacoEditor.addCommand(
					monacoModule.KeyMod.Alt | monacoModule.KeyCode.DownArrow,
					() => {
						const pred = editPredictor.acceptPrediction(0);
						if (pred && pred.file === ide.activeTab?.path) {
							monacoEditor.setPosition({ lineNumber: pred.line, column: 1 });
							monacoEditor.revealLineInCenter(pred.line);
						}
					}
				);

				// Alt+Up: Jump to second prediction
				monacoEditor.addCommand(
					monacoModule.KeyMod.Alt | monacoModule.KeyCode.UpArrow,
					() => {
						const pred = editPredictor.acceptPrediction(1);
						if (pred && pred.file === ide.activeTab?.path) {
							monacoEditor.setPosition({ lineNumber: pred.line, column: 1 });
							monacoEditor.revealLineInCenter(pred.line);
						}
					}
				);

				// Register AI inline completion provider (FIM — Fill-in-the-Middle)
				// Gutter click: toggle breakpoint
				monacoEditor.onMouseDown((e: any) => {
					if (e.target?.type === monacoModule.editor.MouseTargetType.GUTTER_GLYPH_MARGIN ||
						e.target?.type === monacoModule.editor.MouseTargetType.GUTTER_LINE_NUMBERS) {
						const line = e.target.position?.lineNumber;
						if (line) {
							const updated = new Set(breakpoints);
							if (updated.has(line)) updated.delete(line);
							else updated.add(line);
							breakpoints = updated;
							updateGutterDecorations();
						}
					}
				});

				registerAiCompletionProvider(monacoModule);
			}
		}
	});

	// React to prediction changes → update gutter + inline ghost decorations
	$effect(() => {
		const preds = editPredictor.predictions;
		if (!monacoEditor || !monacoModule) {
			if (predictionDecorations.length > 0) {
				predictionDecorations = monacoEditor?.deltaDecorations(predictionDecorations, []) ?? [];
			}
			return;
		}

		// Only show predictions for the current file
		const currentPath = ide.activeTab?.path || '';
		const filePreds = preds.filter(p => p.file === currentPath);

		if (filePreds.length === 0) {
			predictionDecorations = monacoEditor.deltaDecorations(predictionDecorations, []);
			return;
		}

		const decorations = filePreds.map((pred, i) => ({
			range: new monacoModule.Range(pred.line, 1, pred.line, 1),
			options: {
				isWholeLine: true,
				className: 'edit-prediction-line',
				glyphMarginClassName: i === 0 ? 'edit-prediction-glyph-primary' : 'edit-prediction-glyph',
				glyphMarginHoverMessage: {
					value: `**${pred.kind === 'hebbian' ? '🧠 Learned' : '⚡ Predicted'} Edit** (${Math.round(pred.confidence * 100)}%)\n\n${pred.reason}\n\nPress **Alt+↓** to jump here`,
				},
				after: i === 0 ? {
					content: `  ← predicted (${Math.round(pred.confidence * 100)}%)`,
					inlineClassName: 'edit-prediction-ghost-text',
				} : undefined,
				overviewRuler: {
					color: pred.kind === 'hebbian' ? '#c792ea33' : '#00FF6633',
					position: monacoModule.editor.OverviewRulerLane.Right,
				},
			},
		}));
		predictionDecorations = monacoEditor.deltaDecorations(predictionDecorations, decorations);
	});

	function updateGutterDecorations() {
		if (!monacoEditor || !monacoModule) return;
		const decorations = [...breakpoints].map((line) => ({
			range: new monacoModule.Range(line, 1, line, 1),
			options: {
				isWholeLine: true,
				glyphMarginClassName: 'breakpoint-glyph',
				glyphMarginHoverMessage: { value: `Breakpoint on line ${line}` },
				linesDecorationsClassName: 'breakpoint-line-decoration',
			},
		}));
		gutterDecorations = monacoEditor.deltaDecorations(gutterDecorations, decorations);
	}

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

<div class="flex-1 min-h-0 relative" style={containerStyle}>
	{#if ide.openTabs.length > 0}
		<div bind:this={editorContainer} class="absolute inset-0" style={editorAreaStyle}></div>
		<!-- AI Completion Status Bar -->
		<div class="absolute bottom-2 right-2 z-10 flex items-center gap-1.5">
			<!-- Telemetry: model + latency -->
			{#if lastModel && aiCompletionsEnabled}
				<div class="flex items-center gap-1 px-1.5 py-0.5 rounded text-[9px] bg-gx-bg-primary/90 border border-gx-border-subtle text-gx-text-disabled"
					title="Last completion: {lastModel} in {lastLatency}ms{lastFromCache ? ' (cached)' : ''}">
					{#if lastFromCache}
						<Database size={8} class="text-gx-accent-cyan" />
					{:else}
						<Zap size={8} class="text-gx-status-warning" />
					{/if}
					<span class="text-gx-text-muted">{lastModel.length > 15 ? lastModel.slice(0, 15) + '…' : lastModel}</span>
					<span class="text-gx-text-disabled">{lastLatency}ms</span>
				</div>
			{/if}

			<!-- AI Toggle -->
			<button
				onclick={() => aiCompletionsEnabled = !aiCompletionsEnabled}
				class="flex items-center gap-1 px-2 py-1 rounded text-[10px] transition-all
					{aiCompletionsEnabled
						? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
						: 'bg-gx-bg-elevated text-gx-text-disabled border border-gx-border-default'}"
				title={aiCompletionsEnabled ? 'AI Completions: ON (Tab to accept)' : 'AI Completions: OFF'}
			>
				<Sparkles size={10} />
				AI {aiCompletionsEnabled ? 'ON' : 'OFF'}
			</button>
		</div>
	{:else}
		<div class="flex flex-col items-center justify-center h-full text-gx-text-disabled gap-4">
			<Code2 size={48} class="opacity-20" />
			<div class="text-center">
				<p class="text-sm font-medium">CodeForge IDE</p>
				<p class="text-xs mt-1">Open a file from the explorer or press Ctrl+P</p>
			</div>
			<div class="flex flex-col gap-1 text-xs text-gx-text-disabled mt-2">
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+S</kbd> Save file</span>
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+P</kbd> Quick Open</span>
				<span><kbd class="px-1 py-0.5 bg-gx-bg-elevated border border-gx-border-default rounded text-[10px]">Ctrl+`</kbd> Toggle Terminal</span>
			</div>
		</div>
	{/if}
</div>

<style>
	:global(.breakpoint-glyph) {
		background: #e53e3e;
		border-radius: 50%;
		width: 10px !important;
		height: 10px !important;
		margin-top: 4px;
		margin-left: 4px;
	}
	:global(.breakpoint-line-decoration) {
		background: rgba(229, 62, 62, 0.08);
	}
	:global(.edit-prediction-glyph-primary) {
		background: #00FF66;
		border-radius: 50%;
		width: 8px !important;
		height: 8px !important;
		margin-top: 5px;
		margin-left: 5px;
		opacity: 0.7;
		box-shadow: 0 0 4px #00FF6644;
	}
	:global(.edit-prediction-glyph) {
		background: #00FF66;
		border-radius: 2px;
		width: 5px !important;
		height: 5px !important;
		margin-top: 6px;
		margin-left: 6px;
		opacity: 0.4;
	}
	:global(.edit-prediction-line) {
		background: rgba(0, 255, 102, 0.03);
	}
	:global(.edit-prediction-ghost-text) {
		color: #00FF6644 !important;
		font-style: italic;
		font-size: 11px;
	}
</style>
