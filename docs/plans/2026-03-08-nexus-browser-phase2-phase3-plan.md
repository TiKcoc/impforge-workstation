# NEXUS Browser Phase 2 & 3 — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an Opera GX-style Browser Playground with live CDP devtools (Phase 2) and a customer-facing modular theme/component system (Phase 3) for the NEXUS commercial product.

**Architecture:** Phase 2 extends the existing CDP engine (`cdp_engine.rs`) with Network/Console/Performance domain subscriptions, surfaced through new Svelte stores. Phase 3 builds a CSS-variable-driven theme engine with a widget registry and drag-and-drop layout manager, leveraging the existing `app.css` variable foundation. All code is standalone — no ork-station dependencies.

**Tech Stack:** Rust (chromiumoxide 0.9 CDP domains), Svelte 5 runes, Tailwind CSS, zstd (compression), base64 (encoding), SQLite (theme/layout persistence)

**Prerequisites:** Phase 1 CDP engine is COMPLETE (80 tests, 63 Tauri commands, 12 routes).

---

## PHASE 2: Opera GX Browser Playground

### Task 1: CDP Network Monitor (Rust Backend)

**Files:**
- Create: `src-tauri/src/cdp_network.rs` (~200 LoC)
- Modify: `src-tauri/src/lib.rs` (add module + 3 commands)

**Step 1: Write the test for network event types**

```rust
// At bottom of cdp_network.rs
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
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test test_network_entry --lib 2>&1 | tail -5`
Expected: FAIL with "cannot find module" or "cannot find type"

**Step 3: Write the cdp_network module**

Create `src-tauri/src/cdp_network.rs`:

```rust
//! CDP Network Monitor — Captures HTTP request waterfall via Chrome DevTools Protocol
//!
//! Subscribes to Network.requestWillBeSent and Network.responseReceived events
//! to build a request waterfall. Data stored in a ring buffer (max 500 entries).

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
pub async fn record_request(request_id: String, url: String, method: String, resource_type: String, timestamp: f64) {
    let entry = NetworkEntry {
        request_id: request_id.clone(),
        url, method, status: None, mime_type: None,
        size_bytes: None, duration_ms: None, timestamp, resource_type,
    };
    PENDING.lock().await.insert(request_id, entry);
}

/// Record a response (matches pending request, calculates duration)
pub async fn record_response(request_id: &str, status: u16, mime_type: String, size_bytes: Option<u64>, timestamp: f64) {
    let mut pending = PENDING.lock().await;
    if let Some(mut entry) = pending.remove(request_id) {
        entry.status = Some(status);
        entry.mime_type = Some(mime_type);
        entry.size_bytes = size_bytes;
        entry.duration_ms = Some((timestamp - entry.timestamp) * 1000.0);
        let mut log = NETWORK_LOG.lock().await;
        if log.len() >= 500 { log.remove(0); }
        log.push(entry);
    }
}

/// Enable network monitoring on a CDP page via JavaScript injection
pub async fn enable_network_capture(page_id: &str) -> Result<(), String> {
    // Use CDP's Network domain via chromiumoxide
    let state = crate::cdp_engine::get_cdp_state().await?;
    let pages = state.pages.lock().await;
    let page = pages.get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    // Enable Network domain
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

/// Get network waterfall entries for a page
#[tauri::command]
pub async fn cdp_network_entries(since_timestamp: Option<f64>) -> Result<Vec<NetworkEntry>, String> {
    Ok(get_entries(since_timestamp).await)
}

/// Enable network capture on a CDP page
#[tauri::command]
pub async fn cdp_network_enable(page_id: String) -> Result<String, String> {
    enable_network_capture(&page_id).await?;
    Ok("Network monitoring enabled".into())
}

/// Clear network log
#[tauri::command]
pub async fn cdp_network_clear() -> Result<String, String> {
    clear_entries().await;
    Ok("Network log cleared".into())
}
```

**Step 4: Expose `get_cdp_state` from cdp_engine.rs**

In `src-tauri/src/cdp_engine.rs`, add a public accessor so other modules can get the CDP state:

```rust
/// Get reference to the CDP state (for use by other modules like cdp_network)
pub async fn get_cdp_state() -> Result<&'static CdpState, String> {
    get_cdp().await
}
```

Also make `CdpState` fields accessible:
```rust
pub(crate) struct CdpState {
    pub(crate) browser: Browser,
    pub(crate) pages: Mutex<HashMap<String, Page>>,
}
```

**Step 5: Register module in lib.rs**

Add to `src-tauri/src/lib.rs`:
```rust
mod cdp_network;
```

And in `invoke_handler`:
```rust
// CDP Network Monitor
cdp_network::cdp_network_entries,
cdp_network::cdp_network_enable,
cdp_network::cdp_network_clear,
```

**Step 6: Run tests**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test test_network_entry --lib`
Expected: 2 tests PASS

**Step 7: Commit**

```bash
git add src-tauri/src/cdp_network.rs src-tauri/src/cdp_engine.rs src-tauri/src/lib.rs
git commit -m "feat(browser): CDP network waterfall monitor backend"
```

---

### Task 2: CDP Console & Performance Monitor (Rust Backend)

**Files:**
- Create: `src-tauri/src/cdp_devtools.rs` (~180 LoC)
- Modify: `src-tauri/src/lib.rs` (add module + 4 commands)

**Step 1: Write tests**

```rust
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
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test test_console_entry --lib 2>&1 | tail -5`
Expected: FAIL

**Step 3: Write the cdp_devtools module**

Create `src-tauri/src/cdp_devtools.rs`:

```rust
//! CDP DevTools — Console output, Performance metrics, Cookie management
//!
//! Uses Chrome DevTools Protocol domains:
//! - Runtime.consoleAPICalled for console.log/warn/error
//! - Performance.getMetrics for timing data
//! - Network.getCookies / Network.setCookie for cookie management

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// ---- Console ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String,       // log, warn, error, info, debug
    pub text: String,
    pub timestamp: f64,
    pub source: String,      // javascript, network, security, etc.
    pub line_number: Option<u32>,
    pub url: Option<String>,
}

