//! CDP Network Monitor — Captures HTTP request waterfall via Chrome DevTools Protocol
//!
//! Subscribes to Network.requestWillBeSent and Network.responseReceived events
//! to build a request waterfall. Data stored in a ring buffer (max 500 entries).
//!
//! License: MIT/Apache-2.0 (chromiumoxide is MIT/Apache-2.0)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A single network request/response pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub status: Option<u16>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub duration_ms: Option<f64>,
    pub timestamp: f64,
    pub resource_type: String,
}

/// Ring buffer for network entries (max 500)
static NETWORK_LOG: once_cell::sync::Lazy<Arc<Mutex<Vec<NetworkEntry>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::with_capacity(500))));

/// Pending requests (waiting for response)
static PENDING: once_cell::sync::Lazy<Arc<Mutex<HashMap<String, NetworkEntry>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Record a request start (called from CDP event handler)
pub async fn record_request(
    request_id: String,
    url: String,
    method: String,
    resource_type: String,
    timestamp: f64,
) {
    let entry = NetworkEntry {
        request_id: request_id.clone(),
        url,
        method,
        status: None,
        mime_type: None,
        size_bytes: None,
        duration_ms: None,
        timestamp,
        resource_type,
    };
    PENDING.lock().await.insert(request_id, entry);
}

/// Record a response (matches pending request, calculates duration)
pub async fn record_response(
    request_id: &str,
    status: u16,
    mime_type: String,
    size_bytes: Option<u64>,
    timestamp: f64,
) {
    let mut pending = PENDING.lock().await;
    if let Some(mut entry) = pending.remove(request_id) {
        entry.status = Some(status);
        entry.mime_type = Some(mime_type);
        entry.size_bytes = size_bytes;
        entry.duration_ms = Some((timestamp - entry.timestamp) * 1000.0);

        let mut log = NETWORK_LOG.lock().await;
        if log.len() >= 500 {
            log.remove(0);
        }
        log.push(entry);
    }
}

/// Enable network monitoring on a CDP page via Network.enable
pub async fn enable_network_capture(page_id: &str) -> Result<(), String> {
    let state = crate::cdp_engine::get_cdp_state().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    // Enable the CDP Network domain on this page
    use chromiumoxide::cdp::browser_protocol::network::EnableParams;
    page.execute(EnableParams::default())
        .await
        .map_err(|e| format!("Network.enable failed: {e}"))?;

    Ok(())
}

/// Get current network log entries
pub async fn get_entries(since_timestamp: Option<f64>) -> Vec<NetworkEntry> {
    let log = NETWORK_LOG.lock().await;
    match since_timestamp {
        Some(ts) => log.iter().filter(|e| e.timestamp > ts).cloned().collect(),
        None => log.clone(),
    }
}

/// Clear the network log
pub async fn clear_entries() {
    NETWORK_LOG.lock().await.clear();
    PENDING.lock().await.clear();
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Get network waterfall entries
#[tauri::command]
pub async fn cdp_network_entries(
    since_timestamp: Option<f64>,
) -> Result<Vec<NetworkEntry>, String> {
    Ok(get_entries(since_timestamp).await)
}

/// Enable network capture on a CDP page
#[tauri::command]
pub async fn cdp_network_enable(page_id: String) -> Result<String, String> {
    enable_network_capture(&page_id).await?;
    Ok("Network monitoring enabled".into())
}

/// Record a network request/response pair (used by CDP event bridge)
#[tauri::command]
pub async fn cdp_network_record(
    request_id: String,
    url: String,
    method: String,
    resource_type: String,
    status: Option<u16>,
    mime_type: Option<String>,
    size_bytes: Option<u64>,
    duration_ms: Option<f64>,
) -> Result<String, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    record_request(request_id.clone(), url, method, resource_type, ts).await;

    if let Some(s) = status {
        record_response(
            &request_id,
            s,
            mime_type.unwrap_or_default(),
            size_bytes,
            ts + duration_ms.unwrap_or(0.0) / 1000.0,
        ).await;
    }

    Ok("Recorded".into())
}

/// Clear network log
#[tauri::command]
pub async fn cdp_network_clear() -> Result<String, String> {
    clear_entries().await;
    Ok("Network log cleared".into())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_entry_serialization() {
        let entry = NetworkEntry {
            request_id: "req-1".into(),
            url: "https://example.com/api".into(),
            method: "GET".into(),
            status: Some(200),
            mime_type: Some("application/json".into()),
            size_bytes: Some(1234),
            duration_ms: Some(56.7),
            timestamp: 1000.0,
            resource_type: "Fetch".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("example.com"));
        assert!(json.contains("200"));
        let parsed: NetworkEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.size_bytes, Some(1234));
    }

    #[test]
    fn test_network_entry_defaults() {
        let entry = NetworkEntry {
            request_id: "r1".into(),
            url: "https://test.com".into(),
            method: "POST".into(),
            status: None,
            mime_type: None,
            size_bytes: None,
            duration_ms: None,
            timestamp: 0.0,
            resource_type: "XHR".into(),
        };
        assert!(entry.status.is_none());
        assert!(entry.duration_ms.is_none());
        assert_eq!(entry.resource_type, "XHR");
    }

    #[tokio::test]
    async fn test_record_get_and_clear_entries() {
        // Single test to avoid race conditions on global state.
        clear_entries().await;

        record_request(
            "test-req-1".into(),
            "https://example.com".into(),
            "GET".into(),
            "Document".into(),
            100.0,
        )
        .await;

        record_response("test-req-1", 200, "text/html".into(), Some(5000), 100.5)
            .await;

        let entries = get_entries(None).await;
        let found = entries.iter().find(|e| e.request_id == "test-req-1");
        assert!(found.is_some());
        let entry = found.unwrap();
        assert_eq!(entry.status, Some(200));
        assert_eq!(entry.size_bytes, Some(5000));
        assert!((entry.duration_ms.unwrap() - 500.0).abs() < 0.01);

        // Verify clear works
        clear_entries().await;
        let entries = get_entries(None).await;
        assert!(entries.is_empty());
    }
}
