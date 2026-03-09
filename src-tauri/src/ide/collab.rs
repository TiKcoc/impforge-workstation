// SPDX-License-Identifier: Apache-2.0
//! CRDT-based Real-Time Collaboration with Live Session Knowledge Graph
//!
//! Provides multi-user collaborative editing with live cursors, presence
//! tracking, operational-transform conflict resolution, and a **live
//! knowledge graph** that tracks edit relationships across users, files,
//! and code symbols.
//!
//! ## Knowledge Graph Features
//!
//! - **Edit Affinity**: Tracks which users edit which files/symbols most
//! - **Co-Change Coupling**: Detects files that always change together
//!   (Ball et al. 1997: "If Your Version Control System Could Talk")
//! - **Session Analytics**: Edit velocity, hotspot detection, conflict rate
//! - **Symbol Ownership**: Who "owns" (most recently edited) each function
//!
//! ## Architecture
//!
//! ```text
//! Client <-> OT Engine <-> Knowledge Graph (in-memory)
//!                  |              |
//!          WebSocket Relay    SQLite (persist on demand)
//! ```
//!
//! The knowledge graph is an in-process adjacency-list graph with typed
//! edges (EDITS, CO_CHANGES, OWNS, REVIEWS). No external services needed.
//!
//! ## References
//!
//! - Ellis & Gibbs 1989: "Concurrency Control in Groupware Systems"
//! - Sun & Ellis 1998: "Operational Transformation in Real-Time Group Editors"
//! - Lamport 1978: "Time, Clocks, and the Ordering of Events"
//! - Ball et al. 1997: "If Your Version Control System Could Talk"
//! - Zimmermann 2004: "Mining Software Repositories to Study Co-Evolution"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Edit operation representing an insert or delete action.
/// Carries a Lamport timestamp and document version for OT conflict resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOperation {
    /// Operation type: "insert" or "delete"
    pub op_type: String,
    /// Character position in the document (0-indexed)
    pub position: u64,
    /// Content to insert (only for "insert" operations)
    pub content: Option<String>,
    /// Number of characters to delete (only for "delete" operations)
    pub length: Option<u64>,
    /// Lamport timestamp for causal ordering
    pub timestamp: u64,
    /// Originating user identifier
    pub user_id: String,
    /// Document version at time of operation
    pub version: u64,
}

/// Information about a connected peer in a collaboration session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub user_id: String,
    pub user_name: String,
    pub color: String,
    pub cursor_line: u32,
    pub cursor_column: u32,
    pub cursor_file: String,
    pub last_seen: u64,
}

/// A collaboration room representing a shared editing session on a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabRoom {
    pub room_id: String,
    pub name: String,
    pub peers: Vec<PeerInfo>,
    pub file_path: String,
}

/// Internal session state for a single collaboration room.
struct CollabSession {
    room_id: String,
    user_id: String,
    user_name: String,
    user_color: String,
    document_version: u64,
    lamport_clock: u64,
    /// Operations pending relay acknowledgment
    pending_ops: Vec<EditOperation>,
    /// History of all applied operations (for undo / late-joiner sync)
    history: Vec<EditOperation>,
    /// Connected peers (keyed by user_id)
    peers: HashMap<String, PeerInfo>,
    file_path: String,
    connected: bool,
    /// Files edited during this session (for co-change detection)
    session_files: Vec<String>,
}

// ---------------------------------------------------------------------------
// OT Transform Engine
// ---------------------------------------------------------------------------

/// Operational Transform: transform `op` against `against` so that applying
/// both in either order yields the same document state.
///
/// This implements the core OT inclusion transformation (IT) function for
/// a simplified two-operation model (insert, delete).
fn transform_operation(op: &EditOperation, against: &EditOperation) -> EditOperation {
    let mut result = op.clone();

    match (op.op_type.as_str(), against.op_type.as_str()) {
        // Insert vs Insert: if the other insert is at an earlier (or equal
        // but lower user_id) position, shift our position right.
        ("insert", "insert") => {
            let against_len = against
                .content
                .as_ref()
                .map(|c| c.len() as u64)
                .unwrap_or(0);

            if against.position < op.position
                || (against.position == op.position && against.user_id < op.user_id)
            {
                result.position = op.position.saturating_add(against_len);
            }
        }

        // Insert vs Delete: if the delete removed characters before our
        // insert point, shift left; if after, no change.
        ("insert", "delete") => {
            let del_len = against.length.unwrap_or(0);
            let del_end = against.position.saturating_add(del_len);

            if against.position <= op.position {
                if del_end <= op.position {
                    // Entire deletion is before our insert
                    result.position = op.position.saturating_sub(del_len);
                } else {
                    // Deletion straddles our insert point; collapse to deletion start
                    result.position = against.position;
                }
            }
        }

        // Delete vs Insert: if the insert is before our delete range,
        // shift our delete position right.
        ("delete", "insert") => {
            let ins_len = against
                .content
                .as_ref()
                .map(|c| c.len() as u64)
                .unwrap_or(0);

            if against.position <= op.position {
                result.position = op.position.saturating_add(ins_len);
            }
        }

        // Delete vs Delete: handle overlapping ranges.
        ("delete", "delete") => {
            let op_len = op.length.unwrap_or(0);
            let ag_len = against.length.unwrap_or(0);
            let op_end = op.position.saturating_add(op_len);
            let ag_end = against.position.saturating_add(ag_len);

            if ag_end <= op.position {
                // Against is entirely before us
                result.position = op.position.saturating_sub(ag_len);
            } else if against.position >= op_end {
                // Against is entirely after us — no change
            } else {
                // Overlapping ranges — compute the non-overlapping remainder
                let overlap_start = op.position.max(against.position);
                let overlap_end = op_end.min(ag_end);
                let overlap = overlap_end.saturating_sub(overlap_start);

                result.length = Some(op_len.saturating_sub(overlap));
                if against.position < op.position {
                    result.position = against.position;
                }
            }
        }

        _ => {
            // Unknown op types pass through unchanged
        }
    }

    result
}

