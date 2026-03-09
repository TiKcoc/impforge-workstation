// SPDX-License-Identifier: Apache-2.0
//! Billing & License Management — Subscription tiers, feature gating, offline verification
//!
//! Provides a complete license management system for ImpForge CodeForge IDE:
//!   - Six subscription tiers (Free through Enterprise)
//!   - Feature gating per tier with usage tracking
//!   - HMAC-SHA256 offline license verification (no external auth server required)
//!   - Cached license persistence at `~/.impforge/license.json`
//!   - 30-day grace period for offline operation
//!   - Periodic phone-home every 7 days (soft-fail when offline)
//!
//! Design decisions:
//!   - All crypto uses `hmac` + `sha2` crates already in Cargo.toml
//!   - `parking_lot::Mutex` for state (consistent with rest of codebase)
//!   - `tokio::fs` for async file I/O (consistent with shadow.rs, mod.rs)
//!   - `chrono` for date handling (already a dependency)
//!   - License keys are verified locally via HMAC; Stripe is a future integration

use chrono::{NaiveDate, Utc};
use hmac::{Hmac, Mac};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use tokio::fs;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// HMAC secret for offline license verification.
/// In production this would be loaded from a secure keychain or compiled
/// with a build-time environment variable.  Placed here as a constant so
/// the module is fully self-contained and testable without external config.
const LICENSE_HMAC_SECRET: &[u8] = b"impforge-license-hmac-secret-2026-production";

/// How many days a license remains valid after the last successful
/// phone-home before features degrade to Free tier.
const GRACE_PERIOD_DAYS: i64 = 30;

/// Phone-home interval in days.  The app attempts to re-verify the license
/// with the licensing server every N days.  Failure is tolerated silently
/// as long as the grace period has not elapsed.
const PHONE_HOME_INTERVAL_DAYS: i64 = 7;

/// Stripe checkout base URL (placeholder — replaced with real endpoint
/// once Stripe integration is wired up).
const STRIPE_CHECKOUT_BASE: &str = "https://checkout.impforge.dev/subscribe";

/// Default daily AI completion quota for the Free tier.
const FREE_COMPLETIONS_PER_DAY: u32 = 50;

/// Default daily AI completion quota for the Starter tier.
const STARTER_COMPLETIONS_PER_DAY: u32 = 500;

/// Sentinel value meaning "unlimited".
const UNLIMITED: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// Subscription Tiers
// ---------------------------------------------------------------------------

/// The six subscription tiers for ImpForge CodeForge IDE.
///
/// Each tier unlocks progressively more features.  The `Free` tier is
/// always available and requires no license key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubscriptionTier {
    /// Free tier -- basic editor, 1 AI model, no collaboration.
    Free,
    /// Starter -- EUR 29/mo, unlimited AI, 3 models, basic collaboration.
    Starter,
    /// Pro -- EUR 45/mo, all AI models, advanced AI features, priority support.
    Pro,
    /// Team -- EUR 60/user/mo, real-time collaboration, CRDT, admin dashboard.
    Team,
    /// Business -- EUR 75/user/mo, SSO/SAML, audit logs, SLA.
    Business,
    /// Enterprise -- EUR 90/user/mo, custom models, dedicated support, on-prem.
    Enterprise,
}

impl SubscriptionTier {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Starter => "Starter",
            Self::Pro => "Pro",
            Self::Team => "Team",
            Self::Business => "Business",
            Self::Enterprise => "Enterprise",
        }
    }

    /// Monthly price in EUR cents (per user for Team/Business/Enterprise).
    /// Free returns 0.
    pub fn monthly_price_cents(&self) -> u32 {
        match self {
            Self::Free => 0,
            Self::Starter => 2900,
            Self::Pro => 4500,
            Self::Team => 6000,
            Self::Business => 7500,
            Self::Enterprise => 9000,
        }
    }

    /// Yearly price in EUR cents (per user, with discount).
    pub fn yearly_price_cents(&self) -> u32 {
        match self {
            Self::Free => 0,
            Self::Starter => 2900 * 10, // ~17% off (10 months for 12)
            Self::Pro => 4500 * 10,
            Self::Team => 6000 * 10,    // ~17% off
            Self::Business => 7500 * 10,
            Self::Enterprise => 0, // custom pricing
        }
    }

    /// Parse a tier from its string name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "free" => Some(Self::Free),
            "starter" => Some(Self::Starter),
            "pro" => Some(Self::Pro),
            "team" => Some(Self::Team),
            "business" => Some(Self::Business),
            "enterprise" => Some(Self::Enterprise),
            _ => None,
        }
    }
}

