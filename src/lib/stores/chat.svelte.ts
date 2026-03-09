/**
 * Chat Store — Conversations, streaming messages, Tauri Channel-based streaming
 * Class-based Svelte 5 pattern for shared mutable state.
 */

import { invoke, Channel } from '@tauri-apps/api/core';
import { modelStatus } from './model-status.svelte';

export interface Message {
	id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	timestamp: Date;
	model?: string;
	taskType?: string;
	streaming?: boolean;
}

export interface Conversation {
	id: string;
	title: string;
	messages: Message[];
	createdAt: Date;
}

type ChatEvent =
	| { event: 'Started'; data: { model: string; task_type: string } }
	| { event: 'Delta'; data: { content: string } }
	| { event: 'Finished'; data: { total_tokens: number } }
	| { event: 'Error'; data: { message: string } }
	| { event: 'Routing'; data: { task_type: string; selected_model: string; reason: string; classification_ms: number } };

class ChatStore {
	conversations = $state<Conversation[]>([]);
	activeConversationId = $state<string | null>(null);
	isStreaming = $state(false);
	selectedModel = $state<string | null>(null);

	/** ForgeMemory conversation ID — persists messages to SQLite */
	forgeConversationId = $state<string | null>(null);
	/** Toggle memory enrichment on/off (default: on) */
	memoryEnabled = $state(true);

	get activeConversation() {
		return this.conversations.find((c) => c.id === this.activeConversationId) ?? null;
	}

	get messages() {
		return this.activeConversation?.messages ?? [];
	}

	newConversation() {
		const id = crypto.randomUUID();
		this.conversations.unshift({
			id,
			title: 'New Chat',
			messages: [],
			createdAt: new Date(),
		});
		this.activeConversationId = id;
		this.forgeConversationId = null; // Reset for new conversation
		return id;
	}

	setActive(id: string) {
		this.activeConversationId = id;
	}

	async sendMessage(content: string, openrouterKey: string) {
		if (!this.activeConversationId) this.newConversation();
		const conv = this.activeConversation!;

		// ForgeMemory: Create persistent conversation on first message
		if (this.memoryEnabled && !this.forgeConversationId) {
			try {
				this.forgeConversationId = await invoke<string>('forge_memory_create_conversation', {
					title: content.slice(0, 80),
					modelId: this.selectedModel,
				});
			} catch {
				// Memory failure must never block chat
			}
		}

		conv.messages.push({
			id: crypto.randomUUID(),
			role: 'user',
			content,
			timestamp: new Date(),
		});

		const assistantMsg: Message = {
			id: crypto.randomUUID(),
			role: 'assistant',
			content: '',
			timestamp: new Date(),
			streaming: true,
		};
		conv.messages.push(assistantMsg);

		this.isStreaming = true;

		const channel = new Channel<ChatEvent>();
		channel.onmessage = (event: ChatEvent) => {
			switch (event.event) {
				case 'Routing':
					modelStatus.lastRouting = {
						taskType: event.data.task_type,
						model: event.data.selected_model,
						reason: event.data.reason,
					};
					break;
				case 'Started':
					assistantMsg.model = event.data.model;
					assistantMsg.taskType = event.data.task_type;
					modelStatus.onStarted(event.data.model, event.data.task_type);
					break;
				case 'Delta':
					assistantMsg.content += event.data.content;
					modelStatus.onDelta();
					break;
				case 'Finished':
					assistantMsg.streaming = false;
					this.isStreaming = false;
					modelStatus.onFinished(event.data.total_tokens);
					if (conv.title === 'New Chat' && conv.messages.length >= 2) {
						conv.title = conv.messages[0].content.slice(0, 50);
					}
					break;
				case 'Error':
					assistantMsg.content = `Error: ${event.data.message}`;
					assistantMsg.streaming = false;
					this.isStreaming = false;
					modelStatus.onError();
					break;
			}
		};

		try {
			await invoke('chat_stream', {
				message: content,
				modelId: this.selectedModel,
				systemPrompt: null,
				openrouterKey: openrouterKey,
				conversationId: this.memoryEnabled ? this.forgeConversationId : null,
				onEvent: channel,
			});
		} catch (e) {
			assistantMsg.content = `Error: ${e}`;
			assistantMsg.streaming = false;
			this.isStreaming = false;
		}
	}

	deleteConversation(id: string) {
		this.conversations = this.conversations.filter((c) => c.id !== id);
		if (this.activeConversationId === id) {
			this.activeConversationId = this.conversations[0]?.id ?? null;
		}
	}
}

export const chatStore = new ChatStore();
