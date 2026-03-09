// SPDX-License-Identifier: BUSL-1.1
//! Smart Browser Types for ImpForge AI Engine
//!
//! Advanced browser automation planning and analysis types used by the
//! engine's orchestrator to drive intelligent web navigation.
//!
//! Browsing strategies:
//! - Sequential: visit pages one by one in order
//! - Parallel: fetch multiple pages concurrently
//! - Adaptive: adjust strategy based on page relevance scores
//! - DepthFirst: follow links deeply before backtracking
//! - BreadthFirst: explore all links at each level before going deeper
//!
//! The `SmartBrowser` planner computes complexity estimates and relevance
//! scores so the orchestrator can decide whether to continue, switch
//! strategies, or abort a browsing session.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPES — Browsing Strategy & Planning
// ============================================================================

/// Strategy used by the browser planner to traverse pages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BrowsingStrategy {
    /// Visit pages one by one in the order they appear in `target_urls`.
    Sequential,
    /// Fetch multiple pages concurrently (up to a platform-defined limit).
    Parallel,
    /// Start sequential, switch to parallel when relevance is high.
    Adaptive,
    /// Follow links as deep as possible before backtracking (DFS).
    DepthFirst,
    /// Explore all links at the current depth before going deeper (BFS).
    BreadthFirst,
}

/// Analysis result for a single visited page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAnalysis {
    /// The URL that was visited.
    pub url: String,
    /// The page title extracted from the document.
    pub title: String,
    /// A hash of the page content (for deduplication / change detection).
    pub content_hash: String,
    /// Relevance score in [0.0, 1.0] relative to the browsing task.
    pub relevance_score: f64,
    /// Data items extracted from the page (text snippets, values, etc.).
    pub extracted_data: Vec<String>,
    /// When the page was analyzed.
    pub timestamp: DateTime<Utc>,
}

/// A browsing plan that tells the SmartBrowser what to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPlan {
    /// Which traversal strategy to use.
    pub strategy: BrowsingStrategy,
    /// Maximum number of pages to visit before stopping.
    pub max_pages: usize,
    /// Maximum link-follow depth (only meaningful for DepthFirst / BreadthFirst).
    pub max_depth: u32,
    /// Per-page timeout in seconds.
    pub timeout_secs: u64,
    /// Seed URLs to start browsing from.
    pub target_urls: Vec<String>,
}

/// Outcome of a completed browsing session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserResult {
    /// How many distinct pages were visited.
    pub pages_visited: usize,
    /// Analysis data for every page that was examined.
    pub pages_analyzed: Vec<PageAnalysis>,
    /// Wall-clock duration of the entire session in milliseconds.
    pub total_duration_ms: u64,
    /// Whether the session completed without fatal errors.
    pub success: bool,
    /// If the session failed, the reason why.
    pub error: Option<String>,
}

// ============================================================================
// SMART BROWSER — Planner / Estimator
// ============================================================================

/// Intelligent browser planner that estimates complexity, decides whether to
/// continue browsing, and scores content relevance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartBrowser {
    /// The plan driving this browser session.
    pub plan: BrowserPlan,
}

impl SmartBrowser {
    /// Create a new SmartBrowser from a browsing plan.
    pub fn new(plan: BrowserPlan) -> Self {
        Self { plan }
    }

    /// Estimate the complexity of the browsing plan on a 0.0 -- 1.0 scale.
    ///
    /// Factors:
    /// - Number of target URLs (more URLs = higher complexity)
    /// - Maximum depth (deeper crawls are harder)
    /// - Strategy choice (parallel/adaptive are more complex to coordinate)
    ///
    /// Returns a value in [0.0, 1.0].
    pub fn estimate_complexity(&self) -> f64 {
        // Page factor: each page adds complexity, diminishing after 50.
        let page_factor = (self.plan.max_pages as f64 / 50.0).min(1.0);

        // Depth factor: depth 1 = 0.1, depth 10+ = 1.0.
        let depth_factor = (self.plan.max_depth as f64 / 10.0).min(1.0);

        // Strategy weight: tree-traversal strategies are inherently more complex.
        let strategy_weight = match self.plan.strategy {
            BrowsingStrategy::Sequential => 0.2,
            BrowsingStrategy::Parallel => 0.5,
            BrowsingStrategy::Adaptive => 0.6,
            BrowsingStrategy::DepthFirst => 0.7,
            BrowsingStrategy::BreadthFirst => 0.8,
        };

        // URL seed factor: having many seeds increases initial fan-out.
        let url_factor = (self.plan.target_urls.len() as f64 / 20.0).min(1.0);

        // Weighted combination, clamped to [0.0, 1.0].
        let raw = page_factor * 0.3 + depth_factor * 0.25 + strategy_weight * 0.25 + url_factor * 0.2;
        raw.clamp(0.0, 1.0)
    }

