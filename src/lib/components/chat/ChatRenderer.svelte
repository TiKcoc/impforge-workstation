<script lang="ts">
	import { marked } from 'marked';
	import hljs from 'highlight.js/lib/core';
	// Register only common languages to keep bundle small
	import javascript from 'highlight.js/lib/languages/javascript';
	import typescript from 'highlight.js/lib/languages/typescript';
	import python from 'highlight.js/lib/languages/python';
	import rust from 'highlight.js/lib/languages/rust';
	import bash from 'highlight.js/lib/languages/bash';
	import json from 'highlight.js/lib/languages/json';
	import css from 'highlight.js/lib/languages/css';
	import xml from 'highlight.js/lib/languages/xml';
	import sql from 'highlight.js/lib/languages/sql';
	import yaml from 'highlight.js/lib/languages/yaml';
	import dockerfile from 'highlight.js/lib/languages/dockerfile';
	import markdown from 'highlight.js/lib/languages/markdown';
	import go from 'highlight.js/lib/languages/go';
	import csharp from 'highlight.js/lib/languages/csharp';

	hljs.registerLanguage('javascript', javascript);
	hljs.registerLanguage('js', javascript);
	hljs.registerLanguage('typescript', typescript);
	hljs.registerLanguage('ts', typescript);
	hljs.registerLanguage('python', python);
	hljs.registerLanguage('py', python);
	hljs.registerLanguage('rust', rust);
	hljs.registerLanguage('rs', rust);
	hljs.registerLanguage('bash', bash);
	hljs.registerLanguage('sh', bash);
	hljs.registerLanguage('shell', bash);
	hljs.registerLanguage('json', json);
	hljs.registerLanguage('css', css);
	hljs.registerLanguage('html', xml);
	hljs.registerLanguage('xml', xml);
	hljs.registerLanguage('sql', sql);
	hljs.registerLanguage('yaml', yaml);
	hljs.registerLanguage('yml', yaml);
	hljs.registerLanguage('dockerfile', dockerfile);
	hljs.registerLanguage('docker', dockerfile);
	hljs.registerLanguage('markdown', markdown);
	hljs.registerLanguage('md', markdown);
	hljs.registerLanguage('go', go);
	hljs.registerLanguage('csharp', csharp);
	hljs.registerLanguage('cs', csharp);

	interface Props {
		content: string;
		streaming?: boolean;
	}

	let { content, streaming = false }: Props = $props();

	// Configure marked with highlight.js
	const renderer = new marked.Renderer();

	renderer.code = ({ text, lang }: { text: string; lang?: string }) => {
		const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
		let highlighted: string;
		try {
			highlighted = language !== 'plaintext'
				? hljs.highlight(text, { language }).value
				: escapeHtml(text);
		} catch {
			highlighted = escapeHtml(text);
		}
		return `<div class="forge-code-block group relative my-3">
			<div class="flex items-center justify-between px-3 py-1.5 text-[10px] font-mono text-gx-text-muted bg-gx-bg-primary border border-gx-border-default rounded-t-lg border-b-0">
				<span>${language}</span>
				<button class="forge-copy-btn opacity-0 group-hover:opacity-100 transition-opacity text-gx-text-muted hover:text-gx-neon" data-code="${encodeURIComponent(text)}">Copy</button>
			</div>
			<pre class="p-3 bg-gx-bg-primary border border-gx-border-default rounded-b-lg overflow-x-auto text-xs leading-relaxed"><code class="hljs language-${language}">${highlighted}</code></pre>
		</div>`;
	};

	function escapeHtml(text: string): string {
		return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
	}

	marked.setOptions({ renderer, gfm: true, breaks: true });

	// Render with rAF debounce for streaming
	let renderedHtml = $state('');
	let rafId = 0;

	$effect(() => {
		const raw = content;
		if (rafId) cancelAnimationFrame(rafId);
		rafId = requestAnimationFrame(() => {
			renderedHtml = marked.parse(raw) as string;
		});
	});

	// Handle copy button clicks via event delegation
	function handleClick(e: MouseEvent) {
		const btn = (e.target as HTMLElement).closest('.forge-copy-btn');
		if (btn instanceof HTMLElement && btn.dataset.code) {
			navigator.clipboard.writeText(decodeURIComponent(btn.dataset.code));
			btn.textContent = 'Copied!';
			setTimeout(() => { btn.textContent = 'Copy'; }, 2000);
		}
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="forge-renderer prose prose-invert prose-sm max-w-none" onclick={handleClick}>
	{@html renderedHtml}
	{#if streaming}
		<span class="inline-block w-1.5 h-4 bg-gx-neon animate-pulse ml-0.5 align-text-bottom"></span>
	{/if}
</div>

<style>
	.forge-renderer :global(p) {
		margin: 0.25em 0;
	}
	.forge-renderer :global(ul),
	.forge-renderer :global(ol) {
		margin: 0.5em 0;
		padding-left: 1.5em;
	}
	.forge-renderer :global(li) {
		margin: 0.15em 0;
	}
	.forge-renderer :global(code:not(.hljs)) {
		background: var(--color-gx-bg-primary, #1a1a2e);
		border: 1px solid var(--color-gx-border-default, #2a2a3e);
		border-radius: 4px;
		padding: 0.1em 0.35em;
		font-size: 0.85em;
		font-family: 'JetBrains Mono Variable', monospace;
	}
	.forge-renderer :global(blockquote) {
		border-left: 3px solid var(--color-gx-neon, #00d4ff);
		padding-left: 0.75em;
		margin: 0.5em 0;
		opacity: 0.85;
	}
	.forge-renderer :global(a) {
		color: var(--color-gx-neon, #00d4ff);
		text-decoration: underline;
	}
	.forge-renderer :global(table) {
		border-collapse: collapse;
		margin: 0.5em 0;
		font-size: 0.85em;
		width: 100%;
	}
	.forge-renderer :global(th),
	.forge-renderer :global(td) {
		border: 1px solid var(--color-gx-border-default, #2a2a3e);
		padding: 0.35em 0.75em;
		text-align: left;
	}
	.forge-renderer :global(th) {
		background: var(--color-gx-bg-primary, #1a1a2e);
		font-weight: 600;
	}
	.forge-renderer :global(hr) {
		border-color: var(--color-gx-border-default, #2a2a3e);
		margin: 1em 0;
	}
	.forge-renderer :global(h1),
	.forge-renderer :global(h2),
	.forge-renderer :global(h3) {
		margin-top: 0.75em;
		margin-bottom: 0.25em;
		font-weight: 600;
	}
</style>
