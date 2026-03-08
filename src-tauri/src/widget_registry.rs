//! ImpForge Widget Registry — Enterprise Modular Dashboard Components
//!
//! Each widget is a self-contained UI component that customers can
//! place on their dashboard via drag-and-drop layout manager.
//! Inspired by ElvUI's module registration system.
//!
//! Architecture:
//! - WidgetDefinition: Static metadata (size, category, capabilities)
//! - WidgetConfigSchema: Typed configuration fields per widget
//! - Capabilities: Bitflag-style feature flags (resizable, removable, etc.)
//! - Version tracking for schema migration
//!
//! References:
//! - ElvUI module system (GPL → clean-room MIT reimplementation)
//! - Grafana panel plugin architecture (Apache-2.0)
//! - Retool WidgetGrid component model
//!
//! License: MIT (all original code)

use serde::{Deserialize, Serialize};

/// Widget category for organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WidgetCategory {
    Monitoring,
    AI,
    Development,
    Browser,
    Automation,
    System,
}

/// Widget capability flags — what operations are allowed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetCapabilities {
    pub resizable: bool,
    pub removable: bool,
    pub duplicatable: bool,
    pub has_settings: bool,
    pub refreshable: bool,
}

impl Default for WidgetCapabilities {
    fn default() -> Self {
        Self {
            resizable: true,
            removable: true,
            duplicatable: false,
            has_settings: false,
            refreshable: false,
        }
    }
}

/// Config field type for widget settings UI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ConfigFieldType {
    #[serde(rename = "toggle")]
    Toggle { default: bool },
    #[serde(rename = "number")]
    Number { default: f64, min: f64, max: f64, step: f64 },
    #[serde(rename = "select")]
    Select { default: String, options: Vec<String> },
    #[serde(rename = "text")]
    Text { default: String, placeholder: String },
    #[serde(rename = "color")]
    Color { default: String },
}

/// A single config field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub field_type: ConfigFieldType,
}

/// Definition of a dashboard widget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: WidgetCategory,
    pub default_size: (u32, u32), // (width, height) in grid units
    pub min_size: (u32, u32),
    pub max_size: (u32, u32),
    pub configurable: bool,
    pub version: String,
    pub capabilities: WidgetCapabilities,
    pub config_schema: Vec<ConfigField>,
    pub refresh_interval_ms: Option<u64>,
}

/// Helper macro to reduce boilerplate for widget definitions
macro_rules! widget {
    ($id:expr, $name:expr, $desc:expr, $cat:expr,
     default: $dw:expr, $dh:expr, min: $mw:expr, $mh:expr, max: $xw:expr, $xh:expr,
     $($rest:tt)*) => {
        WidgetDefinition {
            id: $id.into(),
            name: $name.into(),
            description: $desc.into(),
            category: $cat,
            default_size: ($dw, $dh),
            min_size: ($mw, $mh),
            max_size: ($xw, $xh),
            version: "1.0.0".into(),
            $($rest)*
        }
    };
}

