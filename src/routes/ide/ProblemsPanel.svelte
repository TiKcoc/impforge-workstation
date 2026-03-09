<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		CircleX, AlertTriangle, Info, Lightbulb,
		CheckCircle, ChevronRight, ChevronDown, File
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';

	interface Props {
		onNavigate?: (filePath: string, line: number, col: number) => void;
	}

	let { onNavigate }: Props = $props();

	// Diagnostic model matching LSP DiagnosticSeverity
	interface Diagnostic {
		file_path: string;
		line: number;
		character: number;
		end_line: number;
		end_character: number;
		severity: 'error' | 'warning' | 'info' | 'hint';
		message: string;
		source: string | null;
	}

	// State
	let diagnostics = $state<Diagnostic[]>([]);
	let collapsedFiles = $state<Set<string>>(new Set());
	let pollInterval = $state<ReturnType<typeof setInterval> | null>(null);

	// Severity ordering for sorting (lower = higher priority)
	const severityOrder: Record<string, number> = {
		error: 0,
		warning: 1,
		info: 2,
		hint: 3,
	};

	// Sorted diagnostics: errors first, then warnings, then info, then hints
	const sortedDiagnostics = $derived(
		[...diagnostics].sort((a, b) => {
			const fileCompare = a.file_path.localeCompare(b.file_path);
			if (fileCompare !== 0) return fileCompare;
			return (severityOrder[a.severity] ?? 4) - (severityOrder[b.severity] ?? 4);
		})
	);

	// Group diagnostics by file path, preserving sort order
	const groupedByFile = $derived.by(() => {
		const groups = new Map<string, Diagnostic[]>();
		for (const diag of sortedDiagnostics) {
			const existing = groups.get(diag.file_path);
			if (existing) {
				existing.push(diag);
			} else {
				groups.set(diag.file_path, [diag]);
			}
		}
		return groups;
	});

	// Counts by severity
	const errorCount = $derived(diagnostics.filter((d) => d.severity === 'error').length);
	const warningCount = $derived(diagnostics.filter((d) => d.severity === 'warning').length);
	const infoCount = $derived(diagnostics.filter((d) => d.severity === 'info' || d.severity === 'hint').length);

	// Total count
	const totalCount = $derived(diagnostics.length);

	// Filename from full path
	function getFileName(path: string): string {
		return path.split('/').pop() || path;
	}

	// Relative path for display (show last 2-3 segments)
	function getDisplayPath(path: string): string {
		const parts = path.split('/');
		if (parts.length <= 3) return path;
		return '.../' + parts.slice(-3).join('/');
	}

	// Toggle file group collapse
	function toggleFile(filePath: string) {
		const newCollapsed = new Set(collapsedFiles);
		if (newCollapsed.has(filePath)) {
			newCollapsed.delete(filePath);
		} else {
			newCollapsed.add(filePath);
		}
		collapsedFiles = newCollapsed;
	}

	// Navigate to diagnostic location
	function handleDiagnosticClick(diag: Diagnostic) {
		onNavigate?.(diag.file_path, diag.line, diag.character);
	}

	// Fetch diagnostics from Tauri backend
	async function fetchDiagnostics() {
		try {
			const result = await invoke<Diagnostic[]>('lsp_diagnostics');
			diagnostics = result;
		} catch {
			// LSP might not be running yet; keep current state (likely empty)
		}
	}

	onMount(() => {
		// Initial fetch
		fetchDiagnostics();
		// Poll every 2 seconds
		pollInterval = setInterval(fetchDiagnostics, 2000);
	});

	onDestroy(() => {
		if (pollInterval) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	});
</script>

<div class="flex flex-col h-full bg-[#0d1117] overflow-hidden">
	<!-- Header with counts -->
	<div class="flex items-center gap-3 px-3 py-1.5 border-b border-white/5 shrink-0">
		<div class="flex items-center gap-1.5 text-[11px]">
			<CircleX size={12} class="text-[#ff5370]" />
			<span class="text-[#ff5370]">{errorCount}</span>
		</div>
		<div class="flex items-center gap-1.5 text-[11px]">
			<AlertTriangle size={12} class="text-[#ffcb6b]" />
			<span class="text-[#ffcb6b]">{warningCount}</span>
		</div>
		<div class="flex items-center gap-1.5 text-[11px]">
			<Info size={12} class="text-[#82aaff]" />
			<span class="text-[#82aaff]">{infoCount}</span>
		</div>
	</div>

	<!-- Diagnostics list -->
	<div class="flex-1 overflow-auto">
		{#if totalCount === 0}
			<!-- Empty state -->
			<div class="flex flex-col items-center justify-center py-8 gap-2">
				<CheckCircle size={24} class="text-[#00FF66]/40" />
				<span class="text-xs text-white/30">No problems detected</span>
			</div>
		{:else}
			{#each [...groupedByFile.entries()] as [filePath, fileDiags]}
				{@const isCollapsed = collapsedFiles.has(filePath)}
				{@const fileErrors = fileDiags.filter((d) => d.severity === 'error').length}
				{@const fileWarnings = fileDiags.filter((d) => d.severity === 'warning').length}

				<!-- File group header -->
				<button
					onclick={() => toggleFile(filePath)}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-left hover:bg-white/5 transition-colors"
				>
					{#if isCollapsed}
						<ChevronRight size={12} class="text-white/30 shrink-0" />
					{:else}
						<ChevronDown size={12} class="text-white/30 shrink-0" />
					{/if}
					<File size={12} class="text-white/40 shrink-0" />
					<span class="text-xs text-white/70 font-medium truncate">{getFileName(filePath)}</span>
					<span class="text-[10px] text-white/20 truncate ml-1">{getDisplayPath(filePath)}</span>
					<div class="flex items-center gap-1.5 ml-auto shrink-0">
						{#if fileErrors > 0}
							<span class="text-[10px] text-[#ff5370]">{fileErrors}</span>
						{/if}
						{#if fileWarnings > 0}
							<span class="text-[10px] text-[#ffcb6b]">{fileWarnings}</span>
						{/if}
					</div>
				</button>

				<!-- Diagnostic entries -->
				{#if !isCollapsed}
					{#each fileDiags as diag}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							onclick={() => handleDiagnosticClick(diag)}
							onkeydown={(e) => e.key === 'Enter' && handleDiagnosticClick(diag)}
							role="button"
							tabindex="0"
							class="flex items-start gap-2 px-4 py-1 pl-7 cursor-pointer hover:bg-white/5 transition-colors group"
						>
							<!-- Severity icon -->
							<div class="shrink-0 mt-0.5">
								{#if diag.severity === 'error'}
									<CircleX size={12} class="text-[#ff5370]" />
								{:else if diag.severity === 'warning'}
									<AlertTriangle size={12} class="text-[#ffcb6b]" />
								{:else if diag.severity === 'info'}
									<Info size={12} class="text-[#82aaff]" />
								{:else}
									<Lightbulb size={12} class="text-[#89ddff]" />
								{/if}
							</div>

							<!-- Message -->
							<span class="flex-1 text-xs text-white/60 group-hover:text-white/80 leading-snug break-words">{diag.message}</span>

							<!-- Location and source -->
							<div class="flex items-center gap-2 shrink-0 text-[10px] text-white/25">
								{#if diag.source}
									<span class="bg-white/5 px-1 rounded">{diag.source}</span>
								{/if}
								<span class="font-mono">{diag.line}:{diag.character}</span>
							</div>
						</div>
					{/each}
				{/if}
			{/each}
		{/if}
	</div>
</div>
