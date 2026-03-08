/**
 * NEXUS Layout Manager — ElvUI-Style Modular Component System
 *
 * Manages widget positions, grid overlay, movers (drag handles),
 * snap-to-grid, and profile-based layout persistence.
 *
 * Architecture inspired by ElvUI/BenikUI:
 * - Every UI component becomes a "mover" when edit mode is active
 * - Movers can be dragged, resized, and snapped to grid
 * - Positions are stored as grid coordinates (x, y, w, h)
 * - Layouts are persisted per-route and exportable as profile strings
 */

import { invoke } from '@tauri-apps/api/core';
import { themeStore, type WidgetPlacement, type WidgetLayout, type WidgetDefinition } from './theme.svelte';

// ============================================================================
// TYPES
// ============================================================================

export interface MoverState {
	widgetId: string;
	x: number;
	y: number;
	w: number;
	h: number;
	isDragging: boolean;
	isResizing: boolean;
	config: Record<string, unknown>;
}

export interface GridConfig {
	columns: number;
	rows: number;
	gap: number;
	cellWidth: number;
	cellHeight: number;
	snapThreshold: number;
}

export interface DragState {
	moverId: string | null;
	startX: number;
	startY: number;
	offsetX: number;
	offsetY: number;
}

// ============================================================================
// STATE
// ============================================================================

let editMode = $state(false);
let showGrid = $state(false);
let movers = $state<Map<string, MoverState>>(new Map());
let selectedMover = $state<string | null>(null);
let dragState = $state<DragState>({ moverId: null, startX: 0, startY: 0, offsetX: 0, offsetY: 0 });
let gridConfig = $state<GridConfig>({
	columns: 24,
	rows: 16,
	gap: 4,
	cellWidth: 0,
	cellHeight: 0,
	snapThreshold: 8,
});
let currentRoute = $state('/');
let isDirty = $state(false);

// ============================================================================
// GRID CALCULATION
// ============================================================================

/** Recalculate grid cell dimensions based on container size */
function updateGridDimensions(containerWidth: number, containerHeight: number) {
	gridConfig.cellWidth = (containerWidth - (gridConfig.columns + 1) * gridConfig.gap) / gridConfig.columns;
	gridConfig.cellHeight = (containerHeight - (gridConfig.rows + 1) * gridConfig.gap) / gridConfig.rows;
}

/** Convert grid coordinates to pixel position */
function gridToPixel(gridX: number, gridY: number): { x: number; y: number } {
	return {
		x: gridX * (gridConfig.cellWidth + gridConfig.gap) + gridConfig.gap,
		y: gridY * (gridConfig.cellHeight + gridConfig.gap) + gridConfig.gap,
	};
}

/** Convert pixel position to grid coordinates (with snap) */
function pixelToGrid(pixelX: number, pixelY: number): { x: number; y: number } {
	const rawX = (pixelX - gridConfig.gap) / (gridConfig.cellWidth + gridConfig.gap);
	const rawY = (pixelY - gridConfig.gap) / (gridConfig.cellHeight + gridConfig.gap);
	return {
		x: Math.max(0, Math.min(Math.round(rawX), gridConfig.columns - 1)),
		y: Math.max(0, Math.min(Math.round(rawY), gridConfig.rows - 1)),
	};
}

/** Get pixel dimensions for a widget at given grid size */
function gridSizeToPixel(w: number, h: number): { width: number; height: number } {
	return {
		width: w * gridConfig.cellWidth + (w - 1) * gridConfig.gap,
		height: h * gridConfig.cellHeight + (h - 1) * gridConfig.gap,
	};
}

// ============================================================================
// MOVER ACTIONS
// ============================================================================

/** Toggle edit mode (unlock/lock movers) */
function toggleEditMode() {
	editMode = !editMode;
	if (!editMode) {
		showGrid = false;
		selectedMover = null;
		if (isDirty) {
			saveCurrentLayout();
			isDirty = false;
		}
	} else {
		showGrid = true;
	}
}

/** Start dragging a mover */
function startDrag(moverId: string, clientX: number, clientY: number) {
	if (!editMode) return;
	const mover = movers.get(moverId);
	if (!mover) return;

	const pos = gridToPixel(mover.x, mover.y);
	dragState = {
		moverId,
		startX: clientX,
		startY: clientY,
		offsetX: clientX - pos.x,
		offsetY: clientY - pos.y,
	};

	const updated = new Map(movers);
	updated.set(moverId, { ...mover, isDragging: true });
	movers = updated;
	selectedMover = moverId;
}

/** Handle drag move */
function onDragMove(clientX: number, clientY: number) {
	if (!dragState.moverId) return;
	const mover = movers.get(dragState.moverId);
	if (!mover) return;

	const newPixelX = clientX - dragState.offsetX;
	const newPixelY = clientY - dragState.offsetY;
	const gridPos = pixelToGrid(newPixelX, newPixelY);

	// Clamp to grid bounds
	const maxX = gridConfig.columns - mover.w;
	const maxY = gridConfig.rows - mover.h;
	gridPos.x = Math.max(0, Math.min(gridPos.x, maxX));
	gridPos.y = Math.max(0, Math.min(gridPos.y, maxY));

	if (gridPos.x !== mover.x || gridPos.y !== mover.y) {
		const updated = new Map(movers);
		updated.set(dragState.moverId, { ...mover, x: gridPos.x, y: gridPos.y });
		movers = updated;
		isDirty = true;
	}
}

