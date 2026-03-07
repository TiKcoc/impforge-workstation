/**
 * Agent System - Frontend Handlers
 * Manages communication with backend agent system
 */

import { invoke } from '@tauri-apps/api/core';

export interface AgentConfig {
	id: string;
	name: string;
	role: AgentRole;
	modelId: string;
	systemPrompt: string;
	enabled: boolean;
}

export type AgentRole =
	| 'orchestrator'
	| 'coder'
	| 'debugger'
	| 'researcher'
	| 'writer'
	| { custom: string };

export interface AgentStatus {
	id: string;
	active: boolean;
	currentTask: string | null;
	messagesProcessed: number;
}

// Default agents
export const DEFAULT_AGENTS: AgentConfig[] = [
	{
		id: 'orchestrator',
		name: 'Orchestrator',
		role: 'orchestrator',
		modelId: 'hermes-3:8b',
		systemPrompt: 'You are an orchestrator agent that coordinates tasks between other agents.',
		enabled: true,
	},
	{
		id: 'coder',
		name: 'Coder',
		role: 'coder',
		modelId: 'qwen2.5-coder:7b',
		systemPrompt: 'You are a coding assistant specialized in writing clean, efficient code.',
		enabled: true,
	},
	{
		id: 'debugger',
		name: 'Debugger',
		role: 'debugger',
		modelId: 'mistralai/devstral-small:free',
		systemPrompt: 'You are a debugging specialist that helps identify and fix bugs.',
		enabled: true,
	},
	{
		id: 'researcher',
		name: 'Researcher',
		role: 'researcher',
		modelId: 'meta-llama/llama-4-scout:free',
		systemPrompt: 'You are a research assistant that helps find and summarize information.',
		enabled: true,
	},
];

// Agent management functions (will connect to Tauri backend)
export async function getAgents(): Promise<AgentConfig[]> {
	// TODO: Implement backend call
	return DEFAULT_AGENTS;
}

export async function getAgentStatus(agentId: string): Promise<AgentStatus | null> {
	// TODO: Implement backend call
	return {
		id: agentId,
		active: false,
		currentTask: null,
		messagesProcessed: 0,
	};
}

export async function runAgentTask(agentId: string, task: string): Promise<string> {
	// TODO: Implement backend call
	return `Agent ${agentId} task completed: ${task}`;
}
