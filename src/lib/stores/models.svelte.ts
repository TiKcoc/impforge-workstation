/**
 * Model Store - Svelte 5 Runes
 * Manages available AI models and routing state
 */

import { invoke } from '@tauri-apps/api/core';

export interface ModelConfig {
	id: string;
	name: string;
	provider: string;
	free: boolean;
}

export interface RoutingPreview {
	taskType: string;
	targetModel: string;
}

// State using Svelte 5 runes
let models = $state<ModelConfig[]>([]);
let selectedModel = $state<string | null>(null);
let loading = $state(false);
let error = $state<string | null>(null);

// Actions
export async function loadModels() {
	loading = true;
	error = null;

	try {
		const result = await invoke<ModelConfig[]>('get_available_models');
		models = result;
	} catch (e) {
		error = e instanceof Error ? e.message : String(e);
	} finally {
		loading = false;
	}
}

export async function getRoutingPreview(prompt: string): Promise<RoutingPreview | null> {
	try {
		const [taskType, targetModel] = await invoke<[string, string]>('get_routing_preview', { prompt });
		return { taskType, targetModel };
	} catch (e) {
		console.error('Failed to get routing preview:', e);
		return null;
	}
}

export function selectModel(modelId: string) {
	selectedModel = modelId;
}

// Getters
export function getModels() {
	return models;
}

export function getSelectedModel() {
	return selectedModel;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}
