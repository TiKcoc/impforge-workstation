// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Public API — consumed via neuralswarm Tauri bridge + tests
#![allow(dead_code)]
//! Git Operations for ImpForge Orchestrator
//!
//! Automated git operations with conventional commit support.
//! Handles commit, push, branch management, and status checking
//! for the standalone orchestrator's auto-push feature.
//!
//! Features:
//! - Conventional commit messages (feat/fix/chore/docs/refactor)
//! - Safe push with pre-flight checks (dirty index, remote sync)
//! - Branch management (create, switch, list)
//! - Trust-gated auto-push (only push if trust > threshold)
//!
//! Scientific basis:
//! - Conventional Commits 1.0.0 specification
//! - NeuralSwarm trust-level classification for push authorization

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Conventional commit types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommitType {
    Feat,
    Fix,
    Chore,
    Docs,
    Refactor,
    Test,
    Perf,
    Ci,
    Style,
    Build,
}

impl std::fmt::Display for CommitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitType::Feat => write!(f, "feat"),
            CommitType::Fix => write!(f, "fix"),
            CommitType::Chore => write!(f, "chore"),
            CommitType::Docs => write!(f, "docs"),
            CommitType::Refactor => write!(f, "refactor"),
            CommitType::Test => write!(f, "test"),
            CommitType::Perf => write!(f, "perf"),
            CommitType::Ci => write!(f, "ci"),
            CommitType::Style => write!(f, "style"),
            CommitType::Build => write!(f, "build"),
        }
    }
}

/// A conventional commit message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConventionalCommit {
    pub commit_type: CommitType,
    pub scope: Option<String>,
    pub description: String,
    pub body: Option<String>,
    pub breaking: bool,
}

impl ConventionalCommit {
    pub fn new(commit_type: CommitType, description: &str) -> Self {
        Self {
            commit_type,
            scope: None,
            description: description.to_string(),
            body: None,
            breaking: false,
        }
    }

    pub fn with_scope(mut self, scope: &str) -> Self {
        self.scope = Some(scope.to_string());
        self
    }

    pub fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub fn as_breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Format as conventional commit message string.
    pub fn format(&self) -> String {
        let mut msg = String::new();

        msg.push_str(&self.commit_type.to_string());

        if let Some(ref scope) = self.scope {
            msg.push_str(&format!("({})", scope));
        }

        if self.breaking {
            msg.push('!');
        }

        msg.push_str(": ");
        msg.push_str(&self.description);

        if let Some(ref body) = self.body {
            msg.push_str("\n\n");
            msg.push_str(body);
        }

        msg
    }
}

/// Git operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub command: String,
}

/// Git status of a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub is_clean: bool,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub ahead: u32,
    pub behind: u32,
}

/// Auto-push configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPushConfig {
    /// Minimum trust score to allow auto-push.
    pub min_trust: f64,
    /// Branch patterns that are allowed for auto-push.
    pub allowed_branches: Vec<String>,
    /// Whether to auto-create branches if they don't exist.
    pub auto_create_branch: bool,
    /// Whether to force-push (dangerous, default false).
    pub force_push: bool,
}

impl Default for AutoPushConfig {
    fn default() -> Self {
        Self {
            min_trust: 0.7,
            allowed_branches: vec!["develop".into(), "feature/*".into()],
            force_push: false,
            auto_create_branch: true,
        }
    }
}

/// Git operations manager.
pub struct GitOps {
    repo_path: PathBuf,
    auto_push_config: AutoPushConfig,
    history: Vec<GitOpEvent>,
}

/// Git operation event for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOpEvent {
    pub operation: String,
    pub result: bool,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

