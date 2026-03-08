//! NEXUS Theme Engine — Customer-facing UI customization
//!
//! CSS-variable-driven themes with SQLite persistence.
//! Inspired by ElvUI/BenikUI modular addon architecture.
//!
//! Customers can:
//! - Switch between built-in themes (Neon Green, Cyberpunk Red, Arctic Blue, etc.)
//! - Create custom themes by overriding CSS variables
//! - Export/import themes (base64 encoded JSON)
//! - Arrange widgets in drag-and-drop layouts
//! - Save/load layout profiles
//!
//! License: MIT (all original code)

use serde::{Deserialize, Serialize};

/// A complete NEXUS theme (CSS variable overrides)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusTheme {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub version: String,
    pub variables: Vec<(String, String)>, // (CSS var name, value)
    pub is_builtin: bool,
}

/// A widget placement in a layout (ElvUI-style grid positioning)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPlacement {
    pub widget_id: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub config: serde_json::Value,
}

/// A complete layout configuration (like an ElvUI profile)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetLayout {
    pub id: String,
    pub name: String,
    pub widgets: Vec<WidgetPlacement>,
    pub route: String, // Which page this layout applies to ("/" = dashboard)
}

/// Theme export format (base64 JSON, like ElvUI profile strings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeExport {
    pub theme: NexusTheme,
    pub layouts: Vec<WidgetLayout>,
    pub nexus_version: String,
    pub export_date: String,
}

// ============================================================================
// BUILT-IN THEMES — 5 Opera GX-style color schemes
// ============================================================================

/// Get all built-in themes
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
                ("--gx-glow-sm".into(), "0 0 8px rgba(255, 51, 102, 0.15)".into()),
                ("--gx-glow-md".into(), "0 0 16px rgba(255, 51, 102, 0.2)".into()),
                ("--gx-glow-lg".into(), "0 0 32px rgba(255, 51, 102, 0.25)".into()),
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
                ("--gx-glow-sm".into(), "0 0 8px rgba(0, 204, 255, 0.15)".into()),
                ("--gx-glow-md".into(), "0 0 16px rgba(0, 204, 255, 0.2)".into()),
                ("--gx-glow-lg".into(), "0 0 32px rgba(0, 204, 255, 0.25)".into()),
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
                ("--gx-glow-sm".into(), "0 0 8px rgba(255, 136, 0, 0.15)".into()),
                ("--gx-glow-md".into(), "0 0 16px rgba(255, 136, 0, 0.2)".into()),
                ("--gx-glow-lg".into(), "0 0 32px rgba(255, 136, 0, 0.25)".into()),
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
                ("--gx-glow-sm".into(), "0 0 8px rgba(153, 51, 255, 0.15)".into()),
                ("--gx-glow-md".into(), "0 0 16px rgba(153, 51, 255, 0.2)".into()),
                ("--gx-glow-lg".into(), "0 0 32px rgba(153, 51, 255, 0.25)".into()),
            ],
            is_builtin: true,
        },
    ]
}

// ============================================================================
// SQLITE PERSISTENCE — Theme & Layout storage
// ============================================================================

fn get_db() -> Result<rusqlite::Connection, String> {
    let data_dir = dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("nexus");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Dir create error: {e}"))?;
    let db_path = data_dir.join("themes.db");
    let conn =
        rusqlite::Connection::open(&db_path).map_err(|e| format!("DB open error: {e}"))?;

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
        );",
    )
    .map_err(|e| format!("Schema error: {e}"))?;

    Ok(conn)
}

// ============================================================================
// TAURI COMMANDS — Theme Management
// ============================================================================

/// List all available themes (built-in + custom)
#[tauri::command]
pub async fn theme_list() -> Result<Vec<NexusTheme>, String> {
    let mut themes = builtin_themes();
    if let Ok(conn) = get_db() {
        if let Ok(mut stmt) = conn.prepare("SELECT data FROM themes") {
            let custom: Vec<NexusTheme> = stmt
                .query_map([], |row| {
                    let data: String = row.get(0)?;
                    Ok(serde_json::from_str(&data).unwrap_or_else(|_| NexusTheme {
                        id: "error".into(),
                        name: "Error".into(),
                        author: None,
                        version: "0".into(),
                        variables: vec![],
                        is_builtin: false,
                    }))
                })
                .map_err(|e| format!("Query error: {e}"))?
                .filter_map(|r| r.ok())
                .collect();
            themes.extend(custom);
        }
    }
    Ok(themes)
}

