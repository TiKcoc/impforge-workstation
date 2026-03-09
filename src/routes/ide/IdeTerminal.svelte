<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Plus, X } from '@lucide/svelte';
	import { terminalStore } from '$lib/stores/terminal.svelte';

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
				background: '#0a0e14',
				foreground: '#e0e0e0',
				cursor: '#00FF66',
				cursorAccent: '#0a0e14',
				selectionBackground: '#1a3a5c',
				black: '#1a1e28',
				red: '#ff5370',
				green: '#00FF66',
				yellow: '#ffcb6b',
				blue: '#82aaff',
				magenta: '#c792ea',
				cyan: '#89ddff',
				white: '#e0e0e0'
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

<div class="flex h-full flex-col">
	<!-- Terminal tabs -->
	<div class="flex items-center gap-1 border-b border-white/5 bg-[#0d1117] px-2 py-1">
		{#each terminalStore.tabs as tab, i}
			<button
				class="flex items-center gap-1.5 rounded px-2 py-0.5 text-xs transition-colors {terminalStore.activeIndex === i
					? 'bg-white/10 text-[#00FF66]'
					: 'text-white/50 hover:text-white/80'}"
				onclick={() => switchTab(i)}
			>
				<span>{tab.title}</span>
				{#if terminalStore.tabs.length > 1}
					<button
						class="ml-1 rounded p-0.5 opacity-0 transition-opacity hover:bg-white/10 group-hover:opacity-100"
						class:opacity-100={terminalStore.activeIndex === i}
						onclick|stopPropagation={() => closeTab(i)}
					>
						<X size={10} />
					</button>
				{/if}
			</button>
		{/each}
		<button
			class="rounded p-1 text-white/30 transition-colors hover:bg-white/10 hover:text-[#00FF66]"
			onclick={addTerminal}
			title="New Terminal"
		>
			<Plus size={14} />
		</button>
	</div>

	<!-- Terminal container -->
	<div bind:this={terminalContainer} class="flex-1 overflow-hidden bg-[#0a0e14] p-1"></div>
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
		background: rgba(255, 255, 255, 0.1);
		border-radius: 3px;
	}
</style>
