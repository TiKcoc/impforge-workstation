<script lang="ts">
	/**
	 * DbClientPanel — Built-in Database Client with BenikUI integration
	 *
	 * SQLite-first database explorer with schema browser, SQL editor,
	 * result grid, and query history. Comparable to JetBrains DataGrip.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - toolbar: Connection bar and actions
	 *   - result-grid: Query results table
	 *   - schema-tree: Schema browser sidebar
	 */

	import { invoke } from '@tauri-apps/api/core';
	import {
		Database, Table, Play, Plus, Trash2, Download,
		ChevronRight, ChevronDown, Columns3, Key, Loader2,
		Clock, X, PlugZap, Unplug
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface ConnectionInfo {
		id: string;
		name: string;
		path: string;
		dialect: string;
	}

	interface ColumnInfo {
		name: string;
		col_type: string;
		nullable: boolean;
		primary_key: boolean;
		default_value: string | null;
	}

	interface TableInfo {
		name: string;
		row_count: number;
		columns: ColumnInfo[];
	}

	interface IndexInfo {
		name: string;
		table_name: string;
		columns: string[];
		unique: boolean;
	}

	interface SchemaOverview {
		tables: TableInfo[];
		views: string[];
		indexes: IndexInfo[];
	}

	interface QueryResult {
		columns: string[];
		rows: (string | number | null)[][];
		row_count: number;
		elapsed_ms: number;
		affected_rows: number | null;
	}

	interface QueryHistoryEntry {
		query: string;
		connection_id: string;
		timestamp: string;
		elapsed_ms: number;
		row_count: number;
		success: boolean;
	}

	// BenikUI style engine
	const widgetId = 'ide-db-client';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let toolbarComp = $derived(styleEngine.getComponentStyle(widgetId, 'toolbar'));
	let toolbarStyle = $derived(hasEngineStyle && toolbarComp ? componentToCSS(toolbarComp) : '');
	let gridComp = $derived(styleEngine.getComponentStyle(widgetId, 'result-grid'));
	let gridStyle = $derived(hasEngineStyle && gridComp ? componentToCSS(gridComp) : '');

	// State
	let connections = $state<ConnectionInfo[]>([]);
	let activeConn = $state<string>('');
	let schema = $state<SchemaOverview | null>(null);
	let query = $state('SELECT * FROM sqlite_master LIMIT 50;');
	let result = $state<QueryResult | null>(null);
	let executing = $state(false);
	let error = $state('');
	let expandedTables = $state<Set<string>>(new Set());
	let tab = $state<'results' | 'history'>('results');
	let history = $state<QueryHistoryEntry[]>([]);

	// New connection form
	let showConnForm = $state(false);
	let newConnName = $state('');
	let newConnPath = $state('');

	async function connect() {
		if (!newConnName.trim() || !newConnPath.trim()) return;
		try {
			const conn = await invoke<ConnectionInfo>('db_connect', {
				name: newConnName,
				path: newConnPath,
			});
			connections = [...connections, conn];
			activeConn = conn.id;
			showConnForm = false;
			newConnName = '';
			newConnPath = '';
			await loadSchema();
		} catch (e) {
			error = String(e);
		}
	}

	async function disconnect(connId: string) {
		try {
			await invoke('db_disconnect', { connectionId: connId });
			connections = connections.filter(c => c.id !== connId);
			if (activeConn === connId) {
				activeConn = connections[0]?.id || '';
				schema = null;
				result = null;
			}
		} catch (e) {
			error = String(e);
		}
	}

	async function loadSchema() {
		if (!activeConn) return;
		try {
			schema = await invoke<SchemaOverview>('db_schema', { connectionId: activeConn });
		} catch (e) {
			error = String(e);
		}
	}

	async function executeQuery() {
		if (!activeConn || !query.trim()) return;
		executing = true;
		error = '';
		try {
			result = await invoke<QueryResult>('db_execute_query', {
				connectionId: activeConn,
				query: query,
			});
			tab = 'results';
		} catch (e) {
			error = String(e);
			result = null;
		}
		executing = false;
	}

	async function loadHistory() {
		try {
			history = await invoke<QueryHistoryEntry[]>('db_query_history', { limit: 50 });
			tab = 'history';
		} catch (e) {
			error = String(e);
		}
	}

	async function exportCsv() {
		if (!result) return;
		try {
			const csv = await invoke<string>('db_export_csv', {
				columns: result.columns,
				rows: result.rows,
			});
			// Copy to clipboard
			await navigator.clipboard.writeText(csv);
		} catch (e) {
			error = String(e);
		}
	}

	function toggleTable(name: string) {
		const updated = new Set(expandedTables);
		if (updated.has(name)) updated.delete(name);
		else updated.add(name);
		expandedTables = updated;
	}

	function selectTable(name: string) {
		query = `SELECT * FROM "${name}" LIMIT 100;`;
	}

	function handleKeydown(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
			e.preventDefault();
			executeQuery();
		}
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Toolbar -->
	<div class="flex items-center gap-2 px-2 py-1.5 border-b border-gx-border-subtle shrink-0" style={toolbarStyle}>
		<Database size={12} class="text-gx-neon shrink-0" />

		<!-- Connection selector -->
		{#if connections.length > 0}
			<select
				bind:value={activeConn}
				onchange={loadSchema}
				class="bg-gx-bg-secondary text-xs text-gx-text-primary border border-gx-border-default rounded px-1.5 py-0.5 outline-none focus:border-gx-neon"
			>
				{#each connections as conn}
					<option value={conn.id}>{conn.name}</option>
				{/each}
			</select>
			<button
				onclick={() => disconnect(activeConn)}
				class="p-1 text-gx-text-muted hover:text-gx-status-error"
				title="Disconnect"
			>
				<Unplug size={11} />
			</button>
		{/if}

		<button
			onclick={() => { showConnForm = !showConnForm; }}
			class="p-1 text-gx-text-muted hover:text-gx-neon"
			title="New connection"
		>
			<Plus size={12} />
		</button>

		<div class="flex-1"></div>

		<button
			onclick={executeQuery}
			disabled={executing || !activeConn}
			class="flex items-center gap-1 px-2 py-0.5 text-xs bg-gx-neon/20 text-gx-neon border border-gx-neon/30 rounded hover:bg-gx-neon/30 disabled:opacity-40"
		>
			{#if executing}
				<Loader2 size={11} class="animate-spin" />
			{:else}
				<Play size={11} />
			{/if}
			Run
		</button>

		<button
			onclick={loadHistory}
			class="p-1 text-gx-text-muted hover:text-gx-text-secondary"
			title="Query history"
		>
			<Clock size={11} />
		</button>

		{#if result}
			<button
				onclick={exportCsv}
				class="p-1 text-gx-text-muted hover:text-gx-text-secondary"
				title="Export CSV to clipboard"
			>
				<Download size={11} />
			</button>
		{/if}
	</div>

	<!-- New connection form -->
	{#if showConnForm}
		<div class="px-2 py-2 border-b border-gx-border-subtle bg-gx-bg-secondary/50 space-y-1.5">
			<input
				bind:value={newConnName}
				placeholder="Connection name"
				class="w-full bg-gx-bg-secondary text-xs text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon"
			/>
			<input
				bind:value={newConnPath}
				placeholder="SQLite path (e.g. /path/to/db.sqlite)"
				class="w-full bg-gx-bg-secondary text-xs text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon"
			/>
			<div class="flex gap-1.5">
				<button
					onclick={connect}
					class="flex items-center gap-1 px-2 py-0.5 text-xs bg-gx-neon/20 text-gx-neon rounded hover:bg-gx-neon/30"
				>
					<PlugZap size={10} />
					Connect
				</button>
				<button
					onclick={() => { showConnForm = false; }}
					class="px-2 py-0.5 text-xs text-gx-text-muted hover:text-gx-text-secondary"
				>
					Cancel
				</button>
			</div>
		</div>
	{/if}

	<!-- Main content: Schema + Query + Results -->
	<div class="flex flex-1 overflow-hidden">
		<!-- Schema browser (left) -->
		{#if schema}
			<div class="w-44 border-r border-gx-border-subtle overflow-auto shrink-0">
				<div class="px-2 py-1 text-[10px] text-gx-text-disabled uppercase tracking-wider">Tables ({schema.tables.length})</div>
				{#each schema.tables as table}
					<div>
						<button
							onclick={() => toggleTable(table.name)}
							ondblclick={() => selectTable(table.name)}
							class="flex items-center gap-1 w-full px-2 py-0.5 text-xs hover:bg-gx-bg-hover text-left"
						>
							{#if expandedTables.has(table.name)}
								<ChevronDown size={10} class="text-gx-text-disabled shrink-0" />
							{:else}
								<ChevronRight size={10} class="text-gx-text-disabled shrink-0" />
							{/if}
							<Table size={10} class="text-gx-accent-cyan shrink-0" />
							<span class="text-gx-text-secondary truncate">{table.name}</span>
							<span class="ml-auto text-[9px] text-gx-text-disabled">{table.row_count}</span>
						</button>
						{#if expandedTables.has(table.name)}
							{#each table.columns as col}
								<div class="flex items-center gap-1 pl-6 pr-2 py-0.5 text-[10px] text-gx-text-muted">
									{#if col.primary_key}
										<Key size={8} class="text-gx-status-warning shrink-0" />
									{:else}
										<Columns3 size={8} class="text-gx-text-disabled shrink-0" />
									{/if}
									<span class="truncate">{col.name}</span>
									<span class="ml-auto text-gx-text-disabled">{col.col_type}</span>
								</div>
							{/each}
						{/if}
					</div>
				{/each}

				{#if schema.views.length > 0}
					<div class="px-2 py-1 mt-1 text-[10px] text-gx-text-disabled uppercase tracking-wider">Views ({schema.views.length})</div>
					{#each schema.views as view}
						<button
							ondblclick={() => { query = `SELECT * FROM "${view}" LIMIT 100;`; }}
							class="flex items-center gap-1 w-full px-2 py-0.5 text-xs hover:bg-gx-bg-hover text-left"
						>
							<Table size={10} class="text-gx-accent-magenta shrink-0" />
							<span class="text-gx-text-secondary truncate">{view}</span>
						</button>
					{/each}
				{/if}
			</div>
		{/if}

		<!-- Query editor + results (right) -->
		<div class="flex-1 flex flex-col overflow-hidden">
			<!-- SQL editor -->
			<div class="shrink-0 border-b border-gx-border-subtle">
				<textarea
					bind:value={query}
					onkeydown={handleKeydown}
					placeholder="Enter SQL query... (Ctrl+Enter to execute)"
					rows="4"
					class="w-full bg-gx-bg-secondary text-xs text-gx-text-primary font-mono px-3 py-2 outline-none resize-none"
					spellcheck="false"
				></textarea>
			</div>

			<!-- Error display -->
			{#if error}
				<div class="flex items-center gap-1.5 px-3 py-1 bg-gx-status-error/10 text-gx-status-error text-xs border-b border-gx-status-error/20">
					<X size={10} class="shrink-0" />
					<span class="truncate">{error}</span>
					<button onclick={() => { error = ''; }} class="ml-auto shrink-0 hover:text-gx-text-primary">
						<X size={10} />
					</button>
				</div>
			{/if}

			<!-- Results tabs -->
			<div class="flex items-center gap-3 px-3 py-1 border-b border-gx-border-subtle text-[10px] shrink-0">
				<button
					onclick={() => { tab = 'results'; }}
					class={tab === 'results' ? 'text-gx-neon border-b border-gx-neon pb-0.5' : 'text-gx-text-muted hover:text-gx-text-secondary'}
				>
					Results {result ? `(${result.row_count})` : ''}
				</button>
				<button
					onclick={loadHistory}
					class={tab === 'history' ? 'text-gx-neon border-b border-gx-neon pb-0.5' : 'text-gx-text-muted hover:text-gx-text-secondary'}
				>
					History
				</button>
				{#if result}
					<span class="ml-auto text-gx-text-disabled">
						{result.elapsed_ms.toFixed(1)}ms
						{#if result.affected_rows !== null}
							| {result.affected_rows} affected
						{/if}
					</span>
				{/if}
			</div>

			<!-- Result grid -->
			<div class="flex-1 overflow-auto" style={gridStyle}>
				{#if tab === 'results'}
					{#if result && result.columns.length > 0}
						<table class="w-full text-xs border-collapse">
							<thead class="sticky top-0 bg-gx-bg-elevated z-10">
								<tr>
									{#each result.columns as col}
										<th class="px-2 py-1 text-left text-gx-text-muted font-medium border-b border-gx-border-default whitespace-nowrap">{col}</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each result.rows as row, i}
									<tr class="hover:bg-gx-bg-hover {i % 2 === 0 ? '' : 'bg-gx-bg-secondary/30'}">
										{#each row as cell}
											<td class="px-2 py-0.5 text-gx-text-secondary font-mono border-b border-gx-border-subtle/50 whitespace-nowrap max-w-xs truncate">
												{#if cell === null}
													<span class="text-gx-text-disabled italic">NULL</span>
												{:else}
													{cell}
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					{:else if result && result.affected_rows !== null}
						<div class="p-4 text-center text-xs text-gx-text-muted">
							Query executed — {result.affected_rows} row{result.affected_rows !== 1 ? 's' : ''} affected ({result.elapsed_ms.toFixed(1)}ms)
						</div>
					{:else if !result}
						<div class="p-4 text-center text-gx-text-disabled">
							<Database size={24} class="mx-auto mb-2 opacity-30" />
							<p class="text-xs">Run a query to see results</p>
							<p class="text-[10px] mt-1">Ctrl+Enter to execute</p>
						</div>
					{/if}
				{:else if tab === 'history'}
					{#if history.length > 0}
						{#each [...history].reverse() as entry}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								onclick={() => { query = entry.query; tab = 'results'; }}
								onkeydown={(e) => e.key === 'Enter' && (() => { query = entry.query; tab = 'results'; })()}
								role="button"
								tabindex="0"
								class="px-3 py-1.5 border-b border-gx-border-subtle/50 hover:bg-gx-bg-hover cursor-pointer"
							>
								<div class="flex items-center gap-2 text-[10px] text-gx-text-disabled mb-0.5">
									<span>{new Date(entry.timestamp).toLocaleTimeString()}</span>
									<span>{entry.elapsed_ms.toFixed(1)}ms</span>
									<span>{entry.row_count} rows</span>
									{#if !entry.success}
										<span class="text-gx-status-error">FAILED</span>
									{/if}
								</div>
								<code class="text-xs text-gx-text-muted font-mono line-clamp-2">{entry.query}</code>
							</div>
						{/each}
					{:else}
						<div class="p-4 text-center text-gx-text-disabled text-xs">No query history yet</div>
					{/if}
				{/if}
			</div>
		</div>
	</div>
</div>
