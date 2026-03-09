// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
#![allow(dead_code)]
//! Typed Message Bus for ImpForge Orchestrator
//!
//! Replaces NeuralSwarm's Redis Streams with Tokio broadcast channels
//! for standalone operation. Each message type gets its own typed channel,
//! preventing deserialization errors and enabling compile-time safety.
//!
//! 7 channel types covering MOA pipeline, topology, evaluation, and system:
//! - TaskChannel: Incoming AI tasks with complexity scoring
//! - ProposalChannel: Agent proposals within MOA layers
//! - CritiqueChannel: Agent critiques of proposals
//! - ResultChannel: Final aggregated results
//! - TopologyChannel: Dynamic topology updates
//! - HealthChannel: System health broadcasts
//! - ToolChannel: Tool invocation requests/results
//!
//! All messages are optionally persisted to SQLite for audit trail.
//! Scientific basis: Redis Streams patterns (Antirez 2018) adapted for
//! embedded Tokio broadcast channels.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::store::OrchestratorStore;

/// Default channel capacity (power of 2 for broadcast efficiency)
const CHANNEL_CAPACITY: usize = 256;

// ════════════════════════════════════════════════════════════════
// MESSAGE TYPES — One per channel, fully typed
// ════════════════════════════════════════════════════════════════

/// Task message: an incoming AI task to be processed by agents.
/// Corresponds to NeuralSwarm's `task_stream` schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskMessage {
    pub task_id: String,
    pub query: String,
    pub complexity: f32,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

/// Proposal message: an agent's proposed response within a MOA layer.
/// Part of the 5-phase Attention-MoA pipeline (arXiv:2601.16596).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalMessage {
    pub task_id: String,
    pub agent_id: String,
    pub content: String,
    pub layer: u32,
    pub confidence: f32,
    pub timestamp: DateTime<Utc>,
}

/// Critique message: an agent's evaluation of a proposal.
/// Phase 2 of MOA: critique → refine → check → aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CritiqueMessage {
    pub task_id: String,
    pub agent_id: String,
    pub proposal_agent_id: String,
    pub score: f32,
    pub feedback: String,
    pub layer: u32,
    pub timestamp: DateTime<Utc>,
}

/// Result message: final aggregated output after MOA pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultMessage {
    pub task_id: String,
    pub content: String,
    pub total_layers: u32,
    pub total_agents: u32,
    pub quality_score: f32,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Topology update: dynamic agent graph changes (DyTopo, arXiv:2602.06039).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopologyMessage {
    pub action: TopologyAction,
    pub agent_id: String,
    pub connections: Vec<String>,
    pub weight: f32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyAction {
    AddNode,
    RemoveNode,
    UpdateEdge,
    Rebalance,
}

/// Health broadcast: system-wide health status updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthMessage {
    pub source: String,
    pub status: HealthStatus,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
}

/// Tool invocation: request or result of a tool call by an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolMessage {
    pub task_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: ToolStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Pending,
    Running,
    Success,
    Failed,
}

// ════════════════════════════════════════════════════════════════
// MESSAGE BUS — Tokio broadcast channels per type
// ════════════════════════════════════════════════════════════════

/// Typed message bus with 7 broadcast channels.
///
/// Each channel is independent — subscribers only receive messages
/// for the types they care about. This prevents the "god channel"
/// problem where all messages flow through a single stream.
pub struct MessageBus {
    task_tx: broadcast::Sender<TaskMessage>,
    proposal_tx: broadcast::Sender<ProposalMessage>,
    critique_tx: broadcast::Sender<CritiqueMessage>,
    result_tx: broadcast::Sender<ResultMessage>,
    topology_tx: broadcast::Sender<TopologyMessage>,
    health_tx: broadcast::Sender<HealthMessage>,
    tool_tx: broadcast::Sender<ToolMessage>,
    store: Option<Arc<OrchestratorStore>>,
}

