<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GitBranch, GitCommit, Plus, Minus, FileEdit,
		FileQuestion, AlertCircle, RefreshCw, ChevronDown, ChevronRight
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';

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

	interface Props {
		workspacePath?: string;
	}

	let { workspacePath = '' }: Props = $props();

	let activeTab = $state<'changes' | 'log'>('changes');
	let branch = $state('');
	let files = $state<GitFileStatus[]>([]);
	let commits = $state<CommitInfo[]>([]);
	let diffContent = $state('');
	let selectedFile = $state('');
	let loading = $state(false);
	let showStaged = $state(true);
	let showUnstaged = $state(true);

	const stagedFiles = $derived(files.filter((f) => f.staged));
	const unstagedFiles = $derived(files.filter((f) => !f.staged));

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

	function statusIcon(status: string) {
		switch (status) {
			case 'added':
			case 'untracked':
				return { icon: Plus, color: 'text-green-400' };
			case 'modified':
				return { icon: FileEdit, color: 'text-yellow-400' };
			case 'deleted':
				return { icon: Minus, color: 'text-red-400' };
			case 'conflict':
				return { icon: AlertCircle, color: 'text-red-500' };
			default:
				return { icon: FileQuestion, color: 'text-white/40' };
		}
	}

	function switchTab(tab: 'changes' | 'log') {
		activeTab = tab;
		if (tab === 'log' && commits.length === 0) loadLog();
	}
</script>

<div class="flex flex-col h-full bg-[#0d1117] overflow-hidden">
	<!-- Header -->
	<div class="flex items-center gap-2 px-2 py-1.5 border-b border-white/5 shrink-0">
		<GitBranch size={14} class="text-[#00FF66]" />
		<span class="text-xs font-semibold text-white/70">{branch || 'No repo'}</span>
		<div class="flex-1"></div>
		<button onclick={refresh} class="p-0.5 text-white/40 hover:text-[#00FF66]" title="Refresh">
			<RefreshCw size={12} class={loading ? 'animate-spin' : ''} />
		</button>
	</div>

	<!-- Tab selector -->
	<div class="flex border-b border-white/5 shrink-0">
		<button
			onclick={() => switchTab('changes')}
			class="flex-1 py-1.5 text-xs text-center transition-colors
				{activeTab === 'changes' ? 'text-[#00FF66] border-b border-[#00FF66]' : 'text-white/40 hover:text-white/60'}"
		>
			Changes ({files.length})
		</button>
		<button
			onclick={() => switchTab('log')}
			class="flex-1 py-1.5 text-xs text-center transition-colors
				{activeTab === 'log' ? 'text-[#00FF66] border-b border-[#00FF66]' : 'text-white/40 hover:text-white/60'}"
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
					class="flex items-center gap-1 w-full px-2 py-1 text-white/50 hover:bg-white/5"
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
							class="flex items-center gap-1.5 w-full px-4 py-1 hover:bg-white/5 text-left group cursor-pointer
								{selectedFile === file.path ? 'bg-white/5' : ''}"
						>
							<si.icon size={12} class={si.color} />
							<span class="text-white/70 truncate flex-1">{file.path}</span>
							<button
								onclick={(e) => { e.stopPropagation(); unstageFile(file.path); }}
								class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/10 rounded text-white/40"
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
					class="flex items-center gap-1 w-full px-2 py-1 text-white/50 hover:bg-white/5"
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
							class="flex items-center gap-1.5 w-full px-4 py-1 hover:bg-white/5 text-left group cursor-pointer
								{selectedFile === file.path ? 'bg-white/5' : ''}"
						>
							<si.icon size={12} class={si.color} />
							<span class="text-white/70 truncate flex-1">{file.path}</span>
							<button
								onclick={(e) => { e.stopPropagation(); stageFile(file.path); }}
								class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/10 rounded text-white/40"
								title="Stage"
							>
								<Plus size={10} />
							</button>
						</div>
					{/each}
				{/if}
			{/if}

			{#if files.length === 0 && !loading}
				<div class="p-4 text-center text-white/30">
					<GitCommit size={24} class="mx-auto mb-2 opacity-30" />
					<p>Working tree clean</p>
				</div>
			{/if}

			<!-- Diff preview -->
			{#if diffContent}
				<div class="border-t border-white/5 mt-2">
					<div class="px-2 py-1 text-[10px] text-white/30 bg-[#161b22]">
						Diff: {selectedFile}
					</div>
					<pre class="p-2 text-[11px] font-mono overflow-auto max-h-60 bg-[#0a0e14]">{#each diffContent.split('\n') as line}<span class="{line.startsWith('+') ? 'text-green-400' : line.startsWith('-') ? 'text-red-400' : 'text-white/50'}">{line}
</span>{/each}</pre>
				</div>
			{/if}
		{:else}
			<!-- Commit log -->
			{#each commits as commit}
				<div class="flex items-start gap-2 px-2 py-1.5 border-b border-white/5 hover:bg-white/5">
					<GitCommit size={14} class="text-[#00FF66] shrink-0 mt-0.5" />
					<div class="min-w-0 flex-1">
						<p class="text-white/70 truncate">{commit.message}</p>
						<div class="flex items-center gap-2 text-[10px] text-white/30 mt-0.5">
							<span class="font-mono text-[#89ddff]">{commit.id}</span>
							<span>{commit.author}</span>
							<span>{commit.time}</span>
						</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
