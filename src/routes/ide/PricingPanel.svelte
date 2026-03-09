<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Crown, Sparkles, Check, X, Users, Shield, Zap,
		KeyRound, Loader2, Copy, Eye, EyeOff, ChevronDown,
		Building2, Cpu, UserPlus, LogOut, BarChart3, RefreshCw
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';

	// -----------------------------------------------------------------------
	// Types
	// -----------------------------------------------------------------------

	interface TierPricing {
		id: string;
		name: string;
		price_monthly_cents: number;
		price_yearly_cents: number;
		is_per_user: boolean;
		popular: boolean;
		features: Record<string, string | boolean>;
		checkout_url: string;
	}

	interface LicenseInfo {
		user_id: string;
		email: string;
		tier: string;
		features: Record<string, unknown>;
		valid_until: string;
		license_key: string;
		team_id: string | null;
		team_name: string | null;
		seat_count: number | null;
		issued_at: string;
		signature: string;
		last_verified: string | null;
	}

	interface TeamMember {
		user_id: string;
		email: string;
		role: string;
		joined_at: string;
		status: string;
	}

	interface UsageInfo {
		tier: string;
		ai_completions_today: number;
		ai_completions_quota: string;
		usage_date: string;
		days_until_expiry: number;
		days_since_verified: number;
		grace_period_days: number;
		needs_phone_home: boolean;
	}

	// -----------------------------------------------------------------------
	// State
	// -----------------------------------------------------------------------

	let tiers = $state<TierPricing[]>([]);
	let currentLicense = $state<LicenseInfo | null>(null);
	let usage = $state<UsageInfo | null>(null);
	let teamMembers = $state<TeamMember[]>([]);
	let loading = $state(true);
	let activating = $state(false);
	let licenseKeyInput = $state('');
	let showLicenseKey = $state(false);
	let billingCycle = $state<'monthly' | 'yearly'>('monthly');
	let activeSection = $state<'pricing' | 'license' | 'team' | 'usage'>('pricing');
	let errorMsg = $state('');
	let successMsg = $state('');

	// -----------------------------------------------------------------------
	// Derived
	// -----------------------------------------------------------------------

	const currentTierName = $derived(
		currentLicense ? (currentLicense.tier as string) : 'Free'
	);

	const currentTierId = $derived(currentTierName.toLowerCase());

	const isTeamTier = $derived(
		currentTierId === 'team' || currentTierId === 'business' || currentTierId === 'enterprise'
	);

	const maskedKey = $derived(
		currentLicense
			? currentLicense.license_key.slice(0, 6) + '****' + currentLicense.license_key.slice(-4)
			: ''
	);

	const completionPercent = $derived(() => {
		if (!usage) return 0;
		if (usage.ai_completions_quota === 'unlimited') return 0;
		const quota = parseInt(usage.ai_completions_quota, 10);
		if (quota <= 0) return 0;
		return Math.min(100, Math.round((usage.ai_completions_today / quota) * 100));
	});

	// -----------------------------------------------------------------------
	// Feature matrix rows (order matters for display)
	// -----------------------------------------------------------------------

	const featureRows: { key: string; label: string; type: 'bool' | 'value' }[] = [
		{ key: 'ai_completion', label: 'AI Code Completion', type: 'bool' },
		{ key: 'ai_models', label: 'Number of AI Models', type: 'value' },
		{ key: 'ai_completions_per_day', label: 'AI Completions / Day', type: 'value' },
		{ key: 'shadow_workspace', label: 'Shadow Workspace', type: 'bool' },
		{ key: 'debug_adapter', label: 'Debug Adapter', type: 'bool' },
		{ key: 'custom_themes', label: 'Custom Themes', type: 'bool' },
		{ key: 'collab', label: 'Real-time Collaboration', type: 'bool' },
		{ key: 'team_members', label: 'Team Management', type: 'value' },
		{ key: 'sso_saml', label: 'SSO / SAML', type: 'bool' },
		{ key: 'audit_logs', label: 'Audit Logs', type: 'bool' },
		{ key: 'custom_models', label: 'Custom Model Training', type: 'bool' },
		{ key: 'on_premise', label: 'On-Premise Deployment', type: 'bool' },
		{ key: 'priority_support', label: 'Priority Support', type: 'bool' },
		{ key: 'git_integration', label: 'Git Integration', type: 'bool' },
		{ key: 'lsp_support', label: 'LSP Support', type: 'bool' },
	];

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	onMount(async () => {
		await loadAll();
	});

	async function loadAll() {
		loading = true;
		errorMsg = '';
		try {
			const [pricingResult, licenseResult, usageResult] = await Promise.allSettled([
				invoke<TierPricing[]>('billing_get_pricing'),
				invoke<LicenseInfo | null>('billing_get_license'),
				invoke<UsageInfo>('billing_get_usage'),
			]);

			if (pricingResult.status === 'fulfilled') tiers = pricingResult.value;
			if (licenseResult.status === 'fulfilled') currentLicense = licenseResult.value;
			if (usageResult.status === 'fulfilled') usage = usageResult.value;

			if (isTeamTier) {
				try {
					teamMembers = await invoke<TeamMember[]>('billing_team_members');
				} catch {
					teamMembers = [];
				}
			}
		} catch (e) {
			errorMsg = `Failed to load billing data: ${e}`;
		}
		loading = false;
	}

	// -----------------------------------------------------------------------
	// Actions
	// -----------------------------------------------------------------------

	async function activateLicense() {
		if (!licenseKeyInput.trim()) return;
		activating = true;
		errorMsg = '';
		successMsg = '';

		try {
			currentLicense = await invoke<LicenseInfo>('billing_activate_license', {
				licenseKey: licenseKeyInput.trim(),
			});
			successMsg = `License activated! You are now on the ${currentLicense.tier} plan.`;
			licenseKeyInput = '';
			// Reload usage
			usage = await invoke<UsageInfo>('billing_get_usage');
		} catch (e) {
			errorMsg = String(e);
		}
		activating = false;
	}

	async function deactivateLicense() {
		errorMsg = '';
		successMsg = '';
		try {
			await invoke('billing_deactivate');
			currentLicense = null;
			successMsg = 'License deactivated. You are now on the Free plan.';
			usage = await invoke<UsageInfo>('billing_get_usage');
		} catch (e) {
			errorMsg = String(e);
		}
	}

	function copyKey() {
		if (currentLicense) {
			navigator.clipboard.writeText(currentLicense.license_key);
			successMsg = 'License key copied to clipboard';
			setTimeout(() => { if (successMsg === 'License key copied to clipboard') successMsg = ''; }, 2000);
		}
	}

	function openCheckout(url: string) {
		// In production: open Stripe checkout in external browser
		window.open(url, '_blank');
	}

	function formatPrice(cents: number, perUser: boolean, yearly: boolean): string {
		if (cents === 0 && !yearly) return 'Free';
		if (cents === 0 && yearly) return 'Custom';
		const eur = (cents / 100).toFixed(0);
		const suffix = perUser ? '/user/mo' : '/mo';
		if (yearly) {
			const monthlyEquiv = Math.round(cents / 12 / 100);
			return `${monthlyEquiv}${suffix}`;
		}
		return `${eur}${suffix}`;
	}

	function yearlyDiscount(tier: TierPricing): string {
		if (tier.price_monthly_cents === 0) return '';
		if (tier.price_yearly_cents === 0) return 'Custom pricing';
		const monthlyTotal = tier.price_monthly_cents * 12;
		const saving = Math.round(((monthlyTotal - tier.price_yearly_cents) / monthlyTotal) * 100);
		if (saving > 0) return `Save ${saving}% yearly`;
		return '';
	}
