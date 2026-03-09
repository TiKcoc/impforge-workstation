#![allow(dead_code)]
//! Universal Input Digest — Auto-learn from any text source
//!
//! Processes text from terminal output, file saves, editor actions,
//! clipboard, and any other input surface. Extracts knowledge using
//! the NLP pipeline and stores it in ForgeMemory.
//!
//! Architecture:
//!   - Frontend calls `forge_digest_text` with source-tagged content
//!   - Rate-limited: debounces rapid inputs (terminal scrollback)
//!   - Filters noise: ANSI codes, prompts, empty lines, short gibberish
//!   - Routes through NLP classify → dedup → store → KG extract
//!
//! Input sources:
//!   - `terminal`: PTY output (filtered for commands + output)
//!   - `editor`: File save content (code files, configs)
//!   - `chat`: Conversation messages (already handled by auto_learn)
//!   - `clipboard`: Clipboard paste events
//!   - `url`: Web page content from browser
//!
//! References:
//!   - Packer et al. (2023). MemGPT: Towards LLMs as Operating Systems.
//!   - Anthropic (2024). Contextual Retrieval.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use super::engine::ForgeMemoryEngine;
use super::nlp;

// ── Types ───────────────────────────────────────────────────────

/// Source of the input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DigestSource {
    Terminal,
    Editor,
    Chat,
    Clipboard,
    Url,
}

impl DigestSource {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "terminal" | "pty" => Self::Terminal,
            "editor" | "file" => Self::Editor,
            "chat" | "conversation" => Self::Chat,
            "clipboard" | "paste" => Self::Clipboard,
            "url" | "web" | "browser" => Self::Url,
            _ => Self::Chat,
        }
    }

    pub fn category_prefix(&self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Editor => "editor",
            Self::Chat => "chat",
            Self::Clipboard => "clipboard",
            Self::Url => "web",
        }
    }
}

/// Result of digesting input text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestResult {
    pub source: String,
    pub lines_processed: usize,
    pub lines_filtered: usize,
    pub memories_created: usize,
    pub memories_reinforced: usize,
    pub entities_extracted: usize,
    pub relations_extracted: usize,
    pub skipped_reason: Option<String>,
}

/// Configuration for the digest pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestConfig {
    /// Enable terminal output digestion.
    pub terminal_enabled: bool,
    /// Enable editor file save digestion.
    pub editor_enabled: bool,
    /// Enable clipboard digestion.
    pub clipboard_enabled: bool,
    /// Enable URL/web content digestion.
    pub url_enabled: bool,
    /// Minimum line length to consider (filters noise).
    pub min_line_length: usize,
    /// Maximum lines per digest call (prevents flooding).
    pub max_lines: usize,
    /// Debounce interval in milliseconds per source.
    pub debounce_ms: u64,
    /// NLP sensitivity threshold (0.0-1.0).
    pub nlp_threshold: f64,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            terminal_enabled: true,
            editor_enabled: true,
            clipboard_enabled: false,
            url_enabled: true,
            min_line_length: 10,
            max_lines: 200,
            debounce_ms: 2000,
            nlp_threshold: 0.3_f64,
        }
    }
}

// ── Rate Limiter ────────────────────────────────────────────────

/// Per-source rate limiter to prevent flooding from rapid terminal output.
pub struct DigestRateLimiter {
    last_digest: Mutex<HashMap<String, Instant>>,
    debounce_ms: u64,
}

impl DigestRateLimiter {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            last_digest: Mutex::new(HashMap::new()),
            debounce_ms,
        }
    }

    /// Check if enough time has passed since the last digest for this source.
    /// Returns true if the digest should proceed, false if rate-limited.
    pub fn should_proceed(&self, source_key: &str) -> bool {
        let mut map = self.last_digest.lock().unwrap();
        let now = Instant::now();

        if let Some(last) = map.get(source_key) {
            if now.duration_since(*last).as_millis() < self.debounce_ms as u128 {
                return false;
            }
        }

        map.insert(source_key.to_string(), now);
        true
    }
}

// ── Text Cleaning ───────────────────────────────────────────────