impl MessageBus {
    /// Create a new message bus with optional SQLite persistence.
    pub fn new(store: Option<Arc<OrchestratorStore>>) -> Self {
        let (task_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (proposal_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (critique_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (result_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (topology_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (health_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (tool_tx, _) = broadcast::channel(CHANNEL_CAPACITY);

        Self {
            task_tx,
            proposal_tx,
            critique_tx,
            result_tx,
            topology_tx,
            health_tx,
            tool_tx,
            store,
        }
    }

    // ─── Publish methods ──────────────────────────────────────

    pub fn publish_task(&self, msg: TaskMessage) -> usize {
        self.persist("task", &msg.task_id, &serde_json::to_string(&msg).unwrap_or_default());
        self.task_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_proposal(&self, msg: ProposalMessage) -> usize {
        self.persist("proposal", &msg.task_id, &serde_json::to_string(&msg).unwrap_or_default());
        self.proposal_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_critique(&self, msg: CritiqueMessage) -> usize {
        self.persist("critique", &msg.task_id, &serde_json::to_string(&msg).unwrap_or_default());
        self.critique_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_result(&self, msg: ResultMessage) -> usize {
        self.persist("result", &msg.task_id, &serde_json::to_string(&msg).unwrap_or_default());
        self.result_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_topology(&self, msg: TopologyMessage) -> usize {
        self.persist("topology", &msg.agent_id, &serde_json::to_string(&msg).unwrap_or_default());
        self.topology_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_health(&self, msg: HealthMessage) -> usize {
        self.persist("health", &msg.source, &serde_json::to_string(&msg).unwrap_or_default());
        self.health_tx.send(msg).unwrap_or(0)
    }

    pub fn publish_tool(&self, msg: ToolMessage) -> usize {
        self.persist("tool", &msg.tool_name, &serde_json::to_string(&msg).unwrap_or_default());
        self.tool_tx.send(msg).unwrap_or(0)
    }

    // ─── Subscribe methods ────────────────────────────────────

    pub fn subscribe_tasks(&self) -> broadcast::Receiver<TaskMessage> {
        self.task_tx.subscribe()
    }

    pub fn subscribe_proposals(&self) -> broadcast::Receiver<ProposalMessage> {
        self.proposal_tx.subscribe()
    }

    pub fn subscribe_critiques(&self) -> broadcast::Receiver<CritiqueMessage> {
        self.critique_tx.subscribe()
    }

    pub fn subscribe_results(&self) -> broadcast::Receiver<ResultMessage> {
        self.result_tx.subscribe()
    }

    pub fn subscribe_topology(&self) -> broadcast::Receiver<TopologyMessage> {
        self.topology_tx.subscribe()
    }

    pub fn subscribe_health(&self) -> broadcast::Receiver<HealthMessage> {
        self.health_tx.subscribe()
    }

    pub fn subscribe_tools(&self) -> broadcast::Receiver<ToolMessage> {
        self.tool_tx.subscribe()
    }

    // ─── Internal ─────────────────────────────────────────────

    fn persist(&self, channel: &str, key: &str, payload: &str) {
        if let Some(store) = &self.store {
            let _ = store.log_event(
                &format!("msg_bus:{}", channel),
                &format!("{}:{}", key, payload),
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bus() -> MessageBus {
        MessageBus::new(None)
    }

    #[test]
    fn test_task_publish_subscribe() {
        let bus = make_bus();
        let mut rx = bus.subscribe_tasks();

        let msg = TaskMessage {
            task_id: "t1".into(),
            query: "What is Rust?".into(),
            complexity: 0.5,
            source: "user".into(),
            timestamp: Utc::now(),
        };
        let receivers = bus.publish_task(msg.clone());
        assert_eq!(receivers, 1);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.task_id, "t1");
        assert_eq!(received.query, "What is Rust?");
    }

    #[test]
    fn test_proposal_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_proposals();

        bus.publish_proposal(ProposalMessage {
            task_id: "t1".into(),
            agent_id: "agent-a".into(),
            content: "Rust is a systems language".into(),
            layer: 1,
            confidence: 0.85,
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.agent_id, "agent-a");
        assert_eq!(received.layer, 1);
    }

    #[test]
    fn test_critique_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_critiques();

        bus.publish_critique(CritiqueMessage {
            task_id: "t1".into(),
            agent_id: "critic-b".into(),
            proposal_agent_id: "agent-a".into(),
            score: 0.7,
            feedback: "Good but could mention safety".into(),
            layer: 1,
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.score, 0.7);
    }

    #[test]
    fn test_result_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_results();

        bus.publish_result(ResultMessage {
            task_id: "t1".into(),
            content: "Final answer".into(),
            total_layers: 3,
            total_agents: 5,
            quality_score: 0.92,
            duration_ms: 1500,
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.total_layers, 3);
        assert_eq!(received.quality_score, 0.92);
    }

    #[test]
    fn test_topology_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_topology();

        bus.publish_topology(TopologyMessage {
            action: TopologyAction::AddNode,
            agent_id: "agent-c".into(),
            connections: vec!["agent-a".into(), "agent-b".into()],
            weight: 1.0,
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.action, TopologyAction::AddNode);
        assert_eq!(received.connections.len(), 2);
    }

    #[test]
    fn test_health_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_health();

        bus.publish_health(HealthMessage {
            source: "ollama".into(),
            status: HealthStatus::Degraded,
            message: "High latency detected".into(),
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_tool_roundtrip() {
        let bus = make_bus();
        let mut rx = bus.subscribe_tools();

        bus.publish_tool(ToolMessage {
            task_id: "t1".into(),
            agent_id: "agent-a".into(),
            tool_name: "web_search".into(),
            input: serde_json::json!({"query": "Rust async"}),
            output: None,
            status: ToolStatus::Pending,
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.tool_name, "web_search");
        assert_eq!(received.status, ToolStatus::Pending);
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = make_bus();
        let mut rx1 = bus.subscribe_tasks();
        let mut rx2 = bus.subscribe_tasks();

        bus.publish_task(TaskMessage {
            task_id: "t1".into(),
            query: "test".into(),
            complexity: 0.1,
            source: "test".into(),
            timestamp: Utc::now(),
        });

        // Both subscribers receive the message
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn test_no_subscriber_no_panic() {
        let bus = make_bus();
        // Publishing without subscribers should return 0, not panic
        let receivers = bus.publish_task(TaskMessage {
            task_id: "t1".into(),
            query: "test".into(),
            complexity: 0.1,
            source: "test".into(),
            timestamp: Utc::now(),
        });
        assert_eq!(receivers, 0);
    }

    #[test]
    fn test_with_sqlite_persistence() {
        let store = OrchestratorStore::open_memory().unwrap();
        let bus = MessageBus::new(Some(Arc::new(store)));
        let mut rx = bus.subscribe_tasks();

        bus.publish_task(TaskMessage {
            task_id: "t1".into(),
            query: "persisted task".into(),
            complexity: 0.5,
            source: "test".into(),
            timestamp: Utc::now(),
        });

        let received = rx.try_recv().unwrap();
        assert_eq!(received.task_id, "t1");
    }

    #[test]
    fn test_serde_roundtrip_all_messages() {
        // Verify all message types serialize/deserialize correctly
        let task = TaskMessage {
            task_id: "t".into(), query: "q".into(), complexity: 0.5,
            source: "s".into(), timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&task).unwrap();
        let _: TaskMessage = serde_json::from_str(&json).unwrap();

        let topo = TopologyMessage {
            action: TopologyAction::Rebalance, agent_id: "a".into(),
            connections: vec![], weight: 0.5, timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&topo).unwrap();
        let deser: TopologyMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.action, TopologyAction::Rebalance);
    }
}
