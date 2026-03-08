//! ImpForge Browser Data Import — Auto-detect and import from installed browsers
//!
//! Imports bookmarks, history, and open tabs from:
//! - Chrome, Chromium, Brave, Edge, Opera, Vivaldi (Chromium-family)
//! - Firefox (Gecko-family)
//!
//! Cross-platform: Linux, Windows, macOS
//!
//! Design: No file download required — auto-detects browser profiles
//! on the customer's system and reads data directly.
//!
//! Password import is handled separately (requires OS keyring access).
//!
//! License: MIT (uses rusqlite bundled, serde_json — all MIT/Apache-2.0)

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ============================================================================
// TYPES
// ============================================================================

/// A detected browser profile with importable data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub browser_name: String,
    pub browser_type: String, // "chrome", "firefox", "brave", etc.
    pub profile_path: String,
    pub profile_name: String,
    pub has_bookmarks: bool,
    pub has_history: bool,
    pub has_passwords: bool,
}

/// An imported bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub folder: String,
    pub date_added: Option<i64>,
}

/// An imported history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: i64,
    pub last_visit: String,
}

/// Import result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub browser_name: String,
    pub bookmarks_imported: usize,
    pub history_imported: usize,
    pub errors: Vec<String>,
}

// ============================================================================
// PROFILE DETECTION — Cross-platform browser profile scanner
// ============================================================================

/// Detect all browser profiles on the system
pub fn detect_profiles() -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

    // Chrome-family browsers (same data format)
    let chromium_browsers = get_chromium_profile_paths(&home);
    for (name, btype, base_path) in chromium_browsers {
        if base_path.exists() {
            // Check Default profile
            let default_profile = base_path.join("Default");
            if default_profile.exists() {
                profiles.push(BrowserProfile {
                    browser_name: name.clone(),
                    browser_type: btype.clone(),
                    profile_path: default_profile.to_string_lossy().to_string(),
                    profile_name: "Default".to_string(),
                    has_bookmarks: default_profile.join("Bookmarks").exists(),
                    has_history: default_profile.join("History").exists(),
                    has_passwords: default_profile.join("Login Data").exists(),
                });
            }

            // Check additional profiles (Profile 1, Profile 2, etc.)
            for i in 1..=5 {
                let profile_dir = base_path.join(format!("Profile {i}"));
                if profile_dir.exists() {
                    profiles.push(BrowserProfile {
                        browser_name: name.clone(),
                        browser_type: btype.clone(),
                        profile_path: profile_dir.to_string_lossy().to_string(),
                        profile_name: format!("Profile {i}"),
                        has_bookmarks: profile_dir.join("Bookmarks").exists(),
                        has_history: profile_dir.join("History").exists(),
                        has_passwords: profile_dir.join("Login Data").exists(),
                    });
                }
            }
        }
    }

    // Firefox profiles
    let firefox_dirs = get_firefox_profile_paths(&home);
    for base_path in firefox_dirs {
        if base_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&base_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Firefox profiles end with .default, .default-release, etc.
                        if name.contains(".default") || name.contains(".dev-edition") {
                            profiles.push(BrowserProfile {
                                browser_name: "Firefox".to_string(),
                                browser_type: "firefox".to_string(),
                                profile_path: path.to_string_lossy().to_string(),
                                profile_name: name,
                                has_bookmarks: path.join("places.sqlite").exists(),
                                has_history: path.join("places.sqlite").exists(),
                                has_passwords: path.join("logins.json").exists(),
                            });
                        }
                    }
                }
            }
        }
    }

    profiles
}

/// Get Chromium-family browser base paths
fn get_chromium_profile_paths(home: &Path) -> Vec<(String, String, PathBuf)> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        let config = home.join(".config");
        paths.extend([
            ("Google Chrome".into(), "chrome".into(), config.join("google-chrome")),
            ("Chromium".into(), "chromium".into(), config.join("chromium")),
            ("Brave".into(), "brave".into(), config.join("BraveSoftware/Brave-Browser")),
            ("Microsoft Edge".into(), "edge".into(), config.join("microsoft-edge")),
            ("Opera".into(), "opera".into(), config.join("opera")),
            ("Vivaldi".into(), "vivaldi".into(), config.join("vivaldi")),
        ]);
    }

    #[cfg(target_os = "windows")]
    {
        let local = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default());
        let roaming = PathBuf::from(std::env::var("APPDATA").unwrap_or_default());
        paths.extend([
            ("Google Chrome".into(), "chrome".into(), local.join("Google/Chrome/User Data")),
            ("Brave".into(), "brave".into(), local.join("BraveSoftware/Brave-Browser/User Data")),
            ("Microsoft Edge".into(), "edge".into(), local.join("Microsoft/Edge/User Data")),
            ("Opera".into(), "opera".into(), roaming.join("Opera Software/Opera Stable")),
            ("Vivaldi".into(), "vivaldi".into(), local.join("Vivaldi/User Data")),
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        let app_support = home.join("Library/Application Support");
        paths.extend([
            ("Google Chrome".into(), "chrome".into(), app_support.join("Google/Chrome")),
            ("Brave".into(), "brave".into(), app_support.join("BraveSoftware/Brave-Browser")),
            ("Microsoft Edge".into(), "edge".into(), app_support.join("Microsoft Edge")),
            ("Opera".into(), "opera".into(), app_support.join("com.operasoftware.Opera")),
            ("Vivaldi".into(), "vivaldi".into(), app_support.join("Vivaldi")),
            ("Chromium".into(), "chromium".into(), app_support.join("Chromium")),
        ]);
    }

    paths
}

