# ChatTerminalBrowserUI Completion Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the 3x3x3 modular chat system with 6 missing components to make ImpForge's AI interface production-ready and futuristic.

**Architecture:** Block-based message rendering with AG-UI event streaming, virtual scrolling via @humanspeak/svelte-virtual-list, dark glassmorphism design tokens, and BenikUI style engine integration throughout.

**Tech Stack:** Svelte 5 (runes), Tauri 2 (Rust SSE channels), SQLite (message persistence), D3.js (DAG views)

**Research Basis:** 28 sources documented in `/opt/ork-station/docs/research/2026-03-09-ai-chat-ui-research.md`

---

## 6 Identified Gaps (from Section 1)

### Gap 1: Block-Type Message Renderer (P0 - Highest Impact)
- **What**: Rich message blocks beyond plain text bubbles
- **Blocks**: TextBlock, CodeBlock, ToolCallBlock, DiffBlock, ReasoningBlock, ErrorBlock, FileRefBlock, ProgressBlock, DiagramBlock, MathBlock
- **Research**: AG-UI protocol events (Section 5.1), Assistant-UI patterns (Section 5.3)
- **Location**: `src/lib/components/chat/blocks/`

### Gap 2: Enhanced ChatInput (P0 - Killer Feature)
- **What**: @-mention autocomplete, context chips, token counter, model selector inline
- **Research**: Cursor Composer (Section 4.2), Devin 2.0 multi-file context (Section 4.1)
- **Location**: `src/lib/components/chat/ChatInput.svelte`

### Gap 3: Split Stream Mode (P1 - Completes 3x3x3)
- **What**: Functional split-stream layout (chat left, live output right)
- **Research**: "Parallel Streams" pattern (Section 5.6), CSCW 2025 transparency
- **Location**: `src/lib/components/chat/layouts/SplitStreamLayout.svelte`

### Gap 4: Mission Control Panel (P1 - Futuristic Dashboard)
- **What**: Permanent dashboard with model gauges, routing history, pipeline DAG, cost ticker
- **Research**: Agentic Visualization 11 patterns (Section 2.1), FlowForge 3-level hierarchy (Section 2.2), LangGraph Studio (Section 2.4)
- **Location**: `src/lib/components/chat/MissionControl.svelte`

### Gap 5: Module Registry (P2 - Infrastructure)
- **What**: Central registry enabling 3x3x3 dynamic composition
- **Location**: `src/lib/stores/module-registry.svelte.ts`

### Gap 6: TokenBudgetBar (P2 - Quick Win)
- **What**: Context window visualization in all placement modes
- **Research**: Token budget visualization (Section 3), color gradient green->yellow->orange->red
- **Location**: `src/lib/components/chat/TokenBudgetBar.svelte`

## Data Model (from Research Section 8)

```typescript
interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  blocks: MessageBlock[];
  model?: string;
  tokens?: { input: number; output: number; cached: number };
  cost?: number;
  timestamp: number;
  status: 'streaming' | 'complete' | 'error';
}

type MessageBlock =
  | { type: 'text'; content: string }
  | { type: 'code'; language: string; content: string }
  | { type: 'diff'; hunks: DiffHunk[] }
  | { type: 'tool_call'; name: string; args: any; result?: any; status: 'pending' | 'running' | 'complete' | 'error' }
  | { type: 'reasoning'; content: string; collapsed: boolean }
  | { type: 'error'; message: string; suggestion?: string }
  | { type: 'file_ref'; path: string; preview?: string }
  | { type: 'image'; url: string; alt?: string }
  | { type: 'progress'; label: string; percent: number };
```

## Virtual Scrolling

Primary: `@humanspeak/svelte-virtual-list` — Svelte 5 runes, bottomToTop mode, 10k+ items, programmatic scroll API.

## Design System

Dark glassmorphism with BenikUI customization. Per-model brand colors:
- Claude: #a855f7
- Qwen: #06b6d4
- Hermes: #f59e0b
- Local: #22c55e
- GPT: #10b981
- Gemini: #3b82f6

## Status

**Design approved** — Ready for implementation planning.
**Date**: 2026-03-09
