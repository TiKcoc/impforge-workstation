//! CDP DevTools — Console output, Performance metrics, Cookie management
//!
//! Uses Chrome DevTools Protocol domains:
//! - Runtime.consoleAPICalled for console.log/warn/error capture
//! - Performance API (via JS) for timing data
//! - document.cookie for cookie management
//!
//! License: MIT/Apache-2.0

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// CONSOLE
// ============================================================================

/// A single console log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String, // log, warn, error, info, debug
    pub text: String,
    pub timestamp: f64,
    pub source: String, // javascript, network, security, etc.
    pub line_number: Option<u32>,
    pub url: Option<String>,
}

/// Ring buffer for console entries (max 200)
static CONSOLE_LOG: once_cell::sync::Lazy<Arc<Mutex<Vec<ConsoleEntry>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::with_capacity(200))));

/// Push a console entry to the buffer
pub async fn push_console(entry: ConsoleEntry) {
    let mut log = CONSOLE_LOG.lock().await;
    if log.len() >= 200 {
        log.remove(0);
    }
    log.push(entry);
}

// ============================================================================
// PERFORMANCE
// ============================================================================

/// Page performance metrics (via JS Performance API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfMetrics {
    pub dom_content_loaded_ms: Option<f64>,
    pub load_event_ms: Option<f64>,
    pub first_paint_ms: Option<f64>,
    pub first_contentful_paint_ms: Option<f64>,
    pub dom_nodes: Option<u64>,
    pub js_heap_size_mb: Option<f64>,
    pub timestamp: f64,
}

// ============================================================================
// COOKIES
// ============================================================================

/// A browser cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires: Option<f64>,
    pub same_site: Option<String>,
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Get console log entries
#[tauri::command]
pub async fn cdp_console_entries() -> Result<Vec<ConsoleEntry>, String> {
    Ok(CONSOLE_LOG.lock().await.clone())
}

/// Clear console log
#[tauri::command]
pub async fn cdp_console_clear() -> Result<String, String> {
    CONSOLE_LOG.lock().await.clear();
    Ok("Console cleared".into())
}

/// Inject console interceptor into a CDP page (captures console.log/warn/error)
#[tauri::command]
pub async fn cdp_console_enable(page_id: String) -> Result<String, String> {
    // Inject a console override that sends entries back via a special marker
    let script = r#"(() => {
        if (window.__impforge_console_hooked) return 'already hooked';
        window.__impforge_console_hooked = true;
        window.__impforge_console_buffer = [];
        const orig = {};
        ['log','warn','error','info','debug'].forEach(level => {
            orig[level] = console[level];
            console[level] = function(...args) {
                window.__impforge_console_buffer.push({
                    level,
                    text: args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' '),
                    timestamp: Date.now() / 1000,
                    source: 'javascript',
                    line_number: null,
                    url: null,
                });
                if (window.__impforge_console_buffer.length > 100) {
                    window.__impforge_console_buffer.shift();
                }
                orig[level].apply(console, args);
            };
        });
        return 'console hooked';
    })()"#;
    let result = crate::cdp_engine::cdp_eval_js(&page_id, script).await?;
    Ok(result.as_str().unwrap_or("done").to_string())
}

/// Flush console buffer from page into our Rust ring buffer
#[tauri::command]
pub async fn cdp_console_flush(page_id: String) -> Result<Vec<ConsoleEntry>, String> {
    let script = r#"(() => {
        const buf = window.__impforge_console_buffer || [];
        window.__impforge_console_buffer = [];
        return buf;
    })()"#;
    let result = crate::cdp_engine::cdp_eval_js(&page_id, script).await?;
    let entries: Vec<ConsoleEntry> =
        serde_json::from_value(result).map_err(|e| format!("Console parse error: {e}"))?;

    // Push into ring buffer
    for entry in &entries {
        push_console(entry.clone()).await;
    }

    Ok(entries)
}

/// Get performance metrics for a page (via JS Performance API)
#[tauri::command]
pub async fn cdp_perf_metrics(page_id: String) -> Result<PerfMetrics, String> {
    let script = r#"(() => {
        const nav = performance.getEntriesByType('navigation')[0] || {};
        const paint = performance.getEntriesByType('paint');
        const fp = paint.find(p => p.name === 'first-paint');
        const fcp = paint.find(p => p.name === 'first-contentful-paint');
        return {
            dom_content_loaded_ms: nav.domContentLoadedEventEnd || null,
            load_event_ms: nav.loadEventEnd || null,
            first_paint_ms: fp ? fp.startTime : null,
            first_contentful_paint_ms: fcp ? fcp.startTime : null,
            dom_nodes: document.querySelectorAll('*').length,
            js_heap_size_mb: performance.memory ? performance.memory.usedJSHeapSize / 1048576 : null,
            timestamp: Date.now() / 1000
        };
    })()"#;

    let result = crate::cdp_engine::cdp_eval_js(&page_id, script).await?;
    serde_json::from_value(result).map_err(|e| format!("Perf parse error: {e}"))
}

