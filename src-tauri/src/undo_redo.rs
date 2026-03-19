// SPDX-License-Identifier: Apache-2.0
//! Undo/Redo System -- Generic Command Pattern for ImpForge
//!
//! Provides a cross-module undo/redo stack that any ImpForge module
//! (ForgeWriter, ForgeSheets, ForgeNotes, ForgeSlides, etc.) can push
//! actions onto. The frontend calls `undo_undo` / `undo_redo` and
//! receives the action to reverse/reapply.
//!
//! Design:
//! - `UndoRedoManager` holds a bounded history (`VecDeque`) and a redo stack (`Vec`).
//! - Each `UndoAction` captures before/after state as `serde_json::Value`,
//!   making the system module-agnostic.
//! - Pushing a new action clears the redo stack (standard undo semantics).
//! - The manager is stored as Tauri managed state behind a `Mutex`.
//!
//! References:
//! - Command Pattern (GoF) for undo/redo
//! - arXiv:2301.13735 — Structured editing with undo trees

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use tauri::State;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single undoable action capturing before/after state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoAction {
    /// Unique identifier for this action.
    pub id: String,
    /// Which module produced this action (e.g. "writer", "sheets", "notes").
    pub module: String,
    /// Machine-readable action type (e.g. "edit_cell", "delete_text", "add_slide").
    pub action_type: String,
    /// Human-readable description (e.g. "Edit cell A1", "Delete paragraph").
    pub description: String,
    /// Serialised state before the action was applied.
    pub before: serde_json::Value,
    /// Serialised state after the action was applied.
    pub after: serde_json::Value,
    /// ISO-8601 timestamp of when the action occurred.
    pub timestamp: String,
}

/// Summary of the current undo/redo state for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRedoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub history_count: usize,
    pub redo_count: usize,
    pub max_history: usize,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// A generic undo/redo manager using the Command Pattern.
///
/// Thread-safe access is ensured by the `Mutex` wrapper in Tauri managed state.
pub struct UndoRedoManager {
    /// Bounded undo history (oldest at front, newest at back).
    history: VecDeque<UndoAction>,
    /// Redo stack (most recently undone action on top).
    redo_stack: Vec<UndoAction>,
    /// Maximum number of undo steps retained.
    max_history: usize,
}

impl UndoRedoManager {
    /// Create a new manager with the given history capacity.
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// Push a new action onto the undo history.
    ///
    /// This clears the redo stack (a new action invalidates any
    /// previously undone actions, per standard undo semantics).
    pub fn push(&mut self, action: UndoAction) {
        self.redo_stack.clear();
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(action);
    }

    /// Undo the most recent action.
    ///
    /// Returns the action that was undone (caller uses `.before` to restore state).
    /// The action is moved to the redo stack.
    pub fn undo(&mut self) -> Option<UndoAction> {
        let action = self.history.pop_back()?;
        self.redo_stack.push(action.clone());
        Some(action)
    }

    /// Redo the most recently undone action.
    ///
    /// Returns the action that was redone (caller uses `.after` to reapply state).
    /// The action is moved back to the history.
    pub fn redo(&mut self) -> Option<UndoAction> {
        let action = self.redo_stack.pop()?;
        self.history.push_back(action.clone());
        Some(action)
    }

    /// Whether an undo operation is possible.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Whether a redo operation is possible.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of actions currently in the undo history.
    pub fn history_count(&self) -> usize {
        self.history.len()
    }

    /// Number of actions on the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Return a snapshot of the full undo history (oldest first).
    pub fn history_list(&self) -> Vec<UndoAction> {
        self.history.iter().cloned().collect()
    }

    /// Return the current status for the frontend.
    pub fn status(&self) -> UndoRedoStatus {
        UndoRedoStatus {
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            history_count: self.history_count(),
            redo_count: self.redo_count(),
            max_history: self.max_history,
        }
    }

