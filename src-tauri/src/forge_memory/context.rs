//! Context Assembly for AI Prompts — Contextual Retrieval Integration
//!
//! Builds enriched system prompts by combining:
//!   1. Core memories (always in context — MemGPT working memory)
//!   2. Relevant search results for the current query (hybrid HNSW+BM25)
//!   3. Knowledge graph neighborhood for key concepts
//!
//! This implements the Contextual Retrieval pattern (Anthropic 2024):
//! prepending relevant context to AI prompts significantly improves
//! response quality, especially for domain-specific questions.
//!
//! Auto-learning extracts key information from conversations and
//! stores it in the recall tier for future retrieval.
//!
//! Frontend usage (Svelte/TypeScript):
//! ```typescript
//! // Before sending to AI:
//! const context = await invoke('forge_memory_get_context', {
//!   query: userMessage,
//!   maxResults: 5,
//! });
//! // context.system_supplement contains formatted memory context
//! // Prepend to system prompt before calling chat_stream
//!
//! // After AI responds:
//! await invoke('forge_memory_auto_learn', {
//!   userMessage: 'How does Rust ownership work?',
//!   aiResponse: '...',
//!   conversationId: convId,
//! });
//! ```
//!
//! References:
//!   - Anthropic (2024). Contextual Retrieval.
//!     https://www.anthropic.com/news/contextual-retrieval
//!   - Lewis, P. et al. (2020). Retrieval-Augmented Generation for
//!     Knowledge-Intensive NLP Tasks. NeurIPS 2020. arXiv:2005.11401

use serde::{Deserialize, Serialize};

use super::engine::ForgeMemoryEngine;

// ── Context result ───────────────────────────────────────────────

/// Assembled context for enriching AI prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContext {
    /// Formatted text to prepend/append to the system prompt.
    /// Contains core memories + relevant search results.
    pub system_supplement: String,

    /// Number of core memories included.
    pub core_memory_count: usize,

    /// Number of relevant search results included.
    pub relevant_results_count: usize,

    /// IDs of memories that were accessed (for FSRS tracking).
    pub accessed_memory_ids: Vec<String>,
}

/// Result of auto-learning from a conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnResult {
    /// Number of new memories created.
    pub memories_created: usize,
    /// Number of existing memories reinforced (FSRS review).
    pub memories_reinforced: usize,
    /// Categories of extracted information.
    pub categories: Vec<String>,
}

// ── Context Builder ──────────────────────────────────────────────

/// Maximum characters per memory entry in the context supplement.
const MAX_ENTRY_CHARS: usize = 500;

/// Build enriched context for an AI prompt.
///
/// Retrieves core memories (always present) and searches for relevant
/// context based on the user's query. Formats everything into a
/// structured system prompt supplement.
pub fn build_context(
    engine: &ForgeMemoryEngine,
    query: &str,
    max_search_results: usize,
) -> Result<MemoryContext, String> {
    let mut parts = Vec::new();
    let mut accessed_ids = Vec::new();

    // 1. Core memories (MemGPT working memory — always in context)
    let core_memories = engine.get_core_memories()?;
    let core_count = core_memories.len();

    if !core_memories.is_empty() {
        parts.push("## Core Knowledge (Always Active)".to_string());
        for mem in &core_memories {
            let truncated = truncate(&mem.content, MAX_ENTRY_CHARS);
            let cat = &mem.category;
            parts.push(format!("- [{cat}] {truncated}"));
            accessed_ids.push(mem.id.clone());
        }
        parts.push(String::new()); // blank line
    }

    // 2. Relevant context from hybrid search (if query is non-trivial)
    let search_results = if query.len() >= 3 {
        engine.search(query, max_search_results)?
    } else {
        Vec::new()
    };
    let relevant_count = search_results.len();

    if !search_results.is_empty() {
        parts.push("## Relevant Context (Retrieved for This Query)".to_string());
        for result in &search_results {
            // Skip if already in core (avoid duplication)
            if accessed_ids.contains(&result.id) {
                continue;
            }
            let source_label = match result.source.as_str() {
                "memory" => "Memory",
                "knowledge" => "Knowledge",
                _ => "Info",
            };
            let title = result
                .title
                .as_deref()
                .unwrap_or(source_label);
            let truncated = truncate(&result.content, MAX_ENTRY_CHARS);
            let score = format!("{:.2}", result.score);
            parts.push(format!("- **{title}** (relevance: {score}): {truncated}"));
            accessed_ids.push(result.id.clone());
        }
    }

    let system_supplement = if parts.is_empty() {
        String::new()
    } else {
        format!(
            "# Your Memory\n\
             The following information is from your persistent memory.\n\
             Use it to provide more personalized and informed responses.\n\n\
             {}\n",
            parts.join("\n")
        )
    };

    Ok(MemoryContext {
        system_supplement,
        core_memory_count: core_count,
        relevant_results_count: relevant_count,
        accessed_memory_ids: accessed_ids,
    })
}

