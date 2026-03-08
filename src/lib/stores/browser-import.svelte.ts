/**
 * ImpForge Browser Import Store — Svelte 5 runes
 *
 * Auto-detect installed browsers and import:
 * - Bookmarks (Chrome JSON, Firefox SQLite)
 * - History (SQLite for both)
 * - No file download required — reads browser profiles directly
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface BrowserProfile {
	browser_name: string;
	browser_type: string;
	profile_path: string;
	profile_name: string;
	has_bookmarks: boolean;
	has_history: boolean;
	has_passwords: boolean;
}

export interface Bookmark {
	title: string;
	url: string;
	folder: string;
	date_added: number | null;
}

export interface HistoryEntry {
	url: string;
	title: string;
	visit_count: number;
	last_visit: string;
}

export interface ImportResult {
	browser_name: string;
	bookmarks_imported: number;
	history_imported: number;
	errors: string[];
}

// ============================================================================
// STATE
// ============================================================================

let profiles = $state<BrowserProfile[]>([]);
let importedBookmarks = $state<Bookmark[]>([]);
let importedHistory = $state<HistoryEntry[]>([]);
let lastImportResult = $state<ImportResult | null>(null);
let isScanning = $state(false);
let isImporting = $state(false);
let error = $state<string | null>(null);

// ============================================================================
// ACTIONS
// ============================================================================

/** Detect all browser profiles on the system */
async function detectProfiles(): Promise<BrowserProfile[]> {
	isScanning = true;
	error = null;
	try {
		const result = await invoke<BrowserProfile[]>('browser_detect_profiles');
		profiles = result;
		return result;
	} catch (e) {
		error = String(e);
		return [];
	} finally {
		isScanning = false;
	}
}

/** Import bookmarks from a browser profile */
async function importBookmarks(profile: BrowserProfile): Promise<Bookmark[]> {
	isImporting = true;
	error = null;
	try {
		const result = await invoke<Bookmark[]>('browser_import_bookmarks', {
			profilePath: profile.profile_path,
			browserType: profile.browser_type
		});
		importedBookmarks = result;
		return result;
	} catch (e) {
		error = String(e);
		return [];
	} finally {
		isImporting = false;
	}
}

/** Import history from a browser profile */
async function importHistory(
	profile: BrowserProfile,
	limit?: number
): Promise<HistoryEntry[]> {
	isImporting = true;
	error = null;
	try {
		const result = await invoke<HistoryEntry[]>('browser_import_history', {
			profilePath: profile.profile_path,
			browserType: profile.browser_type,
			limit: limit ?? 500
		});
		importedHistory = result;
		return result;
	} catch (e) {
		error = String(e);
		return [];
	} finally {
		isImporting = false;
	}
}

/** Import everything from a browser profile */
async function importAll(profile: BrowserProfile): Promise<ImportResult | null> {
	isImporting = true;
	error = null;
	try {
		const result = await invoke<ImportResult>('browser_import_all', {
			profilePath: profile.profile_path,
			browserType: profile.browser_type,
			browserName: profile.browser_name
		});
		lastImportResult = result;
		return result;
	} catch (e) {
		error = String(e);
		return null;
	} finally {
		isImporting = false;
	}
}

/** Clear imported data */
function clearImported() {
	importedBookmarks = [];
	importedHistory = [];
	lastImportResult = null;
	error = null;
}

// ============================================================================
// EXPORT
// ============================================================================

export const browserImportStore = {
	get profiles() {
		return profiles;
	},
	get importedBookmarks() {
		return importedBookmarks;
	},
	get importedHistory() {
		return importedHistory;
	},
	get lastImportResult() {
		return lastImportResult;
	},
	get isScanning() {
		return isScanning;
	},
	get isImporting() {
		return isImporting;
	},
	get error() {
		return error;
	},
	detectProfiles,
	importBookmarks,
	importHistory,
	importAll,
	clearImported
};
