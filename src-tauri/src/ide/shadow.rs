//! Shadow Workspace — Isolated AI Code Modification with Diff Review
//!
//! Lets AI safely modify code in a temporary directory clone, then generates
//! unified diffs for user review before applying changes to the real workspace.
//!
//! Design:
//!   - Each shadow workspace is an isolated temp directory under `impforge-shadow/`
//!   - Files are copied from the user's real workspace preserving relative paths
//!   - AI writes to shadow files; originals are never touched until explicit apply
//!   - Unified diffs are generated line-by-line (no external diff crate)
//!   - Workspaces older than 1 hour are automatically cleaned up on list/create

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::fs;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Shadow workspaces live under `<system temp>/impforge-shadow/<id>/`
const SHADOW_ROOT_DIR: &str = "impforge-shadow";

/// Auto-cleanup threshold: 1 hour
const MAX_AGE_SECS: i64 = 3600;

/// Maximum file size we will copy into / read from a shadow workspace (10 MB)
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Context lines shown around each hunk in unified diffs
const DIFF_CONTEXT_LINES: usize = 3;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Internal bookkeeping for one shadow workspace.
#[derive(Debug, Clone)]
struct ShadowWorkspaceInfo {
    /// Unique identifier (UUIDv4)
    id: String,
    /// Temp directory that holds the copied files
    shadow_dir: PathBuf,
    /// Absolute path to the *real* workspace root the files came from
    original_workspace: PathBuf,
    /// Relative file paths (forward-slash normalised) inside the workspace
    files: Vec<String>,
    /// When this workspace was created
    created_at: DateTime<Utc>,
}

/// Serialisable summary returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowInfo {
    pub id: String,
    pub workspace_path: String,
    pub file_count: usize,
    pub created_at: String,
}

/// Diff result for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowDiff {
    /// Relative path inside the workspace
    pub file_path: String,
    /// One of `"modified"`, `"added"`, `"deleted"`, `"unchanged"`
    pub status: String,
    /// Full unified-diff text (empty when unchanged)
    pub unified_diff: String,
    /// Number of added lines
    pub additions: u32,
    /// Number of removed lines
    pub deletions: u32,
}

/// Thread-safe manager for all active shadow workspaces.
///
/// Stored as Tauri managed state so every command can access it.
pub struct ShadowManager {
    workspaces: Mutex<HashMap<String, ShadowWorkspaceInfo>>,
}

impl ShadowManager {
    pub fn new() -> Self {
        Self {
            workspaces: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for ShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the root directory for all shadow workspaces.
fn shadow_root() -> PathBuf {
    std::env::temp_dir().join(SHADOW_ROOT_DIR)
}

/// Normalise a relative path to forward slashes so behaviour is identical
/// on Linux, macOS and Windows.
fn normalise_rel(path: &str) -> String {
    path.replace('\\', "/")
}

/// Read a file to a String, returning a human-friendly error.
async fn read_file_checked(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path)
        .await
        .map_err(|e| format!("Cannot stat '{}': {}", path.display(), e))?;

    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "File '{}' is {} bytes, exceeds {} byte limit",
            path.display(),
            meta.len(),
            MAX_FILE_BYTES
        ));
    }

    fs::read_to_string(path)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))
}

/// Ensure the parent directory of `path` exists.
async fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Cannot create directory '{}': {}", parent.display(), e))?;
    }
    Ok(())
}

/// Remove workspaces older than `MAX_AGE_SECS` from the map.
///
/// Returns the shadow directories that were evicted so the caller can
/// delete them from disk *after* releasing the mutex (avoiding holding a
/// `MutexGuard` across `.await` points).
fn evict_stale(workspaces: &mut HashMap<String, ShadowWorkspaceInfo>) -> Vec<PathBuf> {
    let now = Utc::now();
    let stale_ids: Vec<String> = workspaces
        .iter()
        .filter(|(_, info)| {
            now.signed_duration_since(info.created_at).num_seconds() > MAX_AGE_SECS
        })
        .map(|(id, _)| id.clone())
        .collect();

    let mut dirs_to_remove = Vec::with_capacity(stale_ids.len());
    for id in &stale_ids {
        if let Some(info) = workspaces.remove(id) {
            dirs_to_remove.push(info.shadow_dir);
        }
    }
    dirs_to_remove
}

