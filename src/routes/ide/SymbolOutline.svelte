<script lang="ts">
	/**
	 * SymbolOutline — Document symbol tree with BenikUI integration
	 *
	 * Extracts functions, classes, interfaces, types, structs, enums from
	 * the active editor file using fast regex scanning. Works without LSP
	 * for instant responsiveness; will upgrade to LSP documentSymbol when available.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - symbol-item: Individual symbol row
	 *   - filter: Search/filter input
	 */

	import {
		Code2, Braces, Hash, Box, Shield, List, Variable,
		ChevronRight, ChevronDown, Search
	} from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Symbol {
		name: string;
		kind: 'function' | 'class' | 'interface' | 'type' | 'struct' | 'enum' | 'const' | 'method' | 'variable' | 'import';
		line: number;
		indent: number;
	}

	interface Props {
		onNavigate?: (line: number) => void;
	}

	let { onNavigate }: Props = $props();

	// BenikUI style engine
	const widgetId = 'ide-symbol-outline';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let itemComp = $derived(styleEngine.getComponentStyle(widgetId, 'symbol-item'));
	let itemStyle = $derived(hasEngineStyle && itemComp ? componentToCSS(itemComp) : '');

	let filter = $state('');
	let expandedKinds = $state<Set<string>>(new Set(['function', 'class', 'interface', 'struct', 'enum', 'type']));

	// Extract symbols from active tab content
	let symbols = $derived(extractSymbols(ide.activeTab?.content || '', ide.activeTab?.language || 'plaintext'));

	let filteredSymbols = $derived(
		filter
			? symbols.filter(s => s.name.toLowerCase().includes(filter.toLowerCase()))
			: symbols
	);

	// Group by kind
	let groupedSymbols = $derived(() => {
		const groups = new Map<string, Symbol[]>();
		for (const s of filteredSymbols) {
			const existing = groups.get(s.kind) || [];
			existing.push(s);
			groups.set(s.kind, existing);
		}
		return groups;
	});

	function extractSymbols(content: string, language: string): Symbol[] {
		if (!content) return [];
		const syms: Symbol[] = [];
		const lines = content.split('\n');

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			const trimmed = line.trim();
			const indent = line.length - line.trimStart().length;

			// TypeScript / JavaScript / Svelte
			if (['typescript', 'javascript', 'svelte', 'typescriptreact', 'javascriptreact'].includes(language)) {
				// Functions
				let m = trimmed.match(/^(?:export\s+)?(?:async\s+)?function\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'function', line: i + 1, indent }); continue; }

				// Arrow functions / const
				m = trimmed.match(/^(?:export\s+)?(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>/);
				if (m) { syms.push({ name: m[1], kind: 'function', line: i + 1, indent }); continue; }

				// Classes
				m = trimmed.match(/^(?:export\s+)?(?:abstract\s+)?class\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'class', line: i + 1, indent }); continue; }

				// Interfaces
				m = trimmed.match(/^(?:export\s+)?interface\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'interface', line: i + 1, indent }); continue; }

				// Type aliases
				m = trimmed.match(/^(?:export\s+)?type\s+(\w+)\s*[=<]/);
				if (m) { syms.push({ name: m[1], kind: 'type', line: i + 1, indent }); continue; }

				// Enums
				m = trimmed.match(/^(?:export\s+)?(?:const\s+)?enum\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'enum', line: i + 1, indent }); continue; }
			}

			// Rust
			if (language === 'rust') {
				let m = trimmed.match(/^(?:pub\s+)?(?:async\s+)?fn\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'function', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:pub\s+)?struct\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'struct', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:pub\s+)?enum\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'enum', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:pub\s+)?trait\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'interface', line: i + 1, indent }); continue; }

				m = trimmed.match(/^impl(?:<[^>]*>)?\s+(\w+)/);
				if (m) { syms.push({ name: `impl ${m[1]}`, kind: 'class', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:pub\s+)?(?:const|static)\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'const', line: i + 1, indent }); continue; }
			}

			// Python
			if (language === 'python') {
				let m = trimmed.match(/^(?:async\s+)?def\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: indent > 0 ? 'method' : 'function', line: i + 1, indent }); continue; }

				m = trimmed.match(/^class\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'class', line: i + 1, indent }); continue; }
			}

			// Go
			if (language === 'go') {
				let m = trimmed.match(/^func\s+(?:\([^)]*\)\s+)?(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'function', line: i + 1, indent }); continue; }

				m = trimmed.match(/^type\s+(\w+)\s+struct/);
				if (m) { syms.push({ name: m[1], kind: 'struct', line: i + 1, indent }); continue; }

				m = trimmed.match(/^type\s+(\w+)\s+interface/);
				if (m) { syms.push({ name: m[1], kind: 'interface', line: i + 1, indent }); continue; }
			}

			// C# / Java
			if (['csharp', 'java'].includes(language)) {
				let m = trimmed.match(/(?:public|private|protected|internal|static|async|override|virtual|abstract)\s+.*?\s+(\w+)\s*\(/);
				if (m && !['if', 'while', 'for', 'switch', 'catch'].includes(m[1])) {
					syms.push({ name: m[1], kind: 'method', line: i + 1, indent });
					continue;
				}

				m = trimmed.match(/^(?:public|internal|private)?\s*(?:static\s+)?(?:abstract\s+)?class\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'class', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:public|internal)?\s*interface\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'interface', line: i + 1, indent }); continue; }

				m = trimmed.match(/^(?:public|internal)?\s*enum\s+(\w+)/);
				if (m) { syms.push({ name: m[1], kind: 'enum', line: i + 1, indent }); continue; }
			}
		}

		return syms;
	}

	function kindIcon(kind: string) {
		switch (kind) {
			case 'function': case 'method': return { icon: Code2, color: 'text-gx-accent-blue' };
			case 'class': return { icon: Box, color: 'text-gx-accent-magenta' };
			case 'interface': return { icon: Shield, color: 'text-gx-accent-cyan' };
			case 'type': return { icon: Hash, color: 'text-gx-status-warning' };
			case 'struct': return { icon: Braces, color: 'text-gx-accent-magenta' };
			case 'enum': return { icon: List, color: 'text-gx-accent-cyan' };
			case 'const': case 'variable': return { icon: Variable, color: 'text-gx-neon' };
			default: return { icon: Code2, color: 'text-gx-text-muted' };
		}
	}

	function toggleKind(kind: string) {
		const updated = new Set(expandedKinds);
		if (updated.has(kind)) updated.delete(kind);
		else updated.add(kind);
		expandedKinds = updated;
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Filter -->
	<div class="px-2 py-1.5 border-b border-gx-border-subtle shrink-0">
		<div class="flex items-center gap-1.5 px-2 py-1 bg-gx-bg-secondary border border-gx-border-default rounded focus-within:border-gx-neon transition-colors">
			<Search size={11} class="text-gx-text-muted shrink-0" />
			<input
				bind:value={filter}
				placeholder="Filter symbols..."
				class="flex-1 bg-transparent text-xs text-gx-text-primary placeholder:text-gx-text-disabled outline-none"
			/>
		</div>
		<p class="text-[10px] text-gx-text-disabled mt-1 px-1">{symbols.length} symbol{symbols.length !== 1 ? 's' : ''}</p>
	</div>

	<!-- Symbol tree -->
	<div class="flex-1 overflow-auto text-xs">
		{#if symbols.length > 0}
			{#each [...groupedSymbols().entries()] as [kind, syms]}
				{@const ki = kindIcon(kind)}
				<button
					onclick={() => toggleKind(kind)}
					class="flex items-center gap-1 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover"
				>
					{#if expandedKinds.has(kind)}
						<ChevronDown size={11} />
					{:else}
						<ChevronRight size={11} />
					{/if}
					<ki.icon size={11} class={ki.color} />
					<span class="font-medium capitalize">{kind}s</span>
					<span class="ml-auto text-gx-text-disabled">{syms.length}</span>
				</button>
				{#if expandedKinds.has(kind)}
					{#each syms as sym}
						{@const symKi = kindIcon(sym.kind)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							onclick={() => onNavigate?.(sym.line)}
							onkeydown={(e) => e.key === 'Enter' && onNavigate?.(sym.line)}
							role="button"
							tabindex="0"
							class="flex items-center gap-1.5 px-4 py-1 hover:bg-gx-bg-hover cursor-pointer"
							style={itemStyle}
						>
							<symKi.icon size={10} class={symKi.color} />
							<span class="text-gx-text-secondary truncate flex-1">{sym.name}</span>
							<span class="text-gx-text-disabled font-mono text-[10px]">:{sym.line}</span>
						</div>
					{/each}
				{/if}
			{/each}
		{:else}
			<div class="p-4 text-center text-gx-text-disabled">
				<Code2 size={24} class="mx-auto mb-2 opacity-30" />
				<p>No symbols found</p>
				<p class="text-[10px] mt-1">Open a file to see its outline</p>
			</div>
		{/if}
	</div>
</div>
