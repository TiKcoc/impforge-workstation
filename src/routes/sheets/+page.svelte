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
		Copy, Scissors, Clipboard, Undo2, Redo2, ArrowDown, ArrowRight,
		Zap, RefreshCw, PieChart, LayoutGrid, Palette, ListChecks,
		MessageSquare, DollarSign, Globe, Bot
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
		type: 'Empty' | 'Text' | 'Number' | 'Bool' | 'Error' | 'Sparkline';
		value?: string | number | boolean | { data: number[]; chart_type: string };
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

	interface MergedRegion {
		startCol: number;
		startRow: number;
		endCol: number;
		endRow: number;
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

	interface AgenticCell {
		cell_ref: string;
		agent_type: { type: string; url?: string; endpoint?: string; method?: string; prompt?: string; path?: string; expression?: string };
		config: Record<string, unknown>;
		refresh_interval: number | null;
		last_fetched: string | null;
	}

	interface ChartSeries {
		name: string;
		values: number[];
	}

	interface ChartConfig {
		chart_type: string;
		title: string;
		data_range: string;
		labels_range: string | null;
		colors: string[];
		series: ChartSeries[];
		categories: string[];
	}

	interface ConditionalFormatRule {
		id: string;
		range: string;
		condition: Record<string, unknown>;
		bg_color: string | null;
		text_color: string | null;
		bold: boolean | null;
		italic: boolean | null;
	}

	interface PivotResult {
		headers: string[];
		rows: { label: string; values: (number | null)[] }[];
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

	// Virtual scrolling state
	const CELL_HEIGHT = 28;
	const DEFAULT_COL_WIDTH = 100;
	const ROW_HEADER_WIDTH = 48;
	const MAX_VIRTUAL_ROWS = 10000;
	const MAX_VIRTUAL_COLS = 702;
	let viewportTop = $state(0);
	let viewportLeft = $state(0);
	let viewportWidth = $state(0);
	let viewportHeight = $state(0);
	let gridContainerEl: HTMLDivElement | undefined = $state();

	// Column widths map (col index -> px width)
	let columnWidths = $state<Record<number, number>>({});
	let resizingCol = $state<number | null>(null);
	let resizeStartX = $state(0);
	let resizeStartWidth = $state(0);

	// Freeze panes
	let freezeFirstRow = $state(true);
	let freezeFirstCol = $state(true);

	// Merged cells
	let mergedRegions = $state<MergedRegion[]>([]);

	// Context menu
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuRef = $state<string | null>(null);

	// Number format dropdown
	let showFormatDropdown = $state(false);
	let customFormatInput = $state('');

	// Grid total dimensions (how many rows/cols exist with data)
	let totalRows = $state(100);
	let totalCols = $state(26);

	// Legacy compat
	let visibleCols = $derived(totalCols);
	let visibleRows = $derived(totalRows);

	// UI panel state
	let sidebarOpen = $state(true);
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state<string | null>(null);
	let aiAnalysis = $state<AnalysisResult | null>(null);
	let aiDescription = $state('');
	let aiError = $state<string | null>(null);

	// Chart state
	let chartConfig = $state<ChartConfig | null>(null);
	let chartLoading = $state(false);

	// Pivot table state
	let showPivotDialog = $state(false);
	let pivotRowField = $state(0);
	let pivotColField = $state(1);
	let pivotValueField = $state(2);
	let pivotAggregation = $state<string>('sum');
	let pivotResult = $state<PivotResult | null>(null);
	let pivotLoading = $state(false);

	// Conditional formatting state
	let showConditionalDialog = $state(false);
	let condType = $state<string>('greater_than');
	let condValue = $state('');
	let condBgColor = $state('#ff4444');
	let condTextColor = $state('');

	// Agentic cells state
	let showAgentDialog = $state(false);
	let agentType = $state<string>('web_fetch');
	let agentUrl = $state('');
	let agentPrompt = $state('');
	let agentRefreshInterval = $state(0);
	let agenticCells = $state<AgenticCell[]>([]);

	// Conditional format highlights (cell_ref -> style overrides)
	let conditionalHighlights = $state<Record<string, { bg_color?: string; text_color?: string; bold?: boolean; italic?: boolean }>>({});

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

	// --- Virtual scrolling computations ---
	function getColWidth(col: number): number {
		if (columnWidths[col] != null) return columnWidths[col];
		const sheetW = activeSheet?.col_widths?.[String(col)];
		if (sheetW != null) return sheetW;
		return DEFAULT_COL_WIDTH;
	}

	// Column left-edge positions (cumulative widths) for visible range calculation
	function getColLeft(col: number): number {
		let x = 0;
		for (let c = 0; c < col; c++) {
			x += getColWidth(c);
		}
		return x;
	}

	let vStartRow = $derived(Math.max(0, Math.floor(viewportTop / CELL_HEIGHT)));
	let vEndRow = $derived(Math.min(vStartRow + Math.ceil(viewportHeight / CELL_HEIGHT) + 3, totalRows));
	let vVisibleRows = $derived(Array.from({ length: vEndRow - vStartRow }, (_, i) => vStartRow + i));

	// For columns, we compute start/end based on cumulative widths
	let vStartCol = $derived.by(() => {
		let x = 0;
		for (let c = 0; c < totalCols; c++) {
			if (x + getColWidth(c) > viewportLeft) return c;
			x += getColWidth(c);
		}
		return 0;
	});
	let vEndCol = $derived.by(() => {
		let x = 0;
		for (let c = 0; c < totalCols; c++) {
			x += getColWidth(c);
			if (x > viewportLeft + viewportWidth + DEFAULT_COL_WIDTH) return Math.min(c + 1, totalCols);
		}
		return totalCols;
	});
	let vVisibleCols = $derived(Array.from({ length: vEndCol - vStartCol }, (_, i) => vStartCol + i));

	// Total content size for scrollbar
	let totalContentWidth = $derived.by(() => {
		let w = 0;
		for (let c = 0; c < totalCols; c++) w += getColWidth(c);
		return w;
	});
	let totalContentHeight = $derived(totalRows * CELL_HEIGHT);

	// Auto-expand grid based on data
	$effect(() => {
		if (!activeSheet) return;
		let maxR = 100;
		let maxC = 26;
		for (const ref of Object.keys(activeSheet.cells)) {
			const parsed = parseCellRef(ref);
			if (parsed) {
				if (parsed.row + 2 > maxR) maxR = parsed.row + 2;
				if (parsed.col + 2 > maxC) maxC = parsed.col + 2;
			}
		}
		// Always show at least some buffer
		totalRows = Math.max(maxR, 100);
		totalCols = Math.max(maxC, 26);
	});

	// Check if a cell is hidden by a merge
	function isCellHiddenByMerge(col: number, row: number): boolean {
		for (const m of mergedRegions) {
			if (col >= m.startCol && col <= m.endCol && row >= m.startRow && row <= m.endRow) {
				if (col !== m.startCol || row !== m.startRow) return true;
			}
		}
		return false;
	}

	function getMergeForCell(col: number, row: number): MergedRegion | null {
		for (const m of mergedRegions) {
			if (col === m.startCol && row === m.startRow) return m;
		}
		return null;
	}

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
			case 'Number': {
				const num = cell.value.value as number;
				if (num == null) return '';
				const fmt = cell.format?.number_format;
				if (fmt === '0%') return `${round(num * 100, 2)}%`;
				if (fmt === '#,##0.00') return num.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
				if (fmt === '$#,##0.00') return `$${num.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
				if (fmt === '\u20AC#,##0.00') return `\u20AC${num.toLocaleString('de-DE', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
				if (fmt === 'yyyy-mm-dd') return String(num);
				if (fmt === '0.00E+00') return num.toExponential(2).toUpperCase();
				if (fmt && fmt.startsWith('custom:')) {
					// Custom format -- just show with precision
					try {
						const prec = parseInt(fmt.split(':')[1]) || 2;
						return num.toFixed(prec);
					} catch { return String(num); }
				}
				return String(num);
			}
			case 'Text': return cell.value.value as string ?? '';
			case 'Bool': return cell.value.value ? 'TRUE' : 'FALSE';
			case 'Error': return `#ERR: ${cell.value.value ?? ''}`;
			case 'Sparkline': return ''; // rendered as SVG, not text
			default: return '';
		}
	}

	function isSparklineCell(cell: Cell | undefined): boolean {
		return cell?.value?.type === 'Sparkline';
	}

	function getSparklineData(cell: Cell | undefined): { data: number[]; chart_type: string } | null {
		if (!cell || cell.value.type !== 'Sparkline') return null;
		const val = cell.value.value as { data: number[]; chart_type: string } | undefined;
		if (val && Array.isArray(val.data)) return val;
		return null;
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
			chartConfig = null;
			pivotResult = null;
			updateFormulaBar();
			await loadAgenticCells();
			await refreshConditionalHighlights();
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
		// Close context menu on any key
		if (contextMenuOpen) { closeContextMenu(); }
		if (!selectedCell || !activeSheet) return;
		const pos = parseCellRef(selectedCell);
		if (!pos) return;

		if (editingCell) {
			if (e.key === 'Enter') {
				e.preventDefault();
				commitEdit();
				// Move down
				selectCell(makeCellRef(pos.col, Math.min(pos.row + 1, totalRows - 1)));
			} else if (e.key === 'Tab') {
				e.preventDefault();
				commitEdit();
				// Move right
				selectCell(makeCellRef(Math.min(pos.col + 1, totalCols - 1), pos.row));
			} else if (e.key === 'Escape') {
				cancelEdit();
			}
			return;
		}

		switch (e.key) {
			case 'ArrowUp':
				e.preventDefault();
				selectCell(makeCellRef(pos.col, Math.max(0, pos.row - 1)), e.shiftKey);
				scrollCellIntoView(pos.col, Math.max(0, pos.row - 1));
				break;
			case 'ArrowDown':
			case 'Enter':
				e.preventDefault();
				selectCell(makeCellRef(pos.col, Math.min(pos.row + 1, totalRows - 1)), e.shiftKey);
				scrollCellIntoView(pos.col, Math.min(pos.row + 1, totalRows - 1));
				break;
			case 'ArrowLeft':
				e.preventDefault();
				selectCell(makeCellRef(Math.max(0, pos.col - 1), pos.row), e.shiftKey);
				scrollCellIntoView(Math.max(0, pos.col - 1), pos.row);
				break;
			case 'ArrowRight':
			case 'Tab':
				e.preventDefault();
				selectCell(makeCellRef(Math.min(pos.col + 1, totalCols - 1), pos.row), e.shiftKey);
				scrollCellIntoView(Math.min(pos.col + 1, totalCols - 1), pos.row);
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
			case 'Escape':
				if (contextMenuOpen) closeContextMenu();
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

	function scrollCellIntoView(col: number, row: number) {
		if (!gridContainerEl) return;
		const cellTop = row * CELL_HEIGHT;
		const cellLeft = getColLeft(col);
		const cellW = getColWidth(col);
		const frozenRowH = freezeFirstRow ? CELL_HEIGHT : 0;
		const frozenColW = freezeFirstCol ? ROW_HEADER_WIDTH + getColWidth(0) : ROW_HEADER_WIDTH;

		if (cellTop < viewportTop + frozenRowH) {
			gridContainerEl.scrollTop = Math.max(0, cellTop - frozenRowH);
		} else if (cellTop + CELL_HEIGHT > viewportTop + viewportHeight) {
			gridContainerEl.scrollTop = cellTop + CELL_HEIGHT - viewportHeight;
		}
		if (cellLeft < viewportLeft + frozenColW) {
			gridContainerEl.scrollLeft = Math.max(0, cellLeft - frozenColW);
		} else if (cellLeft + cellW > viewportLeft + viewportWidth) {
			gridContainerEl.scrollLeft = cellLeft + cellW - viewportWidth;
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

	// ---- Chart generation ----------------------------------------------------
	async function generateChart(chartType?: string) {
		if (!activeSpreadsheet || !activeSheet) return;
		const range = getSelectionRange();
		if (!range || !range.includes(':')) {
			aiError = 'Select a range of cells to generate a chart.';
			return;
		}
		try {
			chartLoading = true;
			aiError = null;
			chartConfig = await invoke<ChartConfig>('sheets_ai_chart', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				range,
				chartType: chartType ? { type: chartType } : null,
				title: null
			});
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			chartLoading = false;
		}
	}

	// ---- Pivot table ---------------------------------------------------------
	async function generatePivot() {
		if (!activeSpreadsheet || !activeSheet) return;
		const range = getSelectionRange();
		if (!range || !range.includes(':')) {
			aiError = 'Select a data range for the pivot table.';
			return;
		}
		try {
			pivotLoading = true;
			aiError = null;
			pivotResult = await invoke<PivotResult>('sheets_pivot', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				dataRange: range,
				rowField: pivotRowField,
				colField: pivotColField,
				valueField: pivotValueField,
				aggregation: pivotAggregation
			});
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		} finally {
			pivotLoading = false;
			showPivotDialog = false;
		}
	}

	// ---- Conditional formatting ----------------------------------------------
	async function addConditionalFormat() {
		if (!activeSpreadsheet || !activeSheet) return;
		const range = getSelectionRange();
		if (!range || !range.includes(':')) {
			aiError = 'Select a range for conditional formatting.';
			return;
		}
		let condition: Record<string, unknown> = { type: condType };
		const numVal = parseFloat(condValue);
		if (condType === 'greater_than') condition = { type: 'greater_than', value: numVal };
		else if (condType === 'less_than') condition = { type: 'less_than', value: numVal };
		else if (condType === 'equal_to') condition = { type: 'equal_to', value: numVal };
		else if (condType === 'between') {
			const parts = condValue.split(',').map(s => parseFloat(s.trim()));
			condition = { type: 'between', min: parts[0] || 0, max: parts[1] || 0 };
		} else if (condType === 'text_contains') condition = { type: 'text_contains', text: condValue };
		else if (condType === 'is_empty') condition = { type: 'is_empty' };
		else if (condType === 'is_not_empty') condition = { type: 'is_not_empty' };

		try {
			await invoke('sheets_add_conditional_format', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				range,
				condition,
				bgColor: condBgColor || null,
				textColor: condTextColor || null,
				bold: null,
				italic: null
			});
			showConditionalDialog = false;
			await refreshConditionalHighlights();
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	async function refreshConditionalHighlights() {
		if (!activeSpreadsheet) return;
		try {
			conditionalHighlights = await invoke<Record<string, { bg_color?: string; text_color?: string; bold?: boolean; italic?: boolean }>>('sheets_eval_conditional_formats', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex
			});
		} catch {
			conditionalHighlights = {};
		}
	}

	// ---- Agentic cells -------------------------------------------------------
	async function addAgenticCell() {
		if (!activeSpreadsheet || !activeSheet || !selectedCell) return;
		let agent: Record<string, unknown> = { type: agentType };
		if (agentType === 'web_fetch') agent = { type: 'web_fetch', url: agentUrl };
		else if (agentType === 'api_call') agent = { type: 'api_call', endpoint: agentUrl, method: 'GET', headers: [] };
		else if (agentType === 'ai_generate') agent = { type: 'ai_generate', prompt: agentPrompt };
		else if (agentType === 'file_watch') agent = { type: 'file_watch', path: agentUrl };
		else if (agentType === 'formula') agent = { type: 'formula', expression: agentPrompt };

		try {
			await invoke('sheets_add_agentic_cell', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				cellRef: selectedCell,
				agentType: agent,
				config: {},
				refreshInterval: agentRefreshInterval > 0 ? agentRefreshInterval : null
			});
			showAgentDialog = false;
			await loadAgenticCells();
			await refreshAgenticCell(selectedCell);
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	async function refreshAgenticCell(cellRef: string) {
		if (!activeSpreadsheet) return;
		try {
			const value = await invoke<CellValue>('sheets_refresh_agentic', {
				id: activeSpreadsheet.id,
				sheetIndex: activeSheetIndex,
				cellRef
			});
			if (activeSheet) {
				if (!activeSheet.cells[cellRef]) {
					activeSheet.cells[cellRef] = {
						value: { type: 'Empty' },
						formula: null,
						format: { bold: false, italic: false, text_color: null, bg_color: null, number_format: null, align: 'left' },
						note: null
					};
				}
				activeSheet.cells[cellRef].value = value;
			}
		} catch (e: any) {
			aiError = typeof e === 'string' ? (JSON.parse(e)?.message ?? e) : String(e);
		}
	}

	async function loadAgenticCells() {
		if (!activeSpreadsheet) return;
		try {
			agenticCells = await invoke<AgenticCell[]>('sheets_list_agentic', { id: activeSpreadsheet.id });
		} catch {
			agenticCells = [];
		}
	}

	function isAgenticCell(ref: string): boolean {
		return agenticCells.some(a => a.cell_ref === ref);
	}

	// ---- Number format helpers -----------------------------------------------
	function setNumberFormat(fmt: string) {
		if (!activeSheet || !selectedCell) return;
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet.cells[ref]) {
				activeSheet.cells[ref].format.number_format = fmt;
			}
		}
		isDirty = true;
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
		if (resizingCol !== null) {
			resizingCol = null;
		}
	}

	// ---- Column resize -------------------------------------------------------
	function startColumnResize(col: number, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		resizingCol = col;
		resizeStartX = e.clientX;
		resizeStartWidth = getColWidth(col);
	}

	function handleResizeMouseMove(e: MouseEvent) {
		if (resizingCol === null) return;
		const delta = e.clientX - resizeStartX;
		const newWidth = Math.max(40, resizeStartWidth + delta);
		columnWidths[resizingCol] = newWidth;
	}

	function handleResizeMouseUp() {
		resizingCol = null;
	}

	function autoFitColumn(col: number) {
		if (!activeSheet) return;
		let maxWidth = 50;
		for (let r = 0; r < totalRows; r++) {
			const ref = makeCellRef(col, r);
			const cell = activeSheet.cells[ref];
			if (cell) {
				const text = getCellDisplay(cell);
				const estimatedWidth = text.length * 8 + 16;
				if (estimatedWidth > maxWidth) maxWidth = estimatedWidth;
			}
		}
		columnWidths[col] = Math.min(maxWidth, 400);
	}

	// ---- Context menu --------------------------------------------------------
	function handleCellContextMenu(ref: string, e: MouseEvent) {
		e.preventDefault();
		contextMenuRef = ref;
		contextMenuX = e.clientX;
		contextMenuY = e.clientY;
		contextMenuOpen = true;
	}

	function closeContextMenu() {
		contextMenuOpen = false;
		contextMenuRef = null;
	}

	function contextCopy() {
		if (!activeSheet) { closeContextMenu(); return; }
		const cells = getSelectedCells();
		const text = cells.map(ref => getCellDisplay(activeSheet!.cells[ref])).join('\t');
		navigator.clipboard.writeText(text).catch(() => {});
		closeContextMenu();
	}

	function contextCut() {
		if (!activeSheet) { closeContextMenu(); return; }
		contextCopy();
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet!.cells[ref]) {
				activeSheet!.cells[ref].value = { type: 'Empty' };
				activeSheet!.cells[ref].formula = null;
			}
		}
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	async function contextPaste() {
		if (!activeSheet || !selectedCell) { closeContextMenu(); return; }
		try {
			const text = await navigator.clipboard.readText();
			const pos = parseCellRef(selectedCell);
			if (!pos) { closeContextMenu(); return; }
			const rows = text.split('\n');
			for (let r = 0; r < rows.length; r++) {
				const cols = rows[r].split('\t');
				for (let c = 0; c < cols.length; c++) {
					const ref = makeCellRef(pos.col + c, pos.row + r);
					const val = cols[c].trim();
					if (!activeSheet!.cells[ref]) {
						activeSheet!.cells[ref] = {
							value: { type: 'Empty' },
							formula: null,
							format: { bold: false, italic: false, text_color: null, bg_color: null, number_format: null, align: 'left' },
							note: null
						};
					}
					if (val === '') {
						activeSheet!.cells[ref].value = { type: 'Empty' };
					} else if (!isNaN(Number(val))) {
						activeSheet!.cells[ref].value = { type: 'Number', value: Number(val) };
					} else {
						activeSheet!.cells[ref].value = { type: 'Text', value: val };
					}
				}
			}
			isDirty = true;
			saveIndicator = 'unsaved';
		} catch {}
		closeContextMenu();
	}

	function contextInsertRowAbove() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		// Shift all rows >= pos.row down by 1
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) { newCells[ref] = cell; continue; }
			if (p.row >= pos.row) {
				newCells[makeCellRef(p.col, p.row + 1)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		totalRows = Math.max(totalRows, totalRows + 1);
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextInsertRowBelow() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		const insertAt = pos.row + 1;
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) { newCells[ref] = cell; continue; }
			if (p.row >= insertAt) {
				newCells[makeCellRef(p.col, p.row + 1)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		totalRows = Math.max(totalRows, totalRows + 1);
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextInsertColLeft() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) { newCells[ref] = cell; continue; }
			if (p.col >= pos.col) {
				newCells[makeCellRef(p.col + 1, p.row)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		totalCols = Math.max(totalCols, totalCols + 1);
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextInsertColRight() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		const insertAt = pos.col + 1;
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) { newCells[ref] = cell; continue; }
			if (p.col >= insertAt) {
				newCells[makeCellRef(p.col + 1, p.row)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		totalCols = Math.max(totalCols, totalCols + 1);
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextDeleteRow() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) continue;
			if (p.row === pos.row) continue; // deleted
			if (p.row > pos.row) {
				newCells[makeCellRef(p.col, p.row - 1)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextDeleteCol() {
		if (!activeSheet || !contextMenuRef) { closeContextMenu(); return; }
		const pos = parseCellRef(contextMenuRef);
		if (!pos) { closeContextMenu(); return; }
		const newCells: Record<string, Cell> = {};
		for (const [ref, cell] of Object.entries(activeSheet.cells)) {
			const p = parseCellRef(ref);
			if (!p) continue;
			if (p.col === pos.col) continue;
			if (p.col > pos.col) {
				newCells[makeCellRef(p.col - 1, p.row)] = cell;
			} else {
				newCells[ref] = cell;
			}
		}
		activeSheet.cells = newCells;
		isDirty = true;
		saveIndicator = 'unsaved';
		closeContextMenu();
	}

	function contextClearContents() {
		if (!activeSheet) { closeContextMenu(); return; }
		const cells = getSelectedCells();
		for (const ref of cells) {
			if (activeSheet.cells[ref]) {
				activeSheet.cells[ref].value = { type: 'Empty' };
				activeSheet.cells[ref].formula = null;
			}
		}
		isDirty = true;
		saveIndicator = 'unsaved';
		updateFormulaBar();
		closeContextMenu();
	}

	// ---- Merge cells ---------------------------------------------------------
	function mergeSelectedCells() {
		if (!activeSheet || !selectionStart || !selectionEnd) return;
		const start = parseCellRef(selectionStart);
		const end = parseCellRef(selectionEnd);
		if (!start || !end) return;
		const region: MergedRegion = {
			startCol: Math.min(start.col, end.col),
			startRow: Math.min(start.row, end.row),
			endCol: Math.max(start.col, end.col),
			endRow: Math.max(start.row, end.row),
		};
		if (region.startCol === region.endCol && region.startRow === region.endRow) return;
		// Remove any overlapping merges
		mergedRegions = mergedRegions.filter(m =>
			!(m.startCol <= region.endCol && m.endCol >= region.startCol &&
			  m.startRow <= region.endRow && m.endRow >= region.startRow)
		);
		mergedRegions = [...mergedRegions, region];
		isDirty = true;
		saveIndicator = 'unsaved';
	}

	function unmergeSelectedCells() {
		if (!selectionStart || !selectionEnd) return;
		const start = parseCellRef(selectionStart);
		const end = parseCellRef(selectionEnd);
		if (!start || !end) return;
		const minCol = Math.min(start.col, end.col);
		const maxCol = Math.max(start.col, end.col);
		const minRow = Math.min(start.row, end.row);
		const maxRow = Math.max(start.row, end.row);
		mergedRegions = mergedRegions.filter(m =>
			!(m.startCol >= minCol && m.endCol <= maxCol &&
			  m.startRow >= minRow && m.endRow <= maxRow)
		);
		isDirty = true;
	}

	// ---- Sparkline SVG generation --------------------------------------------
	function buildSparklineSVG(data: number[], chartType: string, width: number, height: number): string {
		if (data.length === 0) return '';
		const maxVal = Math.max(...data, 0.001);
		const minVal = Math.min(...data, 0);
		const range = maxVal - minVal || 1;
		const pad = 2;
		const w = width - pad * 2;
		const h = height - pad * 2;

		if (chartType === 'bar') {
			const barW = Math.max(2, w / data.length - 1);
			const bars = data.map((v, i) => {
				const barH = ((v - minVal) / range) * h;
				const x = pad + i * (barW + 1);
				const y = pad + h - barH;
				return `<rect x="${x}" y="${y}" width="${barW}" height="${barH}" fill="#22d3ee" rx="1" opacity="0.85"/>`;
			}).join('');
			return `<svg viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">${bars}</svg>`;
		}

		// Line chart
		const points = data.map((v, i) => {
			const x = pad + (i / Math.max(data.length - 1, 1)) * w;
			const y = pad + h - ((v - minVal) / range) * h;
			return `${x},${y}`;
		}).join(' ');
		const dots = data.map((v, i) => {
			const x = pad + (i / Math.max(data.length - 1, 1)) * w;
			const y = pad + h - ((v - minVal) / range) * h;
			return `<circle cx="${x}" cy="${y}" r="1.5" fill="#22d3ee"/>`;
		}).join('');
		return `<svg viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg"><polyline points="${points}" fill="none" stroke="#22d3ee" stroke-width="1.5"/>${dots}</svg>`;
	}

	// ---- Selection range display for status bar ------------------------------
	let selectionDisplay = $derived.by(() => {
		if (!selectionStart || !selectionEnd || selectionStart === selectionEnd) return null;
		const start = parseCellRef(selectionStart);
		const end = parseCellRef(selectionEnd);
		if (!start || !end) return null;
		const rows = Math.abs(end.row - start.row) + 1;
		const cols = Math.abs(end.col - start.col) + 1;
		return `${selectionStart}:${selectionEnd} (${rows * cols} cells)`;
	});

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
		document.addEventListener('mousemove', handleResizeMouseMove);
		document.addEventListener('mouseup', handleResizeMouseUp);
		document.addEventListener('click', handleDocClick);
	});

	onDestroy(() => {
		if (autoSaveTimer) clearInterval(autoSaveTimer);
		document.removeEventListener('mouseup', handleMouseUp);
		document.removeEventListener('mousemove', handleResizeMouseMove);
		document.removeEventListener('mouseup', handleResizeMouseUp);
		document.removeEventListener('click', handleDocClick);
	});

	function handleDocClick() {
		if (contextMenuOpen) closeContextMenu();
		if (showFormatDropdown) showFormatDropdown = false;
	}

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
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<div
							onclick={() => openSpreadsheet(ss.id)}
							class="w-full flex items-start gap-2 px-3 py-2 text-left hover:bg-gx-bg-hover transition-colors group cursor-pointer
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
								onclick={(e) => { e.stopPropagation(); deleteSpreadsheet(ss.id); }}
								class="opacity-0 group-hover:opacity-100 text-gx-text-muted hover:text-gx-status-error transition-all shrink-0"
								title="Delete"
							>
								<Trash2 size={12} />
							</button>
						</div>
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

				<!-- Number formats dropdown -->
				<div class="relative">
					<button
						onclick={(e) => { e.stopPropagation(); showFormatDropdown = !showFormatDropdown; }}
						title="Number Format"
						class="flex items-center gap-0.5 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors text-[11px]"
					>
						<Hash size={13} />
						<ChevronDown size={9} />
					</button>
					{#if showFormatDropdown}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<div class="absolute top-full left-0 mt-1 w-48 bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg z-50 py-1"
							onclick={(e) => e.stopPropagation()}>
							<button onclick={() => { setNumberFormat('#,##0.00'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Number</span><span class="text-gx-text-muted font-mono">1,234.56</span>
							</button>
							<button onclick={() => { setNumberFormat('$#,##0.00'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Currency $</span><span class="text-gx-text-muted font-mono">$1,234.56</span>
							</button>
							<button onclick={() => { setNumberFormat('\u20AC#,##0.00'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Currency EUR</span><span class="text-gx-text-muted font-mono">&#x20AC;1.234,56</span>
							</button>
							<button onclick={() => { setNumberFormat('0%'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Percentage</span><span class="text-gx-text-muted font-mono">45.2%</span>
							</button>
							<button onclick={() => { setNumberFormat('yyyy-mm-dd'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Date</span><span class="text-gx-text-muted font-mono">2026-03-19</span>
							</button>
							<button onclick={() => { setNumberFormat('0.00E+00'); showFormatDropdown = false; }}
								class="w-full text-left px-3 py-1.5 text-[11px] text-gx-text-secondary hover:bg-gx-bg-hover flex justify-between">
								<span>Scientific</span><span class="text-gx-text-muted font-mono">1.23E+04</span>
							</button>
							<div class="border-t border-gx-border-default my-1"></div>
							<div class="px-3 py-1.5 flex items-center gap-1">
								<input type="text" bind:value={customFormatInput} placeholder="Custom format..."
									class="flex-1 px-2 py-0.5 text-[10px] rounded bg-gx-bg-tertiary border border-gx-border-default text-gx-text-primary outline-none" />
								<button onclick={() => { if (customFormatInput.trim()) { setNumberFormat(`custom:${customFormatInput.trim()}`); showFormatDropdown = false; }}}
									class="px-1.5 py-0.5 text-[10px] rounded bg-gx-neon/10 text-gx-neon hover:bg-gx-neon/20">OK</button>
							</div>
						</div>
					{/if}
				</div>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-0.5" />

				<!-- Merge cells -->
				<button onclick={mergeSelectedCells} title="Merge Cells"
					class="p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-text-secondary transition-colors text-[11px]">
					<LayoutGrid size={13} />
				</button>

				<!-- Freeze panes -->
				<button onclick={() => freezeFirstRow = !freezeFirstRow} title="Freeze First Row"
					class="p-1.5 rounded hover:bg-gx-bg-hover transition-colors text-[11px]
						{freezeFirstRow ? 'text-gx-neon bg-gx-neon/10' : 'text-gx-text-muted'}">
					<ArrowDown size={13} />
				</button>
				<button onclick={() => freezeFirstCol = !freezeFirstCol} title="Freeze First Column"
					class="p-1.5 rounded hover:bg-gx-bg-hover transition-colors text-[11px]
						{freezeFirstCol ? 'text-gx-neon bg-gx-neon/10' : 'text-gx-text-muted'}">
					<ArrowRight size={13} />
				</button>

				<Separator orientation="vertical" class="h-5 bg-gx-border-default mx-0.5" />

				<!-- Conditional formatting -->
				<button onclick={() => showConditionalDialog = true} title="Conditional Formatting"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-accent-magenta transition-colors text-[11px]">
					<Palette size={13} />
				</button>

				<!-- Chart insert -->
				<button onclick={() => generateChart()} disabled={chartLoading} title="Generate Chart from Selection"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-accent-blue transition-colors text-[11px]">
					{#if chartLoading}
						<Loader2 size={13} class="animate-spin" />
					{:else}
						<PieChart size={13} />
					{/if}
				</button>

				<!-- Pivot table -->
				<button onclick={() => showPivotDialog = true} title="Pivot Table"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-status-success transition-colors text-[11px]">
					<LayoutGrid size={13} />
				</button>

				<!-- Agentic cell -->
				<button onclick={() => showAgentDialog = true} title="Add Agentic Cell"
					class="flex items-center gap-1 p-1.5 rounded hover:bg-gx-bg-hover text-gx-text-muted hover:text-gx-status-warning transition-colors text-[11px]">
					<Zap size={13} />
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
				<!-- Virtual Scrolling Grid -->
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<div
					class="flex-1 overflow-auto sheets-grid focus:outline-none relative"
					tabindex="0"
					onkeydown={handleGridKeydown}
					bind:this={gridContainerEl}
					bind:clientWidth={viewportWidth}
					bind:clientHeight={viewportHeight}
					onscroll={(e) => {
						const t = e.target as HTMLElement;
						viewportTop = t.scrollTop;
						viewportLeft = t.scrollLeft;
					}}
					style="cursor: {resizingCol !== null ? 'col-resize' : 'default'}"
				>
					<!-- Total content spacer (creates scrollbar for full grid) -->
					<div style="width: {totalContentWidth + ROW_HEADER_WIDTH}px; height: {totalContentHeight + CELL_HEIGHT}px; position: relative;">

						<!-- ===== Column headers ===== -->
						{#each vVisibleCols as ci}
							{@const colLeft = getColLeft(ci)}
							{@const colW = getColWidth(ci)}
							<div
								class="bg-gx-bg-tertiary border-r border-b border-gx-border-default text-[10px] font-medium text-gx-text-muted flex items-center justify-center select-none"
								style="position: absolute; top: {freezeFirstRow ? viewportTop : 0}px; left: {ROW_HEADER_WIDTH + colLeft}px; width: {colW}px; height: {CELL_HEIGHT}px; z-index: 15;"
							>
								{colToLetter(ci)}
								<!-- Column resize handle -->
								<!-- svelte-ignore a11y_no_static_element_interactions -->
								<div
									class="absolute right-0 top-0 bottom-0 w-1.5 cursor-col-resize hover:bg-gx-neon/30 z-20"
									onmousedown={(e) => startColumnResize(ci, e)}
									ondblclick={() => autoFitColumn(ci)}
								></div>
							</div>
						{/each}

						<!-- ===== Row numbers ===== -->
						{#each vVisibleRows as ri}
							<div
								class="bg-gx-bg-tertiary border-r border-b border-gx-border-default text-[10px] text-gx-text-muted flex items-center justify-center font-mono select-none"
								style="position: absolute; top: {CELL_HEIGHT + ri * CELL_HEIGHT}px; left: {freezeFirstCol ? viewportLeft : 0}px; width: {ROW_HEADER_WIDTH}px; height: {CELL_HEIGHT}px; z-index: 10;"
							>
								{ri + 1}
							</div>
						{/each}

						<!-- ===== Frozen corner (top-left) ===== -->
						<div
							class="bg-gx-bg-tertiary border-r border-b border-gx-border-default z-30"
							style="position: absolute; top: {freezeFirstRow ? viewportTop : 0}px; left: {freezeFirstCol ? viewportLeft : 0}px; width: {ROW_HEADER_WIDTH}px; height: {CELL_HEIGHT}px;"
						></div>

						<!-- ===== Visible cells (only rendered in viewport) ===== -->
						{#each vVisibleRows as ri}
							{#each vVisibleCols as ci}
								{@const ref = makeCellRef(ci, ri)}
								{@const cell = activeSheet?.cells[ref]}
								{@const isSelected = isCellSelected(ref)}
								{@const isEditing = editingCell === ref}
								{@const condStyle = conditionalHighlights[ref]}
								{@const hasAgent = isAgenticCell(ref)}
								{@const colLeft = getColLeft(ci)}
								{@const colW = getColWidth(ci)}
								{@const merge = getMergeForCell(ci, ri)}
								{@const hidden = isCellHiddenByMerge(ci, ri)}
								{#if !hidden}
									{@const mergeW = merge ? Array.from({ length: merge.endCol - merge.startCol + 1 }, (_, i) => getColWidth(merge.startCol + i)).reduce((a, b) => a + b, 0) : colW}
									{@const mergeH = merge ? (merge.endRow - merge.startRow + 1) * CELL_HEIGHT : CELL_HEIGHT}
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<div
										class="absolute border-r border-b border-gx-border-default cursor-cell text-xs
											{isSelected ? 'bg-gx-neon/10 outline outline-1 outline-gx-neon z-[2]' : 'hover:bg-gx-bg-hover'}
											{cell?.value?.type === 'Error' ? 'text-gx-status-error' : 'text-gx-text-primary'}"
										style="top: {CELL_HEIGHT + ri * CELL_HEIGHT}px; left: {ROW_HEADER_WIDTH + colLeft}px; width: {mergeW}px; height: {mergeH}px;
											{condStyle?.bg_color ? `background-color: ${condStyle.bg_color};` : (cell?.format?.bg_color ? `background-color: ${cell.format.bg_color};` : '')}
											{condStyle?.text_color ? `color: ${condStyle.text_color};` : (cell?.format?.text_color ? `color: ${cell.format.text_color};` : '')}
											{(condStyle?.bold || cell?.format?.bold) ? 'font-weight: 700;' : ''}
											{(condStyle?.italic || cell?.format?.italic) ? 'font-style: italic;' : ''}
											text-align: {cell?.format?.align ?? 'left'};"
										onmousedown={(e) => handleCellMouseDown(ref, e)}
										onmouseenter={() => handleCellMouseEnter(ref)}
										oncontextmenu={(e) => handleCellContextMenu(ref, e)}
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
										{:else if isSparklineCell(cell)}
											{@const sparkData = getSparklineData(cell)}
											{#if sparkData}
												{@html buildSparklineSVG(sparkData.data, sparkData.chart_type, mergeW - 4, mergeH - 4)}
											{/if}
										{:else}
											<span class="block px-1 truncate leading-7" style="line-height: {mergeH}px;">
												{getCellDisplay(cell)}
											</span>
											{#if hasAgent}
												<button
													class="absolute top-0 right-0 p-0.5 text-gx-status-warning hover:text-gx-status-warning/80"
													title="Agentic cell - click to refresh"
													onclick={(e) => { e.stopPropagation(); refreshAgenticCell(ref); }}
												>
													<Zap size={8} />
												</button>
											{/if}
											{#if cell?.note}
												<div class="absolute top-0 right-0 w-0 h-0 border-t-[6px] border-r-[6px] border-t-gx-accent-blue border-r-transparent" title={cell.note}></div>
											{/if}
										{/if}
									</div>
								{/if}
							{/each}
						{/each}
					</div>
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

							<!-- Chart Panel -->
							{#if chartConfig}
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<PieChart size={13} class="text-gx-accent-blue" />
										<span class="text-xs font-medium text-gx-text-secondary">{chartConfig.title}</span>
										<div class="flex-1"></div>
										<button onclick={() => chartConfig = null} class="text-gx-text-muted hover:text-gx-text-primary">
											<X size={10} />
										</button>
									</div>
									<div class="flex gap-1 mb-2">
										{#each ['bar', 'line', 'pie', 'scatter'] as ct}
											<button
												onclick={() => generateChart(ct)}
												class="px-1.5 py-0.5 text-[9px] rounded border transition-colors
													{chartConfig.chart_type === ct ? 'border-gx-accent-blue text-gx-accent-blue bg-gx-accent-blue/10' : 'border-gx-border-default text-gx-text-muted hover:text-gx-text-secondary'}"
											>
												{ct}
											</button>
										{/each}
									</div>
									<!-- SVG Chart Rendering -->
									<div class="w-full bg-gx-bg-tertiary rounded p-2 overflow-hidden">
										{#if chartConfig.chart_type === 'bar' || chartConfig.chart_type === 'line'}
											<svg viewBox="0 0 260 120" class="w-full h-28">
												{#if chartConfig.series.length > 0}
													{@const maxVal = Math.max(...chartConfig.series[0].values, 1)}
													{@const barCount = chartConfig.series[0].values.length}
													{@const barW = Math.max(8, Math.min(30, 220 / barCount - 4))}
													{#each chartConfig.series[0].values as val, i}
														{@const barH = (val / maxVal) * 90}
														{@const x = 30 + i * (barW + 4)}
														{#if chartConfig.chart_type === 'bar'}
															<rect x={x} y={100 - barH} width={barW} height={barH} fill={chartConfig.colors[i % chartConfig.colors.length]} rx="2" opacity="0.85" />
														{/if}
														<text x={x + barW / 2} y="115" text-anchor="middle" class="fill-gx-text-muted" style="font-size: 6px">{chartConfig.categories[i] ?? i + 1}</text>
													{/each}
													{#if chartConfig.chart_type === 'line'}
														<polyline
															points={chartConfig.series[0].values.map((val, i) => {
																const x = 30 + i * (barW + 4) + barW / 2;
																const y = 100 - (val / maxVal) * 90;
																return `${x},${y}`;
															}).join(' ')}
															fill="none" stroke={chartConfig.colors[0]} stroke-width="2" />
														{#each chartConfig.series[0].values as val, i}
															{@const cx = 30 + i * (barW + 4) + barW / 2}
															{@const cy = 100 - (val / maxVal) * 90}
															<circle cx={cx} cy={cy} r="3" fill={chartConfig.colors[0]} />
														{/each}
													{/if}
												{/if}
											</svg>
										{:else if chartConfig.chart_type === 'pie'}
											<svg viewBox="0 0 130 130" class="w-full h-28">
												{#if chartConfig.series.length > 0}
													{@const total = chartConfig.series[0].values.reduce((a, b) => a + b, 0) || 1}
													{@const cx = 65}
													{@const cy = 65}
													{@const r = 50}
													{#each chartConfig.series[0].values as val, i}
														{@const startAngle = chartConfig.series[0].values.slice(0, i).reduce((a, b) => a + b, 0) / total * Math.PI * 2 - Math.PI / 2}
														{@const endAngle = chartConfig.series[0].values.slice(0, i + 1).reduce((a, b) => a + b, 0) / total * Math.PI * 2 - Math.PI / 2}
														{@const x1 = cx + r * Math.cos(startAngle)}
														{@const y1 = cy + r * Math.sin(startAngle)}
														{@const x2 = cx + r * Math.cos(endAngle)}
														{@const y2 = cy + r * Math.sin(endAngle)}
														{@const largeArc = (endAngle - startAngle) > Math.PI ? 1 : 0}
														<path d={`M${cx},${cy} L${x1},${y1} A${r},${r} 0 ${largeArc} 1 ${x2},${y2} Z`} fill={chartConfig.colors[i % chartConfig.colors.length]} opacity="0.85" />
													{/each}
												{/if}
											</svg>
										{:else}
											<svg viewBox="0 0 260 120" class="w-full h-28">
												{#if chartConfig.series.length >= 2}
													{@const maxX = Math.max(...chartConfig.series[0].values, 1)}
													{@const maxY = Math.max(...chartConfig.series[1].values, 1)}
													{#each chartConfig.series[0].values as xVal, i}
														{@const x = 30 + (xVal / maxX) * 210}
														{@const y = 100 - ((chartConfig.series[1]?.values[i] ?? 0) / maxY) * 90}
														<circle cx={x} cy={y} r="3" fill={chartConfig.colors[0]} opacity="0.8" />
													{/each}
												{:else if chartConfig.series.length === 1}
													{@const maxVal = Math.max(...chartConfig.series[0].values, 1)}
													{#each chartConfig.series[0].values as val, i}
														{@const x = 30 + i * 25}
														{@const y = 100 - (val / maxVal) * 90}
														<circle cx={x} cy={y} r="3" fill={chartConfig.colors[0]} opacity="0.8" />
													{/each}
												{/if}
											</svg>
										{/if}
									</div>
									<!-- Legend -->
									{#if chartConfig.categories.length > 0}
										<div class="flex flex-wrap gap-1 mt-1">
											{#each chartConfig.categories.slice(0, 8) as cat, i}
												<span class="flex items-center gap-1 text-[8px] text-gx-text-muted">
													<span class="w-2 h-2 rounded-full inline-block" style="background-color: {chartConfig.colors[i % chartConfig.colors.length]}"></span>
													{cat}
												</span>
											{/each}
										</div>
									{/if}
								</div>
							{/if}

							<!-- Pivot Table Result -->
							{#if pivotResult}
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<LayoutGrid size={13} class="text-gx-status-success" />
										<span class="text-xs font-medium text-gx-text-secondary">Pivot Table</span>
										<div class="flex-1"></div>
										<button onclick={() => pivotResult = null} class="text-gx-text-muted hover:text-gx-text-primary">
											<X size={10} />
										</button>
									</div>
									<div class="overflow-x-auto">
										<table class="w-full text-[10px] border-collapse">
											<thead>
												<tr>
													<th class="p-1 border border-gx-border-default bg-gx-bg-tertiary text-left text-gx-text-muted"></th>
													{#each pivotResult.headers as header}
														<th class="p-1 border border-gx-border-default bg-gx-bg-tertiary text-center text-gx-text-secondary font-medium">{header}</th>
													{/each}
												</tr>
											</thead>
											<tbody>
												{#each pivotResult.rows as row}
													<tr>
														<td class="p-1 border border-gx-border-default bg-gx-bg-tertiary text-gx-text-secondary font-medium">{row.label}</td>
														{#each row.values as val}
															<td class="p-1 border border-gx-border-default text-center text-gx-text-primary">
																{val != null ? (Number.isInteger(val) ? val : val.toFixed(2)) : '-'}
															</td>
														{/each}
													</tr>
												{/each}
											</tbody>
										</table>
									</div>
								</div>
							{/if}

							<!-- Agentic Cells list -->
							{#if agenticCells.length > 0}
								<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
									<div class="flex items-center gap-2 mb-2">
										<Zap size={13} class="text-gx-status-warning" />
										<span class="text-xs font-medium text-gx-text-secondary">Agentic Cells ({agenticCells.length})</span>
									</div>
									{#each agenticCells as agent}
										<div class="flex items-center gap-2 py-1 text-[10px] border-b border-gx-border-default last:border-b-0">
											<span class="font-mono text-gx-neon">{agent.cell_ref}</span>
											<Badge variant="outline" class="text-[8px] px-1 py-0 h-3.5">{agent.agent_type.type}</Badge>
											<div class="flex-1"></div>
											<button
												onclick={() => refreshAgenticCell(agent.cell_ref)}
												class="text-gx-text-muted hover:text-gx-neon transition-colors"
												title="Refresh"
											>
												<RefreshCw size={10} />
											</button>
										</div>
									{/each}
								</div>
							{/if}

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

				<!-- Selection info -->
				{#if selectionDisplay}
					<span class="text-[10px] text-gx-accent-blue font-mono">{selectionDisplay}</span>
					<Separator orientation="vertical" class="h-3 bg-gx-border-default" />
				{/if}

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
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-80 shadow-gx-glow-lg" onclick={(e) => e.stopPropagation()}>
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

<!-- Pivot Table dialog -->
{#if showPivotDialog}
	<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={() => showPivotDialog = false} role="dialog" aria-modal="true">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-80 shadow-gx-glow-lg" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-sm font-semibold text-gx-text-secondary mb-3 flex items-center gap-2">
				<LayoutGrid size={16} class="text-gx-status-success" />
				Pivot Table
			</h3>
			<p class="text-[10px] text-gx-text-muted mb-3">Select a range first, then configure fields (0-based column indices).</p>
			<div class="space-y-2">
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Row Field (column index)</label>
					<input type="number" bind:value={pivotRowField} min="0" class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
				</div>
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Column Field (column index)</label>
					<input type="number" bind:value={pivotColField} min="0" class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
				</div>
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Value Field (column index)</label>
					<input type="number" bind:value={pivotValueField} min="0" class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
				</div>
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Aggregation</label>
					<select bind:value={pivotAggregation} class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none">
						<option value="sum">Sum</option>
						<option value="average">Average</option>
						<option value="count">Count</option>
						<option value="min">Min</option>
						<option value="max">Max</option>
					</select>
				</div>
			</div>
			<div class="flex justify-end gap-2 mt-3">
				<button onclick={() => showPivotDialog = false} class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors">Cancel</button>
				<button onclick={generatePivot} disabled={pivotLoading} class="px-3 py-1.5 text-xs rounded-gx bg-gx-status-success/10 text-gx-status-success hover:bg-gx-status-success/20 transition-colors disabled:opacity-50">
					{#if pivotLoading}
						<Loader2 size={12} class="animate-spin inline" />
					{/if}
					Generate
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Conditional Formatting dialog -->
{#if showConditionalDialog}
	<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={() => showConditionalDialog = false} role="dialog" aria-modal="true">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-80 shadow-gx-glow-lg" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-sm font-semibold text-gx-text-secondary mb-3 flex items-center gap-2">
				<Palette size={16} class="text-gx-accent-magenta" />
				Conditional Formatting
			</h3>
			<div class="space-y-2">
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Condition</label>
					<select bind:value={condType} class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none">
						<option value="greater_than">Greater Than</option>
						<option value="less_than">Less Than</option>
						<option value="equal_to">Equal To</option>
						<option value="between">Between (min,max)</option>
						<option value="text_contains">Text Contains</option>
						<option value="is_empty">Is Empty</option>
						<option value="is_not_empty">Is Not Empty</option>
					</select>
				</div>
				{#if !['is_empty', 'is_not_empty'].includes(condType)}
					<div>
						<label class="text-[10px] text-gx-text-muted block mb-0.5">
							{condType === 'between' ? 'Values (min, max)' : condType === 'text_contains' ? 'Text' : 'Value'}
						</label>
						<input type="text" bind:value={condValue} placeholder={condType === 'between' ? '10, 50' : '10'} class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
					</div>
				{/if}
				<div class="flex gap-2">
					<div class="flex-1">
						<label class="text-[10px] text-gx-text-muted block mb-0.5">Background Color</label>
						<input type="color" bind:value={condBgColor} class="w-full h-7 rounded-gx border border-gx-border-default cursor-pointer" />
					</div>
					<div class="flex-1">
						<label class="text-[10px] text-gx-text-muted block mb-0.5">Text Color</label>
						<input type="color" bind:value={condTextColor} class="w-full h-7 rounded-gx border border-gx-border-default cursor-pointer" />
					</div>
				</div>
			</div>
			<div class="flex justify-end gap-2 mt-3">
				<button onclick={() => showConditionalDialog = false} class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors">Cancel</button>
				<button onclick={addConditionalFormat} class="px-3 py-1.5 text-xs rounded-gx bg-gx-accent-magenta/10 text-gx-accent-magenta hover:bg-gx-accent-magenta/20 transition-colors">Apply</button>
			</div>
		</div>
	</div>
{/if}

<!-- Agentic Cell dialog -->
{#if showAgentDialog}
	<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={() => showAgentDialog = false} role="dialog" aria-modal="true">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="bg-gx-bg-elevated border border-gx-border-default rounded-gx p-4 w-80 shadow-gx-glow-lg" onclick={(e) => e.stopPropagation()}>
			<h3 class="text-sm font-semibold text-gx-text-secondary mb-3 flex items-center gap-2">
				<Zap size={16} class="text-gx-status-warning" />
				Add Agentic Cell
			</h3>
			<p class="text-[10px] text-gx-text-muted mb-2">Cell <span class="font-mono text-gx-neon">{selectedCell ?? '?'}</span> will autonomously fetch data.</p>
			<div class="space-y-2">
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Agent Type</label>
					<select bind:value={agentType} class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none">
						<option value="web_fetch">Web Fetch (URL)</option>
						<option value="api_call">API Call</option>
						<option value="ai_generate">AI Generate (Ollama)</option>
						<option value="file_watch">File Watch</option>
					</select>
				</div>
				{#if agentType === 'web_fetch' || agentType === 'api_call' || agentType === 'file_watch'}
					<div>
						<label class="text-[10px] text-gx-text-muted block mb-0.5">
							{agentType === 'file_watch' ? 'File Path' : 'URL / Endpoint'}
						</label>
						<input type="text" bind:value={agentUrl} placeholder={agentType === 'file_watch' ? '/path/to/file.txt' : 'https://api.example.com/data'} class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
					</div>
				{:else if agentType === 'ai_generate'}
					<div>
						<label class="text-[10px] text-gx-text-muted block mb-0.5">Prompt</label>
						<textarea bind:value={agentPrompt} placeholder="e.g. Generate today's exchange rate for EUR/USD" class="w-full h-16 p-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary resize-none outline-none"></textarea>
					</div>
				{/if}
				<div>
					<label class="text-[10px] text-gx-text-muted block mb-0.5">Auto-refresh (seconds, 0 = manual)</label>
					<input type="number" bind:value={agentRefreshInterval} min="0" class="w-full px-2 py-1 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-primary outline-none" />
				</div>
			</div>
			<div class="flex justify-end gap-2 mt-3">
				<button onclick={() => showAgentDialog = false} class="px-3 py-1.5 text-xs rounded-gx text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover transition-colors">Cancel</button>
				<button onclick={addAgenticCell} class="px-3 py-1.5 text-xs rounded-gx bg-gx-status-warning/10 text-gx-status-warning hover:bg-gx-status-warning/20 transition-colors">Add Agent</button>
			</div>
		</div>
	</div>
{/if}

<!-- Cell Context Menu (right-click) -->
{#if contextMenuOpen}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="fixed z-[100] bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg py-1 min-w-[180px] text-xs"
		style="left: {contextMenuX}px; top: {contextMenuY}px;"
		onclick={(e) => e.stopPropagation()}
	>
		<button onclick={contextCut} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<Scissors size={12} /> Cut
		</button>
		<button onclick={contextCopy} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<Copy size={12} /> Copy
		</button>
		<button onclick={contextPaste} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<Clipboard size={12} /> Paste
		</button>
		<div class="border-t border-gx-border-default my-1"></div>
		<button onclick={contextInsertRowAbove} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<ArrowDown size={12} class="rotate-180" /> Insert Row Above
		</button>
		<button onclick={contextInsertRowBelow} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<ArrowDown size={12} /> Insert Row Below
		</button>
		<button onclick={contextInsertColLeft} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<ArrowRight size={12} class="rotate-180" /> Insert Column Left
		</button>
		<button onclick={contextInsertColRight} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<ArrowRight size={12} /> Insert Column Right
		</button>
		<div class="border-t border-gx-border-default my-1"></div>
		<button onclick={contextDeleteRow} class="w-full text-left px-3 py-1.5 text-gx-status-error hover:bg-gx-bg-hover flex items-center gap-2">
			<Trash2 size={12} /> Delete Row
		</button>
		<button onclick={contextDeleteCol} class="w-full text-left px-3 py-1.5 text-gx-status-error hover:bg-gx-bg-hover flex items-center gap-2">
			<Trash2 size={12} /> Delete Column
		</button>
		<button onclick={contextClearContents} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<X size={12} /> Clear Contents
		</button>
		<div class="border-t border-gx-border-default my-1"></div>
		<button onclick={() => { showConditionalDialog = true; closeContextMenu(); }} class="w-full text-left px-3 py-1.5 text-gx-text-secondary hover:bg-gx-bg-hover flex items-center gap-2">
			<Palette size={12} /> Format Cells...
		</button>
		<button onclick={() => { showAgentDialog = true; closeContextMenu(); }} class="w-full text-left px-3 py-1.5 text-gx-status-warning hover:bg-gx-bg-hover flex items-center gap-2">
			<Zap size={12} /> Add Agentic Cell
		</button>
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
