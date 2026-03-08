/**
 * NEXUS Style Engine Store — BenikUI-Inspired Deep Sub-Component Customization
 *
 * Every widget decomposes into independently styleable sub-components.
 * This store manages fetching, caching, and applying per-component styles
 * from the Rust backend (SQLite persistence).
 *
 * Architecture:
 * - Each widget has a WidgetStyleMap with multiple ComponentStyles
 * - ComponentStyles are applied as inline CSS / CSS custom properties
 * - Changes are live-previewed and persisted to SQLite
 * - Style profiles allow switching between complete configurations
 *
 * References:
 * - BenikUI fractal customization (clean-room MIT reimplementation)
 * - CSS Custom Properties Level 2
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES (mirror Rust structs)
// ============================================================================

export type FontFamily = 'System' | 'Mono' | { Custom: string };
export type TextOutline = 'None' | 'Thin' | 'Medium' | 'Thick';
export type NumberFormat = 'Raw' | 'Abbreviated' | 'Percent' | 'CurrentMax' | 'CurrentMaxPercent' | 'Hidden';
export type BarFillDirection = 'LeftToRight' | 'RightToLeft' | 'BottomToTop' | 'TopToBottom';
export type BarTexture = 'Flat' | 'Gradient' | 'Striped' | 'Glossy' | 'Minimalist';
export type BorderPattern = 'Solid' | 'Dashed' | 'Dotted' | 'Double' | 'None';
export type GlowType = 'None' | 'BoxGlow' | 'TextGlow' | 'InnerGlow' | 'DualGlow';
export type AnimationType = 'None' | 'Fade' | 'Scale' | 'SlideIn' | 'PulseOnChange' | 'Flash' | 'CountUp' | 'Breathe';
export type Easing = 'Linear' | 'EaseIn' | 'EaseOut' | 'EaseInOut' | 'Spring';
export type BackgroundType = 'Solid' | 'LinearGradient' | 'RadialGradient' | 'ConicGradient' | 'Pattern' | 'Transparent';
export type GraphType = 'Sparkline' | 'Area' | 'Line' | 'BarChart' | 'Gauge' | 'Donut';

export interface TextStyle {
	font_family: FontFamily;
	font_size: number;
	font_weight: number;
	color: string;
	outline: TextOutline;
	shadow: string | null;
	offset: [number, number];
	number_format: NumberFormat;
	letter_spacing: number;
	text_transform: string;
	opacity: number;
	visible: boolean;
}

export interface ColorThreshold {
	threshold: number;
	color: string;
}

export interface BarStyle {
	color: string;
	background_color: string;
	texture: BarTexture;
	fill_direction: BarFillDirection;
	border_radius: number;
	height: number;
	gradient: boolean;
	gradient_end_color: string | null;
	color_thresholds: ColorThreshold[];
	animate_changes: boolean;
	animation_duration_ms: number;
	spark_effect: boolean;
	offset: [number, number];
	opacity: number;
	visible: boolean;
}

export interface BorderStyle {
	pattern: BorderPattern;
	width: number;
	color: string;
	radius: number;
	opacity: number;
	visible: boolean;
}

export interface GlowStyle {
	glow_type: GlowType;
	color: string;
	intensity: number;
	blur: number;
	animated: boolean;
	pulse_duration_ms: number;
	opacity: number;
}

export interface AnimationConfig {
	animation_type: AnimationType;
	duration_ms: number;
	easing: Easing;
	delay_ms: number;
	repeat: boolean;
	respect_reduced_motion: boolean;
}

export interface BackgroundStyle {
	bg_type: BackgroundType;
	color: string;
	color_end: string | null;
	gradient_angle: number;
	pattern: string | null;
	animated: boolean;
	animation_duration_ms: number;
	opacity: number;
	backdrop_blur: number;
}

export interface ComponentStyle {
	id: string;
	label: string;
	parent_id: string | null;
	offset: [number, number];
	size: [number, number] | null;
	z_index: number;
	background: BackgroundStyle;
	border: BorderStyle;
	glow: GlowStyle;
	text: TextStyle | null;
	bar: BarStyle | null;
	animation: AnimationConfig;
	padding: [number, number, number, number];
	visible: boolean;
}

export interface WidgetStyleMap {
	widget_id: string;
	components: ComponentStyle[];
}

export interface FontEntry {
	name: string;
	family: string;
	category: string;
	is_variable: boolean;
	bundled: boolean;
}

export interface GraphStyle {
	graph_type: GraphType;
	color: string;
	fill_opacity: number;
	line_width: number;
	show_points: boolean;
	show_grid: boolean;
	grid_color: string;
	show_labels: boolean;
	data_points: number;
	animate: boolean;
	smooth: boolean;
}

export interface StyleProfile {
	id: string;
	name: string;
	description: string | null;
	created_at: string;
	updated_at: string;
}

// ============================================================================
// STATE
// ============================================================================

let widgetStyles = $state<Map<string, WidgetStyleMap>>(new Map());
let fonts = $state<FontEntry[]>([]);
let profiles = $state<StyleProfile[]>([]);
let activeProfile = $state<string>('default');
let isLoading = $state(false);
let error = $state<string | null>(null);
let editingComponent = $state<string | null>(null); // "widget-id.component-id"

// ============================================================================
// STYLE RESOLUTION — Convert ComponentStyle to CSS
// ============================================================================

/** Convert a FontFamily to CSS font-family string */
function fontFamilyToCSS(ff: FontFamily): string {
	if (ff === 'System') return 'system-ui, sans-serif';
	if (ff === 'Mono') return "'JetBrains Mono', 'Fira Code', monospace";
	if (typeof ff === 'object' && 'Custom' in ff) return ff.Custom;
	return 'system-ui, sans-serif';
}

