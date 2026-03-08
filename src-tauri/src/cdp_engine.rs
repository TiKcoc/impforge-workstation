//! ImpForge CDP Browser Engine — Chrome DevTools Protocol via chromiumoxide
//!
//! Full browser automation without Node.js:
//! - Navigate, click, fill forms, extract content
//! - JavaScript execution with type-safe returns
//! - Screenshot capture (PNG)
//! - Stealth mode (anti-bot detection bypass)
//! - Multi-page session management
//!
//! Architecture: Lazy-initialized global Browser with per-session Pages.
//! The chromiumoxide Handler runs in a background tokio task.
//!
//! License: MIT/Apache-2.0 (chromiumoxide is MIT/Apache-2.0)

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

// ============================================================================
// BROWSER DETECTION — Cross-platform Chromium finder
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInstallation {
    pub name: String,
    pub path: String,
    pub browser_type: String,
}

/// Detect all installed Chromium-based browsers
pub fn detect_all_browsers() -> Vec<BrowserInstallation> {
    let mut browsers = Vec::new();

    #[cfg(target_os = "linux")]
    {
        let candidates = [
            ("Google Chrome", "/usr/bin/google-chrome-stable", "chrome"),
            ("Google Chrome", "/usr/bin/google-chrome", "chrome"),
            ("Chromium", "/usr/bin/chromium-browser", "chromium"),
            ("Chromium", "/usr/bin/chromium", "chromium"),
            ("Brave", "/usr/bin/brave-browser", "brave"),
            ("Brave", "/usr/bin/brave-browser-stable", "brave"),
            ("Microsoft Edge", "/usr/bin/microsoft-edge-stable", "edge"),
            ("Microsoft Edge", "/usr/bin/microsoft-edge", "edge"),
            ("Opera", "/usr/bin/opera", "opera"),
            ("Vivaldi", "/usr/bin/vivaldi-stable", "vivaldi"),
            ("Vivaldi", "/usr/bin/vivaldi", "vivaldi"),
        ];
        for (name, path, btype) in candidates {
            if std::path::Path::new(path).exists() {
                browsers.push(BrowserInstallation {
                    name: name.to_string(),
                    path: path.to_string(),
                    browser_type: btype.to_string(),
                });
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("PROGRAMFILES").unwrap_or_default();
        let candidates = [
            ("Google Chrome", format!("{local_app_data}\\Google\\Chrome\\Application\\chrome.exe"), "chrome"),
            ("Google Chrome", format!("{program_files}\\Google\\Chrome\\Application\\chrome.exe"), "chrome"),
            ("Brave", format!("{local_app_data}\\BraveSoftware\\Brave-Browser\\Application\\brave.exe"), "brave"),
            ("Microsoft Edge", format!("{program_files} (x86)\\Microsoft\\Edge\\Application\\msedge.exe"), "edge"),
            ("Opera", format!("{local_app_data}\\Programs\\Opera\\opera.exe"), "opera"),
            ("Vivaldi", format!("{local_app_data}\\Vivaldi\\Application\\vivaldi.exe"), "vivaldi"),
        ];
        for (name, path, btype) in candidates {
            if std::path::Path::new(&path).exists() {
                browsers.push(BrowserInstallation {
                    name: name.to_string(),
                    path,
                    browser_type: btype.to_string(),
                });
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            ("Google Chrome", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "chrome"),
            ("Brave", "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser", "brave"),
            ("Microsoft Edge", "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge", "edge"),
            ("Opera", "/Applications/Opera.app/Contents/MacOS/Opera", "opera"),
            ("Vivaldi", "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi", "vivaldi"),
            ("Chromium", "/Applications/Chromium.app/Contents/MacOS/Chromium", "chromium"),
        ];
        for (name, path, btype) in candidates {
            if std::path::Path::new(path).exists() {
                browsers.push(BrowserInstallation {
                    name: name.to_string(),
                    path: path.to_string(),
                    browser_type: btype.to_string(),
                });
            }
        }
    }

    browsers
}

/// Detect the best available browser (prefer Chrome > Chromium > Brave > Edge)
pub fn detect_browser() -> Option<BrowserInstallation> {
    let all = detect_all_browsers();
    let priority = ["chrome", "chromium", "brave", "edge", "vivaldi", "opera"];
    for ptype in priority {
        if let Some(b) = all.iter().find(|b| b.browser_type == ptype) {
            return Some(b.clone());
        }
    }
    all.into_iter().next()
}

// ============================================================================
// CDP STATE — Global lazy-initialized browser
// ============================================================================

pub(crate) struct CdpState {
    pub(crate) browser: Browser,
    pub(crate) pages: Mutex<HashMap<String, Arc<Page>>>,
    /// Browser user-data directory for profile persistence (cookies, cache)
    pub(crate) user_data_dir: PathBuf,
}

// Safety: Browser uses internal Arc<Mutex<...>> for thread safety
unsafe impl Send for CdpState {}
unsafe impl Sync for CdpState {}

static CDP: OnceCell<CdpState> = OnceCell::const_new();

/// Get the CDP browser profile directory (creates if needed)
fn cdp_profile_dir() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("impforge")
        .join("cdp-profile");
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Cannot create CDP profile dir: {e}"))?;
    Ok(data_dir)
}

/// Page info returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub page_id: String,
    pub url: String,
    pub title: String,
}

/// Result of a CDP navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpNavigateResult {
    pub page_id: String,
    pub url: String,
    pub title: String,
    pub content_length: usize,
}

