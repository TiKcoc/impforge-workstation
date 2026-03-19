<script lang="ts">
	/**
	 * Achievements Page — Full gamification dashboard
	 *
	 * Shows all 30 achievements in a filterable grid with:
	 * - Level display + XP progress bar
	 * - Streak counter with flame animation
	 * - Category filter tabs
	 * - Rarity filter (Common -> Legendary)
	 * - Locked/unlocked visual states
	 * - Progress bars on partially completed achievements
	 */

	import { onMount } from 'svelte';
	import {
		achievementStore,
		CATEGORY_LABELS,
		RARITY_LABELS,
		RARITY_COLORS,
		RARITY_BG_COLORS,
		type AchievementCategory,
		type Rarity,
		type Achievement,
	} from '$lib/stores/achievements.svelte';
	import { Trophy, Flame, Star, Lock, Filter, TrendingUp, Zap } from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// Style engine
	const widgetId = 'achievements-page';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});

	// Filters
	let selectedCategory = $state<AchievementCategory | 'all'>('all');
	let selectedRarity = $state<Rarity | 'all'>('all');

	const categories: (AchievementCategory | 'all')[] = [
		'all',
		'getting_started',
		'productivity',
		'ai_master',
		'team_player',
		'power_user',
		'creator',
		'explorer',
	];

	const rarities: (Rarity | 'all')[] = ['all', 'common', 'uncommon', 'rare', 'epic', 'legendary'];

	let filteredAchievements = $derived(() => {
		let list = achievementStore.achievements;
		if (selectedCategory !== 'all') {
			list = list.filter((a) => a.category === selectedCategory);
		}
		if (selectedRarity !== 'all') {
			list = list.filter((a) => a.rarity === selectedRarity);
		}
		// Sort: unlocked first, then by progress percentage desc
		return [...list].sort((a, b) => {
			if (a.unlocked !== b.unlocked) return a.unlocked ? -1 : 1;
			const aP = a.target > 0 ? a.progress / a.target : 0;
			const bP = b.target > 0 ? b.progress / b.target : 0;
			return bP - aP;
		});
	});

	// Level stats
	let level = $derived(achievementStore.progress.level);
	let totalXp = $derived(achievementStore.progress.total_xp);
	let xpProgress = $derived(achievementStore.xpProgress);
	let streak = $derived(achievementStore.progress.current_streak);
	let longestStreak = $derived(achievementStore.progress.longest_streak);
	let unlockedCount = $derived(achievementStore.unlockedCount);
	let totalCount = $derived(achievementStore.totalCount);

	function rarityBorderGlow(rarity: Rarity, unlocked: boolean): string {
		if (!unlocked) return 'border-gx-border-default opacity-60';
		return RARITY_BG_COLORS[rarity];
	}

	function progressPercent(a: Achievement): number {
		if (a.target <= 0) return 0;
		return Math.min(100, Math.round((a.progress / a.target) * 100));
	}

	function rarityGradient(rarity: Rarity): string {
		switch (rarity) {
			case 'common':
				return 'from-gray-500/20 to-gray-600/5';
			case 'uncommon':
				return 'from-green-500/20 to-green-600/5';
			case 'rare':
				return 'from-blue-500/20 to-blue-600/5';
			case 'epic':
				return 'from-purple-500/20 to-purple-600/5';
			case 'legendary':
				return 'from-amber-500/20 to-amber-600/5';
		}
	}

	onMount(() => {
		achievementStore.load();
		// Track module usage
		achievementStore.trackAction('module:achievements');
	});
</script>