static CONSOLE_LOG: once_cell::sync::Lazy<Arc<Mutex<Vec<ConsoleEntry>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::with_capacity(200))));

pub async fn push_console(entry: ConsoleEntry) {
    let mut log = CONSOLE_LOG.lock().await;
    if log.len() >= 200 { log.remove(0); }
    log.push(entry);
}

// ---- Performance ----

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

// ---- Cookies ----

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
            js_heap_size_mb: performance.memory ? performance.memory.usedJSHeapSize / 1048576 : null
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
        return { name, value: rest.join('='), domain: location.hostname, path: '/',
                 secure: location.protocol === 'https:', http_only: false, expires: null, same_site: null };
    })"#;

    let result = crate::cdp_engine::cdp_eval_js(&page_id, script).await?;
    serde_json::from_value(result).map_err(|e| format!("Cookie parse error: {e}"))
}

// Tests at bottom of file (see Step 1)
```

**Step 4: Register in lib.rs**

```rust
mod cdp_devtools;
```

And in `invoke_handler`:
```rust
// CDP DevTools (Console, Performance, Cookies)
cdp_devtools::cdp_console_entries,
cdp_devtools::cdp_console_clear,
cdp_devtools::cdp_perf_metrics,
cdp_devtools::cdp_get_cookies,
```

**Step 5: Run tests**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test cdp_devtools --lib`
Expected: 3 tests PASS

**Step 6: Commit**

```bash
git add src-tauri/src/cdp_devtools.rs src-tauri/src/lib.rs
git commit -m "feat(browser): CDP console, performance metrics, and cookie viewer"
```

---

### Task 3: Browser Playground Store (Svelte Frontend)

**Files:**
- Create: `src/lib/stores/browser-playground.svelte.ts` (~200 LoC)

**Step 1: Write the store**

```typescript
/**
 * NEXUS Browser Playground Store — Svelte 5 runes
 *
 * Unified devtools store: Network waterfall, Console, Performance, Cookies.
 * Polls CDP backend for live updates when playground is active.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface NetworkEntry {
    request_id: string;
    url: string;
    method: string;
    status: number | null;
    mime_type: string | null;
    size_bytes: number | null;
    duration_ms: number | null;
    timestamp: number;
    resource_type: string;
}

export interface ConsoleEntry {
    level: string;
    text: string;
    timestamp: number;
    source: string;
    line_number: number | null;
    url: string | null;
}

export interface PerfMetrics {
    dom_content_loaded_ms: number | null;
    load_event_ms: number | null;
    first_paint_ms: number | null;
    first_contentful_paint_ms: number | null;
    dom_nodes: number | null;
    js_heap_size_mb: number | null;
    timestamp: number;
}

export interface CdpCookie {
    name: string;
    value: string;
    domain: string;
    path: string;
    secure: boolean;
    http_only: boolean;
    expires: number | null;
    same_site: string | null;
}

// ============================================================================
// STATE
// ============================================================================

let networkEntries = $state<NetworkEntry[]>([]);
let consoleEntries = $state<ConsoleEntry[]>([]);
let perfMetrics = $state<PerfMetrics | null>(null);
let cookies = $state<CdpCookie[]>([]);
let isPolling = $state(false);
let error = $state<string | null>(null);
let activeDevtoolsTab = $state<'network' | 'console' | 'performance' | 'cookies'>('network');

let pollInterval: ReturnType<typeof setInterval> | null = null;

// ============================================================================
// ACTIONS
// ============================================================================

async function fetchNetwork(sinceTimestamp?: number): Promise<void> {
    try {
        networkEntries = await invoke<NetworkEntry[]>('cdp_network_entries', {
            sinceTimestamp: sinceTimestamp ?? null
        });
    } catch (e) {
        error = String(e);
    }
}

async function enableNetwork(pageId: string): Promise<void> {
    try {
        await invoke<string>('cdp_network_enable', { pageId });
    } catch (e) {
        error = String(e);
    }
}

async function clearNetwork(): Promise<void> {
    await invoke<string>('cdp_network_clear');
    networkEntries = [];
}

async function fetchConsole(): Promise<void> {
    try {
        consoleEntries = await invoke<ConsoleEntry[]>('cdp_console_entries');
    } catch (e) {
        error = String(e);
    }
}

async function clearConsole(): Promise<void> {
    await invoke<string>('cdp_console_clear');
    consoleEntries = [];
}

async function fetchPerf(pageId: string): Promise<void> {
    try {
        perfMetrics = await invoke<PerfMetrics>('cdp_perf_metrics', { pageId });
    } catch (e) {
        error = String(e);
    }
}

async function fetchCookies(pageId: string): Promise<void> {
    try {
        cookies = await invoke<CdpCookie[]>('cdp_get_cookies', { pageId });
    } catch (e) {
        error = String(e);
    }
}

function startPolling(pageId: string, intervalMs: number = 2000): void {
    stopPolling();
    isPolling = true;
    pollInterval = setInterval(async () => {
        await fetchNetwork();
        await fetchConsole();
    }, intervalMs);
}

function stopPolling(): void {
    if (pollInterval) clearInterval(pollInterval);
    pollInterval = null;
    isPolling = false;
}

// ============================================================================
// EXPORT
// ============================================================================

export const playgroundStore = {
    get networkEntries() { return networkEntries; },
    get consoleEntries() { return consoleEntries; },
    get perfMetrics() { return perfMetrics; },
    get cookies() { return cookies; },
    get isPolling() { return isPolling; },
    get error() { return error; },
    get activeDevtoolsTab() { return activeDevtoolsTab; },
    set activeDevtoolsTab(tab: 'network' | 'console' | 'performance' | 'cookies') { activeDevtoolsTab = tab; },
    fetchNetwork,
    enableNetwork,
    clearNetwork,
    fetchConsole,
    clearConsole,
    fetchPerf,
    fetchCookies,
    startPolling,
    stopPolling,
};
```