/** Convert a TextOutline to CSS text-stroke */
function textOutlineToCSS(outline: TextOutline, color: string): string {
	switch (outline) {
		case 'Thin': return `-webkit-text-stroke: 1px ${color}40;`;
		case 'Medium': return `-webkit-text-stroke: 2px ${color}60;`;
		case 'Thick': return `-webkit-text-stroke: 3px ${color}80;`;
		default: return '';
	}
}

/** Convert a GlowStyle to CSS box-shadow or text-shadow */
function glowToCSS(glow: GlowStyle): string {
	if (glow.glow_type === 'None') return '';
	const color = glow.color;
	const blur = glow.blur;
	const spread = glow.intensity;
	const opacity = glow.opacity;

	// Parse hex to rgba
	const r = parseInt(color.slice(1, 3), 16);
	const g = parseInt(color.slice(3, 5), 16);
	const b = parseInt(color.slice(5, 7), 16);
	const rgba = `rgba(${r}, ${g}, ${b}, ${opacity})`;

	switch (glow.glow_type) {
		case 'BoxGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px ${rgba};`;
		case 'TextGlow':
			return `text-shadow: 0 0 ${blur}px ${rgba};`;
		case 'InnerGlow':
			return `box-shadow: inset 0 0 ${blur}px ${spread}px ${rgba};`;
		case 'DualGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px ${rgba}, inset 0 0 ${blur * 0.5}px ${spread * 0.5}px ${rgba};`;
		default:
			return '';
	}
}

/** Convert BackgroundStyle to CSS background property */
function backgroundToCSS(bg: BackgroundStyle): string {
	switch (bg.bg_type) {
		case 'Solid':
			return `background-color: ${bg.color}; opacity: ${bg.opacity};`;
		case 'LinearGradient':
			return `background: linear-gradient(${bg.gradient_angle}deg, ${bg.color}, ${bg.color_end ?? bg.color}); opacity: ${bg.opacity};`;
		case 'RadialGradient':
			return `background: radial-gradient(circle, ${bg.color}, ${bg.color_end ?? bg.color}); opacity: ${bg.opacity};`;
		case 'ConicGradient':
			return `background: conic-gradient(from ${bg.gradient_angle}deg, ${bg.color}, ${bg.color_end ?? bg.color}, ${bg.color}); opacity: ${bg.opacity};`;
		case 'Transparent':
			return 'background: transparent;';
		default:
			return `background-color: ${bg.color};`;
	}
}

/** Convert a BorderStyle to CSS */
function borderToCSS(border: BorderStyle): string {
	if (border.pattern === 'None' || !border.visible) return '';
	const pattern = border.pattern.toLowerCase();
	return `border: ${border.width}px ${pattern} ${border.color}; border-radius: ${border.radius}px; border-opacity: ${border.opacity};`;
}

/** Convert AnimationConfig to CSS animation/transition properties */
function animationToCSS(anim: AnimationConfig): string {
	if (anim.animation_type === 'None') return '';
	const easing = anim.easing === 'Spring' ? 'cubic-bezier(0.175, 0.885, 0.32, 1.275)' :
		anim.easing === 'EaseIn' ? 'ease-in' :
		anim.easing === 'EaseOut' ? 'ease-out' :
		anim.easing === 'EaseInOut' ? 'ease-in-out' : 'linear';
	return `transition: all ${anim.duration_ms}ms ${easing} ${anim.delay_ms}ms;`;
}