</script>

<!-- ======================================================================= -->
<!-- MAIN LAYOUT -->
<!-- ======================================================================= -->

<div class="flex flex-col h-full bg-[#0a0e14] overflow-hidden">
	<!-- Header -->
	<div class="flex items-center gap-3 px-4 py-3 border-b border-white/5 shrink-0 bg-[#0d1117]">
		<Crown size={18} class="text-[#00FF66]" />
		<span class="text-sm font-bold text-white/90">Subscription & Billing</span>
		<div class="flex-1"></div>
		<div class="flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-semibold
			{currentTierId === 'free' ? 'bg-white/5 text-white/40' :
			 currentTierId === 'pro' ? 'bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30' :
			 'bg-purple-500/10 text-purple-400 border border-purple-500/30'}">
			<Sparkles size={10} />
			{currentTierName}
		</div>
		<button onclick={() => loadAll()} class="p-1 text-white/30 hover:text-[#00FF66] transition-colors" title="Refresh">
			<RefreshCw size={14} class={loading ? 'animate-spin' : ''} />
		</button>
	</div>

	<!-- Section tabs -->
	<div class="flex border-b border-white/5 shrink-0 bg-[#0d1117]">
		{#each [
			{ id: 'pricing' as const, label: 'Plans', icon: Crown },
			{ id: 'license' as const, label: 'License', icon: KeyRound },
			{ id: 'usage' as const, label: 'Usage', icon: BarChart3 },
			{ id: 'team' as const, label: 'Team', icon: Users },
		] as tab}
			<button
				onclick={() => activeSection = tab.id}
				disabled={tab.id === 'team' && !isTeamTier}
				class="flex items-center gap-1.5 px-3 py-2 text-xs transition-colors
					{activeSection === tab.id
						? 'text-[#00FF66] border-b-2 border-[#00FF66]'
						: 'text-white/40 hover:text-white/60'}
					{tab.id === 'team' && !isTeamTier ? 'opacity-30 cursor-not-allowed' : ''}"
			>
				<tab.icon size={12} />
				{tab.label}
			</button>
		{/each}
	</div>

	<!-- Messages -->
	{#if errorMsg}
		<div class="mx-4 mt-3 px-3 py-2 rounded bg-red-500/10 border border-red-500/30 text-xs text-red-400 flex items-center gap-2">
			<X size={12} class="shrink-0" />
			<span class="flex-1">{errorMsg}</span>
			<button onclick={() => errorMsg = ''} class="text-red-400/60 hover:text-red-400"><X size={10} /></button>
		</div>
	{/if}
	{#if successMsg}
		<div class="mx-4 mt-3 px-3 py-2 rounded bg-[#00FF66]/10 border border-[#00FF66]/30 text-xs text-[#00FF66] flex items-center gap-2">
			<Check size={12} class="shrink-0" />
			<span class="flex-1">{successMsg}</span>
			<button onclick={() => successMsg = ''} class="text-[#00FF66]/60 hover:text-[#00FF66]"><X size={10} /></button>
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 overflow-auto">
		{#if loading}
			<div class="flex items-center justify-center h-full">
				<Loader2 size={24} class="animate-spin text-[#00FF66]" />
			</div>

		<!-- =================================================================== -->
		<!-- PRICING PLANS -->
		<!-- =================================================================== -->
		{:else if activeSection === 'pricing'}
			<div class="p-4 space-y-6">
				<!-- Billing cycle toggle -->
				<div class="flex items-center justify-center gap-3">
					<span class="text-xs {billingCycle === 'monthly' ? 'text-white/90' : 'text-white/40'}">Monthly</span>
					<button
						onclick={() => billingCycle = billingCycle === 'monthly' ? 'yearly' : 'monthly'}
						class="relative w-10 h-5 rounded-full transition-colors
							{billingCycle === 'yearly' ? 'bg-[#00FF66]/30' : 'bg-white/10'}"
					>
						<span class="absolute top-0.5 w-4 h-4 rounded-full transition-transform
							{billingCycle === 'yearly' ? 'translate-x-5 bg-[#00FF66]' : 'translate-x-0.5 bg-white/60'}"></span>
					</button>
					<span class="text-xs {billingCycle === 'yearly' ? 'text-white/90' : 'text-white/40'}">
						Yearly
						<span class="text-[#00FF66] ml-1 font-semibold">Save up to 17%</span>
					</span>
				</div>

				<!-- Tier cards -->
				<div class="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-3">
					{#each tiers as tier}
						{@const isCurrent = currentTierId === tier.id}
						{@const isPopular = tier.popular}
						<div class="relative flex flex-col rounded-lg border transition-all
							{isPopular
								? 'border-[#00FF66]/50 shadow-[0_0_20px_rgba(0,255,102,0.1)]'
								: isCurrent
									? 'border-[#00FF66]/30 bg-[#00FF66]/5'
									: 'border-white/10 hover:border-white/20'}
							bg-[#0d1117]"
						>
							<!-- Popular badge -->
							{#if isPopular}
								<div class="absolute -top-2.5 left-1/2 -translate-x-1/2 px-2 py-0.5 rounded-full
									bg-[#00FF66] text-black text-[10px] font-bold uppercase tracking-wide whitespace-nowrap">
									Most Popular
								</div>
							{/if}

							<!-- Current badge -->
							{#if isCurrent}
								<div class="absolute -top-2.5 right-2 px-2 py-0.5 rounded-full
									bg-[#00FF66]/20 text-[#00FF66] text-[10px] font-semibold border border-[#00FF66]/40 whitespace-nowrap">
									Current Plan
								</div>
							{/if}

							<div class="p-3 pt-4 flex flex-col flex-1">
								<!-- Tier name -->
								<div class="flex items-center gap-1.5 mb-1">
									{#if tier.id === 'enterprise'}
										<Building2 size={14} class="text-purple-400" />
									{:else if tier.id === 'business'}
										<Shield size={14} class="text-blue-400" />
									{:else if tier.id === 'team'}
										<Users size={14} class="text-cyan-400" />
									{:else if tier.id === 'pro'}
										<Zap size={14} class="text-[#00FF66]" />
									{:else if tier.id === 'starter'}
										<Sparkles size={14} class="text-amber-400" />
									{:else}
										<Cpu size={14} class="text-white/40" />
									{/if}
									<span class="text-sm font-bold text-white/90">{tier.name}</span>
								</div>

								<!-- Price -->
								<div class="mb-3">
									{#if tier.price_monthly_cents === 0 && tier.id === 'free'}
										<span class="text-2xl font-bold text-white/90">€0</span>
									{:else if tier.id === 'enterprise' && billingCycle === 'yearly'}
										<span class="text-lg font-bold text-white/60">Custom</span>
									{:else}
										<span class="text-2xl font-bold text-white/90">
											€{billingCycle === 'monthly'
												? (tier.price_monthly_cents / 100).toFixed(0)
												: Math.round(tier.price_yearly_cents / 12 / 100)}
										</span>
										<span class="text-xs text-white/40">
											{tier.is_per_user ? '/user/mo' : '/mo'}
										</span>
									{/if}
									{#if yearlyDiscount(tier) && billingCycle === 'monthly'}
										<div class="text-[10px] text-[#00FF66]/70 mt-0.5">{yearlyDiscount(tier)}</div>
									{/if}
								</div>

								<!-- Feature highlights (compact) -->
								<div class="space-y-1 flex-1 mb-3">
									{#each featureRows.slice(0, 7) as row}
										{@const val = tier.features[row.key]}
										<div class="flex items-center gap-1.5 text-[11px]">
											{#if row.type === 'bool'}
												{#if val}
													<Check size={10} class="text-[#00FF66] shrink-0" />
													<span class="text-white/60">{row.label}</span>
												{:else}
													<X size={10} class="text-[#ff5370] shrink-0" />
													<span class="text-white/25">{row.label}</span>
												{/if}
											{:else}
												{@const strVal = String(val)}
												{#if strVal === '-' || strVal === '0'}
													<X size={10} class="text-[#ff5370] shrink-0" />
													<span class="text-white/25">{row.label}</span>
												{:else}
													<Check size={10} class="text-[#00FF66] shrink-0" />
													<span class="text-white/60">{row.label}: <span class="text-white/80 font-medium">{strVal}</span></span>
												{/if}
											{/if}
										</div>
									{/each}
								</div>

								<!-- Action button -->
								{#if isCurrent}
									<div class="text-center py-1.5 rounded text-xs font-semibold
										bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30">
										Active
									</div>
								{:else if tier.id === 'free'}
									<button
										onclick={deactivateLicense}
										disabled={currentTierId === 'free'}
										class="w-full py-1.5 rounded text-xs font-semibold transition-all
											bg-white/5 text-white/40 border border-white/10
											hover:bg-white/10 hover:text-white/60
											disabled:opacity-30 disabled:cursor-not-allowed"
									>
										{currentTierId === 'free' ? 'Current' : 'Downgrade'}
									</button>
								{:else if tier.id === 'enterprise'}
									<button
										onclick={() => openCheckout(tier.checkout_url)}
										class="w-full py-1.5 rounded text-xs font-semibold transition-all
											bg-purple-500/10 text-purple-400 border border-purple-500/30
											hover:bg-purple-500/20"
									>
										Contact Sales
									</button>
								{:else}
									<button
										onclick={() => openCheckout(tier.checkout_url)}
										class="w-full py-1.5 rounded text-xs font-bold transition-all
											{isPopular
												? 'bg-[#00FF66] text-black hover:bg-[#00FF66]/90'
												: 'bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30 hover:bg-[#00FF66]/20'}"
									>
										{currentTierId !== 'free' && tiers.findIndex(t => t.id === currentTierId) > tiers.findIndex(t => t.id === tier.id)
											? 'Downgrade'
											: 'Upgrade'}
									</button>
								{/if}
							</div>
						</div>
					{/each}
				</div>

				<!-- Full feature comparison table -->
				<div class="mt-6">
					<button
						onclick={() => {
							const el = document.getElementById('feature-matrix');
							if (el) el.classList.toggle('hidden');
						}}
						class="flex items-center gap-2 text-xs text-white/40 hover:text-white/60 transition-colors mx-auto"
					>
						<ChevronDown size={12} />
						View full feature comparison
					</button>

					<div id="feature-matrix" class="hidden mt-4 rounded-lg border border-white/10 overflow-hidden">
						<table class="w-full text-xs">
							<thead>
								<tr class="bg-[#0d1117] border-b border-white/5">
									<th class="text-left px-3 py-2 text-white/40 font-medium">Feature</th>
									{#each tiers as tier}
										<th class="text-center px-2 py-2 font-medium
											{currentTierId === tier.id ? 'text-[#00FF66]' : 'text-white/60'}">
											{tier.name}
										</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each featureRows as row, i}
									<tr class="border-b border-white/5 {i % 2 === 0 ? 'bg-[#0a0e14]' : 'bg-[#0d1117]'}">
										<td class="px-3 py-1.5 text-white/50">{row.label}</td>
										{#each tiers as tier}
											{@const val = tier.features[row.key]}
											<td class="text-center px-2 py-1.5
												{currentTierId === tier.id ? 'bg-[#00FF66]/5' : ''}">
												{#if row.type === 'bool'}
													{#if val}
														<Check size={12} class="inline text-[#00FF66]" />
													{:else}
														<X size={12} class="inline text-[#ff5370]/50" />
													{/if}
												{:else}
													{@const strVal = String(val)}
													<span class="{strVal === '-' ? 'text-white/20' : 'text-white/70 font-medium'}">
														{strVal}
													</span>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</div>
			</div>

		<!-- =================================================================== -->
		<!-- LICENSE MANAGEMENT -->
		<!-- =================================================================== -->
		{:else if activeSection === 'license'}
			<div class="p-4 space-y-4 max-w-xl mx-auto">
				<!-- Current license info -->
				{#if currentLicense}
					<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4 space-y-3">
						<div class="flex items-center gap-2 mb-2">
							<Shield size={16} class="text-[#00FF66]" />
							<span class="text-sm font-bold text-white/90">Active License</span>
						</div>

						<div class="grid grid-cols-2 gap-3 text-xs">
							<div>
								<span class="text-white/30 block">Plan</span>
								<span class="text-white/80 font-semibold">{currentLicense.tier}</span>
							</div>
							<div>
								<span class="text-white/30 block">Email</span>
								<span class="text-white/80">{currentLicense.email}</span>
							</div>
							<div>
								<span class="text-white/30 block">Valid Until</span>
								<span class="text-white/80">{currentLicense.valid_until}</span>
							</div>
							<div>
								<span class="text-white/30 block">Issued</span>
								<span class="text-white/80">{currentLicense.issued_at}</span>
							</div>
							{#if currentLicense.team_name}
								<div>
									<span class="text-white/30 block">Team</span>
									<span class="text-white/80">{currentLicense.team_name}</span>
								</div>
							{/if}
							{#if currentLicense.seat_count}
								<div>
									<span class="text-white/30 block">Seats</span>
									<span class="text-white/80">{currentLicense.seat_count}</span>
								</div>
							{/if}
						</div>

						<!-- License key display -->
						<div class="mt-3 pt-3 border-t border-white/5">
							<span class="text-white/30 text-xs block mb-1">License Key</span>
							<div class="flex items-center gap-2">
								<code class="flex-1 text-xs font-mono bg-[#0a0e14] rounded px-2 py-1.5 text-white/60 select-all border border-white/5">
									{showLicenseKey ? currentLicense.license_key : maskedKey}
								</code>
								<button onclick={() => showLicenseKey = !showLicenseKey}
									class="p-1.5 text-white/30 hover:text-white/60 transition-colors" title="Toggle visibility">
									{#if showLicenseKey}
										<EyeOff size={14} />
									{:else}
										<Eye size={14} />
									{/if}
								</button>
								<button onclick={copyKey}
									class="p-1.5 text-white/30 hover:text-[#00FF66] transition-colors" title="Copy key">
									<Copy size={14} />
								</button>
							</div>
						</div>

						<!-- Deactivate -->
						<button
							onclick={deactivateLicense}
							class="flex items-center gap-1.5 mt-3 px-3 py-1.5 text-xs rounded
								text-red-400/70 bg-red-500/5 border border-red-500/20
								hover:bg-red-500/10 hover:text-red-400 transition-all"
						>
							<LogOut size={12} />
							Deactivate License
						</button>
					</div>
				{:else}
					<!-- No license — show activation prompt -->
					<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4 text-center">
						<KeyRound size={28} class="mx-auto text-white/20 mb-2" />
						<p class="text-sm text-white/60 mb-1">No active license</p>
						<p class="text-xs text-white/30">You are on the Free plan. Enter a license key below to activate a paid tier.</p>
					</div>
				{/if}

				<!-- License key input -->
				<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4">
					<div class="flex items-center gap-2 mb-3">
						<KeyRound size={14} class="text-[#00FF66]" />
						<span class="text-sm font-semibold text-white/80">
							{currentLicense ? 'Change License Key' : 'Activate License'}
						</span>
					</div>

					<div class="flex gap-2">
						<input
							type="text"
							bind:value={licenseKeyInput}
							placeholder="IF-eyJhbGciOiJI..."
							class="flex-1 bg-[#0a0e14] border border-white/10 rounded px-3 py-2
								text-xs font-mono text-white/80 placeholder:text-white/20
								outline-none focus:border-[#00FF66] transition-colors"
						/>
						<button
							onclick={activateLicense}
							disabled={activating || !licenseKeyInput.trim()}
							class="px-4 py-2 rounded text-xs font-bold transition-all
								bg-[#00FF66] text-black
								hover:bg-[#00FF66]/90
								disabled:opacity-30 disabled:cursor-not-allowed
								flex items-center gap-1.5"
						>
							{#if activating}
								<Loader2 size={12} class="animate-spin" />
							{/if}
							Activate
						</button>
					</div>

					<p class="text-[10px] text-white/20 mt-2">
						License keys start with IF- followed by an encoded payload. Get yours at
						<a href="https://impforge.dev/account" target="_blank" rel="noopener" class="text-[#00FF66]/50 hover:text-[#00FF66] underline">
							impforge.dev/account
						</a>
					</p>
				</div>
			</div>

		<!-- =================================================================== -->
		<!-- USAGE STATISTICS -->
		<!-- =================================================================== -->
		{:else if activeSection === 'usage'}
			<div class="p-4 space-y-4 max-w-xl mx-auto">
				{#if usage}
					<!-- AI Completions -->
					<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4">
						<div class="flex items-center gap-2 mb-3">
							<Zap size={14} class="text-[#00FF66]" />
							<span class="text-sm font-semibold text-white/80">AI Completions Today</span>
						</div>

						<div class="flex items-end gap-3 mb-2">
							<span class="text-3xl font-bold text-white/90">{usage.ai_completions_today}</span>
							<span class="text-sm text-white/30 pb-1">
								/ {usage.ai_completions_quota === 'unlimited' ? 'unlimited' : usage.ai_completions_quota}
							</span>
						</div>

						{#if usage.ai_completions_quota !== 'unlimited'}
							<div class="w-full h-2 bg-white/5 rounded-full overflow-hidden">
								<div
									class="h-full rounded-full transition-all duration-500
										{completionPercent() > 90 ? 'bg-red-500' : completionPercent() > 70 ? 'bg-amber-500' : 'bg-[#00FF66]'}"
									style="width: {completionPercent()}%"
								></div>
							</div>
							<p class="text-[10px] text-white/20 mt-1">{completionPercent()}% used, resets at midnight UTC</p>
						{:else}
							<p class="text-[10px] text-[#00FF66]/40">Unlimited completions with your plan</p>
						{/if}
					</div>

					<!-- License Health -->
					<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4">
						<div class="flex items-center gap-2 mb-3">
							<Shield size={14} class="text-[#00FF66]" />
							<span class="text-sm font-semibold text-white/80">License Health</span>
						</div>

						<div class="grid grid-cols-2 gap-4 text-xs">
							<div>
								<span class="text-white/30 block mb-0.5">Current Tier</span>
								<span class="text-white/80 font-semibold text-sm">{usage.tier}</span>
							</div>
							<div>
								<span class="text-white/30 block mb-0.5">Expires In</span>
								<span class="text-sm font-semibold
									{usage.days_until_expiry < 7 ? 'text-red-400' :
									 usage.days_until_expiry < 30 ? 'text-amber-400' :
									 'text-white/80'}">
									{usage.days_until_expiry > 0 ? `${usage.days_until_expiry} days` : 'Expired'}
								</span>
							</div>
							<div>
								<span class="text-white/30 block mb-0.5">Last Verified</span>
								<span class="text-white/60">
									{usage.days_since_verified === 0 ? 'Today' : `${usage.days_since_verified} days ago`}
								</span>
							</div>
							<div>
								<span class="text-white/30 block mb-0.5">Grace Period</span>
								<span class="text-white/60">
									{usage.grace_period_days - usage.days_since_verified} days remaining
								</span>
							</div>
						</div>

						{#if usage.needs_phone_home}
							<div class="mt-3 px-3 py-2 rounded bg-amber-500/10 border border-amber-500/20 text-xs text-amber-400">
								License verification is due. Connect to the internet to refresh.
							</div>
						{/if}
					</div>
				{:else}
					<div class="text-center py-8 text-white/30 text-xs">
						<BarChart3 size={24} class="mx-auto mb-2 opacity-30" />
						No usage data available
					</div>
				{/if}
			</div>

		<!-- =================================================================== -->
		<!-- TEAM MANAGEMENT -->
		<!-- =================================================================== -->
		{:else if activeSection === 'team'}
			<div class="p-4 space-y-4 max-w-xl mx-auto">
				{#if !isTeamTier}
					<div class="text-center py-8">
						<Users size={32} class="mx-auto text-white/15 mb-3" />
						<p class="text-sm text-white/40 mb-1">Team features require a Team plan or higher</p>
						<button
							onclick={() => activeSection = 'pricing'}
							class="mt-2 px-4 py-1.5 rounded text-xs font-semibold
								bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30
								hover:bg-[#00FF66]/20 transition-all"
						>
							View Plans
						</button>
					</div>
				{:else}
					<!-- Team info -->
					<div class="rounded-lg border border-white/10 bg-[#0d1117] p-4">
						<div class="flex items-center justify-between mb-3">
							<div class="flex items-center gap-2">
								<Users size={14} class="text-cyan-400" />
								<span class="text-sm font-semibold text-white/80">
									{currentLicense?.team_name || 'My Team'}
								</span>
							</div>
							<button class="flex items-center gap-1 px-2 py-1 rounded text-xs
								bg-[#00FF66]/10 text-[#00FF66] border border-[#00FF66]/30
								hover:bg-[#00FF66]/20 transition-all">
								<UserPlus size={10} />
								Invite
							</button>
						</div>

						{#if currentLicense?.seat_count}
							<p class="text-xs text-white/30 mb-3">
								{teamMembers.length} / {currentLicense.seat_count} seats used
							</p>
						{/if}

						<!-- Member list -->
						<div class="space-y-1.5">
							{#each teamMembers as member}
								<div class="flex items-center gap-3 px-3 py-2 rounded bg-[#0a0e14] border border-white/5">
									<div class="w-7 h-7 rounded-full bg-[#00FF66]/10 flex items-center justify-center text-[#00FF66] text-xs font-bold">
										{member.email.charAt(0).toUpperCase()}
									</div>
									<div class="flex-1 min-w-0">
										<p class="text-xs text-white/80 truncate">{member.email}</p>
										<p class="text-[10px] text-white/30">
											{member.role}
											{#if member.joined_at}
												 -- joined {member.joined_at}
											{/if}
										</p>
									</div>
									<span class="text-[10px] px-1.5 py-0.5 rounded-full
										{member.status === 'active'
											? 'bg-[#00FF66]/10 text-[#00FF66]'
											: 'bg-amber-500/10 text-amber-400'}">
										{member.status}
									</span>
								</div>
							{:else}
								<p class="text-center text-xs text-white/30 py-4">No team members yet</p>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
