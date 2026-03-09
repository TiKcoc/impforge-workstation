/**
 * Lazy font loader — loads extended fonts on demand instead of at startup.
 * Core fonts (Inter, JetBrains Mono, Space Grotesk) are loaded eagerly in +layout.svelte.
 * Extended fonts are loaded only when a user selects them in the Style Editor.
 *
 * License: Apache-2.0
 */

const loaded = new Set<string>();

/** Fontsource package map — font name to dynamic CSS import */
const FONT_MAP: Record<string, () => Promise<unknown>> = {};

// Build font map with suppressed type errors (fontsource ships CSS, not TS)
function register(name: string, loader: () => Promise<unknown>) {
	FONT_MAP[name] = loader;
}

/* eslint-disable @typescript-eslint/ban-ts-comment */
// @ts-ignore
register('Outfit Variable', () => import('@fontsource-variable/outfit'));
// @ts-ignore
register('Fira Code Variable', () => import('@fontsource-variable/fira-code'));
// @ts-ignore
register('Orbitron Variable', () => import('@fontsource-variable/orbitron'));
// @ts-ignore
register('Exo 2 Variable', () => import('@fontsource-variable/exo-2'));
// @ts-ignore
register('Geist Mono Variable', () => import('@fontsource-variable/geist-mono'));
// @ts-ignore
register('Comfortaa Variable', () => import('@fontsource-variable/comfortaa'));
// @ts-ignore
register('DM Sans Variable', () => import('@fontsource-variable/dm-sans'));
// @ts-ignore
register('Nunito Variable', () => import('@fontsource-variable/nunito'));
// @ts-ignore
register('Recursive Variable', () => import('@fontsource-variable/recursive'));
// @ts-ignore
register('Mona Sans Variable', () => import('@fontsource-variable/mona-sans'));
// @ts-ignore
register('Roboto Flex Variable', () => import('@fontsource-variable/roboto-flex'));
// @ts-ignore
register('Oxanium Variable', () => import('@fontsource-variable/oxanium'));
// @ts-ignore
register('Montserrat Variable', () => import('@fontsource-variable/montserrat'));
// @ts-ignore
register('Plus Jakarta Sans Variable', () => import('@fontsource-variable/plus-jakarta-sans'));
// @ts-ignore
register('Sora Variable', () => import('@fontsource-variable/sora'));

/** FontFamily → required font names */
const FAMILY_FONTS: Record<string, string[]> = {
	Geometric:  ['Outfit Variable', 'DM Sans Variable'],
	Rounded:    ['Comfortaa Variable'],
	Futuristic: ['Orbitron Variable', 'Exo 2 Variable'],
	Gaming:     ['Oxanium Variable'],
};

/**
 * Load a specific font by name. No-op if already loaded.
 */
export async function loadFont(name: string): Promise<void> {
	if (loaded.has(name)) return;
	const loader = FONT_MAP[name];
	if (loader) {
		await loader();
		loaded.add(name);
	}
}

/**
 * Load all fonts needed for a FontFamily selection.
 * Call this when the user picks a font family in the Style Editor.
 */
export async function loadFontsForFamily(family: string): Promise<void> {
	const fonts = FAMILY_FONTS[family];
	if (fonts) {
		await Promise.all(fonts.map(loadFont));
	}
}

/**
 * Preload all extended fonts (for users who want instant font switching).
 */
export async function preloadAllFonts(): Promise<void> {
	await Promise.all(Object.keys(FONT_MAP).map(loadFont));
}

/** Check if a font is loaded */
export function isFontLoaded(name: string): boolean {
	return loaded.has(name);
}