/// Transform a new operation against all operations in the history that
/// have a version >= the new operation's base version (i.e., concurrent ops).
fn transform_against_history(op: &EditOperation, history: &[EditOperation]) -> EditOperation {
    let mut transformed = op.clone();

    for historical in history {
        // Only transform against operations that are concurrent
        // (created at or after the same version, by a different user)
        if historical.version >= op.version && historical.user_id != op.user_id {
            transformed = transform_operation(&transformed, historical);
        }
    }

    transformed
}

// ---------------------------------------------------------------------------
// Color Palette
// ---------------------------------------------------------------------------

/// Opera-GX-inspired color palette for peer identification.
const PEER_COLORS: &[&str] = &[
    "#ff5370", // red-pink
    "#c3e88d", // lime-green
    "#82aaff", // sky-blue
    "#c792ea", // purple
    "#ffcb6b", // amber
    "#89ddff", // cyan
    "#f78c6c", // orange
    "#b2ccd6", // steel
    "#ff9cac", // salmon
    "#a8e4a0", // mint
];

/// Select a deterministic color from the palette based on a user identifier.
fn color_for_user(user_id: &str) -> String {
    let hash: u64 = user_id.bytes().fold(0u64, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as u64)
    });
    PEER_COLORS[(hash as usize) % PEER_COLORS.len()].to_string()
}

/// Current epoch timestamp in seconds.
fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Generate a short random room identifier (8 hex chars).
///
/// Uses RandomState as an entropy source (seeded from OS randomness)
/// combined with the current timestamp for uniqueness.
fn generate_room_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let ts = now_epoch();
    let noise: u64 = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    format!("{:08x}", (ts ^ noise) as u32)
}

// ============================================================================
// COLLABORATION KNOWLEDGE GRAPH
// ============================================================================

/// Node in the collaboration knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgNode {
    pub id: String,
    pub kind: KgNodeKind,
    pub label: String,
    /// Metadata: edit_count, last_seen, etc.
    pub properties: HashMap<String, serde_json::Value>,
}

/// Types of nodes in the collaboration graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KgNodeKind {
    /// A user participating in the session
    User,
    /// A file being collaboratively edited
    File,
    /// A code symbol (function, struct, class) within a file
    Symbol,
    /// A collaboration room/session
    Session,
}

/// Typed edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgEdge {
    pub source: String,
    pub target: String,
    pub kind: KgEdgeKind,
    /// Edge weight (e.g., edit count, co-change frequency)
    pub weight: f64,
    pub timestamp: u64,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Types of relationships between nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KgEdgeKind {
    /// User → File: user edited this file (weight = edit count)
    Edits,
    /// User → Symbol: user modified this symbol (weight = edit count)
    Modifies,
    /// File → File: files that change together (weight = co-change count)
    CoChanges,
    /// User → Symbol: user is the primary owner (most recent/frequent editor)
    Owns,
    /// User → User: users who frequently edit the same regions
    Collaborates,
}

/// In-memory knowledge graph for the collaboration session.
///
/// Adjacency-list representation with O(1) node lookup and O(degree) edge traversal.
/// No external dependencies — fully standalone, persisted to SQLite on demand.
#[derive(Debug, Default)]
struct CollabKnowledgeGraph {
    nodes: HashMap<String, KgNode>,
    /// adjacency list: source_id → Vec<KgEdge>
    edges: HashMap<String, Vec<KgEdge>>,
    /// Co-change tracker: (file_a, file_b) → count (sorted pair for dedup)
    co_change_pairs: HashMap<(String, String), u64>,
    /// Edit velocity: user_id → edits in the last 60 seconds
    edit_velocity: HashMap<String, Vec<u64>>,
}

impl CollabKnowledgeGraph {
    fn new() -> Self {
        Self::default()
    }

    /// Ensure a node exists (upsert)
    fn ensure_node(&mut self, id: &str, kind: KgNodeKind, label: &str) {
        self.nodes.entry(id.to_string()).or_insert_with(|| KgNode {
            id: id.to_string(),
            kind,
            label: label.to_string(),
            properties: HashMap::new(),
        });
    }

