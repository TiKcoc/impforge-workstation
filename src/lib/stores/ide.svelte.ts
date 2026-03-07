/**
 * IDE Store — File tree, editor state, and AI agent communication
 *
 * Uses Svelte 5 Runes class pattern so state is mutable from components.
 * Communicates with Tauri backend for filesystem operations
 * and AI model tool-use execution.
 */

import { invoke } from '@tauri-apps/api/core';

// Types
export interface FileEntry {
	name: string;
	path: string;
	is_dir: boolean;
	size: number;
	extension: string | null;
}

export interface SearchMatch {
	file: string;
	line: number;
	content: string;
}

export interface ToolResult {
	tool: string;
	success: boolean;
	output: string;
	error: string | null;
}

export interface OpenTab {
	path: string;
	name: string;
	content: string;
	modified: boolean;
	language: string;
}

export interface AgentMessage {
	role: 'user' | 'assistant' | 'tool';
	content: string;
	timestamp: number;
	toolCall?: {
		tool: string;
		args: Record<string, string>;
		result?: ToolResult;
	};
}

// Language detection from file extension
function detectLanguage(filename: string): string {
	const ext = filename.split('.').pop()?.toLowerCase() || '';
	const map: Record<string, string> = {
		rs: 'rust', py: 'python', ts: 'typescript', tsx: 'typescript',
		js: 'javascript', jsx: 'javascript', svelte: 'html', html: 'html',
		css: 'css', scss: 'scss', json: 'json', toml: 'toml',
		yaml: 'yaml', yml: 'yaml', md: 'markdown', sql: 'sql',
		sh: 'shell', bash: 'shell', c: 'c', cpp: 'cpp', h: 'c',
		cs: 'csharp', java: 'java', go: 'go', xml: 'xml', txt: 'plaintext',
	};
	return map[ext] || 'plaintext';
}

class IdeStore {
	currentDir = $state('/home');
	files = $state<FileEntry[]>([]);
	openTabs = $state<OpenTab[]>([]);
	activeTabIndex = $state(0);
	loading = $state(false);
	searchResults = $state<SearchMatch[]>([]);
	agentMessages = $state<AgentMessage[]>([]);
	agentLoading = $state(false);
	terminalOutput = $state('');
	expandedDirs = $state<Set<string>>(new Set());
	subDirFiles = $state<Map<string, FileEntry[]>>(new Map());

	get activeTab(): OpenTab | null {
		return this.openTabs[this.activeTabIndex] || null;
	}

	async loadDirectory(path: string) {
		this.loading = true;
		try {
			const entries = await invoke<FileEntry[]>('ide_read_dir', { path });
			this.files = entries;
			this.currentDir = path;
		} catch (e) {
			console.error('Failed to load directory:', e);
		} finally {
			this.loading = false;
		}
	}

	async openFile(path: string, name: string) {
		const existingIndex = this.openTabs.findIndex((t) => t.path === path);
		if (existingIndex >= 0) {
			this.activeTabIndex = existingIndex;
			return;
		}

		try {
			const content = await invoke<string>('ide_read_file', { path });
			const tab: OpenTab = {
				path, name, content, modified: false,
				language: detectLanguage(name),
			};
			this.openTabs = [...this.openTabs, tab];
			this.activeTabIndex = this.openTabs.length - 1;
		} catch (e) {
			console.error('Failed to open file:', e);
		}
	}

	closeTab(index: number) {
		this.openTabs = this.openTabs.filter((_, i) => i !== index);
		if (this.activeTabIndex >= this.openTabs.length) {
			this.activeTabIndex = Math.max(0, this.openTabs.length - 1);
		}
	}

	async saveFile(index: number) {
		const tab = this.openTabs[index];
		if (!tab) return;
		try {
			await invoke('ide_write_file', { path: tab.path, content: tab.content });
			this.openTabs = this.openTabs.map((t, i) =>
				i === index ? { ...t, modified: false } : t
			);
		} catch (e) {
			console.error('Failed to save file:', e);
		}
	}

	updateTabContent(index: number, content: string) {
		this.openTabs = this.openTabs.map((t, i) =>
			i === index ? { ...t, content, modified: t.content !== content } : t
		);
	}

