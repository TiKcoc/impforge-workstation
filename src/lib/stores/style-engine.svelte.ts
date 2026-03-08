/**
 * ImpForge Style Engine Store — BenikUI-Inspired Deep Sub-Component Customization
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

// Font families — 14 variants (maps to Fontsource bundled + system fonts)
export type FontFamily =
	| 'System' | 'Mono' | 'Display' | 'Geometric' | 'Rounded' | 'Condensed'
	| 'Handwriting' | 'Pixel' | 'Serif' | 'SlabSerif' | 'Futuristic' | 'Gaming'
	| 'TabularNums' | { Custom: string };

// Text outline styles — 8 variants
export type TextOutline =
	| 'None' | 'Thin' | 'Medium' | 'Thick'
	| 'ColoredThin' | 'DoubleOutline' | 'ShadowOutline' | 'Embossed';

// Number formatting — 12 variants
export type NumberFormat =
	| 'Raw' | 'Abbreviated' | 'Percent' | 'CurrentMax' | 'CurrentMaxPercent' | 'Hidden'
	| 'Deficit' | 'Bytes' | 'Duration' | 'Scientific' | 'CompactWithUnit' | 'Locale';

export type BarFillDirection = 'LeftToRight' | 'RightToLeft' | 'BottomToTop' | 'TopToBottom';

// Bar textures — 20 variants
export type BarTexture =
	| 'Flat' | 'Gradient' | 'Striped' | 'Glossy' | 'Minimalist'
	| 'Noise' | 'Lined' | 'Pixelated' | 'Checkerboard' | 'BrushedMetal'
	| 'Diamond' | 'Honeycomb' | 'Circuit' | 'Wave' | 'Frosted'
	| 'CarbonFiber' | 'Scanline' | 'NeonEdge' | 'DualTone' | 'Shimmer';

// Border patterns — 15 variants
export type BorderPattern =
	| 'Solid' | 'Dashed' | 'Dotted' | 'Double' | 'None'
	| 'Ridge' | 'Groove' | 'Inset' | 'Outset' | 'NeonGlow'
	| 'GradientBorder' | 'MarchingAnts' | 'Corners' | 'Pill' | 'PixelBorder';

// Glow types — 15 variants
export type GlowType =
	| 'None' | 'BoxGlow' | 'TextGlow' | 'InnerGlow' | 'DualGlow'
	| 'NeonMultiLayer' | 'AmbientGlow' | 'EdgeGlow' | 'PulsingRing'
	| 'FireGlow' | 'FrostGlow' | 'ElectricGlow' | 'HolographicGlow'
	| 'DropShadow' | 'NeonUnderline';

// Animation types — 20 variants
export type AnimationType =
	| 'None' | 'Fade' | 'Scale' | 'SlideIn' | 'PulseOnChange' | 'Flash' | 'CountUp' | 'Breathe'
	| 'Bounce' | 'Elastic' | 'Flip' | 'Typewriter' | 'Shake' | 'BlurIn' | 'Glitch'
	| 'MatrixRain' | 'Ripple' | 'Morph' | 'StaggerChildren' | 'Heartbeat';

// Easing functions — 15 variants
export type Easing =
	| 'Linear' | 'EaseIn' | 'EaseOut' | 'EaseInOut' | 'Spring'
	| 'BounceOut' | 'ElasticOut' | 'BackOut' | 'SmoothStep'
	| 'ExpoIn' | 'ExpoOut' | 'CircOut' | 'SineInOut' | 'Steps' | 'CustomBezier';

// Background types — 20 variants
export type BackgroundType =
	| 'Solid' | 'LinearGradient' | 'RadialGradient' | 'ConicGradient' | 'Pattern' | 'Transparent'
	| 'Glass' | 'MeshGradient' | 'AnimatedGradient' | 'DiagonalStripes' | 'DotGrid'
	| 'Crosshatch' | 'Hexagons' | 'CarbonFiber' | 'CircuitBoard' | 'Starfield'
	| 'Topographic' | 'NoiseGradient' | 'Waves' | 'Image';

// Graph types — 20 variants
export type GraphType =
	| 'Sparkline' | 'Area' | 'Line' | 'BarChart' | 'Gauge' | 'Donut'
	| 'StackedArea' | 'HorizontalBar' | 'GroupedBar' | 'Radar' | 'Heatmap'
	| 'TreeMap' | 'Scatter' | 'Waterfall' | 'Funnel' | 'Candlestick'
	| 'Sankey' | 'Bubble' | 'StepLine' | 'PolarArea';

// Theme presets — 20 variants (one-click UI restyling)
export type ThemePreset =
	| 'ImpForgeDefault' | 'Cyberpunk' | 'Arctic' | 'Ember' | 'Imperial'
	| 'Matrix' | 'Corporate' | 'Synthwave' | 'Forest' | 'Daylight'
	| 'DeepSea' | 'Crimson' | 'Candy' | 'Monochrome' | 'SolarFlare'
	| 'HighContrast' | 'Minimal' | 'Retro8Bit' | 'Holographic'
	| { Custom: string };

// Theme color palette
export interface ThemePalette {
	accent: string;
	accent_secondary: string;
	bg_primary: string;
	bg_secondary: string;
	bg_tertiary: string;
	border: string;
	text_primary: string;
	text_secondary: string;
	text_muted: string;
	status_success: string;
	status_warning: string;
	status_danger: string;
	neon: string;
}

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
	if (typeof ff === 'object' && 'Custom' in ff) return ff.Custom;
	switch (ff) {
		case 'System': return "'Inter Variable', system-ui, sans-serif";
		case 'Mono': return "'JetBrains Mono Variable', 'Fira Code', monospace";
		case 'Display': return "'Space Grotesk Variable', sans-serif";
		case 'Geometric': return "'Outfit Variable', 'DM Sans Variable', sans-serif";
		case 'Rounded': return "'Quicksand Variable', 'Comfortaa Variable', sans-serif";
		case 'Condensed': return "'Barlow Condensed', 'Oswald Variable', sans-serif";
		case 'Handwriting': return "'Caveat Variable', 'Patrick Hand', cursive";
		case 'Pixel': return "'Press Start 2P', 'VT323', 'Share Tech Mono', monospace";
		case 'Serif': return "'Playfair Display Variable', 'Lora Variable', serif";
		case 'SlabSerif': return "'Roboto Slab Variable', 'Zilla Slab', serif";
		case 'Futuristic': return "'Orbitron Variable', 'Exo 2 Variable', sans-serif";
		case 'Gaming': return "'Chakra Petch', 'Audiowide', 'Russo One', sans-serif";
		case 'TabularNums': return "'JetBrains Mono Variable', monospace; font-variant-numeric: tabular-nums";
		default: return 'system-ui, sans-serif';
	}
}

/** Convert a TextOutline to CSS text-stroke / text-shadow */
function textOutlineToCSS(outline: TextOutline, color: string): string {
	switch (outline) {
		case 'Thin': return `-webkit-text-stroke: 1px ${color}40;`;
		case 'Medium': return `-webkit-text-stroke: 2px ${color}60;`;
		case 'Thick': return `-webkit-text-stroke: 3px ${color}80;`;
		case 'ColoredThin': return `-webkit-text-stroke: 1px ${color};`;
		case 'DoubleOutline': return `-webkit-text-stroke: 2px white; paint-order: stroke fill; text-shadow: 0 0 4px ${color};`;
		case 'ShadowOutline': return `text-shadow: 1px 1px 0 ${color}80, -1px -1px 0 ${color}80, 1px -1px 0 ${color}80, -1px 1px 0 ${color}80;`;
		case 'Embossed': return `text-shadow: 1px 1px 1px rgba(255,255,255,0.3), -1px -1px 1px rgba(0,0,0,0.5);`;
		default: return '';
	}
}