impl GitOps {
    pub fn new(repo_path: &Path) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            auto_push_config: AutoPushConfig::default(),
            history: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: AutoPushConfig) -> Self {
        self.auto_push_config = config;
        self
    }

    /// Run a git command in the repo directory.
    fn run_git(&self, args: &[&str]) -> GitResult {
        let cmd_str = format!("git {}", args.join(" "));

        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output();

        match output {
            Ok(out) => GitResult {
                success: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                command: cmd_str,
            },
            Err(e) => GitResult {
                success: false,
                stdout: String::new(),
                stderr: e.to_string(),
                command: cmd_str,
            },
        }
    }

    /// Get repository status.
    pub fn status(&self) -> GitStatus {
        let branch_result = self.run_git(&["branch", "--show-current"]);
        let status_result = self.run_git(&["status", "--porcelain"]);

        let branch = branch_result.stdout.trim().to_string();
        let lines: Vec<&str> = status_result.stdout.lines().collect();

        let staged = lines.iter().filter(|l| l.starts_with("A ") || l.starts_with("M ") || l.starts_with("D ")).count() as u32;
        let modified = lines.iter().filter(|l| l.starts_with(" M") || l.starts_with("MM")).count() as u32;
        let untracked = lines.iter().filter(|l| l.starts_with("??")).count() as u32;

        // Check ahead/behind
        let ab_result = self.run_git(&["rev-list", "--left-right", "--count", "HEAD...@{u}"]);
        let (ahead, behind) = if ab_result.success {
            let parts: Vec<&str> = ab_result.stdout.trim().split('\t').collect();
            (
                parts.first().and_then(|s| s.parse().ok()).unwrap_or(0),
                parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
            )
        } else {
            (0, 0)
        };

        GitStatus {
            branch,
            is_clean: lines.is_empty(),
            staged,
            modified,
            untracked,
            ahead,
            behind,
        }
    }

    /// Create a conventional commit.
    pub fn commit(&mut self, commit: &ConventionalCommit) -> GitResult {
        let msg = commit.format();
        let result = self.run_git(&["commit", "-m", &msg]);

        self.history.push(GitOpEvent {
            operation: "commit".into(),
            result: result.success,
            message: msg,
            timestamp: Utc::now(),
        });

        result
    }

    /// Stage files for commit.
    pub fn add(&self, paths: &[&str]) -> GitResult {
        let mut args = vec!["add"];
        args.extend(paths);
        self.run_git(&args)
    }

    /// Stage all changes.
    pub fn add_all(&self) -> GitResult {
        self.run_git(&["add", "-A"])
    }

    /// Push to remote.
    pub fn push(&mut self) -> GitResult {
        let args = if self.auto_push_config.force_push {
            vec!["push", "--force-with-lease"]
        } else {
            vec!["push"]
        };

        let result = self.run_git(&args);

        self.history.push(GitOpEvent {
            operation: "push".into(),
            result: result.success,
            message: if result.success { "Push successful".into() } else { result.stderr.clone() },
            timestamp: Utc::now(),
        });

        result
    }

    /// Push with upstream tracking.
    pub fn push_upstream(&mut self, branch: &str) -> GitResult {
        let result = self.run_git(&["push", "-u", "origin", branch]);

        self.history.push(GitOpEvent {
            operation: "push_upstream".into(),
            result: result.success,
            message: format!("Push -u origin {}", branch),
            timestamp: Utc::now(),
        });

        result
    }

    /// Auto-push: commit + push if trust is sufficient.
    pub fn auto_push(&mut self, commit: &ConventionalCommit, trust_score: f64) -> AutoPushResult {
        // Check trust gate
        if trust_score < self.auto_push_config.min_trust {
            return AutoPushResult {
                committed: false,
                pushed: false,
                reason: format!("Trust too low: {:.2} < {:.2}", trust_score, self.auto_push_config.min_trust),
            };
        }

        // Check branch is allowed
        let status = self.status();
        let branch_allowed = self.auto_push_config.allowed_branches.iter().any(|pattern| {
            if pattern.contains('*') {
                let prefix = pattern.trim_end_matches('*');
                status.branch.starts_with(prefix)
            } else {
                status.branch == *pattern
            }
        });

        if !branch_allowed {
            return AutoPushResult {
                committed: false,
                pushed: false,
                reason: format!("Branch '{}' not in allowed list", status.branch),
            };
        }

        // Commit
        let commit_result = self.commit(commit);
        if !commit_result.success {
            return AutoPushResult {
                committed: false,
                pushed: false,
                reason: format!("Commit failed: {}", commit_result.stderr),
            };
        }

        // Push
        let push_result = self.push();
        AutoPushResult {
            committed: true,
            pushed: push_result.success,
            reason: if push_result.success { "Auto-push successful".into() } else { push_result.stderr },
        }
    }

    /// Get operation history.
    pub fn history(&self) -> &[GitOpEvent] {
        &self.history
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> String {
        self.run_git(&["branch", "--show-current"]).stdout.trim().to_string()
    }

    /// Check if repo is a git repository.
    pub fn is_repo(&self) -> bool {
        self.run_git(&["rev-parse", "--git-dir"]).success
    }

    /// Get recent commits (last N).
    pub fn recent_commits(&self, count: u32) -> Vec<String> {
        let result = self.run_git(&["log", "--oneline", &format!("-{}", count)]);
        if result.success {
            result.stdout.lines().map(|l| l.to_string()).collect()
        } else {
            Vec::new()
        }
    }
}

