import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// Tauri expects a fixed port during development
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit()
	],

	// Vite options tailored for Tauri development
	clearScreen: false,

	server: {
		port: 5173,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 5173,
				}
			: undefined,
		watch: {
			// Tell Vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},
	},

	// Env prefix for Tauri
	envPrefix: ['VITE_', 'TAURI_'],

	// Monaco Editor needs worker files to be accessible
	optimizeDeps: {
		include: ['monaco-editor'],
	},

	build: {
		// Tauri uses Chromium on Windows and WebKit on macOS and Linux
		target: process.env.TAURI_PLATFORM == 'windows'
			? 'chrome105'
			: 'safari14',
		// Don't minify for debug builds
		minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
		// Produce sourcemaps for debug builds
		sourcemap: !!process.env.TAURI_DEBUG,
	},
});
