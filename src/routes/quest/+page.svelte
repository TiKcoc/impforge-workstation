<script lang="ts">
	/**
	 * SwarmForge -- OGame-style Colony Builder + Idle RPG
	 *
	 * OGame-inspired layout with resource bar, sidebar navigation,
	 * and center content panels for buildings, research, fleet, galaxy, etc.
	 */

	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import {
		Sword, Shield, Wand2, Hammer, Music, BookOpen,
		Heart, Zap, Star, Package, Map, ScrollText,
		Flame, Trophy, ChevronRight, Swords, Sparkles,
		ArrowUp, ShieldCheck, Brain, Wrench, Crown, GraduationCap,
		Bug, Building, Target, Egg, Timer, Play, Check, Gem, Droplets, Atom,
		Factory, Rocket, Globe, ShoppingBag, BarChart3, Home, FlaskConical,
		Layers, Crosshair, Warehouse, CircleDot, Dna
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	const widgetId = 'quest-page';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});

	// ── Types ──────────────────────────────────────────────────────────────

	interface PlanetResources {
		biomass: number;
		minerals: number;
		crystal: number;
		spore_gas: number;
		energy: number;
		dark_matter: number;
		biomass_per_hour: number;
		minerals_per_hour: number;
		crystal_per_hour: number;
		spore_gas_per_hour: number;
		energy_production: number;
		energy_consumption: number;
	}

	interface PlanetBuilding {
		building_type: string;
		level: number;
		upgrading: boolean;
		upgrade_finish: string | null;
		display_name: string;
		description: string;
		cost_biomass: number;
		cost_minerals: number;
		cost_crystal: number;
		cost_spore_gas: number;
		build_time_seconds: number;
	}

	interface Research {
		tech_type: string;
		level: number;
		researching: boolean;
		research_finish: string | null;
		display_name: string;
		description: string;
		cost_biomass: number;
		cost_minerals: number;
		cost_crystal: number;
		cost_spore_gas: number;
		research_time_seconds: number;
		required_lab_level: number;
	}

	interface Ship {
		ship_type: string;
		count: number;
		display_name: string;
		description: string;
		attack: number;
		shields: number;
		hp: number;
	}

	interface CreepStatus {
		coverage_percent: number;
		spread_rate_per_hour: number;
		flora_corrupted: number;
		fauna_consumed: number;
		biomass_bonus: number;
	}

	interface ShopItem {
		id: string;
		name: string;
		description: string;
		cost_dark_matter: number;
		effect: Record<string, unknown>;
		duration_hours: number | null;
	}

	interface Planet {
		name: string;
		resources: PlanetResources;
		buildings: PlanetBuilding[];
		research: Research[];
		fleet: Ship[];
		creep: CreepStatus;
		storage_biomass_cap: number;
		storage_minerals_cap: number;
		storage_crystal_cap: number;
		storage_spore_gas_cap: number;
	}

	interface PlanetSlot {
		position: number;
		occupied: boolean;
		planet_name: string | null;
		player_name: string | null;
		planet_type: string | null;
	}

	interface CompletedTimer {
		timer_type: string;
		item_name: string;
		completed_at: string;
	}

	interface MutationStats {
		hp_bonus: number;
		attack_bonus: number;
		defense_bonus: number;
		speed_bonus: number;
		production_bonus: number;
	}

	interface MutationChoice {
		id: string;
		name: string;
		description: string;
		mutation_type: string;
		stat_changes: MutationStats;
		special_ability: string | null;
		level_required: number;
		unit_type: string;
	}

	interface AppliedMutation {
		mutation_id: string;
		applied_at_level: number;
	}

	interface UnitMutations {
		unit_id: string;
		unit_type: string;
		unit_level: number;
		applied_mutations: AppliedMutation[];
		pending_choices: MutationChoice[];
	}

	// ── State ──────────────────────────────────────────────────────────────

	let planet = $state<Planet | null>(null);
	let galaxySlots = $state<PlanetSlot[]>([]);
	let shopItems = $state<ShopItem[]>([]);
	let completedTimers = $state<CompletedTimer[]>([]);
	let loading = $state(true);
	let error = $state('');
	let statusMsg = $state('');
	let shipBuildCounts = $state<Record<string, number>>({});
	let galaxyNum = $state(1);
	let systemNum = $state(1);

	type NavSection = 'overview' | 'buildings' | 'research' | 'fleet' | 'defense' | 'galaxy' | 'creep' | 'shop' | 'mutations' | 'stats';
	let activeNav = $state<NavSection>('overview');

	// Mutation system state
	let mutationUnits = $state<{ id: string; unit_type: string; name: string; level: number }[]>([]);
	let selectedMutationUnit = $state<string | null>(null);
	let unitMutations = $state<UnitMutations | null>(null);
	let mutationTree = $state<MutationChoice[][]>([]);
	let mutationCatalog = $state<Record<string, MutationChoice>>({});
	let mutationStatus = $state('');

	// ── Derived ────────────────────────────────────────────────────────────

	let res = $derived(planet?.resources ?? {
		biomass: 0, minerals: 0, crystal: 0, spore_gas: 0, energy: 0, dark_matter: 0,
		biomass_per_hour: 0, minerals_per_hour: 0, crystal_per_hour: 0, spore_gas_per_hour: 0,
		energy_production: 0, energy_consumption: 0
	});

	let energyClass = $derived(res.energy < 0 ? 'text-red-400' : 'text-yellow-300');

	// ── Helpers ────────────────────────────────────────────────────────────

	function fmt(n: number): string {
		if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
		if (n >= 10_000) return (n / 1_000).toFixed(1) + 'K';
		return Math.floor(n).toLocaleString();
	}

	function fmtTime(seconds: number): string {
		if (seconds <= 0) return '00:00';
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.floor(seconds % 60);
		if (h > 0) return `${h}h ${m.toString().padStart(2, '0')}m ${s.toString().padStart(2, '0')}s`;
		return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
	}

	function timerRemaining(finishIso: string | null): string {
		if (!finishIso) return '';
		const finish = new Date(finishIso).getTime();
		const now = Date.now();
		const diff = Math.max(0, (finish - now) / 1000);
		return fmtTime(diff);
	}

	function canAfford(bio: number, min: number, cry: number, gas: number): boolean {
		return res.biomass >= bio && res.minerals >= min && res.crystal >= cry && res.spore_gas >= gas;
	}

	function costClass(have: number, need: number): string {
		return have >= need ? 'text-green-400' : 'text-red-400';
	}

	// ── API calls ──────────────────────────────────────────────────────────

	async function loadPlanet() {
		try {
			planet = await invoke<Planet>('swarm_get_planet');
			error = '';
		} catch (e) {
			error = String(e);
		}
	}

	async function loadShop() {
		try {
			shopItems = await invoke<ShopItem[]>('swarm_shop_items');
		} catch (_) { /* ignore */ }
	}

	async function loadGalaxy() {
		try {
			galaxySlots = await invoke<PlanetSlot[]>('swarm_get_galaxy', { galaxy: galaxyNum, system: systemNum });
		} catch (_) { /* ignore */ }
	}

	async function checkTimers() {
		try {
			completedTimers = await invoke<CompletedTimer[]>('swarm_check_timers');
			if (completedTimers.length > 0) {
				statusMsg = completedTimers.map(t => `${t.item_name} completed!`).join(', ');
				await loadPlanet();
				setTimeout(() => { statusMsg = ''; }, 5000);
			}
		} catch (_) { /* ignore */ }
	}

	async function upgradeBuilding(bt: string) {
		try {
			await invoke('swarm_upgrade_building', { buildingType: bt });
			statusMsg = 'Upgrade started!';
			await loadPlanet();
			setTimeout(() => { statusMsg = ''; }, 3000);
		} catch (e) {
			statusMsg = String(e);
			setTimeout(() => { statusMsg = ''; }, 4000);
		}
	}

	async function startResearch(tech: string) {
		try {
			await invoke('swarm_start_research', { tech });
			statusMsg = 'Research started!';
			await loadPlanet();
			setTimeout(() => { statusMsg = ''; }, 3000);
		} catch (e) {
			statusMsg = String(e);
			setTimeout(() => { statusMsg = ''; }, 4000);
		}
	}

	async function buildShips(shipType: string) {
		const count = shipBuildCounts[shipType] ?? 1;
		if (count <= 0) return;
		try {
			await invoke('swarm_build_ships', { shipType, count });
			statusMsg = `Built ${count} ships!`;
			shipBuildCounts[shipType] = 1;
			await loadPlanet();
			setTimeout(() => { statusMsg = ''; }, 3000);
		} catch (e) {
			statusMsg = String(e);
			setTimeout(() => { statusMsg = ''; }, 4000);
		}
	}

	async function buyShopItem(itemId: string) {
		try {
			await invoke('swarm_shop_buy', { itemId });
			statusMsg = 'Item purchased!';
			await loadPlanet();
			setTimeout(() => { statusMsg = ''; }, 3000);
		} catch (e) {
			statusMsg = String(e);
			setTimeout(() => { statusMsg = ''; }, 4000);
		}
	}

	async function collectResources() {
		try {
			await invoke('swarm_collect_resources');
			await loadPlanet();
			statusMsg = 'Resources collected!';
			setTimeout(() => { statusMsg = ''; }, 2000);
		} catch (_) { /* ignore */ }
	}

	// ── Mutation API ──────────────────────────────────────────────────────

	async function loadMutationUnits() {
		try {
			// Reuse planet data to get swarm units from the quest system
			const swarm = await invoke<{
				units: { id: string; unit_type: string; name: string; level: number }[];
			}>('quest_get_swarm');
			mutationUnits = swarm.units;
		} catch (_) { mutationUnits = []; }
	}

	async function selectMutationUnit(unitId: string) {
		selectedMutationUnit = unitId;
		try {
			unitMutations = await invoke<UnitMutations>('swarm_get_mutations', { unitId });
			// Load the full mutation tree for this unit type
			mutationTree = await invoke<MutationChoice[][]>('swarm_get_mutation_tree', {
				unitType: unitMutations.unit_type
			});
			// Build a lookup catalog from the tree
			const catalog: Record<string, MutationChoice> = {};
			for (const tier of mutationTree) {
				for (const m of tier) {
					catalog[m.id] = m;
				}
			}
			mutationCatalog = catalog;
		} catch (e) {
			mutationStatus = String(e);
			setTimeout(() => { mutationStatus = ''; }, 4000);
		}
	}

	async function applyMutation(mutationId: string) {
		if (!selectedMutationUnit) return;
		try {
			await invoke('swarm_apply_mutation', {
				unitId: selectedMutationUnit,
				mutationId
			});
			mutationStatus = 'Mutation applied!';
			// Reload unit mutations
			await selectMutationUnit(selectedMutationUnit);
			await loadPlanet();
			setTimeout(() => { mutationStatus = ''; }, 3000);
		} catch (e) {
			mutationStatus = String(e);
			setTimeout(() => { mutationStatus = ''; }, 4000);
		}
	}

	function mutTypeColor(mt: string): string {
		switch (mt) {
			case 'defensive': return 'border-green-500 bg-green-950/40';
			case 'offensive': return 'border-red-500 bg-red-950/40';
			case 'utility': return 'border-blue-500 bg-blue-950/40';
			case 'evolution': return 'border-yellow-500 bg-yellow-950/40';
			case 'specialization': return 'border-purple-500 bg-purple-950/40';
			default: return 'border-gray-500 bg-gray-950/40';
		}
	}

	function mutTypeAnimation(mt: string): string {
		switch (mt) {
			case 'defensive': return 'sf-mutation-def';
			case 'offensive': return 'sf-mutation-off';
			case 'utility': return 'sf-mutation-util';
			case 'evolution': return 'sf-mutation-evo';
			case 'specialization': return 'sf-mutation-spec';
			default: return '';
		}
	}

	// Track previous resource values for pulse animation
	let prevBiomass = $state(0);
	let prevMinerals = $state(0);
	let prevCrystal = $state(0);
	let prevSporeGas = $state(0);
	let resourcePulseKey = $state(0);

	$effect(() => {
		const b = res.biomass;
		const m = res.minerals;
		const c = res.crystal;
		const g = res.spore_gas;
		if (b !== prevBiomass || m !== prevMinerals || c !== prevCrystal || g !== prevSporeGas) {
			if (prevBiomass > 0) resourcePulseKey++;
			prevBiomass = b;
			prevMinerals = m;
			prevCrystal = c;
			prevSporeGas = g;
		}
	});

	function mutTypeLabel(mt: string): string {
		switch (mt) {
			case 'defensive': return 'DEF';
			case 'offensive': return 'ATK';
			case 'utility': return 'UTL';
			case 'evolution': return 'EVO';
			case 'specialization': return 'SPEC';
			default: return '???';
		}
	}

	function mutTypeTextColor(mt: string): string {
		switch (mt) {
			case 'defensive': return 'text-green-400';
			case 'offensive': return 'text-red-400';
			case 'utility': return 'text-blue-400';
			case 'evolution': return 'text-yellow-400';
			case 'specialization': return 'text-purple-400';
			default: return 'text-gray-400';
		}
	}

	// ── Lifecycle ──────────────────────────────────────────────────────────

	let timerInterval: ReturnType<typeof setInterval> | null = null;
	let resInterval: ReturnType<typeof setInterval> | null = null;

	onMount(() => {
		loadPlanet().then(() => {
			loading = false;
		});
		loadShop();

		// Poll timers every 10 seconds
		timerInterval = setInterval(checkTimers, 10000);
		// Refresh resources every 30 seconds
		resInterval = setInterval(collectResources, 30000);

		return () => {
			if (timerInterval) clearInterval(timerInterval);
			if (resInterval) clearInterval(resInterval);
		};
	});

	// Navigation items
	const NAV_ITEMS: { id: NavSection; label: string }[] = [
		{ id: 'overview', label: 'Overview' },
		{ id: 'buildings', label: 'Buildings' },
		{ id: 'research', label: 'Research' },
		{ id: 'fleet', label: 'Fleet' },
		{ id: 'defense', label: 'Defense' },
		{ id: 'galaxy', label: 'Galaxy' },
		{ id: 'creep', label: 'Creep' },
		{ id: 'shop', label: 'Shop' },
		{ id: 'mutations', label: 'Mutations' },
		{ id: 'stats', label: 'Stats' },
	];

	const NAV_ICONS: Record<NavSection, string> = {
		overview: 'home',
		buildings: 'building',
		research: 'flask',
		fleet: 'rocket',
		defense: 'shield',
		galaxy: 'globe',
		creep: 'layers',
		shop: 'cart',
		mutations: 'dna',
		stats: 'chart',
	};

	const BUILDING_ICONS: Record<string, string> = {
		biomass_converter: 'B',
		mineral_drill: 'M',
		crystal_synthesizer: 'C',
		spore_extractor: 'G',
		energy_nest: 'E',
		creep_generator: 'Cr',
		brood_nest: 'Br',
		evolution_lab: 'Lab',
		blighthaven: 'BH',
		spore_defense: 'SD',
		biomass_storage: 'BS',
		mineral_silo: 'MS',
	};
