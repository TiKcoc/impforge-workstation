<script lang="ts">
	import {
		Bold, Italic, Underline, Strikethrough,
		AlignLeft, AlignCenter, AlignRight, AlignJustify,
		List, ListOrdered, Indent, Outdent,
		Heading1, Heading2, Heading3, Quote, Code2,
		Table2, Image, Link, Minus, BookOpen, Bookmark,
		FileText, Columns2, LayoutGrid,
		Sparkles, Scissors, Expand, CheckCheck, Languages, BookOpenText,
		// Sheets-specific
		Hash, Percent, DollarSign, Calendar, Paintbrush,
		BarChart3, PieChart, TrendingUp, ArrowDown, ArrowRight,
		Filter, ListChecks, Zap, Brain, Bot, Globe,
		SlidersHorizontal, FunctionSquare, Search,
		// Slides-specific
		Type, Square, Presentation, Play, Video,
		Palette, MonitorPlay, Timer, MoveRight,
		Wand2,
		// PDF-specific
		Eye, Download, Upload, MessageSquare, Copy,
		// Modern mode
		Send
	} from '@lucide/svelte';
	import { Separator } from '$lib/components/ui/separator';
	import ModeSwitcher from './ModeSwitcher.svelte';

	// ---- Props ---------------------------------------------------------------
	interface Props {
		module: 'writer' | 'sheets' | 'slides' | 'pdf';
		mode: 'classic' | 'modern';
		onAction: (action: string, params?: Record<string, unknown>) => void;
		onModeToggle: () => void;
	}

	let { module, mode, onAction, onModeToggle }: Props = $props();

	// ---- Ribbon tab state ----------------------------------------------------
	let activeTab = $state('Home');
	let chatInput = $state('');

	// ---- Tab definitions per module ------------------------------------------
	interface ToolButton {
		icon: typeof Bold;
		label: string;
		action: string;
		params?: Record<string, unknown>;
		accent?: string;
	}

	interface ToolGroup {
		label: string;
		tools: ToolButton[];
	}

	interface RibbonTab {
		name: string;
		groups: ToolGroup[];
	}

	// ---- Writer tabs ---------------------------------------------------------
	const writerTabs: RibbonTab[] = [
		{
			name: 'Home',
			groups: [
				{
					label: 'Font',
					tools: [
						{ icon: Bold, label: 'Bold', action: 'format_bold' },
						{ icon: Italic, label: 'Italic', action: 'format_italic' },
						{ icon: Underline, label: 'Underline', action: 'format_underline' },
						{ icon: Strikethrough, label: 'Strikethrough', action: 'format_strikethrough' },
					],
				},
				{
					label: 'Paragraph',
					tools: [
						{ icon: AlignLeft, label: 'Left', action: 'align', params: { align: 'left' } },
						{ icon: AlignCenter, label: 'Center', action: 'align', params: { align: 'center' } },
						{ icon: AlignRight, label: 'Right', action: 'align', params: { align: 'right' } },
						{ icon: AlignJustify, label: 'Justify', action: 'align', params: { align: 'justify' } },
						{ icon: List, label: 'Bullet List', action: 'insert_list_bullet' },
						{ icon: ListOrdered, label: 'Numbered', action: 'insert_list_ordered' },
						{ icon: Indent, label: 'Indent', action: 'indent' },
						{ icon: Outdent, label: 'Outdent', action: 'outdent' },
					],
				},
				{
					label: 'Styles',
					tools: [
						{ icon: Heading1, label: 'H1', action: 'heading', params: { level: 1 } },
						{ icon: Heading2, label: 'H2', action: 'heading', params: { level: 2 } },
						{ icon: Heading3, label: 'H3', action: 'heading', params: { level: 3 } },
						{ icon: Quote, label: 'Quote', action: 'insert_quote' },
						{ icon: Code2, label: 'Code', action: 'insert_code_inline' },
					],
				},
			],
		},
		{
			name: 'Insert',
			groups: [
				{
					label: 'Elements',
					tools: [
						{ icon: Table2, label: 'Table', action: 'insert_table' },
						{ icon: Image, label: 'Image', action: 'insert_image' },
						{ icon: Link, label: 'Link', action: 'insert_link' },
						{ icon: Minus, label: 'Divider', action: 'insert_divider' },
						{ icon: Code2, label: 'Code Block', action: 'insert_code_block' },
						{ icon: Bookmark, label: 'Footnote', action: 'insert_footnote' },
					],
				},
			],
		},
		{
			name: 'Layout',
			groups: [
				{
					label: 'Page',
					tools: [
						{ icon: FileText, label: 'Page Size', action: 'page_size' },
						{ icon: LayoutGrid, label: 'Margins', action: 'page_margins' },
						{ icon: Columns2, label: 'Columns', action: 'page_columns' },
						{ icon: BookOpenText, label: 'Header/Footer', action: 'header_footer' },
					],
				},
			],
		},
		{
			name: 'AI',
			groups: [
				{
					label: 'AI Assist',
					tools: [
						{ icon: Sparkles, label: 'Improve', action: 'ai_improve', accent: 'magenta' },
						{ icon: Scissors, label: 'Shorten', action: 'ai_shorten', accent: 'magenta' },
						{ icon: Expand, label: 'Expand', action: 'ai_expand', accent: 'magenta' },
						{ icon: CheckCheck, label: 'Fix Grammar', action: 'ai_fix_grammar', accent: 'magenta' },
						{ icon: Languages, label: 'Translate', action: 'ai_translate', accent: 'magenta' },
						{ icon: FileText, label: 'Summarize', action: 'ai_summarize', accent: 'magenta' },
					],
				},
			],
		},
	];

	// ---- Sheets tabs ---------------------------------------------------------
	const sheetsTabs: RibbonTab[] = [
		{
			name: 'Home',
			groups: [
				{
					label: 'Font',
					tools: [
						{ icon: Bold, label: 'Bold', action: 'format_bold' },
						{ icon: Italic, label: 'Italic', action: 'format_italic' },
						{ icon: Paintbrush, label: 'Fill Color', action: 'fill_color' },
					],
				},
				{
					label: 'Alignment',
					tools: [
						{ icon: AlignLeft, label: 'Left', action: 'align', params: { align: 'left' } },
						{ icon: AlignCenter, label: 'Center', action: 'align', params: { align: 'center' } },
						{ icon: AlignRight, label: 'Right', action: 'align', params: { align: 'right' } },
					],
				},
				{
					label: 'Number Format',
					tools: [
						{ icon: DollarSign, label: 'Currency', action: 'number_format', params: { format: '$#,##0.00' } },
						{ icon: Percent, label: 'Percent', action: 'number_format', params: { format: '0.00%' } },
						{ icon: Hash, label: 'Number', action: 'number_format', params: { format: '#,##0.00' } },
						{ icon: Calendar, label: 'Date', action: 'number_format', params: { format: 'YYYY-MM-DD' } },
					],
				},
			],
		},
		{
			name: 'Insert',
			groups: [
				{
					label: 'Charts & Data',
					tools: [
						{ icon: BarChart3, label: 'Chart', action: 'insert_chart' },
						{ icon: PieChart, label: 'Pivot Table', action: 'insert_pivot' },
						{ icon: Image, label: 'Image', action: 'insert_image' },
						{ icon: MessageSquare, label: 'Comment', action: 'insert_comment' },
					],
				},
				{
					label: 'Functions',
					tools: [
						{ icon: FunctionSquare, label: 'Function', action: 'insert_function' },
					],
				},
			],
		},
		{
			name: 'Data',
			groups: [
				{
					label: 'Sort & Filter',
					tools: [
						{ icon: ArrowDown, label: 'Sort A-Z', action: 'sort_asc' },
						{ icon: ArrowRight, label: 'Sort Z-A', action: 'sort_desc' },
						{ icon: Filter, label: 'Filter', action: 'toggle_filter' },
					],
				},
				{
					label: 'Tools',
					tools: [
						{ icon: ListChecks, label: 'Validation', action: 'data_validation' },
						{ icon: Zap, label: 'Remove Dupes', action: 'remove_duplicates' },
					],
				},
			],
		},
		{
			name: 'Formulas',
			groups: [
				{
					label: 'Library',
					tools: [
						{ icon: FunctionSquare, label: 'AutoSum', action: 'formula_autosum' },
						{ icon: DollarSign, label: 'Financial', action: 'formula_financial' },
						{ icon: SlidersHorizontal, label: 'Logical', action: 'formula_logical' },
						{ icon: Type, label: 'Text', action: 'formula_text' },
						{ icon: Calendar, label: 'Date', action: 'formula_date' },
						{ icon: Search, label: 'Lookup', action: 'formula_lookup' },
						{ icon: Hash, label: 'Math', action: 'formula_math' },
					],
				},
			],
		},
		{
			name: 'AI',
			groups: [
				{
					label: 'AI Tools',
					tools: [
						{ icon: Sparkles, label: 'NL to Formula', action: 'ai_nl_formula', accent: 'magenta' },
						{ icon: TrendingUp, label: 'Auto-EDA', action: 'ai_auto_eda', accent: 'magenta' },
						{ icon: Bot, label: 'Agentic Cell', action: 'ai_agentic_cell', accent: 'magenta' },
						{ icon: BarChart3, label: 'AI Chart', action: 'ai_chart', accent: 'magenta' },
					],
				},
			],
		},
	];

	// ---- Slides tabs ---------------------------------------------------------
	const slidesTabs: RibbonTab[] = [
		{
			name: 'Home',
			groups: [
				{
					label: 'Font',
					tools: [
						{ icon: Bold, label: 'Bold', action: 'format_bold' },
						{ icon: Italic, label: 'Italic', action: 'format_italic' },
						{ icon: Underline, label: 'Underline', action: 'format_underline' },
					],
				},
				{
					label: 'Paragraph',
					tools: [
						{ icon: AlignLeft, label: 'Left', action: 'align', params: { align: 'left' } },
						{ icon: AlignCenter, label: 'Center', action: 'align', params: { align: 'center' } },
						{ icon: AlignRight, label: 'Right', action: 'align', params: { align: 'right' } },
					],
				},
				{
					label: 'Shapes',
					tools: [
						{ icon: Square, label: 'Shape', action: 'insert_shape' },
					],
				},
			],
		},
		{
			name: 'Insert',
			groups: [
				{
					label: 'Content',
					tools: [
						{ icon: Type, label: 'Text Box', action: 'insert_textbox' },
						{ icon: Image, label: 'Image', action: 'insert_image' },
						{ icon: BarChart3, label: 'Chart', action: 'insert_chart' },
						{ icon: Table2, label: 'Table', action: 'insert_table' },
						{ icon: Video, label: 'Video', action: 'insert_video' },
						{ icon: Square, label: 'Shape', action: 'insert_shape' },
					],
				},
			],
		},
		{
			name: 'Design',
			groups: [
				{
					label: 'Themes',
					tools: [
						{ icon: Palette, label: 'Themes', action: 'show_themes' },
						{ icon: Paintbrush, label: 'Background', action: 'slide_background' },
						{ icon: Presentation, label: 'Slide Size', action: 'slide_size' },
					],
				},
			],
		},
		{
			name: 'Transitions',
			groups: [
				{
					label: 'Effects',
					tools: [
						{ icon: MonitorPlay, label: 'Fade', action: 'transition', params: { type: 'fade' } },
						{ icon: MoveRight, label: 'Slide', action: 'transition', params: { type: 'slide' } },
						{ icon: Expand, label: 'Zoom', action: 'transition', params: { type: 'zoom' } },
						{ icon: Timer, label: 'Duration', action: 'transition_duration' },
					],
				},
			],
		},
		{
			name: 'AI',
			groups: [
				{
					label: 'AI Tools',
					tools: [
						{ icon: Sparkles, label: 'Generate Slides', action: 'ai_generate', accent: 'magenta' },
						{ icon: Wand2, label: 'Improve Slide', action: 'ai_improve', accent: 'magenta' },
						{ icon: FileText, label: 'Speech Script', action: 'ai_speech_script', accent: 'magenta' },
					],
				},
			],
		},
	];

	// ---- PDF tabs ------------------------------------------------------------
	const pdfTabs: RibbonTab[] = [
		{
			name: 'Home',
			groups: [
				{
					label: 'View',
					tools: [
						{ icon: Eye, label: 'Extract Text', action: 'extract_text' },
						{ icon: Copy, label: 'Copy Text', action: 'copy_text' },
					],
				},
				{
					label: 'Convert',
					tools: [
						{ icon: Type, label: 'To .txt', action: 'convert_txt' },
						{ icon: Hash, label: 'To .md', action: 'convert_md' },
					],
				},
			],
		},
		{
			name: 'Import',
			groups: [
				{
					label: 'Files',
					tools: [
						{ icon: Upload, label: 'Import PDF', action: 'import_pdf' },
						{ icon: Download, label: 'Export', action: 'export' },
					],
				},
			],
		},
		{
			name: 'AI',
			groups: [
				{
					label: 'AI Analysis',
					tools: [
						{ icon: Sparkles, label: 'Summarize', action: 'ai_summarize', accent: 'purple' },
						{ icon: MessageSquare, label: 'Ask Question', action: 'ai_ask', accent: 'purple' },
						{ icon: Brain, label: 'Key Points', action: 'ai_key_points', accent: 'purple' },
					],
				},
			],
		},
	];

	// ---- Tab resolution ------------------------------------------------------
	let tabs = $derived(
		module === 'writer' ? writerTabs :
		module === 'sheets' ? sheetsTabs :
		module === 'slides' ? slidesTabs :
		pdfTabs
	);

	// Reset active tab when module changes or when tabs do not contain current tab
	$effect(() => {
		if (!tabs.find(t => t.name === activeTab)) {
			activeTab = tabs[0]?.name ?? 'Home';
		}
	});

	let activeTabData = $derived(tabs.find(t => t.name === activeTab));

	// ---- Modern mode chat handler -------------------------------------------
	function handleChatSend() {
		const text = chatInput.trim();
		if (!text) return;
		onAction('chat_command', { text });
		chatInput = '';
	}

	function handleChatKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleChatSend();
		}
	}
