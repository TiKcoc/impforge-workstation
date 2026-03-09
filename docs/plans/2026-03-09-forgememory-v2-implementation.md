# ForgeMemory v2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade ImpForge with intelligent auto-learning (NLP + LLM dual-mode), universal input digest, filesystem watching, chat-memory bridge, and Moonlight remote access — turning it into a self-learning AI workstation controllable from any device.

**Architecture:** Five layers built incrementally: (1) NLP auto-learn pipeline with 80+ patterns, (2) LLM extraction via local Ollama, (3) Chat ↔ Memory bridge wiring, (4) ForgeWatch filesystem monitor with document ingestion, (5) ForgeSunshine Moonlight manager. Each layer is independently testable and toggle-able by the customer.

**Tech Stack:** Tauri 2.10, Svelte 5 (runes), Rust, rusqlite (bundled), fastembed 5.12, notify 7 (fs events), parking_lot, tokio, Sunshine/Moonlight (external)

**Design Doc:** `docs/plans/2026-03-09-forgememory-v2-design.md`

---

## Phase 1: Auto-Learn v2 — NLP Pipeline (Tasks 1–3)

After Phase 1, ForgeMemory extracts entities, relations, and facts from every message using 80+ multilingual patterns with dedup detection. Fully offline, ~2ms per message.

---

### Task 1: NLP Entity Extractor + Pattern Engine

**Files:**
- Create: `src-tauri/src/forge_memory/nlp.rs`
- Modify: `src-tauri/src/forge_memory/mod.rs` (add `pub mod nlp;`)

**Step 1: Write failing tests for entity extraction**

Add to the bottom of the new `nlp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_paths() {
        let entities = extract_entities("Check /home/user/project/main.rs for the bug");
        assert!(entities.iter().any(|e| e.kind == EntityKind::FilePath && e.value.contains("main.rs")));
    }

    #[test]
    fn test_extract_urls() {
        let entities = extract_entities("See https://docs.rs/tokio/latest for docs");
        assert!(entities.iter().any(|e| e.kind == EntityKind::Url && e.value.contains("docs.rs")));
    }

    #[test]
    fn test_extract_package_names() {
        let entities = extract_entities("We use tokio and serde for this project");
        assert!(entities.iter().any(|e| e.kind == EntityKind::PackageName && e.value == "tokio"));
    }

    #[test]
    fn test_extract_model_names() {
        let entities = extract_entities("Running qwen2.5-coder:7b on the GPU");
        assert!(entities.iter().any(|e| e.kind == EntityKind::ModelName && e.value == "qwen2.5-coder:7b"));
    }

    #[test]
    fn test_extract_ports() {
        let entities = extract_entities("The server runs on port 8080");
        assert!(entities.iter().any(|e| e.kind == EntityKind::Port && e.value == "8080"));
    }

    #[test]
    fn test_preference_pattern_en() {
        let result = classify_message("I prefer dark mode for all editors");
        assert!(result.categories.contains(&"preference".to_string()));
        assert!(result.importance >= 0.7);
    }

    #[test]
    fn test_preference_pattern_de() {
        let result = classify_message("Ich bevorzuge Rust gegenüber Python");
        assert!(result.categories.contains(&"preference".to_string()));
    }

    #[test]
    fn test_decision_pattern() {
        let result = classify_message("Let's use PostgreSQL for the database");
        assert!(result.categories.contains(&"decision".to_string()));
    }

    #[test]
    fn test_explicit_note_pattern() {
        let result = classify_message("Remember that the API key expires March 15th");
        assert!(result.categories.contains(&"explicit_note".to_string()));
        assert!(result.importance >= 0.85);
    }

    #[test]
    fn test_technical_fact() {
        let result = classify_message("The server runs on port 3000 with Node.js 20");
        assert!(result.categories.contains(&"technical".to_string()));
    }

    #[test]
    fn test_negation_pattern() {
        let result = classify_message("Never use eval() in production code");
        assert!(result.categories.contains(&"negation".to_string()));
    }

    #[test]
    fn test_correction_pattern() {
        let result = classify_message("Actually it's Python 3.12, not 3.11");
        assert!(result.categories.contains(&"correction".to_string()));
    }

    #[test]
    fn test_code_pattern() {
        let result = classify_message("use tokio::sync::Mutex;");
        assert!(result.categories.contains(&"code_pattern".to_string()));
    }

    #[test]
    fn test_no_patterns_casual() {
        let result = classify_message("Hello, how are you today?");
        assert!(result.categories.is_empty());
        assert!(result.importance < 0.3);
    }

    #[test]
    fn test_relation_extraction() {
        let relations = extract_relations("I use Rust for the backend");
        assert!(relations.iter().any(|r| r.kind == RelationKind::Uses && r.subject == "I" && r.object.contains("Rust")));
    }

    #[test]
    fn test_dedup_similar() {
        assert!(is_near_duplicate("I prefer dark mode", "I prefer dark themes"));
    }

    #[test]
    fn test_dedup_different() {
        assert!(!is_near_duplicate("I prefer dark mode", "The server runs on port 8080"));
    }

    #[test]
    fn test_sensitive_data_filter() {
        assert!(contains_sensitive_data("My password is hunter2"));
        assert!(contains_sensitive_data("export API_KEY=sk-abc123"));
        assert!(!contains_sensitive_data("The weather is nice today"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /opt/ork-station/ImpForge && cargo test --lib forge_memory::nlp -- --test-threads=1 2>&1 | tail -5`