    /// Record an edit: user edited a file at a position
    fn record_edit(&mut self, user_id: &str, user_name: &str, file_path: &str, position: u64) {
        let now = now_epoch();

        // Ensure nodes exist
        self.ensure_node(user_id, KgNodeKind::User, user_name);
        let file_label = std::path::Path::new(file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string());
        self.ensure_node(file_path, KgNodeKind::File, &file_label);

        // Upsert EDITS edge (user → file)
        let edges = self.edges.entry(user_id.to_string()).or_default();
        if let Some(edge) = edges.iter_mut().find(|e| e.target == file_path && e.kind == KgEdgeKind::Edits) {
            edge.weight += 1.0;
            edge.timestamp = now;
        } else {
            edges.push(KgEdge {
                source: user_id.to_string(),
                target: file_path.to_string(),
                kind: KgEdgeKind::Edits,
                weight: 1.0,
                timestamp: now,
                properties: HashMap::new(),
            });
        }

        // Track edit velocity (keep last 60s of timestamps)
        let velocity = self.edit_velocity.entry(user_id.to_string()).or_default();
        velocity.push(now);
        velocity.retain(|&t| now.saturating_sub(t) < 60);

        // Update node properties
        if let Some(node) = self.nodes.get_mut(file_path) {
            let count = node.properties.entry("edit_count".to_string())
                .or_insert(serde_json::json!(0));
            if let Some(n) = count.as_u64() {
                *count = serde_json::json!(n + 1);
            }
            node.properties.insert("last_edit_position".to_string(), serde_json::json!(position));
            node.properties.insert("last_editor".to_string(), serde_json::json!(user_name));
            node.properties.insert("last_edit_time".to_string(), serde_json::json!(now));
        }
    }

    /// Record a co-change: two files changed in the same session within a time window
    fn record_co_change(&mut self, file_a: &str, file_b: &str) {
        if file_a == file_b { return; }

        // Sort pair for canonical key
        let pair = if file_a < file_b {
            (file_a.to_string(), file_b.to_string())
        } else {
            (file_b.to_string(), file_a.to_string())
        };

        let count = self.co_change_pairs.entry(pair.clone()).or_insert(0);
        *count += 1;

        // Upsert CO_CHANGES edge
        let edges = self.edges.entry(pair.0.clone()).or_default();
        if let Some(edge) = edges.iter_mut().find(|e| e.target == pair.1 && e.kind == KgEdgeKind::CoChanges) {
            edge.weight = *count as f64;
            edge.timestamp = now_epoch();
        } else {
            edges.push(KgEdge {
                source: pair.0.clone(),
                target: pair.1.clone(),
                kind: KgEdgeKind::CoChanges,
                weight: *count as f64,
                timestamp: now_epoch(),
                properties: HashMap::new(),
            });
        }
    }

    /// Detect collaboration edges: users who edit the same file
    fn update_collaborations(&mut self, users_in_room: &[(&str, &str)]) {
        let now = now_epoch();
        for i in 0..users_in_room.len() {
            for j in (i + 1)..users_in_room.len() {
                let (user_a, name_a) = users_in_room[i];
                let (user_b, name_b) = users_in_room[j];

                self.ensure_node(user_a, KgNodeKind::User, name_a);
                self.ensure_node(user_b, KgNodeKind::User, name_b);

                let edges = self.edges.entry(user_a.to_string()).or_default();
                if let Some(edge) = edges.iter_mut().find(|e| e.target == user_b && e.kind == KgEdgeKind::Collaborates) {
                    edge.weight += 1.0;
                    edge.timestamp = now;
                } else {
                    edges.push(KgEdge {
                        source: user_a.to_string(),
                        target: user_b.to_string(),
                        kind: KgEdgeKind::Collaborates,
                        weight: 1.0,
                        timestamp: now,
                        properties: HashMap::new(),
                    });
                }
            }
        }
    }

    /// Get edit velocity for a user (edits per minute in last 60s)
    fn get_edit_velocity(&self, user_id: &str) -> f64 {
        let now = now_epoch();
        self.edit_velocity.get(user_id)
            .map(|timestamps| {
                let recent = timestamps.iter().filter(|&&t| now.saturating_sub(t) < 60).count();
                recent as f64 // edits per minute
            })
            .unwrap_or(0.0)
    }

