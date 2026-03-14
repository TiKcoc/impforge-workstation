/**
 * Guided Tour Store — Interactive Product Tour System
 *
 * Progressive disclosure with spotlight overlay, tooltips, and role-based tours.
 * Based on: Progressive Disclosure (IxDF), Product Tour Best Practices (Chameleon/Appcues)
 *
 * Features:
 * - Spotlight overlay highlighting UI elements
 * - Step-by-step tooltips with actions
 * - Role-based tour content (Developer sees different tour than Office User)
 * - Module discovery for exploring deactivated modules
 * - Skip/dismiss at any time
 * - Tracks completed tours per user
 */

import { getSetting, saveSetting, getVisibleModules, type AppSettings } from './settings.svelte';

// ── Types ────────────────────────────────────────────────────────────

export interface TourStep {
	/** CSS selector for the target element to spotlight */
	target: string;
	/** Tooltip title */
	title: string;
	/** Tooltip description */
	description: string;
	/** Position relative to target */
	placement: 'top' | 'bottom' | 'left' | 'right';
	/** Optional: navigate to this route before showing step */
	route?: string;
	/** Optional: which module this step belongs to (for filtering) */
	moduleId?: string;
	/** Optional: action button text */
	actionLabel?: string;
	/** Optional: action to perform */
	action?: () => void;
}

export interface Tour {
	id: string;
	name: string;
	description: string;
	/** Which roles see this tour (empty = all) */
	roles: AppSettings['userRole'][];
	steps: TourStep[];
}

// ── Tour Definitions ─────────────────────────────────────────────────

const TOURS: Tour[] = [
	{
		id: 'welcome',
		name: 'Welcome Tour',
		description: 'Get to know ImpForge in 60 seconds',
		roles: [],
		steps: [
			{
				target: '[data-tour="sidebar"]',
				title: 'Navigation',
				description: 'Your modules are here. We\'ve selected the ones that match your role — you can add more anytime in Settings.',
				placement: 'right',
			},
			{
				target: '[data-tour="nav-chat"]',
				title: 'Chat / TerminalUI',
				description: 'Your AI assistant. Chat with local or cloud models, with an integrated code editor and terminal.',
				placement: 'right',
				route: '/chat',
				moduleId: 'chat',
			},
			{
				target: '[data-tour="status-bar"]',
				title: 'System Status',
				description: 'CPU, RAM, GPU, and disk usage — always visible. Click for details.',
				placement: 'top',
			},
			{
				target: '[data-tour="nav-settings"]',
				title: 'Settings',
				description: 'Change your profile, modules, theme, AI providers, and integrations here. Everything is customizable.',
				placement: 'right',
				route: '/settings',
			},
		],
	},
	{
		id: 'developer-tools',
		name: 'Developer Toolkit',
		description: 'IDE, Git, Docker, and Agent tools',
		roles: ['developer', 'custom'],
		steps: [
			{
				target: '[data-tour="nav-ide"]',
				title: 'CodeForge IDE',
				description: 'Full IDE with LSP, syntax highlighting, Git panel, debugger, terminal, and AI completions. Like VS Code — inside ImpForge.',
				placement: 'right',
				route: '/ide',
				moduleId: 'ide',
			},
			{
				target: '[data-tour="nav-docker"]',
				title: 'Docker Dashboard',
				description: 'Start, stop, and inspect containers without leaving ImpForge. Logs, ports, and stats at a glance.',
				placement: 'right',
				route: '/docker',
				moduleId: 'docker',
			},
			{
				target: '[data-tour="nav-github"]',
				title: 'GitHub Integration',
				description: 'Browse repos, issues, and PRs. Push code, review changes — all from here.',
				placement: 'right',
				route: '/github',
				moduleId: 'github',
			},
			{
				target: '[data-tour="nav-agents"]',
				title: 'NeuralSwarm Agents',
				description: 'AI agents that work for you: code review, research, debugging, CI/CD. Start, stop, and monitor them live.',
				placement: 'right',
				route: '/agents',
				moduleId: 'agents',
			},
		],
	},
	{
		id: 'ai-basics',
		name: 'AI Setup Tour',
		description: 'How to use local and cloud AI models',
		roles: ['beginner' as any],
		steps: [
			{
				target: '[data-tour="nav-ai"]',
				title: 'AI Models',
				description: 'Configure which AI models to use. Ollama runs models locally on your machine — 100% private, no internet needed.',
				placement: 'right',
				route: '/ai',
				moduleId: 'ai',
			},
			{
				target: '[data-tour="nav-chat"]',
				title: 'Start Chatting',
				description: 'Select a model in the dropdown, type your question, press Enter. That\'s it! The AI responds in real-time.',
				placement: 'right',
				route: '/chat',
				moduleId: 'chat',
			},
		],
	},
];

// ── Module Discovery Tours (for exploring deactivated modules) ───────

export interface ModuleInfo {
	id: string;
	name: string;
	description: string;
	icon: string;
	features: string[];
	route: string;
}

