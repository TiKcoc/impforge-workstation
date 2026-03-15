<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Users, Plus, Copy, Check, X, Loader2, Send, Pin, PinOff,
		MessageSquare, Activity, Settings as SettingsIcon, BookOpen,
		Crown, ShieldCheck, Eye, UserPlus, UserMinus, LogOut, Trash2,
		Bot, Clock, TrendingUp, Search, Filter, Hash, Pencil,
		Sparkles, FileText, CheckSquare, Lightbulb, Code2, BarChart3,
		MessageCircle, Flag, ChevronDown, ChevronUp, AlertCircle
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-team';
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
	interface TeamMeta {
		id: string;
		name: string;
		member_count: number;
		created_at: string;
		invite_code: string;
	}

	interface TeamMember {
		id: string;
		name: string;
		role: string;
		status: string;
		active_agents: string[];
		last_seen: string;
		trust_score: number;
		contributions: number;
	}

	interface Team {
		id: string;
		name: string;
		members: TeamMember[];
		created_at: string;
		invite_code: string;
	}

	interface ImpBookEntry {
		id: string;
		author_id: string;
		author_name: string;
		entry_type: string;
		title: string;
		content: string;
		source_agent: string | null;
		source_module: string | null;
		tags: string[];
		reactions: Reaction[];
		comments: Comment[];
		attachments: Attachment[];
		created_at: string;
		updated_at: string;
		pinned: boolean;
	}

	interface Comment {
		id: string;
		author_id: string;
		author_name: string;
		content: string;
		created_at: string;
		reactions: Reaction[];
	}

	interface Reaction {
		emoji: string;
		user_id: string;
		user_name: string;
	}

	interface Attachment {
		name: string;
		file_type: string;
		size_bytes: number;
		path: string;
	}

	interface TeamActivity {
		id: string;
		team_id: string;
		user_id: string;
		user_name: string;
		action: string;
		target: string;
		timestamp: string;
	}

	// ---- State ---------------------------------------------------------------
	let activeTab = $state<'dashboard' | 'impbook' | 'activity' | 'settings'>('dashboard');
	let loading = $state(false);
	let error = $state('');

	// Team list / selection
	let teams = $state<TeamMeta[]>([]);
	let selectedTeamId = $state('');
	let selectedTeam = $state<Team | null>(null);

	// Create / Join dialogs
	let showCreateDialog = $state(false);
	let showJoinDialog = $state(false);
	let newTeamName = $state('');
	let joinCode = $state('');
	let joinUserName = $state('');
	let dialogLoading = $state(false);

	// ImpBook
	let entries = $state<ImpBookEntry[]>([]);
	let entryFilter = $state('');
	let entryTypeFilter = $state('');
	let entrySearch = $state('');
	let showCreateEntry = $state(false);
	let newEntryType = $state('document');
	let newEntryTitle = $state('');
	let newEntryContent = $state('');
	let newEntryTags = $state('');
	let expandedComments = $state<Set<string>>(new Set());
	let commentInputs = $state<Record<string, string>>({});

	// Activity
	let activities = $state<TeamActivity[]>([]);

	// Clipboard feedback
	let codeCopied = $state(false);

	// Polling
	let pollTimer: ReturnType<typeof setInterval> | null = null;

	// ---- Entry type metadata -------------------------------------------------
	const ENTRY_TYPES = [
		{ value: 'agent_result', label: 'Agent Result', icon: Bot, color: 'text-gx-accent-blue' },
		{ value: 'document', label: 'Document', icon: FileText, color: 'text-gx-text-secondary' },
		{ value: 'task', label: 'Task', icon: CheckSquare, color: 'text-gx-status-warning' },
		{ value: 'idea', label: 'Idea', icon: Lightbulb, color: 'text-gx-accent-magenta' },
		{ value: 'code_review', label: 'Code Review', icon: Code2, color: 'text-gx-neon' },
		{ value: 'report', label: 'Report', icon: BarChart3, color: 'text-gx-accent-purple' },
		{ value: 'discussion', label: 'Discussion', icon: MessageCircle, color: 'text-gx-accent-blue' },
		{ value: 'milestone', label: 'Milestone', icon: Flag, color: 'text-gx-status-success' },
	];

	const REACTION_EMOJIS = ['\u{1f44d}', '\u{1f389}', '\u{1f680}', '\u{2764}\u{fe0f}', '\u{1f914}', '\u{1f440}'];

	// ---- Derived -------------------------------------------------------------
	let filteredEntries = $derived.by(() => {
		let result = [...entries];

		// Pinned first
		result.sort((a, b) => {
			if (a.pinned && !b.pinned) return -1;
			if (!a.pinned && b.pinned) return 1;
			return b.created_at.localeCompare(a.created_at);
		});

		if (entryTypeFilter) {
			result = result.filter(e => e.entry_type === entryTypeFilter);
		}

		if (entrySearch.trim()) {
			const q = entrySearch.toLowerCase();
			result = result.filter(e =>
				e.title.toLowerCase().includes(q) ||
				e.content.toLowerCase().includes(q) ||
				e.author_name.toLowerCase().includes(q) ||
				e.tags.some(t => t.toLowerCase().includes(q))
			);
		}

		return result;
	});

	let onlineMembers = $derived(
		selectedTeam?.members.filter(m => m.status === 'online').length ?? 0
	);

	let totalAgents = $derived(
		selectedTeam?.members.reduce((sum, m) => sum + m.active_agents.length, 0) ?? 0
	);

	let todayEntries = $derived(() => {
		const today = new Date().toISOString().slice(0, 10);
		return entries.filter(e => e.created_at.slice(0, 10) === today).length;
	});

	// ---- Data loading --------------------------------------------------------
	async function loadTeams() {
		try {
			teams = await invoke<TeamMeta[]>('team_list');
			if (teams.length > 0 && !selectedTeamId) {
				selectedTeamId = teams[0].id;
			}
		} catch (e) {
			console.error('Failed to load teams:', e);
		}
	}

	async function loadTeam(id: string) {
		if (!id) return;
		try {
			selectedTeam = await invoke<Team>('team_get', { id });
		} catch (e) {
			console.error('Failed to load team:', e);
		}
	}

	async function loadEntries() {
		if (!selectedTeamId) return;
		try {
			const typeArg = entryTypeFilter || null;
			entries = await invoke<ImpBookEntry[]>('impbook_list_entries', {
				teamId: selectedTeamId,
				entryType: typeArg,
			});
		} catch (e) {
			console.error('Failed to load entries:', e);
		}
	}

	async function loadActivity() {
		if (!selectedTeamId) return;
		try {
			activities = await invoke<TeamActivity[]>('team_activity_feed', {
				teamId: selectedTeamId,
				limit: 50,
			});
		} catch (e) {
			console.error('Failed to load activity:', e);
		}
	}

	async function refreshAll() {
		if (!selectedTeamId) return;
		await Promise.all([
			loadTeam(selectedTeamId),
			loadEntries(),
			loadActivity(),
		]);
	}

	// Watch selectedTeamId changes
	$effect(() => {
		if (selectedTeamId) {
			refreshAll();
		}
	});

	// ---- Team actions --------------------------------------------------------
	async function createTeam() {
		if (!newTeamName.trim()) return;
		dialogLoading = true;
		error = '';
		try {
			const team = await invoke<Team>('team_create', { name: newTeamName.trim() });
			showCreateDialog = false;
			newTeamName = '';
			await loadTeams();
			selectedTeamId = team.id;
		} catch (e: any) {
			error = parseError(e);
		} finally {
			dialogLoading = false;
		}
	}

	async function joinTeam() {
		if (!joinCode.trim()) return;
		dialogLoading = true;
		error = '';
		try {
			const team = await invoke<Team>('team_join', {
				inviteCode: joinCode.trim(),
				userName: joinUserName.trim(),
			});
			showJoinDialog = false;
			joinCode = '';
			joinUserName = '';
			await loadTeams();
			selectedTeamId = team.id;
		} catch (e: any) {
			error = parseError(e);
		} finally {
			dialogLoading = false;
		}
	}

	async function leaveTeam() {
		if (!selectedTeamId) return;
		if (!confirm('Are you sure you want to leave this team?')) return;
		try {
			await invoke('team_leave', { teamId: selectedTeamId });
			selectedTeamId = '';
			selectedTeam = null;
			entries = [];
			activities = [];
			await loadTeams();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function copyInviteCode() {
		if (!selectedTeam) return;
		try {
			await navigator.clipboard.writeText(selectedTeam.invite_code);
			codeCopied = true;
			setTimeout(() => { codeCopied = false; }, 2000);
		} catch {
			// Fallback for environments without clipboard API
			codeCopied = false;
		}
	}

	async function updateStatus(status: string) {
		if (!selectedTeamId) return;
		try {
			await invoke('team_update_member_status', { teamId: selectedTeamId, status });
			await loadTeam(selectedTeamId);
		} catch (e) {
			console.error('Failed to update status:', e);
		}
	}

	// ---- ImpBook actions -----------------------------------------------------
	async function createEntry() {
		if (!newEntryTitle.trim() || !selectedTeamId) return;
		dialogLoading = true;
		error = '';
		try {
			const tags = newEntryTags
				.split(',')
				.map(t => t.trim())
				.filter(t => t.length > 0);
			await invoke<ImpBookEntry>('impbook_create_entry', {
				teamId: selectedTeamId,
				entryType: newEntryType,
				title: newEntryTitle.trim(),
				content: newEntryContent,
				tags,
			});
			showCreateEntry = false;
			newEntryTitle = '';
			newEntryContent = '';
			newEntryTags = '';
			newEntryType = 'document';
			await loadEntries();
			await loadActivity();
		} catch (e: any) {
			error = parseError(e);
		} finally {
			dialogLoading = false;
		}
	}

	async function deleteEntry(entryId: string) {
		if (!selectedTeamId) return;
		if (!confirm('Delete this entry?')) return;
		try {
			await invoke('impbook_delete_entry', { teamId: selectedTeamId, entryId });
			await loadEntries();
			await loadActivity();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function togglePin(entryId: string, currentlyPinned: boolean) {
		if (!selectedTeamId) return;
		try {
			await invoke('impbook_pin_entry', {
				teamId: selectedTeamId,
				entryId,
				pinned: !currentlyPinned,
			});
			await loadEntries();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function addReaction(entryId: string, emoji: string) {
		if (!selectedTeamId) return;
		try {
			await invoke('impbook_add_reaction', { teamId: selectedTeamId, entryId, emoji });
			await loadEntries();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	async function addComment(entryId: string) {
		const content = commentInputs[entryId]?.trim();
		if (!content || !selectedTeamId) return;
		try {
			await invoke<Comment>('impbook_add_comment', {
				teamId: selectedTeamId,
				entryId,
				content,
			});
			commentInputs[entryId] = '';
			await loadEntries();
			await loadActivity();
		} catch (e: any) {
			error = parseError(e);
		}
	}

	function toggleComments(entryId: string) {
		const next = new Set(expandedComments);
		if (next.has(entryId)) {
			next.delete(entryId);
		} else {
			next.add(entryId);
		}
		expandedComments = next;
	}

	// ---- Helpers -------------------------------------------------------------
	function parseError(e: any): string {
		if (typeof e === 'string') {
			try {
				const parsed = JSON.parse(e);
				return parsed.message || e;
			} catch {
				return e;
			}
		}
		return e?.message || 'Unknown error';
	}

	function formatDate(iso: string): string {
		try {
			const d = new Date(iso);
			const now = new Date();
			const diff = now.getTime() - d.getTime();
			const mins = Math.floor(diff / 60000);
			if (mins < 1) return 'just now';
			if (mins < 60) return `${mins}m ago`;
			const hours = Math.floor(mins / 60);
			if (hours < 24) return `${hours}h ago`;
			const days = Math.floor(hours / 24);
			if (days < 7) return `${days}d ago`;
			return d.toLocaleDateString();
		} catch {
			return iso;
		}
	}

	function roleBadgeClass(role: string): string {
		switch (role) {
			case 'owner': return 'border-gx-status-warning text-gx-status-warning';
			case 'admin': return 'border-gx-accent-purple text-gx-accent-purple';
			case 'viewer': return 'border-gx-text-muted text-gx-text-muted';
			default: return 'border-gx-neon text-gx-neon';
		}
	}

	function statusDotClass(status: string): string {
		switch (status) {
			case 'online': return 'bg-gx-status-success';
			case 'away': return 'bg-gx-status-warning';
			default: return 'bg-gx-text-muted';
		}
	}

	function getEntryTypeMeta(type: string) {
		return ENTRY_TYPES.find(t => t.value === type) || ENTRY_TYPES[1];
	}

	function groupReactions(reactions: Reaction[]): Array<{ emoji: string; count: number; users: string[] }> {
		const map = new Map<string, { count: number; users: string[] }>();
		for (const r of reactions) {
			const existing = map.get(r.emoji);
			if (existing) {
				existing.count++;
				existing.users.push(r.user_name);
			} else {
				map.set(r.emoji, { count: 1, users: [r.user_name] });
			}
		}
		return Array.from(map.entries()).map(([emoji, data]) => ({ emoji, ...data }));
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(async () => {
		loading = true;
		await loadTeams();
		loading = false;

		// Poll for updates every 5 seconds
		pollTimer = setInterval(() => {
			if (selectedTeamId) {
				loadEntries();
				loadActivity();
				loadTeam(selectedTeamId);
			}
		}, 5000);

		// Mark ourselves online
		if (selectedTeamId) {
			updateStatus('online');
		}
	});

	onDestroy(() => {
		if (pollTimer) {
			clearInterval(pollTimer);
			pollTimer = null;
		}
		// Mark offline on leave
		if (selectedTeamId) {
			invoke('team_update_member_status', { teamId: selectedTeamId, status: 'offline' }).catch(() => {});
		}
	});
</script>

<div class="h-full flex flex-col overflow-hidden {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-3 px-4 py-3 border-b border-gx-border-default shrink-0">
		<Users size={20} class="text-gx-neon" />
		<h1 class="text-lg font-semibold text-gx-text-primary">ForgeTeam</h1>
		<Separator orientation="vertical" class="h-5 bg-gx-border-default" />

		<!-- Team selector -->
		{#if teams.length > 0}
			<select
				bind:value={selectedTeamId}
				class="bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-2 py-1 text-sm text-gx-text-secondary focus:border-gx-neon focus:outline-none"
			>
				{#each teams as t (t.id)}
					<option value={t.id}>{t.name} ({t.member_count})</option>
				{/each}
			</select>
		{/if}

		<div class="flex-1"></div>

		<!-- Quick stats -->
		{#if selectedTeam}
			<div class="flex items-center gap-3 text-xs text-gx-text-muted">
				<span class="flex items-center gap-1">
					<span class="w-2 h-2 rounded-full bg-gx-status-success"></span>
					{onlineMembers} online
				</span>
				<span class="flex items-center gap-1">
					<Bot size={12} />
					{totalAgents} agents
				</span>
				<span class="flex items-center gap-1">
					<BookOpen size={12} />
					{todayEntries()} today
				</span>
			</div>
			<Separator orientation="vertical" class="h-5 bg-gx-border-default" />
		{/if}

		<button
			onclick={() => { showCreateDialog = true; }}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 transition-colors"
		>
			<Plus size={14} />
			New Team
		</button>
		<button
			onclick={() => { showJoinDialog = true; }}
			class="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-gx-bg-tertiary text-gx-text-secondary border border-gx-border-default rounded-gx hover:border-gx-neon/50 transition-colors"
		>
			<UserPlus size={14} />
			Join
		</button>
	</div>

	<!-- Error banner -->
	{#if error}
		<div class="flex items-center gap-2 px-4 py-2 bg-gx-status-error/10 border-b border-gx-status-error/30 text-sm text-gx-status-error">
			<AlertCircle size={14} />
			{error}
			<button onclick={() => { error = ''; }} class="ml-auto hover:text-gx-text-primary">
				<X size={14} />
			</button>
		</div>
	{/if}

	{#if loading}
		<div class="flex-1 flex items-center justify-center">
			<Loader2 size={24} class="animate-spin text-gx-neon" />
		</div>
	{:else if !selectedTeam}
		<!-- Empty state: no team selected -->
		<div class="flex-1 flex flex-col items-center justify-center gap-4 text-gx-text-muted">
			<Users size={48} class="opacity-30" />
			<p class="text-lg">No team selected</p>
			<p class="text-sm">Create a new team or join one with an invite code.</p>
			<div class="flex gap-2">
				<button
					onclick={() => { showCreateDialog = true; }}
					class="flex items-center gap-1.5 px-4 py-2 text-sm bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 transition-colors"
				>
					<Plus size={14} />
					Create Team
				</button>
				<button
					onclick={() => { showJoinDialog = true; }}
					class="flex items-center gap-1.5 px-4 py-2 text-sm bg-gx-bg-tertiary text-gx-text-secondary border border-gx-border-default rounded-gx hover:border-gx-neon/50 transition-colors"
				>
					<UserPlus size={14} />
					Join Team
				</button>
			</div>
		</div>
	{:else}
		<!-- Tab bar -->
		<div class="flex items-center gap-1 px-4 pt-2 border-b border-gx-border-default shrink-0">
			{#each [
				{ id: 'dashboard', label: 'Dashboard', icon: Users },
				{ id: 'impbook', label: 'ImpBook', icon: BookOpen },
				{ id: 'activity', label: 'Activity', icon: Activity },
				{ id: 'settings', label: 'Settings', icon: SettingsIcon },
			] as tab (tab.id)}
				<button
					onclick={() => { activeTab = tab.id as typeof activeTab; }}
					class="flex items-center gap-1.5 px-3 py-2 text-sm rounded-t-gx border-b-2 transition-colors
						{activeTab === tab.id
							? 'text-gx-neon border-gx-neon bg-gx-bg-elevated/50'
							: 'text-gx-text-muted border-transparent hover:text-gx-text-secondary hover:bg-gx-bg-hover'}"
				>
					<tab.icon size={14} />
					{tab.label}
					{#if tab.id === 'impbook'}
						<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-neon/30 text-gx-neon ml-1">
							{entries.length}
						</Badge>
					{/if}
				</button>
			{/each}
		</div>

		<!-- Tab content -->
		<div class="flex-1 overflow-y-auto">
			<!-- ============================================================ -->
			<!-- TAB 1: Dashboard                                             -->
			<!-- ============================================================ -->
			{#if activeTab === 'dashboard'}
				<div class="p-4 space-y-4">
					<!-- Team header card -->
					<div class="rounded-gx border border-gx-border-default {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} p-4" style={cardStyle}>
						<div class="flex items-center gap-3 mb-3">
							<div class="w-10 h-10 rounded-gx bg-gx-neon/15 flex items-center justify-center">
								<Users size={20} class="text-gx-neon" />
							</div>
							<div>
								<h2 class="text-base font-semibold text-gx-text-primary">{selectedTeam.name}</h2>
								<p class="text-xs text-gx-text-muted">Created {formatDate(selectedTeam.created_at)}</p>
							</div>
							<div class="flex-1"></div>
							<!-- Invite code -->
							<div class="flex items-center gap-2">
								<span class="text-xs text-gx-text-muted">Invite:</span>
								<code class="px-2 py-0.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs font-mono text-gx-text-secondary">
									{selectedTeam.invite_code}
								</code>
								<button
									onclick={copyInviteCode}
									class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-neon transition-colors"
									title="Copy invite code"
								>
									{#if codeCopied}
										<Check size={14} class="text-gx-status-success" />
									{:else}
										<Copy size={14} />
									{/if}
								</button>
							</div>
						</div>

						<!-- Quick stats row -->
						<div class="grid grid-cols-4 gap-3">
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-center">
								<div class="text-lg font-bold text-gx-text-primary">{selectedTeam.members.length}</div>
								<div class="text-[10px] text-gx-text-muted uppercase tracking-wider">Members</div>
							</div>
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-center">
								<div class="text-lg font-bold text-gx-status-success">{onlineMembers}</div>
								<div class="text-[10px] text-gx-text-muted uppercase tracking-wider">Online</div>
							</div>
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-center">
								<div class="text-lg font-bold text-gx-accent-blue">{totalAgents}</div>
								<div class="text-[10px] text-gx-text-muted uppercase tracking-wider">Agents</div>
							</div>
							<div class="rounded-gx bg-gx-bg-primary border border-gx-border-default p-3 text-center">
								<div class="text-lg font-bold text-gx-accent-magenta">{entries.length}</div>
								<div class="text-[10px] text-gx-text-muted uppercase tracking-wider">Entries</div>
							</div>
						</div>
					</div>

					<!-- Member cards -->
					<div>
						<h3 class="text-sm font-medium text-gx-text-secondary mb-2 flex items-center gap-2">
							<Users size={14} />
							Team Members
						</h3>
						<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
							{#each selectedTeam.members as member (member.id)}
								<div class="rounded-gx border border-gx-border-default {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} p-3" style={cardStyle}>
									<div class="flex items-center gap-2 mb-2">
										<!-- Avatar placeholder -->
										<div class="w-8 h-8 rounded-full bg-gx-bg-elevated flex items-center justify-center text-xs font-bold text-gx-text-secondary border border-gx-border-default">
											{member.name.charAt(0).toUpperCase()}
										</div>
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-1.5">
												<span class="text-sm font-medium text-gx-text-primary truncate">{member.name}</span>
												<span class="w-2 h-2 rounded-full shrink-0 {statusDotClass(member.status)}"></span>
											</div>
											<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {roleBadgeClass(member.role)}">
												{#if member.role === 'owner'}
													<Crown size={8} class="mr-0.5" />
												{:else if member.role === 'admin'}
													<ShieldCheck size={8} class="mr-0.5" />
												{:else if member.role === 'viewer'}
													<Eye size={8} class="mr-0.5" />
												{/if}
												{member.role}
											</Badge>
										</div>
									</div>

									<!-- Trust score bar -->
									<div class="mb-2">
										<div class="flex items-center justify-between text-[10px] text-gx-text-muted mb-0.5">
											<span>Trust</span>
											<span>{(member.trust_score * 100).toFixed(0)}%</span>
										</div>
										<div class="h-1 bg-gx-bg-primary rounded-full overflow-hidden">
											<div
												class="h-full bg-gx-neon rounded-full transition-all"
												style="width: {member.trust_score * 100}%"
											></div>
										</div>
									</div>

									<!-- Stats -->
									<div class="flex items-center justify-between text-[10px] text-gx-text-muted">
										<span class="flex items-center gap-1">
											<TrendingUp size={10} />
											{member.contributions} contributions
										</span>
										<span class="flex items-center gap-1">
											<Clock size={10} />
											{formatDate(member.last_seen)}
										</span>
									</div>

									<!-- Active agents -->
									{#if member.active_agents.length > 0}
										<div class="mt-2 flex flex-wrap gap-1">
											{#each member.active_agents as agent}
												<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-accent-blue/30 text-gx-accent-blue">
													<Bot size={8} class="mr-0.5" />
													{agent}
												</Badge>
											{/each}
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				</div>

			<!-- ============================================================ -->
			<!-- TAB 2: ImpBook                                               -->
			<!-- ============================================================ -->
			{:else if activeTab === 'impbook'}
				<div class="p-4 space-y-3">
					<!-- Toolbar -->
					<div class="flex items-center gap-2 flex-wrap">
						<button
							onclick={() => { showCreateEntry = true; }}
							class="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-gx-neon/10 text-gx-neon border border-gx-neon/30 rounded-gx hover:bg-gx-neon/20 transition-colors"
						>
							<Plus size={14} />
							New Entry
						</button>

						<!-- Search -->
						<div class="flex items-center gap-1.5 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded-gx flex-1 max-w-xs">
							<Search size={13} class="text-gx-text-muted shrink-0" />
							<input
								type="text"
								bind:value={entrySearch}
								placeholder="Search entries..."
								class="bg-transparent text-xs text-gx-text-primary placeholder:text-gx-text-muted focus:outline-none w-full"
							/>
						</div>

						<!-- Type filter -->
						<select
							bind:value={entryTypeFilter}
							class="bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-2 py-1 text-xs text-gx-text-secondary focus:border-gx-neon focus:outline-none"
						>
							<option value="">All Types</option>
							{#each ENTRY_TYPES as et}
								<option value={et.value}>{et.label}</option>
							{/each}
						</select>

						<div class="flex-1"></div>
						<span class="text-[10px] text-gx-text-muted">{filteredEntries.length} entries</span>
					</div>

					<!-- Create Entry form (inline) -->
					{#if showCreateEntry}
						<div class="rounded-gx border-2 border-gx-neon/30 bg-gx-bg-secondary p-4 space-y-3">
							<div class="flex items-center justify-between">
								<h3 class="text-sm font-medium text-gx-text-primary flex items-center gap-2">
									<Sparkles size={14} class="text-gx-neon" />
									New ImpBook Entry
								</h3>
								<button onclick={() => { showCreateEntry = false; }} class="text-gx-text-muted hover:text-gx-text-primary">
									<X size={16} />
								</button>
							</div>

							<!-- Entry type grid -->
							<div class="grid grid-cols-4 gap-1.5">
								{#each ENTRY_TYPES as et}
									<button
										onclick={() => { newEntryType = et.value; }}
										class="flex items-center gap-1.5 px-2 py-1.5 rounded-gx text-[11px] border transition-colors
											{newEntryType === et.value
												? 'border-gx-neon bg-gx-neon/10 text-gx-neon'
												: 'border-gx-border-default bg-gx-bg-primary text-gx-text-muted hover:border-gx-neon/30'}"
									>
										<et.icon size={12} class={newEntryType === et.value ? 'text-gx-neon' : et.color} />
										{et.label}
									</button>
								{/each}
							</div>

							<input
								type="text"
								bind:value={newEntryTitle}
								placeholder="Entry title..."
								class="w-full px-3 py-2 bg-gx-bg-primary border border-gx-border-default rounded-gx text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none"
							/>

							<textarea
								bind:value={newEntryContent}
								placeholder="Write your content... (Markdown supported)"
								rows={5}
								class="w-full px-3 py-2 bg-gx-bg-primary border border-gx-border-default rounded-gx text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none resize-y font-mono"
							></textarea>

							<input
								type="text"
								bind:value={newEntryTags}
								placeholder="Tags (comma separated)..."
								class="w-full px-3 py-1.5 bg-gx-bg-primary border border-gx-border-default rounded-gx text-xs text-gx-text-secondary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none"
							/>

							<div class="flex justify-end gap-2">
								<button
									onclick={() => { showCreateEntry = false; }}
									class="px-3 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-primary transition-colors"
								>
									Cancel
								</button>
								<button
									onclick={createEntry}
									disabled={!newEntryTitle.trim() || dialogLoading}
									class="flex items-center gap-1.5 px-4 py-1.5 text-xs bg-gx-neon text-gx-bg-primary rounded-gx hover:bg-gx-neon/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
								>
									{#if dialogLoading}
										<Loader2 size={12} class="animate-spin" />
									{:else}
										<Plus size={12} />
									{/if}
									Create Entry
								</button>
							</div>
						</div>
					{/if}

					<!-- Entry feed -->
					{#if filteredEntries.length === 0}
						<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
							<BookOpen size={36} class="opacity-30 mb-2" />
							<p class="text-sm">No entries yet</p>
							<p class="text-xs">Create the first entry to start collaborating.</p>
						</div>
					{:else}
						<div class="space-y-3">
							{#each filteredEntries as entry (entry.id)}
								{@const typeMeta = getEntryTypeMeta(entry.entry_type)}
								{@const grouped = groupReactions(entry.reactions)}
								{@const commentsExpanded = expandedComments.has(entry.id)}
								<div class="rounded-gx border border-gx-border-default {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} overflow-hidden" style={cardStyle}>
									<!-- Entry header -->
									<div class="flex items-start gap-3 p-3 pb-2">
										<!-- Author avatar -->
										<div class="w-8 h-8 rounded-full bg-gx-bg-elevated flex items-center justify-center text-xs font-bold text-gx-text-secondary border border-gx-border-default shrink-0 mt-0.5">
											{entry.author_name.charAt(0).toUpperCase()}
										</div>
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="text-sm font-medium text-gx-text-primary">{entry.author_name}</span>
												<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {typeMeta.color} border-current/30">
													<typeMeta.icon size={8} class="mr-0.5" />
													{typeMeta.label}
												</Badge>
												{#if entry.pinned}
													<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 border-gx-status-warning text-gx-status-warning">
														<Pin size={8} class="mr-0.5" />
														Pinned
													</Badge>
												{/if}
												<span class="text-[10px] text-gx-text-muted">{formatDate(entry.created_at)}</span>
											</div>
											<h4 class="text-sm font-semibold text-gx-text-primary mt-0.5">{entry.title}</h4>

											<!-- Source info -->
											{#if entry.source_agent}
												<div class="flex items-center gap-1 mt-0.5 text-[10px] text-gx-text-muted">
													<Bot size={10} class="text-gx-accent-blue" />
													Generated by <span class="text-gx-accent-blue font-medium">{entry.source_agent}</span>
													{#if entry.source_module}
														<span>in {entry.source_module}</span>
													{/if}
												</div>
											{/if}
										</div>

										<!-- Actions -->
										<div class="flex items-center gap-1 shrink-0">
											<button
												onclick={() => togglePin(entry.id, entry.pinned)}
												class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-status-warning transition-colors"
												title={entry.pinned ? 'Unpin' : 'Pin'}
											>
												{#if entry.pinned}
													<PinOff size={13} />
												{:else}
													<Pin size={13} />
												{/if}
											</button>
											<button
												onclick={() => deleteEntry(entry.id)}
												class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-status-error transition-colors"
												title="Delete"
											>
												<Trash2 size={13} />
											</button>
										</div>
									</div>

									<!-- Content -->
									{#if entry.content}
										<div class="px-3 pb-2 pl-14">
											<div class="text-sm text-gx-text-secondary whitespace-pre-wrap break-words leading-relaxed">
												{entry.content}
											</div>
										</div>
									{/if}

									<!-- Tags -->
									{#if entry.tags.length > 0}
										<div class="px-3 pb-2 pl-14 flex flex-wrap gap-1">
											{#each entry.tags as tag}
												<span class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-gx-bg-elevated text-[10px] text-gx-text-muted border border-gx-border-default">
													<Hash size={9} />
													{tag}
												</span>
											{/each}
										</div>
									{/if}

									<!-- Reactions bar -->
									<div class="flex items-center gap-1 px-3 pb-2 pl-14 flex-wrap">
										{#each grouped as r}
											<button
												onclick={() => addReaction(entry.id, r.emoji)}
												class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full bg-gx-bg-elevated border border-gx-border-default text-xs hover:border-gx-neon/40 transition-colors"
												title={r.users.join(', ')}
											>
												<span>{r.emoji}</span>
												<span class="text-[10px] text-gx-text-muted">{r.count}</span>
											</button>
										{/each}

										<!-- Add reaction buttons -->
										<div class="flex items-center gap-0.5 ml-1">
											{#each REACTION_EMOJIS as emoji}
												{#if !grouped.some(g => g.emoji === emoji)}
													<button
														onclick={() => addReaction(entry.id, emoji)}
														class="w-6 h-6 rounded-full flex items-center justify-center text-xs opacity-30 hover:opacity-100 hover:bg-gx-bg-elevated transition-all"
													>
														{emoji}
													</button>
												{/if}
											{/each}
										</div>

										<div class="flex-1"></div>

										<!-- Comment toggle -->
										<button
											onclick={() => toggleComments(entry.id)}
											class="flex items-center gap-1 text-[11px] text-gx-text-muted hover:text-gx-neon transition-colors"
										>
											<MessageSquare size={12} />
											{entry.comments.length}
											{#if commentsExpanded}
												<ChevronUp size={10} />
											{:else}
												<ChevronDown size={10} />
											{/if}
										</button>
									</div>

									<!-- Comments section (expandable) -->
									{#if commentsExpanded}
										<div class="border-t border-gx-border-default bg-gx-bg-primary/50">
											{#each entry.comments as comment (comment.id)}
												<div class="flex items-start gap-2 px-3 py-2 border-b border-gx-border-default last:border-b-0">
													<div class="w-6 h-6 rounded-full bg-gx-bg-elevated flex items-center justify-center text-[10px] font-bold text-gx-text-muted border border-gx-border-default shrink-0">
														{comment.author_name.charAt(0).toUpperCase()}
													</div>
													<div class="flex-1 min-w-0">
														<div class="flex items-center gap-1.5">
															<span class="text-xs font-medium text-gx-text-primary">{comment.author_name}</span>
															<span class="text-[10px] text-gx-text-muted">{formatDate(comment.created_at)}</span>
														</div>
														<p class="text-xs text-gx-text-secondary mt-0.5">{comment.content}</p>
													</div>
												</div>
											{/each}

											<!-- Add comment -->
											<div class="flex items-center gap-2 px-3 py-2">
												<input
													type="text"
													bind:value={commentInputs[entry.id]}
													onkeydown={(e) => { if (e.key === 'Enter') addComment(entry.id); }}
													placeholder="Write a comment..."
													class="flex-1 px-2 py-1 bg-gx-bg-tertiary border border-gx-border-default rounded-gx text-xs text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none"
												/>
												<button
													onclick={() => addComment(entry.id)}
													disabled={!commentInputs[entry.id]?.trim()}
													class="p-1.5 rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
												>
													<Send size={12} />
												</button>
											</div>
										</div>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</div>

			<!-- ============================================================ -->
			<!-- TAB 3: Activity Feed                                         -->
			<!-- ============================================================ -->
			{:else if activeTab === 'activity'}
				<div class="p-4">
					{#if activities.length === 0}
						<div class="flex flex-col items-center justify-center py-12 text-gx-text-muted">
							<Activity size={36} class="opacity-30 mb-2" />
							<p class="text-sm">No activity yet</p>
							<p class="text-xs">Team actions will appear here in real-time.</p>
						</div>
					{:else}
						<div class="space-y-1">
							{#each activities as act (act.id)}
								<div class="flex items-center gap-3 px-3 py-2 rounded-gx hover:bg-gx-bg-hover transition-colors group">
									<div class="w-7 h-7 rounded-full bg-gx-bg-elevated flex items-center justify-center text-[10px] font-bold text-gx-text-muted border border-gx-border-default shrink-0">
										{act.user_name.charAt(0).toUpperCase()}
									</div>
									<div class="flex-1 min-w-0">
										<p class="text-xs text-gx-text-secondary">
											<span class="font-medium text-gx-text-primary">{act.user_name}</span>
											<span class="text-gx-text-muted"> {act.action} </span>
											<span class="font-medium text-gx-text-primary">"{act.target}"</span>
										</p>
									</div>
									<span class="text-[10px] text-gx-text-muted shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
										{formatDate(act.timestamp)}
									</span>
								</div>
							{/each}
						</div>
					{/if}
				</div>

			<!-- ============================================================ -->
			<!-- TAB 4: Settings                                              -->
			<!-- ============================================================ -->
			{:else if activeTab === 'settings'}
				<div class="p-4 space-y-4 max-w-2xl">
					<!-- Team info -->
					<div class="rounded-gx border border-gx-border-default {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} p-4" style={cardStyle}>
						<h3 class="text-sm font-medium text-gx-text-secondary mb-3 flex items-center gap-2">
							<SettingsIcon size={14} />
							Team Information
						</h3>
						<div class="space-y-2">
							<div class="flex items-center justify-between">
								<span class="text-xs text-gx-text-muted">Team Name</span>
								<span class="text-xs text-gx-text-primary font-medium">{selectedTeam.name}</span>
							</div>
							<div class="flex items-center justify-between">
								<span class="text-xs text-gx-text-muted">Team ID</span>
								<code class="text-[10px] text-gx-text-muted font-mono">{selectedTeam.id}</code>
							</div>
							<div class="flex items-center justify-between">
								<span class="text-xs text-gx-text-muted">Created</span>
								<span class="text-xs text-gx-text-secondary">{new Date(selectedTeam.created_at).toLocaleDateString()}</span>
							</div>
							<div class="flex items-center justify-between">
								<span class="text-xs text-gx-text-muted">Invite Code</span>
								<div class="flex items-center gap-1">
									<code class="px-1.5 py-0.5 bg-gx-bg-tertiary border border-gx-border-default rounded text-xs font-mono text-gx-text-secondary">
										{selectedTeam.invite_code}
									</code>
									<button
										onclick={copyInviteCode}
										class="p-0.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-neon transition-colors"
									>
										{#if codeCopied}
											<Check size={12} class="text-gx-status-success" />
										{:else}
											<Copy size={12} />
										{/if}
									</button>
								</div>
							</div>
						</div>
					</div>

					<!-- Members management -->
					<div class="rounded-gx border border-gx-border-default {hasEngineStyle && cardComponent ? '' : 'bg-gx-bg-secondary'} p-4" style={cardStyle}>
						<h3 class="text-sm font-medium text-gx-text-secondary mb-3 flex items-center gap-2">
							<Users size={14} />
							Members ({selectedTeam.members.length})
						</h3>
						<div class="space-y-2">
							{#each selectedTeam.members as member (member.id)}
								<div class="flex items-center gap-2 py-1">
									<span class="w-2 h-2 rounded-full {statusDotClass(member.status)}"></span>
									<span class="text-xs text-gx-text-primary flex-1">{member.name}</span>
									<Badge variant="outline" class="text-[9px] px-1 py-0 h-3.5 {roleBadgeClass(member.role)}">
										{member.role}
									</Badge>
									<span class="text-[10px] text-gx-text-muted">{member.contributions} contrib.</span>
								</div>
							{/each}
						</div>
					</div>

					<!-- Danger zone -->
					<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/5 p-4">
						<h3 class="text-sm font-medium text-gx-status-error mb-3 flex items-center gap-2">
							<AlertCircle size={14} />
							Danger Zone
						</h3>
						<button
							onclick={leaveTeam}
							class="flex items-center gap-1.5 px-3 py-1.5 text-xs text-gx-status-error border border-gx-status-error/30 rounded-gx hover:bg-gx-status-error/10 transition-colors"
						>
							<LogOut size={14} />
							Leave Team
						</button>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

<!-- ============================================================ -->
<!-- Create Team Dialog                                           -->
<!-- ============================================================ -->
{#if showCreateDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center">
		<!-- Backdrop -->
		<button class="absolute inset-0 bg-black/50" onclick={() => { showCreateDialog = false; }}></button>
		<!-- Dialog -->
		<div class="relative bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg p-6 w-full max-w-md">
			<h2 class="text-base font-semibold text-gx-text-primary mb-4 flex items-center gap-2">
				<Plus size={18} class="text-gx-neon" />
				Create New Team
			</h2>

			<input
				type="text"
				bind:value={newTeamName}
				onkeydown={(e) => { if (e.key === 'Enter') createTeam(); }}
				placeholder="Team name..."
				class="w-full px-3 py-2 bg-gx-bg-primary border border-gx-border-default rounded-gx text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none mb-4"
				autofocus
			/>

			<div class="flex justify-end gap-2">
				<button
					onclick={() => { showCreateDialog = false; newTeamName = ''; }}
					class="px-3 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-primary transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={createTeam}
					disabled={!newTeamName.trim() || dialogLoading}
					class="flex items-center gap-1.5 px-4 py-1.5 text-xs bg-gx-neon text-gx-bg-primary rounded-gx hover:bg-gx-neon/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
				>
					{#if dialogLoading}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<Plus size={12} />
					{/if}
					Create
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- ============================================================ -->
<!-- Join Team Dialog                                              -->
<!-- ============================================================ -->
{#if showJoinDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center">
		<!-- Backdrop -->
		<button class="absolute inset-0 bg-black/50" onclick={() => { showJoinDialog = false; }}></button>
		<!-- Dialog -->
		<div class="relative bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg p-6 w-full max-w-md">
			<h2 class="text-base font-semibold text-gx-text-primary mb-4 flex items-center gap-2">
				<UserPlus size={18} class="text-gx-neon" />
				Join Team
			</h2>

			<div class="space-y-3 mb-4">
				<input
					type="text"
					bind:value={joinCode}
					onkeydown={(e) => { if (e.key === 'Enter') joinTeam(); }}
					placeholder="Enter invite code (8 characters)..."
					maxlength={8}
					class="w-full px-3 py-2 bg-gx-bg-primary border border-gx-border-default rounded-gx text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none font-mono uppercase tracking-widest text-center"
					autofocus
				/>
				<input
					type="text"
					bind:value={joinUserName}
					placeholder="Your display name (optional)..."
					class="w-full px-3 py-2 bg-gx-bg-primary border border-gx-border-default rounded-gx text-sm text-gx-text-primary placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none"
				/>
			</div>

			<div class="flex justify-end gap-2">
				<button
					onclick={() => { showJoinDialog = false; joinCode = ''; joinUserName = ''; }}
					class="px-3 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-primary transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={joinTeam}
					disabled={!joinCode.trim() || dialogLoading}
					class="flex items-center gap-1.5 px-4 py-1.5 text-xs bg-gx-neon text-gx-bg-primary rounded-gx hover:bg-gx-neon/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
				>
					{#if dialogLoading}
						<Loader2 size={12} class="animate-spin" />
					{:else}
						<UserPlus size={12} />
					{/if}
					Join
				</button>
			</div>
		</div>
	</div>
{/if}
