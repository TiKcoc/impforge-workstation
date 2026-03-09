<script lang="ts">
	/**
	 * ModelPipelineView — BenikUI-integrated neon pipeline DAG visualization
	 *
	 * SVG-based pipeline graph with flowing particles, neon glows, and
	 * per-model branded colors. Style engine controls container appearance
	 * while SVG internals use the pipeline color system.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - label: Topology/pipeline label
	 */

	import { modelStatus } from '$lib/stores/model-status.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
	}

	let { widgetId = 'chat-pipeline' }: Props = $props();

	const NODE_RADIUS = 26;
	const SVG_HEIGHT = 200;

	/** Per-model accent color from model name */
	function getNodeColor(label: string, status: string): string {
		if (status === 'error') return '#ef4444';
		if (status === 'idle') return '#4b5563';
		const n = label.toLowerCase();
		if (n.includes('claude') || n.includes('anthropic')) return '#a855f7';
		if (n.includes('qwen')) return '#06b6d4';
		if (n.includes('hermes')) return '#f59e0b';
		if (n.includes('ollama') || n.includes('local') || n.includes('llama') || n.includes('dolphin')) return '#22c55e';
		if (n.includes('classifier') || n.includes('input') || n.includes('output')) return '#00ff66';
		if (n.includes('memory') || n.includes('forge')) return '#06b6d4';
		return '#00d4ff';
	}

	const typeIcons: Record<string, string> = {
		input: '▶',
		classifier: '⚡',
		model: '🧠',
		memory: '💾',
		output: '◀',
	};

	/** Topology label based on pipeline shape */
	let topologyLabel = $derived(() => {
		const nodeCount = modelStatus.pipeline.length;
		if (nodeCount <= 3) return 'Sequential';
		const hasParallel = modelStatus.pipeline.filter(n => n.type === 'model').length > 1;
		return hasParallel ? 'Parallel' : 'Sequential';
	});

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let labelComponent = $derived(styleEngine.getComponentStyle(widgetId, 'label'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let labelStyle = $derived(
		hasEngineStyle && labelComponent ? componentToCSS(labelComponent) : ''
	);
</script>

{#if modelStatus.pipeline.length > 0}
	<div
		class="p-2 overflow-x-auto {hasEngineStyle ? '' : 'rounded-gx-lg glass-panel-subtle'}"
		style={containerStyle}
	>
		<!-- Topology label -->
		<div class="flex items-center justify-between mb-1.5 px-1">
			<span
				class="text-[9px] font-medium uppercase tracking-wider {hasEngineStyle ? '' : 'text-gx-text-muted'}"
				style={labelStyle}
			>Pipeline</span>
			<span class="text-[9px] text-gx-neon/70 font-mono">{topologyLabel()}</span>
		</div>

		<svg width="100%" height={SVG_HEIGHT} viewBox="0 0 680 {SVG_HEIGHT}" preserveAspectRatio="xMidYMid meet" class="w-full">
			<defs>
				<!-- Neon glow filter -->
				<filter id="neon-glow" x="-50%" y="-50%" width="200%" height="200%">
					<feGaussianBlur stdDeviation="4" result="blur" />
					<feMerge>
						<feMergeNode in="blur" />
						<feMergeNode in="blur" />
						<feMergeNode in="SourceGraphic" />
					</feMerge>
				</filter>

				<!-- Subtle glow for inactive -->
				<filter id="subtle-glow" x="-30%" y="-30%" width="160%" height="160%">
					<feGaussianBlur stdDeviation="2" result="blur" />
					<feMerge>
						<feMergeNode in="blur" />
						<feMergeNode in="SourceGraphic" />
					</feMerge>
				</filter>

				<!-- Animated arrow marker -->
				<marker id="neon-arrow" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">
					<path d="M0,0 L10,3.5 L0,7" fill="#00d4ff" opacity="0.7" />
				</marker>

				<!-- Flowing particle gradient -->
				<radialGradient id="particle-glow">
					<stop offset="0%" stop-color="#00d4ff" stop-opacity="1" />
					<stop offset="100%" stop-color="#00d4ff" stop-opacity="0" />
				</radialGradient>
			</defs>

			<!-- Edges with neon glow -->
			{#each modelStatus.edges as edge}
				{@const fromNode = modelStatus.pipeline.find((n) => n.id === edge.from)}
				{@const toNode = modelStatus.pipeline.find((n) => n.id === edge.to)}
				{#if fromNode && toNode}
					{@const edgeColor = edge.active ? getNodeColor(toNode.label, toNode.status) : '#374151'}
					<!-- Edge line -->
					<line
						x1={fromNode.x + NODE_RADIUS + 20}
						y1={fromNode.y}
						x2={toNode.x - NODE_RADIUS + 20}
						y2={toNode.y}
						stroke={edgeColor}
						stroke-width={edge.active ? 2 : 1}
						stroke-dasharray={edge.active ? '8 4' : 'none'}
						opacity={edge.active ? 0.8 : 0.3}
						filter={edge.active ? 'url(#subtle-glow)' : 'none'}
						marker-end="url(#neon-arrow)"
					>
						{#if edge.active}
							<animate attributeName="stroke-dashoffset" from="12" to="0" dur="0.8s" repeatCount="indefinite" />
						{/if}
					</line>

					<!-- Flowing particle along active edges -->
					{#if edge.active}
						<circle r="3" fill={edgeColor} opacity="0.9" filter="url(#neon-glow)">
							<animateMotion
								dur="1.5s"
								repeatCount="indefinite"
								path="M{fromNode.x + NODE_RADIUS + 20},{fromNode.y} L{toNode.x - NODE_RADIUS + 20},{toNode.y}"
							/>
							<animate attributeName="opacity" values="0;1;1;0" dur="1.5s" repeatCount="indefinite" />
						</circle>
					{/if}
				{/if}
			{/each}

			<!-- Nodes with neon glow -->
			{#each modelStatus.pipeline as node}
				{@const nodeColor = getNodeColor(node.label, node.status)}
				<g transform="translate({node.x + 20}, {node.y})">
					<!-- Outer glow halo for active nodes -->
					{#if node.status === 'active'}
						<circle r={NODE_RADIUS + 6} fill="none" stroke={nodeColor} stroke-width="1" opacity="0.2" filter="url(#neon-glow)">
							<animate attributeName="r" values="{NODE_RADIUS + 3};{NODE_RADIUS + 8};{NODE_RADIUS + 3}" dur="2.5s" repeatCount="indefinite" />
							<animate attributeName="opacity" values="0.15;0.35;0.15" dur="2.5s" repeatCount="indefinite" />
						</circle>
					{/if}

					<!-- Node circle — glass effect -->
					<circle
						r={NODE_RADIUS}
						fill="rgba(13, 13, 18, 0.8)"
						stroke={nodeColor}
						stroke-width={node.status === 'active' ? 2.5 : 1.5}
						opacity={node.status === 'idle' ? 0.5 : 1}
						filter={node.status === 'active' ? 'url(#subtle-glow)' : 'none'}
					/>

					<!-- Inner glow circle -->
					{#if node.status === 'active' || node.status === 'completed'}
						<circle
							r={NODE_RADIUS - 4}
							fill="none"
							stroke={nodeColor}
							stroke-width="0.5"
							opacity="0.15"
						/>
					{/if}

					<!-- Type icon -->
					<text
						text-anchor="middle"
						dominant-baseline="central"
						fill={nodeColor}
						font-size="14"
						opacity={node.status === 'idle' ? 0.5 : 1}
					>{typeIcons[node.type] ?? '?'}</text>

					<!-- Label below -->
					<text
						text-anchor="middle"
						y={NODE_RADIUS + 16}
						fill={node.status === 'active' ? nodeColor : '#6b7280'}
						font-size="9"
						font-family="var(--font-sans)"
						font-weight={node.status === 'active' ? '600' : '400'}
					>{node.label.length > 20 ? node.label.slice(0, 20) + '…' : node.label}</text>

					<!-- Metrics above (active/completed nodes) -->
					{#if node.metrics}
						<text
							text-anchor="middle"
							y={-NODE_RADIUS - 8}
							fill={nodeColor}
							font-size="8"
							font-family="var(--font-mono)"
							opacity="0.8"
						>{node.metrics.tokens}tk · {node.metrics.latencyMs}ms</text>
					{/if}
				</g>
			{/each}

			<!-- Routing decision label -->
			{#if modelStatus.lastRouting}
				<g transform="translate(340, {SVG_HEIGHT - 12})">
					<text text-anchor="middle" fill="#4b5563" font-size="9" font-family="var(--font-sans)">
						Route: <tspan fill="#00ff66" font-weight="600">{modelStatus.lastRouting.taskType}</tspan>
						→ <tspan fill="#06b6d4" font-family="var(--font-mono)">{modelStatus.lastRouting.model.split('/').pop()}</tspan>
						· {modelStatus.lastRouting.reason}
					</text>
				</g>
			{/if}
		</svg>
	</div>
{/if}
