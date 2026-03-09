<script lang="ts">
	import { errorStore, type ErrorEntry } from '$lib/stores/errors.svelte';
	import { X, AlertTriangle, Wifi, FileWarning, Brain, Globe, Settings, Bug } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine integration
	const widgetId = 'error-toast';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	const categoryIcons: Record<string, typeof AlertTriangle> = {
		service: Wifi,
		validation: AlertTriangle,
		file_system: FileWarning,
		model: Brain,
		browser: Globe,
		config: Settings,
		internal: Bug,
	};

	const categoryColors: Record<string, string> = {
		service: 'border-gx-status-warning/50 bg-gx-status-warning/5',
		validation: 'border-gx-accent-amber/50 bg-gx-accent-amber/5',
		file_system: 'border-gx-status-error/50 bg-gx-status-error/5',
		model: 'border-gx-accent-magenta/50 bg-gx-accent-magenta/5',
		browser: 'border-gx-accent-blue/50 bg-gx-accent-blue/5',
		config: 'border-gx-accent-purple/50 bg-gx-accent-purple/5',
		internal: 'border-gx-status-error/50 bg-gx-status-error/5',
	};

	function getIcon(category: string) {
		return categoryIcons[category] ?? AlertTriangle;
	}

	function getColor(category: string) {
		return categoryColors[category] ?? 'border-gx-status-error/50 bg-gx-status-error/5';
	}
</script>

<!-- Error toast container — fixed bottom-right -->
{#if errorStore.hasErrors}
	<div
		class="fixed bottom-10 right-4 z-50 flex flex-col gap-2 max-w-sm pointer-events-none"
		aria-live="polite"
		aria-label="Error notifications"
		style={containerStyle}
	>
		{#each errorStore.errors as entry (entry.id)}
			{@const Icon = getIcon(entry.error.category)}
			<div
				class="pointer-events-auto rounded-gx border p-3 shadow-gx-glow-sm animate-in slide-in-from-right-full duration-300 {getColor(entry.error.category)}"
				role="alert"
			>
				<div class="flex items-start gap-2">
					<Icon size={16} class="shrink-0 mt-0.5 text-gx-text-secondary" />
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-2">
							<span class="text-xs font-mono text-gx-text-muted">{entry.error.code}</span>
						</div>
						<p class="text-sm text-gx-text-primary mt-0.5 leading-snug">
							{entry.error.message}
						</p>
						{#if entry.error.suggestion}
							<p class="text-xs text-gx-text-muted mt-1">
								{entry.error.suggestion}
							</p>
						{/if}
					</div>
					<button
						onclick={() => errorStore.dismiss(entry.id)}
						class="shrink-0 text-gx-text-muted hover:text-gx-text-primary transition-colors"
						aria-label="Dismiss error"
					>
						<X size={14} />
					</button>
				</div>
			</div>
		{/each}
	</div>
{/if}
