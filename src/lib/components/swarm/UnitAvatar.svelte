<script lang="ts">
	/**
	 * UnitAvatar -- Dynamic SVG unit portrait for SwarmForge
	 *
	 * Renders an SVG avatar that visually reflects the unit's type, level,
	 * applied mutations, and current behavioral state.  Every mutation adds
	 * a distinct visual layer (spikes, shields, antennae, crowns, etc.) so
	 * the player can read a unit's build at a glance.
	 *
	 * Design constraints:
	 *   - Pure SVG (viewBox-based, fully responsive)
	 *   - Uses existing sf-* animation classes from app.css
	 *   - prefers-reduced-motion respected (CSS handles it)
	 *   - Svelte 5 runes only
	 */

	interface Mutation {
		mutation_type: string;
		name: string;
	}

	interface Props {
		unitType: string;
		level: number;
		mutations: Mutation[];
		state?: 'idle' | 'fighting' | 'gathering' | 'evolving';
		size?: 'sm' | 'md' | 'lg' | 'xl';
		showLabel?: boolean;
	}

	let {
		unitType,
		level,
		mutations,
		state = 'idle',
		size = 'md',
		showLabel = true
	}: Props = $props();

	// ── Size map ───────────────────────────────────────────────────────────

	const sizeMap: Record<string, number> = {
		sm: 32,
		md: 48,
		lg: 72,
		xl: 120
	};

	let pxSize = $derived(sizeMap[size] ?? 48);

	// ── Level scaling: 0.8x at Lv.1, 2.0x at Lv.100 ──────────────────────

	let levelScale = $derived(0.8 + (Math.min(level, 100) / 100) * 1.2);

	// ── Mutation counts by category ───────────────────────────────────────

	let defCount = $derived(mutations.filter(m => m.mutation_type === 'defensive').length);
	let offCount = $derived(mutations.filter(m => m.mutation_type === 'offensive').length);
	let utilCount = $derived(mutations.filter(m => m.mutation_type === 'utility').length);
	let evoCount = $derived(mutations.filter(m => m.mutation_type === 'evolution').length);
	let specCount = $derived(mutations.filter(m => m.mutation_type === 'specialization').length);

	// ── Dynamic body geometry ─────────────────────────────────────────────

	let bodyWidth = $derived(12 + defCount * 1.5);
	let bodyHeight = $derived(14 + Math.min(level, 100) * 0.06);
	let borderWidth = $derived(1.5 + defCount * 0.6);
	let eyeSize = $derived(2.2 + utilCount * 0.4);

	// ── Color system per unit type ────────────────────────────────────────

	const BASE_COLORS: Record<string, string> = {
		forge_drone: '#22c55e',
		imp_scout: '#eab308',
		viper: '#8b5cf6',
		shadow_weaver: '#6b7280',
		skyweaver: '#06b6d4',
		overseer: '#f59e0b',
		titan: '#dc2626',
		swarm_mother: '#ec4899',
		ravager: '#991b1b',
		matriarch: '#7c3aed',
		spore_crawler: '#84cc16',
		infestor: '#a3e635',
		nydus_worm: '#78716c',
		gargoyle: '#475569',
		carnifex: '#b91c1c',
		ripper_swarm: '#65a30d',
		haruspex: '#059669',
		hive_guard: '#0369a1',
		dominatrix: '#9333ea',
		broodling: '#fbbf24'
	};

	const BORDER_COLORS: Record<string, string> = {
		forge_drone: '#16a34a',
		imp_scout: '#ca8a04',
		viper: '#7c3aed',
		shadow_weaver: '#4b5563',
		skyweaver: '#0891b2',
		overseer: '#d97706',
		titan: '#b91c1c',
		swarm_mother: '#db2777',
		ravager: '#7f1d1d',
		matriarch: '#6d28d9',
		spore_crawler: '#65a30d',
		infestor: '#84cc16',
		nydus_worm: '#57534e',
		gargoyle: '#334155',
		carnifex: '#991b1b',
		ripper_swarm: '#4d7c0f',
		haruspex: '#047857',
		hive_guard: '#075985',
		dominatrix: '#7e22ce',
		broodling: '#f59e0b'
	};

	let bodyColor = $derived(BASE_COLORS[unitType] ?? '#22c55e');
	let borderColor = $derived(BORDER_COLORS[unitType] ?? '#16a34a');
	let eyeColor = $derived(utilCount > 0 ? '#3b82f6' : '#ffffff');

	// ── Per-type shape config ─────────────────────────────────────────────
	// Each type gets a unique silhouette composed from SVG primitives.
	// shape: 'oval' | 'diamond' | 'elongated' | 'ghost' | 'winged' |
	//        'eye' | 'armored' | 'crowned' | 'x' | 'double'

	interface TypeShape {
		shape: string;
		bodyRx: number;
		bodyRy: number;
		legs: Array<{ x1: number; y1: number; x2: number; y2: number }>;
		extras: string; // Additional SVG path data
	}

	const TYPE_SHAPES: Record<string, TypeShape> = {
		forge_drone: {
			shape: 'oval', bodyRx: 11, bodyRy: 14,
			legs: [
				{ x1: 22, y1: 38, x2: 18, y2: 48 },
				{ x1: 42, y1: 38, x2: 46, y2: 48 },
			],
			extras: 'M 28 18 Q 26 10, 22 12 M 36 18 Q 38 10, 42 12' // antennae
		},
		imp_scout: {
			shape: 'diamond', bodyRx: 10, bodyRy: 13,
			legs: [
				{ x1: 20, y1: 32, x2: 12, y2: 32 },
				{ x1: 44, y1: 32, x2: 52, y2: 32 },
			],
			extras: 'M 12 32 L 8 30 M 52 32 L 56 30' // speed lines
		},
		viper: {
			shape: 'elongated', bodyRx: 14, bodyRy: 10,
			legs: [
				{ x1: 20, y1: 36, x2: 16, y2: 44 },
				{ x1: 32, y1: 38, x2: 32, y2: 46 },
				{ x1: 44, y1: 36, x2: 48, y2: 44 },
			],
			extras: 'M 18 30 L 14 26 M 46 30 L 50 26' // segment marks
		},
		shadow_weaver: {
			shape: 'ghost', bodyRx: 12, bodyRy: 14,
			legs: [
				{ x1: 24, y1: 44, x2: 22, y2: 50 },
				{ x1: 32, y1: 46, x2: 32, y2: 52 },
				{ x1: 40, y1: 44, x2: 42, y2: 50 },
			],
			extras: '' // ghostly = no hard features
		},
		skyweaver: {
			shape: 'winged', bodyRx: 10, bodyRy: 12,
			legs: [],
			extras: 'M 22 28 Q 10 20, 8 30 M 42 28 Q 54 20, 56 30' // wings
		},
		overseer: {
			shape: 'eye', bodyRx: 13, bodyRy: 13,
			legs: [],
			extras: 'M 22 20 Q 32 12, 42 20 M 22 44 Q 32 52, 42 44' // eye ridges
		},
		titan: {
			shape: 'armored', bodyRx: 14, bodyRy: 16,
			legs: [
				{ x1: 20, y1: 42, x2: 16, y2: 52 },
				{ x1: 44, y1: 42, x2: 48, y2: 52 },
			],
			extras: 'M 20 22 L 20 42 M 44 22 L 44 42' // armor plates
		},
		swarm_mother: {
			shape: 'crowned', bodyRx: 13, bodyRy: 15,
			legs: [
				{ x1: 24, y1: 44, x2: 22, y2: 52 },
				{ x1: 40, y1: 44, x2: 42, y2: 52 },
			],
			extras: '' // crown handled separately
		},
		ravager: {
			shape: 'x', bodyRx: 12, bodyRy: 12,
			legs: [
				{ x1: 20, y1: 40, x2: 14, y2: 48 },
				{ x1: 44, y1: 40, x2: 50, y2: 48 },
			],
			extras: 'M 22 22 L 42 42 M 42 22 L 22 42' // X slash
		},
		matriarch: {
			shape: 'double', bodyRx: 14, bodyRy: 16,
			legs: [
				{ x1: 20, y1: 44, x2: 16, y2: 54 },
				{ x1: 44, y1: 44, x2: 48, y2: 54 },
			],
			extras: '' // double crown handled separately
		},
		spore_crawler: {
			shape: 'oval', bodyRx: 12, bodyRy: 11,
			legs: [
				{ x1: 20, y1: 38, x2: 14, y2: 46 },
				{ x1: 32, y1: 40, x2: 32, y2: 48 },
				{ x1: 44, y1: 38, x2: 50, y2: 46 },
			],
			extras: 'M 32 20 Q 32 12, 28 14 M 32 20 Q 32 12, 36 14' // spore tendrils
		},
		infestor: {
			shape: 'ghost', bodyRx: 13, bodyRy: 12,
			legs: [
				{ x1: 24, y1: 42, x2: 20, y2: 50 },
				{ x1: 40, y1: 42, x2: 44, y2: 50 },
			],
			extras: 'M 26 26 L 24 22 M 38 26 L 40 22' // nerve tendrils
		},
		nydus_worm: {
			shape: 'elongated', bodyRx: 10, bodyRy: 18,
			legs: [],
			extras: 'M 26 16 Q 32 10, 38 16' // maw arch
		},
		gargoyle: {
			shape: 'winged', bodyRx: 10, bodyRy: 11,
			legs: [
				{ x1: 28, y1: 40, x2: 26, y2: 48 },
				{ x1: 36, y1: 40, x2: 38, y2: 48 },
			],
			extras: 'M 20 26 Q 8 18, 6 28 M 44 26 Q 56 18, 58 28' // bat wings
		},
		carnifex: {
			shape: 'armored', bodyRx: 15, bodyRy: 16,
			legs: [
				{ x1: 18, y1: 44, x2: 12, y2: 54 },
				{ x1: 46, y1: 44, x2: 52, y2: 54 },
			],
			extras: 'M 18 24 L 10 18 M 46 24 L 54 18' // siege arms
		},
		ripper_swarm: {
			shape: 'oval', bodyRx: 9, bodyRy: 9,
			legs: [
				{ x1: 24, y1: 36, x2: 20, y2: 42 },
				{ x1: 40, y1: 36, x2: 44, y2: 42 },
				{ x1: 28, y1: 38, x2: 26, y2: 44 },
				{ x1: 36, y1: 38, x2: 38, y2: 44 },
			],
			extras: '' // small swarming body
		},
		haruspex: {
			shape: 'elongated', bodyRx: 13, bodyRy: 14,
			legs: [
				{ x1: 22, y1: 42, x2: 18, y2: 50 },
				{ x1: 42, y1: 42, x2: 46, y2: 50 },
			],
			extras: 'M 26 18 L 24 12 M 38 18 L 40 12' // feeding mandibles
		},
		hive_guard: {
			shape: 'armored', bodyRx: 14, bodyRy: 14,
			legs: [
				{ x1: 20, y1: 42, x2: 16, y2: 50 },
				{ x1: 44, y1: 42, x2: 48, y2: 50 },
			],
			extras: 'M 32 18 L 32 8 M 30 10 L 34 10' // bio-cannon barrel
		},
		dominatrix: {
			shape: 'double', bodyRx: 14, bodyRy: 15,
			legs: [
				{ x1: 22, y1: 44, x2: 18, y2: 52 },
				{ x1: 42, y1: 44, x2: 46, y2: 52 },
			],
			extras: '' // aura handled via specialization layer
		},
		broodling: {
			shape: 'oval', bodyRx: 8, bodyRy: 8,
			legs: [
				{ x1: 26, y1: 36, x2: 24, y2: 42 },
				{ x1: 38, y1: 36, x2: 40, y2: 42 },
			],
			extras: '' // tiny, ephemeral
		}
	};

	let typeShape = $derived(TYPE_SHAPES[unitType] ?? TYPE_SHAPES.forge_drone);

	// ── Spike positions (offensive mutations) ─────────────────────────────
	// Radiate outward from the body center, evenly spaced around the unit.

	interface Spike {
		x1: number; y1: number;
		x2: number; y2: number;
	}

	let spikes = $derived.by((): Spike[] => {
		const result: Spike[] = [];
		const cx = 32;
		const cy = 32;
		const innerR = 16;
		const outerR = 22 + offCount * 2;
		const count = Math.min(offCount * 2, 12);
		for (let i = 0; i < count; i++) {
			const angle = (i / count) * Math.PI * 2 - Math.PI / 2;
			result.push({
				x1: cx + Math.cos(angle) * innerR,
				y1: cy + Math.sin(angle) * innerR,
				x2: cx + Math.cos(angle) * outerR,
				y2: cy + Math.sin(angle) * outerR
			});
		}
		return result;
	});

	// ── Shield plates (defensive mutations) ───────────────────────────────
	// Armor rectangles positioned symmetrically around the body.

	interface ShieldPlate {
		x: number; y: number;
		w: number; h: number;
	}

	let shieldPlates = $derived.by((): ShieldPlate[] => {
		const plates: ShieldPlate[] = [];
		if (defCount >= 1) plates.push({ x: 8, y: 24, w: 6, h: 16 });
		if (defCount >= 1) plates.push({ x: 50, y: 24, w: 6, h: 16 });
		if (defCount >= 2) plates.push({ x: 20, y: 8, w: 24, h: 5 });
		if (defCount >= 3) plates.push({ x: 20, y: 50, w: 24, h: 5 });
		if (defCount >= 4) plates.push({ x: 12, y: 16, w: 5, h: 10 });
		if (defCount >= 4) plates.push({ x: 47, y: 16, w: 5, h: 10 });
		return plates;
	});

	// ── Utility antennae (utility mutations) ──────────────────────────────

	interface Antenna {
		d: string;
	}

	let antennae = $derived.by((): Antenna[] => {
		const result: Antenna[] = [];
		if (utilCount >= 1) result.push({ d: 'M 20 18 Q 14 6, 8 10' });
		if (utilCount >= 1) result.push({ d: 'M 44 18 Q 50 6, 56 10' });
		if (utilCount >= 2) result.push({ d: 'M 24 16 Q 22 4, 16 6' });
		if (utilCount >= 2) result.push({ d: 'M 40 16 Q 42 4, 48 6' });
		if (utilCount >= 3) result.push({ d: 'M 32 14 Q 32 2, 28 4' });
		if (utilCount >= 3) result.push({ d: 'M 32 14 Q 32 2, 36 4' });
		return result;
	});

	// ── Body opacity for ghost types ──────────────────────────────────────

	let bodyOpacity = $derived(
		typeShape.shape === 'ghost' ? 0.55 : 0.9
	);

	// ── Glow filter ID (unique per mutation combo) ────────────────────────

	let glowFilterId = $derived(`glow-${unitType}-${level}`);

	// ── State animation class ─────────────────────────────────────────────

	let stateClass = $derived(
		state === 'fighting' ? 'sf-animate-hit'
		: state === 'gathering' ? 'sf-animate-pulse'
		: state === 'evolving' ? 'sf-mutation-evo'
		: ''
	);

	// ── Crown points for SwarmMother/Matriarch/Dominatrix ─────────────────

	let hasCrown = $derived(
		unitType === 'swarm_mother' || unitType === 'matriarch' || unitType === 'dominatrix'
	);

	let hasDoubleCrown = $derived(
		unitType === 'matriarch' || unitType === 'dominatrix'
	);
