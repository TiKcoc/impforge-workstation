# Model Routing System

## Design Goals

1. **Zero-Cost Classification** - Task type detection in <10ms without API calls
2. **100% Free Models** - All routes use free-tier or local models
3. **Offline First** - Prefer local Ollama when available
4. **Bilingual** - Support German and English prompts

## Classification Algorithm

The classifier uses keyword-based pattern matching with priority ordering:

```
Priority 1: Explicit markers (Dockerfile, n8n, etc.)
Priority 2: Code indicators (function, class, impl, etc.)
Priority 3: Research markers (paper, arxiv, etc.)
Priority 4: README detection
Priority 5: Multi-step reasoning
Priority 6: Technical questions
Default: General chat
```

## Model Selection Logic

```rust
match task_type {
    CodeGeneration => devstral-small:free,
    GeneralChat => llama-4-scout:free,
    MultiStepReasoning => qwen3-30b-a3b:free,
    ReadmeSummary => local-t5-onnx,
    ImageGeneration => local-sd OR flux:free,
    ...
}
```

## Fallback Strategy

1. Check if OpenRouter key is configured
2. Check if Ollama is available locally
3. Use OpenRouter free tier as last resort

## Adding New Routes

1. Add TaskType variant in `classifier.rs`
2. Add classification logic in `classify_fast()`
3. Add target selection in `targets.rs`
4. Add tests for the new route
