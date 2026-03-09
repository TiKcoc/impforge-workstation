/**
 * Digest Store — Universal Input Digest configuration & state
 * Svelte 5 runes-based reactive store for auto-learn settings.
 */

import { invoke } from '@tauri-apps/api/core';

export interface DigestConfig {
	terminal_enabled: boolean;
	editor_enabled: boolean;
	clipboard_enabled: boolean;
	url_enabled: boolean;
	min_line_length: number;
	max_lines: number;
	debounce_ms: number;
	nlp_threshold: number;
}

export interface DigestResult {
	source: string;
	lines_processed: number;
	lines_filtered: number;
	memories_created: number;
	memories_reinforced: number;
	entities_extracted: number;
	relations_extracted: number;
	skipped_reason: string | null;
}

class DigestStore {
	config = $state<DigestConfig>({
		terminal_enabled: true,
		editor_enabled: true,
		clipboard_enabled: false,
		url_enabled: true,
		min_line_length: 10,
		max_lines: 200,
		debounce_ms: 2000,
		nlp_threshold: 0.3
	});

	lastResult = $state<DigestResult | null>(null);
	totalDigested = $state(0);
	error = $state<string | null>(null);

	/** Load config from backend. */
	async loadConfig() {
		try {
			this.config = await invoke<DigestConfig>('forge_digest_get_config');
			this.error = null;
		} catch (e) {
			this.error = `Failed to load config: ${e}`;
		}
	}

	/** Digest text from a specific source. */
	async digestText(text: string, source: string): Promise<DigestResult | null> {
		try {
			const result = await invoke<DigestResult>('forge_digest_text', { text, source });
			this.lastResult = result;
			this.totalDigested += result.memories_created;
			this.error = null;
			return result;
		} catch (e) {
			this.error = `Digest failed: ${e}`;
			return null;
		}
	}

	/** Digest with custom config. */
	async digestConfigured(text: string, source: string): Promise<DigestResult | null> {
		try {
			const result = await invoke<DigestResult>('forge_digest_text_configured', {
				text,
				source,
				config: this.config
			});
			this.lastResult = result;
			this.totalDigested += result.memories_created;
			this.error = null;
			return result;
		} catch (e) {
			this.error = `Digest failed: ${e}`;
			return null;
		}
	}

	/** Update a single config field. */
	updateConfig<K extends keyof DigestConfig>(key: K, value: DigestConfig[K]) {
		this.config = { ...this.config, [key]: value };
	}

	/** Init store. */
	async init() {
		await this.loadConfig();
	}
}

export const digestStore = new DigestStore();
