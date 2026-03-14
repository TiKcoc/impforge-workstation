<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		Users, Wifi, WifiOff, Copy, Check, Plus,
		LogIn, LogOut, Eye, RefreshCw, Radio
	} from '@lucide/svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// -----------------------------------------------------------------------
	// Types
	// -----------------------------------------------------------------------

	interface PeerInfo {
		user_id: string;
		user_name: string;
		color: string;
		cursor_line: number;
		cursor_column: number;
		cursor_file: string;
		last_seen: number;
	}

	interface CollabRoom {
		room_id: string;
		name: string;
		peers: PeerInfo[];
		file_path: string;
	}

	interface CollabStatus {
		active_rooms: number;
		total_peers: number;
		total_operations: number;
		pending_sync: number;
		rooms: Array<{
			room_id: string;
			file: string;
			peers: number;
			version: number;
			history_size: number;
			pending_ops: number;
			connected: boolean;
		}>;
		transport: string;
		relay_connected: boolean;
	}

	interface Props {
		currentFile?: string;
		cursorLine?: number;
		cursorColumn?: number;
	}

	// -----------------------------------------------------------------------
	// Props & State
	// -----------------------------------------------------------------------

	let { currentFile = '', cursorLine = 1, cursorColumn = 1 }: Props = $props();

	// BenikUI style engine
	const widgetId = 'ide-collab';
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
	let peerListComp = $derived(styleEngine.getComponentStyle(widgetId, 'peer-list'));
	let peerListStyle = $derived(hasEngineStyle && peerListComp ? componentToCSS(peerListComp) : '');
	let activityComp = $derived(styleEngine.getComponentStyle(widgetId, 'activity'));
	let activityStyle = $derived(hasEngineStyle && activityComp ? componentToCSS(activityComp) : '');

	let activeTab = $state<'rooms' | 'peers'>('rooms');
	let rooms = $state<CollabRoom[]>([]);
	let currentRoom = $state<CollabRoom | null>(null);
	let peers = $state<PeerInfo[]>([]);
	let status = $state<CollabStatus | null>(null);
	let loading = $state(false);
	let error = $state('');
	let copied = $state(false);
	let joinCode = $state('');
	let userName = $state('');
	let showJoinInput = $state(false);
	let showCreateInput = $state(false);
	let pollInterval = $state<ReturnType<typeof setInterval> | null>(null);

	// Derived
	const isConnected = $derived(currentRoom !== null);
	const peerCount = $derived(peers.length);
	const otherPeers = $derived(peers.filter((p) => !p.user_id.startsWith('local')));

	// The color palette matches the Rust backend
	const PEER_COLORS = [
		'#ff5370', '#c3e88d', '#82aaff', '#c792ea',
		'#ffcb6b', '#89ddff', '#f78c6c', '#b2ccd6',
		'#ff9cac', '#a8e4a0'
	];

	// -----------------------------------------------------------------------
	// Effects
	// -----------------------------------------------------------------------

	// Send cursor updates to the backend when cursor position changes
	$effect(() => {
		if (currentRoom && currentFile && cursorLine && cursorColumn) {
			invoke('collab_update_cursor', {
				roomId: currentRoom.room_id,
				line: cursorLine,
				column: cursorColumn,
				filePath: currentFile
			}).catch(() => {
				// Cursor updates are best-effort; do not surface errors
			});
		}
	});

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	onMount(() => {
		// Try to restore user name from localStorage
		try {
			const stored = localStorage.getItem('impforge-collab-username');
			if (stored) userName = stored;
		} catch {
			// localStorage may be unavailable in some Tauri configs
		}

		refreshStatus();
		// Poll for peer updates every 3 seconds while in a room
		pollInterval = setInterval(() => {
			if (currentRoom) {
				refreshPeers();
			}
			refreshStatus();
		}, 3000);
	});

	onDestroy(() => {
		if (pollInterval) {
			clearInterval(pollInterval);
			pollInterval = null;
		}
	});

	// -----------------------------------------------------------------------
	// Backend Communication
	// -----------------------------------------------------------------------

	async function refreshStatus() {
		try {
			status = await invoke<CollabStatus>('collab_status');
			rooms = await invoke<CollabRoom[]>('collab_get_rooms');
		} catch (e) {
			// Status polling failures are non-fatal
			console.warn('Collab status poll failed:', e);
		}
	}

	async function refreshPeers() {
		if (!currentRoom) return;
		try {
			peers = await invoke<PeerInfo[]>('collab_get_peers', {
				roomId: currentRoom.room_id
			});
		} catch (e) {
			console.warn('Peer refresh failed:', e);
		}
	}

	async function createRoom() {
		if (!userName.trim()) {
			error = 'Enter your name first';
			return;
		}
		if (!currentFile) {
			error = 'Open a file to start collaborating';
			return;
		}

		error = '';
		loading = true;
		try {
			persistUserName();
			const room = await invoke<CollabRoom>('collab_create_room', {
				filePath: currentFile,
				userName: userName.trim()
			});
			currentRoom = room;
			peers = room.peers;
			showCreateInput = false;
			activeTab = 'peers';
		} catch (e) {
			error = `Failed to create room: ${e}`;
		}
		loading = false;
	}

	async function joinRoom() {
		if (!userName.trim()) {
			error = 'Enter your name first';
			return;
		}
		if (!joinCode.trim()) {
			error = 'Enter a room code';
			return;
		}

		error = '';
		loading = true;
		try {
			persistUserName();
			const room = await invoke<CollabRoom>('collab_join_room', {
				roomId: joinCode.trim(),
				userName: userName.trim()
			});
			currentRoom = room;
			peers = room.peers;
			joinCode = '';
			showJoinInput = false;
			activeTab = 'peers';
		} catch (e) {
			error = `Failed to join room: ${e}`;
		}
		loading = false;
	}

	async function leaveRoom() {
		if (!currentRoom) return;
		try {
			await invoke('collab_leave_room', { roomId: currentRoom.room_id });
		} catch (e) {
			console.warn('Leave room error:', e);
		}
		currentRoom = null;
		peers = [];
		activeTab = 'rooms';
	}

	// -----------------------------------------------------------------------
	// Helpers
	// -----------------------------------------------------------------------

	function persistUserName() {
		try {
			localStorage.setItem('impforge-collab-username', userName.trim());
		} catch {
			// Best-effort persistence
		}
	}

	async function copyRoomCode() {
		if (!currentRoom) return;
		try {
			await navigator.clipboard.writeText(currentRoom.room_id);
			copied = true;
			setTimeout(() => { copied = false; }, 2000);
		} catch {
			// Clipboard API may not be available
			error = 'Failed to copy to clipboard';
		}
	}

	function getInitial(name: string): string {
		return (name.charAt(0) || '?').toUpperCase();
	}

	function getFileName(path: string): string {
		return path.split('/').pop() || path;
	}

	function timeSince(epoch: number): string {
		const now = Math.floor(Date.now() / 1000);
		const diff = now - epoch;
		if (diff < 5) return 'now';
		if (diff < 60) return `${diff}s ago`;
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		return `${Math.floor(diff / 3600)}h ago`;
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-2 px-2 py-1.5 border-b border-gx-border-default shrink-0" style={headerStyle}>
		{#if isConnected}
			<Wifi size={14} class="text-gx-neon" />
		{:else}
			<WifiOff size={14} class="text-gx-text-muted" />
		{/if}
		<span class="text-xs font-semibold text-gx-text-secondary">
			{#if isConnected}
				Live ({peerCount})
			{:else}
				Collaboration
			{/if}
		</span>
		<div class="flex-1"></div>
		{#if isConnected}
			<button
				onclick={refreshPeers}
				class="p-0.5 text-gx-text-muted hover:text-gx-neon transition-colors"
				title="Refresh peers"
			>
				<RefreshCw size={12} />
			</button>
		{/if}
	</div>

	<!-- Tab selector (only when connected) -->
	{#if isConnected}
		<div class="flex border-b border-gx-border-default shrink-0">
			<button
				onclick={() => activeTab = 'peers'}
				class="flex-1 py-1.5 text-xs text-center transition-colors
					{activeTab === 'peers' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				Peers ({peerCount})
			</button>
			<button
				onclick={() => activeTab = 'rooms'}
				class="flex-1 py-1.5 text-xs text-center transition-colors
					{activeTab === 'rooms' ? 'text-gx-neon border-b-2 border-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}"
			>
				Rooms
			</button>
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 overflow-auto text-xs">

		<!-- ================================================================ -->
		<!-- CONNECTED: Peers Tab                                             -->
		<!-- ================================================================ -->
		{#if isConnected && activeTab === 'peers'}

			<!-- Room info bar -->
			<div class="flex items-center gap-2 px-2 py-1.5 bg-gx-bg-secondary border-b border-gx-border-default">
				<Radio size={10} class="text-gx-neon animate-pulse" />
				<span class="text-[10px] text-gx-text-muted font-mono truncate flex-1">
					{currentRoom?.room_id}
				</span>
				<button
					onclick={copyRoomCode}
					class="p-0.5 text-gx-text-muted hover:text-gx-neon transition-colors"
					title="Copy room code"
				>
					{#if copied}
						<Check size={12} class="text-gx-status-success" />
					{:else}
						<Copy size={12} />
					{/if}
				</button>
			</div>

			<!-- Peer list -->
			{#each peers as peer}
				<div class="flex items-center gap-2 px-2 py-1.5 hover:bg-gx-bg-hover transition-colors group" style={peerListStyle}>
					<!-- Avatar circle -->
					<div
						class="w-6 h-6 rounded-full flex items-center justify-center text-[10px] font-bold text-white shrink-0 relative"
						style="background-color: {peer.color}30; border: 1.5px solid {peer.color};"
					>
						{getInitial(peer.user_name)}
						<!-- Online indicator dot -->
						<div
							class="absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full border border-gx-bg-primary bg-gx-neon"
						></div>
					</div>

					<!-- Name and cursor info -->
					<div class="flex-1 min-w-0">
						<div class="text-gx-text-secondary font-medium truncate">{peer.user_name}</div>
						<div class="flex items-center gap-1.5 text-[10px] text-gx-text-muted">
							<span class="truncate">{getFileName(peer.cursor_file)}</span>
							<span class="font-mono shrink-0" style="color: {peer.color};">
								Ln {peer.cursor_line}, Col {peer.cursor_column}
							</span>
						</div>
					</div>

					<!-- Last seen -->
					<span class="text-[10px] text-gx-text-muted shrink-0">
						{timeSince(peer.last_seen)}
					</span>
				</div>
			{/each}

			<!-- File being edited -->
			{#if currentRoom}
				<div class="mt-2 px-2 py-1.5 border-t border-gx-border-default">
					<div class="text-[10px] text-gx-text-muted mb-1">Shared File</div>
					<div class="text-[11px] text-gx-text-muted font-mono truncate">
						{currentRoom.file_path}
					</div>
				</div>
			{/if}

			<!-- Leave button -->
			<div class="p-2 border-t border-gx-border-default mt-auto">
				<button
					onclick={leaveRoom}
					class="w-full flex items-center justify-center gap-1.5 py-1.5 rounded text-xs
						text-gx-status-error bg-gx-status-error/10 hover:bg-gx-status-error/20 transition-colors"
				>
					<LogOut size={12} />
					Leave Room
				</button>
			</div>

		<!-- ================================================================ -->
		<!-- CONNECTED: Rooms Tab (or NOT CONNECTED default view)             -->
		<!-- ================================================================ -->
		{:else}

			<!-- Username input (always visible when not connected) -->
			{#if !isConnected}
				<div class="px-2 py-2 border-b border-gx-border-default">
					<label for="collab-username" class="text-[10px] text-gx-text-muted block mb-1">Your Name</label>
					<input
						id="collab-username"
						type="text"
						bind:value={userName}
						placeholder="Enter your name..."
						class="w-full bg-gx-bg-secondary border border-gx-border-default rounded px-2 py-1 text-xs text-gx-text-primary
							placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none transition-colors"
					/>
				</div>
			{/if}

			<!-- Action buttons -->
			{#if !isConnected}
				<div class="flex gap-1.5 px-2 py-2 border-b border-gx-border-default">
					<button
						onclick={() => { showCreateInput = true; showJoinInput = false; }}
						class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded text-xs transition-colors
							{showCreateInput
								? 'text-gx-bg-primary bg-gx-neon font-semibold'
								: 'text-gx-text-muted bg-gx-bg-elevated hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
					>
						<Plus size={12} />
						Create
					</button>
					<button
						onclick={() => { showJoinInput = true; showCreateInput = false; }}
						class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded text-xs transition-colors
							{showJoinInput
								? 'text-gx-bg-primary bg-gx-neon font-semibold'
								: 'text-gx-text-muted bg-gx-bg-elevated hover:bg-gx-bg-hover hover:text-gx-text-secondary'}"
					>
						<LogIn size={12} />
						Join
					</button>
				</div>
			{/if}

			<!-- Create room form -->
			{#if showCreateInput && !isConnected}
				<div class="px-2 py-2 border-b border-gx-border-default bg-gx-bg-secondary">
					<div class="text-[10px] text-gx-text-muted mb-1.5">
						Create a room for: <span class="text-gx-text-muted font-mono">{getFileName(currentFile || 'no file open')}</span>
					</div>
					<button
						onclick={createRoom}
						disabled={loading || !userName.trim()}
						class="w-full py-1.5 rounded text-xs font-semibold transition-colors
							{loading || !userName.trim()
								? 'text-gx-text-muted bg-gx-bg-elevated cursor-not-allowed'
								: 'text-gx-bg-primary bg-gx-neon hover:bg-gx-neon-dim'}"
					>
						{#if loading}
							Creating...
						{:else}
							Start Collaboration
						{/if}
					</button>
				</div>
			{/if}

			<!-- Join room form -->
			{#if showJoinInput && !isConnected}
				<div class="px-2 py-2 border-b border-gx-border-default bg-gx-bg-secondary">
					<label for="collab-room-code" class="text-[10px] text-gx-text-muted block mb-1">Room Code</label>
					<div class="flex gap-1.5">
						<input
							id="collab-room-code"
							type="text"
							bind:value={joinCode}
							placeholder="e.g. a1b2c3d4"
							class="flex-1 bg-gx-bg-primary border border-gx-border-default rounded px-2 py-1 text-xs text-gx-text-primary
								font-mono placeholder:text-gx-text-muted focus:border-gx-neon focus:outline-none transition-colors"
						/>
						<button
							onclick={joinRoom}
							disabled={loading || !joinCode.trim() || !userName.trim()}
							class="px-3 py-1 rounded text-xs font-semibold transition-colors
								{loading || !joinCode.trim() || !userName.trim()
									? 'text-gx-text-muted bg-gx-bg-elevated cursor-not-allowed'
									: 'text-gx-bg-primary bg-gx-neon hover:bg-gx-neon-dim'}"
						>
							Join
						</button>
					</div>
				</div>
			{/if}

			<!-- Error message -->
			{#if error}
				<div class="px-2 py-1.5 bg-gx-status-error/10 text-gx-status-error text-[11px]">
					{error}
				</div>
			{/if}

			<!-- Active rooms list -->
			{#if rooms.length > 0}
				<div class="px-2 py-1 text-[10px] text-gx-text-muted font-semibold uppercase tracking-wider">
					Active Rooms
				</div>
				{#each rooms as room}
					<div class="flex items-center gap-2 px-2 py-1.5 hover:bg-gx-bg-hover transition-colors group">
						<div class="w-5 h-5 rounded bg-gx-neon/10 flex items-center justify-center shrink-0">
							<Users size={10} class="text-gx-neon" />
						</div>
						<div class="flex-1 min-w-0">
							<div class="text-gx-text-secondary truncate">{room.name}</div>
							<div class="flex items-center gap-1.5 text-[10px] text-gx-text-muted">
								<span class="font-mono">{room.room_id}</span>
								<span>{room.peers.length} peer{room.peers.length !== 1 ? 's' : ''}</span>
							</div>
						</div>
						<!-- Peer avatars stack -->
						<div class="flex -space-x-1.5 shrink-0">
							{#each room.peers.slice(0, 4) as peer}
								<div
									class="w-5 h-5 rounded-full flex items-center justify-center text-[8px] font-bold text-white
										border border-gx-bg-primary"
									style="background-color: {peer.color};"
									title={peer.user_name}
								>
									{getInitial(peer.user_name)}
								</div>
							{/each}
							{#if room.peers.length > 4}
								<div
									class="w-5 h-5 rounded-full flex items-center justify-center text-[8px] font-bold
										text-gx-text-muted bg-gx-bg-hover border border-gx-bg-primary"
								>
									+{room.peers.length - 4}
								</div>
							{/if}
						</div>
					</div>
				{/each}
			{/if}

			<!-- Empty state -->
			{#if rooms.length === 0 && !showCreateInput && !showJoinInput}
				<div class="flex flex-col items-center justify-center py-8 gap-2">
					<Users size={24} class="text-gx-text-disabled" />
					<p class="text-[11px] text-gx-text-muted text-center px-4">
						No active rooms. Create one to start collaborating on a file.
					</p>
				</div>
			{/if}

			<!-- Status footer -->
			{#if status}
				<div class="mt-auto px-2 py-1.5 border-t border-gx-border-default text-[10px] text-gx-text-muted" style={activityStyle}>
					<div class="flex items-center justify-between">
						<span>{status.active_rooms} room{status.active_rooms !== 1 ? 's' : ''}</span>
						<span>{status.total_peers} peer{status.total_peers !== 1 ? 's' : ''}</span>
						<span>{status.total_operations} ops</span>
					</div>
					<div class="flex items-center gap-1 mt-0.5">
						<Eye size={8} class="text-gx-text-disabled" />
						<span>Transport: {status.transport}</span>
						{#if status.pending_sync > 0}
							<span class="text-gx-status-warning"> | {status.pending_sync} pending</span>
						{/if}
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>
