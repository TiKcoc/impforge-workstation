#![allow(dead_code)]
//! Knowledge Graph Engine -- SQLite-backed with in-memory adjacency index
//!
//! Typed nodes and edges with temporal validity, BFS traversal,
//! ego subgraph extraction, connected-component detection, and degree analytics.
//!
//! In-memory adjacency list (`HashMap<String, Vec<String>>`) enables O(1) neighbor
//! lookup while SQLite provides durable persistence via `kg_nodes` / `kg_edges`
//! tables defined in v001.
//!
//! References:
//!   - Kejriwal, M., Knoblock, C. A., & Szekely, P. (2021). Knowledge Graphs:
//!     Methodology, Tools and Selected Use Cases. Springer.
//!   - Microsoft Research (2024). GraphRAG: graph-enhanced retrieval-augmented
//!     generation for improved answer quality on private corpora.

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ── Public Data Types ───────────────────────────────────────────

/// A node in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub properties: Option<String>,
    pub confidence: f64,
}

/// A directed edge in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub weight: f64,
    pub confidence: f64,
}

/// An ego-centric subgraph extracted around a center node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

// ── Valid Type Enumerations ─────────────────────────────────────

/// Allowed node kinds (must match kg_nodes CHECK constraint in v001).
const VALID_NODE_KINDS: &[&str] = &[
    "file", "symbol", "concept", "session", "user",
    "pattern", "crate", "module", "function", "type",
];

/// Allowed edge kinds (must match kg_edges CHECK constraint in v001).
const VALID_EDGE_KINDS: &[&str] = &[
    "contains", "references", "co_changes", "depends_on",
    "derives", "similar", "edits", "owns", "imports", "calls",
];

// ── Knowledge Graph ─────────────────────────────────────────────

/// In-memory knowledge graph with SQLite-backed persistence.
///
/// The adjacency index maps each node id to the set of edge ids that touch it
/// (both outgoing and incoming), enabling fast neighbor traversal without
/// scanning the entire edge set.
pub struct KnowledgeGraph {
    nodes: HashMap<String, GraphNode>,
    edges: HashMap<String, GraphEdge>,
    /// node_id -> list of edge_ids where node is source or target
    adjacency: HashMap<String, Vec<String>>,
    /// Monotonic counter for deterministic edge id generation in-memory.
    edge_counter: u64,
}

