<script lang="ts">
	/**
	 * ForgeQuest -- Idle RPG Gamification Dashboard
	 *
	 * A medieval fantasy idle RPG where the user's REAL productivity powers
	 * their character. Writing documents = crafting weapons, running workflows
	 * = fighting monsters, AI queries = casting spells.
	 *
	 * Panels:
	 * - Character Card (name, class, level, HP, stats)
	 * - Equipment Panel (8 slots with item icons)
	 * - Inventory Grid (items with rarity-colored borders)
	 * - Skill Tree (6 branches, click to invest)
	 * - Zone Map (10 zones with level indicators, auto-battle)
	 * - Forge/Crafting (recipe list, craft button)
	 * - Quest Board (active quests with progress bars)
	 * - Battle Log (recent fight results)
	 */

	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import {
		Sword, Shield, Wand2, Hammer, Music, BookOpen,
		Heart, Zap, Star, Package, Map, ScrollText,
		Flame, Trophy, ChevronRight, Swords, Sparkles,
		ArrowUp, ShieldCheck, Brain, Wrench, Crown, GraduationCap
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// Style engine integration
	const widgetId = 'quest-page';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});

	// ── Types ──────────────────────────────────────────────────────────────

	interface ItemStats {
		attack: number;
		defense: number;
		magic: number;
		hp_bonus: number;
	}

	interface Item {
		id: string;
		name: string;
		item_type: string;
		rarity: string;
		stats: ItemStats;
		level_req: number;
		description: string;
	}

	interface Equipment {
		weapon: Item | null;
		head: Item | null;
		chest: Item | null;
		legs: Item | null;
		boots: Item | null;
		gloves: Item | null;
		accessory1: Item | null;
		accessory2: Item | null;
	}

	interface Skill {
		id: string;
		name: string;
		description: string;
		tier: number;
		points_invested: number;
		max_points: number;
		prerequisite: string | null;
		effect: string;
		branch: string;
	}

	interface QuestCharacter {
		name: string;
		class: string;
		level: number;
		xp: number;
		hp: number;
		max_hp: number;
		attack: number;
		defense: number;
		magic: number;
		gold: number;
		inventory: Item[];
		equipped: Equipment;
		skill_points: number;
		skills: Skill[];
		quests_completed: number;
		monsters_slain: number;
		current_zone: string;
		guild: string | null;
	}

	interface Zone {
		id: string;
		name: string;
		description: string;
		level_min: number;
		level_max: number;
		monsters: { name: string; level: number; hp: number }[];
		boss: { name: string; level: number; hp: number } | null;
		unlock_condition: string;
	}

	interface Quest {
		id: string;
		name: string;
		description: string;
		objective: string;
		objective_target: number;
		objective_progress: number;
		reward_xp: number;
		reward_gold: number;
		reward_items: string[];
		completed: boolean;
	}

	interface CraftingRecipe {
		id: string;
		name: string;
		result_item_id: string;
		materials: [string, number][];
		required_level: number;
	}

	interface BattleResult {
		victory: boolean;
		monster_name: string;
		monster_level: number;
		damage_dealt: number;
		damage_taken: number;
		xp_earned: number;
		gold_earned: number;
		loot: Item[];
		rounds: number;
	}

	interface ActionResult {
		xp_earned: number;
		gold_earned: number;
		material_gained: string | null;
		level_up: boolean;
		new_level: number;
		battle: BattleResult | null;
		quest_completed: string | null;
	}

	// ── State ──────────────────────────────────────────────────────────────

	let character = $state<QuestCharacter | null>(null);
	let zones = $state<Zone[]>([]);
	let quests = $state<Quest[]>([]);
	let recipes = $state<CraftingRecipe[]>([]);
	let battleLog = $state<BattleResult[]>([]);
	let hasCharacter = $state(false);
	let isCreating = $state(false);
	let isBattling = $state(false);
	let isCrafting = $state(false);

	// Creation form
	let newName = $state('');
	let newClass = $state('warrior');

	// UI tabs
	let activeTab = $state<'character' | 'skills' | 'zones' | 'quests' | 'crafting'>('character');
	let selectedZone = $state<string>('beginners_meadow');
	let selectedSkillBranch = $state<string>('combat');

	// Tooltip
	let tooltipItem = $state<Item | null>(null);

	// XP progress
	let xpForNext = $derived(character ? Math.floor(150 * Math.pow(character.level + 1, 1.6)) : 150);
	let xpForCurrent = $derived(character ? Math.floor(150 * Math.pow(character.level, 1.6)) : 0);
	let xpProgress = $derived(
		character ? Math.min(100, ((character.xp - xpForCurrent) / Math.max(1, xpForNext - xpForCurrent)) * 100) : 0
	);
	let hpPercent = $derived(character ? (character.hp / Math.max(1, character.max_hp)) * 100 : 100);

	// Filtered skills by branch
	let branchSkills = $derived(
		character ? character.skills.filter((s) => s.branch === selectedSkillBranch) : []
	);

	// Active (incomplete) quests
	let activeQuests = $derived(quests.filter((q) => !q.completed));
	let completedQuests = $derived(quests.filter((q) => q.completed));

	// Inventory split: equipable vs materials
	let equipableItems = $derived(
		character ? character.inventory.filter((i) => i.item_type !== 'material') : []
	);
	let materialItems = $derived(
		character ? character.inventory.filter((i) => i.item_type === 'material') : []
	);

	// ── Rarity colors ──────────────────────────────────────────────────────

	const RARITY_COLORS: Record<string, string> = {
		common: 'border-zinc-500 text-zinc-300',
		uncommon: 'border-green-500 text-green-400',
		rare: 'border-blue-500 text-blue-400',
		epic: 'border-purple-500 text-purple-400',
		legendary: 'border-amber-500 text-amber-400',
		mythic: 'border-rose-500 text-rose-400',
	};

	const RARITY_BG: Record<string, string> = {
		common: 'bg-zinc-500/10',
		uncommon: 'bg-green-500/10',
		rare: 'bg-blue-500/10',
		epic: 'bg-purple-500/10',
		legendary: 'bg-amber-500/10',
		mythic: 'bg-rose-500/10',
	};

	const CLASS_ICONS: Record<string, typeof Sword> = {
		warrior: Sword,
		mage: Wand2,
		ranger: ChevronRight,
		blacksmith: Hammer,
		bard: Music,
		scholar: BookOpen,
	};

	const CLASS_LABELS: Record<string, string> = {
		warrior: 'Warrior',
		mage: 'Mage',
		ranger: 'Ranger',
		blacksmith: 'Blacksmith',
		bard: 'Bard',
		scholar: 'Scholar',
	};

	const BRANCH_ICONS: Record<string, typeof Sword> = {
		combat: Swords,
		defense: ShieldCheck,
		magic: Brain,
		crafting: Wrench,
		leadership: Crown,
		wisdom: GraduationCap,
	};

	const BRANCH_LABELS: Record<string, string> = {
		combat: 'Combat',
		defense: 'Defense',
		magic: 'Magic',
		crafting: 'Crafting',
		leadership: 'Leadership',
		wisdom: 'Wisdom',
	};

	// ── Data loading ───────────────────────────────────────────────────────

	async function loadCharacter() {
		try {
			character = await invoke<QuestCharacter>('quest_get_character');
			hasCharacter = true;
		} catch {
			hasCharacter = false;
			character = null;
		}
	}

	async function loadZones() {
		try {
			zones = await invoke<Zone[]>('quest_get_zones');
		} catch {
			zones = [];
		}
	}

	async function loadQuests() {
		try {
			quests = await invoke<Quest[]>('quest_get_quests');
		} catch {
			quests = [];
		}
	}

	async function loadRecipes() {
		try {
			recipes = await invoke<CraftingRecipe[]>('quest_get_recipes');
		} catch {
			recipes = [];
		}
	}

	// ── Actions ────────────────────────────────────────────────────────────

	async function createCharacter() {
		if (!newName.trim()) return;
		isCreating = true;
		try {
			character = await invoke<QuestCharacter>('quest_create_character', {
				name: newName.trim(),
				class: newClass,
			});
			hasCharacter = true;
			await loadQuests();
		} catch (e) {
			console.error('Failed to create character:', e);
		} finally {
			isCreating = false;
		}
	}

	async function autoBattle(zoneId: string) {
		isBattling = true;
		try {
			const result = await invoke<BattleResult>('quest_auto_battle', { zoneId });
			battleLog = [result, ...battleLog.slice(0, 19)];
			await loadCharacter();
			await loadQuests();
		} catch (e) {
			console.error('Battle failed:', e);
		} finally {
			isBattling = false;
		}
	}

	async function craftItem(recipeId: string) {
		isCrafting = true;
		try {
			await invoke<Item>('quest_craft_item', { recipeId });
			await loadCharacter();
		} catch (e) {
			console.error('Crafting failed:', e);
		} finally {
			isCrafting = false;
		}
	}

	async function equipItem(itemId: string, slot: string) {
		try {
			await invoke('quest_equip_item', { itemId, slot });
			await loadCharacter();
		} catch (e) {
			console.error('Equip failed:', e);
		}
	}

	async function unequipSlot(slot: string) {
		try {
			await invoke('quest_unequip', { slot });
			await loadCharacter();
		} catch (e) {
			console.error('Unequip failed:', e);
		}
	}

	async function investSkill(skillId: string) {
		try {
			await invoke<Skill>('quest_invest_skill', { skillId });
			await loadCharacter();
		} catch (e) {
			console.error('Skill invest failed:', e);
		}
	}

	// ── Mount ──────────────────────────────────────────────────────────────

	onMount(async () => {
		await loadCharacter();
		await loadZones();
		await loadQuests();
		await loadRecipes();
	});
