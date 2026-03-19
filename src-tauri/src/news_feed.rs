// SPDX-License-Identifier: Apache-2.0
//! ImpForge News Feed — AI/Dev News Aggregator
//!
//! Fetches news from RSS/Atom feeds relevant to AI development.
//! Supports offline mode with cached built-in content and live feed fetching.
//! Enterprise Bestimmung: Central news intelligence for developer awareness.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ── Offline Cache ────────────────────────────────────────────────────────
// Persists fetched news to disk so the feed works after restart without
// network.  Cache lives in the platform-appropriate data directory.

fn news_cache_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("impforge")
        .join("news_cache.json")
}

fn save_news_cache(items: &[NewsItem]) {
    let cache_path = news_cache_path();
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(items) {
        let _ = std::fs::write(&cache_path, json);
    }
}

fn load_news_cache() -> Vec<NewsItem> {
    let cache_path = news_cache_path();
    std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// A news article from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub date: String,
    pub category: String,
    pub is_sample: bool,
}

/// Feed source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub category: String,
}

/// Result of a news fetch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsFetchResult {
    pub items: Vec<NewsItem>,
    pub sources_checked: usize,
    pub sources_failed: usize,
    pub is_cached: bool,
    pub error: Option<String>,
}

/// Default RSS/Atom feeds for AI development news
fn default_feed_sources() -> Vec<FeedSource> {
    vec![
        FeedSource {
            name: "Hacker News (AI)".into(),
            url: "https://hnrss.org/newest?q=AI+OR+LLM+OR+machine+learning&points=50".into(),
            category: "Industry".into(),
        },
        FeedSource {
            name: "Tauri Blog".into(),
            url: "https://tauri.app/blog/rss.xml".into(),
            category: "Tools".into(),
        },
        FeedSource {
            name: "Svelte Blog".into(),
            url: "https://svelte.dev/blog/rss.xml".into(),
            category: "Tools".into(),
        },
        FeedSource {
            name: "Rust Blog".into(),
            url: "https://blog.rust-lang.org/feed.xml".into(),
            category: "Tools".into(),
        },
        FeedSource {
            name: "Anthropic Research".into(),
            url: "https://www.anthropic.com/feed".into(),
            category: "Research".into(),
        },
        FeedSource {
            name: "Ollama Blog".into(),
            url: "https://ollama.com/blog/rss".into(),
            category: "Models".into(),
        },
    ]
}

/// Built-in sample news items shown when offline or feeds unavailable
fn builtin_sample_items() -> Vec<NewsItem> {
    vec![
        NewsItem {
            id: "sample-1".into(),
            title: "Welcome to ImpForge News".into(),
            summary: "Your AI development news feed. When connected to the internet, this page fetches live articles from curated RSS feeds covering AI models, developer tools, research papers, and industry updates.".into(),
            source: "ImpForge".into(),
            url: String::new(),
            date: "2026-03-09".into(),
            category: "Industry".into(),
            is_sample: true,
        },
        NewsItem {
            id: "sample-2".into(),
            title: "Configure Your Feed Sources".into(),
            summary: "ImpForge monitors Hacker News (AI), Tauri Blog, Svelte Blog, Rust Blog, Anthropic Research, and Ollama Blog by default. Custom feed sources will be configurable in Settings.".into(),
            source: "ImpForge".into(),
            url: String::new(),
            date: "2026-03-09".into(),
            category: "Tools".into(),
            is_sample: true,
        },
    ]
}

