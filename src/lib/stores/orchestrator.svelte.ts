/**
 * Orchestrator Store — ImpForge Standalone AI Orchestrator State
 *
 * Manages the full orchestrator state including:
 * - 42 task workers with trust scores
 * - MAPE-K health loop service states
 * - Real-time event feed
 * - Brain v2.0 memory stats
 *
 * Polls neuralswarm_snapshot every 5s when the orchestrator page is active.
 */

import { invoke } from '@tauri-apps/api/core';

// ─── Types matching Rust backend ────────────────────────────

export interface OrchestratorStatus {
	running: boolean;
	task_count: number;
	tasks_ok: number;
	tasks_fail: number;
	uptime_seconds: number;
	avg_trust: number;
}

export interface TaskStatus {
	name: string;
	description: string;
	status: string;
	duration_ms: number | null;
	trust: number;
	last_run: string | null;
	pool: string;
	enabled: boolean;
}

export interface ServiceHealth {
	name: string;
	status: string;
	endpoint: string | null;
	response_time_ms: number | null;
}

export interface NeuralSwarmSnapshot {
	status: OrchestratorStatus;
	tasks: TaskStatus[];
	services: ServiceHealth[];
}

// ─── Store ──────────────────────────────────────────────────

class OrchestratorStore {
	status = $state<OrchestratorStatus>({
		running: false,
		task_count: 42,
		tasks_ok: 0,
		tasks_fail: 0,
		uptime_seconds: 0,
		avg_trust: 0.5,
	});

	tasks = $state<TaskStatus[]>([]);
	services = $state<ServiceHealth[]>([]);
	logs = $state<string>('');
	error = $state<string | null>(null);
	loading = $state(false);

	private pollInterval: ReturnType<typeof setInterval> | null = null;

	// ─── Actions ────────────────────────────────────────────

	async fetchSnapshot() {
		try {
			const snapshot = await invoke<NeuralSwarmSnapshot>('neuralswarm_snapshot');
			this.status = snapshot.status;
			this.tasks = snapshot.tasks;
			this.services = snapshot.services;
			this.error = null;
		} catch (e) {
			this.error = String(e);
		}
	}

	async fetchLogs(lines = 50) {
		try {
			this.logs = await invoke<string>('neuralswarm_logs', { lines });
		} catch (e) {
			this.logs = `Error fetching logs: ${e}`;
		}
	}

	async start() {
		this.loading = true;
		try {
			const msg = await invoke<string>('neuralswarm_action', { action: 'start' });
			this.error = null;
			await this.fetchSnapshot();
			return msg;
		} catch (e) {
			this.error = String(e);
			throw e;
		} finally {
			this.loading = false;
		}
	}

	async stop() {
		this.loading = true;
		try {
			const msg = await invoke<string>('neuralswarm_action', { action: 'stop' });
			this.error = null;
			await this.fetchSnapshot();
			return msg;
		} catch (e) {
			this.error = String(e);
			throw e;
		} finally {
			this.loading = false;
		}
	}

	async restart() {
		this.loading = true;
		try {
			const msg = await invoke<string>('neuralswarm_action', { action: 'restart' });
			this.error = null;
			await this.fetchSnapshot();
			return msg;
		} catch (e) {
			this.error = String(e);
			throw e;
		} finally {
			this.loading = false;
		}
	}

	// ─── Polling ────────────────────────────────────────────

	startPolling(intervalMs = 5000) {
		this.fetchSnapshot();
		this.fetchLogs();
		this.pollInterval = setInterval(() => {
			this.fetchSnapshot();
			this.fetchLogs(30);
		}, intervalMs);
	}

	stopPolling() {
		if (this.pollInterval) {
			clearInterval(this.pollInterval);
			this.pollInterval = null;
		}
	}

	// ─── Computed helpers ───────────────────────────────────

	get uptimeFormatted(): string {
		const s = this.status.uptime_seconds;
		if (s === 0) return 'Stopped';
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		const sec = s % 60;
		if (h > 0) return `${h}h ${m}m`;
		if (m > 0) return `${m}m ${sec}s`;
		return `${sec}s`;
	}

	get trustColor(): string {
		const t = this.status.avg_trust;
		if (t >= 0.8) return 'text-green-400';
		if (t >= 0.5) return 'text-yellow-400';
		return 'text-red-400';
	}

	get tasksByPool(): Record<string, TaskStatus[]> {
		const pools: Record<string, TaskStatus[]> = {};
		for (const task of this.tasks) {
			if (!pools[task.pool]) pools[task.pool] = [];
			pools[task.pool].push(task);
		}
		return pools;
	}

	get activeTaskCount(): number {
		return this.tasks.filter((t) => t.enabled).length;
	}

	get onlineServiceCount(): number {
		return this.services.filter((s) => s.status === 'online').length;
	}
}

export const orchestrator = new OrchestratorStore();