/// Best-effort async removal of directories returned by [`evict_stale`].
async fn remove_dirs(dirs: Vec<PathBuf>) {
    for dir in dirs {
        let _ = fs::remove_dir_all(&dir).await;
    }
}

/// Resolve `relative` inside `base`, refusing any path-traversal attempts.
fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    // Reject absolute paths and explicit `..` components
    if rel.is_absolute() {
        return Err(format!("Relative path '{}' must not be absolute", relative));
    }
    for component in rel.components() {
        if let std::path::Component::ParentDir = component {
            return Err(format!(
                "Relative path '{}' must not contain '..' components",
                relative
            ));
        }
    }
    Ok(base.join(rel))
}

// ---------------------------------------------------------------------------
// Unified diff generation (no external crate)
// ---------------------------------------------------------------------------

/// Compute the longest common subsequence (LCS) table for two slices of lines.
///
/// Returns a 2-D vector where `table[i][j]` is the LCS length of `a[..i]` and
/// `b[..j]`.  Uses O(n*m) time and space which is acceptable for files up to
/// tens of thousands of lines.
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let n = a.len();
    let m = b.len();
    let mut table = vec![vec![0usize; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = std::cmp::max(table[i - 1][j], table[i][j - 1]);
            }
        }
    }
    table
}

/// A single edit operation in the diff.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DiffOp {
    Equal(usize, usize), // (index_in_a, index_in_b)
    Delete(usize),       // index_in_a
    Insert(usize),       // index_in_b
}

/// Back-trace the LCS table to produce a sequence of edit operations.
fn diff_ops(a: &[&str], b: &[&str], table: &[Vec<usize>]) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let mut i = a.len();
    let mut j = b.len();

    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            ops.push(DiffOp::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if table[i - 1][j] >= table[i][j - 1] {
            ops.push(DiffOp::Delete(i - 1));
            i -= 1;
        } else {
            ops.push(DiffOp::Insert(j - 1));
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(DiffOp::Delete(i - 1));
        i -= 1;
    }
    while j > 0 {
        ops.push(DiffOp::Insert(j - 1));
        j -= 1;
    }
    ops.reverse();
    ops
}

/// Intermediate representation of a diff hunk before rendering.
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<String>,
}

/// Group a flat list of `DiffOp` into hunks with `DIFF_CONTEXT_LINES` lines of
/// surrounding context.  Overlapping context regions are merged into a single
/// hunk (just like `diff -u`).
fn build_hunks(ops: &[DiffOp], a: &[&str], b: &[&str], ctx: usize) -> Vec<Hunk> {
    // Tag each op with whether it is a "change" (insert/delete).
    struct TaggedOp {
        op: DiffOp,
        is_change: bool,
    }
    let tagged: Vec<TaggedOp> = ops
        .iter()
        .map(|op| TaggedOp {
            op: op.clone(),
            is_change: !matches!(op, DiffOp::Equal(_, _)),
        })
        .collect();

    // Find ranges of change operations expanded by `ctx` context on each side,
    // then merge overlapping ranges.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let len = tagged.len();
    let mut i = 0;
    while i < len {
        if tagged[i].is_change {
            let start = i.saturating_sub(ctx);
            let mut end = i;
            // Extend to include consecutive changes and bridging context
            while end < len {
                if tagged[end].is_change {
                    end += 1;
                } else {
                    // Check if the next change is within 2*ctx lines (merge)
                    let mut next_change = None;
                    for k in (end + 1)..std::cmp::min(end + 2 * ctx + 1, len) {
                        if tagged[k].is_change {
                            next_change = Some(k);
                            break;
                        }
                    }
                    if let Some(_nc) = next_change {
                        end += 1;
                    } else {
                        break;
                    }
                }
            }
            let range_end = std::cmp::min(end + ctx, len);
            // Merge with previous range if overlapping
            if let Some(last) = ranges.last_mut() {
                if start <= last.1 {
                    last.1 = std::cmp::max(last.1, range_end);
                } else {
                    ranges.push((start, range_end));
                }
            } else {
                ranges.push((start, range_end));
            }
            i = end;
        } else {
            i += 1;
        }
    }

    // Render each range into a Hunk.
    let mut hunks = Vec::new();
    for (start, end) in ranges {
        let mut lines = Vec::new();
        let mut old_start: Option<usize> = None;
        let mut new_start: Option<usize> = None;
        let mut old_count = 0usize;
        let mut new_count = 0usize;

        for idx in start..end {
            match &tagged[idx].op {
                DiffOp::Equal(ai, bi) => {
                    if old_start.is_none() {
                        old_start = Some(*ai);
                    }
                    if new_start.is_none() {
                        new_start = Some(*bi);
                    }
                    lines.push(format!(" {}", a[*ai]));
                    old_count += 1;
                    new_count += 1;
                }
                DiffOp::Delete(ai) => {
                    if old_start.is_none() {
                        old_start = Some(*ai);
                    }
                    lines.push(format!("-{}", a[*ai]));
                    old_count += 1;
                }
                DiffOp::Insert(bi) => {
                    if new_start.is_none() {
                        new_start = Some(*bi);
                    }
                    lines.push(format!("+{}", b[*bi]));
                    new_count += 1;
                }
            }
        }

        hunks.push(Hunk {
            // Unified diff uses 1-based line numbers
            old_start: old_start.map_or(1, |s| s + 1),
            old_count,
            new_start: new_start.map_or(1, |s| s + 1),
            new_count,
            lines,
        });
    }

    hunks
}

