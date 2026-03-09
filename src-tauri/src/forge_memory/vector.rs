// Public API — consumed via forge_memory::commands Tauri layer
#![allow(dead_code)]
//! Custom HNSW Vector Index (Malkov & Yashunin 2018, arXiv:1603.09320)
//!
//! Hierarchical Navigable Small World graph for approximate k-NN search.
//! Pure Rust, no C bindings. In-memory graph with SQLite-backed persistence.
//!
//! Key parameters (per paper, Section 4):
//!   M            — max bi-directional connections per node per layer (default 16)
//!   ef_construction — beam width during insert (default 200)
//!   ef_search    — beam width during query  (default 50)
//!   m_l          — level multiplier = 1/ln(M)
//!
//! Distance metric: angular distance derived from cosine similarity.
//!   d(a,b) = 1.0 - cosine_similarity(a,b)
//!   Range: [0.0, 2.0], where 0.0 = identical direction.
//!
//! Persistence: serialized to/from SQLite via `hnsw_nodes` + `hnsw_metadata` tables.
//!
//! Thread-safety: wrap `HnswIndex` in `parking_lot::RwLock` for concurrent reads.
//!
//! References:
//!   - Malkov, Y. A., & Yashunin, D. A. (2018). Efficient and robust approximate
//!     nearest neighbor search using Hierarchical Navigable Small World graphs.
//!     IEEE TPAMI. arXiv:1603.09320

use parking_lot::RwLock;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

// ── Lightweight PRNG (xorshift64*) ──────────────────────────────
// Avoids adding `rand` crate. Seed from system time.

struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new() -> Self {
        // Seed from system time; if unavailable, use a fixed non-zero seed.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF_CAFE_BABE);
        // Ensure non-zero
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn from_seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Returns a pseudo-random u64.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Returns a uniform f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ── Distance helpers ────────────────────────────────────────────

/// Cosine similarity in [-1.0, 1.0].
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    let denom = (norm_a.sqrt() * norm_b.sqrt()) as f32;
    if denom < f32::EPSILON {
        return 0.0;
    }
    (dot as f32) / denom
}

/// Angular distance: 1.0 - cosine_similarity. Range [0.0, 2.0].
fn angular_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_similarity(a, b)
}

// ── Priority queue helpers (min-heap via Reverse, max-heap native) ───

/// Element for the binary heap: (distance, internal index).
/// `BinaryHeap` is a max-heap, so we use `Reverse` for min-heap behavior
/// and native ordering for max-heap (candidate pruning) behavior.
#[derive(Clone)]
struct HeapItem {
    distance: f32,
    index: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.index == other.index
    }
}
impl Eq for HeapItem {}

/// Max-heap ordering (largest distance first — for result pruning).
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Wrapper for min-heap ordering (smallest distance first — for search).
#[derive(Clone, Eq, PartialEq)]
struct MinHeapItem(HeapItem);

impl Ord for MinHeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}
impl PartialOrd for MinHeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ── HNSW Node ───────────────────────────────────────────────────

/// A single node in the HNSW graph.
#[derive(Clone, Serialize, Deserialize)]
struct HnswNode {
    /// External string identifier.
    id: String,
    /// The embedding vector.
    vector: Vec<f32>,
    /// Connections per level. connections[level] = set of neighbor indices.
    connections: Vec<Vec<usize>>,
    /// The maximum level this node participates in.
    level: usize,
}

// ── HNSW Index ──────────────────────────────────────────────────

/// Hierarchical Navigable Small World index.
///
/// Wrap in `parking_lot::RwLock<HnswIndex>` for concurrent access:
/// ```ignore
/// let index = RwLock::new(HnswIndex::new(HnswConfig::default()));
/// ```
pub struct HnswIndex {
    /// All nodes, indexed by internal ID (position in this vec).
    nodes: Vec<HnswNode>,
    /// Map from external string ID → internal index.
    id_map: HashMap<String, usize>,
    /// Set of removed internal indices (tombstones).
    removed: HashSet<usize>,
    /// Index of the current entry point (node at highest level).
    entry_point: Option<usize>,
    /// Current maximum level across all live nodes.
    max_level: usize,
    /// Configuration parameters.
    config: HnswConfig,
    /// PRNG for level generation.
    rng: Xorshift64,
}

