<script lang="ts">
	/**
	 * Mover.svelte — ElvUI-Style Draggable Widget Container
	 *
	 * When edit mode is active, this component renders a drag handle overlay
	 * on top of the widget content. Supports:
	 * - Drag to reposition (grid-snapped)
	 * - Resize from corners (min/max constrained)
	 * - Visual feedback: glow border, size indicator, widget name
	 * - Selection state with neon highlight
	 */

	import { layoutManager, type MoverState } from '$lib/stores/layout-manager.svelte';
	import { styleEngine } from '$lib/stores/style-engine.svelte';
	import { Move, X, Maximize2, Paintbrush } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		moverId: string;
		label?: string;
		children: Snippet;
	}

	let { moverId, label = moverId, children }: Props = $props();

	let mover = $derived(layoutManager.movers.get(moverId));
	let isSelected = $derived(layoutManager.selectedMover === moverId);
	let editMode = $derived(layoutManager.editMode);

	// Pixel position & size from grid coordinates
	let pixelPos = $derived(mover ? layoutManager.gridToPixel(mover.x, mover.y) : { x: 0, y: 0 });
	let pixelSize = $derived(mover ? layoutManager.gridSizeToPixel(mover.w, mover.h) : { width: 0, height: 0 });

	// Resize state
	let isResizing = $state(false);
	let resizeStartX = $state(0);
	let resizeStartY = $state(0);
	let resizeStartW = $state(0);
	let resizeStartH = $state(0);

	function handleMouseDown(e: MouseEvent) {
		if (!editMode || !mover) return;
		e.preventDefault();
		e.stopPropagation();
		layoutManager.startDrag(moverId, e.clientX, e.clientY);
	}

	function handleResizeStart(e: MouseEvent) {
		if (!editMode || !mover) return;
		e.preventDefault();
		e.stopPropagation();
		isResizing = true;
		resizeStartX = e.clientX;
		resizeStartY = e.clientY;
		resizeStartW = mover.w;
		resizeStartH = mover.h;

		const onMove = (ev: MouseEvent) => {
			const grid = layoutManager.gridConfig;
			const cellW = grid.cellWidth + grid.gap;
			const cellH = grid.cellHeight + grid.gap;
			const deltaX = Math.round((ev.clientX - resizeStartX) / cellW);
			const deltaY = Math.round((ev.clientY - resizeStartY) / cellH);
			layoutManager.resizeMover(moverId, resizeStartW + deltaX, resizeStartH + deltaY);
		};

		const onUp = () => {
			isResizing = false;
			window.removeEventListener('mousemove', onMove);
			window.removeEventListener('mouseup', onUp);
		};

		window.addEventListener('mousemove', onMove);
		window.addEventListener('mouseup', onUp);
	}

	function handleRemove(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		layoutManager.removeMover(moverId);
	}

	function handleOpenStyleEditor(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		// Set this widget as the one being styled — StyleEditor reads this
		styleEngine.editingComponent = moverId;
		styleEngine.loadWidgetStyle(moverId);
	}

	function handleSelect(e: MouseEvent) {
		e.stopPropagation();
		layoutManager.selectedMover = moverId;
	}

	/** WCAG 2.2 Dragging Movements (2.5.7) — keyboard alternative to drag
	 * Arrow keys move by 1 grid cell, Shift+Arrow resizes by 1 cell */
	function handleKeydown(e: KeyboardEvent) {
		if (!editMode || !mover) return;
		const isShift = e.shiftKey;
		let handled = true;

		switch (e.key) {
			case 'Enter':
			case ' ':
				layoutManager.selectedMover = moverId;
				break;
			case 'ArrowUp':
				if (isShift) layoutManager.resizeMover(moverId, mover.w, mover.h - 1);
				else layoutManager.moveMover(moverId, mover.x, mover.y - 1);
				break;
			case 'ArrowDown':
				if (isShift) layoutManager.resizeMover(moverId, mover.w, mover.h + 1);
				else layoutManager.moveMover(moverId, mover.x, mover.y + 1);
				break;
			case 'ArrowLeft':
				if (isShift) layoutManager.resizeMover(moverId, mover.w - 1, mover.h);
				else layoutManager.moveMover(moverId, mover.x - 1, mover.y);
				break;
			case 'ArrowRight':
				if (isShift) layoutManager.resizeMover(moverId, mover.w + 1, mover.h);
				else layoutManager.moveMover(moverId, mover.x + 1, mover.y);
				break;
			case 'Delete':
			case 'Backspace':
				layoutManager.removeMover(moverId);
				break;
			default:
				handled = false;
		}
		if (handled) {
			e.preventDefault();
			e.stopPropagation();
		}
	}