impl std::fmt::Display for SubscriptionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ---------------------------------------------------------------------------
// Feature Gating
// ---------------------------------------------------------------------------

/// Feature flags and quotas determined by the active subscription tier.
///
/// This is the single source of truth for what each tier can do.
/// The frontend reads this to show/hide UI elements; the backend checks
/// it before executing gated operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierFeatures {
    /// Maximum number of AI models the user can configure.
    /// Free=1, Starter=3, Pro+=unlimited.
    pub max_ai_models: u32,
    /// AI code completions allowed per calendar day.
    /// Free=50, Starter=500, Pro+=unlimited.
    pub ai_completions_per_day: u32,
    /// Whether real-time collaboration is enabled.
    pub collab_enabled: bool,
    /// Maximum team members (0 = solo, UNLIMITED for Enterprise).
    pub max_team_members: u32,
    /// Shadow workspace for isolated AI edits.
    pub shadow_workspace: bool,
    /// Custom theme import/export.
    pub custom_themes: bool,
    /// Debug Adapter Protocol support.
    pub debug_adapter: bool,
    /// Git integration (always on).
    pub git_integration: bool,
    /// Language Server Protocol support (always on).
    pub lsp_support: bool,
    /// Priority support channel.
    pub priority_support: bool,
    /// SSO / SAML authentication.
    pub sso_saml: bool,
    /// Audit logging for compliance.
    pub audit_logs: bool,
    /// Custom model training / fine-tuning.
    pub custom_models: bool,
    /// On-premise deployment option.
    pub on_premise: bool,
}

impl TierFeatures {
    /// Build the feature set for a given tier.
    pub fn for_tier(tier: &SubscriptionTier) -> Self {
        match tier {
            SubscriptionTier::Free => Self {
                max_ai_models: 1,
                ai_completions_per_day: FREE_COMPLETIONS_PER_DAY,
                collab_enabled: false,
                max_team_members: 0,
                shadow_workspace: false,
                custom_themes: false,
                debug_adapter: false,
                git_integration: true,
                lsp_support: true,
                priority_support: false,
                sso_saml: false,
                audit_logs: false,
                custom_models: false,
                on_premise: false,
            },
            SubscriptionTier::Starter => Self {
                max_ai_models: 3,
                ai_completions_per_day: STARTER_COMPLETIONS_PER_DAY,
                collab_enabled: false,
                max_team_members: 0,
                shadow_workspace: true,
                custom_themes: false,
                debug_adapter: true,
                git_integration: true,
                lsp_support: true,
                priority_support: false,
                sso_saml: false,
                audit_logs: false,
                custom_models: false,
                on_premise: false,
            },
            SubscriptionTier::Pro => Self {
                max_ai_models: UNLIMITED,
                ai_completions_per_day: UNLIMITED,
                collab_enabled: false,
                max_team_members: 0,
                shadow_workspace: true,
                custom_themes: true,
                debug_adapter: true,
                git_integration: true,
                lsp_support: true,
                priority_support: false,
                sso_saml: false,
                audit_logs: false,
                custom_models: false,
                on_premise: false,
            },
            SubscriptionTier::Team => Self {
                max_ai_models: UNLIMITED,
                ai_completions_per_day: UNLIMITED,
                collab_enabled: true,
                max_team_members: 10,
                shadow_workspace: true,
                custom_themes: true,
                debug_adapter: true,
                git_integration: true,
                lsp_support: true,
                priority_support: false,
                sso_saml: false,
                audit_logs: false,
                custom_models: false,
                on_premise: false,
            },
            SubscriptionTier::Business => Self {
                max_ai_models: UNLIMITED,
                ai_completions_per_day: UNLIMITED,
                collab_enabled: true,
                max_team_members: 50,
                shadow_workspace: true,
                custom_themes: true,
                debug_adapter: true,
                git_integration: true,
                lsp_support: true,
                priority_support: true,
                sso_saml: true,
                audit_logs: true,
                custom_models: false,
                on_premise: false,
            },
            SubscriptionTier::Enterprise => Self {
                max_ai_models: UNLIMITED,
                ai_completions_per_day: UNLIMITED,
                collab_enabled: true,
                max_team_members: UNLIMITED,
                shadow_workspace: true,
                custom_themes: true,
                debug_adapter: true,
                git_integration: true,
                lsp_support: true,
                priority_support: true,
                sso_saml: true,
                audit_logs: true,
                custom_models: true,
                on_premise: true,
            },
        }
    }