</script>

<!-- Ribbon container with smooth height transition -->
<div class="shrink-0 select-none transition-all duration-300 ease-in-out">
	{#if mode === 'modern'}
		<!-- ===== MODERN MODE: Chat bar ===== -->
		<div class="border-b border-gx-border-default bg-gx-bg-secondary">
			<div class="flex items-center gap-2 px-3 py-2">
				<Sparkles size={14} class="text-gx-accent-magenta shrink-0" />
				<input
					type="text"
					bind:value={chatInput}
					onkeydown={handleChatKeydown}
					placeholder="Tell me what to do..."
					class="flex-1 bg-transparent text-sm text-gx-text-primary
						placeholder:text-gx-text-muted/50 outline-none"
				/>
				<button
					onclick={handleChatSend}
					disabled={!chatInput.trim()}
					class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-medium rounded-gx
						bg-gx-accent-magenta/15 text-gx-accent-magenta border border-gx-accent-magenta/30
						hover:bg-gx-accent-magenta/25 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
				>
					<Send size={12} />
					Send
				</button>
				<Separator orientation="vertical" class="h-5 bg-gx-border-default" />
				<ModeSwitcher {mode} onToggle={onModeToggle} />
			</div>
		</div>
	{:else}
		<!-- ===== CLASSIC MODE: Ribbon with tabs ===== -->
		<div class="border-b border-gx-border-default bg-gx-bg-secondary">
			<!-- Tab bar -->
			<div class="flex items-center h-7 px-1 gap-0 border-b border-gx-border-default/50">
				{#each tabs as tab (tab.name)}
					<button
						onclick={() => { activeTab = tab.name; }}
						class="relative px-3 py-0.5 text-[11px] font-medium rounded-t transition-colors duration-150
							{activeTab === tab.name
								? 'text-gx-neon bg-gx-bg-tertiary'
								: 'text-gx-text-muted hover:text-gx-text-secondary hover:bg-gx-bg-hover/50'}"
					>
						{tab.name}
						{#if activeTab === tab.name}
							<span class="absolute bottom-0 left-1 right-1 h-[2px] rounded-full bg-gx-neon"></span>
						{/if}
					</button>
				{/each}

				<div class="flex-1"></div>
				<ModeSwitcher {mode} onToggle={onModeToggle} />
			</div>

			<!-- Tool groups for active tab -->
			{#if activeTabData}
				<div class="flex items-start gap-0 h-[62px] px-1.5 overflow-x-auto">
					{#each activeTabData.groups as group, groupIdx (group.label)}
						<!-- Group -->
						<div class="flex flex-col h-full py-1 px-1">
							<!-- Tool buttons row -->
							<div class="flex items-center gap-0.5 flex-1">
								{#each group.tools as tool (tool.action)}
									<button
										onclick={() => onAction(tool.action, tool.params)}
										title={tool.label}
										class="flex flex-col items-center justify-center gap-0.5
											w-[42px] h-[42px] rounded transition-colors duration-100
											{tool.accent === 'magenta'
												? 'text-gx-accent-magenta hover:bg-gx-accent-magenta/15'
												: tool.accent === 'purple'
													? 'text-gx-accent-purple hover:bg-gx-accent-purple/15'
													: 'text-gx-text-muted hover:text-gx-text-primary hover:bg-gx-bg-hover'}"
									>
										<tool.icon size={16} />
										<span class="text-[9px] leading-none truncate max-w-[40px]">{tool.label}</span>
									</button>
								{/each}
							</div>
							<!-- Group label -->
							<div class="text-center">
								<span class="text-[8px] text-gx-text-muted/50 uppercase tracking-wider leading-none">
									{group.label}
								</span>
							</div>
						</div>

						<!-- Vertical divider between groups -->
						{#if groupIdx < activeTabData.groups.length - 1}
							<div class="flex items-center h-full py-2">
								<div class="w-px h-full bg-gx-border-default/60"></div>
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>
