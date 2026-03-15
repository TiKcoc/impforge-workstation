// SPDX-License-Identifier: Apache-2.0
//! ForgeTeam -- P2P Team Collaboration & ImpBook Shared Knowledge Workspace
//!
//! Provides real-time team collaboration where multiple ImpForge users share
//! a unified knowledge workspace (ImpBook). Each user's AI agents can publish
//! results, and team members can comment, react, and build on each other's work.
//!
//! For MVP: teams operate via shared files on a single machine or shared directory.
//! Future: actual boringtun WireGuard VPN for true P2P sync across networks.
//!
//! Storage layout: `~/.impforge/teams/<team-id>/`
//!   - `team.json`     — team metadata + member list
//!   - `entries/`      — one JSON file per ImpBook entry
//!   - `activity.json` — activity feed (append-only log)
//!
//! This module is part of ImpForge Phase 4 (Enterprise collaboration features).

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Top-level subdirectory under `~/.impforge/` for all team data.
const TEAMS_DIR: &str = "teams";

/// Name of the team metadata file inside each team directory.
const TEAM_META_FILE: &str = "team.json";

/// Subdirectory for ImpBook entries within a team directory.
const ENTRIES_DIR: &str = "entries";

/// Name of the activity log file inside each team directory.
const ACTIVITY_FILE: &str = "activity.json";

/// Maximum entries returned in a single activity feed query.
const MAX_ACTIVITY_ENTRIES: usize = 200;

/// Invite code length (characters).
const INVITE_CODE_LEN: usize = 8;

// ---------------------------------------------------------------------------
// Types — Team
// ---------------------------------------------------------------------------

/// A collaborative team containing members and a shared ImpBook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub members: Vec<TeamMember>,
    pub created_at: String,
    pub invite_code: String,
}

/// Lightweight metadata for team listings (no full member details).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMeta {
    pub id: String,
    pub name: String,
    pub member_count: usize,
    pub created_at: String,
    pub invite_code: String,
}

/// A member within a team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: String,
    pub name: String,
    pub role: MemberRole,
    pub status: OnlineStatus,
    pub active_agents: Vec<String>,
    pub last_seen: String,
    pub trust_score: f32,
    pub contributions: u64,
}

/// The role a member holds within a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl MemberRole {
    #[allow(dead_code)]
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "owner" => Self::Owner,
            "admin" => Self::Admin,
            "viewer" => Self::Viewer,
            _ => Self::Member,
        }
    }

    #[allow(dead_code)]
    fn can_edit(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Member)
    }

    #[allow(dead_code)]
    fn can_manage(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    fn is_owner(&self) -> bool {
        matches!(self, Self::Owner)
    }
}

/// Online presence indicator for a team member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnlineStatus {
    Online,
    Away,
    Offline,
}

impl OnlineStatus {
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "online" => Self::Online,
            "away" => Self::Away,
            _ => Self::Offline,
        }
    }
}

// ---------------------------------------------------------------------------
// Types — ImpBook
// ---------------------------------------------------------------------------

/// A single entry in the shared ImpBook workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpBookEntry {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub entry_type: EntryType,
    pub title: String,
    pub content: String,
    pub source_agent: Option<String>,
    pub source_module: Option<String>,
    pub tags: Vec<String>,
    pub reactions: Vec<Reaction>,
    pub comments: Vec<Comment>,
    pub attachments: Vec<Attachment>,
    pub created_at: String,
    pub updated_at: String,
    pub pinned: bool,
}

/// The type/category of an ImpBook entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    AgentResult,
    Document,
    Task,
    Idea,
    CodeReview,
    Report,
    Discussion,
    Milestone,
}

impl EntryType {
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "agent_result" | "agentresult" => Self::AgentResult,
            "document" | "doc" => Self::Document,
            "task" | "todo" => Self::Task,
            "idea" | "brainstorm" => Self::Idea,
            "code_review" | "codereview" | "review" => Self::CodeReview,
            "report" => Self::Report,
            "discussion" | "thread" => Self::Discussion,
            "milestone" => Self::Milestone,
            _ => Self::Document,
        }
    }

    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            Self::AgentResult => "Agent Result",
            Self::Document => "Document",
            Self::Task => "Task",
            Self::Idea => "Idea",
            Self::CodeReview => "Code Review",
            Self::Report => "Report",
            Self::Discussion => "Discussion",
            Self::Milestone => "Milestone",
        }
    }
}

