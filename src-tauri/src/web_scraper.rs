//! NEXUS Web Scraper — Built-in web content extraction
//!
//! Provides Firecrawl-like scraping capabilities using MIT-licensed crates.
//! Customers get this built-in with their Nexus purchase — no external API needed.
//!
//! Features:
//! - Scrape any URL to clean markdown
//! - CSS selector-based content extraction
//! - Batch scraping (multiple URLs)
//! - Metadata extraction (title, description, links)
//! - Optional Firecrawl Cloud API integration (customer brings own key)

use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ============================================================================
// TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeResult {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub markdown: String,
    pub links: Vec<String>,
    pub word_count: usize,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeOptions {
    /// CSS selectors to extract specific content (e.g. "article", "main", ".content")
    pub selectors: Option<Vec<String>>,
    /// Remove these selectors before extraction (e.g. "nav", "footer", ".ads")
    pub remove_selectors: Option<Vec<String>>,
    /// Request timeout in seconds (default: 30)
    pub timeout_secs: Option<u64>,
    /// Include links in output
    pub include_links: Option<bool>,
    /// Use Firecrawl Cloud API instead of built-in scraper (requires API key)
    pub use_firecrawl: Option<bool>,
    /// Firecrawl API key (customer's own key)
    pub firecrawl_api_key: Option<String>,
}

impl Default for ScrapeOptions {
    fn default() -> Self {
        Self {
            selectors: None,
            remove_selectors: Some(vec![
                "nav".into(),
                "footer".into(),
                "header".into(),
                "script".into(),
                "style".into(),
                "noscript".into(),
                ".cookie-banner".into(),
                ".advertisement".into(),
                "#cookie-consent".into(),
            ]),
            timeout_secs: Some(30),
            include_links: Some(true),
            use_firecrawl: Some(false),
            firecrawl_api_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchScrapeResult {
    pub results: Vec<ScrapeResult>,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

// ============================================================================
// BUILT-IN SCRAPER (MIT — no external dependency)
// ============================================================================

/// Create a configured HTTP client with browser-like headers
fn build_client(timeout_secs: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Scrape a single URL and return clean markdown content
pub async fn scrape_url(url: &str, options: &ScrapeOptions) -> ScrapeResult {
    let timeout = options.timeout_secs.unwrap_or(30);

    // Use Firecrawl Cloud API if requested
    if options.use_firecrawl.unwrap_or(false) {
        if let Some(ref api_key) = options.firecrawl_api_key {
            return scrape_via_firecrawl(url, api_key, timeout).await;
        }
        return ScrapeResult {
            url: url.to_string(),
            title: None,
            description: None,
            markdown: String::new(),
            links: vec![],
            word_count: 0,
            success: false,
            error: Some("Firecrawl API key required. Add your key in Settings > API Keys.".into()),
        };
    }

    // Built-in scraper
    let client = match build_client(timeout) {
        Ok(c) => c,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(e),
            };
        }
    };

    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(format!("Request failed: {e}")),
            };
        }
    };

    let status = response.status();
    if !status.is_success() {
        return ScrapeResult {
            url: url.to_string(),
            title: None,
            description: None,
            markdown: String::new(),
            links: vec![],
            word_count: 0,
            success: false,
            error: Some(format!("HTTP {status}")),
        };
    }

    let html_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(format!("Failed to read response: {e}")),
            };
        }
    };

    parse_html(url, &html_text, options)
}

/// Parse HTML content and extract structured data
fn parse_html(url: &str, html_text: &str, options: &ScrapeOptions) -> ScrapeResult {
    let document = Html::parse_document(html_text);

    // Extract title
    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string());

    // Extract meta description
    let description = Selector::parse("meta[name=\"description\"]")
        .ok()
        .and_then(|sel| {
            document
                .select(&sel)
                .next()
                .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
        });

    // Extract links
    let links = if options.include_links.unwrap_or(true) {
        extract_links(&document, url)
    } else {
        vec![]
    };

    // Determine which HTML to convert
    let target_html = if let Some(ref selectors) = options.selectors {
        // Extract only content matching the given selectors
        let mut parts = Vec::new();
        for sel_str in selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                for el in document.select(&sel) {
                    parts.push(el.html());
                }
            }
        }
        if parts.is_empty() {
            // Fallback to body if selectors matched nothing
            extract_body_html(&document)
        } else {
            parts.join("\n")
        }
    } else {
        extract_body_html(&document)
    };

    // Remove unwanted elements by re-parsing with removals
    let clean_html = if let Some(ref remove) = options.remove_selectors {
        remove_elements(&target_html, remove)
    } else {
        target_html
    };

    // Convert to markdown
    let markdown = html2md::rewrite_html(&clean_html, false);

    // Clean up excessive whitespace
    let markdown = collapse_blank_lines(&markdown);
    let word_count = markdown.split_whitespace().count();

    ScrapeResult {
        url: url.to_string(),
        title,
        description,
        markdown,
        links,
        word_count,
        success: true,
        error: None,
    }
}