/// Configuration for the HNSW index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Max bi-directional connections per node per layer.
    pub m: usize,
    /// Max connections for layer 0 (typically 2*M, per paper recommendation).
    pub m_max0: usize,
    /// Beam width during construction.
    pub ef_construction: usize,
    /// Beam width during search.
    pub ef_search: usize,
    /// Level generation multiplier: 1/ln(M).
    pub ml: f64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            m,
            m_max0: 2 * m,
            ef_construction: 200,
            ef_search: 50,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

impl HnswConfig {
    /// Create config with custom M parameter, deriving m_max0 and ml.
    pub fn with_m(m: usize) -> Self {
        let m = if m < 2 { 2 } else { m };
        Self {
            m,
            m_max0: 2 * m,
            ef_construction: 200,
            ef_search: 50,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

impl HnswIndex {
    /// Create a new empty HNSW index with the given configuration.
    pub fn new(config: HnswConfig) -> Self {
        Self {
            nodes: Vec::new(),
            id_map: HashMap::new(),
            removed: HashSet::new(),
            entry_point: None,
            max_level: 0,
            config,
            rng: Xorshift64::new(),
        }
    }

    /// Create with a deterministic seed (for reproducible tests).
    pub fn with_seed(config: HnswConfig, seed: u64) -> Self {
        Self {
            nodes: Vec::new(),
            id_map: HashMap::new(),
            removed: HashSet::new(),
            entry_point: None,
            max_level: 0,
            config,
            rng: Xorshift64::from_seed(seed),
        }
    }

    /// Number of live (non-removed) vectors in the index.
    pub fn len(&self) -> usize {
        self.nodes.len() - self.removed.len()
    }

    /// Whether the index contains zero live vectors.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vector dimensionality (inferred from first node, or 0 if empty).
    pub fn dimensions(&self) -> usize {
        self.nodes
            .iter()
            .find(|n| !self.removed.contains(&self.id_map[&n.id]))
            .map(|n| n.vector.len())
            .unwrap_or(0)
    }

    // ── Random level selection (Section 4 of paper) ─────────

    fn random_level(&mut self) -> usize {
        let r = self.rng.next_f64();
        // Clamp to avoid -ln(0)
        let r = if r < 1e-15 { 1e-15 } else { r };
        let level = (-r.ln() * self.config.ml).floor() as usize;
        // Cap at a reasonable maximum to prevent degenerate graphs
        level.min(32)
    }

    // ── Insert ──────────────────────────────────────────────

    /// Insert a vector with the given string ID.
    ///
    /// If the ID already exists, its vector is updated in place.
    pub fn insert(&mut self, id: &str, vector: &[f32]) {
        // Handle duplicate: update vector, rebuild connections
        if let Some(&existing_idx) = self.id_map.get(id) {
            if !self.removed.contains(&existing_idx) {
                // Update vector in place
                self.nodes[existing_idx].vector = vector.to_vec();
                return;
            }
            // Was removed — un-remove and update
            self.removed.remove(&existing_idx);
            self.nodes[existing_idx].vector = vector.to_vec();
            // Reconnect at all levels
            let level = self.nodes[existing_idx].level;
            for l in 0..=level {
                self.nodes[existing_idx].connections[l].clear();
            }
            self.connect_node(existing_idx);
            return;
        }

        let new_level = self.random_level();
        let internal_idx = self.nodes.len();

        let node = HnswNode {
            id: id.to_string(),
            vector: vector.to_vec(),
            connections: vec![Vec::new(); new_level + 1],
            level: new_level,
        };

        self.nodes.push(node);
        self.id_map.insert(id.to_string(), internal_idx);

        if self.entry_point.is_none() {
            // First node
            self.entry_point = Some(internal_idx);
            self.max_level = new_level;
            return;
        }

        self.connect_node(internal_idx);

        // Update entry point if new node has higher level
        if new_level > self.max_level {
            self.entry_point = Some(internal_idx);
            self.max_level = new_level;
        }
    }

    /// Connect a newly inserted node into the graph at all its levels.
    fn connect_node(&mut self, new_idx: usize) {
        let ep = match self.entry_point {
            Some(ep) => ep,
            None => return,
        };

        let new_level = self.nodes[new_idx].level;
        let query = self.nodes[new_idx].vector.clone();
        let mut current_ep = ep;

        // Phase 1: Greedy descent from top level to new_level + 1
        let top = self.max_level;
        if top > new_level {
            for level in (new_level + 1..=top).rev() {
                current_ep = self.search_layer_single(&query, current_ep, level);
            }
        }

        // Phase 2: Insert at levels new_level down to 0
        for level in (0..=new_level.min(top)).rev() {
            let m_max = if level == 0 {
                self.config.m_max0
            } else {
                self.config.m
            };

            let candidates =
                self.search_layer(&query, current_ep, self.config.ef_construction, level);

            // Select M nearest neighbors from candidates
            let neighbors = self.select_neighbors(&query, &candidates, m_max);

            // Set connections for new node at this level
            self.nodes[new_idx].connections[level] = neighbors.clone();

            // Add bi-directional connections
            for &neighbor_idx in &neighbors {
                if self.removed.contains(&neighbor_idx) {
                    continue;
                }
                // Ensure neighbor has enough levels
                if level < self.nodes[neighbor_idx].connections.len() {
                    self.nodes[neighbor_idx].connections[level].push(new_idx);

                    // Prune if over capacity
                    let neighbor_connections =
                        self.nodes[neighbor_idx].connections[level].len();
                    if neighbor_connections > m_max {
                        let nv = self.nodes[neighbor_idx].vector.clone();
                        let old_conns: Vec<usize> =
                            self.nodes[neighbor_idx].connections[level].clone();
                        let pruned = self.select_neighbors(&nv, &old_conns, m_max);
                        self.nodes[neighbor_idx].connections[level] = pruned;
                    }
                }
            }

            // Use nearest candidate as entry point for next (lower) level
            if let Some(nearest) = candidates.first() {
                current_ep = *nearest;
            }
        }
    }

    // ── Search ──────────────────────────────────────────────

    /// k-NN search. Returns up to `k` results as `(id, distance)`,
    /// sorted by ascending distance (nearest first).
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if self.is_empty() || k == 0 {
            return Vec::new();
        }

        let ep = match self.entry_point {
            Some(ep) => ep,
            None => return Vec::new(),
        };

        let mut current_ep = ep;

        // Phase 1: Greedy descent from top level to level 1
        for level in (1..=self.max_level).rev() {
            current_ep = self.search_layer_single(query, current_ep, level);
        }

        // Phase 2: Search at layer 0 with ef_search beam width
        let ef = self.config.ef_search.max(k);
        let candidates = self.search_layer(query, current_ep, ef, 0);

        // Return top-k
        let mut results: Vec<(String, f32)> = candidates
            .iter()
            .filter(|&&idx| !self.removed.contains(&idx))
            .map(|&idx| {
                let dist = angular_distance(query, &self.nodes[idx].vector);
                (self.nodes[idx].id.clone(), dist)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Greedy search on a single layer, returning the single nearest node.
    fn search_layer_single(&self, query: &[f32], entry: usize, level: usize) -> usize {
        let mut current = entry;
        let mut current_dist = angular_distance(query, &self.nodes[current].vector);

        loop {
            let mut changed = false;
            let conns = if level < self.nodes[current].connections.len() {
                &self.nodes[current].connections[level]
            } else {
                break;
            };

            for &neighbor in conns {
                if self.removed.contains(&neighbor) {
                    continue;
                }
                let dist = angular_distance(query, &self.nodes[neighbor].vector);
                if dist < current_dist {
                    current = neighbor;
                    current_dist = dist;
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        current
    }

    /// Beam search on a single layer. Returns candidate indices sorted by
    /// ascending distance. This implements Algorithm 2 from the paper.
    fn search_layer(
        &self,
        query: &[f32],
        entry: usize,
        ef: usize,
        level: usize,
    ) -> Vec<usize> {
        let mut visited = HashSet::new();
        visited.insert(entry);

        let entry_dist = angular_distance(query, &self.nodes[entry].vector);

        // candidates: min-heap (nearest first for expansion)
        let mut candidates: BinaryHeap<MinHeapItem> = BinaryHeap::new();
        candidates.push(MinHeapItem(HeapItem {
            distance: entry_dist,
            index: entry,
        }));

        // results: max-heap (farthest first for pruning)
        let mut results: BinaryHeap<HeapItem> = BinaryHeap::new();
        results.push(HeapItem {
            distance: entry_dist,
            index: entry,
        });

        while let Some(MinHeapItem(current)) = candidates.pop() {
            // If the nearest candidate is farther than the farthest result, stop
            let farthest_result = results.peek().map(|h| h.distance).unwrap_or(f32::MAX);
            if current.distance > farthest_result {
                break;
            }

            let conns = if level < self.nodes[current.index].connections.len() {
                &self.nodes[current.index].connections[level]
            } else {
                continue;
            };

            for &neighbor in conns {
                if !visited.insert(neighbor) {
                    continue;
                }
                if self.removed.contains(&neighbor) {
                    continue;
                }

                let dist = angular_distance(query, &self.nodes[neighbor].vector);
                let farthest = results.peek().map(|h| h.distance).unwrap_or(f32::MAX);

                if dist < farthest || results.len() < ef {
                    candidates.push(MinHeapItem(HeapItem {
                        distance: dist,
                        index: neighbor,
                    }));
                    results.push(HeapItem {
                        distance: dist,
                        index: neighbor,
                    });
                    if results.len() > ef {
                        results.pop(); // remove farthest
                    }
                }
            }
        }

        // Collect and sort by distance
        let mut result_vec: Vec<(usize, f32)> =
            results.into_iter().map(|h| (h.index, h.distance)).collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result_vec.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Simple nearest-neighbor selection: pick the M closest candidates.
    /// (Heuristic neighbor selection from Section 4 of the paper.)
    fn select_neighbors(
        &self,
        query: &[f32],
        candidates: &[usize],
        m: usize,
    ) -> Vec<usize> {
        let mut scored: Vec<(usize, f32)> = candidates
            .iter()
            .filter(|&&idx| !self.removed.contains(&idx))
            .map(|&idx| (idx, angular_distance(query, &self.nodes[idx].vector)))
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        scored.truncate(m);
        scored.into_iter().map(|(idx, _)| idx).collect()
    }

    // ── Remove ──────────────────────────────────────────────

    /// Remove a vector by its string ID.
    ///
    /// Uses tombstone deletion: the node is marked as removed but its
    /// storage is retained to keep internal indices stable.
    pub fn remove(&mut self, id: &str) -> bool {
        let idx = match self.id_map.get(id) {
            Some(&idx) => idx,
            None => return false,
        };

        if self.removed.contains(&idx) {
            return false;
        }

        self.removed.insert(idx);

        // If we removed the entry point, find a new one
        if self.entry_point == Some(idx) {
            self.recompute_entry_point();
        }

        true
    }

    /// Find a new entry point after the current one is removed.
    fn recompute_entry_point(&mut self) {
        self.entry_point = None;
        self.max_level = 0;

        for (i, node) in self.nodes.iter().enumerate() {
            if self.removed.contains(&i) {
                continue;
            }
            if self.entry_point.is_none() || node.level > self.max_level {
                self.entry_point = Some(i);
                self.max_level = node.level;
            }
        }
    }

    // ── Brute-force search (for validation) ─────────────────

    /// Exact k-NN by brute force. Used in tests for correctness validation.
    pub fn brute_force_search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut results: Vec<(String, f32)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.removed.contains(i))
            .map(|(_, node)| {
                let dist = angular_distance(query, &node.vector);
                (node.id.clone(), dist)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    // ── Persistence ─────────────────────────────────────────

    /// Persist the index to SQLite.
    ///
    /// Creates two tables if they don't exist:
    ///   - `hnsw_nodes`: one row per node (id, vector blob, connections json, level)
    ///   - `hnsw_metadata`: config + entry point + max_level
    pub fn save_to_db(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hnsw_nodes (
                 id TEXT PRIMARY KEY,
                 vector BLOB NOT NULL,
                 connections TEXT NOT NULL,
                 level INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS hnsw_metadata (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;

        // Clear existing data (full snapshot)
        conn.execute("DELETE FROM hnsw_nodes", [])?;
        conn.execute("DELETE FROM hnsw_metadata", [])?;

        // Write nodes (skip tombstoned)
        let mut stmt = conn.prepare(
            "INSERT INTO hnsw_nodes (id, vector, connections, level) VALUES (?1, ?2, ?3, ?4)",
        )?;

        // Build a mapping: old internal index → new compacted index
        // This is necessary because we skip removed nodes.
        let mut old_to_new: HashMap<usize, usize> = HashMap::new();
        let mut new_idx = 0usize;
        for (old_idx, _node) in self.nodes.iter().enumerate() {
            if self.removed.contains(&old_idx) {
                continue;
            }
            old_to_new.insert(old_idx, new_idx);
            new_idx += 1;
        }

        for (old_idx, node) in self.nodes.iter().enumerate() {
            if self.removed.contains(&old_idx) {
                continue;
            }

            let vector_bytes: Vec<u8> = node
                .vector
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect();

            // Remap connection indices to compacted indices
            let remapped_connections: Vec<Vec<usize>> = node
                .connections
                .iter()
                .map(|level_conns| {
                    level_conns
                        .iter()
                        .filter_map(|&idx| old_to_new.get(&idx).copied())
                        .collect()
                })
                .collect();

            let connections_json = serde_json::to_string(&remapped_connections)
                .unwrap_or_else(|_| "[]".to_string());

            stmt.execute(params![node.id, vector_bytes, connections_json, node.level])?;
        }

        // Write metadata
        let config_json =
            serde_json::to_string(&self.config).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "INSERT INTO hnsw_metadata (key, value) VALUES ('config', ?1)",
            params![config_json],
        )?;

        // Store entry point by external ID (stable across compaction)
        let entry_id = self
            .entry_point
            .and_then(|idx| {
                if self.removed.contains(&idx) {
                    None
                } else {
                    Some(self.nodes[idx].id.clone())
                }
            })
            .unwrap_or_default();
        conn.execute(
            "INSERT INTO hnsw_metadata (key, value) VALUES ('entry_point_id', ?1)",
            params![entry_id],
        )?;

        let ml_str = self.max_level.to_string();
        conn.execute(
            "INSERT INTO hnsw_metadata (key, value) VALUES ('max_level', ?1)",
            params![ml_str],
        )?;

        let count_str = self.len().to_string();
        conn.execute(
            "INSERT INTO hnsw_metadata (key, value) VALUES ('node_count', ?1)",
            params![count_str],
        )?;

        Ok(())
    }

    /// Load the index from SQLite.
    ///
    /// Returns `None` if the tables don't exist or are empty.
    pub fn load_from_db(conn: &Connection) -> SqlResult<Option<Self>> {
        // Check if tables exist
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hnsw_nodes'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        if !table_exists {
            return Ok(None);
        }

        // Load config
        let config: HnswConfig = conn
            .query_row(
                "SELECT value FROM hnsw_metadata WHERE key = 'config'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let entry_point_id: String = conn
            .query_row(
                "SELECT value FROM hnsw_metadata WHERE key = 'entry_point_id'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();

        // Load nodes in insertion order (by rowid)
        let mut stmt =
            conn.prepare("SELECT id, vector, connections, level FROM hnsw_nodes ORDER BY rowid")?;

        let mut nodes: Vec<HnswNode> = Vec::new();
        let mut id_map: HashMap<String, usize> = HashMap::new();

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let vector_bytes: Vec<u8> = row.get(1)?;
            let connections_json: String = row.get(2)?;
            let level: usize = row.get::<_, i64>(3)? as usize;
            Ok((id, vector_bytes, connections_json, level))
        })?;

        for row_result in rows {
            let (id, vector_bytes, connections_json, level) = row_result?;

            let vector: Vec<f32> = vector_bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();

            let connections: Vec<Vec<usize>> =
                serde_json::from_str(&connections_json).unwrap_or_default();

            let internal_idx = nodes.len();
            id_map.insert(id.clone(), internal_idx);

            nodes.push(HnswNode {
                id,
                vector,
                connections,
                level,
            });
        }

        if nodes.is_empty() {
            return Ok(None);
        }

        // Resolve entry point
        let entry_point = if entry_point_id.is_empty() {
            None
        } else {
            id_map.get(&entry_point_id).copied()
        };

        let max_level = nodes
            .iter()
            .map(|n| n.level)
            .max()
            .unwrap_or(0);

        Ok(Some(Self {
            nodes,
            id_map,
            removed: HashSet::new(),
            entry_point,
            max_level,
            config,
            rng: Xorshift64::new(),
        }))
    }
}

// ── Thread-safe wrapper ─────────────────────────────────────────

/// Thread-safe HNSW index wrapper using `parking_lot::RwLock`.
///
/// Multiple readers can search concurrently; writers get exclusive access.
pub struct SharedHnswIndex {
    inner: RwLock<HnswIndex>,
}

impl SharedHnswIndex {
    pub fn new(config: HnswConfig) -> Self {
        Self {
            inner: RwLock::new(HnswIndex::new(config)),
        }
    }

    pub fn insert(&self, id: &str, vector: &[f32]) {
        self.inner.write().insert(id, vector);
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        self.inner.read().search(query, k)
    }

    pub fn remove(&self, id: &str) -> bool {
        self.inner.write().remove(id)
    }

    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    pub fn save_to_db(&self, conn: &Connection) -> SqlResult<()> {
        self.inner.read().save_to_db(conn)
    }

    pub fn load_from_db(conn: &Connection) -> SqlResult<Option<Self>> {
        match HnswIndex::load_from_db(conn)? {
            Some(index) => Ok(Some(Self {
                inner: RwLock::new(index),
            })),
            None => Ok(None),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a deterministic index for tests.
    fn test_index() -> HnswIndex {
        HnswIndex::with_seed(HnswConfig::default(), 42)
    }

    /// Helper: create an in-memory SQLite connection with WAL mode.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn
    }

    // ── Basic insert and search ─────────────────────────────

    #[test]
    fn test_insert_and_search_basic() {
        let mut index = test_index();

        index.insert("a", &[1.0, 0.0, 0.0]);
        index.insert("b", &[0.0, 1.0, 0.0]);
        index.insert("c", &[0.0, 0.0, 1.0]);

        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());

        // Search for something close to "a"
        let results = index.search(&[0.9, 0.1, 0.0], 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "a"); // "a" should be nearest
    }

    #[test]
    fn test_search_returns_k_results() {
        let mut index = test_index();

        index.insert("v1", &[1.0, 0.0]);
        index.insert("v2", &[0.0, 1.0]);
        index.insert("v3", &[-1.0, 0.0]);
        index.insert("v4", &[0.0, -1.0]);
        index.insert("v5", &[0.7, 0.7]);

        let results = index.search(&[1.0, 0.0], 3);
        assert_eq!(results.len(), 3);

        // Distances should be ascending
        for i in 1..results.len() {
            assert!(results[i].1 >= results[i - 1].1);
        }
    }

    // ── Empty index ─────────────────────────────────────────

    #[test]
    fn test_empty_index_search() {
        let index = test_index();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);

        let results = index.search(&[1.0, 0.0, 0.0], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_k_zero() {
        let mut index = test_index();
        index.insert("a", &[1.0, 0.0]);
        let results = index.search(&[1.0, 0.0], 0);
        assert!(results.is_empty());
    }

    // ── k-NN accuracy (brute force validation) ──────────────

    #[test]
    fn test_knn_accuracy_100_vectors() {
        let mut index = HnswIndex::with_seed(
            HnswConfig {
                ef_construction: 200,
                ef_search: 100,
                ..HnswConfig::default()
            },
            12345,
        );
        let dims = 32;
        let n = 100;

        // Generate deterministic pseudo-random vectors
        let mut rng = Xorshift64::from_seed(99999);
        let mut vectors: Vec<Vec<f32>> = Vec::new();

        for i in 0..n {
            let v: Vec<f32> = (0..dims)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();
            index.insert(&format!("vec_{i}"), &v);
            vectors.push(v);
        }

        assert_eq!(index.len(), n);

        // For several random queries, verify that the HNSW nearest neighbor
        // matches the brute-force nearest neighbor.
        let mut correct_nn1 = 0;
        let num_queries = 20;

        for _q in 0..num_queries {
            let query: Vec<f32> = (0..dims)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();

            let hnsw_results = index.search(&query, 1);
            let brute_results = index.brute_force_search(&query, 1);

            if !hnsw_results.is_empty()
                && !brute_results.is_empty()
                && hnsw_results[0].0 == brute_results[0].0
            {
                correct_nn1 += 1;
            }
        }

        // HNSW should achieve at least 80% recall@1 with ef_search=100
        let recall = correct_nn1 as f64 / num_queries as f64;
        assert!(
            recall >= 0.80,
            "HNSW recall@1 = {recall:.2}, expected >= 0.80"
        );
    }

    #[test]
    fn test_knn_top5_recall() {
        let mut index = HnswIndex::with_seed(
            HnswConfig {
                ef_construction: 200,
                ef_search: 100,
                ..HnswConfig::default()
            },
            77777,
        );
        let dims = 16;
        let n = 100;

        let mut rng = Xorshift64::from_seed(88888);
        for i in 0..n {
            let v: Vec<f32> = (0..dims)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();
            index.insert(&format!("v{i}"), &v);
        }

        let query: Vec<f32> = (0..dims)
            .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
            .collect();

        let hnsw_top5 = index.search(&query, 5);
        let brute_top5 = index.brute_force_search(&query, 5);

        let hnsw_ids: HashSet<&str> = hnsw_top5.iter().map(|(id, _)| id.as_str()).collect();
        let brute_ids: HashSet<&str> = brute_top5.iter().map(|(id, _)| id.as_str()).collect();

        let overlap = hnsw_ids.intersection(&brute_ids).count();
        let recall_at_5 = overlap as f64 / 5.0;
        assert!(
            recall_at_5 >= 0.60,
            "HNSW recall@5 = {recall_at_5:.2}, expected >= 0.60"
        );
    }

    // ── Remove and re-search ────────────────────────────────

    #[test]
    fn test_remove_and_search() {
        let mut index = test_index();

        index.insert("keep", &[1.0, 0.0, 0.0]);
        index.insert("remove_me", &[0.9, 0.1, 0.0]);
        index.insert("other", &[0.0, 1.0, 0.0]);

        assert_eq!(index.len(), 3);

        // Remove the closest vector to [1,0,0]
        assert!(index.remove("remove_me"));
        assert_eq!(index.len(), 2);

        // "remove_me" should no longer appear in results
        let results = index.search(&[0.95, 0.05, 0.0], 3);
        for (id, _) in &results {
            assert_ne!(id, "remove_me");
        }

        // Double-remove should return false
        assert!(!index.remove("remove_me"));
        // Remove non-existent should return false
        assert!(!index.remove("nonexistent"));
    }

    #[test]
    fn test_remove_entry_point() {
        let mut index = test_index();

        index.insert("first", &[1.0, 0.0]);
        index.insert("second", &[0.0, 1.0]);
        index.insert("third", &[-1.0, 0.0]);

        // Remove the entry point (whichever it is)
        let ep_id = index.nodes[index.entry_point.unwrap()].id.clone();
        assert!(index.remove(&ep_id));
        assert_eq!(index.len(), 2);

        // Index should still be searchable
        let results = index.search(&[0.5, 0.5], 2);
        assert!(!results.is_empty());
        for (id, _) in &results {
            assert_ne!(id, &ep_id);
        }
    }

    #[test]
    fn test_remove_all() {
        let mut index = test_index();

        index.insert("a", &[1.0, 0.0]);
        index.insert("b", &[0.0, 1.0]);

        index.remove("a");
        index.remove("b");

        assert_eq!(index.len(), 0);
        assert!(index.is_empty());

        let results = index.search(&[1.0, 0.0], 1);
        assert!(results.is_empty());
    }

    // ── Duplicate IDs ───────────────────────────────────────

    #[test]
    fn test_duplicate_id_updates_vector() {
        let mut index = test_index();

        index.insert("dup", &[1.0, 0.0, 0.0]);
        assert_eq!(index.len(), 1);

        // Insert again with different vector — should update, not add
        index.insert("dup", &[0.0, 1.0, 0.0]);
        assert_eq!(index.len(), 1);

        // Search should find the updated vector direction
        let results = index.search(&[0.0, 1.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "dup");
        // Distance should be 0 (identical vector)
        assert!(results[0].1 < 0.01, "Expected near-zero distance, got {}", results[0].1);
    }

    // ── Zero vectors ────────────────────────────────────────

    #[test]
    fn test_zero_vector() {
        let mut index = test_index();

        index.insert("zero", &[0.0, 0.0, 0.0]);
        index.insert("nonzero", &[1.0, 0.0, 0.0]);

        assert_eq!(index.len(), 2);

        // Searching with a zero vector should still work (returns dist=1.0 for all)
        let results = index.search(&[0.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
    }

    // ── Single element ──────────────────────────────────────

    #[test]
    fn test_single_element() {
        let mut index = test_index();

        index.insert("only", &[0.5, 0.5, 0.5]);
        assert_eq!(index.len(), 1);

        let results = index.search(&[1.0, 1.0, 1.0], 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "only");
    }

    // ── Persistence roundtrip ───────────────────────────────

    #[test]
    fn test_persistence_roundtrip() {
        let conn = test_db();

        // Build index
        let mut index = test_index();
        index.insert("alpha", &[1.0, 0.0, 0.0, 0.0]);
        index.insert("beta", &[0.0, 1.0, 0.0, 0.0]);
        index.insert("gamma", &[0.0, 0.0, 1.0, 0.0]);
        index.insert("delta", &[0.0, 0.0, 0.0, 1.0]);
        index.insert("epsilon", &[0.5, 0.5, 0.0, 0.0]);

        // Search before save
        let query = [0.9, 0.1, 0.0, 0.0];
        let results_before = index.search(&query, 3);

        // Save
        index.save_to_db(&conn).unwrap();

        // Load into new index
        let loaded = HnswIndex::load_from_db(&conn).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();

        // Verify count
        assert_eq!(loaded.len(), 5);

        // Search after load — results should be identical
        let results_after = loaded.search(&query, 3);

        assert_eq!(
            results_before.len(),
            results_after.len(),
            "Result count mismatch after reload"
        );

        for (before, after) in results_before.iter().zip(results_after.iter()) {
            assert_eq!(before.0, after.0, "ID mismatch: {} vs {}", before.0, after.0);
            assert!(
                (before.1 - after.1).abs() < 1e-5,
                "Distance mismatch for {}: {} vs {}",
                before.0,
                before.1,
                after.1
            );
        }
    }

    #[test]
    fn test_persistence_with_removals() {
        let conn = test_db();

        let mut index = test_index();
        index.insert("a", &[1.0, 0.0]);
        index.insert("b", &[0.0, 1.0]);
        index.insert("c", &[-1.0, 0.0]);

        // Remove one before saving
        index.remove("b");

        index.save_to_db(&conn).unwrap();

        let loaded = HnswIndex::load_from_db(&conn).unwrap().unwrap();
        assert_eq!(loaded.len(), 2);

        // "b" should not be in results
        let results = loaded.search(&[0.0, 1.0], 3);
        for (id, _) in &results {
            assert_ne!(id, "b");
        }
    }

    #[test]
    fn test_persistence_empty_index() {
        let conn = test_db();

        let index = test_index();
        index.save_to_db(&conn).unwrap();

        let loaded = HnswIndex::load_from_db(&conn).unwrap();
        assert!(loaded.is_none(), "Empty index should load as None");
    }

    #[test]
    fn test_load_nonexistent_tables() {
        let conn = test_db();
        let loaded = HnswIndex::load_from_db(&conn).unwrap();
        assert!(loaded.is_none());
    }

    // ── SharedHnswIndex (thread-safe wrapper) ───────────────

    #[test]
    fn test_shared_index_basic() {
        let shared = SharedHnswIndex::new(HnswConfig::default());

        shared.insert("x", &[1.0, 0.0, 0.0]);
        shared.insert("y", &[0.0, 1.0, 0.0]);

        assert_eq!(shared.len(), 2);

        let results = shared.search(&[0.9, 0.1, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "x");

        assert!(shared.remove("x"));
        assert_eq!(shared.len(), 1);
    }

    #[test]
    fn test_shared_index_persistence() {
        let conn = test_db();
        let shared = SharedHnswIndex::new(HnswConfig::default());

        shared.insert("p", &[1.0, 0.0]);
        shared.insert("q", &[0.0, 1.0]);

        shared.save_to_db(&conn).unwrap();

        let loaded = SharedHnswIndex::load_from_db(&conn).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().len(), 2);
    }

    // ── Cosine similarity unit tests ────────────────────────

    #[test]
    fn test_cosine_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-5);
    }

    #[test]
    fn test_angular_distance_identical() {
        let v = vec![1.0, 0.0];
        let dist = angular_distance(&v, &v);
        assert!(dist.abs() < 1e-5);
    }

    #[test]
    fn test_angular_distance_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let dist = angular_distance(&a, &b);
        assert!((dist - 2.0).abs() < 1e-5);
    }

    // ── Config ──────────────────────────────────────────────

    #[test]
    fn test_config_default() {
        let config = HnswConfig::default();
        assert_eq!(config.m, 16);
        assert_eq!(config.m_max0, 32);
        assert_eq!(config.ef_construction, 200);
        assert_eq!(config.ef_search, 50);
        assert!(config.ml > 0.0);
    }

    #[test]
    fn test_config_with_m() {
        let config = HnswConfig::with_m(8);
        assert_eq!(config.m, 8);
        assert_eq!(config.m_max0, 16);

        // M=1 should clamp to 2
        let config_min = HnswConfig::with_m(1);
        assert_eq!(config_min.m, 2);
    }

    // ── Larger-scale test ───────────────────────────────────

    #[test]
    fn test_500_vectors() {
        let mut index = HnswIndex::with_seed(HnswConfig::with_m(12), 54321);
        let dims = 64;
        let n = 500;

        let mut rng = Xorshift64::from_seed(11111);
        for i in 0..n {
            let v: Vec<f32> = (0..dims)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();
            index.insert(&format!("item_{i}"), &v);
        }

        assert_eq!(index.len(), n);

        // Search should return exactly k results
        let results = index.search(
            &(0..dims)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect::<Vec<_>>(),
            10,
        );
        assert_eq!(results.len(), 10);

        // Distances should be non-negative and ascending
        for (i, (_, dist)) in results.iter().enumerate() {
            assert!(*dist >= 0.0, "Distance should be non-negative");
            if i > 0 {
                assert!(
                    *dist >= results[i - 1].1 - 1e-7,
                    "Results should be sorted by ascending distance"
                );
            }
        }
    }
}