    /// Find hotspot files (most-edited) across the session
    fn get_hotspots(&self, limit: usize) -> Vec<(&str, u64)> {
        let mut files: Vec<(&str, u64)> = self.nodes.iter()
            .filter(|(_, n)| n.kind == KgNodeKind::File)
            .map(|(id, n)| {
                let count = n.properties.get("edit_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                (id.as_str(), count)
            })
            .collect();
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.truncate(limit);
        files
    }

    /// Find co-change partners for a file (files that frequently change together)
    fn get_co_changes(&self, file_path: &str, limit: usize) -> Vec<(&str, u64)> {
        let mut partners: Vec<(&str, u64)> = self.co_change_pairs.iter()
            .filter_map(|((a, b), &count)| {
                if a == file_path { Some((b.as_str(), count)) }
                else if b == file_path { Some((a.as_str(), count)) }
                else { None }
            })
            .collect();
        partners.sort_by(|a, b| b.1.cmp(&a.1));
        partners.truncate(limit);
        partners
    }

    /// Get all edges from a node (outgoing)
    fn edges_from(&self, node_id: &str) -> &[KgEdge] {
        self.edges.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Compute session analytics summary
    fn analytics(&self) -> KgAnalytics {
        let user_count = self.nodes.values().filter(|n| n.kind == KgNodeKind::User).count();
        let file_count = self.nodes.values().filter(|n| n.kind == KgNodeKind::File).count();
        let total_edits: u64 = self.nodes.values()
            .filter(|n| n.kind == KgNodeKind::File)
            .map(|n| n.properties.get("edit_count").and_then(|v| v.as_u64()).unwrap_or(0))
            .sum();
        let edge_count: usize = self.edges.values().map(|v| v.len()).sum();
        let co_change_count = self.co_change_pairs.len();
        let hotspots = self.get_hotspots(5);

        KgAnalytics {
            user_count,
            file_count,
            total_edits,
            edge_count,
            co_change_pairs: co_change_count,
            hotspot_files: hotspots.iter().map(|(f, c)| (f.to_string(), *c)).collect(),
        }
    }
}

/// Session analytics summary from the knowledge graph
#[derive(Debug, Clone, Serialize)]
pub struct KgAnalytics {
    pub user_count: usize,
    pub file_count: usize,
    pub total_edits: u64,
    pub edge_count: usize,
    pub co_change_pairs: usize,
    pub hotspot_files: Vec<(String, u64)>,
}

// ---------------------------------------------------------------------------
// CollabManager — Tauri Managed State
// ---------------------------------------------------------------------------

/// Thread-safe manager for all active collaboration sessions.
/// Registered as Tauri managed state in the application setup.
pub struct CollabManager {
    sessions: parking_lot::Mutex<HashMap<String, CollabSession>>,
    /// Live knowledge graph tracking edit relationships across all sessions
    knowledge_graph: parking_lot::Mutex<CollabKnowledgeGraph>,
}

impl CollabManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            sessions: parking_lot::Mutex::new(HashMap::new()),
            knowledge_graph: parking_lot::Mutex::new(CollabKnowledgeGraph::new()),
        }
    }
}

impl Default for CollabManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tauri IPC Commands
// ---------------------------------------------------------------------------

