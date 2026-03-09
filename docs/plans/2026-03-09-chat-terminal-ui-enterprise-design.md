# ImpForge ChatTerminalUI Enterprise Upgrade — Design Document

**Date**: 2026-03-09
**Status**: Approved
**Author**: Claude Opus 4.6 + orkel

## Goal

Transform ImpForge's basic chat page into a futuristic, modular ChatTerminalUI system — the "living room" of the app where users spend 80% of their time. Three switchable chat placement modes, three stream rendering modes, and three model visualization levels. Full 3x3x3 modular system — all switchable in Settings, BenikUI-style dependency trees.

## Architecture: "3x3x3" Modular System

### 3 Chat Placement Modes (where the chat lives)

| Mode | Description | Shortcut |
|------|-------------|----------|
| **Side-Panel** | Slide-out chat panel available on every page (Cursor-style) | `Ctrl+J` |
| **Dedicated Page** | Full `/chat` page with sidebar, premium rendering | Activity Bar |
| **Full Convergence** | Chat + IDE + Terminal unified in one view | `Ctrl+Shift+F` |

### 3 Stream Rendering Modes (how messages display)

| Mode | Description |
|------|-------------|
| **Split Panel** | Chat messages top, Terminal output bottom — classic layout |
| **Unified Stream** | All inline: Chat, Tool, Thinking, Terminal, Diff, Diagram blocks |
| **Mission Control** | Split: Chat left, Live Dashboard right (Pipeline, Gauges, Metrics) |

### 3 Model Visualization Levels (how much AI "inner life" is shown)

| Level | Description |
|-------|-------------|
| **Minimal Badges** | Color status dots + model name in message header (idle/thinking/generating/error) |
| **Activity Cards** | Animated cards per model with pulse, token counter, task type, routing reason |
| **Full Pipeline** | Animated DAG: Input → Classifier → Model(s) → Output with live data flow |

#### Full Pipeline Details
- User Input → Classifier (10ms) → Selected Model → Output
- Nodes pulse when active, edges show token flow as animated particles
- ForgeMemory context assembly visible as side-nodes
- MoA (Mixture-of-Agents) shows parallel pipelines
- Hover for details (token count, latency, prompt excerpt)
- Implementation: SVG with CSS animations (max ~20 nodes, no Canvas needed)

#### Activity Cards Details
```
+------------------------------+
| Lightning Devstral-Small     |
| ████████░░ Generating (78%)  |
| Task: CodeGeneration         |
| Tokens: 234/4096 - 45ms/tk  |
| Route: "code detected"       |
| [animated pulse]             |
+------------------------------+
```

### Settings Structure

```typescript
interface ChatLayoutSettings {
  placement: 'side-panel' | 'dedicated' | 'convergence'
  streamMode: 'split' | 'unified' | 'mission-control'
  vizLevel: 'minimal' | 'cards' | 'pipeline'
  showThinking: boolean
  showRouting: boolean
  animationsEnabled: boolean
  compactMode: boolean
}
```

Stored in Tauri Store (`.nexus-settings.json`), reactive via settings.svelte.ts.

---

## Tech Stack

### Rendering Libraries

| Library | Purpose | Size | License |
|---------|---------|------|---------|
| `marked` | Markdown → HTML | 32kb | MIT |
| `highlight.js` | Syntax highlighting (50+ languages) | 25kb core | BSD-3 |
| `katex` | LaTeX/Math rendering | 200kb | MIT |
| `mermaid` | Diagrams (lazy-loaded) | 180kb lazy | MIT |
| `@git-diff-view/svelte` | Inline code diffs | 12kb | MIT |
| `@humanspeak/svelte-virtual-list` | Virtual scrolling | 5kb | MIT |

### Block Type System

```typescript
type BlockType =
  | 'chat'      // Normal text (Markdown rendered)
  | 'code'      // Code block with syntax highlighting
  | 'thinking'  // Chain-of-thought (collapsible, dimmed)
  | 'tool'      // Tool call + result (expandable)
  | 'terminal'  // Terminal output (monospace, themed)
  | 'diff'      // Code diff (git-diff-view)
  | 'diagram'   // Mermaid/flowchart
  | 'math'      // LaTeX/KaTeX
  | 'image'     // Generated/attached image
  | 'routing'   // Model routing decision (animated)
  | 'error'     // Error with stack trace
  | 'system'    // System message (dimmed)
```

### Streaming Markdown Pattern

```typescript
let streamBuffer = $state('')
let renderedHtml = $state('')

function onDelta(content: string) {
  streamBuffer += content
  requestAnimationFrame(() => {
    renderedHtml = marked.parse(streamBuffer)
  })
}
```

---

## @-Mention Context Assembly

```
@file path/to/file.rs     → File content as context
@codebase "search term"   → Semantic search in project
@docs "tauri channel"     → Documentation lookup
@terminal                 → Recent terminal outputs
@memory "topic"           → ForgeMemory lookup
@model "devstral"         → Force specific model
@diff                     → Current git diff
@errors                   → Current lint/build errors
```

Typing `@` opens autocomplete popup. Attached contexts shown as chips above input.

---

## New Svelte Components