/// Get all built-in widgets
pub fn builtin_widgets() -> Vec<WidgetDefinition> {
    vec![
        widget!("system-stats", "System Stats", "CPU, RAM, GPU, Temperature", WidgetCategory::Monitoring,
            default: 4, 2, min: 2, 1, max: 12, 4,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "show_gpu".into(), label: "Show GPU".into(),
                    field_type: ConfigFieldType::Toggle { default: true },
                },
                ConfigField {
                    key: "refresh_rate".into(), label: "Refresh Rate (ms)".into(),
                    field_type: ConfigFieldType::Number { default: 2000.0, min: 500.0, max: 10000.0, step: 500.0 },
                },
            ],
            refresh_interval_ms: Some(2000),
        ),
        widget!("agent-pool", "Agent Pool", "NeuralSwarm agent status", WidgetCategory::AI,
            default: 4, 3, min: 3, 2, max: 8, 6,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "show_model".into(), label: "Show Active Model".into(),
                    field_type: ConfigFieldType::Toggle { default: true },
                },
            ],
            refresh_interval_ms: Some(5000),
        ),
        widget!("quick-chat", "Quick Chat", "Inline AI chat widget", WidgetCategory::AI,
            default: 6, 4, min: 4, 3, max: 12, 8,
            configurable: true,
            capabilities: WidgetCapabilities { duplicatable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "model".into(), label: "Model".into(),
                    field_type: ConfigFieldType::Select {
                        default: "auto".into(),
                        options: vec!["auto".into(), "dolphin3:8b".into(), "qwen2.5-coder:7b".into()],
                    },
                },
                ConfigField {
                    key: "max_tokens".into(), label: "Max Tokens".into(),
                    field_type: ConfigFieldType::Number { default: 1024.0, min: 128.0, max: 8192.0, step: 128.0 },
                },
                ConfigField {
                    key: "system_prompt".into(), label: "System Prompt".into(),
                    field_type: ConfigFieldType::Text { default: "".into(), placeholder: "Custom instructions...".into() },
                },
            ],
            refresh_interval_ms: None,
        ),
        widget!("docker-overview", "Docker Overview", "Running containers summary", WidgetCategory::Development,
            default: 4, 2, min: 3, 2, max: 8, 4,
            configurable: false,
            capabilities: WidgetCapabilities { refreshable: true, ..Default::default() },
            config_schema: vec![],
            refresh_interval_ms: Some(10000),
        ),
        widget!("github-feed", "GitHub Feed", "Recent commits and PRs", WidgetCategory::Development,
            default: 4, 3, min: 3, 2, max: 8, 6,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "max_items".into(), label: "Max Items".into(),
                    field_type: ConfigFieldType::Number { default: 10.0, min: 3.0, max: 50.0, step: 1.0 },
                },
                ConfigField {
                    key: "show_prs".into(), label: "Show Pull Requests".into(),
                    field_type: ConfigFieldType::Toggle { default: true },
                },
            ],
            refresh_interval_ms: Some(30000),
        ),
        widget!("browser-sessions", "Browser Sessions", "Active CDP pages", WidgetCategory::Browser,
            default: 3, 2, min: 2, 1, max: 6, 4,
            configurable: false,
            capabilities: WidgetCapabilities { refreshable: true, ..Default::default() },
            config_schema: vec![],
            refresh_interval_ms: Some(3000),
        ),
        widget!("network-waterfall", "Network Waterfall", "Live HTTP request monitor", WidgetCategory::Browser,
            default: 6, 3, min: 4, 2, max: 12, 6,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "max_entries".into(), label: "Max Entries".into(),
                    field_type: ConfigFieldType::Number { default: 100.0, min: 20.0, max: 500.0, step: 10.0 },
                },
                ConfigField {
                    key: "filter_type".into(), label: "Filter Type".into(),
                    field_type: ConfigFieldType::Select {
                        default: "all".into(),
                        options: vec!["all".into(), "xhr".into(), "js".into(), "css".into(), "img".into(), "doc".into()],
                    },
                },
            ],
            refresh_interval_ms: Some(1000),
        ),
        widget!("model-status", "Model Status", "Ollama/local model health", WidgetCategory::AI,
            default: 3, 2, min: 2, 1, max: 6, 3,
            configurable: false,
            capabilities: WidgetCapabilities { refreshable: true, ..Default::default() },
            config_schema: vec![],
            refresh_interval_ms: Some(15000),
        ),
        widget!("eval-pipeline", "Eval Pipeline", "Agent evaluation results", WidgetCategory::AI,
            default: 4, 3, min: 3, 2, max: 8, 5,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "auto_eval".into(), label: "Auto-Evaluate".into(),
                    field_type: ConfigFieldType::Toggle { default: false },
                },
            ],
            refresh_interval_ms: Some(30000),
        ),
        widget!("news-ticker", "News Ticker", "AI news headlines", WidgetCategory::System,
            default: 4, 1, min: 3, 1, max: 12, 2,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "scroll_speed".into(), label: "Scroll Speed".into(),
                    field_type: ConfigFieldType::Number { default: 3.0, min: 1.0, max: 10.0, step: 1.0 },
                },
            ],
            refresh_interval_ms: Some(60000),
        ),
        widget!("workflow-status", "Workflow Status", "n8n/Zapier workflow monitor", WidgetCategory::Automation,
            default: 4, 2, min: 3, 2, max: 8, 4,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "n8n_url".into(), label: "n8n URL".into(),
                    field_type: ConfigFieldType::Text { default: "http://localhost:5678".into(), placeholder: "n8n instance URL".into() },
                },
            ],
            refresh_interval_ms: Some(10000),
        ),
        widget!("console-output", "Console Output", "Browser console log viewer", WidgetCategory::Browser,
            default: 6, 3, min: 4, 2, max: 12, 6,
            configurable: true,
            capabilities: WidgetCapabilities { refreshable: true, has_settings: true, ..Default::default() },
            config_schema: vec![
                ConfigField {
                    key: "log_level".into(), label: "Min Log Level".into(),
                    field_type: ConfigFieldType::Select {
                        default: "all".into(),
                        options: vec!["all".into(), "info".into(), "warn".into(), "error".into()],
                    },
                },
                ConfigField {
                    key: "max_lines".into(), label: "Max Lines".into(),
                    field_type: ConfigFieldType::Number { default: 200.0, min: 50.0, max: 1000.0, step: 50.0 },
                },
            ],
            refresh_interval_ms: Some(1000),
        ),
    ]
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// List all available widgets
#[tauri::command]
pub async fn widget_list() -> Result<Vec<WidgetDefinition>, String> {
    Ok(builtin_widgets())
}

/// Get a specific widget by ID
#[tauri::command]
pub async fn widget_get(widget_id: String) -> Result<WidgetDefinition, String> {
    builtin_widgets()
        .into_iter()
        .find(|w| w.id == widget_id)
        .ok_or_else(|| format!("Widget '{widget_id}' not found"))
}

