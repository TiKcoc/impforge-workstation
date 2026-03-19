//! Central Keyboard Shortcuts Registry for ImpForge
//!
//! Provides a configurable shortcut system with per-module scoping.
//! Global shortcuts (module = None) apply everywhere; module-specific
//! shortcuts only activate when that module is focused.
//!
//! All shortcuts are persisted via tauri-plugin-store so user
//! customizations survive across sessions.

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

const STORE_FILENAME: &str = "shortcuts.json";
const STORE_KEY: &str = "shortcuts";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single keyboard shortcut binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shortcut {
    /// Unique identifier (e.g. "save", "send_message").
    pub id: String,
    /// Key combination string (e.g. "Ctrl+S", "Ctrl+Shift+F", "F5").
    pub keys: String,
    /// Semantic action name used by the frontend dispatcher.
    pub action: String,
    /// `None` = global shortcut, `Some("chat")` = module-scoped.
    pub module: Option<String>,
    /// Human-readable description shown in the shortcut settings UI.
    pub description: String,
    /// Whether this shortcut is currently active.
    pub enabled: bool,
}

impl Shortcut {
    fn new(id: &str, keys: &str, module: Option<&str>, description: &str) -> Self {
        Self {
            id: id.to_string(),
            keys: keys.to_string(),
            action: id.to_string(),
            module: module.map(String::from),
            description: description.to_string(),
            enabled: true,
        }
    }
}

/// Registry holding every registered shortcut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutRegistry {
    pub shortcuts: Vec<Shortcut>,
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::defaults()
    }
}