	async toggleDir(entry: FileEntry) {
		if (!entry.is_dir) return;
		const newExpanded = new Set(this.expandedDirs);
		if (newExpanded.has(entry.path)) {
			newExpanded.delete(entry.path);
		} else {
			newExpanded.add(entry.path);
			try {
				const entries = await invoke<FileEntry[]>('ide_read_dir', { path: entry.path });
				this.subDirFiles = new Map(this.subDirFiles).set(entry.path, entries);
			} catch (e) {
				console.error('Failed to load subdirectory:', e);
			}
		}
		this.expandedDirs = newExpanded;
	}

	async searchFiles(pattern: string, path?: string) {
		try {
			const results = await invoke<SearchMatch[]>('ide_search_files', {
				pattern, searchPath: path || this.currentDir, maxResults: 50,
			});
			this.searchResults = results;
		} catch (e) {
			console.error('Search failed:', e);
			this.searchResults = [];
		}
	}

	async executeCommand(command: string, cwd?: string) {
		try {
			const result = await invoke<ToolResult>('ide_execute_command', {
				command, cwd: cwd || this.currentDir,
			});
			const prefix = `$ ${command}\n`;
			if (result.success) {
				this.terminalOutput += prefix + result.output + '\n';
			} else {
				this.terminalOutput += prefix + (result.error || 'Command failed') + '\n';
			}
			return result;
		} catch (e) {
			this.terminalOutput += `$ ${command}\nError: ${e}\n`;
			return null;
		}
	}

	async sendAgentMessage(userMessage: string) {
		this.agentMessages = [
			...this.agentMessages,
			{ role: 'user', content: userMessage, timestamp: Date.now() },
		];
		this.agentLoading = true;

		try {
			const context = this.buildAgentContext(userMessage);
			const response = await invoke<string>('route_message', {
				message: { content: context, model_id: null, conversation_id: null },
			});

			const toolCalls = this.extractToolCalls(response);

			if (toolCalls.length > 0) {
				for (const call of toolCalls) {
					const result = await invoke<ToolResult>('ide_agent_tool_call', { tool: call });
					this.agentMessages = [
						...this.agentMessages,
						{
							role: 'tool', content: result.output, timestamp: Date.now(),
							toolCall: { tool: call.tool, args: call.args || {}, result },
						},
					];
				}

				const followUp = await invoke<string>('route_message', {
					message: {
						content: `Tool results received. Summarize what was done:\n${this.agentMessages
							.filter((m) => m.role === 'tool')
							.slice(-toolCalls.length)
							.map((m) => `[${m.toolCall?.tool}]: ${m.content.slice(0, 500)}`)
							.join('\n')}`,
						model_id: null, conversation_id: null,
					},
				});
				this.agentMessages = [
					...this.agentMessages,
					{ role: 'assistant', content: followUp, timestamp: Date.now() },
				];
			} else {
				this.agentMessages = [
					...this.agentMessages,
					{ role: 'assistant', content: response, timestamp: Date.now() },
				];
			}
		} catch (e) {
			this.agentMessages = [
				...this.agentMessages,
				{ role: 'assistant', content: `Error: ${e}`, timestamp: Date.now() },
			];
		} finally {
			this.agentLoading = false;
		}
	}

	private buildAgentContext(userMessage: string): string {
		const openFilesList = this.openTabs.map((t) => t.path).join(', ');
		const currentFile = this.activeTab ? `Current file: ${this.activeTab.path}` : 'No file open';

		return `You are an AI coding assistant with filesystem access. You can use these tools:
- read_file(path): Read a file's contents
- write_file(path, content): Write content to a file
- list_dir(path): List directory contents
- search(pattern, path): Search for text in files
- execute(command, cwd): Execute a shell command

Current working directory: ${this.currentDir}
${currentFile}
Open files: ${openFilesList || 'none'}

User request: ${userMessage}

If you need to use a tool, respond with a JSON block like:
\`\`\`tool
{"tool": "read_file", "args": {"path": "/path/to/file"}}
\`\`\`

Otherwise, respond directly.`;
	}

	private extractToolCalls(response: string): Array<{ tool: string; args: Record<string, string> }> {
		const toolPattern = /```tool\s*\n([\s\S]*?)\n```/g;
		const calls: Array<{ tool: string; args: Record<string, string> }> = [];
		let match;
		while ((match = toolPattern.exec(response)) !== null) {
			try {
				const parsed = JSON.parse(match[1]);
				if (parsed.tool) calls.push(parsed);
			} catch { /* skip invalid JSON */ }
		}
		return calls;
	}
}

export const ide = new IdeStore();