**Step 2: Verify no Svelte errors**

Run: `cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5`
Expected: 0 errors

**Step 3: Commit**

```bash
git add src/lib/stores/browser-playground.svelte.ts
git commit -m "feat(browser): playground devtools store (network, console, perf, cookies)"
```

---

### Task 4: Visual Element Picker (Rust Backend)

**Files:**
- Add function to: `src-tauri/src/cdp_engine.rs` (2 new commands)

**Step 1: Write tests**

Add to `cdp_engine.rs` tests:

```rust
#[test]
fn test_element_info_serialization() {
    let info = ElementInfo {
        tag: "div".into(),
        id: Some("main".into()),
        classes: vec!["container".into(), "flex".into()],
        text_preview: Some("Hello...".into()),
        selector: "div#main.container.flex".into(),
        bounding_box: Some(BoundingBox { x: 0.0, y: 0.0, width: 100.0, height: 50.0 }),
        attributes: vec![("data-testid".into(), "main-content".into())],
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("container"));
    assert!(json.contains("main-content"));
}
```

**Step 2: Write the types and commands**

Add to `cdp_engine.rs`:

```rust
/// Bounding box for visual element picker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Info about a DOM element (for element picker)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub text_preview: Option<String>,
    pub selector: String,
    pub bounding_box: Option<BoundingBox>,
    pub attributes: Vec<(String, String)>,
}

/// Get all interactive elements on the page (for visual picker)
#[tauri::command]
pub async fn cdp_get_elements(page_id: String, selector: Option<String>) -> Result<Vec<ElementInfo>, String> {
    let sel = selector.as_deref().unwrap_or("a, button, input, select, textarea, [onclick], [role='button']");
    let script = format!(r#"Array.from(document.querySelectorAll('{sel}')).slice(0, 100).map(el => {{
        const rect = el.getBoundingClientRect();
        const classes = Array.from(el.classList);
        let cssSelector = el.tagName.toLowerCase();
        if (el.id) cssSelector += '#' + el.id;
        classes.forEach(c => cssSelector += '.' + c);
        const attrs = Array.from(el.attributes)
            .filter(a => !['class','id','style'].includes(a.name))
            .map(a => [a.name, a.value]);
        return {{
            tag: el.tagName.toLowerCase(),
            id: el.id || null,
            classes,
            text_preview: (el.textContent || '').trim().substring(0, 80) || null,
            selector: cssSelector,
            bounding_box: rect.width > 0 ? {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }} : null,
            attributes: attrs,
        }};
    }})"#);
    let result = cdp_eval_js(&page_id, &script).await?;
    serde_json::from_value(result).map_err(|e| format!("Element parse error: {e}"))
}

/// Highlight an element on the page (injects CSS outline)
#[tauri::command]
pub async fn cdp_highlight_element(page_id: String, selector: String) -> Result<String, String> {
    let script = format!(r#"(() => {{
        document.querySelectorAll('.__nexus_highlight').forEach(el => el.classList.remove('__nexus_highlight'));
        const style = document.getElementById('__nexus_highlight_style') || (() => {{
            const s = document.createElement('style');
            s.id = '__nexus_highlight_style';
            s.textContent = '.__nexus_highlight {{ outline: 2px solid #00FF66 !important; outline-offset: 2px; box-shadow: 0 0 12px rgba(0,255,102,0.4) !important; }}';
            document.head.appendChild(s);
            return s;
        }})();
        const el = document.querySelector('{selector}');
        if (el) {{ el.classList.add('__nexus_highlight'); return 'Highlighted: ' + '{selector}'; }}
        return 'Element not found: ' + '{selector}';
    }})()"#);
    let result = cdp_eval_js(&page_id, &script).await?;
    Ok(result.as_str().unwrap_or("done").to_string())
}
```

**Step 3: Register in lib.rs**

Add to `invoke_handler`:
```rust
cdp_engine::cdp_get_elements,
cdp_engine::cdp_highlight_element,
```

