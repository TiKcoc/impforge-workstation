<script lang="ts">
	/**
	 * SpecDrivenPanel v2 — Spec-to-Code Closed-Loop Pipeline
	 *
	 * Full development lifecycle: write spec → generate plan → generate code →
	 * diff preview → accept/reject per file → AI self-verification → apply.
	 *
	 * v2 Upgrades:
	 *   - Per-task code generation with streaming diff preview
	 *   - Accept/reject per generated file
	 *   - AI self-verification loop (Agent-as-Judge, arXiv:2601.05111)
	 *   - Pipeline progress tracking with stage indicators
	 *
	 * Sub-components (via styleEngine.getComponentStyle):
	 *   - container: Root wrapper
	 *   - spec-editor: Specification input area
	 *   - plan-output: Generated plan display
	 *   - task-list: Task breakdown list
	 *   - code-gen: Code generation pipeline view
	 */

	import { invoke } from '@tauri-apps/api/core';
	import {
		FileText, Play, Loader2, CheckCircle, Circle,
		ChevronDown, ChevronRight, Copy, Plus, Trash2,
		Target, ListChecks, FolderTree, TestTube2, Zap,
		Code2, Check, XCircle, Eye, Save, ArrowRight, RotateCcw
	} from '@lucide/svelte';
	import { ide } from '$lib/stores/ide.svelte';
	import { styleEngine, componentToCSS } from '$lib/stores/style-engine.svelte';

	// BenikUI style engine
	const widgetId = 'ide-spec-driven';
	$effect(() => {
		if (!styleEngine.widgetStyles.has(widgetId)) {
			styleEngine.loadWidgetStyle(widgetId);
		}
	});
	let hasEngineStyle = $derived(styleEngine.widgetStyles.has(widgetId));
	let containerComp = $derived(styleEngine.getComponentStyle(widgetId, 'container'));
	let containerStyle = $derived(hasEngineStyle && containerComp ? componentToCSS(containerComp) : '');
	let specEditorComp = $derived(styleEngine.getComponentStyle(widgetId, 'spec-editor'));
	let specEditorStyle = $derived(hasEngineStyle && specEditorComp ? componentToCSS(specEditorComp) : '');
	let planOutputComp = $derived(styleEngine.getComponentStyle(widgetId, 'plan-output'));
	let planOutputStyle = $derived(hasEngineStyle && planOutputComp ? componentToCSS(planOutputComp) : '');

	// Spec templates for quick-start
	const specTemplates: Record<string, { label: string; template: string }> = {
		feature: {
			label: 'Feature',
			template: `## Feature: [Name]

### Goal
[What does this feature accomplish?]

### User Story
As a [role], I want to [action], so that [benefit].

### Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

### Constraints
- [Technical constraints, dependencies]

### Out of Scope
- [What this feature does NOT include]`,
		},
		bugfix: {
			label: 'Bug Fix',
			template: `## Bug: [Title]

### Current Behavior
[What happens now]

### Expected Behavior
[What should happen]

### Reproduction Steps
1. Step 1
2. Step 2
3. Step 3

### Environment
- OS:
- Version:

### Root Cause (if known)
[Analysis]`,
		},
		refactor: {
			label: 'Refactor',
			template: `## Refactor: [Component/Module]

### Current State
[What exists now and why it needs refactoring]

### Target State
[What the code should look like after refactoring]

### Motivation
- [Performance / Maintainability / Readability / DRY]

### Breaking Changes
- [None / List of API changes]

### Migration Plan
[How to migrate existing code]`,
		},
		api: {
			label: 'API Endpoint',
			template: `## API: [Method] /path/to/endpoint

### Purpose
[What this endpoint does]

### Request
\`\`\`json
{
  "field": "type — description"
}
\`\`\`

### Response (200)
\`\`\`json
{
  "field": "type — description"
}
\`\`\`

### Error Responses
- 400: [Validation error]
- 404: [Not found]
- 500: [Server error]

### Authentication
[Required / Optional / None]

### Rate Limiting
[Limits if any]`,
		},
	};

	interface PlanTask {
		id: string;
		title: string;
		description: string;
		files: string[];
		tests: string[];
		done: boolean;
		expanded: boolean;
	}

	interface GeneratedPlan {
		summary: string;
		architecture: string;
		tasks: PlanTask[];
		fileTree: string[];
		testStrategy: string;
		estimatedComplexity: 'low' | 'medium' | 'high';
	}

	// v2: Code Generation Pipeline
	interface GeneratedFile {
		path: string;
		content: string;
		originalContent: string;
		accepted: boolean | null; // null = pending review
	}

	interface CodeGeneration {
		taskId: string;
		status: 'idle' | 'generating' | 'reviewing' | 'verified' | 'failed';
		files: GeneratedFile[];
		verificationResult: string;
		errorMessage: string;
	}

	// State
	let specContent = $state('');
	let generating = $state(false);
	let plan = $state<GeneratedPlan | null>(null);
	let activeView = $state<'spec' | 'plan' | 'code'>('spec');
	let showTemplates = $state(false);
	let specHistory = $state<Array<{ title: string; spec: string; timestamp: number }>>([]);

	// v2: Code generation state
	let codeGens = $state<Map<string, CodeGeneration>>(new Map());
	let activeCodeTaskId = $state<string | null>(null);
	let applyingFiles = $state(false);

	function applyTemplate(key: string) {
		specContent = specTemplates[key].template;
		showTemplates = false;
	}

	async function generatePlan() {
		if (!specContent.trim()) return;
		generating = true;
		plan = null;

		try {
			// Get workspace context for better plan generation
			const openFiles = ide.openTabs.map(t => t.path).filter(Boolean);

			const result = await invoke<string>('ide_agent_tool_call', {
				input: `Generate an implementation plan for the following specification. Return a structured response with:
1. SUMMARY: One paragraph overview
2. ARCHITECTURE: Technical approach (2-3 sentences)
3. TASKS: Numbered list of implementation tasks, each with:
   - Title
   - Description (1-2 sentences)
   - Files to create/modify (exact paths)
   - Tests needed
4. FILE_TREE: List of all files that will be created or modified
5. TEST_STRATEGY: How to verify the implementation
6. COMPLEXITY: low/medium/high

Workspace context - currently open files: ${openFiles.join(', ')}

SPECIFICATION:
${specContent}`,
				context: `spec-driven-development`,
			});

			// Parse the AI response into structured plan
			plan = parseAiPlan(result);

			// Save to history
			const titleMatch = specContent.match(/##\s+\w+:\s*(.+)/);
			specHistory = [
				{ title: titleMatch?.[1]?.trim() || 'Untitled Spec', spec: specContent, timestamp: Date.now() },
				...specHistory.slice(0, 9),
			];

			activeView = 'plan';
		} catch (e) {
			plan = {
				summary: `Error generating plan: ${e}`,
				architecture: '',
				tasks: [],
				fileTree: [],
				testStrategy: '',
				estimatedComplexity: 'medium',
			};
			activeView = 'plan';
		}

		generating = false;
	}

	function parseAiPlan(raw: string): GeneratedPlan {
		const lines = raw.split('\n');
		let summary = '';
		let architecture = '';
		const tasks: PlanTask[] = [];
		const fileTree: string[] = [];
		let testStrategy = '';
		let complexity: 'low' | 'medium' | 'high' = 'medium';
		let section = '';

		for (const line of lines) {
			const trimmed = line.trim();

			if (/^#+\s*SUMMARY/i.test(trimmed) || /^SUMMARY:/i.test(trimmed)) { section = 'summary'; continue; }
			if (/^#+\s*ARCHITECTURE/i.test(trimmed) || /^ARCHITECTURE:/i.test(trimmed)) { section = 'architecture'; continue; }
			if (/^#+\s*TASKS/i.test(trimmed) || /^TASKS:/i.test(trimmed)) { section = 'tasks'; continue; }
			if (/^#+\s*FILE.TREE/i.test(trimmed) || /^FILE.TREE:/i.test(trimmed)) { section = 'filetree'; continue; }
			if (/^#+\s*TEST.STRATEGY/i.test(trimmed) || /^TEST.STRATEGY:/i.test(trimmed)) { section = 'test'; continue; }
			if (/^#+\s*COMPLEXITY/i.test(trimmed) || /^COMPLEXITY:/i.test(trimmed)) { section = 'complexity'; continue; }

			switch (section) {
				case 'summary':
					if (trimmed) summary += (summary ? ' ' : '') + trimmed;
					break;
				case 'architecture':
					if (trimmed) architecture += (architecture ? ' ' : '') + trimmed;
					break;
				case 'tasks': {
					const taskMatch = trimmed.match(/^\d+[\.\)]\s*(.+)/);
					if (taskMatch) {
						tasks.push({
							id: `task_${tasks.length}`,
							title: taskMatch[1],
							description: '',
							files: [],
							tests: [],
							done: false,
							expanded: false,
						});
					} else if (tasks.length > 0 && trimmed) {
						const lastTask = tasks[tasks.length - 1];
						if (/^[-*]\s*files?:/i.test(trimmed) || /^files?:/i.test(trimmed)) {
							const filePart = trimmed.replace(/^[-*]?\s*files?:\s*/i, '');
							lastTask.files.push(...filePart.split(',').map(f => f.trim()).filter(Boolean));
						} else if (/^[-*]\s*tests?:/i.test(trimmed) || /^tests?:/i.test(trimmed)) {
							const testPart = trimmed.replace(/^[-*]?\s*tests?:\s*/i, '');
							lastTask.tests.push(testPart.trim());
						} else if (/^[-*]\s*`/.test(trimmed)) {
							const filePath = trimmed.replace(/^[-*]\s*/, '').replace(/`/g, '').trim();
							if (filePath.includes('/') || filePath.includes('.')) {
								lastTask.files.push(filePath);
							}
						} else if (!lastTask.description) {
							lastTask.description = trimmed.replace(/^[-*]\s*/, '');
						}
					}
					break;
				}
				case 'filetree':
					if (trimmed.startsWith('-') || trimmed.startsWith('*') || trimmed.includes('/')) {
						fileTree.push(trimmed.replace(/^[-*]\s*/, '').replace(/`/g, '').trim());
					}
					break;
				case 'test':
					if (trimmed) testStrategy += (testStrategy ? '\n' : '') + trimmed;
					break;
				case 'complexity':
					if (/low/i.test(trimmed)) complexity = 'low';
					else if (/high/i.test(trimmed)) complexity = 'high';
					break;
			}
		}

		// Fallback: if parsing yielded nothing, use raw text as summary
		if (!summary && !tasks.length) {
			summary = raw.slice(0, 500);
		}

		return { summary, architecture, tasks, fileTree, testStrategy, estimatedComplexity: complexity };
	}

	function toggleTask(taskId: string) {
		if (!plan) return;
		plan.tasks = plan.tasks.map(t =>
			t.id === taskId ? { ...t, done: !t.done } : t
		);
	}

	function toggleExpand(taskId: string) {
		if (!plan) return;
		plan.tasks = plan.tasks.map(t =>
			t.id === taskId ? { ...t, expanded: !t.expanded } : t
		);
	}

	function copyPlan() {
		if (!plan) return;
		const text = [
			`# Implementation Plan`,
			``,
			`## Summary`,
			plan.summary,
			``,
			`## Architecture`,
			plan.architecture,
			``,
			`## Tasks`,
			...plan.tasks.map((t, i) => [
				`${i + 1}. ${t.done ? '[x]' : '[ ]'} ${t.title}`,
				t.description ? `   ${t.description}` : '',
				t.files.length ? `   Files: ${t.files.join(', ')}` : '',
				t.tests.length ? `   Tests: ${t.tests.join(', ')}` : '',
			].filter(Boolean).join('\n')),
			``,
			`## File Tree`,
			...plan.fileTree.map(f => `- ${f}`),
			``,
			`## Test Strategy`,
			plan.testStrategy,
			``,
			`Complexity: ${plan.estimatedComplexity}`,
		].join('\n');

		navigator.clipboard.writeText(text);
	}

	function openFileInEditor(filePath: string) {
		if (!filePath) return;
		const fullPath = filePath.startsWith('/') ? filePath : `${ide.currentDir}/${filePath}`;
		const name = filePath.split('/').pop() || filePath;
		ide.openFile(fullPath, name);
	}

	let completedCount = $derived(plan?.tasks.filter(t => t.done).length ?? 0);
	let totalCount = $derived(plan?.tasks.length ?? 0);
	let complexityColor = $derived(
		plan?.estimatedComplexity === 'low' ? 'text-gx-status-success' :
		plan?.estimatedComplexity === 'high' ? 'text-gx-status-error' :
		'text-gx-status-warning'
	);

	let activeCodeGen = $derived(activeCodeTaskId ? codeGens.get(activeCodeTaskId) ?? null : null);
	let pipelineProgress = $derived.by(() => {
		if (!plan) return { generated: 0, verified: 0, total: 0 };
		let generated = 0, verified = 0;
		for (const [, gen] of codeGens) {
			if (gen.status === 'verified' || gen.status === 'reviewing') generated++;
			if (gen.status === 'verified') verified++;
		}
		return { generated, verified, total: plan.tasks.length };
	});

	// --- Code Generation Pipeline ---

	async function generateCodeForTask(task: PlanTask) {
		const gen: CodeGeneration = {
			taskId: task.id,
			status: 'generating',
			files: [],
			verificationResult: '',
			errorMessage: '',
		};
		codeGens.set(task.id, gen);
		codeGens = new Map(codeGens);
		activeCodeTaskId = task.id;
		activeView = 'code';

		try {
			const openFiles = ide.openTabs.map(t => `${t.path}: ${t.content?.slice(0, 200)}...`).join('\n');

			const result = await invoke<string>('ide_agent_tool_call', {
				input: `Generate implementation code for this task from a spec-driven plan.

TASK: ${task.title}
DESCRIPTION: ${task.description}
TARGET FILES: ${task.files.join(', ')}
TESTS NEEDED: ${task.tests.join(', ')}

FULL SPEC CONTEXT:
${specContent.slice(0, 1000)}

PLAN CONTEXT:
${plan?.summary || ''}
${plan?.architecture || ''}

WORKSPACE FILES:
${openFiles.slice(0, 1500)}

Return your response in this EXACT format for each file:
===FILE: path/to/file.ext===
\`\`\`
full file content here
\`\`\`
===END FILE===

Generate complete, production-ready code for each file listed in TARGET FILES.`,
				context: 'spec-code-generation',
			});

			// Parse generated files from AI response
			const filePattern = /===FILE:\s*(.+?)===\s*```[\w]*\n([\s\S]*?)```\s*===END FILE===/g;
			let match;
			const files: GeneratedFile[] = [];

			while ((match = filePattern.exec(result)) !== null) {
				const filePath = match[1].trim();
				const content = match[2].trim();
				// Try to get existing content for diff
				const existingTab = ide.openTabs.find(t => t.path.endsWith(filePath) || filePath.endsWith(t.name));
				files.push({
					path: filePath,
					content,
					originalContent: existingTab?.content || '',
					accepted: null,
				});
			}

			// Fallback: if no structured files found, create one file from the response
			if (files.length === 0 && task.files.length > 0) {
				files.push({
					path: task.files[0],
					content: result.replace(/```[\w]*\n?/g, '').trim(),
					originalContent: '',
					accepted: null,
				});
			}

			gen.files = files;
			gen.status = 'reviewing';
		} catch (e) {
			gen.status = 'failed';
			gen.errorMessage = String(e);
		}

		codeGens = new Map(codeGens);
	}

	async function verifyCodeForTask(taskId: string) {
		const gen = codeGens.get(taskId);
		if (!gen || gen.files.length === 0) return;

		gen.status = 'generating'; // re-use generating spinner
		codeGens = new Map(codeGens);

		try {
			const task = plan?.tasks.find(t => t.id === taskId);
			const filesContext = gen.files
				.map(f => `--- ${f.path} ---\n${f.content.slice(0, 1000)}`)
				.join('\n\n');

			const result = await invoke<string>('ide_agent_tool_call', {
				input: `You are a code reviewer (Agent-as-Judge). Verify this generated code against the specification.

TASK: ${task?.title || taskId}
SPEC: ${task?.description || ''}
TESTS EXPECTED: ${task?.tests.join(', ') || 'none specified'}

GENERATED CODE:
${filesContext}

Review for:
1. Does the code fulfill the specification?
2. Are there any bugs or logic errors?
3. Is error handling adequate?
4. Would the expected tests pass?

Respond with:
VERDICT: PASS or FAIL
ISSUES: (list any issues found, or "None")
SUGGESTIONS: (improvements, or "None")`,
				context: 'spec-code-verification',
			});

			gen.verificationResult = result;
			gen.status = /VERDICT:\s*PASS/i.test(result) ? 'verified' : 'failed';
		} catch (e) {
			gen.verificationResult = `Verification error: ${e}`;
			gen.status = 'failed';
		}

		codeGens = new Map(codeGens);
	}

	function setFileAccepted(taskId: string, filePath: string, accepted: boolean) {
		const gen = codeGens.get(taskId);
		if (!gen) return;
		gen.files = gen.files.map(f =>
			f.path === filePath ? { ...f, accepted } : f
		);
		codeGens = new Map(codeGens);
	}

	async function applyAcceptedFiles(taskId: string) {
		const gen = codeGens.get(taskId);
		if (!gen) return;
		applyingFiles = true;

		for (const file of gen.files) {
			if (file.accepted !== true) continue;
			try {
				const fullPath = file.path.startsWith('/') ? file.path : `${ide.currentDir}/${file.path}`;
				await invoke('write_file', { path: fullPath, content: file.content });
				const name = file.path.split('/').pop() || file.path;
				ide.openFile(fullPath, name);
			} catch (e) {
				gen.errorMessage += `\nFailed to write ${file.path}: ${e}`;
			}
		}

		// Mark task as done in plan
		if (plan) {
			plan.tasks = plan.tasks.map(t =>
				t.id === taskId ? { ...t, done: true } : t
			);
		}

		applyingFiles = false;
		codeGens = new Map(codeGens);
	}

	function computeSimpleDiff(original: string, generated: string): Array<{ type: 'same' | 'add' | 'remove'; text: string }> {
		if (!original) return generated.split('\n').map(line => ({ type: 'add' as const, text: line }));
		const origLines = original.split('\n');
		const genLines = generated.split('\n');
		const result: Array<{ type: 'same' | 'add' | 'remove'; text: string }> = [];
		const maxLen = Math.max(origLines.length, genLines.length);

		for (let i = 0; i < maxLen; i++) {
			const orig = origLines[i];
			const gen = genLines[i];
			if (orig === gen) {
				result.push({ type: 'same', text: gen || '' });
			} else {
				if (orig !== undefined) result.push({ type: 'remove', text: orig });
				if (gen !== undefined) result.push({ type: 'add', text: gen });
			}
		}
		return result;
	}
</script>

<div class="flex flex-col h-full {hasEngineStyle ? '' : 'bg-gx-bg-primary'} overflow-hidden" style={containerStyle}>
	<!-- Header -->
	<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-subtle shrink-0">
		<FileText size={12} class="text-gx-neon" />
		<span class="text-xs font-medium text-gx-text-primary">Spec-Driven Development</span>
		<div class="flex-1"></div>

		<!-- View tabs -->
		<div class="flex items-center gap-0.5">
			<button
				onclick={() => { activeView = 'spec'; }}
				class="px-2 py-0.5 text-[10px] rounded transition-colors
					{activeView === 'spec'
						? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:bg-gx-bg-hover border border-transparent'}"
			>Spec</button>
			<button
				onclick={() => { activeView = 'plan'; }}
				disabled={!plan}
				class="px-2 py-0.5 text-[10px] rounded transition-colors disabled:opacity-30
					{activeView === 'plan'
						? 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30'
						: 'text-gx-text-muted hover:bg-gx-bg-hover border border-transparent'}"
			>Plan {plan ? `(${completedCount}/${totalCount})` : ''}</button>
			<button
				onclick={() => { activeView = 'code'; }}
				disabled={codeGens.size === 0}
				class="px-2 py-0.5 text-[10px] rounded transition-colors disabled:opacity-30
					{activeView === 'code'
						? 'bg-gx-accent-cyan/10 text-gx-accent-cyan border border-gx-accent-cyan/30'
						: 'text-gx-text-muted hover:bg-gx-bg-hover border border-transparent'}"
			>
				<Code2 size={8} class="inline mr-0.5" />Code {codeGens.size > 0 ? `(${pipelineProgress.verified}/${pipelineProgress.total})` : ''}
			</button>
		</div>
	</div>

	{#if activeView === 'spec'}
		<!-- Spec Editor View -->
		<div class="flex flex-col flex-1 overflow-hidden">
			<!-- Template selector -->
			<div class="flex items-center gap-1 px-2 py-1 border-b border-gx-border-subtle/50 shrink-0">
				<span class="text-[10px] text-gx-text-disabled">Template:</span>
				<div class="relative">
					<button
						onclick={() => { showTemplates = !showTemplates; }}
						class="flex items-center gap-1 px-1.5 py-0.5 text-[10px] text-gx-text-muted hover:text-gx-text-primary bg-gx-bg-secondary rounded border border-gx-border-default"
					>
						Select
						<ChevronDown size={8} />
					</button>
					{#if showTemplates}
						<div class="absolute top-full left-0 mt-1 bg-gx-bg-elevated border border-gx-border-default rounded shadow-lg z-50 py-1 min-w-28">
							{#each Object.entries(specTemplates) as [key, tmpl]}
								<button
									onclick={() => applyTemplate(key)}
									class="flex items-center gap-2 w-full px-3 py-1 text-xs hover:bg-gx-bg-hover text-left text-gx-text-secondary"
								>{tmpl.label}</button>
							{/each}
						</div>
					{/if}
				</div>

				{#if specHistory.length > 0}
					<span class="text-[10px] text-gx-text-disabled ml-2">Recent:</span>
					{#each specHistory.slice(0, 3) as hist}
						<button
							onclick={() => { specContent = hist.spec; }}
							class="px-1.5 py-0.5 text-[10px] text-gx-text-muted hover:text-gx-text-primary bg-gx-bg-secondary rounded truncate max-w-20"
							title={hist.title}
						>{hist.title}</button>
					{/each}
				{/if}
			</div>

			<!-- Spec textarea -->
			<div class="flex-1 overflow-auto p-2" style={specEditorStyle}>
				<textarea
					bind:value={specContent}
					placeholder="Write your feature specification here...

Use a template above or write freeform.
Describe what you want to build, the requirements,
and any constraints."
					class="w-full h-full bg-transparent border-none outline-none resize-none text-xs text-gx-text-primary placeholder:text-gx-text-muted/50 font-mono leading-relaxed"
				></textarea>
			</div>

			<!-- Generate button -->
			<div class="flex items-center gap-2 px-2 py-1.5 border-t border-gx-border-default shrink-0">
				<button
					onclick={generatePlan}
					disabled={generating || !specContent.trim()}
					class="flex items-center gap-1.5 px-3 py-1 rounded text-xs font-medium transition-all
						{generating
							? 'bg-gx-neon/5 text-gx-neon border border-gx-neon/20'
							: 'bg-gx-neon/10 text-gx-neon border border-gx-neon/30 hover:bg-gx-neon/20'}
						disabled:opacity-30"
				>
					{#if generating}
						<Loader2 size={12} class="animate-spin" />
						Generating Plan...
					{:else}
						<Zap size={12} />
						Generate Plan
					{/if}
				</button>
				<span class="text-[10px] text-gx-text-disabled">{specContent.split('\n').length} lines</span>
			</div>
		</div>

	{:else if activeView === 'plan' && plan}
		<!-- Plan View -->
		<div class="flex-1 overflow-auto" style={planOutputStyle}>
			<!-- Plan header with complexity + copy -->
			<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-subtle/50">
				<Target size={10} class="text-gx-neon" />
				<span class="text-[10px] text-gx-text-muted">Complexity:</span>
				<span class="text-[10px] font-medium {complexityColor}">{plan.estimatedComplexity.toUpperCase()}</span>
				<div class="flex-1"></div>
				{#if totalCount > 0}
					<span class="text-[10px] text-gx-text-disabled">{completedCount}/{totalCount} tasks</span>
				{/if}
				<button
					onclick={copyPlan}
					class="p-1 text-gx-text-muted hover:text-gx-neon rounded hover:bg-gx-bg-hover"
					title="Copy plan to clipboard"
				>
					<Copy size={10} />
				</button>
			</div>

			<div class="p-2 space-y-3">
				<!-- Summary -->
				{#if plan.summary}
					<div>
						<h4 class="text-[10px] font-medium text-gx-text-muted uppercase tracking-wider mb-1">Summary</h4>
						<p class="text-xs text-gx-text-secondary leading-relaxed">{plan.summary}</p>
					</div>
				{/if}

				<!-- Architecture -->
				{#if plan.architecture}
					<div>
						<h4 class="text-[10px] font-medium text-gx-text-muted uppercase tracking-wider mb-1">Architecture</h4>
						<p class="text-xs text-gx-text-secondary leading-relaxed">{plan.architecture}</p>
					</div>
				{/if}

				<!-- Tasks -->
				{#if plan.tasks.length > 0}
					<div>
						<h4 class="flex items-center gap-1 text-[10px] font-medium text-gx-text-muted uppercase tracking-wider mb-1.5">
							<ListChecks size={10} />
							Tasks ({completedCount}/{totalCount})
						</h4>
						<div class="space-y-1">
							{#each plan.tasks as task, i}
								<div class="rounded border {task.done ? 'border-gx-status-success/20 bg-gx-status-success/5' : 'border-gx-border-default bg-gx-bg-secondary'}">
									<div class="flex items-start gap-2 px-2 py-1.5">
										<button
											onclick={() => toggleTask(task.id)}
											class="mt-0.5 shrink-0"
										>
											{#if task.done}
												<CheckCircle size={12} class="text-gx-status-success" />
											{:else}
												<Circle size={12} class="text-gx-text-disabled" />
											{/if}
										</button>
										<!-- svelte-ignore a11y_no_static_element_interactions -->
										<div
											class="flex-1 min-w-0 cursor-pointer"
											onclick={() => toggleExpand(task.id)}
											onkeydown={(e) => e.key === 'Enter' && toggleExpand(task.id)}
										>
											<div class="flex items-center gap-1">
												{#if task.expanded}
													<ChevronDown size={10} class="text-gx-text-disabled shrink-0" />
												{:else}
													<ChevronRight size={10} class="text-gx-text-disabled shrink-0" />
												{/if}
												<span class="text-xs {task.done ? 'line-through text-gx-text-disabled' : 'text-gx-text-primary'}">
													{i + 1}. {task.title}
												</span>
											</div>
										</div>
										<!-- Generate Code button -->
										{#if !task.done}
											{@const taskGen = codeGens.get(task.id)}
											<button
												onclick={(e) => { e.stopPropagation(); generateCodeForTask(task); }}
												disabled={taskGen?.status === 'generating'}
												class="flex items-center gap-0.5 px-1.5 py-0.5 text-[9px] rounded shrink-0 transition-colors
													{taskGen?.status === 'verified'
														? 'bg-gx-status-success/10 text-gx-status-success border border-gx-status-success/20'
														: taskGen?.status === 'reviewing'
															? 'bg-gx-accent-cyan/10 text-gx-accent-cyan border border-gx-accent-cyan/20'
															: taskGen?.status === 'failed'
																? 'bg-gx-status-error/10 text-gx-status-error border border-gx-status-error/20'
																: 'bg-gx-accent-magenta/10 text-gx-accent-magenta border border-gx-accent-magenta/20 hover:bg-gx-accent-magenta/20'}
													disabled:opacity-40"
												title="Generate code for this task"
											>
												{#if taskGen?.status === 'generating'}
													<Loader2 size={8} class="animate-spin" />
												{:else if taskGen?.status === 'verified'}
													<Check size={8} />
												{:else}
													<Code2 size={8} />
												{/if}
												{taskGen?.status === 'generating' ? 'Gen...' : taskGen?.status === 'verified' ? 'Done' : taskGen?.status === 'reviewing' ? 'Review' : 'Code'}
											</button>
										{/if}
									</div>

									{#if task.expanded}
										<div class="px-7 pb-2 space-y-1.5">
											{#if task.description}
												<p class="text-[11px] text-gx-text-muted">{task.description}</p>
											{/if}
											{#if task.files.length > 0}
												<div>
													<span class="text-[9px] text-gx-text-disabled uppercase">Files:</span>
													<div class="flex flex-wrap gap-1 mt-0.5">
														{#each task.files as file}
															<button
																onclick={() => openFileInEditor(file)}
																class="px-1.5 py-0.5 text-[10px] bg-gx-bg-primary border border-gx-border-default rounded text-gx-accent-cyan hover:bg-gx-bg-hover font-mono truncate max-w-40"
																title={file}
															>{file.split('/').pop()}</button>
														{/each}
													</div>
												</div>
											{/if}
											{#if task.tests.length > 0}
												<div>
													<span class="text-[9px] text-gx-text-disabled uppercase">Tests:</span>
													{#each task.tests as test}
														<p class="text-[10px] text-gx-text-muted mt-0.5">
															<TestTube2 size={8} class="inline mr-0.5 text-gx-status-success" />{test}
														</p>
													{/each}
												</div>
											{/if}
										</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/if}

				<!-- File Tree -->
				{#if plan.fileTree.length > 0}
					<div>
						<h4 class="flex items-center gap-1 text-[10px] font-medium text-gx-text-muted uppercase tracking-wider mb-1">
							<FolderTree size={10} />
							Affected Files ({plan.fileTree.length})
						</h4>
						<div class="flex flex-wrap gap-1">
							{#each plan.fileTree as file}
								<button
									onclick={() => openFileInEditor(file)}
									class="px-1.5 py-0.5 text-[10px] bg-gx-bg-secondary border border-gx-border-default rounded text-gx-text-muted hover:text-gx-accent-cyan font-mono truncate max-w-48"
									title={file}
								>{file}</button>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Test Strategy -->
				{#if plan.testStrategy}
					<div>
						<h4 class="flex items-center gap-1 text-[10px] font-medium text-gx-text-muted uppercase tracking-wider mb-1">
							<TestTube2 size={10} />
							Test Strategy
						</h4>
						<pre class="text-[11px] text-gx-text-secondary whitespace-pre-wrap font-sans leading-relaxed">{plan.testStrategy}</pre>
					</div>
				{/if}
			</div>
		</div>

		<!-- Plan actions bar -->
		<div class="flex items-center gap-2 px-2 py-1.5 border-t border-gx-border-default shrink-0">
			<button
				onclick={() => { activeView = 'spec'; }}
				class="px-2 py-0.5 text-[10px] text-gx-text-muted hover:text-gx-text-primary bg-gx-bg-secondary rounded border border-gx-border-default"
			>Edit Spec</button>
			<button
				onclick={generatePlan}
				disabled={generating}
				class="flex items-center gap-1 px-2 py-0.5 text-[10px] text-gx-neon hover:bg-gx-neon/10 rounded border border-gx-neon/20 disabled:opacity-30"
			>
				<Zap size={8} />
				Regenerate
			</button>
			{#if pipelineProgress.generated > 0}
				<span class="text-[9px] text-gx-text-disabled ml-auto">
					Pipeline: {pipelineProgress.verified}/{pipelineProgress.total} verified
				</span>
			{/if}
		</div>

	{:else if activeView === 'code'}
		<!-- Code Generation Pipeline View -->
		<div class="flex flex-col flex-1 overflow-hidden">
			<!-- Pipeline header -->
			<div class="flex items-center gap-2 px-3 py-1.5 border-b border-gx-border-subtle/50 shrink-0">
				<Code2 size={10} class="text-gx-accent-cyan" />
				<span class="text-[10px] text-gx-text-muted">Pipeline:</span>
				<!-- Stage indicators -->
				<div class="flex items-center gap-1">
					<span class="text-[9px] px-1.5 py-0.5 rounded bg-gx-neon/10 text-gx-neon">Spec</span>
					<ArrowRight size={8} class="text-gx-text-disabled" />
					<span class="text-[9px] px-1.5 py-0.5 rounded bg-gx-neon/10 text-gx-neon">Plan</span>
					<ArrowRight size={8} class="text-gx-text-disabled" />
					<span class="text-[9px] px-1.5 py-0.5 rounded {activeCodeGen?.status === 'generating' ? 'bg-gx-accent-magenta/10 text-gx-accent-magenta' : activeCodeGen?.status === 'reviewing' || activeCodeGen?.status === 'verified' ? 'bg-gx-accent-cyan/10 text-gx-accent-cyan' : 'bg-gx-bg-secondary text-gx-text-disabled'}">Code</span>
					<ArrowRight size={8} class="text-gx-text-disabled" />
					<span class="text-[9px] px-1.5 py-0.5 rounded {activeCodeGen?.status === 'verified' ? 'bg-gx-status-success/10 text-gx-status-success' : 'bg-gx-bg-secondary text-gx-text-disabled'}">Verify</span>
				</div>
				<div class="flex-1"></div>
				<!-- Task selector -->
				{#if plan}
					<select
						class="text-[10px] bg-gx-bg-secondary border border-gx-border-default rounded px-1 py-0.5 text-gx-text-muted"
						onchange={(e) => activeCodeTaskId = (e.target as HTMLSelectElement).value || null}
					>
						<option value="">Select task...</option>
						{#each plan.tasks as task, i}
							{#if codeGens.has(task.id)}
								<option value={task.id} selected={activeCodeTaskId === task.id}>{i + 1}. {task.title.slice(0, 30)}</option>
							{/if}
						{/each}
					</select>
				{/if}
			</div>

			{#if activeCodeGen}
				<!-- Generated files diff view -->
				<div class="flex-1 overflow-auto">
					{#if activeCodeGen.status === 'generating'}
						<div class="flex items-center justify-center h-32 gap-2">
							<Loader2 size={16} class="animate-spin text-gx-accent-magenta" />
							<span class="text-xs text-gx-text-muted">Generating code...</span>
						</div>
					{:else if activeCodeGen.files.length === 0}
						<div class="flex items-center justify-center h-32">
							<span class="text-xs text-gx-text-disabled">No files generated</span>
						</div>
					{:else}
						<div class="p-2 space-y-3">
							{#each activeCodeGen.files as file}
								<div class="rounded border border-gx-border-default overflow-hidden">
									<!-- File header -->
									<div class="flex items-center gap-2 px-2 py-1 bg-gx-bg-secondary border-b border-gx-border-default">
										<FileText size={10} class="text-gx-accent-cyan shrink-0" />
										<span class="text-[10px] font-mono text-gx-text-primary truncate flex-1">{file.path}</span>
										<span class="text-[9px] text-gx-text-disabled">{file.content.split('\n').length} lines</span>
										<!-- Accept/Reject buttons -->
										<div class="flex items-center gap-0.5">
											<button
												onclick={() => setFileAccepted(activeCodeGen!.taskId, file.path, true)}
												class="p-0.5 rounded transition-colors {file.accepted === true ? 'bg-gx-status-success/20 text-gx-status-success' : 'text-gx-text-disabled hover:text-gx-status-success'}"
												title="Accept this file"
											>
												<Check size={10} />
											</button>
											<button
												onclick={() => setFileAccepted(activeCodeGen!.taskId, file.path, false)}
												class="p-0.5 rounded transition-colors {file.accepted === false ? 'bg-gx-status-error/20 text-gx-status-error' : 'text-gx-text-disabled hover:text-gx-status-error'}"
												title="Reject this file"
											>
												<XCircle size={10} />
											</button>
										</div>
									</div>
									<!-- Diff content -->
									<div class="max-h-60 overflow-auto bg-gx-bg-primary font-mono text-[10px] leading-tight">
										{#each computeSimpleDiff(file.originalContent, file.content).slice(0, 100) as diffLine, lineIdx}
											<div class="flex {diffLine.type === 'add' ? 'bg-gx-status-success/8' : diffLine.type === 'remove' ? 'bg-gx-status-error/8' : ''}">
												<span class="w-8 text-right pr-1 text-gx-text-disabled select-none shrink-0">{lineIdx + 1}</span>
												<span class="px-1 shrink-0 {diffLine.type === 'add' ? 'text-gx-status-success' : diffLine.type === 'remove' ? 'text-gx-status-error' : 'text-gx-text-disabled'}">{diffLine.type === 'add' ? '+' : diffLine.type === 'remove' ? '-' : ' '}</span>
												<span class="flex-1 whitespace-pre {diffLine.type === 'add' ? 'text-gx-text-primary' : diffLine.type === 'remove' ? 'text-gx-text-disabled line-through' : 'text-gx-text-muted'}">{diffLine.text}</span>
											</div>
										{/each}
									</div>
								</div>
							{/each}

							<!-- Verification result -->
							{#if activeCodeGen.verificationResult}
								<div class="rounded border {activeCodeGen.status === 'verified' ? 'border-gx-status-success/30 bg-gx-status-success/5' : 'border-gx-status-error/30 bg-gx-status-error/5'} p-2">
									<div class="flex items-center gap-1 mb-1">
										<Eye size={10} class={activeCodeGen.status === 'verified' ? 'text-gx-status-success' : 'text-gx-status-error'} />
										<span class="text-[10px] font-medium {activeCodeGen.status === 'verified' ? 'text-gx-status-success' : 'text-gx-status-error'}">
											{activeCodeGen.status === 'verified' ? 'Verification Passed' : 'Verification Failed'}
										</span>
									</div>
									<pre class="text-[10px] text-gx-text-secondary whitespace-pre-wrap font-sans">{activeCodeGen.verificationResult.slice(0, 500)}</pre>
								</div>
							{/if}

							{#if activeCodeGen.errorMessage}
								<div class="rounded border border-gx-status-error/30 bg-gx-status-error/5 p-2">
									<span class="text-[10px] text-gx-status-error">{activeCodeGen.errorMessage}</span>
								</div>
							{/if}
						</div>
					{/if}
				</div>

				<!-- Code gen action bar -->
				<div class="flex items-center gap-2 px-2 py-1.5 border-t border-gx-border-default shrink-0">
					<button
						onclick={() => verifyCodeForTask(activeCodeGen!.taskId)}
						disabled={activeCodeGen.status === 'generating' || activeCodeGen.files.length === 0}
						class="flex items-center gap-1 px-2 py-0.5 text-[10px] rounded border transition-colors disabled:opacity-30
							{activeCodeGen.status === 'verified'
								? 'text-gx-status-success border-gx-status-success/20 bg-gx-status-success/5'
								: 'text-gx-accent-cyan border-gx-accent-cyan/20 hover:bg-gx-accent-cyan/10'}"
					>
						<Eye size={8} />
						{activeCodeGen.status === 'verified' ? 'Re-verify' : 'Verify'}
					</button>
					<button
						onclick={() => applyAcceptedFiles(activeCodeGen!.taskId)}
						disabled={applyingFiles || !activeCodeGen.files.some(f => f.accepted === true)}
						class="flex items-center gap-1 px-2 py-0.5 text-[10px] text-gx-neon rounded border border-gx-neon/20 hover:bg-gx-neon/10 disabled:opacity-30"
					>
						{#if applyingFiles}
							<Loader2 size={8} class="animate-spin" />
						{:else}
							<Save size={8} />
						{/if}
						Apply ({activeCodeGen.files.filter(f => f.accepted === true).length} files)
					</button>
					<button
						onclick={() => { const t = plan?.tasks.find(t => t.id === activeCodeGen?.taskId); if (t) generateCodeForTask(t); }}
						disabled={activeCodeGen.status === 'generating'}
						class="flex items-center gap-1 px-2 py-0.5 text-[10px] text-gx-text-muted rounded border border-gx-border-default hover:bg-gx-bg-hover disabled:opacity-30"
					>
						<RotateCcw size={8} />
						Regenerate
					</button>
					<button
						onclick={() => activeView = 'plan'}
						class="text-[10px] text-gx-text-muted hover:text-gx-text-primary ml-auto"
					>Back to Plan</button>
				</div>
			{:else}
				<div class="flex-1 flex items-center justify-center">
					<div class="text-center">
						<Code2 size={24} class="text-gx-text-disabled mx-auto mb-2" />
						<p class="text-xs text-gx-text-disabled">Select a task from the plan and click "Code" to generate implementation</p>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