/** Parse hex color to rgba with opacity */
function hexToRgba(hex: string, opacity: number): string {
	const r = parseInt(hex.slice(1, 3), 16);
	const g = parseInt(hex.slice(3, 5), 16);
	const b = parseInt(hex.slice(5, 7), 16);
	return `rgba(${r}, ${g}, ${b}, ${opacity})`;
}

/** Convert a GlowStyle to CSS box-shadow or text-shadow */
function glowToCSS(glow: GlowStyle): string {
	if (glow.glow_type === 'None') return '';
	const { color, blur, intensity: spread, opacity } = glow;
	const rgba = hexToRgba(color, opacity);
	const rgbaHalf = hexToRgba(color, opacity * 0.5);

	switch (glow.glow_type) {
		case 'BoxGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px ${rgba};`;
		case 'TextGlow':
			return `text-shadow: 0 0 ${blur}px ${rgba};`;
		case 'InnerGlow':
			return `box-shadow: inset 0 0 ${blur}px ${spread}px ${rgba};`;
		case 'DualGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px ${rgba}, inset 0 0 ${blur * 0.5}px ${spread * 0.5}px ${rgba};`;
		case 'NeonMultiLayer':
			return `box-shadow: 0 0 ${blur * 0.5}px 1px rgba(255,255,255,0.8), 0 0 ${blur}px ${spread}px ${rgba}, 0 0 ${blur * 2}px ${spread * 2}px ${rgbaHalf};`;
		case 'AmbientGlow':
			return `box-shadow: 0 0 ${blur * 3}px ${spread * 2}px ${rgbaHalf};`;
		case 'EdgeGlow':
			return `box-shadow: 0 0 ${blur * 0.3}px ${spread * 0.5}px ${rgba};`;
		case 'PulsingRing':
			return `box-shadow: 0 0 0 2px ${rgba}, 0 0 ${blur}px ${spread}px ${rgbaHalf};`;
		case 'FireGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px rgba(255, 100, 0, ${opacity}), 0 0 ${blur * 2}px ${spread * 1.5}px rgba(255, 50, 0, ${opacity * 0.5});`;
		case 'FrostGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px rgba(136, 221, 255, ${opacity}), 0 0 ${blur * 1.5}px ${spread}px rgba(200, 240, 255, ${opacity * 0.3});`;
		case 'ElectricGlow':
			return `box-shadow: 0 0 ${blur * 0.5}px 1px rgba(255,255,255,0.9), 0 0 ${blur}px ${spread}px ${rgba}, 0 -2px ${blur}px ${spread * 0.5}px ${rgbaHalf};`;
		case 'HolographicGlow':
			return `box-shadow: 0 0 ${blur}px ${spread}px ${rgba}, ${blur * 0.3}px 0 ${blur}px rgba(255,0,128,${opacity * 0.4}), -${blur * 0.3}px 0 ${blur}px rgba(0,200,255,${opacity * 0.4});`;
		case 'DropShadow':
			return `box-shadow: 4px 4px ${blur}px ${rgba};`;
		case 'NeonUnderline':
			return `box-shadow: 0 2px ${blur * 0.5}px ${spread * 0.5}px ${rgba};`;
		default:
			return '';
	}
}

