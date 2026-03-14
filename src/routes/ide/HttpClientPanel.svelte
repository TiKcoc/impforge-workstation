<script lang="ts">
	/**
	 * HttpClientPanel — Built-in REST API Client with BenikUI integration
	 *
	 * Postman-like HTTP testing with method selector, URL bar, headers,
	 * body, auth config, response viewer, and cURL export.
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - url-bar: Method + URL input
	 *   - request-config: Headers/Body/Auth tabs
	 *   - response-view: Response body + metadata
	 */

	import { invoke } from '@tauri-apps/api/core';
	import {
		Send, Plus, Trash2, Copy, Clock, ChevronDown,
		Loader2, Code2, FileJson, ArrowRight
	} from '@lucide/svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	interface HttpResponse {
		status: number;
		status_text: string;
		headers: Record<string, string>;
		body: string;
		size_bytes: number;
		elapsed_ms: number;
		content_type: string | null;
	}

	// BenikUI style engine
	const widgetId = 'ide-http-client';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let urlBarComp = $derived(styleEngine.getComponentStyle(widgetId, 'url-bar'));
	let urlBarStyle = $derived(hasEngineStyle && urlBarComp ? componentToCSS(urlBarComp) : '');

	// Request state
	let method = $state('GET');
	let url = $state('');
	let headers = $state<{ key: string; value: string; enabled: boolean }[]>([
		{ key: 'Content-Type', value: 'application/json', enabled: true },
	]);
	let bodyType = $state<'none' | 'json' | 'form' | 'text'>('none');
	let bodyContent = $state('');
	let authType = $state<'none' | 'bearer' | 'basic' | 'api_key'>('none');
	let authToken = $state('');
	let authUser = $state('');
	let authPass = $state('');
	let authKeyName = $state('');
	let authKeyValue = $state('');
	let authKeyLocation = $state<'header' | 'query'>('header');

	// Response state
	let response = $state<HttpResponse | null>(null);
	let sending = $state(false);
	let error = $state('');

	// UI state
	let requestTab = $state<'headers' | 'body' | 'auth'>('headers');
	let responseTab = $state<'body' | 'headers'>('body');

	const methods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

	function methodColor(m: string): string {
		switch (m) {
			case 'GET': return 'text-gx-accent-cyan';
			case 'POST': return 'text-gx-neon';
			case 'PUT': return 'text-gx-status-warning';
			case 'PATCH': return 'text-gx-accent-magenta';
			case 'DELETE': return 'text-gx-status-error';
			default: return 'text-gx-text-muted';
		}
	}

	function statusColor(status: number): string {
		if (status < 200) return 'text-gx-text-muted';
		if (status < 300) return 'text-gx-neon';
		if (status < 400) return 'text-gx-accent-cyan';
		if (status < 500) return 'text-gx-status-warning';
		return 'text-gx-status-error';
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	async function sendRequest() {
		if (!url.trim()) return;
		sending = true;
		error = '';
		response = null;

		const headerMap: Record<string, string> = {};
		for (const h of headers) {
			if (h.enabled && h.key.trim()) {
				headerMap[h.key] = h.value;
			}
		}

		let auth = null;
		if (authType !== 'none') {
			auth = {
				auth_type: authType,
				token: authType === 'bearer' ? authToken : null,
				username: authType === 'basic' ? authUser : null,
				password: authType === 'basic' ? authPass : null,
				key_name: authType === 'api_key' ? authKeyName : null,
				key_value: authType === 'api_key' ? authKeyValue : null,
				key_location: authType === 'api_key' ? authKeyLocation : null,
			};
		}

		try {
			response = await invoke<HttpResponse>('http_send_request', {
				request: {
					method,
					url,
					headers: headerMap,
					body: bodyType !== 'none' ? bodyContent : null,
					body_type: bodyType !== 'none' ? bodyType : null,
					auth,
				},
			});
		} catch (e) {
			error = String(e);
		}
		sending = false;
	}

	async function copyCurl() {
		const headerMap: Record<string, string> = {};
		for (const h of headers) {
			if (h.enabled && h.key.trim()) headerMap[h.key] = h.value;
		}

		let auth = null;
		if (authType !== 'none') {
			auth = {
				auth_type: authType,
				token: authType === 'bearer' ? authToken : null,
				username: authType === 'basic' ? authUser : null,
				password: authType === 'basic' ? authPass : null,
				key_name: authType === 'api_key' ? authKeyName : null,
				key_value: authType === 'api_key' ? authKeyValue : null,
				key_location: authType === 'api_key' ? authKeyLocation : null,
			};
		}

		try {
			const curl = await invoke<string>('http_to_curl', {
				request: {
					method,
					url,
					headers: headerMap,
					body: bodyType !== 'none' ? bodyContent : null,
					body_type: bodyType !== 'none' ? bodyType : null,
					auth,
				},
			});
			await navigator.clipboard.writeText(curl);
		} catch (e) {
			error = String(e);
		}
	}

	function addHeader() {
		headers = [...headers, { key: '', value: '', enabled: true }];
	}

	function removeHeader(idx: number) {
		headers = headers.filter((_, i) => i !== idx);
	}

	function prettyJson(text: string): string {
		try {
			return JSON.stringify(JSON.parse(text), null, 2);
		} catch {
			return text;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
			e.preventDefault();
			sendRequest();
		}
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- URL Bar -->
	<div class="flex items-center gap-1.5 px-2 py-1.5 border-b border-gx-border-subtle shrink-0" style={urlBarStyle}>
		<select
			bind:value={method}
			class={`bg-gx-bg-secondary text-xs font-bold border border-gx-border-default rounded px-1.5 py-1 outline-none focus:border-gx-neon ${methodColor(method)}`}
		>
			{#each methods as m}
				<option value={m}>{m}</option>
			{/each}
		</select>

		<input
			bind:value={url}
			onkeydown={handleKeydown}
			placeholder="https://api.example.com/v1/resource"
			class="flex-1 bg-gx-bg-secondary text-xs text-gx-text-primary font-mono border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon"
		/>

		<button
			onclick={sendRequest}
			disabled={sending || !url.trim()}
			class="flex items-center gap-1 px-3 py-1 text-xs bg-gx-neon/20 text-gx-neon border border-gx-neon/30 rounded hover:bg-gx-neon/30 disabled:opacity-40"
		>
			{#if sending}
				<Loader2 size={11} class="animate-spin" />
			{:else}
				<Send size={11} />
			{/if}
			Send
		</button>

		<button
			onclick={copyCurl}
			class="p-1 text-gx-text-muted hover:text-gx-text-secondary"
			title="Copy as cURL"
		>
			<Copy size={11} />
		</button>
	</div>

	<!-- Request config tabs -->
	<div class="flex items-center gap-3 px-3 py-1 border-b border-gx-border-subtle text-[10px] shrink-0">
		<button
			onclick={() => { requestTab = 'headers'; }}
			class={requestTab === 'headers' ? 'text-gx-neon border-b border-gx-neon pb-0.5' : 'text-gx-text-muted hover:text-gx-text-secondary'}
		>
			Headers ({headers.filter(h => h.enabled).length})
		</button>
		<button
			onclick={() => { requestTab = 'body'; }}
			class={requestTab === 'body' ? 'text-gx-neon border-b border-gx-neon pb-0.5' : 'text-gx-text-muted hover:text-gx-text-secondary'}
		>
			Body
		</button>
		<button
			onclick={() => { requestTab = 'auth'; }}
			class={requestTab === 'auth' ? 'text-gx-neon border-b border-gx-neon pb-0.5' : 'text-gx-text-muted hover:text-gx-text-secondary'}
		>
			Auth {authType !== 'none' ? `(${authType})` : ''}
		</button>
	</div>

	<!-- Request config content -->
	<div class="shrink-0 max-h-36 overflow-auto border-b border-gx-border-subtle">
		{#if requestTab === 'headers'}
			<div class="p-2 space-y-1">
				{#each headers as header, i}
					<div class="flex items-center gap-1.5">
						<input type="checkbox" bind:checked={header.enabled} class="accent-gx-neon" />
						<input
							bind:value={header.key}
							placeholder="Header name"
							class="flex-1 bg-gx-bg-secondary text-[10px] text-gx-text-primary font-mono border border-gx-border-default rounded px-1.5 py-0.5 outline-none focus:border-gx-neon"
						/>
						<input
							bind:value={header.value}
							placeholder="Value"
							class="flex-1 bg-gx-bg-secondary text-[10px] text-gx-text-primary font-mono border border-gx-border-default rounded px-1.5 py-0.5 outline-none focus:border-gx-neon"
						/>
						<button onclick={() => removeHeader(i)} class="p-0.5 text-gx-text-muted hover:text-gx-status-error">
							<Trash2 size={9} />
						</button>
					</div>
				{/each}
				<button onclick={addHeader} class="flex items-center gap-1 text-[10px] text-gx-text-muted hover:text-gx-neon mt-1">
					<Plus size={9} /> Add header
				</button>
			</div>
		{:else if requestTab === 'body'}
			<div class="p-2">
				<div class="flex items-center gap-2 mb-1.5">
					{#each (['none', 'json', 'form', 'text'] as const) as bt}
						<button
							onclick={() => { bodyType = bt; }}
							class={`text-[10px] px-1.5 py-0.5 rounded ${bodyType === bt ? 'bg-gx-neon/20 text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}`}
						>
							{bt}
						</button>
					{/each}
				</div>
				{#if bodyType !== 'none'}
					<textarea
						bind:value={bodyContent}
						placeholder={bodyType === 'json' ? '{ "key": "value" }' : bodyType === 'form' ? 'key=value\nkey2=value2' : 'Raw body...'}
						rows="3"
						class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary font-mono border border-gx-border-default rounded px-2 py-1 outline-none resize-none focus:border-gx-neon"
						spellcheck="false"
					></textarea>
				{/if}
			</div>
		{:else if requestTab === 'auth'}
			<div class="p-2">
				<select
					bind:value={authType}
					class="bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-1.5 py-0.5 outline-none focus:border-gx-neon mb-1.5"
				>
					<option value="none">No Auth</option>
					<option value="bearer">Bearer Token</option>
					<option value="basic">Basic Auth</option>
					<option value="api_key">API Key</option>
				</select>

				{#if authType === 'bearer'}
					<input
						bind:value={authToken}
						placeholder="Bearer token"
						type="password"
						class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary font-mono border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon"
					/>
				{:else if authType === 'basic'}
					<div class="space-y-1">
						<input bind:value={authUser} placeholder="Username" class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon" />
						<input bind:value={authPass} placeholder="Password" type="password" class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon" />
					</div>
				{:else if authType === 'api_key'}
					<div class="space-y-1">
						<input bind:value={authKeyName} placeholder="Key name (e.g. X-API-Key)" class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon" />
						<input bind:value={authKeyValue} placeholder="Key value" type="password" class="w-full bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-2 py-1 outline-none focus:border-gx-neon" />
						<select bind:value={authKeyLocation} class="bg-gx-bg-secondary text-[10px] text-gx-text-primary border border-gx-border-default rounded px-1.5 py-0.5 outline-none">
							<option value="header">Header</option>
							<option value="query">Query Parameter</option>
						</select>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<!-- Error display -->
	{#if error}
		<div class="flex items-center gap-1.5 px-3 py-1 bg-gx-status-error/10 text-gx-status-error text-xs shrink-0">
			<span class="truncate">{error}</span>
			<button onclick={() => { error = ''; }} class="ml-auto shrink-0">
				<Trash2 size={10} />
			</button>
		</div>
	{/if}

	<!-- Response section -->
	<div class="flex-1 flex flex-col overflow-hidden">
		{#if response}
			<!-- Response header -->
			<div class="flex items-center gap-3 px-3 py-1 border-b border-gx-border-subtle text-xs shrink-0">
				<span class={`font-bold ${statusColor(response.status)}`}>
					{response.status} {response.status_text}
				</span>
				<span class="text-gx-text-disabled">{response.elapsed_ms.toFixed(0)}ms</span>
				<span class="text-gx-text-disabled">{formatSize(response.size_bytes)}</span>

				<div class="flex items-center gap-2 ml-auto">
					<button
						onclick={() => { responseTab = 'body'; }}
						class={responseTab === 'body' ? 'text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}
					>
						Body
					</button>
					<button
						onclick={() => { responseTab = 'headers'; }}
						class={responseTab === 'headers' ? 'text-gx-neon' : 'text-gx-text-muted hover:text-gx-text-secondary'}
					>
						Headers ({Object.keys(response.headers).length})
					</button>
					<button
						onclick={() => navigator.clipboard.writeText(response?.body || '')}
						class="p-0.5 text-gx-text-muted hover:text-gx-text-secondary"
						title="Copy response"
					>
						<Copy size={10} />
					</button>
				</div>
			</div>

			<!-- Response content -->
			<div class="flex-1 overflow-auto">
				{#if responseTab === 'body'}
					<pre class="p-3 text-[10px] text-gx-text-secondary font-mono whitespace-pre-wrap break-words">{response.content_type?.includes('json') ? prettyJson(response.body) : response.body}</pre>
				{:else}
					<div class="p-2 space-y-0.5">
						{#each Object.entries(response.headers) as [key, val]}
							<div class="flex gap-2 text-[10px]">
								<span class="text-gx-accent-cyan font-mono font-medium">{key}:</span>
								<span class="text-gx-text-muted font-mono break-all">{val}</span>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{:else}
			<div class="flex-1 flex items-center justify-center">
				<div class="text-center text-gx-text-disabled">
					<Send size={24} class="mx-auto mb-2 opacity-30" />
					<p class="text-xs">Send a request to see the response</p>
					<p class="text-[10px] mt-1">Ctrl+Enter to send</p>
				</div>
			</div>
		{/if}
	</div>
</div>
