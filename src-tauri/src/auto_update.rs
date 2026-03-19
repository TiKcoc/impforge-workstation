//! Auto-Update System for ImpForge
//!
//! Checks the GitHub releases API for newer versions and reports
//! update availability to the frontend. The actual download is
//! handled by opening the release page in the user's browser —
//! keeping the update flow transparent and auditable.
//!
//! Works fully offline-safe: a failed network request simply
//! returns "no update available" without crashing.

use serde::{Deserialize, Serialize};

/// GitHub repository coordinates used for update checks.
const GITHUB_OWNER: &str = "AiImpDevelopment";
const GITHUB_REPO: &str = "impforge";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Information about the current and latest available version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Version currently running (from Cargo.toml).
    pub current_version: String,
    /// Latest version tag from GitHub releases (if reachable).
    pub latest_version: Option<String>,
    /// Whether a newer version is available.
    pub update_available: bool,
    /// Markdown release notes from the GitHub release body.
    pub release_notes: Option<String>,
    /// URL to the GitHub release page for manual download.
    pub download_url: Option<String>,
    /// ISO-8601 timestamp of when this check was performed.
    pub last_checked: String,
}

// ---------------------------------------------------------------------------
// Version comparison
// ---------------------------------------------------------------------------

/// Parse a semver string into a comparable tuple.
/// Handles formats like "0.6.0", "v0.6.0", "0.6.0-beta.1".
/// Only the major.minor.patch portions are compared.
fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.trim().trim_start_matches('v');
    // Take only the part before any pre-release suffix
    let base = v.split('-').next().unwrap_or(v);
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2].parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Returns `true` when `latest` is strictly newer than `current`.
fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Check GitHub releases for a newer version of ImpForge.
///
/// This is fully offline-safe: network failures return a result with
/// `update_available: false` and `latest_version: None`.
#[tauri::command]
pub async fn update_check() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION");
    let now = chrono::Utc::now().to_rfc3339();

    let url = format!(
        "https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = match client
        .get(&url)
        .header("User-Agent", format!("ImpForge/{current}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            // Network unreachable — perfectly fine for offline-first.
            return Ok(UpdateInfo {
                current_version: current.into(),
                latest_version: None,
                update_available: false,
                release_notes: None,
                download_url: None,
                last_checked: now,
            });
        }
    };

    if !resp.status().is_success() {
        return Ok(UpdateInfo {
            current_version: current.into(),
            latest_version: None,
            update_available: false,
            release_notes: None,
            download_url: None,
            last_checked: now,
        });
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))?;

    let tag = body["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    let latest = if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    };

    let update_available = latest
        .as_deref()
        .map(|l| is_newer(current, l))
        .unwrap_or(false);

    Ok(UpdateInfo {
        current_version: current.into(),
        latest_version: latest,
        update_available,
        release_notes: body["body"].as_str().map(String::from),
        download_url: body["html_url"].as_str().map(String::from),
        last_checked: now,
    })
}

/// Return the currently running version (from Cargo.toml at compile time).
#[tauri::command]
pub fn update_current_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_basic() {
        assert_eq!(parse_semver("0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("2.0.0-beta.1"), Some((2, 0, 0)));
    }

    #[test]
    fn parse_semver_invalid() {
        assert_eq!(parse_semver(""), None);
        assert_eq!(parse_semver("abc"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn is_newer_detects_upgrade() {
        assert!(is_newer("0.6.0", "0.7.0"));
        assert!(is_newer("0.6.0", "1.0.0"));
        assert!(is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn is_newer_same_version() {
        assert!(!is_newer("0.6.0", "0.6.0"));
    }

    #[test]
    fn is_newer_downgrade() {
        assert!(!is_newer("1.0.0", "0.9.0"));
        assert!(!is_newer("0.7.0", "0.6.0"));
    }

    #[test]
    fn is_newer_with_prefix() {
        assert!(is_newer("0.6.0", "v0.7.0"));
        assert!(is_newer("v0.6.0", "0.7.0"));
    }

    #[test]
    fn update_info_serialization() {
        let info = UpdateInfo {
            current_version: "0.6.0".into(),
            latest_version: Some("0.7.0".into()),
            update_available: true,
            release_notes: Some("Bug fixes".into()),
            download_url: Some("https://example.com".into()),
            last_checked: "2026-03-18T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        let deser: UpdateInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.current_version, "0.6.0");
        assert!(deser.update_available);
    }

    #[test]
    fn current_version_is_set() {
        let v = env!("CARGO_PKG_VERSION");
        assert!(!v.is_empty());
    }
}
