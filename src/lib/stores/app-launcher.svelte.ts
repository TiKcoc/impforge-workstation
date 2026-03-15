/**
 * App Launcher Store — Self-Extending App Library
 *
 * Manages external applications, websites, APIs, and MCP servers
 * that users can add to ImpForge as sidebar modules.
 *
 * Backed by Rust IPC commands (app_list, app_add, app_remove, etc.)
 * with SQLite persistence in the Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';

// ─── Types matching Rust backend ────────────────────────────

export type AppType =
	| { type: 'NativeProcess'; command: string; args: string[] }
	| { type: 'WebView'; url: string }
	| { type: 'WebService'; url: string; health_endpoint: string | null }
	| { type: 'McpServer'; command: string; args: string[]; port: number | null };

export interface LaunchConfig {
	auto_start: boolean;
	show_in_sidebar: boolean;
	monitoring_enabled: boolean;
}

export interface AppEntry {
	id: string;
	name: string;
	app_type: AppType;
	icon: string | null;
	category: string;
	pinned: boolean;
	launch_config: LaunchConfig;
	usage_count: number;
	last_used: string | null;
	created_at: string;
}

export interface AppHealthStatus {
	id: string;
	healthy: boolean;
	message: string;
}

export type AppCategory = 'All' | 'Favorites' | 'Development' | 'Web' | 'Services' | 'Custom';

export const APP_CATEGORIES: AppCategory[] = [
	'All', 'Favorites', 'Development', 'Web', 'Services', 'Custom'
];

// ─── Store ──────────────────────────────────────────────────

class AppLauncherStore {
	apps = $state<AppEntry[]>([]);
	healthMap = $state<Map<string, AppHealthStatus>>(new Map());
	loading = $state(false);
	error = $state<string | null>(null);
	discovering = $state(false);

	// ─── Derived state ──────────────────────────────────────

	get pinnedApps(): AppEntry[] {
		return this.apps.filter(a => a.pinned);
	}

	get sidebarApps(): AppEntry[] {
		return this.apps.filter(a => a.launch_config.show_in_sidebar);
	}

	get favoriteApps(): AppEntry[] {
		return this.apps.filter(a => a.pinned);
	}

	get appsByCategory(): Record<string, AppEntry[]> {
		const result: Record<string, AppEntry[]> = {};
		for (const app of this.apps) {
			const cat = app.category || 'Custom';
			if (!result[cat]) result[cat] = [];
			result[cat].push(app);
		}
		return result;
	}

	getFilteredApps(category: AppCategory, searchQuery: string): AppEntry[] {
		let filtered = this.apps;

		// Category filter
		if (category === 'Favorites') {
			filtered = filtered.filter(a => a.pinned);
		} else if (category !== 'All') {
			filtered = filtered.filter(a => a.category === category);
		}

		// Search filter
		if (searchQuery.trim()) {
			const q = searchQuery.toLowerCase();
			filtered = filtered.filter(a =>
				a.name.toLowerCase().includes(q) ||
				a.category.toLowerCase().includes(q) ||
				a.app_type.type.toLowerCase().includes(q)
			);
		}

		return filtered;
	}

	getApp(id: string): AppEntry | undefined {
		return this.apps.find(a => a.id === id);
	}

	getHealth(id: string): AppHealthStatus | undefined {
		return this.healthMap.get(id);
	}

	// ─── Actions ────────────────────────────────────────────

	async loadApps(): Promise<void> {
		this.loading = true;
		this.error = null;
		try {
			this.apps = await invoke<AppEntry[]>('app_list');
		} catch (e) {
			this.error = `Failed to load apps: ${e}`;
			console.error('[AppLauncher] loadApps failed:', e);
		} finally {
			this.loading = false;
		}
	}

	async addApp(entry: Omit<AppEntry, 'id' | 'usage_count' | 'last_used' | 'created_at'>): Promise<string | null> {
		this.error = null;
		try {
			const id = await invoke<string>('app_add', {
				name: entry.name,
				appType: entry.app_type,
				icon: entry.icon,
				category: entry.category,
				pinned: entry.pinned,
				launchConfig: entry.launch_config,
			});
			await this.loadApps();
			return id;
		} catch (e) {
			this.error = `Failed to add app: ${e}`;
			console.error('[AppLauncher] addApp failed:', e);
			return null;
		}
	}

	async removeApp(id: string): Promise<boolean> {
		this.error = null;
		try {
			await invoke('app_remove', { id });
			this.apps = this.apps.filter(a => a.id !== id);
			const updated = new Map(this.healthMap);
			updated.delete(id);
			this.healthMap = updated;
			return true;
		} catch (e) {
			this.error = `Failed to remove app: ${e}`;
			console.error('[AppLauncher] removeApp failed:', e);
			return false;
		}
	}

	async launchApp(id: string): Promise<{ url?: string; status?: string } | null> {
		this.error = null;
		try {
			const result = await invoke<{ url?: string; status?: string }>('app_launch', { id });
			// Refresh to update usage_count and last_used
			await this.loadApps();
			return result;
		} catch (e) {
			this.error = `Failed to launch app: ${e}`;
			console.error('[AppLauncher] launchApp failed:', e);
			return null;
		}
	}

	async pinApp(id: string): Promise<void> {
		this.error = null;
		try {
			await invoke('app_pin', { id });
			// Optimistic update
			this.apps = this.apps.map(a =>
				a.id === id ? { ...a, pinned: !a.pinned } : a
			);
		} catch (e) {
			this.error = `Failed to toggle pin: ${e}`;
			console.error('[AppLauncher] pinApp failed:', e);
			// Rollback
			await this.loadApps();
		}
	}

	async checkHealth(id: string): Promise<AppHealthStatus | null> {
		try {
			const status = await invoke<AppHealthStatus>('app_health', { id });
			const updated = new Map(this.healthMap);
			updated.set(id, status);
			this.healthMap = updated;
			return status;
		} catch (e) {
			const fallback: AppHealthStatus = { id, healthy: false, message: String(e) };
			const updated = new Map(this.healthMap);
			updated.set(id, fallback);
			this.healthMap = updated;
			return fallback;
		}
	}

	async discoverInstalled(): Promise<void> {
		this.discovering = true;
		this.error = null;
		try {
			const discovered = await invoke<AppEntry[]>('app_discover_installed');
			if (discovered.length > 0) {
				await this.loadApps();
			}
		} catch (e) {
			this.error = `Discovery failed: ${e}`;
			console.error('[AppLauncher] discoverInstalled failed:', e);
		} finally {
			this.discovering = false;
		}
	}

	async checkAllHealth(): Promise<void> {
		const monitoredApps = this.apps.filter(a => a.launch_config.monitoring_enabled);
		await Promise.allSettled(monitoredApps.map(a => this.checkHealth(a.id)));
	}
}

export const appLauncherStore = new AppLauncherStore();