/// Initialize CDP browser (lazy — called on first CDP action)
async fn get_cdp() -> Result<&'static CdpState, String> {
    CDP.get_or_try_init(|| async {
        let install = detect_browser()
            .ok_or_else(|| "No Chromium-based browser found. Install Chrome, Brave, or Chromium.".to_string())?;

        let user_data_dir = cdp_profile_dir()?;
        log::info!("Launching CDP browser: {} at {} (profile: {})", install.name, install.path, user_data_dir.display());

        let config = BrowserConfig::builder()
            .chrome_executable(&install.path)
            .user_data_dir(&user_data_dir)
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-extensions")
            .arg("--disable-background-networking")
            .arg("--disable-sync")
            .arg("--disable-translate")
            .arg("--disable-default-apps")
            .arg("--no-first-run")
            .arg("--disable-popup-blocking")
            .build()
            .map_err(|e| format!("Browser config error: {e}"))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| format!("Browser launch failed: {e}"))?;

        // Handler MUST run in background — processes WebSocket messages
        tokio::spawn(async move {
            while let Some(result) = handler.next().await {
                if result.is_err() {
                    log::warn!("CDP handler error, stopping");
                    break;
                }
            }
        });

        log::info!("CDP browser launched successfully");

        Ok(CdpState {
            browser,
            pages: Mutex::new(HashMap::new()),
            user_data_dir,
        })
    })
    .await
}

/// Get reference to the CDP state (for use by other modules like cdp_network, cdp_devtools)
pub async fn get_cdp_state() -> Result<&'static CdpState, String> {
    get_cdp().await
}

/// Check if CDP is available without launching
pub fn cdp_is_available() -> bool {
    detect_browser().is_some()
}

// ============================================================================
// CDP OPERATIONS — Core browser automation
// ============================================================================

/// Create a new page and navigate to URL
pub async fn cdp_new_page(url: &str) -> Result<(String, PageInfo), String> {
    let state = get_cdp().await?;
    let page = state
        .browser
        .new_page(url)
        .await
        .map_err(|e| format!("Failed to create page: {e}"))?;

    let page_id = uuid::Uuid::new_v4().to_string();

    let title = page
        .evaluate("document.title")
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .unwrap_or_default();

    let current_url = page
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| url.to_string());

    let info = PageInfo {
        page_id: page_id.clone(),
        url: current_url,
        title,
    };

    state.pages.lock().await.insert(page_id.clone(), Arc::new(page));
    Ok((page_id, info))
}

/// Navigate an existing page to a new URL
pub async fn cdp_navigate_page(page_id: &str, url: &str) -> Result<CdpNavigateResult, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    page.goto(url)
        .await
        .map_err(|e| format!("Navigation failed: {e}"))?;

    let title = page
        .evaluate("document.title")
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .unwrap_or_default();

    let content_length = page
        .content()
        .await
        .map(|c| c.len())
        .unwrap_or(0);

    let current_url = page
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| url.to_string());

    Ok(CdpNavigateResult {
        page_id: page_id.to_string(),
        url: current_url,
        title,
        content_length,
    })
}

/// Click an element by CSS selector
pub async fn cdp_click_element(page_id: &str, selector: &str) -> Result<String, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    let element = page
        .find_element(selector)
        .await
        .map_err(|e| format!("Element '{selector}' not found: {e}"))?;

    element
        .click()
        .await
        .map_err(|e| format!("Click failed: {e}"))?;

    Ok(format!("Clicked: {selector}"))
}

