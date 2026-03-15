<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Briefcase, DollarSign, Clock, Users, FileText, Send, Plus, Timer,
		BarChart3, Loader2, Save, Sparkles, Pencil, Trash2, Check, X,
		AlertCircle, Play, Square, Receipt, TrendingUp, Calendar,
		ChevronDown, Copy, Building2, Mail, Tag, Layers, ToggleLeft, ToggleRight
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-freelancer';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');
	let cardComponent = $derived(styleEngine.getComponentStyle(widgetId, 'card'));
	let cardStyle = $derived(hasEngineStyle && cardComponent ? componentToCSS(cardComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface FreelancerProfile {
		name: string;
		title: string;
		bio: string;
		skills: string[];
		hourly_rate: number;
		portfolio_url: string;
		platforms: string[];
	}

	interface Gig {
		id: string;
		title: string;
		description: string;
		category: string;
		price_min: number;
		price_max: number;
		delivery_days: number;
		tags: string[];
		active: boolean;
		created_at: string;
	}

	interface Client {
		id: string;
		name: string;
		email: string;
		company: string;
		notes: string;
		total_spent: number;
		projects_count: number;
		created_at: string;
	}

	interface Proposal {
		id: string;
		client_name: string;
		project_title: string;
		content: string;
		status: string;
		amount: number;
		created_at: string;
	}

	interface InvoiceItem {
		description: string;
		quantity: number;
		unit_price: number;
		total: number;
	}

	interface Invoice {
		id: string;
		client_id: string;
		client_name: string;
		items: InvoiceItem[];
		total: number;
		status: string;
		due_date: string;
		created_at: string;
	}

	interface TimeEntry {
		id: string;
		project: string;
		description: string;
		start_time: string;
		end_time: string | null;
		duration_minutes: number;
		billable: boolean;
	}

	interface EarningsSummary {
		weekly: number;
		monthly: number;
		yearly: number;
		total_invoiced: number;
		total_paid: number;
		outstanding: number;
		hours_this_month: number;
		effective_rate: number;
	}

	// ---- State ---------------------------------------------------------------
	type TabId = 'profile' | 'gigs' | 'clients' | 'proposals' | 'finance';
	let activeTab = $state<TabId>('profile');
	let loading = $state(true);
	let error = $state<string | null>(null);
	let saving = $state(false);
	let saveSuccess = $state(false);

	// Profile
	let profile = $state<FreelancerProfile>({
		name: '', title: '', bio: '', skills: [], hourly_rate: 0,
		portfolio_url: '', platforms: []
	});
	let skillInput = $state('');

	// Gigs
	let gigs = $state<Gig[]>([]);
	let showGigForm = $state(false);
	let editingGig = $state<Gig | null>(null);
	let gigForm = $state({
		title: '', description: '', category: 'development', price_min: 0, price_max: 0,
		delivery_days: 7, tags: '', active: true
	});

	// Clients
	let clients = $state<Client[]>([]);
	let showClientForm = $state(false);
	let clientForm = $state({ name: '', email: '', company: '', notes: '' });

	// Proposals
	let proposals = $state<Proposal[]>([]);
	let showProposalForm = $state(false);
	let proposalForm = $state({
		client_name: '', project_title: '', content: '', amount: 0, status: 'draft'
	});
	let aiGenerating = $state(false);
	let aiRequirements = $state('');

	// Finance
	let invoices = $state<Invoice[]>([]);
	let timeEntries = $state<TimeEntry[]>([]);
	let earnings = $state<EarningsSummary | null>(null);
	let showInvoiceForm = $state(false);
	let invoiceForm = $state({
		client_id: '', due_date: '',
		items: [{ description: '', quantity: 1, unit_price: 0, total: 0 }] as InvoiceItem[]
	});
	let activeTimer = $derived(timeEntries.find(e => !e.end_time));
	let timerProject = $state('');
	let timerDescription = $state('');
	let financeSubTab = $state<'invoices' | 'time' | 'earnings'>('earnings');

	// ---- Categories ----------------------------------------------------------
	const gigCategories = [
		'development', 'design', 'writing', 'marketing', 'consulting',
		'data', 'ai-ml', 'devops', 'mobile', 'other'
	];

	const categoryLabels: Record<string, string> = {
		development: 'Development', design: 'Design', writing: 'Writing',
		marketing: 'Marketing', consulting: 'Consulting', data: 'Data',
		'ai-ml': 'AI / ML', devops: 'DevOps', mobile: 'Mobile', other: 'Other'
	};

	const statusColors: Record<string, string> = {
		draft: 'bg-gx-text-muted/20 text-gx-text-muted border-gx-text-muted/30',
		sent: 'bg-gx-accent-blue/15 text-gx-accent-blue border-gx-accent-blue/30',
		accepted: 'bg-gx-status-success/15 text-gx-status-success border-gx-status-success/30',
		rejected: 'bg-gx-status-error/15 text-gx-status-error border-gx-status-error/30',
		paid: 'bg-gx-status-success/15 text-gx-status-success border-gx-status-success/30',
	};

	const platformOptions = ['Fiverr', 'Upwork', 'Toptal', 'Freelancer.com', 'Contra', 'Direct'];

	// ---- Helpers -------------------------------------------------------------
	function formatCurrency(val: number): string {
		return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(val);
	}

	function formatDate(dateStr: string): string {
		try {
			return new Date(dateStr).toLocaleDateString('en-US', {
				month: 'short', day: 'numeric', year: 'numeric'
			});
		} catch { return dateStr; }
	}

	function formatDuration(minutes: number): string {
		const h = Math.floor(minutes / 60);
		const m = Math.round(minutes % 60);
		if (h === 0) return `${m}m`;
		return `${h}h ${m}m`;
	}

	function flashSave() {
		saveSuccess = true;
		setTimeout(() => { saveSuccess = false; }, 1500);
	}

	// ---- Data Loading --------------------------------------------------------
	async function loadAll() {
		loading = true;
		error = null;
		try {
			const [p, g, c, pr, inv, te, earn] = await Promise.all([
				invoke<FreelancerProfile>('freelancer_get_profile'),
				invoke<Gig[]>('freelancer_list_gigs'),
				invoke<Client[]>('freelancer_list_clients'),
				invoke<Proposal[]>('freelancer_list_proposals'),
				invoke<Invoice[]>('freelancer_list_invoices'),
				invoke<TimeEntry[]>('freelancer_time_entries', { project: null }),
				invoke<EarningsSummary>('freelancer_earnings_summary'),
			]);
			profile = p;
			gigs = g;
			clients = c;
			proposals = pr;
			invoices = inv;
			timeEntries = te;
			earnings = earn;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	// ---- Profile Actions -----------------------------------------------------
	async function saveProfile() {
		saving = true;
		try {
			await invoke('freelancer_save_profile', { profile });
			flashSave();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	function addSkill() {
		const s = skillInput.trim();
		if (s && !profile.skills.includes(s)) {
			profile.skills = [...profile.skills, s];
			skillInput = '';
		}
	}

	function removeSkill(skill: string) {
		profile.skills = profile.skills.filter(s => s !== skill);
	}

	function togglePlatform(p: string) {
		if (profile.platforms.includes(p)) {
			profile.platforms = profile.platforms.filter(x => x !== p);
		} else {
			profile.platforms = [...profile.platforms, p];
		}
	}

	// ---- Gig Actions ---------------------------------------------------------
	function resetGigForm() {
		gigForm = {
			title: '', description: '', category: 'development', price_min: 0,
			price_max: 0, delivery_days: 7, tags: '', active: true
		};
		editingGig = null;
	}

	function editGig(gig: Gig) {
		editingGig = gig;
		gigForm = {
			title: gig.title, description: gig.description, category: gig.category,
			price_min: gig.price_min, price_max: gig.price_max,
			delivery_days: gig.delivery_days, tags: gig.tags.join(', '), active: gig.active
		};
		showGigForm = true;
	}

	async function saveGig() {
		saving = true;
		try {
			const tags = gigForm.tags.split(',').map(t => t.trim()).filter(Boolean);
			if (editingGig) {
				await invoke('freelancer_update_gig', {
					gig: { ...editingGig, ...gigForm, tags }
				});
			} else {
				await invoke('freelancer_add_gig', {
					gig: { id: '', ...gigForm, tags, created_at: '' }
				});
			}
			gigs = await invoke<Gig[]>('freelancer_list_gigs');
			showGigForm = false;
			resetGigForm();
			flashSave();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	async function deleteGig(id: string) {
		try {
			await invoke('freelancer_delete_gig', { gigId: id });
			gigs = await invoke<Gig[]>('freelancer_list_gigs');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function toggleGigActive(gig: Gig) {
		try {
			await invoke('freelancer_update_gig', {
				gig: { ...gig, active: !gig.active }
			});
			gigs = await invoke<Gig[]>('freelancer_list_gigs');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Client Actions ------------------------------------------------------
	async function saveClient() {
		saving = true;
		try {
			await invoke('freelancer_add_client', {
				client: { id: '', ...clientForm, total_spent: 0, projects_count: 0, created_at: '' }
			});
			clients = await invoke<Client[]>('freelancer_list_clients');
			showClientForm = false;
			clientForm = { name: '', email: '', company: '', notes: '' };
			flashSave();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	// ---- Proposal Actions ----------------------------------------------------
	async function generateProposal() {
		if (!proposalForm.project_title.trim() || !aiRequirements.trim()) return;
		aiGenerating = true;
		try {
			const generated = await invoke<string>('freelancer_generate_proposal', {
				clientName: proposalForm.client_name || 'the client',
				project: proposalForm.project_title,
				requirements: aiRequirements,
			});
			proposalForm.content = generated;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			aiGenerating = false;
		}
	}

	async function saveProposal() {
		saving = true;
		try {
			await invoke('freelancer_save_proposal', {
				proposal: { id: '', ...proposalForm, created_at: '' }
			});
			proposals = await invoke<Proposal[]>('freelancer_list_proposals');
			showProposalForm = false;
			proposalForm = { client_name: '', project_title: '', content: '', amount: 0, status: 'draft' };
			aiRequirements = '';
			flashSave();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	async function updateProposalStatus(proposal: Proposal, status: string) {
		try {
			await invoke('freelancer_save_proposal', {
				proposal: { ...proposal, status }
			});
			proposals = await invoke<Proposal[]>('freelancer_list_proposals');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Invoice Actions -----------------------------------------------------
	function addInvoiceItem() {
		invoiceForm.items = [...invoiceForm.items, { description: '', quantity: 1, unit_price: 0, total: 0 }];
	}

	function removeInvoiceItem(idx: number) {
		invoiceForm.items = invoiceForm.items.filter((_, i) => i !== idx);
	}

	let invoiceTotal = $derived(
		invoiceForm.items.reduce((sum, item) => sum + item.quantity * item.unit_price, 0)
	);

	async function createInvoice() {
		if (!invoiceForm.client_id || invoiceForm.items.length === 0) return;
		saving = true;
		try {
			await invoke('freelancer_create_invoice', {
				clientId: invoiceForm.client_id,
				items: invoiceForm.items,
				dueDate: invoiceForm.due_date || new Date(Date.now() + 30 * 86400000).toISOString().split('T')[0],
			});
			invoices = await invoke<Invoice[]>('freelancer_list_invoices');
			earnings = await invoke<EarningsSummary>('freelancer_earnings_summary');
			showInvoiceForm = false;
			invoiceForm = { client_id: '', due_date: '', items: [{ description: '', quantity: 1, unit_price: 0, total: 0 }] };
			flashSave();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	async function updateInvoiceStatus(invoiceId: string, status: string) {
		try {
			await invoke('freelancer_update_invoice_status', { invoiceId, status });
			invoices = await invoke<Invoice[]>('freelancer_list_invoices');
			earnings = await invoke<EarningsSummary>('freelancer_earnings_summary');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Timer Actions -------------------------------------------------------
	async function startTimer() {
		if (!timerProject.trim()) return;
		try {
			await invoke('freelancer_start_timer', {
				project: timerProject,
				description: timerDescription,
				billable: true,
			});
			timeEntries = await invoke<TimeEntry[]>('freelancer_time_entries', { project: null });
			timerProject = '';
			timerDescription = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function stopTimer(entryId: string) {
		try {
			await invoke('freelancer_stop_timer', { entryId });
			timeEntries = await invoke<TimeEntry[]>('freelancer_time_entries', { project: null });
			earnings = await invoke<EarningsSummary>('freelancer_earnings_summary');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// ---- Clipboard -----------------------------------------------------------
	let copiedId = $state<string | null>(null);
	function copyToClipboard(text: string, id: string) {
		navigator.clipboard.writeText(text).then(() => {
			copiedId = id;
			setTimeout(() => { if (copiedId === id) copiedId = null; }, 2000);
		});
	}

	// ---- Derived Stats -------------------------------------------------------
	let activeGigs = $derived(gigs.filter(g => g.active).length);
	let totalClients = $derived(clients.length);
	let pendingProposals = $derived(proposals.filter(p => p.status === 'draft' || p.status === 'sent').length);
	let unpaidInvoices = $derived(invoices.filter(i => i.status !== 'paid').length);

	onMount(() => { loadAll(); });
</script>

<div class="flex flex-col h-full overflow-hidden" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center justify-between px-6 py-4 border-b border-gx-border-default shrink-0">
		<div class="flex items-center gap-3">
			<div class="flex items-center justify-center w-9 h-9 rounded-gx bg-gx-accent-blue/15">
				<Briefcase size={18} class="text-gx-accent-blue" />
			</div>
			<div>
				<h1 class="text-lg font-semibold text-gx-text-primary">Freelancer Hub</h1>
				<p class="text-xs text-gx-text-muted">Manage gigs, clients, proposals, and finances</p>
			</div>
		</div>

		<div class="flex items-center gap-2">
			{#if activeGigs > 0}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-status-success/30 text-gx-status-success">
					{activeGigs} active gig{activeGigs !== 1 ? 's' : ''}
				</Badge>
			{/if}
			{#if pendingProposals > 0}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-accent-blue/30 text-gx-accent-blue">
					{pendingProposals} pending
				</Badge>
			{/if}
			{#if unpaidInvoices > 0}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-status-warning/30 text-gx-status-warning">
					{unpaidInvoices} unpaid
				</Badge>
			{/if}
			{#if saveSuccess}
				<Badge variant="outline" class="text-[10px] px-2 py-0.5 border-gx-status-success/30 text-gx-status-success animate-pulse">
					<Check size={10} class="mr-1" /> Saved
				</Badge>
			{/if}
		</div>
	</div>

	<!-- Tab Navigation -->
	<div class="flex items-center gap-1 px-6 py-2 border-b border-gx-border-default shrink-0 overflow-x-auto">
		{#each [
			{ id: 'profile' as TabId, label: 'Profile', icon: Briefcase },
			{ id: 'gigs' as TabId, label: 'Gigs', icon: Layers },
			{ id: 'clients' as TabId, label: 'Clients', icon: Users },
			{ id: 'proposals' as TabId, label: 'Proposals', icon: FileText },
			{ id: 'finance' as TabId, label: 'Finance', icon: DollarSign },
		] as tab}
			<button
				onclick={() => activeTab = tab.id}
				class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx transition-all whitespace-nowrap
					{activeTab === tab.id
						? 'bg-gx-bg-elevated text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}"
			>
				<tab.icon size={13} />
				{tab.label}
				{#if tab.id === 'proposals' && pendingProposals > 0}
					<span class="inline-flex items-center justify-center w-4 h-4 text-[9px] font-bold rounded-full bg-gx-accent-blue/20 text-gx-accent-blue">
						{pendingProposals}
					</span>
				{/if}
				{#if tab.id === 'finance' && unpaidInvoices > 0}
					<span class="inline-flex items-center justify-center w-4 h-4 text-[9px] font-bold rounded-full bg-gx-status-warning/20 text-gx-status-warning">
						{unpaidInvoices}
					</span>
				{/if}
			</button>
		{/each}
	</div>

	<!-- Content Area -->
	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex items-center justify-center h-64">
				<div class="flex items-center gap-3 text-gx-text-muted">
					<Loader2 size={20} class="animate-spin" />
					<span class="text-sm">Loading freelancer data...</span>
				</div>
			</div>
		{:else if error}
			<div class="flex items-center justify-center h-64">
				<div class="flex flex-col items-center gap-3 text-center">
					<AlertCircle size={32} class="text-gx-status-error" />
					<p class="text-sm text-gx-status-error">{error}</p>
					<button onclick={() => { error = null; loadAll(); }} class="text-xs text-gx-neon hover:underline">Try again</button>
				</div>
			</div>
		{:else}

		<!-- =========== PROFILE TAB =========== -->
		{#if activeTab === 'profile'}
			<div class="p-6 max-w-3xl mx-auto space-y-6">
				<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
					<CardHeader class="pb-2">
						<div class="flex items-center gap-2">
							<Briefcase size={14} class="text-gx-accent-blue" />
							<CardTitle class="text-sm font-medium text-gx-text-primary">Freelancer Profile</CardTitle>
						</div>
					</CardHeader>
					<CardContent class="space-y-4">
						<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
							<div>
								<label for="prof-name" class="text-[11px] text-gx-text-muted mb-1 block">Full Name</label>
								<input id="prof-name" type="text" bind:value={profile.name} placeholder="Your name"
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
							</div>
							<div>
								<label for="prof-title" class="text-[11px] text-gx-text-muted mb-1 block">Professional Title</label>
								<input id="prof-title" type="text" bind:value={profile.title} placeholder="e.g., Full-Stack Developer"
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
							</div>
						</div>

						<div>
							<label for="prof-bio" class="text-[11px] text-gx-text-muted mb-1 block">Bio</label>
							<textarea id="prof-bio" bind:value={profile.bio} rows="4" placeholder="Tell clients about yourself..."
								class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none resize-y leading-relaxed"></textarea>
						</div>

						<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
							<div>
								<label for="prof-rate" class="text-[11px] text-gx-text-muted mb-1 block">Hourly Rate (USD)</label>
								<input id="prof-rate" type="number" bind:value={profile.hourly_rate} min="0" step="5"
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
							</div>
							<div>
								<label for="prof-portfolio" class="text-[11px] text-gx-text-muted mb-1 block">Portfolio URL</label>
								<input id="prof-portfolio" type="url" bind:value={profile.portfolio_url} placeholder="https://..."
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
							</div>
						</div>

						<!-- Skills -->
						<div>
							<span class="text-[11px] text-gx-text-muted mb-2 block">Skills</span>
							<div class="flex items-center gap-2 mb-2">
								<input type="text" bind:value={skillInput} placeholder="Add a skill..."
									onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); addSkill(); } }}
									class="flex-1 px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								<button onclick={addSkill} class="px-3 py-1.5 text-xs rounded-gx bg-gx-bg-elevated text-gx-text-secondary hover:text-gx-neon border border-gx-border-default hover:border-gx-neon/30 transition-all">
									<Plus size={12} />
								</button>
							</div>
							<div class="flex flex-wrap gap-1.5">
								{#each profile.skills as skill}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] rounded bg-gx-accent-blue/10 text-gx-accent-blue border border-gx-accent-blue/20">
										{skill}
										<button onclick={() => removeSkill(skill)} class="hover:text-gx-status-error transition-colors"><X size={10} /></button>
									</span>
								{/each}
							</div>
						</div>

						<!-- Platforms -->
						<div>
							<span class="text-[11px] text-gx-text-muted mb-2 block">Active Platforms</span>
							<div class="flex flex-wrap gap-2">
								{#each platformOptions as p}
									{@const selected = profile.platforms.includes(p)}
									<button onclick={() => togglePlatform(p)}
										class="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-gx border transition-all
											{selected
												? 'bg-gx-neon/15 text-gx-neon border-gx-neon/40'
												: 'bg-gx-bg-primary text-gx-text-muted border-gx-border-default hover:border-gx-text-muted/50'}">
										{p}
										{#if selected}<Check size={10} />{/if}
									</button>
								{/each}
							</div>
						</div>

						<div class="pt-2">
							<button onclick={saveProfile} disabled={saving}
								class="flex items-center gap-1.5 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
								{#if saving}<Loader2 size={13} class="animate-spin" />{:else}<Save size={13} />{/if}
								Save Profile
							</button>
						</div>
					</CardContent>
				</Card>
			</div>

		<!-- =========== GIGS TAB =========== -->
		{:else if activeTab === 'gigs'}
			<div class="p-6 space-y-4">
				<div class="flex items-center justify-between">
					<span class="text-sm text-gx-text-secondary font-medium">{gigs.length} service{gigs.length !== 1 ? 's' : ''}</span>
					<button onclick={() => { resetGigForm(); showGigForm = true; }}
						class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all">
						<Plus size={14} /> New Gig
					</button>
				</div>

				<!-- Gig Form -->
				{#if showGigForm}
					<Card class="bg-gx-bg-secondary border-gx-neon/20" style={cardStyle}>
						<CardContent class="p-4 space-y-3">
							<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
								<div>
									<label for="gig-title" class="text-[11px] text-gx-text-muted mb-1 block">Title</label>
									<input id="gig-title" type="text" bind:value={gigForm.title} placeholder="Service title..."
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="gig-cat" class="text-[11px] text-gx-text-muted mb-1 block">Category</label>
									<select id="gig-cat" bind:value={gigForm.category}
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none">
										{#each gigCategories as cat}
											<option value={cat}>{categoryLabels[cat]}</option>
										{/each}
									</select>
								</div>
							</div>
							<div>
								<label for="gig-desc" class="text-[11px] text-gx-text-muted mb-1 block">Description</label>
								<textarea id="gig-desc" bind:value={gigForm.description} rows="3" placeholder="Describe what you offer..."
									class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none resize-y leading-relaxed"></textarea>
							</div>
							<div class="grid grid-cols-3 gap-3">
								<div>
									<label for="gig-pmin" class="text-[11px] text-gx-text-muted mb-1 block">Min Price ($)</label>
									<input id="gig-pmin" type="number" bind:value={gigForm.price_min} min="0"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="gig-pmax" class="text-[11px] text-gx-text-muted mb-1 block">Max Price ($)</label>
									<input id="gig-pmax" type="number" bind:value={gigForm.price_max} min="0"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="gig-delivery" class="text-[11px] text-gx-text-muted mb-1 block">Delivery (days)</label>
									<input id="gig-delivery" type="number" bind:value={gigForm.delivery_days} min="1"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
								</div>
							</div>
							<div>
								<label for="gig-tags" class="text-[11px] text-gx-text-muted mb-1 block">Tags (comma-separated)</label>
								<input id="gig-tags" type="text" bind:value={gigForm.tags} placeholder="react, typescript, api..."
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
							</div>
							<div class="flex items-center gap-3 pt-2">
								<button onclick={saveGig} disabled={saving || !gigForm.title.trim()}
									class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
									{#if saving}<Loader2 size={13} class="animate-spin" />{:else}<Save size={13} />{/if}
									{editingGig ? 'Update' : 'Create'} Gig
								</button>
								<button onclick={() => { showGigForm = false; resetGigForm(); }}
									class="px-4 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-secondary transition-colors">Cancel</button>
							</div>
						</CardContent>
					</Card>
				{/if}

				<!-- Gig List -->
				{#if gigs.length === 0 && !showGigForm}
					<div class="flex flex-col items-center justify-center py-16 text-center">
						<Layers size={40} class="text-gx-text-muted/30 mb-3" />
						<p class="text-sm text-gx-text-muted">No gigs yet</p>
						<button onclick={() => { resetGigForm(); showGigForm = true; }} class="mt-3 text-xs text-gx-neon hover:underline">Create your first service</button>
					</div>
				{:else}
					<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
						{#each gigs as gig (gig.id)}
							<Card class="bg-gx-bg-secondary border-gx-border-default hover:border-gx-neon/20 transition-all {gig.active ? '' : 'opacity-60'}" style={cardStyle}>
								<CardContent class="p-4 space-y-3">
									<div class="flex items-start justify-between">
										<div class="flex-1 min-w-0">
											<h3 class="text-xs font-medium text-gx-text-primary truncate">{gig.title}</h3>
											<span class="text-[10px] text-gx-text-muted">{categoryLabels[gig.category] ?? gig.category}</span>
										</div>
										<button onclick={() => toggleGigActive(gig)} title={gig.active ? 'Deactivate' : 'Activate'}
											class="text-gx-text-muted hover:text-gx-neon transition-colors ml-2">
											{#if gig.active}<ToggleRight size={18} class="text-gx-status-success" />{:else}<ToggleLeft size={18} />{/if}
										</button>
									</div>
									<p class="text-[11px] text-gx-text-muted leading-relaxed line-clamp-2">{gig.description}</p>
									<div class="flex items-center gap-3 text-[11px]">
										<span class="text-gx-status-success font-medium">{formatCurrency(gig.price_min)} - {formatCurrency(gig.price_max)}</span>
										<span class="text-gx-text-muted">{gig.delivery_days}d delivery</span>
									</div>
									{#if gig.tags.length > 0}
										<div class="flex flex-wrap gap-1">
											{#each gig.tags.slice(0, 4) as tag}
												<span class="text-[9px] px-1.5 py-0.5 rounded bg-gx-bg-elevated text-gx-text-muted border border-gx-border-default">{tag}</span>
											{/each}
											{#if gig.tags.length > 4}
												<span class="text-[9px] text-gx-text-muted">+{gig.tags.length - 4}</span>
											{/if}
										</div>
									{/if}
									<div class="flex items-center gap-1 pt-1">
										<button onclick={() => editGig(gig)} class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all">
											<Pencil size={12} />
										</button>
										<button onclick={() => deleteGig(gig.id)} class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-all">
											<Trash2 size={12} />
										</button>
									</div>
								</CardContent>
							</Card>
						{/each}
					</div>
				{/if}
			</div>

		<!-- =========== CLIENTS TAB =========== -->
		{:else if activeTab === 'clients'}
			<div class="p-6 space-y-4">
				<div class="flex items-center justify-between">
					<span class="text-sm text-gx-text-secondary font-medium">{clients.length} client{clients.length !== 1 ? 's' : ''}</span>
					<button onclick={() => { showClientForm = !showClientForm; }}
						class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all">
						<Plus size={14} /> Add Client
					</button>
				</div>

				{#if showClientForm}
					<Card class="bg-gx-bg-secondary border-gx-neon/20" style={cardStyle}>
						<CardContent class="p-4 space-y-3">
							<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
								<div>
									<label for="client-name" class="text-[11px] text-gx-text-muted mb-1 block">Name</label>
									<input id="client-name" type="text" bind:value={clientForm.name} placeholder="Client name"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="client-email" class="text-[11px] text-gx-text-muted mb-1 block">Email</label>
									<input id="client-email" type="email" bind:value={clientForm.email} placeholder="email@example.com"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								</div>
							</div>
							<div>
								<label for="client-company" class="text-[11px] text-gx-text-muted mb-1 block">Company</label>
								<input id="client-company" type="text" bind:value={clientForm.company} placeholder="Company name (optional)"
									class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
							</div>
							<div>
								<label for="client-notes" class="text-[11px] text-gx-text-muted mb-1 block">Notes</label>
								<textarea id="client-notes" bind:value={clientForm.notes} rows="2" placeholder="Notes about this client..."
									class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none resize-y leading-relaxed"></textarea>
							</div>
							<div class="flex items-center gap-3 pt-1">
								<button onclick={saveClient} disabled={saving || !clientForm.name.trim()}
									class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
									{#if saving}<Loader2 size={13} class="animate-spin" />{:else}<Save size={13} />{/if}
									Save Client
								</button>
								<button onclick={() => showClientForm = false} class="px-4 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-secondary transition-colors">Cancel</button>
							</div>
						</CardContent>
					</Card>
				{/if}

				{#if clients.length === 0 && !showClientForm}
					<div class="flex flex-col items-center justify-center py-16 text-center">
						<Users size={40} class="text-gx-text-muted/30 mb-3" />
						<p class="text-sm text-gx-text-muted">No clients yet</p>
						<button onclick={() => showClientForm = true} class="mt-3 text-xs text-gx-neon hover:underline">Add your first client</button>
					</div>
				{:else}
					<div class="space-y-3">
						{#each clients as client (client.id)}
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4">
									<div class="flex items-start gap-3">
										<div class="flex items-center justify-center w-9 h-9 rounded-gx bg-gx-bg-elevated shrink-0">
											<Users size={16} class="text-gx-accent-blue" />
										</div>
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="text-xs font-medium text-gx-text-primary">{client.name}</span>
												{#if client.company}
													<span class="text-[10px] text-gx-text-muted flex items-center gap-1"><Building2 size={10} />{client.company}</span>
												{/if}
											</div>
											{#if client.email}
												<div class="text-[10px] text-gx-text-muted flex items-center gap-1 mt-0.5"><Mail size={10} />{client.email}</div>
											{/if}
											{#if client.notes}
												<p class="text-[11px] text-gx-text-muted mt-1 leading-relaxed line-clamp-2">{client.notes}</p>
											{/if}
										</div>
										<div class="text-right shrink-0 space-y-1">
											<div class="text-xs font-medium text-gx-status-success">{formatCurrency(client.total_spent)}</div>
											<div class="text-[10px] text-gx-text-muted">{client.projects_count} project{client.projects_count !== 1 ? 's' : ''}</div>
										</div>
									</div>
								</CardContent>
							</Card>
						{/each}
					</div>
				{/if}
			</div>

		<!-- =========== PROPOSALS TAB =========== -->
		{:else if activeTab === 'proposals'}
			<div class="p-6 space-y-4">
				<div class="flex items-center justify-between">
					<span class="text-sm text-gx-text-secondary font-medium">{proposals.length} proposal{proposals.length !== 1 ? 's' : ''}</span>
					<button onclick={() => { showProposalForm = !showProposalForm; }}
						class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all">
						<Plus size={14} /> New Proposal
					</button>
				</div>

				{#if showProposalForm}
					<Card class="bg-gx-bg-secondary border-gx-neon/20" style={cardStyle}>
						<CardHeader class="pb-2">
							<div class="flex items-center gap-2">
								<Sparkles size={14} class="text-gx-accent-magenta" />
								<CardTitle class="text-sm font-medium text-gx-text-primary">AI Proposal Generator</CardTitle>
								<Badge variant="outline" class="text-[9px] px-1.5 py-0 border-gx-accent-magenta/30 text-gx-accent-magenta">Ollama</Badge>
							</div>
						</CardHeader>
						<CardContent class="space-y-3">
							<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
								<div>
									<label for="prop-client" class="text-[11px] text-gx-text-muted mb-1 block">Client Name</label>
									<input id="prop-client" type="text" bind:value={proposalForm.client_name} placeholder="Client name"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="prop-title" class="text-[11px] text-gx-text-muted mb-1 block">Project Title</label>
									<input id="prop-title" type="text" bind:value={proposalForm.project_title} placeholder="Project name"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
								</div>
								<div>
									<label for="prop-amount" class="text-[11px] text-gx-text-muted mb-1 block">Proposed Amount ($)</label>
									<input id="prop-amount" type="number" bind:value={proposalForm.amount} min="0"
										class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
								</div>
							</div>

							<!-- AI Requirements Input -->
							<div>
								<label for="prop-reqs" class="text-[11px] text-gx-text-muted mb-1 block">Project Requirements (for AI generation)</label>
								<textarea id="prop-reqs" bind:value={aiRequirements} rows="3" placeholder="Describe the project requirements and what the client needs..."
									class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none resize-y leading-relaxed"></textarea>
							</div>

							<button onclick={generateProposal} disabled={aiGenerating || !proposalForm.project_title.trim() || !aiRequirements.trim()}
								class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-accent-magenta/15 text-gx-accent-magenta border border-gx-accent-magenta/30 hover:bg-gx-accent-magenta/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
								{#if aiGenerating}<Loader2 size={13} class="animate-spin" /> Generating...{:else}<Sparkles size={13} /> Generate with AI{/if}
							</button>

							<Separator class="bg-gx-border-default" />

							<!-- Content Editor -->
							<div>
								<label for="prop-content" class="text-[11px] text-gx-text-muted mb-1 block">Proposal Content</label>
								<textarea id="prop-content" bind:value={proposalForm.content} rows="10" placeholder="Write or generate your proposal..."
									class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none resize-y font-mono leading-relaxed"></textarea>
							</div>

							<div class="flex items-center gap-3 pt-1">
								<button onclick={saveProposal} disabled={saving || !proposalForm.content.trim()}
									class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
									{#if saving}<Loader2 size={13} class="animate-spin" />{:else}<Save size={13} />{/if}
									Save Proposal
								</button>
								<button onclick={() => { showProposalForm = false; }} class="px-4 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-secondary transition-colors">Cancel</button>
							</div>
						</CardContent>
					</Card>
				{/if}

				<!-- Proposal List -->
				{#if proposals.length === 0 && !showProposalForm}
					<div class="flex flex-col items-center justify-center py-16 text-center">
						<FileText size={40} class="text-gx-text-muted/30 mb-3" />
						<p class="text-sm text-gx-text-muted">No proposals yet</p>
						<button onclick={() => showProposalForm = true} class="mt-3 text-xs text-gx-neon hover:underline">Draft your first proposal</button>
					</div>
				{:else}
					<div class="space-y-3">
						{#each proposals as proposal (proposal.id)}
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4">
									<div class="flex items-start gap-3">
										<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated shrink-0 mt-0.5">
											<FileText size={14} class="text-gx-accent-blue" />
										</div>
										<div class="flex-1 min-w-0 space-y-2">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="text-xs font-medium text-gx-text-primary">{proposal.project_title}</span>
												<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors[proposal.status] ?? statusColors.draft}">
													{proposal.status}
												</Badge>
												{#if proposal.amount > 0}
													<span class="text-[10px] text-gx-status-success font-medium">{formatCurrency(proposal.amount)}</span>
												{/if}
											</div>
											<div class="text-[10px] text-gx-text-muted">Client: {proposal.client_name || 'N/A'}</div>
											<div class="text-[11px] text-gx-text-secondary whitespace-pre-wrap line-clamp-3 font-mono leading-relaxed bg-gx-bg-primary rounded p-2">
												{proposal.content}
											</div>
											<div class="flex items-center gap-2 text-[10px] text-gx-text-muted">
												<span>Created {formatDate(proposal.created_at)}</span>
											</div>
										</div>
										<div class="flex flex-col items-center gap-1 shrink-0">
											<button onclick={() => copyToClipboard(proposal.content, proposal.id)} title="Copy"
												class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover transition-all">
												{#if copiedId === proposal.id}<Check size={13} class="text-gx-status-success" />{:else}<Copy size={13} />{/if}
											</button>
											{#if proposal.status === 'draft'}
												<button onclick={() => updateProposalStatus(proposal, 'sent')} title="Mark as sent"
													class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-accent-blue hover:bg-gx-accent-blue/10 transition-all">
													<Send size={13} />
												</button>
											{/if}
											{#if proposal.status === 'sent'}
												<button onclick={() => updateProposalStatus(proposal, 'accepted')} title="Mark accepted"
													class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-success hover:bg-gx-status-success/10 transition-all">
													<Check size={13} />
												</button>
												<button onclick={() => updateProposalStatus(proposal, 'rejected')} title="Mark rejected"
													class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 transition-all">
													<X size={13} />
												</button>
											{/if}
										</div>
									</div>
								</CardContent>
							</Card>
						{/each}
					</div>
				{/if}
			</div>

		<!-- =========== FINANCE TAB =========== -->
		{:else if activeTab === 'finance'}
			<div class="p-6 space-y-6">
				<!-- Finance Sub-tabs -->
				<div class="flex items-center gap-1 mb-2">
					{#each [
						{ id: 'earnings' as const, label: 'Earnings', icon: BarChart3 },
						{ id: 'invoices' as const, label: 'Invoices', icon: Receipt },
						{ id: 'time' as const, label: 'Time Tracking', icon: Timer },
					] as sub}
						<button onclick={() => financeSubTab = sub.id}
							class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx transition-all
								{financeSubTab === sub.id
									? 'bg-gx-bg-elevated text-gx-accent-blue border border-gx-accent-blue/30'
									: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover border border-transparent'}">
							<sub.icon size={12} />
							{sub.label}
						</button>
					{/each}
				</div>

				<!-- ---- EARNINGS SUB-TAB ---- -->
				{#if financeSubTab === 'earnings'}
					{#if earnings}
						<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4 text-center">
									<DollarSign size={16} class="text-gx-status-success mx-auto mb-1" />
									<div class="text-xl font-bold text-gx-text-primary">{formatCurrency(earnings.weekly)}</div>
									<div class="text-[10px] text-gx-text-muted mt-1">This Week</div>
								</CardContent>
							</Card>
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4 text-center">
									<TrendingUp size={16} class="text-gx-accent-blue mx-auto mb-1" />
									<div class="text-xl font-bold text-gx-text-primary">{formatCurrency(earnings.monthly)}</div>
									<div class="text-[10px] text-gx-text-muted mt-1">This Month</div>
								</CardContent>
							</Card>
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4 text-center">
									<BarChart3 size={16} class="text-gx-accent-magenta mx-auto mb-1" />
									<div class="text-xl font-bold text-gx-text-primary">{formatCurrency(earnings.yearly)}</div>
									<div class="text-[10px] text-gx-text-muted mt-1">This Year</div>
								</CardContent>
							</Card>
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4 text-center">
									<Clock size={16} class="text-gx-status-warning mx-auto mb-1" />
									<div class="text-xl font-bold text-gx-text-primary">{earnings.hours_this_month.toFixed(1)}h</div>
									<div class="text-[10px] text-gx-text-muted mt-1">Hours This Month</div>
								</CardContent>
							</Card>
						</div>

						<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4">
									<div class="text-[11px] text-gx-text-muted mb-1">Total Invoiced</div>
									<div class="text-lg font-bold text-gx-text-primary">{formatCurrency(earnings.total_invoiced)}</div>
								</CardContent>
							</Card>
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4">
									<div class="text-[11px] text-gx-text-muted mb-1">Outstanding</div>
									<div class="text-lg font-bold text-gx-status-warning">{formatCurrency(earnings.outstanding)}</div>
								</CardContent>
							</Card>
							<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
								<CardContent class="p-4">
									<div class="text-[11px] text-gx-text-muted mb-1">Effective Rate</div>
									<div class="text-lg font-bold text-gx-accent-blue">{formatCurrency(earnings.effective_rate)}/hr</div>
								</CardContent>
							</Card>
						</div>
					{/if}

				<!-- ---- INVOICES SUB-TAB ---- -->
				{:else if financeSubTab === 'invoices'}
					<div class="flex items-center justify-between mb-4">
						<span class="text-sm text-gx-text-secondary font-medium">{invoices.length} invoice{invoices.length !== 1 ? 's' : ''}</span>
						<button onclick={() => { showInvoiceForm = !showInvoiceForm; }}
							class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all">
							<Plus size={14} /> New Invoice
						</button>
					</div>

					{#if showInvoiceForm}
						<Card class="bg-gx-bg-secondary border-gx-neon/20 mb-4" style={cardStyle}>
							<CardContent class="p-4 space-y-3">
								<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
									<div>
										<label for="inv-client" class="text-[11px] text-gx-text-muted mb-1 block">Client</label>
										<select id="inv-client" bind:value={invoiceForm.client_id}
											class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none">
											<option value="">Select client...</option>
											{#each clients as c}
												<option value={c.id}>{c.name}{c.company ? ` (${c.company})` : ''}</option>
											{/each}
										</select>
									</div>
									<div>
										<label for="inv-due" class="text-[11px] text-gx-text-muted mb-1 block">Due Date</label>
										<input id="inv-due" type="date" bind:value={invoiceForm.due_date}
											class="w-full px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none" />
									</div>
								</div>

								<!-- Line Items -->
								<div class="space-y-2">
									<span class="text-[11px] text-gx-text-muted">Line Items</span>
									{#each invoiceForm.items as item, idx}
										<div class="flex items-center gap-2">
											<input type="text" bind:value={item.description} placeholder="Description"
												class="flex-1 px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
											<input type="number" bind:value={item.quantity} min="0.25" step="0.25" placeholder="Qty"
												class="w-16 px-2 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none text-center" />
											<input type="number" bind:value={item.unit_price} min="0" step="1" placeholder="Price"
												class="w-24 px-2 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary focus:border-gx-neon/50 focus:outline-none text-right" />
											<span class="text-[11px] text-gx-text-muted w-20 text-right">{formatCurrency(item.quantity * item.unit_price)}</span>
											{#if invoiceForm.items.length > 1}
												<button onclick={() => removeInvoiceItem(idx)} class="text-gx-text-muted hover:text-gx-status-error transition-colors"><X size={14} /></button>
											{/if}
										</div>
									{/each}
									<button onclick={addInvoiceItem} class="text-[11px] text-gx-neon hover:underline">+ Add item</button>
								</div>

								<div class="flex items-center justify-between pt-2 border-t border-gx-border-default">
									<span class="text-sm font-medium text-gx-text-primary">Total: {formatCurrency(invoiceTotal)}</span>
									<div class="flex items-center gap-2">
										<button onclick={createInvoice} disabled={saving || !invoiceForm.client_id}
											class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
											{#if saving}<Loader2 size={13} class="animate-spin" />{:else}<Receipt size={13} />{/if}
											Create Invoice
										</button>
										<button onclick={() => showInvoiceForm = false} class="px-4 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-secondary transition-colors">Cancel</button>
									</div>
								</div>
							</CardContent>
						</Card>
					{/if}

					{#if invoices.length === 0 && !showInvoiceForm}
						<div class="flex flex-col items-center justify-center py-16 text-center">
							<Receipt size={40} class="text-gx-text-muted/30 mb-3" />
							<p class="text-sm text-gx-text-muted">No invoices yet</p>
							<button onclick={() => showInvoiceForm = true} class="mt-3 text-xs text-gx-neon hover:underline">Create your first invoice</button>
						</div>
					{:else}
						<div class="space-y-3">
							{#each invoices as inv (inv.id)}
								<Card class="bg-gx-bg-secondary border-gx-border-default" style={cardStyle}>
									<CardContent class="p-4">
										<div class="flex items-center gap-3">
											<div class="flex items-center justify-center w-8 h-8 rounded-gx bg-gx-bg-elevated shrink-0">
												<Receipt size={14} class="text-gx-accent-blue" />
											</div>
											<div class="flex-1 min-w-0">
												<div class="flex items-center gap-2 flex-wrap">
													<span class="text-xs font-mono font-medium text-gx-text-primary">{inv.id}</span>
													<Badge variant="outline" class="text-[9px] px-1.5 py-0 {statusColors[inv.status] ?? statusColors.draft}">
														{inv.status}
													</Badge>
													<span class="text-[10px] text-gx-text-muted">{inv.client_name}</span>
												</div>
												<div class="text-[10px] text-gx-text-muted mt-0.5">
													{inv.items.length} item{inv.items.length !== 1 ? 's' : ''} | Due {formatDate(inv.due_date)} | Created {formatDate(inv.created_at)}
												</div>
											</div>
											<div class="text-right shrink-0">
												<div class="text-sm font-bold text-gx-text-primary">{formatCurrency(inv.total)}</div>
											</div>
											<div class="flex items-center gap-1 shrink-0">
												{#if inv.status === 'draft'}
													<button onclick={() => updateInvoiceStatus(inv.id, 'sent')} title="Mark as sent"
														class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-accent-blue hover:bg-gx-accent-blue/10 transition-all">
														<Send size={13} />
													</button>
												{/if}
												{#if inv.status === 'sent'}
													<button onclick={() => updateInvoiceStatus(inv.id, 'paid')} title="Mark as paid"
														class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-text-muted hover:text-gx-status-success hover:bg-gx-status-success/10 transition-all">
														<Check size={13} />
													</button>
												{/if}
											</div>
										</div>
									</CardContent>
								</Card>
							{/each}
						</div>
					{/if}

				<!-- ---- TIME TRACKING SUB-TAB ---- -->
				{:else if financeSubTab === 'time'}
					<!-- Start Timer -->
					<Card class="bg-gx-bg-secondary border-gx-border-default mb-4" style={cardStyle}>
						<CardContent class="p-4">
							{#if activeTimer}
								<div class="flex items-center gap-3">
									<div class="w-2 h-2 rounded-full bg-gx-status-success animate-pulse shrink-0"></div>
									<div class="flex-1 min-w-0">
										<div class="text-xs font-medium text-gx-text-primary">Timer running: {activeTimer.project}</div>
										<div class="text-[10px] text-gx-text-muted">{activeTimer.description || 'No description'} | Started {formatDate(activeTimer.start_time)}</div>
									</div>
									<button onclick={() => stopTimer(activeTimer!.id)}
										class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-status-error/15 text-gx-status-error border border-gx-status-error/30 hover:bg-gx-status-error/25 transition-all">
										<Square size={12} /> Stop
									</button>
								</div>
							{:else}
								<div class="flex items-center gap-2">
									<input type="text" bind:value={timerProject} placeholder="Project name"
										class="flex-1 px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
									<input type="text" bind:value={timerDescription} placeholder="What are you working on?"
										class="flex-1 px-3 py-1.5 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none" />
									<button onclick={startTimer} disabled={!timerProject.trim()}
										class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-gx bg-gx-status-success/15 text-gx-status-success border border-gx-status-success/30 hover:bg-gx-status-success/25 transition-all disabled:opacity-40 disabled:cursor-not-allowed">
										<Play size={12} /> Start
									</button>
								</div>
							{/if}
						</CardContent>
					</Card>

					<!-- Time Entries -->
					{#if timeEntries.length === 0}
						<div class="flex flex-col items-center justify-center py-12 text-center">
							<Timer size={40} class="text-gx-text-muted/30 mb-3" />
							<p class="text-sm text-gx-text-muted">No time entries yet</p>
							<p class="text-[11px] text-gx-text-muted mt-1">Start a timer above to track your work</p>
						</div>
					{:else}
						<div class="space-y-2">
							{#each timeEntries as entry (entry.id)}
								<div class="flex items-center gap-3 px-4 py-2.5 rounded-gx bg-gx-bg-secondary border border-gx-border-default">
									<div class="w-1.5 h-1.5 rounded-full shrink-0 {entry.end_time ? 'bg-gx-text-muted' : 'bg-gx-status-success animate-pulse'}"></div>
									<div class="flex-1 min-w-0">
										<div class="text-xs font-medium text-gx-text-primary">{entry.project}</div>
										<div class="text-[10px] text-gx-text-muted">{entry.description || 'No description'}</div>
									</div>
									<div class="text-right shrink-0">
										<div class="text-xs font-mono text-gx-text-primary">{formatDuration(entry.duration_minutes)}</div>
										<div class="text-[9px] text-gx-text-muted">{entry.billable ? 'Billable' : 'Non-billable'}</div>
									</div>
									{#if !entry.end_time}
										<button onclick={() => stopTimer(entry.id)}
											class="flex items-center justify-center w-7 h-7 rounded-gx text-gx-status-error hover:bg-gx-status-error/10 transition-all">
											<Square size={12} />
										</button>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				{/if}
			</div>
		{/if}
		{/if}
	</div>
</div>
