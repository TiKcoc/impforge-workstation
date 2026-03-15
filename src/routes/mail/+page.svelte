<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';
	import {
		Mail, MailPlus, MailOpen, MailCheck,
		Inbox, Send, FileText, Star, Trash2, Archive,
		Search, Loader2, AlertCircle, X, ChevronDown,
		Sparkles, Reply, Forward, RefreshCw,
		User, Clock, Tag, MoreVertical,
		Plus, Settings, Globe, ArrowLeft,
		Copy, Check, Wand2, ListFilter
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// ---- BenikUI Style Engine ------------------------------------------------
	const widgetId = 'page-mail';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComponent = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComponent ? componentToCSS(containerComponent) : '');

	// ---- Types ---------------------------------------------------------------
	interface EmailProvider {
		type: 'gmail' | 'outlook' | 'yahoo' | 'proton_mail' | 'custom';
		imap_host?: string;
		imap_port?: number;
		smtp_host?: string;
		smtp_port?: number;
	}

	interface EmailAccount {
		id: string;
		name: string;
		email: string;
		provider: EmailProvider;
		connected: boolean;
		created_at: string;
	}

	interface AccountWithCounts {
		account: EmailAccount;
		unread_count: number;
		total_count: number;
	}

	interface EmailListItem {
		id: string;
		account_id: string;
		from: string;
		subject: string;
		preview: string;
		date: string;
		is_read: boolean;
		is_starred: boolean;
		folder: string;
		labels: string[];
	}

	interface FullEmail {
		id: string;
		account_id: string;
		from: string;
		to: string[];
		cc: string[];
		subject: string;
		body: string;
		body_html: string;
		date: string;
		is_read: boolean;
		is_starred: boolean;
		folder: string;
		labels: string[];
	}

	type FolderCount = [string, number, number]; // [name, total, unread]

	// ---- State ---------------------------------------------------------------
	// View state
	let view = $state<'inbox' | 'compose' | 'setup'>('inbox');
	let accounts = $state<AccountWithCounts[]>([]);
	let activeAccountId = $state<string | null>(null);
	let emails = $state<EmailListItem[]>([]);
	let selectedEmail = $state<FullEmail | null>(null);
	let folderCounts = $state<FolderCount[]>([]);
	let activeFolder = $state('inbox');
	let searchQuery = $state('');
	let loading = $state(true);
	let emailsLoading = $state(false);
	let error = $state<string | null>(null);

	// Compose state
	let composeTo = $state('');
	let composeCc = $state('');
	let composeBcc = $state('');
	let composeSubject = $state('');
	let composeBody = $state('');
	let composeReplyTo = $state<string | null>(null);
	let composeSending = $state(false);

	// AI state
	let aiPanelOpen = $state(false);
	let aiLoading = $state(false);
	let aiResult = $state('');
	let aiError = $state<string | null>(null);
	let aiTone = $state('professional');
	let aiCopied = $state(false);

	// Setup wizard state
	let setupProviderType = $state<string>('gmail');
	let setupName = $state('');
	let setupEmail = $state('');
	let setupImapHost = $state('');
	let setupImapPort = $state('993');
	let setupSmtpHost = $state('');
	let setupSmtpPort = $state('587');

	// Context menu
	let contextMenuOpen = $state(false);
	let contextMenuEmailId = $state<string | null>(null);

	// ---- Derived -------------------------------------------------------------
	let activeAccount = $derived(
		accounts.find(a => a.account.id === activeAccountId) ?? null
	);

	let filteredEmails = $derived(
		searchQuery.trim()
			? emails.filter(e =>
				e.subject.toLowerCase().includes(searchQuery.toLowerCase()) ||
				e.from.toLowerCase().includes(searchQuery.toLowerCase()) ||
				e.preview.toLowerCase().includes(searchQuery.toLowerCase())
			)
			: emails
	);

	let folderIcon = $derived((folder: string) => {
		switch (folder) {
			case 'inbox': return Inbox;
			case 'sent': return Send;
			case 'drafts': return FileText;
			case 'starred': return Star;
			case 'trash': return Trash2;
			case 'archive': return Archive;
			default: return Mail;
		}
	});

	// ---- Data Loading --------------------------------------------------------
	async function loadAccounts() {
		loading = true;
		error = null;
		try {
			accounts = await invoke<AccountWithCounts[]>('mail_list_accounts');
			if (accounts.length > 0 && !activeAccountId) {
				activeAccountId = accounts[0].account.id;
				await loadEmails();
				await loadFolderCounts();
			} else if (accounts.length === 0) {
				view = 'setup';
			}
		} catch (e) {
			error = parseError(e);
		} finally {
			loading = false;
		}
	}

	async function loadEmails() {
		if (!activeAccountId) return;
		emailsLoading = true;
		try {
			emails = await invoke<EmailListItem[]>('mail_list_emails', {
				accountId: activeAccountId,
				folder: activeFolder,
				limit: 50,
			});
		} catch (e) {
			error = parseError(e);
		} finally {
			emailsLoading = false;
		}
	}

	async function loadFolderCounts() {
		if (!activeAccountId) return;
		try {
			folderCounts = await invoke<FolderCount[]>('mail_folder_counts', {
				accountId: activeAccountId,
			});
		} catch (e) {
			// Non-critical
			console.error('Failed to load folder counts:', e);
		}
	}

	async function selectFolder(folder: string) {
		activeFolder = folder;
		selectedEmail = null;
		await loadEmails();
	}

	async function openEmail(id: string) {
		try {
			selectedEmail = await invoke<FullEmail>('mail_get_email', { id });
			if (!selectedEmail.is_read) {
				await invoke('mail_mark_read', { id, isRead: true });
				// Update list item
				const idx = emails.findIndex(e => e.id === id);
				if (idx >= 0) {
					emails[idx] = { ...emails[idx], is_read: true };
				}
			}
		} catch (e) {
			error = parseError(e);
		}
	}

	async function toggleStar(id: string, currentStarred: boolean) {
		try {
			await invoke('mail_star', { id, starred: !currentStarred });
			const idx = emails.findIndex(e => e.id === id);
			if (idx >= 0) {
				emails[idx] = { ...emails[idx], is_starred: !currentStarred };
			}
			if (selectedEmail?.id === id) {
				selectedEmail = { ...selectedEmail, is_starred: !currentStarred };
			}
		} catch (e) {
			error = parseError(e);
		}
	}

	async function deleteEmail(id: string) {
		try {
			await invoke('mail_delete', { id });
			emails = emails.filter(e => e.id !== id);
			if (selectedEmail?.id === id) {
				selectedEmail = null;
			}
			await loadFolderCounts();
		} catch (e) {
			error = parseError(e);
		}
	}

	async function moveEmail(id: string, folder: string) {
		try {
			await invoke('mail_move', { id, folder });
			emails = emails.filter(e => e.id !== id);
			if (selectedEmail?.id === id) {
				selectedEmail = null;
			}
			await loadFolderCounts();
		} catch (e) {
			error = parseError(e);
		}
	}

	async function searchEmails() {
		if (!activeAccountId || !searchQuery.trim()) return;
		emailsLoading = true;
		try {
			emails = await invoke<EmailListItem[]>('mail_search', {
				accountId: activeAccountId,
				query: searchQuery,
			});
		} catch (e) {
			error = parseError(e);
		} finally {
			emailsLoading = false;
		}
	}

	// ---- Account Setup -------------------------------------------------------
	async function addAccount() {
		error = null;
		if (!setupEmail.trim()) {
			error = 'Email address is required';
			return;
		}

		let provider: EmailProvider;
		switch (setupProviderType) {
			case 'gmail':
				provider = { type: 'gmail' };
				break;
			case 'outlook':
				provider = { type: 'outlook' };
				break;
			case 'yahoo':
				provider = { type: 'yahoo' };
				break;
			case 'protonmail':
				provider = { type: 'proton_mail' };
				break;
			case 'custom':
				provider = {
					type: 'custom',
					imap_host: setupImapHost,
					imap_port: parseInt(setupImapPort) || 993,
					smtp_host: setupSmtpHost,
					smtp_port: parseInt(setupSmtpPort) || 587,
				};
				break;
			default:
				provider = { type: 'gmail' };
		}

		try {
			await invoke('mail_add_account', {
				name: setupName || setupEmail,
				email: setupEmail,
				provider,
			});
			// Reset form
			setupName = '';
			setupEmail = '';
			setupImapHost = '';
			setupSmtpHost = '';
			view = 'inbox';
			await loadAccounts();
		} catch (e) {
			error = parseError(e);
		}
	}

	async function removeAccount(id: string) {
		try {
			await invoke('mail_remove_account', { id });
			if (activeAccountId === id) {
				activeAccountId = null;
				selectedEmail = null;
				emails = [];
			}
			await loadAccounts();
		} catch (e) {
			error = parseError(e);
		}
	}

	// ---- Compose -------------------------------------------------------------
	function startCompose(replyTo?: FullEmail) {
		view = 'compose';
		composeTo = '';
		composeCc = '';
		composeBcc = '';
		composeSubject = '';
		composeBody = '';
		composeReplyTo = null;
		aiResult = '';
		aiError = null;

		if (replyTo) {
			composeTo = replyTo.from;
			composeSubject = replyTo.subject.startsWith('Re:')
				? replyTo.subject
				: `Re: ${replyTo.subject}`;
			composeReplyTo = replyTo.id;
			composeBody = `\n\n---\nOn ${formatDate(replyTo.date)}, ${replyTo.from} wrote:\n> ${replyTo.body.split('\n').join('\n> ')}`;
		}
	}

	function startForward(email: FullEmail) {
		view = 'compose';
		composeTo = '';
		composeCc = '';
		composeBcc = '';
		composeSubject = email.subject.startsWith('Fwd:')
			? email.subject
			: `Fwd: ${email.subject}`;
		composeReplyTo = null;
		composeBody = `\n\n---------- Forwarded message ----------\nFrom: ${email.from}\nDate: ${formatDate(email.date)}\nSubject: ${email.subject}\n\n${email.body}`;
		aiResult = '';
		aiError = null;
	}

	async function sendDraft() {
		if (!activeAccountId) return;
		if (!composeTo.trim()) {
			error = 'At least one recipient is required';
			return;
		}

		composeSending = true;
		error = null;

		try {
			const toList = composeTo.split(',').map(s => s.trim()).filter(Boolean);
			const ccList = composeCc ? composeCc.split(',').map(s => s.trim()).filter(Boolean) : [];
			const bccList = composeBcc ? composeBcc.split(',').map(s => s.trim()).filter(Boolean) : [];

			const result = await invoke<string>('mail_send_draft', {
				accountId: activeAccountId,
				to: toList,
				cc: ccList.length > 0 ? ccList : null,
				bcc: bccList.length > 0 ? bccList : null,
				subject: composeSubject,
				body: composeBody,
				replyTo: composeReplyTo,
			});

			// Show success and go back
			view = 'inbox';
			await loadEmails();
			await loadFolderCounts();
			console.log(result);
		} catch (e) {
			error = parseError(e);
		} finally {
			composeSending = false;
		}
	}

	// ---- AI ------------------------------------------------------------------
	async function aiCompose(action: string) {
		aiLoading = true;
		aiError = null;
		aiResult = '';
		aiPanelOpen = true;
		aiCopied = false;

		const context = action === 'reply' && selectedEmail
			? `From: ${selectedEmail.from}\nSubject: ${selectedEmail.subject}\n\n${selectedEmail.body}`
			: action === 'forward' && selectedEmail
			? `From: ${selectedEmail.from}\nSubject: ${selectedEmail.subject}\n\n${selectedEmail.body}`
			: composeBody || composeSubject || 'No context provided';

		try {
			aiResult = await invoke<string>('mail_ai_compose', {
				context,
				action,
				tone: aiTone,
			});
		} catch (e) {
			aiError = parseError(e);
		} finally {
			aiLoading = false;
		}
	}

	function applyAiResult() {
		if (!aiResult) return;
		composeBody = aiResult;
		aiPanelOpen = false;
		if (view !== 'compose') {
			view = 'compose';
		}
	}

	function copyAiResult() {
		if (!aiResult) return;
		navigator.clipboard.writeText(aiResult);
		aiCopied = true;
		setTimeout(() => { aiCopied = false; }, 2000);
	}

	async function openWebmail() {
		if (!activeAccountId) return;
		try {
			const url = await invoke<string | null>('mail_webmail_url', { accountId: activeAccountId });
			if (url) {
				window.open(url, '_blank');
			}
		} catch (e) {
			error = parseError(e);
		}
	}

	// ---- Helpers -------------------------------------------------------------
	function parseError(e: unknown): string {
		if (e instanceof Error) return e.message;
		if (typeof e === 'string') {
			try {
				const parsed = JSON.parse(e);
				return parsed.message || e;
			} catch {
				return e;
			}
		}
		return String(e);
	}

	function formatDate(dateStr: string): string {
		try {
			const d = new Date(dateStr);
			const now = new Date();
			const diffMs = now.getTime() - d.getTime();
			const diffMins = Math.floor(diffMs / 60000);
			if (diffMins < 1) return 'Just now';
			if (diffMins < 60) return `${diffMins}m ago`;
			const diffHrs = Math.floor(diffMins / 60);
			if (diffHrs < 24) return `${diffHrs}h ago`;
			const diffDays = Math.floor(diffHrs / 24);
			if (diffDays < 7) return `${diffDays}d ago`;
			return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined });
		} catch {
			return dateStr;
		}
	}

	function getFolderUnread(folder: string): number {
		const fc = folderCounts.find(f => f[0] === folder);
		return fc ? fc[2] : 0;
	}

	function getFolderTotal(folder: string): number {
		const fc = folderCounts.find(f => f[0] === folder);
		return fc ? fc[1] : 0;
	}

	const providers = [
		{ id: 'gmail', name: 'Gmail', color: 'text-red-400' },
		{ id: 'outlook', name: 'Outlook', color: 'text-blue-400' },
		{ id: 'yahoo', name: 'Yahoo', color: 'text-purple-400' },
		{ id: 'protonmail', name: 'ProtonMail', color: 'text-green-400' },
		{ id: 'custom', name: 'Custom IMAP', color: 'text-gx-text-muted' },
	];

	const folders = ['inbox', 'sent', 'drafts', 'starred', 'trash'];

	const tones = [
		{ id: 'professional', label: 'Professional' },
		{ id: 'casual', label: 'Casual' },
		{ id: 'formal', label: 'Formal' },
		{ id: 'friendly', label: 'Friendly' },
	];

	// ---- Lifecycle -----------------------------------------------------------
	onMount(() => {
		loadAccounts();
	});