/** Convert BackgroundStyle to CSS background property */
function backgroundToCSS(bg: BackgroundStyle): string {
	const end = bg.color_end ?? bg.color;
	const o = bg.opacity;
	switch (bg.bg_type) {
		case 'Solid':
			return `background-color: ${bg.color}; opacity: ${o};`;
		case 'LinearGradient':
			return `background: linear-gradient(${bg.gradient_angle}deg, ${bg.color}, ${end}); opacity: ${o};`;
		case 'RadialGradient':
			return `background: radial-gradient(circle, ${bg.color}, ${end}); opacity: ${o};`;
		case 'ConicGradient':
			return `background: conic-gradient(from ${bg.gradient_angle}deg, ${bg.color}, ${end}, ${bg.color}); opacity: ${o};`;
		case 'Transparent':
			return 'background: transparent;';
		case 'Glass':
			return `background: ${bg.color}40; backdrop-filter: blur(${bg.backdrop_blur || 12}px); opacity: ${o};`;
		case 'MeshGradient':
			return `background: radial-gradient(at 0% 0%, ${bg.color} 0%, transparent 50%), radial-gradient(at 100% 0%, ${end} 0%, transparent 50%), radial-gradient(at 100% 100%, ${bg.color} 0%, transparent 50%), ${bg.color}20; opacity: ${o};`;
		case 'AnimatedGradient':
			return `background: linear-gradient(${bg.gradient_angle}deg, ${bg.color}, ${end}, ${bg.color}); background-size: 200% 200%; opacity: ${o};`;
		case 'DiagonalStripes':
			return `background: repeating-linear-gradient(45deg, ${bg.color}, ${bg.color} 2px, transparent 2px, transparent 8px); opacity: ${o};`;
		case 'DotGrid':
			return `background: radial-gradient(circle, ${bg.color}40 1px, transparent 1px); background-size: 12px 12px; background-color: ${end}; opacity: ${o};`;
		case 'Crosshatch':
			return `background: repeating-linear-gradient(0deg, ${bg.color}20, ${bg.color}20 1px, transparent 1px, transparent 8px), repeating-linear-gradient(90deg, ${bg.color}20, ${bg.color}20 1px, transparent 1px, transparent 8px); background-color: ${end}; opacity: ${o};`;
		case 'Hexagons':
			return `background-color: ${bg.color}; background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='49'%3E%3Cpath d='M14 0l14 24.5L14 49 0 24.5z' fill='none' stroke='${encodeURIComponent(end)}' stroke-opacity='0.15'/%3E%3C/svg%3E"); opacity: ${o};`;
		case 'CarbonFiber':
			return `background: repeating-linear-gradient(45deg, ${bg.color}, ${bg.color} 1px, ${end} 1px, ${end} 3px); opacity: ${o};`;
		case 'CircuitBoard':
			return `background: repeating-linear-gradient(0deg, transparent, transparent 2px, ${bg.color}10 2px, ${bg.color}10 4px); background-color: ${end}; opacity: ${o};`;
		case 'Starfield':
			return `background: radial-gradient(1px 1px at 20% 30%, white 0%, transparent 100%), radial-gradient(1px 1px at 40% 60%, white 0%, transparent 100%), radial-gradient(1px 1px at 80% 10%, white 0%, transparent 100%), radial-gradient(1px 1px at 60% 80%, rgba(255,255,255,0.6) 0%, transparent 100%); background-color: ${bg.color}; opacity: ${o};`;
		case 'Topographic':
			return `background-color: ${bg.color}; opacity: ${o};`;
		case 'NoiseGradient':
			return `background: linear-gradient(${bg.gradient_angle}deg, ${bg.color}, ${end}); opacity: ${o};`;
		case 'Waves':
			return `background-color: ${bg.color}; opacity: ${o};`;
		case 'Image':
			return bg.pattern ? `background-image: url('${bg.pattern}'); background-size: cover; background-position: center; opacity: ${o};` : `background-color: ${bg.color}; opacity: ${o};`;
		default:
			return `background-color: ${bg.color}; opacity: ${o};`;
	}
}

