<script lang="ts">
	/**
	 * InnerThoughtsSuggestion — Subtle proactive AI suggestion toast.
	 *
	 * Based on arXiv:2501.00383 "Proactive Conversational Agents with Inner Thoughts"
	 * (CHI 2025). Shows a small floating card at bottom-right when the AI has a
	 * suggestion worth sharing. Only appears when the user is idle (not typing).
	 *
	 * - Maximum 1 suggestion visible at a time (queued behind the scenes)
	 * - Auto-hides after 30 seconds
	 * - Fades in with slide-up animation
	 * - Polls every 30 seconds via thoughts_get_suggestions()
	 */
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Sparkles, X, ChevronRight } from '@lucide/svelte';
	import { goto } from '$app/navigation';

	interface Suggestion {
		id: string;
		thought_id: string;
		title: string;
		description: string;
		action_label: string | null;
		action_route: string | null;
		priority: 'low' | 'medium' | 'high' | 'critical';
		created_at: string;
	}

	let currentSuggestion: Suggestion | null = $state(null);
	let fading = $state(false);
	let pollTimer: ReturnType<typeof setInterval> | null = null;
	let autoDismissTimer: ReturnType<typeof setTimeout> | null = null;

	const AUTO_DISMISS_MS = 30_000;
	const POLL_INTERVAL_MS = 30_000;
	const FADE_DURATION_MS = 250;

	// Priority accent color mapping
	let priorityColor = $derived(
		currentSuggestion?.priority === 'critical'
			? 'text-gx-accent-red'
			: currentSuggestion?.priority === 'high'
				? 'text-gx-accent-orange'
				: 'text-gx-neon'
	);

	let priorityBorder = $derived(
		currentSuggestion?.priority === 'critical'
			? 'border-gx-accent-red/30'
			: currentSuggestion?.priority === 'high'
				? 'border-gx-accent-orange/30'
				: 'border-gx-neon/20'
	);

	let priorityGlow = $derived(
		currentSuggestion?.priority === 'critical'
			? 'shadow-[0_0_20px_rgba(255,51,102,0.15)]'
			: currentSuggestion?.priority === 'high'
				? 'shadow-[0_0_20px_rgba(255,102,0,0.12)]'
				: 'shadow-[0_0_16px_rgba(0,255,102,0.08)]'
	);

	async function fetchSuggestions() {
		try {
			const suggestions: Suggestion[] = await invoke('thoughts_get_suggestions');
			if (suggestions.length > 0 && !currentSuggestion) {
				showSuggestion(suggestions[0]);
			}
		} catch {
			// Engine might be disabled or Ollama offline — silently skip
		}
	}

	function showSuggestion(suggestion: Suggestion) {
		fading = false;
		currentSuggestion = suggestion;

		// Auto-dismiss after 30 seconds
		clearAutoDismiss();
		autoDismissTimer = setTimeout(() => {
			dismissSuggestion();
		}, AUTO_DISMISS_MS);
	}

	function clearAutoDismiss() {
		if (autoDismissTimer) {
			clearTimeout(autoDismissTimer);
			autoDismissTimer = null;
		}
	}

	async function acceptSuggestion() {
		if (!currentSuggestion) return;
		const suggestion = currentSuggestion;

		try {
			await invoke('thoughts_accept', { suggestionId: suggestion.id });
		} catch {
			// Best-effort feedback
		}

		// Navigate if action_route is set
		if (suggestion.action_route) {
			goto(suggestion.action_route);
		}

		hideSuggestion();
	}

	async function dismissSuggestion() {
		if (!currentSuggestion) return;

		try {
			await invoke('thoughts_dismiss', { suggestionId: currentSuggestion.id });
		} catch {
			// Best-effort feedback
		}

		hideSuggestion();
	}

	function hideSuggestion() {
		clearAutoDismiss();
		fading = true;
		setTimeout(() => {
			currentSuggestion = null;
			fading = false;
		}, FADE_DURATION_MS);
	}

	onMount(() => {
		// Start polling after a short delay (let app settle)
		const startDelay = setTimeout(() => {
			fetchSuggestions();
			pollTimer = setInterval(fetchSuggestions, POLL_INTERVAL_MS);
		}, 5000);

		return () => {
			clearTimeout(startDelay);
			if (pollTimer) clearInterval(pollTimer);
			clearAutoDismiss();
		};
	});
</script>

{#if currentSuggestion}
	<div
		class="fixed bottom-4 right-4 w-80 bg-gx-bg-elevated border {priorityBorder}
				rounded-lg {priorityGlow} p-4 z-40
				{fading ? 'animate-gx-fade-out' : 'animate-gx-slide-up'}"
		role="alert"
		aria-live="polite"
		aria-label="AI Suggestion"
	>
		<!-- Header row -->
		<div class="flex items-start gap-3">
			<Sparkles size={16} class="{priorityColor} mt-0.5 shrink-0" />
			<div class="flex-1 min-w-0">
				<h4 class="text-sm font-medium text-gx-text-primary leading-tight">
					{currentSuggestion.title}
				</h4>
				<p class="text-xs text-gx-text-muted mt-1.5 leading-relaxed">
					{currentSuggestion.description}
				</p>

				<!-- Actions -->
				<div class="flex items-center gap-2 mt-3">
					{#if currentSuggestion.action_label}
						<button
							onclick={acceptSuggestion}
							class="flex items-center gap-1 px-2.5 py-1 text-xs font-medium
									bg-gx-neon/10 text-gx-neon border border-gx-neon/30
									rounded hover:bg-gx-neon/20 transition-colors"
						>
							{currentSuggestion.action_label}
							<ChevronRight size={12} />
						</button>
					{:else}
						<button
							onclick={acceptSuggestion}
							class="flex items-center gap-1 px-2.5 py-1 text-xs font-medium
									bg-gx-neon/10 text-gx-neon border border-gx-neon/30
									rounded hover:bg-gx-neon/20 transition-colors"
						>
							Accept
						</button>
					{/if}
					<button
						onclick={dismissSuggestion}
						class="px-2.5 py-1 text-xs text-gx-text-muted
								hover:text-gx-text-secondary transition-colors"
					>
						Dismiss
					</button>
				</div>
			</div>

			<!-- Close button -->
			<button
				onclick={dismissSuggestion}
				class="text-gx-text-muted hover:text-gx-text-secondary transition-colors shrink-0 -mt-1 -mr-1"
				aria-label="Close suggestion"
			>
				<X size={14} />
			</button>
		</div>
	</div>
{/if}
