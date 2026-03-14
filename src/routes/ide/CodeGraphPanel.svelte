<script lang="ts">
	/**
	 * CodeGraphPanel v2 — Live Code Knowledge Graph Visualization Engine
	 *
	 * Visualizes code relationships as an interactive force-directed graph:
	 * imports/exports, function calls, type dependencies, module coupling.
	 *
	 * v2 Upgrades (research-backed):
	 *   - Live Mode: Auto-re-analyzes when tabs change ($effect on ide.openTabs)
	 *   - Ego-Graph Focus: Centers on active file, dims distant nodes (NetworkX pattern)
	 *   - Semantic Zoom: 4-level detail (overview → module → file → detail)
	 *   - ForceAtlas2-inspired layout: Degree-dependent repulsion, adaptive damping
	 *   - Directory clustering: Color-coded by folder with hull overlays
	 *
	 * Research: ForceAtlas2 (Jacomy 2014), Semantic Zoom (arXiv:2510.00003v1),
	 *           Ego-graph (NetworkX), SVG perf (svggenie.com 2025 benchmark)
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - graph-canvas: SVG rendering area
	 *   - node-detail: Selected node info panel
	 */

	import { onMount, onDestroy } from 'svelte';
	import {
		Network, ZoomIn, ZoomOut, Maximize2, RefreshCw,
		FileCode, Box, FunctionSquare, Type, ArrowRight,
		Filter, Loader2, Radio, Focus, Layers
	} from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'ide-code-graph';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let canvasComp = $derived(styleEngine.getComponentStyle(widgetId, 'graph-canvas'));
	let canvasStyle = $derived(hasEngineStyle && canvasComp ? componentToCSS(canvasComp) : '');

	type NodeKind = 'file' | 'function' | 'class' | 'type' | 'module';
	type EdgeKind = 'import' | 'call' | 'extends' | 'implements' | 'uses';
	type ZoomLevel = 'overview' | 'module' | 'file' | 'detail';

	interface GraphNode {
		id: string;
		label: string;
		kind: NodeKind;
		file: string;
		dir: string;
		line?: number;
		x: number;
		y: number;
		vx: number;
		vy: number;
		size: number;
		degree: number;
	}

	interface GraphEdge {
		source: string;
		target: string;
		kind: EdgeKind;
	}

	// Core state
	let nodes = $state<GraphNode[]>([]);
	let edges = $state<GraphEdge[]>([]);
	let selectedNode = $state<GraphNode | null>(null);
	let analyzing = $state(false);
	let zoom = $state(1);
	let panX = $state(0);
	let panY = $state(0);
	let dragging = $state(false);
	let dragStart = $state({ x: 0, y: 0 });
	let dragNode = $state<GraphNode | null>(null);
	let filterKind = $state<NodeKind | 'all'>('all');
	let svgEl = $state<SVGSVGElement>(undefined!);
	let animFrame = 0;
	let simulating = $state(false);

	// v2: Live Mode + Ego-Graph + Semantic Zoom
	let liveMode = $state(true);
	let egoFocus = $state(true);
	let egoRadius = $state(2);
	let liveTabSignature = '';

	const kindConfig: Record<NodeKind, { color: string; icon: typeof FileCode }> = {
		file: { color: '#00FF66', icon: FileCode },
		function: { color: '#82aaff', icon: FunctionSquare },
		class: { color: '#ffcb6b', icon: Box },
		type: { color: '#c792ea', icon: Type },
		module: { color: '#89ddff', icon: Box },
	};

	const edgeColors: Record<EdgeKind, string> = {
		import: '#3a5a3a',
		call: '#3a4a6a',
		extends: '#6a4a3a',
		implements: '#4a3a6a',
		uses: '#3a3a3a',
	};

	const clusterPalette = ['#00FF66', '#82aaff', '#ffcb6b', '#c792ea', '#89ddff', '#f78c6c', '#ff5370', '#c3e88d'];

	// --- Utility functions ---

	function getZoomLevel(z: number): ZoomLevel {
		if (z < 0.5) return 'overview';
		if (z < 1.0) return 'module';
		if (z < 1.8) return 'file';
		return 'detail';
	}

	function getDirectory(path: string): string {
		const parts = path.split('/');
		return parts.length > 1 ? parts.slice(0, -1).join('/') : '/';
	}

	function egoGraphBFS(focusId: string, radius: number): Set<string> {
		const visited = new Set<string>();
		const queue: [string, number][] = [[focusId, 0]];
		while (queue.length > 0) {
			const entry = queue.shift()!;
			const nodeId = entry[0];
			const depth = entry[1];
			if (visited.has(nodeId) || depth > radius) continue;
			visited.add(nodeId);
			for (const edge of edges) {
				if (edge.source === nodeId && !visited.has(edge.target)) {
					queue.push([edge.target, depth + 1]);
				}
				if (edge.target === nodeId && !visited.has(edge.source)) {
					queue.push([edge.source, depth + 1]);
				}
			}
		}
		return visited;
	}

	// --- Derived computations ---

	let zoomLevel = $derived(getZoomLevel(zoom));

	let dirColorMap = $derived.by(() => {
		const dirs = new Set(nodes.filter(n => n.kind === 'file').map(n => n.dir));
		const map = new Map<string, string>();
		let idx = 0;
		for (const dir of dirs) {
			map.set(dir, clusterPalette[idx % clusterPalette.length]);
			idx++;
		}
		return map;
	});

	let activeFileNodeId = $derived(ide.activeTab?.path ? `file:${ide.activeTab.path}` : '');

	let egoNodeIds = $derived.by(() => {
		if (!egoFocus || !activeFileNodeId || nodes.length === 0) {
			return new Set(nodes.map(n => n.id));
		}
		return egoGraphBFS(activeFileNodeId, egoRadius);
	});

	let filteredNodes = $derived(
		filterKind === 'all' ? nodes : nodes.filter(n => n.kind === filterKind)
	);
	let filteredNodeIds = $derived(new Set(filteredNodes.map(n => n.id)));
	let filteredEdges = $derived(
		edges.filter(e => filteredNodeIds.has(e.source) && filteredNodeIds.has(e.target))
	);

	// Semantic zoom: which nodes get labels
	let visibleLabelIds = $derived.by(() => {
		if (zoomLevel === 'detail') return new Set(filteredNodes.map(n => n.id));
		if (zoomLevel === 'file') return new Set(filteredNodes.filter(n => n.kind === 'file' || n.kind === 'class').map(n => n.id));
		if (zoomLevel === 'module') return new Set(filteredNodes.filter(n => n.kind === 'file').map(n => n.id));
		return new Set<string>();
	});

	// Semantic zoom: which nodes render at all
	let visibleNodeIds = $derived.by(() => {
		if (zoomLevel === 'detail' || zoomLevel === 'file') return filteredNodeIds;
		if (zoomLevel === 'module') return new Set(filteredNodes.filter(n => n.kind === 'file' || n.kind === 'class' || n.kind === 'module').map(n => n.id));
		return new Set(filteredNodes.filter(n => n.kind === 'file').map(n => n.id));
	});

	let connectedEdges = $derived.by(() => {
		const sel = selectedNode;
		return sel ? edges.filter(e => e.source === sel.id || e.target === sel.id) : [];
	});

	// --- Live Mode: watch tab changes ---

	$effect(() => {
		if (!liveMode) return;
		const sig = ide.openTabs.map(t => t.path).join('|');
		if (sig !== liveTabSignature && ide.openTabs.length > 0) {
			liveTabSignature = sig;
			analyzeOpenFiles();
		}
	});

	// --- Analysis ---

	function analyzeOpenFiles() {
		analyzing = true;
		const newNodes: GraphNode[] = [];
		const newEdges: GraphEdge[] = [];
		const nodeMap = new Map<string, string>();
		const degreeCount = new Map<string, number>();
		const width = 600;
		const height = 300;

		for (const tab of ide.openTabs) {
			if (!tab.content || !tab.path) continue;
			const fileName = tab.path.split('/').pop() || tab.path;
			const fileId = `file:${tab.path}`;
			const dir = getDirectory(tab.path);

			newNodes.push({
				id: fileId,
				label: fileName,
				kind: 'file',
				file: tab.path,
				dir,
				x: Math.random() * width,
				y: Math.random() * height,
				vx: 0, vy: 0,
				size: Math.min(30, 10 + tab.content.split('\n').length / 20),
				degree: 0,
			});

			const lines = tab.content.split('\n');

			for (let i = 0; i < lines.length; i++) {
				const line = lines[i].trim();

				// Detect imports
				const importMatch = line.match(/^import\s+(?:\{([^}]+)\}|\*\s+as\s+(\w+)|(\w+))\s+from\s+['"]([^'"]+)['"]/);
				if (importMatch) {
					const symbols = importMatch[1]?.split(',').map(s => s.trim().split(' as ')[0].trim()) || [importMatch[2] || importMatch[3]];
					const source = importMatch[4];
					for (const sym of symbols) {
						if (!sym) continue;
						const symId = `import:${source}:${sym}`;
						if (!nodeMap.has(symId)) {
							nodeMap.set(symId, symId);
							newNodes.push({
								id: symId, label: sym, kind: 'function',
								file: source, dir: getDirectory(source),
								line: i + 1,
								x: Math.random() * width, y: Math.random() * height,
								vx: 0, vy: 0, size: 8, degree: 0,
							});
						}
						newEdges.push({ source: fileId, target: symId, kind: 'import' });
						degreeCount.set(fileId, (degreeCount.get(fileId) || 0) + 1);
						degreeCount.set(symId, (degreeCount.get(symId) || 0) + 1);
					}
				}

				// Detect declarations
				const funcMatch = line.match(/^(?:export\s+)?(?:async\s+)?function\s+(\w+)/);
				const classMatch = line.match(/^(?:export\s+)?class\s+(\w+)/);
				const typeMatch = line.match(/^(?:export\s+)?(?:interface|type)\s+(\w+)/);
				const rustFnMatch = line.match(/^(?:pub\s+)?(?:async\s+)?fn\s+(\w+)/);
				const rustStructMatch = line.match(/^(?:pub\s+)?(?:struct|enum|trait)\s+(\w+)/);

				const declName = funcMatch?.[1] || classMatch?.[1] || typeMatch?.[1] || rustFnMatch?.[1] || rustStructMatch?.[1];
				const declKind: NodeKind = funcMatch || rustFnMatch ? 'function' : classMatch || rustStructMatch ? 'class' : typeMatch ? 'type' : 'function';

				if (declName) {
					const declId = `decl:${tab.path}:${declName}`;
					if (!nodeMap.has(declId)) {
						nodeMap.set(declId, declId);
						newNodes.push({
							id: declId, label: declName, kind: declKind,
							file: tab.path, dir,
							line: i + 1,
							x: Math.random() * width, y: Math.random() * height,
							vx: 0, vy: 0, size: 12, degree: 0,
						});
						newEdges.push({ source: fileId, target: declId, kind: 'uses' });
						degreeCount.set(fileId, (degreeCount.get(fileId) || 0) + 1);
						degreeCount.set(declId, (degreeCount.get(declId) || 0) + 1);
					}
				}

				// Detect extends
				const extendsMatch = line.match(/class\s+\w+\s+extends\s+(\w+)/);
				if (extendsMatch) {
					const parentId = `ref:${extendsMatch[1]}`;
					if (!nodeMap.has(parentId)) {
						nodeMap.set(parentId, parentId);
						newNodes.push({
							id: parentId, label: extendsMatch[1], kind: 'class',
							file: '', dir: '',
							x: Math.random() * width, y: Math.random() * height,
							vx: 0, vy: 0, size: 10, degree: 0,
						});
					}
					const childId = `decl:${tab.path}:${classMatch?.[1] || ''}`;
					if (nodeMap.has(childId)) {
						newEdges.push({ source: childId, target: parentId, kind: 'extends' });
						degreeCount.set(childId, (degreeCount.get(childId) || 0) + 1);
						degreeCount.set(parentId, (degreeCount.get(parentId) || 0) + 1);
					}
				}

				// Detect implements
				const implMatch = line.match(/(?:implements|:\s*)(\w+(?:\s*,\s*\w+)*)\s*\{/);
				if (implMatch && classMatch) {
					const ifaces = implMatch[1].split(',').map(s => s.trim());
					for (const iface of ifaces) {
						const ifaceId = `ref:${iface}`;
						if (!nodeMap.has(ifaceId)) {
							nodeMap.set(ifaceId, ifaceId);
							newNodes.push({
								id: ifaceId, label: iface, kind: 'type',
								file: '', dir: '',
								x: Math.random() * width, y: Math.random() * height,
								vx: 0, vy: 0, size: 10, degree: 0,
							});
						}
						const srcId = `decl:${tab.path}:${classMatch[1]}`;
						newEdges.push({ source: srcId, target: ifaceId, kind: 'implements' });
						degreeCount.set(srcId, (degreeCount.get(srcId) || 0) + 1);
						degreeCount.set(ifaceId, (degreeCount.get(ifaceId) || 0) + 1);
					}
				}
			}
		}

		// Apply degree counts to nodes
		for (const node of newNodes) {
			node.degree = degreeCount.get(node.id) || 0;
		}

		nodes = newNodes;
		edges = newEdges;
		selectedNode = null;
		analyzing = false;
		startSimulation();
	}

	// --- ForceAtlas2-Inspired Simulation (Jacomy et al. 2014) ---

	function startSimulation() {
		simulating = true;
		let iterations = 0;
		const maxIterations = 200;

		function tick() {
			if (iterations >= maxIterations || !simulating) {
				simulating = false;
				return;
			}

			const alpha = 1 - iterations / maxIterations;
			const n = nodes.length;
			const k = Math.sqrt((600 * 300) / Math.max(n, 1));

			// ForceAtlas2: Degree-dependent repulsion
			// Higher-degree nodes push harder, naturally separating hubs
			for (let i = 0; i < n; i++) {
				for (let j = i + 1; j < n; j++) {
					const dx = nodes[j].x - nodes[i].x;
					const dy = nodes[j].y - nodes[i].y;
					const dist = Math.sqrt(dx * dx + dy * dy) || 1;
					const di = nodes[i].degree + 1;
					const dj = nodes[j].degree + 1;
					const force = k * (di + dj) / dist * alpha * 0.25;
					const fx = (dx / dist) * force;
					const fy = (dy / dist) * force;
					nodes[i].vx -= fx;
					nodes[i].vy -= fy;
					nodes[j].vx += fx;
					nodes[j].vy += fy;
				}
			}

			// Spring attraction along edges
			for (const edge of edges) {
				const source = nodes.find(nd => nd.id === edge.source);
				const target = nodes.find(nd => nd.id === edge.target);
				if (!source || !target) continue;
				const dx = target.x - source.x;
				const dy = target.y - source.y;
				const dist = Math.sqrt(dx * dx + dy * dy) || 1;
				// Same-directory edges pull tighter (cluster formation)
				const sameDir = source.dir && source.dir === target.dir;
				const idealDist = sameDir ? k * 0.5 : k;
				const force = (dist - idealDist) / dist * alpha * 0.15;
				const fx = dx * force;
				const fy = dy * force;
				source.vx += fx;
				source.vy += fy;
				target.vx -= fx;
				target.vy -= fy;
			}

			// Center gravity
			const cx = 300, cy = 150;
			for (const node of nodes) {
				node.vx += (cx - node.x) * 0.01 * alpha;
				node.vy += (cy - node.y) * 0.01 * alpha;
			}

			// ForceAtlas2: Adaptive speed capping
			for (const node of nodes) {
				const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
				const maxSpeed = 10 * alpha + 1;
				if (speed > maxSpeed) {
					const scale = maxSpeed / speed;
					node.vx *= scale;
					node.vy *= scale;
				}
				node.vx *= 0.65;
				node.vy *= 0.65;
				node.x += node.vx;
				node.y += node.vy;
				node.x = Math.max(20, Math.min(580, node.x));
				node.y = Math.max(20, Math.min(280, node.y));
			}

			nodes = [...nodes];
			iterations++;
			animFrame = requestAnimationFrame(tick);
		}

		tick();
	}

	// --- Interaction ---

	function handleWheel(e: WheelEvent) {
		e.preventDefault();
		const delta = e.deltaY > 0 ? 0.9 : 1.1;
		zoom = Math.max(0.2, Math.min(4, zoom * delta));
	}

	function handleMouseDown(e: MouseEvent) {
		if (dragNode) return;
		dragging = true;
		dragStart = { x: e.clientX - panX, y: e.clientY - panY };
	}

	function handleMouseMove(e: MouseEvent) {
		if (dragNode) {
			const rect = svgEl.getBoundingClientRect();
			dragNode.x = (e.clientX - rect.left - panX) / zoom;
			dragNode.y = (e.clientY - rect.top - panY) / zoom;
			nodes = [...nodes];
		} else if (dragging) {
			panX = e.clientX - dragStart.x;
			panY = e.clientY - dragStart.y;
		}
	}

	function handleMouseUp() {
		dragging = false;
		dragNode = null;
	}

	function selectNode(node: GraphNode) {
		selectedNode = selectedNode?.id === node.id ? null : node;
	}

	function startDragNode(e: MouseEvent, node: GraphNode) {
		e.stopPropagation();
		dragNode = node;
	}

	function openInEditor(node: GraphNode) {
		if (!node.file || node.file.startsWith('ref:')) return;
		const name = node.file.split('/').pop() || node.file;
		ide.openFile(node.file, name);
	}

	function resetView() {
		zoom = 1;
		panX = 0;
		panY = 0;
	}

	function focusActiveFile() {
		if (!activeFileNodeId) return;
		const node = nodes.find(n => n.id === activeFileNodeId);
		if (node) {
			panX = 300 - node.x * zoom;
			panY = 150 - node.y * zoom;
		}
	}

	onMount(() => {
		if (ide.openTabs.length > 0) {
			liveTabSignature = ide.openTabs.map(t => t.path).join('|');
			analyzeOpenFiles();
		}
	});

	onDestroy(() => {
		simulating = false;
		if (animFrame) cancelAnimationFrame(animFrame);
	});
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Toolbar -->
	<div class="flex items-center gap-1 px-2 py-1 border-b border-gx-border-subtle shrink-0">
		<Network size={12} class="text-gx-neon" />
		<span class="text-[10px] font-medium text-gx-text-primary">Code Graph</span>

		<div class="flex items-center gap-0.5 ml-2">
			<button
				onclick={analyzeOpenFiles}
				disabled={analyzing}
				class="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded bg-gx-neon/10 text-gx-neon border border-gx-neon/20 hover:bg-gx-neon/20 disabled:opacity-30"
			>
				{#if analyzing}
					<Loader2 size={8} class="animate-spin" />
				{:else}
					<RefreshCw size={8} />
				{/if}
				Analyze
			</button>
		</div>

		<!-- Live Mode Toggle -->
		<button
			onclick={() => liveMode = !liveMode}
			class="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded transition-colors
				{liveMode
					? 'bg-gx-status-success/10 text-gx-status-success border border-gx-status-success/30'
					: 'text-gx-text-disabled border border-transparent hover:text-gx-text-muted'}"
			title="Live Mode: Auto-re-analyze when tabs change"
		>
			<Radio size={8} />
			Live
		</button>

		<!-- Ego Focus Toggle -->
		<button
			onclick={() => { egoFocus = !egoFocus; if (egoFocus) focusActiveFile(); }}
			class="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded transition-colors
				{egoFocus
					? 'bg-gx-accent-cyan/10 text-gx-accent-cyan border border-gx-accent-cyan/30'
					: 'text-gx-text-disabled border border-transparent hover:text-gx-text-muted'}"
			title="Ego Focus: Center on active file, dim distant nodes"
		>
			<Focus size={8} />
			Ego
		</button>

		<!-- Filter -->
		<div class="flex items-center gap-0.5 ml-1">
			<Filter size={8} class="text-gx-text-disabled" />
			{#each ['all', 'file', 'function', 'class', 'type'] as kind}
				<button
					onclick={() => filterKind = kind as NodeKind | 'all'}
					class="px-1.5 py-0.5 text-[9px] rounded transition-colors
						{filterKind === kind
							? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
							: 'text-gx-text-disabled hover:text-gx-text-muted border border-transparent'}"
				>{kind}</button>
			{/each}
		</div>

		<div class="flex-1"></div>

		<!-- Semantic Zoom Level -->
		<div class="flex items-center gap-1 mr-1">
			<Layers size={8} class="text-gx-text-disabled" />
			<span class="text-[8px] text-gx-text-disabled uppercase">{zoomLevel}</span>
		</div>

		<!-- Zoom controls -->
		<span class="text-[9px] text-gx-text-disabled">{Math.round(zoom * 100)}%</span>
		<button onclick={() => zoom = Math.min(4, zoom * 1.2)} class="p-0.5 text-gx-text-muted hover:text-gx-neon"><ZoomIn size={10} /></button>
		<button onclick={() => zoom = Math.max(0.2, zoom / 1.2)} class="p-0.5 text-gx-text-muted hover:text-gx-neon"><ZoomOut size={10} /></button>
		<button onclick={resetView} class="p-0.5 text-gx-text-muted hover:text-gx-neon"><Maximize2 size={10} /></button>

		<span class="text-[9px] text-gx-text-disabled ml-1">{filteredNodes.length}n · {filteredEdges.length}e</span>
	</div>

	<!-- Graph Canvas + Detail panel -->
	<div class="flex flex-1 min-h-0 overflow-hidden">
		<!-- SVG Canvas -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<svg
			bind:this={svgEl}
			class="flex-1 min-w-0 cursor-grab"
			style={canvasStyle}
			onwheel={handleWheel}
			onmousedown={handleMouseDown}
			onmousemove={handleMouseMove}
			onmouseup={handleMouseUp}
			onmouseleave={handleMouseUp}
		>
			<g transform="translate({panX},{panY}) scale({zoom})">
				<!-- Directory cluster hulls (subtle background circles) -->
				{#if zoomLevel !== 'overview'}
					{#each [...dirColorMap] as [dir, color]}
						{@const dirNodes = filteredNodes.filter(n => n.dir === dir && n.kind === 'file')}
						{#if dirNodes.length >= 2}
							{@const hullCx = dirNodes.reduce((s, n) => s + n.x, 0) / dirNodes.length}
							{@const hullCy = dirNodes.reduce((s, n) => s + n.y, 0) / dirNodes.length}
							{@const maxDist = Math.max(40, ...dirNodes.map(n => Math.sqrt((n.x - hullCx) ** 2 + (n.y - hullCy) ** 2))) + 25}
							<circle
								cx={hullCx}
								cy={hullCy}
								r={maxDist}
								fill="{color}08"
								stroke="{color}15"
								stroke-width="1"
								stroke-dasharray="4 2"
							/>
							{#if zoomLevel === 'detail' || zoomLevel === 'file'}
								<text
									x={hullCx}
									y={hullCy - maxDist + 8}
									text-anchor="middle"
									fill="{color}40"
									font-size="7"
									font-family="monospace"
								>{dir.split('/').pop() || dir}</text>
							{/if}
						{/if}
					{/each}
				{/if}

				<!-- Edges -->
				{#each filteredEdges as edge}
					{@const source = nodes.find(n => n.id === edge.source)}
					{@const target = nodes.find(n => n.id === edge.target)}
					{#if source && target && visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target)}
						{@const inEgo = egoNodeIds.has(edge.source) && egoNodeIds.has(edge.target)}
						{@const isSelected = selectedNode !== null && (edge.source === selectedNode.id || edge.target === selectedNode.id)}
						<line
							x1={source.x}
							y1={source.y}
							x2={target.x}
							y2={target.y}
							stroke={edgeColors[edge.kind]}
							stroke-width={isSelected ? 2 : 0.8}
							stroke-opacity={isSelected ? 0.9 : inEgo ? 0.5 : 0.1}
						/>
						{#if zoomLevel === 'file' || zoomLevel === 'detail'}
							{@const adx = target.x - source.x}
							{@const ady = target.y - source.y}
							{@const adist = Math.sqrt(adx * adx + ady * ady) || 1}
							{@const anx = adx / adist}
							{@const any_ = ady / adist}
							{@const ax = target.x - anx * (target.size + 4)}
							{@const ay = target.y - any_ * (target.size + 4)}
							<polygon
								points="{ax},{ay} {ax - anx * 4 + any_ * 2},{ay - any_ * 4 - anx * 2} {ax - anx * 4 - any_ * 2},{ay - any_ * 4 + anx * 2}"
								fill={edgeColors[edge.kind]}
								opacity={inEgo ? 0.5 : 0.1}
							/>
						{/if}
					{/if}
				{/each}

				<!-- Nodes -->
				{#each filteredNodes as node}
					{#if visibleNodeIds.has(node.id)}
						{@const cfg = kindConfig[node.kind]}
						{@const inEgo = egoNodeIds.has(node.id)}
						{@const isActive = node.id === activeFileNodeId}
						{@const isSelected = selectedNode?.id === node.id}
						{@const nodeColor = node.kind === 'file' ? (dirColorMap.get(node.dir) || cfg.color) : cfg.color}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<g
							transform="translate({node.x},{node.y})"
							class="cursor-pointer"
							role="button"
							tabindex="-1"
							onmousedown={(e) => startDragNode(e, node)}
							onclick={() => selectNode(node)}
							onkeydown={(e) => e.key === 'Enter' && selectNode(node)}
							ondblclick={() => openInEditor(node)}
							opacity={inEgo ? 1 : 0.15}
						>
							{#if isActive}
								<circle
									r={node.size + 4}
									fill="none"
									stroke="#00FF66"
									stroke-width="1.5"
									stroke-opacity="0.6"
									stroke-dasharray="3 2"
								/>
							{/if}
							<circle
								r={node.size}
								fill="{nodeColor}15"
								stroke={isSelected ? '#00FF66' : nodeColor}
								stroke-width={isSelected ? 2 : isActive ? 1.5 : 1}
								stroke-opacity={isSelected ? 1 : inEgo ? 0.7 : 0.3}
							/>
							{#if node.degree > 3 && (zoomLevel === 'file' || zoomLevel === 'detail')}
								<circle
									r={2}
									cx={node.size - 2}
									cy={-node.size + 2}
									fill={nodeColor}
									opacity="0.8"
								/>
							{/if}
							{#if visibleLabelIds.has(node.id)}
								<text
									y={node.size + 10}
									text-anchor="middle"
									fill={inEgo ? '#8a9aaa' : '#3a4a5a'}
									font-size={zoomLevel === 'detail' ? '9' : '8'}
									font-family="monospace"
								>{node.label.length > 20 ? node.label.slice(0, 19) + '\u2026' : node.label}</text>
							{/if}
						</g>
					{/if}
				{/each}
			</g>

			{#if nodes.length === 0 && !analyzing}
				<text
					x="50%"
					y="50%"
					text-anchor="middle"
					fill="#3a4a5a"
					font-size="12"
				>Open files and click "Analyze" to build the code graph</text>
			{/if}
		</svg>

		<!-- Detail sidebar -->
		{#if selectedNode}
			{@const cfg = kindConfig[selectedNode.kind]}
			<div class="w-48 border-l border-gx-border-subtle p-2 overflow-auto shrink-0">
				<div class="flex items-center gap-1.5 mb-2">
					<div class="w-3 h-3 rounded-full" style="background: {cfg.color}"></div>
					<span class="text-xs font-medium text-gx-text-primary truncate">{selectedNode.label}</span>
				</div>

				<div class="space-y-1.5 text-[10px]">
					<div>
						<span class="text-gx-text-disabled">Kind:</span>
						<span class="text-gx-text-muted ml-1">{selectedNode.kind}</span>
					</div>
					<div>
						<span class="text-gx-text-disabled">Degree:</span>
						<span class="text-gx-text-muted ml-1">{selectedNode.degree}</span>
					</div>
					{#if selectedNode.dir}
						<div>
							<span class="text-gx-text-disabled">Dir:</span>
							<span class="text-gx-text-muted ml-1 truncate">{selectedNode.dir.split('/').pop()}</span>
						</div>
					{/if}
					{#if selectedNode.file}
						<div>
							<span class="text-gx-text-disabled">File:</span>
							<button
								onclick={() => openInEditor(selectedNode!)}
								class="text-gx-accent-cyan hover:underline ml-1 truncate max-w-32 inline-block align-bottom"
								title={selectedNode.file}
							>{selectedNode.file.split('/').pop()}</button>
						</div>
					{/if}
					{#if selectedNode.line}
						<div>
							<span class="text-gx-text-disabled">Line:</span>
							<span class="text-gx-text-muted ml-1">{selectedNode.line}</span>
						</div>
					{/if}

					{#if egoFocus}
						<div>
							<span class="text-gx-text-disabled">In ego:</span>
							<span class={egoNodeIds.has(selectedNode.id) ? 'text-gx-status-success ml-1' : 'text-gx-text-disabled ml-1'}>
								{egoNodeIds.has(selectedNode.id) ? 'yes' : 'no'}
							</span>
						</div>
					{/if}

					{#if connectedEdges.length > 0}
						<div class="mt-2 pt-2 border-t border-gx-border-subtle/50">
							<span class="text-gx-text-disabled">Connections ({connectedEdges.length}):</span>
							<div class="mt-1 space-y-0.5">
								{#each connectedEdges.slice(0, 12) as edge}
									{@const other = edge.source === selectedNode?.id
										? nodes.find(n => n.id === edge.target)
										: nodes.find(n => n.id === edge.source)}
									{#if other}
										<div class="flex items-center gap-1">
											<ArrowRight size={6} class="text-gx-text-disabled shrink-0" />
											<span class="text-gx-text-muted truncate">{other.label}</span>
											<span class="text-[8px] text-gx-text-disabled ml-auto">{edge.kind}</span>
										</div>
									{/if}
								{/each}
							</div>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</div>
</div>