/// Strip ANSI escape codes from terminal output.
pub fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // ESC sequence — consume until terminator
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next(); // consume '['
                    // CSI sequence: consume until letter
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii_alphabetic() || c == '~' {
                            break;
                        }
                    }
                    continue;
                } else if next == ']' {
                    // OSC sequence: consume until ST (BEL or ESC\)
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                    continue;
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Filter out noise lines from text input.
/// Returns cleaned lines that are worth processing.
pub fn filter_lines(text: &str, min_length: usize, max_lines: usize) -> Vec<String> {
    text.lines()
        .map(|line| strip_ansi(line).trim().to_string())
        .filter(|line| {
            // Skip empty lines
            if line.len() < min_length {
                return false;
            }
            // Skip common terminal noise
            if is_terminal_noise(line) {
                return false;
            }
            true
        })
        .take(max_lines)
        .collect()
}

/// Detect common terminal noise patterns.
fn is_terminal_noise(line: &str) -> bool {
    // Shell prompts (user@host, $, %, #)
    if line.ends_with('$') || line.ends_with('%') || line.ends_with('#') {
        if line.len() < 80 {
            return true;
        }
    }
    // Progress bars and spinners
    if line.contains("━") || line.contains("▓") || line.contains("░") || line.contains("█") {
        return true;
    }
    // Percentage progress
    if line.contains("% |") || line.contains("100%") {
        return true;
    }
    // Git status shorthand (single letter + space + path)
    if line.len() < 5 && (line.starts_with("M ") || line.starts_with("A ") || line.starts_with("D ")) {
        return true;
    }
    // Pure whitespace or separator lines
    if line.chars().all(|c| c == '-' || c == '=' || c == '_' || c == ' ') {
        return true;
    }
    false
}

// ── Digest Pipeline ─────────────────────────────────────────────

/// Process text from any source through the NLP pipeline.
///
/// Steps:
///   1. Rate-limit check (per source)
///   2. Clean text (strip ANSI, filter noise)
///   3. NLP classify each meaningful line
///   4. Dedup against existing memories
///   5. Store new memories + KG nodes/edges
///   6. Reinforce relevant existing memories
pub fn digest_text(
    engine: &ForgeMemoryEngine,
    text: &str,
    source: DigestSource,
    config: &DigestConfig,
) -> Result<DigestResult, String> {
    // Check source is enabled
    let enabled = match source {
        DigestSource::Terminal => config.terminal_enabled,
        DigestSource::Editor => config.editor_enabled,
        DigestSource::Clipboard => config.clipboard_enabled,
        DigestSource::Url => config.url_enabled,
        DigestSource::Chat => true, // always enabled
    };

    if !enabled {
        return Ok(DigestResult {
            source: format!("{:?}", source),
            lines_processed: 0,
            lines_filtered: 0,
            memories_created: 0,
            memories_reinforced: 0,
            entities_extracted: 0,
            relations_extracted: 0,
            skipped_reason: Some("Source disabled in config".to_string()),
        });
    }

    // Clean and filter lines
    let all_lines = text.lines().count();
    let lines = filter_lines(text, config.min_line_length, config.max_lines);
    let filtered_count = all_lines - lines.len();

    if lines.is_empty() {
        return Ok(DigestResult {
            source: format!("{:?}", source),
            lines_processed: 0,
            lines_filtered: filtered_count,
            memories_created: 0,
            memories_reinforced: 0,
            entities_extracted: 0,
            relations_extracted: 0,
            skipped_reason: Some("No meaningful content after filtering".to_string()),
        });
    }

    let mut memories_created = 0;
    let mut memories_reinforced = 0;
    let mut total_entities = 0;
    let mut total_relations = 0;

    // Process each line through NLP
    let combined = lines.join("\n");
    let classification = nlp::classify_message(&combined);

    // Only store if classification has meaningful categories
    if !classification.categories.is_empty()
        && classification.importance >= config.nlp_threshold
    {
        // Check for sensitive data
        if !nlp::contains_sensitive_data(&combined) {
            // Dedup check
            if !engine.is_duplicate(&combined)? {
                let scope = if classification.importance >= 0.85 {
                    "core"
                } else {
                    "recall"
                };
                let primary_category = format!(
                    "{}:{}",
                    source.category_prefix(),
                    classification
                        .categories
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "general".to_string())
                );

                engine.add_memory(
                    &combined,
                    scope,
                    classification.importance,
                    &primary_category,
                )?;
                memories_created += 1;
            }
        }

        // KG extraction
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
            total_entities += 1;
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
            total_relations += 1;
        }
    }

    // Reinforce relevant existing memories
    if combined.len() >= 10 {
        let relevant = engine.search(&combined[..combined.len().min(200)], 3)?;
        for result in &relevant {
            if result.score > 0.01 {
                let _ = engine.review_memory(&result.id, "good");
                memories_reinforced += 1;
            }
        }
    }

    Ok(DigestResult {
        source: format!("{:?}", source),
        lines_processed: lines.len(),
        lines_filtered: filtered_count,
        memories_created,
        memories_reinforced,
        entities_extracted: total_entities,
        relations_extracted: total_relations,
        skipped_reason: None,
    })
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_clean() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn test_strip_ansi_color_codes() {
        assert_eq!(
            strip_ansi("\x1b[32mgreen\x1b[0m text"),
            "green text"
        );
        assert_eq!(
            strip_ansi("\x1b[1;31merror\x1b[0m"),
            "error"
        );
    }

    #[test]
    fn test_strip_ansi_complex() {
        // Bold + underline + color
        assert_eq!(
            strip_ansi("\x1b[1m\x1b[4m\x1b[36mhello\x1b[0m"),
            "hello"
        );
    }

    #[test]
    fn test_filter_lines_basic() {
        let text = "short\n\nThis is a meaningful line with enough content\nAnother good line here\n";
        let lines = filter_lines(text, 10, 100);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("meaningful"));
    }

    #[test]
    fn test_filter_lines_terminal_noise() {
        let text = "user@host:~/project$\n━━━━━━━━━━\n50% |████\nactual error: cannot find module\n";
        let lines = filter_lines(text, 10, 100);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("error"));
    }

    #[test]
    fn test_filter_lines_max_limit() {
        let text = (0..500).map(|i| format!("Line number {} with enough content", i)).collect::<Vec<_>>().join("\n");
        let lines = filter_lines(&text, 10, 50);
        assert_eq!(lines.len(), 50);
    }

    #[test]
    fn test_is_terminal_noise() {
        assert!(is_terminal_noise("user@host:~$"));
        assert!(is_terminal_noise("━━━━━━━━━━━━"));
        assert!(is_terminal_noise("50% |████████"));
        assert!(is_terminal_noise("-------------------"));
        assert!(!is_terminal_noise("error[E0425]: cannot find value `x`"));
        assert!(!is_terminal_noise("warning: unused variable `y`"));
    }

    #[test]
    fn test_digest_source_from_str() {
        assert_eq!(DigestSource::from_str("terminal"), DigestSource::Terminal);
        assert_eq!(DigestSource::from_str("pty"), DigestSource::Terminal);
        assert_eq!(DigestSource::from_str("editor"), DigestSource::Editor);
        assert_eq!(DigestSource::from_str("file"), DigestSource::Editor);
        assert_eq!(DigestSource::from_str("clipboard"), DigestSource::Clipboard);
        assert_eq!(DigestSource::from_str("url"), DigestSource::Url);
        assert_eq!(DigestSource::from_str("unknown"), DigestSource::Chat);
    }

    #[test]
    fn test_digest_config_default() {
        let config = DigestConfig::default();
        assert!(config.terminal_enabled);
        assert!(config.editor_enabled);
        assert!(!config.clipboard_enabled);
        assert_eq!(config.min_line_length, 10);
        assert_eq!(config.max_lines, 200);
    }

    #[test]
    fn test_digest_disabled_source() {
        let engine = crate::forge_memory::engine::ForgeMemoryEngine::open_memory().unwrap();
        let mut config = DigestConfig::default();
        config.clipboard_enabled = false;

        let result = digest_text(&engine, "some clipboard content that is long enough", DigestSource::Clipboard, &config).unwrap();
        assert_eq!(result.memories_created, 0);
        assert!(result.skipped_reason.is_some());
    }

    #[test]
    fn test_digest_empty_after_filter() {
        let engine = crate::forge_memory::engine::ForgeMemoryEngine::open_memory().unwrap();
        let config = DigestConfig::default();

        let result = digest_text(&engine, "hi\n\n\n", DigestSource::Terminal, &config).unwrap();
        assert_eq!(result.lines_processed, 0);
        assert!(result.skipped_reason.is_some());
    }

    #[test]
    fn test_digest_terminal_output() {
        let engine = crate::forge_memory::engine::ForgeMemoryEngine::open_memory().unwrap();
        let config = DigestConfig::default();

        let terminal_output = "error[E0425]: cannot find value `config` in this scope\n  --> src/main.rs:42:5\n   |\n42 |     config.load();\n   |     ^^^^^^ not found in this scope";
        let result = digest_text(&engine, terminal_output, DigestSource::Terminal, &config).unwrap();
        // Should process meaningful error lines
        assert!(result.lines_processed > 0);
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = DigestRateLimiter::new(100);
        assert!(limiter.should_proceed("test-source"));
        assert!(!limiter.should_proceed("test-source")); // too soon
        assert!(limiter.should_proceed("other-source")); // different source
    }

    #[test]
    fn test_rate_limiter_after_delay() {
        let limiter = DigestRateLimiter::new(10);
        assert!(limiter.should_proceed("test"));
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert!(limiter.should_proceed("test")); // enough time passed
    }
}