    /// Decide whether the browser should keep visiting more pages.
    ///
    /// Returns `true` when the number of pages already visited is below the
    /// plan's `max_pages` limit.
    pub fn should_continue(&self, visited: usize) -> bool {
        visited < self.plan.max_pages
    }

    /// Calculate a term-frequency-based relevance score for `content` given a
    /// set of `keywords`.
    ///
    /// Each keyword is searched case-insensitively. The score is the fraction
    /// of keywords that appear at least once, weighted by how often they occur.
    ///
    /// Returns a value in [0.0, 1.0].
    pub fn calculate_relevance(content: &str, keywords: &[&str]) -> f64 {
        if keywords.is_empty() || content.is_empty() {
            return 0.0;
        }

        let lower = content.to_lowercase();
        let total_words = lower.split_whitespace().count().max(1) as f64;

        let mut matched_keywords = 0usize;
        let mut total_tf = 0.0f64;

        for kw in keywords {
            let kw_lower = kw.to_lowercase();
            let count = lower.matches(&kw_lower).count();
            if count > 0 {
                matched_keywords += 1;
                // TF = occurrences / total_words (standard term frequency)
                total_tf += count as f64 / total_words;
            }
        }

        // Coverage: what fraction of keywords matched at all.
        let coverage = matched_keywords as f64 / keywords.len() as f64;

        // Average TF across all keywords (matched or not).
        let avg_tf = total_tf / keywords.len() as f64;

        // Combine: coverage dominates, TF fine-tunes.
        // Scaled so that "all keywords present with moderate frequency" -> ~1.0.
        let raw = coverage * 0.7 + (avg_tf * 100.0).min(1.0) * 0.3;
        raw.clamp(0.0, 1.0)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Strategy creation ---------------------------------------------------

    #[test]
    fn test_strategy_creation_sequential() {
        let s = BrowsingStrategy::Sequential;
        assert_eq!(s, BrowsingStrategy::Sequential);
    }

    #[test]
    fn test_strategy_serde_roundtrip() {
        let strategies = vec![
            BrowsingStrategy::Sequential,
            BrowsingStrategy::Parallel,
            BrowsingStrategy::Adaptive,
            BrowsingStrategy::DepthFirst,
            BrowsingStrategy::BreadthFirst,
        ];
        for s in &strategies {
            let json = serde_json::to_string(s).unwrap();
            let back: BrowsingStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, s);
        }
    }

    // -- Plan validation -----------------------------------------------------

    #[test]
    fn test_plan_with_valid_fields() {
        let plan = BrowserPlan {
            strategy: BrowsingStrategy::Parallel,
            max_pages: 100,
            max_depth: 5,
            timeout_secs: 30,
            target_urls: vec!["https://example.com".into()],
        };
        assert_eq!(plan.max_pages, 100);
        assert_eq!(plan.max_depth, 5);
        assert_eq!(plan.timeout_secs, 30);
        assert_eq!(plan.target_urls.len(), 1);
    }

    #[test]
    fn test_plan_serde_roundtrip() {
        let plan = BrowserPlan {
            strategy: BrowsingStrategy::DepthFirst,
            max_pages: 10,
            max_depth: 3,
            timeout_secs: 60,
            target_urls: vec![
                "https://a.example.com".into(),
                "https://b.example.com".into(),
            ],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: BrowserPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back.max_pages, 10);
        assert_eq!(back.target_urls.len(), 2);
    }

    // -- Complexity estimation -----------------------------------------------

