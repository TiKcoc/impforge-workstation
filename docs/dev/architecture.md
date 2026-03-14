# ImpForge Architecture

## Overview

ImpForge follows a modular architecture with clear separation between the frontend (SvelteKit) and backend (Tauri/Rust).

## Intelligent Model Router

The router is the core of ImpForge, providing automatic task classification and model routing.

### Task Classification

```
User Prompt → Fast Classifier (<10ms) → TaskType → Model Selection → Execution
```

### Supported Task Types

| Task Type | Default Model | Cost |
|-----------|---------------|------|
| CodeGeneration | Devstral Small | FREE |
| CodeExplanation | Devstral Small | FREE |
| DockerfileGen | Devstral Small | FREE |
| N8nWorkflowGen | Devstral Small | FREE |
| GeneralChat | Llama 4 Scout | FREE |
| TechQuestion | Llama 4 Scout | FREE |
| ResearchDigest | Llama 4 Scout | FREE |
| MultiStepReasoning | Qwen3-30B | FREE |
| WebAutomation | Devstral Small | FREE |
| ImageGeneration | FLUX / Local SD | FREE |
| ReadmeSummary | Local T5-ONNX | FREE |
| SimpleClassification | Local MiniLM | FREE |

### Model Providers

1. **OpenRouter** - Cloud API with free tier models
2. **Ollama** - Local inference (requires local installation)
3. **Local ONNX** - Lightweight local models for specific tasks

## Agent System

Agents are specialized AI instances with specific roles and system prompts.

### Default Agents

- **Orchestrator** - Coordinates multi-agent tasks
- **Coder** - Code generation and review
- **Debugger** - Bug identification and fixing
- **Researcher** - Information gathering and summarization

## State Management

Using Svelte 5 Runes for reactive state:

- `models.svelte.ts` - Model configuration and routing
- `chat.svelte.ts` - Conversation and message handling
- `settings.svelte.ts` - Persistent settings with Tauri Store

## Integrations

### Docker

- List containers
- Start/Stop/Restart containers
- View logs

### GitHub

- List repositories
- View issues and PRs
- Quick actions

### n8n

- Workflow management
- Execution monitoring
