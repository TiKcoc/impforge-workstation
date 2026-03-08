/**
 * NEXUS CDP Browser Store — Svelte 5 runes
 *
 * Full browser automation via Chrome DevTools Protocol.
 * Controls CDP pages: navigate, click, fill, screenshot, JS execution.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface BrowserInstallation {
	name: string;
	path: string;
	browser_type: string;
}

export interface PageInfo {
	page_id: string;
	url: string;
	title: string;
}

export interface CdpNavigateResult {
	page_id: string;
	url: string;
	title: string;
	content_length: number;
}

export interface ElementInfo {
	tag: string;
	id: string | null;
	classes: string[];
	text_preview: string | null;
	selector: string;
	bounding_box: { x: number; y: number; width: number; height: number } | null;
	attributes: [string, string][];
}

// ============================================================================
// STATE
// ============================================================================

let pages = $state<PageInfo[]>([]);
let activePage = $state<string | null>(null);
let installedBrowsers = $state<BrowserInstallation[]>([]);
let isLoading = $state(false);
let error = $state<string | null>(null);
let lastScreenshot = $state<string | null>(null);
let jsResult = $state<unknown>(null);
let elements = $state<ElementInfo[]>([]);

// ============================================================================
// ACTIONS
// ============================================================================

/** Detect installed Chromium-based browsers */
async function detectBrowsers(): Promise<BrowserInstallation[]> {
	try {
		const result = await invoke<BrowserInstallation[]>('cdp_detect_browsers');
		installedBrowsers = result;
		return result;
	} catch (e) {
		error = String(e);
		return [];
	}
}

/** Open a new CDP page at URL */
async function openPage(url: string): Promise<PageInfo | null> {
	isLoading = true;
	error = null;
	try {
		const info = await invoke<PageInfo>('cdp_open_page', { url });
		pages = [...pages, info];
		activePage = info.page_id;
		return info;
	} catch (e) {
		error = String(e);
		return null;
	} finally {
		isLoading = false;
	}
}

/** Navigate active page to URL */
async function navigate(url: string): Promise<CdpNavigateResult | null> {
	if (!activePage) {
		error = 'No active page';
		return null;
	}
	isLoading = true;
	error = null;
	try {
		const result = await invoke<CdpNavigateResult>('cdp_navigate', {
			pageId: activePage,
			url
		});
		// Update page info
		pages = pages.map((p) =>
			p.page_id === activePage ? { ...p, url: result.url, title: result.title } : p
		);
		return result;
	} catch (e) {
		error = String(e);
		return null;
	} finally {
		isLoading = false;
	}
}

/** Click an element by CSS selector */
async function click(selector: string): Promise<string | null> {
	if (!activePage) return null;
	try {
		return await invoke<string>('cdp_click', { pageId: activePage, selector });
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Fill a form field */
async function fill(selector: string, value: string): Promise<string | null> {
	if (!activePage) return null;
	try {
		return await invoke<string>('cdp_fill', { pageId: activePage, selector, value });
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Execute JavaScript */
async function executeJs(script: string): Promise<unknown> {
	if (!activePage) return null;
	try {
		const result = await invoke<unknown>('cdp_execute_js', { pageId: activePage, script });
		jsResult = result;
		return result;
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Extract text matching CSS selector */
async function extract(selector: string): Promise<string | null> {
	if (!activePage) return null;
	try {
		return await invoke<string>('cdp_extract', { pageId: activePage, selector });
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Take a screenshot (returns base64 PNG) */
async function screenshot(fullPage?: boolean): Promise<string | null> {
	if (!activePage) return null;
	isLoading = true;
	try {
		const b64 = await invoke<string>('cdp_screenshot', {
			pageId: activePage,
			fullPage: fullPage ?? false
		});
		lastScreenshot = b64;
		return b64;
	} catch (e) {
		error = String(e);
		return null;
	} finally {
		isLoading = false;
	}
}

/** Get full page HTML */
async function getContent(): Promise<string | null> {
	if (!activePage) return null;
	try {
		return await invoke<string>('cdp_get_page_content', { pageId: activePage });
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Scroll the page */
async function scroll(direction: 'up' | 'down' | 'top' | 'bottom'): Promise<void> {
	if (!activePage) return;
	try {
		await invoke<string>('cdp_page_scroll', { pageId: activePage, direction });
	} catch (e) {
		error = String(e);
	}
}

/** Close a page */
async function closePage(pageId?: string): Promise<void> {
	const id = pageId ?? activePage;
	if (!id) return;
	try {
		await invoke<void>('cdp_close_page', { pageId: id });
		pages = pages.filter((p) => p.page_id !== id);
		if (activePage === id) {
			activePage = pages.length > 0 ? pages[pages.length - 1].page_id : null;
		}
	} catch (e) {
		error = String(e);
	}
}

/** Get interactive elements on the page (for visual picker) */
async function getElements(selector?: string): Promise<ElementInfo[]> {
	if (!activePage) return [];
	try {
		const result = await invoke<ElementInfo[]>('cdp_get_elements', {
			pageId: activePage,
			selector: selector ?? null
		});
		elements = result;
		return result;
	} catch (e) {
		error = String(e);
		return [];
	}
}

/** Highlight an element on the page with neon glow */
async function highlightElement(selector: string): Promise<void> {
	if (!activePage) return;
	try {
		await invoke<string>('cdp_highlight_element', { pageId: activePage, selector });
	} catch (e) {
		error = String(e);
	}
}

/** Refresh pages list from backend */
async function refreshPages(): Promise<void> {
	try {
		pages = await invoke<PageInfo[]>('cdp_pages');
	} catch (e) {
		error = String(e);
	}
}

// ============================================================================
// EXPORT
// ============================================================================

export const cdpStore = {
	get pages() {
		return pages;
	},
	get activePage() {
		return activePage;
	},
	set activePage(id: string | null) {
		activePage = id;
	},
	get installedBrowsers() {
		return installedBrowsers;
	},
	get isLoading() {
		return isLoading;
	},
	get error() {
		return error;
	},
	get lastScreenshot() {
		return lastScreenshot;
	},
	get jsResult() {
		return jsResult;
	},
	get elements() {
		return elements;
	},
	detectBrowsers,
	openPage,
	navigate,
	click,
	fill,
	executeJs,
	extract,
	screenshot,
	getContent,
	scroll,
	closePage,
	refreshPages,
	getElements,
	highlightElement
};
