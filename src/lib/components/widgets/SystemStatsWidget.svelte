<script lang="ts">
	/**
	 * SystemStatsWidget — Live CPU/RAM/GPU monitoring (BenikUI-styled)
	 *
	 * Every sub-component (container, title, cpu-bar, cpu-label, ram-bar, etc.)
	 * is independently styleable via the style engine. Falls back to default
	 * hardcoded styles when no custom styles are loaded.
	 */
	import { Cpu, HardDrive, Monitor, Thermometer } from '@lucide/svelte';
	import { system } from '$lib/stores/system.svelte';
	import { styleEngine, componentToCSS, type ComponentStyle } from '$lib/stores/style-engine.svelte';
	import { StyledContainer, StyledBar, StyledText, StyledSparkline } from '$lib/components/styled';

	const WIDGET_ID = 'system-stats';

	// Load styles on mount
	$effect(() => {
		if (!styleEngine.widgetStyles.has(WIDGET_ID)) {
			styleEngine.loadWidgetStyle(WIDGET_ID);
		}
	});

	// Get individual sub-component styles
	let containerStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'container'));
	let titleStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'title'));
	let cpuBarStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'cpu-bar'));
	let cpuLabelStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'cpu-label'));
	let ramBarStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'ram-bar'));
	let ramLabelStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'ram-label'));
	let gpuBarStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'gpu-bar'));
	let gpuLabelStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'gpu-label'));
	let tempTextStyle = $derived(styleEngine.getComponentStyle(WIDGET_ID, 'temp-text'));

	let hasStyles = $derived(styleEngine.widgetStyles.has(WIDGET_ID));

	// System data
	let cpuPercent = $derived(system.stats?.cpu_percent ?? 0);
	let ramUsed = $derived(system.stats?.ram_used_gb ?? 0);
	let ramTotal = $derived(system.stats?.ram_total_gb ?? 1);
	let ramPercent = $derived(ramTotal > 0 ? (ramUsed / ramTotal) * 100 : 0);
	let vramUsed = $derived((system.stats?.gpu_vram_used_mb ?? 0) / 1024);
	let vramTotal = $derived((system.stats?.gpu_vram_total_mb ?? 1) / 1024);
	let vramPercent = $derived(vramTotal > 0 ? (vramUsed / vramTotal) * 100 : 0);
	let gpuTemp = $derived(system.stats?.gpu_temp_c ?? null);

	// CPU history for sparkline (ring buffer of last 30 readings)
	let cpuHistory = $state<number[]>([]);
	$effect(() => {
		if (cpuPercent > 0) {
			cpuHistory = [...cpuHistory.slice(-29), cpuPercent];
		}
	});
</script>

