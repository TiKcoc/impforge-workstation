<script lang="ts">
	/**
	 * OnboardingProgress -- dashboard widget that tracks the user's
	 * first-session journey through ImpForge.  Shows a progress bar
	 * and checklist of key milestones.  Dismissible once all steps
	 * are completed (or via explicit close).
	 *
	 * Persists completion state via the settings store so it survives
	 * restarts and only shows until every step is done.
	 */

	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { getSetting, saveSetting } from '$lib/stores/settings.svelte';
	import {
		CheckCircle2,
		Circle,
		X,
		Rocket
	} from '@lucide/svelte';

	interface OnboardingStep {
		id: string;
		label: string;
		description: string;
		done: boolean;
	}

	let steps = $state<OnboardingStep[]>([
		{
			id: 'profile',
			label: 'Complete Profile',
			description: 'Set your role and experience level in Settings.',
			done: false,
		},
		{
			id: 'first_chat',
			label: 'Send first message',
			description: 'Open Chat and talk to an AI model.',
			done: false,
		},
		{
			id: 'explore_modules',
			label: 'Try 3 modules',
			description: 'Visit at least three different modules.',
			done: false,
		},
		{
			id: 'create_document',
			label: 'Create a document',
			description: 'Write something in ForgeWriter or ForgeNotes.',
			done: false,
		},
		{
			id: 'create_workflow',
			label: 'Build a workflow',
			description: 'Create a ForgeFlow automation.',
			done: false,
		},
		{
			id: 'customize_theme',
			label: 'Customize your theme',
			description: 'Pick or create a theme in Settings.',
			done: false,
		},
		{
			id: 'invite_team',
			label: 'Invite a team member',
			description: 'Add a collaborator in ForgeTeam.',
			done: false,
		},
	]);

	let dismissed = $state(false);

	let completedCount = $derived(steps.filter((s) => s.done).length);
	let totalCount = $derived(steps.length);
	let progress = $derived(totalCount > 0 ? (completedCount / totalCount) * 100 : 0);
	let allDone = $derived(completedCount === totalCount);

	// Load persisted step completion from settings
	onMount(async () => {
		try {
			const saved = getSetting('onboardingComplete');
			if (saved) {
				dismissed = true;
				return;
			}
		} catch {
			// first run -- no settings yet
		}

		// Check each step against real state
		await checkStepCompletion();
	});

	async function checkStepCompletion() {
		// profile: check if userRole is set (non-empty means user picked a role)
		const role = getSetting('userRole');
		if (role) {
			markDone('profile');
		}

		// theme: check if user changed from default
		const theme = getSetting('theme');
		if (theme && theme !== 'dark') {
			markDone('customize_theme');
		}

		// first_chat: check via Tauri if any chat history exists
		try {
			const chatCount = await invoke<number>('chat_history_count').catch(() => 0);
			if (chatCount > 0) {
				markDone('first_chat');
			}
		} catch {
			// command may not exist yet -- skip
		}
	}

	function markDone(stepId: string) {
		const step = steps.find((s) => s.id === stepId);
		if (step) {
			step.done = true;
		}
	}

	async function dismiss() {
		dismissed = true;
		await saveSetting('onboardingComplete', true);
	}
</script>

{#if !dismissed && !allDone}
	<div
		class="rounded-gx-lg border border-gx-border-default bg-gx-bg-secondary p-4 space-y-3"
		role="region"
		aria-label="Getting started progress"
	>
		<!-- Header -->
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-2">
				<Rocket size={16} class="text-gx-neon" />
				<span class="text-sm font-bold text-gx-text-primary">
					Getting Started: {completedCount}/{totalCount}
				</span>
			</div>
			<button
				onclick={dismiss}
				aria-label="Dismiss onboarding progress"
				class="p-1 rounded text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
			>
				<X size={14} />
			</button>
		</div>

		<!-- Progress bar -->
		<div class="w-full bg-gx-bg-tertiary rounded-full h-2 overflow-hidden">
			<div
				class="bg-gx-neon h-2 rounded-full transition-all duration-500 ease-out"
				style="width: {progress}%"
				role="progressbar"
				aria-valuenow={completedCount}
				aria-valuemin={0}
				aria-valuemax={totalCount}
			></div>
		</div>

		<!-- Step checklist -->
		<div class="space-y-1.5">
			{#each steps as step (step.id)}
				<div class="flex items-start gap-2 py-1 px-1 rounded transition-colors {step.done ? 'opacity-60' : ''}">
					{#if step.done}
						<CheckCircle2
							size={16}
							class="text-gx-status-success mt-0.5 shrink-0"
						/>
					{:else}
						<Circle
							size={16}
							class="text-gx-text-muted mt-0.5 shrink-0"
						/>
					{/if}
					<div class="min-w-0">
						<span
							class="text-xs font-medium {step.done
								? 'text-gx-text-muted line-through'
								: 'text-gx-text-primary'}"
						>
							{step.label}
						</span>
						{#if !step.done}
							<p class="text-[10px] text-gx-text-muted mt-0.5">
								{step.description}
							</p>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	</div>
{/if}