/// Get Firefox profile base paths
fn get_firefox_profile_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    paths.push(home.join(".mozilla/firefox"));

    #[cfg(target_os = "windows")]
    paths.push(PathBuf::from(
        std::env::var("APPDATA").unwrap_or_default()
    ).join("Mozilla/Firefox/Profiles"));

    #[cfg(target_os = "macos")]
    paths.push(home.join("Library/Application Support/Firefox/Profiles"));

    paths
}

// ============================================================================
// BOOKMARK IMPORT
// ============================================================================

/// Import bookmarks from a browser profile
pub fn import_bookmarks(profile: &BrowserProfile) -> Result<Vec<Bookmark>, String> {
    match profile.browser_type.as_str() {
        "chrome" | "brave" | "edge" | "opera" | "vivaldi" | "chromium" => {
            import_chromium_bookmarks(&profile.profile_path)
        }
        "firefox" => import_firefox_bookmarks(&profile.profile_path),
        _ => Err(format!("Unsupported browser type: {}", profile.browser_type)),
    }
}

/// Import bookmarks from Chrome-family browser (JSON format)
fn import_chromium_bookmarks(profile_path: &str) -> Result<Vec<Bookmark>, String> {
    let bookmarks_path = Path::new(profile_path).join("Bookmarks");
    let content = std::fs::read_to_string(&bookmarks_path)
        .map_err(|e| format!("Failed to read bookmarks file: {e}"))?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse bookmarks JSON: {e}"))?;

    let mut bookmarks = Vec::new();

    if let Some(roots) = json.get("roots").and_then(|v| v.as_object()) {
        for (folder_name, folder) in roots {
            extract_chrome_bookmarks(folder, folder_name, &mut bookmarks);
        }
    }

    Ok(bookmarks)
}

/// Recursively extract bookmarks from Chrome JSON structure
fn extract_chrome_bookmarks(
    node: &serde_json::Value,
    folder: &str,
    bookmarks: &mut Vec<Bookmark>,
) {
    match node.get("type").and_then(|v| v.as_str()) {
        Some("url") => {
            let title = node.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let url = node.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let date_added = node
                .get("date_added")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok());

            if !url.is_empty() {
                bookmarks.push(Bookmark {
                    title,
                    url,
                    folder: folder.to_string(),
                    date_added,
                });
            }
        }
        Some("folder") => {
            let folder_name = node
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(folder);

            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_chrome_bookmarks(child, folder_name, bookmarks);
                }
            }
        }
        _ => {
            // Root level — check for children directly
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_chrome_bookmarks(child, folder, bookmarks);
                }
            }
        }
    }
}

