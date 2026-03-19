<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		CalendarDays, Plus, ChevronLeft, ChevronRight, Loader2,
		AlertCircle, X, Clock, MapPin, Users, Sparkles, RefreshCw,
		Eye, EyeOff, Trash2, Upload, Link, FileText, Sun, Brain,
		Calendar as CalendarIcon, LayoutGrid, List, Columns,
		Bell, Palette, Check, ChevronDown, Download
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-calendar';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface EventSource {
		type: 'local' | 'google_import' | 'outlook_import' | 'apple_import' | 'ics_import';
		url?: string;
	}

	interface CalendarEvent {
		id: string;
		title: string;
		description: string | null;
		start: string;
		end: string;
		all_day: boolean;
		location: string | null;
		color: string | null;
		calendar_id: string;
		recurrence: string | null;
		reminder_minutes: number | null;
		attendees: string[];
		source: EventSource;
		created_at: string;
		updated_at: string;
	}

	interface CalendarData {
		id: string;
		name: string;
		color: string;
		source_type: string;
		source_url: string | null;
		visible: boolean;
		auto_sync: boolean;
		last_synced: string | null;
		created_at: string;
	}

	interface ImportResult {
		calendar_id: string;
		calendar_name: string;
		imported_count: number;
		skipped_count: number;
		errors: string[];
	}

	interface TimeSlot {
		start: string;
		end: string;
		score: number;
		reason: string;
	}

	// ---- State ---------------------------------------------------------------
	type ViewMode = 'month' | 'week' | 'day';

	let viewMode = $state<ViewMode>('month');
	let currentDate = $state(new Date());
	let calendars = $state<CalendarData[]>([]);
	let events = $state<CalendarEvent[]>([]);
	let loading = $state(true);
	let error = $state('');

	// Event dialog
	let showEventDialog = $state(false);
	let editingEvent = $state<CalendarEvent | null>(null);
	let eventForm = $state({
		title: '',
		description: '',
		start: '',
		end: '',
		all_day: false,
		location: '',
		color: '',
		calendar_id: '',
		reminder_minutes: 15 as number | null,
		attendees: ''
	});

	// Add calendar dialog
	let showAddCalendar = $state(false);
	let addCalendarMode = $state<'local' | 'url' | 'file'>('local');
	let newCalendarName = $state('');
	let newCalendarColor = $state('#22c55e');
	let importUrl = $state('');
	let importLoading = $state(false);

	// AI panel
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiBriefing = $state('');
	let aiSuggestions = $state<TimeSlot[]>([]);
	let aiAgenda = $state('');
	let findDuration = $state(60);
	let findPreferredHours = $state('9-17');

	// Syncing
	let syncingCalId = $state('');

	// ---- Derived values ------------------------------------------------------
	let currentYear = $derived(currentDate.getFullYear());
	let currentMonth = $derived(currentDate.getMonth());
	let currentDay = $derived(currentDate.getDate());
	let monthName = $derived(currentDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' }));

	let today = $derived(new Date());
	let todayStr = $derived(formatDateStr(today));

	// Month grid computation
	let monthGrid = $derived(computeMonthGrid(currentYear, currentMonth));

	// Week grid computation
	let weekDays = $derived(computeWeekDays(currentDate));

	// Selected day for day view
	let selectedDayStr = $derived(formatDateStr(currentDate));

	// Events grouped by date for fast lookup
	let eventsByDate = $derived(groupEventsByDate(events));

	// ---- Helper functions ----------------------------------------------------
	function formatDateStr(d: Date): string {
		const y = d.getFullYear();
		const m = String(d.getMonth() + 1).padStart(2, '0');
		const day = String(d.getDate()).padStart(2, '0');
		return `${y}-${m}-${day}`;
	}

	function computeMonthGrid(year: number, month: number): { date: Date; inMonth: boolean }[][] {
		const firstDay = new Date(year, month, 1);
		const startDow = firstDay.getDay(); // 0=Sun
		const daysInMonth = new Date(year, month + 1, 0).getDate();
		const grid: { date: Date; inMonth: boolean }[][] = [];
		let row: { date: Date; inMonth: boolean }[] = [];

		// Fill days from previous month
		const prevMonth = new Date(year, month, 0);
		const prevDays = prevMonth.getDate();
		for (let i = startDow - 1; i >= 0; i--) {
			row.push({ date: new Date(year, month - 1, prevDays - i), inMonth: false });
		}

		// Current month days
		for (let d = 1; d <= daysInMonth; d++) {
			if (row.length === 7) {
				grid.push(row);
				row = [];
			}
			row.push({ date: new Date(year, month, d), inMonth: true });
		}

		// Fill remaining with next month
		let nextDay = 1;
		while (row.length < 7) {
			row.push({ date: new Date(year, month + 1, nextDay++), inMonth: false });
		}
		grid.push(row);

		// Ensure 6 rows for consistent layout
		while (grid.length < 6) {
			row = [];
			for (let i = 0; i < 7; i++) {
				row.push({ date: new Date(year, month + 1, nextDay++), inMonth: false });
			}
			grid.push(row);
		}

		return grid;
	}

	function computeWeekDays(d: Date): Date[] {
		const start = new Date(d);
		start.setDate(start.getDate() - start.getDay()); // Start on Sunday
		const days: Date[] = [];
		for (let i = 0; i < 7; i++) {
			const day = new Date(start);
			day.setDate(start.getDate() + i);
			days.push(day);
		}
		return days;
	}

	function groupEventsByDate(evts: CalendarEvent[]): Map<string, CalendarEvent[]> {
		const map = new Map<string, CalendarEvent[]>();
		for (const evt of evts) {
			const dateKey = evt.start.substring(0, 10);
			if (!map.has(dateKey)) map.set(dateKey, []);
			map.get(dateKey)!.push(evt);
		}
		return map;
	}

	function getEventsForDate(dateStr: string): CalendarEvent[] {
		return eventsByDate.get(dateStr) ?? [];
	}

	function getEventColor(evt: CalendarEvent): string {
		if (evt.color) return evt.color;
		const cal = calendars.find(c => c.id === evt.calendar_id);
		return cal?.color ?? '#22c55e';
	}

	function formatTime(iso: string): string {
		const timePart = iso.substring(11, 16);
		if (!timePart || timePart === '') return '';
		const [h, m] = timePart.split(':').map(Number);
		const period = h >= 12 ? 'PM' : 'AM';
		const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
		return `${h12}:${String(m).padStart(2, '0')} ${period}`;
	}

	function isToday(d: Date): boolean {
		return formatDateStr(d) === todayStr;
	}

	// ---- Navigation ----------------------------------------------------------
	function prevPeriod() {
		const d = new Date(currentDate);
		if (viewMode === 'month') {
			d.setMonth(d.getMonth() - 1);
		} else if (viewMode === 'week') {
			d.setDate(d.getDate() - 7);
		} else {
			d.setDate(d.getDate() - 1);
		}
		currentDate = d;
		loadEvents();
	}

	function nextPeriod() {
		const d = new Date(currentDate);
		if (viewMode === 'month') {
			d.setMonth(d.getMonth() + 1);
		} else if (viewMode === 'week') {
			d.setDate(d.getDate() + 7);
		} else {
			d.setDate(d.getDate() + 1);
		}
		currentDate = d;
		loadEvents();
	}

	function goToday() {
		currentDate = new Date();
		loadEvents();
	}

	function selectDay(d: Date) {
		currentDate = new Date(d);
		if (viewMode === 'month') {
			viewMode = 'day';
		}
		loadEvents();
	}

	// ---- Data loading --------------------------------------------------------
	async function loadCalendars() {
		try {
			calendars = await invoke<CalendarData[]>('calendar_list');
		} catch (e: any) {
			console.error('Failed to load calendars:', e);
		}
	}

	async function loadEvents() {
		try {
			let startDate: string;
			let endDate: string;

			if (viewMode === 'month') {
				const first = new Date(currentYear, currentMonth, 1);
				const last = new Date(currentYear, currentMonth + 1, 0);
				// Include surrounding days for the grid
				const gridStart = new Date(first);
				gridStart.setDate(gridStart.getDate() - first.getDay());
				const gridEnd = new Date(last);
				gridEnd.setDate(gridEnd.getDate() + (6 - last.getDay()));
				startDate = formatDateStr(gridStart);
				endDate = formatDateStr(gridEnd);
			} else if (viewMode === 'week') {
				startDate = formatDateStr(weekDays[0]);
				endDate = formatDateStr(weekDays[6]);
			} else {
				startDate = selectedDayStr;
				endDate = selectedDayStr;
			}

			events = await invoke<CalendarEvent[]>('calendar_list_events', {
				startDate,
				endDate,
				calendarIds: null,
			});
		} catch (e: any) {
			console.error('Failed to load events:', e);
		}
	}

	async function loadAll() {
		loading = true;
		error = '';
		try {
			await loadCalendars();
			await loadEvents();
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Unknown error';
		} finally {
			loading = false;
		}
	}

	// ---- Calendar management -------------------------------------------------
	async function createCalendar() {
		if (!newCalendarName.trim()) return;
		try {
			await invoke('calendar_create', { name: newCalendarName, color: newCalendarColor });
			newCalendarName = '';
			newCalendarColor = '#22c55e';
			showAddCalendar = false;
			await loadCalendars();
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Failed to create calendar';
		}
	}

	async function importFromUrl() {
		if (!importUrl.trim()) return;
		importLoading = true;
		try {
			const result = await invoke<ImportResult>('calendar_import_ics', { urlOrPath: importUrl });
			importUrl = '';
			showAddCalendar = false;
			await loadAll();
			alert(`Imported ${result.imported_count} events into "${result.calendar_name}"`);
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Import failed';
		} finally {
			importLoading = false;
		}
	}

	async function deleteCalendar(id: string) {
		if (!confirm('Delete this calendar and all its events?')) return;
		try {
			await invoke('calendar_delete', { id });
			await loadAll();
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Delete failed';
		}
	}

	async function toggleCalendarVisibility(cal: CalendarData) {
		cal.visible = !cal.visible;
		calendars = [...calendars];
		await loadEvents();
	}

	async function syncCalendar(calId: string) {
		syncingCalId = calId;
		try {
			const result = await invoke<ImportResult>('calendar_sync_ics', { calendarId: calId });
			await loadAll();
			alert(`Synced ${result.imported_count} events from "${result.calendar_name}"`);
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Sync failed';
		} finally {
			syncingCalId = '';
		}
	}

	// ---- Event CRUD ----------------------------------------------------------
	function openNewEventDialog(dateStr?: string) {
		editingEvent = null;
		const date = dateStr ?? selectedDayStr;
		eventForm = {
			title: '',
			description: '',
			start: `${date}T09:00`,
			end: `${date}T10:00`,
			all_day: false,
			location: '',
			color: '',
			calendar_id: calendars.length > 0 ? calendars[0].id : '',
			reminder_minutes: 15,
			attendees: ''
		};
		showEventDialog = true;
	}

	function openEditEventDialog(evt: CalendarEvent) {
		editingEvent = evt;
		eventForm = {
			title: evt.title,
			description: evt.description ?? '',
			start: evt.start.substring(0, 16),
			end: evt.end.substring(0, 16),
			all_day: evt.all_day,
			location: evt.location ?? '',
			color: evt.color ?? '',
			calendar_id: evt.calendar_id,
			reminder_minutes: evt.reminder_minutes ?? 15,
			attendees: evt.attendees.join(', ')
		};
		showEventDialog = true;
	}

	async function saveEvent() {
		if (!eventForm.title.trim()) return;

		try {
			if (editingEvent) {
				await invoke('calendar_update_event', {
					id: editingEvent.id,
					updates: {
						title: eventForm.title,
						description: eventForm.description || null,
						start: eventForm.start.length > 16 ? eventForm.start : eventForm.start + ':00',
						end: eventForm.end.length > 16 ? eventForm.end : eventForm.end + ':00',
						all_day: eventForm.all_day,
						location: eventForm.location || null,
						color: eventForm.color || null,
						calendar_id: eventForm.calendar_id,
						reminder_minutes: eventForm.reminder_minutes,
						attendees: eventForm.attendees.split(',').map((s: string) => s.trim()).filter((s: string) => s.length > 0),
					}
				});
			} else {
				await invoke('calendar_create_event', {
					event: {
						title: eventForm.title,
						description: eventForm.description || null,
						start: eventForm.start.length > 16 ? eventForm.start : eventForm.start + ':00',
						end: eventForm.end.length > 16 ? eventForm.end : eventForm.end + ':00',
						all_day: eventForm.all_day,
						location: eventForm.location || null,
						color: eventForm.color || null,
						calendar_id: eventForm.calendar_id,
						recurrence: null,
						reminder_minutes: eventForm.reminder_minutes,
						attendees: eventForm.attendees.split(',').map((s: string) => s.trim()).filter((s: string) => s.length > 0),
					}
				});
			}
			showEventDialog = false;
			await loadEvents();
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Failed to save event';
		}
	}

	async function deleteEvent(id: string) {
		if (!confirm('Delete this event?')) return;
		try {
			await invoke('calendar_delete_event', { id });
			showEventDialog = false;
			await loadEvents();
		} catch (e: any) {
			error = typeof e === 'string' ? e : e?.message ?? 'Failed to delete event';
		}
	}

	// ---- AI features ---------------------------------------------------------
	async function loadDailyBriefing() {
		aiLoading = true;
		aiBriefing = '';
		try {
			aiBriefing = await invoke<string>('calendar_ai_daily_briefing', { date: todayStr });
		} catch (e: any) {
			aiBriefing = `AI unavailable: ${typeof e === 'string' ? e : e?.message ?? 'Unknown error'}`;
		} finally {
			aiLoading = false;
		}
	}

	async function findFreeTime() {
		aiLoading = true;
		aiSuggestions = [];
		try {
			aiSuggestions = await invoke<TimeSlot[]>('calendar_ai_suggest_time', {
				durationMinutes: findDuration,
				participants: [],
				preferredHours: findPreferredHours,
			});
		} catch (e: any) {
			console.error('AI suggest time failed:', e);
		} finally {
			aiLoading = false;
		}
	}

	async function generateAgenda(eventId: string) {
		aiLoading = true;
		aiAgenda = '';
		try {
			aiAgenda = await invoke<string>('calendar_ai_generate_agenda', { eventId });
		} catch (e: any) {
			aiAgenda = `AI unavailable: ${typeof e === 'string' ? e : e?.message ?? 'Unknown error'}`;
		} finally {
			aiLoading = false;
		}
	}

	// ---- Color presets -------------------------------------------------------
	const colorPresets = [
		'#ef4444', '#f97316', '#eab308', '#22c55e', '#14b8a6',
		'#3b82f6', '#6366f1', '#8b5cf6', '#ec4899', '#f43f5e',
	];

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadAll();
	});

	// Reload events when view mode changes
	$effect(() => {
		// Access viewMode to register the dependency
		const _vm = viewMode;
		if (!loading) {
			loadEvents();
		}
	});

	// Hours for week/day view
	const hours = Array.from({ length: 24 }, (_, i) => i);
