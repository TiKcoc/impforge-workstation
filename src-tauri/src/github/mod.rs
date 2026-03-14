//! GitHub Integration
//!
//! Provides GitHub API integration for repository management,
//! issues, pull requests, and more.
//!
//! Uses the GitHub REST API v3 with token authentication.

use serde::{Deserialize, Serialize};

const GITHUB_API_BASE: &str = "https://api.github.com";

/// Repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub stargazers_count: u32,
    pub open_issues_count: u32,
    #[serde(rename = "private")]
    pub is_private: bool,
    pub language: Option<String>,
    pub updated_at: String,
}

/// Issue information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInfo {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub created_at: String,
    pub labels: Vec<LabelInfo>,
    pub user: UserInfo,
    pub body: Option<String>,
}

/// Label information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelInfo {
    pub name: String,
    pub color: String,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub login: String,
    pub avatar_url: String,
}

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestInfo {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub created_at: String,
    pub user: UserInfo,
    pub head: BranchRef,
    pub base: BranchRef,
    pub merged: bool,
    pub draft: bool,
}

/// Branch reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchRef {
    #[serde(rename = "ref")]
    pub branch_ref: String,
    pub sha: String,
}

/// Get HTTP client with GitHub headers
fn get_client(token: Option<&str>) -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        "application/vnd.github+json".parse().expect("valid header"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        "2022-11-28".parse().expect("valid header"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        "ImpForge-AI-Workstation/1.0".parse().expect("valid header"),
    );

    if let Some(t) = token {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", t).parse().map_err(|e| format!("Invalid token: {}", e))?,
        );
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Get GitHub token from environment
fn get_github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN").ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
}

/// Get user's repositories
#[tauri::command]
pub async fn get_repos() -> Result<Vec<RepoInfo>, String> {
    log::info!("Fetching GitHub repositories");

    let token = get_github_token()
        .ok_or_else(|| "GitHub token not configured. Set GITHUB_TOKEN environment variable.".to_string())?;

    let client = get_client(Some(&token))?;

    let response = client
        .get(format!("{}/user/repos", GITHUB_API_BASE))
        .query(&[
            ("sort", "updated"),
            ("per_page", "50"),
            ("type", "all"),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to fetch repos: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let repos: Vec<RepoInfo> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    log::info!("Found {} repositories", repos.len());
    Ok(repos)
}

/// Get issues for a repository
#[tauri::command]
pub async fn get_issues(repo: String) -> Result<Vec<IssueInfo>, String> {
    log::info!("Fetching issues for repo: {}", repo);

    let token = get_github_token();
    let client = get_client(token.as_deref())?;

    let response = client
        .get(format!("{}/repos/{}/issues", GITHUB_API_BASE, repo))
        .query(&[
            ("state", "all"),
            ("per_page", "30"),
            ("sort", "updated"),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to fetch issues: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let issues: Vec<IssueInfo> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    log::info!("Found {} issues", issues.len());
    Ok(issues)
}

/// Get pull requests for a repository
#[tauri::command]
pub async fn get_pull_requests(repo: String) -> Result<Vec<PullRequestInfo>, String> {
    log::info!("Fetching pull requests for repo: {}", repo);

    let token = get_github_token();
    let client = get_client(token.as_deref())?;

    let response = client
        .get(format!("{}/repos/{}/pulls", GITHUB_API_BASE, repo))
        .query(&[
            ("state", "all"),
            ("per_page", "30"),
            ("sort", "updated"),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to fetch PRs: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    let prs: Vec<PullRequestInfo> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    log::info!("Found {} pull requests", prs.len());
    Ok(prs)
}

/// Get authenticated user info
#[tauri::command]
pub async fn get_user() -> Result<UserInfo, String> {
    log::info!("Fetching GitHub user info");

    let token = get_github_token()
        .ok_or_else(|| "GitHub token not configured".to_string())?;

    let client = get_client(Some(&token))?;

    let response = client
        .get(format!("{}/user", GITHUB_API_BASE))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user: {}", e))?;

    if !response.status().is_success() {
        return Err("Failed to authenticate with GitHub".to_string());
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse user info: {}", e))
}
