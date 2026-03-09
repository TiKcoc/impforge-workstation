<script lang="ts">
	/**
	 * WidgetPalette.svelte — Widget Catalog Panel
	 *
	 * Shows available widgets organized by category. Users can click
	 * to add widgets to the current layout. Displays min/max sizes
	 * and configuration options.
	 *
	 * Pattern: ElvUI's "Toggle Anchors" sidebar with available frames
	 */

	import { layoutManager } from '$lib/stores/layout-manager.svelte';
	import { themeStore, type WidgetDefinition } from '$lib/stores/theme.svelte';
	import { Plus, Package, Grid3x3, Minus } from '@lucide/svelte';

	// Group widgets by category
	let categories = $derived.by(() => {
		const map = new Map<string, WidgetDefinition[]>();
		for (const w of themeStore.widgets) {
			const cat = w.category || 'Other';
			if (!map.has(cat)) map.set(cat, []);
			map.get(cat)!.push(w);
		}
		return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
	});

	// Track which widgets are already placed
	let placedWidgets = $derived(new Set(layoutManager.movers.keys()));

	function handleAddWidget(widgetId: string) {
		layoutManager.addWidget(widgetId);
	}

	function handleRemoveWidget(widgetId: string) {
		layoutManager.removeMover(widgetId);
	}
</script>

<div class="flex flex-col h-full bg-gx-bg-secondary border-l border-gx-border-default">
	<!-- Header -->
	<div class="flex items-center gap-2 h-9 px-3 border-b border-gx-border-default shrink-0">
		<Package size={14} class="text-gx-accent-purple" />
		<span class="text-xs font-medium text-gx-text-secondary">Widget Palette</span>
		<div class="flex-1"></div>
		<span class="text-[10px] text-gx-text-muted font-mono">
			{layoutManager.movers.size} placed
		</span>
	</div>

	<!-- Widget list by category -->
	<div class="flex-1 overflow-y-auto p-2 space-y-3">
		{#each categories as [category, widgets]}
			<div>
				<!-- Category header -->
				<div class="flex items-center gap-1.5 mb-1.5 px-1">
					<Grid3x3 size={10} class="text-gx-text-muted" />
					<span class="text-[10px] font-semibold uppercase tracking-wider text-gx-text-muted">
						{category}
					</span>
					<span class="text-[9px] text-gx-text-muted">({widgets.length})</span>
				</div>

				<!-- Widget cards -->
				<div class="space-y-1">
					{#each widgets as widget}
						{@const isPlaced = placedWidgets.has(widget.id)}
						<div
							class="flex items-center gap-2 px-2 py-1.5 rounded-gx border transition-all
								{isPlaced
									? 'border-gx-neon/30 bg-gx-neon/5'
									: 'border-gx-border-default bg-gx-bg-primary hover:border-gx-accent-purple/50 hover:bg-gx-bg-hover'}"
						>
							<!-- Widget info -->
							<div class="flex-1 min-w-0">
								<div class="text-[11px] text-gx-text-secondary font-medium truncate">
									{widget.name}
								</div>
								<div class="text-[9px] text-gx-text-muted truncate">
									{widget.description}
								</div>
								<div class="text-[8px] font-mono text-gx-text-muted mt-0.5">
									{widget.default_size[0]}×{widget.default_size[1]} (min {widget.min_size[0]}×{widget.min_size[1]})
								</div>
							</div>

							<!-- Add/Remove button -->
							{#if isPlaced}
								<button
									onclick={() => handleRemoveWidget(widget.id)}
									class="p-1 rounded hover:bg-gx-status-error/20 text-gx-neon hover:text-gx-status-error transition-colors shrink-0"
									title="Remove from layout"
								>
									<Minus size={12} />
								</button>
							{:else}
								<button
									onclick={() => handleAddWidget(widget.id)}
									class="p-1 rounded hover:bg-gx-neon/20 text-gx-text-muted hover:text-gx-neon transition-colors shrink-0"
									title="Add to layout"
								>
									<Plus size={12} />
								</button>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		{/each}

		{#if categories.length === 0}
			<div class="text-center py-8 text-gx-text-muted text-xs">
				No widgets registered yet.
				<br />
				<span class="text-[10px]">Widgets will appear when the registry loads.</span>
			</div>
		{/if}
	</div>

	<!-- Footer: Layout actions -->
	<div class="border-t border-gx-border-default p-2 space-y-1.5 shrink-0">
		<button
			onclick={() => layoutManager.saveCurrentLayout()}
			disabled={!layoutManager.isDirty}
			class="w-full text-[11px] py-1.5 px-2 rounded-gx border transition-all
				{layoutManager.isDirty
					? 'border-gx-neon/50 text-gx-neon bg-gx-neon/10 hover:bg-gx-neon/20'
					: 'border-gx-border-default text-gx-text-muted bg-gx-bg-primary cursor-not-allowed'}"
		>
			Save Layout
		</button>
		<button
			onclick={() => layoutManager.resetLayout()}
			class="w-full text-[11px] py-1.5 px-2 rounded-gx border border-gx-border-default text-gx-text-muted bg-gx-bg-primary hover:border-gx-status-error/50 hover:text-gx-status-error transition-all"
		>
			Reset Layout
		</button>
	</div>
</div>
