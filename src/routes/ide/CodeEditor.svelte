<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Code2, Sparkles } from '@lucide/svelte';
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
	 * AI Inline Completion Provider — Ollama FIM (Fill-in-the-Middle)
	 *
	 * Sends code context (prefix + suffix) to the local Ollama model
	 * and returns ghost text suggestions. Debounced at 500ms to avoid
	 * API spam during rapid typing.
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
							const result = await invoke<{ text: string; insert_text: string }>('ai_complete', {
								request: {
									file_path: ide.activeTab?.path || '',
									language: ide.activeTab?.language || 'plaintext',
									prefix: textUntilPosition.slice(-3000),
									suffix: textAfterPosition.slice(0, 1000),
									line: position.lineNumber,
									column: position.column,
								}
							});

							if (!result.insert_text || token.isCancellationRequested) {
								resolve({ items: [] });
								return;
							}

							resolve({
								items: [{
									insertText: result.insert_text,
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
		<!-- AI Completion Toggle -->
		<button
			onclick={() => aiCompletionsEnabled = !aiCompletionsEnabled}
			class="absolute bottom-2 right-2 z-10 flex items-center gap-1 px-2 py-1 rounded text-[10px] transition-all
				{aiCompletionsEnabled
					? 'bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30'
					: 'bg-white/5 text-white/30 border border-white/10'}"
			title={aiCompletionsEnabled ? 'AI Completions: ON (Tab to accept)' : 'AI Completions: OFF'}
		>
			<Sparkles size={10} />
			AI {aiCompletionsEnabled ? 'ON' : 'OFF'}
		</button>
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