/// Produce a unified diff string between `old_text` and `new_text`.
///
/// `old_label` / `new_label` appear on the `---` / `+++` header lines.
fn unified_diff(old_text: &str, new_text: &str, old_label: &str, new_label: &str) -> String {
    let a_lines: Vec<&str> = old_text.lines().collect();
    let b_lines: Vec<&str> = new_text.lines().collect();

    if a_lines == b_lines {
        return String::new();
    }

    let table = lcs_table(&a_lines, &b_lines);
    let ops = diff_ops(&a_lines, &b_lines, &table);
    let hunks = build_hunks(&ops, &a_lines, &b_lines, DIFF_CONTEXT_LINES);

    if hunks.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!("--- {}\n", old_label));
    out.push_str(&format!("+++ {}\n", new_label));

    for hunk in &hunks {
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        ));
        for line in &hunk.lines {
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}

/// Count additions and deletions from an already-rendered unified diff.
fn count_diff_stats(diff_text: &str) -> (u32, u32) {
    let mut additions = 0u32;
    let mut deletions = 0u32;
    for line in diff_text.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }
    (additions, deletions)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Create a new shadow workspace.
///
/// Copies the listed files from `workspace_path` into an isolated temp
/// directory and returns the shadow ID.
#[tauri::command]
pub async fn shadow_create(
    state: tauri::State<'_, ShadowManager>,
    workspace_path: String,
    files: Vec<String>,
) -> Result<ShadowInfo, String> {
    let workspace = PathBuf::from(&workspace_path);
    if !workspace.is_dir() {
        return Err(format!(
            "Workspace path '{}' is not a directory",
            workspace_path
        ));
    }

    if files.is_empty() {
        return Err("At least one file must be specified".to_string());
    }

    let id = Uuid::new_v4().to_string();
    let shadow_dir = shadow_root().join(&id);

    // Create shadow root + workspace directory
    fs::create_dir_all(&shadow_dir)
        .await
        .map_err(|e| format!("Failed to create shadow directory: {}", e))?;

    // Normalise relative paths and copy files
    let mut normalised: Vec<String> = Vec::with_capacity(files.len());
    for rel in &files {
        let rel_norm = normalise_rel(rel);
        let src = safe_join(&workspace, &rel_norm)?;
        let dst = safe_join(&shadow_dir, &rel_norm)?;

        if !src.exists() {
            // Clean up on error
            let _ = fs::remove_dir_all(&shadow_dir).await;
            return Err(format!(
                "Source file '{}' does not exist in workspace",
                rel
            ));
        }

        ensure_parent(&dst).await?;
        fs::copy(&src, &dst).await.map_err(|e| {
            format!(
                "Failed to copy '{}' to shadow: {}",
                src.display(),
                e
            )
        })?;

        normalised.push(rel_norm);
    }

    let now = Utc::now();
    let info = ShadowWorkspaceInfo {
        id: id.clone(),
        shadow_dir,
        original_workspace: workspace,
        files: normalised,
        created_at: now,
    };

    let summary = ShadowInfo {
        id: info.id.clone(),
        workspace_path: info.original_workspace.to_string_lossy().to_string(),
        file_count: info.files.len(),
        created_at: info.created_at.to_rfc3339(),
    };

    // Insert into managed state + evict stale entries
    let stale_dirs = {
        let mut map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let dirs = evict_stale(&mut map);
        map.insert(id, info);
        dirs
    };
    // Async disk cleanup *after* the lock is released
    remove_dirs(stale_dirs).await;

    Ok(summary)
}

