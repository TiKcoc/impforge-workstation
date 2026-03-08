/**
 * ImpForge Web Scraper Store — Svelte 5 runes
 *
 * Provides frontend access to the built-in web scraper (Rust, MIT-licensed)
 * and optional Firecrawl Cloud API (customer brings own key).
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface ScrapeResult {
	url: string;
	title: string | null;
	description: string | null;
	markdown: string;
	links: string[];
	word_count: number;
	success: boolean;
	error: string | null;
}

export interface ScrapeOptions {
	selectors?: string[];
	remove_selectors?: string[];
	timeout_secs?: number;
	include_links?: boolean;
	use_firecrawl?: boolean;
	firecrawl_api_key?: string;
}

export interface BatchScrapeResult {
	results: ScrapeResult[];
	total: number;
	succeeded: number;
	failed: number;
}

// ============================================================================
// STATE
// ============================================================================

let results = $state<ScrapeResult[]>([]);
let isLoading = $state(false);
let error = $state<string | null>(null);
let lastBatch = $state<BatchScrapeResult | null>(null);

// ============================================================================
// ACTIONS
// ============================================================================

/** Scrape a single URL */
async function scrape(url: string, options?: ScrapeOptions): Promise<ScrapeResult> {
	isLoading = true;
	error = null;
	try {
		const result = await invoke<ScrapeResult>('web_scrape', { url, options: options ?? null });
		results = [result, ...results].slice(0, 50); // keep last 50
		return result;
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isLoading = false;
	}
}

/** Scrape multiple URLs concurrently */
async function scrapeBatch(urls: string[], options?: ScrapeOptions): Promise<BatchScrapeResult> {
	isLoading = true;
	error = null;
	try {
		const batch = await invoke<BatchScrapeResult>('web_scrape_batch', {
			urls,
			options: options ?? null
		});
		lastBatch = batch;
		results = [...batch.results, ...results].slice(0, 50);
		return batch;
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isLoading = false;
	}
}

/** Extract metadata only (fast, no full content) */
async function extractMetadata(url: string): Promise<ScrapeResult> {
	isLoading = true;
	error = null;
	try {
		const result = await invoke<ScrapeResult>('web_extract_metadata', { url });
		return result;
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isLoading = false;
	}
}

/** Clear results history */
function clearResults() {
	results = [];
	lastBatch = null;
	error = null;
}

// ============================================================================
// EXPORT
// ============================================================================

export const scraperStore = {
	get results() {
		return results;
	},
	get isLoading() {
		return isLoading;
	},
	get error() {
		return error;
	},
	get lastBatch() {
		return lastBatch;
	},
	scrape,
	scrapeBatch,
	extractMetadata,
	clearResults
};
