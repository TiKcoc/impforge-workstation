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

	// Chat Layout (3x3x3 modular system)
	chatPlacement: 'side-panel' | 'dedicated' | 'convergence';
	chatStreamMode: 'split' | 'unified' | 'mission-control';
	chatVizLevel: 'minimal' | 'cards' | 'pipeline';
	chatShowThinking: boolean;
	chatShowRouting: boolean;
	chatAnimations: boolean;
	chatCompactMode: boolean;

	// Onboarding
	onboardingComplete: boolean;

	// User Profile (Adaptive Onboarding — arXiv:2412.16837)
	userRole: 'developer' | 'office' | 'freelancer' | 'manager' | 'marketing' | 'student' | 'entrepreneur' | 'custom' | '';
	userExperience: 'beginner' | 'intermediate' | 'expert' | '';

	// Window geometry
	windowGeometry: { x: number; y: number; w: number; h: number } | null;
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
	chatPlacement: 'dedicated',
	chatStreamMode: 'unified',
	chatVizLevel: 'cards',
	chatShowThinking: true,
	chatShowRouting: true,
	chatAnimations: true,
	chatCompactMode: false,
	onboardingComplete: false,
	userRole: '',
	userExperience: '',
	windowGeometry: null,
};

// State
let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS });
let loaded = $state(false);
let store: Store | null = null;

// Initialize store
async function getStore(): Promise<Store> {
	if (!store) {
		store = await load('.impforge-settings.json');
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
					case 'chatShowThinking':
					case 'chatShowRouting':
					case 'chatAnimations':
					case 'chatCompactMode':
					case 'onboardingComplete':
						settings[key] = value as boolean;
						break;
					case 'chatPlacement':
						settings[key] = value as 'side-panel' | 'dedicated' | 'convergence';
						break;
					case 'chatStreamMode':
						settings[key] = value as 'split' | 'unified' | 'mission-control';
						break;
					case 'chatVizLevel':
						settings[key] = value as 'minimal' | 'cards' | 'pipeline';
						break;
					case 'userRole':
						settings[key] = value as AppSettings['userRole'];
						break;
					case 'userExperience':
						settings[key] = value as AppSettings['userExperience'];
						break;
					case 'windowGeometry':
						settings[key] = value as { x: number; y: number; w: number; h: number } | null;
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

/**
 * Adaptive Module Visibility — returns which navigation modules are visible
 * based on the user's selected role. Custom/unset shows everything.
 * Based on: arXiv:2412.16837 (Adaptive UI via Reinforcement Learning)
 */
const MODULE_MAP: Record<string, string[]> = {
	developer:    ['home', 'chat', 'github', 'docker', 'n8n', 'ide', 'agents', 'evaluation', 'ai', 'browser', 'news', 'social', 'apps', 'settings'],
	office:       ['home', 'chat', 'news', 'apps', 'settings'],
	freelancer:   ['home', 'chat', 'github', 'browser', 'news', 'social', 'apps', 'settings'],
	manager:      ['home', 'chat', 'agents', 'evaluation', 'news', 'apps', 'settings'],
	marketing:    ['home', 'chat', 'browser', 'news', 'social', 'apps', 'settings'],
	student:      ['home', 'chat', 'ide', 'ai', 'news', 'apps', 'settings'],
	entrepreneur: ['home', 'chat', 'agents', 'browser', 'news', 'social', 'apps', 'settings'],
	custom:       ['home', 'chat', 'github', 'docker', 'n8n', 'ide', 'agents', 'evaluation', 'ai', 'browser', 'news', 'social', 'apps', 'settings'],
};

export function getVisibleModules(): string[] {
	const role = settings.userRole;
	if (!role || role === 'custom') return MODULE_MAP.custom;
	return MODULE_MAP[role] ?? MODULE_MAP.custom;
}