    /// Clear all history and redo state.
    pub fn clear(&mut self) {
        self.history.clear();
        self.redo_stack.clear();
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Push a new undoable action.
#[tauri::command]
pub fn undo_push(
    action: UndoAction,
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<UndoRedoStatus, String> {
    let mut mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    mgr.push(action);
    Ok(mgr.status())
}

/// Undo the most recent action.
///
/// Returns the undone action (with `.before` state) or `null` if nothing to undo.
#[tauri::command]
pub fn undo_undo(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<Option<UndoAction>, String> {
    let mut mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.undo())
}

/// Redo the most recently undone action.
///
/// Returns the redone action (with `.after` state) or `null` if nothing to redo.
#[tauri::command]
pub fn undo_redo(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<Option<UndoAction>, String> {
    let mut mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.redo())
}

/// Check whether undo is possible.
#[tauri::command]
pub fn undo_can_undo(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<bool, String> {
    let mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.can_undo())
}

/// Check whether redo is possible.
#[tauri::command]
pub fn undo_can_redo(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<bool, String> {
    let mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.can_redo())
}

/// Get the full undo history (oldest first).
#[tauri::command]
pub fn undo_history(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<Vec<UndoAction>, String> {
    let mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.history_list())
}

/// Get the current undo/redo status (counts, capabilities).
#[tauri::command]
pub fn undo_status(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<UndoRedoStatus, String> {
    let mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    Ok(mgr.status())
}

/// Clear all undo/redo history.
#[tauri::command]
pub fn undo_clear(
    state: State<'_, Mutex<UndoRedoManager>>,
) -> Result<(), String> {
    let mut mgr = state
        .lock()
        .map_err(|e| format!("Undo state lock poisoned: {e}"))?;
    mgr.clear();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_action(id: &str, module: &str, action_type: &str) -> UndoAction {
        UndoAction {
            id: id.to_string(),
            module: module.to_string(),
            action_type: action_type.to_string(),
            description: format!("Test action {id}"),
            before: serde_json::json!({"value": "old"}),
            after: serde_json::json!({"value": "new"}),
            timestamp: "2026-03-18T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_new_manager() {
        let mgr = UndoRedoManager::new(50);
        assert_eq!(mgr.max_history, 50);
        assert!(!mgr.can_undo());
        assert!(!mgr.can_redo());
        assert_eq!(mgr.history_count(), 0);
        assert_eq!(mgr.redo_count(), 0);
    }

    #[test]
    fn test_push_and_undo() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "writer", "edit_text"));
        mgr.push(make_action("2", "writer", "delete_text"));

        assert!(mgr.can_undo());
        assert!(!mgr.can_redo());
        assert_eq!(mgr.history_count(), 2);

        let undone = mgr.undo();
        assert!(undone.is_some());
        let undone = undone.expect("should have action");
        assert_eq!(undone.id, "2");
        assert_eq!(undone.before, serde_json::json!({"value": "old"}));

        assert!(mgr.can_undo());
        assert!(mgr.can_redo());
        assert_eq!(mgr.history_count(), 1);
        assert_eq!(mgr.redo_count(), 1);
    }

    #[test]
    fn test_redo() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "sheets", "edit_cell"));
        mgr.undo();

        assert!(mgr.can_redo());
        let redone = mgr.redo();
        assert!(redone.is_some());
        let redone = redone.expect("should have action");
        assert_eq!(redone.id, "1");
        assert_eq!(redone.after, serde_json::json!({"value": "new"}));

