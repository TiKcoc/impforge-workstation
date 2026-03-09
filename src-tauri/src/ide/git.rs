//! Git operations for the IDE — powered by libgit2
//!
//! Provides status, diff, log, and basic staging/commit operations.
//! All operations run in spawn_blocking to avoid blocking the async runtime.

use git2::{DiffOptions, Repository, StatusOptions};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusResult {
    pub branch: String,
    pub files: Vec<GitFileStatus>,
    pub clean: bool,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub time: String,
}

/// Find a git repo starting from the given path, walking up
fn find_repo(start_path: &str) -> Result<Repository, String> {
    Repository::discover(start_path).map_err(|e| format!("No git repository found: {}", e))
}

/// Get status of all files in the working directory
#[tauri::command]
pub async fn git_status(workspace_path: String) -> Result<GitStatusResult, String> {
    let path = workspace_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&path)?;

        // Get current branch
        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string());

        // Get file statuses
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| format!("Failed to get status: {}", e))?;

        let mut files = Vec::new();
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let st = entry.status();

            let (status_str, staged) = if st.is_index_new() {
                ("added", true)
            } else if st.is_index_modified() {
                ("modified", true)
            } else if st.is_index_deleted() {
                ("deleted", true)
            } else if st.is_wt_new() {
                ("untracked", false)
            } else if st.is_wt_modified() {
                ("modified", false)
            } else if st.is_wt_deleted() {
                ("deleted", false)
            } else if st.is_conflicted() {
                ("conflict", false)
            } else {
                continue;
            };

            files.push(GitFileStatus {
                path,
                status: status_str.to_string(),
                staged,
            });
        }

        let clean = files.is_empty();

        Ok(GitStatusResult {
            branch,
            files,
            clean,
            ahead: 0,
            behind: 0,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Get diff for working directory changes
#[tauri::command]
pub async fn git_diff(workspace_path: String, staged: Option<bool>) -> Result<String, String> {
    let path = workspace_path.clone();
    let show_staged = staged.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&path)?;

        let mut diff_opts = DiffOptions::new();
        diff_opts.include_untracked(true);

        let diff = if show_staged {
            let head_tree = repo
                .head()
                .ok()
                .and_then(|h| h.peel_to_tree().ok());

            repo.diff_tree_to_index(
                head_tree.as_ref(),
                None,
                Some(&mut diff_opts),
            )
            .map_err(|e| format!("Failed to get staged diff: {}", e))?
        } else {
            repo.diff_index_to_workdir(None, Some(&mut diff_opts))
                .map_err(|e| format!("Failed to get diff: {}", e))?
        };

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let prefix = match line.origin() {
                '+' => "+",
                '-' => "-",
                ' ' => " ",
                _ => "",
            };
            output.push_str(prefix);
            output.push_str(&String::from_utf8_lossy(line.content()));
            true
        })
        .map_err(|e| format!("Failed to format diff: {}", e))?;

        Ok(output)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Get commit log
#[tauri::command]
pub async fn git_log(
    workspace_path: String,
    limit: Option<usize>,
) -> Result<Vec<CommitInfo>, String> {
    let path = workspace_path.clone();
    let max = limit.unwrap_or(50);

    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&path)?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| format!("Failed to create revwalk: {}", e))?;

        revwalk
            .push_head()
            .map_err(|e| format!("Failed to push HEAD: {}", e))?;

        let mut commits = Vec::new();
        for oid in revwalk.take(max) {
            let oid = oid.map_err(|e| format!("Revwalk error: {}", e))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| format!("Failed to find commit: {}", e))?;

            let time = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            commits.push(CommitInfo {
                id: oid.to_string()[..7].to_string(),
                message: commit.message().unwrap_or("").trim().to_string(),
                author: commit.author().name().unwrap_or("").to_string(),
                time,
            });
        }

        Ok(commits)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Stage a file
#[tauri::command]
pub async fn git_stage(workspace_path: String, file_path: String) -> Result<(), String> {
    let ws = workspace_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&ws)?;
        let mut index = repo.index().map_err(|e| format!("Failed to get index: {}", e))?;

        let relative = Path::new(&file_path)
            .strip_prefix(repo.workdir().unwrap_or(Path::new("/")))
            .unwrap_or(Path::new(&file_path));

        index
            .add_path(relative)
            .map_err(|e| format!("Failed to stage file: {}", e))?;
        index
            .write()
            .map_err(|e| format!("Failed to write index: {}", e))?;

        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Unstage a file
#[tauri::command]
pub async fn git_unstage(workspace_path: String, file_path: String) -> Result<(), String> {
    let ws = workspace_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&ws)?;

        let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let target = head.target().ok_or("HEAD has no target")?;
        let commit = repo.find_commit(target).map_err(|e| format!("Failed to find commit: {}", e))?;

        let relative = Path::new(&file_path)
            .strip_prefix(repo.workdir().unwrap_or(Path::new("/")))
            .unwrap_or(Path::new(&file_path));

        repo.reset_default(Some(commit.as_object()), [relative])
            .map_err(|e| format!("Failed to unstage: {}", e))?;

        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