```
src/lib/components/chat/
├── ChatRenderer.svelte        # Universal markdown renderer (marked.js)
├── ChatInput.svelte           # Input with @-mention autocomplete
├── ChatSidebar.svelte         # Conversations list (shared across modes)
├── ChatMessage.svelte         # Single message with typed blocks
├── ChatStream.svelte          # Unified stream container
├── ModelStatusBadge.svelte    # Minimal viz (status dot + name)
├── ModelActivityCard.svelte   # Animated card per model
├── ModelPipelineView.svelte   # Full DAG visualization (SVG)
├── ThinkingBlock.svelte       # Collapsible chain-of-thought
├── ToolCallBlock.svelte       # Expandable tool execution
├── TerminalBlock.svelte       # Terminal-style output block
├── DiffBlock.svelte           # Git-diff-view wrapper
├── DiagramBlock.svelte        # Mermaid renderer (lazy)
├── MathBlock.svelte           # KaTeX renderer
├── ContextChip.svelte         # @-mention context badge
├── MissionControlPanel.svelte # Dashboard with pipeline + gauges
├── ChatSidePanel.svelte       # Side-panel mode wrapper
├── ChatConvergence.svelte     # Full convergence mode layout
└── ChatModuleRegistry.ts      # BenikUI-style module system
```

### New/Extended Stores

```typescript
// model-status.svelte.ts — NEW
class ModelStatusStore {
  models = $state<ModelState[]>([])
  activeModel = $derived(this.models.find(m => m.status === 'generating'))
  pipeline = $state<PipelineNode[]>([])

  updateFromEvent(event: ChatEvent) { /* reactive updates */ }
}

interface ModelState {
  id: string
  name: string
  status: 'idle' | 'thinking' | 'generating' | 'error'
  currentTask: string | null
  tokensGenerated: number
  tokensTotal: number | null
  latencyMs: number
  routingReason: string | null
  lastActive: Date
}

interface PipelineNode {
  id: string
  type: 'input' | 'classifier' | 'model' | 'memory' | 'output'
  label: string
  status: 'idle' | 'active' | 'completed' | 'error'
  x: number
  y: number
  connections: string[]  // target node IDs
  metrics?: { tokens: number; latencyMs: number }
}

// chat.svelte.ts — EXTENDED
// Add: chatLayout settings, block-type parsing, @-mention state
```

---

## Rust Backend Extensions

### Extended ChatEvent Enum

```rust
pub enum ChatEvent {
    // Existing
    Started { model: String, task_type: String },
    Delta { content: String },
    Finished { total_tokens: u32 },
    Error { message: String },

    // NEW: Pipeline Visualization
    Routing {
        task_type: String,
        selected_model: String,
        reason: String,
        alternatives: Vec<ModelCandidate>,
        classification_ms: f64,
    },
    Thinking { content: String },
    ToolCall { tool: String, args: serde_json::Value },
    ToolResult { tool: String, result: serde_json::Value, duration_ms: f64 },
    ModelStatus { model: String, status: String, tokens: u32 },
    ContextAssembled { sources: Vec<ContextSource> },
}
```

### New Tauri Commands

```rust
#[tauri::command]
pub async fn context_assemble(
    engine: State<'_, ForgeMemoryEngine>,
    mentions: Vec<Mention>,
) -> Result<Vec<ContextSource>, String>

#[tauri::command]
pub async fn get_model_status() -> Result<Vec<ModelState>, String>

#[tauri::command]
pub async fn get_pipeline_state() -> Result<Vec<PipelineNode>, String>
```

---

## Visual Design: "Futuristic Living Room"

### Design Principles
- Dark mode default with glassmorphism accents (frosted glass panels)
- Neon accents: Cyan (AI active), Green (success), Amber (thinking)
- Subtle animations: Pulse for active models, smooth slide-in for messages
- Font mix: JetBrains Mono (code/terminal) + Inter (chat text)
- Breathing effect: Idle pipeline nodes have gentle glow oscillation

### Color Tokens

```css
--forge-ai-active: oklch(0.75 0.18 195);
--forge-ai-thinking: oklch(0.75 0.15 85);
--forge-ai-idle: oklch(0.55 0.05 250);
--forge-stream-bg: oklch(0.15 0.01 250);
--forge-glass: oklch(0.2 0.02 250 / 0.6);
--forge-glass-border: oklch(0.4 0.05 250 / 0.3);
--forge-terminal-bg: oklch(0.1 0.01 150);
--forge-terminal-text: oklch(0.8 0.15 150);
```

---

## Convergence Mode Layout

```
+-----------------------------------------------------------+
| ForgeStudio (Convergence Mode)                      - [] x |
+------------+------------------------+---------------------+
| Explorer   | Editor                 | Chat Stream         |
| +-- src/   | fn main() {           | User: Fix the bug   |
| +-- lib/   |   // AI edit          | Route -> Devstral   |
| +-- test/  |   let x = 42;         | Thinking...         |
|            | }                      | Diff: -0 / +42     |
| Context:   | [Accept] [Reject]      | Applied edit        |
| @file      |                        | cargo test OK       |
|            +------------------------+                     |
| Pipeline:  | Terminal               | [Input with @...]   |
| [DAG view] | $ cargo test           |                     |
|            | 841 tests... OK        |                     |
+------------+------------------------+---------------------+
```

---

## Research References

- arXiv 2410.22370: "Survey of UI Design in Generative AI Applications" (Luera et al.)
- arXiv 2510.07685: "LiveThinking: Real-Time Reasoning" (30x cost reduction)
- arXiv 2601.10253: "Developer Interaction Patterns with Proactive AI" (post-commit timing)
- arXiv 2502.18658: "Codellaborator: Proactive AI Programming Support" (presence indicators)
- arXiv 2601.16596: Mixture-of-Agents pipeline architecture
- arXiv 2602.06039: Task-adaptive multi-agent topology
- Open WebUI: marked.js + highlight.js streaming pattern
- Cursor/Windsurf: Side-panel + @-mention context assembly
- Claude Code: Unified stream with tool-use visualization
- BenikUI/ElvUI: Modular UI dependency tree system
