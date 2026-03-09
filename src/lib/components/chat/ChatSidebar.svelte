<script lang="ts">
	/**
	 * ChatSidebar — BenikUI-integrated conversation list sidebar
	 *
	 * Conversation history with new chat button and conversation management.
	 * Uses style engine for deep customization of all sidebar elements.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Sidebar wrapper
	 *   - header: Header with new chat button
	 *   - item: Conversation list item
	 *   - item-active: Active conversation item
	 *   - footer: Footer with stats
	 */

	import { chatStore } from '$lib/stores/chat.svelte';
	import { MessageSquare, Plus, Trash2 } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		collapsed?: boolean;
	}

	let { widgetId = 'chat-sidebar', collapsed = false }: Props = $props();

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let headerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let itemComponent = $derived(styleEngine.getComponentStyle(widgetId, 'item'));
	let itemActiveComponent = $derived(styleEngine.getComponentStyle(widgetId, 'item-active'));
	let footerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'footer'));

	let containerStyle = $derived(
		hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : ''
	);
	let headerStyle = $derived(
		hasEngineStyle && headerComponent ? componentToCSS(headerComponent) : ''
	);
	let footerStyle = $derived(
		hasEngineStyle && footerComponent ? componentToCSS(footerComponent) : ''
	);

	function getItemStyle(isActive: boolean): string {
		if (!hasEngineStyle) return '';
		const comp = isActive ? itemActiveComponent : itemComponent;
		return comp ? componentToCSS(comp) : '';
	}
</script>

{#if !collapsed}
	<div
		class="w-[250px] border-r border-gx-border-default flex flex-col shrink-0 {hasEngineStyle ? '' : 'bg-gx-bg-secondary'}"
		style={containerStyle}
	>
		<div class="p-3 border-b border-gx-border-default" style={headerStyle}>
			<button
				onclick={() => chatStore.newConversation()}
				class="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors shadow-gx-glow-sm"
			>
				<Plus size={14} />
				New Chat
			</button>
		</div>

		<div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5">
			{#each chatStore.conversations as conv (conv.id)}
				{@const isActive = chatStore.activeConversationId === conv.id}
				<div
					role="button"
					tabindex="0"
					onclick={() => chatStore.setActive(conv.id)}
					onkeydown={(e) => e.key === 'Enter' && chatStore.setActive(conv.id)}
					class="group flex items-center gap-2 px-3 py-2.5 text-xs rounded-gx cursor-pointer transition-colors
						{isActive
							? hasEngineStyle ? '' : 'bg-gx-bg-elevated text-gx-text-primary border-l-2 border-gx-neon'
							: hasEngineStyle ? '' : 'text-gx-text-muted hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
					style={getItemStyle(isActive)}
				>
					<MessageSquare size={12} class="shrink-0 {isActive ? 'text-gx-neon' : ''}" />
					<span class="flex-1 truncate">{conv.title}</span>
					<button
						onclick={(e) => { e.stopPropagation(); chatStore.deleteConversation(conv.id); }}
						class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all p-0.5 rounded"
						aria-label="Delete conversation"
					>
						<Trash2 size={12} />
					</button>
				</div>
			{/each}

			{#if chatStore.conversations.length === 0}
				<div class="text-[11px] text-gx-text-muted text-center py-8 px-4">
					No conversations yet. Start a new chat!
				</div>
			{/if}
		</div>

		<div class="p-3 border-t border-gx-border-default" style={footerStyle}>
			<div class="text-[10px] text-gx-text-muted text-center">
				{chatStore.conversations.length} conversation{chatStore.conversations.length !== 1 ? 's' : ''}
			</div>
		</div>
	</div>
{/if}
