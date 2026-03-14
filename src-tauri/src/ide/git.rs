//! Git operations for the IDE — powered by libgit2
//!
//! Provides status, diff, log, and basic staging/commit operations.
//! All operations run in spawn_blocking to avoid blocking the async runtime.

use git2::{BranchType, DiffOptions, Repository, Signature, StatusOptions};
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

/// Commit staged changes
#[tauri::command]
pub async fn git_commit(workspace_path: String, message: String) -> Result<CommitInfo, String> {
    let ws = workspace_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&ws)?;

        // Build index tree
        let mut index = repo.index().map_err(|e| format!("Failed to get index: {}", e))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| format!("Failed to write tree: {}", e))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| format!("Failed to find tree: {}", e))?;

        // Get signature from git config or fallback
        let sig = repo
            .signature()
            .or_else(|_| Signature::now("ImpForge User", "user@impforge.dev"))
            .map_err(|e| format!("Failed to create signature: {}", e))?;

        // Get parent commit (if any)
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
            .map_err(|e| format!("Failed to commit: {}", e))?;

        let time = chrono::DateTime::from_timestamp(sig.when().seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "now".to_string());

        Ok(CommitInfo {
            id: oid.to_string()[..7].to_string(),
            message: message.trim().to_string(),
            author: sig.name().unwrap_or("").to_string(),
            time,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Branch information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub last_commit: Option<String>,
}

/// Blame line information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub line_no: u32,
    pub author: String,
    pub commit_id: String,
    pub date: String,
    pub content: String,
}

/// List all branches (local + remote)
#[tauri::command]
pub async fn git_branches(workspace_path: String) -> Result<Vec<BranchInfo>, String> {
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&workspace_path)?;
        let head_ref = repo.head().ok();
        let head_name = head_ref
            .as_ref()
            .and_then(|h| h.shorthand().map(|s| s.to_string()));

        let mut branches = Vec::new();

        for branch_type in &[BranchType::Local, BranchType::Remote] {
            let iter = repo
                .branches(Some(*branch_type))
                .map_err(|e| format!("Failed to list branches: {}", e))?;

            for item in iter {
                let (branch, bt) = item.map_err(|e| format!("Branch iter error: {}", e))?;
                let name = branch
                    .name()
                    .map_err(|e| format!("Invalid branch name: {}", e))?
                    .unwrap_or("")
                    .to_string();

                let is_remote = bt == BranchType::Remote;
                let is_head = !is_remote && head_name.as_deref() == Some(&name);

                let upstream = branch
                    .upstream()
                    .ok()
                    .and_then(|u| u.name().ok().flatten().map(|s| s.to_string()));

                let last_commit = branch
                    .get()
                    .peel_to_commit()
                    .ok()
                    .map(|c| c.message().unwrap_or("").lines().next().unwrap_or("").to_string());

                branches.push(BranchInfo {
                    name,
                    is_head,
                    is_remote,
                    upstream,
                    last_commit,
                });
            }
        }

        // Sort: HEAD first, then local alphabetical, then remote alphabetical
        branches.sort_by(|a, b| {
            b.is_head
                .cmp(&a.is_head)
                .then(a.is_remote.cmp(&b.is_remote))
                .then(a.name.cmp(&b.name))
        });

        Ok(branches)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Create a new branch from HEAD
