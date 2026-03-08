/**
 * ImpForge Theme Store — Svelte 5 runes
 *
 * Manages theme switching, CSS variable injection, and custom theme creation.
 * Themes are persisted in SQLite via Tauri backend.
 *
 * Architecture inspired by ElvUI's profile system:
 * - Built-in themes ship with the app
 * - Custom themes override CSS variables at :root level
 * - Export/Import via base64 strings (like ElvUI profile strings)
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface NexusTheme {
	id: string;
	name: string;
	author: string | null;
	version: string;
	variables: [string, string][];
	is_builtin: boolean;
}

export interface WidgetDefinition {
	id: string;
	name: string;
	description: string;
	category: string;
	default_size: [number, number];
	min_size: [number, number];
	max_size: [number, number];
	configurable: boolean;
}

export interface WidgetPlacement {
	widget_id: string;
	x: number;
	y: number;
	w: number;
	h: number;
	config: Record<string, unknown>;
}

export interface WidgetLayout {
	id: string;
	name: string;
	widgets: WidgetPlacement[];
	route: string;
}

// ============================================================================
// STATE
// ============================================================================

let themes = $state<NexusTheme[]>([]);
let activeTheme = $state<NexusTheme | null>(null);
let widgets = $state<WidgetDefinition[]>([]);
let currentLayout = $state<WidgetLayout | null>(null);
let isLoading = $state(false);
let error = $state<string | null>(null);

// ============================================================================
// THEME ACTIONS
// ============================================================================

/** Apply theme CSS variables to document root */
function applyTheme(theme: NexusTheme) {
	const root = document.documentElement;
	// Remove previous custom overrides
	root.style.cssText = '';
	// Apply theme variables
	for (const [name, value] of theme.variables) {
		root.style.setProperty(name, value);
	}
}

/** Load all themes and apply active one */
async function loadThemes(): Promise<void> {
	isLoading = true;
	try {
		themes = await invoke<NexusTheme[]>('theme_list');
		const active = await invoke<NexusTheme>('theme_get_active');
		activeTheme = active;
		applyTheme(active);
	} catch (e) {
		error = String(e);
	} finally {
		isLoading = false;
	}
}

/** Switch to a different theme */
async function setTheme(themeId: string): Promise<void> {
	try {
		await invoke<string>('theme_set_active', { themeId });
		const theme = themes.find((t) => t.id === themeId);
		if (theme) {
			activeTheme = theme;
			applyTheme(theme);
		}
	} catch (e) {
		error = String(e);
	}
}

/** Save a custom theme */
async function saveCustomTheme(theme: NexusTheme): Promise<void> {
	try {
		await invoke<string>('theme_save', { theme });
		await loadThemes();
	} catch (e) {
		error = String(e);
	}
}

/** Delete a custom theme */
async function deleteTheme(themeId: string): Promise<void> {
	try {
		await invoke<string>('theme_delete', { themeId });
		themes = themes.filter((t) => t.id !== themeId);
	} catch (e) {
		error = String(e);
	}
}

/** Export a theme as base64 string */
async function exportTheme(themeId: string): Promise<string | null> {
	try {
		return await invoke<string>('theme_export', { themeId });
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Import a theme from base64 string */
async function importTheme(encoded: string): Promise<void> {
	try {
		await invoke<NexusTheme>('theme_import', { encoded });
		await loadThemes();
	} catch (e) {
		error = String(e);
	}
}

// ============================================================================
// WIDGET ACTIONS
// ============================================================================

/** Load available widgets from registry */
async function loadWidgets(): Promise<void> {
	try {
		widgets = await invoke<WidgetDefinition[]>('widget_list');
	} catch (e) {
		error = String(e);
	}
}

/** Load layout for current route */
async function loadLayout(route: string): Promise<void> {
	try {
		currentLayout = await invoke<WidgetLayout | null>('layout_get', { route });
	} catch (e) {
		error = String(e);
	}
}

/** Save current layout */
async function saveLayout(layout: WidgetLayout): Promise<void> {
	try {
		await invoke<string>('layout_save', { layout });
		currentLayout = layout;
	} catch (e) {
		error = String(e);
	}
}

/** Delete a layout */
async function deleteLayout(layoutId: string): Promise<void> {
	try {
		await invoke<string>('layout_delete', { layoutId });
		if (currentLayout?.id === layoutId) currentLayout = null;
	} catch (e) {
		error = String(e);
	}
}

// ============================================================================
// EXPORT
// ============================================================================

export const themeStore = {
	get themes() {
		return themes;
	},
	get activeTheme() {
		return activeTheme;
	},
	get widgets() {
		return widgets;
	},
	get currentLayout() {
		return currentLayout;
	},
	get isLoading() {
		return isLoading;
	},
	get error() {
		return error;
	},
	loadThemes,
	setTheme,
	saveCustomTheme,
	deleteTheme,
	exportTheme,
	importTheme,
	applyTheme,
	loadWidgets,
	loadLayout,
	saveLayout,
	deleteLayout
};