</script>

<div class="flex flex-col h-full bg-[#0a0e1a] text-gray-200 font-sans select-none">
	<!-- ═══════════ TOP RESOURCE BAR ═══════════ -->
	<div class="flex items-center gap-4 px-4 py-2 bg-[#0d1225]/90 border-b border-blue-900/40 text-xs shrink-0 overflow-x-auto sf-resource-bar">
		<span class="text-blue-300 font-bold tracking-wider text-sm mr-2">SWARMFORGE</span>

		<!-- Biomass -->
		{#key resourcePulseKey}
		<div class="flex items-center gap-1 sf-animate-count" title="Biomass ({fmt(res.biomass_per_hour)}/h)">
			<span class="text-green-400 text-base">B</span>
			<span class="text-green-300 font-mono">{fmt(res.biomass)}</span>
			<span class="text-green-700 text-[10px]">+{fmt(res.biomass_per_hour)}/h</span>
		</div>
		{/key}

		<div class="w-px h-4 bg-blue-900/40"></div>

		<!-- Minerals -->
		{#key resourcePulseKey}
		<div class="flex items-center gap-1 sf-animate-count" title="Minerals ({fmt(res.minerals_per_hour)}/h)">
			<span class="text-cyan-400 text-base">M</span>
			<span class="text-cyan-300 font-mono">{fmt(res.minerals)}</span>
			<span class="text-cyan-700 text-[10px]">+{fmt(res.minerals_per_hour)}/h</span>
		</div>
		{/key}

		<div class="w-px h-4 bg-blue-900/40"></div>

		<!-- Crystal -->
		{#key resourcePulseKey}
		<div class="flex items-center gap-1 sf-animate-count" title="Crystal ({fmt(res.crystal_per_hour)}/h)">
			<span class="text-purple-400 text-base">C</span>
			<span class="text-purple-300 font-mono">{fmt(res.crystal)}</span>
			<span class="text-purple-700 text-[10px]">+{fmt(res.crystal_per_hour)}/h</span>
		</div>
		{/key}

		<div class="w-px h-4 bg-blue-900/40"></div>

		<!-- Spore Gas -->
		{#key resourcePulseKey}
		<div class="flex items-center gap-1 sf-animate-count" title="Spore Gas ({fmt(res.spore_gas_per_hour)}/h)">
			<span class="text-amber-400 text-base">G</span>
			<span class="text-amber-300 font-mono">{fmt(res.spore_gas)}</span>
			<span class="text-amber-700 text-[10px]">+{fmt(res.spore_gas_per_hour)}/h</span>
		</div>
		{/key}

		<div class="w-px h-4 bg-blue-900/40"></div>

		<!-- Energy -->
		<div class="flex items-center gap-1" title="Energy: {res.energy_production} produced, {res.energy_consumption} consumed">
			<span class="text-yellow-400 text-base">E</span>
			<span class="{energyClass} font-mono">{res.energy}</span>
			<span class="text-yellow-700 text-[10px]">{res.energy_production}/{res.energy_consumption}</span>
		</div>

		<div class="w-px h-4 bg-blue-900/40"></div>

		<!-- Dark Matter -->
		<div class="flex items-center gap-1 sf-animate-dm" title="Dark Matter (earned from achievements only)">
			<span class="text-indigo-400 text-base">DM</span>
			<span class="text-indigo-300 font-mono">{fmt(res.dark_matter)}</span>
		</div>
	</div>

	<!-- Status message -->
	{#if statusMsg}
		<div class="px-4 py-1.5 bg-blue-900/30 text-blue-200 text-xs text-center border-b border-blue-800/30">
			{statusMsg}
		</div>
	{/if}

	<!-- ═══════════ MAIN AREA (sidebar + content) ═══════════ -->
	<div class="flex flex-1 overflow-hidden">

		<!-- ── LEFT SIDEBAR (200px) ── -->
		<nav class="w-[200px] shrink-0 bg-[#0b1020] border-r border-blue-900/30 flex flex-col py-2 overflow-y-auto">
			<div class="px-3 py-1 text-[10px] text-blue-500 uppercase tracking-widest mb-1">Navigation</div>
			{#each NAV_ITEMS as item}
				<button
					class="flex items-center gap-2 px-3 py-2 text-sm transition-colors {activeNav === item.id
						? 'bg-blue-900/40 text-blue-200 border-l-2 border-blue-400'
						: 'text-gray-400 hover:bg-blue-900/20 hover:text-gray-200 border-l-2 border-transparent'}"
					onclick={() => {
						activeNav = item.id;
						if (item.id === 'galaxy') loadGalaxy();
						if (item.id === 'mutations') loadMutationUnits();
					}}
				>
					{#if item.id === 'overview'}<Home size={14} />
					{:else if item.id === 'buildings'}<Factory size={14} />
					{:else if item.id === 'research'}<FlaskConical size={14} />
					{:else if item.id === 'fleet'}<Rocket size={14} />
					{:else if item.id === 'defense'}<Shield size={14} />
					{:else if item.id === 'galaxy'}<Globe size={14} />
					{:else if item.id === 'creep'}<Layers size={14} />
					{:else if item.id === 'shop'}<ShoppingBag size={14} />
					{:else if item.id === 'mutations'}<Dna size={14} />
					{:else if item.id === 'stats'}<BarChart3 size={14} />
					{/if}
					{item.label}
				</button>
			{/each}

			<!-- Planet info at bottom -->
			<div class="mt-auto px-3 py-2 border-t border-blue-900/30 text-[10px] text-gray-500">
				<div class="text-blue-400 font-bold">{planet?.name ?? 'Loading...'}</div>
				<div>Creep: {planet?.creep.coverage_percent.toFixed(1) ?? 0}%</div>
				<div>Fleet: {planet?.fleet.reduce((s, f) => s + f.count, 0) ?? 0} ships</div>
			</div>
		</nav>

		<!-- ── CENTER CONTENT ── -->
		<main class="flex-1 overflow-y-auto p-4">
			{#if loading}
				<div class="flex items-center justify-center h-full">
					<div class="text-blue-400 animate-pulse text-lg">Loading SwarmForge...</div>
				</div>
			{:else if error}
				<div class="bg-red-900/20 border border-red-800/40 rounded p-4 text-red-300">
					{error}
				</div>
			{:else if !planet}
				<div class="text-gray-500">No planet data available.</div>

			<!-- ═══ OVERVIEW ═══ -->
			{:else if activeNav === 'overview'}
				<div class="space-y-4 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-blue-300 sf-section-header px-2 rounded">Colony Overview -- {planet.name}</h2>

					<!-- Planet visualization -->
					<div class="flex items-center gap-8">
						<div class="relative sf-animate-planet" style="animation-duration: 120s;">
							<svg width="180" height="180" viewBox="0 0 180 180">
								<!-- Planet -->
								<circle cx="90" cy="90" r="70" fill="#1a3a2a" stroke="#2d5a3a" stroke-width="2" />
								<!-- Creep coverage overlay -->
								<circle cx="90" cy="90" r="70" fill="none" stroke="#4ade80" stroke-width="4"
									stroke-dasharray="{planet.creep.coverage_percent * 4.4} {440 - planet.creep.coverage_percent * 4.4}"
									transform="rotate(-90 90 90)" opacity="0.6" class="sf-animate-creep" />
								<!-- Center label -->
								<text x="90" y="85" text-anchor="middle" fill="#e2e8f0" font-size="14" font-weight="bold">
									{planet.creep.coverage_percent.toFixed(0)}%
								</text>
								<text x="90" y="102" text-anchor="middle" fill="#94a3b8" font-size="10">CREEP</text>
							</svg>
						</div>

						<div class="grid grid-cols-2 gap-3 text-sm">
							<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
								<div class="text-[10px] text-gray-500 uppercase">Buildings</div>
								<div class="text-lg font-bold text-blue-300">
									{planet.buildings.reduce((s, b) => s + b.level, 0)}
								</div>
								<div class="text-[10px] text-gray-500">total levels</div>
							</div>
							<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
								<div class="text-[10px] text-gray-500 uppercase">Research</div>
								<div class="text-lg font-bold text-purple-300">
									{planet.research.reduce((s, r) => s + r.level, 0)}
								</div>
								<div class="text-[10px] text-gray-500">total levels</div>
							</div>
							<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
								<div class="text-[10px] text-gray-500 uppercase">Fleet</div>
								<div class="text-lg font-bold text-cyan-300">
									{planet.fleet.reduce((s, f) => s + f.count, 0)}
								</div>
								<div class="text-[10px] text-gray-500">ships</div>
							</div>
							<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
								<div class="text-[10px] text-gray-500 uppercase">Energy</div>
								<div class="text-lg font-bold {energyClass}">{res.energy}</div>
								<div class="text-[10px] text-gray-500">{res.energy_production}P / {res.energy_consumption}C</div>
							</div>
						</div>
					</div>

					<!-- Active timers -->
					{#each planet.buildings.filter(b => b.upgrading) as bldg}
						<div class="bg-amber-900/20 border border-amber-800/30 rounded p-3 flex items-center gap-3 sf-animate-build">
							<Timer size={16} class="text-amber-400" />
							<span class="text-amber-200 text-sm">Building: {bldg.display_name} Lv.{bldg.level + 1}</span>
							<span class="text-amber-400 font-mono ml-auto">{timerRemaining(bldg.upgrade_finish)}</span>
						</div>
					{/each}
					{#each planet.research.filter(r => r.researching) as tech}
						<div class="bg-purple-900/20 border border-purple-800/30 rounded p-3 flex items-center gap-3 sf-animate-build">
							<FlaskConical size={16} class="text-purple-400" />
							<span class="text-purple-200 text-sm">Research: {tech.display_name} Lv.{tech.level + 1}</span>
							<span class="text-purple-400 font-mono ml-auto">{timerRemaining(tech.research_finish)}</span>
						</div>
					{/each}

					<!-- Creep milestones -->
					<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
						<h3 class="text-sm font-bold text-green-400 mb-2">Creep Milestones</h3>
						<div class="grid grid-cols-4 gap-2 text-xs">
							{#each [
								{ pct: 25, label: 'Unlock Blighthaven', done: planet.creep.coverage_percent >= 25 },
								{ pct: 50, label: '+20% production', done: planet.creep.coverage_percent >= 50 },
								{ pct: 75, label: 'Unlock World Eater', done: planet.creep.coverage_percent >= 75 },
								{ pct: 100, label: '+50% all stats', done: planet.creep.coverage_percent >= 100 },
							] as milestone}
								<div class="p-2 rounded text-center {milestone.done ? 'bg-green-900/30 text-green-300' : 'bg-gray-900/30 text-gray-500'}">
									<div class="font-bold">{milestone.pct}%</div>
									<div class="text-[10px]">{milestone.label}</div>
								</div>
							{/each}
						</div>
					</div>
				</div>

			<!-- ═══ BUILDINGS ═══ -->
			{:else if activeNav === 'buildings'}
				<div class="space-y-3 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-blue-300 mb-3 sf-section-header px-2 rounded">Buildings</h2>
					<div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
						{#each planet.buildings as bldg}
							<div class="bg-[#111833] rounded-lg border border-blue-900/30 p-3 transition-all duration-300 {bldg.upgrading ? 'sf-animate-build sf-active-glow' : 'hover:border-blue-700/50'}">
								<div class="flex items-center justify-between mb-2">
									<div>
										<span class="text-blue-300 font-bold text-sm">{bldg.display_name}</span>
										<span class="text-blue-500 text-xs ml-2">Lv.{bldg.level}</span>
									</div>
									<span class="text-xs text-blue-700 font-mono px-2 py-0.5 bg-blue-900/20 rounded">
										{BUILDING_ICONS[bldg.building_type] ?? '?'}
									</span>
								</div>
								<p class="text-[11px] text-gray-400 mb-2">{bldg.description}</p>

								<!-- Costs -->
								<div class="flex gap-3 text-[10px] mb-2">
									{#if bldg.cost_biomass > 0}
										<span class="{costClass(res.biomass, bldg.cost_biomass)}">B: {fmt(bldg.cost_biomass)}</span>
									{/if}
									{#if bldg.cost_minerals > 0}
										<span class="{costClass(res.minerals, bldg.cost_minerals)}">M: {fmt(bldg.cost_minerals)}</span>
									{/if}
									{#if bldg.cost_crystal > 0}
										<span class="{costClass(res.crystal, bldg.cost_crystal)}">C: {fmt(bldg.cost_crystal)}</span>
									{/if}
									{#if bldg.cost_spore_gas > 0}
										<span class="{costClass(res.spore_gas, bldg.cost_spore_gas)}">G: {fmt(bldg.cost_spore_gas)}</span>
									{/if}
									<span class="text-gray-500 ml-auto">{fmtTime(bldg.build_time_seconds)}</span>
								</div>

								{#if bldg.upgrading}
									<div class="flex items-center gap-2 text-amber-400 text-xs">
										<Timer size={12} />
										<span>Upgrading... {timerRemaining(bldg.upgrade_finish)}</span>
									</div>
								{:else}
									<button
										class="w-full py-1.5 rounded text-xs font-bold transition-colors
											{canAfford(bldg.cost_biomass, bldg.cost_minerals, bldg.cost_crystal, bldg.cost_spore_gas)
												? 'bg-blue-600 hover:bg-blue-500 text-white'
												: 'bg-gray-700 text-gray-500 cursor-not-allowed'}"
										disabled={!canAfford(bldg.cost_biomass, bldg.cost_minerals, bldg.cost_crystal, bldg.cost_spore_gas)}
										onclick={() => upgradeBuilding(bldg.building_type)}
									>
										Upgrade to Lv.{bldg.level + 1}
									</button>
								{/if}
							</div>
						{/each}
					</div>
				</div>

			<!-- ═══ RESEARCH ═══ -->
			{:else if activeNav === 'research'}
				<div class="space-y-3 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-purple-300 mb-3 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(88, 28, 135, 0.3) 0%, transparent 100%); border-color: rgba(168, 85, 247, 0.15);">Research</h2>
					<div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
						{#each planet.research as tech}
							<div class="bg-[#111833] rounded-lg border border-purple-900/30 p-3 transition-all duration-300 {tech.researching ? 'sf-animate-build' : 'hover:border-purple-700/50'}">
								<div class="flex items-center justify-between mb-2">
									<div>
										<span class="text-purple-300 font-bold text-sm">{tech.display_name}</span>
										<span class="text-purple-500 text-xs ml-2">Lv.{tech.level}</span>
									</div>
									<span class="text-[10px] text-gray-500">Lab Lv.{tech.required_lab_level} req.</span>
								</div>
								<p class="text-[11px] text-gray-400 mb-2">{tech.description}</p>

								<div class="flex gap-3 text-[10px] mb-2">
									{#if tech.cost_biomass > 0}
										<span class="{costClass(res.biomass, tech.cost_biomass)}">B: {fmt(tech.cost_biomass)}</span>
									{/if}
									{#if tech.cost_minerals > 0}
										<span class="{costClass(res.minerals, tech.cost_minerals)}">M: {fmt(tech.cost_minerals)}</span>
									{/if}
									{#if tech.cost_crystal > 0}
										<span class="{costClass(res.crystal, tech.cost_crystal)}">C: {fmt(tech.cost_crystal)}</span>
									{/if}
									{#if tech.cost_spore_gas > 0}
										<span class="{costClass(res.spore_gas, tech.cost_spore_gas)}">G: {fmt(tech.cost_spore_gas)}</span>
									{/if}
									<span class="text-gray-500 ml-auto">{fmtTime(tech.research_time_seconds)}</span>
								</div>

								{#if tech.researching}
									<div class="flex items-center gap-2 text-purple-400 text-xs">
										<Timer size={12} />
										<span>Researching... {timerRemaining(tech.research_finish)}</span>
									</div>
								{:else}
									<button
										class="w-full py-1.5 rounded text-xs font-bold transition-colors
											{canAfford(tech.cost_biomass, tech.cost_minerals, tech.cost_crystal, tech.cost_spore_gas)
												? 'bg-purple-600 hover:bg-purple-500 text-white'
												: 'bg-gray-700 text-gray-500 cursor-not-allowed'}"
										disabled={!canAfford(tech.cost_biomass, tech.cost_minerals, tech.cost_crystal, tech.cost_spore_gas)}
										onclick={() => startResearch(tech.tech_type)}
									>
										Research Lv.{tech.level + 1}
									</button>
								{/if}
							</div>
						{/each}
					</div>
				</div>

			<!-- ═══ FLEET ═══ -->
			{:else if activeNav === 'fleet'}
				<div class="space-y-3 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-cyan-300 mb-3 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(8, 51, 68, 0.5) 0%, transparent 100%); border-color: rgba(34, 211, 238, 0.15);">Fleet</h2>
					<div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
						{#each planet.fleet as ship}
							<div class="bg-[#111833] rounded-lg border border-cyan-900/30 p-3">
								<div class="flex items-center justify-between mb-1">
									<span class="text-cyan-300 font-bold text-sm">{ship.display_name}</span>
									<span class="text-cyan-400 font-mono text-sm">{ship.count}x</span>
								</div>
								<p class="text-[11px] text-gray-400 mb-2">{ship.description}</p>

								<div class="flex gap-4 text-[10px] text-gray-400 mb-2">
									<span>ATK: <span class="text-red-400">{ship.attack}</span></span>
									<span>SHD: <span class="text-blue-400">{ship.shields}</span></span>
									<span>HP: <span class="text-green-400">{ship.hp}</span></span>
								</div>

								<div class="flex items-center gap-2">
									<input
										type="number"
										min="1"
										max="9999"
										class="w-20 bg-[#0a0e1a] border border-cyan-900/40 rounded px-2 py-1 text-xs text-cyan-200 focus:outline-none focus:border-cyan-500"
										value={shipBuildCounts[ship.ship_type] ?? 1}
										oninput={(e) => { shipBuildCounts[ship.ship_type] = parseInt((e.target as HTMLInputElement).value) || 1; }}
									/>
									<button
										class="flex-1 py-1.5 rounded text-xs font-bold bg-cyan-700 hover:bg-cyan-600 text-white transition-colors"
										onclick={() => buildShips(ship.ship_type)}
									>
										Build
									</button>
								</div>
							</div>
						{/each}
					</div>
				</div>

			<!-- ═══ DEFENSE ═══ -->
			{:else if activeNav === 'defense'}
				<div class="space-y-3 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-orange-300 mb-3 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(124, 45, 18, 0.3) 0%, transparent 100%); border-color: rgba(251, 146, 60, 0.15);">Defense</h2>
					{#each planet.buildings.filter(b => b.building_type === 'spore_defense') as defBuilding}
						<div class="bg-[#111833] rounded-lg border border-orange-900/30 p-4">
							<div class="flex items-center gap-3 mb-3">
								<Shield size={24} class="text-orange-400" />
								<div>
									<div class="text-orange-300 font-bold">{defBuilding.display_name} Lv.{defBuilding.level}</div>
									<div class="text-[11px] text-gray-400">{defBuilding.description}</div>
								</div>
							</div>

							<div class="grid grid-cols-2 gap-3 text-sm mb-3">
								<div class="bg-[#0a0e1a] rounded p-2 text-center">
									<div class="text-[10px] text-gray-500">Defense Power</div>
									<div class="text-orange-300 font-bold">{defBuilding.level * 500}</div>
								</div>
								<div class="bg-[#0a0e1a] rounded p-2 text-center">
									<div class="text-[10px] text-gray-500">Armor Bonus</div>
									<div class="text-orange-300 font-bold">+{defBuilding.level * 10}%</div>
								</div>
							</div>

							{#if !defBuilding.upgrading}
								<button
									class="w-full py-2 rounded text-xs font-bold transition-colors
										{canAfford(defBuilding.cost_biomass, defBuilding.cost_minerals, defBuilding.cost_crystal, defBuilding.cost_spore_gas)
											? 'bg-orange-600 hover:bg-orange-500 text-white'
											: 'bg-gray-700 text-gray-500 cursor-not-allowed'}"
									disabled={!canAfford(defBuilding.cost_biomass, defBuilding.cost_minerals, defBuilding.cost_crystal, defBuilding.cost_spore_gas)}
									onclick={() => upgradeBuilding(defBuilding.building_type)}
								>
									Upgrade to Lv.{defBuilding.level + 1}
								</button>
							{:else}
								<div class="flex items-center gap-2 text-amber-400 text-xs">
									<Timer size={12} />
									<span>Upgrading... {timerRemaining(defBuilding.upgrade_finish)}</span>
								</div>
							{/if}
						</div>
					{/each}

					<!-- Fleet defense summary -->
					<div class="bg-[#111833] rounded-lg border border-orange-900/30 p-4">
						<h3 class="text-sm font-bold text-orange-300 mb-2">Fleet Defense Force</h3>
						<div class="space-y-1 text-xs">
							{#each planet.fleet.filter(f => f.count > 0) as ship}
								<div class="flex justify-between text-gray-300">
									<span>{ship.display_name}</span>
									<span class="font-mono">{ship.count}x (ATK: {ship.attack * ship.count})</span>
								</div>
							{/each}
							{#if planet.fleet.every(f => f.count === 0)}
								<div class="text-gray-500 italic">No ships built yet. Build fleet in the Fleet tab.</div>
							{/if}
						</div>
					</div>
				</div>

			<!-- ═══ GALAXY ═══ -->
			{:else if activeNav === 'galaxy'}
				<div class="space-y-3 sf-animate-tab-enter sf-space-bg rounded-lg p-2">
					<div class="flex items-center gap-3 mb-3">
						<h2 class="text-xl font-bold text-emerald-300 sf-animate-twinkle" style="animation-duration: 4s;">Galaxy View</h2>
						<div class="flex items-center gap-1 ml-auto">
							<label class="text-xs text-gray-400">G:</label>
							<input type="number" min="1" max="9" class="w-12 bg-[#0a0e1a] border border-emerald-900/40 rounded px-1 py-0.5 text-xs text-emerald-200"
								bind:value={galaxyNum} onchange={() => loadGalaxy()} />
							<label class="text-xs text-gray-400 ml-2">S:</label>
							<input type="number" min="1" max="499" class="w-16 bg-[#0a0e1a] border border-emerald-900/40 rounded px-1 py-0.5 text-xs text-emerald-200"
								bind:value={systemNum} onchange={() => loadGalaxy()} />
						</div>
					</div>

					<div class="bg-[#111833] rounded-lg border border-emerald-900/30 overflow-hidden">
						<div class="grid grid-cols-[40px_1fr_1fr_80px] gap-0 text-[10px] text-gray-500 px-3 py-1.5 bg-[#0d1225] border-b border-emerald-900/20 font-bold uppercase">
							<span>Pos</span><span>Planet</span><span>Player</span><span>Type</span>
						</div>
						{#each galaxySlots as slot}
							<div class="grid grid-cols-[40px_1fr_1fr_80px] gap-0 px-3 py-1.5 text-xs border-b border-blue-900/10
								{slot.position === 4 ? 'bg-emerald-900/20' : slot.occupied ? 'bg-[#0f1528]' : ''}">
								<span class="text-gray-500 font-mono {slot.occupied ? 'sf-animate-twinkle' : ''}" style="animation-delay: {slot.position * 0.4}s;">{slot.position}</span>
								{#if slot.occupied}
									<span class="{slot.position === 4 ? 'text-emerald-300 font-bold' : 'text-gray-300'}">
										{slot.planet_name}
									</span>
									<span class="{slot.position === 4 ? 'text-emerald-400' : 'text-gray-400'}">
										{slot.player_name}
									</span>
									<span class="text-gray-500">{slot.planet_type}</span>
								{:else}
									<span class="text-gray-600">--</span>
									<span class="text-gray-600">--</span>
									<span class="text-gray-600">--</span>
								{/if}
							</div>
						{/each}
						{#if galaxySlots.length === 0}
							<div class="p-4 text-center text-gray-500 text-xs">Click the galaxy/system inputs to explore.</div>
						{/if}
					</div>
				</div>

			<!-- ═══ CREEP ═══ -->
			{:else if activeNav === 'creep'}
				<div class="space-y-4 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-green-300 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(20, 83, 45, 0.3) 0%, transparent 100%); border-color: rgba(74, 222, 128, 0.15);">Creep Spread</h2>

					<!-- Creep visualization -->
					<div class="bg-[#111833] rounded-lg border border-green-900/30 p-4 sf-animate-creep">
						<div class="flex items-center gap-6">
							<div class="relative w-32 h-32">
								<svg width="128" height="128" viewBox="0 0 128 128">
									<circle cx="64" cy="64" r="56" fill="none" stroke="#1a3a2a" stroke-width="8" />
									<circle cx="64" cy="64" r="56" fill="none" stroke="#4ade80" stroke-width="8"
										stroke-dasharray="{planet.creep.coverage_percent * 3.52} {352 - planet.creep.coverage_percent * 3.52}"
										transform="rotate(-90 64 64)" />
									<text x="64" y="60" text-anchor="middle" fill="#4ade80" font-size="20" font-weight="bold">
										{planet.creep.coverage_percent.toFixed(1)}%
									</text>
									<text x="64" y="78" text-anchor="middle" fill="#6b7280" font-size="10">coverage</text>
								</svg>
							</div>

							<div class="grid grid-cols-2 gap-3 text-sm flex-1">
								<div class="bg-[#0a0e1a] rounded p-2">
									<div class="text-[10px] text-gray-500">Spread Rate</div>
									<div class="text-green-400 font-bold">{planet.creep.spread_rate_per_hour.toFixed(1)}%/h</div>
								</div>
								<div class="bg-[#0a0e1a] rounded p-2">
									<div class="text-[10px] text-gray-500">Biomass Bonus</div>
									<div class="text-green-400 font-bold">+{planet.creep.biomass_bonus.toFixed(1)}%</div>
								</div>
								<div class="bg-[#0a0e1a] rounded p-2">
									<div class="text-[10px] text-gray-500">Flora Corrupted</div>
									<div class="text-amber-400 font-bold">{planet.creep.flora_corrupted.toFixed(1)}%</div>
								</div>
								<div class="bg-[#0a0e1a] rounded p-2">
									<div class="text-[10px] text-gray-500">Fauna Consumed</div>
									<div class="text-red-400 font-bold">{planet.creep.fauna_consumed.toFixed(1)}%</div>
								</div>
							</div>
						</div>
					</div>

					<div class="bg-[#111833] rounded p-3 border border-green-900/30 text-xs text-gray-400">
						<p class="mb-1">Build a <span class="text-green-300">Creep Generator</span> and research <span class="text-green-300">Creep Biology</span> to increase spread rate.</p>
						<p>Creep coverage unlocks milestones: 25% (Blighthaven), 50% (+20% production), 75% (World Eater), 100% (+50% all stats).</p>
					</div>
				</div>

			<!-- ═══ SHOP ═══ -->
			{:else if activeNav === 'shop'}
				<div class="space-y-3 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-indigo-300 mb-1 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(49, 46, 129, 0.3) 0%, transparent 100%); border-color: rgba(129, 140, 248, 0.15);">Dark Matter Shop</h2>
					<p class="text-[11px] text-gray-500 mb-3">Dark Matter is earned from achievements, daily logins, and quests. Never from real money.</p>

					<div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
						{#each shopItems as item}
							<div class="bg-[#111833] rounded-lg border border-indigo-900/30 p-3">
								<div class="flex items-center justify-between mb-1">
									<span class="text-indigo-300 font-bold text-sm">{item.name}</span>
									<span class="text-indigo-400 font-mono text-xs">{item.cost_dark_matter} DM</span>
								</div>
								<p class="text-[11px] text-gray-400 mb-2">{item.description}</p>
								{#if item.duration_hours}
									<p class="text-[10px] text-gray-500 mb-2">Duration: {item.duration_hours}h</p>
								{:else}
									<p class="text-[10px] text-amber-500 mb-2">Permanent</p>
								{/if}
								<button
									class="w-full py-1.5 rounded text-xs font-bold transition-colors
										{res.dark_matter >= item.cost_dark_matter
											? 'bg-indigo-600 hover:bg-indigo-500 text-white'
											: 'bg-gray-700 text-gray-500 cursor-not-allowed'}"
									disabled={res.dark_matter < item.cost_dark_matter}
									onclick={() => buyShopItem(item.id)}
								>
									Buy ({item.cost_dark_matter} DM)
								</button>
							</div>
						{/each}
					</div>
				</div>

			<!-- ═══ MUTATIONS ═══ -->
			{:else if activeNav === 'mutations'}
				<div class="space-y-4 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-purple-300 sf-section-header px-2 rounded" style="background: linear-gradient(90deg, rgba(88, 28, 135, 0.3) 0%, transparent 100%); border-color: rgba(192, 132, 252, 0.15);">Unit Mutations</h2>
					<p class="text-xs text-gray-400">
						Every 5 levels, units earn a permanent mutation choice. Pick wisely -- mutations cannot be undone.
					</p>

					{#if mutationStatus}
						<div class="px-3 py-1.5 bg-purple-900/30 text-purple-200 text-xs rounded border border-purple-800/30">
							{mutationStatus}
						</div>
					{/if}

					<div class="flex gap-4">
						<!-- Unit list (left) -->
						<div class="w-[220px] shrink-0 space-y-1">
							<div class="text-[10px] text-gray-500 uppercase tracking-widest mb-2">Select Unit</div>
							{#if mutationUnits.length === 0}
								<div class="text-xs text-gray-500 italic">No units spawned yet.</div>
							{/if}
							{#each mutationUnits as u}
								<button
									class="w-full text-left px-3 py-2 rounded text-xs transition-colors
										{selectedMutationUnit === u.id
											? 'bg-purple-900/40 text-purple-200 border border-purple-500/50'
											: 'bg-[#111833] text-gray-300 border border-transparent hover:bg-purple-900/20'}"
									onclick={() => selectMutationUnit(u.id)}
								>
									<div class="font-bold">{u.name}</div>
									<div class="text-[10px] text-gray-500">
										{u.unit_type.replace(/_/g, ' ')} -- Lv.{u.level}
										{#if u.level >= 5 && u.level % 5 === 0}
											<span class="text-yellow-400 ml-1">NEW</span>
										{/if}
									</div>
								</button>
							{/each}
						</div>

						<!-- Mutation detail (right) -->
						<div class="flex-1 min-w-0">
							{#if !unitMutations}
								<div class="flex items-center justify-center h-40 text-gray-500 text-sm">
									Select a unit to view its mutations.
								</div>
							{:else}
								<!-- Unit header -->
								<div class="bg-[#111833] rounded p-3 border border-purple-900/30 mb-4">
									<div class="flex items-center justify-between">
										<div>
											<div class="text-sm font-bold text-purple-300">
												{mutationUnits.find(u => u.id === selectedMutationUnit)?.name ?? 'Unit'}
											</div>
											<div class="text-[10px] text-gray-400">
												{unitMutations.unit_type.replace(/_/g, ' ')} -- Level {unitMutations.unit_level}
											</div>
										</div>
										<div class="text-[10px] text-gray-500">
											{unitMutations.applied_mutations.length} mutation{unitMutations.applied_mutations.length !== 1 ? 's' : ''} applied
										</div>
									</div>
								</div>

								<!-- Pending mutation choice -->
								{#if unitMutations.pending_choices.length > 0}
									<div class="mb-4">
										<div class="text-xs font-bold text-yellow-400 mb-2 flex items-center gap-1">
											<Sparkles size={12} />
											Mutation Available! Choose one:
										</div>
										<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
											{#each unitMutations.pending_choices as choice}
												<button
													class="text-left p-3 rounded border-2 transition-all hover:scale-[1.02] hover:sf-shimmer {mutTypeColor(choice.mutation_type)} {mutTypeAnimation(choice.mutation_type)}"
													onclick={() => applyMutation(choice.id)}
												>
													<div class="flex items-center justify-between mb-1">
														<span class="text-xs font-bold {mutTypeTextColor(choice.mutation_type)}">
															{mutTypeLabel(choice.mutation_type)}
														</span>
														<span class="text-[10px] text-gray-500">Lv.{choice.level_required}</span>
													</div>
													<div class="text-sm font-bold text-gray-200 mb-1">{choice.name}</div>
													<div class="text-[10px] text-gray-400 mb-2">{choice.description}</div>
													<!-- Stat preview -->
													<div class="flex flex-wrap gap-1 text-[10px]">
														{#if choice.stat_changes.hp_bonus !== 0}
															<span class="px-1 rounded bg-green-900/40 text-green-300">
																HP {choice.stat_changes.hp_bonus > 0 ? '+' : ''}{choice.stat_changes.hp_bonus}
															</span>
														{/if}
														{#if choice.stat_changes.attack_bonus !== 0}
															<span class="px-1 rounded bg-red-900/40 text-red-300">
																ATK {choice.stat_changes.attack_bonus > 0 ? '+' : ''}{choice.stat_changes.attack_bonus}
															</span>
														{/if}
														{#if choice.stat_changes.defense_bonus !== 0}
															<span class="px-1 rounded bg-blue-900/40 text-blue-300">
																DEF {choice.stat_changes.defense_bonus > 0 ? '+' : ''}{choice.stat_changes.defense_bonus}
															</span>
														{/if}
														{#if choice.stat_changes.speed_bonus !== 0}
															<span class="px-1 rounded bg-cyan-900/40 text-cyan-300">
																SPD {choice.stat_changes.speed_bonus > 0 ? '+' : ''}{choice.stat_changes.speed_bonus}
															</span>
														{/if}
														{#if choice.stat_changes.production_bonus !== 0}
															<span class="px-1 rounded bg-amber-900/40 text-amber-300">
																PROD +{(choice.stat_changes.production_bonus * 100).toFixed(0)}%
															</span>
														{/if}
													</div>
													{#if choice.special_ability}
														<div class="mt-1.5 text-[10px] text-purple-300 italic">
															Ability: {choice.special_ability}
														</div>
													{/if}
												</button>
											{/each}
										</div>
									</div>
								{/if}

								<!-- Mutation timeline -->
								<div class="text-xs font-bold text-gray-400 mb-2">Mutation History</div>
								{#if unitMutations.applied_mutations.length === 0 && unitMutations.pending_choices.length === 0}
									<div class="text-xs text-gray-500 italic">
										This unit has no mutations yet. Reach level 5 to unlock the first mutation choice.
									</div>
								{:else}
									<div class="relative pl-6">
										<!-- Vertical line -->
										<div class="absolute left-2.5 top-0 bottom-0 w-px bg-purple-800/40"></div>

										{#each mutationTree as tier, tierIdx}
											{@const tierLevel = tier[0]?.level_required ?? 0}
											{@const applied = unitMutations.applied_mutations.find(a =>
												tier.some(m => m.id === a.mutation_id)
											)}
											{@const chosenMutation = applied ? mutationCatalog[applied.mutation_id] : null}
											{@const isReached = unitMutations.unit_level >= tierLevel}

											<div class="relative mb-4">
												<!-- Dot on timeline -->
												<div class="absolute -left-3.5 top-1 w-3 h-3 rounded-full border-2
													{applied
														? 'bg-purple-500 border-purple-400'
														: isReached
															? 'bg-yellow-500 border-yellow-400 animate-pulse'
															: 'bg-gray-700 border-gray-600'
													}"></div>

												<div class="bg-[#111833] rounded p-2.5 border
													{applied
														? 'border-purple-800/40'
														: isReached
															? 'border-yellow-800/40'
															: 'border-gray-800/30 opacity-50'
													}">
													<div class="flex items-center justify-between mb-1">
														<span class="text-[10px] font-bold text-gray-400">Level {tierLevel}</span>
														{#if applied && chosenMutation}
															<span class="text-[10px] px-1.5 py-0.5 rounded {mutTypeColor(chosenMutation.mutation_type)} {mutTypeTextColor(chosenMutation.mutation_type)}">
																{mutTypeLabel(chosenMutation.mutation_type)}
															</span>
														{:else if isReached}
															<span class="text-[10px] text-yellow-400">PENDING</span>
														{:else}
															<span class="text-[10px] text-gray-600">LOCKED</span>
														{/if}
													</div>
													{#if applied && chosenMutation}
														<div class="text-xs font-bold text-purple-300">{chosenMutation.name}</div>
														<div class="text-[10px] text-gray-400">{chosenMutation.description}</div>
													{:else}
														<div class="text-[10px] text-gray-500 italic">
															{isReached ? 'Choose a mutation above!' : `Reach level ${tierLevel} to unlock`}
														</div>
													{/if}
												</div>
											</div>
										{/each}
									</div>
								{/if}
							{/if}
						</div>
					</div>
				</div>

			<!-- ═══ STATS ═══ -->
			{:else if activeNav === 'stats'}
				<div class="space-y-4 sf-animate-tab-enter">
					<h2 class="text-xl font-bold text-blue-300 sf-section-header px-2 rounded">Colony Statistics</h2>

					<div class="grid grid-cols-2 lg:grid-cols-3 gap-3">
						<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Total Buildings</div>
							<div class="text-2xl font-bold text-blue-300">{planet.buildings.reduce((s, b) => s + b.level, 0)}</div>
						</div>
						<div class="bg-[#111833] rounded p-3 border border-purple-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Total Research</div>
							<div class="text-2xl font-bold text-purple-300">{planet.research.reduce((s, r) => s + r.level, 0)}</div>
						</div>
						<div class="bg-[#111833] rounded p-3 border border-cyan-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Fleet Size</div>
							<div class="text-2xl font-bold text-cyan-300">{planet.fleet.reduce((s, f) => s + f.count, 0)}</div>
						</div>
						<div class="bg-[#111833] rounded p-3 border border-green-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Biomass/h</div>
							<div class="text-2xl font-bold text-green-300">{fmt(res.biomass_per_hour)}</div>
						</div>
						<div class="bg-[#111833] rounded p-3 border border-amber-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Creep Coverage</div>
							<div class="text-2xl font-bold text-green-400">{planet.creep.coverage_percent.toFixed(1)}%</div>
						</div>
						<div class="bg-[#111833] rounded p-3 border border-indigo-900/30">
							<div class="text-[10px] text-gray-500 uppercase">Dark Matter</div>
							<div class="text-2xl font-bold text-indigo-300">{fmt(res.dark_matter)}</div>
						</div>
					</div>

					<!-- Storage bars -->
					<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
						<h3 class="text-sm font-bold text-blue-300 mb-2">Storage Capacity</h3>
						<div class="space-y-2">
							{#each [
								{ label: 'Biomass', current: res.biomass, cap: planet.storage_biomass_cap, color: 'bg-green-500' },
								{ label: 'Minerals', current: res.minerals, cap: planet.storage_minerals_cap, color: 'bg-cyan-500' },
								{ label: 'Crystal', current: res.crystal, cap: planet.storage_crystal_cap, color: 'bg-purple-500' },
								{ label: 'Spore Gas', current: res.spore_gas, cap: planet.storage_spore_gas_cap, color: 'bg-amber-500' },
							] as bar}
								<div>
									<div class="flex justify-between text-[10px] text-gray-400 mb-0.5">
										<span>{bar.label}</span>
										<span>{fmt(bar.current)} / {fmt(bar.cap)}</span>
									</div>
									<div class="h-1.5 bg-gray-800 rounded-full overflow-hidden">
										<div class="{bar.color} h-full rounded-full transition-all" style="width: {Math.min(100, (bar.current / Math.max(1, bar.cap)) * 100)}%"></div>
									</div>
								</div>
							{/each}
						</div>
					</div>

					<!-- Production per resource -->
					<div class="bg-[#111833] rounded p-3 border border-blue-900/30">
						<h3 class="text-sm font-bold text-blue-300 mb-2">Production Rates (per hour)</h3>
						<div class="grid grid-cols-4 gap-3 text-center text-xs">
							<div>
								<div class="text-green-400 font-bold text-lg">{fmt(res.biomass_per_hour)}</div>
								<div class="text-gray-500">Biomass</div>
							</div>
							<div>
								<div class="text-cyan-400 font-bold text-lg">{fmt(res.minerals_per_hour)}</div>
								<div class="text-gray-500">Minerals</div>
							</div>
							<div>
								<div class="text-purple-400 font-bold text-lg">{fmt(res.crystal_per_hour)}</div>
								<div class="text-gray-500">Crystal</div>
							</div>
							<div>
								<div class="text-amber-400 font-bold text-lg">{fmt(res.spore_gas_per_hour)}</div>
								<div class="text-gray-500">Spore Gas</div>
							</div>
						</div>
					</div>
				</div>
			{/if}
		</main>
	</div>
</div>
