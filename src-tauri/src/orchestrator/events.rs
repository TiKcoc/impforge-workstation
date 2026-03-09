// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Event System for ImpForge Standalone Orchestrator
//!
//! Four event types for the ImpForge standalone orchestrator:
//! - FileChanged — a watched file was modified
//! - ServiceDown — a monitored service went offline
//! - TaskCompleted — a worker finished execution
//! - TerminalOutput — terminal/IDE produced output worth digesting
//!
//! Events are stored in SQLite (not Redis Streams) for standalone operation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use parking_lot::Mutex;

/// Maximum events in the in-memory ring buffer
const MAX_EVENTS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    FileChanged,
    ServiceDown,
    TaskCompleted,
    TerminalOutput,
    HealthCheck,
    TrustUpdate,
    MemoryReview,
    OrchestratorAction,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::FileChanged => write!(f, "file_changed"),
            EventType::ServiceDown => write!(f, "service_down"),
            EventType::TaskCompleted => write!(f, "task_completed"),
            EventType::TerminalOutput => write!(f, "terminal_output"),
            EventType::HealthCheck => write!(f, "health_check"),
            EventType::TrustUpdate => write!(f, "trust_update"),
            EventType::MemoryReview => write!(f, "memory_review"),
            EventType::OrchestratorAction => write!(f, "orchestrator_action"),
        }
    }
}

/// A single event in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorEvent {
    pub event_type: EventType,
    pub source: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl OrchestratorEvent {
    pub fn file_changed(path: &str, repo: &str) -> Self {
        Self {
            event_type: EventType::FileChanged,
            source: "file_watcher".to_string(),
            payload: serde_json::json!({
                "file_path": path,
                "repo": repo,
            }),
            timestamp: Utc::now(),
        }
    }

    pub fn service_down(service_name: &str) -> Self {
        Self {
            event_type: EventType::ServiceDown,
            source: "health_loop".to_string(),
            payload: serde_json::json!({
                "service_name": service_name,
            }),
            timestamp: Utc::now(),
        }
    }

    pub fn task_completed(worker: &str, success: bool, duration_ms: u64) -> Self {
        Self {
            event_type: EventType::TaskCompleted,
            source: "orchestrator".to_string(),
            payload: serde_json::json!({
                "task_name": worker,
                "success": success,
                "duration_ms": duration_ms,
            }),
            timestamp: Utc::now(),
        }
    }

    pub fn terminal_output(source: &str, content: &str) -> Self {
        Self {
            event_type: EventType::TerminalOutput,
            source: source.to_string(),
            payload: serde_json::json!({
                "content": content,
            }),
            timestamp: Utc::now(),
        }
    }
}

/// Parsed trigger filter from `Schedule::EventDriven` strings.
///
/// Trigger strings follow the pattern: `"event_type"` or `"event_type(filter)"`.
/// Examples:
/// - `"file:changed"` → event_type=FileChanged, filter=None
/// - `"file:changed(.rs,.ts)"` → event_type=FileChanged, filter=Some([".rs",".ts"])
/// - `"service:down"` → event_type=ServiceDown, filter=None
/// - `"tag:release"` → event_type=OrchestratorAction, filter=Some(["release"])
/// - `"commit:ready"` → event_type=OrchestratorAction, filter=Some(["ready"])
#[derive(Debug, Clone)]
pub struct TriggerFilter {
    pub extensions: Vec<String>,
}

/// Parse a trigger string into (EventType, optional filter).
///
/// Returns None if the trigger string doesn't map to a known event type.
pub fn parse_trigger(trigger: &str) -> Option<(EventType, Option<TriggerFilter>)> {
    // Split off filter: "file:changed(.rs,.ts)" → ("file:changed", ".rs,.ts")
    let (base, filter) = if let Some(paren_start) = trigger.find('(') {
        let base = &trigger[..paren_start];
        let filter_str = trigger[paren_start + 1..].trim_end_matches(')');
        let exts: Vec<String> = filter_str.split(',').map(|s| s.trim().to_string()).collect();
        (base, Some(TriggerFilter { extensions: exts }))
    } else {
        (trigger, None)
    };

    let event_type = match base {
        "file:changed" | "file_changed" => EventType::FileChanged,
        "service:down" | "service_down" => EventType::ServiceDown,
        "tag:release" | "tag_release" => EventType::OrchestratorAction,
        "commit:ready" | "commit_ready" => EventType::OrchestratorAction,
        "health:check" | "health_check" => EventType::HealthCheck,
        "trust:update" | "trust_update" => EventType::TrustUpdate,
        "memory:review" | "memory_review" => EventType::MemoryReview,
        _ => return None,
    };

    Some((event_type, filter))
}