/** Convert a BorderStyle to CSS */
function borderToCSS(border: BorderStyle): string {
	if (border.pattern === 'None' || !border.visible) return '';
	const w = border.width;
	const c = border.color;
	const r = border.radius;
	switch (border.pattern) {
		case 'Solid': return `border: ${w}px solid ${c}; border-radius: ${r}px;`;
		case 'Dashed': return `border: ${w}px dashed ${c}; border-radius: ${r}px;`;
		case 'Dotted': return `border: ${w}px dotted ${c}; border-radius: ${r}px;`;
		case 'Double': return `border: ${w}px double ${c}; border-radius: ${r}px;`;
		case 'Ridge': return `border: ${w}px ridge ${c}; border-radius: ${r}px;`;
		case 'Groove': return `border: ${w}px groove ${c}; border-radius: ${r}px;`;
		case 'Inset': return `border: ${w}px inset ${c}; border-radius: ${r}px;`;
		case 'Outset': return `border: ${w}px outset ${c}; border-radius: ${r}px;`;
		case 'NeonGlow': return `border: ${w}px solid ${c}; border-radius: ${r}px; box-shadow: 0 0 4px ${c}, inset 0 0 4px ${c}40;`;
		case 'GradientBorder': return `border: ${w}px solid transparent; border-radius: ${r}px; background-clip: padding-box; box-shadow: inset 0 0 0 ${w}px ${c};`;
		case 'MarchingAnts': return `border: ${w}px dashed ${c}; border-radius: ${r}px;`;
		case 'Corners': return `border: 0; border-radius: 0; box-shadow: ${w}px ${w}px 0 -${w - 1}px ${c}, -${w}px ${w}px 0 -${w - 1}px ${c}, ${w}px -${w}px 0 -${w - 1}px ${c}, -${w}px -${w}px 0 -${w - 1}px ${c};`;
		case 'Pill': return `border: ${w}px solid ${c}; border-radius: 9999px;`;
		case 'PixelBorder': return `border: ${w}px solid ${c}; border-radius: 0; image-rendering: pixelated;`;
		default: return `border: ${w}px solid ${c}; border-radius: ${r}px;`;
	}
}

