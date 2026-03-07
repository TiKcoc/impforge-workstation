import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	// Consult https://svelte.dev/docs/kit/integrations
	// for more information about preprocessors
	preprocess: vitePreprocess(),

	kit: {
		// Use static adapter for Tauri (generates static files)
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: false,
			strict: true
		}),

		// Alias for cleaner imports
		alias: {
			'$components': './src/lib/components',
			'$stores': './src/lib/stores',
			'$agents': './src/lib/agents'
		}
	}
};

export default config;
