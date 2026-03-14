<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- GuidedTour — Spotlight overlay with progressive disclosure tooltips -->
<script lang="ts">
	import { tourStore } from '$lib/stores/guided-tour.svelte';
	import { goto } from '$app/navigation';
	import { ChevronLeft, ChevronRight, X, Sparkles } from '@lucide/svelte';
	import { tick } from 'svelte';

	let spotlightRect = $state<DOMRect | null>(null);

	// Position the spotlight on the target element
	$effect(() => {
		const step = tourStore.currentStep;
		if (!step) {
			spotlightRect = null;
			return;
		}

		async function positionSpotlight() {
			// Navigate if needed
			if (step!.route) {
				await goto(step!.route);
				await tick();
				// Wait for DOM update after navigation
				await new Promise(r => setTimeout(r, 300));
			}

			const el = document.querySelector(step!.target);
			if (el) {
				spotlightRect = el.getBoundingClientRect();
			} else {
				// Target not found — show tooltip centered
				spotlightRect = null;
			}
		}

		positionSpotlight();
	});

	// Calculate tooltip position
	let tooltipStyle = $derived.by(() => {
		if (!spotlightRect || !tourStore.currentStep) return 'top: 50%; left: 50%; transform: translate(-50%, -50%);';

		const step = tourStore.currentStep;
		const pad = 16;
		const r = spotlightRect;

		switch (step.placement) {
			case 'right':
				return `top: ${r.top + r.height / 2}px; left: ${r.right + pad}px; transform: translateY(-50%);`;
			case 'left':
				return `top: ${r.top + r.height / 2}px; right: ${window.innerWidth - r.left + pad}px; transform: translateY(-50%);`;
			case 'bottom':
				return `top: ${r.bottom + pad}px; left: ${r.left + r.width / 2}px; transform: translateX(-50%);`;
			case 'top':
				return `bottom: ${window.innerHeight - r.top + pad}px; left: ${r.left + r.width / 2}px; transform: translateX(-50%);`;
			default:
				return `top: 50%; left: 50%; transform: translate(-50%, -50%);`;
		}
	});
</script>

{#if tourStore.isActive && tourStore.currentStep}
	<!-- Backdrop with spotlight cutout -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-[9998]"
		onkeydown={(e) => { if (e.key === 'Escape') tourStore.skipTour(); }}
	>
		<!-- Dark overlay -->
		<div class="absolute inset-0 bg-black/50 backdrop-blur-[1px]" onclick={() => tourStore.skipTour()}></div>

		<!-- Spotlight cutout (bright area around target) -->
		{#if spotlightRect}
			<div
				class="absolute rounded-lg border-2 border-gx-neon shadow-gx-glow-lg pointer-events-none"
				style="
					top: {spotlightRect.top - 4}px;
					left: {spotlightRect.left - 4}px;
					width: {spotlightRect.width + 8}px;
					height: {spotlightRect.height + 8}px;
					box-shadow: 0 0 0 9999px rgba(0,0,0,0.5), 0 0 20px rgba(0,255,102,0.3);
					z-index: 9999;
				"
			></div>
		{/if}

		<!-- Tooltip card -->
		<div
			class="fixed z-[10000] w-80 rounded-gx-lg bg-gx-bg-secondary border border-gx-border-default shadow-gx-glow-lg overflow-hidden"
			style={tooltipStyle}
		>
			<!-- Progress bar -->
			<div class="h-1 bg-gx-bg-primary">
				<div
					class="h-full bg-gx-neon transition-all duration-300"
					style="width: {tourStore.progress}%"
				></div>
			</div>

			<!-- Content -->
			<div class="p-4">
				<div class="flex items-start justify-between gap-2 mb-2">
					<div class="flex items-center gap-2">
						<Sparkles size={14} class="text-gx-neon shrink-0" />
						<h3 class="text-sm font-bold text-gx-text-primary">{tourStore.currentStep.title}</h3>
					</div>
					<button
						onclick={() => tourStore.skipTour()}
						class="p-1 rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
						aria-label="Close tour"
					>
						<X size={14} />
					</button>
				</div>

				<p class="text-xs text-gx-text-secondary leading-relaxed mb-4">
					{tourStore.currentStep.description}
				</p>

				<!-- Navigation -->
				<div class="flex items-center justify-between">
					<span class="text-[10px] text-gx-text-disabled">
						{tourStore.currentStepIndex + 1} / {tourStore.totalSteps}
					</span>

					<div class="flex items-center gap-2">
						{#if tourStore.currentStepIndex > 0}
							<button
								onclick={() => tourStore.prevStep()}
								class="flex items-center gap-1 px-2 py-1 text-[11px] text-gx-text-muted hover:text-gx-text-primary rounded hover:bg-gx-bg-hover transition-colors"
							>
								<ChevronLeft size={12} />
								Back
							</button>
						{/if}

						<button
							onclick={() => tourStore.nextStep()}
							class="flex items-center gap-1 px-3 py-1.5 text-[11px] font-semibold rounded-gx bg-gx-neon text-gx-bg-primary hover:brightness-110 transition-all"
						>
							{#if tourStore.currentStepIndex === tourStore.totalSteps - 1}
								Finish
							{:else}
								Next
								<ChevronRight size={12} />
							{/if}
						</button>
					</div>
				</div>
			</div>
		</div>
	</div>
{/if}
