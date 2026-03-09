// Public API — consumed via forge_memory::commands Tauri layer
#![allow(dead_code)]
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

/// Extract structured entities from text using pattern matching.
pub fn extract_entities(text: &str) -> Vec<ExtractedEntity> {
    let mut entities = Vec::new();

    // File paths: /absolute/path or ~/relative/path or C:\windows\path
    {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '/' || (chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '/') {
                let start = i;
                let mut end = i + 1;
                while end < chars.len()
                    && (chars[end].is_alphanumeric()
                        || chars[end] == '/'
                        || chars[end] == '.'
                        || chars[end] == '-'
                        || chars[end] == '_')
                {
                    end += 1;
                }
                let candidate: String = chars[start..end].iter().collect();
                // Must have a dot (file extension) and be longer than 3 chars
                if candidate.len() > 3 && candidate.contains('.') && candidate.contains('/') {
                    entities.push(ExtractedEntity {
                        kind: EntityKind::FilePath,
                        value: candidate,
                        start,
                        end,
                    });
                }
                i = end;
            } else {
                i += 1;
            }
        }
    }

    // URLs: https://... or http://...
    {
        let lower = text.to_lowercase();
        for prefix in ["https://", "http://"] {
            let mut search_from = 0;
            while let Some(pos) = lower[search_from..].find(prefix) {
                let abs_pos = search_from + pos;
                let start = abs_pos;
                let mut end = start + prefix.len();
                let bytes = text.as_bytes();
                while end < bytes.len()
                    && !bytes[end].is_ascii_whitespace()
                    && bytes[end] != b','
                    && bytes[end] != b')'
                    && bytes[end] != b'>'
                {
                    end += 1;
                }
                let url = &text[start..end];
                entities.push(ExtractedEntity {
                    kind: EntityKind::Url,
                    value: url.to_string(),
                    start,
                    end,
                });
                search_from = end;
            }
        }
    }

    // Model names (ollama format: name:version e.g. qwen2.5-coder:7b)
    {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut offset = 0;
        for word in &words {
            if let Some(colon_pos) = word.find(':') {
                let before_colon = &word[..colon_pos];
                let after_colon = &word[colon_pos + 1..];
                // Must have digit after colon and alphanumeric before
                if !before_colon.is_empty()
                    && after_colon
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                    && !before_colon.starts_with("http")
                    && !before_colon.starts_with("//")
                {
                    let clean = word.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != ':' && c != '.' && c != '-' && c != '/'
                    });
                    if let Some(abs_pos) = text[offset..].find(clean) {
                        entities.push(ExtractedEntity {
                            kind: EntityKind::ModelName,
                            value: clean.to_string(),
                            start: offset + abs_pos,
                            end: offset + abs_pos + clean.len(),
                        });
                    }
                }
            }
            offset += word.len() + 1;
        }
    }

    // Ports: "port 8080" or "port: 3000"
    {
        let lower = text.to_lowercase();
        let mut search_from = 0;
        while let Some(pos) = lower[search_from..].find("port") {
            let abs_pos = search_from + pos;
            let after = &text[abs_pos + 4..];
            let trimmed = after.trim_start_matches(|c: char| c == ' ' || c == ':' || c == '=');
            let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 2 && digits.len() <= 5 {
                entities.push(ExtractedEntity {
                    kind: EntityKind::Port,
                    value: digits,
                    start: abs_pos,
                    end: abs_pos + 4 + after.len() - trimmed.len(),
                });
            }
            search_from = abs_pos + 4;
        }
    }

    // Email addresses
    {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut offset = 0;
        for word in &words {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.' && c != '-' && c != '_' && c != '+');
            if clean.contains('@') && clean.contains('.') {
                let parts: Vec<&str> = clean.split('@').collect();
                if parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') {
                    if let Some(abs_pos) = text[offset..].find(clean) {
                        entities.push(ExtractedEntity {
                            kind: EntityKind::Email,
                            value: clean.to_string(),
                            start: offset + abs_pos,
                            end: offset + abs_pos + clean.len(),
                        });
                    }
                }
            }
            offset += word.len() + 1;
        }
    }

    // IP addresses (simple: 4 groups of 1-3 digits separated by dots)
    {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut offset = 0;
        for word in &words {
            let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
            let parts: Vec<&str> = clean.split('.').collect();
            if parts.len() == 4
                && parts.iter().all(|p| {
                    !p.is_empty()
                        && p.len() <= 3
                        && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
                })
            {
                if let Some(abs_pos) = text[offset..].find(clean) {
                    entities.push(ExtractedEntity {
                        kind: EntityKind::IpAddress,
                        value: clean.to_string(),
                        start: offset + abs_pos,
                        end: offset + abs_pos + clean.len(),
                    });
                }
            }
            offset += word.len() + 1;
        }
    }

    // Well-known package/crate names
    let known_packages = [
        "tokio", "serde", "reqwest", "axum", "actix", "diesel", "sqlx", "rusqlite",
        "react", "svelte", "vue", "angular", "next", "nuxt", "express", "fastapi",
        "django", "flask", "numpy", "pandas", "pytorch", "tensorflow", "transformers",
        "ollama", "docker", "kubernetes", "redis", "postgresql", "sqlite", "mongodb",
        "tauri", "electron", "vite", "webpack", "rollup", "esbuild", "deno", "bun",
    ];
    {
        let lower = text.to_lowercase();
        for pkg in &known_packages {
            if let Some(pos) = lower.find(pkg) {
                let before = if pos > 0 {
                    lower.as_bytes()[pos - 1]
                } else {
                    b' '
                };
                let after_pos = pos + pkg.len();
                let after = if after_pos < lower.len() {
                    lower.as_bytes()[after_pos]
                } else {
                    b' '
                };
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

/// Classify a message using 80+ multilingual patterns (DE + EN).
///
/// Returns categories (preference, decision, explicit_note, technical,
/// negation, correction, code_pattern) with computed importance [0.0, 1.0].
pub fn classify_message(text: &str) -> ClassificationResult {
    let lower = text.to_lowercase();
    let mut categories = Vec::new();
    let mut importance: f64 = 0.1; // baseline

    // ── Preference patterns (EN + DE) ──
    let preference_patterns = [
        "i prefer ", "i like ", "i always use ", "i don't like ", "my favorite ",
        "i usually ", "i want ", "i love ", "i hate ", "i enjoy ",
        "ich bevorzuge ", "ich mag ", "ich nutze immer ", "ich will ",
        "mein favorit ", "ich benutze immer ", "am liebsten ",
    ];
    if preference_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("preference".to_string());
        importance = importance.max(0.7);
    }

    // ── Decision patterns (EN + DE) ──
    let decision_patterns = [
        "let's use ", "we decided ", "the plan is ", "we should ", "let's go with ",
        "i chose ", "we'll ", "we will ", "going to use ", "switching to ",
        "wir nutzen ", "wir verwenden ", "der plan ist ", "wir haben entschieden ",
        "lass uns ", "nehmen wir ",
    ];
    if decision_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("decision".to_string());
        importance = importance.max(0.7);
    }

    // ── Explicit note / remember patterns (EN + DE) ──
    let note_patterns = [
        "remember that ", "remember:", "note:", "important:", "don't forget ",
        "keep in mind ", "merke dir ", "merke:", "achtung:", "wichtig:",
        "merk dir gut:", "vergiss nicht ", "beachte:", "remember this:",
    ];
    if note_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("explicit_note".to_string());
        importance = importance.max(0.85);
    }

    // ── Technical fact patterns (EN + DE) ──
    let tech_patterns = [
        "runs on port", "version is", "version:", "api key", "endpoint is",
        "installed at", "configured to", "runs on", "built with", "compiled with",
        "läuft auf port", "version ist", "installiert in", "konfiguriert mit",
        "gebaut mit", "node.js", "python ", "rust ", "java ",
    ];
    if tech_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("technical".to_string());
        importance = importance.max(0.6);
    }

    // ── Negation / prohibition patterns (EN + DE) ──
    let negation_patterns = [
        "never use ", "don't use ", "avoid ", "stop using ", "deprecated ",
        "do not ", "must not ", "should not ", "shouldn't ",
        "niemals ", "nicht verwenden ", "vermeiden ", "nicht benutzen ",
        "verboten ", "auf keinen fall ",
    ];
    if negation_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("negation".to_string());
        importance = importance.max(0.75);
    }

    // ── Correction patterns (EN + DE) ──
    let correction_patterns = [
        "actually it's", "actually, it's", "actually it is", "i meant ",
        "correction:", "that's wrong", "not correct", "i was wrong",
        "nein, das ist", "eigentlich ist", "das stimmt nicht", "korrektur:",
        "falsch, es ist", "das ist falsch",
    ];
    if correction_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("correction".to_string());
        importance = importance.max(0.8);
    }

    // ── Code pattern detection ──
    let code_indicators = [
        "use ", "import ", "from ", "require(", "include ", "#include ",
        "fn ", "def ", "class ", "struct ", "enum ", "interface ",
        "pub fn ", "async fn ", "pub struct ", "pub enum ",
    ];
    let is_code_like = code_indicators.iter().any(|p| text.contains(p))
        && text.len() < 200
        && (text.contains(';') || text.contains('{') || text.contains("::"));
    if is_code_like {
        categories.push("code_pattern".to_string());
        importance = importance.max(0.5);
    }

    // ── Problem / bug report patterns (EN + DE) ──
    let problem_patterns = [
        "error:", "bug:", "crash", "fails with", "broken", "doesn't work",
        "not working", "segfault", "panic", "exception",
        "fehler:", "bug:", "stürzt ab", "funktioniert nicht", "kaputt",
    ];
    if problem_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("problem".to_string());
        importance = importance.max(0.7);
    }

    // ── Workflow / process patterns (EN + DE) ──
    let workflow_patterns = [
        "first ", "then ", "after that ", "finally ", "step 1", "step 2",
        "the workflow is", "the process is", "pipeline:",
        "zuerst ", "dann ", "danach ", "schritt 1", "der workflow ist",
    ];
    if workflow_patterns.iter().any(|p| lower.contains(p)) {
        categories.push("workflow".to_string());
        importance = importance.max(0.55);
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

/// Extract simple relations from text using pattern matching.
pub fn extract_relations(text: &str) -> Vec<ExtractedRelation> {
    let mut relations = Vec::new();
    let lower = text.to_lowercase();

    // "X use(s) Y" pattern
    for pattern in ["i use ", "we use ", "using "] {
        if let Some(pos) = lower.find(pattern) {
            let after = &text[pos + pattern.len()..];
            let object: String = after
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            if !object.is_empty() {
                let subject = if pattern.starts_with('i') {
                    "I"
                } else {
                    "we"
                };
                relations.push(ExtractedRelation {
                    kind: RelationKind::Uses,
                    subject: subject.to_string(),
                    object: object
                        .trim_end_matches(|c: char| c.is_ascii_punctuation())
                        .to_string(),
                });
            }
        }
    }

    // "X depends on Y"
    if let Some(pos) = lower.find("depends on ") {
        let after = &text[pos + 11..];
        let object: String = after
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        relations.push(ExtractedRelation {
            kind: RelationKind::DependsOn,
            subject: text[..pos]
                .split_whitespace()
                .last()
                .unwrap_or("it")
                .to_string(),
            object,
        });
    }

    // "prefer X over Y"
    if let Some(pos) = lower.find(" over ") {
        if lower[..pos].contains("prefer") {
            let before_over = &text[..pos];
            let after_over = &text[pos + 6..];
            let preferred = before_over
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_string();
            let other: String = after_over
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            if !preferred.is_empty() && !other.is_empty() {
                relations.push(ExtractedRelation {
                    kind: RelationKind::PrefersOver,
                    subject: preferred,
                    object: other
                        .trim_end_matches(|c: char| c.is_ascii_punctuation())
                        .to_string(),
                });
            }
        }
    }

    // "X runs on Y"
    if let Some(pos) = lower.find("runs on ") {
        let after = &text[pos + 8..];
        let object: String = after
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        let subject = text[..pos]
            .split_whitespace()
            .last()
            .unwrap_or("it")
            .to_string();
        if !object.is_empty() {
            relations.push(ExtractedRelation {
                kind: RelationKind::RunsOn,
                subject,
                object: object
                    .trim_end_matches(|c: char| c.is_ascii_punctuation())
                    .to_string(),
            });
        }
    }

    // "X version Y" or "X v1.2.3"
    if let Some(pos) = lower.find("version ") {
        let after = &text[pos + 8..];
        let version: String = after
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(|c: char| c.is_ascii_punctuation() && c != '.')
            .to_string();
        let subject = text[..pos]
            .split_whitespace()
            .last()
            .unwrap_or("it")
            .to_string();
        if !version.is_empty() {
            relations.push(ExtractedRelation {
                kind: RelationKind::VersionIs,
                subject,
                object: version,
            });
        }
    }

    relations
}

// ── Dedup Detection ─────────────────────────────────────────────

/// Check if two texts are near-duplicates using combined word + trigram similarity.
///
/// Two-stage approach for robust dedup:
///   1. Word-level Jaccard (catches semantic similarity, e.g., "dark mode" ≈ "dark themes")
///   2. Trigram-level Jaccard (catches character-level similarity)
/// Final score = max(word_sim, trigram_sim). Threshold: 0.5.
pub fn is_near_duplicate(a: &str, b: &str) -> bool {
    let threshold = 0.5;
    let la = a.to_lowercase();
    let lb = b.to_lowercase();

    if la == lb {
        return true;
    }

    // Word-level Jaccard
    let words_a: Vec<&str> = la.split_whitespace().collect();
    let words_b: Vec<&str> = lb.split_whitespace().collect();
    let word_intersection = words_a.iter().filter(|w| words_b.contains(w)).count();
    let word_union = words_a.len() + words_b.len() - word_intersection;
    let word_sim = if word_union > 0 {
        word_intersection as f64 / word_union as f64
    } else {
        0.0
    };

    // Trigram-level Jaccard
    let trigrams_a = trigrams(&la);
    let trigrams_b = trigrams(&lb);
    let tri_intersection = trigrams_a.iter().filter(|t| trigrams_b.contains(t)).count();
    let tri_union = trigrams_a.len() + trigrams_b.len() - tri_intersection;
    let tri_sim = if tri_union > 0 {
        tri_intersection as f64 / tri_union as f64
    } else {
        0.0
    };

    // Use the higher of both signals
    word_sim.max(tri_sim) >= threshold
}

fn trigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        return vec![text.to_string()];
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

// ── Sensitive Data Filter ───────────────────────────────────────

/// Detect sensitive data that should NEVER be stored in memory.
///
/// Checks for passwords, API keys, tokens, secrets, and common
/// credential patterns. Returns true if ANY sensitive pattern is found.
pub fn contains_sensitive_data(text: &str) -> bool {
    let lower = text.to_lowercase();
    let sensitive_patterns = [
        // Password patterns
        "password=", "password:", "password is ", "my password", "passwd=",
        // API key patterns
        "api_key=", "api-key=", "apikey=", "api key=", "api_key:",
        // Token/Secret patterns
        "token=", "secret=", "secret_key=", "access_key=", "private_key=",
        // Export patterns (shell)
        "export api_key", "export secret", "export token", "export password",
        // Common token prefixes
        "sk-", "ghp_", "gho_", "glpat-", "xoxb-", "xoxp-",
        // Auth headers
        "bearer ", "authorization:",
        // Database connection strings
        "postgres://", "mysql://", "mongodb+srv://",
    ];
    sensitive_patterns.iter().any(|p| lower.contains(p))
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_paths() {
        let entities = extract_entities("Check /home/user/project/main.rs for the bug");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::FilePath && e.value.contains("main.rs")),
            "Expected FilePath entity containing 'main.rs', got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_urls() {
        let entities = extract_entities("See https://docs.rs/tokio/latest for docs");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::Url && e.value.contains("docs.rs")),
            "Expected Url entity containing 'docs.rs', got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_package_names() {
        let entities = extract_entities("We use tokio and serde for this project");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::PackageName && e.value == "tokio"),
            "Expected PackageName 'tokio', got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_model_names() {
        let entities = extract_entities("Running qwen2.5-coder:7b on the GPU");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::ModelName && e.value == "qwen2.5-coder:7b"),
            "Expected ModelName 'qwen2.5-coder:7b', got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_ports() {
        let entities = extract_entities("The server runs on port 8080");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::Port && e.value == "8080"),
            "Expected Port '8080', got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_emails() {
        let entities = extract_entities("Contact us at hello@impforge.dev for support");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::Email && e.value.contains("hello@impforge.dev")),
            "Expected Email entity, got: {:?}",
            entities
        );
    }

    #[test]
    fn test_extract_ip_addresses() {
        let entities = extract_entities("Server is at 192.168.1.100 on the LAN");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::IpAddress && e.value == "192.168.1.100"),
            "Expected IpAddress '192.168.1.100', got: {:?}",
            entities
        );
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
        assert!(
            result.categories.contains(&"preference".to_string()),
            "Expected 'preference' category, got: {:?}",
            result.categories
        );
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
        assert!(
            result.categories.contains(&"technical".to_string()),
            "Expected 'technical' category, got: {:?}",
            result.categories
        );
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
        assert!(
            result.categories.contains(&"code_pattern".to_string()),
            "Expected 'code_pattern' category, got: {:?}",
            result.categories
        );
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
        assert!(
            relations
                .iter()
                .any(|r| r.kind == RelationKind::Uses && r.subject == "I" && r.object.contains("Rust")),
            "Expected Uses relation with Rust, got: {:?}",
            relations
        );
    }

    #[test]
    fn test_dedup_similar() {
        assert!(is_near_duplicate("I prefer dark mode", "I prefer dark themes"));
    }

    #[test]
    fn test_dedup_different() {
        assert!(!is_near_duplicate(
            "I prefer dark mode",
            "The server runs on port 8080"
        ));
    }

    #[test]
    fn test_sensitive_data_filter() {
        assert!(contains_sensitive_data("My password is hunter2"));
        assert!(contains_sensitive_data("export API_KEY=sk-abc123"));
        assert!(!contains_sensitive_data("The weather is nice today"));
    }

    #[test]
    fn test_problem_pattern() {
        let result = classify_message("Error: the build fails with a segfault on startup");
        assert!(
            result.categories.contains(&"problem".to_string()),
            "Expected 'problem' category, got: {:?}",
            result.categories
        );
    }

    #[test]
    fn test_workflow_pattern() {
        let result = classify_message("First compile the project, then run the tests");
        assert!(
            result.categories.contains(&"workflow".to_string()),
            "Expected 'workflow' category, got: {:?}",
            result.categories
        );
    }
}