impl ShortcutRegistry {
    /// Built-in default shortcuts shipped with ImpForge.
    pub fn defaults() -> Self {
        Self {
            shortcuts: vec![
                // -- Global --------------------------------------------------
                Shortcut::new("save", "Ctrl+S", None, "Save current document"),
                Shortcut::new("undo", "Ctrl+Z", None, "Undo last action"),
                Shortcut::new("redo", "Ctrl+Y", None, "Redo last action"),
                Shortcut::new("search", "Ctrl+K", None, "Open command palette"),
                Shortcut::new("settings", "Ctrl+,", None, "Open settings"),
                Shortcut::new("new", "Ctrl+N", None, "New document/item"),
                Shortcut::new("close", "Ctrl+W", None, "Close current tab"),
                Shortcut::new("fullscreen", "F11", None, "Toggle fullscreen"),
                // -- Chat ----------------------------------------------------
                Shortcut::new("send_message", "Enter", Some("chat"), "Send message"),
                Shortcut::new("new_chat", "Ctrl+Shift+N", Some("chat"), "New conversation"),
                // -- IDE -----------------------------------------------------
                Shortcut::new("format", "Ctrl+Shift+F", Some("ide"), "Format document"),
                Shortcut::new("find", "Ctrl+F", Some("ide"), "Find in file"),
                Shortcut::new("terminal", "Ctrl+`", Some("ide"), "Toggle terminal"),
                Shortcut::new("run", "F5", Some("ide"), "Run/debug"),
                // -- Sheets --------------------------------------------------
                Shortcut::new("bold", "Ctrl+B", Some("sheets"), "Bold"),
                Shortcut::new("italic", "Ctrl+I", Some("sheets"), "Italic"),
                Shortcut::new("formula_bar", "F2", Some("sheets"), "Edit cell"),
                // -- Writer --------------------------------------------------
                Shortcut::new("preview", "Ctrl+P", Some("writer"), "Toggle preview"),
                // -- Slides --------------------------------------------------
                Shortcut::new("present", "F5", Some("slides"), "Start presentation"),
                Shortcut::new("new_slide", "Ctrl+M", Some("slides"), "New slide"),
                // -- Workflows -----------------------------------------------
                Shortcut::new("run_workflow", "Ctrl+Enter", Some("workflows"), "Run workflow"),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

/// Load the shortcut registry from disk, falling back to defaults.
fn load_registry(app: &tauri::AppHandle) -> ShortcutRegistry {
    let store = match app.store(STORE_FILENAME) {
        Ok(s) => s,
        Err(_) => return ShortcutRegistry::defaults(),
    };

    match store.get(STORE_KEY) {
        Some(val) => serde_json::from_value::<ShortcutRegistry>(val.clone())
            .unwrap_or_else(|_| ShortcutRegistry::defaults()),
        None => ShortcutRegistry::defaults(),
    }
}

/// Persist the shortcut registry to disk.
fn save_registry(app: &tauri::AppHandle, registry: &ShortcutRegistry) -> Result<(), String> {
    let store = app
        .store(STORE_FILENAME)
        .map_err(|e| format!("Failed to open shortcut store: {e}"))?;

    let value = serde_json::to_value(registry)
        .map_err(|e| format!("Failed to serialize shortcuts: {e}"))?;

    store.set(STORE_KEY, value);

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Return every registered shortcut (all modules).
#[tauri::command]
pub fn shortcuts_list(app: tauri::AppHandle) -> Vec<Shortcut> {
    load_registry(&app).shortcuts
}

/// Return shortcuts filtered by module.
///
/// - `module = None`  -->  global shortcuts only (where `shortcut.module` is `None`)
/// - `module = Some("chat")`  -->  shortcuts scoped to the chat module
#[tauri::command]
pub fn shortcuts_get(app: tauri::AppHandle, module: Option<String>) -> Vec<Shortcut> {
    load_registry(&app)
        .shortcuts
        .into_iter()
        .filter(|s| s.module == module)
        .collect()
}

/// Update the key binding for an existing shortcut.
///
/// Returns an error if the shortcut `id` does not exist or if the new `keys`
/// string conflicts with another shortcut in the same scope.
#[tauri::command]
pub fn shortcuts_update(
    app: tauri::AppHandle,
    id: String,
    keys: String,
) -> Result<(), String> {
    let mut registry = load_registry(&app);

    // Find the target shortcut and its module scope.
    let target_module = registry
        .shortcuts
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.module.clone())
        .ok_or_else(|| format!("Shortcut '{id}' not found"))?;

    // Check for conflicts: same keys in same scope (excluding the shortcut being edited).
    let conflict = registry.shortcuts.iter().any(|s| {
        s.id != id && s.keys == keys && s.module == target_module && s.enabled
    });
    if conflict {
        return Err(format!(
            "Key binding '{keys}' already used by another shortcut in the same scope"
        ));
    }

    // Apply the change.
    if let Some(shortcut) = registry.shortcuts.iter_mut().find(|s| s.id == id) {
        shortcut.keys = keys;
    }

    save_registry(&app, &registry)
}

/// Reset all shortcuts to built-in defaults and return the fresh list.
#[tauri::command]
pub fn shortcuts_reset(app: tauri::AppHandle) -> Result<Vec<Shortcut>, String> {
    let registry = ShortcutRegistry::defaults();
    save_registry(&app, &registry)?;
    Ok(registry.shortcuts)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_non_empty() {
        let reg = ShortcutRegistry::defaults();
        assert!(!reg.shortcuts.is_empty());
    }

    #[test]
    fn defaults_have_unique_ids_per_scope() {
        let reg = ShortcutRegistry::defaults();
        let mut seen = std::collections::HashSet::new();
        for s in &reg.shortcuts {
            let key = format!("{}:{}", s.module.as_deref().unwrap_or("global"), &s.id);
            assert!(seen.insert(key.clone()), "Duplicate shortcut id: {key}");
        }
    }

    #[test]
    fn shortcut_new_sets_action_from_id() {
        let s = Shortcut::new("save", "Ctrl+S", None, "Save");
        assert_eq!(s.action, "save");
        assert!(s.enabled);
        assert!(s.module.is_none());
    }

    #[test]
    fn shortcut_module_scoped() {
        let s = Shortcut::new("bold", "Ctrl+B", Some("sheets"), "Bold");
        assert_eq!(s.module.as_deref(), Some("sheets"));
    }

    #[test]
    fn default_global_shortcuts_count() {
        let reg = ShortcutRegistry::defaults();
        let globals: Vec<_> = reg.shortcuts.iter().filter(|s| s.module.is_none()).collect();
        assert_eq!(globals.len(), 8, "Expected 8 global shortcuts");
    }

    #[test]
    fn serialization_roundtrip() {
        let reg = ShortcutRegistry::defaults();
        let json = serde_json::to_string(&reg).expect("serialize");
        let deser: ShortcutRegistry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reg.shortcuts.len(), deser.shortcuts.len());
    }
}
