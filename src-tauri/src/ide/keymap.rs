//! Keymap Profile Engine -- configurable keyboard shortcuts for CodeForge IDE.
//!
//! Provides a JetBrains-like keymap system where the user can choose from
//! pre-built profiles (ImpForge Default, VS Code, JetBrains, Vim, Emacs)
//! and the frontend receives the active bindings to configure its shortcut
//! handler accordingly.
//!
//! The active profile is persisted to `~/.impforge/keymap.json`.
//!
//! References:
//! - JetBrains Keymap Settings (2024): profile switching, per-action override
//! - VS Code keybindings.json (2024): action-based binding model

use crate::error::{AppResult, ImpForgeError};
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPES
// ============================================================================

/// A single keyboard shortcut binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    /// The action identifier (e.g. "editor.save", "editor.find", "editor.format").
    pub action: String,
    /// The key chord (e.g. "Ctrl+S", "Ctrl+Shift+F", "Cmd+P").
    pub keys: String,
}

/// A complete keymap profile containing all bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapProfile {
    /// Profile name shown in the UI.
    pub name: String,
    /// All key bindings in this profile.
    pub bindings: Vec<KeyBinding>,
}

/// Persisted state: which profile is active.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeymapState {
    active_profile: String,
}

// ============================================================================
// PROFILE DEFINITIONS
// ============================================================================

/// All available profile names.
const PROFILE_NAMES: &[&str] = &[
    "ImpForge Default",
    "VS Code",
    "JetBrains",
    "Vim",
    "Emacs",
];

fn kb(action: &str, keys: &str) -> KeyBinding {
    KeyBinding {
        action: action.to_string(),
        keys: keys.to_string(),
    }
}

/// Build the ImpForge Default keymap (similar to VS Code with ImpForge additions).
fn profile_impforge_default() -> KeymapProfile {
    KeymapProfile {
        name: "ImpForge Default".to_string(),
        bindings: vec![
            // File operations
            kb("editor.save", "Ctrl+S"),
            kb("editor.saveAll", "Ctrl+Shift+S"),
            kb("editor.close", "Ctrl+W"),
            kb("editor.newFile", "Ctrl+N"),
            // Navigation
            kb("editor.find", "Ctrl+F"),
            kb("editor.findReplace", "Ctrl+H"),
            kb("editor.findInFiles", "Ctrl+Shift+F"),
            kb("editor.goToLine", "Ctrl+G"),
            kb("editor.goToFile", "Ctrl+P"),
            kb("editor.goToSymbol", "Ctrl+Shift+O"),
            kb("editor.goToDefinition", "F12"),
            // Editing
            kb("editor.format", "Ctrl+Shift+I"),
            kb("editor.commentLine", "Ctrl+/"),
            kb("editor.undo", "Ctrl+Z"),
            kb("editor.redo", "Ctrl+Shift+Z"),
            kb("editor.duplicate", "Ctrl+Shift+D"),
            kb("editor.deleteLine", "Ctrl+Shift+K"),
            kb("editor.moveLineUp", "Alt+Up"),
            kb("editor.moveLineDown", "Alt+Down"),
            // Panels
            kb("panel.commandPalette", "Ctrl+Shift+P"),
            kb("panel.toggleTerminal", "Ctrl+`"),
            kb("panel.toggleExplorer", "Ctrl+B"),
            kb("panel.toggleAi", "Ctrl+Shift+A"),
            kb("panel.toggleGit", "Ctrl+Shift+G"),
            kb("panel.toggleProblems", "Ctrl+Shift+M"),
            // AI (ImpForge-specific)
            kb("forge.aiComplete", "Ctrl+Space"),
            kb("forge.aiEdit", "Ctrl+K"),
            kb("forge.aiExplain", "Ctrl+Shift+E"),
            kb("forge.aiFix", "Ctrl+Shift+."),
            kb("forge.aiTest", "Ctrl+Shift+T"),
            // Debug
            kb("debug.start", "F5"),
            kb("debug.stop", "Shift+F5"),
            kb("debug.stepOver", "F10"),
            kb("debug.stepIn", "F11"),
            kb("debug.stepOut", "Shift+F11"),
            kb("debug.toggleBreakpoint", "F9"),
        ],
    }
}

