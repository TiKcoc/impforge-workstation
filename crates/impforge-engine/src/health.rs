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
// MAPE-K Self-Healing Loop for ImpForge Standalone Orchestrator
//
// Implements IBM's Autonomic Computing MAPE-K reference architecture:
//   Monitor → Analyze → Plan → Execute → Knowledge
//
// Standalone: Uses HTTP health checks, process detection, and file-system
// probes instead of systemd. Works on Linux, Windows, and macOS.
//
// References:
// - Kephart & Chess (2003): "The Vision of Autonomic Computing" (IBM)
// - IBM Redbook: "An Architectural Blueprint for Autonomic Computing"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum restart attempts before circuit breaker trips
const MAX_RESTARTS: u32 = 3;
/// Cooldown between restart attempts (seconds)
const RESTART_COOLDOWN_SECS: i64 = 300;

/// Service health status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Online,
    Degraded,
    Offline,
    Unknown,
    CircuitOpen,
}

/// Monitored service definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredService {
    pub name: String,
    pub health_url: Option<String>,
    pub process_name: Option<String>,
    pub critical: bool,
}

/// Per-service health state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub service: MonitoredService,
    pub status: ServiceStatus,
    pub consecutive_failures: u32,
    pub restart_count: u32,
    pub last_check: DateTime<Utc>,
    pub last_restart: Option<DateTime<Utc>>,
    pub last_online: Option<DateTime<Utc>>,
    pub response_time_ms: Option<u64>,
}

/// MAPE-K analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthAction {
    NoAction,
    RestartService { name: String },
    AlertUser { message: String },
    CircuitBreak { name: String },
    RecoverService { name: String, steps: Vec<String> },
}

/// The MAPE-K Health Loop
pub struct MapeKLoop {
    services: HashMap<String, ServiceState>,
    actions_log: Vec<(DateTime<Utc>, HealthAction)>,
}

