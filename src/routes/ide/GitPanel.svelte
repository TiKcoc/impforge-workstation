<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GitBranch, GitCommit, Plus, Minus, FileEdit,
		FileQuestion, AlertCircle, RefreshCw, ChevronDown, ChevronRight, Send,
		ArrowUp, ArrowDown, Trash2, Check, X, Copy
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface GitFileStatus {
		path: string;
		status: string;
		staged: boolean;
	}

	interface CommitInfo {
		id: string;
		message: string;
		author: string;
		time: string;
	}

	interface BranchInfo {
		name: string;
		is_head: boolean;
		is_remote: boolean;
		upstream: string | null;
		last_commit: string | null;
	}

	interface Props {
		workspacePath?: string;
	}

	let { workspacePath = '' }: Props = $props();

	// BenikUI style engine
	const widgetId = 'ide-git-panel';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let headerComp = $derived(styleEngine.getComponentStyle(widgetId, 'header'));
	let headerStyle = $derived(hasEngineStyle && headerComp ? componentToCSS(headerComp) : '');
	let tabSelectorComp = $derived(styleEngine.getComponentStyle(widgetId, 'tab-selector'));
	let tabSelectorStyle = $derived(hasEngineStyle && tabSelectorComp ? componentToCSS(tabSelectorComp) : '');
	let stagedComp = $derived(styleEngine.getComponentStyle(widgetId, 'staged-section'));
	let stagedStyle = $derived(hasEngineStyle && stagedComp ? componentToCSS(stagedComp) : '');
	let unstagedComp = $derived(styleEngine.getComponentStyle(widgetId, 'unstaged-section'));
	let unstagedStyle = $derived(hasEngineStyle && unstagedComp ? componentToCSS(unstagedComp) : '');
	let diffComp = $derived(styleEngine.getComponentStyle(widgetId, 'diff-view'));
	let diffStyle = $derived(hasEngineStyle && diffComp ? componentToCSS(diffComp) : '');
	let commitComp = $derived(styleEngine.getComponentStyle(widgetId, 'commit-input'));
	let commitStyle = $derived(hasEngineStyle && commitComp ? componentToCSS(commitComp) : '');

	let activeTab = $state<'changes' | 'branches' | 'log'>('changes');
	let branch = $state('');
	let files = $state<GitFileStatus[]>([]);
	let commits = $state<CommitInfo[]>([]);
	let branches = $state<BranchInfo[]>([]);
	let diffContent = $state('');
	let selectedFile = $state('');
	let loading = $state(false);
	let showStaged = $state(true);
	let showUnstaged = $state(true);
	let commitMessage = $state('');
	let committing = $state(false);
	let pushing = $state(false);
	let pulling = $state(false);
	let pushResult = $state('');
	let newBranchName = $state('');
	let creatingBranch = $state(false);
	let showNewBranchInput = $state(false);

	const stagedFiles = $derived(files.filter((f) => f.staged));
	const unstagedFiles = $derived(files.filter((f) => !f.staged));
	const localBranches = $derived(branches.filter((b) => !b.is_remote));
	const remoteBranches = $derived(branches.filter((b) => b.is_remote));

	onMount(() => {
		if (workspacePath) refresh();
	});

	async function refresh() {
		if (!workspacePath) return;
		loading = true;
		try {
			const result = await invoke<{
				branch: string;
				files: GitFileStatus[];
				clean: boolean;
			}>('git_status', { workspacePath });
			branch = result.branch;
			files = result.files;
		} catch (e) {
			console.error('Git status failed:', e);
		}
		loading = false;
	}

	async function loadLog() {
		if (!workspacePath) return;
		try {
			commits = await invoke<CommitInfo[]>('git_log', {
				workspacePath,
				limit: 50,
			});
		} catch (e) {
			console.error('Git log failed:', e);
		}
	}

	async function loadBranches() {
		if (!workspacePath) return;
		try {
			branches = await invoke<BranchInfo[]>('git_branches', { workspacePath });
		} catch (e) {
			console.error('Git branches failed:', e);
		}
	}

	async function showDiff(filePath: string) {
		selectedFile = filePath;
		try {
			diffContent = await invoke<string>('git_diff', {
				workspacePath,
				staged: false,
			});
		} catch (e) {
			diffContent = `Error: ${e}`;
		}
	}

	async function stageFile(filePath: string) {
		try {
			await invoke('git_stage', { workspacePath, filePath: `${workspacePath}/${filePath}` });
			await refresh();
		} catch (e) {
			console.error('Stage failed:', e);
		}
	}

	async function unstageFile(filePath: string) {
		try {
			await invoke('git_unstage', { workspacePath, filePath: `${workspacePath}/${filePath}` });
			await refresh();
		} catch (e) {
			console.error('Unstage failed:', e);
		}
	}

	async function commitChanges() {
		if (!commitMessage.trim() || !workspacePath || stagedFiles.length === 0) return;
		committing = true;
		try {
			await invoke('git_commit', { workspacePath, message: commitMessage.trim() });
			commitMessage = '';
			await refresh();
		} catch (e) {
			console.error('Commit failed:', e);
		}
		committing = false;
	}

	async function pushChanges() {
		if (!workspacePath) return;
		pushing = true;
		pushResult = '';
		try {
			const result = await invoke<string>('git_push', { workspacePath });
			pushResult = result || 'Pushed successfully';
			setTimeout(() => pushResult = '', 4000);
		} catch (e) {
			pushResult = `Error: ${e}`;
		}
		pushing = false;
	}

	async function pullChanges() {
		if (!workspacePath) return;
		pulling = true;
		pushResult = '';
		try {
			const result = await invoke<string>('git_pull', { workspacePath });
			pushResult = result || 'Pulled successfully';
			await refresh();
			setTimeout(() => pushResult = '', 4000);
		} catch (e) {
			pushResult = `Error: ${e}`;
		}
		pulling = false;
	}

	async function checkoutBranch(branchName: string) {
		if (!workspacePath) return;
		try {
			await invoke('git_checkout', { workspacePath, branchName });
			await refresh();
			await loadBranches();
		} catch (e) {
			console.error('Checkout failed:', e);
		}
	}

	async function createBranch() {
		if (!workspacePath || !newBranchName.trim()) return;
		creatingBranch = true;
		try {
			await invoke('git_create_branch', { workspacePath, branchName: newBranchName.trim() });
			newBranchName = '';
			showNewBranchInput = false;
			await loadBranches();
		} catch (e) {
			console.error('Create branch failed:', e);
		}
		creatingBranch = false;
	}

	async function deleteBranch(branchName: string) {
		if (!workspacePath) return;
		try {
			await invoke('git_delete_branch', { workspacePath, branchName });
			await loadBranches();
		} catch (e) {
			console.error('Delete branch failed:', e);
		}
	}

	function statusIcon(status: string) {
		switch (status) {
			case 'added':
			case 'untracked':
				return { icon: Plus, color: 'text-gx-status-success' };
			case 'modified':
				return { icon: FileEdit, color: 'text-gx-status-warning' };
			case 'deleted':
				return { icon: Minus, color: 'text-gx-status-error' };
			case 'conflict':
				return { icon: AlertCircle, color: 'text-gx-status-error' };
			default:
				return { icon: FileQuestion, color: 'text-gx-text-muted' };
		}
	}

	function switchTab(tab: 'changes' | 'branches' | 'log') {
		activeTab = tab;
		if (tab === 'log' && commits.length === 0) loadLog();
		if (tab === 'branches' && branches.length === 0) loadBranches();
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-2 px-2 py-1.5 border-b border-gx-border-subtle shrink-0" style={headerStyle}>
		<GitBranch size={14} class="text-gx-neon" />
		<span class="text-xs font-semibold text-gx-text-secondary">{branch || 'No repo'}</span>
		<div class="flex-1"></div>
		<!-- Push / Pull / Refresh -->
		<button
			onclick={pullChanges}
			disabled={pulling}
			class="p-0.5 text-gx-text-muted hover:text-gx-accent-cyan disabled:opacity-40"
			title="Pull"
		>
			<ArrowDown size={12} class={pulling ? 'animate-bounce' : ''} />
		</button>
		<button
			onclick={pushChanges}
			disabled={pushing}
			class="p-0.5 text-gx-text-muted hover:text-gx-neon disabled:opacity-40"
			title="Push"
		>
			<ArrowUp size={12} class={pushing ? 'animate-bounce' : ''} />
		</button>
		<button onclick={refresh} class="p-0.5 text-gx-text-muted hover:text-gx-neon" title="Refresh">
			<RefreshCw size={12} class={loading ? 'animate-spin' : ''} />
		</button>
	</div>

	<!-- Push/Pull result toast -->
	{#if pushResult}
		<div class="px-2 py-1 text-[10px] border-b border-gx-border-subtle {pushResult.startsWith('Error') ? 'bg-gx-status-error/10 text-gx-status-error' : 'bg-gx-neon/10 text-gx-neon'}">
			{pushResult}
		</div>
	{/if}

	<!-- Tab selector -->
	<div class="flex border-b border-gx-border-subtle shrink-0" style={tabSelectorStyle}>
		<button
			onclick={() => switchTab('changes')}
			class="flex-1 py-1.5 text-xs text-center transition-colors
				{activeTab === 'changes' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
		>
			Changes ({files.length})
		</button>
		<button
			onclick={() => switchTab('branches')}
			class="flex-1 py-1.5 text-xs text-center transition-colors
				{activeTab === 'branches' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
		>
			Branches
		</button>
		<button
			onclick={() => switchTab('log')}
			class="flex-1 py-1.5 text-xs text-center transition-colors
				{activeTab === 'log' ? 'text-gx-neon border-b border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
		>
			History
		</button>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-auto text-xs">
		{#if activeTab === 'changes'}
			<!-- Staged files -->
			{#if stagedFiles.length > 0}
				<button
					onclick={() => showStaged = !showStaged}
					class="flex items-center gap-1 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover"
					style={stagedStyle}
				>
					{#if showStaged}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold">Staged ({stagedFiles.length})</span>
				</button>
				{#if showStaged}
					{#each stagedFiles as file}
						{@const si = statusIcon(file.status)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							onclick={() => showDiff(file.path)}
							onkeydown={(e) => e.key === 'Enter' && showDiff(file.path)}
							role="option"
							tabindex="0"
							aria-selected={selectedFile === file.path}
							class="flex items-center gap-1.5 w-full px-4 py-1 hover:bg-gx-bg-hover text-left group cursor-pointer
								{selectedFile === file.path ? 'bg-gx-bg-elevated' : ''}"
						>
							<si.icon size={12} class={si.color} />
							<span class="text-gx-text-secondary truncate flex-1">{file.path}</span>
							<button
								onclick={(e) => { e.stopPropagation(); unstageFile(file.path); }}
								class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-gx-bg-hover rounded text-gx-text-muted"
								title="Unstage"
							>
								<Minus size={10} />
							</button>
						</div>
					{/each}
				{/if}
			{/if}

			<!-- Unstaged files -->
			{#if unstagedFiles.length > 0}
				<button
					onclick={() => showUnstaged = !showUnstaged}
					class="flex items-center gap-1 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover"
					style={unstagedStyle}
				>
					{#if showUnstaged}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold">Changes ({unstagedFiles.length})</span>
				</button>
				{#if showUnstaged}
					{#each unstagedFiles as file}
						{@const si = statusIcon(file.status)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							onclick={() => showDiff(file.path)}
							onkeydown={(e) => e.key === 'Enter' && showDiff(file.path)}
							role="option"
							tabindex="0"
							aria-selected={selectedFile === file.path}
							class="flex items-center gap-1.5 w-full px-4 py-1 hover:bg-gx-bg-hover text-left group cursor-pointer
								{selectedFile === file.path ? 'bg-gx-bg-elevated' : ''}"
						>
							<si.icon size={12} class={si.color} />
							<span class="text-gx-text-secondary truncate flex-1">{file.path}</span>
							<button
								onclick={(e) => { e.stopPropagation(); stageFile(file.path); }}
								class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-gx-bg-hover rounded text-gx-text-muted"
								title="Stage"
							>
								<Plus size={10} />
							</button>
						</div>
					{/each}
				{/if}
			{/if}

			{#if files.length === 0 && !loading}
				<div class="p-4 text-center text-gx-text-disabled">
					<GitCommit size={24} class="mx-auto mb-2 opacity-30" />
					<p>Working tree clean</p>
				</div>
			{/if}

			<!-- Commit input -->
			{#if stagedFiles.length > 0}
				<div class="px-2 py-2 border-t border-gx-border-subtle" style={commitStyle}>
					<textarea
						bind:value={commitMessage}
						onkeydown={(e) => { if (e.ctrlKey && e.key === 'Enter') commitChanges(); }}
						placeholder="Commit message... (Ctrl+Enter to commit)"
						rows="2"
						class="w-full px-2 py-1.5 text-xs bg-gx-bg-secondary border border-gx-border-default rounded text-gx-text-primary placeholder:text-gx-text-disabled resize-none focus:border-gx-neon focus:outline-none"
					></textarea>
					<button
						onclick={commitChanges}
						disabled={!commitMessage.trim() || committing}
						class="mt-1 w-full flex items-center justify-center gap-1.5 px-2 py-1 text-xs rounded bg-gx-neon/20 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/30 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
					>
						<Send size={11} />
						{committing ? 'Committing...' : `Commit (${stagedFiles.length} file${stagedFiles.length > 1 ? 's' : ''})`}
					</button>
				</div>
			{/if}

			<!-- Diff preview -->
			{#if diffContent}
				<div class="border-t border-gx-border-subtle mt-2" style={diffStyle}>
					<div class="px-2 py-1 text-[10px] text-gx-text-disabled bg-gx-bg-secondary">
						Diff: {selectedFile}
					</div>
					<pre class="p-2 text-[11px] font-mono overflow-auto max-h-60 bg-gx-bg-primary">{#each diffContent.split('\n') as line}<span class="{line.startsWith('+') ? 'text-gx-status-success' : line.startsWith('-') ? 'text-gx-status-error' : 'text-gx-text-muted'}">{line}
</span>{/each}</pre>
				</div>
			{/if}

		{:else if activeTab === 'branches'}
			<!-- Branch Manager -->
			<div class="px-2 py-1.5 border-b border-gx-border-subtle flex items-center gap-1">
				{#if showNewBranchInput}
					<input
						bind:value={newBranchName}
						onkeydown={(e) => { if (e.key === 'Enter') createBranch(); if (e.key === 'Escape') { showNewBranchInput = false; newBranchName = ''; } }}
						placeholder="Branch name..."
						class="flex-1 px-2 py-1 text-xs bg-gx-bg-secondary border border-gx-border-default rounded text-gx-text-primary placeholder:text-gx-text-disabled focus:border-gx-neon focus:outline-none"
					/>
					<button onclick={createBranch} disabled={!newBranchName.trim() || creatingBranch} class="p-1 text-gx-neon hover:bg-gx-neon/10 rounded disabled:opacity-40" title="Create">
						<Check size={12} />
					</button>
					<button onclick={() => { showNewBranchInput = false; newBranchName = ''; }} class="p-1 text-gx-text-muted hover:bg-gx-bg-hover rounded" title="Cancel">
						<X size={12} />
					</button>
				{:else}
					<span class="flex-1 text-[10px] text-gx-text-muted uppercase tracking-wider font-medium">Local Branches</span>
					<button onclick={() => showNewBranchInput = true} class="p-1 text-gx-text-muted hover:text-gx-neon rounded" title="New branch">
						<Plus size={12} />
					</button>
					<button onclick={loadBranches} class="p-1 text-gx-text-muted hover:text-gx-neon rounded" title="Refresh branches">
						<RefreshCw size={10} />
					</button>
				{/if}
			</div>

			<!-- Local branches -->
			{#each localBranches as b}
				<div class="flex items-center gap-1.5 px-2 py-1.5 hover:bg-gx-bg-hover group {b.is_head ? 'bg-gx-neon/5' : ''}">
					<GitBranch size={12} class={b.is_head ? 'text-gx-neon' : 'text-gx-text-muted'} />
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-1">
							<span class="text-gx-text-secondary truncate {b.is_head ? 'font-semibold text-gx-neon' : ''}">{b.name}</span>
							{#if b.is_head}
								<span class="text-[9px] text-gx-neon bg-gx-neon/10 px-1 rounded">HEAD</span>
							{/if}
						</div>
						{#if b.last_commit}
							<p class="text-[10px] text-gx-text-disabled truncate">{b.last_commit}</p>
						{/if}
					</div>
					{#if !b.is_head}
						<button
							onclick={() => checkoutBranch(b.name)}
							class="opacity-0 group-hover:opacity-100 p-0.5 text-gx-text-muted hover:text-gx-neon rounded"
							title="Checkout"
						>
							<Check size={10} />
						</button>
						<button
							onclick={() => deleteBranch(b.name)}
							class="opacity-0 group-hover:opacity-100 p-0.5 text-gx-text-muted hover:text-gx-status-error rounded"
							title="Delete branch"
						>
							<Trash2 size={10} />
						</button>
					{/if}
				</div>
			{/each}

			<!-- Remote branches -->
			{#if remoteBranches.length > 0}
				<div class="px-2 py-1.5 border-t border-gx-border-subtle">
					<span class="text-[10px] text-gx-text-muted uppercase tracking-wider font-medium">Remote Branches</span>
				</div>
				{#each remoteBranches as b}
					<div class="flex items-center gap-1.5 px-2 py-1 hover:bg-gx-bg-hover group">
						<GitBranch size={12} class="text-gx-text-disabled" />
						<span class="text-gx-text-muted truncate flex-1">{b.name}</span>
						<button
							onclick={() => { navigator.clipboard.writeText(b.name); }}
							class="opacity-0 group-hover:opacity-100 p-0.5 text-gx-text-muted hover:text-gx-neon rounded"
							title="Copy name"
						>
							<Copy size={10} />
						</button>
					</div>
				{/each}
			{/if}

			{#if branches.length === 0}
				<div class="p-4 text-center text-gx-text-disabled">
					<GitBranch size={24} class="mx-auto mb-2 opacity-30" />
					<p>No branches found</p>
				</div>
			{/if}

		{:else}
			<!-- Commit log -->
			{#each commits as commit}
				<div class="flex items-start gap-2 px-2 py-1.5 border-b border-gx-border-subtle hover:bg-gx-bg-hover">
					<GitCommit size={14} class="text-gx-neon shrink-0 mt-0.5" />
					<div class="min-w-0 flex-1">
						<p class="text-gx-text-secondary truncate">{commit.message}</p>
						<div class="flex items-center gap-2 text-[10px] text-gx-text-disabled mt-0.5">
							<span class="font-mono text-gx-accent-cyan">{commit.id}</span>
							<span>{commit.author}</span>
							<span>{commit.time}</span>
						</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
