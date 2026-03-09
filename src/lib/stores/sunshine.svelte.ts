/**
 * ForgeSunshine Store — Moonlight Remote Access state management
 * Svelte 5 runes-based reactive store for Sunshine streaming server.
 */

import { invoke } from '@tauri-apps/api/core';

export interface SunshineInfo {
	installed: boolean;
	version: string | null;
	binary_path: string | null;
	config_path: string | null;
	running: boolean;
	web_ui_url: string | null;
	platform: string;
}

export interface SunshineConfig {
	resolution_width: number;
	resolution_height: number;
	fps: number;
	bitrate_kbps: number;
	encoder: 'Auto' | 'Nvenc' | 'Vaapi' | 'Qsv' | 'Software';
	audio_enabled: boolean;
	auto_start: boolean;
	port: number;
	web_port: number;
}

export interface SunshineStatus {
	running: boolean;
	uptime_seconds: number | null;
	connected_clients: number;
	encoder_in_use: string | null;
	current_fps: number | null;
	current_bitrate_kbps: number | null;
}

class SunshineStore {
	info = $state<SunshineInfo | null>(null);
	config = $state<SunshineConfig | null>(null);
	status = $state<SunshineStatus | null>(null);
	installCommand = $state<string | null>(null);
	isStarting = $state(false);
	isStopping = $state(false);
	isSaving = $state(false);
	error = $state<string | null>(null);

	/** Detect Sunshine installation. */
	async detect() {
		try {
			this.info = await invoke<SunshineInfo>('sunshine_detect');
			this.error = null;
		} catch (e) {
			this.error = `Detection failed: ${e}`;
		}
	}

	/** Get current config. */
	async loadConfig() {
		try {
			this.config = await invoke<SunshineConfig>('sunshine_get_config');
			this.error = null;
		} catch (e) {
			this.error = `Failed to load config: ${e}`;
		}
	}

	/** Save config to disk. */
	async saveConfig(config: SunshineConfig) {
		this.isSaving = true;
		this.error = null;
		try {
			await invoke('sunshine_save_config', { config });
			this.config = config;
		} catch (e) {
			this.error = `Failed to save config: ${e}`;
		} finally {
			this.isSaving = false;
		}
	}

	/** Get the platform install command. */
	async getInstallCommand() {
		try {
			this.installCommand = await invoke<string>('sunshine_install_cmd');
		} catch (e) {
			this.error = `Failed to get install command: ${e}`;
		}
	}

	/** Start Sunshine server. */
	async start() {
		this.isStarting = true;
		this.error = null;
		try {
			await invoke<number>('sunshine_start');
			await this.refreshStatus();
		} catch (e) {
			this.error = `Failed to start: ${e}`;
		} finally {
			this.isStarting = false;
		}
	}

	/** Stop Sunshine server. */
	async stop() {
		this.isStopping = true;
		this.error = null;
		try {
			await invoke('sunshine_stop');
			await this.refreshStatus();
		} catch (e) {
			this.error = `Failed to stop: ${e}`;
		} finally {
			this.isStopping = false;
		}
	}

	/** Refresh running status. */
	async refreshStatus() {
		try {
			this.status = await invoke<SunshineStatus>('sunshine_status');
			this.error = null;
		} catch (e) {
			this.error = `Failed to get status: ${e}`;
		}
	}

	/** Initialize all sunshine state. */
	async init() {
		await Promise.all([this.detect(), this.loadConfig(), this.refreshStatus()]);
		if (!this.info?.installed) {
			await this.getInstallCommand();
		}
	}
}

export const sunshineStore = new SunshineStore();