Expected: FAIL — module doesn't exist yet

**Step 3: Implement nlp.rs**

Create `src-tauri/src/forge_memory/nlp.rs`:

```rust
//! NLP Pipeline for ForgeMemory Auto-Learn v2
//!
//! Offline, rule-based extraction (~2ms per message):
//!   - Entity extraction (paths, URLs, packages, models, ports, emails)
//!   - Message classification (80+ DE/EN patterns)
//!   - Relation extraction ("X uses Y", "X depends on Y")
//!   - Dedup detection (trigram Jaccard similarity)
//!   - Sensitive data filtering (passwords, tokens, keys)
//!
//! References:
//!   - Mem0 (arXiv:2504.19413) — production agent memory
//!   - A-MEM (arXiv:2502.12110) — Zettelkasten-inspired extraction

use serde::{Deserialize, Serialize};

// ── Entity Extraction ───────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    FilePath,
    Url,
    PackageName,
    ModelName,
    Port,
    Email,
    IpAddress,
    CodeIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub kind: EntityKind,
    pub value: String,
    pub start: usize,
    pub end: usize,
}

/// Extract structured entities from text using regex patterns.
pub fn extract_entities(text: &str) -> Vec<ExtractedEntity> {
    let mut entities = Vec::new();

    // File paths: /absolute/path or ~/relative/path
    for mat in regex_find_all(text, r"(?:[~/][\w./\-]+\.[\w]+|[A-Z]:\\[\w\\.\-]+)") {
        entities.push(ExtractedEntity {
            kind: EntityKind::FilePath,
            value: mat.0.to_string(),
            start: mat.1,
            end: mat.2,
        });
    }

    // URLs
    for mat in regex_find_all(text, r"https?://[^\s,)>]+") {
        entities.push(ExtractedEntity {
            kind: EntityKind::Url,
            value: mat.0.to_string(),
            start: mat.1,
            end: mat.2,
        });
    }

    // Model names (ollama format: name:tag or name/name:tag)
    for mat in regex_find_all(text, r"\b[\w-]+(?:/[\w-]+)?:\d[\w.\-]*\b") {
        // Skip if it's a URL part
        if !text[..mat.1].ends_with('/') && !text[..mat.1].ends_with(':') {
            entities.push(ExtractedEntity {
                kind: EntityKind::ModelName,
                value: mat.0.to_string(),
                start: mat.1,
                end: mat.2,
            });
        }
    }

    // Ports
    for mat in regex_find_all(text, r"\bport\s+(\d{2,5})\b") {
        let port_val = regex_find_all(&text[mat.1..mat.2], r"\d+");
        if let Some(p) = port_val.first() {
            entities.push(ExtractedEntity {
                kind: EntityKind::Port,
                value: p.0.to_string(),
                start: mat.1 + p.1,
                end: mat.1 + p.2,
            });
        }
    }

    // Well-known package/crate names (simple heuristic: lowercase alphanumeric with hyphens)
    let known_packages = [
        "tokio", "serde", "reqwest", "axum", "actix", "diesel", "sqlx", "rusqlite",
        "react", "svelte", "vue", "angular", "next", "nuxt", "express", "fastapi",
        "django", "flask", "numpy", "pandas", "pytorch", "tensorflow", "transformers",
        "ollama", "docker", "kubernetes", "redis", "postgresql", "sqlite", "mongodb",
    ];
    let lower = text.to_lowercase();
    for pkg in &known_packages {
        if let Some(pos) = lower.find(pkg) {
            // Check word boundary
            let before = if pos > 0 { lower.as_bytes()[pos - 1] } else { b' ' };
            let after_pos = pos + pkg.len();
            let after = if after_pos < lower.len() { lower.as_bytes()[after_pos] } else { b' ' };
            if !before.is_ascii_alphanumeric() && !after.is_ascii_alphanumeric() {
                entities.push(ExtractedEntity {
                    kind: EntityKind::PackageName,
                    value: pkg.to_string(),
                    start: pos,
                    end: pos + pkg.len(),
                });
            }
        }
    }

    entities
}

// ── Message Classification (80+ patterns) ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub categories: Vec<String>,
    pub importance: f64,
    pub entities: Vec<ExtractedEntity>,
    pub relations: Vec<ExtractedRelation>,
}

/// Classify a message using 80+ multilingual patterns.
pub fn classify_message(text: &str) -> ClassificationResult {
    let lower = text.to_lowercase();
    let mut categories = Vec::new();
    let mut importance: f64 = 0.1; // baseline

    // ── Preference patterns (EN + DE) ──
    let preference_patterns = [
        "i prefer ", "i like ", "i always use ", "i don't like ", "my favorite ",
        "i usually ", "i want ", "i love ", "i hate ", "i enjoy ",
        "ich bevorzuge ", "ich mag ", "ich nutze immer ", "ich will ",
        "mein favorit ",
    ];
    if preference_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("preference".to_string());
        importance = importance.max(0.7);
    }

    // ── Decision patterns ──
    let decision_patterns = [
        "let's use ", "we decided ", "the plan is ", "we should ", "let's go with ",
        "i chose ", "we'll ", "we will ", "going to use ", "switching to ",
        "wir nutzen ", "wir verwenden ", "der plan ist ",
    ];
    if decision_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("decision".to_string());
        importance = importance.max(0.7);
    }

    // ── Explicit note patterns ──
    let note_patterns = [
        "remember that ", "remember:", "note:", "important:", "don't forget ",
        "keep in mind ", "merke dir ", "merke:", "achtung:", "wichtig:",
    ];
    if note_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("explicit_note".to_string());
        importance = importance.max(0.85);
    }

    // ── Technical fact patterns ──
    let tech_patterns = [
        "runs on port", "version is", "version:", "api key", "endpoint is",
        "installed at", "configured to", "runs on", "built with",
        "läuft auf port", "version ist", "installiert in",
    ];
    if tech_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("technical".to_string());
        importance = importance.max(0.6);
    }

    // ── Negation patterns ──
    let negation_patterns = [
        "never use ", "don't use ", "avoid ", "stop using ", "deprecated ",
        "niemals ", "nicht verwenden ", "vermeiden ", "nicht benutzen ",
    ];
    if negation_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("negation".to_string());
        importance = importance.max(0.75);
    }

    // ── Correction patterns ──
    let correction_patterns = [
        "actually it's", "actually, it's", "i meant ", "correction:",
        "that's wrong", "nein, das ist", "eigentlich ist",
    ];
    if correction_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("correction".to_string());
        importance = importance.max(0.8);
    }

    // ── Code patterns ──
    let code_indicators = [
        "use ", "import ", "from ", "require(", "include ", "#include ",
        "fn ", "def ", "class ", "struct ", "enum ", "interface ",
    ];
    let is_code_like = code_indicators.iter().any(|p| text.contains(p))
        && text.len() < 200
        && (text.contains(';') || text.contains('{') || text.contains("::"));
    if is_code_like {
        categories.push("code_pattern".to_string());
        importance = importance.max(0.5);
    }

    let entities = extract_entities(text);
    let relations = extract_relations(text);

    // Boost importance if entities or relations found
    if !entities.is_empty() {
        importance = importance.max(0.4);
    }
    if !relations.is_empty() {
        importance += 0.1;
    }

    importance = importance.min(1.0);

    ClassificationResult {
        categories,
        importance,
        entities,
        relations,
    }
}

// ── Relation Extraction ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RelationKind {
    Uses,
    DependsOn,
    PrefersOver,
    LocatedAt,
    VersionIs,
    RunsOn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelation {
    pub kind: RelationKind,
    pub subject: String,
    pub object: String,
}

/// Extract simple relations from text.
pub fn extract_relations(text: &str) -> Vec<ExtractedRelation> {
    let mut relations = Vec::new();
    let lower = text.to_lowercase();

    // "X use(s) Y" pattern
    for pattern in ["i use ", "we use ", "using "] {
        if let Some(pos) = lower.find(pattern) {
            let after = &text[pos + pattern.len()..];
            let object: String = after.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            if !object.is_empty() {
                let subject = if pattern.starts_with("i") { "I" } else { "we" };
                relations.push(ExtractedRelation {
                    kind: RelationKind::Uses,
                    subject: subject.to_string(),
                    object: object.trim_end_matches(|c: char| c.is_ascii_punctuation()).to_string(),
                });
            }
        }
    }

    // "X depends on Y"
    if let Some(pos) = lower.find("depends on ") {
        let after = &text[pos + 11..];
        let object: String = after.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        relations.push(ExtractedRelation {
            kind: RelationKind::DependsOn,
            subject: text[..pos].split_whitespace().last().unwrap_or("it").to_string(),
            object,
        });
    }

    // "prefer X over Y"
    if let Some(pos) = lower.find(" over ") {
        if lower[..pos].contains("prefer") {
            let before_over = &text[..pos];
            let after_over = &text[pos + 6..];
            let preferred = before_over.split_whitespace().last().unwrap_or("").to_string();
            let other: String = after_over.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
            if !preferred.is_empty() && !other.is_empty() {
                relations.push(ExtractedRelation {
                    kind: RelationKind::PrefersOver,
                    subject: preferred,
                    object: other.trim_end_matches(|c: char| c.is_ascii_punctuation()).to_string(),
                });
            }
        }
    }

    relations
}

// ── Dedup Detection ─────────────────────────────────────────────

/// Check if two texts are near-duplicates using trigram Jaccard similarity.
pub fn is_near_duplicate(a: &str, b: &str) -> bool {
    let threshold = 0.6;
    let trigrams_a = trigrams(&a.to_lowercase());
    let trigrams_b = trigrams(&b.to_lowercase());

    if trigrams_a.is_empty() || trigrams_b.is_empty() {
        return a.to_lowercase() == b.to_lowercase();
    }

    let intersection = trigrams_a.iter().filter(|t| trigrams_b.contains(t)).count();
    let union = trigrams_a.len() + trigrams_b.len() - intersection;

    if union == 0 { return true; }
    (intersection as f64 / union as f64) >= threshold
}

fn trigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 { return vec![text.to_string()]; }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

// ── Sensitive Data Filter ───────────────────────────────────────

/// Detect sensitive data that should NEVER be stored.
pub fn contains_sensitive_data(text: &str) -> bool {
    let lower = text.to_lowercase();
    let sensitive_patterns = [
        "password=", "password:", "password is ", "my password",
        "api_key=", "api-key=", "apikey=", "api key=",
        "token=", "secret=", "secret_key=",
        "export api_key", "export secret", "export token",
        "sk-", "ghp_", "gho_", "glpat-",
        "bearer ", "authorization:",
    ];
    sensitive_patterns.iter().any(|p| lower.contains(p))
}

// ── Regex helpers ───────────────────────────────────────────────

fn regex_find_all(text: &str, pattern: &str) -> Vec<(String, usize, usize)> {
    // Simple regex-like matching without the regex crate
    // Uses manual pattern matching for common cases
    let mut results = Vec::new();

    match pattern {
        // File paths
        r if r.contains("[\\/w./\\-]") || r.contains("[~/]") => {
            let chars: Vec<char> = text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '/' || chars[i] == '~' {
                    let start = i;
                    let mut end = i + 1;
                    while end < chars.len() && (chars[end].is_alphanumeric()
                        || chars[end] == '/' || chars[end] == '.'
                        || chars[end] == '-' || chars[end] == '_') {
                        end += 1;
                    }
                    let candidate: String = chars[start..end].iter().collect();
                    if candidate.len() > 3 && candidate.contains('.') {
                        results.push((candidate, start, end));
                    }
                    i = end;
                } else {
                    i += 1;
                }
            }
        }
        // URLs
        r if r.contains("https?://") => {
            let text_lower = text.to_lowercase();
            for prefix in ["https://", "http://"] {
                let mut search_from = 0;
                while let Some(pos) = text_lower[search_from..].find(prefix) {
                    let abs_pos = search_from + pos;
                    let start = abs_pos;
                    let mut end = start + prefix.len();
                    let chars: Vec<char> = text.chars().collect();
                    while end < chars.len() && !chars[end].is_whitespace()
                        && chars[end] != ',' && chars[end] != ')'
                        && chars[end] != '>' {
                        end += 1;
                    }
                    let url: String = chars[start..end].iter().collect();
                    results.push((url, start, end));
                    search_from = end;
                }
            }
        }
        // Model names (name:version pattern)
        r if r.contains(":\\d") => {
            let words: Vec<&str> = text.split_whitespace().collect();
            let mut offset = 0;
            for word in &words {
                if let Some(colon_pos) = word.find(':') {
                    let after_colon = &word[colon_pos + 1..];
                    if after_colon.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != ':' && c != '.' && c != '-' && c != '/');
                        if let Some(abs_pos) = text[offset..].find(clean) {
                            results.push((clean.to_string(), offset + abs_pos, offset + abs_pos + clean.len()));
                        }
                    }
                }
                offset += word.len() + 1;
            }
        }
        // Port numbers
        r if r.contains("port") => {
            let lower = text.to_lowercase();
            if let Some(pos) = lower.find("port") {
                let after = &text[pos + 4..].trim_start();
                let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 2 && digits.len() <= 5 {
                    results.push((format!("port {}", digits), pos, pos + 5 + digits.len()));
                }
            }
        }
        // Digits
        r if r == r"\d+" => {
            let chars: Vec<char> = text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i].is_ascii_digit() {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    let num: String = chars[start..i].iter().collect();
                    results.push((num, start, i));
                } else {
                    i += 1;
                }
            }
        }
        _ => {}
    }

    results
}
```