#[tauri::command]
pub async fn git_create_branch(
    workspace_path: String,
    branch_name: String,
) -> Result<BranchInfo, String> {
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&workspace_path)?;
        let head = repo
            .head()
            .map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let commit = head
            .peel_to_commit()
            .map_err(|e| format!("HEAD is not a commit: {}", e))?;

        let branch = repo
            .branch(&branch_name, &commit, false)
            .map_err(|e| format!("Failed to create branch: {}", e))?;

        let last_commit = commit
            .message()
            .map(|m| m.lines().next().unwrap_or("").to_string());

        let upstream = branch
            .upstream()
            .ok()
            .and_then(|u| u.name().ok().flatten().map(|s| s.to_string()));

        Ok(BranchInfo {
            name: branch_name,
            is_head: false,
            is_remote: false,
            upstream,
            last_commit,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Checkout (switch to) a branch
#[tauri::command]
pub async fn git_checkout(
    workspace_path: String,
    branch_name: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&workspace_path)?;

        let (object, reference) = repo
            .revparse_ext(&branch_name)
            .map_err(|e| format!("Branch '{}' not found: {}", branch_name, e))?;

        repo.checkout_tree(&object, None)
            .map_err(|e| format!("Failed to checkout: {}", e))?;

        if let Some(ref_name) = reference {
            repo.set_head(
                ref_name
                    .name()
                    .ok_or("Invalid reference name")?,
            )
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        } else {
            // Detached HEAD
            repo.set_head_detached(object.id())
                .map_err(|e| format!("Failed to detach HEAD: {}", e))?;
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Delete a local branch
#[tauri::command]
pub async fn git_delete_branch(
    workspace_path: String,
    branch_name: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&workspace_path)?;
        let mut branch = repo
            .find_branch(&branch_name, BranchType::Local)
            .map_err(|e| format!("Branch '{}' not found: {}", branch_name, e))?;

        if branch.is_head() {
            return Err("Cannot delete the currently checked-out branch".to_string());
        }

        branch
            .delete()
            .map_err(|e| format!("Failed to delete branch: {}", e))?;

        Ok(())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Push to remote — uses system git for authentication compatibility
#[tauri::command]
pub async fn git_push(
    workspace_path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    let remote_name = remote.unwrap_or_else(|| "origin".to_string());
    let branch_name = branch.unwrap_or_else(|| {
        // Detect current branch from repo
        if let Ok(repo) = Repository::discover(&workspace_path) {
            if let Ok(head) = repo.head() {
                if let Some(name) = head.shorthand() {
                    return name.to_string();
                }
            }
        }
        "main".to_string()
    });

    let output = tokio::process::Command::new("git")
        .args(["push", &remote_name, &branch_name])
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| format!("Failed to run git push: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(format!("{}{}", stdout, stderr).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Push failed: {}", stderr.trim()))
    }
}

/// Pull from remote — uses system git for authentication compatibility
#[tauri::command]
pub async fn git_pull(
    workspace_path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    let remote_name = remote.unwrap_or_else(|| "origin".to_string());

    let mut args = vec!["pull".to_string(), remote_name];
    if let Some(b) = branch {
        args.push(b);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(&workspace_path)
        .output()
        .await
        .map_err(|e| format!("Failed to run git pull: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(format!("{}{}", stdout, stderr).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Pull failed: {}", stderr.trim()))
    }
}

/// Blame a file — returns per-line author/commit attribution
#[tauri::command]
pub async fn git_blame(
    workspace_path: String,
    file_path: String,
) -> Result<Vec<BlameLine>, String> {
    let ws = workspace_path.clone();
    tokio::task::spawn_blocking(move || {
        let repo = find_repo(&ws)?;

        let relative = Path::new(&file_path)
            .strip_prefix(repo.workdir().unwrap_or(Path::new("/")))
            .unwrap_or(Path::new(&file_path));

        let blame = repo
            .blame_file(relative, None)
            .map_err(|e| format!("Failed to blame '{}': {}", file_path, e))?;

        // Read file content for line text
        let abs_path = repo
            .workdir()
            .unwrap_or(Path::new("/"))
            .join(relative);
        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let lines: Vec<&str> = content.lines().collect();

        let mut result = Vec::with_capacity(lines.len());
        for (i, line_text) in lines.iter().enumerate() {
            let line_no = (i + 1) as u32;
            if let Some(hunk) = blame.get_line(line_no as usize) {
                let sig = hunk.final_signature();
                let date = chrono::DateTime::from_timestamp(sig.when().seconds(), 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();

                result.push(BlameLine {
                    line_no,
                    author: sig.name().unwrap_or("").to_string(),
                    commit_id: hunk.final_commit_id().to_string()[..7].to_string(),
                    date,
                    content: line_text.to_string(),
                });
            }
        }

        Ok(result)
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