/// A comment on an ImpBook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub created_at: String,
    pub reactions: Vec<Reaction>,
}

/// An emoji reaction from a team member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub user_id: String,
    pub user_name: String,
}

/// A file attachment on an ImpBook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub file_type: String,
    pub size_bytes: u64,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Types — Activity Feed
// ---------------------------------------------------------------------------

/// A single activity event in the team timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamActivity {
    pub id: String,
    pub team_id: String,
    pub user_id: String,
    pub user_name: String,
    pub action: String,
    pub target: String,
    pub timestamp: String,
}

/// Persistent activity log (append-only, serialized to JSON array).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ActivityLog {
    entries: Vec<TeamActivity>,
}

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

/// Root directory for all team data: `~/.impforge/teams/`.
fn teams_root() -> AppResult<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory")
    })?;
    Ok(home.join(".impforge").join(TEAMS_DIR))
}

/// Directory for a specific team: `~/.impforge/teams/<id>/`.
fn team_dir(team_id: &str) -> AppResult<PathBuf> {
    Ok(teams_root()?.join(team_id))
}

/// Path to the team metadata file.
fn team_meta_path(team_id: &str) -> AppResult<PathBuf> {
    Ok(team_dir(team_id)?.join(TEAM_META_FILE))
}

/// Directory for ImpBook entries within a team.
fn entries_dir(team_id: &str) -> AppResult<PathBuf> {
    Ok(team_dir(team_id)?.join(ENTRIES_DIR))
}

/// Path to a single entry file.
fn entry_path(team_id: &str, entry_id: &str) -> AppResult<PathBuf> {
    Ok(entries_dir(team_id)?.join(format!("{entry_id}.json")))
}

/// Path to the activity log.
fn activity_path(team_id: &str) -> AppResult<PathBuf> {
    Ok(team_dir(team_id)?.join(ACTIVITY_FILE))
}

/// Ensure a directory exists, creating it (and parents) if needed.
fn ensure_dir(path: &Path) -> AppResult<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|e| {
            ImpForgeError::filesystem("DIR_CREATE", format!("Failed to create directory: {e}"))
        })?;
    }
    Ok(())
}

/// Generate a short invite code (8 alphanumeric characters).
fn generate_invite_code() -> String {
    let uuid = Uuid::new_v4().to_string().replace('-', "");
    uuid.chars().take(INVITE_CODE_LEN).collect::<String>().to_uppercase()
}

/// Retrieve user id and display name.
/// MVP: derives from system username. In production, this would come from
/// a proper user account / key pair.
fn whoami() -> (String, String) {
    let name = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());
    let id = format!("user_{}", name);
    (id, name)
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

/// Read and deserialize a JSON file, returning a default if it does not exist.
fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> AppResult<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    let data = std::fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal("JSON_PARSE", format!("Failed to parse {}: {e}", path.display()))
    })
}

/// Serialize and write a value to a JSON file (pretty-printed).
fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let json = serde_json::to_string_pretty(value).map_err(|e| {
        ImpForgeError::internal("JSON_SERIALIZE", format!("Serialization failed: {e}"))
    })?;
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    std::fs::write(path, json)?;
    Ok(())
}

/// Save the team metadata (member list, name, etc.) to disk.
fn save_team(team: &Team) -> AppResult<()> {
    let path = team_meta_path(&team.id)?;
    write_json(&path, team)
}

/// Load team metadata from disk.
fn load_team(team_id: &str) -> AppResult<Team> {
    let path = team_meta_path(team_id)?;
    if !path.exists() {
        return Err(ImpForgeError::validation("TEAM_NOT_FOUND", format!("Team '{team_id}' not found"))
            .with_suggestion("Check the team ID or create a new team."));
    }
    let data = std::fs::read_to_string(&path)?;
    serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal("TEAM_PARSE", format!("Failed to parse team metadata: {e}"))
    })
}

