<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Plus, X } from '@lucide/svelte';
	import { terminalStore } from '$lib/stores/terminal.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'ide-terminal';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let headerComp = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let headerStyle = $derived(hasEngineStyle && headerComp ? componentToCSS(headerComp) : '');
	let tabBarComp = $derived(styleEngine.getComponentStyle(widgetId, 'tab-bar'));
	let tabBarStyle = $derived(hasEngineStyle && tabBarComp ? componentToCSS(tabBarComp) : '');
	let terminalAreaComp = $derived(styleEngine.getComponentStyle(widgetId, 'terminal-area'));
	let terminalAreaStyle = $derived(hasEngineStyle && terminalAreaComp ? componentToCSS(terminalAreaComp) : '');

	let terminalContainer = $state<HTMLDivElement>(undefined!);
	let terminal: any = null;
	let fitAddon: any = null;
	let terminalInstances = new Map<number, any>();
	let fitAddons = new Map<number, any>();
	let resizeObserver: ResizeObserver | null = null;

	// Cache module references
	let TerminalClass: any = null;
	let FitAddonClass: any = null;
	let WebLinksAddonClass: any = null;
	let SearchAddonClass: any = null;

	onMount(async () => {
		const xtermModule = await import('@xterm/xterm');
		const fitModule = await import('@xterm/addon-fit');
		const webLinksModule = await import('@xterm/addon-web-links');
		const searchModule = await import('@xterm/addon-search');

		TerminalClass = xtermModule.Terminal;
		FitAddonClass = fitModule.FitAddon;
		WebLinksAddonClass = webLinksModule.WebLinksAddon;
		SearchAddonClass = searchModule.SearchAddon;

		// Set up data callback
		terminalStore.onData = (id: number, data: string) => {
			const term = terminalInstances.get(id);
			if (term) term.write(data);
		};

		terminalStore.onExit = (id: number) => {
			const term = terminalInstances.get(id);
			if (term) {
				term.writeln('\r\n\x1b[90m[Process exited]\x1b[0m');
			}
		};

		// Spawn first terminal
		const ptyId = await terminalStore.spawn();
		createTerminalInstance(ptyId);

		// Auto-resize on container change
		resizeObserver = new ResizeObserver(() => {
			const activeTab = terminalStore.activeTab;
			if (activeTab) {
				const fit = fitAddons.get(activeTab.ptyId);
				if (fit) {
					fit.fit();
					const dims = fit.proposeDimensions();
					if (dims) {
						terminalStore.resize(activeTab.ptyId, dims.cols, dims.rows);
					}
				}
			}
		});
		resizeObserver.observe(terminalContainer);
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		terminalStore.killAll();
		for (const term of terminalInstances.values()) {
			term.dispose();
		}
	});

	function createTerminalInstance(ptyId: number) {
		if (!TerminalClass) return;

		const term = new TerminalClass({
			cursorBlink: true,
			cursorStyle: 'bar',
			fontSize: 13,
			fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
			theme: {
				background: '#0D0D0D',      // gx-bg-primary
				foreground: '#B3B3B3',      // gx-text-secondary
				cursor: '#00FF66',          // gx-neon
				cursorAccent: '#0D0D0D',    // gx-bg-primary
				selectionBackground: '#252525', // gx-bg-hover
				black: '#1A1A1A',           // gx-bg-tertiary
				red: '#FF3366',             // gx-status-error
				green: '#00FF66',           // gx-neon
				yellow: '#FFCC00',          // gx-status-warning
				blue: '#3366FF',            // gx-accent-blue
				magenta: '#9933FF',         // gx-accent-purple
				cyan: '#00FFFF',            // gx-accent-cyan
				white: '#FFFFFF'            // gx-text-primary
			},
			allowProposedApi: true
		});

		const fit = new FitAddonClass();
		const webLinks = new WebLinksAddonClass();
		const search = new SearchAddonClass();

		term.loadAddon(fit);
		term.loadAddon(webLinks);
		term.loadAddon(search);

		term.open(terminalContainer);
		fit.fit();

		term.onData((data: string) => {
			terminalStore.write(ptyId, data);
		});

		terminalInstances.set(ptyId, term);
		fitAddons.set(ptyId, fit);
		terminal = term;
		fitAddon = fit;

		// Resize after mount
		requestAnimationFrame(() => {
			fit.fit();
			const dims = fit.proposeDimensions();
			if (dims) {
				terminalStore.resize(ptyId, dims.cols, dims.rows);
			}
		});
	}

	async function addTerminal() {
		const ptyId = await terminalStore.spawn();
		// Hide all other terminals
		for (const [id, term] of terminalInstances) {
			term.element?.parentElement && (term.element.style.display = 'none');
		}
		createTerminalInstance(ptyId);
	}

	function switchTab(index: number) {
		terminalStore.activeIndex = index;
		const tab = terminalStore.tabs[index];
		if (!tab) return;

		// Show only the active terminal
		for (const [id, term] of terminalInstances) {
			if (term.element) {
				term.element.style.display = id === tab.ptyId ? '' : 'none';
			}
		}

		const fit = fitAddons.get(tab.ptyId);
		if (fit) {
			requestAnimationFrame(() => fit.fit());
		}
	}

	async function closeTab(index: number) {
		const tab = terminalStore.tabs[index];
		if (!tab) return;

		const term = terminalInstances.get(tab.ptyId);
		if (term) {
			term.dispose();
			terminalInstances.delete(tab.ptyId);
			fitAddons.delete(tab.ptyId);
		}

		await terminalStore.kill(tab.ptyId);
	}
