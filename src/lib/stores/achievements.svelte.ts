/**
 * Achievements Store — Gamification state management (Svelte 5 runes)
 *
 * Tracks achievements, XP, levels, and streaks via Tauri IPC.
 * Provides reactive state for the achievements page and toast notifications.
 */

import { invoke } from '@tauri-apps/api/core';

export interface Achievement {
	id: string;
	name: string;
	description: string;
	icon: string;
	category: AchievementCategory;
	unlocked: boolean;
	unlocked_at: string | null;
	progress: number;
	target: number;
	xp_reward: number;
	rarity: Rarity;
}

export type AchievementCategory =
	| 'getting_started'
	| 'productivity'
	| 'ai_master'
	| 'team_player'
	| 'power_user'
	| 'creator'
	| 'explorer';

export type Rarity = 'common' | 'uncommon' | 'rare' | 'epic' | 'legendary';

export interface UserProgress {
	level: number;
	total_xp: number;
	current_streak: number;
	longest_streak: number;
	documents_created: number;
	ai_queries: number;
	workflows_run: number;
	modules_used: string[];
	xp_for_next_level: number;
}

export const CATEGORY_LABELS: Record<AchievementCategory, string> = {
	getting_started: 'Getting Started',
	productivity: 'Productivity',
	ai_master: 'AI Master',
	team_player: 'Team Player',
	power_user: 'Power User',
	creator: 'Creator',
	explorer: 'Explorer',
};

export const RARITY_LABELS: Record<Rarity, string> = {
	common: 'Common',
	uncommon: 'Uncommon',
	rare: 'Rare',
	epic: 'Epic',
	legendary: 'Legendary',
};

export const RARITY_COLORS: Record<Rarity, string> = {
	common: 'text-gx-text-muted',
	uncommon: 'text-green-400',
	rare: 'text-blue-400',
	epic: 'text-purple-400',
	legendary: 'text-amber-400',
};

export const RARITY_BG_COLORS: Record<Rarity, string> = {
	common: 'border-gx-border-default',
	uncommon: 'border-green-500/30',
	rare: 'border-blue-500/30',
	epic: 'border-purple-500/30',
	legendary: 'border-amber-500/30',
};

class AchievementStore {
	achievements = $state<Achievement[]>([]);
	progress = $state<UserProgress>({
		level: 1,
		total_xp: 0,
		current_streak: 0,
		longest_streak: 0,
		documents_created: 0,
		ai_queries: 0,
		workflows_run: 0,
		modules_used: [],
		xp_for_next_level: 100,
	});
	loading = $state(false);
	toastQueue = $state<Achievement[]>([]);

	get unlockedCount() {
		return this.achievements.filter((a) => a.unlocked).length;
	}

	get totalCount() {
		return this.achievements.length;
	}

	get xpProgress() {
		const current = this.progress.total_xp;
		const nextLevel = this.progress.xp_for_next_level;
		// XP within current level band
		const prevLevel = this.progress.level <= 1 ? 0 : Math.floor(100 * Math.pow(this.progress.level, 1.5));
		const band = nextLevel - prevLevel;
		const inBand = current - prevLevel;
		return band > 0 ? Math.min(100, Math.round((inBand / band) * 100)) : 0;
	}

	async load() {
		this.loading = true;
		try {
			const [list, prog] = await Promise.all([
				invoke<Achievement[]>('achievements_list'),
				invoke<UserProgress>('achievements_get_progress'),
			]);
			this.achievements = list;
			this.progress = prog;
		} catch (e) {
			console.error('Failed to load achievements:', e);
		} finally {
			this.loading = false;
		}
	}

	/**
	 * Track a user action. Returns any newly unlocked achievement (for toast).
	 * Call this from anywhere in the app when a trackable event occurs.
	 */
	async trackAction(action: string): Promise<Achievement | null> {
		try {
			const unlocked = await invoke<Achievement | null>('achievements_track_action', {
				action,
			});
			if (unlocked) {
				this.toastQueue.push(unlocked);
				// Reload to get fresh state
				await this.load();
			}
			return unlocked;
		} catch (e) {
			console.error('Failed to track action:', e);
			return null;
		}
	}

	/** Remove the oldest toast from the queue (after it's been displayed). */
	dismissToast() {
		this.toastQueue.shift();
	}

	/** Filter achievements by category. */
	byCategory(category: AchievementCategory): Achievement[] {
		return this.achievements.filter((a) => a.category === category);
	}

	/** Filter achievements by rarity. */
	byRarity(rarity: Rarity): Achievement[] {
		return this.achievements.filter((a) => a.rarity === rarity);
	}
}

export const achievementStore = new AchievementStore();
