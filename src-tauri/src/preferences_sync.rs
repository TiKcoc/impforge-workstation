// SPDX-License-Identifier: Apache-2.0
//! User Preferences Sync — export, import, and fingerprint user settings.
//!
//! Enables users to transfer their ImpForge configuration between devices
//! by exporting preferences as JSON and importing them on another machine.
//! The fingerprint command produces a hash of current preferences so the
//! frontend can detect when settings have changed since last sync.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_plugin_store::StoreExt;

use crate::error::{AppResult, ImpForgeError};

/// Portable subset of user preferences that can be synced across devices.
/// Intentionally excludes machine-specific data (window geometry, API keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncablePreferences {
    pub theme: String,
    pub user_role: String,
    pub user_experience: String,
    pub module_visibility: Vec<String>,
    pub keymap_profile: String,
    pub shortcuts: Vec<(String, String)>,
    pub last_synced: String,
    // Chat layout preferences
    pub chat_placement: String,
    pub chat_stream_mode: String,
    pub chat_viz_level: String,
    pub chat_show_thinking: bool,
    pub chat_show_routing: bool,
    pub chat_animations: bool,
    pub chat_compact_mode: bool,
    // AI preferences
    pub prefer_local_models: bool,
    pub auto_routing: bool,
    pub font_size: u64,
}

impl Default for SyncablePreferences {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            user_role: String::new(),
            user_experience: String::new(),
            module_visibility: Vec::new(),
            keymap_profile: "default".to_string(),
            shortcuts: Vec::new(),
            last_synced: String::new(),
            chat_placement: "dedicated".to_string(),
            chat_stream_mode: "unified".to_string(),
            chat_viz_level: "cards".to_string(),
            chat_show_thinking: true,
            chat_show_routing: true,
            chat_animations: true,
            chat_compact_mode: false,
            prefer_local_models: true,
            auto_routing: true,
            font_size: 14,
        }
    }
}

/// Helper: read a string value from the store with a fallback.
fn read_string(store: &impl StoreRead, key: &str, fallback: &str) -> String {
    store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| fallback.to_string())
}

/// Helper: read a bool from the store with a fallback.
fn read_bool(store: &impl StoreRead, key: &str, fallback: bool) -> bool {
    store
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(fallback)
}

/// Helper: read a u64 from the store with a fallback.
fn read_u64(store: &impl StoreRead, key: &str, fallback: u64) -> u64 {
    store
        .get(key)
        .and_then(|v| v.as_u64())
        .unwrap_or(fallback)
}

/// Trait to abstract over store read access (for testability).
trait StoreRead {
    fn get(&self, key: &str) -> Option<serde_json::Value>;
}

/// Adapter for tauri_plugin_store.
struct TauriStoreAdapter<'a> {
    store: &'a tauri_plugin_store::Store<tauri::Wry>,
}

impl<'a> StoreRead for TauriStoreAdapter<'a> {
    fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.store.get(key)
    }
}

/// Build a `SyncablePreferences` from the store.
fn build_preferences(store: &impl StoreRead) -> SyncablePreferences {
    SyncablePreferences {
        theme: read_string(store, "theme", "dark"),
        user_role: read_string(store, "userRole", ""),
        user_experience: read_string(store, "userExperience", ""),
        module_visibility: store
            .get("moduleVisibility")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default(),
        keymap_profile: read_string(store, "keymapProfile", "default"),
        shortcuts: store
            .get("customShortcuts")
            .and_then(|v| serde_json::from_value::<Vec<(String, String)>>(v).ok())
            .unwrap_or_default(),
        last_synced: chrono::Utc::now().to_rfc3339(),
        chat_placement: read_string(store, "chatPlacement", "dedicated"),
        chat_stream_mode: read_string(store, "chatStreamMode", "unified"),
        chat_viz_level: read_string(store, "chatVizLevel", "cards"),
        chat_show_thinking: read_bool(store, "chatShowThinking", true),
        chat_show_routing: read_bool(store, "chatShowRouting", true),
        chat_animations: read_bool(store, "chatAnimations", true),
        chat_compact_mode: read_bool(store, "chatCompactMode", false),
        prefer_local_models: read_bool(store, "preferLocalModels", true),
        auto_routing: read_bool(store, "autoRouting", true),
        font_size: read_u64(store, "font_size", 14),
    }
}

/// Export all syncable user preferences as a JSON string.
#[tauri::command]
pub fn sync_export_preferences(app: tauri::AppHandle) -> AppResult<String> {
    let store = app
        .store(".impforge-settings.json")
        .map_err(|e| ImpForgeError::config("STORE_OPEN", e.to_string()))?;
    let adapter = TauriStoreAdapter { store: &store };
    let prefs = build_preferences(&adapter);
    serde_json::to_string_pretty(&prefs).map_err(|e| {
        ImpForgeError::internal("SERIALIZE_PREFS", format!("Failed to serialize preferences: {e}"))
    })
}