/// List all widget categories
#[tauri::command]
pub async fn widget_categories() -> Result<Vec<String>, String> {
    Ok(vec![
        "Monitoring".into(),
        "AI".into(),
        "Development".into(),
        "Browser".into(),
        "Automation".into(),
        "System".into(),
    ])
}

/// Get config schema for a specific widget
#[tauri::command]
pub async fn widget_config_schema(widget_id: String) -> Result<Vec<ConfigField>, String> {
    builtin_widgets()
        .into_iter()
        .find(|w| w.id == widget_id)
        .map(|w| w.config_schema)
        .ok_or_else(|| format!("Widget '{widget_id}' not found"))
}

// ============================================================================
// TESTS
// ============================================================================

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
            version: "1.0.0".into(),
            capabilities: WidgetCapabilities::default(),
            config_schema: vec![],
            refresh_interval_ms: Some(2000),
        };
        assert_eq!(widget.id, "system-stats");
        assert!(widget.configurable);
        assert_eq!(widget.default_size, (4, 2));
        assert!(widget.capabilities.resizable);
        assert_eq!(widget.refresh_interval_ms, Some(2000));
    }

    #[test]
    fn test_widget_capabilities_default() {
        let caps = WidgetCapabilities::default();
        assert!(caps.resizable);
        assert!(caps.removable);
        assert!(!caps.duplicatable);
        assert!(!caps.has_settings);
        assert!(!caps.refreshable);
    }

    #[test]
    fn test_config_field_types() {
        let toggle = ConfigFieldType::Toggle { default: true };
        let json = serde_json::to_string(&toggle).unwrap();
        assert!(json.contains("toggle"));

        let number = ConfigFieldType::Number { default: 5.0, min: 0.0, max: 100.0, step: 1.0 };
        let json = serde_json::to_string(&number).unwrap();
        assert!(json.contains("number"));

        let select = ConfigFieldType::Select { default: "auto".into(), options: vec!["auto".into(), "manual".into()] };
        let json = serde_json::to_string(&select).unwrap();
        assert!(json.contains("select"));
    }

    #[test]
    fn test_builtin_widgets_exist() {
        let widgets = builtin_widgets();
        assert!(widgets.len() >= 10);
        assert!(widgets.iter().any(|w| w.id == "system-stats"));
        assert!(widgets.iter().any(|w| w.id == "agent-pool"));
        assert!(widgets.iter().any(|w| w.id == "quick-chat"));
        assert!(widgets.iter().any(|w| w.id == "network-waterfall"));
        assert!(widgets.iter().any(|w| w.id == "console-output"));
    }

    #[test]
    fn test_widget_category_serialization() {
        let cat = WidgetCategory::AI;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"AI\"");

        let parsed: WidgetCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WidgetCategory::AI);
    }

    #[test]
    fn test_widget_sizes_valid() {
        for widget in builtin_widgets() {
            assert!(
                widget.min_size.0 <= widget.default_size.0,
                "{}: min_w > default_w",
                widget.id
            );
            assert!(
                widget.min_size.1 <= widget.default_size.1,
                "{}: min_h > default_h",
                widget.id
            );
            assert!(
                widget.default_size.0 <= widget.max_size.0,
                "{}: default_w > max_w",
                widget.id
            );
            assert!(
                widget.default_size.1 <= widget.max_size.1,
                "{}: default_h > max_h",
                widget.id
            );
        }
    }

    #[test]
    fn test_widget_serialization_roundtrip() {
        let widget = WidgetDefinition {
            id: "test-widget".into(),
            name: "Test Widget".into(),
            description: "For testing".into(),
            category: WidgetCategory::Development,
            default_size: (3, 2),
            min_size: (2, 1),
            max_size: (6, 4),
            configurable: false,
            version: "1.0.0".into(),
            capabilities: WidgetCapabilities::default(),
            config_schema: vec![ConfigField {
                key: "test_key".into(),
                label: "Test Field".into(),
                field_type: ConfigFieldType::Toggle { default: false },
            }],
            refresh_interval_ms: None,
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: WidgetDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-widget");
        assert_eq!(parsed.category, WidgetCategory::Development);
        assert!(!parsed.configurable);
        assert_eq!(parsed.config_schema.len(), 1);
        assert_eq!(parsed.config_schema[0].key, "test_key");
    }

    #[test]
    fn test_widgets_have_config_schemas() {
        let widgets = builtin_widgets();
        let configurable: Vec<_> = widgets.iter().filter(|w| w.configurable).collect();
        for w in &configurable {
            assert!(
                !w.config_schema.is_empty(),
                "Configurable widget '{}' should have config schema",
                w.id
            );
        }
    }

    #[test]
    fn test_widgets_have_versions() {
        for widget in builtin_widgets() {
            assert!(
                !widget.version.is_empty(),
                "Widget '{}' should have a version",
                widget.id
            );
        }
    }
}