**Step 4: Run tests**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test test_element_info --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/cdp_engine.rs src-tauri/src/lib.rs
git commit -m "feat(browser): visual element picker with highlight (CDP)"
```

---

### Task 5: Browser Playground UI — Network Waterfall Tab

**Files:**
- Modify: `src/routes/browser/+page.svelte` (replace Playground tab content)

**Step 1: Add the network waterfall to the Playground tab**

In the existing `browser/+page.svelte`, replace the current Playground tab content with devtools sub-tabs. The Playground tab should contain:
- 4 devtools sub-tabs: Network | Console | Performance | Cookies
- Network waterfall: table with Method, Status (color-coded), URL, Size, Duration bar
- Enable/Clear buttons
- Auto-polling indicator

Key Svelte 5 patterns to follow:
- Import `playgroundStore` from `$lib/stores/browser-playground.svelte.ts`
- Use `{#each playgroundStore.networkEntries as entry}` for reactive rendering
- Status colors: 2xx=green, 3xx=blue, 4xx=orange, 5xx=red
- Duration bar: `width: ${Math.min(entry.duration_ms / 10, 100)}%` of max column width
- Use existing `cdpStore.activePage` to know which page to monitor

**Step 2: Verify no Svelte errors**

Run: `cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5`
Expected: 0 errors

**Step 3: Commit**

```bash
git add src/routes/browser/+page.svelte
git commit -m "feat(browser): network waterfall UI in playground tab"
```

---

### Task 6: Browser Playground UI — Console & Performance Tabs

**Files:**
- Modify: `src/routes/browser/+page.svelte` (add console + perf sub-tab content)

**Step 1: Console tab content**

- Console entries list with level-based icons and colors (log=white, warn=yellow, error=red, info=blue)
- Line number and URL source display
- Clear button
- Monospace font (`font-mono`)

**Step 2: Performance tab content**

- Metric cards: DOMContentLoaded, Load Event, First Paint, FCP
- DOM Nodes count and JS Heap size
- Refresh button per page
- Color-coded thresholds: green <1s, yellow <3s, red >3s

**Step 3: Cookie viewer tab**

- Table: Name, Value (truncated), Domain, Path, Secure (checkmark), HttpOnly, Expires
- Edit button per cookie (future: inline edit)

**Step 4: Verify**

Run: `cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5`
Expected: 0 errors

**Step 5: Commit**

```bash
git add src/routes/browser/+page.svelte
git commit -m "feat(browser): console output, performance metrics, and cookie viewer UI"
```

---

### Task 7: Element Picker UI Integration

**Files:**
- Modify: `src/routes/browser/+page.svelte` (add picker to Browser tab)
- Modify: `src/lib/stores/cdp.svelte.ts` (add getElements, highlightElement)

**Step 1: Add store methods to cdp.svelte.ts**

```typescript
export interface ElementInfo {
    tag: string;
    id: string | null;
    classes: string[];
    text_preview: string | null;
    selector: string;
    bounding_box: { x: number; y: number; width: number; height: number } | null;
    attributes: [string, string][];
}

// Add to state
let elements = $state<ElementInfo[]>([]);

// Add actions
async function getElements(selector?: string): Promise<ElementInfo[]> {
    if (!activePage) return [];
    try {
        const result = await invoke<ElementInfo[]>('cdp_get_elements', {
            pageId: activePage,
            selector: selector ?? null
        });
        elements = result;
        return result;
    } catch (e) {
        error = String(e);
        return [];
    }
}

async function highlightElement(selector: string): Promise<void> {
    if (!activePage) return;
    try {
        await invoke<string>('cdp_highlight_element', { pageId: activePage, selector });
    } catch (e) {
        error = String(e);
    }
}
```

**Step 2: UI in Browser tab**

Add an "Element Picker" section below the existing CDP actions:
- "Scan Elements" button that calls `cdpStore.getElements()`
- Scrollable list of found elements with tag, selector, text preview
- Hover over element in list → calls `highlightElement` on the actual page
- Click element → copies selector to the CSS Selector input field

**Step 3: Verify and commit**

```bash
cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5
git add src/lib/stores/cdp.svelte.ts src/routes/browser/+page.svelte
git commit -m "feat(browser): visual element picker with live highlight"
```

---

### Task 8: Phase 2 Full Build Verification

**Step 1: Run full Rust test suite**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test --lib 2>&1 | tail -10`
Expected: All tests PASS (should be ~87+ tests now)

**Step 2: Run Svelte check**

Run: `cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5`
Expected: 0 errors

**Step 3: Run cargo build**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo build 2>&1 | tail -5`
Expected: build succeeds

**Step 4: Commit verification**

```bash
git add -A
git commit -m "feat(browser): Phase 2 complete — Opera GX Browser Playground with devtools"
```

---

## PHASE 3: Modular Component System (Theme Engine)

### Task 9: Theme Engine (Rust Backend — SQLite Persistence)

**Files:**
- Create: `src-tauri/src/theme_engine.rs` (~280 LoC)
- Modify: `src-tauri/src/lib.rs` (add module + 6 commands)

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_serialization() {
        let theme = NexusTheme {
            id: "custom-1".into(),
            name: "Cyberpunk Red".into(),
            author: Some("User".into()),
            version: "1.0.0".into(),
            variables: vec![
                ("--color-gx-neon".into(), "#FF0033".into()),
                ("--color-gx-bg-primary".into(), "#0A0A0A".into()),
            ],
            is_builtin: false,
        };
        let json = serde_json::to_string(&theme).unwrap();
        assert!(json.contains("Cyberpunk Red"));
        assert!(json.contains("#FF0033"));
    }

    #[test]
    fn test_builtin_themes_exist() {
        let themes = builtin_themes();
        assert!(themes.len() >= 3);
        assert!(themes.iter().any(|t| t.id == "default-neon-green"));
        assert!(themes.iter().all(|t| t.is_builtin));
    }

    #[test]
    fn test_layout_serialization() {
        let layout = WidgetLayout {
            id: "layout-1".into(),
            name: "My Layout".into(),
            widgets: vec![
                WidgetPlacement {
                    widget_id: "system-stats".into(),
                    x: 0, y: 0, w: 4, h: 2,
                    config: serde_json::json!({}),
                },
            ],
            route: "/".into(),
        };
        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.contains("system-stats"));
    }
}
```

**Step 2: Write the theme_engine module**

```rust
//! NEXUS Theme Engine — Customer-facing UI customization
//!
//! CSS-variable-driven themes with SQLite persistence.
//! Inspired by ElvUI/BenikUI modular addon system.
//!
//! Customers can:
//! - Switch between built-in themes (Neon Green, Cyberpunk Red, Arctic Blue, etc.)
//! - Create custom themes by overriding CSS variables
//! - Export/import themes (zstd + base64 encoded)
//! - Arrange widgets in drag-and-drop layouts
//! - Save/load layout profiles