</script>

<div class="flex h-full overflow-hidden" style={containerStyle}>
	<!-- LEFT SIDEBAR (220px) — Folders + Account -->
	{#if view !== 'setup'}
		<div class="flex flex-col w-56 border-r border-gx-border-default bg-gx-bg-secondary shrink-0">
			<!-- Compose Button -->
			<div class="px-3 pt-3 pb-2">
				<button
					onclick={() => startCompose()}
					class="w-full flex items-center justify-center gap-2 px-3 py-2 text-xs font-semibold rounded-gx
						bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
				>
					<MailPlus size={14} />
					Compose
				</button>
			</div>

			<!-- Account Switcher -->
			{#if accounts.length > 0}
				<div class="px-3 pb-2">
					<select
						bind:value={activeAccountId}
						onchange={async () => { selectedEmail = null; await loadEmails(); await loadFolderCounts(); }}
						class="w-full px-2 py-1.5 text-[11px] rounded bg-gx-bg-primary border border-gx-border-default
							text-gx-text-primary focus:border-gx-neon/50 focus:outline-none transition-colors"
					>
						{#each accounts as acc (acc.account.id)}
							<option value={acc.account.id}>
								{acc.account.name} ({acc.unread_count})
							</option>
						{/each}
					</select>
				</div>
			{/if}

			<Separator class="bg-gx-border-default" />

			<!-- Folders -->
			<div class="flex-1 overflow-y-auto py-1">
				{#each folders as folder}
					{@const FolderIconComp = folderIcon(folder)}
					{@const unread = getFolderUnread(folder)}
					<button
						onclick={() => selectFolder(folder)}
						class="w-full flex items-center gap-2.5 px-3 py-2 text-[11px] transition-all
							{activeFolder === folder && view === 'inbox'
								? 'bg-gx-bg-elevated text-gx-neon border-l-2 border-gx-neon'
								: 'text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-text-primary'}"
					>
						<FolderIconComp size={14} class="shrink-0" />
						<span class="capitalize flex-1 text-left">{folder}</span>
						{#if unread > 0}
							<Badge variant="outline" class="text-[9px] px-1 py-0 h-4 border-gx-neon/30 text-gx-neon font-semibold">
								{unread}
							</Badge>
						{/if}
					</button>
				{/each}

				<Separator class="bg-gx-border-default my-1 mx-3" />

				<!-- Labels placeholder -->
				<div class="px-3 py-2">
					<div class="flex items-center gap-1.5 text-[10px] text-gx-text-muted">
						<Tag size={11} />
						<span>Labels</span>
					</div>
					<p class="text-[10px] text-gx-text-muted/50 mt-1 pl-4">
						AI categorization available
					</p>
				</div>
			</div>

			<!-- Sidebar Footer -->
			<div class="px-3 py-2 border-t border-gx-border-default space-y-1">
				<button
					onclick={openWebmail}
					class="w-full flex items-center gap-2 px-2 py-1 text-[10px] text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover rounded transition-all"
				>
					<Globe size={11} />
					Open Webmail
				</button>
				<button
					onclick={() => { view = 'setup'; }}
					class="w-full flex items-center gap-2 px-2 py-1 text-[10px] text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover rounded transition-all"
				>
					<Plus size={11} />
					Add Account
				</button>
			</div>
		</div>
	{/if}

	<!-- CENTER + RIGHT AREA -->
	<div class="flex flex-1 min-w-0 overflow-hidden">
		{#if view === 'setup'}
			<!-- ================ SETUP WIZARD ================ -->
			<div class="flex-1 flex items-center justify-center p-8">
				<div class="w-full max-w-md space-y-6">
					<!-- Header -->
					<div class="text-center">
						{#if accounts.length > 0}
							<button
								onclick={() => { view = 'inbox'; error = null; }}
								class="absolute top-4 left-4 flex items-center gap-1 text-xs text-gx-text-muted hover:text-gx-neon transition-colors"
							>
								<ArrowLeft size={14} />
								Back to inbox
							</button>
						{/if}
						<div class="w-16 h-16 rounded-2xl bg-gx-bg-secondary border border-gx-border-default flex items-center justify-center mx-auto mb-4">
							<Mail size={28} class="text-gx-neon/40" />
						</div>
						<h2 class="text-lg font-semibold text-gx-text-primary mb-1">Add Email Account</h2>
						<p class="text-xs text-gx-text-muted">
							Connect your email to manage it with AI assistance
						</p>
					</div>

					<!-- Provider Selection -->
					<div class="grid grid-cols-3 gap-2">
						{#each providers as p}
							<button
								onclick={() => { setupProviderType = p.id; }}
								class="flex flex-col items-center gap-1.5 px-3 py-3 rounded-gx border transition-all text-center
									{setupProviderType === p.id
										? 'border-gx-neon bg-gx-neon/10 text-gx-neon'
										: 'border-gx-border-default bg-gx-bg-primary text-gx-text-muted hover:border-gx-text-muted/50 hover:bg-gx-bg-hover'}"
							>
								<Mail size={18} class={setupProviderType === p.id ? 'text-gx-neon' : p.color} />
								<span class="text-[10px] font-medium">{p.name}</span>
							</button>
						{/each}
					</div>

					<!-- Account Fields -->
					<div class="space-y-3">
						<div>
							<label for="setup-name" class="text-[11px] text-gx-text-muted mb-1 block">Display Name</label>
							<input
								id="setup-name"
								type="text"
								bind:value={setupName}
								placeholder="My Email"
								class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
							/>
						</div>
						<div>
							<label for="setup-email" class="text-[11px] text-gx-text-muted mb-1 block">Email Address</label>
							<input
								id="setup-email"
								type="email"
								bind:value={setupEmail}
								placeholder="you@example.com"
								onkeydown={(e) => { if (e.key === 'Enter') addAccount(); }}
								class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
							/>
						</div>

						<!-- Custom IMAP/SMTP fields -->
						{#if setupProviderType === 'custom'}
							<Separator class="bg-gx-border-default" />
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label for="setup-imap" class="text-[11px] text-gx-text-muted mb-1 block">IMAP Host</label>
									<input
										id="setup-imap"
										type="text"
										bind:value={setupImapHost}
										placeholder="imap.example.com"
										class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
								</div>
								<div>
									<label for="setup-imap-port" class="text-[11px] text-gx-text-muted mb-1 block">IMAP Port</label>
									<input
										id="setup-imap-port"
										type="text"
										bind:value={setupImapPort}
										placeholder="993"
										class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
								</div>
								<div>
									<label for="setup-smtp" class="text-[11px] text-gx-text-muted mb-1 block">SMTP Host</label>
									<input
										id="setup-smtp"
										type="text"
										bind:value={setupSmtpHost}
										placeholder="smtp.example.com"
										class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
								</div>
								<div>
									<label for="setup-smtp-port" class="text-[11px] text-gx-text-muted mb-1 block">SMTP Port</label>
									<input
										id="setup-smtp-port"
										type="text"
										bind:value={setupSmtpPort}
										placeholder="587"
										class="w-full px-3 py-2 text-xs rounded-gx bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
									/>
								</div>
							</div>
						{/if}

						<!-- Info box for webmail providers -->
						{#if setupProviderType !== 'custom'}
							<div class="flex items-start gap-2 px-3 py-2.5 rounded-gx bg-gx-bg-elevated border border-gx-border-default">
								<Globe size={13} class="text-gx-neon shrink-0 mt-0.5" />
								<p class="text-[10px] text-gx-text-muted leading-relaxed">
									For {providers.find(p => p.id === setupProviderType)?.name ?? 'this provider'}, ImpForge opens your webmail in the built-in browser.
									Native IMAP/SMTP integration is coming in a future update.
								</p>
							</div>
						{/if}
					</div>

					<!-- Error -->
					{#if error}
						<div class="flex items-center gap-2 px-3 py-2 rounded bg-gx-status-error/10 border border-gx-status-error/20">
							<AlertCircle size={13} class="text-gx-status-error shrink-0" />
							<span class="text-[11px] text-gx-status-error">{error}</span>
						</div>
					{/if}

					<!-- Actions -->
					<div class="flex justify-end gap-2">
						{#if accounts.length > 0}
							<button
								onclick={() => { view = 'inbox'; error = null; }}
								class="px-3 py-1.5 text-xs rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
							>
								Cancel
							</button>
						{/if}
						<button
							onclick={addAccount}
							class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
						>
							<Plus size={13} />
							Add Account
						</button>
					</div>

					<!-- Existing accounts (if adding more) -->
					{#if accounts.length > 0}
						<Separator class="bg-gx-border-default" />
						<div>
							<h3 class="text-xs font-medium text-gx-text-secondary mb-2">Connected Accounts</h3>
							{#each accounts as acc (acc.account.id)}
								<div class="flex items-center justify-between px-3 py-2 rounded-gx hover:bg-gx-bg-hover transition-all">
									<div class="flex items-center gap-2">
										<User size={13} class="text-gx-text-muted" />
										<div>
											<p class="text-[11px] text-gx-text-primary">{acc.account.name}</p>
											<p class="text-[10px] text-gx-text-muted">{acc.account.email}</p>
										</div>
									</div>
									<button
										onclick={() => removeAccount(acc.account.id)}
										class="text-gx-text-muted/50 hover:text-gx-status-error transition-colors"
										title="Remove account"
									>
										<Trash2 size={12} />
									</button>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>

		{:else if view === 'compose'}
			<!-- ================ COMPOSE VIEW ================ -->
			<div class="flex-1 flex flex-col min-w-0 overflow-hidden">
				<!-- Compose Toolbar -->
				<div class="flex items-center gap-2 px-4 py-2 border-b border-gx-border-default bg-gx-bg-secondary shrink-0">
					<button
						onclick={() => { view = 'inbox'; error = null; }}
						class="flex items-center gap-1 text-xs text-gx-text-muted hover:text-gx-neon transition-colors"
					>
						<ArrowLeft size={14} />
						Back
					</button>
					<div class="flex-1"></div>
					<button
						onclick={() => aiPanelOpen = !aiPanelOpen}
						class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded transition-all
							{aiPanelOpen
								? 'text-gx-accent-magenta bg-gx-accent-magenta/10 border border-gx-accent-magenta/30'
								: 'text-gx-text-muted hover:text-gx-accent-magenta hover:bg-gx-accent-magenta/10 border border-transparent'}"
					>
						<Sparkles size={13} />
						AI Assist
					</button>
					<button
						onclick={sendDraft}
						disabled={composeSending || !composeTo.trim()}
						class="flex items-center gap-1.5 px-4 py-1.5 text-xs font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
					>
						{#if composeSending}
							<Loader2 size={13} class="animate-spin" />
						{:else}
							<Send size={13} />
						{/if}
						Send
					</button>
				</div>

				<!-- Compose Form -->
				<div class="flex flex-1 min-h-0 overflow-hidden">
					<div class="flex-1 flex flex-col overflow-y-auto">
						<div class="px-6 pt-4 space-y-2 shrink-0">
							<div class="flex items-center gap-2">
								<label class="text-[11px] text-gx-text-muted w-10 shrink-0">To</label>
								<input
									type="text"
									bind:value={composeTo}
									placeholder="recipient@example.com"
									class="flex-1 px-3 py-1.5 text-xs rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>
							<div class="flex items-center gap-2">
								<label class="text-[11px] text-gx-text-muted w-10 shrink-0">Cc</label>
								<input
									type="text"
									bind:value={composeCc}
									placeholder="Optional"
									class="flex-1 px-3 py-1.5 text-xs rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>
							<div class="flex items-center gap-2">
								<label class="text-[11px] text-gx-text-muted w-10 shrink-0">Bcc</label>
								<input
									type="text"
									bind:value={composeBcc}
									placeholder="Optional"
									class="flex-1 px-3 py-1.5 text-xs rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>
							<div class="flex items-center gap-2">
								<label class="text-[11px] text-gx-text-muted w-10 shrink-0">Subj</label>
								<input
									type="text"
									bind:value={composeSubject}
									placeholder="Subject"
									class="flex-1 px-3 py-1.5 text-xs font-medium rounded bg-gx-bg-primary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
								/>
							</div>
						</div>

						<Separator class="bg-gx-border-default mx-6 my-2" />

						<!-- Body -->
						<textarea
							bind:value={composeBody}
							placeholder="Write your email..."
							class="flex-1 w-full px-6 py-2 bg-transparent text-sm text-gx-text-secondary
								placeholder:text-gx-text-muted/30 focus:outline-none resize-none leading-relaxed"
						></textarea>
					</div>

					<!-- AI Panel (compose context) -->
					{#if aiPanelOpen}
						<div class="w-72 border-l border-gx-border-default bg-gx-bg-secondary flex flex-col shrink-0 overflow-hidden">
							<div class="flex items-center justify-between px-3 py-2.5 border-b border-gx-border-default shrink-0">
								<div class="flex items-center gap-1.5">
									<Sparkles size={13} class="text-gx-accent-magenta" />
									<span class="text-xs font-medium text-gx-text-primary">AI Email Assistant</span>
								</div>
								<button
									onclick={() => aiPanelOpen = false}
									class="text-gx-text-muted hover:text-gx-text-primary transition-colors"
								>
									<X size={14} />
								</button>
							</div>

							<!-- Tone Selector -->
							<div class="px-3 py-2 border-b border-gx-border-default/50 shrink-0">
								<label class="text-[10px] text-gx-text-muted mb-1 block">Tone</label>
								<div class="flex gap-1 flex-wrap">
									{#each tones as t}
										<button
											onclick={() => { aiTone = t.id; }}
											class="px-2 py-0.5 text-[10px] rounded border transition-all
												{aiTone === t.id
													? 'border-gx-neon/50 bg-gx-neon/10 text-gx-neon'
													: 'border-gx-border-default text-gx-text-muted hover:border-gx-text-muted/50'}"
										>
											{t.label}
										</button>
									{/each}
								</div>
							</div>

							<!-- Actions -->
							<div class="px-3 py-2 space-y-1 shrink-0 border-b border-gx-border-default/50">
								<button
									onclick={() => aiCompose('compose')}
									disabled={aiLoading}
									class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
								>
									<Wand2 size={12} class="text-gx-accent-magenta shrink-0" />
									AI Compose
								</button>
								<button
									onclick={() => aiCompose('reply')}
									disabled={aiLoading}
									class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
								>
									<Reply size={12} class="text-gx-accent-blue shrink-0" />
									AI Reply
								</button>
								<button
									onclick={() => aiCompose('summarize')}
									disabled={aiLoading}
									class="w-full flex items-center gap-2 px-2.5 py-1.5 text-[11px] text-gx-text-secondary rounded hover:bg-gx-bg-hover hover:text-gx-neon transition-all disabled:opacity-40"
								>
									<ListFilter size={12} class="text-gx-accent-purple shrink-0" />
									Summarize Thread
								</button>
							</div>

							<!-- AI Result -->
							<div class="flex-1 overflow-y-auto px-3 py-3">
								{#if aiLoading}
									<div class="flex flex-col items-center justify-center py-8 gap-2">
										<Loader2 size={18} class="animate-spin text-gx-accent-magenta" />
										<span class="text-[11px] text-gx-text-muted">AI is composing...</span>
									</div>
								{:else if aiError}
									<div class="rounded bg-gx-status-error/10 border border-gx-status-error/20 p-3">
										<div class="flex items-start gap-2">
											<AlertCircle size={13} class="text-gx-status-error shrink-0 mt-0.5" />
											<p class="text-[11px] text-gx-status-error leading-relaxed">{aiError}</p>
										</div>
									</div>
								{:else if aiResult}
									<div class="space-y-3">
										<div class="rounded bg-gx-bg-primary border border-gx-border-default p-3">
											<p class="text-[11px] text-gx-text-secondary leading-relaxed whitespace-pre-wrap">
												{aiResult}
											</p>
										</div>
										<div class="flex gap-2">
											<button
												onclick={applyAiResult}
												class="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
											>
												<Check size={12} />
												Use This
											</button>
											<button
												onclick={copyAiResult}
												class="px-3 py-1.5 text-[11px] rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
											>
												{#if aiCopied}
													<Check size={12} class="text-gx-status-success" />
												{:else}
													<Copy size={12} />
												{/if}
											</button>
										</div>
									</div>
								{:else}
									<div class="flex flex-col items-center justify-center py-8 text-center">
										<Sparkles size={24} class="text-gx-text-muted/20 mb-2" />
										<p class="text-[11px] text-gx-text-muted">
											Choose an action to let AI help compose your email
										</p>
									</div>
								{/if}
							</div>
						</div>
					{/if}
				</div>

				<!-- Error bar -->
				{#if error}
					<div class="flex items-center gap-2 px-4 py-2 bg-gx-status-error/10 border-t border-gx-status-error/20 shrink-0">
						<AlertCircle size={13} class="text-gx-status-error shrink-0" />
						<span class="text-[11px] text-gx-status-error flex-1">{error}</span>
						<button onclick={() => { error = null; }} class="text-gx-status-error/60 hover:text-gx-status-error">
							<X size={12} />
						</button>
					</div>
				{/if}
			</div>

		{:else}
			<!-- ================ INBOX VIEW (email list + reading pane) ================ -->

			<!-- Email List -->
			<div class="flex flex-col w-96 border-r border-gx-border-default bg-gx-bg-primary shrink-0 overflow-hidden">
				<!-- Search Bar -->
				<div class="px-3 py-2 border-b border-gx-border-default shrink-0">
					<div class="relative">
						<Search size={12} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gx-text-muted" />
						<input
							type="text"
							bind:value={searchQuery}
							placeholder="Search emails..."
							onkeydown={(e) => { if (e.key === 'Enter') searchEmails(); }}
							class="w-full pl-7 pr-8 py-1.5 text-[11px] rounded bg-gx-bg-secondary border border-gx-border-default text-gx-text-primary placeholder:text-gx-text-muted/50 focus:border-gx-neon/50 focus:outline-none transition-colors"
						/>
						{#if searchQuery}
							<button
								onclick={() => { searchQuery = ''; loadEmails(); }}
								class="absolute right-2 top-1/2 -translate-y-1/2 text-gx-text-muted hover:text-gx-text-primary"
							>
								<X size={12} />
							</button>
						{/if}
					</div>
				</div>

				<!-- List Header -->
				<div class="flex items-center justify-between px-3 py-1.5 border-b border-gx-border-default/50 shrink-0">
					<span class="text-[10px] text-gx-text-muted capitalize font-medium">
						{activeFolder} ({filteredEmails.length})
					</span>
					<button
						onclick={() => { loadEmails(); loadFolderCounts(); }}
						class="text-gx-text-muted hover:text-gx-neon transition-colors"
						title="Refresh"
					>
						<RefreshCw size={12} />
					</button>
				</div>

				<!-- Email Items -->
				<div class="flex-1 overflow-y-auto">
					{#if loading || emailsLoading}
						<div class="flex items-center justify-center py-12">
							<Loader2 size={18} class="animate-spin text-gx-text-muted" />
						</div>
					{:else if filteredEmails.length === 0}
						<div class="flex flex-col items-center justify-center py-12 px-4 text-center">
							<MailOpen size={32} class="text-gx-text-muted/20 mb-3" />
							<p class="text-xs text-gx-text-muted mb-1">
								{searchQuery ? 'No matching emails' : 'No emails yet'}
							</p>
							<p class="text-[10px] text-gx-text-muted/60">
								{searchQuery ? 'Try a different search' : 'Add an account and use the built-in browser to access your webmail'}
							</p>
						</div>
					{:else}
						{#each filteredEmails as email (email.id)}
							<!-- Use div+role to avoid nested <button> issues (Svelte 5 validation) -->
							<div
								role="button"
								tabindex="0"
								onclick={() => openEmail(email.id)}
								onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openEmail(email.id); } }}
								class="w-full text-left px-3 py-2.5 border-b border-gx-border-default/30 transition-all group cursor-pointer
									{selectedEmail?.id === email.id
										? 'bg-gx-bg-elevated border-l-2 border-l-gx-neon'
										: 'hover:bg-gx-bg-hover'}"
							>
								<div class="flex items-start gap-2">
									<!-- Star -->
									<button
										onclick={(e) => { e.stopPropagation(); toggleStar(email.id, email.is_starred); }}
										class="shrink-0 mt-0.5 transition-colors
											{email.is_starred ? 'text-yellow-400' : 'text-gx-text-muted/30 hover:text-yellow-400/60'}"
									>
										<Star size={12} fill={email.is_starred ? 'currentColor' : 'none'} />
									</button>

									<div class="flex-1 min-w-0">
										<!-- From + Date row -->
										<div class="flex items-center justify-between gap-2">
											<span class="text-[11px] truncate {email.is_read ? 'text-gx-text-muted' : 'text-gx-text-primary font-semibold'}">
												{email.from}
											</span>
											<span class="text-[10px] text-gx-text-muted/60 shrink-0">
												{formatDate(email.date)}
											</span>
										</div>
										<!-- Subject -->
										<p class="text-[11px] truncate mt-0.5 {email.is_read ? 'text-gx-text-muted' : 'text-gx-text-secondary font-medium'}">
											{email.subject || '(no subject)'}
										</p>
										<!-- Preview -->
										<p class="text-[10px] text-gx-text-muted/60 truncate mt-0.5">
											{email.preview}
										</p>
										<!-- Labels -->
										{#if email.labels.length > 0}
											<div class="flex gap-1 mt-1 flex-wrap">
												{#each email.labels.slice(0, 2) as label}
													<span class="text-[9px] px-1 py-0 rounded bg-gx-accent-magenta/10 text-gx-accent-magenta border border-gx-accent-magenta/20">
														{label}
													</span>
												{/each}
											</div>
										{/if}
									</div>

									<!-- Delete on hover -->
									<button
										onclick={(e) => { e.stopPropagation(); deleteEmail(email.id); }}
										class="shrink-0 mt-0.5 text-transparent group-hover:text-gx-text-muted/50 hover:!text-gx-status-error transition-all"
										title="Delete"
									>
										<Trash2 size={12} />
									</button>
								</div>
							</div>
						{/each}
					{/if}
				</div>
			</div>

			<!-- Reading Pane -->
			<div class="flex-1 flex flex-col min-w-0 overflow-hidden">
				{#if selectedEmail}
					<!-- Email Header -->
					<div class="px-6 py-4 border-b border-gx-border-default bg-gx-bg-secondary/50 shrink-0">
						<div class="flex items-start justify-between gap-4">
							<div class="min-w-0 flex-1">
								<h2 class="text-base font-semibold text-gx-text-primary mb-2">
									{selectedEmail.subject || '(no subject)'}
								</h2>
								<div class="flex items-center gap-2 text-xs">
									<div class="w-8 h-8 rounded-full bg-gx-bg-elevated border border-gx-border-default flex items-center justify-center shrink-0">
										<User size={14} class="text-gx-text-muted" />
									</div>
									<div>
										<p class="text-gx-text-primary font-medium">{selectedEmail.from}</p>
										<p class="text-[10px] text-gx-text-muted">
											to {selectedEmail.to.join(', ')}
											{#if selectedEmail.cc.length > 0}
												<span class="text-gx-text-muted/50"> cc {selectedEmail.cc.join(', ')}</span>
											{/if}
										</p>
									</div>
								</div>
							</div>
							<div class="flex items-center gap-1 shrink-0">
								<span class="text-[10px] text-gx-text-muted mr-2">
									{formatDate(selectedEmail.date)}
								</span>
								<button
									onclick={() => startCompose(selectedEmail!)}
									class="flex items-center gap-1 px-2 py-1 text-[11px] text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover rounded transition-all"
									title="Reply"
								>
									<Reply size={13} />
								</button>
								<button
									onclick={() => startForward(selectedEmail!)}
									class="flex items-center gap-1 px-2 py-1 text-[11px] text-gx-text-muted hover:text-gx-neon hover:bg-gx-bg-hover rounded transition-all"
									title="Forward"
								>
									<Forward size={13} />
								</button>
								<button
									onclick={() => toggleStar(selectedEmail!.id, selectedEmail!.is_starred)}
									class="px-2 py-1 rounded hover:bg-gx-bg-hover transition-all
										{selectedEmail.is_starred ? 'text-yellow-400' : 'text-gx-text-muted hover:text-yellow-400'}"
									title={selectedEmail.is_starred ? 'Unstar' : 'Star'}
								>
									<Star size={13} fill={selectedEmail.is_starred ? 'currentColor' : 'none'} />
								</button>
								<button
									onclick={() => deleteEmail(selectedEmail!.id)}
									class="px-2 py-1 text-gx-text-muted hover:text-gx-status-error hover:bg-gx-status-error/10 rounded transition-all"
									title="Delete"
								>
									<Trash2 size={13} />
								</button>
							</div>
						</div>
					</div>

					<!-- Email Body -->
					<div class="flex-1 overflow-y-auto px-6 py-4">
						{#if selectedEmail.body_html}
							{@html selectedEmail.body_html}
						{:else}
							<div class="text-sm text-gx-text-secondary leading-relaxed whitespace-pre-wrap">
								{selectedEmail.body}
							</div>
						{/if}
					</div>

					<!-- Quick Actions Bar -->
					<div class="flex items-center gap-2 px-6 py-2 border-t border-gx-border-default bg-gx-bg-secondary/50 shrink-0">
						<button
							onclick={() => startCompose(selectedEmail!)}
							class="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded bg-gx-neon/10 text-gx-neon border border-gx-neon/20 hover:bg-gx-neon/20 transition-all"
						>
							<Reply size={12} />
							Reply
						</button>
						<button
							onclick={() => startForward(selectedEmail!)}
							class="flex items-center gap-1.5 px-3 py-1.5 text-[11px] rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
						>
							<Forward size={12} />
							Forward
						</button>
						<div class="flex-1"></div>
						<button
							onclick={() => { aiPanelOpen = true; aiCompose('reply'); }}
							class="flex items-center gap-1.5 px-3 py-1.5 text-[11px] rounded text-gx-accent-magenta hover:bg-gx-accent-magenta/10 border border-gx-accent-magenta/20 transition-all"
						>
							<Sparkles size={12} />
							AI Reply
						</button>
						<button
							onclick={() => { aiPanelOpen = true; aiCompose('summarize'); }}
							class="flex items-center gap-1.5 px-3 py-1.5 text-[11px] rounded text-gx-text-muted hover:text-gx-accent-purple hover:bg-gx-accent-purple/10 border border-gx-border-default transition-all"
						>
							<ListFilter size={12} />
							Summarize
						</button>
					</div>

					<!-- AI Panel (reading context) -->
					{#if aiPanelOpen && view === 'inbox'}
						<div class="border-t border-gx-border-default bg-gx-bg-secondary max-h-64 overflow-y-auto">
							<div class="flex items-center justify-between px-4 py-2 border-b border-gx-border-default/50">
								<div class="flex items-center gap-1.5">
									<Sparkles size={12} class="text-gx-accent-magenta" />
									<span class="text-[11px] font-medium text-gx-text-primary">AI Assistant</span>
								</div>
								<button onclick={() => { aiPanelOpen = false; }} class="text-gx-text-muted hover:text-gx-text-primary">
									<X size={12} />
								</button>
							</div>
							<div class="px-4 py-3">
								{#if aiLoading}
									<div class="flex items-center gap-2">
										<Loader2 size={14} class="animate-spin text-gx-accent-magenta" />
										<span class="text-[11px] text-gx-text-muted">AI is thinking...</span>
									</div>
								{:else if aiError}
									<div class="flex items-start gap-2">
										<AlertCircle size={12} class="text-gx-status-error shrink-0 mt-0.5" />
										<p class="text-[11px] text-gx-status-error">{aiError}</p>
									</div>
								{:else if aiResult}
									<div class="space-y-2">
										<p class="text-[11px] text-gx-text-secondary leading-relaxed whitespace-pre-wrap">{aiResult}</p>
										<div class="flex gap-2">
											<button
												onclick={() => { applyAiResult(); view = 'compose'; }}
												class="flex items-center gap-1 px-2.5 py-1 text-[10px] font-medium rounded bg-gx-neon/10 text-gx-neon border border-gx-neon/20 hover:bg-gx-neon/20 transition-all"
											>
												<Reply size={10} />
												Use as Reply
											</button>
											<button
												onclick={copyAiResult}
												class="flex items-center gap-1 px-2.5 py-1 text-[10px] rounded text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover border border-gx-border-default transition-all"
											>
												{#if aiCopied}
													<Check size={10} class="text-gx-status-success" />
													Copied
												{:else}
													<Copy size={10} />
													Copy
												{/if}
											</button>
										</div>
									</div>
								{/if}
							</div>
						</div>
					{/if}
				{:else}
					<!-- No email selected — welcome state -->
					<div class="flex-1 flex flex-col items-center justify-center px-8 text-center">
						<div class="w-16 h-16 rounded-2xl bg-gx-bg-secondary border border-gx-border-default flex items-center justify-center mb-6">
							<Mail size={28} class="text-gx-neon/40" />
						</div>
						<h2 class="text-lg font-semibold text-gx-text-primary mb-2">ForgeMail</h2>
						<p class="text-sm text-gx-text-muted mb-6 max-w-sm">
							AI-powered email management. Compose, reply, and categorize emails with AI assistance. Select an email from the list to get started.
						</p>
						<div class="flex gap-3">
							<button
								onclick={() => startCompose()}
								class="flex items-center gap-2 px-4 py-2 text-xs font-medium rounded-gx bg-gx-neon/15 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/25 transition-all"
							>
								<MailPlus size={14} />
								New Email
							</button>
							<button
								onclick={openWebmail}
								class="flex items-center gap-2 px-4 py-2 text-xs font-medium rounded-gx text-gx-text-secondary border border-gx-border-default hover:border-gx-text-muted/50 hover:bg-gx-bg-hover transition-all"
							>
								<Globe size={14} />
								Open Webmail
							</button>
						</div>

						<!-- Keyboard shortcuts -->
						<div class="mt-8 grid grid-cols-2 gap-x-6 gap-y-1.5 text-[10px] text-gx-text-muted">
							<div class="flex items-center gap-2">
								<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">N</kbd>
								<span>New email</span>
							</div>
							<div class="flex items-center gap-2">
								<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">R</kbd>
								<span>Reply</span>
							</div>
							<div class="flex items-center gap-2">
								<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">F</kbd>
								<span>Forward</span>
							</div>
							<div class="flex items-center gap-2">
								<kbd class="px-1 py-0.5 rounded bg-gx-bg-secondary border border-gx-border-default text-[9px]">Del</kbd>
								<span>Delete</span>
							</div>
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

<!-- Error Toast (global) -->
{#if error && view === 'inbox'}
	<div class="fixed bottom-4 right-4 z-50 flex items-center gap-2 px-4 py-2.5 rounded-gx bg-gx-bg-elevated border border-gx-status-error/30 shadow-lg max-w-sm">
		<AlertCircle size={14} class="text-gx-status-error shrink-0" />
		<span class="text-[11px] text-gx-status-error flex-1">{error}</span>
		<button onclick={() => { error = null; }} class="text-gx-text-muted hover:text-gx-text-primary shrink-0">
			<X size={12} />
		</button>
	</div>
{/if}
