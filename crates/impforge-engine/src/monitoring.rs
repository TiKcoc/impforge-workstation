// SPDX-License-Identifier: BUSL-1.1
//
// Copyright (c) 2026 AiImp Development
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://github.com/AiImp/ImpForge/blob/main/LICENSE-ENGINE
//
// Change Date: 2030-03-09
// Change License: Apache-2.0
//
// Advanced Monitoring Dashboard for ImpForge Pro/Enterprise
//
// Provides metric collection, alert rule evaluation, and dashboard
// summaries beyond the community-tier MAPE-K health loop.
//
// Metric types follow the Prometheus data model:
// - Counter: monotonically increasing cumulative metric
// - Gauge: single numeric value that can go up or down
// - Histogram: observations bucketed by configurable boundaries
// - Summary: streaming quantile estimates over a sliding window
//
// References:
// - Prometheus Data Model: https://prometheus.io/docs/concepts/data_model/
// - Google SRE Book, Ch. 6: "Monitoring Distributed Systems"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Metric Types ───────────────────────────────────────────

/// Classification of a recorded metric following the Prometheus data model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    /// Monotonically increasing cumulative value (e.g. total requests).
    Counter,
    /// Instantaneous value that can go up or down (e.g. CPU usage).
    Gauge,
    /// Observations placed into configurable buckets (e.g. latency distribution).
    Histogram,
    /// Streaming quantile estimates over a sliding window.
    Summary,
}

/// A single recorded metric observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

impl MetricPoint {
    /// Create a new metric point stamped at the current time.
    pub fn new(name: &str, metric_type: MetricType, value: f64) -> Self {
        Self {
            name: name.to_string(),
            metric_type,
            value,
            labels: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Builder helper: attach a label key-value pair.
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// Builder helper: set a specific timestamp (useful in tests).
    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = ts;
        self
    }
}

// ─── Alert Types ────────────────────────────────────────────

/// Severity classification for triggered alerts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// A rule that evaluates the latest value of a named metric against a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub metric_name: String,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub cooldown_secs: u64,
}

impl AlertRule {
    pub fn new(
        name: &str,
        metric_name: &str,
        threshold: f64,
        severity: AlertSeverity,
        cooldown_secs: u64,
    ) -> Self {
        Self {
            name: name.to_string(),
            metric_name: metric_name.to_string(),
            threshold,
            severity,
            cooldown_secs,
        }
    }
}

/// A triggered alert produced when a metric exceeds its rule threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub metric_value: f64,
}

// ─── Dashboard Summary ──────────────────────────────────────

/// Aggregate statistics exposed by the monitoring dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSummary {
    pub total_metrics: usize,
    pub active_alerts: usize,
    pub uptime_pct: f64,
}

// ─── MonitoringDashboard ────────────────────────────────────

/// Advanced monitoring dashboard that collects metrics, evaluates alert
/// rules, and produces operational summaries.
pub struct MonitoringDashboard {
    /// All recorded metric points, keyed by metric name, ordered oldest-first.
    metrics: HashMap<String, Vec<MetricPoint>>,
    /// Alert rules to evaluate against latest metric values.
    rules: Vec<AlertRule>,
    /// Timestamps of the last alert firing per rule name (for cooldown).
    last_fired: HashMap<String, DateTime<Utc>>,
    /// Dashboard creation time (used for uptime calculation).
    created_at: DateTime<Utc>,
    /// Count of "up" observations (for uptime percentage).
    uptime_checks: u64,
    /// Count of "down" observations.
    downtime_checks: u64,
}