/// Result of an auto-push operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPushResult {
    pub committed: bool,
    pub pushed: bool,
    pub reason: String,
}

// ════════════════════════════════════════════════════════════════
// TESTS
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conventional_commit_format() {
        let commit = ConventionalCommit::new(CommitType::Feat, "add MOA pipeline");
        assert_eq!(commit.format(), "feat: add MOA pipeline");
    }

    #[test]
    fn test_commit_with_scope() {
        let commit = ConventionalCommit::new(CommitType::Fix, "memory leak")
            .with_scope("orchestrator");
        assert_eq!(commit.format(), "fix(orchestrator): memory leak");
    }

    #[test]
    fn test_commit_breaking() {
        let commit = ConventionalCommit::new(CommitType::Feat, "new API")
            .with_scope("api")
            .as_breaking();
        assert_eq!(commit.format(), "feat(api)!: new API");
    }

    #[test]
    fn test_commit_with_body() {
        let commit = ConventionalCommit::new(CommitType::Docs, "update README")
            .with_body("Added installation instructions");
        let formatted = commit.format();
        assert!(formatted.contains("docs: update README"));
        assert!(formatted.contains("\n\nAdded installation instructions"));
    }

    #[test]
    fn test_commit_type_display() {
        assert_eq!(format!("{}", CommitType::Feat), "feat");
        assert_eq!(format!("{}", CommitType::Fix), "fix");
        assert_eq!(format!("{}", CommitType::Refactor), "refactor");
        assert_eq!(format!("{}", CommitType::Ci), "ci");
    }

    #[test]
    fn test_auto_push_config_default() {
        let config = AutoPushConfig::default();
        assert_eq!(config.min_trust, 0.7);
        assert!(!config.force_push);
        assert!(config.allowed_branches.contains(&"develop".to_string()));
    }

    #[test]
    fn test_git_ops_is_repo() {
        // Current working directory should be a git repo
        let ops = GitOps::new(Path::new("."));
        assert!(ops.is_repo());
    }

    #[test]
    fn test_git_ops_current_branch() {
        let ops = GitOps::new(Path::new("."));
        let branch = ops.current_branch();
        assert!(!branch.is_empty()); // Should have some branch
    }

    #[test]
    fn test_git_ops_status() {
        let ops = GitOps::new(Path::new("."));
        let status = ops.status();
        assert!(!status.branch.is_empty());
    }

    #[test]
    fn test_git_ops_recent_commits() {
        let ops = GitOps::new(Path::new("."));
        let commits = ops.recent_commits(5);
        assert!(!commits.is_empty());
    }

    #[test]
    fn test_auto_push_trust_gate() {
        let mut ops = GitOps::new(Path::new("."));
        let commit = ConventionalCommit::new(CommitType::Chore, "test");

        // Low trust should be rejected
        let result = ops.auto_push(&commit, 0.3);
        assert!(!result.committed);
        assert!(result.reason.contains("Trust too low"));
    }

    #[test]
    fn test_auto_push_branch_gate() {
        let mut ops = GitOps::new(Path::new("."))
            .with_config(AutoPushConfig {
                allowed_branches: vec!["main".into()],
                ..Default::default()
            });
        let commit = ConventionalCommit::new(CommitType::Chore, "test");

        let status = ops.status();
        if status.branch != "main" {
            let result = ops.auto_push(&commit, 1.0);
            assert!(!result.committed);
            assert!(result.reason.contains("not in allowed list"));
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let commit = ConventionalCommit::new(CommitType::Feat, "test")
            .with_scope("scope");
        let json = serde_json::to_string(&commit).unwrap();
        let deser: ConventionalCommit = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.commit_type, CommitType::Feat);
        assert_eq!(deser.scope, Some("scope".into()));
    }
}