</script>

<div class="flex h-full flex-col" style={containerStyle}>
	<!-- Terminal tabs -->
	<div class="flex items-center gap-1 border-b border-gx-border-default {hasEngineStyle ? '' : 'bg-gx-bg-primary'} px-2 py-1" style={headerStyle}>
		{#each terminalStore.tabs as tab, i}
			<div
				role="tab"
				tabindex="0"
				aria-selected={terminalStore.activeIndex === i}
				class="group flex items-center gap-1.5 rounded px-2 py-0.5 text-xs transition-colors cursor-pointer {terminalStore.activeIndex === i
					? 'bg-gx-bg-hover text-gx-neon'
					: 'text-gx-text-muted hover:text-gx-text-primary'}"
				onclick={() => switchTab(i)}
				onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') switchTab(i); }}
			>
				<span>{tab.title}</span>
				{#if terminalStore.tabs.length > 1}
					<button
						class="ml-1 rounded p-0.5 opacity-0 transition-opacity hover:bg-gx-bg-hover group-hover:opacity-100"
						class:opacity-100={terminalStore.activeIndex === i}
						onclick={(e) => { e.stopPropagation(); closeTab(i); }}
						aria-label="Close {tab.title}"
					>
						<X size={10} />
					</button>
				{/if}
			</div>
		{/each}
		<button
			class="rounded p-1 text-gx-text-muted transition-colors hover:bg-gx-bg-hover hover:text-gx-neon"
			onclick={addTerminal}
			title="New Terminal"
		>
			<Plus size={14} />
		</button>
	</div>

	<!-- Terminal container -->
	<div bind:this={terminalContainer} class="flex-1 overflow-hidden {hasEngineStyle ? '' : 'bg-gx-bg-primary'} p-1" style={terminalAreaStyle}></div>
</div>

<style>
	:global(.xterm) {
		height: 100%;
	}
	:global(.xterm-viewport) {
		overflow-y: auto !important;
	}
	:global(.xterm-viewport::-webkit-scrollbar) {
		width: 6px;
	}
	:global(.xterm-viewport::-webkit-scrollbar-thumb) {
		background: rgb(42 42 42); /* gx-border-default #2A2A2A */
		border-radius: 3px;
	}
</style>