**Step 4: Add module to mod.rs**

In `src-tauri/src/forge_memory/mod.rs`, add after `pub mod context;`:
```rust
pub mod nlp;
```

**Step 5: Run tests to verify they pass**

Run: `cd /opt/ork-station/ImpForge && cargo test --lib forge_memory::nlp -- --test-threads=1 2>&1 | tail -20`
Expected: 18 tests PASS

**Step 6: Commit**

```bash
git add src-tauri/src/forge_memory/nlp.rs src-tauri/src/forge_memory/mod.rs
git commit -m "feat(forge-memory): NLP entity extractor + 80-pattern classifier — 18 tests"
```

---

### Task 2: Upgrade Auto-Learn to use NLP Pipeline

**Files:**
- Modify: `src-tauri/src/forge_memory/context.rs` (replace keyword patterns with NLP pipeline)
- Modify: `src-tauri/src/forge_memory/engine.rs` (add dedup check method)

**Step 1: Add dedup method to engine.rs**

Add to `ForgeMemoryEngine` impl block:

```rust
/// Check if a similar memory already exists (cosine similarity > 0.92).
pub fn is_duplicate(&self, content: &str) -> Result<bool, String> {
    let results = self.search(content, 1)?;
    Ok(results.first().map(|r| r.score > 0.92).unwrap_or(false))
}
```