</script>

{#if mover}
	<!-- svelte-ignore a11y_no_static_element_interactions a11y_no_noninteractive_tabindex -->
	<div
		role={editMode ? 'button' : undefined}
		tabindex={editMode ? 0 : -1}
		aria-label="{label} widget{editMode ? ` — position ${mover.x},${mover.y}, size ${mover.w}×${mover.h}. Arrow keys to move, Shift+Arrow to resize, Delete to remove.` : ''}"
		aria-roledescription={editMode ? 'draggable widget' : undefined}
		class="absolute transition-[left,top] duration-75"
		style="left: {pixelPos.x}px; top: {pixelPos.y}px; width: {pixelSize.width}px; height: {pixelSize.height}px; z-index: {editMode ? 50 : 1};"
		onclick={handleSelect}
		onkeydown={editMode ? handleKeydown : undefined}
		onmousedown={editMode ? handleMouseDown : undefined}
	>
		<!-- Widget content (always rendered) -->
		<div
			class="w-full h-full overflow-hidden rounded-gx"
			class:pointer-events-none={editMode}
		>
			{@render children()}
		</div>

		<!-- Edit mode overlay (ElvUI mover handle) -->
		{#if editMode}
			<div
				class="absolute inset-0 rounded-gx border-2 transition-all duration-150 cursor-move
					{isSelected
						? 'border-gx-neon bg-gx-neon/5 shadow-[0_0_20px_rgba(0,255,102,0.3)]'
						: 'border-gx-accent-purple/60 bg-gx-accent-purple/5 hover:border-gx-neon/80 hover:bg-gx-neon/5'}"
			>
				<!-- Top bar with label + controls -->
				<div class="absolute top-0 left-0 right-0 flex items-center gap-1 px-1.5 py-0.5 bg-black/70 rounded-t-gx">
					<Move size={10} class="text-gx-text-muted shrink-0" />
					<span class="text-[9px] font-mono text-gx-text-secondary truncate flex-1">{label}</span>

					<!-- Size indicator -->
					<span class="text-[8px] font-mono text-gx-text-muted shrink-0">
						{mover.w}×{mover.h}
					</span>

					<!-- Style editor button (BenikUI) -->
					<button
						onclick={handleOpenStyleEditor}
						class="p-0.5 rounded hover:bg-gx-accent-purple/20 text-gx-text-muted hover:text-gx-accent-purple transition-colors shrink-0"
						title="Customize style (BenikUI)"
					>
						<Paintbrush size={10} />
					</button>

					<!-- Remove button -->
					<button
						onclick={handleRemove}
						class="p-0.5 rounded hover:bg-gx-status-error/20 text-gx-text-muted hover:text-gx-status-error transition-colors shrink-0"
						title="Remove widget"
					>
						<X size={10} />
					</button>
				</div>

				<!-- Grid position indicator (bottom-left) -->
				<div class="absolute bottom-0 left-0 px-1.5 py-0.5 bg-black/70 rounded-br-gx">
					<span class="text-[8px] font-mono text-gx-text-muted">
						({mover.x},{mover.y})
					</span>
				</div>

				<!-- Resize handle (bottom-right corner) -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="absolute bottom-0 right-0 w-5 h-5 cursor-se-resize flex items-end justify-end p-0.5
						{isResizing ? 'text-gx-neon' : 'text-gx-text-muted hover:text-gx-neon'}"
					onmousedown={handleResizeStart}
				>
					<Maximize2 size={10} class="rotate-90" />
				</div>
			</div>
		{/if}
	</div>
{/if}
