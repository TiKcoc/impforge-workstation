/**
 * Settings Store - Svelte 5 Runes
 * Manages application settings with persistent storage
 */

import { load, type Store } from '@tauri-apps/plugin-store';

export interface AppSettings {
	// API Keys
	openrouterKey: string;
	githubToken: string;

	// Preferences
	theme: 'dark' | 'light' | 'system';
	preferLocalModels: boolean;
	autoRouting: boolean;

	// Ollama
	ollamaUrl: string;
	ollamaAvailable: boolean;

	// Docker
	dockerEnabled: boolean;

	// n8n
	n8nUrl: string;
	n8nEnabled: boolean;
}

const DEFAULT_SETTINGS: AppSettings = {
	openrouterKey: '',
	githubToken: '',
	theme: 'dark',
	preferLocalModels: true,
	autoRouting: true,
	ollamaUrl: 'http://localhost:11434',
	ollamaAvailable: false,
	dockerEnabled: false,
	n8nUrl: 'http://localhost:5678',
	n8nEnabled: false,
};

// State
let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS });
let loaded = $state(false);
let store: Store | null = null;

// Initialize store
async function getStore(): Promise<Store> {
	if (!store) {
		store = await load('.nexus-settings.json');
	}
	return store;
}

// Actions
export async function loadSettings() {
	try {
		const s = await getStore();

		// Load each setting
		for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof AppSettings)[]) {
			const value = await s.get<AppSettings[typeof key]>(key);
			if (value !== undefined && value !== null) {
				// Type-safe assignment
				switch (key) {
					case 'openrouterKey':
					case 'githubToken':
					case 'ollamaUrl':
					case 'n8nUrl':
						settings[key] = value as string;
						break;
					case 'theme':
						settings[key] = value as 'dark' | 'light' | 'system';
						break;
					case 'preferLocalModels':
					case 'autoRouting':
					case 'ollamaAvailable':
					case 'dockerEnabled':
					case 'n8nEnabled':
						settings[key] = value as boolean;
						break;
				}
			}
		}

		loaded = true;
	} catch (e) {
		console.error('Failed to load settings:', e);
	}
}

export async function saveSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
	settings[key] = value;

	try {
		const s = await getStore();
		await s.set(key, value);
		await s.save();
	} catch (e) {
		console.error('Failed to save setting:', e);
	}
}

export async function saveAllSettings() {
	try {
		const s = await getStore();

		const entries = Object.entries(settings) as [keyof AppSettings, AppSettings[keyof AppSettings]][];
		for (const [key, value] of entries) {
			await s.set(key, value);
		}

		await s.save();
	} catch (e) {
		console.error('Failed to save settings:', e);
	}
}

export async function resetSettings() {
	settings = { ...DEFAULT_SETTINGS };
	await saveAllSettings();
}

// Getters
export function getSettings() {
	return settings;
}

export function isLoaded() {
	return loaded;
}

export function getSetting<K extends keyof AppSettings>(key: K): AppSettings[K] {
	return settings[key];
}
