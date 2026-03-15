<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Table2, FilePlus, Save, Download, Upload, Bold, Italic, AlignLeft,
		AlignCenter, AlignRight, Paintbrush, Type, Sparkles, Brain,
		TrendingUp, BarChart3, Search, Loader2, Trash2, AlertCircle,
		X, ChevronDown, Plus, FileSpreadsheet, Hash, Percent, Calendar,
		PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen,
		Copy, Scissors, Clipboard, Undo2, Redo2, ArrowDown, ArrowRight
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-sheets';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface CellValue {
		type: 'Empty' | 'Text' | 'Number' | 'Bool' | 'Error';
		value?: string | number | boolean;
	}

	interface CellFormat {
		bold: boolean;
		italic: boolean;
		text_color: string | null;
		bg_color: string | null;
		number_format: string | null;
		align: 'left' | 'center' | 'right';
	}

	interface Cell {
		value: CellValue;
		formula: string | null;
		format: CellFormat;
		note: string | null;
	}

	interface Sheet {
		name: string;
		cells: Record<string, Cell>;
		col_widths: Record<string, number>;
		row_heights: Record<string, number>;
	}

	interface Spreadsheet {
		id: string;
		name: string;
		sheets: Sheet[];
		created_at: string;
		updated_at: string;
	}

	interface SpreadsheetMeta {
		id: string;
		name: string;
		sheet_count: number;
		cell_count: number;
		updated_at: string;
	}

	interface AnalysisResult {
		summary: string;
		trends: string[];
		outliers: { cell_ref: string; value: number; reason: string }[];
		correlations: { columns: [string, string]; coefficient: number; description: string }[];
		suggested_charts: { chart_type: string; reason: string; data_range: string }[];
		stats: { count: number; sum: number; average: number; min: number; max: number; std_dev: number };
	}

	// ---- State ---------------------------------------------------------------
	let spreadsheets = $state<SpreadsheetMeta[]>([]);
	let activeSpreadsheet = $state<Spreadsheet | null>(null);
	let activeSheetIndex = $state(0);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state<string | null>(null);

	// Grid state
	let selectedCell = $state<string | null>(null);
	let selectionStart = $state<string | null>(null);
	let selectionEnd = $state<string | null>(null);
	let editingCell = $state<string | null>(null);
	let editValue = $state('');
	let formulaBarValue = $state('');
	let isDragging = $state(false);

	// Grid dimensions (visible viewport)
	let visibleCols = $state(26); // A-Z default
	let visibleRows = $state(100);

	// UI panel state
	let sidebarOpen = $state(true);
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state<string | null>(null);
	let aiAnalysis = $state<AnalysisResult | null>(null);
	let aiDescription = $state('');
	let aiError = $state<string | null>(null);

	// New spreadsheet dialog
	let showNewDialog = $state(false);
	let newSheetName = $state('');

	// Auto-save
	let autoSaveTimer: ReturnType<typeof setInterval> | null = null;
	let isDirty = $state(false);
	let saveIndicator = $state<'saved' | 'saving' | 'unsaved' | 'idle'>('idle');

	// File input for import
	let fileInputEl: HTMLInputElement | undefined = $state();

	// Search
	let searchQuery = $state('');

	// ---- Derived -------------------------------------------------------------
	let activeSheet = $derived(
		activeSpreadsheet?.sheets[activeSheetIndex] ?? null
	);

	let filteredSpreadsheets = $derived(
		searchQuery.trim()
			? spreadsheets.filter(s =>
				s.name.toLowerCase().includes(searchQuery.toLowerCase())
			)
			: spreadsheets
	);

	// Selection stats (SUM/AVG/COUNT of selected range)
	let selectionStats = $derived.by(() => {
		if (!activeSheet || !selectedCell) return null;
		const cells = getSelectedCells();
		if (cells.length <= 1) return null;

		let sum = 0;
		let count = 0;
		for (const ref of cells) {
			const cell = activeSheet.cells[ref];
			if (cell?.value?.type === 'Number' && cell.value.value != null) {
				sum += cell.value.value as number;
				count++;
			}
		}
		if (count === 0) return null;
		return { sum: round(sum, 4), average: round(sum / count, 4), count };
	});

	// ---- Helpers -------------------------------------------------------------
	function round(n: number, decimals: number): number {
		const f = Math.pow(10, decimals);
		return Math.round(n * f) / f;
	}

	function colToLetter(col: number): string {
		let result = '';
		let c = col;
		do {
			result = String.fromCharCode(65 + (c % 26)) + result;
			c = Math.floor(c / 26) - 1;
		} while (c >= 0);
		return result;
	}

	function letterToCol(letters: string): number {
		let result = 0;
		for (const ch of letters) {
			result = result * 26 + (ch.charCodeAt(0) - 64);
		}
		return result - 1;
	}

	function parseCellRef(ref: string): { col: number; row: number } | null {
		const match = ref.match(/^([A-Z]+)(\d+)$/);
		if (!match) return null;
		return { col: letterToCol(match[1]), row: parseInt(match[2]) - 1 };
	}

	function makeCellRef(col: number, row: number): string {
		return `${colToLetter(col)}${row + 1}`;
	}

	function getCellDisplay(cell: Cell | undefined): string {
		if (!cell) return '';
		switch (cell.value.type) {
			case 'Number':
				if (cell.format?.number_format === '0%' && cell.value.value != null) {
					return `${round(cell.value.value as number * 100, 2)}%`;
				}
				if (cell.format?.number_format === '#,##0.00' && cell.value.value != null) {
					return (cell.value.value as number).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
				}
				return cell.value.value != null ? String(cell.value.value) : '';
			case 'Text': return cell.value.value as string ?? '';
			case 'Bool': return cell.value.value ? 'TRUE' : 'FALSE';
			case 'Error': return `#ERR: ${cell.value.value ?? ''}`;
			default: return '';
		}
	}

	function getCellEditValue(cell: Cell | undefined): string {
		if (!cell) return '';
		if (cell.formula) return cell.formula;
		return getCellDisplay(cell);
	}

	function getSelectedCells(): string[] {
		if (!selectionStart || !selectionEnd) {
			return selectedCell ? [selectedCell] : [];
		}
		const start = parseCellRef(selectionStart);
		const end = parseCellRef(selectionEnd);
		if (!start || !end) return selectedCell ? [selectedCell] : [];

		const minCol = Math.min(start.col, end.col);
		const maxCol = Math.max(start.col, end.col);
		const minRow = Math.min(start.row, end.row);
		const maxRow = Math.max(start.row, end.row);

		const cells: string[] = [];
		for (let r = minRow; r <= maxRow; r++) {
			for (let c = minCol; c <= maxCol; c++) {
				cells.push(makeCellRef(c, r));
			}
		}
		return cells;
	}

	function isCellSelected(ref: string): boolean {
		if (!selectionStart || !selectionEnd) return ref === selectedCell;
		const start = parseCellRef(selectionStart);
		const end = parseCellRef(selectionEnd);
		const cell = parseCellRef(ref);
		if (!start || !end || !cell) return false;

		return cell.col >= Math.min(start.col, end.col) &&
			cell.col <= Math.max(start.col, end.col) &&
			cell.row >= Math.min(start.row, end.row) &&
			cell.row <= Math.max(start.row, end.row);
	}

	function getSelectionRange(): string | null {
		if (!selectionStart || !selectionEnd) return selectedCell;
		if (selectionStart === selectionEnd) return selectionStart;
		return `${selectionStart}:${selectionEnd}`;
	}

	// ---- Data loading --------------------------------------------------------
	async function loadSpreadsheets() {
		try {
			loading = true;
			error = null;
			spreadsheets = await invoke<SpreadsheetMeta[]>('sheets_list');
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			loading = false;
		}
	}

	async function openSpreadsheet(id: string) {
		try {
			loading = true;
			error = null;
			activeSpreadsheet = await invoke<Spreadsheet>('sheets_open', { id });
			activeSheetIndex = 0;
			selectedCell = 'A1';
			selectionStart = null;
			selectionEnd = null;
			editingCell = null;
			isDirty = false;
			saveIndicator = 'idle';
			updateFormulaBar();
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			loading = false;
		}
	}

	async function createSpreadsheet() {
		const name = newSheetName.trim() || 'Untitled Spreadsheet';
		try {
			const ss = await invoke<Spreadsheet>('sheets_create', { name });
			showNewDialog = false;
			newSheetName = '';
			await loadSpreadsheets();
			await openSpreadsheet(ss.id);
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	async function deleteSpreadsheet(id: string) {
		try {
			await invoke('sheets_delete', { id });
			if (activeSpreadsheet?.id === id) {
				activeSpreadsheet = null;
				selectedCell = null;
			}
			await loadSpreadsheets();
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	async function saveSpreadsheet() {
		if (!activeSpreadsheet) return;
		try {
			saving = true;
			saveIndicator = 'saving';
			await invoke('sheets_save', {
				id: activeSpreadsheet.id,
				data: activeSpreadsheet
			});
			isDirty = false;
			saveIndicator = 'saved';
			setTimeout(() => { if (saveIndicator === 'saved') saveIndicator = 'idle'; }, 2000);
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
			saveIndicator = 'unsaved';
		} finally {
			saving = false;
		}
	}

	function triggerImport() {
		fileInputEl?.click();
	}

	async function handleFileSelected(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		// Tauri webview gives us the real file path via webkitRelativePath or name.
		// For Tauri 2.x, we can read the file path from the input's value on supported platforms.
		// We use the file name and write to a temp location for import, OR use the
		// webview's actual path. Tauri FS plugin gives us the path via convertFileSrc.
		// Simplest: read the file content, write to temp, then import.
		try {
			loading = true;
			error = null;

			// Read file as ArrayBuffer, write to temp dir, then import
			const buffer = await file.arrayBuffer();
			const bytes = new Uint8Array(buffer);

			// Use Tauri FS to write to app temp directory
			const { appDataDir } = await import('@tauri-apps/api/path');
			const { writeFile, mkdir, exists } = await import('@tauri-apps/plugin-fs');
			const appData = await appDataDir();
			const importDir = `${appData}/import_temp`;

			if (!(await exists(importDir))) {
				await mkdir(importDir, { recursive: true });
			}

			const tempPath = `${importDir}/${file.name}`;
			await writeFile(tempPath, bytes);

			const ss = await invoke<Spreadsheet>('sheets_import_file', { path: tempPath });
			await loadSpreadsheets();
			await openSpreadsheet(ss.id);
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			loading = false;
			// Reset the input so the same file can be re-selected
			if (input) input.value = '';
		}
	}

	async function exportSpreadsheet(format: string) {
		if (!activeSpreadsheet) return;
		try {
			const path = await invoke<string>('sheets_export', {
				id: activeSpreadsheet.id,
				format
			});
			error = null;
			// Show brief success notification
			aiResult = `Exported to: ${path}`;
			setTimeout(() => { if (aiResult?.startsWith('Exported')) aiResult = null; }, 5000);
		} catch (e: any) {
			error = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	// ---- Cell editing --------------------------------------------------------
	function selectCell(ref: string, shiftKey = false) {
		if (editingCell && editingCell !== ref) {
			commitEdit();
		}

		if (shiftKey && selectedCell) {
			selectionStart = selectionStart ?? selectedCell;
			selectionEnd = ref;
		} else {
			selectedCell = ref;
			selectionStart = ref;
			selectionEnd = ref;
		}
		updateFormulaBar();
	}

	function startEdit(ref: string) {
		editingCell = ref;
		const cell = activeSheet?.cells[ref];
		editValue = getCellEditValue(cell);
	}

	async function commitEdit() {
		if (!editingCell || !activeSpreadsheet || !activeSheet) return;

		const ref = editingCell;
		const val = editValue.trim();
		editingCell = null;

		// Determine if it is a formula
		const isFormula = val.startsWith('=');

		// Update local state immediately for responsiveness
		if (!activeSheet.cells[ref]) {
			activeSheet.cells[ref] = {
				value: { type: 'Empty' },
				formula: null,
				format: { bold: false, italic: false, text_color: null, bg_color: null, number_format: null, align: 'left' },
				note: null
			};
		}

		if (isFormula) {
			activeSheet.cells[ref].formula = val;
		} else {
			activeSheet.cells[ref].formula = null;
		}

		// Parse value locally for instant feedback
		if (!isFormula) {
			if (val === '') {
				activeSheet.cells[ref].value = { type: 'Empty' };
			} else if (!isNaN(Number(val)) && val !== '') {
				activeSheet.cells[ref].value = { type: 'Number', value: Number(val) };
			} else if (val.toLowerCase() === 'true') {
				activeSheet.cells[ref].value = { type: 'Bool', value: true };
			} else if (val.toLowerCase() === 'false') {
				activeSheet.cells[ref].value = { type: 'Bool', value: false };
			} else {
				activeSheet.cells[ref].value = { type: 'Text', value: val };
			}
		}

		isDirty = true;
		saveIndicator = 'unsaved';

		// Evaluate formula on backend if needed
		if (isFormula) {
			try {
				// Build context from nearby cells
				const context: Record<string, string> = {};
				for (const [cRef, cell] of Object.entries(activeSheet.cells)) {
					if (cRef !== ref && cell.value.type !== 'Empty') {
						context[cRef] = getCellDisplay(cell);
					}
				}
				const result = await invoke<CellValue>('sheets_evaluate_formula', {
					formula: val,
					cellContext: context
				});
				if (activeSheet.cells[ref]) {
					activeSheet.cells[ref].value = result;
				}
			} catch {
				if (activeSheet.cells[ref]) {
					activeSheet.cells[ref].value = { type: 'Error', value: 'EVAL' };
				}
			}
		}

		updateFormulaBar();
	}

	function cancelEdit() {
		editingCell = null;
		editValue = '';
	}

	function updateFormulaBar() {
		if (!activeSheet || !selectedCell) {
			formulaBarValue = '';
			return;
		}
		const cell = activeSheet.cells[selectedCell];
		formulaBarValue = getCellEditValue(cell);
	}

	function handleFormulaBarChange() {
		if (!selectedCell) return;
		editingCell = selectedCell;
		editValue = formulaBarValue;
	}

	function handleFormulaBarKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			editValue = formulaBarValue;
			commitEdit();
		} else if (e.key === 'Escape') {
			cancelEdit();
			updateFormulaBar();
		}
	}

	// ---- Keyboard navigation -------------------------------------------------
	function handleGridKeydown(e: KeyboardEvent) {
		if (!selectedCell || !activeSheet) return;
		const pos = parseCellRef(selectedCell);
		if (!pos) return;

		if (editingCell) {
			if (e.key === 'Enter') {
				e.preventDefault();
				commitEdit();
				// Move down
				selectCell(makeCellRef(pos.col, Math.min(pos.row + 1, visibleRows - 1)));
			} else if (e.key === 'Tab') {
				e.preventDefault();
				commitEdit();
				// Move right
				selectCell(makeCellRef(Math.min(pos.col + 1, visibleCols - 1), pos.row));
			} else if (e.key === 'Escape') {
				cancelEdit();
			}
			return;
		}

		switch (e.key) {
			case 'ArrowUp':
				e.preventDefault();
				selectCell(makeCellRef(pos.col, Math.max(0, pos.row - 1)), e.shiftKey);
				break;
			case 'ArrowDown':
			case 'Enter':
				e.preventDefault();
				selectCell(makeCellRef(pos.col, Math.min(pos.row + 1, visibleRows - 1)), e.shiftKey);
				break;
			case 'ArrowLeft':
				e.preventDefault();
				selectCell(makeCellRef(Math.max(0, pos.col - 1), pos.row), e.shiftKey);
				break;
			case 'ArrowRight':
			case 'Tab':
				e.preventDefault();
				selectCell(makeCellRef(Math.min(pos.col + 1, visibleCols - 1), pos.row), e.shiftKey);
				break;
			case 'Delete':
			case 'Backspace':
				e.preventDefault();
				if (activeSheet.cells[selectedCell]) {
					activeSheet.cells[selectedCell].value = { type: 'Empty' };
					activeSheet.cells[selectedCell].formula = null;
					isDirty = true;
					saveIndicator = 'unsaved';
					updateFormulaBar();
				}
				break;
			case 'F2':
				e.preventDefault();
				startEdit(selectedCell);
				break;
			default:
				// Start typing to edit
				if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
					startEdit(selectedCell);
					editValue = e.key;
				}
				break;
		}
	}

	// ---- Formatting ----------------------------------------------------------
	function toggleBold() {
		if (!activeSheet || !selectedCell) return;
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet.cells[ref]) {
				activeSheet.cells[ref].format.bold = !activeSheet.cells[ref].format.bold;
			}
		}
		isDirty = true;
	}

	function toggleItalic() {
		if (!activeSheet || !selectedCell) return;
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet.cells[ref]) {
				activeSheet.cells[ref].format.italic = !activeSheet.cells[ref].format.italic;
			}
		}
		isDirty = true;
	}

	function setAlign(align: 'left' | 'center' | 'right') {
		if (!activeSheet || !selectedCell) return;
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet.cells[ref]) {
				activeSheet.cells[ref].format.align = align;
			}
		}
		isDirty = true;
	}

	// ---- Sheet tabs ----------------------------------------------------------
	function addSheet() {
		if (!activeSpreadsheet) return;
		const name = `Sheet ${activeSpreadsheet.sheets.length + 1}`;
		activeSpreadsheet.sheets.push({
			name,
			cells: {},
			col_widths: {},
			row_heights: {}
		});
		activeSheetIndex = activeSpreadsheet.sheets.length - 1;
		isDirty = true;
	}

	// ---- AI Panel ------------------------------------------------------------
	async function aiGenerateFormula() {
		if (!aiDescription.trim()) return;
		try {
			aiLoading = true;
			aiError = null;

			// Build context from selected cells
			const contextCells: Record<string, string> = {};
			if (activeSheet) {
				const cells = getSelectedCells();
				for (const ref of cells) {
					const cell = activeSheet.cells[ref];
					if (cell) contextCells[ref] = getCellDisplay(cell);
				}
			}

			const formula = await invoke<string>('sheets_ai_formula', {
				description: aiDescription,
				contextCells: Object.keys(contextCells).length > 0 ? contextCells : null
			});
			aiResult = formula;
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			aiLoading = false;
		}
	}

	function insertAiFormula() {
		if (!aiResult || !selectedCell || !activeSheet) return;
		startEdit(selectedCell);
		editValue = aiResult;
		commitEdit();
		aiResult = null;
	}

	async function aiAnalyzeRange() {
		if (!activeSpreadsheet || !activeSheet) return;
		const range = getSelectionRange();
		if (!range || !range.includes(':')) {
			aiError = 'Select a range of cells (drag or Shift+click) to analyze.';
			return;
		}
		try {
			aiLoading = true;
			aiError = null;
			aiAnalysis = await invoke<AnalysisResult>('sheets_ai_analyze', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				range
			});
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			aiLoading = false;
		}
	}

	// ---- Keyboard shortcuts (global) -----------------------------------------
	function handleGlobalKeydown(e: KeyboardEvent) {
		const mod = e.ctrlKey || e.metaKey;
		if (mod && e.key === 's') {
			e.preventDefault();
			saveSpreadsheet();
		}
		if (mod && e.key === 'b' && activeSheet && selectedCell && !e.shiftKey) {
			// Only intercept Ctrl+B if we have a spreadsheet open and focused
			if (document.activeElement?.closest('.sheets-grid')) {
				e.preventDefault();
				toggleBold();
			}
		}
		if (mod && e.key === 'i' && activeSheet && selectedCell) {
			if (document.activeElement?.closest('.sheets-grid')) {
				e.preventDefault();
				toggleItalic();
			}
		}
	}

	// ---- Mouse drag selection ------------------------------------------------
	function handleCellMouseDown(ref: string, e: MouseEvent) {
		if (e.button !== 0) return;
		if (e.detail === 2) {
			// Double-click: start editing
			startEdit(ref);
			return;
		}
		selectCell(ref, e.shiftKey);
		isDragging = true;
	}

	function handleCellMouseEnter(ref: string) {
		if (!isDragging) return;
		selectionEnd = ref;
	}

	function handleMouseUp() {
		isDragging = false;
	}

	// ---- Lifecycle -----------------------------------------------------------
	onMount(async () => {
		await loadSpreadsheets();

		// Auto-save every 30 seconds if dirty
		autoSaveTimer = setInterval(() => {
			if (isDirty && activeSpreadsheet) {
				saveSpreadsheet();
			}
		}, 30000);

		document.addEventListener('mouseup', handleMouseUp);
	});

	onDestroy(() => {
		if (autoSaveTimer) clearInterval(autoSaveTimer);
		document.removeEventListener('mouseup', handleMouseUp);
	});

	function formatDate(iso: string): string {
		try {
			return new Date(iso).toLocaleDateString(undefined, {
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit'
			});
		} catch {
			return iso;
		}
	}
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex h-full overflow-hidden {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- ===== File Sidebar (left) ===== -->
	{#if sidebarOpen}
		<div class="w-64 shrink-0 flex flex-col border-r border-gx-border-default bg-gx-bg-secondary">
			<!-- Sidebar header -->
			<div class="flex items-center gap-2 h-10 px-3 border-b border-gx-border-default shrink-0">
				<Table2 size={15} class="text-gx-neon" />
				<span class="text-sm font-semibold text-gx-text-secondary">ForgeSheets</span>
				<div class="flex-1"></div>
				<button onclick={() => sidebarOpen = false} class="text-gx-text-muted hover:text-gx-neon transition-colors">
					<PanelLeftClose size={14} />
				</button>
			</div>

			<!-- Actions -->
			<div class="flex items-center gap-1 px-2 py-2 border-b border-gx-border-default">
				<button
					onclick={() => showNewDialog = true}
					class="flex items-center gap-1.5 px-2 py-1 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors"
				>
					<FilePlus size={12} />
					New
				</button>
				<button
					onclick={triggerImport}
					class="flex items-center gap-1.5 px-2 py-1 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				>
					<Upload size={12} />
					Import
				</button>
			</div>

			<!-- Search -->
			<div class="px-2 py-1.5 border-b border-gx-border-default">
				<div class="flex items-center gap-1.5 px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
					<Search size={12} class="text-gx-text-muted shrink-0" />
					<input
						type="text"
						placeholder="Search spreadsheets..."
						bind:value={searchQuery}
						class="w-full bg-transparent text-xs text-gx-text-primary placeholder:text-gx-text-muted outline-none"
					/>
				</div>
			</div>

			<!-- Spreadsheet list -->
			<div class="flex-1 overflow-y-auto">
				{#if loading && spreadsheets.length === 0}
					<div class="flex items-center justify-center p-4">
						<Loader2 size={16} class="animate-spin text-gx-text-muted" />
					</div>
				{:else if filteredSpreadsheets.length === 0}
					<div class="p-4 text-center text-xs text-gx-text-muted">
						{searchQuery ? 'No matching spreadsheets' : 'No spreadsheets yet. Create one or import a file.'}
					</div>
				{:else}
					{#each filteredSpreadsheets as ss (ss.id)}
						<button
							onclick={() => openSpreadsheet(ss.id)}
							class="w-full flex items-start gap-2 px-3 py-2 text-left hover:bg-gx-bg-hover transition-colors group
								{activeSpreadsheet?.id === ss.id ? 'bg-gx-bg-elevated border-l-2 border-gx-neon' : 'border-l-2 border-transparent'}"
						>
							<FileSpreadsheet size={14} class="text-gx-neon shrink-0 mt-0.5" />
							<div class="flex-1 min-w-0">
								<div class="text-xs font-medium text-gx-text-secondary truncate">{ss.name}</div>
								<div class="text-[10px] text-gx-text-muted">
									{ss.sheet_count} sheet{ss.sheet_count !== 1 ? 's' : ''} &middot; {ss.cell_count} cells &middot; {formatDate(ss.updated_at)}
								</div>
							</div>
							<button
								onclick|stopPropagation={() => deleteSpreadsheet(ss.id)}
								class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all shrink-0"
								title="Delete"
							>
								<Trash2 size={12} />
							</button>
						</button>
					{/each}
				{/if}
			</div>
		</div>
	{/if}

	<!-- ===== Main Area ===== -->
	<div class="flex-1 flex flex-col min-w-0 overflow-hidden">
		{#if !activeSpreadsheet}
			<!-- Empty state -->
			<div class="flex-1 flex flex-col items-center justify-center gap-4 text-center p-8">
				{#if !sidebarOpen}
					<button onclick={() => sidebarOpen = true} class="absolute top-2 left-2 text-gx-text-muted hover:text-gx-neon transition-colors">
						<PanelLeftOpen size={16} />
					</button>
				{/if}
				<Table2 size={48} class="text-gx-neon/40" />
				<div>
					<h2 class="text-lg font-semibold text-gx-text-secondary">ForgeSheets</h2>
					<p class="text-sm text-gx-text-muted mt-1">KI-native spreadsheet engine. Create, import, or open a spreadsheet.</p>
				</div>
				<div class="flex gap-3">
					<button
						onclick={() => showNewDialog = true}
						class="flex items-center gap-2 px-4 py-2 rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors text-sm"
					>
						<FilePlus size={16} />
						Create New
					</button>
					<button
						onclick={triggerImport}
						class="flex items-center gap-2 px-4 py-2 rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors text-sm"
					>
						<Upload size={16} />
						Import File
					</button>
				</div>
			</div>
		{:else}
			<!-- Toolbar -->
			<div class="flex items-center gap-1 h-9 px-2 border-b border-gx-border-default bg-gx-bg-secondary shrink-0 overflow-x-auto">
				{#if !sidebarOpen}
					<button onclick={() => sidebarOpen = true} class="text-gx-text-muted hover:text-gx-neon transition-colors mr-1">
						<PanelLeftOpen size={14} />
					</button>
					<Separator orientation="vertical" class="h-5 bg-gx-border-default" />
				{/if}

				<!-- Save -->
				<button onclick={saveSpreadsheet} disabled={saving} title="Save (Ctrl+S)"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-neon transition-colors disabled:opacity-50">
					{#if saving}
						<Loader2 size={14} class="animate-spin" />
					{:else}
						<Save size={14} />
					{/if}
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-0.5" />

				<!-- Formatting -->
				<button onclick={toggleBold} title="Bold (Ctrl+B)"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors">
					<Bold size={14} />
				</button>
				<button onclick={toggleItalic} title="Italic (Ctrl+I)"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors">
					<Italic size={14} />
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-0.5" />

				<!-- Alignment -->
				<button onclick={() => setAlign('left')} title="Align Left"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors">
					<AlignLeft size={14} />
				</button>
				<button onclick={() => setAlign('center')} title="Align Center"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors">
					<AlignCenter size={14} />
				</button>
				<button onclick={() => setAlign('right')} title="Align Right"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors">
					<AlignRight size={14} />
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-0.5" />

				<!-- Export -->
				<button onclick={() => exportSpreadsheet('xlsx')} title="Export as .xlsx"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors text-[11px]">
					<Download size={13} />
					<span>.xlsx</span>
				</button>
				<button onclick={() => exportSpreadsheet('csv')} title="Export as .csv"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors text-[11px]">
					<Download size={13} />
					<span>.csv</span>
				</button>

				<div class="flex-1"></div>

				<!-- Save indicator -->
				{#if saveIndicator === 'saving'}
					<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-warning/30 text-gx-status-warning">Saving...</Badge>
				{:else if saveIndicator === 'saved'}
					<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-success/30 text-gx-status-success">Saved</Badge>
				{:else if saveIndicator === 'unsaved'}
					<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-status-warning/30 text-gx-status-warning">Unsaved</Badge>
				{/if}

				<!-- AI Panel toggle -->
				<button
					onclick={() => aiPanelOpen = !aiPanelOpen}
					class="flex items-center gap-1 px-2 py-1 rounded-gx text-[11px] transition-colors
						{aiPanelOpen ? 'bg-gx-accent-purple/15 text-gx-accent-purple' : 'text-gx-text-muted hover:text-gx-accent-purple hover:bg-gx-accent-purple/10'}"
				>
					<Sparkles size={13} />
					<span>AI</span>
				</button>
			</div>

			<!-- Formula bar -->
			<div class="flex items-center h-7 px-2 border-b border-gx-border-default bg-gx-bg-secondary shrink-0 gap-2">
				<span class="text-[11px] font-mono text-gx-neon w-12 text-center shrink-0">{selectedCell ?? ''}</span>
				<Separator orientation="vertical" class="h-4 bg-gx-border-default" />
				<span class="text-[11px] text-gx-text-muted italic shrink-0">fx</span>
				<input
					type="text"
					bind:value={formulaBarValue}
					oninput={handleFormulaBarChange}
					onkeydown={handleFormulaBarKeydown}
					placeholder={selectedCell ? 'Enter value or formula (=SUM, =IF, ...)' : ''}
					class="flex-1 bg-transparent text-xs font-mono text-gx-text-primary placeholder:text-gx-text-muted outline-none"
				/>
			</div>

			<!-- Content area (grid + optional AI panel) -->
			<div class="flex-1 flex overflow-hidden">
				<!-- Grid -->
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<div
					class="flex-1 overflow-auto sheets-grid focus:outline-none"
					tabindex="0"
					onkeydown={handleGridKeydown}
				>
					<table class="border-collapse text-xs select-none" style="table-layout: fixed;">
						<thead class="sticky top-0 z-10">
							<tr>
								<!-- Row number header (top-left corner) -->
								<th class="w-10 h-6 bg-gx-bg-tertiary border border-gx-border-default sticky left-0 z-20"></th>
								<!-- Column headers A, B, C, ... -->
								{#each Array(visibleCols) as _, ci}
									<th
										class="h-6 min-w-[80px] bg-gx-bg-tertiary border border-gx-border-default text-[10px] font-medium text-gx-text-muted text-center px-1 select-none"
										style="width: {activeSheet?.col_widths?.[String(ci)] ?? 80}px"
									>
										{colToLetter(ci)}
									</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#each Array(visibleRows) as _, ri}
								<tr>
									<!-- Row number -->
									<td class="w-10 h-6 bg-gx-bg-tertiary border border-gx-border-default text-[10px] text-gx-text-muted text-center font-mono sticky left-0 z-[5] select-none">
										{ri + 1}
									</td>
									<!-- Cells -->
									{#each Array(visibleCols) as _, ci}
										{@const ref = makeCellRef(ci, ri)}
										{@const cell = activeSheet?.cells[ref]}
										{@const isSelected = isCellSelected(ref)}
										{@const isEditing = editingCell === ref}
										<td
											class="h-6 min-w-[80px] border border-gx-border-default relative cursor-cell transition-colors
												{isSelected ? 'bg-gx-neon/10 outline outline-1 outline-gx-neon z-[2]' : 'hover:bg-gx-bg-hover'}
												{cell?.value?.type === 'Error' ? 'text-gx-status-error' : 'text-gx-text-primary'}"
											style="{cell?.format?.bg_color ? `background-color: ${cell.format.bg_color};` : ''}
												{cell?.format?.text_color ? `color: ${cell.format.text_color};` : ''}
												{cell?.format?.bold ? 'font-weight: 700;' : ''}
												{cell?.format?.italic ? 'font-style: italic;' : ''}
												text-align: {cell?.format?.align ?? 'left'};"
											onmousedown={(e) => handleCellMouseDown(ref, e)}
											onmouseenter={() => handleCellMouseEnter(ref)}
											role="gridcell"
											aria-selected={isSelected}
										>
											{#if isEditing}
												<input
													type="text"
													bind:value={editValue}
													class="absolute inset-0 w-full h-full px-1 bg-gx-bg-primary text-xs font-mono text-gx-text-primary outline-none border-2 border-gx-neon z-10"
													autofocus
												/>
											{:else}
												<span class="block px-1 truncate text-xs leading-6">
													{getCellDisplay(cell)}
												</span>
											{/if}
										</td>
									{/each}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>

				<!-- AI Panel (right) -->
				{#if aiPanelOpen}
					<div class="w-72 shrink-0 border-l border-gx-border-default bg-gx-bg-secondary flex flex-col overflow-hidden">
						<div class="flex items-center gap-2 h-9 px-3 border-b border-gx-border-default shrink-0">
							<Sparkles size={14} class="text-gx-accent-purple" />
							<span class="text-xs font-medium text-gx-text-secondary">AI Assistant</span>
							<div class="flex-1"></div>
							<button onclick={() => aiPanelOpen = false} class="text-gx-text-muted hover:text-gx-neon transition-colors">
								<PanelRightClose size={14} />
							</button>
						</div>

						<div class="flex-1 overflow-y-auto p-3 space-y-3">
							<!-- Formula Generator -->
							<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
								<div class="flex items-center gap-2 mb-2">
									<Brain size={13} class="text-gx-accent-purple" />
									<span class="text-xs font-medium text-gx-text-secondary">Formula from Description</span>
								</div>
								<textarea
									bind:value={aiDescription}
									placeholder="e.g. Calculate total revenue minus costs for each row..."
									class="w-full h-16 p-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary placeholder:text-gx-text-muted resize-none outline-none focus:border-gx-accent-purple transition-colors"
								></textarea>
								<button
									onclick={aiGenerateFormula}
									disabled={aiLoading || !aiDescription.trim()}
									class="w-full mt-2 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-gx text-xs bg-gx-accent-purple/15 text-gx-accent-purple hover:bg-gx-accent-purple/25 transition-colors disabled:opacity-50"
								>
									{#if aiLoading}
										<Loader2 size={12} class="animate-spin" />
										Generating...
									{:else}
										<Sparkles size={12} />
										Generate Formula
									{/if}
								</button>

								{#if aiResult && !aiResult.startsWith('Exported')}
									<div class="mt-2 p-2 rounded-gx bg-gx-bg-tertiary border border-gx-accent-purple/30">
										<code class="text-xs text-gx-neon font-mono break-all">{aiResult}</code>
										<button
											onclick={insertAiFormula}
											class="w-full mt-1.5 flex items-center justify-center gap-1 px-2 py-1 rounded text-[10px] bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors"
										>
											<ArrowDown size={10} />
											Insert into {selectedCell ?? 'cell'}
										</button>
									</div>
								{/if}
							</div>

							<!-- Auto-EDA Analyzer -->
							<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
								<div class="flex items-center gap-2 mb-2">
									<TrendingUp size={13} class="text-gx-accent-blue" />
									<span class="text-xs font-medium text-gx-text-secondary">Auto-EDA</span>
								</div>
								<p class="text-[10px] text-gx-text-muted mb-2">
									Select a range of cells, then click Analyze to detect trends, outliers, and correlations.
								</p>
								{#if getSelectionRange()?.includes(':')}
									<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-neon/30 text-gx-neon mb-2">
										{getSelectionRange()}
									</Badge>
								{/if}
								<button
									onclick={aiAnalyzeRange}
									disabled={aiLoading}
									class="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-gx text-xs bg-gx-accent-blue/15 text-gx-accent-blue hover:bg-gx-accent-blue/25 transition-colors disabled:opacity-50"
								>
									{#if aiLoading}
										<Loader2 size={12} class="animate-spin" />
										Analyzing...
									{:else}
										<BarChart3 size={12} />
										Analyze Selection
									{/if}
								</button>

								{#if aiAnalysis}
									<div class="mt-2 space-y-2">
										<p class="text-[10px] text-gx-text-secondary">{aiAnalysis.summary}</p>

										{#if aiAnalysis.stats.count > 0}
											<div class="grid grid-cols-2 gap-1 text-[10px]">
												<div class="p-1 rounded bg-gx-bg-tertiary">
													<span class="text-gx-text-muted">Count:</span>
													<span class="text-gx-text-primary ml-1">{aiAnalysis.stats.count}</span>
												</div>
												<div class="p-1 rounded bg-gx-bg-tertiary">
													<span class="text-gx-text-muted">Sum:</span>
													<span class="text-gx-text-primary ml-1">{aiAnalysis.stats.sum}</span>
												</div>
												<div class="p-1 rounded bg-gx-bg-tertiary">
													<span class="text-gx-text-muted">Avg:</span>
													<span class="text-gx-text-primary ml-1">{aiAnalysis.stats.average}</span>
												</div>
												<div class="p-1 rounded bg-gx-bg-tertiary">
													<span class="text-gx-text-muted">Std:</span>
													<span class="text-gx-text-primary ml-1">{aiAnalysis.stats.std_dev}</span>
												</div>
											</div>
										{/if}

										{#if aiAnalysis.trends.length > 0}
											<div>
												<span class="text-[10px] font-medium text-gx-accent-blue">Trends</span>
												{#each aiAnalysis.trends as trend}
													<p class="text-[10px] text-gx-text-muted mt-0.5">{trend}</p>
												{/each}
											</div>
										{/if}

										{#if aiAnalysis.outliers.length > 0}
											<div>
												<span class="text-[10px] font-medium text-gx-status-warning">Outliers ({aiAnalysis.outliers.length})</span>
												{#each aiAnalysis.outliers.slice(0, 5) as outlier}
													<p class="text-[10px] text-gx-text-muted mt-0.5">
														<span class="font-mono text-gx-neon">{outlier.cell_ref}</span>: {outlier.value} ({outlier.reason})
													</p>
												{/each}
											</div>
										{/if}

										{#if aiAnalysis.correlations.length > 0}
											<div>
												<span class="text-[10px] font-medium text-gx-accent-magenta">Correlations</span>
												{#each aiAnalysis.correlations as corr}
													<p class="text-[10px] text-gx-text-muted mt-0.5">
														{corr.columns[0]} vs {corr.columns[1]}: r={corr.coefficient} ({corr.description})
													</p>
												{/each}
											</div>
										{/if}

										{#if aiAnalysis.suggested_charts.length > 0}
											<div>
												<span class="text-[10px] font-medium text-gx-status-success">Chart Suggestions</span>
												{#each aiAnalysis.suggested_charts as chart}
													<p class="text-[10px] text-gx-text-muted mt-0.5">
														<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5 mr-1">{chart.chart_type}</Badge>
														{chart.reason}
													</p>
												{/each}
											</div>
										{/if}
									</div>
								{/if}
							</div>

							{#if aiError}
								<div class="rounded-gx border border-gx-status-error/30 bg-gx-status-error/5 p-2 flex items-start gap-2">
									<AlertCircle size={12} class="text-gx-status-error shrink-0 mt-0.5" />
									<p class="text-[10px] text-gx-status-error">{aiError}</p>
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>

			<!-- Sheet tabs + status bar -->
			<div class="flex items-center h-6 px-2 border-t border-gx-border-default bg-gx-bg-secondary shrink-0 gap-1">
				<!-- Sheet tabs -->
				{#if activeSpreadsheet}
					{#each activeSpreadsheet.sheets as sheet, i}
						<button
							onclick={() => activeSheetIndex = i}
							class="px-2 py-0.5 text-[10px] rounded-t border border-b-0 transition-colors
								{activeSheetIndex === i
									? 'bg-gx-bg-primary text-gx-neon border-gx-border-default font-medium'
									: 'bg-gx-bg-tertiary text-gx-text-muted border-transparent hover:text-gx-text-secondary'}"
						>
							{sheet.name}
						</button>
					{/each}
					<button
						onclick={addSheet}
						class="p-0.5 text-gx-text-muted hover:text-gx-neon transition-colors"
						title="Add sheet"
					>
						<Plus size={12} />
					</button>
				{/if}

				<div class="flex-1"></div>

				<!-- Selection stats -->
				{#if selectionStats}
					<span class="text-[10px] text-gx-text-muted">
						SUM: <span class="text-gx-text-secondary">{selectionStats.sum}</span>
					</span>
					<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
					<span class="text-[10px] text-gx-text-muted">
						AVG: <span class="text-gx-text-secondary">{selectionStats.average}</span>
					</span>
					<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
					<span class="text-[10px] text-gx-text-muted">
						COUNT: <span class="text-gx-text-secondary">{selectionStats.count}</span>
					</span>
				{:else if selectedCell}
					<span class="text-[10px] text-gx-text-muted">{selectedCell}</span>
				{/if}
			</div>
		{/if}
	</div>
</div>

<!-- New spreadsheet dialog -->
{#if showNewDialog}
	<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={() => showNewDialog = false} role="dialog" aria-modal="true">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-80 shadow-gx-glow-lg" onclick|stopPropagation>
			<h3 class="text-sm font-semibold text-gx-text-secondary mb-3">New Spreadsheet</h3>
			<input
				type="text"
				bind:value={newSheetName}
				placeholder="Spreadsheet name..."
				class="w-full px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-sm text-gx-text-primary placeholder:text-gx-text-muted outline-none focus:border-gx-neon transition-colors"
				onkeydown={(e) => { if (e.key === 'Enter') createSpreadsheet(); }}
				autofocus
			/>
			<div class="flex justify-end gap-2 mt-3">
				<button
					onclick={() => showNewDialog = false}
					class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={createSpreadsheet}
					class="px-3 py-1.5 text-xs rounded-gx bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20 transition-colors"
				>
					Create
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Hidden file input for import -->
<input
	type="file"
	accept=".xlsx,.xls,.ods,.csv,.tsv,.json,.xlsb"
	class="hidden"
	bind:this={fileInputEl}
	onchange={handleFileSelected}
/>

<!-- Error notification -->
{#if error}
	<div class="fixed bottom-12 right-4 z-50 max-w-sm">
		<div class="bg-gx-bg-elevated border border-gx-status-error/30 rounded-gx p-3 shadow-gx-glow-lg flex items-start gap-2">
			<AlertCircle size={14} class="text-gx-status-error shrink-0 mt-0.5" />
			<div class="flex-1 min-w-0">
				<p class="text-xs text-gx-status-error">{error}</p>
			</div>
			<button onclick={() => error = null} class="text-gx-text-muted hover:text-gx-text-primary transition-colors shrink-0">
				<X size={12} />
			</button>
		</div>
	</div>
{/if}
