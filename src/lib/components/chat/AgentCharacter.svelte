<script lang="ts">
	/**
	 * AgentCharacter -- HyperChat agent character with animated state indicators
	 *
	 * Each AI agent gets a visual personality with state-driven animations.
	 * States follow the MoodFlow state machine pattern (arXiv:2601.01027).
	 *
	 * States: idle, thinking, working, success, error, waiting, sleeping
	 * Each state has a distinct icon, color, animation, and optional progress ring.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Outer wrapper
	 *   - ring: Progress/glow ring
	 *   - icon: State icon area
	 */

	import {
		Smile, Brain, Cog, CheckCircle2, AlertTriangle,
		Hourglass, Moon
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface Props {
		widgetId?: string;
		agentName: string;
		state: 'idle' | 'thinking' | 'working' | 'success' | 'error' | 'waiting' | 'sleeping';
		model?: string;
		progress?: number;
		size?: 'sm' | 'md' | 'lg';
	}

	let {
		widgetId = 'agent-character',
		agentName,
		state,
		model,
		progress = 0,
		size = 'md'
	}: Props = $props();

	// --- Size map ---
	const sizeMap = {
		sm: { px: 36, icon: 14, ring: 40, stroke: 2, fontSize: '9px', labelSize: '8px' },
		md: { px: 52, icon: 20, ring: 58, stroke: 2.5, fontSize: '11px', labelSize: '9px' },
		lg: { px: 72, icon: 28, ring: 80, stroke: 3, fontSize: '13px', labelSize: '10px' },
	};
	let dims = $derived(sizeMap[size]);

	// --- State config ---
	interface StateConfig {
		icon: typeof Smile;
		color: string;
		cssVar: string;
		animationClass: string;
		label: string;
		glowIntensity: number;
	}

	const stateConfigs: Record<Props['state'], StateConfig> = {
		idle: {
			icon: Smile,
			color: 'var(--agent-idle)',
			cssVar: '--agent-idle',
			animationClass: 'animate-agent-pulse',
			label: 'Idle',
			glowIntensity: 0.3,
		},
		thinking: {
			icon: Brain,
			color: 'var(--agent-thinking)',
			cssVar: '--agent-thinking',
			animationClass: 'animate-agent-think',
			label: 'Thinking',
			glowIntensity: 0.6,
		},
		working: {
			icon: Cog,
			color: 'var(--agent-working)',
			cssVar: '--agent-working',
			animationClass: 'animate-agent-work-spin',
			label: 'Working',
			glowIntensity: 0.5,
		},
		success: {
			icon: CheckCircle2,
			color: 'var(--agent-success)',
			cssVar: '--agent-success',
			animationClass: 'animate-agent-success-burst',
			label: 'Success',
			glowIntensity: 0.8,
		},
		error: {
			icon: AlertTriangle,
			color: 'var(--agent-error)',
			cssVar: '--agent-error',
			animationClass: 'animate-agent-error-pulse',
			label: 'Error',
			glowIntensity: 0.7,
		},
		waiting: {
			icon: Hourglass,
			color: 'var(--agent-waiting)',
			cssVar: '--agent-waiting',
			animationClass: 'animate-agent-waiting-breathe',
			label: 'Waiting',
			glowIntensity: 0.4,
		},
		sleeping: {
			icon: Moon,
			color: 'var(--agent-sleeping)',
			cssVar: '--agent-sleeping',
			animationClass: 'animate-agent-sleeping-drift',
			label: 'Sleeping',
			glowIntensity: 0.15,
		},
	};

	let config = $derived(stateConfigs[state]);
	let showProgress = $derived(state === 'working' && progress > 0 && progress <= 100);

	// --- SVG progress ring calculations ---
	let ringRadius = $derived((dims.ring - dims.stroke) / 2);
	let ringCircumference = $derived(2 * Math.PI * ringRadius);
	let ringOffset = $derived(ringCircumference - (progress / 100) * ringCircumference);

	// --- Tooltip text ---
	let tooltipText = $derived(() => {
		let text = `${agentName} - ${config.label}`;
		if (model) text += ` (${model})`;
		if (showProgress) text += ` ${Math.round(progress)}%`;
		return text;
	});

	// Style engine integration
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let ringComponent = $derived(styleEngine.getComponentStyle(widgetId, 'ring'));

	let containerStyle = $derived(() => {
		if (hasEngineStyle && containerComponent) {
			return componentToCSS(containerComponent);
		}
		return '';
	});
	let ringEngineStyle = $derived(() => {
		if (hasEngineStyle && ringComponent) {
			return componentToCSS(ringComponent);
		}
		return '';
	});
</script>

<div
	class="relative inline-flex flex-col items-center gap-1 group"
	style={containerStyle()}
	title={tooltipText()}
>
	<!-- Outer glow ring -->
	<div
		class="absolute rounded-full {config.animationClass}"
		style="
			width: {dims.ring}px;
			height: {dims.ring}px;
			top: 50%;
			left: 50%;
			transform: translate(-50%, calc(-50% - {size === 'sm' ? 4 : 6}px));
			box-shadow: 0 0 {12 * config.glowIntensity}px {config.color},
			            0 0 {24 * config.glowIntensity}px color-mix(in srgb, {config.color} 40%, transparent);
			border: 1px solid color-mix(in srgb, {config.color} 25%, transparent);
			{ringEngineStyle()}
		"
	></div>

	<!-- SVG progress ring (working state only) -->
	{#if showProgress}
		<svg
			class="absolute"
			width={dims.ring}
			height={dims.ring}
			style="
				top: 50%;
				left: 50%;
				transform: translate(-50%, calc(-50% - {size === 'sm' ? 4 : 6}px)) rotate(-90deg);
			"
		>
			<!-- Track -->
			<circle
				cx={dims.ring / 2}
				cy={dims.ring / 2}
				r={ringRadius}
				fill="none"
				stroke="rgba(255,255,255,0.06)"
				stroke-width={dims.stroke}
			/>
			<!-- Progress arc -->
			<circle
				cx={dims.ring / 2}
				cy={dims.ring / 2}
				r={ringRadius}
				fill="none"
				stroke={config.color}
				stroke-width={dims.stroke}
				stroke-linecap="round"
				stroke-dasharray={ringCircumference}
				stroke-dashoffset={ringOffset}
				style="transition: stroke-dashoffset 0.3s ease;"
			/>
		</svg>
	{/if}

	<!-- Agent avatar circle -->
	<div
		class="relative rounded-full flex items-center justify-center shrink-0"
		style="
			width: {dims.px}px;
			height: {dims.px}px;
			background: color-mix(in srgb, {config.color} 10%, var(--color-gx-bg-primary));
			border: 1.5px solid color-mix(in srgb, {config.color} {state === 'sleeping' ? '15' : '40'}%, transparent);
			transition: border-color 0.3s, box-shadow 0.3s;
			{state !== 'sleeping' && state !== 'idle' ? `box-shadow: 0 0 8px color-mix(in srgb, ${config.color} 20%, transparent);` : ''}
		"
	>
		<!-- State icon -->
		{#if state === 'working'}
			<div class="animate-agent-work-spin">
				<config.icon size={dims.icon} style="color: {config.color};" />
			</div>
		{:else if state === 'thinking'}
			<div class="animate-agent-think">
				<config.icon size={dims.icon} style="color: {config.color};" />
			</div>
		{:else}
			<config.icon size={dims.icon} style="color: {config.color}; opacity: {state === 'sleeping' ? 0.4 : 1};" />
		{/if}

		<!-- Sleeping ZZZ overlay -->
		{#if state === 'sleeping'}
			<div
				class="absolute animate-agent-sleeping-drift"
				style="
					top: -{Math.round(dims.px * 0.15)}px;
					right: -{Math.round(dims.px * 0.1)}px;
					font-size: {dims.labelSize};
					color: var(--agent-sleeping);
					opacity: 0.6;
					font-weight: 600;
					letter-spacing: 1px;
				"
			>ZZZ</div>
		{/if}

		<!-- Progress percentage (working state, md/lg only) -->
		{#if showProgress && size !== 'sm'}
			<div
				class="absolute tabular-nums"
				style="
					bottom: -{Math.round(dims.px * 0.12)}px;
					font-size: {dims.labelSize};
					color: {config.color};
					font-weight: 600;
				"
			>{Math.round(progress)}%</div>
		{/if}
	</div>

	<!-- Agent name label -->
	<span
		class="text-center truncate max-w-[80px]"
		style="
			font-size: {dims.labelSize};
			color: {state === 'sleeping' ? 'var(--color-gx-text-disabled)' : 'var(--color-gx-text-muted)'};
			font-weight: 500;
		"
	>{agentName}</span>

	<!-- Model name (shown on hover for md/lg) -->
	{#if model && size !== 'sm'}
		<span
			class="opacity-0 group-hover:opacity-100 transition-opacity truncate max-w-[90px]"
			style="
				font-size: {dims.labelSize};
				color: {config.color};
				font-family: var(--font-mono);
			"
		>{model}</span>
	{/if}
</div>
