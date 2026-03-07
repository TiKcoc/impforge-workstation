<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Tabs, TabsList, TabsTrigger, TabsContent } from '$lib/components/ui/tabs';
	import { Badge } from '$lib/components/ui/badge';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import {
		RefreshCw, Star, CircleDot, GitPullRequest, ArrowLeft,
		Lock, Globe, AlertCircle, GitBranch, Clock
	} from '@lucide/svelte';

	interface Label { name: string; color: string }
	interface GhUser { login: string; avatar_url: string }
	interface RepoInfo {
		id: number; name: string; full_name: string; description: string | null;
		html_url: string; default_branch: string; stargazers_count: number;
		open_issues_count: number; is_private: boolean; language: string | null; updated_at: string;
	}
	interface IssueInfo {
		id: number; number: number; title: string; state: string; html_url: string;
		created_at: string; labels: Label[]; user: GhUser; body: string | null;
	}
	interface PullRequestInfo {
		id: number; number: number; title: string; state: string; html_url: string;
		created_at: string; user: GhUser; head: { branch_ref: string; sha: string };
		base: { branch_ref: string; sha: string }; merged: boolean; draft: boolean;
	}

	const LANG_COLORS: Record<string, string> = {
		TypeScript: '#3178C6', Rust: '#DEA584', Python: '#3572A5',
		JavaScript: '#F1E05A', Go: '#00ADD8', Java: '#B07219',
		C: '#555555', 'C++': '#F34B7D', 'C#': '#178600',
		Shell: '#89E051', HTML: '#E34C26', CSS: '#563D7C', Svelte: '#FF3E00',
	};

	let user: GhUser | null = $state(null);
	let repos: RepoInfo[] = $state([]);
	let issues: IssueInfo[] = $state([]);
	let pullRequests: PullRequestInfo[] = $state([]);
	let selectedRepo: RepoInfo | null = $state(null);
	let activeTab: string = $state('issues');
	let loading = $state(true);
	let detailLoading = $state(false);
	let error: string | null = $state(null);
	let noToken = $state(false);

	function relativeTime(dateStr: string): string {
		const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
		if (seconds < 60) return 'just now';
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours}h ago`;
		const days = Math.floor(hours / 24);
		if (days < 30) return `${days}d ago`;
		const months = Math.floor(days / 30);
		return `${months}mo ago`;
	}

	function langColor(lang: string | null): string {
		return lang ? (LANG_COLORS[lang] ?? '#666') : '#666';
	}

	async function loadUser() {
		try {
			user = await invoke<GhUser>('get_user');
		} catch { user = null; }
	}

	async function loadRepos() {
		loading = true;
		error = null;
		noToken = false;
		try {
			await loadUser();
			repos = await invoke<RepoInfo[]>('get_repos');
		} catch (e: any) {
			const msg = String(e);
			if (msg.includes('token') || msg.includes('auth') || msg.includes('401') || msg.includes('configured')) {
				noToken = true;
			} else {
				error = msg;
			}
		} finally {
			loading = false;
		}
	}

	async function selectRepo(repo: RepoInfo) {
		selectedRepo = repo;
		activeTab = 'issues';
		detailLoading = true;
		issues = [];
		pullRequests = [];
		try {
			const [i, p] = await Promise.all([
				invoke<IssueInfo[]>('get_issues', { repo: repo.full_name }),
				invoke<PullRequestInfo[]>('get_pull_requests', { repo: repo.full_name }),
			]);
			issues = i;
			pullRequests = p;
		} catch (e: any) {
			error = String(e);
		} finally {
			detailLoading = false;
		}
	}

	function goBack() {
		selectedRepo = null;
		issues = [];
		pullRequests = [];
		error = null;
	}

	function prStateBadge(pr: PullRequestInfo): { label: string; cls: string } {
		if (pr.merged) return { label: 'Merged', cls: 'bg-purple-600/20 text-purple-400 border-purple-500/30' };
		if (pr.state === 'closed') return { label: 'Closed', cls: 'bg-red-600/20 text-red-400 border-red-500/30' };
		return { label: 'Open', cls: 'bg-green-600/20 text-green-400 border-green-500/30' };
	}

	onMount(loadRepos);
</script>

<main class="flex flex-col h-screen">
	<header class="h-14 border-b border-gx-border-default bg-gx-bg-secondary flex items-center px-4 gap-3 shrink-0">
		{#if selectedRepo}
			<button onclick={goBack} class="text-gx-text-muted hover:text-gx-neon transition-colors">
				<ArrowLeft size={18} />
			</button>
		{:else}
			<a href="/" class="text-gx-text-muted hover:text-gx-neon transition-colors">
				<ArrowLeft size={18} />
			</a>
		{/if}
		<h1 class="text-lg font-semibold text-gx-text-primary">
			{#if selectedRepo}{selectedRepo.full_name}{:else}GitHub{/if}
		</h1>
		<div class="flex-1"></div>
		{#if user}
			<div class="flex items-center gap-2">
				<img src={user.avatar_url} alt={user.login} class="w-6 h-6 rounded-full" />
				<span class="text-sm text-gx-text-secondary">{user.login}</span>
			</div>
		{/if}
		<button onclick={loadRepos} class="text-gx-text-muted hover:text-gx-neon transition-colors p-1.5 rounded-gx hover:bg-gx-bg-tertiary" title="Refresh">
			<RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
		</button>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if loading && !selectedRepo}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
				{#each Array(6) as _}
					<div class="rounded-gx border border-gx-border-default bg-gx-bg-elevated p-4 space-y-3">
						<Skeleton class="h-5 w-3/4" />
						<Skeleton class="h-4 w-full" />
						<Skeleton class="h-4 w-1/2" />
					</div>
				{/each}
			</div>
		{:else if noToken}
			<div class="flex flex-col items-center justify-center h-full gap-4 max-w-md mx-auto text-center">
				<AlertCircle size={48} class="text-gx-text-muted" />
				<h2 class="text-xl font-semibold text-gx-text-primary">GitHub Token Not Configured</h2>
				<p class="text-gx-text-secondary text-sm">
					Set your GitHub personal access token in Settings to connect your repositories.
				</p>
				<div class="text-left bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-full text-sm space-y-2">
					<p class="text-gx-text-secondary">1. Go to <a href="https://github.com/settings/tokens" target="_blank" class="text-gx-neon hover:underline">github.com/settings/tokens</a></p>
					<p class="text-gx-text-secondary">2. Generate a token with <code class="text-gx-neon bg-gx-bg-tertiary px-1 rounded">repo</code> scope</p>
					<p class="text-gx-text-secondary">3. Add it in <a href="/settings" class="text-gx-neon hover:underline">Nexus Settings</a></p>
				</div>
			</div>
		{:else if error && !selectedRepo}
			<div class="flex flex-col items-center justify-center h-full gap-3">
				<AlertCircle size={36} class="text-red-400" />
				<p class="text-red-400 text-sm max-w-md text-center">{error}</p>
				<button onclick={loadRepos} class="text-sm text-gx-neon hover:underline">Try again</button>
			</div>
		{:else if !selectedRepo}
			{#if repos.length === 0}
				<div class="flex flex-col items-center justify-center h-full gap-3 text-gx-text-muted">
					<Globe size={36} />
					<p>No repositories found.</p>
				</div>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
					{#each repos as repo (repo.id)}
						<button
							onclick={() => selectRepo(repo)}
							class="text-left rounded-gx border border-gx-border-default bg-gx-bg-elevated p-4 hover:border-gx-neon/40 hover:bg-gx-bg-tertiary transition-colors group"
						>
							<div class="flex items-center gap-2 mb-2">
								{#if repo.is_private}
									<Lock size={14} class="text-gx-text-muted shrink-0" />
								{/if}
								<span class="font-medium text-gx-text-primary truncate group-hover:text-gx-neon transition-colors">{repo.name}</span>
							</div>
							{#if repo.description}
								<p class="text-sm text-gx-text-secondary line-clamp-2 mb-3">{repo.description}</p>
							{:else}
								<p class="text-sm text-gx-text-muted italic mb-3">No description</p>
							{/if}
							<div class="flex items-center gap-3 text-xs text-gx-text-muted">
								{#if repo.language}
									<span class="flex items-center gap-1">
										<span class="w-2.5 h-2.5 rounded-full inline-block" style="background-color: {langColor(repo.language)}"></span>
										{repo.language}
									</span>
								{/if}
								<span class="flex items-center gap-1"><Star size={12} />{repo.stargazers_count}</span>
								<span class="flex items-center gap-1"><CircleDot size={12} />{repo.open_issues_count}</span>
								<span class="flex items-center gap-1 ml-auto"><Clock size={12} />{relativeTime(repo.updated_at)}</span>
							</div>
						</button>
					{/each}
				</div>
			{/if}
		{:else}
			{#if error}
				<div class="flex flex-col items-center gap-3 py-8">
					<AlertCircle size={28} class="text-red-400" />
					<p class="text-red-400 text-sm">{error}</p>
				</div>
			{:else}
				<Tabs bind:value={activeTab} class="w-full">
					<TabsList class="bg-gx-bg-tertiary border border-gx-border-default">
						<TabsTrigger value="issues" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon gap-1.5">
							<CircleDot size={14} /> Issues
							{#if !detailLoading}<span class="text-xs text-gx-text-muted">({issues.length})</span>{/if}
						</TabsTrigger>
						<TabsTrigger value="prs" class="data-[state=active]:bg-gx-bg-elevated data-[state=active]:text-gx-neon gap-1.5">
							<GitPullRequest size={14} /> Pull Requests
							{#if !detailLoading}<span class="text-xs text-gx-text-muted">({pullRequests.length})</span>{/if}
						</TabsTrigger>
					</TabsList>

					<TabsContent value="issues" class="mt-3">
						{#if detailLoading}
							<div class="space-y-2">
								{#each Array(4) as _}
									<div class="rounded-gx border border-gx-border-default bg-gx-bg-elevated p-3 space-y-2">
										<Skeleton class="h-4 w-2/3" />
										<Skeleton class="h-3 w-1/3" />
									</div>
								{/each}
							</div>
						{:else if issues.length === 0}
							<div class="text-center py-12 text-gx-text-muted">
								<CircleDot size={32} class="mx-auto mb-2 opacity-40" />
								<p>No issues found</p>
							</div>
						{:else}
							<div class="space-y-1.5">
								{#each issues as issue (issue.id)}
									<a
										href={issue.html_url}
										target="_blank"
										class="flex items-start gap-3 rounded-gx border border-gx-border-default bg-gx-bg-elevated p-3 hover:border-gx-neon/40 hover:bg-gx-bg-tertiary transition-colors"
									>
										<span class="text-sm text-gx-text-muted font-mono shrink-0">#{issue.number}</span>
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="text-sm font-medium text-gx-text-primary">{issue.title}</span>
												{#if issue.state === 'open'}
													<span class="text-[10px] px-1.5 py-0.5 rounded-full border bg-green-600/20 text-green-400 border-green-500/30">Open</span>
												{:else}
													<span class="text-[10px] px-1.5 py-0.5 rounded-full border bg-red-600/20 text-red-400 border-red-500/30">Closed</span>
												{/if}
											</div>
											<div class="flex items-center gap-2 mt-1 flex-wrap">
												{#each issue.labels as label}
													<span
														class="text-[10px] px-1.5 py-0.5 rounded-full border"
														style="background-color: #{label.color}22; color: #{label.color}; border-color: #{label.color}44"
													>{label.name}</span>
												{/each}
											</div>
										</div>
										<div class="flex items-center gap-1.5 shrink-0">
											<img src={issue.user.avatar_url} alt={issue.user.login} class="w-5 h-5 rounded-full" />
											<span class="text-xs text-gx-text-muted">{relativeTime(issue.created_at)}</span>
										</div>
									</a>
								{/each}
							</div>
						{/if}
					</TabsContent>

					<TabsContent value="prs" class="mt-3">
						{#if detailLoading}
							<div class="space-y-2">
								{#each Array(4) as _}
									<div class="rounded-gx border border-gx-border-default bg-gx-bg-elevated p-3 space-y-2">
										<Skeleton class="h-4 w-2/3" />
										<Skeleton class="h-3 w-1/3" />
									</div>
								{/each}
							</div>
						{:else if pullRequests.length === 0}
							<div class="text-center py-12 text-gx-text-muted">
								<GitPullRequest size={32} class="mx-auto mb-2 opacity-40" />
								<p>No pull requests found</p>
							</div>
						{:else}
							<div class="space-y-1.5">
								{#each pullRequests as pr (pr.id)}
									<a
										href={pr.html_url}
										target="_blank"
										class="flex items-start gap-3 rounded-gx border border-gx-border-default bg-gx-bg-elevated p-3 hover:border-gx-neon/40 hover:bg-gx-bg-tertiary transition-colors"
									>
										<span class="text-sm text-gx-text-muted font-mono shrink-0">#{pr.number}</span>
										<div class="flex-1 min-w-0">
											<div class="flex items-center gap-2 flex-wrap">
												<span class="text-sm font-medium text-gx-text-primary">{pr.title}</span>
												<span class="text-[10px] px-1.5 py-0.5 rounded-full border {prStateBadge(pr).cls}">{prStateBadge(pr).label}</span>
												{#if pr.draft}
													<span class="text-[10px] px-1.5 py-0.5 rounded-full border bg-gx-bg-tertiary text-gx-text-muted border-gx-border-default">Draft</span>
												{/if}
											</div>
											<div class="flex items-center gap-1.5 mt-1 text-xs text-gx-text-muted">
												<GitBranch size={12} />
												<span class="font-mono">{pr.head.branch_ref}</span>
												<span>&rarr;</span>
												<span class="font-mono">{pr.base.branch_ref}</span>
											</div>
										</div>
										<div class="flex items-center gap-1.5 shrink-0">
											<img src={pr.user.avatar_url} alt={pr.user.login} class="w-5 h-5 rounded-full" />
											<span class="text-xs text-gx-text-muted">{relativeTime(pr.created_at)}</span>
										</div>
									</a>
								{/each}
							</div>
						{/if}
					</TabsContent>
				</Tabs>
			{/if}
		{/if}
	</div>
</main>
