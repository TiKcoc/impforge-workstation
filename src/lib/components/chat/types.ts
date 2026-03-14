/** Block types for the Unified Stream renderer */
export type BlockType =
	| 'chat'
	| 'code'
	| 'thinking'
	| 'tool'
	| 'terminal'
	| 'diff'
	| 'diagram'
	| 'math'
	| 'image'
	| 'routing'
	| 'error'
	| 'system';

export interface ChatBlock {
	id: string;
	type: BlockType;
	content: string;
	meta?: {
		lang?: string;
		tool?: string;
		model?: string;
		status?: string;
		durationMs?: number;
	};
}

/** Parse raw assistant content into typed blocks */
export function parseBlocks(content: string): ChatBlock[] {
	const blocks: ChatBlock[] = [];

	// Split on thinking, tool_call, and error tags
	const parts = content.split(
		/(<thinking>[\s\S]*?<\/thinking>|<tool_call>[\s\S]*?<\/tool_call>|<error>[\s\S]*?<\/error>|<progress>[\s\S]*?<\/progress>)/
	);

	for (const part of parts) {
		if (!part) continue;

		// Thinking block
		const thinkMatch = part.match(/^<thinking>([\s\S]*)<\/thinking>$/);
		if (thinkMatch) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'thinking',
				content: thinkMatch[1].trim(),
			});
			continue;
		}

		// Tool call block
		const toolMatch = part.match(/^<tool_call>([\s\S]*)<\/tool_call>$/);
		if (toolMatch) {
			const raw = toolMatch[1].trim();
			let toolName = 'tool';
			let status = 'running';
			try {
				const parsed = JSON.parse(raw);
				toolName = parsed.name || toolName;
				status = parsed.status || status;
			} catch { /* not JSON, use raw */ }
			blocks.push({
				id: crypto.randomUUID(),
				type: 'tool',
				content: raw,
				meta: { tool: toolName, status },
			});
			continue;
		}

		// Error block
		const errorMatch = part.match(/^<error>([\s\S]*)<\/error>$/);
		if (errorMatch) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'error',
				content: errorMatch[1].trim(),
			});
			continue;
		}

		// Progress block
		const progressMatch = part.match(/^<progress>([\s\S]*)<\/progress>$/);
		if (progressMatch) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'system',
				content: progressMatch[1].trim(),
				meta: { status: 'progress' },
			});
			continue;
		}

		// Everything else is chat (marked.js handles code/tables/links internally)
		if (part.trim()) {
			blocks.push({
				id: crypto.randomUUID(),
				type: 'chat',
				content: part,
			});
		}
	}

	return blocks.length > 0 ? blocks : [{ id: crypto.randomUUID(), type: 'chat', content }];
}