/// Extract body HTML from document
fn extract_body_html(document: &Html) -> String {
    Selector::parse("body")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|el| el.html())
        .unwrap_or_default()
}

/// Extract all links from a page, resolving relative URLs
fn extract_links(document: &Html, base_url: &str) -> Vec<String> {
    let base = url::Url::parse(base_url).ok();
    let mut links = Vec::new();

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let resolved = if let Some(ref base) = base {
                    base.join(href)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| href.to_string())
                } else {
                    href.to_string()
                };

                // Skip anchors, javascript, mailto
                if !resolved.starts_with('#')
                    && !resolved.starts_with("javascript:")
                    && !resolved.starts_with("mailto:")
                {
                    links.push(resolved);
                }
            }
        }
    }

    links.sort();
    links.dedup();
    links
}

/// Remove elements matching CSS selectors from HTML
fn remove_elements(html: &str, selectors: &[String]) -> String {
    let doc = Html::parse_fragment(html);

    // Collect the HTML of elements to remove
    let mut to_remove = Vec::new();
    for sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in doc.select(&sel) {
                to_remove.push(el.html());
            }
        }
    }

    if to_remove.is_empty() {
        return html.to_string();
    }

    // Remove matched element HTML from source
    let mut result = html.to_string();
    for fragment in &to_remove {
        result = result.replace(fragment, "");
    }
    result
}