/// Parse a simple RSS/Atom XML feed into NewsItems.
/// Uses basic string parsing — no heavy XML crate needed for RSS.
fn parse_feed_xml(xml: &str, source: &FeedSource) -> Vec<NewsItem> {
    let mut items = Vec::new();

    // Try RSS format first (<item> tags)
    let tag = if xml.contains("<item>") || xml.contains("<item ") {
        "item"
    } else if xml.contains("<entry>") || xml.contains("<entry ") {
        "entry"
    } else {
        return items;
    };

    let open_tag = format!("<{tag}");
    let close_tag = format!("</{tag}>");

    for (idx, chunk) in xml.split(&open_tag).skip(1).enumerate() {
        if idx >= 20 {
            break; // Cap at 20 items per feed
        }
        let Some(entry_end) = chunk.find(&close_tag) else {
            continue;
        };
        let entry = &chunk[..entry_end];

        let title = extract_tag_content(entry, "title").unwrap_or_default();
        let link = extract_link(entry);
        let description = extract_tag_content(entry, "description")
            .or_else(|| extract_tag_content(entry, "summary"))
            .or_else(|| extract_tag_content(entry, "content"))
            .unwrap_or_default();
        let date = extract_tag_content(entry, "pubDate")
            .or_else(|| extract_tag_content(entry, "published"))
            .or_else(|| extract_tag_content(entry, "updated"))
            .unwrap_or_default();

        // Strip HTML tags from description
        let summary = strip_html(&description);
        // Truncate to 300 chars
        let summary = if summary.len() > 300 {
            format!("{}...", &summary[..297])
        } else {
            summary
        };

        if !title.is_empty() {
            items.push(NewsItem {
                id: format!("{}-{}", source.name.to_lowercase().replace(' ', "-"), idx),
                title: decode_html_entities(&title),
                summary,
                source: source.name.clone(),
                url: link,
                date: normalize_date(&date),
                category: source.category.clone(),
                is_sample: false,
            });
        }
    }

    items
}

/// Extract text content from an XML tag
fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
    // Handle CDATA
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after_open = &xml[start + open.len()..];
    // Skip attributes until >
    let content_start = after_open.find('>')? + 1;
    let content = &after_open[content_start..];
    let end = content.find(&close)?;
    let text = &content[..end];

    // Handle CDATA sections
    let text = if text.contains("<![CDATA[") {
        text.replace("<![CDATA[", "").replace("]]>", "")
    } else {
        text.to_string()
    };

    Some(text.trim().to_string())
}

/// Extract link from RSS <link> or Atom <link href="">
fn extract_link(entry: &str) -> String {
    // Try Atom-style <link href="..."/>
    if let Some(pos) = entry.find("<link") {
        let after = &entry[pos..];
        if let Some(href_pos) = after.find("href=\"") {
            let url_start = &after[href_pos + 6..];
            if let Some(end) = url_start.find('"') {
                return url_start[..end].to_string();
            }
        }
        // Try RSS-style <link>url</link>
        if let Some(content) = extract_tag_content(entry, "link") {
            if content.starts_with("http") {
                return content;
            }
        }
    }
    String::new()
}

/// Strip HTML tags from text
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    decode_html_entities(&result).trim().to_string()
}

/// Decode common HTML entities
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Normalize date strings to YYYY-MM-DD format
fn normalize_date(date: &str) -> String {
    // Try to parse common RSS date formats and return YYYY-MM-DD
    // RFC 2822: "Mon, 09 Mar 2026 10:00:00 +0000"
    // ISO 8601: "2026-03-09T10:00:00Z"
    if date.len() >= 10 && date.chars().nth(4) == Some('-') {
        // Already ISO-like, take first 10 chars
        return date[..10].to_string();
    }

    // Try RFC 2822 month parsing
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    for (i, month) in months.iter().enumerate() {
        if date.contains(month) {
            // Extract day and year
            let parts: Vec<&str> = date.split_whitespace().collect();
            if parts.len() >= 4 {
                let day = parts.iter().find(|p| p.parse::<u32>().is_ok() && p.len() <= 2);
                let year = parts.iter().find(|p| p.parse::<u32>().is_ok() && p.len() == 4);
                if let (Some(day), Some(year)) = (day, year) {
                    return format!("{}-{:02}-{:0>2}", year, i + 1, day);
                }
            }
        }
    }

    // Fallback: return as-is (truncated)
    if date.len() > 10 {
        date[..10].to_string()
    } else {
        date.to_string()
    }
}