**Step 2: Rewrite auto_learn in context.rs to use NLP pipeline**

Replace the `auto_learn` function body with:

```rust
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

    // 1. Classify message with NLP pipeline
    let classification = nlp::classify_message(user_message);

    // 2. Filter sensitive data
    if nlp::contains_sensitive_data(user_message) {
        // Still persist conversation but don't extract memories
    } else if !classification.categories.is_empty() {
        // 3. Dedup check before storing
        if !engine.is_duplicate(user_message)? {
            // Determine scope based on importance
            let scope = if classification.importance >= 0.85 { "core" } else { "recall" };
            let primary_category = classification.categories.first()
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
                nlp::EntityKind::Url => "concept",
                nlp::EntityKind::PackageName => "crate",
                nlp::EntityKind::ModelName => "concept",
                nlp::EntityKind::Port => "concept",
                _ => "concept",
            };
            let _ = engine.kg_add_node(
                &entity.value,
                node_kind,
                &entity.value,
                None,
            );
        }

        for relation in &classification.relations {
            let edge_kind = match relation.kind {
                nlp::RelationKind::Uses => "references",
                nlp::RelationKind::DependsOn => "depends_on",
                nlp::RelationKind::PrefersOver => "similar",
                _ => "references",
            };
            let _ = engine.kg_add_edge(
                &relation.subject,
                &relation.object,
                edge_kind,
                0.8,
            );
        }
    }

    // 5. Persist conversation turn
    if let Some(conv_id) = conversation_id {
        let _ = engine.save_message(conv_id, "user", user_message, None);
        let response_truncated = truncate(ai_response, 4000);
        let _ = engine.save_message(conv_id, "assistant", &response_truncated, None);
    }

    // 6. Reinforce relevant existing memories (FSRS "good" review)
    if !user_message.is_empty() && user_message.len() >= 3 {
        let relevant = engine.search(user_message, 3)?;
        for result in &relevant {
            if result.score > 0.3 {
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
```