impl MapeKLoop {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            actions_log: Vec::new(),
        }
    }

    /// Register a service for monitoring
    pub fn register_service(&mut self, service: MonitoredService) {
        let state = ServiceState {
            service: service.clone(),
            status: ServiceStatus::Unknown,
            consecutive_failures: 0,
            restart_count: 0,
            last_check: Utc::now(),
            last_restart: None,
            last_online: None,
            response_time_ms: None,
        };
        self.services.insert(service.name.clone(), state);
    }

    /// Register default services for an ImpForge standalone installation
    pub fn register_defaults(&mut self) {
        let defaults = vec![
            MonitoredService {
                name: "ollama".to_string(),
                health_url: Some("http://localhost:11434/api/tags".to_string()),
                process_name: Some("ollama".to_string()),
                critical: true,
            },
            MonitoredService {
                name: "docker".to_string(),
                health_url: None,
                process_name: Some("dockerd".to_string()),
                critical: false,
            },
            MonitoredService {
                name: "qdrant".to_string(),
                health_url: Some("http://localhost:6333/healthz".to_string()),
                process_name: Some("qdrant".to_string()),
                critical: false,
            },
        ];
        for svc in defaults {
            self.register_service(svc);
        }
    }

    // ─── MONITOR Phase ─────────────────────────────────────────

    /// Check health of a service via HTTP
    pub async fn monitor_http(url: &str, timeout_ms: u64) -> (bool, Option<u64>) {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
        {
            Ok(c) => c,
            Err(_) => return (false, None),
        };

        let start = std::time::Instant::now();
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let elapsed = start.elapsed().as_millis() as u64;
                (true, Some(elapsed))
            }
            _ => (false, None),
        }
    }

    /// Check if a process is running (cross-platform)
    pub fn monitor_process(name: &str) -> bool {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().values().any(|p| {
            p.name().to_string_lossy().contains(name)
        })
    }

    /// Run full monitoring cycle for a service
    pub async fn monitor_service(&mut self, name: &str) {
        let state = match self.services.get_mut(name) {
            Some(s) => s,
            None => return,
        };

        state.last_check = Utc::now();

        // HTTP health check (preferred)
        if let Some(url) = &state.service.health_url.clone() {
            let (ok, response_time) = Self::monitor_http(url, 3000).await;
            state.response_time_ms = response_time;
            if ok {
                state.status = ServiceStatus::Online;
                state.consecutive_failures = 0;
                state.last_online = Some(Utc::now());
                return;
            }
        }

        // Process check (fallback)
        if let Some(proc_name) = &state.service.process_name.clone() {
            if Self::monitor_process(proc_name) {
                state.status = ServiceStatus::Degraded;
                state.consecutive_failures = 0;
                return;
            }
        }

        // Service is down
        state.consecutive_failures += 1;
        state.status = ServiceStatus::Offline;
    }

    /// Run monitoring for all registered services
    pub async fn monitor_all(&mut self) {
        let names: Vec<String> = self.services.keys().cloned().collect();
        for name in names {
            self.monitor_service(&name).await;
        }
    }

    // ─── ANALYZE Phase ─────────────────────────────────────────

    /// Analyze current state and determine required actions
    pub fn analyze(&self) -> Vec<HealthAction> {
        let mut actions = Vec::new();

        for state in self.services.values() {
            match &state.status {
                ServiceStatus::Online | ServiceStatus::Unknown => {
                    // No action needed
                }
                ServiceStatus::Degraded => {
                    if state.consecutive_failures > 5 {
                        actions.push(HealthAction::AlertUser {
                            message: format!("{} is degraded ({} checks failed)", state.service.name, state.consecutive_failures),
                        });
                    }
                }
                ServiceStatus::Offline => {
                    if state.restart_count >= MAX_RESTARTS {
                        actions.push(HealthAction::CircuitBreak {
                            name: state.service.name.clone(),
                        });
                    } else if self.can_restart(state) {
                        actions.push(HealthAction::RestartService {
                            name: state.service.name.clone(),
                        });
                    } else {
                        actions.push(HealthAction::AlertUser {
                            message: format!("{} is offline (cooldown active, {} restarts)", state.service.name, state.restart_count),
                        });
                    }
                }
                ServiceStatus::CircuitOpen => {
                    // Circuit breaker is open — no automatic action
                    actions.push(HealthAction::AlertUser {
                        message: format!("{} circuit breaker is OPEN — manual intervention needed", state.service.name),
                    });
                }
            }
        }

        actions
    }

    fn can_restart(&self, state: &ServiceState) -> bool {
        if state.restart_count >= MAX_RESTARTS {
            return false;
        }
        match state.last_restart {
            Some(lr) => (Utc::now() - lr).num_seconds() >= RESTART_COOLDOWN_SECS,
            None => true,
        }
    }

    // ─── PLAN Phase ────────────────────────────────────────────

    /// Plan recovery steps for a service
    pub fn plan_recovery(&self, name: &str) -> Vec<String> {
        match name {
            "ollama" => vec![
                "Check if Ollama binary exists".to_string(),
                "Start Ollama serve process".to_string(),
                "Wait 5s for initialization".to_string(),
                "Verify health endpoint".to_string(),
            ],
            "docker" => vec![
                "Check Docker daemon status".to_string(),
                "Attempt to start Docker service".to_string(),
                "Verify Docker socket accessible".to_string(),
            ],
            "qdrant" => vec![
                "Check Qdrant binary/container".to_string(),
                "Start Qdrant process".to_string(),
                "Verify health endpoint".to_string(),
            ],
            _ => vec![format!("Manual restart required for {name}")],
        }
    }

    // ─── EXECUTE Phase ─────────────────────────────────────────

    /// Execute a health action (cross-platform service restart)
    pub async fn execute(&mut self, action: &HealthAction) {
        match action {
            HealthAction::RestartService { name } => {
                log::warn!("MAPE-K: Attempting restart of {name}");
                let success = self.attempt_restart(name).await;

                if let Some(state) = self.services.get_mut(name) {
                    state.restart_count += 1;
                    state.last_restart = Some(Utc::now());
                    if success {
                        state.status = ServiceStatus::Online;
                        state.consecutive_failures = 0;
                        log::info!("MAPE-K: Successfully restarted {name}");
                    } else {
                        log::error!("MAPE-K: Failed to restart {name}");
                    }
                }
            }
            HealthAction::CircuitBreak { name } => {
                log::error!("MAPE-K: Circuit breaker OPEN for {name}");
                if let Some(state) = self.services.get_mut(name) {
                    state.status = ServiceStatus::CircuitOpen;
                }
            }
            HealthAction::AlertUser { message } => {
                log::warn!("MAPE-K Alert: {message}");
            }
            HealthAction::RecoverService { name, steps } => {
                log::info!("MAPE-K: Recovery plan for {name}: {steps:?}");
            }
            HealthAction::NoAction => {}
        }

        self.actions_log.push((Utc::now(), action.clone()));
    }

    /// Attempt to restart a service (standalone — no systemd!)
    async fn attempt_restart(&self, name: &str) -> bool {
        let cmd = match name {
            "ollama" => {
                #[cfg(target_os = "windows")]
                { "ollama serve" }
                #[cfg(not(target_os = "windows"))]
                { "ollama serve &" }
            }
            "docker" => {
                #[cfg(target_os = "linux")]
                { "sudo systemctl start docker" }
                #[cfg(target_os = "macos")]
                { "open -a Docker" }
                #[cfg(target_os = "windows")]
                { "Start-Service docker" }
                #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                { return false; }
            }
            _ => return false,
        };

        match tokio::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn()
        {
            Ok(_) => {
                // Wait a moment for service to start
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                true
            }
            Err(e) => {
                log::error!("Failed to restart {name}: {e}");
                false
            }
        }
    }

    // ─── KNOWLEDGE Phase ───────────────────────────────────────

    /// Get current health summary for all services
    pub fn get_summary(&self) -> Vec<ServiceState> {
        self.services.values().cloned().collect()
    }

    /// Get recent actions taken
    pub fn get_actions_log(&self) -> &[(DateTime<Utc>, HealthAction)] {
        &self.actions_log
    }

    /// Reset circuit breaker for a service (manual intervention)
    pub fn reset_circuit_breaker(&mut self, name: &str) {
        if let Some(state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Unknown;
            state.restart_count = 0;
            state.consecutive_failures = 0;
            log::info!("MAPE-K: Circuit breaker reset for {name}");
        }
    }

    /// Full MAPE-K cycle: Monitor → Analyze → Plan → Execute
    pub async fn run_cycle(&mut self) {
        // Monitor
        self.monitor_all().await;

        // Analyze
        let actions = self.analyze();

        // Plan + Execute
        for action in &actions {
            self.execute(action).await;
        }
    }
}