impl MonitoringDashboard {
    /// Create a new, empty monitoring dashboard.
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
            rules: Vec::new(),
            last_fired: HashMap::new(),
            created_at: Utc::now(),
            uptime_checks: 0,
            downtime_checks: 0,
        }
    }

    /// Record a metric observation. Points are stored in insertion order
    /// per metric name.
    pub fn record_metric(&mut self, point: MetricPoint) {
        // Track uptime from a well-known gauge named "system.up".
        if point.name == "system.up" {
            if point.value >= 1.0 {
                self.uptime_checks += 1;
            } else {
                self.downtime_checks += 1;
            }
        }

        self.metrics
            .entry(point.name.clone())
            .or_default()
            .push(point);
    }

    /// Register an alert rule to be evaluated during `check_alerts`.
    pub fn add_alert_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    /// Evaluate every registered alert rule against the most recent value
    /// of its target metric. Returns alerts for all rules whose metric
    /// exceeds the configured threshold and whose cooldown has elapsed.
    pub fn check_alerts(&self) -> Vec<Alert> {
        let now = Utc::now();
        let mut alerts = Vec::new();

        for rule in &self.rules {
            // Look up the latest metric point for this rule.
            let latest = match self.metrics.get(&rule.metric_name).and_then(|v| v.last()) {
                Some(p) => p,
                None => continue,
            };

            // Check threshold (fires when value >= threshold).
            if latest.value < rule.threshold {
                continue;
            }

            // Check cooldown.
            if let Some(last) = self.last_fired.get(&rule.name) {
                let elapsed = (now - *last).num_seconds().unsigned_abs();
                if elapsed < rule.cooldown_secs {
                    continue;
                }
            }

            alerts.push(Alert {
                rule_name: rule.name.clone(),
                severity: rule.severity.clone(),
                message: format!(
                    "{}: {} = {:.2} (threshold {:.2})",
                    rule.name, rule.metric_name, latest.value, rule.threshold
                ),
                triggered_at: now,
                metric_value: latest.value,
            });
        }

        alerts
    }

    /// Acknowledge alerts by updating cooldown timestamps. Call this after
    /// processing the result of `check_alerts` to prevent immediate re-fire.
    pub fn acknowledge_alerts(&mut self, alerts: &[Alert]) {
        for alert in alerts {
            self.last_fired
                .insert(alert.rule_name.clone(), alert.triggered_at);
        }
    }

    /// Retrieve the most recent `limit` metric points for a named metric,
    /// ordered oldest-first.
    pub fn get_metric_history(&self, name: &str, limit: usize) -> Vec<&MetricPoint> {
        match self.metrics.get(name) {
            Some(points) => {
                let start = points.len().saturating_sub(limit);
                points[start..].iter().collect()
            }
            None => Vec::new(),
        }
    }

    /// Produce an aggregate summary of the dashboard state.
    pub fn summary(&self) -> MonitoringSummary {
        let total_metrics: usize = self.metrics.values().map(|v| v.len()).sum();
        let active_alerts = self.check_alerts().len();

        let total_checks = self.uptime_checks + self.downtime_checks;
        let uptime_pct = if total_checks > 0 {
            (self.uptime_checks as f64 / total_checks as f64) * 100.0
        } else {
            100.0 // No checks yet — assume healthy
        };

        MonitoringSummary {
            total_metrics,
            active_alerts,
            uptime_pct,
        }
    }

    /// Return the dashboard creation time.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Return the number of registered alert rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Return the set of distinct metric names currently tracked.
    pub fn metric_names(&self) -> Vec<String> {
        self.metrics.keys().cloned().collect()
    }
}

impl Default for MonitoringDashboard {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a simple gauge metric point.
    fn gauge(name: &str, value: f64) -> MetricPoint {
        MetricPoint::new(name, MetricType::Gauge, value)
    }

    // Helper: create a counter metric point.
    fn counter(name: &str, value: f64) -> MetricPoint {
        MetricPoint::new(name, MetricType::Counter, value)
    }

    // ── 1. Metric Recording ────────────────────────────────

    #[test]
    fn test_record_metric_stores_point() {
        let mut dash = MonitoringDashboard::new();
        dash.record_metric(gauge("cpu.usage", 42.0));
        dash.record_metric(gauge("cpu.usage", 55.0));
        dash.record_metric(gauge("mem.free", 1024.0));

        let cpu = dash.get_metric_history("cpu.usage", 10);
        assert_eq!(cpu.len(), 2);
        assert!((cpu[0].value - 42.0).abs() < f64::EPSILON);
        assert!((cpu[1].value - 55.0).abs() < f64::EPSILON);

        let mem = dash.get_metric_history("mem.free", 10);
        assert_eq!(mem.len(), 1);
    }

