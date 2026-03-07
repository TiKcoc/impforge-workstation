/**
 * System Store — Live metrics + service health for status bar
 * Polls cmd_get_quick_stats every 2s, checks services every 30s
 */

import { invoke } from '@tauri-apps/api/core';

export interface QuickStats {
	cpu_percent: number;
	ram_used_gb: number;
	ram_total_gb: number;
	gpu_name: string | null;
	gpu_temp_c: number | null;
	gpu_vram_used_mb: number | null;
	gpu_vram_total_mb: number | null;
	gpu_usage_percent: number | null;
}

export type ServiceState = 'online' | 'offline' | 'checking';

export interface ServiceStatus {
	ollama: ServiceState;
	docker: ServiceState;
	n8n: ServiceState;
}

class SystemStore {
	stats = $state<QuickStats | null>(null);
	services = $state<ServiceStatus>({
		ollama: 'checking',
		docker: 'checking',
		n8n: 'checking',
	});

	private statsInterval: ReturnType<typeof setInterval> | null = null;
	private servicesInterval: ReturnType<typeof setInterval> | null = null;

	async pollStats() {
		try {
			this.stats = await invoke<QuickStats>('cmd_get_quick_stats');
		} catch {
			// Backend not ready yet, ignore
		}
	}

	async checkServices() {
		const checks: Array<[keyof ServiceStatus, string]> = [
			['ollama', 'ollama'],
			['docker', 'docker'],
			['n8n', 'n8n'],
		];

		for (const [key, service] of checks) {
			try {
				const ok = await invoke<boolean>('cmd_check_service_health', { service });
				this.services = { ...this.services, [key]: ok ? 'online' : 'offline' };
			} catch {
				this.services = { ...this.services, [key]: 'offline' };
			}
		}
	}

	startPolling() {
		this.pollStats();
		this.checkServices();
		this.statsInterval = setInterval(() => this.pollStats(), 2000);
		this.servicesInterval = setInterval(() => this.checkServices(), 30000);
	}

	stopPolling() {
		if (this.statsInterval) clearInterval(this.statsInterval);
		if (this.servicesInterval) clearInterval(this.servicesInterval);
	}
}

export const system = new SystemStore();
