#![allow(dead_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
//! Cron + Interval + Event-driven scheduling for ImpForge Orchestrator.
//!
//! Replaces the old `interval_secs: u64` + `trigger: Option<String>` scheme
//! with a unified `Schedule` enum that supports:
//!
//! - **Interval**: Fixed duration (e.g., every 60 seconds)
//! - **Cron**: Standard cron expressions via `croner` (e.g., "0 */6 * * *")
//! - **EventDriven**: Triggered by event bus events (e.g., FileChanged)
//!
//! Scientific basis: APScheduler patterns adapted for Tokio async runtime.

use chrono::Utc;
use croner::Cron;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Duration;

/// Scheduling mode for orchestrator workers.
///
/// Each worker declares how it should be triggered:
/// - `Interval` for periodic tasks (system monitoring, health checks)
/// - `Cron` for time-specific tasks ("weekdays at 9am", "every 6 hours")
/// - `EventDriven` for reactive tasks (file changes, service failures)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Schedule {
    /// Run every N seconds (backward-compatible with interval_secs)
    Interval(u64),

    /// Cron expression parsed by `croner` (MIT, extended syntax).
    ///
    /// Standard 5-field: "minute hour day month weekday"
    /// Extended 6-field: "second minute hour day month weekday"
    ///
    /// Examples:
    /// - `"*/5 * * * *"` — every 5 minutes
    /// - `"0 */6 * * *"` — every 6 hours
    /// - `"0 9 * * 1-5"` — weekdays at 9:00 AM
    /// - `"0 0 1 * *"` — first day of each month at midnight
    Cron(String),

    /// Triggered by an event type name (not time-based).
    ///
    /// The scheduler skips these in the polling loop; they are
    /// dispatched by the EventBus trigger system instead.
    ///
    /// Trigger strings follow the pattern: `"event_type"` or
    /// `"event_type(filter)"`, e.g.:
    /// - `"file_changed"` — any file change
    /// - `"file_changed(.rs,.ts)"` — only Rust/TypeScript files
    /// - `"service_down"` — any service failure
    /// - `"commit_ready"` — staged commit ready for validation
    /// - `"tag_release"` — new git tag created
    EventDriven(String),
}

impl Schedule {
    /// Compute the duration until the next scheduled run.
    ///
    /// Returns `None` for `EventDriven` (triggered externally, not polled).
    /// Returns `Some(Duration)` for `Interval` and `Cron`.
    ///
    /// For `Cron`, uses `croner` to find the next occurrence from now.
    /// Falls back to 60 seconds if the cron expression is invalid.
    pub fn next_run(&self) -> Option<Duration> {
        match self {
            Schedule::Interval(secs) => {
                if *secs == 0 {
                    None
                } else {
                    Some(Duration::from_secs(*secs))
                }
            }
            Schedule::Cron(expr) => {
                let cron = match Cron::from_str(expr) {
                    Ok(c) => c,
                    Err(e) => {
                        log::warn!("Invalid cron expression '{}': {}", expr, e);
                        return Some(Duration::from_secs(60));
                    }
                };
                let now = Utc::now();
                match cron.find_next_occurrence(&now, false) {
                    Ok(next) => {
                        let delta = next - now;
                        let secs = delta.num_seconds().max(1) as u64;
                        Some(Duration::from_secs(secs))
                    }
                    Err(_) => Some(Duration::from_secs(60)),
                }
            }
            Schedule::EventDriven(_) => None,
        }
    }

    /// Check if this is an event-driven schedule.
    pub fn is_event_driven(&self) -> bool {
        matches!(self, Schedule::EventDriven(_))
    }

    /// Check if this is a time-based schedule (Interval or Cron).
    pub fn is_time_based(&self) -> bool {
        !self.is_event_driven()
    }

    /// Get the event trigger string (if EventDriven).
    pub fn trigger(&self) -> Option<&str> {
        match self {
            Schedule::EventDriven(t) => Some(t),
            _ => None,
        }
    }

    /// Create from the legacy `interval_secs` + `trigger` format.
    ///
    /// Used for backward compatibility with existing `build_default_schedules()`.
    pub fn from_legacy(interval_secs: u64, trigger: Option<&str>) -> Self {
        match trigger {
            Some(t) if !t.is_empty() => Schedule::EventDriven(t.to_string()),
            _ if interval_secs > 0 => Schedule::Interval(interval_secs),
            _ => Schedule::Interval(0),
        }
    }
}

impl std::fmt::Display for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Schedule::Interval(s) => write!(f, "every {}s", s),
            Schedule::Cron(expr) => write!(f, "cron({})", expr),
            Schedule::EventDriven(t) => write!(f, "on:{}", t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_next_run() {
        let s = Schedule::Interval(60);
        let next = s.next_run().unwrap();
        assert_eq!(next, Duration::from_secs(60));
    }

    #[test]
    fn test_interval_zero_returns_none() {
        let s = Schedule::Interval(0);
        assert!(s.next_run().is_none());
    }

    #[test]
    fn test_event_driven_returns_none() {
        let s = Schedule::EventDriven("file_changed".into());
        assert!(s.next_run().is_none());
        assert!(s.is_event_driven());
        assert!(!s.is_time_based());
        assert_eq!(s.trigger(), Some("file_changed"));
    }

    #[test]
    fn test_cron_every_5_minutes() {
        let s = Schedule::Cron("*/5 * * * *".into());
        let next = s.next_run().unwrap();
        // Should be between 0 and 300 seconds (5 minutes)
        assert!(next.as_secs() <= 300);
        assert!(next.as_secs() >= 1);
    }

    #[test]
    fn test_cron_every_6_hours() {
        let s = Schedule::Cron("0 */6 * * *".into());
        let next = s.next_run().unwrap();
        // Should be between 1 second and 6 hours (21600 seconds)
        assert!(next.as_secs() <= 21600);
        assert!(next.as_secs() >= 1);
    }

    #[test]
    fn test_cron_invalid_expression_fallback() {
        let s = Schedule::Cron("not a cron".into());
        let next = s.next_run().unwrap();
        // Invalid cron falls back to 60 seconds
        assert_eq!(next, Duration::from_secs(60));
    }

    #[test]
    fn test_from_legacy_interval() {
        let s = Schedule::from_legacy(900, None);
        assert_eq!(s, Schedule::Interval(900));
    }

    #[test]
    fn test_from_legacy_trigger() {
        let s = Schedule::from_legacy(0, Some("file:changed(.rs)"));
        assert_eq!(s, Schedule::EventDriven("file:changed(.rs)".into()));
    }

    #[test]
    fn test_from_legacy_trigger_takes_precedence() {
        // When both interval and trigger are set, trigger wins
        let s = Schedule::from_legacy(3600, Some("service:down"));
        assert_eq!(s, Schedule::EventDriven("service:down".into()));
    }

    #[test]
    fn test_display() {
        assert_eq!(Schedule::Interval(60).to_string(), "every 60s");
        assert_eq!(Schedule::Cron("*/5 * * * *".into()).to_string(), "cron(*/5 * * * *)");
        assert_eq!(Schedule::EventDriven("file_changed".into()).to_string(), "on:file_changed");
    }

    #[test]
    fn test_serde_roundtrip() {
        let schedules = vec![
            Schedule::Interval(300),
            Schedule::Cron("0 */6 * * *".into()),
            Schedule::EventDriven("service_down".into()),
        ];
        for s in &schedules {
            let json = serde_json::to_string(s).unwrap();
            let deserialized: Schedule = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, deserialized);
        }
    }
}
