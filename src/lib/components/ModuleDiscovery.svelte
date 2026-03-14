<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ModuleDiscovery — Explore and activate modules not in your current profile -->
<script lang="ts">
	import { tourStore, ALL_MODULES, type ModuleInfo } from '$lib/stores/guided-tour.svelte';
	import { getSetting, saveSetting, getVisibleModules } from '$lib/stores/settings.svelte';
	import { X, Plus, Eye, Check, Grid3x3 } from '@lucide/svelte';

	let hiddenModules = $derived(tourStore.getHiddenModules());
	let visibleIds = $derived(getVisibleModules());

	async function activateModule(moduleId: string) {
		// Switch to 'custom' role and manually manage modules
		// For now, the simplest approach: switch user to 'custom' (shows everything)
		// Future: per-module toggle without changing role
		await saveSetting('userRole', 'custom');
	}
</script>

{#if tourStore.showModuleDiscovery}
	<div
		class="fixed inset-0 z-[9990] flex items-center justify-center bg-black/50 backdrop-blur-sm"
		role="dialog"
		aria-modal="true"
		aria-label="Discover Modules"
	>
		<div class="w-full max-w-2xl mx-4 rounded-gx-lg bg-gx-bg-secondary border border-gx-border-default shadow-gx-glow-lg overflow-hidden max-h-[80vh] flex flex-col">
			<!-- Header -->
			<div class="flex items-center justify-between px-6 py-4 border-b border-gx-border-default">
				<div class="flex items-center gap-3">
					<Grid3x3 size={20} class="text-gx-neon" />
					<div>
						<h2 class="text-lg font-bold text-gx-text-primary">Discover Modules</h2>
						<p class="text-xs text-gx-text-muted">
							{hiddenModules.length > 0
								? `${hiddenModules.length} modules not in your current profile`
								: 'All modules are active'}
						</p>
					</div>
				</div>
				<button
					onclick={() => tourStore.toggleModuleDiscovery()}
					class="p-2 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors"
					aria-label="Close"
				>
					<X size={18} />
				</button>
			</div>

			<!-- Module Grid -->
			<div class="flex-1 overflow-y-auto p-6">
				{#if hiddenModules.length > 0}
					<h3 class="text-xs font-medium text-gx-text-muted uppercase tracking-wider mb-3">Available to Activate</h3>
					<div class="grid grid-cols-2 gap-3 mb-6">
						{#each hiddenModules as mod (mod.id)}
							<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-4 hover:border-gx-neon/30 transition-colors">
								<div class="flex items-center justify-between mb-2">
									<h4 class="text-sm font-medium text-gx-text-primary">{mod.name}</h4>
									<button
										onclick={() => activateModule(mod.id)}
										class="flex items-center gap-1 px-2 py-1 text-[10px] font-medium rounded bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors"
									>
										<Plus size={10} />
										Activate
									</button>
								</div>
								<p class="text-xs text-gx-text-muted mb-2">{mod.description}</p>
								<div class="flex flex-wrap gap-1">
									{#each mod.features.slice(0, 3) as feature}
										<span class="px-1.5 py-0.5 text-[9px] rounded bg-gx-bg-secondary text-gx-text-disabled">{feature}</span>
									{/each}
								</div>
							</div>
						{/each}
					</div>
				{/if}

				<h3 class="text-xs font-medium text-gx-text-muted uppercase tracking-wider mb-3">Currently Active</h3>
				<div class="grid grid-cols-2 gap-3">
					{#each ALL_MODULES.filter(m => visibleIds.includes(m.id)) as mod (mod.id)}
						<div class="rounded-gx border border-gx-neon/20 bg-gx-neon/5 p-4">
							<div class="flex items-center justify-between mb-2">
								<h4 class="text-sm font-medium text-gx-text-primary">{mod.name}</h4>
								<Check size={14} class="text-gx-neon" />
							</div>
							<p class="text-xs text-gx-text-muted">{mod.description}</p>
						</div>
					{/each}
				</div>
			</div>

			<!-- Footer -->
			<div class="px-6 py-3 border-t border-gx-border-default flex items-center justify-between">
				<span class="text-[10px] text-gx-text-disabled">
					Current profile: <span class="text-gx-neon capitalize">{getSetting('userRole') || 'Custom'}</span>
				</span>
				<button
					onclick={() => tourStore.toggleModuleDiscovery()}
					class="px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon text-gx-bg-primary hover:brightness-110 transition-all"
				>
					Done
				</button>
			</div>
		</div>
	</div>
{/if}