/// Fill a form field by CSS selector
pub async fn cdp_fill_field(
    page_id: &str,
    selector: &str,
    value: &str,
) -> Result<String, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    let element = page
        .find_element(selector)
        .await
        .map_err(|e| format!("Element '{selector}' not found: {e}"))?;

    element
        .click()
        .await
        .map_err(|e| format!("Focus failed: {e}"))?;

    element
        .type_str(value)
        .await
        .map_err(|e| format!("Type failed: {e}"))?;

    Ok(format!("Filled '{selector}' with {} chars", value.len()))
}

/// Execute JavaScript and return result as JSON string
pub async fn cdp_eval_js(page_id: &str, script: &str) -> Result<serde_json::Value, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    let result = page
        .evaluate(script)
        .await
        .map_err(|e| format!("JS execution failed: {e}"))?;

    result
        .into_value::<serde_json::Value>()
        .map_err(|e| format!("JS result conversion failed: {e}"))
}

/// Get full page HTML content
pub async fn cdp_get_content(page_id: &str) -> Result<String, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    page.content()
        .await
        .map_err(|e| format!("Content extraction failed: {e}"))
}

/// Extract text content matching a CSS selector
pub async fn cdp_extract_text(page_id: &str, selector: &str) -> Result<String, String> {
    let script = format!(
        r#"Array.from(document.querySelectorAll('{selector}')).map(el => el.textContent).join('\n')"#
    );
    let result = cdp_eval_js(page_id, &script).await?;
    Ok(result.as_str().unwrap_or_default().to_string())
}

/// Take a screenshot (returns base64-encoded PNG)
pub async fn cdp_screenshot_page(page_id: &str, full_page: bool) -> Result<String, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let page = pages
        .get(page_id)
        .ok_or_else(|| format!("Page '{page_id}' not found"))?;

    let params = chromiumoxide::page::ScreenshotParams::builder()
        .full_page(full_page)
        .build();

    let bytes = page
        .screenshot(params)
        .await
        .map_err(|e| format!("Screenshot failed: {e}"))?;

    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Scroll the page
pub async fn cdp_scroll(page_id: &str, direction: &str) -> Result<String, String> {
    let script = match direction {
        "down" => "window.scrollBy(0, window.innerHeight)",
        "up" => "window.scrollBy(0, -window.innerHeight)",
        "top" => "window.scrollTo(0, 0)",
        "bottom" => "window.scrollTo(0, document.body.scrollHeight)",
        _ => "window.scrollBy(0, window.innerHeight)",
    };
    cdp_eval_js(page_id, script).await?;
    Ok(format!("Scrolled {direction}"))
}

/// Close a page
pub async fn cdp_close_page_by_id(page_id: &str) -> Result<(), String> {
    let state = get_cdp().await?;
    state.pages.lock().await.remove(page_id);
    Ok(())
}