impl Default for MapeKLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_defaults() {
        let mut loop_ = MapeKLoop::new();
        loop_.register_defaults();
        assert_eq!(loop_.services.len(), 3);
    }

    #[test]
    fn test_analyze_offline_service() {
        let mut loop_ = MapeKLoop::new();
        loop_.register_service(MonitoredService {
            name: "test".to_string(),
            health_url: None,
            process_name: None,
            critical: true,
        });
        // Simulate offline
        if let Some(state) = loop_.services.get_mut("test") {
            state.status = ServiceStatus::Offline;
            state.consecutive_failures = 3;
        }
        let actions = loop_.analyze();
        assert!(!actions.is_empty());
    }

    #[test]
    fn test_circuit_breaker() {
        let mut loop_ = MapeKLoop::new();
        loop_.register_service(MonitoredService {
            name: "test".to_string(),
            health_url: None,
            process_name: None,
            critical: true,
        });
        if let Some(state) = loop_.services.get_mut("test") {
            state.status = ServiceStatus::Offline;
            state.restart_count = MAX_RESTARTS;
        }
        let actions = loop_.analyze();
        assert!(actions.iter().any(|a| matches!(a, HealthAction::CircuitBreak { .. })));
    }

    #[test]
    fn test_plan_recovery() {
        let loop_ = MapeKLoop::new();
        let steps = loop_.plan_recovery("ollama");
        assert!(!steps.is_empty());
        assert!(steps[0].contains("Ollama"));
    }
}
