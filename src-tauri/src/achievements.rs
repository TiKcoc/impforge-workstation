// SPDX-License-Identifier: Apache-2.0
//! Achievement & Gamification System
//!
//! Tracks user progress, unlocks achievements, manages XP/levels, and
//! maintains usage streaks. All data persisted to SQLite (WAL mode).
//!
//! ## XP Formula
//! Level thresholds follow: `xp_needed = 100 * level^1.5`
//! This gives a smooth curve that rewards early engagement while keeping
//! long-term goals meaningful.
//!
//! ## Architecture
//! - `AchievementEngine` owns the SQLite connection and all logic
//! - Tauri commands are thin wrappers that delegate to the engine
//! - `track_action` is the primary entry point — call it from any module
//!   when a trackable event occurs (chat sent, doc created, etc.)

use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::ImpForgeError;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AchievementCategory {
    GettingStarted,
    Productivity,
    AiMaster,
    TeamPlayer,
    PowerUser,
    Creator,
    Explorer,
}

impl AchievementCategory {
    fn as_str(&self) -> &'static str {
        match self {
            Self::GettingStarted => "getting_started",
            Self::Productivity => "productivity",
            Self::AiMaster => "ai_master",
            Self::TeamPlayer => "team_player",
            Self::PowerUser => "power_user",
            Self::Creator => "creator",
            Self::Explorer => "explorer",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "getting_started" => Self::GettingStarted,
            "productivity" => Self::Productivity,
            "ai_master" => Self::AiMaster,
            "team_player" => Self::TeamPlayer,
            "power_user" => Self::PowerUser,
            "creator" => Self::Creator,
            "explorer" => Self::Explorer,
            _ => Self::Explorer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "common" => Self::Common,
            "uncommon" => Self::Uncommon,
            "rare" => Self::Rare,
            "epic" => Self::Epic,
            "legendary" => Self::Legendary,
            _ => Self::Common,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: AchievementCategory,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
    pub progress: u32,
    pub target: u32,
    pub xp_reward: u32,
    pub rarity: Rarity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProgress {
    pub level: u32,
    pub total_xp: u64,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub documents_created: u64,
    pub ai_queries: u64,
    pub workflows_run: u64,
    pub modules_used: Vec<String>,
    pub xp_for_next_level: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Achievement Definitions — the 30 canonical achievements
// ─────────────────────────────────────────────────────────────────────────────

fn all_achievement_definitions() -> Vec<Achievement> {
    vec![
        // Getting Started (5)
        ach("first_chat", "First Conversation", "Send your first chat message", "\u{1F4AC}", AchievementCategory::GettingStarted, 1, 10, Rarity::Common),
        ach("first_document", "Author", "Create your first document", "\u{1F4C4}", AchievementCategory::GettingStarted, 1, 10, Rarity::Common),
        ach("first_workflow", "Automator", "Create your first workflow", "\u{26A1}", AchievementCategory::GettingStarted, 1, 15, Rarity::Common),
        ach("profile_complete", "Identity", "Complete your user profile", "\u{1F464}", AchievementCategory::GettingStarted, 1, 10, Rarity::Common),
        ach("tour_complete", "Explorer", "Finish the guided tour", "\u{1F9ED}", AchievementCategory::GettingStarted, 1, 10, Rarity::Common),
        // Productivity (5)
        ach("power_hour", "Power Hour", "Use ImpForge for 1 hour straight", "\u{23F0}", AchievementCategory::Productivity, 1, 25, Rarity::Uncommon),
        ach("streak_7", "Weekly Warrior", "7-day usage streak", "\u{1F525}", AchievementCategory::Productivity, 7, 50, Rarity::Uncommon),
        ach("streak_30", "Monthly Master", "30-day streak", "\u{1F3C6}", AchievementCategory::Productivity, 30, 200, Rarity::Rare),
        ach("docs_10", "Prolific Writer", "Create 10 documents", "\u{270D}\u{FE0F}", AchievementCategory::Productivity, 10, 30, Rarity::Common),
        ach("docs_100", "Novelist", "Create 100 documents", "\u{1F4DA}", AchievementCategory::Productivity, 100, 100, Rarity::Rare),
        // AI Master (5)
        ach("ai_100", "AI Apprentice", "Send 100 AI queries", "\u{1F916}", AchievementCategory::AiMaster, 100, 30, Rarity::Common),
        ach("ai_1000", "AI Master", "Send 1000 AI queries", "\u{1F9E0}", AchievementCategory::AiMaster, 1000, 100, Rarity::Rare),
        ach("moa_first", "Ensemble", "Use Mixture-of-Agents", "\u{1F3AD}", AchievementCategory::AiMaster, 1, 25, Rarity::Uncommon),
        ach("local_only", "Sovereign", "Complete a task 100% offline", "\u{1F3F0}", AchievementCategory::AiMaster, 1, 50, Rarity::Rare),
        ach("confidence_save", "Cost Saver", "Save $1 via confidence routing", "\u{1F4B0}", AchievementCategory::AiMaster, 1, 25, Rarity::Uncommon),
        // Team Player (5)
        ach("team_create", "Leader", "Create a team", "\u{1F451}", AchievementCategory::TeamPlayer, 1, 20, Rarity::Common),
        ach("impbook_10", "Contributor", "Create 10 ImpBook entries", "\u{1F4D6}", AchievementCategory::TeamPlayer, 10, 30, Rarity::Common),
        ach("reactions_50", "Cheerleader", "Give 50 reactions", "\u{1F389}", AchievementCategory::TeamPlayer, 50, 25, Rarity::Uncommon),
        ach("comments_20", "Reviewer", "Write 20 comments", "\u{1F4AC}", AchievementCategory::TeamPlayer, 20, 25, Rarity::Uncommon),
        ach("goal_complete", "Achiever", "Complete a team goal", "\u{1F3AF}", AchievementCategory::TeamPlayer, 1, 50, Rarity::Uncommon),
        // Power User (5)
        ach("modules_10", "Swiss Knife", "Use 10 different modules", "\u{1F527}", AchievementCategory::PowerUser, 10, 50, Rarity::Rare),
        ach("modules_all", "Completionist", "Use ALL modules", "\u{2B50}", AchievementCategory::PowerUser, 30, 200, Rarity::Epic),
        ach("workflow_10", "Automation King", "Run 10 workflows", "\u{1F451}", AchievementCategory::PowerUser, 10, 50, Rarity::Uncommon),
        ach("agentic_cell", "Agent Smith", "Create an Agentic Cell", "\u{1F576}\u{FE0F}", AchievementCategory::PowerUser, 1, 30, Rarity::Uncommon),
        ach("custom_theme", "Designer", "Create a custom theme", "\u{1F3A8}", AchievementCategory::PowerUser, 1, 20, Rarity::Common),
        // Creator (3)
        ach("slides_present", "Speaker", "Give a presentation", "\u{1F3A4}", AchievementCategory::Creator, 1, 30, Rarity::Uncommon),
        ach("social_post", "Influencer", "Post to social media", "\u{1F4F1}", AchievementCategory::Creator, 1, 25, Rarity::Common),
        ach("canvas_export", "Publisher", "Export a ForgeCanvas document", "\u{1F4CA}", AchievementCategory::Creator, 1, 30, Rarity::Uncommon),
        // Explorer (2)
        ach("health_100", "Healthy", "Achieve 100% health score", "\u{1F49A}", AchievementCategory::Explorer, 1, 50, Rarity::Rare),
        ach("all_achievements", "Legend", "Unlock all achievements", "\u{1F31F}", AchievementCategory::Explorer, 29, 500, Rarity::Legendary),
    ]
}

/// Helper to build an achievement definition with default unlocked=false state.
fn ach(
    id: &str,
    name: &str,
    description: &str,
    icon: &str,
    category: AchievementCategory,
    target: u32,
    xp_reward: u32,
    rarity: Rarity,
) -> Achievement {
    Achievement {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        icon: icon.to_string(),
        category,
        unlocked: false,
        unlocked_at: None,
        progress: 0,
        target,
        xp_reward,
        rarity,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// XP / Level calculations
// ─────────────────────────────────────────────────────────────────────────────

/// XP required to reach a given level: `100 * level^1.5`
fn xp_for_level(level: u32) -> u64 {
    (100.0 * (level as f64).powf(1.5)) as u64
}

/// Determine level from total XP.
fn level_from_xp(total_xp: u64) -> u32 {
    let mut level = 1u32;
    while xp_for_level(level + 1) <= total_xp && level < 100 {
        level += 1;
    }
    level
}

// ─────────────────────────────────────────────────────────────────────────────
// Engine — owns the DB connection and all mutation logic
// ─────────────────────────────────────────────────────────────────────────────

pub struct AchievementEngine {
    conn: Mutex<Connection>,
}

impl AchievementEngine {
    /// Open (or create) the achievements database.
    pub fn new(data_dir: &PathBuf) -> Result<Self, ImpForgeError> {
        let db_path = data_dir.join("achievements.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("ACH_DB_OPEN", format!("Failed to open achievements DB: {e}"))
        })?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| {
                ImpForgeError::internal("ACH_DB_PRAGMA", format!("DB pragma failed: {e}"))
            })?;
        let engine = Self {
            conn: Mutex::new(conn),
        };
        engine.init_tables()?;
        engine.seed_achievements()?;
        Ok(engine)
    }

    fn init_tables(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS achievements (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                icon TEXT NOT NULL,
                category TEXT NOT NULL,
                unlocked INTEGER NOT NULL DEFAULT 0,
                unlocked_at TEXT,
                progress INTEGER NOT NULL DEFAULT 0,
                target INTEGER NOT NULL DEFAULT 1,
                xp_reward INTEGER NOT NULL DEFAULT 10,
                rarity TEXT NOT NULL DEFAULT 'common'
            );
            CREATE TABLE IF NOT EXISTS user_progress (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                total_xp INTEGER NOT NULL DEFAULT 0,
                current_streak INTEGER NOT NULL DEFAULT 0,
                longest_streak INTEGER NOT NULL DEFAULT 0,
                last_active_date TEXT,
                documents_created INTEGER NOT NULL DEFAULT 0,
                ai_queries INTEGER NOT NULL DEFAULT 0,
                workflows_run INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS modules_used (
                module TEXT PRIMARY KEY,
                first_used TEXT NOT NULL
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("ACH_INIT", format!("Table creation failed: {e}"))
        })?;
        // Ensure the single user_progress row exists
        conn.execute(
            "INSERT OR IGNORE INTO user_progress (id, total_xp) VALUES (1, 0)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("ACH_SEED_PROGRESS", format!("Progress seed failed: {e}"))
        })?;
        Ok(())
    }

    fn seed_achievements(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;
        let defs = all_achievement_definitions();
        for a in &defs {
            conn.execute(
                "INSERT OR IGNORE INTO achievements (id, name, description, icon, category, target, xp_reward, rarity)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    a.id,
                    a.name,
                    a.description,
                    a.icon,
                    a.category.as_str(),
                    a.target,
                    a.xp_reward,
                    a.rarity.as_str(),
                ],
            )
            .map_err(|e| {
                ImpForgeError::internal("ACH_SEED", format!("Seed failed: {e}"))
            })?;
        }
        Ok(())
    }

    /// List all achievements with their current progress/unlock status.
    pub fn list(&self) -> Result<Vec<Achievement>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, icon, category, unlocked, unlocked_at,
                        progress, target, xp_reward, rarity
                 FROM achievements ORDER BY category, id",
            )
            .map_err(|e| ImpForgeError::internal("ACH_QUERY", e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let cat_str: String = row.get(4)?;
                let rarity_str: String = row.get(10)?;
                Ok(Achievement {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    icon: row.get(3)?,
                    category: AchievementCategory::from_str(&cat_str),
                    unlocked: row.get::<_, i32>(5)? != 0,
                    unlocked_at: row.get(6)?,
                    progress: row.get::<_, u32>(7)?,
                    target: row.get::<_, u32>(8)?,
                    xp_reward: row.get::<_, u32>(9)?,
                    rarity: Rarity::from_str(&rarity_str),
                })
            })
            .map_err(|e| ImpForgeError::internal("ACH_QUERY", e.to_string()))?;
        let mut achievements = Vec::new();
        for r in rows {
            achievements.push(
                r.map_err(|e| ImpForgeError::internal("ACH_ROW", e.to_string()))?,
            );
        }
        Ok(achievements)
    }

    /// Get current user progress (level, xp, streak, counters).
    pub fn get_progress(&self) -> Result<UserProgress, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;
        let (total_xp, current_streak, longest_streak, documents_created, ai_queries, workflows_run): (
            i64, i32, i32, i64, i64, i64,
        ) = conn
            .query_row(
                "SELECT total_xp, current_streak, longest_streak,
                        documents_created, ai_queries, workflows_run
                 FROM user_progress WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .map_err(|e| ImpForgeError::internal("ACH_PROGRESS", e.to_string()))?;

        let mut mod_stmt = conn
            .prepare("SELECT module FROM modules_used ORDER BY module")
            .map_err(|e| ImpForgeError::internal("ACH_MODULES", e.to_string()))?;
        let modules: Vec<String> = mod_stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| ImpForgeError::internal("ACH_MODULES", e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let total_xp = total_xp as u64;
        let level = level_from_xp(total_xp);
        let xp_for_next = xp_for_level(level + 1);

        Ok(UserProgress {
            level,
            total_xp,
            current_streak: current_streak as u32,
            longest_streak: longest_streak as u32,
            documents_created: documents_created as u64,
            ai_queries: ai_queries as u64,
            workflows_run: workflows_run as u64,
            modules_used: modules,
            xp_for_next_level: xp_for_next,
        })
    }

    /// Track an action and return any newly unlocked achievement.
    ///
    /// Actions: "chat", "document", "workflow", "moa", "local_complete",
    /// "confidence_save", "team_create", "impbook", "reaction", "comment",
    /// "goal_complete", "module:{name}", "theme_create", "slides_present",
    /// "social_post", "canvas_export", "health_100", "profile_complete",
    /// "tour_complete", "power_hour"
    pub fn track_action(&self, action: &str) -> Result<Option<Achievement>, ImpForgeError> {
        // Update streak
        self.update_streak()?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Map action to (counter_column, achievement_ids_to_check)
        let checks: Vec<(&str, &str, u32)> = match action {
            "chat" => {
                conn.execute(
                    "UPDATE user_progress SET ai_queries = ai_queries + 1 WHERE id = 1",
                    [],
                )
                .ok();
                let count: u32 = conn
                    .query_row(
                        "SELECT ai_queries FROM user_progress WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                vec![
                    ("first_chat", "first_chat", 1.min(count)),
                    ("ai_100", "ai_100", count),
                    ("ai_1000", "ai_1000", count),
                ]
            }
            "document" => {
                conn.execute(
                    "UPDATE user_progress SET documents_created = documents_created + 1 WHERE id = 1",
                    [],
                )
                .ok();
                let count: u32 = conn
                    .query_row(
                        "SELECT documents_created FROM user_progress WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                vec![
                    ("first_document", "first_document", 1.min(count)),
                    ("docs_10", "docs_10", count),
                    ("docs_100", "docs_100", count),
                ]
            }
            "workflow" => {
                conn.execute(
                    "UPDATE user_progress SET workflows_run = workflows_run + 1 WHERE id = 1",
                    [],
                )
                .ok();
                let count: u32 = conn
                    .query_row(
                        "SELECT workflows_run FROM user_progress WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                vec![
                    ("first_workflow", "first_workflow", 1.min(count)),
                    ("workflow_10", "workflow_10", count),
                ]
            }
            "reaction" => {
                vec![("reactions_50", "reactions_50", 0)] // progress tracked in achievement row
            }
            "comment" => {
                vec![("comments_20", "comments_20", 0)]
            }
            "impbook" => {
                vec![("impbook_10", "impbook_10", 0)]
            }
            _ if action.starts_with("module:") => {
                let module_name = action.strip_prefix("module:").unwrap_or("");
                conn.execute(
                    "INSERT OR IGNORE INTO modules_used (module, first_used) VALUES (?1, ?2)",
                    params![module_name, Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()],
                )
                .ok();
                let count: u32 = conn
                    .query_row("SELECT COUNT(*) FROM modules_used", [], |row| row.get(0))
                    .unwrap_or(0);
                vec![
                    ("modules_10", "modules_10", count),
                    ("modules_all", "modules_all", count),
                ]
            }
            // Simple one-shot achievements
            "moa" => vec![("moa_first", "moa_first", 1)],
            "local_complete" => vec![("local_only", "local_only", 1)],
            "confidence_save" => vec![("confidence_save", "confidence_save", 1)],
            "team_create" => vec![("team_create", "team_create", 1)],
            "goal_complete" => vec![("goal_complete", "goal_complete", 1)],
            "theme_create" => vec![("custom_theme", "custom_theme", 1)],
            "slides_present" => vec![("slides_present", "slides_present", 1)],
            "social_post" => vec![("social_post", "social_post", 1)],
            "canvas_export" => vec![("canvas_export", "canvas_export", 1)],
            "health_100" => vec![("health_100", "health_100", 1)],
            "profile_complete" => vec![("profile_complete", "profile_complete", 1)],
            "tour_complete" => vec![("tour_complete", "tour_complete", 1)],
            "power_hour" => vec![("power_hour", "power_hour", 1)],
            "agentic_cell" => vec![("agentic_cell", "agentic_cell", 1)],
            _ => return Ok(None),
        };

        // For simple incremental achievements (reaction, comment, impbook) — bump progress
        if matches!(action, "reaction" | "comment" | "impbook") {
            let ach_id = checks.first().map(|c| c.0).unwrap_or("");
            conn.execute(
                "UPDATE achievements SET progress = MIN(progress + 1, target) WHERE id = ?1 AND unlocked = 0",
                params![ach_id],
            )
            .ok();
        }

        // Check each candidate for unlock
        let mut newly_unlocked: Option<Achievement> = None;
        for (ach_id, _, new_progress) in &checks {
            let already_unlocked: bool = conn
                .query_row(
                    "SELECT unlocked FROM achievements WHERE id = ?1",
                    params![ach_id],
                    |row| row.get::<_, i32>(0).map(|v| v != 0),
                )
                .unwrap_or(true);

            if already_unlocked {
                continue;
            }

            // Update progress
            if *new_progress > 0 {
                conn.execute(
                    "UPDATE achievements SET progress = ?1 WHERE id = ?2 AND unlocked = 0",
                    params![new_progress, ach_id],
                )
                .ok();
            }

            // Check if target reached
            let (progress, target): (u32, u32) = conn
                .query_row(
                    "SELECT progress, target FROM achievements WHERE id = ?1",
                    params![ach_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, u32::MAX));

            if progress >= target {
                let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                conn.execute(
                    "UPDATE achievements SET unlocked = 1, unlocked_at = ?1 WHERE id = ?2",
                    params![now, ach_id],
                )
                .ok();

                // Award XP
                let xp: u32 = conn
                    .query_row(
                        "SELECT xp_reward FROM achievements WHERE id = ?1",
                        params![ach_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                conn.execute(
                    "UPDATE user_progress SET total_xp = total_xp + ?1 WHERE id = 1",
                    params![xp],
                )
                .ok();

                // Load the unlocked achievement to return
                if newly_unlocked.is_none() {
                    newly_unlocked = conn
                        .query_row(
                            "SELECT id, name, description, icon, category, unlocked, unlocked_at,
                                    progress, target, xp_reward, rarity
                             FROM achievements WHERE id = ?1",
                            params![ach_id],
                            |row| {
                                let cat_str: String = row.get(4)?;
                                let rarity_str: String = row.get(10)?;
                                Ok(Achievement {
                                    id: row.get(0)?,
                                    name: row.get(1)?,
                                    description: row.get(2)?,
                                    icon: row.get(3)?,
                                    category: AchievementCategory::from_str(&cat_str),
                                    unlocked: true,
                                    unlocked_at: row.get(6)?,
                                    progress: row.get(7)?,
                                    target: row.get(8)?,
                                    xp_reward: row.get(9)?,
                                    rarity: Rarity::from_str(&rarity_str),
                                })
                            },
                        )
                        .ok();

                    // Check "all_achievements" meta-achievement
                    self.check_all_achievements_unlocked(&conn);
                }
            }
        }

        // Also check streak achievements
        drop(conn);
        if newly_unlocked.is_none() {
            newly_unlocked = self.check_streak_achievements()?;
        }

        Ok(newly_unlocked)
    }

    /// Update the daily streak counter.
    fn update_streak(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let last_date: Option<String> = conn
            .query_row(
                "SELECT last_active_date FROM user_progress WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(None);

        match last_date {
            Some(ref d) if d == &today => {
                // Already active today — no change
            }
            Some(ref d) => {
                // Check if yesterday
                if let Ok(last) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                    if let Ok(tod) = NaiveDate::parse_from_str(&today, "%Y-%m-%d") {
                        let diff = (tod - last).num_days();
                        if diff == 1 {
                            // Consecutive day
                            conn.execute(
                                "UPDATE user_progress SET current_streak = current_streak + 1,
                                 longest_streak = MAX(longest_streak, current_streak + 1),
                                 last_active_date = ?1 WHERE id = 1",
                                params![today],
                            )
                            .ok();
                        } else {
                            // Streak broken
                            conn.execute(
                                "UPDATE user_progress SET current_streak = 1, last_active_date = ?1 WHERE id = 1",
                                params![today],
                            )
                            .ok();
                        }
                    }
                }
            }
            None => {
                // First ever active day
                conn.execute(
                    "UPDATE user_progress SET current_streak = 1, longest_streak = 1, last_active_date = ?1 WHERE id = 1",
                    params![today],
                )
                .ok();
            }
        }

        Ok(())
    }

    fn check_streak_achievements(&self) -> Result<Option<Achievement>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ACH_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let streak: u32 = conn
            .query_row(
                "SELECT current_streak FROM user_progress WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Update streak progress on the achievements
        conn.execute(
            "UPDATE achievements SET progress = ?1 WHERE id = 'streak_7' AND unlocked = 0",
            params![streak.min(7)],
        )
        .ok();
        conn.execute(
            "UPDATE achievements SET progress = ?1 WHERE id = 'streak_30' AND unlocked = 0",
            params![streak.min(30)],
        )
        .ok();

        for (ach_id, required) in [("streak_7", 7u32), ("streak_30", 30)] {
            if streak >= required {
                let already: bool = conn
                    .query_row(
                        "SELECT unlocked FROM achievements WHERE id = ?1",
                        params![ach_id],
                        |row| row.get::<_, i32>(0).map(|v| v != 0),
                    )
                    .unwrap_or(true);

                if !already {
                    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                    conn.execute(
                        "UPDATE achievements SET unlocked = 1, unlocked_at = ?1, progress = target WHERE id = ?2",
                        params![now, ach_id],
                    )
                    .ok();
                    let xp: u32 = conn
                        .query_row(
                            "SELECT xp_reward FROM achievements WHERE id = ?1",
                            params![ach_id],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);
                    conn.execute(
                        "UPDATE user_progress SET total_xp = total_xp + ?1 WHERE id = 1",
                        params![xp],
                    )
                    .ok();

                    return conn
                        .query_row(
                            "SELECT id, name, description, icon, category, unlocked, unlocked_at,
                                    progress, target, xp_reward, rarity
                             FROM achievements WHERE id = ?1",
                            params![ach_id],
                            |row| {
                                let cat_str: String = row.get(4)?;
                                let rarity_str: String = row.get(10)?;
                                Ok(Some(Achievement {
                                    id: row.get(0)?,
                                    name: row.get(1)?,
                                    description: row.get(2)?,
                                    icon: row.get(3)?,
                                    category: AchievementCategory::from_str(&cat_str),
                                    unlocked: true,
                                    unlocked_at: row.get(6)?,
                                    progress: row.get(7)?,
                                    target: row.get(8)?,
                                    xp_reward: row.get(9)?,
                                    rarity: Rarity::from_str(&rarity_str),
                                }))
                            },
                        )
                        .map_err(|e| ImpForgeError::internal("ACH_STREAK", e.to_string()));
                }
            }
        }

        Ok(None)
    }

    fn check_all_achievements_unlocked(&self, conn: &Connection) {
        let unlocked_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE unlocked = 1 AND id != 'all_achievements'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let total_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE id != 'all_achievements'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(30);

        // Update progress on meta-achievement
        conn.execute(
            "UPDATE achievements SET progress = ?1 WHERE id = 'all_achievements'",
            params![unlocked_count],
        )
        .ok();

        if unlocked_count >= total_count {
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            conn.execute(
                "UPDATE achievements SET unlocked = 1, unlocked_at = ?1 WHERE id = 'all_achievements'",
                params![now],
            )
            .ok();
            conn.execute(
                "UPDATE user_progress SET total_xp = total_xp + 500 WHERE id = 1",
                [],
            )
            .ok();
        }
    }

    /// Get level, current XP, and XP needed for next level.
    pub fn get_level(&self) -> Result<(u32, u64, u64), ImpForgeError> {
        let progress = self.get_progress()?;
        Ok((progress.level, progress.total_xp, progress.xp_for_next_level))
    }

    /// Get current and longest streak.
    pub fn get_streak(&self) -> Result<(u32, u32), ImpForgeError> {
        self.update_streak()?;
        let progress = self.get_progress()?;
        Ok((progress.current_streak, progress.longest_streak))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn achievements_list(
    engine: tauri::State<'_, AchievementEngine>,
) -> Result<Vec<Achievement>, ImpForgeError> {
    engine.list()
}

#[tauri::command]
pub fn achievements_get_progress(
    engine: tauri::State<'_, AchievementEngine>,
) -> Result<UserProgress, ImpForgeError> {
    engine.get_progress()
}

#[tauri::command]
pub fn achievements_track_action(
    engine: tauri::State<'_, AchievementEngine>,
    action: String,
) -> Result<Option<Achievement>, ImpForgeError> {
    engine.track_action(&action)
}

#[tauri::command]
pub fn achievements_get_level(
    engine: tauri::State<'_, AchievementEngine>,
) -> Result<(u32, u64, u64), ImpForgeError> {
    engine.get_level()
}

#[tauri::command]
pub fn achievements_get_streak(
    engine: tauri::State<'_, AchievementEngine>,
) -> Result<(u32, u32), ImpForgeError> {
    engine.get_streak()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> AchievementEngine {
        let dir = std::env::temp_dir().join(format!("impforge_ach_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        AchievementEngine::new(&dir).expect("create engine")
    }

    #[test]
    fn test_seed_30_achievements() {
        let engine = test_engine();
        let list = engine.list().expect("list");
        assert_eq!(list.len(), 30);
        assert!(list.iter().all(|a| !a.unlocked));
    }

    #[test]
    fn test_xp_formula() {
        assert_eq!(xp_for_level(1), 100);
        assert_eq!(xp_for_level(2), 282); // 100 * 2^1.5
        assert!(xp_for_level(10) > xp_for_level(9));
    }

    #[test]
    fn test_level_from_xp() {
        assert_eq!(level_from_xp(0), 1);
        assert_eq!(level_from_xp(100), 1);
        assert_eq!(level_from_xp(283), 2);
        assert_eq!(level_from_xp(10_000), level_from_xp(10_000));
    }

    #[test]
    fn test_track_chat_unlocks_first_chat() {
        let engine = test_engine();
        let result = engine.track_action("chat").expect("track");
        assert!(result.is_some());
        let ach = result.as_ref().expect("achievement");
        assert_eq!(ach.id, "first_chat");
        assert!(ach.unlocked);
        assert_eq!(ach.xp_reward, 10);

        // XP should be awarded
        let (level, xp, _) = engine.get_level().expect("level");
        assert_eq!(xp, 10);
        assert_eq!(level, 1);
    }

    #[test]
    fn test_duplicate_unlock_ignored() {
        let engine = test_engine();
        let first = engine.track_action("chat").expect("first");
        assert!(first.is_some());
        let second = engine.track_action("chat").expect("second");
        // first_chat already unlocked, ai_100 not yet reached
        assert!(second.is_none());
    }

    #[test]
    fn test_progress_increments() {
        let engine = test_engine();
        for _ in 0..5 {
            engine.track_action("chat").ok();
        }
        let progress = engine.get_progress().expect("progress");
        assert_eq!(progress.ai_queries, 5);
    }

    #[test]
    fn test_module_tracking() {
        let engine = test_engine();
        engine.track_action("module:chat").ok();
        engine.track_action("module:github").ok();
        engine.track_action("module:docker").ok();
        let progress = engine.get_progress().expect("progress");
        assert_eq!(progress.modules_used.len(), 3);
    }

    #[test]
    fn test_document_achievements() {
        let engine = test_engine();
        let first = engine.track_action("document").expect("first doc");
        assert!(first.is_some());
        assert_eq!(first.as_ref().map(|a| a.id.as_str()), Some("first_document"));

        for _ in 0..9 {
            engine.track_action("document").ok();
        }
        let progress = engine.get_progress().expect("progress");
        assert_eq!(progress.documents_created, 10);
    }

    #[test]
    fn test_streak_tracking() {
        let engine = test_engine();
        engine.track_action("chat").ok();
        let (current, longest) = engine.get_streak().expect("streak");
        assert!(current >= 1);
        assert!(longest >= 1);
    }

    #[test]
    fn test_all_categories_present() {
        let defs = all_achievement_definitions();
        let categories: std::collections::HashSet<String> =
            defs.iter().map(|a| a.category.as_str().to_string()).collect();
        assert!(categories.contains("getting_started"));
        assert!(categories.contains("productivity"));
        assert!(categories.contains("ai_master"));
        assert!(categories.contains("team_player"));
        assert!(categories.contains("power_user"));
        assert!(categories.contains("creator"));
        assert!(categories.contains("explorer"));
    }

    #[test]
    fn test_one_shot_achievements() {
        let engine = test_engine();
        for action in [
            "moa", "local_complete", "confidence_save", "team_create",
            "theme_create", "slides_present", "social_post", "canvas_export",
            "health_100", "profile_complete", "tour_complete", "power_hour",
            "agentic_cell",
        ] {
            let result = engine.track_action(action).expect(action);
            assert!(result.is_some(), "Expected unlock for action: {action}");
        }
    }
}