{#if hasStyles && containerStyle}
	<!-- ═══ BenikUI Styled Mode ═══ -->
	<StyledContainer style={containerStyle} class="h-full flex flex-col overflow-hidden">
		<!-- Title bar -->
		{#if titleStyle}
			<div style={componentToCSS(titleStyle)} class="flex items-center gap-1.5 px-2.5 py-1.5">
				<Cpu size={12} class="text-gx-neon" />
				{#if titleStyle.text}
					<StyledText style={titleStyle.text} label="System" />
				{:else}
					<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">System</span>
				{/if}
				{#if system.stats}
					<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success animate-pulse ml-auto"></span>
				{/if}
			</div>
		{/if}

		<div class="flex-1 p-2.5 space-y-2 overflow-auto">
			<!-- CPU -->
			<div>
				<div class="flex justify-between text-[10px] mb-0.5">
					<span class="text-gx-text-muted flex items-center gap-1"><Cpu size={9} /> CPU</span>
					{#if cpuLabelStyle?.text}
						<StyledText style={cpuLabelStyle.text} value={cpuPercent} max={100} />
					{:else}
						<span class="text-gx-text-secondary font-mono">{cpuPercent.toFixed(0)}%</span>
					{/if}
				</div>
				{#if cpuBarStyle?.bar}
					<StyledBar bar={cpuBarStyle.bar} text={null} value={cpuPercent} max={100} />
				{:else}
					<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
						<div class="h-full bg-gx-neon transition-[width] duration-300" style="width: {cpuPercent}%"></div>
					</div>
				{/if}
				<!-- Sparkline (if enough data) -->
				{#if cpuHistory.length > 5}
					<StyledSparkline data={cpuHistory} color="#00ff66" height={16} class="mt-0.5 w-full opacity-60" />
				{/if}
			</div>

			<!-- RAM -->
			<div>
				<div class="flex justify-between text-[10px] mb-0.5">
					<span class="text-gx-text-muted flex items-center gap-1"><HardDrive size={9} /> RAM</span>
					{#if ramLabelStyle?.text}
						<StyledText style={ramLabelStyle.text} value={ramUsed} max={ramTotal} />
					{:else}
						<span class="text-gx-text-secondary font-mono">{ramUsed.toFixed(1)}/{ramTotal.toFixed(0)}G</span>
					{/if}
				</div>
				{#if ramBarStyle?.bar}
					<StyledBar bar={ramBarStyle.bar} text={null} value={ramUsed} max={ramTotal} />
				{:else}
					<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
						<div class="h-full bg-gx-accent-cyan transition-[width] duration-300" style="width: {ramPercent}%"></div>
					</div>
				{/if}
			</div>

			<!-- GPU VRAM -->
			{#if system.stats?.gpu_vram_used_mb != null}
				<div>
					<div class="flex justify-between text-[10px] mb-0.5">
						<span class="text-gx-text-muted flex items-center gap-1"><Monitor size={9} class="text-gx-accent-magenta" /> VRAM</span>
						{#if gpuLabelStyle?.text}
							<StyledText style={gpuLabelStyle.text} value={vramUsed} max={vramTotal} />
						{:else}
							<span class="text-gx-text-secondary font-mono">{vramUsed.toFixed(1)}/{vramTotal.toFixed(0)}G</span>
						{/if}
					</div>
					{#if gpuBarStyle?.bar}
						<StyledBar bar={gpuBarStyle.bar} text={null} value={vramUsed} max={vramTotal} />
					{:else}
						<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
							<div class="h-full bg-gx-accent-magenta transition-[width] duration-300" style="width: {vramPercent}%"></div>
						</div>
					{/if}
				</div>

				<!-- GPU Temperature -->
				{#if gpuTemp != null}
					<div class="flex items-center justify-between text-[10px]">
						<span class="text-gx-text-muted flex items-center gap-1"><Thermometer size={9} /> Temp</span>
						{#if tempTextStyle?.text}
							<StyledText style={tempTextStyle.text} value={gpuTemp}>
								{gpuTemp.toFixed(0)}°C
							</StyledText>
						{:else}
							<span class="text-gx-text-secondary font-mono">{gpuTemp.toFixed(0)}°C</span>
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	</StyledContainer>
{:else}
	<!-- ═══ Default (no custom styles loaded) ═══ -->
	<div class="h-full flex flex-col bg-gx-bg-secondary border border-gx-border-default rounded-gx overflow-hidden">
		<div class="flex items-center gap-1.5 px-2.5 py-1.5 border-b border-gx-border-default bg-gx-bg-tertiary">
			<Cpu size={12} class="text-gx-neon" />
			<span class="text-[10px] font-semibold text-gx-text-secondary uppercase tracking-wider">System</span>
			{#if system.stats}
				<span class="w-1.5 h-1.5 rounded-full bg-gx-status-success animate-pulse ml-auto"></span>
			{/if}
		</div>
		<div class="flex-1 p-2.5 space-y-2 overflow-auto">
			<div>
				<div class="flex justify-between text-[10px] mb-0.5">
					<span class="text-gx-text-muted flex items-center gap-1"><Cpu size={9} /> CPU</span>
					<span class="text-gx-text-secondary font-mono">{cpuPercent.toFixed(0)}%</span>
				</div>
				<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
					<div class="h-full bg-gx-neon transition-[width] duration-300" style="width: {cpuPercent}%"></div>
				</div>
				{#if cpuHistory.length > 5}
					<StyledSparkline data={cpuHistory} color="#00ff66" height={16} class="mt-0.5 w-full opacity-60" />
				{/if}
			</div>
			<div>
				<div class="flex justify-between text-[10px] mb-0.5">
					<span class="text-gx-text-muted flex items-center gap-1"><HardDrive size={9} /> RAM</span>
					<span class="text-gx-text-secondary font-mono">{ramUsed.toFixed(1)}/{ramTotal.toFixed(0)}G</span>
				</div>
				<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
					<div class="h-full bg-gx-accent-cyan transition-[width] duration-300" style="width: {ramPercent}%"></div>
				</div>
			</div>
			{#if system.stats?.gpu_vram_used_mb != null}
				<div>
					<div class="flex justify-between text-[10px] mb-0.5">
						<span class="text-gx-text-muted flex items-center gap-1"><Monitor size={9} class="text-gx-accent-magenta" /> VRAM</span>
						<span class="text-gx-text-secondary font-mono">{vramUsed.toFixed(1)}/{vramTotal.toFixed(0)}G</span>
					</div>
					<div class="h-1 bg-gx-border-default rounded-full overflow-hidden">
						<div class="h-full bg-gx-accent-magenta transition-[width] duration-300" style="width: {vramPercent}%"></div>
					</div>
				</div>
				{#if gpuTemp != null}
					<div class="flex items-center justify-between text-[10px]">
						<span class="text-gx-text-muted flex items-center gap-1"><Thermometer size={9} /> Temp</span>
						<span class="text-gx-text-secondary font-mono">{gpuTemp.toFixed(0)}°C</span>
					</div>
				{/if}
			{/if}
		</div>
	</div>
{/if}
