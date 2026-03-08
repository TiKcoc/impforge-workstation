/**
 * ImpForge Browser Playground Store — Svelte 5 runes
 *
 * Unified devtools store: Network waterfall, Console, Performance, Cookies.
 * Polls CDP backend for live updates when playground is active.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface NetworkEntry {
	request_id: string;
	url: string;
	method: string;
	status: number | null;
	mime_type: string | null;
	size_bytes: number | null;
	duration_ms: number | null;
	timestamp: number;
	resource_type: string;
}

export interface ConsoleEntry {
	level: string;
	text: string;
	timestamp: number;
	source: string;
	line_number: number | null;
	url: string | null;
}

export interface PerfMetrics {
	dom_content_loaded_ms: number | null;
	load_event_ms: number | null;
	first_paint_ms: number | null;
	first_contentful_paint_ms: number | null;
	dom_nodes: number | null;
	js_heap_size_mb: number | null;
	timestamp: number;
}

export interface CdpCookie {
	name: string;
	value: string;
	domain: string;
	path: string;
	secure: boolean;
	http_only: boolean;
	expires: number | null;
	same_site: string | null;
}

// ============================================================================
// STATE
// ============================================================================

let networkEntries = $state<NetworkEntry[]>([]);
let consoleEntries = $state<ConsoleEntry[]>([]);
let perfMetrics = $state<PerfMetrics | null>(null);
let cookies = $state<CdpCookie[]>([]);
let isPolling = $state(false);
let error = $state<string | null>(null);
let activeDevtoolsTab = $state<'network' | 'console' | 'performance' | 'cookies'>('network');

let pollInterval: ReturnType<typeof setInterval> | null = null;

// ============================================================================
// ACTIONS
// ============================================================================

/** Fetch network waterfall entries */
async function fetchNetwork(sinceTimestamp?: number): Promise<void> {
	try {
		networkEntries = await invoke<NetworkEntry[]>('cdp_network_entries', {
			sinceTimestamp: sinceTimestamp ?? null
		});
	} catch (e) {
		error = String(e);
	}
}

/** Enable CDP network monitoring on a page */
async function enableNetwork(pageId: string): Promise<void> {
	try {
		await invoke<string>('cdp_network_enable', { pageId });
	} catch (e) {
		error = String(e);
	}
}

/** Clear network log */
async function clearNetwork(): Promise<void> {
	await invoke<string>('cdp_network_clear');
	networkEntries = [];
}

/** Enable console capture on a page */
async function enableConsole(pageId: string): Promise<void> {
	try {
		await invoke<string>('cdp_console_enable', { pageId });
	} catch (e) {
		error = String(e);
	}
}

/** Flush console buffer from page into backend */
async function flushConsole(pageId: string): Promise<void> {
	try {
		const flushed = await invoke<ConsoleEntry[]>('cdp_console_flush', { pageId });
		consoleEntries = [...consoleEntries, ...flushed].slice(-200);
	} catch (e) {
		error = String(e);
	}
}

/** Get all console entries from backend */
async function fetchConsole(): Promise<void> {
	try {
		consoleEntries = await invoke<ConsoleEntry[]>('cdp_console_entries');
	} catch (e) {
		error = String(e);
	}
}

/** Clear console log */
async function clearConsole(): Promise<void> {
	await invoke<string>('cdp_console_clear');
	consoleEntries = [];
}

/** Fetch performance metrics for a page */
async function fetchPerf(pageId: string): Promise<void> {
	try {
		perfMetrics = await invoke<PerfMetrics>('cdp_perf_metrics', { pageId });
	} catch (e) {
		error = String(e);
	}
}

/** Fetch cookies for a page */
async function fetchCookies(pageId: string): Promise<void> {
	try {
		cookies = await invoke<CdpCookie[]>('cdp_get_cookies', { pageId });
	} catch (e) {
		error = String(e);
	}
}

/** Delete a cookie */
async function deleteCookie(pageId: string, name: string): Promise<void> {
	try {
		await invoke<string>('cdp_delete_cookie', { pageId, name });
		cookies = cookies.filter((c) => c.name !== name);
	} catch (e) {
		error = String(e);
	}
}

/** Start polling for network/console updates */
function startPolling(pageId: string, intervalMs: number = 2000): void {
	stopPolling();
	isPolling = true;
	pollInterval = setInterval(async () => {
		await fetchNetwork();
		await flushConsole(pageId);
	}, intervalMs);
}

/** Stop polling */
function stopPolling(): void {
	if (pollInterval) clearInterval(pollInterval);
	pollInterval = null;
	isPolling = false;
}

// ============================================================================
// EXPORT
// ============================================================================

export const playgroundStore = {
	get networkEntries() {
		return networkEntries;
	},
	get consoleEntries() {
		return consoleEntries;
	},
	get perfMetrics() {
		return perfMetrics;
	},
	get cookies() {
		return cookies;
	},
	get isPolling() {
		return isPolling;
	},
	get error() {
		return error;
	},
	get activeDevtoolsTab() {
		return activeDevtoolsTab;
	},
	set activeDevtoolsTab(tab: 'network' | 'console' | 'performance' | 'cookies') {
		activeDevtoolsTab = tab;
	},
	fetchNetwork,
	enableNetwork,
	clearNetwork,
	enableConsole,
	flushConsole,
	fetchConsole,
	clearConsole,
	fetchPerf,
	fetchCookies,
	deleteCookie,
	startPolling,
	stopPolling
};