/// Import preferences from a JSON string (from another device).
/// Only imports the safe, syncable keys -- never overwrites API keys or geometry.
#[tauri::command]
pub fn sync_import_preferences(app: tauri::AppHandle, json: String) -> AppResult<()> {
    let prefs: SyncablePreferences = serde_json::from_str(&json).map_err(|e| {
        ImpForgeError::validation("INVALID_PREFS_JSON", format!("Invalid preferences JSON: {e}"))
            .with_suggestion("Make sure you pasted the complete export string.")
    })?;

    let store = app
        .store(".impforge-settings.json")
        .map_err(|e| ImpForgeError::config("STORE_OPEN", e.to_string()))?;

    // Write each syncable field back into the store
    store.set("theme", serde_json::Value::String(prefs.theme));
    store.set("userRole", serde_json::Value::String(prefs.user_role));
    store.set(
        "userExperience",
        serde_json::Value::String(prefs.user_experience),
    );
    store.set(
        "chatPlacement",
        serde_json::Value::String(prefs.chat_placement),
    );
    store.set(
        "chatStreamMode",
        serde_json::Value::String(prefs.chat_stream_mode),
    );
    store.set(
        "chatVizLevel",
        serde_json::Value::String(prefs.chat_viz_level),
    );
    store.set(
        "chatShowThinking",
        serde_json::Value::Bool(prefs.chat_show_thinking),
    );
    store.set(
        "chatShowRouting",
        serde_json::Value::Bool(prefs.chat_show_routing),
    );
    store.set(
        "chatAnimations",
        serde_json::Value::Bool(prefs.chat_animations),
    );
    store.set(
        "chatCompactMode",
        serde_json::Value::Bool(prefs.chat_compact_mode),
    );
    store.set(
        "preferLocalModels",
        serde_json::Value::Bool(prefs.prefer_local_models),
    );
    store.set("autoRouting", serde_json::Value::Bool(prefs.auto_routing));
    store.set(
        "font_size",
        serde_json::Value::Number(serde_json::Number::from(prefs.font_size)),
    );

    if let Ok(vis_json) = serde_json::to_value(&prefs.module_visibility) {
        store.set("moduleVisibility", vis_json);
    }
    if let Ok(sc_json) = serde_json::to_value(&prefs.shortcuts) {
        store.set("customShortcuts", sc_json);
    }
    store.set("keymapProfile", serde_json::Value::String(prefs.keymap_profile));

    store.save().map_err(|e| {
        ImpForgeError::config("STORE_SAVE", format!("Failed to save imported preferences: {e}"))
    })?;

    Ok(())
}

/// Compute a SHA-256 fingerprint of the current syncable preferences.
/// Returns a hex string. Two devices with identical settings produce the same hash.
#[tauri::command]
pub fn sync_get_fingerprint(app: tauri::AppHandle) -> AppResult<String> {
    let store = app
        .store(".impforge-settings.json")
        .map_err(|e| ImpForgeError::config("STORE_OPEN", e.to_string()))?;
    let adapter = TauriStoreAdapter { store: &store };
    let prefs = build_preferences(&adapter);

    // Deterministic JSON (keys sorted via serde default for structs)
    let canonical = serde_json::to_string(&prefs).map_err(|e| {
        ImpForgeError::internal("SERIALIZE_PREFS", format!("Failed to serialize for hash: {e}"))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let hash = hasher.finalize();
    Ok(format!("{hash:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences_serializes() {
        let prefs = SyncablePreferences::default();
        let json = serde_json::to_string(&prefs).expect("should serialize");
        assert!(json.contains("\"theme\""));
        assert!(json.contains("\"dark\""));
    }

    #[test]
    fn test_preferences_roundtrip() {
        let prefs = SyncablePreferences {
            theme: "cyberpunk-neon".to_string(),
            user_role: "developer".to_string(),
            font_size: 16,
            ..Default::default()
        };
        let json = serde_json::to_string(&prefs).expect("serialize");
        let restored: SyncablePreferences = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.theme, "cyberpunk-neon");
        assert_eq!(restored.user_role, "developer");
        assert_eq!(restored.font_size, 16);
    }

    #[test]
    fn test_invalid_json_import_fails() {
        let bad_json = "{ this is not valid json }";
        let result: Result<SyncablePreferences, _> = serde_json::from_str(bad_json);
        assert!(result.is_err());
    }
}