/** Convert Easing to CSS timing function */
function easingToCSS(e: Easing): string {
	switch (e) {
		case 'Linear': return 'linear';
		case 'EaseIn': return 'ease-in';
		case 'EaseOut': return 'ease-out';
		case 'EaseInOut': return 'ease-in-out';
		case 'Spring': return 'cubic-bezier(0.175, 0.885, 0.32, 1.275)';
		case 'BounceOut': return 'cubic-bezier(0.34, 1.56, 0.64, 1)';
		case 'ElasticOut': return 'cubic-bezier(0.68, -0.55, 0.27, 1.55)';
		case 'BackOut': return 'cubic-bezier(0.34, 1.56, 0.64, 1)';
		case 'SmoothStep': return 'cubic-bezier(0.4, 0, 0.2, 1)';
		case 'ExpoIn': return 'cubic-bezier(0.7, 0, 0.84, 0)';
		case 'ExpoOut': return 'cubic-bezier(0.16, 1, 0.3, 1)';
		case 'CircOut': return 'cubic-bezier(0, 0.55, 0.45, 1)';
		case 'SineInOut': return 'cubic-bezier(0.37, 0, 0.63, 1)';
		case 'Steps': return 'steps(8, end)';
		case 'CustomBezier': return 'cubic-bezier(0.25, 0.1, 0.25, 1)';
		default: return 'ease';
	}
}

/** Convert AnimationConfig to CSS animation/transition properties */
function animationToCSS(anim: AnimationConfig): string {
	if (anim.animation_type === 'None') return '';
	const easing = easingToCSS(anim.easing);
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
			if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`;
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
		case 'Deficit':
			return max != null ? `${(max - value).toLocaleString()} remaining` : value.toLocaleString();
		case 'Bytes': {
			const units = ['B', 'KB', 'MB', 'GB', 'TB'];
			let v = value; let u = 0;
			while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
			return `${v.toFixed(u > 0 ? 1 : 0)} ${units[u]}`;
		}
		case 'Duration': {
			const h = Math.floor(value / 3600);
			const m = Math.floor((value % 3600) / 60);
			const s = Math.floor(value % 60);
			return h > 0 ? `${h}h ${m}m ${s}s` : m > 0 ? `${m}m ${s}s` : `${s}s`;
		}
		case 'Scientific':
			return value.toExponential(2);
		case 'CompactWithUnit': {
			if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
			if (value >= 1_000) return `${(value / 1_000).toFixed(2)}K`;
			return value.toFixed(1);
		}
		case 'Locale':
			return value.toLocaleString(undefined, { maximumFractionDigits: 2 });
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
