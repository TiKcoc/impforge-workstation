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
	// Match thinking blocks: <thinking>...</thinking>
	// Everything else is chat (marked.js handles code blocks internally)

	const parts = content.split(/(<thinking>[\s\S]*?<\/thinking>)/);

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

		// For non-thinking parts, just treat as chat (marked.js handles code blocks internally)
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