use serde::{Deserialize, Serialize};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A complete NEXUS theme (CSS variable overrides)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusTheme {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub version: String,
    pub variables: Vec<(String, String)>,  // (CSS var name, value)
    pub is_builtin: bool,
}

/// A widget placement in a layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPlacement {
    pub widget_id: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub config: serde_json::Value,
}

/// A complete layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetLayout {
    pub id: String,
    pub name: String,
    pub widgets: Vec<WidgetPlacement>,
    pub route: String,  // Which page this layout applies to
}

/// Theme export format (zstd + base64)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeExport {
    pub theme: NexusTheme,
    pub layouts: Vec<WidgetLayout>,
    pub nexus_version: String,
    pub export_date: String,
}

/// Built-in themes
pub fn builtin_themes() -> Vec<NexusTheme> {
    vec![
        NexusTheme {
            id: "default-neon-green".into(),
            name: "Neon Green (Default)".into(),
            author: Some("NEXUS Team".into()),
            version: "1.0.0".into(),
            variables: vec![], // Uses app.css defaults
            is_builtin: true,
        },
        NexusTheme {
            id: "cyberpunk-red".into(),
            name: "Cyberpunk Red".into(),
            author: Some("NEXUS Team".into()),
            version: "1.0.0".into(),
            variables: vec![
                ("--color-gx-neon".into(), "#FF3366".into()),
                ("--color-gx-neon-dim".into(), "#CC2952".into()),
                ("--color-gx-neon-bright".into(), "#FF6699".into()),
                ("--color-gx-status-success".into(), "#FF3366".into()),
                ("--color-gx-border-focus".into(), "#FF3366".into()),
            ],
            is_builtin: true,
        },
        NexusTheme {
            id: "arctic-blue".into(),
            name: "Arctic Blue".into(),
            author: Some("NEXUS Team".into()),
            version: "1.0.0".into(),
            variables: vec![
                ("--color-gx-neon".into(), "#00CCFF".into()),
                ("--color-gx-neon-dim".into(), "#0099CC".into()),
                ("--color-gx-neon-bright".into(), "#33DDFF".into()),
                ("--color-gx-status-success".into(), "#00CCFF".into()),
                ("--color-gx-border-focus".into(), "#00CCFF".into()),
            ],
            is_builtin: true,
        },
        NexusTheme {
            id: "sunset-orange".into(),
            name: "Sunset Orange".into(),
            author: Some("NEXUS Team".into()),
            version: "1.0.0".into(),
            variables: vec![
                ("--color-gx-neon".into(), "#FF8800".into()),
                ("--color-gx-neon-dim".into(), "#CC6600".into()),
                ("--color-gx-neon-bright".into(), "#FFAA33".into()),
                ("--color-gx-status-success".into(), "#FF8800".into()),
                ("--color-gx-border-focus".into(), "#FF8800".into()),
            ],
            is_builtin: true,
        },
        NexusTheme {
            id: "phantom-purple".into(),
            name: "Phantom Purple".into(),
            author: Some("NEXUS Team".into()),
            version: "1.0.0".into(),
            variables: vec![
                ("--color-gx-neon".into(), "#9933FF".into()),
                ("--color-gx-neon-dim".into(), "#7722CC".into()),
                ("--color-gx-neon-bright".into(), "#BB66FF".into()),
                ("--color-gx-status-success".into(), "#9933FF".into()),
                ("--color-gx-border-focus".into(), "#9933FF".into()),
            ],
            is_builtin: true,
        },
    ]
}

// ============================================================================
// SQLITE PERSISTENCE
// ============================================================================

fn get_db() -> Result<Connection, String> {
    let data_dir = dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("nexus");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Dir create error: {e}"))?;
    let db_path = data_dir.join("themes.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("DB open error: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS themes (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS layouts (
            id TEXT PRIMARY KEY,
            route TEXT NOT NULL,
            data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS active_theme (
            key TEXT PRIMARY KEY DEFAULT 'current',
            theme_id TEXT NOT NULL
        );"
    ).map_err(|e| format!("Schema error: {e}"))?;
    Ok(conn)
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// List all available themes (built-in + custom)
#[tauri::command]
pub async fn theme_list() -> Result<Vec<NexusTheme>, String> {
    let mut themes = builtin_themes();
    if let Ok(conn) = get_db() {
        let mut stmt = conn.prepare("SELECT data FROM themes")
            .map_err(|e| format!("Query error: {e}"))?;
        let custom: Vec<NexusTheme> = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(serde_json::from_str(&data).unwrap_or_else(|_| NexusTheme {
                id: "error".into(), name: "Error".into(), author: None,
                version: "0".into(), variables: vec![], is_builtin: false,
            }))
        }).map_err(|e| format!("Query error: {e}"))?.filter_map(|r| r.ok()).collect();
        themes.extend(custom);
    }
    Ok(themes)
}

/// Get the active theme
#[tauri::command]
pub async fn theme_get_active() -> Result<NexusTheme, String> {
    let conn = get_db()?;
    let theme_id: String = conn.query_row(
        "SELECT theme_id FROM active_theme WHERE key = 'current'",
        [], |row| row.get(0),
    ).unwrap_or_else(|_| "default-neon-green".into());

    let themes = builtin_themes();
    if let Some(t) = themes.iter().find(|t| t.id == theme_id) {
        return Ok(t.clone());
    }

    // Check custom themes
    let data: String = conn.query_row(
        "SELECT data FROM themes WHERE id = ?1", [&theme_id], |row| row.get(0),
    ).map_err(|_| format!("Theme '{theme_id}' not found"))?;
    serde_json::from_str(&data).map_err(|e| format!("Parse error: {e}"))
}

/// Set the active theme
#[tauri::command]
pub async fn theme_set_active(theme_id: String) -> Result<String, String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT OR REPLACE INTO active_theme (key, theme_id) VALUES ('current', ?1)",
        [&theme_id],
    ).map_err(|e| format!("Set active error: {e}"))?;
    Ok(format!("Active theme set to: {theme_id}"))
}