/// Fetch news from all configured feed sources
async fn fetch_feeds(sources: &[FeedSource]) -> NewsFetchResult {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("ImpForge/1.0 (AI Workstation Builder)")
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut all_items = Vec::new();
    let mut sources_failed = 0usize;

    for source in sources {
        match client.get(&source.url).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    let items = parse_feed_xml(&body, source);
                    all_items.extend(items);
                } else {
                    sources_failed += 1;
                }
            }
            Err(_) => {
                sources_failed += 1;
            }
        }
    }

    // Sort by date descending
    all_items.sort_by(|a, b| b.date.cmp(&a.date));

    // Deduplicate by title similarity
    let mut seen_titles = std::collections::HashSet::new();
    all_items.retain(|item| {
        let key = item.title.to_lowercase();
        seen_titles.insert(key)
    });

    // Cap total items
    all_items.truncate(50);

    NewsFetchResult {
        sources_checked: sources.len(),
        sources_failed,
        is_cached: false,
        error: if sources_failed == sources.len() && !sources.is_empty() {
            Some("All feed sources unreachable. Showing sample content.".into())
        } else {
            None
        },
        items: all_items,
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────────

/// Fetch live news from RSS feeds. Falls back to disk cache, then built-in samples.
#[tauri::command]
pub async fn news_fetch() -> Result<NewsFetchResult, String> {
    let sources = default_feed_sources();
    let mut result = fetch_feeds(&sources).await;

    if !result.items.is_empty() {
        // Successfully fetched live news -- persist to disk cache
        save_news_cache(&result.items);
    } else {
        // No live items -- try loading from disk cache
        let cached = load_news_cache();
        if !cached.is_empty() {
            result.items = cached;
            result.is_cached = true;
        } else {
            // No cache either -- fall back to built-in samples
            result.items = builtin_sample_items();
            result.is_cached = true;
        }
    }

    Ok(result)
}

/// Get the list of configured feed sources
#[tauri::command]
pub fn news_sources() -> Vec<FeedSource> {
    default_feed_sources()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<p>Hello <b>world</b></p>"), "Hello world");
        assert_eq!(strip_html("No tags here"), "No tags here");
    }

    #[test]
    fn test_decode_html_entities() {
        assert_eq!(decode_html_entities("A &amp; B"), "A & B");
        assert_eq!(decode_html_entities("&lt;tag&gt;"), "<tag>");
    }

    #[test]
    fn test_normalize_date_iso() {
        assert_eq!(normalize_date("2026-03-09T10:00:00Z"), "2026-03-09");
    }

    #[test]
    fn test_normalize_date_rfc2822() {
        assert_eq!(
            normalize_date("Mon, 09 Mar 2026 10:00:00 +0000"),
            "2026-03-09"
        );
    }

    #[test]
    fn test_parse_rss_feed() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0">
        <channel>
        <title>Test Feed</title>
        <item>
            <title>Test Article</title>
            <link>https://example.com/1</link>
            <description>This is a test article</description>
            <pubDate>Mon, 09 Mar 2026 10:00:00 +0000</pubDate>
        </item>
        </channel>
        </rss>"#;
        let source = FeedSource {
            name: "Test".into(),
            url: "https://example.com/rss".into(),
            category: "Tools".into(),
        };
        let items = parse_feed_xml(xml, &source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test Article");
        assert_eq!(items[0].url, "https://example.com/1");
        assert_eq!(items[0].category, "Tools");
        assert!(!items[0].is_sample);
    }

    #[test]
    fn test_parse_atom_feed() {
        let xml = r#"<?xml version="1.0"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
        <title>Test Atom</title>
        <entry>
            <title>Atom Entry</title>
            <link href="https://example.com/atom/1"/>
            <summary>Atom summary text</summary>
            <updated>2026-03-08T12:00:00Z</updated>
        </entry>
        </feed>"#;
        let source = FeedSource {
            name: "AtomTest".into(),
            url: "https://example.com/atom".into(),
            category: "Research".into(),
        };
        let items = parse_feed_xml(xml, &source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Atom Entry");
        assert_eq!(items[0].url, "https://example.com/atom/1");
        assert_eq!(items[0].date, "2026-03-08");
    }

    #[test]
    fn test_builtin_samples() {
        let samples = builtin_sample_items();
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|s| s.is_sample));
    }
}
