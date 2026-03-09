<script lang="ts">
	/**
	 * StyleEditor.svelte — BenikUI-Inspired Deep Sub-Component Style Editor
	 *
	 * Drill into any widget's sub-components and configure every detail:
	 * - Font face, size, weight, outline, shadow
	 * - Colors with alpha, gradients, glow/aura effects
	 * - Position offset from parent
	 * - Number format (percent, current/max, abbreviated)
	 * - Bar textures, thresholds, animations
	 * - Border styles, patterns, corner radius
	 * - Background effects (solid, gradient, glassmorphism)
	 * - Graph/chart display configuration
	 *
	 * Inspired by BenikUI's fractal customization where every
	 * sub-element has its own independent configuration panel.
	 */

	import { styleEngine, componentToCSS, type ComponentStyle, type TextStyle, type BarStyle, type GlowStyle, type BorderStyle, type BackgroundStyle, type AnimationConfig } from '$lib/stores/style-engine.svelte';
	import { X, ChevronDown, ChevronRight, RotateCcw, Save, Eye, EyeOff, Type, Palette, Sparkles, Move, BarChart3, Square, Layers } from '@lucide/svelte';
	import { loadFontsForFamily } from '$lib/utils/lazy-fonts';

	interface Props {
		widgetId: string;
		onClose: () => void;
	}

	let { widgetId, onClose }: Props = $props();

	let styles = $derived(styleEngine.widgetStyles.get(widgetId));
	let components = $derived(styles?.components ?? []);
	let selectedComponentId = $state<string | null>(null);
	let selectedComponent = $derived(
		selectedComponentId ? components.find(c => c.id === selectedComponentId) : null
	);
	let expandedSections = $state<Set<string>>(new Set(['background', 'text', 'border']));
	let isSaving = $state(false);
	let hasChanges = $state(false);

	// Load styles on mount
	$effect(() => {
		styleEngine.loadWidgetStyle(widgetId);
		styleEngine.loadFonts();
	});

	// Auto-select first component
	$effect(() => {
		if (components.length > 0 && !selectedComponentId) {
			selectedComponentId = components[0].id;
		}
	});

	function toggleSection(section: string) {
		const next = new Set(expandedSections);
		if (next.has(section)) next.delete(section);
		else next.add(section);
		expandedSections = next;
	}

	function updateComponent(updates: Partial<ComponentStyle>) {
		if (!selectedComponentId) return;
		styleEngine.updateComponentStyle(widgetId, selectedComponentId, updates);
		hasChanges = true;
	}

	function updateText(updates: Partial<TextStyle>) {
		if (!selectedComponent?.text) return;
		updateComponent({ text: { ...selectedComponent.text, ...updates } });
	}

	function updateBar(updates: Partial<BarStyle>) {
		if (!selectedComponent?.bar) return;
		updateComponent({ bar: { ...selectedComponent.bar, ...updates } });
	}

	function updateGlow(updates: Partial<GlowStyle>) {
		updateComponent({ glow: { ...selectedComponent!.glow, ...updates } });
	}

	function updateBorder(updates: Partial<BorderStyle>) {
		updateComponent({ border: { ...selectedComponent!.border, ...updates } });
	}

	function updateBackground(updates: Partial<BackgroundStyle>) {
		updateComponent({ background: { ...selectedComponent!.background, ...updates } });
	}

	function updateAnimation(updates: Partial<AnimationConfig>) {
		updateComponent({ animation: { ...selectedComponent!.animation, ...updates } });
	}

	async function handleSave() {
		if (!styles) return;
		isSaving = true;
		await styleEngine.saveWidgetStyle(styles);
		isSaving = false;
		hasChanges = false;
	}

	async function handleReset() {
		await styleEngine.resetWidgetStyle(widgetId);
		hasChanges = false;
	}

	// Icon for component type
	function componentIcon(comp: ComponentStyle) {
		if (comp.bar) return BarChart3;
		if (comp.text) return Type;
		if (comp.id.includes('container')) return Square;
		return Layers;
	}
</script>