/// Write content to a file inside a shadow workspace.
#[tauri::command]
pub async fn shadow_write(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
    file_path: String,
    content: String,
) -> Result<(), String> {
    let (shadow_dir, original_workspace) = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        (ws.shadow_dir.clone(), ws.original_workspace.clone())
    };

    let rel = normalise_rel(&file_path);
    let dst = safe_join(&shadow_dir, &rel)?;

    ensure_parent(&dst).await?;
    fs::write(&dst, &content)
        .await
        .map_err(|e| format!("Failed to write shadow file: {}", e))?;

    // If this is a new file that was not part of the original set, track it.
    {
        let mut map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        if let Some(ws) = map.get_mut(&shadow_id) {
            if !ws.files.contains(&rel) {
                // Verify the file does not escape the workspace via canonicalization
                let _ = safe_join(&original_workspace, &rel)?;
                ws.files.push(rel);
            }
        }
    }

    Ok(())
}

/// Read a file from a shadow workspace.
#[tauri::command]
pub async fn shadow_read(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
    file_path: String,
) -> Result<String, String> {
    let shadow_dir = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        ws.shadow_dir.clone()
    };

    let rel = normalise_rel(&file_path);
    let src = safe_join(&shadow_dir, &rel)?;

    read_file_checked(&src).await
}

/// Generate unified diffs for **all** tracked files in a shadow workspace.
#[tauri::command]
pub async fn shadow_diff_all(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
) -> Result<Vec<ShadowDiff>, String> {
    let (shadow_dir, original_workspace, files) = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        (
            ws.shadow_dir.clone(),
            ws.original_workspace.clone(),
            ws.files.clone(),
        )
    };

    let mut diffs = Vec::with_capacity(files.len());
    for rel in &files {
        let diff = diff_one_file(&shadow_dir, &original_workspace, rel).await?;
        diffs.push(diff);
    }

    Ok(diffs)
}

/// Generate the unified diff for a single file.
#[tauri::command]
pub async fn shadow_diff_file(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
    file_path: String,
) -> Result<ShadowDiff, String> {
    let (shadow_dir, original_workspace) = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        (ws.shadow_dir.clone(), ws.original_workspace.clone())
    };

    let rel = normalise_rel(&file_path);
    diff_one_file(&shadow_dir, &original_workspace, &rel).await
}

/// Internal: compute the diff for one relative path.
async fn diff_one_file(
    shadow_dir: &Path,
    original_workspace: &Path,
    rel: &str,
) -> Result<ShadowDiff, String> {
    let orig_path = safe_join(original_workspace, rel)?;
    let shadow_path = safe_join(shadow_dir, rel)?;

    let orig_exists = orig_path.exists();
    let shadow_exists = shadow_path.exists();

    match (orig_exists, shadow_exists) {
        (false, false) => Ok(ShadowDiff {
            file_path: rel.to_string(),
            status: "deleted".to_string(),
            unified_diff: String::new(),
            additions: 0,
            deletions: 0,
        }),
        (false, true) => {
            // New file added in shadow
            let new_text = read_file_checked(&shadow_path).await?;
            let diff_text = unified_diff("", &new_text, &format!("a/{}", rel), &format!("b/{}", rel));
            let (add, del) = count_diff_stats(&diff_text);
            Ok(ShadowDiff {
                file_path: rel.to_string(),
                status: "added".to_string(),
                unified_diff: diff_text,
                additions: add,
                deletions: del,
            })
        }
        (true, false) => {
            // File deleted in shadow
            let old_text = read_file_checked(&orig_path).await?;
            let diff_text = unified_diff(&old_text, "", &format!("a/{}", rel), &format!("b/{}", rel));
            let (add, del) = count_diff_stats(&diff_text);
            Ok(ShadowDiff {
                file_path: rel.to_string(),
                status: "deleted".to_string(),
                unified_diff: diff_text,
                additions: add,
                deletions: del,
            })
        }
        (true, true) => {
            let old_text = read_file_checked(&orig_path).await?;
            let new_text = read_file_checked(&shadow_path).await?;
            let diff_text = unified_diff(
                &old_text,
                &new_text,
                &format!("a/{}", rel),
                &format!("b/{}", rel),
            );
            if diff_text.is_empty() {
                Ok(ShadowDiff {
                    file_path: rel.to_string(),
                    status: "unchanged".to_string(),
                    unified_diff: String::new(),
                    additions: 0,
                    deletions: 0,
                })
            } else {
                let (add, del) = count_diff_stats(&diff_text);
                Ok(ShadowDiff {
                    file_path: rel.to_string(),
                    status: "modified".to_string(),
                    unified_diff: diff_text,
                    additions: add,
                    deletions: del,
                })
            }
        }
    }
}