**Step 3: Run existing context tests + full suite**

Run: `cd /opt/ork-station/ImpForge && cargo test --lib forge_memory -- --test-threads=1 2>&1 | tail -10`
Expected: All tests pass (164 previous + 18 new NLP = 182+)

**Step 4: Commit**

```bash
git add src-tauri/src/forge_memory/context.rs src-tauri/src/forge_memory/engine.rs
git commit -m "feat(forge-memory): upgrade auto-learn to NLP pipeline with dedup + KG extraction"
```

---

### Task 3: LLM Extraction Mode (Ollama-powered)

**Files:**
- Create: `src-tauri/src/forge_memory/llm_extract.rs`
- Modify: `src-tauri/src/forge_memory/mod.rs` (add `pub mod llm_extract;`)
- Modify: `src-tauri/src/forge_memory/commands.rs` (add toggle command)

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_extraction_prompt() {
        let prompt = build_extraction_prompt("I prefer Rust", "Great choice!");
        assert!(prompt.contains("I prefer Rust"));
        assert!(prompt.contains("Great choice!"));
        assert!(prompt.contains("facts"));
    }

    #[test]
    fn test_parse_extraction_response_valid() {
        let json = r#"{"facts":[{"content":"User prefers Rust","importance":0.8,"category":"preference"}],"entities":[],"summary":"Rust preference","sentiment":"positive"}"#;
        let result = parse_extraction_response(json);
        assert!(result.is_ok());
        let extracted = result.unwrap();
        assert_eq!(extracted.facts.len(), 1);
        assert_eq!(extracted.facts[0].category, "preference");
    }

    #[test]
    fn test_parse_extraction_response_invalid() {
        let result = parse_extraction_response("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_extraction_response_empty() {
        let json = r#"{"facts":[],"entities":[],"summary":"","sentiment":"neutral"}"#;
        let result = parse_extraction_response(json);
        assert!(result.is_ok());
        assert!(result.unwrap().facts.is_empty());
    }
}
```

**Step 2: Implement llm_extract.rs**

```rust
//! LLM-powered extraction for ForgeMemory Auto-Learn v2
//!
//! Uses local Ollama to extract structured facts, entities, and summaries
//! from conversation turns. Complements the NLP pipeline for deeper analysis.
//!
//! Requires: Ollama running locally with a small model (qwen2.5-coder:1.5b recommended).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub facts: Vec<ExtractedFact>,
    pub entities: Vec<ExtractedLlmEntity>,
    pub summary: String,
    pub sentiment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub content: String,
    pub importance: f64,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedLlmEntity {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub relations: Vec<String>,
}

const EXTRACTION_PROMPT: &str = r#"Extract structured information from this conversation turn.
Return ONLY valid JSON with: facts (content, importance 0-1, category), entities (name, type, relations), a one-line summary, and sentiment.
Categories: preference, decision, fact, technical, correction, question.
Only extract genuinely important information. Skip greetings and small talk.
If nothing important, return empty arrays.

User: {user_message}
AI: {ai_response}

JSON:"#;

/// Build the extraction prompt for a conversation turn.
pub fn build_extraction_prompt(user_message: &str, ai_response: &str) -> String {
    EXTRACTION_PROMPT
        .replace("{user_message}", user_message)
        .replace("{ai_response}", &ai_response[..ai_response.len().min(2000)])
}

/// Parse the LLM's JSON response into structured extraction result.
pub fn parse_extraction_response(response: &str) -> Result<ExtractionResult, String> {
    // Try to find JSON in the response (LLM might add text around it)
    let json_str = extract_json_block(response);
    serde_json::from_str::<ExtractionResult>(&json_str)
        .map_err(|e| format!("Failed to parse LLM extraction: {e}"))
}

