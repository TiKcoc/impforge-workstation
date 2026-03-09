<script lang="ts">
	/**
	 * StyledSparkline — Inline SVG sparkline with style engine integration
	 *
	 * Renders a lightweight sparkline chart using SVG path.
	 * Supports area fill, configurable colors, smooth curves, and animated drawing.
	 * No external dependencies — pure SVG.
	 *
	 * Usage:
	 *   <StyledSparkline data={[45, 52, 48, 67, 72, 65]} color="#00ff88" />
	 */

	interface Props {
		data: number[];
		color?: string;
		fillOpacity?: number;
		lineWidth?: number;
		height?: number;
		width?: number;
		smooth?: boolean;
		showArea?: boolean;
		animate?: boolean;
		class?: string;
	}

	let {
		data,
		color = '#00ff88',
		fillOpacity = 0.15,
		lineWidth = 1.5,
		height = 24,
		width = 96,
		smooth = true,
		showArea = true,
		animate = false,
		class: className = '',
	}: Props = $props();

	// Compute SVG path from data points
	let pathD = $derived(() => {
		if (data.length < 2) return '';
		const min = Math.min(...data);
		const max = Math.max(...data);
		const range = max - min || 1;
		const stepX = width / (data.length - 1);
		const pad = 2; // Top/bottom padding
		const h = height - pad * 2;

		const points = data.map((v, i) => ({
			x: i * stepX,
			y: pad + h - ((v - min) / range) * h,
		}));

		if (smooth && points.length > 2) {
			// Catmull-Rom to cubic bezier
			let d = `M ${points[0].x},${points[0].y}`;
			for (let i = 0; i < points.length - 1; i++) {
				const p0 = points[Math.max(0, i - 1)];
				const p1 = points[i];
				const p2 = points[i + 1];
				const p3 = points[Math.min(points.length - 1, i + 2)];
				const cp1x = p1.x + (p2.x - p0.x) / 6;
				const cp1y = p1.y + (p2.y - p0.y) / 6;
				const cp2x = p2.x - (p3.x - p1.x) / 6;
				const cp2y = p2.y - (p3.y - p1.y) / 6;
				d += ` C ${cp1x},${cp1y} ${cp2x},${cp2y} ${p2.x},${p2.y}`;
			}
			return d;
		}

		return points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x},${p.y}`).join(' ');
	});

	// Area path (closed polygon below the line)
	let areaD = $derived(() => {
		const line = pathD();
		if (!line) return '';
		return `${line} L ${width},${height} L 0,${height} Z`;
	});

	// Approximate path length for stroke animation
	let pathLength = $derived(data.length * 20);
</script>

<svg
	{width}
	{height}
	viewBox="0 0 {width} {height}"
	class="styled-sparkline {className}"
	preserveAspectRatio="none"
	aria-label="Sparkline chart with {data.length} data points"
	role="img"
>
	<!-- Area fill -->
	{#if showArea}
		<path
			d={areaD()}
			fill={color}
			fill-opacity={fillOpacity}
			stroke="none"
		/>
	{/if}

	<!-- Line -->
	<path
		d={pathD()}
		fill="none"
		stroke={color}
		stroke-width={lineWidth}
		stroke-linecap="round"
		stroke-linejoin="round"
		class:sparkline-animate={animate}
		style:--sparkline-length="{pathLength}"
	/>
</svg>

<style>
	.sparkline-animate {
		stroke-dasharray: var(--sparkline-length);
		stroke-dashoffset: var(--sparkline-length);
		animation: sparkline-draw 1s ease-out forwards;
	}

	@keyframes sparkline-draw {
		to { stroke-dashoffset: 0; }
	}

	@media (prefers-reduced-motion: reduce) {
		.sparkline-animate {
			animation: none;
			stroke-dasharray: none;
			stroke-dashoffset: 0;
		}
	}
</style>