    #[test]
    fn test_complexity_minimal_plan() {
        let browser = SmartBrowser::new(BrowserPlan {
            strategy: BrowsingStrategy::Sequential,
            max_pages: 1,
            max_depth: 1,
            timeout_secs: 10,
            target_urls: vec!["https://example.com".into()],
        });
        let c = browser.estimate_complexity();
        assert!(c > 0.0, "Complexity should be positive for any valid plan");
        assert!(c < 0.5, "A minimal plan should have low complexity, got {c}");
    }

    #[test]
    fn test_complexity_large_plan() {
        let browser = SmartBrowser::new(BrowserPlan {
            strategy: BrowsingStrategy::BreadthFirst,
            max_pages: 200,
            max_depth: 20,
            timeout_secs: 120,
            target_urls: (0..30).map(|i| format!("https://{i}.example.com")).collect(),
        });
        let c = browser.estimate_complexity();
        assert!(c > 0.7, "A large BFS plan should be highly complex, got {c}");
        assert!(c <= 1.0, "Complexity must not exceed 1.0");
    }

    // -- Continuation logic --------------------------------------------------

    #[test]
    fn test_should_continue_under_limit() {
        let browser = SmartBrowser::new(BrowserPlan {
            strategy: BrowsingStrategy::Sequential,
            max_pages: 10,
            max_depth: 1,
            timeout_secs: 30,
            target_urls: vec![],
        });
        assert!(browser.should_continue(0));
        assert!(browser.should_continue(5));
        assert!(browser.should_continue(9));
    }

    #[test]
    fn test_should_continue_at_and_over_limit() {
        let browser = SmartBrowser::new(BrowserPlan {
            strategy: BrowsingStrategy::Sequential,
            max_pages: 10,
            max_depth: 1,
            timeout_secs: 30,
            target_urls: vec![],
        });
        assert!(!browser.should_continue(10));
        assert!(!browser.should_continue(100));
    }

    // -- Relevance calculation -----------------------------------------------

    #[test]
    fn test_relevance_all_keywords_present() {
        let content = "Rust is a systems programming language focused on safety and performance";
        let keywords = &["rust", "safety", "performance"];
        let score = SmartBrowser::calculate_relevance(content, keywords);
        assert!(score > 0.5, "All keywords present should yield high relevance, got {score}");
        assert!(score <= 1.0);
    }

    #[test]
    fn test_relevance_no_keywords_present() {
        let content = "The quick brown fox jumps over the lazy dog";
        let keywords = &["rust", "tauri", "svelte"];
        let score = SmartBrowser::calculate_relevance(content, keywords);
        assert!(
            score < f64::EPSILON,
            "No matching keywords should yield zero relevance, got {score}"
        );
    }

    #[test]
    fn test_relevance_empty_inputs() {
        assert!(SmartBrowser::calculate_relevance("", &["rust"]) < f64::EPSILON);
        assert!(SmartBrowser::calculate_relevance("some content", &[]) < f64::EPSILON);
        assert!(SmartBrowser::calculate_relevance("", &[]) < f64::EPSILON);
    }

    #[test]
    fn test_relevance_case_insensitive() {
        let content = "RUST is Great for Systems Programming in Rust";
        let keywords = &["rust"];
        let score = SmartBrowser::calculate_relevance(content, keywords);
        assert!(score > 0.5, "Case-insensitive matching should find RUST/Rust, got {score}");
    }

    // -- PageAnalysis & BrowserResult ----------------------------------------

    #[test]
    fn test_page_analysis_serde() {
        let pa = PageAnalysis {
            url: "https://example.com".into(),
            title: "Example".into(),
            content_hash: "abc123".into(),
            relevance_score: 0.85,
            extracted_data: vec!["data1".into(), "data2".into()],
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&pa).unwrap();
        let back: PageAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(back.url, "https://example.com");
        assert_eq!(back.relevance_score, 0.85);
        assert_eq!(back.extracted_data.len(), 2);
    }

    #[test]
    fn test_browser_result_success_and_failure() {
        let success = BrowserResult {
            pages_visited: 5,
            pages_analyzed: vec![],
            total_duration_ms: 1200,
            success: true,
            error: None,
        };
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = BrowserResult {
            pages_visited: 0,
            pages_analyzed: vec![],
            total_duration_ms: 50,
            success: false,
            error: Some("connection refused".into()),
        };
        assert!(!failure.success);
        assert_eq!(failure.error.as_deref(), Some("connection refused"));
    }
}
