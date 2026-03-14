<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- OnboardingWizard — First-run setup wizard for ImpForge -->
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import {
		Sparkles,
		Brain,
		Plug,
		Rocket,
		ChevronLeft,
		ChevronRight,
		Check,
		Loader2,
		AlertCircle,
		CircleDot,
		Circle,
		Eye,
		EyeOff,
	} from '@lucide/svelte';
	import {
		User, Briefcase, GraduationCap, TrendingUp,
		Megaphone, Building2, Wrench, Layers
	} from '@lucide/svelte';
	import { saveSetting, getSetting, type AppSettings } from '$lib/stores/settings.svelte';
	import { completeOnboarding } from '$lib/stores/onboarding.svelte';

	// --- State ---
	let currentStep = $state(0);
	const totalSteps = 6;

	// Step 1: User Role (NEW)
	type UserRole = AppSettings['userRole'];
	let selectedRole = $state<UserRole>('');

	const roles: Array<{ id: UserRole; label: string; icon: typeof User; desc: string }> = [
		{ id: 'developer', label: 'Developer', icon: Code2, desc: 'Code, Git, Docker, IDE' },
		{ id: 'office', label: 'Office User', icon: Briefcase, desc: 'Docs, Chat, Email' },
		{ id: 'freelancer', label: 'Freelancer', icon: User, desc: 'Clients, Portfolio, Gigs' },
		{ id: 'manager', label: 'Manager', icon: Building2, desc: 'Teams, Reports, KPIs' },
		{ id: 'marketing', label: 'Marketing', icon: Megaphone, desc: 'Social Media, Content' },
		{ id: 'student', label: 'Student', icon: GraduationCap, desc: 'Learn, Research, Code' },
		{ id: 'entrepreneur', label: 'Entrepreneur', icon: TrendingUp, desc: 'Business, Automation' },
		{ id: 'custom', label: 'Show Everything', icon: Layers, desc: 'All modules visible' },
	];

	// Step 2: Experience Level (NEW)
	type UserExp = AppSettings['userExperience'];
	let selectedExperience = $state<UserExp>('');

	const experiences: Array<{ id: UserExp; label: string; desc: string; navHint: string }> = [
		{ id: 'beginner', label: 'Beginner', desc: 'New to AI tools — guide me step by step', navHint: 'Simple navigation, tooltips, large buttons' },
		{ id: 'intermediate', label: 'Intermediate', desc: 'Used AI before — show me the essentials', navHint: 'Standard navigation, keyboard shortcuts' },
		{ id: 'expert', label: 'Expert', desc: 'Power user — give me everything', navHint: 'Compact navigation, all modules, full control' },
	];

	// Step 3: AI Setup (was step 1)
	let ollamaStatus = $state<'checking' | 'online' | 'offline'>('checking');
	let ollamaModels = $state<string[]>([]);
	let openrouterKey = $state('');
	let openrouterValid = $state<boolean | null>(null);
	let openrouterChecking = $state(false);
	let showOpenrouterKey = $state(false);

	// Step 4: Integrations (was step 2)
	let dockerStatus = $state<'checking' | 'online' | 'offline'>('checking');
	let githubToken = $state('');
	let showGithubToken = $state(false);

	// --- Step definitions ---
	const steps = [
		{ title: 'Welcome', icon: Sparkles },
		{ title: 'Your Role', icon: User },
		{ title: 'Experience', icon: Brain },
		{ title: 'AI Setup', icon: Brain },
		{ title: 'Integrations', icon: Plug },
		{ title: 'Ready', icon: Rocket },
	];

	// --- Navigation ---
	function next() {
		if (currentStep < totalSteps - 1) {
			currentStep += 1;
			if (currentStep === 1 && selectedRole) saveSetting('userRole', selectedRole);
			if (currentStep === 2 && selectedExperience) saveSetting('userExperience', selectedExperience);
			if (currentStep === 3) checkOllama();
			if (currentStep === 4) checkDocker();
		}
	}

	function back() {
		if (currentStep > 0) {
			currentStep -= 1;
		}
	}

	async function skip() {
		await completeOnboarding();
	}

	async function finish() {
		// Save profile
		if (selectedRole) await saveSetting('userRole', selectedRole);
		if (selectedExperience) await saveSetting('userExperience', selectedExperience);
		// Save any entered keys
		if (openrouterKey.trim()) await saveSetting('openrouterKey', openrouterKey.trim());
		if (githubToken.trim()) await saveSetting('githubToken', githubToken.trim());
		await completeOnboarding();
	}

	// --- Service checks ---
	async function checkOllama() {
		ollamaStatus = 'checking';
		try {
			const url = getSetting('ollamaUrl') || 'http://localhost:11434';
			const result = await invoke<string>('cmd_test_ollama', { url });
			ollamaStatus = 'online';
			// Parse model names from the result string (comma-separated list or JSON)
			try {
				const parsed = JSON.parse(result);
				if (Array.isArray(parsed)) {
					ollamaModels = parsed.map((m: { name?: string }) => m.name ?? String(m));
				} else {
					ollamaModels = result.split(',').map((s: string) => s.trim()).filter(Boolean);
				}
			} catch {
				ollamaModels = result.split(',').map((s: string) => s.trim()).filter(Boolean);
			}
		} catch {
			ollamaStatus = 'offline';
			ollamaModels = [];
		}
	}

	async function checkDocker() {
		dockerStatus = 'checking';
		try {
			await invoke('docker_info');
			dockerStatus = 'online';
		} catch {
			dockerStatus = 'offline';
		}
	}

	async function validateOpenrouterKey() {
		if (!openrouterKey.trim()) return;
		openrouterChecking = true;
		openrouterValid = null;
		try {
			await invoke<string>('cmd_validate_openrouter_key', { key: openrouterKey.trim() });
			openrouterValid = true;
		} catch {
			openrouterValid = false;
		}
		openrouterChecking = false;
	}

	// Check Ollama on mount if we start on step 1
	$effect(() => {
		if (currentStep === 0) {
			// Pre-check Ollama so step 1 is ready
			checkOllama();
			checkDocker();
		}
	});

	// --- Derived summary ---
	let configuredItems = $derived.by(() => {
		const items: Array<{ label: string; status: 'ok' | 'skip' }> = [];

		items.push({
			label: 'Ollama (Local AI)',
			status: ollamaStatus === 'online' ? 'ok' : 'skip',
		});
		items.push({
			label: 'OpenRouter API',
			status: openrouterKey.trim() ? 'ok' : 'skip',
		});
		items.push({
			label: 'Docker',
			status: dockerStatus === 'online' ? 'ok' : 'skip',
		});
		items.push({
			label: 'GitHub',
			status: githubToken.trim() ? 'ok' : 'skip',
		});

		return items;
	});
