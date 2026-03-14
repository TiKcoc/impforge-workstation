/**
 * Module Registry — Central 3x3x3 chat composition system
 *
 * Manages the dynamic composition of chat placement, stream mode,
 * and visualization level. Exposes the active configuration and
 * available modules for runtime switching.
 */

import { getSetting, saveSetting } from './settings.svelte';

export type Placement = 'side-panel' | 'dedicated' | 'convergence';
export type StreamMode = 'split' | 'unified' | 'mission-control';
export type VizLevel = 'minimal' | 'cards' | 'pipeline';

export interface ModuleDef {
	id: string;
	label: string;
	description: string;
}

export const PLACEMENTS: ModuleDef[] = [
	{ id: 'side-panel', label: 'Side Panel', description: 'Chat docked to the IDE sidebar' },
	{ id: 'dedicated', label: 'Dedicated', description: 'Full-screen chat experience' },
	{ id: 'convergence', label: 'Convergence', description: 'Chat embedded in every view' },
];

export const STREAM_MODES: ModuleDef[] = [
	{ id: 'split', label: 'Split', description: 'Chat left, live output right' },
	{ id: 'unified', label: 'Unified', description: 'Single stream with inline blocks' },
	{ id: 'mission-control', label: 'Mission Control', description: 'Dashboard with model gauges and routing' },
];

export const VIZ_LEVELS: ModuleDef[] = [
	{ id: 'minimal', label: 'Minimal', description: 'Status badge only' },
	{ id: 'cards', label: 'Cards', description: 'Model activity cards panel' },
	{ id: 'pipeline', label: 'Pipeline', description: 'Full DAG pipeline visualization' },
];

class ModuleRegistry {
	get placement(): Placement {
		return getSetting('chatPlacement');
	}

	get streamMode(): StreamMode {
		return getSetting('chatStreamMode');
	}

	get vizLevel(): VizLevel {
		return getSetting('chatVizLevel');
	}

	async setPlacement(p: Placement) {
		await saveSetting('chatPlacement', p);
	}

	async setStreamMode(m: StreamMode) {
		await saveSetting('chatStreamMode', m);
	}

	async setVizLevel(v: VizLevel) {
		await saveSetting('chatVizLevel', v);
	}

	get configLabel(): string {
		return `${this.placement} / ${this.streamMode} / ${this.vizLevel}`;
	}
}

export const moduleRegistry = new ModuleRegistry();