/** Generate complete inline style string for a ComponentStyle */
export function componentToCSS(comp: ComponentStyle): string {
	if (!comp.visible) return 'display: none;';

	const parts: string[] = [];

	// Position offset
	if (comp.offset[0] !== 0 || comp.offset[1] !== 0) {
		parts.push(`transform: translate(${comp.offset[0]}px, ${comp.offset[1]}px);`);
	}

	// Size override
	if (comp.size) {
		parts.push(`width: ${comp.size[0]}px; height: ${comp.size[1]}px;`);
	}

	// Z-index
	if (comp.z_index !== 0) {
		parts.push(`z-index: ${comp.z_index};`);
	}

	// Background
	parts.push(backgroundToCSS(comp.background));

	// Backdrop blur
	if (comp.background.backdrop_blur > 0) {
		parts.push(`backdrop-filter: blur(${comp.background.backdrop_blur}px);`);
	}

	// Border
	parts.push(borderToCSS(comp.border));

	// Glow
	parts.push(glowToCSS(comp.glow));

	// Text styles
	if (comp.text) {
		const t = comp.text;
		parts.push(`font-family: ${fontFamilyToCSS(t.font_family)};`);
		parts.push(`font-size: ${t.font_size}px;`);
		parts.push(`font-weight: ${t.font_weight};`);
		parts.push(`color: ${t.color};`);
		parts.push(`letter-spacing: ${t.letter_spacing}px;`);
		parts.push(`text-transform: ${t.text_transform};`);
		if (t.shadow) parts.push(`text-shadow: ${t.shadow};`);
		parts.push(textOutlineToCSS(t.outline, t.color));
		if (t.offset[0] !== 0 || t.offset[1] !== 0) {
			parts.push(`transform: translate(${t.offset[0]}px, ${t.offset[1]}px);`);
		}
	}

	// Padding
	parts.push(`padding: ${comp.padding[0]}px ${comp.padding[1]}px ${comp.padding[2]}px ${comp.padding[3]}px;`);

	// Animation
	parts.push(animationToCSS(comp.animation));

	return parts.filter(p => p.length > 0).join(' ');
}

/** Generate CSS for a bar sub-component */
export function barToCSS(bar: BarStyle, value: number): { containerStyle: string; fillStyle: string; fillWidth: string } {
	if (!bar.visible) return { containerStyle: 'display: none;', fillStyle: '', fillWidth: '0%' };

	// Determine color based on thresholds
	let fillColor = bar.color;
	const sorted = [...bar.color_thresholds].sort((a, b) => a.threshold - b.threshold);
	for (const threshold of sorted) {
		if (value <= threshold.threshold) {
			fillColor = threshold.color;
			break;
		}
	}

	const containerStyle = [
		`background-color: ${bar.background_color};`,
		`border-radius: ${bar.border_radius}px;`,
		`height: ${bar.height}px;`,
		`opacity: ${bar.opacity};`,
		bar.offset[0] !== 0 || bar.offset[1] !== 0
			? `transform: translate(${bar.offset[0]}px, ${bar.offset[1]}px);`
			: '',
	].filter(s => s).join(' ');

	const fillParts = [`background-color: ${fillColor};`, `border-radius: ${bar.border_radius}px;`];
	if (bar.gradient && bar.gradient_end_color) {
		fillParts.push(`background: linear-gradient(90deg, ${fillColor}, ${bar.gradient_end_color});`);
	}
	if (bar.animate_changes) {
		fillParts.push(`transition: width ${bar.animation_duration_ms}ms ease-out;`);
	}

	const pct = Math.max(0, Math.min(100, value * 100));

	return {
		containerStyle,
		fillStyle: fillParts.join(' '),
		fillWidth: `${pct}%`,
	};
}

/** Format a number according to NumberFormat */
export function formatNumber(value: number, max: number | null, format: NumberFormat): string {
	switch (format) {
		case 'Raw':
			return value.toLocaleString();
		case 'Abbreviated': {
			if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
			if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
			return value.toString();
		}
		case 'Percent':
			return max ? `${Math.round((value / max) * 100)}%` : `${value}%`;
		case 'CurrentMax':
			return max != null ? `${value.toLocaleString()} / ${max.toLocaleString()}` : value.toLocaleString();
		case 'CurrentMaxPercent':
			return max != null
				? `${value.toLocaleString()} / ${max.toLocaleString()} (${Math.round((value / max) * 100)}%)`
				: value.toLocaleString();
		case 'Hidden':
			return '';
		default:
			return value.toString();
	}
}