</script>

<!-- Full-screen overlay backdrop -->
<div
	class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/60 backdrop-blur-sm"
	role="dialog"
	aria-modal="true"
	aria-label="ImpForge Setup Wizard"
>
	<!-- Wizard card -->
	<div class="w-full max-w-lg mx-4 rounded-gx-lg bg-gx-bg-secondary border border-gx-border-default shadow-gx-glow-lg overflow-hidden">
		<!-- Step indicator bar -->
		<div class="flex items-center justify-center gap-2 pt-5 pb-2">
			{#each steps as step, i}
				<button
					onclick={() => {
						if (i <= currentStep) currentStep = i;
					}}
					disabled={i > currentStep}
					class="flex items-center gap-1 transition-all duration-300"
					aria-label="Step {i + 1}: {step.title}"
					aria-current={i === currentStep ? 'step' : undefined}
				>
					{#if i === currentStep}
						<CircleDot size={10} class="text-gx-neon" />
					{:else if i < currentStep}
						<Check size={10} class="text-gx-neon" />
					{:else}
						<Circle size={10} class="text-gx-text-muted" />
					{/if}
				</button>
				{#if i < steps.length - 1}
					<div
						class="w-8 h-px transition-colors duration-300 {i < currentStep
							? 'bg-gx-neon/50'
							: 'bg-gx-border-default'}"
					></div>
				{/if}
			{/each}
		</div>

		<!-- Step content area -->
		<div class="px-8 py-6 min-h-[340px] flex flex-col">
			<!-- Step 0: Welcome -->
			{#if currentStep === 0}
				<div class="flex flex-col items-center text-center flex-1">
					<div class="w-20 h-20 rounded-gx-lg bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center mb-5">
						<Sparkles size={36} class="text-gx-neon" />
					</div>
					<h2 class="text-2xl font-bold text-gx-text-primary mb-2">Welcome to ImpForge</h2>
					<p class="text-sm text-gx-text-secondary mb-4 max-w-sm">
						Your personal AI workstation — adapts to exactly how you work.
					</p>
					<div class="grid grid-cols-2 gap-3 w-full max-w-xs mt-2">
						<div class="flex items-center gap-2 text-xs text-gx-text-muted p-2 rounded-gx bg-gx-bg-primary border border-gx-border-default">
							<Brain size={14} class="text-gx-neon shrink-0" />
							<span>Local AI Models</span>
						</div>
						<div class="flex items-center gap-2 text-xs text-gx-text-muted p-2 rounded-gx bg-gx-bg-primary border border-gx-border-default">
							<Plug size={14} class="text-gx-neon shrink-0" />
							<span>Cloud APIs</span>
						</div>
						<div class="flex items-center gap-2 text-xs text-gx-text-muted p-2 rounded-gx bg-gx-bg-primary border border-gx-border-default">
							<Rocket size={14} class="text-gx-neon shrink-0" />
							<span>Agent Swarms</span>
						</div>
						<div class="flex items-center gap-2 text-xs text-gx-text-muted p-2 rounded-gx bg-gx-bg-primary border border-gx-border-default">
							<Sparkles size={14} class="text-gx-neon shrink-0" />
							<span>100% Offline</span>
						</div>
					</div>
					<p class="text-xs text-gx-text-muted mt-6">
						We'll personalize ImpForge for you in 4 quick steps. Change anytime in Settings.
					</p>
				</div>

			<!-- Step 1: Your Role (NEW — Adaptive Onboarding) -->
			{:else if currentStep === 1}
				<div class="flex flex-col flex-1">
					<div class="flex items-center gap-3 mb-4">
						<div class="w-10 h-10 rounded-gx bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center">
							<User size={20} class="text-gx-neon" />
						</div>
						<div>
							<h2 class="text-lg font-bold text-gx-text-primary">What describes you best?</h2>
							<p class="text-xs text-gx-text-muted">We'll show you the right tools for your workflow</p>
						</div>
					</div>
					<div class="grid grid-cols-2 gap-2 flex-1 content-start">
						{#each roles as role (role.id)}
							<button
								onclick={() => { selectedRole = role.id; }}
								class="flex items-center gap-3 p-3 rounded-gx border text-left transition-all
									{selectedRole === role.id
										? 'border-gx-neon bg-gx-neon/10 shadow-gx-glow'
										: 'border-gx-border-default bg-gx-bg-primary hover:border-gx-neon/30 hover:bg-gx-bg-hover'}"
							>
								<div class="w-8 h-8 rounded-gx flex items-center justify-center shrink-0
									{selectedRole === role.id ? 'bg-gx-neon/20' : 'bg-gx-bg-secondary'}">
									<role.icon size={16} class={selectedRole === role.id ? 'text-gx-neon' : 'text-gx-text-muted'} />
								</div>
								<div class="min-w-0">
									<div class="text-sm font-medium {selectedRole === role.id ? 'text-gx-neon' : 'text-gx-text-primary'}">
										{role.label}
									</div>
									<div class="text-[10px] text-gx-text-muted truncate">{role.desc}</div>
								</div>
							</button>
						{/each}
					</div>
				</div>

			<!-- Step 2: Experience Level (NEW — Adaptive Onboarding) -->
			{:else if currentStep === 2}
				<div class="flex flex-col flex-1">
					<div class="flex items-center gap-3 mb-4">
						<div class="w-10 h-10 rounded-gx bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center">
							<Brain size={20} class="text-gx-neon" />
						</div>
						<div>
							<h2 class="text-lg font-bold text-gx-text-primary">How experienced are you with AI?</h2>
							<p class="text-xs text-gx-text-muted">This adjusts the interface complexity</p>
						</div>
					</div>
					<div class="space-y-3 flex-1">
						{#each experiences as exp (exp.id)}
							<button
								onclick={() => { selectedExperience = exp.id; }}
								class="w-full flex items-start gap-4 p-4 rounded-gx border text-left transition-all
									{selectedExperience === exp.id
										? 'border-gx-neon bg-gx-neon/10 shadow-gx-glow'
										: 'border-gx-border-default bg-gx-bg-primary hover:border-gx-neon/30 hover:bg-gx-bg-hover'}"
							>
								<div class="w-10 h-10 rounded-gx-lg flex items-center justify-center shrink-0 text-lg font-bold
									{selectedExperience === exp.id ? 'bg-gx-neon/20 text-gx-neon' : 'bg-gx-bg-secondary text-gx-text-muted'}">
									{exp.id === 'beginner' ? '1' : exp.id === 'intermediate' ? '2' : '3'}
								</div>
								<div class="min-w-0">
									<div class="text-sm font-medium {selectedExperience === exp.id ? 'text-gx-neon' : 'text-gx-text-primary'}">
										{exp.label}
									</div>
									<div class="text-xs text-gx-text-muted mt-0.5">{exp.desc}</div>
									<div class="text-[10px] text-gx-text-disabled mt-1">{exp.navHint}</div>
								</div>
							</button>
						{/each}
					</div>
				</div>

			<!-- Step 3: AI Setup (was step 1) -->
			{:else if currentStep === 3}
				<div class="flex flex-col flex-1">
					<div class="flex items-center gap-3 mb-4">
						<div class="w-10 h-10 rounded-gx bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center">
							<Brain size={20} class="text-gx-neon" />
						</div>
						<div>
							<h2 class="text-lg font-bold text-gx-text-primary">AI Setup</h2>
							<p class="text-xs text-gx-text-muted">Configure your AI inference backends</p>
						</div>
					</div>

					<!-- Ollama check -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-4 mb-4">
						<div class="flex items-center justify-between mb-2">
							<span class="text-sm font-medium text-gx-text-primary">Ollama (Local AI)</span>
							{#if ollamaStatus === 'checking'}
								<span class="flex items-center gap-1 text-xs text-gx-text-muted">
									<Loader2 size={12} class="animate-spin" />
									Checking...
								</span>
							{:else if ollamaStatus === 'online'}
								<span class="flex items-center gap-1 text-xs text-gx-status-success">
									<Check size={12} />
									Connected
								</span>
							{:else}
								<span class="flex items-center gap-1 text-xs text-gx-status-error">
									<AlertCircle size={12} />
									Not detected
								</span>
							{/if}
						</div>
						{#if ollamaStatus === 'online' && ollamaModels.length > 0}
							<p class="text-xs text-gx-text-muted">
								{ollamaModels.length} model{ollamaModels.length !== 1 ? 's' : ''} available: {ollamaModels.slice(0, 3).join(', ')}{ollamaModels.length > 3 ? '...' : ''}
							</p>
						{:else if ollamaStatus === 'offline'}
							<p class="text-xs text-gx-text-muted">
								Install Ollama from <span class="text-gx-neon">ollama.com</span> to run AI models locally. You can set this up later.
							</p>
						{/if}
					</div>

					<!-- OpenRouter key (optional) -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-4">
						<div class="flex items-center justify-between mb-2">
							<span class="text-sm font-medium text-gx-text-primary">OpenRouter API (Optional)</span>
							{#if openrouterValid === true}
								<span class="flex items-center gap-1 text-xs text-gx-status-success">
									<Check size={12} />
									Valid
								</span>
							{:else if openrouterValid === false}
								<span class="flex items-center gap-1 text-xs text-gx-status-error">
									<AlertCircle size={12} />
									Invalid
								</span>
							{/if}
						</div>
						<p class="text-xs text-gx-text-muted mb-3">
							Access cloud models (GPT-4o, Claude, etc.) via OpenRouter. Free tier available.
						</p>
						<div class="flex gap-2">
							<div class="relative flex-1">
								<input
									type={showOpenrouterKey ? 'text' : 'password'}
									bind:value={openrouterKey}
									placeholder="sk-or-..."
									class="w-full px-3 py-2 text-sm bg-gx-bg-secondary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon/50 transition-colors pr-9"
								/>
								<button
									type="button"
									onclick={() => showOpenrouterKey = !showOpenrouterKey}
									class="absolute right-2 top-1/2 -translate-y-1/2 text-gx-text-muted hover:text-gx-text-secondary transition-colors"
									aria-label={showOpenrouterKey ? 'Hide key' : 'Show key'}
								>
									{#if showOpenrouterKey}
										<EyeOff size={14} />
									{:else}
										<Eye size={14} />
									{/if}
								</button>
							</div>
							<button
								onclick={validateOpenrouterKey}
								disabled={!openrouterKey.trim() || openrouterChecking}
								class="px-3 py-2 text-xs font-medium rounded-gx border border-gx-neon/30 text-gx-neon hover:bg-gx-neon/10 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
							>
								{#if openrouterChecking}
									<Loader2 size={14} class="animate-spin" />
								{:else}
									Verify
								{/if}
							</button>
						</div>
					</div>
				</div>

			<!-- Step 4: Integrations (was step 2) -->
			{:else if currentStep === 4}
				<div class="flex flex-col flex-1">
					<div class="flex items-center gap-3 mb-4">
						<div class="w-10 h-10 rounded-gx bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center">
							<Plug size={20} class="text-gx-neon" />
						</div>
						<div>
							<h2 class="text-lg font-bold text-gx-text-primary">Integrations</h2>
							<p class="text-xs text-gx-text-muted">Optional tools and services</p>
						</div>
					</div>

					<!-- Docker check -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-4 mb-4">
						<div class="flex items-center justify-between mb-2">
							<span class="text-sm font-medium text-gx-text-primary">Docker</span>
							{#if dockerStatus === 'checking'}
								<span class="flex items-center gap-1 text-xs text-gx-text-muted">
									<Loader2 size={12} class="animate-spin" />
									Checking...
								</span>
							{:else if dockerStatus === 'online'}
								<span class="flex items-center gap-1 text-xs text-gx-status-success">
									<Check size={12} />
									Available
								</span>
							{:else}
								<span class="flex items-center gap-1 text-xs text-gx-status-error">
									<AlertCircle size={12} />
									Not detected
								</span>
							{/if}
						</div>
						<p class="text-xs text-gx-text-muted">
							{#if dockerStatus === 'online'}
								Docker is running. You can manage containers from ImpForge.
							{:else}
								Docker enables container-based workflows. Install it from <span class="text-gx-neon">docker.com</span> when needed.
							{/if}
						</p>
					</div>

					<!-- GitHub token (optional) -->
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-4">
						<div class="flex items-center justify-between mb-2">
							<span class="text-sm font-medium text-gx-text-primary">GitHub Token (Optional)</span>
							{#if githubToken.trim()}
								<span class="flex items-center gap-1 text-xs text-gx-status-success">
									<Check size={12} />
									Set
								</span>
							{/if}
						</div>
						<p class="text-xs text-gx-text-muted mb-3">
							A personal access token lets ImpForge interact with your repositories, issues, and pull requests.
						</p>
						<div class="relative">
							<input
								type={showGithubToken ? 'text' : 'password'}
								bind:value={githubToken}
								placeholder="ghp_..."
								class="w-full px-3 py-2 text-sm bg-gx-bg-secondary border border-gx-border-default rounded-gx text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none focus:border-gx-neon/50 transition-colors pr-9"
							/>
							<button
								type="button"
								onclick={() => showGithubToken = !showGithubToken}
								class="absolute right-2 top-1/2 -translate-y-1/2 text-gx-text-muted hover:text-gx-text-secondary transition-colors"
								aria-label={showGithubToken ? 'Hide token' : 'Show token'}
							>
								{#if showGithubToken}
									<EyeOff size={14} />
								{:else}
									<Eye size={14} />
								{/if}
							</button>
						</div>
					</div>
				</div>

			<!-- Step 5: Ready (was step 3) -->
			{:else if currentStep === 5}
				<div class="flex flex-col items-center text-center flex-1">
					<div class="w-16 h-16 rounded-gx-lg bg-gx-neon/10 border border-gx-neon/30 flex items-center justify-center mb-5">
						<Rocket size={28} class="text-gx-neon" />
					</div>
					<h2 class="text-xl font-bold text-gx-text-primary mb-2">You're All Set</h2>
					<p class="text-sm text-gx-text-secondary mb-5 max-w-xs">
						Here's a summary of your configuration. You can change any of these in Settings at any time.
					</p>

					<!-- Summary list -->
					<div class="w-full max-w-xs space-y-2">
						{#each configuredItems as item}
							<div class="flex items-center justify-between px-3 py-2 rounded-gx bg-gx-bg-primary border border-gx-border-default text-sm">
								<span class="text-gx-text-secondary">{item.label}</span>
								{#if item.status === 'ok'}
									<span class="flex items-center gap-1 text-xs text-gx-status-success">
										<Check size={12} />
										Ready
									</span>
								{:else}
									<span class="text-xs text-gx-text-muted">Skipped</span>
								{/if}
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Footer: navigation buttons -->
		<div class="flex items-center justify-between px-8 py-4 border-t border-gx-border-default">
			<!-- Left side: Skip or Back -->
			<div class="flex items-center gap-2">
				{#if currentStep > 0}
					<button
						onclick={back}
						class="flex items-center gap-1 px-3 py-2 text-xs font-medium text-gx-text-muted hover:text-gx-text-secondary transition-colors rounded-gx hover:bg-gx-bg-primary"
					>
						<ChevronLeft size={14} />
						Back
					</button>
				{/if}
			</div>

			<div class="flex items-center gap-2">
				<!-- Skip always available -->
				{#if currentStep < totalSteps - 1}
					<button
						onclick={skip}
						class="px-3 py-2 text-xs font-medium text-gx-text-muted hover:text-gx-text-secondary transition-colors rounded-gx hover:bg-gx-bg-primary"
					>
						Skip Setup
					</button>
				{/if}

				<!-- Next / Get Started -->
				{#if currentStep < totalSteps - 1}
					<button
						onclick={next}
						class="flex items-center gap-1 px-4 py-2 text-xs font-semibold rounded-gx bg-gx-neon text-gx-bg-primary hover:brightness-110 transition-all"
					>
						Next
						<ChevronRight size={14} />
					</button>
				{:else}
					<button
						onclick={finish}
						class="flex items-center gap-1 px-5 py-2 text-sm font-semibold rounded-gx bg-gx-neon text-gx-bg-primary hover:brightness-110 transition-all"
					>
						<Sparkles size={14} />
						Get Started
					</button>
				{/if}
			</div>
		</div>
	</div>
</div>