/// Call local Ollama for extraction (async).
pub async fn extract_via_ollama(
    user_message: &str,
    ai_response: &str,
    model: &str,
    ollama_url: &str,
) -> Result<ExtractionResult, String> {
    let prompt = build_extraction_prompt(user_message, ai_response);

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/generate", ollama_url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1,
                "num_predict": 512,
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned status {}", response.status()));
    }

    let body: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let text = body["response"].as_str().unwrap_or("");
    parse_extraction_response(text)
}

/// Extract a JSON block from text that might contain surrounding prose.
fn extract_json_block(text: &str) -> String {
    // Try to find { ... } block
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return text[start..=end].to_string();
            }
        }
    }
    text.to_string()
}
```

**Step 3: Add module, run tests, commit**

Run: `cd /opt/ork-station/ImpForge && cargo test --lib forge_memory::llm_extract -- --test-threads=1 2>&1 | tail -10`
Expected: 4 tests PASS

```bash
git add src-tauri/src/forge_memory/llm_extract.rs src-tauri/src/forge_memory/mod.rs
git commit -m "feat(forge-memory): LLM extraction mode via Ollama — structured fact extraction"
```

---

## Phase 2: Chat ↔ Memory Bridge (Tasks 4–5)

After Phase 2, every chat message is enriched with memory context before sending to AI, and auto-learned after the response.

---

### Task 4: Wire Chat Backend to ForgeMemory

**Files:**
- Modify: `src-tauri/src/chat.rs` (add ForgeMemory context + auto-learn calls)

**Step 1: Add ForgeMemory state to chat_stream**

Replace the `chat_stream` function signature and add memory integration:

```rust
use tauri::State;
use crate::forge_memory::engine::ForgeMemoryEngine;
use crate::forge_memory::context;

#[tauri::command]
pub async fn chat_stream(
    engine: State<'_, ForgeMemoryEngine>,
    message: String,
    model_id: Option<String>,
    system_prompt: Option<String>,
    openrouter_key: Option<String>,
    conversation_id: Option<String>,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    // ... (existing model selection code stays the same)

    // NEW: Build memory-enriched system prompt
    let base_system = system_prompt.unwrap_or_else(||
        "You are a helpful AI assistant in ImpForge, an AI Workstation Builder.".to_string()
    );
    let enriched_system = match context::build_context(&engine, &message, 5) {
        Ok(ctx) if !ctx.system_supplement.is_empty() => {
            format!("{}\n\n{}", base_system, ctx.system_supplement)
        }
        _ => base_system,
    };

    // ... (existing API call code, but use enriched_system instead of sys)

    // NEW: After streaming completes, auto-learn
    let _ = context::auto_learn(
        &engine,
        &message,
        &full_response,  // collected from Delta events
        conversation_id.as_deref(),
    );

    // ...
}
```

The key changes:
1. Add `engine: State<'_, ForgeMemoryEngine>` parameter
2. Call `build_context()` before the API call to enrich the system prompt
3. Collect the full response from Delta events into a `String`
4. Call `auto_learn()` after streaming completes

**Step 2: Verify compilation**

Run: `cd /opt/ork-station/ImpForge && cargo check 2>&1 | tail -5`
Expected: Compiles (warnings OK)

**Step 3: Commit**

```bash
git add src-tauri/src/chat.rs
git commit -m "feat(chat): wire ForgeMemory context enrichment + auto-learn into chat stream"
```

---

### Task 5: Wire Chat Frontend to ForgeMemory

**Files:**
- Modify: `src/lib/stores/chat.svelte.ts` (add conversation persistence + context calls)

**Step 1: Add ForgeMemory integration to sendMessage**

Update the `ChatStore` class to persist conversations and call auto-learn:

```typescript
class ChatStore {
    // ... existing fields ...
    forgeConversationId = $state<string | null>(null);

    async sendMessage(content: string, openrouterKey: string) {
        if (!this.activeConversationId) this.newConversation();
        const conv = this.activeConversation!;

        // Create ForgeMemory conversation if needed
        if (!this.forgeConversationId) {
            try {
                this.forgeConversationId = await invoke<string>('forge_memory_create_conversation', {
                    title: content.slice(0, 50),
                    modelId: this.selectedModel,
                });
            } catch { /* ForgeMemory not available — continue without */ }
        }

        // ... existing message creation code ...

        // Pass conversation_id to backend for auto-learn
        try {
            await invoke('chat_stream', {
                message: content,
                modelId: this.selectedModel,
                systemPrompt: null,
                openrouterKey: openrouterKey,
                conversationId: this.forgeConversationId,
                onEvent: channel,
            });
        } catch (e) {
            // ... existing error handling ...
        }
    }

    // Reset forge conversation when creating new UI conversation
    newConversation() {
        this.forgeConversationId = null;
        // ... existing code ...
    }
}
```

**Step 2: Verify frontend builds**

Run: `cd /opt/ork-station/ImpForge && pnpm check 2>&1 | tail -10`

**Step 3: Commit**

```bash
git add src/lib/stores/chat.svelte.ts
git commit -m "feat(chat): wire frontend to ForgeMemory conversation persistence"
```

---

## Phase 3: ForgeWatch — Filesystem Watcher (Tasks 6–9)

After Phase 3, ImpForge monitors user-configured directories, auto-discovers git repos and doc collections, and indexes file content into the memory system.

---