/// Auto-learn from a conversation turn using the NLP pipeline.
///
/// Upgraded from 22 keyword patterns to 80+ multilingual NLP patterns.
/// Extracts entities, relations, preferences, decisions, and facts,
/// then stores them in the appropriate MemGPT tier with dedup protection.
///
/// Pipeline: classify → filter sensitive → dedup → store → KG extract → reinforce
///
/// Features:
///   - 80+ DE/EN patterns across 9 categories
///   - Trigram + word Jaccard dedup (no duplicate memories)
///   - Sensitive data filter (passwords, tokens NEVER stored)
///   - Auto KG node/edge creation for extracted entities/relations
///   - FSRS-5 reinforcement of relevant existing memories
pub fn auto_learn(
    engine: &ForgeMemoryEngine,
    user_message: &str,
    ai_response: &str,
    conversation_id: Option<&str>,
) -> Result<LearnResult, String> {
    use super::nlp;

    let mut memories_created = 0;
    let mut memories_reinforced = 0;
    let mut categories = Vec::new();

    // 1. Classify message with NLP pipeline (80+ patterns, ~2ms)
    let classification = nlp::classify_message(user_message);

    // 2. Filter sensitive data — NEVER store passwords/tokens/keys
    if nlp::contains_sensitive_data(user_message) {
        // Still persist conversation but don't extract memories
    } else if !classification.categories.is_empty() {
        // 3. Dedup check before storing (avoid near-duplicate memories)
        if !engine.is_duplicate(user_message)? {
            // Determine scope based on importance
            // High importance (explicit notes, corrections) → core
            // Normal importance (preferences, decisions) → recall
            let scope = if classification.importance >= 0.85 {
                "core"
            } else {
                "recall"
            };
            let primary_category = classification
                .categories
                .first()
                .cloned()
                .unwrap_or_else(|| "general".to_string());

            engine.add_memory(
                user_message,
                scope,
                classification.importance,
                &primary_category,
            )?;
            memories_created += 1;
            categories = classification.categories.clone();
        }

        // 4. Create KG nodes/edges for extracted entities and relations
        for entity in &classification.entities {
            let node_kind = match entity.kind {
                nlp::EntityKind::FilePath => "file",
                nlp::EntityKind::PackageName => "crate",
                nlp::EntityKind::Url => "concept",
                nlp::EntityKind::ModelName => "concept",
                nlp::EntityKind::Port => "concept",
                nlp::EntityKind::Email => "concept",
                nlp::EntityKind::IpAddress => "concept",
                nlp::EntityKind::CodeIdentifier => "symbol",
            };
            let _ = engine.kg_add_node(&entity.value, node_kind, &entity.value, None);
        }

        for relation in &classification.relations {
            let edge_kind = match relation.kind {
                nlp::RelationKind::Uses => "references",
                nlp::RelationKind::DependsOn => "depends_on",
                nlp::RelationKind::PrefersOver => "similar",
                nlp::RelationKind::LocatedAt => "contains",
                nlp::RelationKind::VersionIs => "references",
                nlp::RelationKind::RunsOn => "depends_on",
            };
            let _ = engine.kg_add_edge(&relation.subject, &relation.object, edge_kind, 0.8);
        }
    }

    // 5. Persist conversation turn (if conversation_id provided)
    if let Some(conv_id) = conversation_id {
        let _ = engine.save_message(conv_id, "user", user_message, None);
        let response_truncated = truncate(ai_response, 4000);
        let _ = engine.save_message(conv_id, "assistant", &response_truncated, None);
    }

    // 6. Reinforce relevant existing memories (FSRS-5 "good" review)
    // Threshold 0.01: RRF fusion produces small scores (1/(k+rank)),
    // so even strong matches score 0.01–0.1 in small corpora.
    if !user_message.is_empty() && user_message.len() >= 3 {
        let relevant = engine.search(user_message, 3)?;
        for result in &relevant {
            if result.score > 0.01 {
                let _ = engine.review_memory(&result.id, "good");
                memories_reinforced += 1;
            }
        }
    }

    Ok(LearnResult {
        memories_created,
        memories_reinforced,
        categories,
    })
}

