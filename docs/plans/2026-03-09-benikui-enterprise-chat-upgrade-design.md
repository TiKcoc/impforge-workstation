# BenikUI Enterprise Chat Upgrade — Design Document

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform ImpForge's chat system into a BenikUI-inspired modular enterprise interface with dark glassmorphism, per-model branded indicators, hierarchical reasoning display, and neon DAG pipeline visualization.

**Architecture:** BenikUI module registry pattern (register/unregister/configure panels) translated to Svelte 5 runes. Each chat panel becomes a registered module with independent config, position, and lifecycle. Dark glassmorphism surfaces with per-model neon accents.

**Tech Stack:** Svelte 5 runes, Tailwind CSS with gx-* design tokens, CSS Houdini @property, SVG animations, Tauri Channel streaming

**Research Basis:** 34 academic papers (see `docs/research/2026-03-09-chat-terminal-ui-agent-visualization-research.md`), BenikUI/ElvUI plugin architecture, LobeChat/Warp/Claude UI patterns

---

## 1. Dark Glassmorphism Design System

### Problem
Current panels use flat `bg-gx-bg-secondary` with solid borders. Looks functional but not enterprise-premium.

### Solution
Frosted glass surfaces leveraging existing `BackgroundType = 'Glass'` in `style-engine.svelte.ts`:

```css
.glass-panel {
  background: rgba(13, 13, 18, 0.7);
  backdrop-filter: blur(16px) saturate(1.6);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.04);
}
```

### Per-Model Accent Colors
```
Claude:     #a855f7 (purple)
Qwen:       #06b6d4 (cyan)
Hermes:     #f59e0b (amber)
Ollama/Local: #22c55e (green)
System:     #00ff66 (gx-neon)
Error:      #ef4444 (red)
```

### Ambient Gradient Orbs
Floating radial gradients behind panels that subtly shift with the active model's color.

---

## 2. BenikUI Module Registry

### Pattern Translation
| BenikUI (Lua) | ImpForge (Svelte 5) |
|---------------|---------------------|
| `E:NewModule('name', Module)` | `registry.register({ id, component, position, config })` |
| `EP:RegisterPlugin(addon, ...)` | Module exports `moduleConfig` object |
| `Module:Initialize()` | Svelte `onMount` + registry `init` callback |
| `Module:InsertOptions()` | Settings page auto-generates UI from module config schema |
| Toggle Anchors (edit mode) | `Ctrl+E` layout edit mode with drag handles |
| Profile Export (compressed) | `exportProfile()` → JSON → base64 string |

### Registry Store
```typescript
interface ChatModule {
  id: string;
  label: string;
  icon: Component;
  component: Component;
  position: 'left' | 'center' | 'right' | 'header' | 'footer';
  defaultSize: { width: string; height: string };
  collapsible: boolean;
  visible: boolean;
  order: number;
  config: Record<string, unknown>;
}
```

### Default Modules
1. **AgentTopology** (left) — Model states + routing visualization
2. **ChatStream** (center) — Messages + input
3. **ReasoningInspector** (right) — CoT hierarchy + context
4. **TokenBudgetBar** (header) — Context usage visualization
5. **ContextAssembly** (footer) — Enhanced @-mention input

---

## 3. Per-Model Branded Typing Indicators

### ModelAvatar Component
Each model gets a distinct avatar with branded animation during streaming:

| Model Pattern | Animation | CSS Keyframe |
|---------------|-----------|-------------|
| Claude (purple) | Orbital ring | `rotate(0deg)` → `rotate(360deg)` around avatar |
| Qwen (cyan) | Pulse breathing | `scale(0.95)` → `scale(1.05)` with glow expand |
| Hermes (amber) | Lightning flicker | Random `opacity: 0.4-1.0` at 100ms intervals |
| Local/Ollama (green) | Steady emanation | Constant soft glow with subtle radius pulse |

### Implementation
- New `ModelAvatar.svelte` component with `modelId` prop
- Color derived from model name via `getModelAccent()` utility
- Animation class applied when `streaming === true`
- Replaces current generic `Bot` icon in `ChatMessage.svelte`