/** End drag */
function endDrag() {
	if (!dragState.moverId) return;
	const mover = movers.get(dragState.moverId);
	if (mover) {
		const updated = new Map(movers);
		updated.set(dragState.moverId, { ...mover, isDragging: false });
		movers = updated;
	}
	dragState = { moverId: null, startX: 0, startY: 0, offsetX: 0, offsetY: 0 };
}

/** Resize a mover */
function resizeMover(moverId: string, newW: number, newH: number) {
	const mover = movers.get(moverId);
	if (!mover) return;

	// Get widget definition for min/max
	const widget = themeStore.widgets.find(w => w.id === moverId);
	const minW = widget?.min_size[0] ?? 1;
	const minH = widget?.min_size[1] ?? 1;
	const maxW = widget?.max_size[0] ?? gridConfig.columns;
	const maxH = widget?.max_size[1] ?? gridConfig.rows;

	const clampedW = Math.max(minW, Math.min(newW, maxW, gridConfig.columns - mover.x));
	const clampedH = Math.max(minH, Math.min(newH, maxH, gridConfig.rows - mover.y));

	if (clampedW !== mover.w || clampedH !== mover.h) {
		const updated = new Map(movers);
		updated.set(moverId, { ...mover, w: clampedW, h: clampedH });
		movers = updated;
		isDirty = true;
	}
}

/** Remove a mover from the layout */
function removeMover(moverId: string) {
	const updated = new Map(movers);
	updated.delete(moverId);
	movers = updated;
	if (selectedMover === moverId) selectedMover = null;
	isDirty = true;
}

/** Add a widget to the layout */
function addWidget(widgetId: string, x?: number, y?: number) {
	if (movers.has(widgetId)) return;

	const widget = themeStore.widgets.find(w => w.id === widgetId);
	const defaultW = widget?.default_size[0] ?? 4;
	const defaultH = widget?.default_size[1] ?? 3;

	// Find first available position if not specified
	const posX = x ?? findFreePosition(defaultW, defaultH).x;
	const posY = y ?? findFreePosition(defaultW, defaultH).y;

	const updated = new Map(movers);
	updated.set(widgetId, {
		widgetId,
		x: posX,
		y: posY,
		w: defaultW,
		h: defaultH,
		isDragging: false,
		isResizing: false,
		config: {},
	});
	movers = updated;
	isDirty = true;
}

/** Find first free grid position for a widget of given size */
function findFreePosition(w: number, h: number): { x: number; y: number } {
	const occupied = new Set<string>();
	for (const mover of movers.values()) {
		for (let gx = mover.x; gx < mover.x + mover.w; gx++) {
			for (let gy = mover.y; gy < mover.y + mover.h; gy++) {
				occupied.add(`${gx},${gy}`);
			}
		}
	}

	for (let y = 0; y <= gridConfig.rows - h; y++) {
		for (let x = 0; x <= gridConfig.columns - w; x++) {
			let fits = true;
			for (let gx = x; gx < x + w && fits; gx++) {
				for (let gy = y; gy < y + h && fits; gy++) {
					if (occupied.has(`${gx},${gy}`)) fits = false;
				}
			}
			if (fits) return { x, y };
		}
	}
	return { x: 0, y: 0 };
}

// ============================================================================
// PERSISTENCE
// ============================================================================

/** Load layout for current route from backend */
async function loadLayout(route: string) {
	currentRoute = route;
	try {
		await themeStore.loadLayout(route);
		const layout = themeStore.currentLayout;
		if (layout) {
			const newMovers = new Map<string, MoverState>();
			for (const placement of layout.widgets) {
				newMovers.set(placement.widget_id, {
					widgetId: placement.widget_id,
					x: placement.x,
					y: placement.y,
					w: placement.w,
					h: placement.h,
					isDragging: false,
					isResizing: false,
					config: placement.config as Record<string, unknown>,
				});
			}
			movers = newMovers;
		}
	} catch (e) {
		console.error('Failed to load layout:', e);
	}
}

/** Save current layout to backend */
async function saveCurrentLayout() {
	const placements: WidgetPlacement[] = [];
	for (const mover of movers.values()) {
		placements.push({
			widget_id: mover.widgetId,
			x: mover.x,
			y: mover.y,
			w: mover.w,
			h: mover.h,
			config: mover.config,
		});
	}

	const layout: WidgetLayout = {
		id: `layout-${currentRoute.replace(/\//g, '-')}`,
		name: `Layout for ${currentRoute}`,
		widgets: placements,
		route: currentRoute,
	};

	await themeStore.saveLayout(layout);
}

/** Reset layout to default positions */
function resetLayout() {
	movers = new Map();
	isDirty = true;
}

// ============================================================================
// EXPORT
// ============================================================================

export const layoutManager = {
	get editMode() { return editMode; },
	get showGrid() { return showGrid; },
	set showGrid(v: boolean) { showGrid = v; },
	get movers() { return movers; },
	get selectedMover() { return selectedMover; },
	set selectedMover(id: string | null) { selectedMover = id; },
	get gridConfig() { return gridConfig; },
	get isDirty() { return isDirty; },
	get currentRoute() { return currentRoute; },
	get dragState() { return dragState; },

	// Grid
	updateGridDimensions,
	gridToPixel,
	pixelToGrid,
	gridSizeToPixel,

	// Movers
	toggleEditMode,
	startDrag,
	onDragMove,
	endDrag,
	resizeMover,
	removeMover,
	addWidget,

	// Persistence
	loadLayout,
	saveCurrentLayout,
	resetLayout,
};