/// Get the active theme
#[tauri::command]
pub async fn theme_get_active() -> Result<NexusTheme, String> {
    let conn = get_db()?;
    let theme_id: String = conn
        .query_row(
            "SELECT theme_id FROM active_theme WHERE key = 'current'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "default-neon-green".into());

    // Check built-in themes first
    let builtins = builtin_themes();
    if let Some(t) = builtins.iter().find(|t| t.id == theme_id) {
        return Ok(t.clone());
    }

    // Check custom themes
    let data: String = conn
        .query_row(
            "SELECT data FROM themes WHERE id = ?1",
            [&theme_id],
            |row| row.get(0),
        )
        .map_err(|_| format!("Theme '{theme_id}' not found"))?;
    serde_json::from_str(&data).map_err(|e| format!("Parse error: {e}"))
}

/// Set the active theme
#[tauri::command]
pub async fn theme_set_active(theme_id: String) -> Result<String, String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT OR REPLACE INTO active_theme (key, theme_id) VALUES ('current', ?1)",
        [&theme_id],
    )
    .map_err(|e| format!("Set active error: {e}"))?;
    Ok(format!("Active theme set to: {theme_id}"))
}

/// Save a custom theme
#[tauri::command]
pub async fn theme_save(theme: NexusTheme) -> Result<String, String> {
    if theme.is_builtin {
        return Err("Cannot modify built-in themes".into());
    }
    let conn = get_db()?;
    let data = serde_json::to_string(&theme).map_err(|e| format!("Serialize error: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO themes (id, data) VALUES (?1, ?2)",
        [&theme.id, &data],
    )
    .map_err(|e| format!("Save error: {e}"))?;
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

/// Export theme + layouts as base64 string (ElvUI-style profile string)
#[tauri::command]
pub async fn theme_export(theme_id: String) -> Result<String, String> {
    // Find theme
    let builtins = builtin_themes();
    let theme = if let Some(t) = builtins.iter().find(|t| t.id == theme_id) {
        t.clone()
    } else {
        let conn = get_db()?;
        let data: String = conn
            .query_row(
                "SELECT data FROM themes WHERE id = ?1",
                [&theme_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("Theme '{theme_id}' not found"))?;
        serde_json::from_str(&data).map_err(|e| format!("Parse: {e}"))?
    };

    // Get layouts for this theme
    let layouts = layout_list_all().await.unwrap_or_default();

    let export = ThemeExport {
        theme,
        layouts,
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
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .map_err(|e| format!("Base64 decode error: {e}"))?;
    let json = String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {e}"))?;
    let export: ThemeExport =
        serde_json::from_str(&json).map_err(|e| format!("Parse error: {e}"))?;

    // Save the imported theme (force non-builtin)
    let mut theme = export.theme;
    theme.is_builtin = false;
    theme_save(theme.clone()).await?;

    // Save imported layouts
    for layout in export.layouts {
        let _ = layout_save(layout).await;
    }

    Ok(theme)
}

// ============================================================================
// TAURI COMMANDS — Layout Management
// ============================================================================

/// Save a widget layout for a route
#[tauri::command]
pub async fn layout_save(layout: WidgetLayout) -> Result<String, String> {
    let conn = get_db()?;
    let data = serde_json::to_string(&layout).map_err(|e| format!("Serialize: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO layouts (id, route, data) VALUES (?1, ?2, ?3)",
        [&layout.id, &layout.route, &data],
    )
    .map_err(|e| format!("Save layout error: {e}"))?;
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
            let layout: WidgetLayout =
                serde_json::from_str(&data).map_err(|e| format!("Parse: {e}"))?;
            Ok(Some(layout))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Query error: {e}")),
    }
}

/// List all layouts
async fn layout_list_all() -> Result<Vec<WidgetLayout>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare("SELECT data FROM layouts")
        .map_err(|e| format!("Prepare: {e}"))?;
    let layouts: Vec<WidgetLayout> = stmt
        .query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(serde_json::from_str(&data).unwrap_or_else(|_| WidgetLayout {
                id: "error".into(),
                name: "Error".into(),
                widgets: vec![],
                route: "/".into(),
            }))
        })
        .map_err(|e| format!("Query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(layouts)
}

/// Delete a layout
#[tauri::command]
pub async fn layout_delete(layout_id: String) -> Result<String, String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM layouts WHERE id = ?1", [&layout_id])
        .map_err(|e| format!("Delete error: {e}"))?;
    Ok(format!("Layout '{layout_id}' deleted"))
}

// ============================================================================
// TESTS
// ============================================================================

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
        let parsed: NexusTheme = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.variables.len(), 2);
        assert!(!parsed.is_builtin);
    }

    #[test]
    fn test_builtin_themes_exist() {
        let themes = builtin_themes();
        assert!(themes.len() >= 5);
        assert!(themes.iter().any(|t| t.id == "default-neon-green"));
        assert!(themes.iter().any(|t| t.id == "cyberpunk-red"));
        assert!(themes.iter().any(|t| t.id == "arctic-blue"));
        assert!(themes.iter().any(|t| t.id == "sunset-orange"));
        assert!(themes.iter().any(|t| t.id == "phantom-purple"));
        assert!(themes.iter().all(|t| t.is_builtin));
    }

    #[test]
    fn test_default_theme_has_no_overrides() {
        let themes = builtin_themes();
        let default = themes.iter().find(|t| t.id == "default-neon-green").unwrap();
        assert!(default.variables.is_empty()); // Uses CSS defaults
    }

    #[test]
    fn test_layout_serialization() {
        let layout = WidgetLayout {
            id: "layout-1".into(),
            name: "My Layout".into(),
            widgets: vec![WidgetPlacement {
                widget_id: "system-stats".into(),
                x: 0,
                y: 0,
                w: 4,
                h: 2,
                config: serde_json::json!({}),
            }],
            route: "/".into(),
        };
        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.contains("system-stats"));
        let parsed: WidgetLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.widgets.len(), 1);
        assert_eq!(parsed.widgets[0].w, 4);
    }

    #[test]
    fn test_widget_placement_serialization() {
        let wp = WidgetPlacement {
            widget_id: "quick-chat".into(),
            x: 4,
            y: 0,
            w: 6,
            h: 4,
            config: serde_json::json!({"model": "dolphin3", "max_tokens": 1024}),
        };
        let json = serde_json::to_string(&wp).unwrap();
        assert!(json.contains("quick-chat"));
        assert!(json.contains("dolphin3"));
    }

    #[test]
    fn test_theme_export_serialization() {
        let export = ThemeExport {
            theme: NexusTheme {
                id: "test".into(),
                name: "Test".into(),
                author: None,
                version: "1.0.0".into(),
                variables: vec![],
                is_builtin: false,
            },
            layouts: vec![],
            nexus_version: "0.1.0".into(),
            export_date: "2026-03-08T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&export).unwrap();
        assert!(json.contains("0.1.0"));
        assert!(json.contains("2026-03-08"));
    }

    #[test]
    fn test_theme_export_base64_roundtrip() {
        let theme = NexusTheme {
            id: "roundtrip".into(),
            name: "Roundtrip Test".into(),
            author: Some("Test".into()),
            version: "1.0.0".into(),
            variables: vec![("--color-gx-neon".into(), "#FF0000".into())],
            is_builtin: false,
        };
        let export = ThemeExport {
            theme: theme.clone(),
            layouts: vec![],
            nexus_version: "0.1.0".into(),
            export_date: "2026-03-08T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&export).unwrap();
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        let decoded_json = String::from_utf8(decoded_bytes).unwrap();
        let decoded: ThemeExport = serde_json::from_str(&decoded_json).unwrap();
        assert_eq!(decoded.theme.id, "roundtrip");
        assert_eq!(decoded.theme.variables[0].1, "#FF0000");
    }
}