</script>

<div class="flex h-full {hasEngineStyle && containerComponent ? '' : 'bg-gx-bg-primary'}" style={containerStyle}>
	<!-- Left Sidebar (250px) -->
	<aside class="w-64 border-r border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 overflow-y-auto">
		<!-- Mini Month Calendar -->
		<div class="p-3 border-b border-gx-border-default">
			<div class="flex items-center justify-between mb-2">
				<button onclick={() => { const d = new Date(currentDate); d.setMonth(d.getMonth() - 1); currentDate = d; loadEvents(); }}
					class="p-0.5 text-gx-text-muted hover:text-gx-neon transition-colors">
					<ChevronLeft size={14} />
				</button>
				<span class="text-xs font-medium text-gx-text-secondary">
					{currentDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}
				</span>
				<button onclick={() => { const d = new Date(currentDate); d.setMonth(d.getMonth() + 1); currentDate = d; loadEvents(); }}
					class="p-0.5 text-gx-text-muted hover:text-gx-neon transition-colors">
					<ChevronRight size={14} />
				</button>
			</div>

			<!-- Mini grid -->
			<div class="grid grid-cols-7 gap-0">
				{#each ['S', 'M', 'T', 'W', 'T', 'F', 'S'] as day}
					<div class="text-center text-[9px] text-gx-text-muted font-medium py-0.5">{day}</div>
				{/each}
				{#each monthGrid as row}
					{#each row as cell}
						<button
							onclick={() => selectDay(cell.date)}
							class="text-center text-[10px] py-0.5 rounded transition-colors
								{isToday(cell.date)
									? 'bg-gx-neon text-gx-bg-primary font-bold'
									: cell.inMonth
										? 'text-gx-text-secondary hover:bg-gx-bg-hover'
										: 'text-gx-text-muted/40'}"
						>
							{cell.date.getDate()}
						</button>
					{/each}
				{/each}
			</div>
		</div>

		<!-- Calendars List -->
		<div class="p-3 flex-1">
			<div class="flex items-center justify-between mb-2">
				<span class="text-xs font-semibold text-gx-text-secondary uppercase tracking-wider">Calendars</span>
				<button onclick={() => showAddCalendar = true}
					class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors" title="Add Calendar">
					<Plus size={14} />
				</button>
			</div>

			{#if calendars.length === 0}
				<p class="text-[11px] text-gx-text-muted italic">No calendars yet. Create one to start.</p>
			{:else}
				<div class="space-y-1">
					{#each calendars as cal (cal.id)}
						<div class="group flex items-center gap-2 px-2 py-1.5 rounded-gx hover:bg-gx-bg-hover transition-colors">
							<button onclick={() => toggleCalendarVisibility(cal)}
								class="w-3 h-3 rounded-sm border shrink-0 transition-colors"
								style="background-color: {cal.visible ? cal.color : 'transparent'}; border-color: {cal.color};"
								title={cal.visible ? 'Hide calendar' : 'Show calendar'}>
							</button>
							<span class="text-[11px] text-gx-text-secondary truncate flex-1">{cal.name}</span>
							<div class="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
								{#if cal.source_url}
									<button onclick={() => syncCalendar(cal.id)}
										class="p-0.5 text-gx-text-muted hover:text-gx-neon" title="Sync">
										{#if syncingCalId === cal.id}
											<Loader2 size={11} class="animate-spin" />
										{:else}
											<RefreshCw size={11} />
										{/if}
									</button>
								{/if}
								<button onclick={() => deleteCalendar(cal.id)}
									class="p-0.5 text-gx-text-muted hover:text-gx-status-error" title="Delete">
									<Trash2 size={11} />
								</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Upcoming Events -->
		<div class="p-3 border-t border-gx-border-default">
			<span class="text-xs font-semibold text-gx-text-secondary uppercase tracking-wider mb-2 block">Upcoming</span>
			{#if events.filter(e => e.start >= todayStr).length === 0}
				<p class="text-[11px] text-gx-text-muted italic">No upcoming events</p>
			{:else}
				<div class="space-y-1.5">
					{#each events.filter(e => e.start >= todayStr).slice(0, 5) as evt (evt.id)}
						<button onclick={() => openEditEventDialog(evt)}
							class="w-full text-left px-2 py-1 rounded-gx hover:bg-gx-bg-hover transition-colors">
							<div class="flex items-center gap-1.5">
								<span class="w-2 h-2 rounded-full shrink-0" style="background-color: {getEventColor(evt)};"></span>
								<span class="text-[11px] text-gx-text-secondary truncate">{evt.title}</span>
							</div>
							<div class="text-[9px] text-gx-text-muted ml-3.5">
								{#if evt.all_day}
									{evt.start.substring(0, 10)} (all day)
								{:else}
									{evt.start.substring(0, 10)} {formatTime(evt.start)}
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</aside>

	<!-- Main Calendar Area -->
	<div class="flex-1 flex flex-col min-w-0">
		<!-- Header toolbar -->
		<header class="flex items-center h-11 px-4 border-b border-gx-border-default bg-gx-bg-secondary shrink-0 gap-3">
			<CalendarDays size={18} class="text-gx-neon shrink-0" />
			<h1 class="text-sm font-semibold text-gx-text-primary">ForgeCalendar</h1>

			<Separator orientation="vertical" class="h-5 bg-gx-border-default" />

			<!-- Navigation -->
			<button onclick={prevPeriod} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors">
				<ChevronLeft size={16} />
			</button>
			<button onclick={goToday}
				class="px-2 py-0.5 text-xs text-gx-text-secondary hover:text-gx-neon hover:bg-gx-bg-hover rounded-gx transition-colors">
				Today
			</button>
			<button onclick={nextPeriod} class="p-1 text-gx-text-muted hover:text-gx-neon transition-colors">
				<ChevronRight size={16} />
			</button>

			<span class="text-sm font-medium text-gx-text-primary">
				{#if viewMode === 'month'}
					{monthName}
				{:else if viewMode === 'week'}
					{weekDays[0].toLocaleDateString('en-US', { month: 'short', day: 'numeric' })} - {weekDays[6].toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}
				{:else}
					{currentDate.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' })}
				{/if}
			</span>

			<div class="flex-1"></div>

			<!-- View mode toggles -->
			<div class="flex items-center bg-gx-bg-tertiary rounded-gx border border-gx-border-default p-0.5">
				<button onclick={() => viewMode = 'month'}
					class="px-2 py-0.5 text-[11px] rounded transition-colors
						{viewMode === 'month' ? 'bg-gx-bg-elevated text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}">
					Month
				</button>
				<button onclick={() => viewMode = 'week'}
					class="px-2 py-0.5 text-[11px] rounded transition-colors
						{viewMode === 'week' ? 'bg-gx-bg-elevated text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}">
					Week
				</button>
				<button onclick={() => viewMode = 'day'}
					class="px-2 py-0.5 text-[11px] rounded transition-colors
						{viewMode === 'day' ? 'bg-gx-bg-elevated text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}">
					Day
				</button>
			</div>

			<!-- AI Panel toggle -->
			<button onclick={() => aiPanelOpen = !aiPanelOpen}
				class="flex items-center gap-1 px-2 py-1 text-[11px] rounded-gx border transition-colors
					{aiPanelOpen
						? 'border-gx-neon/50 text-gx-neon bg-gx-neon/10'
						: 'border-gx-border-default text-gx-text-muted hover:text-gx-neon hover:border-gx-neon/30'}">
				<Sparkles size={12} />
				AI
			</button>

			<!-- New event -->
			<button onclick={() => openNewEventDialog()}
				class="flex items-center gap-1 px-2.5 py-1 text-[11px] bg-gx-neon text-gx-bg-primary rounded-gx font-medium hover:opacity-90 transition-opacity">
				<Plus size={13} />
				Event
			</button>
		</header>

		<!-- Calendar content -->
		<div class="flex-1 flex overflow-hidden">
			<!-- Calendar grid -->
			<div class="flex-1 overflow-auto">
				{#if loading}
					<div class="flex items-center justify-center h-full">
						<Loader2 size={24} class="animate-spin text-gx-neon" />
					</div>
				{:else if viewMode === 'month'}
					<!-- Month View -->
					<div class="h-full flex flex-col">
						<!-- Day headers -->
						<div class="grid grid-cols-7 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
							{#each ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'] as dayName}
								<div class="text-center text-[11px] text-gx-text-muted font-medium py-1.5 border-r border-gx-border-default last:border-r-0">
									{dayName}
								</div>
							{/each}
						</div>

						<!-- Month grid -->
						<div class="flex-1 grid grid-rows-6">
							{#each monthGrid as row}
								<div class="grid grid-cols-7 border-b border-gx-border-default last:border-b-0">
									{#each row as cell}
										{@const dateStr = formatDateStr(cell.date)}
										{@const dayEvents = getEventsForDate(dateStr)}
										<button
											onclick={() => selectDay(cell.date)}
											class="border-r border-gx-border-default last:border-r-0 p-1 text-left transition-colors hover:bg-gx-bg-hover/50 min-h-0 overflow-hidden
												{cell.inMonth ? '' : 'bg-gx-bg-secondary/30'}"
										>
											<div class="flex items-start justify-between">
												<span class="text-[11px] leading-tight font-medium w-6 h-6 flex items-center justify-center rounded-full
													{isToday(cell.date)
														? 'bg-gx-neon text-gx-bg-primary font-bold'
														: cell.inMonth
															? 'text-gx-text-secondary'
															: 'text-gx-text-muted/40'}">
													{cell.date.getDate()}
												</span>
											</div>
											{#if dayEvents.length > 0}
												<div class="mt-0.5 space-y-px">
													{#each dayEvents.slice(0, 3) as evt (evt.id)}
														<div class="text-[9px] px-1 py-px rounded truncate text-white/90 leading-tight"
															style="background-color: {getEventColor(evt)};">
															{#if !evt.all_day}{formatTime(evt.start)} {/if}{evt.title}
														</div>
													{/each}
													{#if dayEvents.length > 3}
														<div class="text-[9px] text-gx-text-muted px-1">+{dayEvents.length - 3} more</div>
													{/if}
												</div>
											{/if}
										</button>
									{/each}
								</div>
							{/each}
						</div>
					</div>
				{:else if viewMode === 'week'}
					<!-- Week View -->
					<div class="h-full flex flex-col">
						<!-- Day headers -->
						<div class="grid grid-cols-[60px_repeat(7,1fr)] border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
							<div class="border-r border-gx-border-default"></div>
							{#each weekDays as day}
								<div class="text-center py-1.5 border-r border-gx-border-default last:border-r-0">
									<div class="text-[10px] text-gx-text-muted">{day.toLocaleDateString('en-US', { weekday: 'short' })}</div>
									<div class="text-sm font-medium
										{isToday(day) ? 'text-gx-neon' : 'text-gx-text-secondary'}">
										{day.getDate()}
									</div>
								</div>
							{/each}
						</div>

						<!-- All-day events row -->
						{#if weekDays.some(d => getEventsForDate(formatDateStr(d)).some(e => e.all_day))}
							<div class="grid grid-cols-[60px_repeat(7,1fr)] border-b border-gx-border-default bg-gx-bg-secondary/50 shrink-0">
								<div class="border-r border-gx-border-default text-[9px] text-gx-text-muted flex items-center justify-center">ALL DAY</div>
								{#each weekDays as weekDay}
									{@const dayAllDay = getEventsForDate(formatDateStr(weekDay)).filter(e => e.all_day)}
									<div class="border-r border-gx-border-default last:border-r-0 p-0.5 space-y-px">
										{#each dayAllDay.slice(0, 2) as evt (evt.id)}
											<button onclick={() => openEditEventDialog(evt)}
												class="w-full text-[9px] px-1 py-px rounded truncate text-white/90 text-left"
												style="background-color: {getEventColor(evt)};">
												{evt.title}
											</button>
										{/each}
									</div>
								{/each}
							</div>
						{/if}

						<!-- Hour grid -->
						<div class="flex-1 overflow-y-auto">
							<div class="grid grid-cols-[60px_repeat(7,1fr)] relative">
								{#each hours as hour}
									<div class="border-r border-b border-gx-border-default h-12 flex items-start justify-end pr-1.5 pt-0.5">
										<span class="text-[9px] text-gx-text-muted">{hour === 0 ? '12 AM' : hour < 12 ? `${hour} AM` : hour === 12 ? '12 PM' : `${hour - 12} PM`}</span>
									</div>
									{#each weekDays as day}
										{@const dateStr = formatDateStr(day)}
										{@const hourEvents = getEventsForDate(dateStr).filter(e => {
											if (e.all_day) return false;
											const h = parseInt(e.start.substring(11, 13) || '0');
											return h === hour;
										})}
										<div role="gridcell" tabindex="0"
											onclick={() => { currentDate = new Date(day); openNewEventDialog(dateStr); }}
											onkeydown={(e) => { if (e.key === 'Enter') { currentDate = new Date(day); openNewEventDialog(dateStr); } }}
											class="border-r border-b border-gx-border-default last:border-r-0 h-12 p-px relative hover:bg-gx-bg-hover/30 transition-colors cursor-pointer
												{isToday(day) ? 'bg-gx-neon/5' : ''}">
											{#each hourEvents as evt (evt.id)}
												<button onclick={(e) => { e.stopPropagation(); openEditEventDialog(evt); }}
													class="absolute left-px right-px text-[9px] px-1 py-px rounded text-white/90 truncate z-10 hover:opacity-90"
													style="background-color: {getEventColor(evt)}; top: 0;">
													{formatTime(evt.start)} {evt.title}
												</button>
											{/each}
										</div>
									{/each}
								{/each}
							</div>
						</div>
					</div>
				{:else}
					<!-- Day View -->
					<div class="h-full flex flex-col">
						<!-- Day info bar -->
						<div class="px-4 py-2 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
							<div class="flex items-center gap-2">
								<span class="text-2xl font-bold {isToday(currentDate) ? 'text-gx-neon' : 'text-gx-text-primary'}">
									{currentDate.getDate()}
								</span>
								<div>
									<div class="text-xs text-gx-text-secondary">{currentDate.toLocaleDateString('en-US', { weekday: 'long' })}</div>
									<div class="text-[10px] text-gx-text-muted">{getEventsForDate(selectedDayStr).length} events</div>
								</div>
								{#if isToday(currentDate)}
									<Badge variant="outline" class="text-[9px] px-1.5 py-0 h-4 border-gx-neon/50 text-gx-neon">Today</Badge>
								{/if}
							</div>
						</div>

						<!-- All-day events -->
						{#if getEventsForDate(selectedDayStr).some(e => e.all_day)}
							<div class="px-4 py-1.5 border-b border-gx-border-default bg-gx-bg-secondary/50">
								<div class="flex items-center gap-2 flex-wrap">
									{#each getEventsForDate(selectedDayStr).filter(e => e.all_day) as evt (evt.id)}
										<button onclick={() => openEditEventDialog(evt)}
											class="text-[11px] px-2 py-0.5 rounded text-white/90 hover:opacity-80 transition-opacity"
											style="background-color: {getEventColor(evt)};">
											{evt.title}
										</button>
									{/each}
								</div>
							</div>
						{/if}

						<!-- Hour slots -->
						<div class="flex-1 overflow-y-auto">
							{#each hours as hour}
								{@const hourEvents = getEventsForDate(selectedDayStr).filter(e => {
									if (e.all_day) return false;
									const h = parseInt(e.start.substring(11, 13) || '0');
									return h === hour;
								})}
								<div class="flex border-b border-gx-border-default min-h-[48px]">
									<div class="w-16 shrink-0 flex items-start justify-end pr-2 pt-1 border-r border-gx-border-default">
										<span class="text-[10px] text-gx-text-muted">
											{hour === 0 ? '12 AM' : hour < 12 ? `${hour} AM` : hour === 12 ? '12 PM' : `${hour - 12} PM`}
										</span>
									</div>
									<div class="flex-1 p-1 space-y-0.5 relative">
										{#each hourEvents as evt (evt.id)}
											<button onclick={() => openEditEventDialog(evt)}
												class="w-full text-left px-2 py-1 rounded text-white/90 hover:opacity-90 transition-opacity flex items-start gap-2"
												style="background-color: {getEventColor(evt)};">
												<div class="flex-1 min-w-0">
													<div class="text-xs font-medium truncate">{evt.title}</div>
													<div class="text-[10px] opacity-80">
														{formatTime(evt.start)} - {formatTime(evt.end)}
														{#if evt.location}
															<span class="ml-1">| {evt.location}</span>
														{/if}
													</div>
												</div>
											</button>
										{/each}
										{#if hourEvents.length === 0}
											<button onclick={() => openNewEventDialog()}
												class="w-full h-full min-h-[36px] rounded-gx hover:bg-gx-bg-hover/50 transition-colors text-[10px] text-gx-text-muted/0 hover:text-gx-text-muted flex items-center justify-center">
												+ Add event
											</button>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>

			<!-- AI Panel (right sidebar) -->
			{#if aiPanelOpen}
				<aside class="w-72 border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 overflow-y-auto">
					<div class="flex items-center gap-2 h-9 px-3 border-b border-gx-border-default shrink-0">
						<Sparkles size={14} class="text-gx-neon" />
						<span class="text-xs font-medium text-gx-text-secondary">AI Calendar Assistant</span>
						<div class="flex-1"></div>
						<button onclick={() => aiPanelOpen = false} class="text-gx-text-muted hover:text-gx-neon transition-colors">
							<X size={14} />
						</button>
					</div>

					<div class="p-3 space-y-3 flex-1">
						<!-- Daily Briefing -->
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
							<div class="flex items-center justify-between mb-2">
								<div class="flex items-center gap-1.5">
									<Sun size={13} class="text-gx-accent-yellow" />
									<span class="text-xs font-medium text-gx-text-secondary">Daily Briefing</span>
								</div>
								<button onclick={loadDailyBriefing}
									disabled={aiLoading}
									class="text-[10px] text-gx-neon hover:underline disabled:opacity-50">
									{aiLoading ? 'Loading...' : 'Generate'}
								</button>
							</div>
							{#if aiBriefing}
								<p class="text-[11px] text-gx-text-secondary leading-relaxed whitespace-pre-wrap">{aiBriefing}</p>
							{:else}
								<p class="text-[10px] text-gx-text-muted italic">Click Generate for today's briefing</p>
							{/if}
						</div>

						<!-- Find Free Time -->
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
							<div class="flex items-center gap-1.5 mb-2">
								<Clock size={13} class="text-gx-accent-blue" />
								<span class="text-xs font-medium text-gx-text-secondary">Find Free Time</span>
							</div>
							<div class="space-y-1.5 mb-2">
								<div class="flex items-center gap-1.5">
									<label for="cal-find-duration" class="text-[10px] text-gx-text-muted w-16">Duration</label>
									<select id="cal-find-duration" bind:value={findDuration}
										class="flex-1 bg-gx-bg-tertiary border border-gx-border-default rounded px-1.5 py-0.5 text-[10px] text-gx-text-secondary">
										<option value={15}>15 min</option>
										<option value={30}>30 min</option>
										<option value={60}>1 hour</option>
										<option value={90}>1.5 hours</option>
										<option value={120}>2 hours</option>
									</select>
								</div>
								<div class="flex items-center gap-1.5">
									<label for="cal-find-hours" class="text-[10px] text-gx-text-muted w-16">Hours</label>
									<input id="cal-find-hours" bind:value={findPreferredHours}
										class="flex-1 bg-gx-bg-tertiary border border-gx-border-default rounded px-1.5 py-0.5 text-[10px] text-gx-text-secondary"
										placeholder="9-17" />
								</div>
							</div>
							<button onclick={findFreeTime}
								disabled={aiLoading}
								class="w-full px-2 py-1 text-[10px] bg-gx-neon/10 border border-gx-neon/30 text-gx-neon rounded-gx hover:bg-gx-neon/20 transition-colors disabled:opacity-50">
								{aiLoading ? 'Searching...' : 'Find Slots'}
							</button>
							{#if aiSuggestions.length > 0}
								<div class="mt-2 space-y-1">
									{#each aiSuggestions as slot, i}
										<button onclick={() => {
											const dateStr = slot.start.substring(0, 10);
											eventForm.start = slot.start.substring(0, 16);
											eventForm.end = slot.end.substring(0, 16);
											openNewEventDialog(dateStr);
										}}
											class="w-full text-left px-2 py-1.5 rounded-gx border border-gx-border-default hover:border-gx-neon/30 transition-colors">
											<div class="flex items-center gap-1">
												<span class="text-[10px] font-medium text-gx-neon">#{i + 1}</span>
												<span class="text-[10px] text-gx-text-secondary">{formatTime(slot.start)} - {formatTime(slot.end)}</span>
											</div>
											<div class="text-[9px] text-gx-text-muted">{slot.reason} ({Math.round(slot.score * 100)}% match)</div>
										</button>
									{/each}
								</div>
							{/if}
						</div>

						<!-- Generate Agenda -->
						<div class="rounded-gx border border-gx-border-default bg-gx-bg-primary p-3">
							<div class="flex items-center gap-1.5 mb-2">
								<FileText size={13} class="text-gx-accent-magenta" />
								<span class="text-xs font-medium text-gx-text-secondary">Meeting Agenda</span>
							</div>
							<p class="text-[10px] text-gx-text-muted mb-2">Select an event to generate a professional agenda.</p>
							{#if getEventsForDate(selectedDayStr).filter(e => !e.all_day).length > 0}
								<div class="space-y-0.5 mb-2">
									{#each getEventsForDate(selectedDayStr).filter(e => !e.all_day) as evt (evt.id)}
										<button onclick={() => generateAgenda(evt.id)}
											disabled={aiLoading}
											class="w-full text-left px-2 py-1 rounded-gx hover:bg-gx-bg-hover transition-colors text-[10px] text-gx-text-secondary disabled:opacity-50">
											{formatTime(evt.start)} {evt.title}
										</button>
									{/each}
								</div>
							{:else}
								<p class="text-[10px] text-gx-text-muted italic">No timed events for the selected day.</p>
							{/if}
							{#if aiAgenda}
								<div class="mt-2 p-2 rounded bg-gx-bg-tertiary">
									<pre class="text-[10px] text-gx-text-secondary whitespace-pre-wrap leading-relaxed">{aiAgenda}</pre>
								</div>
							{/if}
						</div>
					</div>
				</aside>
			{/if}
		</div>
	</div>
</div>

<!-- Event Dialog Modal -->
{#if showEventDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onclick={() => showEventDialog = false} onkeydown={(e) => { if (e.key === "Escape") showEventDialog = false; }} role="dialog" aria-modal="true" tabindex="-1">
		<div class="w-[480px] max-h-[85vh] bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg overflow-y-auto"
			onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="document">
			<div class="flex items-center justify-between px-4 py-3 border-b border-gx-border-default">
				<h2 class="text-sm font-semibold text-gx-text-primary">
					{editingEvent ? 'Edit Event' : 'New Event'}
				</h2>
				<button onclick={() => showEventDialog = false} class="text-gx-text-muted hover:text-gx-neon" aria-label="Close dialog">
					<X size={16} />
				</button>
			</div>

			<div class="p-4 space-y-3">
				<!-- Title -->
				<div>
					<label for="cal-evt-title" class="text-[11px] text-gx-text-muted block mb-1">Title</label>
					<input id="cal-evt-title" bind:value={eventForm.title}
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-sm text-gx-text-primary focus:border-gx-neon focus:outline-none"
						placeholder="Event title" />
				</div>

				<!-- Date/Time -->
				<div class="grid grid-cols-2 gap-3">
					<div>
						<label for="cal-evt-start" class="text-[11px] text-gx-text-muted block mb-1">Start</label>
						<input id="cal-evt-start" type={eventForm.all_day ? 'date' : 'datetime-local'} bind:value={eventForm.start}
							class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none" />
					</div>
					<div>
						<label for="cal-evt-end" class="text-[11px] text-gx-text-muted block mb-1">End</label>
						<input id="cal-evt-end" type={eventForm.all_day ? 'date' : 'datetime-local'} bind:value={eventForm.end}
							class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none" />
					</div>
				</div>

				<!-- All-day toggle -->
				<label class="flex items-center gap-2 cursor-pointer">
					<input type="checkbox" bind:checked={eventForm.all_day}
						class="rounded border-gx-border-default text-gx-neon focus:ring-gx-neon" />
					<span class="text-[11px] text-gx-text-secondary">All-day event</span>
				</label>

				<!-- Location -->
				<div>
					<label class="text-[11px] text-gx-text-muted block mb-1">
						<span class="flex items-center gap-1"><MapPin size={10} /> Location</span>
					</label>
					<input bind:value={eventForm.location}
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none"
						placeholder="Conference Room, Zoom link, etc." />
				</div>

				<!-- Description -->
				<div>
					<label for="cal-evt-desc" class="text-[11px] text-gx-text-muted block mb-1">Description</label>
					<textarea id="cal-evt-desc" bind:value={eventForm.description} rows="3"
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none resize-none"
						placeholder="Notes about this event..."></textarea>
				</div>

				<!-- Calendar selector -->
				<div>
					<label for="cal-evt-calendar" class="text-[11px] text-gx-text-muted block mb-1">Calendar</label>
					<select id="cal-evt-calendar" bind:value={eventForm.calendar_id}
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none">
						{#each calendars as cal (cal.id)}
							<option value={cal.id}>{cal.name}</option>
						{/each}
					</select>
				</div>

				<!-- Color -->
				<div>
					<label class="text-[11px] text-gx-text-muted block mb-1">
						<span class="flex items-center gap-1"><Palette size={10} /> Color (optional)</span>
					</label>
					<div class="flex items-center gap-1.5">
						{#each colorPresets as c}
							<button onclick={() => eventForm.color = c}
								aria-label="Select color" class="w-5 h-5 rounded-full border-2 transition-transform hover:scale-110
									{eventForm.color === c ? 'border-white scale-110' : 'border-transparent'}"
								style="background-color: {c};">
							</button>
						{/each}
						<button onclick={() => eventForm.color = ''}
							class="w-5 h-5 rounded-full border border-gx-border-default flex items-center justify-center text-[8px] text-gx-text-muted hover:border-gx-neon"
							title="Use calendar default"
							aria-label="Use calendar default">
							<X size={10} />
						</button>
					</div>
				</div>

				<!-- Reminder -->
				<div>
					<label class="text-[11px] text-gx-text-muted block mb-1">
						<span class="flex items-center gap-1"><Bell size={10} /> Reminder</span>
					</label>
					<select bind:value={eventForm.reminder_minutes}
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none">
						<option value={null}>None</option>
						<option value={5}>5 minutes before</option>
						<option value={15}>15 minutes before</option>
						<option value={30}>30 minutes before</option>
						<option value={60}>1 hour before</option>
						<option value={1440}>1 day before</option>
					</select>
				</div>

				<!-- Attendees -->
				<div>
					<label class="text-[11px] text-gx-text-muted block mb-1">
						<span class="flex items-center gap-1"><Users size={10} /> Attendees</span>
					</label>
					<input bind:value={eventForm.attendees}
						class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none"
						placeholder="email1@example.com, email2@example.com" />
				</div>
			</div>

			<!-- Dialog footer -->
			<div class="flex items-center justify-between px-4 py-3 border-t border-gx-border-default">
				{#if editingEvent}
					<button onclick={() => deleteEvent(editingEvent!.id)}
						class="flex items-center gap-1 px-3 py-1.5 text-xs text-gx-status-error hover:bg-gx-status-error/10 rounded-gx transition-colors">
						<Trash2 size={12} />
						Delete
					</button>
				{:else}
					<div></div>
				{/if}
				<div class="flex items-center gap-2">
					<button onclick={() => showEventDialog = false}
						class="px-3 py-1.5 text-xs text-gx-text-muted hover:text-gx-text-secondary border border-gx-border-default rounded-gx transition-colors">
						Cancel
					</button>
					<button onclick={saveEvent}
						class="flex items-center gap-1 px-4 py-1.5 text-xs bg-gx-neon text-gx-bg-primary rounded-gx font-medium hover:opacity-90 transition-opacity">
						<Check size={12} />
						{editingEvent ? 'Update' : 'Create'}
					</button>
				</div>
			</div>
		</div>
	</div>
{/if}

<!-- Add Calendar Dialog -->
{#if showAddCalendar}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onclick={() => showAddCalendar = false} onkeydown={(e) => { if (e.key === "Escape") showAddCalendar = false; }} role="dialog" aria-modal="true" tabindex="-1">
		<div class="w-[400px] bg-gx-bg-elevated border border-gx-border-default rounded-gx shadow-gx-glow-lg"
			onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="document">
			<div class="flex items-center justify-between px-4 py-3 border-b border-gx-border-default">
				<h2 class="text-sm font-semibold text-gx-text-primary">Add Calendar</h2>
				<button onclick={() => showAddCalendar = false} class="text-gx-text-muted hover:text-gx-neon" aria-label="Close dialog">
					<X size={16} />
				</button>
			</div>

			<!-- Mode tabs -->
			<div class="flex border-b border-gx-border-default">
				<button onclick={() => addCalendarMode = 'local'}
					class="flex-1 px-3 py-2 text-xs text-center transition-colors
						{addCalendarMode === 'local'
							? 'text-gx-neon border-b-2 border-gx-neon'
							: 'text-gx-text-muted hover:text-gx-text-secondary'}">
					<CalendarIcon size={12} class="inline mr-1" />
					New Calendar
				</button>
				<button onclick={() => addCalendarMode = 'url'}
					class="flex-1 px-3 py-2 text-xs text-center transition-colors
						{addCalendarMode === 'url'
							? 'text-gx-neon border-b-2 border-gx-neon'
							: 'text-gx-text-muted hover:text-gx-text-secondary'}">
					<Link size={12} class="inline mr-1" />
					Import URL
				</button>
				<button onclick={() => addCalendarMode = 'file'}
					class="flex-1 px-3 py-2 text-xs text-center transition-colors
						{addCalendarMode === 'file'
							? 'text-gx-neon border-b-2 border-gx-neon'
							: 'text-gx-text-muted hover:text-gx-text-secondary'}">
					<Upload size={12} class="inline mr-1" />
					Import File
				</button>
			</div>

			<div class="p-4">
				{#if addCalendarMode === 'local'}
					<div class="space-y-3">
						<div>
							<label for="cal-new-name" class="text-[11px] text-gx-text-muted block mb-1">Calendar Name</label>
							<input id="cal-new-name" bind:value={newCalendarName}
								class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-sm text-gx-text-primary focus:border-gx-neon focus:outline-none"
								placeholder="Work, Personal, etc." />
						</div>
						<div>
							<span class="text-[11px] text-gx-text-muted block mb-1">Color</span>
							<div class="flex items-center gap-2">
								{#each colorPresets as c}
									<button onclick={() => newCalendarColor = c}
										class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110
											{newCalendarColor === c ? 'border-white scale-110' : 'border-transparent'}"
										style="background-color: {c};">
									</button>
								{/each}
							</div>
						</div>
						<button onclick={createCalendar}
							class="w-full px-3 py-2 text-xs bg-gx-neon text-gx-bg-primary rounded-gx font-medium hover:opacity-90 transition-opacity">
							Create Calendar
						</button>
					</div>
				{:else if addCalendarMode === 'url'}
					<div class="space-y-3">
						<div>
							<label for="cal-import-url" class="text-[11px] text-gx-text-muted block mb-1">ICS / iCal URL</label>
							<input id="cal-import-url" bind:value={importUrl}
								class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none"
								placeholder="https://calendar.google.com/calendar/ical/..." />
						</div>
						<div class="text-[10px] text-gx-text-muted space-y-0.5">
							<p>Supported sources:</p>
							<ul class="list-disc list-inside ml-1">
								<li>Google Calendar (Settings > Calendar > Secret ICS address)</li>
								<li>Outlook / Office 365 (Settings > Calendar > Shared calendars)</li>
								<li>Apple iCloud Calendar (Sharing link)</li>
								<li>Any standard .ics / iCal URL</li>
							</ul>
						</div>
						<button onclick={importFromUrl}
							disabled={importLoading || !importUrl.trim()}
							class="w-full flex items-center justify-center gap-1 px-3 py-2 text-xs bg-gx-neon text-gx-bg-primary rounded-gx font-medium hover:opacity-90 transition-opacity disabled:opacity-50">
							{#if importLoading}
								<Loader2 size={12} class="animate-spin" />
								Importing...
							{:else}
								<Download size={12} />
								Import Calendar
							{/if}
						</button>
					</div>
				{:else}
					<div class="space-y-3">
						<div>
							<label for="cal-import-file" class="text-[11px] text-gx-text-muted block mb-1">ICS File Path</label>
							<input id="cal-import-file" bind:value={importUrl}
								class="w-full bg-gx-bg-tertiary border border-gx-border-default rounded-gx px-3 py-1.5 text-xs text-gx-text-primary focus:border-gx-neon focus:outline-none"
								placeholder="/path/to/calendar-export.ics" />
						</div>
						<p class="text-[10px] text-gx-text-muted">
							Export a .ics file from Google Calendar, Outlook, or Apple Calendar and provide the file path above.
						</p>
						<button onclick={importFromUrl}
							disabled={importLoading || !importUrl.trim()}
							class="w-full flex items-center justify-center gap-1 px-3 py-2 text-xs bg-gx-neon text-gx-bg-primary rounded-gx font-medium hover:opacity-90 transition-opacity disabled:opacity-50">
							{#if importLoading}
								<Loader2 size={12} class="animate-spin" />
								Importing...
							{:else}
								<Upload size={12} />
								Import File
							{/if}
						</button>
					</div>
				{/if}
			</div>
		</div>
	</div>
{/if}

<!-- Error toast -->
{#if error}
	<div class="fixed bottom-4 right-4 z-50 max-w-sm bg-gx-status-error/15 border border-gx-status-error/40 rounded-gx px-4 py-3 shadow-lg">
		<div class="flex items-start gap-2">
			<AlertCircle size={14} class="text-gx-status-error shrink-0 mt-0.5" />
			<div class="flex-1 min-w-0">
				<p class="text-xs text-gx-status-error">{error}</p>
			</div>
			<button onclick={() => error = ''} class="text-gx-status-error/70 hover:text-gx-status-error shrink-0">
				<X size={12} />
			</button>
		</div>
	</div>
{/if}