impl KnowledgeGraph {
    /// Create an empty knowledge graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
            edge_counter: 0,
        }
    }

    // ── Node operations ─────────────────────────────────────

    /// Add or replace a node. Returns `Err` if `kind` is not in the valid set.
    pub fn add_node(
        &mut self,
        id: &str,
        kind: &str,
        label: &str,
        properties: Option<&str>,
    ) -> Result<(), String> {
        if !VALID_NODE_KINDS.contains(&kind) {
            return Err(format!(
                "Invalid node kind '{kind}'. Valid: {VALID_NODE_KINDS:?}"
            ));
        }
        let node = GraphNode {
            id: id.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            properties: properties.map(|s| s.to_string()),
            confidence: 1.0,
        };
        self.nodes.insert(id.to_string(), node);
        // Ensure adjacency entry exists
        self.adjacency.entry(id.to_string()).or_default();
        Ok(())
    }

    /// Retrieve a node by id.
    pub fn get_node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.get(id)
    }

    /// Remove a node and all edges connected to it (cascade).
    pub fn remove_node(&mut self, id: &str) {
        self.nodes.remove(id);
        // Collect edge ids to remove
        if let Some(edge_ids) = self.adjacency.remove(id) {
            for edge_id in &edge_ids {
                if let Some(edge) = self.edges.remove(edge_id) {
                    // Remove this edge from the *other* node's adjacency list
                    let other = if edge.source_id == id {
                        &edge.target_id
                    } else {
                        &edge.source_id
                    };
                    if let Some(other_edges) = self.adjacency.get_mut(other.as_str()) {
                        other_edges.retain(|eid| eid != edge_id);
                    }
                }
            }
        }
    }

    /// Substring search on node labels, optionally filtered by kind.
    pub fn search_nodes(&self, label_pattern: &str, kind: Option<&str>) -> Vec<GraphNode> {
        let pattern_lower = label_pattern.to_lowercase();
        self.nodes
            .values()
            .filter(|n| {
                n.label.to_lowercase().contains(&pattern_lower)
                    && kind.map_or(true, |k| n.kind == k)
            })
            .cloned()
            .collect()
    }

    // ── Edge operations ─────────────────────────────────────

    /// Add a directed edge. Returns the generated edge id.
    /// Returns `Err` if source/target nodes do not exist or kind is invalid.
    pub fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        kind: &str,
        weight: f64,
    ) -> Result<String, String> {
        if !VALID_EDGE_KINDS.contains(&kind) {
            return Err(format!(
                "Invalid edge kind '{kind}'. Valid: {VALID_EDGE_KINDS:?}"
            ));
        }
        if !self.nodes.contains_key(source) {
            return Err(format!("Source node '{source}' does not exist"));
        }
        if !self.nodes.contains_key(target) {
            return Err(format!("Target node '{target}' does not exist"));
        }

        self.edge_counter += 1;
        let edge_id = format!("edge_{:016x}", self.edge_counter);

        let edge = GraphEdge {
            id: edge_id.clone(),
            source_id: source.to_string(),
            target_id: target.to_string(),
            kind: kind.to_string(),
            weight,
            confidence: 1.0,
        };

        self.edges.insert(edge_id.clone(), edge);
        self.adjacency
            .entry(source.to_string())
            .or_default()
            .push(edge_id.clone());
        self.adjacency
            .entry(target.to_string())
            .or_default()
            .push(edge_id.clone());

        Ok(edge_id)
    }

    /// Remove a single edge by id.
    pub fn remove_edge(&mut self, id: &str) {
        if let Some(edge) = self.edges.remove(id) {
            if let Some(src_edges) = self.adjacency.get_mut(&edge.source_id) {
                src_edges.retain(|eid| eid != id);
            }
            if let Some(tgt_edges) = self.adjacency.get_mut(&edge.target_id) {
                tgt_edges.retain(|eid| eid != id);
            }
        }
    }

    // ── Traversal ───────────────────────────────────────────

    /// Get all neighbors of a node, optionally filtered by edge kind.
    /// Returns tuples of (neighbor_node, connecting_edge).
    /// Follows edges in both directions (the graph is stored directed but
    /// traversal is bidirectional for discovery).
    pub fn get_neighbors(
        &self,
        id: &str,
        edge_kind: Option<&str>,
    ) -> Vec<(GraphNode, GraphEdge)> {
        let Some(edge_ids) = self.adjacency.get(id) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for edge_id in edge_ids {
            let Some(edge) = self.edges.get(edge_id) else {
                continue;
            };
            if let Some(filter) = edge_kind {
                if edge.kind != filter {
                    continue;
                }
            }
            // Determine the neighbor (the node on the other side of the edge)
            let neighbor_id = if edge.source_id == id {
                &edge.target_id
            } else {
                &edge.source_id
            };
            if let Some(neighbor) = self.nodes.get(neighbor_id) {
                results.push((neighbor.clone(), edge.clone()));
            }
        }
        results
    }

    /// BFS shortest path from `from` to `to`, with a maximum search depth.
    /// Returns the sequence of node ids forming the path, or `None` if
    /// no path exists within `max_depth` hops.
    pub fn find_path(&self, from: &str, to: &str, max_depth: usize) -> Option<Vec<String>> {
        if from == to {
            return Some(vec![from.to_string()]);
        }
        if !self.nodes.contains_key(from) || !self.nodes.contains_key(to) {
            return None;
        }

        // BFS with parent tracking
        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<(&str, usize)> = VecDeque::new();
        let mut parent: HashMap<&str, &str> = HashMap::new();

        visited.insert(from);
        queue.push_back((from, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let Some(edge_ids) = self.adjacency.get(current) else {
                continue;
            };
            for edge_id in edge_ids {
                let Some(edge) = self.edges.get(edge_id) else {
                    continue;
                };
                let neighbor_id = if edge.source_id == current {
                    edge.target_id.as_str()
                } else {
                    edge.source_id.as_str()
                };
                if visited.contains(neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id);
                parent.insert(neighbor_id, current);

                if neighbor_id == to {
                    // Reconstruct path
                    let mut path = vec![to.to_string()];
                    let mut cursor = to;
                    while let Some(&prev) = parent.get(cursor) {
                        path.push(prev.to_string());
                        cursor = prev;
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back((neighbor_id, depth + 1));
            }
        }

        None
    }

    /// Extract an ego-centric subgraph around `center_id` up to `depth` hops.
    /// Uses BFS to collect all reachable nodes within the radius.
    pub fn get_subgraph(&self, center_id: &str, depth: usize) -> SubGraph {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut sub_nodes: Vec<GraphNode> = Vec::new();
        let mut sub_edge_ids: HashSet<String> = HashSet::new();

        if let Some(center) = self.nodes.get(center_id) {
            visited.insert(center_id.to_string());
            queue.push_back((center_id.to_string(), 0));
            sub_nodes.push(center.clone());
        }

        while let Some((current, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }
            let Some(edge_ids) = self.adjacency.get(&current) else {
                continue;
            };
            for edge_id in edge_ids {
                let Some(edge) = self.edges.get(edge_id) else {
                    continue;
                };
                let neighbor_id = if edge.source_id == current {
                    &edge.target_id
                } else {
                    &edge.source_id
                };
                sub_edge_ids.insert(edge_id.clone());
                if !visited.contains(neighbor_id.as_str()) {
                    visited.insert(neighbor_id.clone());
                    if let Some(neighbor) = self.nodes.get(neighbor_id) {
                        sub_nodes.push(neighbor.clone());
                    }
                    queue.push_back((neighbor_id.clone(), d + 1));
                }
            }
        }

        // Only include edges where both endpoints are in the subgraph
        let sub_edges: Vec<GraphEdge> = sub_edge_ids
            .iter()
            .filter_map(|eid| {
                let edge = self.edges.get(eid)?;
                if visited.contains(&edge.source_id) && visited.contains(&edge.target_id) {
                    Some(edge.clone())
                } else {
                    None
                }
            })
            .collect();

        SubGraph {
            nodes: sub_nodes,
            edges: sub_edges,
        }
    }

    // ── Analytics ────────────────────────────────────────────

    /// Returns (in_degree, out_degree) for a node.
    /// in_degree  = number of edges where this node is the target.
    /// out_degree = number of edges where this node is the source.
    pub fn node_degree(&self, id: &str) -> (usize, usize) {
        let Some(edge_ids) = self.adjacency.get(id) else {
            return (0, 0);
        };
        let mut in_deg = 0usize;
        let mut out_deg = 0usize;
        for edge_id in edge_ids {
            if let Some(edge) = self.edges.get(edge_id) {
                if edge.source_id == id {
                    out_deg += 1;
                }
                if edge.target_id == id {
                    in_deg += 1;
                }
            }
        }
        (in_deg, out_deg)
    }

    /// Return the top `limit` nodes by total degree (in + out), descending.
    pub fn most_connected(&self, limit: usize) -> Vec<(GraphNode, usize)> {
        let mut scored: Vec<(GraphNode, usize)> = self
            .nodes
            .values()
            .map(|node| {
                let (i, o) = self.node_degree(&node.id);
                (node.clone(), i + o)
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(limit);
        scored
    }

    /// Find connected components (treating edges as undirected).
    /// Returns a list of components, each being a list of node ids.
    pub fn find_communities(&self) -> Vec<Vec<String>> {
        let mut visited: HashSet<&str> = HashSet::new();
        let mut components: Vec<Vec<String>> = Vec::new();

        for node_id in self.nodes.keys() {
            if visited.contains(node_id.as_str()) {
                continue;
            }
            // BFS to find all reachable nodes
            let mut component: Vec<String> = Vec::new();
            let mut queue: VecDeque<&str> = VecDeque::new();
            visited.insert(node_id.as_str());
            queue.push_back(node_id.as_str());

            while let Some(current) = queue.pop_front() {
                component.push(current.to_string());
                if let Some(edge_ids) = self.adjacency.get(current) {
                    for edge_id in edge_ids {
                        if let Some(edge) = self.edges.get(edge_id) {
                            let neighbor = if edge.source_id == current {
                                edge.target_id.as_str()
                            } else {
                                edge.source_id.as_str()
                            };
                            if !visited.contains(neighbor) {
                                visited.insert(neighbor);
                                queue.push_back(neighbor);
                            }
                        }
                    }
                }
            }

            component.sort();
            components.push(component);
        }

        components.sort_by_key(|c| std::cmp::Reverse(c.len()));
        components
    }

    /// Total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ── Persistence ─────────────────────────────────────────

    /// Write the entire in-memory graph to SQLite (full snapshot).
    /// Uses the existing `kg_nodes` and `kg_edges` tables from v001.
    pub fn sync_to_db(&self, conn: &Connection) -> SqlResult<()> {
        // Clear existing data (full snapshot strategy, same as HNSW persistence)
        conn.execute("DELETE FROM kg_edges", [])?;
        conn.execute("DELETE FROM kg_nodes", [])?;

        // Write nodes
        let mut node_stmt = conn.prepare(
            "INSERT INTO kg_nodes (id, kind, label, properties, confidence, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        )?;
        for node in self.nodes.values() {
            node_stmt.execute(params![
                node.id,
                node.kind,
                node.label,
                node.properties,
                node.confidence,
            ])?;
        }

        // Write edges
        let mut edge_stmt = conn.prepare(
            "INSERT INTO kg_edges (id, source_id, target_id, kind, weight, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for edge in self.edges.values() {
            edge_stmt.execute(params![
                edge.id,
                edge.source_id,
                edge.target_id,
                edge.kind,
                edge.weight,
                edge.confidence,
            ])?;
        }

        Ok(())
    }

    /// Load the entire graph from SQLite into memory.
    /// Rebuilds the adjacency index from persisted nodes and edges.
    pub fn load_from_db(conn: &Connection) -> SqlResult<Self> {
        let mut graph = Self::new();

        // Load nodes
        let mut node_stmt = conn.prepare(
            "SELECT id, kind, label, properties, confidence FROM kg_nodes",
        )?;
        let node_rows = node_stmt.query_map([], |row| {
            Ok(GraphNode {
                id: row.get(0)?,
                kind: row.get(1)?,
                label: row.get(2)?,
                properties: row.get(3)?,
                confidence: row.get(4)?,
            })
        })?;
        for node_result in node_rows {
            let node = node_result?;
            let id = node.id.clone();
            graph.nodes.insert(id.clone(), node);
            graph.adjacency.entry(id).or_default();
        }

        // Load edges and rebuild adjacency
        let mut edge_stmt = conn.prepare(
            "SELECT id, source_id, target_id, kind, weight, confidence FROM kg_edges",
        )?;
        let edge_rows = edge_stmt.query_map([], |row| {
            Ok(GraphEdge {
                id: row.get(0)?,
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                kind: row.get(3)?,
                weight: row.get(4)?,
                confidence: row.get(5)?,
            })
        })?;
        for edge_result in edge_rows {
            let edge = edge_result?;
            let eid = edge.id.clone();
            graph
                .adjacency
                .entry(edge.source_id.clone())
                .or_default()
                .push(eid.clone());
            graph
                .adjacency
                .entry(edge.target_id.clone())
                .or_default()
                .push(eid.clone());

            // Track highest counter if edge ids follow our format
            if let Some(hex_str) = eid.strip_prefix("edge_") {
                if let Ok(val) = u64::from_str_radix(hex_str, 16) {
                    if val > graph.edge_counter {
                        graph.edge_counter = val;
                    }
                }
            }

            graph.edges.insert(eid, edge);
        }

        Ok(graph)
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_memory::store::ForgeMemoryStore;

    /// Helper: create a graph with a small project structure.
    ///
    /// ```text
    ///   file:main.rs --contains--> fn:main
    ///   file:main.rs --contains--> fn:run
    ///   fn:main      --calls-----> fn:run
    ///   file:lib.rs  --contains--> fn:init
    ///   fn:run        --calls----> fn:init
    /// ```
    fn sample_graph() -> KnowledgeGraph {
        let mut g = KnowledgeGraph::new();
        g.add_node("file:main.rs", "file", "main.rs", None).unwrap();
        g.add_node("file:lib.rs", "file", "lib.rs", None).unwrap();
        g.add_node("fn:main", "function", "main", None).unwrap();
        g.add_node("fn:run", "function", "run", None).unwrap();
        g.add_node("fn:init", "function", "init", None).unwrap();

        g.add_edge("file:main.rs", "fn:main", "contains", 1.0).unwrap();
        g.add_edge("file:main.rs", "fn:run", "contains", 1.0).unwrap();
        g.add_edge("fn:main", "fn:run", "calls", 1.0).unwrap();
        g.add_edge("file:lib.rs", "fn:init", "contains", 1.0).unwrap();
        g.add_edge("fn:run", "fn:init", "calls", 0.8).unwrap();
        g
    }

    /// Helper: create an in-memory ForgeMemoryStore (runs all migrations).
    fn test_db_conn() -> Connection {
        let store = ForgeMemoryStore::open_memory().unwrap();
        // Extract the inner connection for direct use.
        // We need a raw Connection for sync_to_db / load_from_db.
        // Since ForgeMemoryStore wraps it in Mutex, we create a fresh one
        // and run migrations manually.
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        crate::forge_memory::migration::run_migrations(&conn).unwrap();
        // Drop `store` -- we only created it to prove migrations are valid.
        drop(store);
        conn
    }

    // ── 1. Add nodes and edges ──────────────────────────────

    #[test]
    fn test_add_nodes_and_edges() {
        let g = sample_graph();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 5);

        let main_node = g.get_node("fn:main").unwrap();
        assert_eq!(main_node.kind, "function");
        assert_eq!(main_node.label, "main");
        assert_eq!(main_node.confidence, 1.0);
    }

    // ── 2. Get neighbors (directed, filtered by kind) ───────

    #[test]
    fn test_get_neighbors_directed_filtered() {
        let g = sample_graph();

        // All neighbors of file:main.rs (should include fn:main, fn:run)
        let all = g.get_neighbors("file:main.rs", None);
        assert_eq!(all.len(), 2);

        // Only "contains" edges from file:main.rs
        let contains = g.get_neighbors("file:main.rs", Some("contains"));
        assert_eq!(contains.len(), 2);
        let labels: HashSet<String> = contains.iter().map(|(n, _)| n.label.clone()).collect();
        assert!(labels.contains("main"));
        assert!(labels.contains("run"));

        // Only "calls" edges -- file:main.rs has none as source, but
        // fn:main calls fn:run so fn:main's adjacency should show it
        let calls_from_main = g.get_neighbors("fn:main", Some("calls"));
        assert_eq!(calls_from_main.len(), 1);
        assert_eq!(calls_from_main[0].0.label, "run");
    }

    // ── 3. BFS shortest path ────────────────────────────────

    #[test]
    fn test_bfs_shortest_path() {
        let g = sample_graph();

        // Direct neighbor
        let path = g.find_path("file:main.rs", "fn:main", 5).unwrap();
        assert_eq!(path, vec!["file:main.rs", "fn:main"]);

        // Two hops: file:main.rs -> fn:run -> fn:init
        let path = g.find_path("file:main.rs", "fn:init", 5).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], "file:main.rs");
        assert_eq!(path.last().unwrap(), "fn:init");

        // Path to self
        let path = g.find_path("fn:main", "fn:main", 5).unwrap();
        assert_eq!(path, vec!["fn:main"]);
    }

    // ── 4. Ego subgraph extraction ──────────────────────────

    #[test]
    fn test_ego_subgraph() {
        let g = sample_graph();

        // Depth 1 from fn:run: should get fn:main, file:main.rs, fn:init
        let sub = g.get_subgraph("fn:run", 1);
        assert!(sub.nodes.len() >= 3); // fn:run + its direct neighbors
        let node_ids: HashSet<String> = sub.nodes.iter().map(|n| n.id.clone()).collect();
        assert!(node_ids.contains("fn:run"));
        assert!(node_ids.contains("fn:main")); // calls edge
        assert!(node_ids.contains("file:main.rs")); // contains edge
        assert!(node_ids.contains("fn:init")); // calls edge

        // Depth 0: only the center node, no edges
        let sub0 = g.get_subgraph("fn:run", 0);
        assert_eq!(sub0.nodes.len(), 1);
        assert_eq!(sub0.edges.len(), 0);
    }

    // ── 5. Remove node cascades edges ───────────────────────

    #[test]
    fn test_remove_node_cascades_edges() {
        let mut g = sample_graph();
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 5);

        // fn:run is connected by 3 edges:
        //   file:main.rs -> fn:run (contains)
        //   fn:main -> fn:run (calls)
        //   fn:run -> fn:init (calls)
        g.remove_node("fn:run");
        assert_eq!(g.node_count(), 4);
        assert_eq!(g.edge_count(), 2); // only file:main.rs->fn:main, file:lib.rs->fn:init remain

        // Neighbors of file:main.rs should now only include fn:main
        let neighbors = g.get_neighbors("file:main.rs", None);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id, "fn:main");
    }

    // ── 6. Node degree calculation ──────────────────────────

    #[test]
    fn test_node_degree() {
        let g = sample_graph();

        // fn:run: in_degree=2 (from file:main.rs contains, from fn:main calls),
        //         out_degree=1 (to fn:init calls)
        let (in_deg, out_deg) = g.node_degree("fn:run");
        assert_eq!(in_deg, 2);
        assert_eq!(out_deg, 1);

        // file:main.rs: in=0, out=2 (contains fn:main, contains fn:run)
        let (in_deg, out_deg) = g.node_degree("file:main.rs");
        assert_eq!(in_deg, 0);
        assert_eq!(out_deg, 2);

        // Nonexistent node
        let (in_deg, out_deg) = g.node_degree("nonexistent");
        assert_eq!(in_deg, 0);
        assert_eq!(out_deg, 0);
    }

    // ── 7. Most connected nodes ─────────────────────────────

    #[test]
    fn test_most_connected() {
        let g = sample_graph();

        let top = g.most_connected(3);
        assert_eq!(top.len(), 3);

        // fn:run has total degree 3 (in=2, out=1), should be first
        assert_eq!(top[0].0.id, "fn:run");
        assert_eq!(top[0].1, 3);
    }

    // ── 8. Connected components ─────────────────────────────

    #[test]
    fn test_connected_components_single() {
        let g = sample_graph();
        let communities = g.find_communities();
        // All 5 nodes are connected in one component
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].len(), 5);
    }

    #[test]
    fn test_connected_components_multiple() {
        let mut g = KnowledgeGraph::new();
        // Component 1
        g.add_node("a", "concept", "Alpha", None).unwrap();
        g.add_node("b", "concept", "Beta", None).unwrap();
        g.add_edge("a", "b", "references", 1.0).unwrap();

        // Component 2 (isolated)
        g.add_node("c", "concept", "Gamma", None).unwrap();
        g.add_node("d", "concept", "Delta", None).unwrap();
        g.add_edge("c", "d", "similar", 0.9).unwrap();

        // Component 3 (single node)
        g.add_node("e", "concept", "Epsilon", None).unwrap();

        let communities = g.find_communities();
        assert_eq!(communities.len(), 3);
        // Sorted by size descending, then alphabetically within each
        assert_eq!(communities[0].len(), 2);
        assert_eq!(communities[1].len(), 2);
        assert_eq!(communities[2].len(), 1);
        assert_eq!(communities[2][0], "e");
    }

    // ── 9. Search nodes by label ────────────────────────────

    #[test]
    fn test_search_nodes_by_label() {
        let g = sample_graph();

        let results = g.search_nodes("main", None);
        assert_eq!(results.len(), 2); // file:main.rs and fn:main

        let results = g.search_nodes("MAIN", None); // case-insensitive
        assert_eq!(results.len(), 2);

        let results = g.search_nodes("init", None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "fn:init");
    }

    // ── 10. Search nodes by kind ────────────────────────────

    #[test]
    fn test_search_nodes_by_kind() {
        let g = sample_graph();

        let files = g.search_nodes("", Some("file"));
        assert_eq!(files.len(), 2);

        let functions = g.search_nodes("", Some("function"));
        assert_eq!(functions.len(), 3);

        // Combined: label + kind filter
        let main_fn = g.search_nodes("main", Some("function"));
        assert_eq!(main_fn.len(), 1);
        assert_eq!(main_fn[0].id, "fn:main");
    }

    // ── 11. Persistence roundtrip ───────────────────────────

    #[test]
    fn test_persistence_roundtrip() {
        let conn = test_db_conn();
        let g = sample_graph();

        // Save
        g.sync_to_db(&conn).unwrap();

        // Verify rows in SQLite
        let node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM kg_nodes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(node_count, 5);
        let edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM kg_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edge_count, 5);

        // Load into a new graph
        let g2 = KnowledgeGraph::load_from_db(&conn).unwrap();
        assert_eq!(g2.node_count(), 5);
        assert_eq!(g2.edge_count(), 5);

        // Verify structure is intact
        let node = g2.get_node("fn:main").unwrap();
        assert_eq!(node.kind, "function");
        assert_eq!(node.label, "main");

        // Verify traversal works after reload
        let neighbors = g2.get_neighbors("file:main.rs", Some("contains"));
        assert_eq!(neighbors.len(), 2);

        // Verify path still works
        let path = g2.find_path("file:main.rs", "fn:init", 5);
        assert!(path.is_some());
    }

    // ── 12. Empty graph operations ──────────────────────────

    #[test]
    fn test_empty_graph_operations() {
        let g = KnowledgeGraph::new();

        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
        assert!(g.get_node("anything").is_none());
        assert_eq!(g.get_neighbors("x", None).len(), 0);
        assert!(g.find_path("a", "b", 10).is_none());
        assert_eq!(g.most_connected(5).len(), 0);
        assert_eq!(g.find_communities().len(), 0);
        assert_eq!(g.search_nodes("test", None).len(), 0);

        let sub = g.get_subgraph("x", 3);
        assert_eq!(sub.nodes.len(), 0);
        assert_eq!(sub.edges.len(), 0);

        let (i, o) = g.node_degree("x");
        assert_eq!(i, 0);
        assert_eq!(o, 0);
    }

    // ── 13. Cyclic graph handling ───────────────────────────

    #[test]
    fn test_cyclic_graph() {
        let mut g = KnowledgeGraph::new();
        g.add_node("a", "module", "A", None).unwrap();
        g.add_node("b", "module", "B", None).unwrap();
        g.add_node("c", "module", "C", None).unwrap();

        // Create a cycle: A -> B -> C -> A
        g.add_edge("a", "b", "imports", 1.0).unwrap();
        g.add_edge("b", "c", "imports", 1.0).unwrap();
        g.add_edge("c", "a", "imports", 1.0).unwrap();

        // BFS should still find shortest path (not infinite loop).
        // Because traversal is bidirectional, BFS can go A -> C via the C->A
        // edge in a single hop, yielding path length 2 (shortest).
        let path = g.find_path("a", "c", 10).unwrap();
        assert_eq!(path.len(), 2); // a -> c (via the c->a edge traversed backwards)

        // Subgraph extraction shouldn't loop infinitely
        let sub = g.get_subgraph("a", 5);
        assert_eq!(sub.nodes.len(), 3);
        assert_eq!(sub.edges.len(), 3);

        // Components: all one component
        let communities = g.find_communities();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].len(), 3);
    }

    // ── 14. Path not found returns None ─────────────────────

    #[test]
    fn test_path_not_found() {
        let mut g = KnowledgeGraph::new();
        g.add_node("island1", "concept", "Island One", None).unwrap();
        g.add_node("island2", "concept", "Island Two", None).unwrap();
        // No edge between them

        assert!(g.find_path("island1", "island2", 10).is_none());

        // Nonexistent nodes
        assert!(g.find_path("nonexistent", "island1", 10).is_none());
        assert!(g.find_path("island1", "nonexistent", 10).is_none());

        // Max depth too shallow
        let mut g2 = KnowledgeGraph::new();
        g2.add_node("x", "concept", "X", None).unwrap();
        g2.add_node("y", "concept", "Y", None).unwrap();
        g2.add_node("z", "concept", "Z", None).unwrap();
        g2.add_edge("x", "y", "references", 1.0).unwrap();
        g2.add_edge("y", "z", "references", 1.0).unwrap();

        // Path x->y->z requires depth 2, but max_depth=1 should fail
        assert!(g2.find_path("x", "z", 1).is_none());
        // With sufficient depth it works
        assert!(g2.find_path("x", "z", 2).is_some());
    }

    // ── 15. Large graph (100+ nodes) ────────────────────────

    #[test]
    fn test_large_graph() {
        let mut g = KnowledgeGraph::new();

        // Create a chain of 150 nodes: n_0 -> n_1 -> ... -> n_149
        for i in 0..150 {
            let id = format!("n_{i}");
            let label = format!("Node {i}");
            g.add_node(&id, "concept", &label, None).unwrap();
        }
        for i in 0..149 {
            let src = format!("n_{i}");
            let tgt = format!("n_{}", i + 1);
            g.add_edge(&src, &tgt, "references", 1.0).unwrap();
        }

        assert_eq!(g.node_count(), 150);
        assert_eq!(g.edge_count(), 149);

        // Shortest path from start to end (length = 150 nodes)
        let path = g.find_path("n_0", "n_149", 200).unwrap();
        assert_eq!(path.len(), 150);
        assert_eq!(path[0], "n_0");
        assert_eq!(path[149], "n_149");

        // Most connected: middle nodes have degree 2, endpoints have degree 1
        let top = g.most_connected(3);
        assert_eq!(top[0].1, 2); // any middle node
        // Endpoints (n_0 and n_149) have degree 1

        // Single connected component
        let communities = g.find_communities();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].len(), 150);
    }

    // ── 16. Invalid kind validation ─────────────────────────

    #[test]
    fn test_invalid_node_kind() {
        let mut g = KnowledgeGraph::new();
        let result = g.add_node("bad", "invalid_kind", "Bad Node", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid node kind"));
    }

    #[test]
    fn test_invalid_edge_kind() {
        let mut g = KnowledgeGraph::new();
        g.add_node("a", "concept", "A", None).unwrap();
        g.add_node("b", "concept", "B", None).unwrap();
        let result = g.add_edge("a", "b", "invalid_edge", 1.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid edge kind"));
    }

    // ── 17. Edge to nonexistent node ────────────────────────

    #[test]
    fn test_edge_nonexistent_nodes() {
        let mut g = KnowledgeGraph::new();
        g.add_node("a", "concept", "A", None).unwrap();

        let result = g.add_edge("a", "missing", "references", 1.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));

        let result = g.add_edge("missing", "a", "references", 1.0);
        assert!(result.is_err());
    }

    // ── 18. Remove edge ─────────────────────────────────────

    #[test]
    fn test_remove_edge() {
        let mut g = sample_graph();
        assert_eq!(g.edge_count(), 5);

        // Get the edge id for file:main.rs -> fn:main
        let neighbors = g.get_neighbors("file:main.rs", Some("contains"));
        let edge_to_main: Vec<_> = neighbors
            .iter()
            .filter(|(n, _)| n.id == "fn:main")
            .collect();
        assert_eq!(edge_to_main.len(), 1);
        let edge_id = edge_to_main[0].1.id.clone();

        g.remove_edge(&edge_id);
        assert_eq!(g.edge_count(), 4);

        // fn:main should no longer be a neighbor via "contains"
        let remaining = g.get_neighbors("file:main.rs", Some("contains"));
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0.id, "fn:run");
    }

    // ── 19. Node properties ─────────────────────────────────

    #[test]
    fn test_node_properties() {
        let mut g = KnowledgeGraph::new();
        let props = r#"{"lines": 42, "language": "rust"}"#;
        g.add_node("file:test.rs", "file", "test.rs", Some(props))
            .unwrap();

        let node = g.get_node("file:test.rs").unwrap();
        assert_eq!(node.properties.as_deref(), Some(props));
    }

    // ── 20. Persistence with empty graph ────────────────────

    #[test]
    fn test_persistence_empty_graph() {
        let conn = test_db_conn();
        let g = KnowledgeGraph::new();

        g.sync_to_db(&conn).unwrap();

        let g2 = KnowledgeGraph::load_from_db(&conn).unwrap();
        assert_eq!(g2.node_count(), 0);
        assert_eq!(g2.edge_count(), 0);
    }

    // ── 21. Add edges after persistence roundtrip ───────────

    #[test]
    fn test_add_after_reload() {
        let conn = test_db_conn();

        // Create, save, load
        let mut g = KnowledgeGraph::new();
        g.add_node("a", "concept", "Alpha", None).unwrap();
        g.add_node("b", "concept", "Beta", None).unwrap();
        g.add_edge("a", "b", "references", 1.0).unwrap();
        g.sync_to_db(&conn).unwrap();

        let mut g2 = KnowledgeGraph::load_from_db(&conn).unwrap();

        // Should be able to add more nodes and edges without id collision
        g2.add_node("c", "concept", "Gamma", None).unwrap();
        let edge_id = g2.add_edge("b", "c", "derives", 0.5).unwrap();
        assert!(!edge_id.is_empty());
        assert_eq!(g2.node_count(), 3);
        assert_eq!(g2.edge_count(), 2);

        // Second roundtrip
        g2.sync_to_db(&conn).unwrap();
        let g3 = KnowledgeGraph::load_from_db(&conn).unwrap();
        assert_eq!(g3.node_count(), 3);
        assert_eq!(g3.edge_count(), 2);
    }
}