/// Import bookmarks from Firefox (SQLite places.sqlite)
fn import_firefox_bookmarks(profile_path: &str) -> Result<Vec<Bookmark>, String> {
    let db_path = Path::new(profile_path).join("places.sqlite");

    // Copy the database to avoid locking conflicts with running Firefox
    let temp_path = std::env::temp_dir().join("impforge_firefox_places.sqlite");
    std::fs::copy(&db_path, &temp_path)
        .map_err(|e| format!("Failed to copy Firefox database: {e}"))?;

    let conn = Connection::open(&temp_path)
        .map_err(|e| format!("Failed to open Firefox database: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT b.title, p.url, b.dateAdded, parent.title as folder
             FROM moz_bookmarks b
             JOIN moz_places p ON b.fk = p.id
             LEFT JOIN moz_bookmarks parent ON b.parent = parent.id
             WHERE b.type = 1 AND p.url NOT LIKE 'place:%'
             ORDER BY b.dateAdded DESC",
        )
        .map_err(|e| format!("SQL error: {e}"))?;

    let bookmarks = stmt
        .query_map([], |row| {
            Ok(Bookmark {
                title: row.get::<_, String>(0).unwrap_or_default(),
                url: row.get::<_, String>(1).unwrap_or_default(),
                date_added: row.get::<_, Option<i64>>(2).ok().flatten(),
                folder: row.get::<_, String>(3).unwrap_or_else(|_| "Other".to_string()),
            })
        })
        .map_err(|e| format!("Query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    Ok(bookmarks)
}

// ============================================================================
// HISTORY IMPORT
// ============================================================================

/// Import browsing history from a browser profile
pub fn import_history(profile: &BrowserProfile, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    match profile.browser_type.as_str() {
        "chrome" | "brave" | "edge" | "opera" | "vivaldi" | "chromium" => {
            import_chromium_history(&profile.profile_path, limit)
        }
        "firefox" => import_firefox_history(&profile.profile_path, limit),
        _ => Err(format!("Unsupported browser type: {}", profile.browser_type)),
    }
}

/// Import history from Chrome-family browser (SQLite)
fn import_chromium_history(profile_path: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let db_path = Path::new(profile_path).join("History");

    // Copy to avoid lock conflicts
    let temp_path = std::env::temp_dir().join("impforge_chrome_history.sqlite");
    std::fs::copy(&db_path, &temp_path)
        .map_err(|e| format!("Failed to copy Chrome history database: {e}"))?;

    let conn = Connection::open(&temp_path)
        .map_err(|e| format!("Failed to open Chrome history database: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, last_visit_time
             FROM urls
             ORDER BY last_visit_time DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("SQL error: {e}"))?;

    let entries = stmt
        .query_map([limit as i64], |row| {
            let last_visit_raw: i64 = row.get(3).unwrap_or(0);
            // Chrome stores time as microseconds since 1601-01-01
            // Convert to Unix timestamp: subtract 11644473600000000 microseconds
            let unix_us = last_visit_raw - 11_644_473_600_000_000;
            let unix_secs = unix_us / 1_000_000;
            let datetime = chrono::DateTime::from_timestamp(unix_secs, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string());

            Ok(HistoryEntry {
                url: row.get(0).unwrap_or_default(),
                title: row.get(1).unwrap_or_default(),
                visit_count: row.get(2).unwrap_or(0),
                last_visit: datetime,
            })
        })
        .map_err(|e| format!("Query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let _ = std::fs::remove_file(&temp_path);

    Ok(entries)
}

/// Import history from Firefox (SQLite places.sqlite)
fn import_firefox_history(profile_path: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let db_path = Path::new(profile_path).join("places.sqlite");

    let temp_path = std::env::temp_dir().join("impforge_firefox_history.sqlite");
    std::fs::copy(&db_path, &temp_path)
        .map_err(|e| format!("Failed to copy Firefox history database: {e}"))?;

    let conn = Connection::open(&temp_path)
        .map_err(|e| format!("Failed to open Firefox history database: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, last_visit_date
             FROM moz_places
             WHERE visit_count > 0
             ORDER BY last_visit_date DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("SQL error: {e}"))?;

    let entries = stmt
        .query_map([limit as i64], |row| {
            let last_visit_raw: i64 = row.get(3).unwrap_or(0);
            // Firefox stores time as microseconds since Unix epoch
            let unix_secs = last_visit_raw / 1_000_000;
            let datetime = chrono::DateTime::from_timestamp(unix_secs, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string());

            Ok(HistoryEntry {
                url: row.get(0).unwrap_or_default(),
                title: row.get::<_, Option<String>>(1).ok().flatten().unwrap_or_default(),
                visit_count: row.get(2).unwrap_or(0),
                last_visit: datetime,
            })
        })
        .map_err(|e| format!("Query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let _ = std::fs::remove_file(&temp_path);

    Ok(entries)
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Detect all browser profiles on the system
#[tauri::command]
pub async fn browser_detect_profiles() -> Result<Vec<BrowserProfile>, String> {
    Ok(detect_profiles())
}

/// Import bookmarks from a specific browser profile
#[tauri::command]
pub async fn browser_import_bookmarks(
    profile_path: String,
    browser_type: String,
) -> Result<Vec<Bookmark>, String> {
    let profile = BrowserProfile {
        browser_name: String::new(),
        browser_type,
        profile_path,
        profile_name: String::new(),
        has_bookmarks: true,
        has_history: false,
        has_passwords: false,
    };
    import_bookmarks(&profile)
}

/// Import history from a specific browser profile
#[tauri::command]
pub async fn browser_import_history(
    profile_path: String,
    browser_type: String,
    limit: Option<usize>,
) -> Result<Vec<HistoryEntry>, String> {
    let profile = BrowserProfile {
        browser_name: String::new(),
        browser_type,
        profile_path,
        profile_name: String::new(),
        has_bookmarks: false,
        has_history: true,
        has_passwords: false,
    };
    import_history(&profile, limit.unwrap_or(500))
}

/// Import all data from a browser profile (bookmarks + history)
#[tauri::command]
pub async fn browser_import_all(
    profile_path: String,
    browser_type: String,
    browser_name: String,
) -> Result<ImportResult, String> {
    let profile = BrowserProfile {
        browser_name: browser_name.clone(),
        browser_type,
        profile_path,
        profile_name: String::new(),
        has_bookmarks: true,
        has_history: true,
        has_passwords: false,
    };

    let mut errors = Vec::new();

    let bookmarks_count = match import_bookmarks(&profile) {
        Ok(b) => b.len(),
        Err(e) => {
            errors.push(format!("Bookmarks: {e}"));
            0
        }
    };

    let history_count = match import_history(&profile, 1000) {
        Ok(h) => h.len(),
        Err(e) => {
            errors.push(format!("History: {e}"));
            0
        }
    };

    Ok(ImportResult {
        browser_name,
        bookmarks_imported: bookmarks_count,
        history_imported: history_count,
        errors,
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_profiles() {
        let profiles = detect_profiles();
        // Should find at least Brave on this system
        for p in &profiles {
            assert!(!p.browser_name.is_empty());
            assert!(!p.profile_path.is_empty());
        }
    }

    #[test]
    fn test_chromium_bookmark_parsing() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "children": [
                        {
                            "name": "Example Site",
                            "type": "url",
                            "url": "https://example.com",
                            "date_added": "13300000000000000"
                        },
                        {
                            "name": "Dev Tools",
                            "type": "folder",
                            "children": [
                                {
                                    "name": "Rust Docs",
                                    "type": "url",
                                    "url": "https://doc.rust-lang.org"
                                }
                            ]
                        }
                    ],
                    "type": "folder",
                    "name": "Bookmarks Bar"
                },
                "other": {
                    "children": [],
                    "type": "folder",
                    "name": "Other Bookmarks"
                }
            }
        }"#;

        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut bookmarks = Vec::new();

        if let Some(roots) = parsed.get("roots").and_then(|v| v.as_object()) {
            for (folder_name, folder) in roots {
                extract_chrome_bookmarks(folder, folder_name, &mut bookmarks);
            }
        }

        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].title, "Example Site");
        assert_eq!(bookmarks[0].url, "https://example.com");
        assert_eq!(bookmarks[1].title, "Rust Docs");
        assert_eq!(bookmarks[1].folder, "Dev Tools");
    }

    #[test]
    fn test_chrome_timestamp_conversion() {
        // Chrome epoch: 1601-01-01 00:00:00 UTC
        // Test: 13300000000000000 microseconds since Chrome epoch
        let chrome_time: i64 = 13_300_000_000_000_000;
        let unix_us = chrome_time - 11_644_473_600_000_000;
        let unix_secs = unix_us / 1_000_000;
        // Should be roughly 2022 (1655526400 = 2022-06-18)
        assert!(unix_secs > 1_600_000_000); // After 2020
        assert!(unix_secs < 2_000_000_000); // Before 2033
    }

    #[test]
    fn test_browser_profile_serialization() {
        let profile = BrowserProfile {
            browser_name: "Brave".to_string(),
            browser_type: "brave".to_string(),
            profile_path: "/home/user/.config/BraveSoftware/Brave-Browser/Default".to_string(),
            profile_name: "Default".to_string(),
            has_bookmarks: true,
            has_history: true,
            has_passwords: true,
        };

        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("Brave"));
        assert!(json.contains("brave"));
        assert!(json.contains("has_bookmarks"));
    }

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            browser_name: "Chrome".to_string(),
            bookmarks_imported: 42,
            history_imported: 1000,
            errors: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: ImportResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bookmarks_imported, 42);
        assert_eq!(parsed.history_imported, 1000);
        assert!(parsed.errors.is_empty());
    }

    #[test]
    fn test_bookmark_struct() {
        let bookmark = Bookmark {
            title: "Test".to_string(),
            url: "https://test.com".to_string(),
            folder: "Root".to_string(),
            date_added: Some(1234567890),
        };
        assert_eq!(bookmark.title, "Test");
        assert_eq!(bookmark.url, "https://test.com");
    }

    #[test]
    fn test_history_entry_struct() {
        let entry = HistoryEntry {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            visit_count: 5,
            last_visit: "2026-01-01T00:00:00+00:00".to_string(),
        };
        assert_eq!(entry.visit_count, 5);
    }
}