    /// Check whether a named feature is enabled.
    pub fn has_feature(&self, feature: &str) -> bool {
        match feature {
            "ai_models" => self.max_ai_models > 0,
            "ai_completions" => self.ai_completions_per_day > 0,
            "collab" | "collaboration" => self.collab_enabled,
            "team" | "team_members" => self.max_team_members > 0,
            "shadow_workspace" | "shadow" => self.shadow_workspace,
            "custom_themes" | "themes" => self.custom_themes,
            "debug_adapter" | "debug" | "dap" => self.debug_adapter,
            "git" | "git_integration" => self.git_integration,
            "lsp" | "lsp_support" => self.lsp_support,
            "priority_support" | "support" => self.priority_support,
            "sso" | "saml" | "sso_saml" => self.sso_saml,
            "audit" | "audit_logs" => self.audit_logs,
            "custom_models" | "fine_tuning" => self.custom_models,
            "on_premise" | "on_prem" => self.on_premise,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// License Info
// ---------------------------------------------------------------------------

/// Persisted license information.
///
/// Cached to `~/.impforge/license.json` and verified on startup.
/// The `signature` field is an HMAC-SHA256 hex digest computed over
/// the canonical license payload, allowing offline verification
/// without contacting a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// Unique user identifier (UUID).
    pub user_id: String,
    /// User email address.
    pub email: String,
    /// Active subscription tier.
    pub tier: SubscriptionTier,
    /// Computed feature flags for this tier.
    pub features: TierFeatures,
    /// Expiry date in ISO 8601 format (YYYY-MM-DD).
    pub valid_until: String,
    /// The license key that was activated.
    pub license_key: String,
    /// Team identifier (if Team+ tier).
    pub team_id: Option<String>,
    /// Team display name (if Team+ tier).
    pub team_name: Option<String>,
    /// Number of seats purchased (if Team+ tier).
    pub seat_count: Option<u32>,
    /// When the license was issued (ISO 8601).
    pub issued_at: String,
    /// HMAC-SHA256 hex signature for offline verification.
    pub signature: String,
    /// Timestamp of the last successful phone-home (ISO 8601).
    #[serde(default)]
    pub last_verified: Option<String>,
}

/// Daily usage counters, reset at midnight UTC.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageCounters {
    /// Number of AI completions used today.
    pub ai_completions_today: u32,
    /// The date these counters apply to (YYYY-MM-DD).
    pub date: String,
}

// ---------------------------------------------------------------------------
// License Manager (Tauri managed state)
// ---------------------------------------------------------------------------

/// Thread-safe license manager stored as Tauri managed state.
///
/// Handles license activation, caching, verification, and usage tracking.
pub struct LicenseManager {
    license: Mutex<Option<LicenseInfo>>,
    usage: Mutex<UsageCounters>,
    cache_path: PathBuf,
}

impl LicenseManager {
    /// Create a new LicenseManager.
    ///
    /// The cache path defaults to `~/.impforge/license.json`.
    pub fn new() -> Self {
        let cache_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".impforge")
            .join("license.json");

        Self {
            license: Mutex::new(None),
            usage: Mutex::new(UsageCounters::default()),
            cache_path,
        }
    }

    /// Load a cached license from disk (if present and valid).
    ///
    /// Called once during app startup.  If the cached license has expired
    /// or fails signature verification it is discarded silently and the
    /// user falls back to the Free tier.
    pub async fn load_cached(&self) -> Option<LicenseInfo> {
        let data = fs::read_to_string(&self.cache_path).await.ok()?;
        let info: LicenseInfo = serde_json::from_str(&data).ok()?;

        // Verify signature
        if !verify_signature(&info) {
            log::warn!("Cached license has invalid signature — ignoring");
            return None;
        }

        // Check hard expiry
        if is_expired(&info.valid_until) {
            log::info!("Cached license expired on {} — falling back to Free", info.valid_until);
            return None;
        }

        // Check grace period (days since last verified)
        if let Some(ref last) = info.last_verified {
            if days_since(last) > GRACE_PERIOD_DAYS {
                log::warn!(
                    "License not verified for {} days (grace={}) — degrading to Free",
                    days_since(last),
                    GRACE_PERIOD_DAYS
                );
                return None;
            }
        }

        let mut lock = self.license.lock();
        *lock = Some(info.clone());
        Some(info)
    }