// ============================================================================
// ACTIONS
// ============================================================================

/** Load styles for a specific widget */
async function loadWidgetStyle(widgetId: string): Promise<WidgetStyleMap | null> {
	try {
		const styles = await invoke<WidgetStyleMap>('style_get_widget', {
			widgetId,
			profileId: activeProfile
		});
		const updated = new Map(widgetStyles);
		updated.set(widgetId, styles);
		widgetStyles = updated;
		return styles;
	} catch (e) {
		error = String(e);
		return null;
	}
}

/** Save styles for a widget */
async function saveWidgetStyle(styles: WidgetStyleMap): Promise<void> {
	try {
		await invoke<string>('style_save_widget', { styles, profileId: activeProfile });
		const updated = new Map(widgetStyles);
		updated.set(styles.widget_id, styles);
		widgetStyles = updated;
	} catch (e) {
		error = String(e);
	}
}

/** Reset widget styles to defaults */
async function resetWidgetStyle(widgetId: string): Promise<void> {
	try {
		const defaults = await invoke<WidgetStyleMap>('style_reset_widget', {
			widgetId,
			profileId: activeProfile
		});
		const updated = new Map(widgetStyles);
		updated.set(widgetId, defaults);
		widgetStyles = updated;
	} catch (e) {
		error = String(e);
	}
}

/** Load available fonts */
async function loadFonts(): Promise<void> {
	try {
		fonts = await invoke<FontEntry[]>('style_list_fonts');
	} catch (e) {
		error = String(e);
	}
}

/** Load style profiles */
async function loadProfiles(): Promise<void> {
	try {
		profiles = await invoke<StyleProfile[]>('style_list_profiles');
	} catch (e) {
		error = String(e);
	}
}

/** Create a new style profile */
async function createProfile(id: string, name: string, description?: string): Promise<void> {
	try {
		await invoke<string>('style_create_profile', { id, name, description });
		await loadProfiles();
	} catch (e) {
		error = String(e);
	}
}

/** Delete a style profile */
async function deleteProfile(profileId: string): Promise<void> {
	try {
		await invoke<string>('style_delete_profile', { profileId });
		if (activeProfile === profileId) activeProfile = 'default';
		await loadProfiles();
	} catch (e) {
		error = String(e);
	}
}

/** Switch active profile and reload all cached styles */
async function setActiveProfile(profileId: string): Promise<void> {
	activeProfile = profileId;
	// Reload all cached widget styles with new profile
	const widgetIds = [...widgetStyles.keys()];
	for (const id of widgetIds) {
		await loadWidgetStyle(id);
	}
}

/** Update a single component style within a widget */
function updateComponentStyle(widgetId: string, componentId: string, updates: Partial<ComponentStyle>): void {
	const existing = widgetStyles.get(widgetId);
	if (!existing) return;

	const updatedComponents = existing.components.map(c => {
		if (c.id === componentId) {
			return { ...c, ...updates };
		}
		return c;
	});

	const updated = new Map(widgetStyles);
	updated.set(widgetId, { ...existing, components: updatedComponents });
	widgetStyles = updated;
}

/** Get a specific component style */
function getComponentStyle(widgetId: string, componentId: string): ComponentStyle | undefined {
	return widgetStyles.get(widgetId)?.components.find(c => c.id === componentId);
}

// ============================================================================
// EXPORT
// ============================================================================

export const styleEngine = {
	// State
	get widgetStyles() { return widgetStyles; },
	get fonts() { return fonts; },
	get profiles() { return profiles; },
	get activeProfile() { return activeProfile; },
	get isLoading() { return isLoading; },
	get error() { return error; },
	get editingComponent() { return editingComponent; },
	set editingComponent(id: string | null) { editingComponent = id; },

	// Actions
	loadWidgetStyle,
	saveWidgetStyle,
	resetWidgetStyle,
	loadFonts,
	loadProfiles,
	createProfile,
	deleteProfile,
	setActiveProfile,
	updateComponentStyle,
	getComponentStyle,

	// CSS helpers (pure functions, exported for components)
	componentToCSS,
	barToCSS,
	formatNumber,
};