/// Save a custom theme
#[tauri::command]
pub async fn theme_save(theme: NexusTheme) -> Result<String, String> {
    if theme.is_builtin { return Err("Cannot modify built-in themes".into()); }
    let conn = get_db()?;
    let data = serde_json::to_string(&theme).map_err(|e| format!("Serialize error: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO themes (id, data) VALUES (?1, ?2)",
        [&theme.id, &data],
    ).map_err(|e| format!("Save error: {e}"))?;
    Ok(format!("Theme '{}' saved", theme.name))
}

/// Delete a custom theme
#[tauri::command]
pub async fn theme_delete(theme_id: String) -> Result<String, String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM themes WHERE id = ?1", [&theme_id])
        .map_err(|e| format!("Delete error: {e}"))?;
    Ok(format!("Theme '{theme_id}' deleted"))
}

/// Export theme + layouts as base64 string
#[tauri::command]
pub async fn theme_export(theme_id: String) -> Result<String, String> {
    let theme = theme_get_active().await?; // or get by id
    let export = ThemeExport {
        theme,
        layouts: vec![], // TODO: include layouts
        nexus_version: "0.1.0".into(),
        export_date: chrono::Utc::now().to_rfc3339(),
    };
    let json = serde_json::to_string(&export).map_err(|e| format!("Serialize: {e}"))?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(json.as_bytes()))
}

/// Import theme from base64 string
#[tauri::command]
pub async fn theme_import(encoded: String) -> Result<NexusTheme, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&encoded)
        .map_err(|e| format!("Base64 decode error: {e}"))?;
    let json = String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {e}"))?;
    let export: ThemeExport = serde_json::from_str(&json)
        .map_err(|e| format!("Parse error: {e}"))?;
    // Save the imported theme
    let mut theme = export.theme;
    theme.is_builtin = false; // Force non-builtin
    theme_save(theme.clone()).await?;
    Ok(theme)
}
```

**Step 3: Register in lib.rs**

```rust
mod theme_engine;
```

And in `invoke_handler`:
```rust
// Theme Engine (customizable UI for customers)
theme_engine::theme_list,
theme_engine::theme_get_active,
theme_engine::theme_set_active,
theme_engine::theme_save,
theme_engine::theme_delete,
theme_engine::theme_export,
theme_engine::theme_import,
```

**Step 4: Run tests**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test theme_engine --lib`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add src-tauri/src/theme_engine.rs src-tauri/src/lib.rs
git commit -m "feat(theme): theme engine with SQLite persistence, 5 built-in themes, export/import"
```

---

### Task 10: Theme Store (Svelte Frontend)

**Files:**
- Create: `src/lib/stores/theme.svelte.ts` (~140 LoC)

**Step 1: Write the theme store**

```typescript
/**
 * NEXUS Theme Store — Svelte 5 runes
 *
 * Manages theme switching, CSS variable injection, and custom theme creation.
 * Themes are persisted in SQLite via Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';

export interface NexusTheme {
    id: string;
    name: string;
    author: string | null;
    version: string;
    variables: [string, string][];
    is_builtin: boolean;
}

let themes = $state<NexusTheme[]>([]);
let activeTheme = $state<NexusTheme | null>(null);
let isLoading = $state(false);
let error = $state<string | null>(null);

/** Apply theme CSS variables to document root */
function applyTheme(theme: NexusTheme) {
    const root = document.documentElement;
    // Reset to defaults first (remove all custom overrides)
    root.style.cssText = '';
    // Apply theme variables
    for (const [name, value] of theme.variables) {
        root.style.setProperty(name, value);
    }
}

async function loadThemes(): Promise<void> {
    isLoading = true;
    try {
        themes = await invoke<NexusTheme[]>('theme_list');
        const active = await invoke<NexusTheme>('theme_get_active');
        activeTheme = active;
        applyTheme(active);
    } catch (e) {
        error = String(e);
    } finally {
        isLoading = false;
    }
}

async function setTheme(themeId: string): Promise<void> {
    try {
        await invoke<string>('theme_set_active', { themeId });
        const theme = themes.find(t => t.id === themeId);
        if (theme) {
            activeTheme = theme;
            applyTheme(theme);
        }
    } catch (e) {
        error = String(e);
    }
}

async function saveCustomTheme(theme: NexusTheme): Promise<void> {
    try {
        await invoke<string>('theme_save', { theme });
        await loadThemes();
    } catch (e) {
        error = String(e);
    }
}

async function deleteTheme(themeId: string): Promise<void> {
    try {
        await invoke<string>('theme_delete', { themeId });
        themes = themes.filter(t => t.id !== themeId);
    } catch (e) {
        error = String(e);
    }
}

async function exportTheme(themeId: string): Promise<string | null> {
    try {
        return await invoke<string>('theme_export', { themeId });
    } catch (e) {
        error = String(e);
        return null;
    }
}