export const ALL_MODULES: ModuleInfo[] = [
	{ id: 'chat', name: 'Chat / TerminalUI', description: 'Multi-model AI chat with code editor and terminal', icon: 'MessageSquare', features: ['Ollama local models', 'OpenRouter cloud', 'Streaming responses', 'Code highlighting'], route: '/chat' },
	{ id: 'ide', name: 'CodeForge IDE', description: 'Full code editor with LSP, Git, debugger, and AI completions', icon: 'Code2', features: ['25-language syntax', 'LSP completions', 'Git panel', 'Debug (DAP)', 'Terminal'], route: '/ide' },
	{ id: 'github', name: 'GitHub', description: 'Repository management, issues, and pull requests', icon: 'GitBranch', features: ['Repo browser', 'Issue tracker', 'PR reviews', 'OAuth login'], route: '/github' },
	{ id: 'docker', name: 'Docker', description: 'Container orchestration and management', icon: 'Container', features: ['Start/Stop containers', 'View logs', 'Port mapping', 'Image management'], route: '/docker' },
	{ id: 'n8n', name: 'n8n & Services', description: 'Workflow automation and service monitoring', icon: 'Workflow', features: ['Workflow builder', 'Service health', 'API monitoring'], route: '/n8n' },
	{ id: 'agents', name: 'NeuralSwarm', description: 'AI agent orchestration with trust-based scheduling', icon: 'Network', features: ['42 workers', 'Live dashboard', 'Start/stop agents', 'Log viewer'], route: '/agents' },
	{ id: 'evaluation', name: 'Evaluation', description: 'AI output quality assessment and leaderboards', icon: 'Shield', features: ['Agent-as-Judge', 'Quality metrics', 'Model comparison'], route: '/evaluation' },
	{ id: 'ai', name: 'AI Models', description: 'Model management and provider configuration', icon: 'Brain', features: ['Model browser', 'Download manager', 'Provider setup'], route: '/ai' },
	{ id: 'browser', name: 'Browser Agent', description: 'Web automation, scraping, and browser control', icon: 'Globe', features: ['CDP automation', 'Web scraping', 'Page screenshots', 'Form filling'], route: '/browser' },
	{ id: 'news', name: 'AI News', description: 'Curated feed from AI development sources', icon: 'Newspaper', features: ['6 RSS sources', 'Hacker News AI', 'Tauri/Svelte/Rust blogs'], route: '/news' },
];

// ── Store State ──────────────────────────────────────────────────────

let activeTour = $state<Tour | null>(null);
let currentStepIndex = $state(0);
let completedTours = $state<string[]>([]);
let showModuleDiscovery = $state(false);

// ── Derived ──────────────────────────────────────────────────────────

let currentStep = $derived(activeTour?.steps[currentStepIndex] ?? null);
let totalSteps = $derived(activeTour?.steps.length ?? 0);
let isActive = $derived(activeTour !== null);
let progress = $derived(totalSteps > 0 ? ((currentStepIndex + 1) / totalSteps) * 100 : 0);

// ── Actions ──────────────────────────────────────────────────────────

function startTour(tourId: string) {
	const tour = TOURS.find(t => t.id === tourId);
	if (!tour) return;

	// Filter steps to only show modules the user has access to (or all for discovery)
	const visible = getVisibleModules();
	const filteredSteps = tour.steps.filter(s =>
		!s.moduleId || visible.includes(s.moduleId)
	);

	if (filteredSteps.length === 0) return;

	activeTour = { ...tour, steps: filteredSteps };
	currentStepIndex = 0;
}

function nextStep() {
	if (!activeTour) return;
	if (currentStepIndex < activeTour.steps.length - 1) {
		currentStepIndex += 1;
	} else {
		completeTour();
	}
}

function prevStep() {
	if (currentStepIndex > 0) {
		currentStepIndex -= 1;
	}
}

function skipTour() {
	if (activeTour) {
		completedTours = [...completedTours, activeTour.id];
	}
	activeTour = null;
	currentStepIndex = 0;
}

function completeTour() {
	if (activeTour) {
		completedTours = [...completedTours, activeTour.id];
		saveSetting('onboardingComplete', true);
	}
	activeTour = null;
	currentStepIndex = 0;
}

function toggleModuleDiscovery() {
	showModuleDiscovery = !showModuleDiscovery;
}

/** Get tours available for the current user role */
function getAvailableTours(): Tour[] {
	const role = getSetting('userRole');
	return TOURS.filter(t =>
		t.roles.length === 0 || t.roles.includes(role)
	);
}

/** Get modules the user has NOT activated (for discovery) */
function getHiddenModules(): ModuleInfo[] {
	const visible = getVisibleModules();
	return ALL_MODULES.filter(m => !visible.includes(m.id));
}

// ── Export ────────────────────────────────────────────────────────────

export const tourStore = {
	get activeTour() { return activeTour; },
	get currentStep() { return currentStep; },
	get currentStepIndex() { return currentStepIndex; },
	get totalSteps() { return totalSteps; },
	get isActive() { return isActive; },
	get progress() { return progress; },
	get completedTours() { return completedTours; },
	get showModuleDiscovery() { return showModuleDiscovery; },
	startTour,
	nextStep,
	prevStep,
	skipTour,
	completeTour,
	toggleModuleDiscovery,
	getAvailableTours,
	getHiddenModules,
};