/// Build the VS Code-compatible keymap.
fn profile_vscode() -> KeymapProfile {
    KeymapProfile {
        name: "VS Code".to_string(),
        bindings: vec![
            kb("editor.save", "Ctrl+S"),
            kb("editor.saveAll", "Ctrl+K S"),
            kb("editor.close", "Ctrl+W"),
            kb("editor.newFile", "Ctrl+N"),
            kb("editor.find", "Ctrl+F"),
            kb("editor.findReplace", "Ctrl+H"),
            kb("editor.findInFiles", "Ctrl+Shift+F"),
            kb("editor.goToLine", "Ctrl+G"),
            kb("editor.goToFile", "Ctrl+P"),
            kb("editor.goToSymbol", "Ctrl+Shift+O"),
            kb("editor.goToDefinition", "F12"),
            kb("editor.format", "Ctrl+Shift+I"),
            kb("editor.commentLine", "Ctrl+/"),
            kb("editor.undo", "Ctrl+Z"),
            kb("editor.redo", "Ctrl+Y"),
            kb("editor.duplicate", "Ctrl+Shift+D"),
            kb("editor.deleteLine", "Ctrl+Shift+K"),
            kb("editor.moveLineUp", "Alt+Up"),
            kb("editor.moveLineDown", "Alt+Down"),
            kb("panel.commandPalette", "Ctrl+Shift+P"),
            kb("panel.toggleTerminal", "Ctrl+`"),
            kb("panel.toggleExplorer", "Ctrl+B"),
            kb("panel.toggleAi", "Ctrl+Shift+A"),
            kb("panel.toggleGit", "Ctrl+Shift+G"),
            kb("panel.toggleProblems", "Ctrl+Shift+M"),
            kb("forge.aiComplete", "Ctrl+Space"),
            kb("forge.aiEdit", "Ctrl+K"),
            kb("forge.aiExplain", "Ctrl+Shift+E"),
            kb("forge.aiFix", "Ctrl+Shift+."),
            kb("forge.aiTest", "Ctrl+Shift+T"),
            kb("debug.start", "F5"),
            kb("debug.stop", "Shift+F5"),
            kb("debug.stepOver", "F10"),
            kb("debug.stepIn", "F11"),
            kb("debug.stepOut", "Shift+F11"),
            kb("debug.toggleBreakpoint", "F9"),
        ],
    }
}

/// Build the JetBrains-compatible keymap (IntelliJ / Rider style).
fn profile_jetbrains() -> KeymapProfile {
    KeymapProfile {
        name: "JetBrains".to_string(),
        bindings: vec![
            kb("editor.save", "Ctrl+S"),
            kb("editor.saveAll", "Ctrl+Shift+S"),
            kb("editor.close", "Ctrl+F4"),
            kb("editor.newFile", "Ctrl+Alt+Insert"),
            kb("editor.find", "Ctrl+F"),
            kb("editor.findReplace", "Ctrl+R"),
            kb("editor.findInFiles", "Ctrl+Shift+F"),
            kb("editor.goToLine", "Ctrl+G"),
            kb("editor.goToFile", "Ctrl+Shift+N"),
            kb("editor.goToSymbol", "Ctrl+Alt+Shift+N"),
            kb("editor.goToDefinition", "Ctrl+B"),
            kb("editor.format", "Ctrl+Alt+L"),
            kb("editor.commentLine", "Ctrl+/"),
            kb("editor.undo", "Ctrl+Z"),
            kb("editor.redo", "Ctrl+Shift+Z"),
            kb("editor.duplicate", "Ctrl+D"),
            kb("editor.deleteLine", "Ctrl+Y"),
            kb("editor.moveLineUp", "Alt+Shift+Up"),
            kb("editor.moveLineDown", "Alt+Shift+Down"),
            kb("panel.commandPalette", "Ctrl+Shift+A"),
            kb("panel.toggleTerminal", "Alt+F12"),
            kb("panel.toggleExplorer", "Alt+1"),
            kb("panel.toggleAi", "Alt+A"),
            kb("panel.toggleGit", "Alt+9"),
            kb("panel.toggleProblems", "Alt+6"),
            kb("forge.aiComplete", "Ctrl+Space"),
            kb("forge.aiEdit", "Alt+Enter"),
            kb("forge.aiExplain", "Ctrl+Shift+E"),
            kb("forge.aiFix", "Alt+Shift+Enter"),
            kb("forge.aiTest", "Ctrl+Shift+T"),
            kb("debug.start", "Shift+F9"),
            kb("debug.stop", "Ctrl+F2"),
            kb("debug.stepOver", "F8"),
            kb("debug.stepIn", "F7"),
            kb("debug.stepOut", "Shift+F8"),
            kb("debug.toggleBreakpoint", "Ctrl+F8"),
        ],
    }
}