    /// Persist the current license to disk.
    async fn save_to_disk(&self) -> Result<(), String> {
        let info = {
            let lock = self.license.lock();
            lock.clone()
        };

        let Some(info) = info else {
            return Ok(());
        };

        // Ensure parent directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create license directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(&info)
            .map_err(|e| format!("Failed to serialize license: {}", e))?;

        fs::write(&self.cache_path, json)
            .await
            .map_err(|e| format!("Failed to write license cache: {}", e))?;

        Ok(())
    }

    /// Get the current effective tier.
    pub fn current_tier(&self) -> SubscriptionTier {
        let lock = self.license.lock();
        lock.as_ref()
            .map(|l| l.tier.clone())
            .unwrap_or(SubscriptionTier::Free)
    }

    /// Get the current feature set.
    pub fn current_features(&self) -> TierFeatures {
        TierFeatures::for_tier(&self.current_tier())
    }

    /// Record one AI completion.  Returns `true` if within quota, `false` if over.
    pub fn record_completion(&self) -> bool {
        let features = self.current_features();
        if features.ai_completions_per_day == UNLIMITED {
            return true;
        }

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let mut usage = self.usage.lock();

        // Reset counters on new day
        if usage.date != today {
            usage.ai_completions_today = 0;
            usage.date = today;
        }

        if usage.ai_completions_today >= features.ai_completions_per_day {
            return false;
        }

        usage.ai_completions_today += 1;
        true
    }
}

impl Default for LicenseManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cryptographic helpers
// ---------------------------------------------------------------------------

type HmacSha256 = Hmac<Sha256>;

/// Build the canonical payload string that is signed.
///
/// The payload is deterministic: fields are concatenated in a fixed order
/// separated by `|` so that both license generation and verification
/// produce the same input to HMAC.
fn canonical_payload(info: &LicenseInfo) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        info.user_id,
        info.email,
        info.tier.display_name(),
        info.valid_until,
        info.license_key,
        info.issued_at,
    )
}

/// Compute the HMAC-SHA256 signature for a license.
fn compute_signature(info: &LicenseInfo) -> String {
    let payload = canonical_payload(info);
    let mut mac =
        HmacSha256::new_from_slice(LICENSE_HMAC_SECRET).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify that a license's signature matches its payload.
fn verify_signature(info: &LicenseInfo) -> bool {
    let expected = compute_signature(info);
    // Constant-time comparison to prevent timing attacks
    constant_time_eq(expected.as_bytes(), info.signature.as_bytes())
}

/// Constant-time byte comparison (prevents timing side-channels).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Check if a YYYY-MM-DD date string is in the past.
fn is_expired(date_str: &str) -> bool {
    let Ok(expiry) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return true; // unparseable = expired
    };
    let today = Utc::now().date_naive();
    expiry < today
}

/// Number of days elapsed since a YYYY-MM-DD date string.
fn days_since(date_str: &str) -> i64 {
    let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return i64::MAX;
    };
    let today = Utc::now().date_naive();
    (today - date).num_days()
}

// ---------------------------------------------------------------------------
// License key parsing
// ---------------------------------------------------------------------------

/// Decode a license key into a `LicenseInfo`.
///
/// License keys are base64-encoded JSON payloads prefixed with `IF-`.
/// Example: `IF-eyJ1c2VyX2lkIjoi...`
///
/// In a production system the key would be a compact JWT or an opaque
/// token validated against a server.  For offline-first operation we
/// embed the full payload and verify via HMAC.
fn decode_license_key(key: &str) -> Result<LicenseInfo, String> {
    let stripped = key
        .trim()
        .strip_prefix("IF-")
        .ok_or_else(|| "Invalid license key format: must start with 'IF-'".to_string())?;

    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        stripped,
    )
    .map_err(|e| format!("Invalid license key encoding: {}", e))?;

    let info: LicenseInfo = serde_json::from_slice(&decoded)
        .map_err(|e| format!("Invalid license key payload: {}", e))?;

    Ok(info)
}