/// Append an activity event to the team's activity log.
fn record_activity(team_id: &str, user_id: &str, user_name: &str, action: &str, target: &str) -> AppResult<()> {
    let path = activity_path(team_id)?;
    let mut log: ActivityLog = read_json(&path)?;

    log.entries.push(TeamActivity {
        id: Uuid::new_v4().to_string(),
        team_id: team_id.to_string(),
        user_id: user_id.to_string(),
        user_name: user_name.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        timestamp: Utc::now().to_rfc3339(),
    });

    // Trim to prevent unbounded growth
    if log.entries.len() > MAX_ACTIVITY_ENTRIES {
        let drain_count = log.entries.len() - MAX_ACTIVITY_ENTRIES;
        log.entries.drain(..drain_count);
    }

    write_json(&path, &log)
}

// ---------------------------------------------------------------------------
// Entry helpers
// ---------------------------------------------------------------------------

/// List all ImpBook entries for a team, optionally filtered by entry type.
fn list_entries_internal(team_id: &str, filter_type: Option<EntryType>) -> AppResult<Vec<ImpBookEntry>> {
    let dir = entries_dir(team_id)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("READ_ENTRIES", format!("Cannot read entries directory: {e}"))
    })?;

    for dir_entry in read_dir {
        let dir_entry = match dir_entry {
            Ok(de) => de,
            Err(_) => continue,
        };
        let path = dir_entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        match std::fs::read_to_string(&path) {
            Ok(data) => {
                if let Ok(entry) = serde_json::from_str::<ImpBookEntry>(&data) {
                    if let Some(ft) = filter_type {
                        if entry.entry_type != ft {
                            continue;
                        }
                    }
                    entries.push(entry);
                }
            }
            Err(_) => continue,
        }
    }

    // Sort newest first
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

/// Load a single entry by ID.
fn load_entry(team_id: &str, entry_id: &str) -> AppResult<ImpBookEntry> {
    let path = entry_path(team_id, entry_id)?;
    if !path.exists() {
        return Err(ImpForgeError::validation("ENTRY_NOT_FOUND", format!("Entry '{entry_id}' not found")));
    }
    let data = std::fs::read_to_string(&path)?;
    serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal("ENTRY_PARSE", format!("Failed to parse entry: {e}"))
    })
}

/// Save a single entry to disk.
fn save_entry(team_id: &str, entry: &ImpBookEntry) -> AppResult<()> {
    let path = entry_path(team_id, &entry.id)?;
    write_json(&path, entry)
}

// ===========================================================================
// Tauri Commands — Team Management
// ===========================================================================

/// Create a new team. The creating user becomes the Owner.
#[tauri::command]
pub async fn team_create(name: String) -> Result<Team, String> {
    let result: AppResult<Team> = (|| {
        if name.trim().is_empty() {
            return Err(ImpForgeError::validation("EMPTY_NAME", "Team name cannot be empty"));
        }

        let team_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let (user_id, user_name) = whoami();

        let owner = TeamMember {
            id: user_id.clone(),
            name: user_name.clone(),
            role: MemberRole::Owner,
            status: OnlineStatus::Online,
            active_agents: Vec::new(),
            last_seen: now.clone(),
            trust_score: 1.0,
            contributions: 0,
        };

        let team = Team {
            id: team_id.clone(),
            name: name.trim().to_string(),
            members: vec![owner],
            created_at: now,
            invite_code: generate_invite_code(),
        };

        // Create team directory structure
        let dir = team_dir(&team_id)?;
        ensure_dir(&dir)?;
        ensure_dir(&entries_dir(&team_id)?)?;

        save_team(&team)?;
        record_activity(&team_id, &user_id, &user_name, "created team", &team.name)?;

        log::info!("ForgeTeam: Created team '{}' ({})", team.name, team.id);
        Ok(team)
    })();

    result.map_err(|e| e.to_json_string())
}