/// Build the Vim-style keymap (modal bindings noted in the key names).
fn profile_vim() -> KeymapProfile {
    KeymapProfile {
        name: "Vim".to_string(),
        bindings: vec![
            kb("editor.save", ":w"),
            kb("editor.saveAll", ":wa"),
            kb("editor.close", ":q"),
            kb("editor.newFile", ":enew"),
            kb("editor.find", "/"),
            kb("editor.findReplace", ":%s/"),
            kb("editor.findInFiles", "Ctrl+Shift+F"),
            kb("editor.goToLine", ":N"),
            kb("editor.goToFile", "Ctrl+P"),
            kb("editor.goToSymbol", "gd"),
            kb("editor.goToDefinition", "gd"),
            kb("editor.format", "="),
            kb("editor.commentLine", "gcc"),
            kb("editor.undo", "u"),
            kb("editor.redo", "Ctrl+R"),
            kb("editor.duplicate", "yyp"),
            kb("editor.deleteLine", "dd"),
            kb("editor.moveLineUp", "ddkP"),
            kb("editor.moveLineDown", "ddp"),
            kb("panel.commandPalette", ":"),
            kb("panel.toggleTerminal", "Ctrl+`"),
            kb("panel.toggleExplorer", "Ctrl+B"),
            kb("panel.toggleAi", "Ctrl+Shift+A"),
            kb("panel.toggleGit", "Ctrl+Shift+G"),
            kb("panel.toggleProblems", "Ctrl+Shift+M"),
            kb("forge.aiComplete", "Ctrl+Space"),
            kb("forge.aiEdit", "Ctrl+K"),
            kb("forge.aiExplain", "Ctrl+Shift+E"),
            kb("forge.aiFix", "Ctrl+Shift+."),
            kb("forge.aiTest", "Ctrl+Shift+T"),
            kb("debug.start", "F5"),
            kb("debug.stop", "Shift+F5"),
            kb("debug.stepOver", "F10"),
            kb("debug.stepIn", "F11"),
            kb("debug.stepOut", "Shift+F11"),
            kb("debug.toggleBreakpoint", "F9"),
        ],
    }
}

