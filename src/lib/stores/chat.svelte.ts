/**
 * Chat Store — Conversations, streaming messages, model routing
 * Class-based Svelte 5 pattern for shared mutable state.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Message {
	id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	timestamp: Date;
	model?: string;
	taskType?: string;
}

export interface Conversation {
	id: string;
	title: string;
	messages: Message[];
	createdAt: Date;
	updatedAt: Date;
}

function uid(): string {
	return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

class ChatStore {
	conversations = $state<Conversation[]>([]);
	activeConversationId = $state<string | null>(null);
	sending = $state(false);
	streamingContent = $state('');

	get activeConversation(): Conversation | undefined {
		return this.conversations.find((c) => c.id === this.activeConversationId);
	}

	get sortedConversations(): Conversation[] {
		return [...this.conversations].sort(
			(a, b) => b.updatedAt.getTime() - a.updatedAt.getTime()
		);
	}

	createConversation(title = 'New Chat'): string {
		const id = uid();
		const conv: Conversation = {
			id,
			title,
			messages: [],
			createdAt: new Date(),
			updatedAt: new Date(),
		};
		this.conversations = [conv, ...this.conversations];
		this.activeConversationId = id;
		return id;
	}

	setActive(id: string) {
		this.activeConversationId = id;
	}

	deleteConversation(id: string) {
		this.conversations = this.conversations.filter((c) => c.id !== id);
		if (this.activeConversationId === id) {
			this.activeConversationId = this.conversations[0]?.id ?? null;
		}
	}

	async sendMessage(content: string, modelId?: string) {
		if (!this.activeConversationId || this.sending) return;

		const conv = this.activeConversation;
		if (!conv) return;

		this.sending = true;
		this.streamingContent = '';

		// Add user message
		conv.messages = [
			...conv.messages,
			{ id: uid(), role: 'user', content, timestamp: new Date() },
		];
		conv.updatedAt = new Date();

		// Auto-title from first message
		if (conv.messages.length === 1) {
			conv.title = content.slice(0, 40) + (content.length > 40 ? '...' : '');
		}

		// Listen for streaming tokens
		const unlisten = await listen<{
			conversation_id: string;
			delta: string;
			done: boolean;
			full_content?: string;
		}>('chat-stream', (event) => {
			if (event.payload.conversation_id !== this.activeConversationId) return;

			if (event.payload.done) {
				const finalContent = event.payload.full_content || this.streamingContent;
				conv.messages = [
					...conv.messages,
					{
						id: uid(),
						role: 'assistant',
						content: finalContent,
						timestamp: new Date(),
						model: modelId,
					},
				];
				conv.updatedAt = new Date();
				this.streamingContent = '';
				this.sending = false;
				unlisten();
			} else {
				this.streamingContent += event.payload.delta;
			}
		});

		try {
			await invoke('route_message_stream', {
				message: {
					content,
					model_id: modelId || null,
					conversation_id: this.activeConversationId,
				},
			});
		} catch (e) {
			conv.messages = [
				...conv.messages,
				{
					id: uid(),
					role: 'assistant',
					content: `Error: ${e}`,
					timestamp: new Date(),
				},
			];
			this.streamingContent = '';
			this.sending = false;
			unlisten();
		}
	}
}

export const chat = new ChatStore();