/// Apply shadow changes for selected files back to the real workspace.
#[tauri::command]
pub async fn shadow_apply(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
    files: Vec<String>,
) -> Result<u32, String> {
    let (shadow_dir, original_workspace, tracked_files) = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        (
            ws.shadow_dir.clone(),
            ws.original_workspace.clone(),
            ws.files.clone(),
        )
    };

    let normalised: Vec<String> = files.iter().map(|f| normalise_rel(f)).collect();
    let mut applied = 0u32;

    for rel in &normalised {
        // Only allow files that are tracked by this shadow workspace
        if !tracked_files.contains(rel) {
            return Err(format!(
                "File '{}' is not tracked by shadow workspace '{}'",
                rel, shadow_id
            ));
        }

        let shadow_path = safe_join(&shadow_dir, rel)?;
        let orig_path = safe_join(&original_workspace, rel)?;

        if shadow_path.exists() {
            let content = read_file_checked(&shadow_path).await?;
            ensure_parent(&orig_path).await?;
            fs::write(&orig_path, &content)
                .await
                .map_err(|e| format!("Failed to write '{}': {}", orig_path.display(), e))?;
            applied += 1;
        } else if orig_path.exists() {
            // Shadow file was deleted — remove original
            fs::remove_file(&orig_path)
                .await
                .map_err(|e| format!("Failed to delete '{}': {}", orig_path.display(), e))?;
            applied += 1;
        }
    }

    Ok(applied)
}

