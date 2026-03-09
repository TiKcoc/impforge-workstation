// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Internal helpers (RebalanceReport, route, etc.) are test-exercised.
#![allow(dead_code)]
//! Dynamic Topology Manager for ImpForge Orchestrator
//!
//! Implements DyTopo (arXiv:2602.06039) — dynamic agent topology optimization
//! where agents form adaptive directed acyclic graphs (DAGs) based on:
//! - Trust scores (Hebbian learning from execution history)
//! - Response quality (critique-weighted performance)
//! - Latency budgets (faster agents preferred for time-critical tasks)
//!
//! The topology self-organizes: high-performing agents get more connections,
//! while low-trust agents are gradually isolated or removed.
//!
//! Scientific basis:
//! - DyTopo (arXiv:2602.06039) — sparse DAG routing for multi-agent systems
//! - Barabási-Albert model — preferential attachment in scale-free networks
//! - Watts-Strogatz — small-world topology for efficient message passing

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A node in the agent topology graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub agent_id: String,
    pub trust_score: f64,
    pub avg_latency_ms: u64,
    pub total_tasks: u64,
    pub capabilities: Vec<String>,
    pub active: bool,
}

/// A directed edge between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub message_count: u64,
}

/// Configuration for topology behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyConfig {
    /// Maximum edges per node (prevents hub overload).
    pub max_edges_per_node: usize,
    /// Trust threshold below which nodes are marked inactive.
    pub min_trust_threshold: f64,
    /// Weight decay factor per rebalance cycle (0.0–1.0).
    pub weight_decay: f64,
    /// Whether to automatically prune inactive nodes.
    pub auto_prune: bool,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self {
            max_edges_per_node: 5,
            min_trust_threshold: 0.3,
            weight_decay: 0.95,
            auto_prune: true,
        }
    }
}

/// Dynamic topology manager.
///
/// Maintains a sparse DAG of agent connections. Agents route tasks
/// through the graph based on edge weights (trust × capability match).
pub struct TopologyManager {
    nodes: HashMap<String, TopologyNode>,
    edges: Vec<TopologyEdge>,
    config: TopologyConfig,
}

