//! NEXUS Widget Registry — Modular dashboard components
//!
//! Each widget is a self-contained UI component that customers can
//! place on their dashboard via drag-and-drop layout manager.
//! Inspired by ElvUI's module registration system.
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
}

/// Get all built-in widgets
pub fn builtin_widgets() -> Vec<WidgetDefinition> {
    vec![
        WidgetDefinition {
            id: "system-stats".into(),
            name: "System Stats".into(),
            description: "CPU, RAM, GPU, Temperature".into(),
            category: WidgetCategory::Monitoring,
            default_size: (4, 2),
            min_size: (2, 1),
            max_size: (12, 4),
            configurable: true,
        },
        WidgetDefinition {
            id: "agent-pool".into(),
            name: "Agent Pool".into(),
            description: "NeuralSwarm agent status".into(),
            category: WidgetCategory::AI,
            default_size: (4, 3),
            min_size: (3, 2),
            max_size: (8, 6),
            configurable: true,
        },
        WidgetDefinition {
            id: "quick-chat".into(),
            name: "Quick Chat".into(),
            description: "Inline AI chat widget".into(),
            category: WidgetCategory::AI,
            default_size: (6, 4),
            min_size: (4, 3),
            max_size: (12, 8),
            configurable: true,
        },
        WidgetDefinition {
            id: "docker-overview".into(),
            name: "Docker Overview".into(),
            description: "Running containers summary".into(),
            category: WidgetCategory::Development,
            default_size: (4, 2),
            min_size: (3, 2),
            max_size: (8, 4),
            configurable: false,
        },
        WidgetDefinition {
            id: "github-feed".into(),
            name: "GitHub Feed".into(),
            description: "Recent commits and PRs".into(),
            category: WidgetCategory::Development,
            default_size: (4, 3),
            min_size: (3, 2),
            max_size: (8, 6),
            configurable: true,
        },
        WidgetDefinition {
            id: "browser-sessions".into(),
            name: "Browser Sessions".into(),
            description: "Active CDP pages".into(),
            category: WidgetCategory::Browser,
            default_size: (3, 2),
            min_size: (2, 1),
            max_size: (6, 4),
            configurable: false,
        },
        WidgetDefinition {
            id: "network-waterfall".into(),
            name: "Network Waterfall".into(),
            description: "Live HTTP request monitor".into(),
            category: WidgetCategory::Browser,
            default_size: (6, 3),
            min_size: (4, 2),
            max_size: (12, 6),
            configurable: true,
        },
        WidgetDefinition {
            id: "model-status".into(),
            name: "Model Status".into(),
            description: "Ollama/local model health".into(),
            category: WidgetCategory::AI,
            default_size: (3, 2),
            min_size: (2, 1),
            max_size: (6, 3),
            configurable: false,
        },
        WidgetDefinition {
            id: "eval-pipeline".into(),
            name: "Eval Pipeline".into(),
            description: "Agent evaluation results".into(),
            category: WidgetCategory::AI,
            default_size: (4, 3),
            min_size: (3, 2),
            max_size: (8, 5),
            configurable: true,
        },
        WidgetDefinition {
            id: "news-ticker".into(),
            name: "News Ticker".into(),
            description: "AI news headlines".into(),
            category: WidgetCategory::System,
            default_size: (4, 1),
            min_size: (3, 1),
            max_size: (12, 2),
            configurable: true,
        },
        WidgetDefinition {
            id: "workflow-status".into(),
            name: "Workflow Status".into(),
            description: "n8n/Zapier workflow monitor".into(),
            category: WidgetCategory::Automation,
            default_size: (4, 2),
            min_size: (3, 2),
            max_size: (8, 4),
            configurable: true,
        },
        WidgetDefinition {
            id: "console-output".into(),
            name: "Console Output".into(),
            description: "Browser console log viewer".into(),
            category: WidgetCategory::Browser,
            default_size: (6, 3),
            min_size: (4, 2),
            max_size: (12, 6),
            configurable: true,
        },
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
        };
        assert_eq!(widget.id, "system-stats");
        assert!(widget.configurable);
        assert_eq!(widget.default_size, (4, 2));
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
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: WidgetDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-widget");
        assert_eq!(parsed.category, WidgetCategory::Development);
        assert!(!parsed.configurable);
    }
}
