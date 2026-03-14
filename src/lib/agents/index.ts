/**
 * Agent System - Frontend API
 *
 * Provides typed wrappers around Tauri IPC commands for managing and
 * executing AI agents. All functions handle errors gracefully and
 * return sensible defaults on backend failure.
 */

import { invoke } from '@tauri-apps/api/core';

// ═══════════════════════════════════════════════════════════════
// TYPES (must match src-tauri/src/agents/mod.rs)
// ═══════════════════════════════════════════════════════════════

export type AgentRole =
	| 'orchestrator'
	| 'coder'
	| 'debugger'
	| 'researcher'
	| 'writer'
	| 'reviewer'
	| 'architect'
	| { custom: string };

export type AgentState = 'idle' | 'running' | 'error' | 'disabled';

export interface AgentConfig {
	id: string;
	name: string;
	role: AgentRole;
	model_id: string;
	system_prompt: string;
	enabled: boolean;
	temperature: number;
	max_tokens: number;
}

export interface AgentStatus {
	id: string;
	state: AgentState;
	current_task: string | null;
	messages_processed: number;
	tasks_completed: number;
	tasks_failed: number;
	last_active: string | null;
	uptime_seconds: number | null;
}

export interface AgentLog {
	timestamp: string;
	level: 'info' | 'warn' | 'error' | 'debug';
	message: string;
	task_id: string | null;
}

// ═══════════════════════════════════════════════════════════════
// DEFAULT AGENTS (fallback when backend is unavailable)
// ═══════════════════════════════════════════════════════════════

export const DEFAULT_AGENTS: AgentConfig[] = [
	{
		id: 'orchestrator',
		name: 'Orchestrator',
		role: 'orchestrator',
		model_id: 'meta-llama/llama-4-scout:free',
		system_prompt:
			'You are an AI orchestrator that coordinates complex tasks by breaking them into subtasks and delegating to specialized agents.',
		enabled: true,
		temperature: 0.7,
		max_tokens: 4096
	},
	{
		id: 'coder',
		name: 'Code Expert',
		role: 'coder',
		model_id: 'mistralai/devstral-small:free',
		system_prompt:
			'You are an expert software engineer. Write clean, efficient, well-documented code. Follow best practices and design patterns.',
		enabled: true,
		temperature: 0.7,
		max_tokens: 4096
	},
	{
		id: 'debugger',
		name: 'Debug Specialist',
		role: 'debugger',
		model_id: 'mistralai/devstral-small:free',
		system_prompt:
			'You are a debugging specialist. Analyze errors, trace issues, and provide precise fixes with explanations.',
		enabled: true,
		temperature: 0.7,
		max_tokens: 4096
	},
	{
		id: 'researcher',
		name: 'Research Analyst',
		role: 'researcher',
		model_id: 'meta-llama/llama-4-scout:free',
		system_prompt:
			'You are a research analyst. Find relevant information, synthesize findings, and provide comprehensive summaries with sources.',
		enabled: true,
		temperature: 0.7,
		max_tokens: 4096
	}
];

// ═══════════════════════════════════════════════════════════════
// CRUD OPERATIONS
// ═══════════════════════════════════════════════════════════════

/** List all configured agents. Falls back to DEFAULT_AGENTS on error. */
export async function listAgents(): Promise<AgentConfig[]> {
	try {
		const agents = await invoke<AgentConfig[]>('list_agents');
		return agents.length > 0 ? agents : DEFAULT_AGENTS;
	} catch {
		return DEFAULT_AGENTS;
	}
}

/** Get a single agent by ID. */
export async function getAgent(id: string): Promise<AgentConfig | null> {
	try {
		return await invoke<AgentConfig | null>('get_agent', { id });
	} catch {
		return null;
	}
}

/** Create a new agent. Returns the created config. */
export async function createAgent(params: {
	id: string;
	name: string;
	role: AgentRole;
	system_prompt?: string;
	model_id?: string;
}): Promise<AgentConfig> {
	return await invoke<AgentConfig>('create_agent', {
		id: params.id,
		name: params.name,
		role: params.role,
		system_prompt: params.system_prompt ?? null,
		model_id: params.model_id ?? null
	});
}

/** Update an existing agent. Returns the updated config. */
export async function updateAgent(params: {
	id: string;
	name?: string;
	system_prompt?: string;
	model_id?: string;
	enabled?: boolean;
	temperature?: number;
}): Promise<AgentConfig> {
	return await invoke<AgentConfig>('update_agent', {
		id: params.id,
		name: params.name ?? null,
		system_prompt: params.system_prompt ?? null,
		model_id: params.model_id ?? null,
		enabled: params.enabled ?? null,
		temperature: params.temperature ?? null
	});
}

/** Delete an agent by ID. Returns true if the agent existed. */
export async function deleteAgent(id: string): Promise<boolean> {
	try {
		return await invoke<boolean>('delete_agent', { id });
	} catch {
		return false;
	}
}

/** Find the first enabled agent matching a role. */
export async function getAgentByRole(role: AgentRole): Promise<AgentConfig | null> {
	try {
		return await invoke<AgentConfig | null>('get_agent_by_role', { role });
	} catch {
		return null;
	}
}

// ═══════════════════════════════════════════════════════════════
// RUNTIME OPERATIONS
// ═══════════════════════════════════════════════════════════════

/** Get runtime statuses for all agents. */
export async function getAgentStatuses(): Promise<AgentStatus[]> {
	try {
		return await invoke<AgentStatus[]>('get_agent_statuses');
	} catch {
		return [];
	}
}

/** Get runtime status for a single agent. */
export async function getAgentStatus(agentId: string): Promise<AgentStatus | null> {
	try {
		return await invoke<AgentStatus>('agent_status', { agentId });
	} catch {
		return null;
	}
}

/**
 * Run a task using a specific agent.
 *
 * The agent's configured model and system prompt are used automatically.
 * The agent state transitions: idle -> running -> idle/error.
 *
 * @returns The LLM response text.
 */
export async function runAgent(agentId: string, task: string): Promise<string> {
	return await invoke<string>('run_agent', { agentId, task });
}

/**
 * Stop an agent's current task and reset to idle.
 *
 * Note: this does not cancel in-flight HTTP requests but resets the
 * agent's state so the UI reflects it as idle.
 */
export async function stopAgent(agentId: string): Promise<void> {
	await invoke<void>('stop_agent', { agentId });
}

/**
 * Get recent logs for an agent.
 *
 * @param agentId - The agent to get logs for.
 * @param limit - Maximum number of log entries (default 100, max 500).
 * @returns Log entries, newest first.
 */
export async function getAgentLogs(
	agentId: string,
	limit: number = 100
): Promise<AgentLog[]> {
	try {
		return await invoke<AgentLog[]>('agent_logs', { agentId, limit });
	} catch {
		return [];
	}
}

// ═══════════════════════════════════════════════════════════════
// CONVENIENCE
// ═══════════════════════════════════════════════════════════════

/** Legacy alias — use listAgents() instead */
export const getAgents = listAgents;

/** Legacy alias — use runAgent() instead */
export async function runAgentTask(agentId: string, task: string): Promise<string> {
	try {
		return await runAgent(agentId, task);
	} catch (e) {
		return `Error: ${e}`;
	}
}
