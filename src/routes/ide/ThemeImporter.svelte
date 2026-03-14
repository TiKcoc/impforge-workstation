<script lang="ts">
	import {
		Palette, Upload, Check, ChevronDown, Sun, Moon,
		Import, Paintbrush, X, Copy, Eye
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'ide-theme-importer';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let previewComp = $derived(styleEngine.getComponentStyle(widgetId, 'preview'));
	let previewStyle = $derived(hasEngineStyle && previewComp ? componentToCSS(previewComp) : '');
	let controlsComp = $derived(styleEngine.getComponentStyle(widgetId, 'controls'));
	let controlsStyle = $derived(hasEngineStyle && controlsComp ? componentToCSS(controlsComp) : '');

	// --- Types ---

	interface ThemeColors {
		'editor.background': string;
		'editor.foreground': string;
		'editor.lineHighlightBackground': string;
		'editor.selectionBackground': string;
		'editorCursor.foreground': string;
		'editorLineNumber.foreground': string;
		'editorLineNumber.activeForeground': string;
	}

	interface TokenRule {
		token: string;
		foreground: string;
		fontStyle?: string;
	}

	interface ThemeDef {
		id: string;
		name: string;
		type: 'dark' | 'light';
		colors: ThemeColors;
		rules: TokenRule[];
	}

	interface Props {
		onApply?: (theme: ThemeDef) => void;
	}

	let { onApply }: Props = $props();

	// --- Built-in themes ---

	const builtInThemes: ThemeDef[] = [
		{
			id: 'impforge-dark', name: 'ImpForge Dark (Default)', type: 'dark',
			colors: {
				'editor.background': '#0a0e14',
				'editor.foreground': '#e0e0e0',
				'editor.lineHighlightBackground': '#141820',
				'editor.selectionBackground': '#1a3a5c',
				'editorCursor.foreground': '#00FF66',
				'editorLineNumber.foreground': '#3a4a5a',
				'editorLineNumber.activeForeground': '#00FF66',
			},
			rules: [
				{ token: 'comment', foreground: '#5a6a7a', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#c792ea' },
				{ token: 'string', foreground: '#c3e88d' },
				{ token: 'number', foreground: '#f78c6c' },
				{ token: 'type', foreground: '#ffcb6b' },
				{ token: 'function', foreground: '#82aaff' },
				{ token: 'variable', foreground: '#eeffff' },
				{ token: 'operator', foreground: '#89ddff' },
			],
		},
		{
			id: 'impforge-light', name: 'ImpForge Light', type: 'light',
			colors: {
				'editor.background': '#fafafa',
				'editor.foreground': '#383a42',
				'editor.lineHighlightBackground': '#f0f0f0',
				'editor.selectionBackground': '#d7d8dc',
				'editorCursor.foreground': '#526fff',
				'editorLineNumber.foreground': '#9d9d9f',
				'editorLineNumber.activeForeground': '#383a42',
			},
			rules: [
				{ token: 'comment', foreground: '#a0a1a7', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#a626a4' },
				{ token: 'string', foreground: '#50a14f' },
				{ token: 'number', foreground: '#986801' },
				{ token: 'type', foreground: '#c18401' },
				{ token: 'function', foreground: '#4078f2' },
				{ token: 'variable', foreground: '#383a42' },
				{ token: 'operator', foreground: '#0184bc' },
			],
		},
		{
			id: 'monokai', name: 'Monokai Pro', type: 'dark',
			colors: {
				'editor.background': '#2d2a2e',
				'editor.foreground': '#fcfcfa',
				'editor.lineHighlightBackground': '#363337',
				'editor.selectionBackground': '#5b595c',
				'editorCursor.foreground': '#fcfcfa',
				'editorLineNumber.foreground': '#5b595c',
				'editorLineNumber.activeForeground': '#c1c0c0',
			},
			rules: [
				{ token: 'comment', foreground: '#727072', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#ff6188' },
				{ token: 'string', foreground: '#ffd866' },
				{ token: 'number', foreground: '#ab9df2' },
				{ token: 'type', foreground: '#78dce8' },
				{ token: 'function', foreground: '#a9dc76' },
				{ token: 'variable', foreground: '#fcfcfa' },
				{ token: 'operator', foreground: '#ff6188' },
			],
		},
		{
			id: 'dracula', name: 'Dracula', type: 'dark',
			colors: {
				'editor.background': '#282a36',
				'editor.foreground': '#f8f8f2',
				'editor.lineHighlightBackground': '#44475a',
				'editor.selectionBackground': '#44475a',
				'editorCursor.foreground': '#f8f8f2',
				'editorLineNumber.foreground': '#6272a4',
				'editorLineNumber.activeForeground': '#f8f8f2',
			},
			rules: [
				{ token: 'comment', foreground: '#6272a4', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#ff79c6' },
				{ token: 'string', foreground: '#f1fa8c' },
				{ token: 'number', foreground: '#bd93f9' },
				{ token: 'type', foreground: '#8be9fd', fontStyle: 'italic' },
				{ token: 'function', foreground: '#50fa7b' },
				{ token: 'variable', foreground: '#f8f8f2' },
				{ token: 'operator', foreground: '#ff79c6' },
			],
		},
		{
			id: 'nord', name: 'Nord', type: 'dark',
			colors: {
				'editor.background': '#2e3440',
				'editor.foreground': '#d8dee9',
				'editor.lineHighlightBackground': '#3b4252',
				'editor.selectionBackground': '#434c5e',
				'editorCursor.foreground': '#d8dee9',
				'editorLineNumber.foreground': '#4c566a',
				'editorLineNumber.activeForeground': '#d8dee9',
			},
			rules: [
				{ token: 'comment', foreground: '#616e88', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#81a1c1' },
				{ token: 'string', foreground: '#a3be8c' },
				{ token: 'number', foreground: '#b48ead' },
				{ token: 'type', foreground: '#8fbcbb' },
				{ token: 'function', foreground: '#88c0d0' },
				{ token: 'variable', foreground: '#d8dee9' },
				{ token: 'operator', foreground: '#81a1c1' },
			],
		},
		{
			id: 'github-dark', name: 'GitHub Dark', type: 'dark',
			colors: {
				'editor.background': '#0d1117',
				'editor.foreground': '#c9d1d9',
				'editor.lineHighlightBackground': '#161b22',
				'editor.selectionBackground': '#264f78',
				'editorCursor.foreground': '#c9d1d9',
				'editorLineNumber.foreground': '#484f58',
				'editorLineNumber.activeForeground': '#c9d1d9',
			},
			rules: [
				{ token: 'comment', foreground: '#8b949e', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#ff7b72' },
				{ token: 'string', foreground: '#a5d6ff' },
				{ token: 'number', foreground: '#79c0ff' },
				{ token: 'type', foreground: '#ffa657' },
				{ token: 'function', foreground: '#d2a8ff' },
				{ token: 'variable', foreground: '#c9d1d9' },
				{ token: 'operator', foreground: '#ff7b72' },
			],
		},
		{
			id: 'catppuccin-mocha', name: 'Catppuccin Mocha', type: 'dark',
			colors: {
				'editor.background': '#1e1e2e',
				'editor.foreground': '#cdd6f4',
				'editor.lineHighlightBackground': '#2a2b3d',
				'editor.selectionBackground': '#45475a',
				'editorCursor.foreground': '#f5e0dc',
				'editorLineNumber.foreground': '#45475a',
				'editorLineNumber.activeForeground': '#cdd6f4',
			},
			rules: [
				{ token: 'comment', foreground: '#6c7086', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#cba6f7' },
				{ token: 'string', foreground: '#a6e3a1' },
				{ token: 'number', foreground: '#fab387' },
				{ token: 'type', foreground: '#f9e2af' },
				{ token: 'function', foreground: '#89b4fa' },
				{ token: 'variable', foreground: '#cdd6f4' },
				{ token: 'operator', foreground: '#89dceb' },
			],
		},
		{
			id: 'one-dark-pro', name: 'One Dark Pro', type: 'dark',
			colors: {
				'editor.background': '#282c34',
				'editor.foreground': '#abb2bf',
				'editor.lineHighlightBackground': '#2c313a',
				'editor.selectionBackground': '#3e4451',
				'editorCursor.foreground': '#528bff',
				'editorLineNumber.foreground': '#495162',
				'editorLineNumber.activeForeground': '#abb2bf',
			},
			rules: [
				{ token: 'comment', foreground: '#5c6370', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#c678dd' },
				{ token: 'string', foreground: '#98c379' },
				{ token: 'number', foreground: '#d19a66' },
				{ token: 'type', foreground: '#e5c07b' },
				{ token: 'function', foreground: '#61afef' },
				{ token: 'variable', foreground: '#e06c75' },
				{ token: 'operator', foreground: '#56b6c2' },
			],
		},
		{
			id: 'tokyo-night', name: 'Tokyo Night', type: 'dark',
			colors: {
				'editor.background': '#1a1b26',
				'editor.foreground': '#a9b1d6',
				'editor.lineHighlightBackground': '#1e202e',
				'editor.selectionBackground': '#33467c',
				'editorCursor.foreground': '#c0caf5',
				'editorLineNumber.foreground': '#3b4261',
				'editorLineNumber.activeForeground': '#737aa2',
			},
			rules: [
				{ token: 'comment', foreground: '#565f89', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#9d7cd8' },
				{ token: 'string', foreground: '#9ece6a' },
				{ token: 'number', foreground: '#ff9e64' },
				{ token: 'type', foreground: '#2ac3de' },
				{ token: 'function', foreground: '#7aa2f7' },
				{ token: 'variable', foreground: '#c0caf5' },
				{ token: 'operator', foreground: '#89ddff' },
			],
		},
		{
			id: 'solarized-dark', name: 'Solarized Dark', type: 'dark',
			colors: {
				'editor.background': '#002b36',
				'editor.foreground': '#839496',
				'editor.lineHighlightBackground': '#073642',
				'editor.selectionBackground': '#073642',
				'editorCursor.foreground': '#839496',
				'editorLineNumber.foreground': '#586e75',
				'editorLineNumber.activeForeground': '#93a1a1',
			},
			rules: [
				{ token: 'comment', foreground: '#586e75', fontStyle: 'italic' },
				{ token: 'keyword', foreground: '#859900' },
				{ token: 'string', foreground: '#2aa198' },
				{ token: 'number', foreground: '#d33682' },
				{ token: 'type', foreground: '#b58900' },
				{ token: 'function', foreground: '#268bd2' },
				{ token: 'variable', foreground: '#839496' },
				{ token: 'operator', foreground: '#859900' },
			],
		},
	];

	// --- State ---

	let selectedThemeId = $state('impforge-dark');
	let importJson = $state('');
	let importError = $state('');
	let importedTheme = $state<ThemeDef | null>(null);
	let showImport = $state(false);
	let applied = $state(false);
	let dragOver = $state(false);

	// --- Derived ---

	const selectedTheme = $derived(
		importedTheme && selectedThemeId === importedTheme.id
			? importedTheme
			: builtInThemes.find((t) => t.id === selectedThemeId) ?? builtInThemes[0]
	);

	const allThemes = $derived.by(() => {
		const themes = [...builtInThemes];
		const imported = importedTheme;
		if (imported && !themes.some((t) => t.id === imported.id)) {
			themes.push(imported);
		}
		return themes;
	});

	/** Six preview colors for the theme card swatches */
	function getSwatchColors(theme: ThemeDef): string[] {
		return [
			theme.colors['editor.background'],
			theme.colors['editor.foreground'],
			theme.colors['editorCursor.foreground'],
			theme.rules.find((r) => r.token === 'keyword')?.foreground ?? '#888',
			theme.rules.find((r) => r.token === 'string')?.foreground ?? '#888',
			theme.rules.find((r) => r.token === 'function')?.foreground ?? '#888',
		];
	}

	/** Full preview grid: 4x3 of all important colors */
	function getPreviewGrid(theme: ThemeDef): Array<{ label: string; color: string }> {
		return [
			{ label: 'Background', color: theme.colors['editor.background'] },
			{ label: 'Foreground', color: theme.colors['editor.foreground'] },
			{ label: 'Cursor', color: theme.colors['editorCursor.foreground'] },
			{ label: 'Selection', color: theme.colors['editor.selectionBackground'] },
			{ label: 'Keyword', color: theme.rules.find((r) => r.token === 'keyword')?.foreground ?? '#888' },
			{ label: 'String', color: theme.rules.find((r) => r.token === 'string')?.foreground ?? '#888' },
			{ label: 'Number', color: theme.rules.find((r) => r.token === 'number')?.foreground ?? '#888' },
			{ label: 'Function', color: theme.rules.find((r) => r.token === 'function')?.foreground ?? '#888' },
			{ label: 'Type', color: theme.rules.find((r) => r.token === 'type')?.foreground ?? '#888' },
			{ label: 'Comment', color: theme.rules.find((r) => r.token === 'comment')?.foreground ?? '#888' },
			{ label: 'Variable', color: theme.rules.find((r) => r.token === 'variable')?.foreground ?? '#888' },
			{ label: 'Operator', color: theme.rules.find((r) => r.token === 'operator')?.foreground ?? '#888' },
		];
	}

	// --- VS Code theme import ---

	// Map common VS Code color keys to our ThemeColors keys
	const vscodeColorMap: Record<string, keyof ThemeColors> = {
		'editor.background': 'editor.background',
		'editor.foreground': 'editor.foreground',
		'editor.lineHighlightBackground': 'editor.lineHighlightBackground',
		'editor.selectionBackground': 'editor.selectionBackground',
		'editorCursor.foreground': 'editorCursor.foreground',
		'editorLineNumber.foreground': 'editorLineNumber.foreground',
		'editorLineNumber.activeForeground': 'editorLineNumber.activeForeground',
	};

	// Map VS Code scope names to Monaco token names
	const scopeToToken: Record<string, string> = {
		'comment': 'comment',
		'comment.line': 'comment',
		'comment.block': 'comment',
		'keyword': 'keyword',
		'keyword.control': 'keyword',
		'keyword.operator': 'operator',
		'storage.type': 'keyword',
		'storage.modifier': 'keyword',
		'string': 'string',
		'string.quoted': 'string',
		'string.quoted.double': 'string',
		'string.quoted.single': 'string',
		'constant.numeric': 'number',
		'constant.language': 'number',
		'entity.name.type': 'type',
		'support.type': 'type',
		'entity.name.function': 'function',
		'support.function': 'function',
		'variable': 'variable',
		'variable.other': 'variable',
		'variable.parameter': 'variable',
		'entity.name.tag': 'tag',
		'entity.other.attribute-name': 'attribute.name',
		'punctuation': 'delimiter',
		'meta.embedded': 'variable',
	};

	function parseVSCodeTheme(jsonStr: string): ThemeDef | null {
		importError = '';
		let parsed: any;
		try {
			parsed = JSON.parse(jsonStr);
		} catch {
			importError = 'Invalid JSON format';
			return null;
		}

		// Determine theme type
		const themeType: 'dark' | 'light' =
			parsed.type === 'light' ? 'light' : 'dark';

		// Extract editor colors
		const vsColors = parsed.colors || {};
		const colors: ThemeColors = {
			'editor.background': vsColors['editor.background'] || (themeType === 'dark' ? '#1e1e1e' : '#ffffff'),
			'editor.foreground': vsColors['editor.foreground'] || (themeType === 'dark' ? '#d4d4d4' : '#333333'),
			'editor.lineHighlightBackground': vsColors['editor.lineHighlightBackground'] || vsColors['editor.background'] || '#2a2a2a',
			'editor.selectionBackground': vsColors['editor.selectionBackground'] || '#264f78',
			'editorCursor.foreground': vsColors['editorCursor.foreground'] || vsColors['editor.foreground'] || '#ffffff',
			'editorLineNumber.foreground': vsColors['editorLineNumber.foreground'] || '#858585',
			'editorLineNumber.activeForeground': vsColors['editorLineNumber.activeForeground'] || vsColors['editor.foreground'] || '#c6c6c6',
		};

		// Extract token rules from tokenColors
		const tokenColors = parsed.tokenColors || [];
		const seenTokens = new Set<string>();
		const rules: TokenRule[] = [];

		for (const tc of tokenColors) {
			if (!tc.settings?.foreground) continue;

			const scopes: string[] = Array.isArray(tc.scope)
				? tc.scope
				: typeof tc.scope === 'string'
					? tc.scope.split(',').map((s: string) => s.trim())
					: [];

			for (const scope of scopes) {
				// Try direct match first, then prefix match
				const token = scopeToToken[scope]
					?? scopeToToken[scope.split('.')[0]]
					?? null;

				if (token && !seenTokens.has(token)) {
					seenTokens.add(token);
					const rule: TokenRule = {
						token,
						foreground: tc.settings.foreground,
					};
					if (tc.settings.fontStyle) {
						rule.fontStyle = tc.settings.fontStyle;
					}
					rules.push(rule);
				}
			}
		}

		// Ensure minimum token rules exist
		if (rules.length === 0) {
			importError = 'No valid tokenColors found in theme';
			return null;
		}

		const themeName = parsed.name || 'Imported Theme';
		const themeId = 'imported-' + themeName.toLowerCase().replace(/[^a-z0-9]+/g, '-');

		return {
			id: themeId,
			name: themeName,
			type: themeType,
			colors,
			rules,
		};
	}

	function handleImport() {
		if (!importJson.trim()) {
			importError = 'Paste VS Code theme JSON above';
			return;
		}
		const theme = parseVSCodeTheme(importJson);
		if (theme) {
			importedTheme = theme;
			selectedThemeId = theme.id;
			importJson = '';
			showImport = false;
			importError = '';
		}
	}

	function handleFileDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		const file = e.dataTransfer?.files?.[0];
		if (!file || !file.name.endsWith('.json')) {
			importError = 'Please drop a .json file';
			return;
		}
		const reader = new FileReader();
		reader.onload = () => {
			if (typeof reader.result === 'string') {
				importJson = reader.result;
				handleImport();
			}
		};
		reader.readAsText(file);
	}

	function handleFileInput(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const reader = new FileReader();
		reader.onload = () => {
			if (typeof reader.result === 'string') {
				importJson = reader.result;
				handleImport();
			}
		};
		reader.readAsText(file);
		// Reset so the same file can be re-selected
		input.value = '';
	}

	// --- Apply ---

	function applyTheme() {
		applied = true;
		onApply?.(selectedTheme);
		setTimeout(() => { applied = false; }, 1500);
	}

	function selectTheme(id: string) {
		selectedThemeId = id;
		applied = false;
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-secondary'} overflow-hidden text-xs" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-2 px-3 py-2 border-b border-gx-border-subtle shrink-0" style={controlsStyle}>
		<Palette size={14} class="text-gx-neon" />
		<span class="text-xs font-semibold text-gx-text-primary">Theme</span>
		<div class="flex-1"></div>
		<button
			onclick={() => showImport = !showImport}
			class="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] transition-colors
				{showImport ? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30' : 'text-gx-text-muted hover:text-gx-text-secondary border border-gx-border-default'}"
			title="Import VS Code theme"
		>
			<Import size={11} />
			Import
		</button>
	</div>

	<div class="flex-1 overflow-auto">
		<!-- Import Section (collapsible) -->
		{#if showImport}
			<div class="p-3 border-b border-gx-border-subtle space-y-2">
				<div class="text-[11px] text-gx-text-muted font-medium">Import VS Code Theme</div>

				<!-- Drop zone / paste area -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					ondragover={(e) => { e.preventDefault(); dragOver = true; }}
					ondragleave={() => dragOver = false}
					ondrop={handleFileDrop}
					class="relative border border-dashed rounded p-3 text-center transition-colors
						{dragOver ? 'border-gx-neon bg-gx-neon/5' : 'border-gx-border-default hover:border-gx-border-hover'}"
				>
					<Upload size={16} class="mx-auto text-gx-text-disabled mb-1" />
					<div class="text-[11px] text-gx-text-disabled">Drop .json file or</div>
					<label class="text-[11px] text-gx-neon cursor-pointer hover:underline">
						browse
						<input
							type="file"
							accept=".json"
							onchange={handleFileInput}
							class="hidden"
						/>
					</label>
				</div>

				<div class="text-[10px] text-gx-text-disabled text-center">or paste JSON below</div>

				<textarea
					bind:value={importJson}
					placeholder={'{"name": "My Theme", "type": "dark", "colors": {...}, "tokenColors": [...]}'}
					rows="4"
					class="w-full bg-gx-bg-secondary border border-gx-border-default rounded px-2 py-1.5 text-[11px] text-gx-text-primary placeholder:text-gx-text-disabled resize-none outline-none focus:border-gx-neon/50 font-mono transition-colors"
				></textarea>

				{#if importError}
					<div class="text-[11px] text-gx-status-error">{importError}</div>
				{/if}

				<div class="flex gap-2">
					<button
						onclick={handleImport}
						disabled={!importJson.trim()}
						class="flex items-center gap-1 px-3 py-1 rounded text-[11px] bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
					>
						<Eye size={11} />
						Preview
					</button>
					<button
						onclick={() => { showImport = false; importJson = ''; importError = ''; }}
						class="px-3 py-1 rounded text-[11px] text-gx-text-muted hover:text-gx-text-secondary border border-gx-border-default transition-colors"
					>
						Cancel
					</button>
				</div>
			</div>
		{/if}

		<!-- Theme List -->
		<div class="p-2 space-y-1">
			{#each allThemes as theme}
				{@const isSelected = theme.id === selectedThemeId}
				{@const swatches = getSwatchColors(theme)}
				<button
					onclick={() => selectTheme(theme.id)}
					class="flex items-center gap-2 w-full px-2 py-1.5 rounded text-left transition-all
						{isSelected
							? 'bg-gx-neon/5 border border-gx-neon/40'
							: 'border border-transparent hover:bg-gx-bg-hover hover:border-gx-border-default'}"
				>
					<!-- Type icon -->
					{#if theme.type === 'dark'}
						<Moon size={12} class={isSelected ? 'text-gx-neon' : 'text-gx-text-disabled'} />
					{:else}
						<Sun size={12} class={isSelected ? 'text-gx-neon' : 'text-gx-text-disabled'} />
					{/if}

					<!-- Name -->
					<span class="flex-1 text-[11px] truncate {isSelected ? 'text-gx-text-primary' : 'text-gx-text-secondary'}">
						{theme.name}
					</span>

					<!-- Color swatches -->
					<div class="flex gap-0.5 shrink-0">
						{#each swatches as color}
							<div
								class="w-3 h-3 rounded-sm border border-gx-border-default"
								style="background-color: {color}"
								title={color}
							></div>
						{/each}
					</div>

					{#if isSelected}
						<Check size={12} class="text-gx-neon shrink-0" />
					{/if}
				</button>
			{/each}
		</div>

		<!-- Selected Theme Preview -->
		<div class="px-3 pb-3 space-y-3" style={previewStyle}>
			<div class="text-[11px] text-gx-text-muted font-medium flex items-center gap-1.5">
				<Paintbrush size={11} />
				Preview: {selectedTheme.name}
			</div>

			<!-- Color grid (4x3) -->
			<div class="grid grid-cols-4 gap-1.5">
				{#each getPreviewGrid(selectedTheme) as swatch}
					<div class="flex flex-col items-center gap-0.5">
						<div
							class="w-full h-6 rounded border border-gx-border-default"
							style="background-color: {swatch.color}"
							title="{swatch.label}: {swatch.color}"
						></div>
						<span class="text-[9px] text-gx-text-disabled truncate w-full text-center">{swatch.label}</span>
					</div>
				{/each}
			</div>

			<!-- Code preview mock -->
			<div
				class="rounded border border-gx-border-default p-3 font-mono text-[11px] leading-relaxed"
				style="background-color: {selectedTheme.colors['editor.background']}"
			>
				<div>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'comment')?.foreground ?? '#666'}; font-style: italic">// Theme preview</span>
				</div>
				<div>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'keyword')?.foreground ?? '#c792ea'}">fn</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'function')?.foreground ?? '#82aaff'}"> main</span>
					<span style="color: {selectedTheme.colors['editor.foreground']}">()</span>
					<span style="color: {selectedTheme.colors['editor.foreground']}"> {'{'}</span>
				</div>
				<div style="background-color: {selectedTheme.colors['editor.lineHighlightBackground']}; margin: 0 -12px; padding: 0 12px;">
					<span style="color: {selectedTheme.rules.find(r => r.token === 'keyword')?.foreground ?? '#c792ea'}">    let</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'variable')?.foreground ?? '#eeffff'}"> x</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'operator')?.foreground ?? '#89ddff'}"> =</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'number')?.foreground ?? '#f78c6c'}"> 42</span>
					<span style="color: {selectedTheme.colors['editor.foreground']}">;</span>
				</div>
				<div>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'keyword')?.foreground ?? '#c792ea'}">    let</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'variable')?.foreground ?? '#eeffff'}"> msg</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'operator')?.foreground ?? '#89ddff'}"> =</span>
					<span style="color: {selectedTheme.rules.find(r => r.token === 'string')?.foreground ?? '#c3e88d'}"> "hello"</span>
					<span style="color: {selectedTheme.colors['editor.foreground']}">;</span>
				</div>
				<div>
					<span style="color: {selectedTheme.colors['editor.foreground']}">{'}'}</span>
				</div>
			</div>

			<!-- Apply button -->
			<button
				onclick={applyTheme}
				class="flex items-center justify-center gap-1.5 w-full py-2 rounded text-[11px] font-medium transition-all
					{applied
						? 'bg-gx-neon/20 text-gx-neon border border-gx-neon/40'
						: 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20'}"
			>
				{#if applied}
					<Check size={12} />
					Applied
				{:else}
					<Paintbrush size={12} />
					Apply Theme
				{/if}
			</button>
		</div>
	</div>
</div>
