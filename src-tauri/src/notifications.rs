// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Notification Center — In-app notification system for ImpForge.
//!
//! Manages notifications of various types (achievements, reminders, AI suggestions,
//! workflow completions, etc.) with read/unread tracking, persistent storage,
//! and unread badge counts.
//!
//! All data lives in `~/.impforge/notifications.json` — fully offline.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Notification type categories — each has a distinct visual treatment in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    /// Unlocked achievement or XP milestone
    Achievement,
    /// Team activity or collaboration update
    TeamUpdate,
    /// Scheduled reminder or deadline
    Reminder,
    /// System warning or error alert
    SystemAlert,
    /// AI-generated suggestion or insight
    AiSuggestion,
    /// Background workflow finished
    WorkflowComplete,
}

impl NotificationType {
    fn from_str(s: &str) -> Self {
        match s {
            "achievement" => Self::Achievement,
            "team_update" => Self::TeamUpdate,
            "reminder" => Self::Reminder,
            "system_alert" => Self::SystemAlert,
            "ai_suggestion" => Self::AiSuggestion,
            "workflow_complete" => Self::WorkflowComplete,
            _ => Self::SystemAlert,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Achievement => "Trophy",
            Self::TeamUpdate => "Users",
            Self::Reminder => "Clock",
            Self::SystemAlert => "AlertTriangle",
            Self::AiSuggestion => "Sparkles",
            Self::WorkflowComplete => "CheckCircle",
        }
    }
}

/// A single notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub message: String,
    pub notification_type: NotificationType,
    pub read: bool,
    pub created_at: String,
    /// Optional route to navigate to when the notification is clicked.
    pub action_route: Option<String>,
}

/// In-memory store protected by a Mutex (managed by Tauri).
pub struct NotificationStore {
    notifications: Vec<Notification>,
    dirty: bool,
}

impl NotificationStore {
    pub fn new() -> Self {
        let notifications = load_from_disk();
        Self {
            notifications,
            dirty: false,
        }
    }

    fn persist(&mut self) {
        if self.dirty {
            save_to_disk(&self.notifications);
            self.dirty = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impforge")
}

fn storage_path() -> PathBuf {
    data_dir().join("notifications.json")
}

fn load_from_disk() -> Vec<Notification> {
    let path = storage_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_to_disk(notifications: &[Notification]) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let json = serde_json::to_string_pretty(notifications).unwrap_or_default();
    let _ = std::fs::write(storage_path(), json);
}

fn generate_id() -> String {
    format!("notif_{}", uuid::Uuid::new_v4().as_simple())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// List notifications, optionally filtering to unread only.
#[tauri::command]
pub fn notifications_list(
    unread_only: bool,
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<Vec<Notification>> {
    let guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let results: Vec<Notification> = if unread_only {
        guard
            .notifications
            .iter()
            .filter(|n| !n.read)
            .cloned()
            .collect()
    } else {
        guard.notifications.clone()
    };

    Ok(results)
}

/// Mark a single notification as read.
#[tauri::command]
pub fn notifications_mark_read(
    id: String,
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<()> {
    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    if let Some(notif) = guard.notifications.iter_mut().find(|n| n.id == id) {
        notif.read = true;
        guard.dirty = true;
        guard.persist();
    }

    Ok(())
}

/// Mark all notifications as read.
#[tauri::command]
pub fn notifications_mark_all_read(
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<()> {
    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let mut changed = false;
    for notif in &mut guard.notifications {
        if !notif.read {
            notif.read = true;
            changed = true;
        }
    }

    if changed {
        guard.dirty = true;
        guard.persist();
    }

    Ok(())
}

/// Push a new notification. Returns the created notification.
#[tauri::command]
pub fn notifications_push(
    title: String,
    message: String,
    ntype: String,
    action_route: Option<String>,
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<Notification> {
    let notification = Notification {
        id: generate_id(),
        title,
        message,
        notification_type: NotificationType::from_str(&ntype),
        read: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        action_route,
    };

    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    // Prepend (newest first)
    guard.notifications.insert(0, notification.clone());

    // Cap at 200 notifications
    guard.notifications.truncate(200);
    guard.dirty = true;
    guard.persist();

    Ok(notification)
}

/// Get the count of unread notifications (for badge display).
#[tauri::command]
pub fn notifications_unread_count(
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<u32> {
    let guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let count = guard.notifications.iter().filter(|n| !n.read).count() as u32;
    Ok(count)
}

/// Delete a single notification by ID.
#[tauri::command]
pub fn notifications_delete(
    id: String,
    store: tauri::State<'_, Mutex<NotificationStore>>,
) -> AppResult<()> {
    let mut guard = store.lock().map_err(|e| {
        ImpForgeError::internal("LOCK_ERROR", format!("Failed to acquire lock: {e}"))
    })?;

    let before = guard.notifications.len();
    guard.notifications.retain(|n| n.id != id);

    if guard.notifications.len() != before {
        guard.dirty = true;
        guard.persist();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type_from_str() {
        assert_eq!(
            NotificationType::from_str("achievement"),
            NotificationType::Achievement
        );
        assert_eq!(
            NotificationType::from_str("ai_suggestion"),
            NotificationType::AiSuggestion
        );
        assert_eq!(
            NotificationType::from_str("unknown_value"),
            NotificationType::SystemAlert
        );
    }

    #[test]
    fn test_notification_type_icon() {
        assert_eq!(NotificationType::Achievement.icon(), "Trophy");
        assert_eq!(NotificationType::WorkflowComplete.icon(), "CheckCircle");
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.starts_with("notif_"));
        assert!(id.len() > 10);
    }

    #[test]
    fn test_notification_store_new() {
        // Constructs without panic (disk may not have the file)
        let store = NotificationStore::new();
        // Just check it does not panic
        assert!(!store.dirty);
    }

    #[test]
    fn test_notification_serialization() {
        let n = Notification {
            id: "test_1".to_string(),
            title: "Test".to_string(),
            message: "Hello".to_string(),
            notification_type: NotificationType::Reminder,
            read: false,
            created_at: "2026-03-18T00:00:00Z".to_string(),
            action_route: Some("/calendar".to_string()),
        };

        let json = serde_json::to_string(&n).expect("serialize");
        assert!(json.contains("reminder"));
        assert!(json.contains("/calendar"));

        let parsed: Notification = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, "test_1");
        assert_eq!(parsed.notification_type, NotificationType::Reminder);
    }
}