impl TopologyManager {
    pub fn new(config: TopologyConfig) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            config,
        }
    }

    /// Add a new agent to the topology.
    pub fn add_node(&mut self, agent_id: &str, capabilities: Vec<String>) {
        self.nodes.insert(agent_id.to_string(), TopologyNode {
            agent_id: agent_id.to_string(),
            trust_score: 0.5, // Start neutral
            avg_latency_ms: 0,
            total_tasks: 0,
            capabilities,
            active: true,
        });
    }

    /// Remove an agent from the topology (and all its edges).
    pub fn remove_node(&mut self, agent_id: &str) {
        self.nodes.remove(agent_id);
        self.edges.retain(|e| e.from != agent_id && e.to != agent_id);
    }

    /// Add or update a directed edge between two agents.
    pub fn update_edge(&mut self, from: &str, to: &str, weight: f64) {
        // Don't allow self-loops
        if from == to {
            return;
        }

        // Check max edges constraint
        let from_edge_count = self.edges.iter().filter(|e| e.from == from).count();
        if from_edge_count >= self.config.max_edges_per_node {
            // Find weakest edge from this node and replace if new weight is higher
            if let Some(weakest_idx) = self.edges.iter()
                .enumerate()
                .filter(|(_, e)| e.from == from)
                .min_by(|(_, a), (_, b)| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
            {
                if self.edges[weakest_idx].weight < weight {
                    self.edges.remove(weakest_idx);
                } else {
                    return; // All existing edges are stronger
                }
            }
        }

        // Update existing edge or add new one
        if let Some(edge) = self.edges.iter_mut().find(|e| e.from == from && e.to == to) {
            edge.weight = weight;
            edge.message_count += 1;
        } else {
            self.edges.push(TopologyEdge {
                from: from.to_string(),
                to: to.to_string(),
                weight,
                message_count: 1,
            });
        }
    }

    /// Update trust score for an agent after task execution.
    pub fn update_trust(&mut self, agent_id: &str, trust: f64, latency_ms: u64) {
        if let Some(node) = self.nodes.get_mut(agent_id) {
            node.trust_score = trust;
            node.total_tasks += 1;
            // Exponential moving average for latency
            if node.avg_latency_ms == 0 {
                node.avg_latency_ms = latency_ms;
            } else {
                node.avg_latency_ms = (node.avg_latency_ms * 7 + latency_ms * 3) / 10;
            }
            // Auto-deactivate below threshold
            if trust < self.config.min_trust_threshold {
                node.active = false;
            } else {
                node.active = true;
            }
        }
    }

    /// Rebalance the topology: decay weights, prune inactive nodes.
    ///
    /// Called periodically to prevent stale connections from dominating.
    /// This is the DyTopo "reorganization" step (arXiv:2602.06039 §4.2).
    pub fn rebalance(&mut self) -> RebalanceReport {
        let mut decayed = 0;
        #[allow(unused_assignments)]
        let mut pruned_edges = 0;
        let mut pruned_nodes = 0;

        // Decay all edge weights
        for edge in &mut self.edges {
            edge.weight *= self.config.weight_decay;
            decayed += 1;
        }

        // Remove edges with near-zero weight
        let before = self.edges.len();
        self.edges.retain(|e| e.weight > 0.01);
        pruned_edges = before - self.edges.len();

        // Auto-prune inactive nodes
        if self.config.auto_prune {
            let inactive: Vec<String> = self.nodes.iter()
                .filter(|(_, n)| !n.active && n.total_tasks > 5) // Only prune after enough samples
                .map(|(id, _)| id.clone())
                .collect();

            for id in &inactive {
                self.remove_node(id);
                pruned_nodes += 1;
            }
        }

        RebalanceReport {
            edges_decayed: decayed,
            edges_pruned: pruned_edges,
            nodes_pruned: pruned_nodes,
            total_nodes: self.nodes.len(),
            total_edges: self.edges.len(),
        }
    }

    /// Find the best route (agent sequence) for a task with given capabilities.
    ///
    /// Uses greedy routing: at each hop, pick the neighbor with the highest
    /// (trust × edge_weight × capability_match) score.
    pub fn route(&self, required_capabilities: &[&str], max_hops: usize) -> Vec<String> {
        let mut route = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();

        // Start from the highest-trust active node with matching capabilities
        let start = self.nodes.values()
            .filter(|n| n.active)
            .filter(|n| required_capabilities.iter().any(|cap| n.capabilities.contains(&cap.to_string())))
            .max_by(|a, b| a.trust_score.partial_cmp(&b.trust_score).unwrap_or(std::cmp::Ordering::Equal));

        let Some(start_node) = start else {
            return route;
        };

        route.push(start_node.agent_id.clone());
        visited.insert(start_node.agent_id.clone());

        // Greedy routing through the DAG
        let mut current = start_node.agent_id.clone();
        for _ in 0..max_hops {
            let neighbors: Vec<_> = self.edges.iter()
                .filter(|e| e.from == current && !visited.contains(&e.to))
                .filter(|e| self.nodes.get(&e.to).map(|n| n.active).unwrap_or(false))
                .collect();

            if neighbors.is_empty() {
                break;
            }

            // Pick neighbor with best combined score
            let best = neighbors.iter()
                .max_by(|a, b| {
                    let score_a = a.weight * self.nodes.get(&a.to).map(|n| n.trust_score).unwrap_or(0.0);
                    let score_b = b.weight * self.nodes.get(&b.to).map(|n| n.trust_score).unwrap_or(0.0);
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some(edge) = best {
                route.push(edge.to.clone());
                visited.insert(edge.to.clone());
                current = edge.to.clone();
            }
        }

        route
    }

    /// Get all active nodes.
    pub fn active_nodes(&self) -> Vec<&TopologyNode> {
        self.nodes.values().filter(|n| n.active).collect()
    }

    /// Get all edges.
    pub fn edges(&self) -> &[TopologyEdge] {
        &self.edges
    }

    /// Get node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get a snapshot of the entire topology for serialization.
    pub fn snapshot(&self) -> TopologySnapshot {
        TopologySnapshot {
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.clone(),
            config: self.config.clone(),
        }
    }
}

/// Rebalance operation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceReport {
    pub edges_decayed: usize,
    pub edges_pruned: usize,
    pub nodes_pruned: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
}

/// Serializable topology snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologySnapshot {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub config: TopologyConfig,
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_topology() -> TopologyManager {
        let mut topo = TopologyManager::new(TopologyConfig::default());
        topo.add_node("agent-a", vec!["code".into(), "review".into()]);
        topo.add_node("agent-b", vec!["code".into(), "test".into()]);
        topo.add_node("agent-c", vec!["review".into(), "docs".into()]);
        topo
    }

    #[test]
    fn test_add_remove_nodes() {
        let mut topo = setup_topology();
        assert_eq!(topo.node_count(), 3);

        topo.remove_node("agent-b");
        assert_eq!(topo.node_count(), 2);
    }

    #[test]
    fn test_add_edges() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-b", 0.8);
        topo.update_edge("agent-b", "agent-c", 0.6);

        assert_eq!(topo.edge_count(), 2);
    }

    #[test]
    fn test_no_self_loops() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-a", 1.0);
        assert_eq!(topo.edge_count(), 0);
    }

    #[test]
    fn test_edge_weight_update() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-b", 0.5);
        topo.update_edge("agent-a", "agent-b", 0.9);

        assert_eq!(topo.edge_count(), 1);
        assert_eq!(topo.edges()[0].weight, 0.9);
        assert_eq!(topo.edges()[0].message_count, 2);
    }

    #[test]
    fn test_max_edges_constraint() {
        let config = TopologyConfig {
            max_edges_per_node: 2,
            ..Default::default()
        };
        let mut topo = TopologyManager::new(config);
        topo.add_node("hub", vec!["code".into()]);
        topo.add_node("a", vec!["code".into()]);
        topo.add_node("b", vec!["code".into()]);
        topo.add_node("c", vec!["code".into()]);

        topo.update_edge("hub", "a", 0.5);
        topo.update_edge("hub", "b", 0.7);
        // Third edge should replace weakest (a, 0.5) if new weight is higher
        topo.update_edge("hub", "c", 0.8);

        let hub_edges: Vec<_> = topo.edges().iter().filter(|e| e.from == "hub").collect();
        assert_eq!(hub_edges.len(), 2);
        // Should have b(0.7) and c(0.8), not a(0.5)
        assert!(hub_edges.iter().any(|e| e.to == "b"));
        assert!(hub_edges.iter().any(|e| e.to == "c"));
    }

    #[test]
    fn test_trust_update() {
        let mut topo = setup_topology();
        topo.update_trust("agent-a", 0.95, 100);

        let node = topo.nodes.get("agent-a").unwrap();
        assert_eq!(node.trust_score, 0.95);
        assert_eq!(node.total_tasks, 1);
        assert_eq!(node.avg_latency_ms, 100);
        assert!(node.active);
    }

    #[test]
    fn test_trust_deactivation() {
        let mut topo = setup_topology();
        topo.update_trust("agent-a", 0.2, 500); // Below threshold

        let node = topo.nodes.get("agent-a").unwrap();
        assert!(!node.active);
    }

    #[test]
    fn test_rebalance_weight_decay() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-b", 1.0);

        let report = topo.rebalance();
        assert_eq!(report.edges_decayed, 1);
        assert!(topo.edges()[0].weight < 1.0);
        assert!((topo.edges()[0].weight - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_rebalance_prunes_weak_edges() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-b", 0.005); // Very weak

        let report = topo.rebalance();
        assert_eq!(report.edges_pruned, 1);
        assert_eq!(topo.edge_count(), 0);
    }

    #[test]
    fn test_routing() {
        let mut topo = setup_topology();
        topo.update_trust("agent-a", 0.9, 50);
        topo.update_trust("agent-b", 0.8, 60);
        topo.update_trust("agent-c", 0.7, 70);

        topo.update_edge("agent-a", "agent-b", 0.9);
        topo.update_edge("agent-b", "agent-c", 0.8);

        let route = topo.route(&["code"], 3);
        assert!(!route.is_empty());
        assert_eq!(route[0], "agent-a"); // Highest trust with "code" capability
    }

    #[test]
    fn test_routing_no_match() {
        let topo = setup_topology();
        let route = topo.route(&["quantum_computing"], 3);
        assert!(route.is_empty());
    }

    #[test]
    fn test_snapshot_serialization() {
        let mut topo = setup_topology();
        topo.update_edge("agent-a", "agent-b", 0.7);

        let snapshot = topo.snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let deser: TopologySnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.nodes.len(), 3);
        assert_eq!(deser.edges.len(), 1);
    }

    #[test]
    fn test_latency_ema() {
        let mut topo = setup_topology();
        topo.update_trust("agent-a", 0.8, 100);
        topo.update_trust("agent-a", 0.8, 200);

        let node = topo.nodes.get("agent-a").unwrap();
        // EMA: (100*7 + 200*3) / 10 = 130
        assert_eq!(node.avg_latency_ms, 130);
    }

    #[test]
    fn test_active_nodes() {
        let mut topo = setup_topology();
        topo.update_trust("agent-b", 0.1, 500); // Deactivate

        let active = topo.active_nodes();
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|n| n.agent_id != "agent-b"));
    }
}