        assert!(mgr.can_undo());
        assert!(!mgr.can_redo());
    }

    #[test]
    fn test_new_push_clears_redo() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "notes", "add_note"));
        mgr.push(make_action("2", "notes", "edit_note"));
        mgr.undo(); // undo "2", now redo has "2"
        assert!(mgr.can_redo());

        // New action invalidates redo
        mgr.push(make_action("3", "notes", "delete_note"));
        assert!(!mgr.can_redo());
        assert_eq!(mgr.redo_count(), 0);
        assert_eq!(mgr.history_count(), 2); // "1" and "3"
    }

    #[test]
    fn test_max_history_cap() {
        let mut mgr = UndoRedoManager::new(3);

        mgr.push(make_action("1", "writer", "a"));
        mgr.push(make_action("2", "writer", "b"));
        mgr.push(make_action("3", "writer", "c"));
        assert_eq!(mgr.history_count(), 3);

        // Fourth push should evict the oldest
        mgr.push(make_action("4", "writer", "d"));
        assert_eq!(mgr.history_count(), 3);

        let list = mgr.history_list();
        assert_eq!(list[0].id, "2");
        assert_eq!(list[1].id, "3");
        assert_eq!(list[2].id, "4");
    }

    #[test]
    fn test_undo_empty() {
        let mut mgr = UndoRedoManager::new(100);
        assert!(mgr.undo().is_none());
    }

    #[test]
    fn test_redo_empty() {
        let mut mgr = UndoRedoManager::new(100);
        assert!(mgr.redo().is_none());
    }

    #[test]
    fn test_clear() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "slides", "add_slide"));
        mgr.push(make_action("2", "slides", "edit_slide"));
        mgr.undo();

        assert!(mgr.can_undo());
        assert!(mgr.can_redo());

        mgr.clear();

        assert!(!mgr.can_undo());
        assert!(!mgr.can_redo());
        assert_eq!(mgr.history_count(), 0);
        assert_eq!(mgr.redo_count(), 0);
    }

    #[test]
    fn test_status() {
        let mut mgr = UndoRedoManager::new(50);

        let status = mgr.status();
        assert!(!status.can_undo);
        assert!(!status.can_redo);
        assert_eq!(status.history_count, 0);
        assert_eq!(status.redo_count, 0);
        assert_eq!(status.max_history, 50);

        mgr.push(make_action("1", "canvas", "draw"));
        mgr.push(make_action("2", "canvas", "erase"));
        mgr.undo();

        let status = mgr.status();
        assert!(status.can_undo);
        assert!(status.can_redo);
        assert_eq!(status.history_count, 1);
        assert_eq!(status.redo_count, 1);
    }

    #[test]
    fn test_history_list_order() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("a", "writer", "x"));
        mgr.push(make_action("b", "writer", "y"));
        mgr.push(make_action("c", "writer", "z"));

        let list = mgr.history_list();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, "a");
        assert_eq!(list[1].id, "b");
        assert_eq!(list[2].id, "c");
    }

    #[test]
    fn test_multiple_undo_redo_cycles() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "sheets", "a"));
        mgr.push(make_action("2", "sheets", "b"));
        mgr.push(make_action("3", "sheets", "c"));

        // Undo all three
        assert_eq!(mgr.undo().map(|a| a.id), Some("3".into()));
        assert_eq!(mgr.undo().map(|a| a.id), Some("2".into()));
        assert_eq!(mgr.undo().map(|a| a.id), Some("1".into()));
        assert!(mgr.undo().is_none());

        assert_eq!(mgr.history_count(), 0);
        assert_eq!(mgr.redo_count(), 3);

        // Redo all three
        assert_eq!(mgr.redo().map(|a| a.id), Some("1".into()));
        assert_eq!(mgr.redo().map(|a| a.id), Some("2".into()));
        assert_eq!(mgr.redo().map(|a| a.id), Some("3".into()));
        assert!(mgr.redo().is_none());

        assert_eq!(mgr.history_count(), 3);
        assert_eq!(mgr.redo_count(), 0);
    }

    #[test]
    fn test_cross_module_actions() {
        let mut mgr = UndoRedoManager::new(100);

        mgr.push(make_action("1", "writer", "edit_text"));
        mgr.push(make_action("2", "sheets", "edit_cell"));
        mgr.push(make_action("3", "notes", "add_note"));

        let undone = mgr.undo().expect("should undo");
        assert_eq!(undone.module, "notes");

        let undone = mgr.undo().expect("should undo");
        assert_eq!(undone.module, "sheets");
    }

    #[test]
    fn test_action_serialization() {
        let action = make_action("ser-1", "writer", "bold_text");
        let json = serde_json::to_string(&action).expect("should serialize");
        let deserialized: UndoAction =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.id, "ser-1");
        assert_eq!(deserialized.module, "writer");
        assert_eq!(deserialized.action_type, "bold_text");
        assert_eq!(deserialized.before, serde_json::json!({"value": "old"}));
        assert_eq!(deserialized.after, serde_json::json!({"value": "new"}));
    }
}