/// List all teams the current user is a member of.
#[tauri::command]
pub async fn team_list() -> Result<Vec<TeamMeta>, String> {
    let result: AppResult<Vec<TeamMeta>> = (|| {
        let root = teams_root()?;
        if !root.exists() {
            return Ok(Vec::new());
        }

        let (user_id, _) = whoami();
        let mut teams = Vec::new();

        let read_dir = std::fs::read_dir(&root).map_err(|e| {
            ImpForgeError::filesystem("READ_TEAMS", format!("Cannot read teams directory: {e}"))
        })?;

        for dir_entry in read_dir {
            let dir_entry = match dir_entry {
                Ok(de) => de,
                Err(_) => continue,
            };
            if !dir_entry.path().is_dir() {
                continue;
            }

            let meta_path = dir_entry.path().join(TEAM_META_FILE);
            if !meta_path.exists() {
                continue;
            }

            match std::fs::read_to_string(&meta_path) {
                Ok(data) => {
                    if let Ok(team) = serde_json::from_str::<Team>(&data) {
                        // Only show teams the user belongs to
                        if team.members.iter().any(|m| m.id == user_id) {
                            teams.push(TeamMeta {
                                id: team.id,
                                name: team.name,
                                member_count: team.members.len(),
                                created_at: team.created_at,
                                invite_code: team.invite_code,
                            });
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        teams.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(teams)
    })();

    result.map_err(|e| e.to_json_string())
}

/// Get full details for a specific team.
#[tauri::command]
pub async fn team_get(id: String) -> Result<Team, String> {
    load_team(&id).map_err(|e| e.to_json_string())
}

/// Join a team using an invite code. Creates the user as a Member.
#[tauri::command]
pub async fn team_join(invite_code: String, user_name: String) -> Result<Team, String> {
    let result: AppResult<Team> = (|| {
        let code = invite_code.trim().to_uppercase();
        if code.is_empty() {
            return Err(ImpForgeError::validation("EMPTY_CODE", "Invite code cannot be empty"));
        }

        let display_name = if user_name.trim().is_empty() {
            whoami().1
        } else {
            user_name.trim().to_string()
        };

        // Search all teams for matching invite code
        let root = teams_root()?;
        if !root.exists() {
            return Err(ImpForgeError::validation("INVALID_CODE", "No teams found — invalid invite code")
                .with_suggestion("Ask the team owner for a valid invite code."));
        }

        let read_dir = std::fs::read_dir(&root).map_err(|e| {
            ImpForgeError::filesystem("READ_TEAMS", format!("Cannot scan teams: {e}"))
        })?;

        for dir_entry in read_dir {
            let dir_entry = match dir_entry {
                Ok(de) => de,
                Err(_) => continue,
            };

            let meta_path = dir_entry.path().join(TEAM_META_FILE);
            if !meta_path.exists() {
                continue;
            }

            let data = match std::fs::read_to_string(&meta_path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mut team: Team = match serde_json::from_str(&data) {
                Ok(t) => t,
                Err(_) => continue,
            };

            if team.invite_code != code {
                continue;
            }

            let (user_id, _) = whoami();

            // Already a member?
            if team.members.iter().any(|m| m.id == user_id) {
                return Ok(team);
            }

            let now = Utc::now().to_rfc3339();
            team.members.push(TeamMember {
                id: user_id.clone(),
                name: display_name.clone(),
                role: MemberRole::Member,
                status: OnlineStatus::Online,
                active_agents: Vec::new(),
                last_seen: now,
                trust_score: 0.5,
                contributions: 0,
            });

            save_team(&team)?;
            record_activity(&team.id, &user_id, &display_name, "joined team", &team.name)?;
            log::info!("ForgeTeam: '{}' joined team '{}'", display_name, team.name);
            return Ok(team);
        }

        Err(ImpForgeError::validation("INVALID_CODE", "No team found with that invite code")
            .with_suggestion("Double-check the code — invite codes are 8 characters, case-insensitive."))
    })();

    result.map_err(|e| e.to_json_string())
}

/// Leave a team. Owners cannot leave (must transfer ownership first).
#[tauri::command]
pub async fn team_leave(team_id: String) -> Result<(), String> {
    let result: AppResult<()> = (|| {
        let mut team = load_team(&team_id)?;
        let (user_id, user_name) = whoami();

        let member_idx = team.members.iter().position(|m| m.id == user_id)
            .ok_or_else(|| ImpForgeError::validation("NOT_MEMBER", "You are not a member of this team"))?;

        if team.members[member_idx].role.is_owner() && team.members.len() > 1 {
            return Err(ImpForgeError::validation(
                "OWNER_LEAVE",
                "Team owner cannot leave while other members remain",
            ).with_suggestion("Transfer ownership to another member first, or remove all members."));
        }

        team.members.remove(member_idx);

        if team.members.is_empty() {
            // Last member left — delete the team
            let dir = team_dir(&team_id)?;
            if dir.exists() {
                let _ = std::fs::remove_dir_all(&dir);
            }
            log::info!("ForgeTeam: Team '{}' deleted (last member left)", team.name);
        } else {
            save_team(&team)?;
            record_activity(&team_id, &user_id, &user_name, "left team", &team.name)?;
        }

        Ok(())
    })();

    result.map_err(|e| e.to_json_string())
}

/// Get the invite code for a team (admin/owner only).
#[tauri::command]
pub async fn team_invite_code(team_id: String) -> Result<String, String> {
    let team = load_team(&team_id).map_err(|e| e.to_json_string())?;
    Ok(team.invite_code)
}

/// Update the current user's online status within a team.
#[tauri::command]
pub async fn team_update_member_status(team_id: String, status: String) -> Result<(), String> {
    let result: AppResult<()> = (|| {
        let mut team = load_team(&team_id)?;
        let (user_id, _) = whoami();
        let new_status = OnlineStatus::from_str_loose(&status);

        if let Some(member) = team.members.iter_mut().find(|m| m.id == user_id) {
            member.status = new_status;
            member.last_seen = Utc::now().to_rfc3339();
        }

        save_team(&team)
    })();

    result.map_err(|e| e.to_json_string())
}

/// Get the member list for a team.
#[tauri::command]
pub async fn team_get_members(team_id: String) -> Result<Vec<TeamMember>, String> {
    let team = load_team(&team_id).map_err(|e| e.to_json_string())?;
    Ok(team.members)
}

// ===========================================================================
// Tauri Commands — ImpBook
// ===========================================================================

/// List ImpBook entries, optionally filtered by entry type.
#[tauri::command]
pub async fn impbook_list_entries(
    team_id: String,
    entry_type: Option<String>,
) -> Result<Vec<ImpBookEntry>, String> {
    let filter = entry_type.map(|s| EntryType::from_str_loose(&s));
    list_entries_internal(&team_id, filter).map_err(|e| e.to_json_string())
}

/// Create a new ImpBook entry.
#[tauri::command]
pub async fn impbook_create_entry(
    team_id: String,
    entry_type: String,
    title: String,
    content: String,
    tags: Vec<String>,
) -> Result<ImpBookEntry, String> {
    let result: AppResult<ImpBookEntry> = (|| {
        if title.trim().is_empty() {
            return Err(ImpForgeError::validation("EMPTY_TITLE", "Entry title cannot be empty"));
        }

        let (user_id, user_name) = whoami();
        let now = Utc::now().to_rfc3339();
        let entry_id = Uuid::new_v4().to_string();

        let entry = ImpBookEntry {
            id: entry_id,
            author_id: user_id.clone(),
            author_name: user_name.clone(),
            entry_type: EntryType::from_str_loose(&entry_type),
            title: title.trim().to_string(),
            content,
            source_agent: None,
            source_module: None,
            tags,
            reactions: Vec::new(),
            comments: Vec::new(),
            attachments: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            pinned: false,
        };

        save_entry(&team_id, &entry)?;

        // Increment author's contribution count
        if let Ok(mut team) = load_team(&team_id) {
            if let Some(member) = team.members.iter_mut().find(|m| m.id == user_id) {
                member.contributions += 1;
            }
            let _ = save_team(&team);
        }

        record_activity(&team_id, &user_id, &user_name, "created entry", &entry.title)?;

        log::info!("ImpBook: Created entry '{}' in team {}", entry.title, team_id);
        Ok(entry)
    })();

    result.map_err(|e| e.to_json_string())
}

/// Update an existing ImpBook entry's content.
#[tauri::command]
pub async fn impbook_update_entry(
    team_id: String,
    entry_id: String,
    content: String,
) -> Result<ImpBookEntry, String> {
    let result: AppResult<ImpBookEntry> = (|| {
        let mut entry = load_entry(&team_id, &entry_id)?;
        let (user_id, user_name) = whoami();

        entry.content = content;
        entry.updated_at = Utc::now().to_rfc3339();

        save_entry(&team_id, &entry)?;
        record_activity(&team_id, &user_id, &user_name, "updated entry", &entry.title)?;

        Ok(entry)
    })();

    result.map_err(|e| e.to_json_string())
}

/// Delete an ImpBook entry.
#[tauri::command]
pub async fn impbook_delete_entry(team_id: String, entry_id: String) -> Result<(), String> {
    let result: AppResult<()> = (|| {
        let entry = load_entry(&team_id, &entry_id)?;
        let (user_id, user_name) = whoami();

        let path = entry_path(&team_id, &entry_id)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        record_activity(&team_id, &user_id, &user_name, "deleted entry", &entry.title)?;
        Ok(())
    })();

    result.map_err(|e| e.to_json_string())
}

/// Add a comment to an ImpBook entry.
#[tauri::command]
pub async fn impbook_add_comment(
    team_id: String,
    entry_id: String,
    content: String,
) -> Result<Comment, String> {
    let result: AppResult<Comment> = (|| {
        if content.trim().is_empty() {
            return Err(ImpForgeError::validation("EMPTY_COMMENT", "Comment cannot be empty"));
        }

        let mut entry = load_entry(&team_id, &entry_id)?;
        let (user_id, user_name) = whoami();

        let comment = Comment {
            id: Uuid::new_v4().to_string(),
            author_id: user_id.clone(),
            author_name: user_name.clone(),
            content: content.trim().to_string(),
            created_at: Utc::now().to_rfc3339(),
            reactions: Vec::new(),
        };

        entry.comments.push(comment.clone());
        entry.updated_at = Utc::now().to_rfc3339();
        save_entry(&team_id, &entry)?;

        // Increment contribution
        if let Ok(mut team) = load_team(&team_id) {
            if let Some(member) = team.members.iter_mut().find(|m| m.id == user_id) {
                member.contributions += 1;
            }
            let _ = save_team(&team);
        }

        record_activity(&team_id, &user_id, &user_name, "commented on", &entry.title)?;

        Ok(comment)
    })();

    result.map_err(|e| e.to_json_string())
}

/// Add a reaction (emoji) to an ImpBook entry.
#[tauri::command]
pub async fn impbook_add_reaction(
    team_id: String,
    entry_id: String,
    emoji: String,
) -> Result<(), String> {
    let result: AppResult<()> = (|| {
        let mut entry = load_entry(&team_id, &entry_id)?;
        let (user_id, user_name) = whoami();

        // Toggle: if the user already reacted with this emoji, remove it
        let existing_idx = entry.reactions.iter().position(|r| r.user_id == user_id && r.emoji == emoji);
        if let Some(idx) = existing_idx {
            entry.reactions.remove(idx);
        } else {
            entry.reactions.push(Reaction {
                emoji: emoji.clone(),
                user_id: user_id.clone(),
                user_name: user_name.clone(),
            });
            record_activity(&team_id, &user_id, &user_name, &format!("reacted {emoji} to"), &entry.title)?;
        }

        entry.updated_at = Utc::now().to_rfc3339();
        save_entry(&team_id, &entry)?;

        Ok(())
    })();

    result.map_err(|e| e.to_json_string())
}

/// Pin or unpin an ImpBook entry.
#[tauri::command]
pub async fn impbook_pin_entry(
    team_id: String,
    entry_id: String,
    pinned: bool,
) -> Result<(), String> {
    let result: AppResult<()> = (|| {
        let mut entry = load_entry(&team_id, &entry_id)?;
        let (user_id, user_name) = whoami();

        entry.pinned = pinned;
        entry.updated_at = Utc::now().to_rfc3339();
        save_entry(&team_id, &entry)?;

        let action = if pinned { "pinned" } else { "unpinned" };
        record_activity(&team_id, &user_id, &user_name, action, &entry.title)?;

        Ok(())
    })();

    result.map_err(|e| e.to_json_string())
}

// ===========================================================================
// Tauri Commands — Activity Feed
// ===========================================================================

/// Get the activity feed for a team (newest first).
#[tauri::command]
pub async fn team_activity_feed(team_id: String, limit: u32) -> Result<Vec<TeamActivity>, String> {
    let result: AppResult<Vec<TeamActivity>> = (|| {
        let path = activity_path(&team_id)?;
        let log: ActivityLog = read_json(&path)?;

        let limit = (limit as usize).min(MAX_ACTIVITY_ENTRIES).max(1);
        let mut entries = log.entries;
        entries.reverse(); // newest first
        entries.truncate(limit);

        Ok(entries)
    })();

    result.map_err(|e| e.to_json_string())
}

// ===========================================================================
// Tauri Commands — Agent Collaboration
// ===========================================================================

/// Share an AI agent's output to ImpBook as an AgentResult entry.
/// Called automatically when an agent completes a task.
#[tauri::command]
pub async fn team_share_agent_result(
    team_id: String,
    agent_name: String,
    result: String,
    module: Option<String>,
) -> Result<ImpBookEntry, String> {
    let cmd_result: AppResult<ImpBookEntry> = (|| {
        let (user_id, user_name) = whoami();
        let now = Utc::now().to_rfc3339();
        let entry_id = Uuid::new_v4().to_string();

        let title = format!("{} — Result from {}", agent_name, user_name);

        let entry = ImpBookEntry {
            id: entry_id,
            author_id: user_id.clone(),
            author_name: user_name.clone(),
            entry_type: EntryType::AgentResult,
            title: title.clone(),
            content: result,
            source_agent: Some(agent_name.clone()),
            source_module: module,
            tags: vec!["agent-output".to_string(), agent_name.to_lowercase()],
            reactions: Vec::new(),
            comments: Vec::new(),
            attachments: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            pinned: false,
        };

        save_entry(&team_id, &entry)?;

        // Increment contribution
        if let Ok(mut team) = load_team(&team_id) {
            if let Some(member) = team.members.iter_mut().find(|m| m.id == user_id) {
                member.contributions += 1;
            }
            let _ = save_team(&team);
        }

        record_activity(
            &team_id,
            &user_id,
            &user_name,
            &format!("{agent_name} completed"),
            &title,
        )?;

        log::info!("ImpBook: Agent '{}' shared result to team {}", agent_name, team_id);
        Ok(entry)
    })();

    cmd_result.map_err(|e| e.to_json_string())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invite_code_generation() {
        let code = generate_invite_code();
        assert_eq!(code.len(), INVITE_CODE_LEN);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_entry_type_from_str() {
        assert_eq!(EntryType::from_str_loose("agent_result"), EntryType::AgentResult);
        assert_eq!(EntryType::from_str_loose("AgentResult"), EntryType::AgentResult);
        assert_eq!(EntryType::from_str_loose("task"), EntryType::Task);
        assert_eq!(EntryType::from_str_loose("todo"), EntryType::Task);
        assert_eq!(EntryType::from_str_loose("idea"), EntryType::Idea);
        assert_eq!(EntryType::from_str_loose("code_review"), EntryType::CodeReview);
        assert_eq!(EntryType::from_str_loose("codereview"), EntryType::CodeReview);
        assert_eq!(EntryType::from_str_loose("review"), EntryType::CodeReview);
        assert_eq!(EntryType::from_str_loose("discussion"), EntryType::Discussion);
        assert_eq!(EntryType::from_str_loose("milestone"), EntryType::Milestone);
        assert_eq!(EntryType::from_str_loose("unknown"), EntryType::Document);
    }

    #[test]
    fn test_member_role_from_str() {
        assert_eq!(MemberRole::from_str_loose("owner"), MemberRole::Owner);
        assert_eq!(MemberRole::from_str_loose("ADMIN"), MemberRole::Admin);
        assert_eq!(MemberRole::from_str_loose("viewer"), MemberRole::Viewer);
        assert_eq!(MemberRole::from_str_loose("member"), MemberRole::Member);
        assert_eq!(MemberRole::from_str_loose("anything"), MemberRole::Member);
    }

    #[test]
    fn test_member_role_permissions() {
        assert!(MemberRole::Owner.can_edit());
        assert!(MemberRole::Owner.can_manage());
        assert!(MemberRole::Owner.is_owner());

        assert!(MemberRole::Admin.can_edit());
        assert!(MemberRole::Admin.can_manage());
        assert!(!MemberRole::Admin.is_owner());

        assert!(MemberRole::Member.can_edit());
        assert!(!MemberRole::Member.can_manage());

        assert!(!MemberRole::Viewer.can_edit());
        assert!(!MemberRole::Viewer.can_manage());
    }

    #[test]
    fn test_online_status_from_str() {
        assert_eq!(OnlineStatus::from_str_loose("online"), OnlineStatus::Online);
        assert_eq!(OnlineStatus::from_str_loose("AWAY"), OnlineStatus::Away);
        assert_eq!(OnlineStatus::from_str_loose("offline"), OnlineStatus::Offline);
        assert_eq!(OnlineStatus::from_str_loose("unknown"), OnlineStatus::Offline);
    }

    #[test]
    fn test_entry_type_labels() {
        assert_eq!(EntryType::AgentResult.label(), "Agent Result");
        assert_eq!(EntryType::CodeReview.label(), "Code Review");
        assert_eq!(EntryType::Discussion.label(), "Discussion");
        assert_eq!(EntryType::Milestone.label(), "Milestone");
    }

    #[test]
    fn test_whoami_returns_values() {
        let (id, name) = whoami();
        assert!(!id.is_empty());
        assert!(!name.is_empty());
        assert!(id.starts_with("user_"));
    }

    #[test]
    fn test_team_serialization_roundtrip() {
        let team = Team {
            id: "t-1".to_string(),
            name: "Test Team".to_string(),
            members: vec![TeamMember {
                id: "user_test".to_string(),
                name: "Test".to_string(),
                role: MemberRole::Owner,
                status: OnlineStatus::Online,
                active_agents: vec!["CodeAgent".to_string()],
                last_seen: "2026-01-01T00:00:00Z".to_string(),
                trust_score: 1.0,
                contributions: 5,
            }],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            invite_code: "ABCD1234".to_string(),
        };

        let json = serde_json::to_string(&team).expect("serialize");
        let parsed: Team = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.id, "t-1");
        assert_eq!(parsed.name, "Test Team");
        assert_eq!(parsed.members.len(), 1);
        assert_eq!(parsed.members[0].role, MemberRole::Owner);
        assert_eq!(parsed.invite_code, "ABCD1234");
    }

    #[test]
    fn test_entry_serialization_roundtrip() {
        let entry = ImpBookEntry {
            id: "e-1".to_string(),
            author_id: "user_alice".to_string(),
            author_name: "Alice".to_string(),
            entry_type: EntryType::AgentResult,
            title: "Code Review Done".to_string(),
            content: "All tests pass.".to_string(),
            source_agent: Some("CodeAgent".to_string()),
            source_module: Some("ForgeWriter".to_string()),
            tags: vec!["review".to_string()],
            reactions: vec![Reaction {
                emoji: "\u{1f44d}".to_string(),
                user_id: "user_bob".to_string(),
                user_name: "Bob".to_string(),
            }],
            comments: vec![Comment {
                id: "c-1".to_string(),
                author_id: "user_bob".to_string(),
                author_name: "Bob".to_string(),
                content: "Great work!".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                reactions: Vec::new(),
            }],
            attachments: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            pinned: true,
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: ImpBookEntry = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.entry_type, EntryType::AgentResult);
        assert!(parsed.pinned);
        assert_eq!(parsed.reactions.len(), 1);
        assert_eq!(parsed.comments.len(), 1);
        assert_eq!(parsed.source_agent.as_deref(), Some("CodeAgent"));
    }

    #[test]
    fn test_activity_log_default() {
        let log = ActivityLog::default();
        assert!(log.entries.is_empty());
    }

    #[test]
    fn test_team_meta_from_team() {
        let team = Team {
            id: "t-2".to_string(),
            name: "Dev Team".to_string(),
            members: vec![
                TeamMember {
                    id: "u1".to_string(),
                    name: "A".to_string(),
                    role: MemberRole::Owner,
                    status: OnlineStatus::Online,
                    active_agents: Vec::new(),
                    last_seen: String::new(),
                    trust_score: 1.0,
                    contributions: 0,
                },
                TeamMember {
                    id: "u2".to_string(),
                    name: "B".to_string(),
                    role: MemberRole::Member,
                    status: OnlineStatus::Offline,
                    active_agents: Vec::new(),
                    last_seen: String::new(),
                    trust_score: 0.5,
                    contributions: 0,
                },
            ],
            created_at: "2026-03-15T00:00:00Z".to_string(),
            invite_code: "XYZ12345".to_string(),
        };

        let meta = TeamMeta {
            id: team.id.clone(),
            name: team.name.clone(),
            member_count: team.members.len(),
            created_at: team.created_at.clone(),
            invite_code: team.invite_code.clone(),
        };

        assert_eq!(meta.member_count, 2);
        assert_eq!(meta.invite_code, "XYZ12345");
    }
}