### Task 6: ForgeWatch Core — Filesystem Watcher Engine

**Files:**
- Create: `src-tauri/src/forge_memory/watch.rs`
- Modify: `src-tauri/src/forge_memory/mod.rs`

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_should_index_file() {
        assert!(should_index_file("main.rs"));
        assert!(should_index_file("README.md"));
        assert!(should_index_file("config.toml"));
        assert!(!should_index_file("image.png"));
        assert!(!should_index_file("binary.exe"));
        assert!(!should_index_file("package-lock.json"));
    }

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(should_skip_directory("target"));
        assert!(should_skip_directory("__pycache__"));
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("docs"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust"));
        assert_eq!(detect_language("app.ts"), Some("typescript"));
        assert_eq!(detect_language("README.md"), Some("markdown"));
        assert_eq!(detect_language("photo.jpg"), None);
    }

    #[test]
    fn test_chunk_code_by_function() {
        let code = "fn hello() {\n    println!(\"hi\");\n}\n\nfn world() {\n    println!(\"world\");\n}";
        let chunks = chunk_content(code, "rust");
        assert!(chunks.len() >= 1);
    }

    #[test]
    fn test_chunk_markdown_by_heading() {
        let md = "# Title\n\nIntro text.\n\n## Section 1\n\nContent 1.\n\n## Section 2\n\nContent 2.";
        let chunks = chunk_content(md, "markdown");
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_auto_discover() {
        let tmp = TempDir::new().unwrap();
        let proj = tmp.path().join("myproject");
        fs::create_dir_all(proj.join(".git")).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let discoveries = auto_discover(tmp.path());
        assert!(!discoveries.is_empty());
        assert!(discoveries[0].reason.contains("git"));
    }
}
```

**Step 2: Implement watch.rs**

Core watcher engine with:
- `auto_discover(root: &Path) -> Vec<DiscoveredPath>` — scan home dir for repos/doc collections
- `should_index_file(filename) -> bool` — file type filter
- `should_skip_directory(name) -> bool` — skip node_modules, .git, target, etc.
- `detect_language(filename) -> Option<&str>` — map extension to language
- `chunk_content(text, language) -> Vec<ContentChunk>` — intelligent chunking
- `ForgeWatcher` struct wrapping `notify::RecommendedWatcher`

**Step 3: Run tests, commit**

```bash
git add src-tauri/src/forge_memory/watch.rs src-tauri/src/forge_memory/mod.rs
git commit -m "feat(forge-watch): filesystem watcher engine with auto-discovery — 6 tests"
```

---

### Task 7: ForgeWatch Ingestion Pipeline

**Files:**
- Create: `src-tauri/src/forge_memory/ingest.rs`
- Modify: `src-tauri/src/forge_memory/mod.rs`

**Step 1: Implement ingestion pipeline**

```rust
//! Document Ingestion Pipeline for ForgeWatch
//!
//! Detect Type → Chunk → Embed → Dedup → Store → Index

pub struct IngestionResult {
    pub file_id: String,
    pub chunks_created: usize,
    pub chunks_skipped: usize, // dedup
    pub language: Option<String>,
}

