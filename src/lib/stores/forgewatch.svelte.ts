/**
 * ForgeWatch Store — Filesystem watching + ingestion state management
 * Svelte 5 runes-based reactive store for ForgeWatch configuration.
 */

import { invoke } from '@tauri-apps/api/core';

export interface DiscoveredPath {
	path: string;
	reason: string;
	project_type: string | null;
	estimated_files: number;
}

export interface WatchedPath {
	path: string;
	label: string | null;
	recursive: boolean;
	enabled: boolean;
	scan_mode: 'Realtime' | 'Hourly' | 'Manual';
}

export interface WatchStatus {
	running: boolean;
	watched_paths: number;
	total_files_indexed: number;
	events_processed: number;
	last_event_at: string | null;
}

export interface IngestionResult {
	file_path: string;
	language: string | null;
	chunks_created: number;
	chunks_skipped: number;
	chunks_total: number;
	file_size_bytes: number;
}

class ForgeWatchStore {
	watchedPaths = $state<WatchedPath[]>([]);
	discoveredPaths = $state<DiscoveredPath[]>([]);
	status = $state<WatchStatus | null>(null);
	isDiscovering = $state(false);
	isReindexing = $state(false);
	error = $state<string | null>(null);
	lastReindexResult = $state<IngestionResult | null>(null);

	/** Refresh the list of watched paths from the backend. */
	async refreshPaths() {
		try {
			this.watchedPaths = await invoke<WatchedPath[]>('forge_watch_list_paths');
			this.error = null;
		} catch (e) {
			this.error = `Failed to list paths: ${e}`;
		}
	}

	/** Refresh the watcher status. */
	async refreshStatus() {
		try {
			this.status = await invoke<WatchStatus>('forge_watch_status');
			this.error = null;
		} catch (e) {
			this.error = `Failed to get status: ${e}`;
		}
	}

	/** Auto-discover projects in a directory (typically $HOME). */
	async discover(homePath: string) {
		this.isDiscovering = true;
		this.error = null;
		try {
			this.discoveredPaths = await invoke<DiscoveredPath[]>('forge_watch_discover', {
				homePath,
			});
		} catch (e) {
			this.error = `Discovery failed: ${e}`;
		} finally {
			this.isDiscovering = false;
		}
	}

	/** Add a path to the watch list. */
	async addPath(path: string, label?: string) {
		try {
			await invoke('forge_watch_add_path', { path, label: label ?? null });
			await this.refreshPaths();
			this.error = null;
		} catch (e) {
			this.error = `Failed to add path: ${e}`;
		}
	}

	/** Remove a path from the watch list. */
	async removePath(path: string) {
		try {
			await invoke('forge_watch_remove_path', { path });
			await this.refreshPaths();
			this.error = null;
		} catch (e) {
			this.error = `Failed to remove path: ${e}`;
		}
	}

	/** Manually reindex a file or directory. */
	async reindex(path: string) {
		this.isReindexing = true;
		this.error = null;
		try {
			const result = await invoke<IngestionResult>('forge_watch_reindex', { path });
			this.lastReindexResult = result;
		} catch (e) {
			this.error = `Reindex failed: ${e}`;
		} finally {
			this.isReindexing = false;
		}
	}

	/** Add a discovered path and start watching it. */
	async addDiscovered(discovered: DiscoveredPath) {
		await this.addPath(discovered.path, discovered.project_type ?? undefined);
	}

	/** Initialize store state from backend. */
	async init() {
		await Promise.all([this.refreshPaths(), this.refreshStatus()]);
	}
}

export const forgeWatchStore = new ForgeWatchStore();