<div class="h-full overflow-y-auto p-6 space-y-6">
	<!-- Header: Level + Stats Row -->
	<div class="flex flex-wrap items-start gap-4">
		<!-- Level Card -->
		<div class="flex-1 min-w-[280px] rounded-xl border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-4">
				<!-- Level badge -->
				<div class="relative">
					<div class="w-16 h-16 rounded-full border-2 border-amber-500/50 bg-gradient-to-br from-amber-500/20 to-amber-600/5 flex items-center justify-center">
						<span class="text-2xl font-bold text-amber-400">{level}</span>
					</div>
					<div class="absolute -bottom-1 left-1/2 -translate-x-1/2 px-2 py-0.5 rounded-full bg-gx-bg-elevated border border-gx-border-default text-[9px] font-bold text-gx-text-muted uppercase whitespace-nowrap">
						Level
					</div>
				</div>

				<div class="flex-1 min-w-0">
					<div class="flex items-center justify-between mb-1">
						<span class="text-sm font-semibold text-gx-text-primary">
							{totalXp.toLocaleString()} XP
						</span>
						<span class="text-[10px] text-gx-text-muted">
							Next: {achievementStore.progress.xp_for_next_level.toLocaleString()} XP
						</span>
					</div>
					<!-- XP Progress Bar -->
					<div class="h-2.5 rounded-full bg-gx-bg-tertiary overflow-hidden">
						<div
							class="h-full rounded-full bg-gradient-to-r from-amber-500 to-amber-400 transition-all duration-700"
							style="width: {xpProgress}%"
						></div>
					</div>
					<div class="flex items-center justify-between mt-1.5">
						<span class="text-[10px] text-gx-text-muted">
							{unlockedCount}/{totalCount} achievements
						</span>
						<span class="text-[10px] text-gx-text-muted">
							{xpProgress}%
						</span>
					</div>
				</div>
			</div>
		</div>

		<!-- Streak Card -->
		<div class="min-w-[180px] rounded-xl border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-3">
				<div class="w-12 h-12 rounded-full bg-orange-500/10 border border-orange-500/20 flex items-center justify-center">
					{#if streak > 0}
						<Flame size={24} class="text-orange-400 {streak >= 7 ? 'animate-pulse' : ''}" />
					{:else}
						<Flame size={24} class="text-gx-text-disabled" />
					{/if}
				</div>
				<div>
					<div class="text-2xl font-bold text-gx-text-primary">{streak}</div>
					<div class="text-[10px] text-gx-text-muted uppercase tracking-wider">Day Streak</div>
				</div>
			</div>
			<div class="mt-2 text-[10px] text-gx-text-muted">
				Best: {longestStreak} days
			</div>
		</div>

		<!-- Quick Stats Card -->
		<div class="min-w-[200px] rounded-xl border border-gx-border-default bg-gx-bg-secondary p-5">
			<div class="flex items-center gap-2 mb-3">
				<TrendingUp size={14} class="text-gx-neon" />
				<span class="text-xs font-medium text-gx-text-secondary">Statistics</span>
			</div>
			<div class="space-y-1.5">
				<div class="flex justify-between text-[11px]">
					<span class="text-gx-text-muted">AI Queries</span>
					<span class="text-gx-text-secondary font-mono">{achievementStore.progress.ai_queries.toLocaleString()}</span>
				</div>
				<div class="flex justify-between text-[11px]">
					<span class="text-gx-text-muted">Documents</span>
					<span class="text-gx-text-secondary font-mono">{achievementStore.progress.documents_created.toLocaleString()}</span>
				</div>
				<div class="flex justify-between text-[11px]">
					<span class="text-gx-text-muted">Workflows</span>
					<span class="text-gx-text-secondary font-mono">{achievementStore.progress.workflows_run.toLocaleString()}</span>
				</div>
				<div class="flex justify-between text-[11px]">
					<span class="text-gx-text-muted">Modules Used</span>
					<span class="text-gx-text-secondary font-mono">{achievementStore.progress.modules_used.length}</span>
				</div>
			</div>
		</div>
	</div>

	<!-- Filter Bar -->
	<div class="flex flex-wrap items-center gap-3">
		<!-- Category Tabs -->
		<div class="flex items-center gap-1 bg-gx-bg-secondary rounded-lg p-1 border border-gx-border-default">
			{#each categories as cat (cat)}
				<button
					onclick={() => selectedCategory = cat}
					class="px-2.5 py-1.5 text-[11px] rounded-md transition-all whitespace-nowrap
						{selectedCategory === cat
							? 'bg-gx-neon/15 text-gx-neon font-medium'
							: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
				>
					{cat === 'all' ? 'All' : CATEGORY_LABELS[cat]}
				</button>
			{/each}
		</div>

		<!-- Rarity Filter -->
		<div class="flex items-center gap-1 ml-auto">
			<Filter size={12} class="text-gx-text-muted mr-1" />
			{#each rarities as r (r)}
				<button
					onclick={() => selectedRarity = r}
					class="px-2 py-1 text-[10px] rounded transition-all
						{selectedRarity === r
							? `bg-gx-bg-elevated font-medium ${r === 'all' ? 'text-gx-text-primary' : RARITY_COLORS[r]}`
							: 'text-gx-text-muted hover:text-gx-text-secondary'}"
				>
					{r === 'all' ? 'All' : RARITY_LABELS[r]}
				</button>
			{/each}
		</div>
	</div>

	<!-- Achievement Grid -->
	{#if achievementStore.loading}
		<div class="flex items-center justify-center h-40 text-gx-text-muted">
			<Zap size={20} class="animate-pulse mr-2" />
			Loading achievements...
		</div>
	{:else}
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-3">
			{#each filteredAchievements() as ach (ach.id)}
				{@const pct = progressPercent(ach)}
				<div
					class="group relative rounded-xl border bg-gx-bg-secondary p-4 transition-all duration-200
						hover:bg-gx-bg-elevated hover:shadow-lg
						{rarityBorderGlow(ach.rarity, ach.unlocked)}"
				>
					<!-- Unlocked glow gradient -->
					{#if ach.unlocked}
						<div
							class="absolute inset-0 rounded-xl bg-gradient-to-br {rarityGradient(ach.rarity)} pointer-events-none"
						></div>
					{/if}

					<div class="relative">
						<!-- Top row: icon + rarity badge -->
						<div class="flex items-start justify-between mb-2">
							<div
								class="w-10 h-10 rounded-lg flex items-center justify-center text-xl
									{ach.unlocked
										? 'bg-gx-bg-primary/80'
										: 'bg-gx-bg-tertiary grayscale'}"
							>
								{#if ach.unlocked}
									{ach.icon}
								{:else}
									<Lock size={16} class="text-gx-text-disabled" />
								{/if}
							</div>
							<span
								class="text-[9px] px-1.5 py-0.5 rounded-full bg-gx-bg-tertiary/80 {RARITY_COLORS[ach.rarity]}"
							>
								{RARITY_LABELS[ach.rarity]}
							</span>
						</div>

						<!-- Name -->
						<h3
							class="text-sm font-semibold truncate
								{ach.unlocked ? 'text-gx-text-primary' : 'text-gx-text-muted'}"
						>
							{ach.name}
						</h3>

						<!-- Description -->
						<p
							class="text-[11px] mt-0.5 line-clamp-2 leading-snug
								{ach.unlocked ? 'text-gx-text-secondary' : 'text-gx-text-disabled'}"
						>
							{ach.description}
						</p>

						<!-- Progress bar (for partial achievements) -->
						{#if !ach.unlocked && ach.target > 1}
							<div class="mt-2.5">
								<div class="flex justify-between text-[9px] mb-0.5">
									<span class="text-gx-text-muted">{ach.progress}/{ach.target}</span>
									<span class="text-gx-text-disabled">{pct}%</span>
								</div>
								<div class="h-1.5 rounded-full bg-gx-bg-tertiary overflow-hidden">
									<div
										class="h-full rounded-full bg-gx-neon/60 transition-all duration-500"
										style="width: {pct}%"
									></div>
								</div>
							</div>
						{/if}

						<!-- Footer: XP + unlock date -->
						<div class="flex items-center justify-between mt-2.5">
							<div class="flex items-center gap-1">
								<Star size={10} class="text-amber-400" />
								<span class="text-[10px] font-medium {ach.unlocked ? 'text-amber-400' : 'text-gx-text-disabled'}">
									{ach.xp_reward} XP
								</span>
							</div>
							{#if ach.unlocked && ach.unlocked_at}
								<span class="text-[9px] text-gx-text-disabled">
									{new Date(ach.unlocked_at).toLocaleDateString()}
								</span>
							{:else if ach.unlocked}
								<span class="text-[9px] text-green-400">Unlocked</span>
							{/if}
						</div>
					</div>
				</div>
			{/each}
		</div>

		{#if filteredAchievements().length === 0}
			<div class="flex flex-col items-center justify-center h-40 text-gx-text-muted">
				<Trophy size={32} class="mb-2 opacity-30" />
				<p class="text-sm">No achievements match this filter</p>
			</div>
		{/if}
	{/if}
</div>