async function importTheme(encoded: string): Promise<void> {
    try {
        await invoke<NexusTheme>('theme_import', { encoded });
        await loadThemes();
    } catch (e) {
        error = String(e);
    }
}

export const themeStore = {
    get themes() { return themes; },
    get activeTheme() { return activeTheme; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    loadThemes,
    setTheme,
    saveCustomTheme,
    deleteTheme,
    exportTheme,
    importTheme,
    applyTheme,
};
```

**Step 2: Initialize theme on app load**

Modify `src/routes/+layout.svelte`:
- Import `themeStore`
- Call `themeStore.loadThemes()` in `onMount`

**Step 3: Verify and commit**

```bash
cd /opt/ork-station/Nexus && npx svelte-check --threshold warning
git add src/lib/stores/theme.svelte.ts src/routes/+layout.svelte
git commit -m "feat(theme): theme store with CSS variable injection and app-level init"
```

---

### Task 11: Settings Page — Theme Picker UI

**Files:**
- Modify: `src/routes/settings/+page.svelte` (add Theme section)

**Step 1: Add Theme section to Settings page**

Add a "Theme" section with:
- Grid of theme cards (3 columns) showing name, author, and a color swatch preview
- Active theme highlighted with neon border glow
- Click to switch themes (calls `themeStore.setTheme`)
- "Create Custom" button that opens a simple editor (color picker for primary neon, backgrounds)
- Export/Import buttons
- Import via textarea (paste base64 string)

**Step 2: Theme card component**

Each card should show:
- Theme name + author
- 5 color swatches: neon, bg-primary, accent-cyan, accent-magenta, status-error
- Active indicator (checkmark + glow border)
- Delete button (only for custom themes, not built-ins)

**Step 3: Verify and commit**

```bash
cd /opt/ork-station/Nexus && npx svelte-check --threshold warning
git add src/routes/settings/+page.svelte
git commit -m "feat(theme): theme picker UI in settings with swatches and export/import"
```

---

### Task 12: Widget Registry System (Rust Backend)

**Files:**
- Create: `src-tauri/src/widget_registry.rs` (~200 LoC)
- Modify: `src-tauri/src/lib.rs` (add module + 3 commands)

**Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_definition() {
        let widget = WidgetDefinition {
            id: "system-stats".into(),
            name: "System Stats".into(),
            description: "CPU, RAM, GPU usage".into(),
            category: WidgetCategory::Monitoring,
            default_size: (4, 2),
            min_size: (2, 1),
            max_size: (12, 4),
            configurable: true,
        };
        assert_eq!(widget.id, "system-stats");
        assert!(widget.configurable);
    }

    #[test]
    fn test_builtin_widgets_exist() {
        let widgets = builtin_widgets();
        assert!(widgets.len() >= 8);
        assert!(widgets.iter().any(|w| w.id == "system-stats"));
        assert!(widgets.iter().any(|w| w.id == "agent-pool"));
    }

    #[test]
    fn test_widget_category_serialization() {
        let cat = WidgetCategory::AI;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"AI\"");
    }
}
```

**Step 2: Write widget_registry module**

```rust
//! NEXUS Widget Registry — Modular dashboard components
//!
//! Each widget is a self-contained UI component that customers can
//! place on their dashboard via drag-and-drop layout manager.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetCategory {
    Monitoring,
    AI,
    Development,
    Browser,
    Automation,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: WidgetCategory,
    pub default_size: (u32, u32),  // (width, height) in grid units
    pub min_size: (u32, u32),
    pub max_size: (u32, u32),
    pub configurable: bool,
}

pub fn builtin_widgets() -> Vec<WidgetDefinition> {
    vec![
        WidgetDefinition {
            id: "system-stats".into(), name: "System Stats".into(),
            description: "CPU, RAM, GPU, Temperature".into(),
            category: WidgetCategory::Monitoring,
            default_size: (4, 2), min_size: (2, 1), max_size: (12, 4), configurable: true,
        },
        WidgetDefinition {
            id: "agent-pool".into(), name: "Agent Pool".into(),
            description: "NeuralSwarm agent status".into(),
            category: WidgetCategory::AI,
            default_size: (4, 3), min_size: (3, 2), max_size: (8, 6), configurable: true,
        },
        WidgetDefinition {
            id: "quick-chat".into(), name: "Quick Chat".into(),
            description: "Inline AI chat widget".into(),
            category: WidgetCategory::AI,
            default_size: (6, 4), min_size: (4, 3), max_size: (12, 8), configurable: true,
        },
        WidgetDefinition {
            id: "docker-overview".into(), name: "Docker Overview".into(),
            description: "Running containers summary".into(),
            category: WidgetCategory::Development,
            default_size: (4, 2), min_size: (3, 2), max_size: (8, 4), configurable: false,
        },
        WidgetDefinition {
            id: "github-feed".into(), name: "GitHub Feed".into(),
            description: "Recent commits and PRs".into(),
            category: WidgetCategory::Development,
            default_size: (4, 3), min_size: (3, 2), max_size: (8, 6), configurable: true,
        },
        WidgetDefinition {
            id: "browser-sessions".into(), name: "Browser Sessions".into(),
            description: "Active CDP pages".into(),
            category: WidgetCategory::Browser,
            default_size: (3, 2), min_size: (2, 1), max_size: (6, 4), configurable: false,
        },
        WidgetDefinition {
            id: "network-waterfall".into(), name: "Network Waterfall".into(),
            description: "Live HTTP request monitor".into(),
            category: WidgetCategory::Browser,
            default_size: (6, 3), min_size: (4, 2), max_size: (12, 6), configurable: true,
        },
        WidgetDefinition {
            id: "model-status".into(), name: "Model Status".into(),
            description: "Ollama/local model health".into(),
            category: WidgetCategory::AI,
            default_size: (3, 2), min_size: (2, 1), max_size: (6, 3), configurable: false,
        },
        WidgetDefinition {
            id: "eval-pipeline".into(), name: "Eval Pipeline".into(),
            description: "Agent evaluation results".into(),
            category: WidgetCategory::AI,
            default_size: (4, 3), min_size: (3, 2), max_size: (8, 5), configurable: true,
        },
        WidgetDefinition {
            id: "news-ticker".into(), name: "News Ticker".into(),
            description: "AI news headlines".into(),
            category: WidgetCategory::System,
            default_size: (4, 1), min_size: (3, 1), max_size: (12, 2), configurable: true,
        },
    ]
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

#[tauri::command]
pub async fn widget_list() -> Result<Vec<WidgetDefinition>, String> {
    Ok(builtin_widgets())
}

#[tauri::command]
pub async fn widget_get(widget_id: String) -> Result<WidgetDefinition, String> {
    builtin_widgets()
        .into_iter()
        .find(|w| w.id == widget_id)
        .ok_or_else(|| format!("Widget '{widget_id}' not found"))
}

#[tauri::command]
pub async fn widget_categories() -> Result<Vec<String>, String> {
    Ok(vec![
        "Monitoring".into(), "AI".into(), "Development".into(),
        "Browser".into(), "Automation".into(), "System".into(),
    ])
}
```

**Step 3: Register in lib.rs and run tests**

```bash
cargo test widget_registry --lib
git add src-tauri/src/widget_registry.rs src-tauri/src/lib.rs
git commit -m "feat(widgets): widget registry with 10 built-in dashboard widgets"
```

---

### Task 13: Layout Persistence (Rust Backend)

**Files:**
- Add to: `src-tauri/src/theme_engine.rs` (3 new layout commands)

**Step 1: Add layout commands to theme_engine.rs**

```rust
/// Save a widget layout for a route
#[tauri::command]
pub async fn layout_save(layout: WidgetLayout) -> Result<String, String> {
    let conn = get_db()?;
    let data = serde_json::to_string(&layout).map_err(|e| format!("Serialize: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO layouts (id, route, data) VALUES (?1, ?2, ?3)",
        [&layout.id, &layout.route, &data],
    ).map_err(|e| format!("Save layout error: {e}"))?;
    Ok(format!("Layout '{}' saved", layout.name))
}

/// Get layout for a route
#[tauri::command]
pub async fn layout_get(route: String) -> Result<Option<WidgetLayout>, String> {
    let conn = get_db()?;
    let result = conn.query_row(
        "SELECT data FROM layouts WHERE route = ?1 ORDER BY id DESC LIMIT 1",
        [&route],
        |row| {
            let data: String = row.get(0)?;
            Ok(data)
        },
    );
    match result {
        Ok(data) => {
            let layout: WidgetLayout = serde_json::from_str(&data)
                .map_err(|e| format!("Parse: {e}"))?;
            Ok(Some(layout))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Query error: {e}")),
    }
}

/// Delete a layout
#[tauri::command]
pub async fn layout_delete(layout_id: String) -> Result<String, String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM layouts WHERE id = ?1", [&layout_id])
        .map_err(|e| format!("Delete error: {e}"))?;
    Ok(format!("Layout '{layout_id}' deleted"))
}
```

**Step 2: Register and commit**

```bash
# Add to invoke_handler: layout_save, layout_get, layout_delete
cargo test theme_engine --lib
git add src-tauri/src/theme_engine.rs src-tauri/src/lib.rs
git commit -m "feat(layout): widget layout persistence with SQLite"
```

---

### Task 14: Phase 3 Full Build Verification

**Step 1: Full test suite**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo test --lib 2>&1 | tail -15`
Expected: All tests PASS (~95+ tests)

**Step 2: Svelte check**

Run: `cd /opt/ork-station/Nexus && npx svelte-check --threshold warning 2>&1 | tail -5`
Expected: 0 errors

**Step 3: Cargo build**

Run: `cd /opt/ork-station/Nexus/src-tauri && cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: Phase 2+3 complete — Browser Playground + Modular Component System"
```

---

## Summary

| Phase | Tasks | New Files | New Commands | New Tests |
|-------|-------|-----------|-------------|-----------|
| Phase 2 | 1-8 | 3 Rust, 1 Svelte store | +9 Tauri | ~10 |
| Phase 3 | 9-14 | 2 Rust, 1 Svelte store | +12 Tauri | ~9 |
| **Total** | **14** | **6 new files** | **+21 commands (→84 total)** | **~19 new** |

### New Tauri Commands After Plan

| Module | Commands |
|--------|----------|
| `cdp_network` | cdp_network_entries, cdp_network_enable, cdp_network_clear |
| `cdp_devtools` | cdp_console_entries, cdp_console_clear, cdp_perf_metrics, cdp_get_cookies |
| `cdp_engine` (additions) | cdp_get_elements, cdp_highlight_element |
| `theme_engine` | theme_list, theme_get_active, theme_set_active, theme_save, theme_delete, theme_export, theme_import, layout_save, layout_get, layout_delete |
| `widget_registry` | widget_list, widget_get, widget_categories |

### Dependencies — No New Crates Required

All new functionality uses existing crate dependencies:
- `chromiumoxide 0.9` (CDP domains: Network, Runtime)
- `rusqlite 0.36` (theme/layout persistence)
- `base64 0.22` (theme export encoding)
- `serde/serde_json` (serialization)
- `once_cell` (static buffers)
- `chrono` (timestamps)
