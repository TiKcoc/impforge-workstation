<script lang="ts">
	/**
	 * EvolutionAnimation -- Full-screen overlay for unit evolution events
	 *
	 * Plays a dramatic transition when a unit evolves:
	 *   1. Old unit shrinks and fades
	 *   2. Gold flash burst fills the center
	 *   3. New unit spawns in with the sf-animate-spawn animation
	 *   4. "EVOLUTION!" text with mutation details
	 *   5. Auto-dismiss after a few seconds or on click
	 *
	 * Uses existing sf-* keyframes from app.css.
	 * Svelte 5 runes only.
	 */

	import UnitAvatar from './UnitAvatar.svelte';

	interface Mutation {
		mutation_type: string;
		name: string;
	}

	interface Props {
		unitName: string;
		oldType: string;
		oldLevel: number;
		oldMutations: Mutation[];
		newType: string;
		newLevel: number;
		newMutations: Mutation[];
		newMutationName?: string;
		onclose?: () => void;
	}

	let {
		unitName,
		oldType,
		oldLevel,
		oldMutations,
		newType,
		newLevel,
		newMutations,
		newMutationName,
		onclose
	}: Props = $props();

	// ── Animation phase state machine ─────────────────────────────────────
	// 0 = old shrinking, 1 = flash, 2 = new spawning, 3 = text reveal

	let phase = $state(0);
	let visible = $state(true);

	$effect(() => {
		// Phase 0 -> 1 after 600ms (old unit shrinks)
		const t0 = setTimeout(() => { phase = 1; }, 600);
		// Phase 1 -> 2 after 1000ms (flash)
		const t1 = setTimeout(() => { phase = 2; }, 1000);
		// Phase 2 -> 3 after 1600ms (new unit appears)
		const t2 = setTimeout(() => { phase = 3; }, 1600);
		// Auto-close after 5s
		const t3 = setTimeout(() => { dismiss(); }, 5000);
		return () => { clearTimeout(t0); clearTimeout(t1); clearTimeout(t2); clearTimeout(t3); };
	});

	function dismiss() {
		visible = false;
		onclose?.();
	}

	// Format unit type for display
	function formatType(t: string): string {
		return t.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
	}
</script>

{#if visible}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex items-center justify-center"
		onclick={dismiss}
		onkeydown={(e) => { if (e.key === 'Escape' || e.key === 'Enter') dismiss(); }}
		role="dialog"
		aria-label="Unit evolution animation"
		aria-modal="true"
		tabindex="-1"
	>
		<!-- Backdrop -->
		<div
			class="absolute inset-0 bg-black/80 transition-opacity duration-500"
			class:opacity-0={!visible}
		></div>

		<!-- Content -->
		<div class="relative flex flex-col items-center gap-4 z-10">

			<!-- Old unit (shrinks away) -->
			<div
				class="transition-all duration-500 ease-in"
				class:scale-100={phase === 0}
				class:opacity-100={phase === 0}
				class:scale-0={phase >= 1}
				class:opacity-0={phase >= 1}
			>
				<UnitAvatar
					unitType={oldType}
					level={oldLevel}
					mutations={oldMutations}
					size="xl"
					state="idle"
					showLabel={false}
				/>
				<div class="text-center text-xs text-gray-400 mt-1">
					{formatType(oldType)} Lv.{oldLevel}
				</div>
			</div>

			<!-- Gold flash burst -->
			<div
				class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full sf-mutation-evo transition-all duration-300"
				class:w-0={phase < 1}
				class:h-0={phase < 1}
				class:opacity-0={phase < 1 || phase >= 3}
				class:w-40={phase >= 1 && phase < 3}
				class:h-40={phase >= 1 && phase < 3}
				class:opacity-100={phase >= 1 && phase < 3}
				style="background: radial-gradient(circle, rgba(234,179,8,0.4) 0%, rgba(234,179,8,0) 70%);"
			></div>

			<!-- New unit (spawns in) -->
			{#if phase >= 2}
				<div class="sf-animate-spawn">
					<UnitAvatar
						unitType={newType}
						level={newLevel}
						mutations={newMutations}
						size="xl"
						state="evolving"
						showLabel={false}
					/>
				</div>
			{/if}

			<!-- Evolution text -->
			{#if phase >= 3}
				<div class="text-center sf-animate-tab-enter">
					<h2
						class="text-2xl font-bold font-display tracking-wider mb-1"
						style="color: #eab308; text-shadow: 0 0 20px rgba(234,179,8,0.5);"
					>
						EVOLUTION!
					</h2>
					<p class="text-sm text-gray-300">
						{unitName} evolved to <span class="text-yellow-300 font-bold">{formatType(newType)}</span>
					</p>
					{#if newMutationName}
						<p class="text-xs text-purple-300 mt-1">
							New mutation: <span class="font-bold">{newMutationName}</span>
						</p>
					{/if}
					<p class="text-[10px] text-gray-500 mt-3">Click or press Escape to continue</p>
				</div>
			{/if}
		</div>
	</div>
{/if}