    #[test]
    fn test_record_metric_preserves_labels() {
        let mut dash = MonitoringDashboard::new();
        let point = gauge("http.requests", 100.0)
            .with_label("method", "GET")
            .with_label("path", "/api/v1");
        dash.record_metric(point);

        let history = dash.get_metric_history("http.requests", 1);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].labels.get("method").unwrap(), "GET");
        assert_eq!(history[0].labels.get("path").unwrap(), "/api/v1");
    }

    // ── 2. Alert Rule Registration ─────────────────────────

    #[test]
    fn test_add_alert_rule() {
        let mut dash = MonitoringDashboard::new();
        assert_eq!(dash.rule_count(), 0);

        dash.add_alert_rule(AlertRule::new(
            "high_cpu",
            "cpu.usage",
            90.0,
            AlertSeverity::Warning,
            60,
        ));
        assert_eq!(dash.rule_count(), 1);

        dash.add_alert_rule(AlertRule::new(
            "oom",
            "mem.free",
            50.0,
            AlertSeverity::Critical,
            120,
        ));
        assert_eq!(dash.rule_count(), 2);
    }

    // ── 3. Alert Triggering ────────────────────────────────

    #[test]
    fn test_alert_triggers_when_threshold_exceeded() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "high_cpu",
            "cpu.usage",
            90.0,
            AlertSeverity::Critical,
            0, // No cooldown for test
        ));

        // Below threshold — no alert.
        dash.record_metric(gauge("cpu.usage", 50.0));
        let alerts = dash.check_alerts();
        assert!(alerts.is_empty());

        // At threshold — fires.
        dash.record_metric(gauge("cpu.usage", 90.0));
        let alerts = dash.check_alerts();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_name, "high_cpu");
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert!((alerts[0].metric_value - 90.0).abs() < f64::EPSILON);

        // Above threshold — still fires.
        dash.record_metric(gauge("cpu.usage", 99.5));
        let alerts = dash.check_alerts();
        assert_eq!(alerts.len(), 1);
        assert!((alerts[0].metric_value - 99.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_alert_does_not_fire_when_below_threshold() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "high_latency",
            "http.latency_ms",
            500.0,
            AlertSeverity::Warning,
            0,
        ));

        dash.record_metric(gauge("http.latency_ms", 120.0));
        dash.record_metric(gauge("http.latency_ms", 499.9));
        let alerts = dash.check_alerts();
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_alert_no_metric_recorded_no_fire() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "ghost_rule",
            "nonexistent.metric",
            1.0,
            AlertSeverity::Info,
            0,
        ));

        let alerts = dash.check_alerts();
        assert!(alerts.is_empty(), "Rule should not fire for missing metric");
    }

    // ── 4. History Retrieval ───────────────────────────────

    #[test]
    fn test_get_metric_history_limit() {
        let mut dash = MonitoringDashboard::new();
        for i in 0..20 {
            dash.record_metric(counter("requests.total", i as f64));
        }

        // Request only last 5
        let history = dash.get_metric_history("requests.total", 5);
        assert_eq!(history.len(), 5);
        // Should be the last 5 values: 15, 16, 17, 18, 19
        assert!((history[0].value - 15.0).abs() < f64::EPSILON);
        assert!((history[4].value - 19.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_metric_history_empty() {
        let dash = MonitoringDashboard::new();
        let history = dash.get_metric_history("does.not.exist", 10);
        assert!(history.is_empty());
    }

    // ── 5. Summary Calculation ─────────────────────────────

    #[test]
    fn test_summary_total_metrics() {
        let mut dash = MonitoringDashboard::new();
        dash.record_metric(gauge("cpu.usage", 10.0));
        dash.record_metric(gauge("cpu.usage", 20.0));
        dash.record_metric(gauge("mem.free", 512.0));

        let summary = dash.summary();
        assert_eq!(summary.total_metrics, 3);
    }

    #[test]
    fn test_summary_uptime_percentage() {
        let mut dash = MonitoringDashboard::new();

        // 3 up, 1 down = 75% uptime
        dash.record_metric(gauge("system.up", 1.0));
        dash.record_metric(gauge("system.up", 1.0));
        dash.record_metric(gauge("system.up", 1.0));
        dash.record_metric(gauge("system.up", 0.0));

        let summary = dash.summary();
        assert!(
            (summary.uptime_pct - 75.0).abs() < f64::EPSILON,
            "Expected 75% uptime, got {:.1}%",
            summary.uptime_pct,
        );
    }

    #[test]
    fn test_summary_no_checks_defaults_100() {
        let dash = MonitoringDashboard::new();
        let summary = dash.summary();
        assert!((summary.uptime_pct - 100.0).abs() < f64::EPSILON);
        assert_eq!(summary.total_metrics, 0);
        assert_eq!(summary.active_alerts, 0);
    }

    #[test]
    fn test_summary_active_alerts_count() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "r1",
            "m1",
            50.0,
            AlertSeverity::Warning,
            0,
        ));
        dash.add_alert_rule(AlertRule::new(
            "r2",
            "m2",
            80.0,
            AlertSeverity::Critical,
            0,
        ));

        dash.record_metric(gauge("m1", 60.0)); // exceeds 50
        dash.record_metric(gauge("m2", 70.0)); // below 80

        let summary = dash.summary();
        assert_eq!(summary.active_alerts, 1);
    }

    // ── 6. Cooldown Prevents Re-fire ───────────────────────

    #[test]
    fn test_alert_cooldown_prevents_refire() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "high_cpu",
            "cpu.usage",
            90.0,
            AlertSeverity::Warning,
            3600, // 1 hour cooldown
        ));
        dash.record_metric(gauge("cpu.usage", 95.0));

        // First check fires.
        let alerts = dash.check_alerts();
        assert_eq!(alerts.len(), 1);

        // Acknowledge to set cooldown.
        dash.acknowledge_alerts(&alerts);

        // Second check within cooldown — suppressed.
        let alerts = dash.check_alerts();
        assert!(alerts.is_empty(), "Alert should be suppressed during cooldown");
    }

    // ── 7. Multiple Alert Rules on Different Metrics ───────

    #[test]
    fn test_multiple_rules_independent() {
        let mut dash = MonitoringDashboard::new();
        dash.add_alert_rule(AlertRule::new(
            "cpu_warn",
            "cpu",
            80.0,
            AlertSeverity::Warning,
            0,
        ));
        dash.add_alert_rule(AlertRule::new(
            "mem_crit",
            "mem",
            95.0,
            AlertSeverity::Critical,
            0,
        ));
        dash.add_alert_rule(AlertRule::new(
            "disk_info",
            "disk",
            70.0,
            AlertSeverity::Info,
            0,
        ));

        dash.record_metric(gauge("cpu", 85.0)); // fires cpu_warn
        dash.record_metric(gauge("mem", 50.0)); // does NOT fire mem_crit
        dash.record_metric(gauge("disk", 72.0)); // fires disk_info

        let alerts = dash.check_alerts();
        assert_eq!(alerts.len(), 2);

        let names: Vec<&str> = alerts.iter().map(|a| a.rule_name.as_str()).collect();
        assert!(names.contains(&"cpu_warn"));
        assert!(names.contains(&"disk_info"));
        assert!(!names.contains(&"mem_crit"));
    }

    // ── 8. MetricType Variants ─────────────────────────────

    #[test]
    fn test_all_metric_types_recordable() {
        let mut dash = MonitoringDashboard::new();
        dash.record_metric(MetricPoint::new("c", MetricType::Counter, 1.0));
        dash.record_metric(MetricPoint::new("g", MetricType::Gauge, 2.0));
        dash.record_metric(MetricPoint::new("h", MetricType::Histogram, 3.0));
        dash.record_metric(MetricPoint::new("s", MetricType::Summary, 4.0));

        assert_eq!(dash.get_metric_history("c", 10).len(), 1);
        assert_eq!(dash.get_metric_history("g", 10).len(), 1);
        assert_eq!(dash.get_metric_history("h", 10).len(), 1);
        assert_eq!(dash.get_metric_history("s", 10).len(), 1);

        // Verify types round-trip correctly.
        assert_eq!(
            dash.get_metric_history("c", 1)[0].metric_type,
            MetricType::Counter
        );
        assert_eq!(
            dash.get_metric_history("h", 1)[0].metric_type,
            MetricType::Histogram
        );
    }
}