/// Truncate text to max chars, adding "..." if truncated.
fn truncate(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        let boundary = text
            .char_indices()
            .take_while(|(i, _)| *i < max_chars - 3)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_chars - 3);
        format!("{}...", &text[..boundary])
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_memory::engine::ForgeMemoryEngine;

    fn test_engine() -> ForgeMemoryEngine {
        ForgeMemoryEngine::open_memory().unwrap()
    }

    #[test]
    fn test_build_context_empty() {
        let engine = test_engine();
        let ctx = build_context(&engine, "test query", 5).unwrap();
        assert_eq!(ctx.core_memory_count, 0);
        assert_eq!(ctx.relevant_results_count, 0);
        assert!(ctx.system_supplement.is_empty());
    }

    #[test]
    fn test_build_context_with_core_memories() {
        let engine = test_engine();

        engine
            .add_memory("User prefers Rust over Python", "core", 0.9, "preference")
            .unwrap();
        engine
            .add_memory("Project uses SQLite for storage", "core", 0.8, "architecture")
            .unwrap();

        let ctx = build_context(&engine, "what language should I use", 5).unwrap();
        assert_eq!(ctx.core_memory_count, 2);
        assert!(ctx.system_supplement.contains("Core Knowledge"));
        assert!(ctx.system_supplement.contains("Rust over Python"));
        assert!(ctx.system_supplement.contains("SQLite"));
    }

    #[test]
    fn test_build_context_with_search_results() {
        let engine = test_engine();

        // Add to recall (not core), so it's found via search
        engine
            .add_memory(
                "HNSW uses hierarchical graphs for approximate nearest neighbor search",
                "recall",
                0.5,
                "algorithms",
            )
            .unwrap();

        let ctx = build_context(&engine, "HNSW vector search", 5).unwrap();
        // Should find the recall memory via BM25
        assert!(ctx.relevant_results_count > 0 || ctx.core_memory_count > 0);
    }

    #[test]
    fn test_auto_learn_preference() {
        let engine = test_engine();

        let result = auto_learn(
            &engine,
            "I prefer dark mode for all my editors",
            "I'll remember that you prefer dark mode!",
            None,
        )
        .unwrap();

        assert!(result.memories_created >= 1);
        assert!(result.categories.contains(&"preference".to_string()));

        // Verify the preference was stored
        let search = engine.search("dark mode editor", 5).unwrap();
        assert!(!search.is_empty());
    }

    #[test]
    fn test_auto_learn_decision() {
        let engine = test_engine();

        let result = auto_learn(
            &engine,
            "Let's use PostgreSQL for the database backend",
            "Good choice! PostgreSQL is excellent for...",
            None,
        )
        .unwrap();

        assert!(result.memories_created >= 1);
        assert!(result.categories.contains(&"decision".to_string()));
    }

    #[test]
    fn test_auto_learn_explicit_note() {
        let engine = test_engine();

        let result = auto_learn(
            &engine,
            "Remember that the API key expires on March 15th",
            "Noted, I'll keep that in mind.",
            None,
        )
        .unwrap();

        assert!(result.memories_created >= 1);
        assert!(result.categories.contains(&"explicit_note".to_string()));
    }

    #[test]
    fn test_auto_learn_with_conversation() {
        let engine = test_engine();

        let conv_id = engine
            .create_conversation(Some("Test"), None)
            .unwrap();

        let result = auto_learn(
            &engine,
            "Hello, how are you?",
            "I'm doing well, thanks!",
            Some(&conv_id),
        )
        .unwrap();

        // Messages should be persisted
        let messages = engine.get_messages(&conv_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        // No special patterns, so no new memories
        assert_eq!(result.memories_created, 0);
    }

    #[test]
    fn test_auto_learn_reinforcement() {
        let engine = test_engine();

        // Pre-populate a memory with distinctive content
        engine
            .add_memory("Rust ownership model prevents data races", "recall", 0.5, "programming")
            .unwrap();

        // Use a query with high keyword overlap to ensure BM25 match
        let result = auto_learn(
            &engine,
            "Rust ownership model data races prevention",
            "Rust's ownership system...",
            None,
        )
        .unwrap();

        // Should have reinforced the existing memory (BM25 score > 0.3)
        // The query shares 4 key terms with the stored memory
        assert!(
            result.memories_reinforced > 0,
            "Expected reinforcement, got: created={}, reinforced={}",
            result.memories_created,
            result.memories_reinforced,
        );
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 100), "short");
        assert_eq!(truncate("hello world", 8), "hello...");

        // Unicode safety
        let unicode = "Hëllö Wörld";
        let truncated = truncate(unicode, 8);
        assert!(truncated.len() <= 11); // 8 chars + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_build_context_short_query() {
        let engine = test_engine();
        engine
            .add_memory("Core fact", "core", 0.9, "test")
            .unwrap();

        // Very short query (< 3 chars) skips search
        let ctx = build_context(&engine, "hi", 5).unwrap();
        assert_eq!(ctx.core_memory_count, 1);
        assert_eq!(ctx.relevant_results_count, 0);
    }
}
