<script lang="ts">
	import { page } from '$app/stores';
	import { AlertTriangle, RefreshCw, Home, ArrowLeft } from '@lucide/svelte';
</script>

<!-- SvelteKit error page — catches route-level errors and 404s -->
<div class="flex items-center justify-center h-screen bg-gx-bg-primary text-gx-text-primary">
	<div class="flex flex-col items-center gap-6 max-w-md px-8 text-center">
		<!-- Error icon -->
		<div class="flex items-center justify-center w-16 h-16 rounded-full bg-gx-status-error/10 border border-gx-status-error/30">
			<AlertTriangle size={32} class="text-gx-status-error" />
		</div>

		<!-- Error code -->
		<h1 class="text-5xl font-bold text-gx-neon font-mono">
			{$page.status}
		</h1>

		<!-- Error message -->
		<p class="text-lg text-gx-text-secondary">
			{#if $page.status === 404}
				Page not found
			{:else}
				{$page.error?.message ?? 'Something went wrong'}
			{/if}
		</p>

		<!-- Actions -->
		<div class="flex items-center gap-3 mt-4">
			<button
				onclick={() => history.back()}
				class="flex items-center gap-2 px-4 py-2 text-sm rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
			>
				<ArrowLeft size={16} />
				Go Back
			</button>

			<a
				href="/"
				class="flex items-center gap-2 px-4 py-2 text-sm rounded-gx bg-gx-neon text-gx-bg-primary font-medium hover:opacity-90 transition-opacity"
			>
				<Home size={16} />
				Dashboard
			</a>

			<button
				onclick={() => location.reload()}
				class="flex items-center gap-2 px-4 py-2 text-sm rounded-gx border border-gx-border-default text-gx-text-secondary hover:bg-gx-bg-hover transition-colors"
			>
				<RefreshCw size={16} />
				Reload
			</button>
		</div>

		<!-- Technical details (dev mode) -->
		{#if $page.error?.message}
			<details class="mt-6 w-full text-left">
				<summary class="text-xs text-gx-text-muted cursor-pointer hover:text-gx-text-secondary">
					Technical Details
				</summary>
				<pre class="mt-2 p-3 rounded-gx bg-gx-bg-secondary border border-gx-border-default text-xs text-gx-text-muted overflow-x-auto font-mono">
{$page.error.message}
				</pre>
			</details>
		{/if}
	</div>
</div>
