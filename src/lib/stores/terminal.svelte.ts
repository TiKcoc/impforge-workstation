/**
 * Terminal Store — PTY session management for xterm.js
 *
 * Manages multiple terminal tabs, each backed by a real PTY process.
 * Uses Tauri events for bidirectional PTY communication.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';

export interface PtyInfo {
	id: number;
	shell: string;
	cols: number;
	rows: number;
}

export interface TerminalTab {
	ptyId: number;
	title: string;
	shell: string;
	unlisten?: UnlistenFn;
	unlistenExit?: UnlistenFn;
}

class TerminalStore {
	tabs = $state<TerminalTab[]>([]);
	activeIndex = $state(0);
	onData = $state<((id: number, data: string) => void) | null>(null);
	onExit = $state<((id: number) => void) | null>(null);

	get activeTab(): TerminalTab | null {
		return this.tabs[this.activeIndex] || null;
	}

	async spawn(cwd?: string): Promise<number> {
		const info = await invoke<PtyInfo>('pty_spawn', { shell: null, cwd, cols: 80, rows: 24 });

		const unlisten = await listen<{ id: number; data: string }>(
			`pty-output-${info.id}`,
			(event) => {
				this.onData?.(event.payload.id, event.payload.data);
			}
		);

		const unlistenExit = await listen<number>(`pty-exit-${info.id}`, (event) => {
			this.onExit?.(event.payload);
			this.removeTab(info.id);
		});

		const tab: TerminalTab = {
			ptyId: info.id,
			title: `Terminal ${this.tabs.length + 1}`,
			shell: info.shell,
			unlisten,
			unlistenExit
		};

		this.tabs = [...this.tabs, tab];
		this.activeIndex = this.tabs.length - 1;

		return info.id;
	}

	async write(id: number, data: string): Promise<void> {
		await invoke('pty_write', { id, data });
	}

	async resize(id: number, cols: number, rows: number): Promise<void> {
		await invoke('pty_resize', { id, cols, rows });
	}

	async kill(id: number): Promise<void> {
		const tab = this.tabs.find((t) => t.ptyId === id);
		if (tab) {
			tab.unlisten?.();
			tab.unlistenExit?.();
		}
		await invoke('pty_kill', { id });
		this.removeTab(id);
	}

	private removeTab(ptyId: number) {
		this.tabs = this.tabs.filter((t) => t.ptyId !== ptyId);
		if (this.activeIndex >= this.tabs.length) {
			this.activeIndex = Math.max(0, this.tabs.length - 1);
		}
	}

	async killAll(): Promise<void> {
		for (const tab of this.tabs) {
			tab.unlisten?.();
			tab.unlistenExit?.();
			try {
				await invoke('pty_kill', { id: tab.ptyId });
			} catch {
				// Already dead
			}
		}
		this.tabs = [];
		this.activeIndex = 0;
	}
}

export const terminalStore = new TerminalStore();