/// Encode a `LicenseInfo` into a license key string.
///
/// Used by the licensing server (or tests) to generate keys.
#[allow(dead_code)]
fn encode_license_key(info: &LicenseInfo) -> String {
    let json = serde_json::to_vec(info).expect("LicenseInfo is always serialisable");
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &json);
    format!("IF-{}", encoded)
}

// ---------------------------------------------------------------------------
// Tauri IPC Commands
// ---------------------------------------------------------------------------

/// Activate a license key.
///
/// Decodes and verifies the key, stores it in memory and persists it to disk.
/// Returns the full `LicenseInfo` on success.
#[tauri::command]
pub async fn billing_activate_license(
    manager: tauri::State<'_, LicenseManager>,
    license_key: String,
) -> Result<LicenseInfo, String> {
    let mut info = decode_license_key(&license_key)?;

    // Validate that the tier name in the payload maps to a known tier
    let _validated_tier = SubscriptionTier::from_name(info.tier.display_name())
        .ok_or_else(|| format!("Unknown subscription tier: {}", info.tier.display_name()))?;

    // Verify HMAC signature
    if !verify_signature(&info) {
        return Err("License key signature verification failed".to_string());
    }

    // Check expiry
    if is_expired(&info.valid_until) {
        return Err(format!(
            "License expired on {}. Please renew your subscription.",
            info.valid_until
        ));
    }

    // Stamp the verification time
    info.last_verified = Some(Utc::now().format("%Y-%m-%d").to_string());

    // Store in memory
    {
        let mut lock = manager.license.lock();
        *lock = Some(info.clone());
    }

    // Persist to disk
    manager.save_to_disk().await?;

    log::info!(
        "License activated: tier={}, user={}, valid_until={}",
        info.tier,
        info.email,
        info.valid_until
    );

    Ok(info)
}