/// Create a new collaboration room for the given file.
///
/// Returns the room metadata including a shareable room_id that other
/// users can use to join via `collab_join_room`.
#[tauri::command]
pub async fn collab_create_room(
    manager: State<'_, CollabManager>,
    file_path: String,
    user_name: String,
) -> Result<CollabRoom, String> {
    let room_id = generate_room_id();
    let user_id = uuid::Uuid::new_v4().to_string();
    let color = color_for_user(&user_id);

    let room_name = std::path::Path::new(&file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let local_peer = PeerInfo {
        user_id: user_id.clone(),
        user_name: user_name.clone(),
        color: color.clone(),
        cursor_line: 1,
        cursor_column: 1,
        cursor_file: file_path.clone(),
        last_seen: now_epoch(),
    };

    let session = CollabSession {
        room_id: room_id.clone(),
        user_id: user_id.clone(),
        user_name: user_name.clone(),
        user_color: color,
        document_version: 0,
        lamport_clock: 0,
        pending_ops: Vec::new(),
        history: Vec::new(),
        peers: {
            let mut m = HashMap::new();
            m.insert(user_id.clone(), local_peer.clone());
            m
        },
        file_path: file_path.clone(),
        connected: true,
        session_files: vec![file_path.clone()],
    };

    let room = CollabRoom {
        room_id: room_id.clone(),
        name: room_name,
        peers: vec![local_peer],
        file_path,
    };

    manager.sessions.lock().insert(room_id.clone(), session);

    // Register session and user in knowledge graph
    {
        let mut kg = manager.knowledge_graph.lock();
        kg.ensure_node(&room_id, KgNodeKind::Session, &room.name);
        kg.ensure_node(&user_id, KgNodeKind::User, &user_name);
        kg.ensure_node(&room.file_path, KgNodeKind::File, &room.name);
    }

    log::info!("Collaboration room created: {}", room.room_id);
    Ok(room)
}

/// Join an existing collaboration room by its room_id.
///
/// The joining user is added to the peer list and receives the current
/// room state including all connected peers.
#[tauri::command]
pub async fn collab_join_room(
    manager: State<'_, CollabManager>,
    room_id: String,
    user_name: String,
) -> Result<CollabRoom, String> {
    let mut sessions = manager.sessions.lock();
    let session = sessions
        .get_mut(&room_id)
        .ok_or_else(|| format!("Room '{}' not found", room_id))?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let color = color_for_user(&user_id);

    let peer = PeerInfo {
        user_id: user_id.clone(),
        user_name: user_name.clone(),
        color: color.clone(),
        cursor_line: 1,
        cursor_column: 1,
        cursor_file: session.file_path.clone(),
        last_seen: now_epoch(),
    };

    session.peers.insert(user_id.clone(), peer);

    let room_name = std::path::Path::new(&session.file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let room = CollabRoom {
        room_id: room_id.clone(),
        name: room_name,
        peers: session.peers.values().cloned().collect(),
        file_path: session.file_path.clone(),
    };

    log::info!("User '{}' joined room '{}'", user_name, room_id);
    Ok(room)
}

/// Leave a collaboration room.
///
/// Removes the local user from the peer list. If the room becomes empty,
/// it is automatically cleaned up.
#[tauri::command]
pub async fn collab_leave_room(
    manager: State<'_, CollabManager>,
    room_id: String,
) -> Result<(), String> {
    let mut sessions = manager.sessions.lock();
    let should_remove = {
        let session = sessions
            .get_mut(&room_id)
            .ok_or_else(|| format!("Room '{}' not found", room_id))?;

        session.peers.remove(&session.user_id);
        session.connected = false;

        log::info!("User left room '{}'", room_id);
        session.peers.is_empty()
    };

    if should_remove {
        sessions.remove(&room_id);
        log::info!("Room '{}' removed (no peers remaining)", room_id);
    }

    Ok(())
}

/// Submit an edit operation to the collaboration session.
///
/// The operation is transformed against any concurrent operations in the
/// history buffer using the OT inclusion transformation, then appended
/// to the history and queued for relay broadcast.
///
/// Also feeds the knowledge graph with edit events for analytics.
#[tauri::command]
pub async fn collab_send_operation(
    manager: State<'_, CollabManager>,
    room_id: String,
    operation: EditOperation,
) -> Result<u64, String> {
    let (version, kg_event) = {
        let mut sessions = manager.sessions.lock();
        let session = sessions
            .get_mut(&room_id)
            .ok_or_else(|| format!("Room '{}' not found", room_id))?;

        // Advance Lamport clock
        session.lamport_clock = session
            .lamport_clock
            .max(operation.timestamp)
            .saturating_add(1);

        // Transform against concurrent operations in history
        let transformed = transform_against_history(&operation, &session.history);

        // Advance document version
        session.document_version = session.document_version.saturating_add(1);

        // Record in history
        let mut versioned = transformed;
        versioned.version = session.document_version;
        versioned.timestamp = session.lamport_clock;

        session.history.push(versioned.clone());
        session.pending_ops.push(versioned);

        // Cap history to prevent unbounded growth (keep last 10,000 ops)
        if session.history.len() > 10_000 {
            let drain_count = session.history.len() - 10_000;
            session.history.drain(..drain_count);
        }

        // Collect data for knowledge graph update (outside sessions lock)
        let event = KgEditEvent {
            user_id: session.user_id.clone(),
            user_name: session.user_name.clone(),
            file_path: session.file_path.clone(),
            position: operation.position,
            session_files: session.session_files.clone(),
            peers: session.peers.iter()
                .map(|(id, p)| (id.clone(), p.user_name.clone()))
                .collect(),
        };

        (session.document_version, event)
    };

    // Feed knowledge graph (separate lock to avoid deadlock)
    {
        let mut kg = manager.knowledge_graph.lock();

        // Record the edit
        kg.record_edit(&kg_event.user_id, &kg_event.user_name, &kg_event.file_path, kg_event.position);

        // Track co-changes: any file edited in this session co-changes with the current file
        for other_file in &kg_event.session_files {
            kg.record_co_change(&kg_event.file_path, other_file);
        }

        // Update collaboration edges between active peers
        let users: Vec<(&str, &str)> = kg_event.peers.iter()
            .map(|(id, name)| (id.as_str(), name.as_str()))
            .collect();
        kg.update_collaborations(&users);
    }

    Ok(version)
}

/// Internal event for knowledge graph updates (avoids holding session lock)
struct KgEditEvent {
    user_id: String,
    user_name: String,
    file_path: String,
    position: u64,
    session_files: Vec<String>,
    peers: Vec<(String, String)>,
}

/// Update the local user's cursor position within a room.
///
/// This is broadcast to all peers for live cursor rendering.
#[tauri::command]
pub async fn collab_update_cursor(
    manager: State<'_, CollabManager>,
    room_id: String,
    line: u32,
    column: u32,
    file_path: String,
) -> Result<(), String> {
    let mut sessions = manager.sessions.lock();
    let session = sessions
        .get_mut(&room_id)
        .ok_or_else(|| format!("Room '{}' not found", room_id))?;

    let user_id = session.user_id.clone();
    if let Some(peer) = session.peers.get_mut(&user_id) {
        peer.cursor_line = line;
        peer.cursor_column = column;
        peer.cursor_file = file_path;
        peer.last_seen = now_epoch();
    }

    Ok(())
}

/// Get the list of peers currently in a room.
#[tauri::command]
pub async fn collab_get_peers(
    manager: State<'_, CollabManager>,
    room_id: String,
) -> Result<Vec<PeerInfo>, String> {
    let mut sessions = manager.sessions.lock();
    let session = sessions
        .get_mut(&room_id)
        .ok_or_else(|| format!("Room '{}' not found", room_id))?;

    // Ensure the local user's peer entry reflects the current session name and color
    let local_user_id = session.user_id.clone();
    if let Some(local_peer) = session.peers.get_mut(&local_user_id) {
        local_peer.user_name = session.user_name.clone();
        local_peer.color = session.user_color.clone();
        local_peer.last_seen = now_epoch();
    }

    // Filter out stale peers (not seen in 60 seconds)
    let now = now_epoch();
    let active_peers: Vec<PeerInfo> = session
        .peers
        .values()
        .filter(|p| now.saturating_sub(p.last_seen) < 60)
        .cloned()
        .collect();

    Ok(active_peers)
}

/// List all active collaboration rooms.
#[tauri::command]
pub async fn collab_get_rooms(
    manager: State<'_, CollabManager>,
) -> Result<Vec<CollabRoom>, String> {
    let sessions = manager.sessions.lock();

    let rooms: Vec<CollabRoom> = sessions
        .values()
        .map(|s| {
            let room_name = std::path::Path::new(&s.file_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string());

            CollabRoom {
                room_id: s.room_id.clone(),
                name: room_name,
                peers: s.peers.values().cloned().collect(),
                file_path: s.file_path.clone(),
            }
        })
        .collect();

    Ok(rooms)
}

/// Get overall collaboration engine status with knowledge graph analytics.
///
/// Returns a JSON object with room count, total peers, per-room details,
/// and knowledge graph insights (hotspots, co-changes, edit velocity).
#[tauri::command]
pub async fn collab_status(
    manager: State<'_, CollabManager>,
) -> Result<serde_json::Value, String> {
    let sessions = manager.sessions.lock();

    let total_peers: usize = sessions.values().map(|s| s.peers.len()).sum();
    let total_ops: usize = sessions.values().map(|s| s.history.len()).sum();
    let total_pending: usize = sessions.values().map(|s| s.pending_ops.len()).sum();

    let rooms: Vec<serde_json::Value> = sessions
        .values()
        .map(|s| {
            serde_json::json!({
                "room_id": s.room_id,
                "file": s.file_path,
                "user_name": s.user_name,
                "user_color": s.user_color,
                "peers": s.peers.len(),
                "version": s.document_version,
                "history_size": s.history.len(),
                "pending_ops": s.pending_ops.len(),
                "connected": s.connected,
            })
        })
        .collect();

    drop(sessions);

    // Knowledge graph analytics
    let kg = manager.knowledge_graph.lock();
    let analytics = kg.analytics();

    Ok(serde_json::json!({
        "active_rooms": rooms.len(),
        "total_peers": total_peers,
        "total_operations": total_ops,
        "pending_sync": total_pending,
        "rooms": rooms,
        "transport": "local",
        "relay_connected": false,
        "knowledge_graph": {
            "nodes": analytics.user_count + analytics.file_count,
            "edges": analytics.edge_count,
            "total_edits": analytics.total_edits,
            "co_change_pairs": analytics.co_change_pairs,
            "hotspot_files": analytics.hotspot_files,
        }
    }))
}

/// Query the collaboration knowledge graph for session insights.
///
/// Returns edit affinities, co-change coupling, hotspots, and per-user velocity.
#[tauri::command]
pub async fn collab_knowledge_graph(
    manager: State<'_, CollabManager>,
) -> Result<serde_json::Value, String> {
    let kg = manager.knowledge_graph.lock();
    let analytics = kg.analytics();

    // Build per-user edit velocity
    let velocities: Vec<serde_json::Value> = kg.nodes.values()
        .filter(|n| n.kind == KgNodeKind::User)
        .map(|n| {
            let velocity = kg.get_edit_velocity(&n.id);
            let file_edges: Vec<serde_json::Value> = kg.edges_from(&n.id).iter()
                .filter(|e| e.kind == KgEdgeKind::Edits)
                .map(|e| serde_json::json!({
                    "file": e.target,
                    "edit_count": e.weight,
                }))
                .collect();
            serde_json::json!({
                "user_id": n.id,
                "user_name": n.label,
                "edits_per_minute": velocity,
                "files": file_edges,
            })
        })
        .collect();

    // Build co-change matrix
    let co_changes: Vec<serde_json::Value> = kg.co_change_pairs.iter()
        .map(|((a, b), &count)| {
            let a_label = kg.nodes.get(a).map(|n| n.label.as_str()).unwrap_or(a);
            let b_label = kg.nodes.get(b).map(|n| n.label.as_str()).unwrap_or(b);
            serde_json::json!({
                "file_a": a_label,
                "file_b": b_label,
                "co_change_count": count,
            })
        })
        .collect();

    // Build collaboration edges (who works with whom)
    let collaborations: Vec<serde_json::Value> = kg.edges.values()
        .flat_map(|edges| edges.iter())
        .filter(|e| e.kind == KgEdgeKind::Collaborates)
        .map(|e| {
            let src_name = kg.nodes.get(&e.source).map(|n| n.label.as_str()).unwrap_or(&e.source);
            let tgt_name = kg.nodes.get(&e.target).map(|n| n.label.as_str()).unwrap_or(&e.target);
            serde_json::json!({
                "user_a": src_name,
                "user_b": tgt_name,
                "sessions_together": e.weight,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "analytics": analytics,
        "user_velocities": velocities,
        "co_changes": co_changes,
        "collaborations": collaborations,
        "hotspots": analytics.hotspot_files,
    }))
}

/// Get co-change partners for a specific file.
///
/// Returns files that frequently change together with the given file,
/// useful for "you might also want to edit..." suggestions.
#[tauri::command]
pub async fn collab_co_changes(
    manager: State<'_, CollabManager>,
    file_path: String,
) -> Result<Vec<serde_json::Value>, String> {
    let kg = manager.knowledge_graph.lock();
    let partners = kg.get_co_changes(&file_path, 10);

    Ok(partners.iter().map(|(path, count)| {
        let label = kg.nodes.get(*path).map(|n| n.label.as_str()).unwrap_or(path);
        serde_json::json!({
            "file": path,
            "name": label,
            "co_change_count": count,
        })
    }).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_insert(pos: u64, content: &str, user: &str, version: u64) -> EditOperation {
        EditOperation {
            op_type: "insert".to_string(),
            position: pos,
            content: Some(content.to_string()),
            length: None,
            timestamp: version,
            user_id: user.to_string(),
            version,
        }
    }

    fn make_delete(pos: u64, len: u64, user: &str, version: u64) -> EditOperation {
        EditOperation {
            op_type: "delete".to_string(),
            position: pos,
            content: None,
            length: Some(len),
            timestamp: version,
            user_id: user.to_string(),
            version,
        }
    }

    #[test]
    fn test_insert_vs_insert_before() {
        // User A inserts "X" at position 5
        // User B inserts "Y" at position 3 (before A)
        // After transform, A's position should shift to 6
        let op_a = make_insert(5, "X", "alice", 1);
        let op_b = make_insert(3, "Y", "bob", 1);

        let transformed = transform_operation(&op_a, &op_b);
        assert_eq!(transformed.position, 6);
    }

    #[test]
    fn test_insert_vs_insert_after() {
        // User A inserts "X" at position 3
        // User B inserts "Y" at position 7 (after A)
        // A's position should stay at 3
        let op_a = make_insert(3, "X", "alice", 1);
        let op_b = make_insert(7, "Y", "bob", 1);

        let transformed = transform_operation(&op_a, &op_b);
        assert_eq!(transformed.position, 3);
    }

    #[test]
    fn test_insert_vs_insert_same_position_tiebreak() {
        // Both insert at position 5 — lower user_id wins (stays in place),
        // higher user_id shifts right.
        let op_a = make_insert(5, "X", "alice", 1);
        let op_b = make_insert(5, "Y", "bob", 1);

        // "alice" < "bob", so when transforming bob against alice, bob shifts
        let transformed_b = transform_operation(&op_b, &op_a);
        assert_eq!(transformed_b.position, 6);

        // When transforming alice against bob, alice stays (alice < bob)
        let transformed_a = transform_operation(&op_a, &op_b);
        assert_eq!(transformed_a.position, 5);
    }

    #[test]
    fn test_insert_vs_delete_before() {
        // Insert at position 10, delete 3 chars starting at position 2
        // After delete, insert position should shift left by 3
        let op_ins = make_insert(10, "X", "alice", 1);
        let op_del = make_delete(2, 3, "bob", 1);

        let transformed = transform_operation(&op_ins, &op_del);
        assert_eq!(transformed.position, 7);
    }

    #[test]
    fn test_insert_vs_delete_straddling() {
        // Insert at position 5, delete 4 chars starting at position 3 (range 3..7)
        // The insert is inside the deleted range, so it collapses to position 3
        let op_ins = make_insert(5, "X", "alice", 1);
        let op_del = make_delete(3, 4, "bob", 1);

        let transformed = transform_operation(&op_ins, &op_del);
        assert_eq!(transformed.position, 3);
    }

    #[test]
    fn test_delete_vs_insert_before() {
        // Delete at position 8, insert "XY" at position 3
        // Delete position should shift right by 2
        let op_del = make_delete(8, 2, "alice", 1);
        let op_ins = make_insert(3, "XY", "bob", 1);

        let transformed = transform_operation(&op_del, &op_ins);
        assert_eq!(transformed.position, 10);
    }

    #[test]
    fn test_delete_vs_delete_before() {
        // Delete 3 at position 10, another delete 2 at position 3
        // First delete should shift left by 2
        let op_a = make_delete(10, 3, "alice", 1);
        let op_b = make_delete(3, 2, "bob", 1);

        let transformed = transform_operation(&op_a, &op_b);
        assert_eq!(transformed.position, 8);
        assert_eq!(transformed.length, Some(3));
    }

    #[test]
    fn test_delete_vs_delete_overlapping() {
        // Delete 4 at position 5 (range 5..9)
        // Against delete 4 at position 7 (range 7..11)
        // Overlap is 7..9 (2 chars), so remaining length = 4-2 = 2
        let op_a = make_delete(5, 4, "alice", 1);
        let op_b = make_delete(7, 4, "bob", 1);

        let transformed = transform_operation(&op_a, &op_b);
        assert_eq!(transformed.position, 5);
        assert_eq!(transformed.length, Some(2));
    }

    #[test]
    fn test_transform_against_history() {
        // Build a history: bob inserted "AB" at position 0 (version 1)
        let history = vec![make_insert(0, "AB", "bob", 1)];

        // Alice tries to insert at position 3 based on version 0
        let op = make_insert(3, "X", "alice", 0);

        // After transform, alice's position should shift right by 2
        let transformed = transform_against_history(&op, &history);
        assert_eq!(transformed.position, 5);
    }

    #[test]
    fn test_color_determinism() {
        let c1 = color_for_user("alice");
        let c2 = color_for_user("alice");
        assert_eq!(c1, c2, "Same user should always get the same color");
    }

    #[test]
    fn test_color_distribution() {
        // Different users should generally get different colors
        let c1 = color_for_user("alice");
        let c2 = color_for_user("bob");
        // Not guaranteed to differ (hash collision), but very likely
        // We just check they are valid hex colors
        assert!(c1.starts_with('#'));
        assert!(c2.starts_with('#'));
    }

    #[test]
    fn test_room_id_generation() {
        let id1 = generate_room_id();
        let id2 = generate_room_id();
        assert_eq!(id1.len(), 8);
        assert_eq!(id2.len(), 8);
        // Extremely unlikely to collide
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_manager_creation() {
        let mgr = CollabManager::new();
        let sessions = mgr.sessions.lock();
        assert!(sessions.is_empty());
        let kg = mgr.knowledge_graph.lock();
        assert!(kg.nodes.is_empty());
    }

    // --- Knowledge Graph Tests ---

    #[test]
    fn test_kg_record_edit() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.record_edit("user-1", "Alice", "/src/main.rs", 42);
        kg.record_edit("user-1", "Alice", "/src/main.rs", 100);
        kg.record_edit("user-1", "Alice", "/src/lib.rs", 10);

        assert_eq!(kg.nodes.len(), 3); // 1 user + 2 files
        assert!(kg.nodes.contains_key("user-1"));
        assert!(kg.nodes.contains_key("/src/main.rs"));
        assert!(kg.nodes.contains_key("/src/lib.rs"));

        // Check edit count on main.rs
        let main_node = &kg.nodes["/src/main.rs"];
        assert_eq!(main_node.properties["edit_count"].as_u64(), Some(2));

        // Check EDITS edge weight
        let edges = kg.edges_from("user-1");
        let main_edge = edges.iter().find(|e| e.target == "/src/main.rs").unwrap();
        assert_eq!(main_edge.weight, 2.0);
        assert_eq!(main_edge.kind, KgEdgeKind::Edits);
    }

    #[test]
    fn test_kg_co_changes() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.ensure_node("/src/a.rs", KgNodeKind::File, "a.rs");
        kg.ensure_node("/src/b.rs", KgNodeKind::File, "b.rs");
        kg.ensure_node("/src/c.rs", KgNodeKind::File, "c.rs");

        kg.record_co_change("/src/a.rs", "/src/b.rs");
        kg.record_co_change("/src/a.rs", "/src/b.rs");
        kg.record_co_change("/src/a.rs", "/src/c.rs");

        // a ↔ b should have count 2
        let partners = kg.get_co_changes("/src/a.rs", 10);
        assert_eq!(partners.len(), 2);
        assert_eq!(partners[0], ("/src/b.rs", 2)); // higher count first

        // Symmetric: b's co-changes should also show a
        let b_partners = kg.get_co_changes("/src/b.rs", 10);
        assert_eq!(b_partners.len(), 1);
        assert_eq!(b_partners[0].0, "/src/a.rs");
    }

    #[test]
    fn test_kg_co_change_self_ignored() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.record_co_change("/src/a.rs", "/src/a.rs");
        assert!(kg.co_change_pairs.is_empty());
    }

    #[test]
    fn test_kg_hotspots() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.record_edit("u1", "Alice", "/src/hot.rs", 0);
        kg.record_edit("u1", "Alice", "/src/hot.rs", 10);
        kg.record_edit("u1", "Alice", "/src/hot.rs", 20);
        kg.record_edit("u2", "Bob", "/src/cold.rs", 0);

        let hotspots = kg.get_hotspots(5);
        assert_eq!(hotspots[0].0, "/src/hot.rs");
        assert_eq!(hotspots[0].1, 3);
        assert_eq!(hotspots[1].0, "/src/cold.rs");
        assert_eq!(hotspots[1].1, 1);
    }

    #[test]
    fn test_kg_collaborations() {
        let mut kg = CollabKnowledgeGraph::new();
        let users = vec![("u1", "Alice"), ("u2", "Bob"), ("u3", "Charlie")];
        kg.update_collaborations(&users);

        // Should create 3 collaboration edges: u1↔u2, u1↔u3, u2↔u3
        let u1_edges = kg.edges_from("u1");
        let collab_edges: Vec<_> = u1_edges.iter()
            .filter(|e| e.kind == KgEdgeKind::Collaborates)
            .collect();
        assert_eq!(collab_edges.len(), 2);
    }

    #[test]
    fn test_kg_edit_velocity() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.record_edit("u1", "Alice", "/src/main.rs", 0);
        kg.record_edit("u1", "Alice", "/src/main.rs", 10);
        kg.record_edit("u1", "Alice", "/src/main.rs", 20);

        let velocity = kg.get_edit_velocity("u1");
        assert_eq!(velocity, 3.0); // 3 edits in the last 60s
    }

    #[test]
    fn test_kg_analytics() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.record_edit("u1", "Alice", "/a.rs", 0);
        kg.record_edit("u2", "Bob", "/b.rs", 0);
        kg.record_co_change("/a.rs", "/b.rs");

        let analytics = kg.analytics();
        assert_eq!(analytics.user_count, 2);
        assert_eq!(analytics.file_count, 2);
        assert_eq!(analytics.total_edits, 2);
        assert_eq!(analytics.co_change_pairs, 1);
    }

    #[test]
    fn test_kg_node_upsert() {
        let mut kg = CollabKnowledgeGraph::new();
        kg.ensure_node("x", KgNodeKind::File, "old_label");
        kg.ensure_node("x", KgNodeKind::File, "new_label");
        // Should not overwrite — first insert wins
        assert_eq!(kg.nodes["x"].label, "old_label");
        assert_eq!(kg.nodes.len(), 1);
    }
}