<!-- BenikUI-Style Sub-Component Editor Panel -->
<div class="fixed right-0 top-0 bottom-0 w-[380px] bg-gx-bg-secondary border-l border-gx-border-default z-[100] flex flex-col shadow-[-4px_0_24px_rgba(0,0,0,0.5)]"
	role="dialog"
	aria-label="Widget style editor for {widgetId}"
>
	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-3 border-b border-gx-border-default bg-gx-bg-primary">
		<div>
			<h2 class="text-sm font-semibold text-gx-text-primary">Style Editor</h2>
			<p class="text-[10px] text-gx-text-muted font-mono">{widgetId}</p>
		</div>
		<div class="flex items-center gap-1">
			{#if hasChanges}
				<button
					onclick={handleSave}
					disabled={isSaving}
					class="px-2 py-1 text-[10px] font-semibold bg-gx-neon/20 text-gx-neon border border-gx-neon/30 rounded hover:bg-gx-neon/30 transition-colors target-size-min"
					aria-label="Save style changes"
				>
					<Save size={12} class="inline mr-1" />
					{isSaving ? 'Saving...' : 'Save'}
				</button>
			{/if}
			<button
				onclick={handleReset}
				class="p-1.5 text-gx-text-muted hover:text-gx-status-warning rounded hover:bg-gx-bg-hover transition-colors target-size-min"
				title="Reset to defaults"
				aria-label="Reset styles to defaults"
			>
				<RotateCcw size={14} />
			</button>
			<button
				onclick={onClose}
				class="p-1.5 text-gx-text-muted hover:text-gx-text-primary rounded hover:bg-gx-bg-hover transition-colors target-size-min"
				aria-label="Close style editor"
			>
				<X size={14} />
			</button>
		</div>
	</div>

	<!-- Component Tree (left sidebar within panel) -->
	<div class="border-b border-gx-border-default">
		<div class="px-3 py-2 text-[10px] font-semibold text-gx-text-muted uppercase tracking-wider">
			Sub-Components
		</div>
		<div class="max-h-[200px] overflow-y-auto px-1 pb-2" role="listbox" aria-label="Widget sub-components">
			{#each components as comp (comp.id)}
				{@const isSelected = comp.id === selectedComponentId}
				{@const depth = comp.parent_id ? 1 : 0}
				{@const IconComp = componentIcon(comp)}
				<button
					role="option"
					aria-selected={isSelected}
					onclick={() => { selectedComponentId = comp.id; }}
					class="w-full flex items-center gap-2 px-2 py-1.5 rounded text-left transition-colors
						{isSelected
							? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/20'
							: 'text-gx-text-secondary hover:bg-gx-bg-hover hover:text-gx-text-primary border border-transparent'}"
					style="margin-left: {depth * 16}px"
				>
					<IconComp size={12} class="shrink-0 {isSelected ? 'text-gx-neon' : 'text-gx-text-muted'}" />
					<span class="text-[11px] font-mono truncate flex-1">{comp.label}</span>
					{#if !comp.visible}
						<EyeOff size={10} class="text-gx-text-muted shrink-0" />
					{/if}
				</button>
			{/each}
		</div>
	</div>

	<!-- Style Properties (scrollable) -->
	{#if selectedComponent}
		<div class="flex-1 overflow-y-auto px-3 py-2 space-y-1">

			<!-- Visibility & Offset -->
			<div class="flex items-center justify-between py-2">
				<button
					onclick={() => updateComponent({ visible: !selectedComponent!.visible })}
					class="flex items-center gap-2 text-[11px] {selectedComponent.visible ? 'text-gx-neon' : 'text-gx-text-muted'}"
					aria-label="Toggle component visibility"
				>
					{#if selectedComponent.visible}
						<Eye size={14} /> Visible
					{:else}
						<EyeOff size={14} /> Hidden
					{/if}
				</button>
				<div class="flex items-center gap-1 text-[10px] text-gx-text-muted">
					<Move size={10} />
					<label class="sr-only" for="offset-x">Offset X</label>
					<input id="offset-x" type="number" value={selectedComponent.offset[0]} step="1"
						oninput={(e) => updateComponent({ offset: [parseFloat(e.currentTarget.value) || 0, selectedComponent!.offset[1]] })}
						class="w-12 h-6 text-[10px] text-center bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
						aria-label="X offset"
					/>
					<label class="sr-only" for="offset-y">Offset Y</label>
					<input id="offset-y" type="number" value={selectedComponent.offset[1]} step="1"
						oninput={(e) => updateComponent({ offset: [selectedComponent!.offset[0], parseFloat(e.currentTarget.value) || 0] })}
						class="w-12 h-6 text-[10px] text-center bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
						aria-label="Y offset"
					/>
				</div>
			</div>

			<!-- BACKGROUND SECTION -->
			<button
				onclick={() => toggleSection('background')}
				class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
				aria-expanded={expandedSections.has('background')}
			>
				{#if expandedSections.has('background')}
					<ChevronDown size={12} />
				{:else}
					<ChevronRight size={12} />
				{/if}
				<Palette size={12} />
				Background
			</button>
			{#if expandedSections.has('background')}
				<div class="space-y-2 pl-2 pb-2">
					<!-- Background type -->
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Type</span>
						<select
							value={selectedComponent.background.bg_type}
							onchange={(e) => updateBackground({ bg_type: e.currentTarget.value as any })}
							class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
							aria-label="Background type"
						>
							<option value="Solid">Solid</option>
							<option value="LinearGradient">Linear Gradient</option>
							<option value="RadialGradient">Radial Gradient</option>
							<option value="ConicGradient">Conic Gradient</option>
							<option value="Transparent">Transparent</option>
						</select>
					</div>
					<!-- Color -->
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Color</span>
						<div class="flex items-center gap-1">
							<input type="color" value={selectedComponent.background.color}
								oninput={(e) => updateBackground({ color: e.currentTarget.value })}
								class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
								aria-label="Background color"
							/>
							<input type="text" value={selectedComponent.background.color}
								oninput={(e) => updateBackground({ color: e.currentTarget.value })}
								class="w-20 h-6 text-[10px] font-mono bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Background color hex"
							/>
						</div>
					</div>
					<!-- Gradient end color -->
					{#if selectedComponent.background.bg_type !== 'Solid' && selectedComponent.background.bg_type !== 'Transparent'}
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">End Color</span>
							<div class="flex items-center gap-1">
								<input type="color" value={selectedComponent.background.color_end ?? selectedComponent.background.color}
									oninput={(e) => updateBackground({ color_end: e.currentTarget.value })}
									class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
									aria-label="Gradient end color"
								/>
							</div>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Angle</span>
							<input type="range" min="0" max="360" value={selectedComponent.background.gradient_angle}
								oninput={(e) => updateBackground({ gradient_angle: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Gradient angle"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.background.gradient_angle}°</span>
						</div>
					{/if}
					<!-- Opacity -->
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Opacity</span>
						<input type="range" min="0" max="1" step="0.05" value={selectedComponent.background.opacity}
							oninput={(e) => updateBackground({ opacity: parseFloat(e.currentTarget.value) })}
							class="w-24 h-4"
							aria-label="Background opacity"
						/>
						<span class="text-[10px] text-gx-text-muted w-8 text-right">{Math.round(selectedComponent.background.opacity * 100)}%</span>
					</div>
					<!-- Backdrop blur -->
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Blur</span>
						<input type="range" min="0" max="32" step="1" value={selectedComponent.background.backdrop_blur}
							oninput={(e) => updateBackground({ backdrop_blur: parseFloat(e.currentTarget.value) })}
							class="w-24 h-4"
							aria-label="Backdrop blur"
						/>
						<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.background.backdrop_blur}px</span>
					</div>
				</div>
			{/if}

			<!-- BORDER SECTION -->
			<button
				onclick={() => toggleSection('border')}
				class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
				aria-expanded={expandedSections.has('border')}
			>
				{#if expandedSections.has('border')}
					<ChevronDown size={12} />
				{:else}
					<ChevronRight size={12} />
				{/if}
				<Square size={12} />
				Border
			</button>
			{#if expandedSections.has('border')}
				<div class="space-y-2 pl-2 pb-2">
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Style</span>
						<select
							value={selectedComponent.border.pattern}
							onchange={(e) => updateBorder({ pattern: e.currentTarget.value as any })}
							class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
							aria-label="Border style"
						>
							<option value="None">None</option>
							<option value="Solid">Solid</option>
							<option value="Dashed">Dashed</option>
							<option value="Dotted">Dotted</option>
							<option value="Double">Double</option>
						</select>
					</div>
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Width</span>
						<input type="range" min="0" max="8" step="0.5" value={selectedComponent.border.width}
							oninput={(e) => updateBorder({ width: parseFloat(e.currentTarget.value) })}
							class="w-24 h-4"
							aria-label="Border width"
						/>
						<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.border.width}px</span>
					</div>
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Color</span>
						<input type="color" value={selectedComponent.border.color}
							oninput={(e) => updateBorder({ color: e.currentTarget.value })}
							class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
							aria-label="Border color"
						/>
					</div>
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Radius</span>
						<input type="range" min="0" max="32" step="1" value={selectedComponent.border.radius}
							oninput={(e) => updateBorder({ radius: parseFloat(e.currentTarget.value) })}
							class="w-24 h-4"
							aria-label="Border radius"
						/>
						<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.border.radius}px</span>
					</div>
				</div>
			{/if}

			<!-- TEXT SECTION (only if component has text) -->
			{#if selectedComponent.text}
				<button
					onclick={() => toggleSection('text')}
					class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
					aria-expanded={expandedSections.has('text')}
				>
					{#if expandedSections.has('text')}
						<ChevronDown size={12} />
					{:else}
						<ChevronRight size={12} />
					{/if}
					<Type size={12} />
					Text
				</button>
				{#if expandedSections.has('text')}
					<div class="space-y-2 pl-2 pb-2">
						<!-- Font family -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Font</span>
							<select
								value={typeof selectedComponent.text.font_family === 'string' ? selectedComponent.text.font_family : 'Custom'}
								onchange={(e) => {
									const val = e.currentTarget.value;
									loadFontsForFamily(val);
									updateText({ font_family: val as any });
								}}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1 max-w-[140px]"
								aria-label="Font family"
							>
								<option value="System">System UI</option>
								<option value="Mono">Monospace</option>
								{#each styleEngine.fonts as font (font.name)}
									<option value={font.name}>{font.name}</option>
								{/each}
							</select>
						</div>
						<!-- Font size -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Size</span>
							<input type="range" min="8" max="48" step="1" value={selectedComponent.text.font_size}
								oninput={(e) => updateText({ font_size: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Font size"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.text.font_size}px</span>
						</div>
						<!-- Font weight -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Weight</span>
							<select
								value={selectedComponent.text.font_weight}
								onchange={(e) => updateText({ font_weight: parseInt(e.currentTarget.value) })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Font weight"
							>
								<option value={300}>Light (300)</option>
								<option value={400}>Regular (400)</option>
								<option value={500}>Medium (500)</option>
								<option value={600}>Semibold (600)</option>
								<option value={700}>Bold (700)</option>
								<option value={800}>Extra Bold (800)</option>
							</select>
						</div>
						<!-- Text color -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Color</span>
							<div class="flex items-center gap-1">
								<input type="color" value={selectedComponent.text.color}
									oninput={(e) => updateText({ color: e.currentTarget.value })}
									class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
									aria-label="Text color"
								/>
								<input type="text" value={selectedComponent.text.color}
									oninput={(e) => updateText({ color: e.currentTarget.value })}
									class="w-20 h-6 text-[10px] font-mono bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
									aria-label="Text color hex"
								/>
							</div>
						</div>
						<!-- Text outline -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Outline</span>
							<select
								value={selectedComponent.text.outline}
								onchange={(e) => updateText({ outline: e.currentTarget.value as any })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Text outline"
							>
								<option value="None">None</option>
								<option value="Thin">Thin</option>
								<option value="Medium">Medium</option>
								<option value="Thick">Thick</option>
							</select>
						</div>
						<!-- Number format -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Number</span>
							<select
								value={selectedComponent.text.number_format}
								onchange={(e) => updateText({ number_format: e.currentTarget.value as any })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Number format"
							>
								<option value="Raw">Raw (12345)</option>
								<option value="Abbreviated">Abbreviated (12.3K)</option>
								<option value="Percent">Percent (85%)</option>
								<option value="CurrentMax">Current/Max</option>
								<option value="CurrentMaxPercent">Current/Max + %</option>
								<option value="Hidden">Hidden</option>
							</select>
						</div>
						<!-- Text transform -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Transform</span>
							<select
								value={selectedComponent.text.text_transform}
								onchange={(e) => updateText({ text_transform: e.currentTarget.value })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Text transform"
							>
								<option value="none">None</option>
								<option value="uppercase">UPPERCASE</option>
								<option value="lowercase">lowercase</option>
								<option value="capitalize">Capitalize</option>
							</select>
						</div>
						<!-- Letter spacing -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Spacing</span>
							<input type="range" min="-2" max="10" step="0.5" value={selectedComponent.text.letter_spacing}
								oninput={(e) => updateText({ letter_spacing: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Letter spacing"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.text.letter_spacing}px</span>
						</div>
						<!-- Position offset -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Offset</span>
							<div class="flex items-center gap-1">
								<input type="number" value={selectedComponent.text.offset[0]} step="1"
									oninput={(e) => updateText({ offset: [parseFloat(e.currentTarget.value) || 0, selectedComponent!.text!.offset[1]] })}
									class="w-12 h-6 text-[10px] text-center bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
									aria-label="Text X offset"
								/>
								<input type="number" value={selectedComponent.text.offset[1]} step="1"
									oninput={(e) => updateText({ offset: [selectedComponent!.text!.offset[0], parseFloat(e.currentTarget.value) || 0] })}
									class="w-12 h-6 text-[10px] text-center bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
									aria-label="Text Y offset"
								/>
							</div>
						</div>
					</div>
				{/if}
			{/if}

			<!-- BAR SECTION (only if component has bar) -->
			{#if selectedComponent.bar}
				<button
					onclick={() => toggleSection('bar')}
					class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
					aria-expanded={expandedSections.has('bar')}
				>
					{#if expandedSections.has('bar')}
						<ChevronDown size={12} />
					{:else}
						<ChevronRight size={12} />
					{/if}
					<BarChart3 size={12} />
					Bar
				</button>
				{#if expandedSections.has('bar')}
					<div class="space-y-2 pl-2 pb-2">
						<!-- Bar color -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Fill Color</span>
							<input type="color" value={selectedComponent.bar.color}
								oninput={(e) => updateBar({ color: e.currentTarget.value })}
								class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
								aria-label="Bar fill color"
							/>
						</div>
						<!-- Background color -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Background</span>
							<input type="color" value={selectedComponent.bar.background_color}
								oninput={(e) => updateBar({ background_color: e.currentTarget.value })}
								class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
								aria-label="Bar background color"
							/>
						</div>
						<!-- Texture -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Texture</span>
							<select
								value={selectedComponent.bar.texture}
								onchange={(e) => updateBar({ texture: e.currentTarget.value as any })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Bar texture"
							>
								<option value="Flat">Flat</option>
								<option value="Gradient">Gradient</option>
								<option value="Striped">Striped</option>
								<option value="Glossy">Glossy</option>
								<option value="Minimalist">Minimalist</option>
							</select>
						</div>
						<!-- Height -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Height</span>
							<input type="range" min="4" max="48" step="1" value={selectedComponent.bar.height}
								oninput={(e) => updateBar({ height: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Bar height"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.bar.height}px</span>
						</div>
						<!-- Border radius -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Radius</span>
							<input type="range" min="0" max="24" step="1" value={selectedComponent.bar.border_radius}
								oninput={(e) => updateBar({ border_radius: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Bar border radius"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.bar.border_radius}px</span>
						</div>
						<!-- Fill direction -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Direction</span>
							<select
								value={selectedComponent.bar.fill_direction}
								onchange={(e) => updateBar({ fill_direction: e.currentTarget.value as any })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Bar fill direction"
							>
								<option value="LeftToRight">Left → Right</option>
								<option value="RightToLeft">Right → Left</option>
								<option value="BottomToTop">Bottom → Top</option>
								<option value="TopToBottom">Top → Bottom</option>
							</select>
						</div>
						<!-- Animate changes -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Animate</span>
							<button
								onclick={() => updateBar({ animate_changes: !selectedComponent!.bar!.animate_changes })}
								class="h-6 px-2 text-[10px] rounded {selectedComponent.bar.animate_changes ? 'bg-gx-neon/20 text-gx-neon' : 'bg-gx-bg-tertiary text-gx-text-muted'}"
								aria-label="Toggle bar animation"
								aria-pressed={selectedComponent.bar.animate_changes}
							>
								{selectedComponent.bar.animate_changes ? 'ON' : 'OFF'}
							</button>
						</div>
						<!-- Spark effect -->
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Spark</span>
							<button
								onclick={() => updateBar({ spark_effect: !selectedComponent!.bar!.spark_effect })}
								class="h-6 px-2 text-[10px] rounded {selectedComponent.bar.spark_effect ? 'bg-gx-neon/20 text-gx-neon' : 'bg-gx-bg-tertiary text-gx-text-muted'}"
								aria-label="Toggle spark effect"
								aria-pressed={selectedComponent.bar.spark_effect}
							>
								{selectedComponent.bar.spark_effect ? 'ON' : 'OFF'}
							</button>
						</div>
					</div>
				{/if}
			{/if}

			<!-- GLOW SECTION -->
			<button
				onclick={() => toggleSection('glow')}
				class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
				aria-expanded={expandedSections.has('glow')}
			>
				{#if expandedSections.has('glow')}
					<ChevronDown size={12} />
				{:else}
					<ChevronRight size={12} />
				{/if}
				<Sparkles size={12} />
				Glow / Aura
			</button>
			{#if expandedSections.has('glow')}
				<div class="space-y-2 pl-2 pb-2">
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Type</span>
						<select
							value={selectedComponent.glow.glow_type}
							onchange={(e) => updateGlow({ glow_type: e.currentTarget.value as any })}
							class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
							aria-label="Glow type"
						>
							<option value="None">None</option>
							<option value="BoxGlow">Box Glow</option>
							<option value="TextGlow">Text Glow</option>
							<option value="InnerGlow">Inner Glow</option>
							<option value="DualGlow">Dual Glow</option>
						</select>
					</div>
					{#if selectedComponent.glow.glow_type !== 'None'}
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Color</span>
							<input type="color" value={selectedComponent.glow.color}
								oninput={(e) => updateGlow({ color: e.currentTarget.value })}
								class="w-6 h-6 rounded cursor-pointer border border-gx-border-default"
								aria-label="Glow color"
							/>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Intensity</span>
							<input type="range" min="0" max="40" step="1" value={selectedComponent.glow.intensity}
								oninput={(e) => updateGlow({ intensity: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Glow intensity"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.glow.intensity}px</span>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Blur</span>
							<input type="range" min="0" max="64" step="1" value={selectedComponent.glow.blur}
								oninput={(e) => updateGlow({ blur: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Glow blur"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{selectedComponent.glow.blur}px</span>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Opacity</span>
							<input type="range" min="0" max="1" step="0.05" value={selectedComponent.glow.opacity}
								oninput={(e) => updateGlow({ opacity: parseFloat(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Glow opacity"
							/>
							<span class="text-[10px] text-gx-text-muted w-8 text-right">{Math.round(selectedComponent.glow.opacity * 100)}%</span>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Pulse</span>
							<button
								onclick={() => updateGlow({ animated: !selectedComponent!.glow.animated })}
								class="h-6 px-2 text-[10px] rounded {selectedComponent.glow.animated ? 'bg-gx-neon/20 text-gx-neon' : 'bg-gx-bg-tertiary text-gx-text-muted'}"
								aria-label="Toggle glow pulse animation"
								aria-pressed={selectedComponent.glow.animated}
							>
								{selectedComponent.glow.animated ? 'ON' : 'OFF'}
							</button>
						</div>
					{/if}
				</div>
			{/if}

			<!-- ANIMATION SECTION -->
			<button
				onclick={() => toggleSection('animation')}
				class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
				aria-expanded={expandedSections.has('animation')}
			>
				{#if expandedSections.has('animation')}
					<ChevronDown size={12} />
				{:else}
					<ChevronRight size={12} />
				{/if}
				Animation
			</button>
			{#if expandedSections.has('animation')}
				<div class="space-y-2 pl-2 pb-2">
					<div class="flex items-center justify-between">
						<span class="text-[10px] text-gx-text-muted">Type</span>
						<select
							value={selectedComponent.animation.animation_type}
							onchange={(e) => updateAnimation({ animation_type: e.currentTarget.value as any })}
							class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
							aria-label="Animation type"
						>
							<option value="None">None</option>
							<option value="Fade">Fade</option>
							<option value="Scale">Scale</option>
							<option value="SlideIn">Slide In</option>
							<option value="PulseOnChange">Pulse on Change</option>
							<option value="Flash">Flash</option>
							<option value="CountUp">Count Up</option>
							<option value="Breathe">Breathe</option>
						</select>
					</div>
					{#if selectedComponent.animation.animation_type !== 'None'}
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Duration</span>
							<input type="range" min="100" max="3000" step="100" value={selectedComponent.animation.duration_ms}
								oninput={(e) => updateAnimation({ duration_ms: parseInt(e.currentTarget.value) })}
								class="w-24 h-4"
								aria-label="Animation duration"
							/>
							<span class="text-[10px] text-gx-text-muted w-10 text-right">{selectedComponent.animation.duration_ms}ms</span>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-[10px] text-gx-text-muted">Easing</span>
							<select
								value={selectedComponent.animation.easing}
								onchange={(e) => updateAnimation({ easing: e.currentTarget.value as any })}
								class="h-6 text-[10px] bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="Animation easing"
							>
								<option value="Linear">Linear</option>
								<option value="EaseIn">Ease In</option>
								<option value="EaseOut">Ease Out</option>
								<option value="EaseInOut">Ease In/Out</option>
								<option value="Spring">Spring</option>
							</select>
						</div>
					{/if}
				</div>
			{/if}

			<!-- PADDING SECTION -->
			<button
				onclick={() => toggleSection('padding')}
				class="w-full flex items-center gap-2 py-2 text-[11px] font-semibold text-gx-text-secondary uppercase tracking-wider hover:text-gx-text-primary"
				aria-expanded={expandedSections.has('padding')}
			>
				{#if expandedSections.has('padding')}
					<ChevronDown size={12} />
				{:else}
					<ChevronRight size={12} />
				{/if}
				Padding
			</button>
			{#if expandedSections.has('padding')}
				<div class="grid grid-cols-2 gap-2 pl-2 pb-2">
					{#each ['Top', 'Right', 'Bottom', 'Left'] as side, i}
						<div class="flex items-center gap-1">
							<span class="text-[10px] text-gx-text-muted w-10">{side}</span>
							<input type="number" value={selectedComponent.padding[i]} min="0" max="64" step="1"
								oninput={(e) => {
									const p = [...selectedComponent!.padding] as [number, number, number, number];
									p[i] = parseFloat(e.currentTarget.value) || 0;
									updateComponent({ padding: p });
								}}
								class="w-14 h-6 text-[10px] text-center bg-gx-bg-tertiary border border-gx-border-default rounded px-1"
								aria-label="{side} padding"
							/>
						</div>
					{/each}
				</div>
			{/if}

		</div>
	{:else}
		<div class="flex-1 flex items-center justify-center text-gx-text-muted text-sm">
			Select a sub-component to edit
		</div>
	{/if}

	<!-- Live Preview Footer -->
	{#if selectedComponent}
		<div class="border-t border-gx-border-default px-3 py-2">
			<div class="text-[9px] text-gx-text-muted mb-1 font-mono">Live Preview</div>
			<div
				class="rounded overflow-hidden"
				style={componentToCSS(selectedComponent)}
			>
				{#if selectedComponent.bar}
					<div class="w-full rounded" style="background-color: {selectedComponent.bar.background_color}; height: {selectedComponent.bar.height}px;">
						<div class="h-full rounded" style="background-color: {selectedComponent.bar.color}; width: 65%; transition: width 300ms;"></div>
					</div>
				{:else}
					<div class="px-2 py-1 text-center">
						{selectedComponent.label}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