/// Clean up markdown output — collapse blank lines, trim
fn collapse_blank_lines(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut blank_count = 0;

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

// ============================================================================
// FIRECRAWL CLOUD API (optional — customer brings own key)
// ============================================================================

/// Scrape using Firecrawl Cloud API (customer's API key, no AGPL bundling)
async fn scrape_via_firecrawl(url: &str, api_key: &str, timeout: u64) -> ScrapeResult {
    let client = match build_client(timeout) {
        Ok(c) => c,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(e),
            };
        }
    };

    let body = serde_json::json!({
        "url": url,
        "formats": ["markdown"],
        "onlyMainContent": true,
    });

    let response = match client
        .post("https://api.firecrawl.dev/v1/scrape")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(format!("Firecrawl API request failed: {e}")),
            };
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            return ScrapeResult {
                url: url.to_string(),
                title: None,
                description: None,
                markdown: String::new(),
                links: vec![],
                word_count: 0,
                success: false,
                error: Some(format!("Failed to parse Firecrawl response: {e}")),
            };
        }
    };

    let data = &json["data"];
    let markdown = data["markdown"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let title = data["metadata"]["title"].as_str().map(|s| s.to_string());
    let description = data["metadata"]["description"]
        .as_str()
        .map(|s| s.to_string());
    let links = data["links"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let word_count = markdown.split_whitespace().count();

    ScrapeResult {
        url: url.to_string(),
        title,
        description,
        markdown,
        links,
        word_count,
        success: json["success"].as_bool().unwrap_or(false),
        error: json["error"].as_str().map(|s| s.to_string()),
    }
}

// ============================================================================
// BATCH SCRAPING
// ============================================================================

/// Scrape multiple URLs concurrently
pub async fn scrape_batch(urls: &[String], options: &ScrapeOptions) -> BatchScrapeResult {
    let futures: Vec<_> = urls
        .iter()
        .map(|url| {
            let opts = options.clone();
            let url = url.clone();
            async move { scrape_url(&url, &opts).await }
        })
        .collect();

    let results = futures_util::future::join_all(futures).await;
    let total = results.len();
    let succeeded = results.iter().filter(|r| r.success).count();

    BatchScrapeResult {
        results,
        total,
        succeeded,
        failed: total - succeeded,
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Scrape a URL and return markdown content
#[tauri::command]
pub async fn web_scrape(url: String, options: Option<ScrapeOptions>) -> Result<ScrapeResult, String> {
    let opts = options.unwrap_or_default();
    Ok(scrape_url(&url, &opts).await)
}

/// Scrape multiple URLs concurrently
#[tauri::command]
pub async fn web_scrape_batch(
    urls: Vec<String>,
    options: Option<ScrapeOptions>,
) -> Result<BatchScrapeResult, String> {
    if urls.len() > 50 {
        return Err("Maximum 50 URLs per batch".to_string());
    }
    let opts = options.unwrap_or_default();
    Ok(scrape_batch(&urls, &opts).await)
}

/// Extract metadata only (title, description, links) without full content
#[tauri::command]
pub async fn web_extract_metadata(url: String) -> Result<ScrapeResult, String> {
    let opts = ScrapeOptions {
        selectors: Some(vec!["head".into()]),
        include_links: Some(true),
        ..Default::default()
    };
    Ok(scrape_url(&url, &opts).await)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_basic() {
        let html = r#"
        <html>
        <head><title>Test Page</title>
        <meta name="description" content="A test page">
        </head>
        <body>
        <h1>Hello World</h1>
        <p>This is a test paragraph with <a href="/about">a link</a>.</p>
        </body>
        </html>"#;

        let result = parse_html("https://example.com", html, &ScrapeOptions::default());
        assert!(result.success);
        assert_eq!(result.title.as_deref(), Some("Test Page"));
        assert_eq!(result.description.as_deref(), Some("A test page"));
        assert!(result.markdown.contains("Hello World"));
        assert!(result.markdown.contains("test paragraph"));
        assert!(result.word_count > 0);
    }

    #[test]
    fn test_parse_html_with_selectors() {
        let html = r#"
        <html><body>
        <nav>Navigation stuff</nav>
        <article><h2>Article Title</h2><p>Article content here.</p></article>
        <footer>Footer stuff</footer>
        </body></html>"#;

        let opts = ScrapeOptions {
            selectors: Some(vec!["article".into()]),
            remove_selectors: None,
            ..Default::default()
        };
        let result = parse_html("https://example.com", html, &opts);
        assert!(result.success);
        assert!(result.markdown.contains("Article Title"));
        assert!(result.markdown.contains("Article content"));
        // nav/footer should not appear when using article selector
        assert!(!result.markdown.contains("Navigation stuff"));
        assert!(!result.markdown.contains("Footer stuff"));
    }

    #[test]
    fn test_parse_html_remove_selectors() {
        let html = r#"
        <html><body>
        <nav>Remove me</nav>
        <main><p>Keep this content.</p></main>
        <footer>Remove me too</footer>
        </body></html>"#;

        let opts = ScrapeOptions {
            remove_selectors: Some(vec!["nav".into(), "footer".into()]),
            ..Default::default()
        };
        let result = parse_html("https://example.com", html, &opts);
        assert!(result.success);
        assert!(result.markdown.contains("Keep this content"));
        assert!(!result.markdown.contains("Remove me"));
    }

    #[test]
    fn test_extract_links() {
        let html = r##"
        <html><body>
        <a href="/about">About</a>
        <a href="https://other.com/page">Other</a>
        <a href="#section">Anchor</a>
        <a href="javascript:void(0)">JS Link</a>
        </body></html>"##;

        let doc = Html::parse_document(html);
        let links = extract_links(&doc, "https://example.com");
        assert!(links.contains(&"https://example.com/about".to_string()));
        assert!(links.contains(&"https://other.com/page".to_string()));
        // Anchors and javascript should be excluded
        assert!(!links.iter().any(|l| l.starts_with('#')));
        assert!(!links.iter().any(|l| l.starts_with("javascript:")));
    }

    #[test]
    fn test_collapse_blank_lines() {
        let messy = "Hello\n\n\n\n\n\nWorld\n\n\nEnd";
        let clean = collapse_blank_lines(messy);
        // Should collapse excessive blank lines
        assert!(!clean.contains("\n\n\n\n"));
        assert!(clean.contains("Hello"));
        assert!(clean.contains("World"));
    }

    #[test]
    fn test_default_options() {
        let opts = ScrapeOptions::default();
        assert_eq!(opts.timeout_secs, Some(30));
        assert_eq!(opts.include_links, Some(true));
        assert_eq!(opts.use_firecrawl, Some(false));
        assert!(opts.remove_selectors.is_some());
        let remove = opts.remove_selectors.unwrap();
        assert!(remove.contains(&"nav".to_string()));
        assert!(remove.contains(&"script".to_string()));
    }

    #[tokio::test]
    async fn test_batch_limit() {
        let urls: Vec<String> = (0..51).map(|i| format!("https://example.com/{i}")).collect();
        let result = web_scrape_batch(urls, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Maximum 50"));
    }
}
