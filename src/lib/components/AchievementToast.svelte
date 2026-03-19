<script lang="ts">
	/**
	 * AchievementToast -- Celebration pop-up when an achievement is unlocked.
	 *
	 * Shows an animated notification with the achievement icon, name, and XP reward.
	 * Auto-dismisses after 5 seconds. Renders sparkle particles for visual flair.
	 */

	import { achievementStore, RARITY_LABELS, RARITY_COLORS } from '$lib/stores/achievements.svelte';
	import { X, Star } from '@lucide/svelte';

	let visible = $state(false);
	let currentAchievement = $derived(achievementStore.toastQueue[0] ?? null);
	let dismissTimer: ReturnType<typeof setTimeout> | null = null;

	// Sparkle positions — pseudo-random decorative particles
	const sparkles = Array.from({ length: 8 }, (_, i) => ({
		x: 10 + (i * 37) % 80,
		y: 5 + (i * 23) % 60,
		delay: i * 120,
		size: 4 + (i % 3) * 2,
	}));

	$effect(() => {
		if (currentAchievement && !visible) {
			visible = true;
			if (dismissTimer) clearTimeout(dismissTimer);
			dismissTimer = setTimeout(() => {
				dismiss();
			}, 5000);
		}
	});

	function dismiss() {
		visible = false;
		if (dismissTimer) {
			clearTimeout(dismissTimer);
			dismissTimer = null;
		}
		// Small delay for exit animation before removing from queue
		setTimeout(() => {
			achievementStore.dismissToast();
		}, 300);
	}
</script>

{#if visible && currentAchievement}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed top-6 right-6 z-[100] pointer-events-auto"
		role="status"
		aria-live="polite"
	>
		<div
			class="relative overflow-hidden rounded-xl border border-amber-500/40 bg-gx-bg-elevated shadow-2xl shadow-amber-500/10 min-w-[320px] max-w-[400px] animate-in slide-in-from-right-full duration-500"
		>
			<!-- Sparkle particles -->
			{#each sparkles as sp (sp.delay)}
				<div
					class="absolute pointer-events-none opacity-0"
					style="
						left: {sp.x}%;
						top: {sp.y}%;
						animation: sparkle-fade 1.5s ease-out {sp.delay}ms forwards;
					"
				>
					<Star size={sp.size} class="text-amber-400/60" />
				</div>
			{/each}

			<!-- Glow gradient background -->
			<div
				class="absolute inset-0 pointer-events-none"
				style="background: radial-gradient(ellipse at 30% 50%, rgba(245, 158, 11, 0.08) 0%, transparent 70%);"
			></div>

			<div class="relative flex items-start gap-3 p-4">
				<!-- Achievement icon -->
				<div class="flex items-center justify-center w-12 h-12 rounded-lg bg-amber-500/10 border border-amber-500/20 text-2xl shrink-0 animate-in zoom-in-50 duration-700">
					{currentAchievement.icon}
				</div>

				<div class="flex-1 min-w-0">
					<!-- Header -->
					<div class="flex items-center gap-2 mb-0.5">
						<span class="text-[10px] font-bold uppercase tracking-wider text-amber-400">
							Achievement Unlocked
						</span>
						<span class="text-[9px] px-1.5 py-0.5 rounded-full bg-gx-bg-tertiary {RARITY_COLORS[currentAchievement.rarity]}">
							{RARITY_LABELS[currentAchievement.rarity]}
						</span>
					</div>

					<!-- Name -->
					<h3 class="text-sm font-semibold text-gx-text-primary truncate">
						{currentAchievement.name}
					</h3>

					<!-- Description -->
					<p class="text-xs text-gx-text-muted mt-0.5 leading-snug">
						{currentAchievement.description}
					</p>

					<!-- XP reward -->
					<div class="flex items-center gap-1 mt-1.5">
						<span class="text-xs font-bold text-amber-400">+{currentAchievement.xp_reward} XP</span>
					</div>
				</div>

				<!-- Close button -->
				<button
					onclick={dismiss}
					class="shrink-0 text-gx-text-muted hover:text-gx-text-primary transition-colors p-0.5"
					aria-label="Dismiss notification"
				>
					<X size={14} />
				</button>
			</div>

			<!-- Progress bar auto-dismiss indicator -->
			<div class="h-0.5 bg-gx-bg-tertiary">
				<div
					class="h-full bg-amber-500/60 transition-all"
					style="animation: shrink-bar 5s linear forwards;"
				></div>
			</div>
		</div>
	</div>
{/if}

<style>
	@keyframes sparkle-fade {
		0% {
			opacity: 0;
			transform: scale(0) rotate(0deg);
		}
		30% {
			opacity: 1;
			transform: scale(1.2) rotate(30deg);
		}
		100% {
			opacity: 0;
			transform: scale(0.5) rotate(90deg) translateY(-10px);
		}
	}

	@keyframes shrink-bar {
		from {
			width: 100%;
		}
		to {
			width: 0%;
		}
	}
</style>