</script>

<!-- ══════════════════════════════════════════════════════════════════════════
     CHARACTER CREATION (shown when no character exists)
     ══════════════════════════════════════════════════════════════════════ -->
{#if !hasCharacter}
	<div class="flex h-full items-center justify-center bg-gx-bg-primary p-8">
		<div class="w-full max-w-lg rounded-xl border border-gx-border bg-gx-bg-secondary p-8 shadow-2xl">
			<div class="mb-6 text-center">
				<div class="mb-2 flex items-center justify-center gap-2">
					<Swords size={28} class="text-gx-accent-cyan" />
					<h1 class="text-2xl font-bold text-gx-text-primary">ForgeQuest</h1>
				</div>
				<p class="text-sm text-gx-text-muted">Your productivity fuels your adventure.</p>
			</div>

			<div class="mb-4">
				<label for="hero-name" class="mb-1 block text-xs font-medium text-gx-text-secondary">Hero Name</label>
				<input
					id="hero-name"
					type="text"
					bind:value={newName}
					placeholder="Enter your hero's name..."
					maxlength="24"
					class="w-full rounded-lg border border-gx-border bg-gx-bg-primary px-3 py-2 text-sm text-gx-text-primary
					       placeholder:text-gx-text-muted focus:border-gx-accent-cyan focus:outline-none"
				/>
			</div>

			<div class="mb-6">
				<label class="mb-2 block text-xs font-medium text-gx-text-secondary">Choose Your Class</label>
				<div class="grid grid-cols-3 gap-2">
					{#each Object.entries(CLASS_LABELS) as [cls, label]}
						{@const Icon = CLASS_ICONS[cls]}
						<button
							onclick={() => (newClass = cls)}
							class="flex flex-col items-center gap-1 rounded-lg border p-3 text-xs transition-all
							       {newClass === cls
								? 'border-gx-accent-cyan bg-gx-accent-cyan/10 text-gx-accent-cyan'
								: 'border-gx-border bg-gx-bg-primary text-gx-text-secondary hover:border-gx-text-muted'}"
						>
							<Icon size={20} />
							<span class="font-medium">{label}</span>
						</button>
					{/each}
				</div>
			</div>

			<button
				onclick={createCharacter}
				disabled={isCreating || !newName.trim()}
				class="w-full rounded-lg bg-gx-accent-cyan px-4 py-2.5 text-sm font-semibold text-black
				       transition-all hover:bg-gx-accent-cyan/80 disabled:opacity-40"
			>
				{isCreating ? 'Forging Hero...' : 'Begin Your Quest'}
			</button>
		</div>
	</div>

<!-- ══════════════════════════════════════════════════════════════════════════
     MAIN QUEST DASHBOARD (shown when character exists)
     ══════════════════════════════════════════════════════════════════════ -->
{:else if character}
	<div class="flex h-full flex-col bg-gx-bg-primary">
		<!-- ── Top Bar: Character Summary ──────────────────────────────────── -->
		<div class="flex items-center gap-4 border-b border-gx-border bg-gx-bg-secondary px-4 py-2">
			<!-- Class icon + Name -->
			<div class="flex items-center gap-2">
				{@const ClassIcon = CLASS_ICONS[character.class] ?? Sword}
				<div class="flex h-8 w-8 items-center justify-center rounded-full border border-gx-accent-cyan bg-gx-accent-cyan/10">
					<ClassIcon size={16} class="text-gx-accent-cyan" />
				</div>
				<div>
					<span class="text-sm font-bold text-gx-text-primary">{character.name}</span>
					<span class="ml-1 text-xs text-gx-text-muted">Lv.{character.level} {CLASS_LABELS[character.class] ?? 'Warrior'}</span>
				</div>
			</div>

			<!-- HP Bar -->
			<div class="flex items-center gap-1.5">
				<Heart size={12} class="text-red-400" />
				<div class="h-2 w-24 overflow-hidden rounded-full bg-gx-bg-primary">
					<div
						class="h-full rounded-full transition-all {hpPercent > 50 ? 'bg-green-500' : hpPercent > 25 ? 'bg-yellow-500' : 'bg-red-500'}"
						style="width: {hpPercent}%"
					></div>
				</div>
				<span class="text-[10px] text-gx-text-muted">{character.hp}/{character.max_hp}</span>
			</div>

			<!-- XP Bar -->
			<div class="flex items-center gap-1.5">
				<Star size={12} class="text-amber-400" />
				<div class="h-2 w-28 overflow-hidden rounded-full bg-gx-bg-primary">
					<div class="h-full rounded-full bg-amber-500 transition-all" style="width: {xpProgress}%"></div>
				</div>
				<span class="text-[10px] text-gx-text-muted">{character.xp}/{xpForNext} XP</span>
			</div>

			<!-- Stats -->
			<div class="ml-auto flex items-center gap-3 text-[10px]">
				<span class="flex items-center gap-1 text-red-400" title="Attack">
					<Sword size={10} /> {character.attack}
				</span>
				<span class="flex items-center gap-1 text-blue-400" title="Defense">
					<Shield size={10} /> {character.defense}
				</span>
				<span class="flex items-center gap-1 text-purple-400" title="Magic">
					<Wand2 size={10} /> {character.magic}
				</span>
				<span class="flex items-center gap-1 text-amber-400" title="Gold">
					<Sparkles size={10} /> {character.gold}
				</span>
				<span class="flex items-center gap-1 text-gx-text-muted" title="Monsters Slain">
					<Swords size={10} /> {character.monsters_slain}
				</span>
			</div>
		</div>

		<!-- ── Tab Navigation ──────────────────────────────────────────────── -->
		<div class="flex border-b border-gx-border bg-gx-bg-secondary/50 px-4">
			{#each [
				{ id: 'character', label: 'Character', icon: Sword },
				{ id: 'skills', label: 'Skills', icon: Zap },
				{ id: 'zones', label: 'Zones', icon: Map },
				{ id: 'quests', label: 'Quests', icon: ScrollText },
				{ id: 'crafting', label: 'Forge', icon: Hammer },
			] as tab}
				<button
					onclick={() => (activeTab = tab.id as typeof activeTab)}
					class="flex items-center gap-1.5 border-b-2 px-3 py-2 text-xs font-medium transition-all
					       {activeTab === tab.id
						? 'border-gx-accent-cyan text-gx-accent-cyan'
						: 'border-transparent text-gx-text-muted hover:text-gx-text-secondary'}"
				>
					<tab.icon size={13} />
					{tab.label}
					{#if tab.id === 'skills' && character.skill_points > 0}
						<span class="ml-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-gx-accent-cyan text-[9px] font-bold text-black">
							{character.skill_points}
						</span>
					{/if}
					{#if tab.id === 'quests'}
						<span class="ml-0.5 text-[9px] text-gx-text-muted">({activeQuests.length})</span>
					{/if}
				</button>
			{/each}
		</div>

		<!-- ── Tab Content ─────────────────────────────────────────────────── -->
		<div class="flex-1 overflow-y-auto p-4">

			<!-- ────────────────────── CHARACTER TAB ────────────────────── -->
			{#if activeTab === 'character'}
				<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
					<!-- Equipment Panel -->
					<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-4">
						<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">Equipment</h3>
						<div class="grid grid-cols-2 gap-2">
							{#each [
								{ slot: 'weapon', label: 'Weapon', item: character.equipped.weapon },
								{ slot: 'head', label: 'Head', item: character.equipped.head },
								{ slot: 'chest', label: 'Chest', item: character.equipped.chest },
								{ slot: 'legs', label: 'Legs', item: character.equipped.legs },
								{ slot: 'boots', label: 'Boots', item: character.equipped.boots },
								{ slot: 'gloves', label: 'Gloves', item: character.equipped.gloves },
								{ slot: 'accessory1', label: 'Accessory 1', item: character.equipped.accessory1 },
								{ slot: 'accessory2', label: 'Accessory 2', item: character.equipped.accessory2 },
							] as eq}
								<div
									class="flex items-center gap-2 rounded-md border p-2 text-xs
									       {eq.item ? RARITY_COLORS[eq.item.rarity] ?? 'border-gx-border' : 'border-gx-border border-dashed'}"
								>
									<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded bg-gx-bg-primary text-[10px] text-gx-text-muted">
										{eq.item ? eq.item.name.slice(0, 2).toUpperCase() : eq.label.slice(0, 2)}
									</div>
									<div class="min-w-0 flex-1">
										{#if eq.item}
											<div class="truncate font-medium">{eq.item.name}</div>
											<div class="text-[9px] text-gx-text-muted">
												{#if eq.item.stats.attack > 0}+{eq.item.stats.attack} ATK{/if}
												{#if eq.item.stats.defense > 0} +{eq.item.stats.defense} DEF{/if}
												{#if eq.item.stats.magic > 0} +{eq.item.stats.magic} MAG{/if}
											</div>
										{:else}
											<div class="text-gx-text-muted">{eq.label}</div>
											<div class="text-[9px] text-gx-text-muted">Empty</div>
										{/if}
									</div>
									{#if eq.item}
										<button
											onclick={() => unequipSlot(eq.slot)}
											class="text-[9px] text-gx-text-muted hover:text-red-400"
											title="Unequip"
										>X</button>
									{/if}
								</div>
							{/each}
						</div>
					</div>

					<!-- Inventory -->
					<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-4">
						<h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">
							Inventory ({character.inventory.length} items)
						</h3>

						{#if equipableItems.length > 0}
							<div class="mb-3">
								<div class="mb-1 text-[9px] font-medium text-gx-text-muted">EQUIPABLE</div>
								<div class="grid grid-cols-3 gap-1.5 sm:grid-cols-4">
									{#each equipableItems as item}
										<button
											onclick={() => {
												const slot = item.item_type === 'weapon' ? 'weapon' : item.item_type === 'armor' ? 'chest' : 'accessory1';
												equipItem(item.id, slot);
											}}
											onmouseenter={() => (tooltipItem = item)}
											onmouseleave={() => (tooltipItem = null)}
											class="flex flex-col items-center gap-0.5 rounded border p-1.5 text-[9px] transition-all hover:bg-gx-bg-primary
											       {RARITY_COLORS[item.rarity] ?? 'border-gx-border'} {RARITY_BG[item.rarity] ?? ''}"
										>
											<span class="truncate font-medium">{item.name}</span>
											<span class="text-gx-text-muted">{item.rarity}</span>
										</button>
									{/each}
								</div>
							</div>
						{/if}

						{#if materialItems.length > 0}
							<div>
								<div class="mb-1 text-[9px] font-medium text-gx-text-muted">MATERIALS</div>
								<div class="flex flex-wrap gap-1">
									{#each materialItems as mat}
										<span class="rounded bg-gx-bg-primary px-1.5 py-0.5 text-[9px] text-gx-text-secondary">
											{mat.name}
										</span>
									{/each}
								</div>
							</div>
						{/if}

						{#if character.inventory.length === 0}
							<p class="text-center text-xs text-gx-text-muted">No items yet. Complete actions to earn loot.</p>
						{/if}
					</div>
				</div>

			<!-- ────────────────────── SKILLS TAB ───────────────────────── -->
			{:else if activeTab === 'skills'}
				<div class="grid grid-cols-1 gap-4 lg:grid-cols-[200px_1fr]">
					<!-- Branch selector -->
					<div class="flex flex-col gap-1">
						{#each Object.entries(BRANCH_LABELS) as [branch, label]}
							{@const BIcon = BRANCH_ICONS[branch]}
							<button
								onclick={() => (selectedSkillBranch = branch)}
								class="flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-medium transition-all
								       {selectedSkillBranch === branch
									? 'bg-gx-accent-cyan/10 text-gx-accent-cyan'
									: 'text-gx-text-muted hover:bg-gx-bg-secondary hover:text-gx-text-secondary'}"
							>
								<BIcon size={14} />
								{label}
							</button>
						{/each}
						{#if character.skill_points > 0}
							<div class="mt-2 rounded-lg bg-gx-accent-cyan/10 px-3 py-2 text-center text-xs font-semibold text-gx-accent-cyan">
								{character.skill_points} skill points available
							</div>
						{/if}
					</div>

					<!-- Skill list for branch -->
					<div class="flex flex-col gap-2">
						{#each branchSkills as skill}
							<div class="flex items-center gap-3 rounded-lg border border-gx-border bg-gx-bg-secondary p-3">
								<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-gx-bg-primary">
									<span class="text-lg font-bold text-gx-text-muted">T{skill.tier}</span>
								</div>
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-2">
										<span class="text-sm font-semibold text-gx-text-primary">{skill.name}</span>
										<span class="text-[9px] text-gx-text-muted">
											{skill.points_invested}/{skill.max_points}
										</span>
									</div>
									<p class="text-[10px] text-gx-text-muted">{skill.description}</p>
									<p class="text-[10px] text-gx-accent-cyan">{skill.effect}</p>
									<!-- Progress bar -->
									<div class="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-gx-bg-primary">
										<div
											class="h-full rounded-full bg-gx-accent-cyan transition-all"
											style="width: {(skill.points_invested / Math.max(1, skill.max_points)) * 100}%"
										></div>
									</div>
								</div>
								<button
									onclick={() => investSkill(skill.id)}
									disabled={character.skill_points === 0 || skill.points_invested >= skill.max_points}
									class="shrink-0 rounded-lg bg-gx-accent-cyan/10 px-3 py-1.5 text-[10px] font-semibold
									       text-gx-accent-cyan transition-all hover:bg-gx-accent-cyan/20
									       disabled:cursor-not-allowed disabled:opacity-30"
								>
									<ArrowUp size={12} />
								</button>
							</div>
						{/each}

						{#if branchSkills.length === 0}
							<p class="py-8 text-center text-xs text-gx-text-muted">No skills in this branch yet.</p>
						{/if}
					</div>
				</div>

			<!-- ────────────────────── ZONES TAB ────────────────────────── -->
			{:else if activeTab === 'zones'}
				<div class="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_300px]">
					<!-- Zone list -->
					<div class="flex flex-col gap-2">
						{#each zones as zone}
							{@const locked = character.level < zone.level_min}
							<button
								onclick={() => { if (!locked) selectedZone = zone.id; }}
								disabled={locked}
								class="flex items-start gap-3 rounded-lg border p-3 text-left transition-all
								       {selectedZone === zone.id
									? 'border-gx-accent-cyan bg-gx-accent-cyan/5'
									: locked
										? 'border-gx-border/50 opacity-40'
										: 'border-gx-border bg-gx-bg-secondary hover:border-gx-text-muted'}"
							>
								<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-gx-bg-primary">
									<Map size={16} class="{locked ? 'text-gx-text-muted' : 'text-gx-accent-cyan'}" />
								</div>
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-2">
										<span class="text-sm font-semibold text-gx-text-primary">{zone.name}</span>
										<span class="text-[9px] text-gx-text-muted">Lv.{zone.level_min}-{zone.level_max}</span>
									</div>
									<p class="text-[10px] text-gx-text-muted">{zone.description}</p>
									<div class="mt-1 flex flex-wrap gap-1">
										{#each zone.monsters.slice(0, 3) as mon}
											<span class="rounded bg-gx-bg-primary px-1 py-0.5 text-[8px] text-gx-text-secondary">
												{mon.name} Lv.{mon.level}
											</span>
										{/each}
										{#if zone.boss}
											<span class="rounded bg-red-500/10 px-1 py-0.5 text-[8px] text-red-400">
												Boss: {zone.boss.name}
											</span>
										{/if}
									</div>
								</div>
							</button>
						{/each}
					</div>

					<!-- Battle panel + log -->
					<div class="flex flex-col gap-3">
						<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-4">
							<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">Battle</h3>
							{@const currentZone = zones.find((z) => z.id === selectedZone)}
							{#if currentZone}
								<p class="mb-3 text-sm font-medium text-gx-text-primary">{currentZone.name}</p>
								<button
									onclick={() => autoBattle(selectedZone)}
									disabled={isBattling}
									class="w-full rounded-lg bg-red-500/80 px-4 py-2 text-sm font-semibold text-white
									       transition-all hover:bg-red-500 disabled:opacity-40"
								>
									{isBattling ? 'Fighting...' : 'Fight!'}
								</button>
							{:else}
								<p class="text-xs text-gx-text-muted">Select a zone to battle.</p>
							{/if}
						</div>

						<!-- Battle Log -->
						<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-4">
							<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">
								Battle Log ({battleLog.length})
							</h3>
							<div class="flex max-h-64 flex-col gap-1.5 overflow-y-auto">
								{#each battleLog as entry}
									<div class="rounded border border-gx-border/50 bg-gx-bg-primary p-2 text-[10px]">
										<div class="flex items-center gap-1">
											{#if entry.victory}
												<Trophy size={10} class="text-amber-400" />
												<span class="font-medium text-green-400">Victory</span>
											{:else}
												<Shield size={10} class="text-red-400" />
												<span class="font-medium text-red-400">Defeat</span>
											{/if}
											<span class="text-gx-text-muted">vs {entry.monster_name} Lv.{entry.monster_level}</span>
										</div>
										<div class="mt-0.5 flex gap-2 text-gx-text-muted">
											<span>{entry.rounds} rounds</span>
											<span>+{entry.xp_earned} XP</span>
											<span>+{entry.gold_earned} gold</span>
										</div>
										{#if entry.loot.length > 0}
											<div class="mt-0.5 flex flex-wrap gap-1">
												{#each entry.loot as lootItem}
													<span class="rounded bg-gx-accent-cyan/10 px-1 text-gx-accent-cyan">
														{lootItem.name}
													</span>
												{/each}
											</div>
										{/if}
									</div>
								{/each}

								{#if battleLog.length === 0}
									<p class="py-4 text-center text-[10px] text-gx-text-muted">No battles yet.</p>
								{/if}
							</div>
						</div>
					</div>
				</div>

			<!-- ────────────────────── QUESTS TAB ───────────────────────── -->
			{:else if activeTab === 'quests'}
				<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
					<!-- Active Quests -->
					<div>
						<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">
							Active Quests ({activeQuests.length})
						</h3>
						<div class="flex flex-col gap-2">
							{#each activeQuests as quest}
								{@const progress = Math.min(100, (quest.objective_progress / Math.max(1, quest.objective_target)) * 100)}
								<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-3">
									<div class="flex items-start justify-between gap-2">
										<div>
											<span class="text-sm font-semibold text-gx-text-primary">{quest.name}</span>
											<p class="text-[10px] text-gx-text-muted">{quest.description}</p>
										</div>
										<div class="shrink-0 text-right text-[9px]">
											<div class="text-amber-400">+{quest.reward_xp} XP</div>
											<div class="text-gx-text-muted">+{quest.reward_gold} gold</div>
										</div>
									</div>
									<div class="mt-2 flex items-center gap-2">
										<div class="h-1.5 flex-1 overflow-hidden rounded-full bg-gx-bg-primary">
											<div
												class="h-full rounded-full bg-gx-accent-cyan transition-all"
												style="width: {progress}%"
											></div>
										</div>
										<span class="text-[9px] text-gx-text-muted">
											{quest.objective_progress}/{quest.objective_target}
										</span>
									</div>
								</div>
							{/each}

							{#if activeQuests.length === 0}
								<p class="py-6 text-center text-xs text-gx-text-muted">All quests completed. Check back later.</p>
							{/if}
						</div>
					</div>

					<!-- Completed Quests -->
					<div>
						<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-gx-text-muted">
							Completed ({completedQuests.length})
						</h3>
						<div class="flex flex-col gap-1.5">
							{#each completedQuests as quest}
								<div class="flex items-center gap-2 rounded-lg border border-green-500/20 bg-green-500/5 p-2 text-xs">
									<Trophy size={12} class="text-green-400" />
									<span class="font-medium text-gx-text-secondary">{quest.name}</span>
									<span class="ml-auto text-[9px] text-gx-text-muted">+{quest.reward_xp} XP</span>
								</div>
							{/each}

							{#if completedQuests.length === 0}
								<p class="py-4 text-center text-[10px] text-gx-text-muted">No quests completed yet.</p>
							{/if}
						</div>
					</div>
				</div>

			<!-- ────────────────────── CRAFTING TAB ─────────────────────── -->
			{:else if activeTab === 'crafting'}
				<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
					{#each recipes as recipe}
						{@const canCraft = character.level >= recipe.required_level}
						<div class="rounded-lg border border-gx-border bg-gx-bg-secondary p-3 {!canCraft ? 'opacity-50' : ''}">
							<div class="flex items-center gap-2">
								<Hammer size={14} class="text-amber-400" />
								<span class="text-sm font-semibold text-gx-text-primary">{recipe.name}</span>
							</div>
							<div class="mt-2 text-[10px] text-gx-text-muted">
								<div>Required Level: {recipe.required_level}</div>
								<div class="mt-1">Materials:</div>
								<ul class="mt-0.5 space-y-0.5">
									{#each recipe.materials as [material, count]}
										{@const owned = materialItems.filter((m) => m.name === material).length}
										<li class="{owned >= count ? 'text-green-400' : 'text-red-400'}">
											{material} x{count} (have: {owned})
										</li>
									{/each}
								</ul>
							</div>
							<button
								onclick={() => craftItem(recipe.id)}
								disabled={isCrafting || !canCraft}
								class="mt-2 w-full rounded-lg bg-amber-500/20 px-3 py-1.5 text-[10px] font-semibold
								       text-amber-400 transition-all hover:bg-amber-500/30
								       disabled:cursor-not-allowed disabled:opacity-30"
							>
								{isCrafting ? 'Crafting...' : 'Craft'}
							</button>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{/if}

<!-- ── Item Tooltip ─────────────────────────────────────────────────────── -->
{#if tooltipItem}
	<div class="pointer-events-none fixed bottom-4 right-4 z-50 w-48 rounded-lg border
	            {RARITY_COLORS[tooltipItem.rarity] ?? 'border-gx-border'}
	            bg-gx-bg-secondary p-3 shadow-xl">
		<div class="text-sm font-semibold {RARITY_COLORS[tooltipItem.rarity]?.split(' ')[1] ?? 'text-gx-text-primary'}">
			{tooltipItem.name}
		</div>
		<div class="text-[9px] capitalize text-gx-text-muted">{tooltipItem.rarity} {tooltipItem.item_type}</div>
		<div class="mt-1 space-y-0.5 text-[9px] text-gx-text-secondary">
			{#if tooltipItem.stats.attack > 0}<div>+{tooltipItem.stats.attack} Attack</div>{/if}
			{#if tooltipItem.stats.defense > 0}<div>+{tooltipItem.stats.defense} Defense</div>{/if}
			{#if tooltipItem.stats.magic > 0}<div>+{tooltipItem.stats.magic} Magic</div>{/if}
			{#if tooltipItem.stats.hp_bonus > 0}<div>+{tooltipItem.stats.hp_bonus} HP</div>{/if}
		</div>
		{#if tooltipItem.level_req > 1}
			<div class="mt-1 text-[8px] text-gx-text-muted">Requires Level {tooltipItem.level_req}</div>
		{/if}
		<p class="mt-1 text-[8px] italic text-gx-text-muted">{tooltipItem.description}</p>
	</div>
{/if}