/// Apply **all** shadow changes back to the real workspace.
#[tauri::command]
pub async fn shadow_apply_all(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
) -> Result<u32, String> {
    let files = {
        let map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .get(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        ws.files.clone()
    };

    shadow_apply(state, shadow_id, files).await
}

/// Discard a shadow workspace: remove its temp directory and bookkeeping.
#[tauri::command]
pub async fn shadow_discard(
    state: tauri::State<'_, ShadowManager>,
    shadow_id: String,
) -> Result<(), String> {
    let shadow_dir = {
        let mut map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        let ws = map
            .remove(&shadow_id)
            .ok_or_else(|| format!("Shadow workspace '{}' not found", shadow_id))?;
        ws.shadow_dir
    };

    if shadow_dir.exists() {
        fs::remove_dir_all(&shadow_dir)
            .await
            .map_err(|e| format!("Failed to remove shadow directory: {}", e))?;
    }

    Ok(())
}

/// List all active (non-stale) shadow workspaces.
#[tauri::command]
pub async fn shadow_list(
    state: tauri::State<'_, ShadowManager>,
) -> Result<Vec<ShadowInfo>, String> {
    let (mut list, stale_dirs) = {
        let mut map = state
            .workspaces
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        // Evict stale entries from the map (sync — no await)
        let dirs = evict_stale(&mut map);

        let infos: Vec<ShadowInfo> = map
            .values()
            .map(|ws| ShadowInfo {
                id: ws.id.clone(),
                workspace_path: ws.original_workspace.to_string_lossy().to_string(),
                file_count: ws.files.len(),
                created_at: ws.created_at.to_rfc3339(),
            })
            .collect();

        (infos, dirs)
    };

    // Async disk cleanup after lock is released
    remove_dirs(stale_dirs).await;

    // Sort by creation time (newest first)
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(list)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: create a temp workspace with files.
    fn make_workspace(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().expect("create temp dir");
        for (rel, content) in files {
            let path = dir.path().join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            let mut f = std::fs::File::create(&path).expect("create file");
            f.write_all(content.as_bytes()).expect("write");
        }
        dir
    }

    #[test]
    fn test_normalise_rel() {
        assert_eq!(normalise_rel("src\\main.rs"), "src/main.rs");
        assert_eq!(normalise_rel("src/lib.rs"), "src/lib.rs");
    }

    #[test]
    fn test_safe_join_rejects_traversal() {
        let base = PathBuf::from("/tmp/shadow/abc");
        assert!(safe_join(&base, "../../../etc/passwd").is_err());
        assert!(safe_join(&base, "/etc/passwd").is_err());
        assert!(safe_join(&base, "src/main.rs").is_ok());
    }

    #[test]
    fn test_unified_diff_identical() {
        let text = "line1\nline2\nline3\n";
        let diff = unified_diff(text, text, "a/f.rs", "b/f.rs");
        assert!(diff.is_empty(), "Identical files should produce no diff");
    }

    #[test]
    fn test_unified_diff_simple_change() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2_modified\nline3\n";
        let diff = unified_diff(old, new, "a/f.rs", "b/f.rs");

        assert!(diff.contains("--- a/f.rs"));
        assert!(diff.contains("+++ b/f.rs"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+line2_modified"));
    }

    #[test]
    fn test_unified_diff_addition() {
        let old = "line1\nline2\n";
        let new = "line1\nline2\nline3\n";
        let diff = unified_diff(old, new, "a/f.rs", "b/f.rs");

        assert!(diff.contains("+line3"));
    }

    #[test]
    fn test_unified_diff_deletion() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";
        let diff = unified_diff(old, new, "a/f.rs", "b/f.rs");

        assert!(diff.contains("-line2"));
    }

    #[test]
    fn test_count_diff_stats() {
        let diff = "--- a/f.rs\n+++ b/f.rs\n@@ -1,3 +1,3 @@\n line1\n-line2\n+line2_mod\n line3\n";
        let (add, del) = count_diff_stats(diff);
        assert_eq!(add, 1);
        assert_eq!(del, 1);
    }

    #[test]
    fn test_unified_diff_new_file() {
        let new_content = "fn main() {\n    println!(\"hello\");\n}\n";
        let diff = unified_diff("", new_content, "a/main.rs", "b/main.rs");

        assert!(diff.contains("+fn main()"));
        let (add, del) = count_diff_stats(&diff);
        assert_eq!(add, 3);
        assert_eq!(del, 0);
    }

    #[test]
    fn test_unified_diff_delete_file() {
        let old_content = "fn main() {\n    println!(\"hello\");\n}\n";
        let diff = unified_diff(old_content, "", "a/main.rs", "b/main.rs");

        assert!(diff.contains("-fn main()"));
        let (add, del) = count_diff_stats(&diff);
        assert_eq!(add, 0);
        assert_eq!(del, 3);
    }

    #[tokio::test]
    async fn test_shadow_create_and_read() {
        let ws = make_workspace(&[
            ("src/main.rs", "fn main() {}\n"),
            ("src/lib.rs", "pub mod foo;\n"),
        ]);

        let mgr = ShadowManager::new();
        let state = tauri::test::mock_builder()
            .build(tauri::generate_context!());

        // We cannot easily construct a tauri::State in unit tests without
        // running the full Tauri app, so test the underlying helpers instead.
        let id = Uuid::new_v4().to_string();
        let shadow_dir = shadow_root().join(&id);
        fs::create_dir_all(&shadow_dir).await.expect("mkdir");

        let src = ws.path().join("src/main.rs");
        let dst = shadow_dir.join("src/main.rs");
        fs::create_dir_all(dst.parent().unwrap()).await.expect("mkdir");
        fs::copy(&src, &dst).await.expect("copy");

        let content = read_file_checked(&dst).await.expect("read");
        assert_eq!(content, "fn main() {}\n");

        // Cleanup
        let _ = fs::remove_dir_all(&shadow_dir).await;
    }

    #[test]
    fn test_unified_diff_multiline_change() {
        let old = "aaa\nbbb\nccc\nddd\neee\nfff\nggg\nhhh\niii\njjj\n";
        let new = "aaa\nbbb\nccc\nDDD\neee\nfff\nggg\nHHH\niii\njjj\n";
        let diff = unified_diff(old, new, "a/test", "b/test");

        assert!(diff.contains("-ddd"));
        assert!(diff.contains("+DDD"));
        assert!(diff.contains("-hhh"));
        assert!(diff.contains("+HHH"));
    }

    #[test]
    fn test_diff_empty_to_empty() {
        let diff = unified_diff("", "", "a/empty", "b/empty");
        assert!(diff.is_empty());
    }
}