/// In-memory event bus with ring buffer and trigger registry
pub struct EventBus {
    events: Mutex<VecDeque<OrchestratorEvent>>,
    subscribers: Mutex<Vec<Box<dyn Fn(&OrchestratorEvent) + Send + Sync>>>,
    /// Maps EventType → list of (worker_name, optional_filter) pairs.
    /// Built from TaskSchedule entries with Schedule::EventDriven triggers.
    triggers: Mutex<HashMap<String, Vec<(String, Option<TriggerFilter>)>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(MAX_EVENTS)),
            subscribers: Mutex::new(Vec::new()),
            triggers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a worker to be triggered by a specific event type.
    ///
    /// The `trigger_str` is parsed from `Schedule::EventDriven` values
    /// (e.g., "file:changed(.rs,.ts)"). Workers are looked up when
    /// `get_triggered_workers()` is called after an event fires.
    pub fn register_trigger(&self, trigger_str: &str, worker_name: &str) {
        if let Some((event_type, filter)) = parse_trigger(trigger_str) {
            let key = event_type.to_string();
            let mut triggers = self.triggers.lock();
            triggers
                .entry(key)
                .or_default()
                .push((worker_name.to_string(), filter));
        } else {
            log::warn!("Unknown trigger string '{}' for worker '{}'", trigger_str, worker_name);
        }
    }

    /// Get worker names that should run in response to an event.
    ///
    /// For FileChanged events, optionally filters by file extension.
    /// Returns all matching workers sorted by name for deterministic ordering.
    pub fn get_triggered_workers(&self, event: &OrchestratorEvent) -> Vec<String> {
        let key = event.event_type.to_string();
        let triggers = self.triggers.lock();

        let Some(workers) = triggers.get(&key) else {
            return Vec::new();
        };

        let file_path = event.payload.get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut result: Vec<String> = workers
            .iter()
            .filter(|(_, filter)| {
                match filter {
                    None => true,
                    Some(f) if f.extensions.is_empty() => true,
                    Some(f) => {
                        // Check if the file path ends with any of the filter extensions
                        f.extensions.iter().any(|ext| file_path.ends_with(ext))
                    }
                }
            })
            .map(|(name, _)| name.clone())
            .collect();

        result.sort();
        result.dedup();
        result
    }

    /// Publish an event to the bus
    pub fn publish(&self, event: OrchestratorEvent) {
        // Notify subscribers
        {
            let subs = self.subscribers.lock();
            for sub in subs.iter() {
                sub(&event);
            }
        }

        // Store in ring buffer
        let mut events = self.events.lock();
        if events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Subscribe to all events
    pub fn subscribe(&self, callback: Box<dyn Fn(&OrchestratorEvent) + Send + Sync>) {
        let mut subs = self.subscribers.lock();
        subs.push(callback);
    }

    /// Get recent events (newest first)
    pub fn recent(&self, limit: usize) -> Vec<OrchestratorEvent> {
        let events = self.events.lock();
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get events by type
    pub fn by_type(&self, event_type: &EventType, limit: usize) -> Vec<OrchestratorEvent> {
        let events = self.events.lock();
        events
            .iter()
            .rev()
            .filter(|e| &e.event_type == event_type)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Count events of a given type in the last N seconds
    pub fn count_recent(&self, event_type: &EventType, seconds: i64) -> usize {
        let cutoff = Utc::now() - chrono::Duration::seconds(seconds);
        let events = self.events.lock();
        events
            .iter()
            .filter(|e| &e.event_type == event_type && e.timestamp > cutoff)
            .count()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.lock().clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_recent() {
        let bus = EventBus::new();
        bus.publish(OrchestratorEvent::task_completed("w1", true, 100));
        bus.publish(OrchestratorEvent::task_completed("w2", false, 200));
        let recent = bus.recent(10);
        assert_eq!(recent.len(), 2);
        // Newest first
        assert_eq!(recent[0].payload["task_name"], "w2");
    }

    #[test]
    fn test_by_type() {
        let bus = EventBus::new();
        bus.publish(OrchestratorEvent::task_completed("w1", true, 100));
        bus.publish(OrchestratorEvent::service_down("ollama"));
        bus.publish(OrchestratorEvent::task_completed("w2", true, 50));
        let tasks = bus.by_type(&EventType::TaskCompleted, 10);
        assert_eq!(tasks.len(), 2);
        let services = bus.by_type(&EventType::ServiceDown, 10);
        assert_eq!(services.len(), 1);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let bus = EventBus::new();
        for i in 0..1100 {
            bus.publish(OrchestratorEvent::task_completed(&format!("w{i}"), true, 10));
        }
        let recent = bus.recent(2000);
        assert_eq!(recent.len(), MAX_EVENTS);
    }

    // ─── Trigger System Tests ─────────────────────────────────

    #[test]
    fn test_parse_trigger_file_changed() {
        let (et, filter) = parse_trigger("file:changed").unwrap();
        assert_eq!(et, EventType::FileChanged);
        assert!(filter.is_none());
    }

    #[test]
    fn test_parse_trigger_with_filter() {
        let (et, filter) = parse_trigger("file:changed(.rs,.ts)").unwrap();
        assert_eq!(et, EventType::FileChanged);
        let f = filter.unwrap();
        assert_eq!(f.extensions, vec![".rs", ".ts"]);
    }

    #[test]
    fn test_parse_trigger_service_down() {
        let (et, _) = parse_trigger("service:down").unwrap();
        assert_eq!(et, EventType::ServiceDown);
    }

    #[test]
    fn test_parse_trigger_unknown() {
        assert!(parse_trigger("unknown:event").is_none());
    }

    #[test]
    fn test_register_and_get_triggered_workers() {
        let bus = EventBus::new();
        bus.register_trigger("file:changed(.rs,.ts)", "test_runner");
        bus.register_trigger("file:changed(.md)", "doc_sync");
        bus.register_trigger("file:changed", "semantic_diff");
        bus.register_trigger("service:down", "self_healer");

        // .rs file should trigger test_runner + semantic_diff (no filter = all files)
        let event = OrchestratorEvent::file_changed("src/main.rs", "impforge");
        let workers = bus.get_triggered_workers(&event);
        assert!(workers.contains(&"test_runner".to_string()));
        assert!(workers.contains(&"semantic_diff".to_string()));
        assert!(!workers.contains(&"doc_sync".to_string())); // .md only

        // .md file should trigger doc_sync + semantic_diff
        let md_event = OrchestratorEvent::file_changed("README.md", "impforge");
        let md_workers = bus.get_triggered_workers(&md_event);
        assert!(md_workers.contains(&"doc_sync".to_string()));
        assert!(md_workers.contains(&"semantic_diff".to_string()));
        assert!(!md_workers.contains(&"test_runner".to_string()));

        // ServiceDown should trigger self_healer
        let svc_event = OrchestratorEvent::service_down("ollama");
        let svc_workers = bus.get_triggered_workers(&svc_event);
        assert_eq!(svc_workers, vec!["self_healer"]);
    }

    #[test]
    fn test_triggered_workers_empty_for_unregistered() {
        let bus = EventBus::new();
        let event = OrchestratorEvent::file_changed("test.py", "proj");
        let workers = bus.get_triggered_workers(&event);
        assert!(workers.is_empty());
    }

    #[test]
    fn test_trigger_deduplication() {
        let bus = EventBus::new();
        bus.register_trigger("file:changed", "worker_a");
        bus.register_trigger("file:changed", "worker_a"); // duplicate
        let event = OrchestratorEvent::file_changed("test.rs", "proj");
        let workers = bus.get_triggered_workers(&event);
        // Should be deduplicated
        assert_eq!(workers.iter().filter(|w| *w == "worker_a").count(), 1);
    }
}