</script>

<div
	class="sf-unit-avatar inline-flex flex-col items-center {stateClass}"
	style="width: {pxSize}px;"
	role="img"
	aria-label="{unitType.replace(/_/g, ' ')} level {level}{mutations.length > 0 ? `, ${mutations.length} mutation${mutations.length !== 1 ? 's' : ''}` : ''}"
>
	<svg
		viewBox="0 0 64 64"
		width={pxSize}
		height={pxSize}
		style="transform: scale({levelScale}); transform-origin: center;"
		xmlns="http://www.w3.org/2000/svg"
	>
		<defs>
			<!-- Glow filter for mutation auras -->
			<filter id={glowFilterId} x="-40%" y="-40%" width="180%" height="180%">
				<feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
				<feMerge>
					<feMergeNode in="blur" />
					<feMergeNode in="SourceGraphic" />
				</feMerge>
			</filter>

			<!-- Defensive green glow -->
			{#if defCount > 0}
				<filter id="{glowFilterId}-def" x="-50%" y="-50%" width="200%" height="200%">
					<feGaussianBlur in="SourceGraphic" stdDeviation={1.5 + defCount * 0.5} result="blur" />
					<feFlood flood-color="#22c55e" flood-opacity={0.15 + defCount * 0.08} result="color" />
					<feComposite in="color" in2="blur" operator="in" result="glow" />
					<feMerge>
						<feMergeNode in="glow" />
						<feMergeNode in="SourceGraphic" />
					</feMerge>
				</filter>
			{/if}

			<!-- Offensive red glow -->
			{#if offCount > 0}
				<filter id="{glowFilterId}-off" x="-50%" y="-50%" width="200%" height="200%">
					<feGaussianBlur in="SourceGraphic" stdDeviation={1 + offCount * 0.4} result="blur" />
					<feFlood flood-color="#ef4444" flood-opacity={0.12 + offCount * 0.06} result="color" />
					<feComposite in="color" in2="blur" operator="in" result="glow" />
					<feMerge>
						<feMergeNode in="glow" />
						<feMergeNode in="SourceGraphic" />
					</feMerge>
				</filter>
			{/if}
		</defs>

		<!-- ═══ LAYER 0: Specialization aura (purple particle ring) ═══ -->
		{#if specCount > 0}
			<circle
				cx="32" cy="32" r={26 + specCount}
				fill="none"
				stroke="#a855f7"
				stroke-width="1"
				stroke-dasharray="3 4"
				opacity={0.3 + specCount * 0.1}
				class="sf-mutation-spec"
			/>
			{#if specCount >= 2}
				<circle
					cx="32" cy="32" r={30 + specCount}
					fill="none"
					stroke="#c084fc"
					stroke-width="0.5"
					stroke-dasharray="2 6"
					opacity="0.25"
					class="sf-mutation-spec"
				/>
			{/if}
		{/if}

		<!-- ═══ LAYER 1: Defensive green glow ring ═══ -->
		{#if defCount > 0}
			<circle
				cx="32" cy="32" r={18 + defCount}
				fill="none"
				stroke="#22c55e"
				stroke-width={0.8 + defCount * 0.3}
				opacity={0.2 + defCount * 0.08}
				class="sf-mutation-def"
			/>
		{/if}

		<!-- ═══ LAYER 2: Offensive energy glow ═══ -->
		{#if offCount > 0}
			<circle
				cx="32" cy="32" r={17 + offCount * 0.5}
				fill="none"
				stroke="#ef4444"
				stroke-width={0.6 + offCount * 0.2}
				opacity={0.15 + offCount * 0.06}
				class="sf-mutation-off"
			/>
		{/if}

		<!-- ═══ LAYER 3: Shield / armor plates (defensive) ═══ -->
		{#each shieldPlates as plate}
			<rect
				x={plate.x} y={plate.y}
				width={plate.w} height={plate.h}
				rx="1.5"
				fill="rgba(34,197,94,0.2)"
				stroke="#22c55e"
				stroke-width="0.6"
				opacity="0.7"
				class="sf-mutation-def"
			/>
		{/each}

		<!-- ═══ LAYER 4: Base body ═══ -->
		<ellipse
			cx="32" cy="32"
			rx={typeShape.bodyRx + bodyWidth - 12}
			ry={typeShape.bodyRy + bodyHeight - 14}
			fill={bodyColor}
			fill-opacity={bodyOpacity}
			stroke={borderColor}
			stroke-width={borderWidth}
			filter={defCount > 0 ? `url(#${glowFilterId}-def)` : undefined}
		/>

		<!-- Inner body highlight (depth illusion) -->
		<ellipse
			cx="31" cy="30"
			rx={(typeShape.bodyRx + bodyWidth - 12) * 0.6}
			ry={(typeShape.bodyRy + bodyHeight - 14) * 0.5}
			fill="white"
			opacity="0.07"
		/>

		<!-- ═══ LAYER 5: Type-specific appendages (legs, wings, etc.) ═══ -->
		{#each typeShape.legs as leg}
			<line
				x1={leg.x1} y1={leg.y1}
				x2={leg.x2} y2={leg.y2}
				stroke={borderColor}
				stroke-width="2"
				stroke-linecap="round"
			/>
		{/each}

		<!-- Type-specific extra paths (antennae, wings, segments) -->
		{#if typeShape.extras}
			<path
				d={typeShape.extras}
				stroke={borderColor}
				stroke-width="1.5"
				fill="none"
				stroke-linecap="round"
			/>
		{/if}

		<!-- ═══ LAYER 6: Eyes ═══ -->
		{#if typeShape.shape === 'eye'}
			<!-- Overseer: single large eye -->
			<ellipse cx="32" cy="32" rx={eyeSize + 2} ry={eyeSize + 1}
				fill="#0f172a" stroke={eyeColor} stroke-width="0.8" />
			<circle cx="32" cy="32" r={eyeSize * 0.6}
				fill={eyeColor} class="sf-animate-pulse" />
			<circle cx="33" cy="31" r={eyeSize * 0.2} fill="white" opacity="0.8" />
		{:else}
			<!-- Standard dual eyes -->
			<circle cx={28} cy={28} r={eyeSize} fill={eyeColor} />
			<circle cx={36} cy={28} r={eyeSize} fill={eyeColor} />
			<!-- Pupil highlights -->
			<circle cx={28.5} cy={27.5} r={eyeSize * 0.3} fill="white" opacity="0.7" />
			<circle cx={36.5} cy={27.5} r={eyeSize * 0.3} fill="white" opacity="0.7" />
		{/if}

		<!-- ═══ LAYER 7: Offensive spikes ═══ -->
		{#each spikes as spike}
			<line
				x1={spike.x1} y1={spike.y1}
				x2={spike.x2} y2={spike.y2}
				stroke="#ef4444"
				stroke-width={1.2 + offCount * 0.2}
				stroke-linecap="round"
				opacity={0.7 + offCount * 0.05}
				filter={offCount >= 2 ? `url(#${glowFilterId}-off)` : undefined}
			/>
		{/each}

		<!-- ═══ LAYER 8: Utility antennae / sensors ═══ -->
		{#each antennae as ant}
			<path
				d={ant.d}
				stroke="#3b82f6"
				stroke-width="1.5"
				fill="none"
				stroke-linecap="round"
				opacity="0.85"
			/>
		{/each}
		<!-- Utility circuit patterns on body -->
		{#if utilCount >= 2}
			<path
				d="M 26 34 Q 32 30, 38 34"
				stroke="#3b82f6"
				stroke-width="0.6"
				fill="none"
				opacity="0.4"
				stroke-dasharray="2 2"
			/>
		{/if}
		{#if utilCount >= 3}
			<path
				d="M 24 38 Q 32 34, 40 38"
				stroke="#60a5fa"
				stroke-width="0.5"
				fill="none"
				opacity="0.3"
				stroke-dasharray="1.5 2.5"
			/>
		{/if}

		<!-- ═══ LAYER 9: Crown (SwarmMother / Matriarch / Dominatrix) ═══ -->
		{#if hasCrown}
			<polygon
				points="24,14 28,6 32,12 36,6 40,14"
				fill={hasDoubleCrown ? '#7c3aed' : '#ec4899'}
				stroke={hasDoubleCrown ? '#a855f7' : '#f472b6'}
				stroke-width="0.8"
				opacity="0.85"
			/>
		{/if}
		{#if hasDoubleCrown}
			<!-- Second crown layer (offset up) -->
			<polygon
				points="26,10 29,3 32,8 35,3 38,10"
				fill="#a855f7"
				stroke="#c084fc"
				stroke-width="0.6"
				opacity="0.65"
			/>
		{/if}

		<!-- ═══ LAYER 10: Specialization crown / halo ═══ -->
		{#if specCount > 0 && !hasCrown}
			<polygon
				points="26,14 29,7 32,12 35,7 38,14"
				fill="#a855f7"
				stroke="#c084fc"
				stroke-width="0.7"
				opacity={0.5 + specCount * 0.15}
			/>
		{/if}
		{#if specCount >= 2}
			<!-- Elite border pattern -->
			<rect
				x="14" y="14" width="36" height="36" rx="6"
				fill="none"
				stroke="#a855f7"
				stroke-width="0.6"
				stroke-dasharray="4 2"
				opacity="0.35"
			/>
		{/if}

		<!-- ═══ LAYER 11: Evolution shimmer ═══ -->
		{#if evoCount > 0}
			<ellipse
				cx="32" cy="32"
				rx={typeShape.bodyRx + bodyWidth - 10}
				ry={typeShape.bodyRy + bodyHeight - 12}
				fill="none"
				stroke="#eab308"
				stroke-width={0.6 + evoCount * 0.3}
				stroke-dasharray="3 3"
				opacity={0.3 + evoCount * 0.1}
				class="sf-mutation-evo"
			/>
		{/if}

		<!-- ═══ LAYER 12: State overlays ═══ -->
		{#if state === 'fighting'}
			<circle
				cx="32" cy="32" r="28"
				fill="none"
				stroke="#ef4444"
				stroke-width="1.2"
				opacity="0.4"
				class="sf-animate-hit"
			/>
		{/if}
		{#if state === 'evolving'}
			<circle
				cx="32" cy="32" r="26"
				fill="none"
				stroke="#eab308"
				stroke-width="2"
				class="sf-mutation-evo"
			/>
			<!-- Radiant particles during evolution -->
			<circle cx="32" cy="8" r="1.5" fill="#fbbf24" opacity="0.7" class="sf-animate-bubble" />
			<circle cx="52" cy="24" r="1.2" fill="#fde68a" opacity="0.5" class="sf-animate-bubble" />
			<circle cx="12" cy="28" r="1" fill="#fbbf24" opacity="0.6" class="sf-animate-bubble" />
		{/if}
		{#if state === 'gathering'}
			<!-- Pulsing gather ring -->
			<circle
				cx="32" cy="32" r="24"
				fill="none"
				stroke="#22c55e"
				stroke-width="0.8"
				opacity="0.3"
				class="sf-animate-pulse"
			/>
		{/if}

		<!-- ═══ LAYER 13: Level indicator ═══ -->
		{#if showLabel}
			<text
				x="32" y="58"
				text-anchor="middle"
				font-size="7"
				font-family="var(--font-mono)"
				fill="white"
				opacity="0.6"
			>
				Lv.{level}
			</text>
		{/if}
	</svg>
</div>