---

## 4. Hierarchical Reasoning Display (iGraph-inspired)

### Problem
Current `<thinking>` blocks render as raw text in a collapsible panel. Research (arXiv:2510.22922) shows graph-based visualization achieves 85.6% error detection vs 73.5% for raw text.

### Solution: ReasoningBlock Component
```
▼ Reasoning (3 steps, 1.2s)
  ├─ 🔍 Analysis: "User wants to refactor the auth module..."
  ├─ 📋 Plan: "1. Extract interface 2. Move to separate file..."
  └─ ⚡ Action: "Applying pattern: Strategy with DI..."
```

- Parse thinking content into steps (split on numbered lists, bullet points, or paragraph breaks)
- Show step icons based on content keywords (analysis, plan, action, error)
- Progressive disclosure: summary line → full content on click
- Elapsed time badge per step (from streaming timestamps)
- Color accent matches the active model

---

## 5. Enhanced Pipeline DAG with Neon Effects

### Current State
`ModelPipelineView.svelte` — 5-node SVG DAG (Input→Classifier→Model⟷Memory→Output) with basic dash animation.

### Upgrades
1. **Neon glow borders** — Node stroke uses model accent color with `feGaussianBlur` glow
2. **Flowing particles** — Animated circles along edge paths (not just dashed lines)
3. **Metrics overlay** — tokens/s, latency, cost displayed on active nodes
4. **Topology label** — Shows orchestration pattern (parallel/sequential/hierarchical)
5. **Responsive SVG** — `viewBox` with `preserveAspectRatio` for fluid scaling
6. **Clickable nodes** — Click to expand node details (model info, config)

---

## 6. Token Budget Visualization Bar

### New Component: TokenBudgetBar.svelte
```
┌──────────────────────────────────────────────────────┐
│ [████████████████░░░░░░] 72% · 92K / 128K tokens     │
│  System: 8K │ History: 42K │ @file: 12K │ Free: 36K  │
└──────────────────────────────────────────────────────┘
```

- Segmented bar with color-coded sections
- Color transitions: `#22c55e` (green) → `#f59e0b` (amber) → `#ef4444` (red)
- Tooltip on hover showing per-source breakdown
- Integrates with ChatInput @-mention system

---

## 7. Enhanced ChatInput

### Upgrades
1. **Context preview cards** — Hovering @-mention shows file preview above input
2. **Token counter** — Live token estimate for current message (`~${tokens} tokens`)
3. **Model selector pill** — Click to override auto-routing for this message
4. **Slash commands** — `/clear`, `/model`, `/export` inline commands
5. **Voice input button** — (future, placeholder)

---

## Phase Plan

| Phase | Components | Estimated LoC |
|-------|-----------|---------------|
| P1: Glass Design System | `GlassPanel.svelte`, CSS vars, `app.css` updates | ~200 |
| P2: Module Registry | `module-registry.svelte.ts`, panel refactor | ~350 |
| P3: Model Avatars | `ModelAvatar.svelte`, `ChatMessage.svelte` update | ~200 |
| P4: Reasoning Hierarchy | `ReasoningBlock.svelte`, `types.ts` parser upgrade | ~250 |
| P5: Pipeline DAG Upgrade | `ModelPipelineView.svelte` neon + particles | ~200 |
| P6: Token Budget + Input | `TokenBudgetBar.svelte`, `ChatInput.svelte` upgrade | ~250 |
| **Total** | | **~1,450 LoC** |

---

## Anti-Patterns to Avoid (Research-Backed)

| Don't | Do Instead | Evidence |
|-------|-----------|----------|
| Raw CoT text dumps | Hierarchical topic viz | arXiv:2510.22922 |
| Mid-task interruptions | Workflow boundary delivery | arXiv:2601.10253 |
| Hidden AI reasoning | Progressive disclosure | arXiv:2506.23678 |
| Chat-only for all tasks | Adaptive interface | arXiv:2508.19227 |
| Generic typing indicators | Per-model branded | 2025 AI Agent Index |
| Fixed panel layouts | BenikUI module registry | ElvUI plugin system |
