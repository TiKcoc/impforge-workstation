<script lang="ts">
	/**
	 * LayoutGrid.svelte — Visual Grid Overlay for Edit Mode
	 *
	 * Renders a 24×16 grid overlay when edit mode is active.
	 * Grid lines appear as subtle guides for widget snap alignment.
	 * Uses CSS Grid for pixel-perfect rendering.
	 *
	 * Features:
	 * - Responsive to container resize (ResizeObserver)
	 * - Grid cell highlighting on hover
	 * - Fade in/out animation
	 */

	import { layoutManager } from '$lib/stores/layout-manager.svelte';
	import { styleEngine } from '$lib/stores/style-engine.svelte';
	import StyleEditor from './StyleEditor.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();

	let containerEl = $state<HTMLDivElement>(undefined!);
	let hoverCell = $state<{ x: number; y: number } | null>(null);

	let grid = $derived(layoutManager.gridConfig);
	let showGrid = $derived(layoutManager.showGrid);

	// Build grid cells array for rendering
	let cells = $derived.by(() => {
		const result: { x: number; y: number; key: string }[] = [];
		for (let y = 0; y < grid.rows; y++) {
			for (let x = 0; x < grid.columns; x++) {
				result.push({ x, y, key: `${x}-${y}` });
			}
		}
		return result;
	});

	// ResizeObserver to track container dimensions
	$effect(() => {
		if (!containerEl) return;
		const observer = new ResizeObserver((entries) => {
			const entry = entries[0];
			if (entry) {
				layoutManager.updateGridDimensions(
					entry.contentRect.width,
					entry.contentRect.height
				);
			}
		});
		observer.observe(containerEl);
		return () => observer.disconnect();
	});

	function handleCellHover(x: number, y: number) {
		hoverCell = { x, y };
	}

	function handleCellLeave() {
		hoverCell = null;
	}

	function handleMouseMove(e: MouseEvent) {
		if (!containerEl) return;
		layoutManager.onDragMove(e.clientX, e.clientY);
	}

	function handleMouseUp() {
		layoutManager.endDrag();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={containerEl}
	class="absolute inset-0 overflow-hidden"
	onmousemove={handleMouseMove}
	onmouseup={handleMouseUp}
	onmouseleave={handleMouseUp}
>
	<!-- Grid overlay (visible in edit mode) -->
	{#if showGrid && grid.cellWidth > 0}
		<div
			class="absolute inset-0 transition-opacity duration-300"
			style="
				display: grid;
				grid-template-columns: repeat({grid.columns}, {grid.cellWidth}px);
				grid-template-rows: repeat({grid.rows}, {grid.cellHeight}px);
				gap: {grid.gap}px;
				padding: {grid.gap}px;
				opacity: 0.4;
				pointer-events: none;
			"
		>
			{#each cells as cell (cell.key)}
				<div
					class="rounded-sm border transition-colors duration-100
						{hoverCell?.x === cell.x && hoverCell?.y === cell.y
							? 'border-gx-neon/40 bg-gx-neon/10'
							: 'border-gx-border-default/30 bg-transparent'}"
				></div>
			{/each}
		</div>
	{/if}

	<!-- Content (widgets render here) -->
	{@render children()}

	<!-- Style Editor side panel (BenikUI sub-component editor) -->
	{#if styleEngine.editingComponent}
		<div class="absolute top-0 right-0 bottom-0 z-[60]">
			<StyleEditor
				widgetId={styleEngine.editingComponent}
				onClose={() => { styleEngine.editingComponent = null; }}
			/>
		</div>
	{/if}
</div>
