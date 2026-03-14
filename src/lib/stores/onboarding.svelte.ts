/**
 * Onboarding Store - Svelte 5 Runes
 * Tracks whether the first-run setup wizard has been completed.
 * Persists via the settings store (onboardingComplete key).
 */

import { getSetting, saveSetting } from './settings.svelte';

/**
 * Returns true when the user has finished (or skipped) onboarding.
 */
export function isOnboardingComplete(): boolean {
	return getSetting('onboardingComplete');
}

/**
 * Mark onboarding as done and persist the flag.
 */
export async function completeOnboarding(): Promise<void> {
	await saveSetting('onboardingComplete', true);
}

/**
 * Reset onboarding so the wizard shows again on next launch.
 * Useful for testing or if the user wants to re-run setup.
 */
export async function resetOnboarding(): Promise<void> {
	await saveSetting('onboardingComplete', false);
}