/// Build the Emacs-style keymap.
fn profile_emacs() -> KeymapProfile {
    KeymapProfile {
        name: "Emacs".to_string(),
        bindings: vec![
            kb("editor.save", "Ctrl+X Ctrl+S"),
            kb("editor.saveAll", "Ctrl+X s"),
            kb("editor.close", "Ctrl+X k"),
            kb("editor.newFile", "Ctrl+X Ctrl+F"),
            kb("editor.find", "Ctrl+S"),
            kb("editor.findReplace", "Alt+%"),
            kb("editor.findInFiles", "Ctrl+Shift+F"),
            kb("editor.goToLine", "Alt+G G"),
            kb("editor.goToFile", "Ctrl+X Ctrl+F"),
            kb("editor.goToSymbol", "Alt+."),
            kb("editor.goToDefinition", "Alt+."),
            kb("editor.format", "Ctrl+Alt+\\"),
            kb("editor.commentLine", "Alt+;"),
            kb("editor.undo", "Ctrl+/"),
            kb("editor.redo", "Ctrl+Shift+/"),
            kb("editor.duplicate", "Ctrl+K Ctrl+Y"),
            kb("editor.deleteLine", "Ctrl+Shift+K"),
            kb("editor.moveLineUp", "Alt+Up"),
            kb("editor.moveLineDown", "Alt+Down"),
            kb("panel.commandPalette", "Alt+X"),
            kb("panel.toggleTerminal", "Ctrl+`"),
            kb("panel.toggleExplorer", "Ctrl+B"),
            kb("panel.toggleAi", "Ctrl+Shift+A"),
            kb("panel.toggleGit", "Ctrl+Shift+G"),
            kb("panel.toggleProblems", "Ctrl+Shift+M"),
            kb("forge.aiComplete", "Alt+/"),
            kb("forge.aiEdit", "Ctrl+C Ctrl+E"),
            kb("forge.aiExplain", "Ctrl+C Ctrl+X"),
            kb("forge.aiFix", "Ctrl+C Ctrl+F"),
            kb("forge.aiTest", "Ctrl+C Ctrl+T"),
            kb("debug.start", "F5"),
            kb("debug.stop", "Shift+F5"),
            kb("debug.stepOver", "F10"),
            kb("debug.stepIn", "F11"),
            kb("debug.stepOut", "Shift+F11"),
            kb("debug.toggleBreakpoint", "F9"),
        ],
    }
}

/// Look up a profile by name.
fn get_profile(name: &str) -> Option<KeymapProfile> {
    match name {
        "ImpForge Default" => Some(profile_impforge_default()),
        "VS Code" => Some(profile_vscode()),
        "JetBrains" => Some(profile_jetbrains()),
        "Vim" => Some(profile_vim()),
        "Emacs" => Some(profile_emacs()),
        _ => None,
    }
}

/// Path to the keymap state file.
fn state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".impforge")
        .join("keymap.json")
}

/// Read the persisted active profile name. Defaults to "ImpForge Default".
fn read_active_profile() -> String {
    let path = state_path();
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<KeymapState>(&data) {
                return state.active_profile;
            }
        }
    }
    "ImpForge Default".to_string()
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Returns the list of all available keymap profile names.
#[tauri::command]
pub fn keymap_get_profiles() -> Vec<String> {
    PROFILE_NAMES.iter().map(|s| s.to_string()).collect()
}

/// Returns the key bindings for a given profile name.
///
/// If the profile name is not recognized, returns an error.
#[tauri::command]
pub fn keymap_get_bindings(profile: String) -> AppResult<Vec<KeyBinding>> {
    let p = get_profile(&profile).ok_or_else(|| {
        ImpForgeError::validation(
            "UNKNOWN_PROFILE",
            format!("Unknown keymap profile: {profile}"),
        )
    })?;
    Ok(p.bindings)
}

/// Returns the full KeymapProfile for a given profile name.
#[tauri::command]
pub fn keymap_get_profile(profile: String) -> AppResult<KeymapProfile> {
    get_profile(&profile).ok_or_else(|| {
        ImpForgeError::validation(
            "UNKNOWN_PROFILE",
            format!("Unknown keymap profile: {profile}"),
        )
    })
}

/// Set the active keymap profile and persist the choice to disk.
///
/// The frontend should call `keymap_get_bindings` afterwards to reconfigure
/// its shortcut handler.
#[tauri::command]
pub fn keymap_set_profile(profile: String) -> AppResult<()> {
    // Validate the profile name
    if get_profile(&profile).is_none() {
        return Err(ImpForgeError::validation(
            "UNKNOWN_PROFILE",
            format!("Unknown keymap profile: {profile}"),
        ));
    }

    let config_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".impforge");

    std::fs::create_dir_all(&config_dir).map_err(|e| {
        ImpForgeError::filesystem("KEYMAP_DIR", format!("Failed to create config dir: {e}"))
    })?;

    let state = KeymapState {
        active_profile: profile,
    };
    let json = serde_json::to_string_pretty(&state).map_err(|e| {
        ImpForgeError::internal("KEYMAP_SERIALIZE", format!("Failed to serialize state: {e}"))
    })?;

    std::fs::write(state_path(), json).map_err(|e| {
        ImpForgeError::filesystem("KEYMAP_WRITE", format!("Failed to write keymap state: {e}"))
    })?;

    Ok(())
}

