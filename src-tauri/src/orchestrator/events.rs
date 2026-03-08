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
use std::collections::VecDeque;
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

/// In-memory event bus with ring buffer
pub struct EventBus {
    events: Mutex<VecDeque<OrchestratorEvent>>,
    subscribers: Mutex<Vec<Box<dyn Fn(&OrchestratorEvent) + Send + Sync>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(MAX_EVENTS)),
            subscribers: Mutex::new(Vec::new()),
        }
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
}