/// Get cookies for a page's domain
#[tauri::command]
pub async fn cdp_get_cookies(page_id: String) -> Result<Vec<CdpCookie>, String> {
    let script = r#"document.cookie.split('; ').filter(c => c).map(c => {
        const [name, ...rest] = c.split('=');
        return {
            name: name,
            value: rest.join('='),
            domain: location.hostname,
            path: '/',
            secure: location.protocol === 'https:',
            http_only: false,
            expires: null,
            same_site: null
        };
    })"#;

    let result = crate::cdp_engine::cdp_eval_js(&page_id, script).await?;
    serde_json::from_value(result).map_err(|e| format!("Cookie parse error: {e}"))
}

/// Delete a cookie by name
#[tauri::command]
pub async fn cdp_delete_cookie(page_id: String, name: String) -> Result<String, String> {
    let script = format!(
        "document.cookie = '{name}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/'"
    );
    crate::cdp_engine::cdp_eval_js(&page_id, &script).await?;
    Ok(format!("Cookie '{name}' deleted"))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_entry_serialization() {
        let entry = ConsoleEntry {
            level: "log".into(),
            text: "Hello world".into(),
            timestamp: 1000.0,
            source: "javascript".into(),
            line_number: Some(42),
            url: Some("https://example.com/app.js".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: ConsoleEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.level, "log");
        assert_eq!(parsed.line_number, Some(42));
        assert_eq!(parsed.text, "Hello world");
    }

    #[test]
    fn test_console_entry_no_optionals() {
        let entry = ConsoleEntry {
            level: "error".into(),
            text: "Something failed".into(),
            timestamp: 2000.0,
            source: "network".into(),
            line_number: None,
            url: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("network"));
    }

    #[test]
    fn test_perf_metrics_serialization() {
        let metrics = PerfMetrics {
            dom_content_loaded_ms: Some(123.4),
            load_event_ms: Some(456.7),
            first_paint_ms: Some(89.0),
            first_contentful_paint_ms: Some(112.0),
            dom_nodes: Some(450),
            js_heap_size_mb: Some(12.5),
            timestamp: 1000.0,
        };
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("123.4"));
        let parsed: PerfMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.dom_nodes, Some(450));
    }

    #[test]
    fn test_perf_metrics_empty() {
        let metrics = PerfMetrics {
            dom_content_loaded_ms: None,
            load_event_ms: None,
            first_paint_ms: None,
            first_contentful_paint_ms: None,
            dom_nodes: None,
            js_heap_size_mb: None,
            timestamp: 0.0,
        };
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("null"));
    }

    #[test]
    fn test_cookie_serialization() {
        let cookie = CdpCookie {
            name: "session".into(),
            value: "abc123".into(),
            domain: "example.com".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            expires: Some(1700000000.0),
            same_site: Some("Lax".into()),
        };
        let json = serde_json::to_string(&cookie).unwrap();
        assert!(json.contains("session"));
        assert!(json.contains("abc123"));
        let parsed: CdpCookie = serde_json::from_str(&json).unwrap();
        assert!(parsed.secure);
        assert_eq!(parsed.same_site.as_deref(), Some("Lax"));
    }

    #[test]
    fn test_cookie_minimal() {
        let cookie = CdpCookie {
            name: "test".into(),
            value: "1".into(),
            domain: "localhost".into(),
            path: "/".into(),
            secure: false,
            http_only: false,
            expires: None,
            same_site: None,
        };
        assert!(!cookie.secure);
        assert!(cookie.expires.is_none());
    }

    #[tokio::test]
    async fn test_push_console_ring_buffer() {
        // Clear
        CONSOLE_LOG.lock().await.clear();

        for i in 0..210 {
            push_console(ConsoleEntry {
                level: "log".into(),
                text: format!("msg {i}"),
                timestamp: i as f64,
                source: "test".into(),
                line_number: None,
                url: None,
            })
            .await;
        }

        let log = CONSOLE_LOG.lock().await;
        // Ring buffer max 200
        assert_eq!(log.len(), 200);
        // First entry should be msg 10 (0-9 were pushed out)
        assert_eq!(log[0].text, "msg 10");
    }
}