/// Get the currently active license (if any).
///
/// If no license is currently loaded in memory, attempts to load a cached
/// license from `~/.impforge/license.json` before returning.
#[tauri::command]
pub async fn billing_get_license(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<Option<LicenseInfo>, String> {
    // Check in-memory first
    {
        let lock = manager.license.lock();
        if lock.is_some() {
            return Ok(lock.clone());
        }
    }

    // Try loading from disk cache
    if let Some(cached) = manager.load_cached().await {
        log::info!("Loaded cached license: tier={}, user={}", cached.tier, cached.email);
        return Ok(Some(cached));
    }

    Ok(None)
}

/// Check whether a named feature is available under the current license.
///
/// Feature names: `"shadow"`, `"collab"`, `"debug"`, `"themes"`, `"sso"`,
/// `"audit"`, `"custom_models"`, `"on_prem"`, `"git"`, `"lsp"`, etc.
#[tauri::command]
pub async fn billing_check_feature(
    manager: tauri::State<'_, LicenseManager>,
    feature: String,
) -> Result<bool, String> {
    let features = manager.current_features();
    Ok(features.has_feature(&feature))
}

/// Get the name of the current subscription tier.
#[tauri::command]
pub async fn billing_get_tier(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<String, String> {
    Ok(manager.current_tier().display_name().to_string())
}

/// Get current usage statistics as a JSON value.
///
/// Returns AI completions used today, quota, tier info, and grace period status.
#[tauri::command]
pub async fn billing_get_usage(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<serde_json::Value, String> {
    let tier = manager.current_tier();
    let features = manager.current_features();

    let (completions_today, usage_date) = {
        let usage = manager.usage.lock();
        (usage.ai_completions_today, usage.date.clone())
    };

    let (days_until_expiry, last_verified_days) = {
        let lock = manager.license.lock();
        match lock.as_ref() {
            Some(info) => {
                let expiry_days = -days_since(&info.valid_until); // negative = days remaining
                let verified_days = info
                    .last_verified
                    .as_deref()
                    .map(days_since)
                    .unwrap_or(0);
                (expiry_days, verified_days)
            }
            None => (0, 0),
        }
    };

    let quota_display = if features.ai_completions_per_day == UNLIMITED {
        "unlimited".to_string()
    } else {
        features.ai_completions_per_day.to_string()
    };

    Ok(serde_json::json!({
        "tier": tier.display_name(),
        "ai_completions_today": completions_today,
        "ai_completions_quota": quota_display,
        "usage_date": usage_date,
        "days_until_expiry": days_until_expiry,
        "days_since_verified": last_verified_days,
        "grace_period_days": GRACE_PERIOD_DAYS,
        "needs_phone_home": last_verified_days >= PHONE_HOME_INTERVAL_DAYS,
    }))
}

/// Record an AI completion against the daily usage quota.
///
/// Returns `true` if the completion was within quota, `false` if over the
/// daily limit for the current tier.  The frontend should call this after
/// every successful AI completion to enforce rate limits.
#[tauri::command]
pub async fn billing_record_completion(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<bool, String> {
    Ok(manager.record_completion())
}

/// Deactivate the current license and revert to Free tier.
///
/// Removes the cached license file from disk.
#[tauri::command]
pub async fn billing_deactivate(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<(), String> {
    {
        let mut lock = manager.license.lock();
        *lock = None;
    }

    // Remove cached file (best-effort)
    let _ = fs::remove_file(&manager.cache_path).await;

    log::info!("License deactivated — reverted to Free tier");
    Ok(())
}

/// Get full pricing information for all tiers.
///
/// Returns an array of JSON objects suitable for rendering the pricing UI.
#[tauri::command]
pub async fn billing_get_pricing() -> Result<Vec<serde_json::Value>, String> {
    let tiers = [
        SubscriptionTier::Free,
        SubscriptionTier::Starter,
        SubscriptionTier::Pro,
        SubscriptionTier::Team,
        SubscriptionTier::Business,
        SubscriptionTier::Enterprise,
    ];

    let pricing: Vec<serde_json::Value> = tiers
        .iter()
        .map(|tier| {
            let features = TierFeatures::for_tier(tier);
            let models_display = if features.max_ai_models == UNLIMITED {
                "Unlimited".to_string()
            } else {
                features.max_ai_models.to_string()
            };
            let completions_display = if features.ai_completions_per_day == UNLIMITED {
                "Unlimited".to_string()
            } else {
                features.ai_completions_per_day.to_string()
            };
            let team_display = if features.max_team_members == UNLIMITED {
                "Unlimited".to_string()
            } else if features.max_team_members == 0 {
                "-".to_string()
            } else {
                features.max_team_members.to_string()
            };

            serde_json::json!({
                "id": tier.display_name().to_lowercase(),
                "name": tier.display_name(),
                "price_monthly_cents": tier.monthly_price_cents(),
                "price_yearly_cents": tier.yearly_price_cents(),
                "is_per_user": matches!(tier, SubscriptionTier::Team | SubscriptionTier::Business | SubscriptionTier::Enterprise),
                "popular": matches!(tier, SubscriptionTier::Pro),
                "features": {
                    "ai_completion": true,
                    "ai_models": models_display,
                    "ai_completions_per_day": completions_display,
                    "shadow_workspace": features.shadow_workspace,
                    "debug_adapter": features.debug_adapter,
                    "custom_themes": features.custom_themes,
                    "collab": features.collab_enabled,
                    "team_members": team_display,
                    "sso_saml": features.sso_saml,
                    "audit_logs": features.audit_logs,
                    "custom_models": features.custom_models,
                    "on_premise": features.on_premise,
                    "priority_support": features.priority_support,
                    "git_integration": features.git_integration,
                    "lsp_support": features.lsp_support,
                },
                "checkout_url": format!("{}/{}", STRIPE_CHECKOUT_BASE, tier.display_name().to_lowercase()),
            })
        })
        .collect();

    Ok(pricing)
}

/// Get team members for the current license (if Team+ tier).
///
/// In the current offline-first implementation this returns placeholder data.
/// A future Stripe/backend integration will return real seat assignments.
#[tauri::command]
pub async fn billing_team_members(
    manager: tauri::State<'_, LicenseManager>,
) -> Result<Vec<serde_json::Value>, String> {
    let lock = manager.license.lock();
    let info = lock
        .as_ref()
        .ok_or_else(|| "No active license".to_string())?;

    if info.tier < SubscriptionTier::Team {
        return Err("Team management requires a Team, Business, or Enterprise plan".to_string());
    }

    // Placeholder: return the license holder as the sole member.
    // Real implementation would query a team management API.
    let members = vec![serde_json::json!({
        "user_id": info.user_id,
        "email": info.email,
        "role": "owner",
        "joined_at": info.issued_at,
        "status": "active",
    })];

    Ok(members)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a test license for a given tier.
    fn make_test_license(tier: SubscriptionTier) -> LicenseInfo {
        let mut info = LicenseInfo {
            user_id: "test-user-001".to_string(),
            email: "dev@impforge.dev".to_string(),
            tier: tier.clone(),
            features: TierFeatures::for_tier(&tier),
            valid_until: "2027-12-31".to_string(),
            license_key: "IF-test".to_string(),
            team_id: None,
            team_name: None,
            seat_count: None,
            issued_at: "2026-01-01".to_string(),
            signature: String::new(),
            last_verified: Some("2026-03-09".to_string()),
        };
        info.signature = compute_signature(&info);
        info
    }

    #[test]
    fn test_tier_display_names() {
        assert_eq!(SubscriptionTier::Free.display_name(), "Free");
        assert_eq!(SubscriptionTier::Starter.display_name(), "Starter");
        assert_eq!(SubscriptionTier::Pro.display_name(), "Pro");
        assert_eq!(SubscriptionTier::Team.display_name(), "Team");
        assert_eq!(SubscriptionTier::Business.display_name(), "Business");
        assert_eq!(SubscriptionTier::Enterprise.display_name(), "Enterprise");
    }

    #[test]
    fn test_tier_from_name() {
        assert_eq!(SubscriptionTier::from_name("pro"), Some(SubscriptionTier::Pro));
        assert_eq!(SubscriptionTier::from_name("PRO"), Some(SubscriptionTier::Pro));
        assert_eq!(SubscriptionTier::from_name("Enterprise"), Some(SubscriptionTier::Enterprise));
        assert_eq!(SubscriptionTier::from_name("invalid"), None);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(SubscriptionTier::Free < SubscriptionTier::Starter);
        assert!(SubscriptionTier::Starter < SubscriptionTier::Pro);
        assert!(SubscriptionTier::Pro < SubscriptionTier::Team);
        assert!(SubscriptionTier::Team < SubscriptionTier::Business);
        assert!(SubscriptionTier::Business < SubscriptionTier::Enterprise);
    }

    #[test]
    fn test_tier_pricing() {
        assert_eq!(SubscriptionTier::Free.monthly_price_cents(), 0);
        assert_eq!(SubscriptionTier::Starter.monthly_price_cents(), 2900);
        assert_eq!(SubscriptionTier::Pro.monthly_price_cents(), 4500);
        assert_eq!(SubscriptionTier::Team.monthly_price_cents(), 6000);
        assert_eq!(SubscriptionTier::Business.monthly_price_cents(), 7500);
        assert_eq!(SubscriptionTier::Enterprise.monthly_price_cents(), 9000);
    }

    #[test]
    fn test_feature_gating_free() {
        let f = TierFeatures::for_tier(&SubscriptionTier::Free);
        assert_eq!(f.max_ai_models, 1);
        assert_eq!(f.ai_completions_per_day, FREE_COMPLETIONS_PER_DAY);
        assert!(!f.collab_enabled);
        assert!(!f.shadow_workspace);
        assert!(!f.debug_adapter);
        assert!(!f.custom_themes);
        assert!(f.git_integration);
        assert!(f.lsp_support);
        assert!(!f.sso_saml);
        assert!(!f.audit_logs);
        assert!(!f.custom_models);
        assert!(!f.on_premise);
    }

    #[test]
    fn test_feature_gating_pro() {
        let f = TierFeatures::for_tier(&SubscriptionTier::Pro);
        assert_eq!(f.max_ai_models, UNLIMITED);
        assert_eq!(f.ai_completions_per_day, UNLIMITED);
        assert!(f.shadow_workspace);
        assert!(f.custom_themes);
        assert!(f.debug_adapter);
        assert!(!f.collab_enabled); // Pro does not include collab
        assert!(!f.sso_saml);
    }

    #[test]
    fn test_feature_gating_enterprise() {
        let f = TierFeatures::for_tier(&SubscriptionTier::Enterprise);
        assert_eq!(f.max_ai_models, UNLIMITED);
        assert!(f.collab_enabled);
        assert_eq!(f.max_team_members, UNLIMITED);
        assert!(f.sso_saml);
        assert!(f.audit_logs);
        assert!(f.custom_models);
        assert!(f.on_premise);
        assert!(f.priority_support);
    }

    #[test]
    fn test_has_feature_lookup() {
        let f = TierFeatures::for_tier(&SubscriptionTier::Business);
        assert!(f.has_feature("sso"));
        assert!(f.has_feature("saml"));
        assert!(f.has_feature("sso_saml"));
        assert!(f.has_feature("audit"));
        assert!(f.has_feature("audit_logs"));
        assert!(f.has_feature("collab"));
        assert!(f.has_feature("collaboration"));
        assert!(f.has_feature("shadow"));
        assert!(f.has_feature("debug"));
        assert!(f.has_feature("dap"));
        assert!(!f.has_feature("custom_models"));
        assert!(!f.has_feature("on_prem"));
        assert!(!f.has_feature("nonexistent_feature"));
    }

    #[test]
    fn test_signature_roundtrip() {
        let info = make_test_license(SubscriptionTier::Pro);
        assert!(verify_signature(&info), "Signature should verify after compute");
    }

    #[test]
    fn test_signature_tamper_detection() {
        let mut info = make_test_license(SubscriptionTier::Pro);
        // Tamper with the tier
        info.tier = SubscriptionTier::Enterprise;
        assert!(
            !verify_signature(&info),
            "Tampered license should fail verification"
        );
    }

    #[test]
    fn test_signature_tamper_email() {
        let mut info = make_test_license(SubscriptionTier::Starter);
        info.email = "hacker@evil.com".to_string();
        assert!(!verify_signature(&info));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_is_expired() {
        assert!(is_expired("2020-01-01"));
        assert!(!is_expired("2099-12-31"));
        assert!(is_expired("not-a-date"));
    }

    #[test]
    fn test_days_since() {
        assert!(days_since("2020-01-01") > 0);
        assert!(days_since("2099-12-31") < 0);
        assert_eq!(days_since("invalid"), i64::MAX);
    }

    #[test]
    fn test_license_key_encode_decode_roundtrip() {
        let original = make_test_license(SubscriptionTier::Team);
        let key = encode_license_key(&original);
        assert!(key.starts_with("IF-"), "Key must start with IF- prefix");

        let decoded = decode_license_key(&key).expect("Should decode successfully");
        assert_eq!(decoded.user_id, original.user_id);
        assert_eq!(decoded.email, original.email);
        assert_eq!(decoded.tier, original.tier);
        assert_eq!(decoded.valid_until, original.valid_until);
        assert_eq!(decoded.signature, original.signature);
    }

    #[test]
    fn test_decode_invalid_prefix() {
        let result = decode_license_key("XX-invalid");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("must start with 'IF-'"));
    }

    #[test]
    fn test_decode_invalid_base64() {
        let result = decode_license_key("IF-not!valid!base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_license_manager_defaults_to_free() {
        let mgr = LicenseManager::new();
        assert_eq!(mgr.current_tier(), SubscriptionTier::Free);
    }

    #[test]
    fn test_usage_tracking_within_quota() {
        let mgr = LicenseManager::new();
        // Free tier: 50 completions/day
        for _ in 0..FREE_COMPLETIONS_PER_DAY {
            assert!(mgr.record_completion(), "Should be within quota");
        }
        // One more should fail
        assert!(!mgr.record_completion(), "Should exceed quota");
    }

    #[test]
    fn test_usage_tracking_unlimited() {
        let mgr = LicenseManager::new();
        let info = make_test_license(SubscriptionTier::Pro);
        {
            let mut lock = mgr.license.lock();
            *lock = Some(info);
        }
        // Pro tier: unlimited completions
        for _ in 0..1000 {
            assert!(mgr.record_completion());
        }
    }

    #[test]
    fn test_canonical_payload_deterministic() {
        let info = make_test_license(SubscriptionTier::Starter);
        let a = canonical_payload(&info);
        let b = canonical_payload(&info);
        assert_eq!(a, b, "Canonical payload must be deterministic");
    }

    #[test]
    fn test_all_tiers_have_git_and_lsp() {
        let tiers = [
            SubscriptionTier::Free,
            SubscriptionTier::Starter,
            SubscriptionTier::Pro,
            SubscriptionTier::Team,
            SubscriptionTier::Business,
            SubscriptionTier::Enterprise,
        ];
        for tier in &tiers {
            let f = TierFeatures::for_tier(tier);
            assert!(f.git_integration, "{} must have git", tier);
            assert!(f.lsp_support, "{} must have LSP", tier);
        }
    }
}