/// Get the currently active keymap profile name.
#[tauri::command]
pub fn keymap_get_active() -> String {
    read_active_profile()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_names() {
        let names = keymap_get_profiles();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"ImpForge Default".to_string()));
        assert!(names.contains(&"VS Code".to_string()));
        assert!(names.contains(&"JetBrains".to_string()));
        assert!(names.contains(&"Vim".to_string()));
        assert!(names.contains(&"Emacs".to_string()));
    }

    #[test]
    fn test_default_profile_has_bindings() {
        let profile = get_profile("ImpForge Default").expect("should exist");
        assert!(!profile.bindings.is_empty());
        // Should have at least save and find
        assert!(profile.bindings.iter().any(|b| b.action == "editor.save"));
        assert!(profile.bindings.iter().any(|b| b.action == "editor.find"));
    }

    #[test]
    fn test_all_profiles_have_core_actions() {
        let core_actions = [
            "editor.save",
            "editor.find",
            "editor.undo",
            "editor.redo",
            "forge.aiComplete",
        ];

        for name in PROFILE_NAMES {
            let profile = get_profile(name).unwrap_or_else(|| panic!("profile {name} missing"));
            for action in &core_actions {
                assert!(
                    profile.bindings.iter().any(|b| b.action == *action),
                    "Profile '{name}' missing action '{action}'"
                );
            }
        }
    }

    #[test]
    fn test_unknown_profile_returns_none() {
        assert!(get_profile("NonExistent").is_none());
    }

    #[test]
    fn test_vscode_uses_ctrl_y_redo() {
        let profile = get_profile("VS Code").expect("should exist");
        let redo = profile
            .bindings
            .iter()
            .find(|b| b.action == "editor.redo")
            .expect("should have redo");
        assert_eq!(redo.keys, "Ctrl+Y");
    }

    #[test]
    fn test_jetbrains_uses_ctrl_alt_l_format() {
        let profile = get_profile("JetBrains").expect("should exist");
        let format = profile
            .bindings
            .iter()
            .find(|b| b.action == "editor.format")
            .expect("should have format");
        assert_eq!(format.keys, "Ctrl+Alt+L");
    }

    #[test]
    fn test_vim_save_is_colon_w() {
        let profile = get_profile("Vim").expect("should exist");
        let save = profile
            .bindings
            .iter()
            .find(|b| b.action == "editor.save")
            .expect("should have save");
        assert_eq!(save.keys, ":w");
    }

    #[test]
    fn test_emacs_save_chord() {
        let profile = get_profile("Emacs").expect("should exist");
        let save = profile
            .bindings
            .iter()
            .find(|b| b.action == "editor.save")
            .expect("should have save");
        assert_eq!(save.keys, "Ctrl+X Ctrl+S");
    }

    #[test]
    fn test_keybinding_serialization() {
        let binding = kb("editor.save", "Ctrl+S");
        let json = serde_json::to_string(&binding).expect("serialize");
        assert!(json.contains("editor.save"));
        assert!(json.contains("Ctrl+S"));
        let restored: KeyBinding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.action, "editor.save");
        assert_eq!(restored.keys, "Ctrl+S");
    }

    #[test]
    fn test_keymap_profile_serialization() {
        let profile = profile_impforge_default();
        let json = serde_json::to_string(&profile).expect("serialize");
        let restored: KeymapProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "ImpForge Default");
        assert_eq!(restored.bindings.len(), profile.bindings.len());
    }

    #[test]
    fn test_keymap_get_bindings_valid() {
        let bindings = keymap_get_bindings("VS Code".to_string());
        assert!(bindings.is_ok());
        assert!(!bindings.as_ref().ok().map_or(true, |b| b.is_empty()));
    }

    #[test]
    fn test_keymap_get_bindings_invalid() {
        let result = keymap_get_bindings("FooBar".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_keymap_set_profile_invalid() {
        let result = keymap_set_profile("NonExistent".to_string());
        assert!(result.is_err());
    }
}
