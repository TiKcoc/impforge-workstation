<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		Play, Pause, SkipForward, ArrowDownToLine, ArrowUpFromLine,
		Square, RotateCw, Bug, ChevronDown, ChevronRight,
		Plus, X, Eye, Circle, Send
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// --- Types ---

	interface DebugState {
		status: 'idle' | 'running' | 'stopped' | 'terminated';
		stopped_reason?: string;
		stopped_thread_id?: number;
	}

	interface StackFrame {
		id: number;
		name: string;
		file_path?: string;
		line: number;
		column: number;
	}

	interface Variable {
		name: string;
		value: string;
		var_type?: string;
		children_ref: number;
	}

	interface Breakpoint {
		id: number;
		file_path: string;
		line: number;
		verified: boolean;
		condition?: string;
	}

	interface ConsoleEntry {
		type: 'input' | 'output' | 'error';
		text: string;
		timestamp: number;
	}

	// --- Props ---

	interface Props {
		sessionId?: string;
		onNavigate?: (filePath: string, line: number) => void;
	}

	let { sessionId = '', onNavigate }: Props = $props();

	// BenikUI style engine
	const widgetId = 'ide-debug';
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
	let controlsComp = $derived(styleEngine.getComponentStyle(widgetId, 'controls'));
	let controlsStyle = $derived(hasEngineStyle && controlsComp ? componentToCSS(controlsComp) : '');
	let callStackComp = $derived(styleEngine.getComponentStyle(widgetId, 'call-stack'));
	let callStackStyle = $derived(hasEngineStyle && callStackComp ? componentToCSS(callStackComp) : '');
	let variablesComp = $derived(styleEngine.getComponentStyle(widgetId, 'variables'));
	let variablesStyle = $derived(hasEngineStyle && variablesComp ? componentToCSS(variablesComp) : '');
	let watchComp = $derived(styleEngine.getComponentStyle(widgetId, 'watch'));
	let watchStyle = $derived(hasEngineStyle && watchComp ? componentToCSS(watchComp) : '');
	let consoleComp = $derived(styleEngine.getComponentStyle(widgetId, 'console'));
	let consoleStyle = $derived(hasEngineStyle && consoleComp ? componentToCSS(consoleComp) : '');

	// --- State ---

	let debugState = $state<DebugState>({ status: 'idle' });
	let stackFrames = $state<StackFrame[]>([]);
	let selectedFrameId = $state<number | null>(null);
	let localVariables = $state<Variable[]>([]);
	let globalVariables = $state<Variable[]>([]);
	let watchExpressions = $state<string[]>([]);
	let watchResults = $state<Map<string, string>>(new Map());
	let breakpoints = $state<Breakpoint[]>([]);
	let consoleEntries = $state<ConsoleEntry[]>([]);
	let consoleInput = $state('');
	let pollInterval = $state<ReturnType<typeof setInterval> | null>(null);

	// Section collapse state
	let showCallStack = $state(true);
	let showLocals = $state(true);
	let showGlobals = $state(false);
	let showWatch = $state(true);
	let showBreakpoints = $state(true);
	let showConsole = $state(true);

	// Variable expand state (tracks children_ref IDs that are expanded)
	let expandedVars = $state<Set<number>>(new Set());
	let childVarCache = $state<Map<number, Variable[]>>(new Map());

	// New watch input
	let newWatchExpr = $state('');
	let addingWatch = $state(false);

	// --- Derived ---

	const isActive = $derived(!!sessionId);
	const isRunning = $derived(debugState.status === 'running');
	const isStopped = $derived(debugState.status === 'stopped');
	const isTerminated = $derived(debugState.status === 'terminated');

	const statusColorClass = $derived.by(() => {
		switch (debugState.status) {
			case 'running': return 'text-gx-status-success';
			case 'stopped': return 'text-gx-status-warning';
			case 'terminated': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	});

	const statusLabel = $derived.by(() => {
		switch (debugState.status) {
			case 'running': return 'Running';
			case 'stopped': return debugState.stopped_reason || 'Paused';
			case 'terminated': return 'Terminated';
			default: return 'Idle';
		}
	});

	// --- Polling ---

	$effect(() => {
		if (isActive) {
			fetchDebugStatus();
			pollInterval = setInterval(fetchDebugStatus, 1000);
		} else {
			if (pollInterval) {
				clearInterval(pollInterval);
				pollInterval = null;
			}
			resetState();
		}
	});

	onDestroy(() => {
		if (pollInterval) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	});

	function resetState() {
		debugState = { status: 'idle' };
		stackFrames = [];
		selectedFrameId = null;
		localVariables = [];
		globalVariables = [];
		watchResults = new Map();
		consoleEntries = [];
	}

	// --- Data fetching ---

	async function fetchDebugStatus() {
		if (!sessionId) return;
		try {
			const state = await invoke<DebugState>('debug_status', { sessionId });
			const prevStatus = debugState.status;
			debugState = state;

			// When newly stopped, load context
			if (state.status === 'stopped' && prevStatus !== 'stopped') {
				await loadStoppedContext();
			}
		} catch {
			// Debug session might not be active yet
		}
	}

	async function loadStoppedContext() {
		await Promise.all([
			fetchStackTrace(),
			fetchBreakpoints(),
			evaluateWatchExpressions(),
		]);
	}

	async function fetchStackTrace() {
		if (!sessionId) return;
		try {
			const frames = await invoke<StackFrame[]>('debug_stack_trace', {
				sessionId,
				threadId: debugState.stopped_thread_id ?? 0,
			});
			stackFrames = frames;
			if (frames.length > 0 && selectedFrameId === null) {
				selectedFrameId = frames[0].id;
				await fetchVariables(frames[0].id);
			}
		} catch {
			stackFrames = [];
		}
	}

	async function fetchVariables(frameId: number) {
		if (!sessionId) return;
		try {
			const vars = await invoke<{ locals: Variable[]; globals: Variable[] }>(
				'debug_variables',
				{ sessionId, frameId },
			);
			localVariables = vars.locals;
			globalVariables = vars.globals;
		} catch {
			localVariables = [];
			globalVariables = [];
		}
	}

	async function fetchChildVariables(ref: number) {
		if (!sessionId || ref === 0) return;
		try {
			const children = await invoke<Variable[]>('debug_variable_children', {
				sessionId,
				variablesRef: ref,
			});
			childVarCache = new Map(childVarCache).set(ref, children);
		} catch {
			// Ignore
		}
	}

	async function fetchBreakpoints() {
		if (!sessionId) return;
		try {
			breakpoints = await invoke<Breakpoint[]>('debug_breakpoints', { sessionId });
		} catch {
			breakpoints = [];
		}
	}

	async function evaluateWatchExpressions() {
		if (!sessionId || !isStopped) return;
		const results = new Map<string, string>();
		for (const expr of watchExpressions) {
			try {
				const result = await invoke<string>('debug_evaluate', {
					sessionId,
					expression: expr,
					frameId: selectedFrameId ?? 0,
				});
				results.set(expr, result);
			} catch (e) {
				results.set(expr, `Error: ${e}`);
			}
		}
		watchResults = results;
	}

	// --- Debug control actions ---

	async function debugAction(action: string) {
		if (!sessionId) return;
		try {
			await invoke('debug_action', { sessionId, action });
			if (action === 'stop' || action === 'disconnect') {
				debugState = { status: 'terminated' };
			}
		} catch (e) {
			consoleEntries = [
				...consoleEntries,
				{ type: 'error', text: `Action "${action}" failed: ${e}`, timestamp: Date.now() },
			];
		}
	}

	// --- Stack frame navigation ---

	async function selectFrame(frame: StackFrame) {
		selectedFrameId = frame.id;
		await fetchVariables(frame.id);
		if (frame.file_path) {
			onNavigate?.(frame.file_path, frame.line);
		}
	}

	// --- Variable tree expand/collapse ---

	async function toggleVariable(v: Variable) {
		if (v.children_ref === 0) return;
		const newExpanded = new Set(expandedVars);
		if (newExpanded.has(v.children_ref)) {
			newExpanded.delete(v.children_ref);
		} else {
			newExpanded.add(v.children_ref);
			if (!childVarCache.has(v.children_ref)) {
				await fetchChildVariables(v.children_ref);
			}
		}
		expandedVars = newExpanded;
	}

	// --- Watch expressions ---

	function addWatch() {
		const expr = newWatchExpr.trim();
		if (!expr || watchExpressions.includes(expr)) return;
		watchExpressions = [...watchExpressions, expr];
		newWatchExpr = '';
		addingWatch = false;
		if (isStopped) evaluateWatchExpressions();
	}

	function removeWatch(expr: string) {
		watchExpressions = watchExpressions.filter((w) => w !== expr);
		const newResults = new Map(watchResults);
		newResults.delete(expr);
		watchResults = newResults;
	}

	function handleWatchKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			addWatch();
		} else if (e.key === 'Escape') {
			addingWatch = false;
			newWatchExpr = '';
		}
	}

	// --- Breakpoint toggle ---

	async function toggleBreakpoint(bp: Breakpoint) {
		if (!sessionId) return;
		try {
			await invoke('debug_toggle_breakpoint', {
				sessionId,
				breakpointId: bp.id,
			});
			await fetchBreakpoints();
		} catch {
			// Ignore
		}
	}

	async function removeBreakpoint(bp: Breakpoint) {
		if (!sessionId) return;
		try {
			await invoke('debug_remove_breakpoint', {
				sessionId,
				breakpointId: bp.id,
			});
			breakpoints = breakpoints.filter((b) => b.id !== bp.id);
		} catch {
			// Ignore
		}
	}

	// --- Debug console ---

	async function evaluateConsole() {
		const expr = consoleInput.trim();
		if (!expr || !sessionId) return;

		consoleEntries = [
			...consoleEntries,
			{ type: 'input', text: expr, timestamp: Date.now() },
		];
		consoleInput = '';

		try {
			const result = await invoke<string>('debug_evaluate', {
				sessionId,
				expression: expr,
				frameId: selectedFrameId ?? 0,
			});
			consoleEntries = [
				...consoleEntries,
				{ type: 'output', text: result, timestamp: Date.now() },
			];
		} catch (e) {
			consoleEntries = [
				...consoleEntries,
				{ type: 'error', text: `${e}`, timestamp: Date.now() },
			];
		}
	}

	function handleConsoleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			evaluateConsole();
		}
	}

	// --- Helpers ---

	function getFileName(path: string): string {
		return path.split(/[\\/]/).pop() || path;
	}

	function varValueColorClass(v: Variable): string {
		if (v.var_type === 'string' || v.value.startsWith('"')) return 'text-gx-status-success';
		if (v.var_type === 'number' || v.var_type === 'float' || v.var_type === 'int' || /^-?\d/.test(v.value)) return 'text-gx-accent-orange';
		if (v.value === 'true' || v.value === 'false' || v.var_type === 'boolean') return 'text-gx-accent-purple';
		if (v.value === 'null' || v.value === 'None' || v.value === 'nil') return 'text-gx-status-error';
		return 'text-gx-text-secondary';
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden text-xs" style={containerStyle}>
	<!-- Debug Toolbar -- always visible -->
	<div class="flex items-center gap-1 px-2 py-1.5 border-b border-gx-border-default shrink-0" style={headerStyle}>
		<Bug size={14} class={statusColorClass} />
		<span class="text-[11px] font-medium mr-1 {statusColorClass}">{statusLabel}</span>

		<div class="flex-1"></div>

		{#if isActive}
			<div class="flex items-center gap-0.5" style={controlsStyle}>
				{#if isStopped}
					<button onclick={() => debugAction('continue')} class="p-1 rounded hover:bg-gx-bg-hover text-gx-neon" title="Continue (F5)">
						<Play size={14} />
					</button>
				{:else if isRunning}
					<button onclick={() => debugAction('pause')} class="p-1 rounded hover:bg-gx-bg-hover text-gx-status-warning" title="Pause (F6)">
						<Pause size={14} />
					</button>
				{/if}
				<button onclick={() => debugAction('step_over')} disabled={!isStopped} class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-secondary disabled:opacity-20 disabled:cursor-not-allowed" title="Step Over (F10)">
					<SkipForward size={14} />
				</button>
				<button onclick={() => debugAction('step_in')} disabled={!isStopped} class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-secondary disabled:opacity-20 disabled:cursor-not-allowed" title="Step Into (F11)">
					<ArrowDownToLine size={14} />
				</button>
				<button onclick={() => debugAction('step_out')} disabled={!isStopped} class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-secondary disabled:opacity-20 disabled:cursor-not-allowed" title="Step Out (Shift+F11)">
					<ArrowUpFromLine size={14} />
				</button>

				<div class="w-px h-4 bg-gx-border-default mx-0.5"></div>

				<button onclick={() => debugAction('restart')} class="p-1 rounded hover:bg-gx-bg-hover text-gx-text-secondary" title="Restart (Ctrl+Shift+F5)">
					<RotateCw size={14} />
				</button>
				<button onclick={() => debugAction('stop')} class="p-1 rounded hover:bg-gx-bg-hover text-gx-status-error" title="Stop (Shift+F5)">
					<Square size={14} />
				</button>
			</div>
		{/if}
	</div>

	{#if !isActive}
		<!-- Empty state: no debug session -->
		<div class="flex flex-col items-center justify-center flex-1 gap-2 py-8">
			<Bug size={28} class="text-gx-text-disabled" />
			<span class="text-gx-text-muted">Start debugging</span>
			<span class="text-[10px] text-gx-text-muted">No active debug session</span>
		</div>
	{:else}
		<!-- Scrollable panels -->
		<div class="flex-1 overflow-auto">
			<!-- Call Stack Section -->
			<div class="border-b border-gx-border-default" style={callStackStyle}>
				<button
					onclick={() => showCallStack = !showCallStack}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
				>
					{#if showCallStack}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold text-[11px]">Call Stack</span>
					{#if stackFrames.length > 0}
						<span class="text-[10px] text-gx-text-muted ml-auto">{stackFrames.length}</span>
					{/if}
				</button>
				{#if showCallStack}
					{#if stackFrames.length === 0}
						<div class="px-4 py-2 text-gx-text-muted text-[11px]">
							{isStopped ? 'No frames available' : isRunning ? 'Running...' : 'Not paused'}
						</div>
					{:else}
						{#each stackFrames as frame, i}
							{@const isSelected = frame.id === selectedFrameId}
							<button
								onclick={() => selectFrame(frame)}
								class="flex items-center gap-1.5 w-full px-3 py-1 text-left hover:bg-gx-bg-hover transition-colors
									{isSelected ? 'bg-gx-neon/10' : ''}"
							>
								<span class="text-[10px] text-gx-text-muted w-4 text-right shrink-0">{i}</span>
								<span class="text-gx-text-primary truncate" class:text-gx-neon={isSelected}>{frame.name}</span>
								{#if frame.file_path}
									<span class="text-[10px] text-gx-text-muted ml-auto shrink-0">
										{getFileName(frame.file_path)}:{frame.line}
									</span>
								{/if}
							</button>
						{/each}
					{/if}
				{/if}
			</div>

			<!-- Variables: Locals -->
			<div class="border-b border-gx-border-default" style={variablesStyle}>
				<button
					onclick={() => showLocals = !showLocals}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
				>
					{#if showLocals}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold text-[11px]">Locals</span>
					{#if localVariables.length > 0}
						<span class="text-[10px] text-gx-text-muted ml-auto">{localVariables.length}</span>
					{/if}
				</button>
				{#if showLocals}
					{#if localVariables.length === 0}
						<div class="px-4 py-2 text-gx-text-muted text-[11px]">No local variables</div>
					{:else}
						{#each localVariables as v}
							{@const hasChildren = v.children_ref > 0}
							{@const isExpanded = expandedVars.has(v.children_ref)}
							<button
								onclick={() => toggleVariable(v)}
								class="flex items-center gap-1 w-full px-3 py-0.5 text-left hover:bg-gx-bg-hover transition-colors {hasChildren ? 'cursor-pointer' : 'cursor-default'}"
							>
								<span class="w-3 shrink-0">
									{#if hasChildren}
										{#if isExpanded}<ChevronDown size={10} class="text-gx-text-muted" />{:else}<ChevronRight size={10} class="text-gx-text-muted" />{/if}
									{/if}
								</span>
								<span class="text-gx-accent-blue truncate">{v.name}</span>
								{#if v.var_type}
									<span class="text-[10px] text-gx-accent-purple/60">:{v.var_type}</span>
								{/if}
								<span class="text-gx-text-muted mx-0.5">=</span>
								<span class="truncate {varValueColorClass(v)}">{v.value}</span>
							</button>
							<!-- Children (one level) -->
							{#if hasChildren && isExpanded}
								{@const children = childVarCache.get(v.children_ref) ?? []}
								{#each children as child}
									<div class="flex items-center gap-1 w-full pl-7 pr-3 py-0.5 hover:bg-gx-bg-hover">
										<span class="text-gx-accent-blue/70 truncate">{child.name}</span>
										{#if child.var_type}
											<span class="text-[10px] text-gx-accent-purple/40">:{child.var_type}</span>
										{/if}
										<span class="text-gx-text-muted mx-0.5">=</span>
										<span class="truncate {varValueColorClass(child)}">{child.value}</span>
									</div>
								{/each}
								{#if children.length === 0}
									<div class="pl-7 pr-3 py-0.5 text-gx-text-muted text-[10px]">Loading...</div>
								{/if}
							{/if}
						{/each}
					{/if}
				{/if}
			</div>

			<!-- Variables: Globals -->
			<div class="border-b border-gx-border-default">
				<button
					onclick={() => showGlobals = !showGlobals}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
				>
					{#if showGlobals}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold text-[11px]">Globals</span>
					{#if globalVariables.length > 0}
						<span class="text-[10px] text-gx-text-muted ml-auto">{globalVariables.length}</span>
					{/if}
				</button>
				{#if showGlobals}
					{#if globalVariables.length === 0}
						<div class="px-4 py-2 text-gx-text-muted text-[11px]">No global variables</div>
					{:else}
						{#each globalVariables as v}
							<div class="flex items-center gap-1 w-full px-3 py-0.5 hover:bg-gx-bg-hover">
								<span class="w-3 shrink-0"></span>
								<span class="text-gx-accent-blue truncate">{v.name}</span>
								{#if v.var_type}
									<span class="text-[10px] text-gx-accent-purple/60">:{v.var_type}</span>
								{/if}
								<span class="text-gx-text-muted mx-0.5">=</span>
								<span class="truncate {varValueColorClass(v)}">{v.value}</span>
							</div>
						{/each}
					{/if}
				{/if}
			</div>

			<!-- Watch Expressions -->
			<div class="border-b border-gx-border-default" style={watchStyle}>
				<div class="flex items-center w-full">
					<button
						onclick={() => showWatch = !showWatch}
						class="flex items-center gap-1.5 flex-1 px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
					>
						{#if showWatch}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
						<span class="font-semibold text-[11px]">Watch</span>
						{#if watchExpressions.length > 0}
							<span class="text-[10px] text-gx-text-muted">{watchExpressions.length}</span>
						{/if}
					</button>
					<button
						onclick={() => { addingWatch = true; showWatch = true; }}
						class="p-1 mr-1 text-gx-text-muted hover:text-gx-neon transition-colors"
						title="Add Watch Expression"
					>
						<Plus size={12} />
					</button>
				</div>
				{#if showWatch}
					{#each watchExpressions as expr}
						{@const result = watchResults.get(expr)}
						<div class="flex items-center gap-1 w-full px-3 py-0.5 hover:bg-gx-bg-hover group">
							<Eye size={10} class="text-gx-text-muted shrink-0" />
							<span class="text-gx-accent-blue truncate">{expr}</span>
							{#if result !== undefined}
								<span class="text-gx-text-muted mx-0.5">=</span>
								<span class="truncate {result.startsWith('Error') ? 'text-gx-status-error' : 'text-gx-text-secondary'}">{result}</span>
							{:else}
								<span class="text-gx-text-muted ml-0.5">not evaluated</span>
							{/if}
							<button
								onclick={() => removeWatch(expr)}
								class="ml-auto opacity-0 group-hover:opacity-100 p-0.5 hover:bg-gx-bg-hover rounded text-gx-text-muted shrink-0"
								title="Remove"
							>
								<X size={10} />
							</button>
						</div>
					{/each}
					{#if addingWatch}
						<div class="flex items-center gap-1 px-3 py-0.5">
							<Eye size={10} class="text-gx-neon/40 shrink-0" />
							<input
								type="text"
								bind:value={newWatchExpr}
								onkeydown={handleWatchKeydown}
								onblur={() => { if (!newWatchExpr.trim()) addingWatch = false; }}
								placeholder="Expression to watch"
								class="flex-1 bg-transparent border-b border-gx-neon/30 text-gx-text-primary placeholder:text-gx-text-muted outline-none py-0.5 text-xs"
							/>
						</div>
					{/if}
					{#if watchExpressions.length === 0 && !addingWatch}
						<div class="px-4 py-2 text-gx-text-muted text-[11px]">No watch expressions</div>
					{/if}
				{/if}
			</div>

			<!-- Breakpoints -->
			<div class="border-b border-gx-border-default">
				<button
					onclick={() => showBreakpoints = !showBreakpoints}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
				>
					{#if showBreakpoints}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold text-[11px]">Breakpoints</span>
					{#if breakpoints.length > 0}
						<span class="text-[10px] text-gx-text-muted ml-auto">{breakpoints.length}</span>
					{/if}
				</button>
				{#if showBreakpoints}
					{#if breakpoints.length === 0}
						<div class="px-4 py-2 text-gx-text-muted text-[11px]">No breakpoints set</div>
					{:else}
						{#each breakpoints as bp}
							<div class="flex items-center gap-1.5 w-full px-3 py-0.5 hover:bg-gx-bg-hover group">
								<button
									onclick={() => toggleBreakpoint(bp)}
									class="shrink-0"
									title={bp.verified ? 'Disable breakpoint' : 'Enable breakpoint'}
								>
									<Circle size={10} class={bp.verified ? 'text-gx-status-error fill-gx-status-error' : 'text-gx-text-muted'} />
								</button>
								<button
									onclick={() => onNavigate?.(bp.file_path, bp.line)}
									class="flex items-center gap-1 flex-1 min-w-0 text-left"
								>
									<span class="text-gx-text-secondary truncate">{getFileName(bp.file_path)}</span>
									<span class="text-[10px] text-gx-text-muted shrink-0">:{bp.line}</span>
								</button>
								{#if bp.condition}
									<span class="text-[10px] text-gx-status-warning/60 truncate max-w-[80px]" title={bp.condition}>
										if {bp.condition}
									</span>
								{/if}
								<button
									onclick={() => removeBreakpoint(bp)}
									class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-gx-bg-hover rounded text-gx-text-muted shrink-0"
									title="Remove breakpoint"
								>
									<X size={10} />
								</button>
							</div>
						{/each}
					{/if}
				{/if}
			</div>

			<!-- Debug Console -->
			<div style={consoleStyle}>
				<button
					onclick={() => showConsole = !showConsole}
					class="flex items-center gap-1.5 w-full px-2 py-1 text-gx-text-muted hover:bg-gx-bg-hover transition-colors"
				>
					{#if showConsole}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
					<span class="font-semibold text-[11px]">Debug Console</span>
				</button>
				{#if showConsole}
					<div class="flex flex-col max-h-48">
						<!-- Console output -->
						<div class="flex-1 overflow-auto px-3 py-1 min-h-[40px] max-h-36 font-mono text-[11px]">
							{#if consoleEntries.length === 0}
								<div class="text-gx-text-disabled py-1">Evaluate expressions when paused</div>
							{/if}
							{#each consoleEntries as entry}
								<div class="py-0.5 {entry.type === 'input' ? 'text-gx-accent-blue' : entry.type === 'error' ? 'text-gx-status-error' : 'text-gx-text-secondary'}">
									{#if entry.type === 'input'}
										<span class="text-gx-neon/50 select-none">{'>'} </span>
									{/if}
									{entry.text}
								</div>
							{/each}
						</div>
						<!-- Console input -->
						<div class="flex items-center gap-1 px-2 py-1 border-t border-gx-border-default">
							<span class="text-gx-neon/40 text-[11px] font-mono select-none">{'>'}</span>
							<input
								type="text"
								bind:value={consoleInput}
								onkeydown={handleConsoleKeydown}
								disabled={!isStopped}
								placeholder={isStopped ? 'Evaluate expression...' : 'Pause to evaluate'}
								class="flex-1 bg-transparent text-gx-text-primary placeholder:text-gx-text-disabled outline-none text-[11px] font-mono disabled:cursor-not-allowed"
							/>
							<button
								onclick={evaluateConsole}
								disabled={!isStopped || !consoleInput.trim()}
								class="p-0.5 text-gx-text-muted hover:text-gx-neon disabled:opacity-20 disabled:cursor-not-allowed transition-colors"
								title="Evaluate"
							>
								<Send size={10} />
							</button>
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