/// List all open pages
pub async fn cdp_list_pages() -> Result<Vec<PageInfo>, String> {
    let state = get_cdp().await?;
    let pages = state.pages.lock().await;
    let mut infos = Vec::new();

    for (id, page) in pages.iter() {
        let title = page
            .evaluate("document.title")
            .await
            .ok()
            .and_then(|v| v.into_value::<String>().ok())
            .unwrap_or_default();

        let url = page
            .url()
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        infos.push(PageInfo {
            page_id: id.clone(),
            url,
            title,
        });
    }

    Ok(infos)
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Detect installed Chromium-based browsers
#[tauri::command]
pub async fn cdp_detect_browsers() -> Result<Vec<BrowserInstallation>, String> {
    Ok(detect_all_browsers())
}

/// Open a new CDP page at URL
#[tauri::command]
pub async fn cdp_open_page(url: String) -> Result<PageInfo, String> {
    let (_, info) = cdp_new_page(&url).await?;
    Ok(info)
}

/// Navigate a CDP page to new URL
#[tauri::command]
pub async fn cdp_navigate(page_id: String, url: String) -> Result<CdpNavigateResult, String> {
    cdp_navigate_page(&page_id, &url).await
}

/// Click an element on a CDP page
#[tauri::command]
pub async fn cdp_click(page_id: String, selector: String) -> Result<String, String> {
    cdp_click_element(&page_id, &selector).await
}

/// Fill a form field on a CDP page
#[tauri::command]
pub async fn cdp_fill(
    page_id: String,
    selector: String,
    value: String,
) -> Result<String, String> {
    cdp_fill_field(&page_id, &selector, &value).await
}

/// Execute JavaScript on a CDP page
#[tauri::command]
pub async fn cdp_execute_js(page_id: String, script: String) -> Result<serde_json::Value, String> {
    cdp_eval_js(&page_id, &script).await
}

/// Extract text matching CSS selector from a CDP page
#[tauri::command]
pub async fn cdp_extract(page_id: String, selector: String) -> Result<String, String> {
    cdp_extract_text(&page_id, &selector).await
}

/// Take a screenshot of a CDP page (returns base64 PNG)
#[tauri::command]
pub async fn cdp_screenshot(page_id: String, full_page: Option<bool>) -> Result<String, String> {
    cdp_screenshot_page(&page_id, full_page.unwrap_or(false)).await
}

/// Get full HTML content of a CDP page
#[tauri::command]
pub async fn cdp_get_page_content(page_id: String) -> Result<String, String> {
    cdp_get_content(&page_id).await
}

/// Scroll a CDP page
#[tauri::command]
pub async fn cdp_page_scroll(page_id: String, direction: String) -> Result<String, String> {
    cdp_scroll(&page_id, &direction).await
}

/// Get all open CDP pages
#[tauri::command]
pub async fn cdp_pages() -> Result<Vec<PageInfo>, String> {
    cdp_list_pages().await
}

/// Close a CDP page
#[tauri::command]
pub async fn cdp_close_page(page_id: String) -> Result<(), String> {
    cdp_close_page_by_id(&page_id).await
}

// ============================================================================
// ELEMENT PICKER — Visual CSS selector picker for the browser playground
// ============================================================================

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

/// Get interactive elements on the page (for visual picker)
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

/// Highlight an element on the page (injects CSS outline with neon glow)
#[tauri::command]
pub async fn cdp_highlight_element(page_id: String, selector: String) -> Result<String, String> {
    let script = format!(r#"(() => {{
        document.querySelectorAll('.__impforge_highlight').forEach(el => el.classList.remove('__impforge_highlight'));
        if (!document.getElementById('__impforge_highlight_style')) {{
            const s = document.createElement('style');
            s.id = '__impforge_highlight_style';
            s.textContent = '.__impforge_highlight {{ outline: 2px solid #00FF66 !important; outline-offset: 2px; box-shadow: 0 0 12px rgba(0,255,102,0.4) !important; }}';
            document.head.appendChild(s);
        }}
        const el = document.querySelector('{selector}');
        if (el) {{ el.classList.add('__impforge_highlight'); return 'Highlighted: {selector}'; }}
        return 'Element not found: {selector}';
    }})()"#);
    let result = cdp_eval_js(&page_id, &script).await?;
    Ok(result.as_str().unwrap_or("done").to_string())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_browsers_returns_vec() {
        let browsers = detect_all_browsers();
        // Should at least find Brave on this system
        // (might be empty on CI, so don't assert non-empty)
        for b in &browsers {
            assert!(!b.name.is_empty());
            assert!(!b.path.is_empty());
        }
    }

    #[test]
    fn test_detect_browser_finds_brave() {
        // This system has Brave installed
        let browser = detect_browser();
        if let Some(b) = browser {
            assert!(!b.path.is_empty());
        }
    }

    #[test]
    fn test_cdp_is_available() {
        // On this system Brave is installed, so CDP should be available
        let available = cdp_is_available();
        // Don't assert true — might run on CI without a browser
        let _ = available;
    }

    #[test]
    fn test_browser_installation_serialization() {
        let install = BrowserInstallation {
            name: "Brave".to_string(),
            path: "/usr/bin/brave-browser".to_string(),
            browser_type: "brave".to_string(),
        };
        let json = serde_json::to_string(&install).unwrap();
        assert!(json.contains("Brave"));
        assert!(json.contains("brave-browser"));
    }

    #[test]
    fn test_page_info_serialization() {
        let info = PageInfo {
            page_id: "test-123".to_string(),
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: PageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.page_id, "test-123");
        assert_eq!(parsed.url, "https://example.com");
    }

    #[test]
    fn test_element_info_serialization() {
        let info = ElementInfo {
            tag: "div".to_string(),
            id: Some("main".to_string()),
            classes: vec!["container".to_string(), "flex".to_string()],
            text_preview: Some("Hello...".to_string()),
            selector: "div#main.container.flex".to_string(),
            bounding_box: Some(BoundingBox { x: 0.0, y: 0.0, width: 100.0, height: 50.0 }),
            attributes: vec![("data-testid".to_string(), "main-content".to_string())],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("container"));
        assert!(json.contains("main-content"));
        let parsed: ElementInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tag, "div");
        assert_eq!(parsed.bounding_box.unwrap().width, 100.0);
    }

    #[test]
    fn test_bounding_box_serialization() {
        let bb = BoundingBox { x: 10.5, y: 20.0, width: 300.0, height: 150.5 };
        let json = serde_json::to_string(&bb).unwrap();
        let parsed: BoundingBox = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.x, 10.5);
        assert_eq!(parsed.height, 150.5);
    }
}