/// Ingest a file into ForgeMemory.
pub fn ingest_file(
    engine: &ForgeMemoryEngine,
    path: &Path,
) -> Result<IngestionResult, String> {
    // 1. Read file content
    // 2. Detect language from extension
    // 3. Chunk content (code by function, markdown by heading, etc.)
    // 4. For each chunk: embed → dedup check → store in file_chunks + index
    // 5. Create KG node for the file + edges to entities found in chunks
}
```

Tests: 8+ tests covering each file type and dedup behavior.

**Step 2: Commit**

```bash
git commit -m "feat(forge-watch): document ingestion pipeline — chunk, embed, dedup, store"
```

---

### Task 8: ForgeWatch Tauri Commands

**Files:**
- Modify: `src-tauri/src/forge_memory/commands.rs` (add watch commands)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Commands to add:**

```rust
#[tauri::command] pub async fn forge_watch_discover(home_path: String) -> Result<Vec<DiscoveredPath>, String>
#[tauri::command] pub async fn forge_watch_add_path(path: String, label: Option<String>) -> Result<(), String>
#[tauri::command] pub async fn forge_watch_remove_path(path: String) -> Result<(), String>
#[tauri::command] pub async fn forge_watch_list_paths() -> Result<Vec<WatchedPathInfo>, String>
#[tauri::command] pub async fn forge_watch_start() -> Result<(), String>
#[tauri::command] pub async fn forge_watch_stop() -> Result<(), String>
#[tauri::command] pub async fn forge_watch_status() -> Result<WatchStatus, String>
#[tauri::command] pub async fn forge_watch_reindex(path: String) -> Result<IngestionResult, String>
```

**Commit:**
```bash
git commit -m "feat(forge-watch): 8 Tauri commands for filesystem monitoring"
```

---

### Task 9: ForgeWatch Settings UI

**Files:**
- Create: `src/lib/stores/forgewatch.svelte.ts`
- Modify: `src/routes/settings/+page.svelte` (add ForgeWatch settings section)

**Svelte store + Settings UI** with:
- Watched paths list (add/remove/toggle)
- Auto-discover button
- Scan mode selector (realtime/hourly/manual)
- File type filters
- Index size display
- Reindex / Clear buttons

**Commit:**
```bash
git commit -m "feat(forge-watch): settings UI with path management and auto-discovery"
```

---

## Phase 4: ForgeSunshine — Moonlight Remote Access (Tasks 10–12)

After Phase 4, users can install, configure, and manage Sunshine directly from ImpForge, enabling Moonlight remote access from any device.

---

### Task 10: ForgeSunshine Backend

**Files:**
- Create: `src-tauri/src/sunshine.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod sunshine;`)

**Implement:**
- `detect_sunshine()` — check if Sunshine is installed (which/where)
- `install_sunshine()` — platform-specific package manager command
- `configure_sunshine(config)` — write config file
- `start_sunshine()` / `stop_sunshine()` — child process management
- `sunshine_status()` — parse Sunshine web UI status
- `SunshineConfig` struct (resolution, fps, bitrate, encoder, audio, auto_start)

**Tests:** 5+ tests (detect, config serialization, platform detection)

**Commit:**
```bash
git commit -m "feat(sunshine): Moonlight remote access backend — detect, install, configure, start/stop"
```

---

### Task 11: ForgeSunshine Tauri Commands

**Files:**
- Modify: `src-tauri/src/sunshine.rs` (add #[tauri::command] functions)
- Modify: `src-tauri/src/lib.rs` (register 7 commands)

**Commands:**
```rust
sunshine_detect, sunshine_install, sunshine_configure,
sunshine_start, sunshine_stop, sunshine_pair, sunshine_status
```

**Commit:**
```bash
git commit -m "feat(sunshine): 7 Tauri commands for Moonlight remote access management"
```

---

### Task 12: ForgeSunshine Settings UI

**Files:**
- Create: `src/lib/stores/sunshine.svelte.ts`
- Modify: `src/routes/settings/+page.svelte` (add Remote Access section)

**Settings UI with:**
- Status indicator (Running/Stopped, connected clients)
- Resolution/FPS/Bitrate controls
- Encoder selection (AMD VAAPI, NVENC, Software)
- Audio toggle, auto-start toggle
- Pair New Device button (shows PIN)
- Connected devices list with latency/bandwidth
- Moonlight download links (Android, iOS, Steam Deck)
- Start/Stop streaming button

**Commit:**
```bash
git commit -m "feat(sunshine): remote access settings UI with device pairing"
```

---

## Phase 5: Universal Input Digest + Polish (Tasks 13–15)

---

### Task 13: Universal Input Digest

**Files:**
- Modify: `src-tauri/src/chat.rs` (already done in Task 4)
- Modify: `src-tauri/src/ide/mod.rs` (add digest hooks to ide_write_file, ide_execute_command)
- Modify: `src-tauri/src/ide/pty.rs` (add digest hook for terminal commands)

**Implementation:** Add lightweight hooks to existing commands that pipe significant user actions through the auto-learn pipeline. Not every keystroke — only:
- File saves (path + language + diff size)
- Terminal commands (the command string, not output)
- Search queries
- Git operations (commit messages, branch names)

**Commit:**
```bash
git commit -m "feat(input-digest): universal input capture from IDE, terminal, and git operations"
```

---

### Task 14: Auto-Learn Settings UI

**Files:**
- Create: `src/lib/stores/autolearn.svelte.ts`
- Modify: `src/routes/settings/+page.svelte` (add Auto-Learn section)

**Settings UI with:**
- NLP Mode toggle (on/off)
- LLM Mode toggle (on/off, requires Ollama)
- LLM Model selector dropdown
- Extraction sensitivity slider
- Category checkboxes (preferences, decisions, facts, technical, code patterns)
- Dedup threshold slider
- Max memories per day limit
- Memory stats display (total memories, today's extractions)

**Commit:**
```bash
git commit -m "feat(settings): auto-learn configuration UI with dual-mode toggles"
```

---

### Task 15: Integration Tests + Documentation

**Files:**
- Create: `src-tauri/src/forge_memory/tests_integration.rs`
- Modify: `docs/plans/2026-03-09-forgememory-v2-design.md` (update with final stats)

**Integration tests covering:**
1. Full chat flow: message → context enrichment → AI response → auto-learn → verify memory stored
2. ForgeWatch: create temp dir with files → trigger watcher → verify chunks indexed
3. NLP + LLM pipeline: message → both pipelines → verify no duplicates
4. Dedup: similar messages → only one memory created

**Commit:**
```bash
git commit -m "test(forge-memory-v2): integration tests for full chat-memory-watch pipeline"
```

---

## Summary

| Phase | Tasks | What's Built |
|-------|-------|-------------|
| **Phase 1** | 1-3 | NLP Pipeline (80+ patterns) + LLM Extraction (Ollama) |
| **Phase 2** | 4-5 | Chat ↔ Memory Bridge (backend + frontend) |
| **Phase 3** | 6-9 | ForgeWatch filesystem watcher + ingestion + UI |
| **Phase 4** | 10-12 | ForgeSunshine Moonlight remote access |
| **Phase 5** | 13-15 | Universal Input Digest + Settings UI + Integration Tests |

**Estimated Total:** ~3,500 LoC Rust + ~500 LoC Svelte/TypeScript
**New Tests:** ~60+ (unit + integration)
**New Tauri Commands:** ~17
