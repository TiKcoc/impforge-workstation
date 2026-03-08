/**
 * NEXUS Browser Agent Store — Svelte 5 runes
 *
 * AI-powered web automation with n8n/Zapier webhook integration.
 * Architecture: OpAgent (Planner→Grounder→Reflector→Summarizer)
 *
 * Features:
 * - AI-planned web navigation (via Ollama models)
 * - CSS selector-based extraction
 * - Structured data extraction (multiple selectors)
 * - n8n webhook triggers for workflow automation
 * - Zapier webhook integration
 * - Session management (multiple concurrent sessions)
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface AgentStep {
	step_number: number;
	thought: string;
	action: BrowserAction;
	observation: string;
	success: boolean;
	timestamp: string;
}

export type BrowserAction =
	| { type: 'Navigate'; url: string }
	| { type: 'Click'; selector: string }
	| { type: 'Fill'; selector: string; value: string }
	| { type: 'Extract'; selector: string | null }
	| { type: 'Screenshot' }
	| { type: 'ExecuteJs'; script: string }
	| { type: 'ScrollDown' }
	| { type: 'ScrollUp' }
	| { type: 'GoBack' }
	| { type: 'Wait'; ms: number }
	| { type: 'Done'; summary: string };

export interface AgentTaskResult {
	task: string;
	steps: AgentStep[];
	final_result: string;
	extracted_data: ExtractedContent[];
	total_steps: number;
	success: boolean;
	error: string | null;
}

export interface ExtractedContent {
	url: string;
	title: string | null;
	content: string;
	content_type: 'Markdown' | 'Text' | 'Json' | 'Html' | 'Table';
	metadata: Record<string, string>;
}

export interface BrowserAgentConfig {
	max_steps?: number;
	step_timeout_secs?: number;
	ollama_url?: string;
	model?: string;
	remove_selectors?: string[];
	webhook?: { url: string; method: string; headers: Record<string, string> };
	zapier_webhook?: string;
}

// ============================================================================
// STATE
// ============================================================================

let taskResults = $state<AgentTaskResult[]>([]);
let currentTask = $state<AgentTaskResult | null>(null);
let isRunning = $state(false);
let error = $state<string | null>(null);

// Playground state
let playgroundUrl = $state('');
let playgroundSelector = $state('');
let playgroundContent = $state('');
let playgroundHistory = $state<{ url: string; title: string | null; timestamp: string }[]>([]);

// ============================================================================
// ACTIONS
// ============================================================================

/** Run an AI-planned browser agent task */
async function runTask(task: string, config?: BrowserAgentConfig): Promise<AgentTaskResult> {
	isRunning = true;
	error = null;
	try {
		const result = await invoke<AgentTaskResult>('browser_agent_run', {
			task,
			config: config ?? null
		});
		currentTask = result;
		taskResults = [result, ...taskResults].slice(0, 20);
		return result;
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isRunning = false;
	}
}

/** Quick extract: navigate + extract without AI planning */
async function quickExtract(url: string, selector?: string): Promise<ExtractedContent> {
	isRunning = true;
	error = null;
	try {
		const result = await invoke<ExtractedContent>('browser_agent_quick_extract', {
			url,
			selector: selector ?? null
		});
		playgroundContent = result.content;
		playgroundUrl = url;
		playgroundHistory = [
			{ url, title: result.title, timestamp: new Date().toISOString() },
			...playgroundHistory
		].slice(0, 50);
		return result;
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isRunning = false;
	}
}

/** Extract structured data using named CSS selectors */
async function structuredExtract(
	url: string,
	selectors: Record<string, string>
): Promise<Record<string, string>> {
	isRunning = true;
	error = null;
	try {
		return await invoke<Record<string, string>>('browser_agent_structured_extract', {
			url,
			selectors
		});
	} catch (e) {
		error = String(e);
		throw e;
	} finally {
		isRunning = false;
	}
}

/** Send data to an n8n or Zapier webhook */
async function sendWebhook(webhookUrl: string, data: unknown): Promise<string> {
	try {
		return await invoke<string>('browser_agent_send_webhook', {
			webhookUrl,
			data
		});
	} catch (e) {
		error = String(e);
		throw e;
	}
}

/** Clear all results */
function clearResults() {
	taskResults = [];
	currentTask = null;
	playgroundContent = '';
	error = null;
}

// ============================================================================
// EXPORT
// ============================================================================

export const browserAgentStore = {
	get taskResults() {
		return taskResults;
	},
	get currentTask() {
		return currentTask;
	},
	get isRunning() {
		return isRunning;
	},
	get error() {
		return error;
	},
	get playgroundUrl() {
		return playgroundUrl;
	},
	set playgroundUrl(v: string) {
		playgroundUrl = v;
	},
	get playgroundSelector() {
		return playgroundSelector;
	},
	set playgroundSelector(v: string) {
		playgroundSelector = v;
	},
	get playgroundContent() {
		return playgroundContent;
	},
	get playgroundHistory() {
		return playgroundHistory;
	},
	runTask,
	quickExtract,
	structuredExtract,
	sendWebhook,
	clearResults
};
