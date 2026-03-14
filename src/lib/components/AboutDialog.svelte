<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- AboutDialog — Version, license, and system information for ImpForge -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { Info, ExternalLink, X, Shield, Cpu } from '@lucide/svelte';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	let appVersion = $state('0.0.0');
	let tauriVersion = $state('--');
	let osPlatform = $state('--');
	let osArch = $state('--');

	onMount(async () => {
		// Fetch app metadata from Tauri APIs with fallback for dev/SSR
		try {
			const { getVersion, getTauriVersion } = await import('@tauri-apps/api/app');
			appVersion = await getVersion();
			tauriVersion = await getTauriVersion();
		} catch {
			// Dev mode or SSR — read from package.json fallback
			appVersion = '0.6.0';
			tauriVersion = 'dev';
		}

		try {
			const { platform, arch } = await import('@tauri-apps/plugin-os');
			osPlatform = platform();
			osArch = arch();
		} catch {
			// Fallback for dev mode without the native plugin
			osPlatform = navigator.platform?.toLowerCase().includes('win')
				? 'windows'
				: navigator.platform?.toLowerCase().includes('mac')
					? 'macos'
					: 'linux';
			osArch = navigator.userAgent.includes('x86_64') || navigator.userAgent.includes('x64')
				? 'x86_64'
				: navigator.userAgent.includes('aarch64') || navigator.userAgent.includes('arm64')
					? 'aarch64'
					: 'unknown';
		}
	});

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onclose();
		}
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
		role="dialog"
		aria-modal="true"
		aria-label="About ImpForge"
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<div class="relative w-full max-w-md mx-4 bg-gx-bg-secondary border border-gx-border-default rounded-gx-lg shadow-gx-glow overflow-hidden">

			<!-- Close button -->
			<button
				onclick={onclose}
				class="absolute top-3 right-3 p-1.5 rounded-gx text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover transition-colors z-10"
				aria-label="Close dialog"
			>
				<X size={16} />
			</button>

			<!-- Header -->
			<div class="px-6 pt-6 pb-4 text-center border-b border-gx-border-default bg-gx-bg-tertiary">
				<div class="flex items-center justify-center w-12 h-12 mx-auto mb-3 rounded-gx-lg bg-gx-neon/10 border border-gx-neon/30">
					<Info size={24} class="text-gx-neon" />
				</div>
				<h2 class="text-lg font-bold text-gx-text-primary tracking-tight">ImpForge</h2>
				<p class="text-xs text-gx-text-muted mt-0.5">AI Workstation Builder</p>
				<p class="mt-2 font-mono text-sm text-gx-neon">v{appVersion}</p>
			</div>

			<!-- Content -->
			<div class="px-6 py-4 space-y-4">

				<!-- System Info -->
				<div class="space-y-2">
					<div class="flex items-center gap-2 text-xs font-semibold text-gx-text-secondary uppercase tracking-wider">
						<Cpu size={12} class="text-gx-neon" />
						<span>System</span>
					</div>
					<div class="grid grid-cols-2 gap-2">
						<div class="px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
							<span class="block text-[10px] text-gx-text-muted">Platform</span>
							<span class="text-xs font-medium text-gx-text-primary capitalize">{osPlatform}</span>
						</div>
						<div class="px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
							<span class="block text-[10px] text-gx-text-muted">Architecture</span>
							<span class="text-xs font-medium text-gx-text-primary">{osArch}</span>
						</div>
						<div class="px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
							<span class="block text-[10px] text-gx-text-muted">Tauri</span>
							<span class="text-xs font-medium text-gx-text-primary">{tauriVersion}</span>
						</div>
						<div class="px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default">
							<span class="block text-[10px] text-gx-text-muted">App Version</span>
							<span class="text-xs font-medium text-gx-text-primary">{appVersion}</span>
						</div>
					</div>
				</div>

				<!-- Licenses -->
				<div class="space-y-2">
					<div class="flex items-center gap-2 text-xs font-semibold text-gx-text-secondary uppercase tracking-wider">
						<Shield size={12} class="text-gx-neon" />
						<span>Licenses</span>
					</div>
					<div class="flex flex-wrap gap-2">
						<span class="inline-flex items-center gap-1.5 px-2.5 py-1 text-[11px] font-medium rounded-gx bg-gx-neon/10 text-gx-neon border border-gx-neon/30">
							Apache-2.0
							<span class="text-gx-text-muted">(App)</span>
						</span>
						<span class="inline-flex items-center gap-1.5 px-2.5 py-1 text-[11px] font-medium rounded-gx bg-purple-500/10 text-purple-400 border border-purple-500/30">
							BSL 1.1
							<span class="text-gx-text-muted">(Engine)</span>
						</span>
					</div>
				</div>

				<!-- Links -->
				<div class="space-y-2">
					<div class="flex items-center gap-2 text-xs font-semibold text-gx-text-secondary uppercase tracking-wider">
						<ExternalLink size={12} class="text-gx-neon" />
						<span>Links</span>
					</div>
					<div class="space-y-1.5">
						<a
							href="https://github.com/AiImpDevelopment/impforge"
							target="_blank"
							rel="noopener noreferrer"
							class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-secondary hover:text-gx-neon hover:border-gx-neon/30 transition-colors"
						>
							<ExternalLink size={12} class="shrink-0" />
							<span class="font-mono">github.com/AiImpDevelopment/impforge</span>
						</a>
						<a
							href="https://impforge.dev"
							target="_blank"
							rel="noopener noreferrer"
							class="flex items-center gap-2 px-3 py-2 rounded-gx bg-gx-bg-tertiary border border-gx-border-default text-xs text-gx-text-secondary hover:text-gx-neon hover:border-gx-neon/30 transition-colors"
						>
							<ExternalLink size={12} class="shrink-0" />
							<span class="font-mono">impforge.dev</span>
						</a>
					</div>
				</div>
			</div>

			<!-- Footer -->
			<div class="px-6 py-3 border-t border-gx-border-default bg-gx-bg-tertiary">
				<p class="text-[10px] text-gx-text-muted text-center">
					Built with Tauri, Svelte, and Rust. Your complete AI stack — one desktop app.
				</p>
			</div>
		</div>
	</div>
{/if}
