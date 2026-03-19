// SPDX-License-Identifier: Apache-2.0
//! ForgeQuest -- Idle RPG Gamification System
//!
//! A medieval fantasy idle RPG where the user's REAL productivity powers their
//! character. Writing documents = crafting weapons, running workflows = fighting
//! monsters, AI queries = casting spells.
//!
//! ## Architecture
//! - `ForgeQuestEngine` owns the SQLite connection (WAL mode) and all game logic
//! - Tauri commands are thin wrappers that delegate to the engine
//! - `quest_track_action` is the primary entry-point -- call it from any module
//!   when a trackable event occurs to grant XP, gold, materials, and auto-battles
//!
//! ## XP Formula
//! Level thresholds follow: `xp_needed = 150 * level^1.6`
//! This keeps early levels fast (encouraging engagement) while making high
//! levels a long-term aspiration.

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::error::ImpForgeError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterClass {
    Warrior,    // Bonus from documents (strength)
    Mage,       // Bonus from AI queries (magic)
    Ranger,     // Bonus from workflows (speed)
    Blacksmith, // Bonus from spreadsheets (crafting)
    Bard,       // Bonus from social media (charisma)
    Scholar,    // Bonus from notes/research (wisdom)
}

impl CharacterClass {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Warrior => "warrior",
            Self::Mage => "mage",
            Self::Ranger => "ranger",
            Self::Blacksmith => "blacksmith",
            Self::Bard => "bard",
            Self::Scholar => "scholar",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "warrior" => Self::Warrior,
            "mage" => Self::Mage,
            "ranger" => Self::Ranger,
            "blacksmith" => Self::Blacksmith,
            "bard" => Self::Bard,
            "scholar" => Self::Scholar,
            _ => Self::Warrior,
        }
    }

    /// Class-specific stat bonus multiplier for matching actions.
    fn bonus_multiplier(&self, action: &str) -> f64 {
        match (self, action) {
            (Self::Warrior, "create_document") => 1.5,
            (Self::Mage, "ai_query") => 1.5,
            (Self::Ranger, "run_workflow") => 1.5,
            (Self::Blacksmith, "create_spreadsheet") => 1.5,
            (Self::Bard, "social_post") => 1.5,
            (Self::Scholar, "create_note") => 1.5,
            _ => 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl ItemRarity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
            Self::Mythic => "mythic",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "common" => Self::Common,
            "uncommon" => Self::Uncommon,
            "rare" => Self::Rare,
            "epic" => Self::Epic,
            "legendary" => Self::Legendary,
            "mythic" => Self::Mythic,
            _ => Self::Common,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WeaponType {
    Sword,
    Staff,
    Bow,
    Hammer,
    Lute,
    Tome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Boots,
    Gloves,
    Shield,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ItemType {
    Weapon(WeaponType),
    Armor(ArmorSlot),
    Accessory,
    Material,
    Potion,
    QuestItem,
}

#[allow(dead_code)]
impl ItemType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Weapon(_) => "weapon",
            Self::Armor(_) => "armor",
            Self::Accessory => "accessory",
            Self::Material => "material",
            Self::Potion => "potion",
            Self::QuestItem => "quest_item",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStats {
    pub attack: i32,
    pub defense: i32,
    pub magic: i32,
    pub hp_bonus: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub rarity: ItemRarity,
    pub stats: ItemStats,
    pub level_req: u32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipment {
    pub weapon: Option<Item>,
    pub head: Option<Item>,
    pub chest: Option<Item>,
    pub legs: Option<Item>,
    pub boots: Option<Item>,
    pub gloves: Option<Item>,
    pub accessory1: Option<Item>,
    pub accessory2: Option<Item>,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            weapon: None,
            head: None,
            chest: None,
            legs: None,
            boots: None,
            gloves: None,
            accessory1: None,
            accessory2: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SkillBranch {
    Combat,
    Defense,
    Magic,
    Crafting,
    Leadership,
    Wisdom,
}

#[allow(dead_code)]
impl SkillBranch {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Combat => "combat",
            Self::Defense => "defense",
            Self::Magic => "magic",
            Self::Crafting => "crafting",
            Self::Leadership => "leadership",
            Self::Wisdom => "wisdom",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "combat" => Self::Combat,
            "defense" => Self::Defense,
            "magic" => Self::Magic,
            "crafting" => Self::Crafting,
            "leadership" => Self::Leadership,
            "wisdom" => Self::Wisdom,
            _ => Self::Combat,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: u32,
    pub points_invested: u32,
    pub max_points: u32,
    pub prerequisite: Option<String>,
    pub effect: String,
    pub branch: SkillBranch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestCharacter {
    pub name: String,
    pub class: CharacterClass,
    pub level: u32,
    pub xp: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub magic: u32,
    pub gold: u64,
    pub inventory: Vec<Item>,
    pub equipped: Equipment,
    pub skill_points: u32,
    pub skills: Vec<Skill>,
    pub quests_completed: u32,
    pub monsters_slain: u64,
    pub current_zone: String,
    pub guild: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monster {
    pub name: String,
    pub level: u32,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub xp_reward: u64,
    pub gold_reward: u64,
    pub loot_table: Vec<(String, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum QuestObjective {
    CreateDocuments(u32),
    RunWorkflows(u32),
    AiQueries(u32),
    SlayMonsters(u32),
    CraftItems(u32),
    ReachLevel(u32),
    CompleteStreak(u32),
    UseModules(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub objective: String,
    pub objective_target: u32,
    pub objective_progress: u32,
    pub reward_xp: u64,
    pub reward_gold: u64,
    pub reward_items: Vec<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRecipe {
    pub id: String,
    pub name: String,
    pub result_item_id: String,
    pub materials: Vec<(String, u32)>,
    pub required_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level_min: u32,
    pub level_max: u32,
    pub monsters: Vec<Monster>,
    pub boss: Option<Monster>,
    pub unlock_condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpgReward {
    pub xp: u64,
    pub gold: u64,
    pub material: Option<String>,
    pub monster_fight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleResult {
    pub victory: bool,
    pub monster_name: String,
    pub monster_level: u32,
    pub damage_dealt: u32,
    pub damage_taken: u32,
    pub xp_earned: u64,
    pub gold_earned: u64,
    pub loot: Vec<Item>,
    pub rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub xp_earned: u64,
    pub gold_earned: u64,
    pub material_gained: Option<String>,
    pub level_up: bool,
    pub new_level: u32,
    pub battle: Option<BattleResult>,
    pub quest_completed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub name: String,
    pub class: CharacterClass,
    pub level: u32,
    pub xp: u64,
    pub monsters_slain: u64,
    pub quests_completed: u32,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub struct ForgeQuestEngine {
    conn: Mutex<Connection>,
}

impl ForgeQuestEngine {
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem("QUEST_DIR", format!("Cannot create quest data dir: {e}"))
        })?;

        let db_path = data_dir.join("forge_quest.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("QUEST_DB_OPEN", format!("Cannot open quest DB: {e}"))
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_DB_PRAGMA", format!("Pragma failed: {e}"))
        })?;

        let engine = Self {
            conn: Mutex::new(conn),
        };
        engine.init_tables()?;
        Ok(engine)
    }

    fn init_tables(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS quest_character (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                name TEXT NOT NULL,
                class TEXT NOT NULL DEFAULT 'warrior',
                level INTEGER NOT NULL DEFAULT 1,
                xp INTEGER NOT NULL DEFAULT 0,
                hp INTEGER NOT NULL DEFAULT 100,
                max_hp INTEGER NOT NULL DEFAULT 100,
                attack INTEGER NOT NULL DEFAULT 10,
                defense INTEGER NOT NULL DEFAULT 5,
                magic INTEGER NOT NULL DEFAULT 5,
                gold INTEGER NOT NULL DEFAULT 0,
                skill_points INTEGER NOT NULL DEFAULT 0,
                quests_completed INTEGER NOT NULL DEFAULT 0,
                monsters_slain INTEGER NOT NULL DEFAULT 0,
                current_zone TEXT NOT NULL DEFAULT 'beginners_meadow',
                guild TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS quest_inventory (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                item_type TEXT NOT NULL,
                rarity TEXT NOT NULL DEFAULT 'common',
                attack INTEGER NOT NULL DEFAULT 0,
                defense INTEGER NOT NULL DEFAULT 0,
                magic INTEGER NOT NULL DEFAULT 0,
                hp_bonus INTEGER NOT NULL DEFAULT 0,
                level_req INTEGER NOT NULL DEFAULT 1,
                description TEXT NOT NULL DEFAULT '',
                equipped_slot TEXT
            );

            CREATE TABLE IF NOT EXISTS quest_skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                tier INTEGER NOT NULL DEFAULT 1,
                points_invested INTEGER NOT NULL DEFAULT 0,
                max_points INTEGER NOT NULL DEFAULT 5,
                prerequisite TEXT,
                effect TEXT NOT NULL,
                branch TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS quest_quests (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                objective TEXT NOT NULL,
                objective_target INTEGER NOT NULL DEFAULT 1,
                objective_progress INTEGER NOT NULL DEFAULT 0,
                reward_xp INTEGER NOT NULL DEFAULT 0,
                reward_gold INTEGER NOT NULL DEFAULT 0,
                reward_items TEXT NOT NULL DEFAULT '[]',
                completed INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS quest_battle_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monster_name TEXT NOT NULL,
                monster_level INTEGER NOT NULL,
                victory INTEGER NOT NULL,
                xp_earned INTEGER NOT NULL DEFAULT 0,
                gold_earned INTEGER NOT NULL DEFAULT 0,
                fought_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_DB_INIT", format!("Table creation failed: {e}"))
        })?;

        Ok(())
    }

    // -- Character management -------------------------------------------------

    pub fn create_character(
        &self,
        name: &str,
        class: &str,
    ) -> Result<QuestCharacter, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if character already exists
        let exists: bool = conn
            .query_row("SELECT COUNT(*) FROM quest_character", [], |r| r.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if exists {
            return Err(ImpForgeError::validation(
                "QUEST_CHAR_EXISTS",
                "Character already exists. Use quest_get_character instead.",
            ));
        }

        let class_enum = CharacterClass::from_str(class);
        let (hp, atk, def, mag) = match &class_enum {
            CharacterClass::Warrior => (120, 14, 8, 3),
            CharacterClass::Mage => (80, 5, 4, 16),
            CharacterClass::Ranger => (100, 11, 5, 8),
            CharacterClass::Blacksmith => (110, 12, 10, 3),
            CharacterClass::Bard => (90, 7, 5, 12),
            CharacterClass::Scholar => (85, 6, 4, 14),
        };

        conn.execute(
            "INSERT INTO quest_character (id, name, class, hp, max_hp, attack, defense, magic)
             VALUES (1, ?1, ?2, ?3, ?3, ?4, ?5, ?6)",
            params![name, class_enum.as_str(), hp, atk, def, mag],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_CHAR_CREATE", format!("Create failed: {e}"))
        })?;

        // Seed starter quests
        self.seed_quests(&conn)?;
        // Seed default skills
        self.seed_skills(&conn)?;

        drop(conn);
        self.get_character()
    }

    pub fn get_character(&self) -> Result<QuestCharacter, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let char_row = conn
            .query_row("SELECT * FROM quest_character WHERE id = 1", [], |row| {
                Ok((
                    row.get::<_, String>(1)?,  // name
                    row.get::<_, String>(2)?,  // class
                    row.get::<_, u32>(3)?,     // level
                    row.get::<_, u64>(4)?,     // xp
                    row.get::<_, u32>(5)?,     // hp
                    row.get::<_, u32>(6)?,     // max_hp
                    row.get::<_, u32>(7)?,     // attack
                    row.get::<_, u32>(8)?,     // defense
                    row.get::<_, u32>(9)?,     // magic
                    row.get::<_, u64>(10)?,    // gold
                    row.get::<_, u32>(11)?,    // skill_points
                    row.get::<_, u32>(12)?,    // quests_completed
                    row.get::<_, u64>(13)?,    // monsters_slain
                    row.get::<_, String>(14)?, // current_zone
                    row.get::<_, Option<String>>(15)?, // guild
                ))
            })
            .map_err(|_| {
                ImpForgeError::validation(
                    "QUEST_NO_CHAR",
                    "No character found. Create one with quest_create_character.",
                )
            })?;

        let inventory = self.load_inventory(&conn)?;
        let equipped = self.build_equipment(&conn)?;
        let skills = self.load_skills(&conn)?;

        Ok(QuestCharacter {
            name: char_row.0,
            class: CharacterClass::from_str(&char_row.1),
            level: char_row.2,
            xp: char_row.3,
            hp: char_row.4,
            max_hp: char_row.5,
            attack: char_row.6,
            defense: char_row.7,
            magic: char_row.8,
            gold: char_row.9,
            inventory,
            equipped,
            skill_points: char_row.10,
            skills,
            quests_completed: char_row.11,
            monsters_slain: char_row.12,
            current_zone: char_row.13,
            guild: char_row.14,
        })
    }

    // -- Action tracking (productivity -> RPG) --------------------------------

    pub fn track_action(&self, action: &str) -> Result<ActionResult, ImpForgeError> {
        let reward = map_action_to_rpg(action);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load current character stats for class bonus
        let (class_str, level, xp, gold, skill_pts, current_zone): (String, u32, u64, u64, u32, String) = conn
            .query_row(
                "SELECT class, level, xp, gold, skill_points, current_zone FROM quest_character WHERE id = 1",
                [],
                |row| Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?,
                    row.get(3)?, row.get(4)?, row.get(5)?,
                )),
            )
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        let class = CharacterClass::from_str(&class_str);
        let multiplier = class.bonus_multiplier(action);

        let xp_earned = (reward.xp as f64 * multiplier) as u64;
        let gold_earned = (reward.gold as f64 * multiplier) as u64;
        let new_xp = xp + xp_earned;
        let new_gold = gold + gold_earned;

        // Level-up check
        let old_level = level;
        let mut current_level = level;
        let mut accumulated_sp = skill_pts;
        loop {
            let needed = xp_for_level(current_level + 1);
            if new_xp >= needed {
                current_level += 1;
                accumulated_sp += 2; // 2 skill points per level
            } else {
                break;
            }
            if current_level >= 100 {
                break;
            }
        }

        let level_up = current_level > old_level;

        // Update character
        conn.execute(
            "UPDATE quest_character SET xp = ?1, gold = ?2, level = ?3, skill_points = ?4 WHERE id = 1",
            params![new_xp, new_gold, current_level, accumulated_sp],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_UPDATE", format!("Update failed: {e}"))
        })?;

        // If leveled up, increase stats
        if level_up {
            conn.execute(
                "UPDATE quest_character SET
                    max_hp = max_hp + ?1,
                    hp = min(hp + ?1, max_hp + ?1),
                    attack = attack + ?2,
                    defense = defense + ?3,
                    magic = magic + ?4
                 WHERE id = 1",
                params![
                    5 * (current_level - old_level),
                    2 * (current_level - old_level),
                    1 * (current_level - old_level),
                    1 * (current_level - old_level),
                ],
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_LEVELUP", format!("Level-up update failed: {e}"))
            })?;
        }

        // Grant material if any
        if let Some(ref mat_name) = reward.material {
            self.grant_material(&conn, mat_name)?;
        }

        // Auto-battle if the action triggers it
        let battle = if reward.monster_fight {
            Some(self.run_auto_battle(&conn, &current_zone, current_level)?)
        } else {
            None
        };

        // Update quest progress
        let quest_completed = self.update_quest_progress(&conn, action)?;

        Ok(ActionResult {
            xp_earned,
            gold_earned,
            material_gained: reward.material,
            level_up,
            new_level: current_level,
            battle,
            quest_completed,
        })
    }

    // -- Auto-battle ----------------------------------------------------------

    pub fn auto_battle(&self, zone_id: &str) -> Result<BattleResult, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let level: u32 = conn
            .query_row("SELECT level FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        self.run_auto_battle(&conn, zone_id, level)
    }

    fn run_auto_battle(
        &self,
        conn: &Connection,
        zone_id: &str,
        char_level: u32,
    ) -> Result<BattleResult, ImpForgeError> {
        let zones = all_zones();
        let zone = zones
            .iter()
            .find(|z| z.id == zone_id)
            .unwrap_or_else(|| &zones[0]);

        // Pick a monster from the zone (deterministic based on timestamp to avoid
        // needing the `rand` crate -- keeps the dependency list clean)
        let now = Utc::now().timestamp_millis() as usize;
        let monster = if zone.monsters.is_empty() {
            zone.boss.as_ref().unwrap_or(&zone.monsters[0]).clone()
        } else {
            let idx = now % zone.monsters.len();
            zone.monsters[idx].clone()
        };

        // Load character combat stats
        let (hp, atk, def, mag): (u32, u32, u32, u32) = conn
            .query_row(
                "SELECT hp, attack, defense, magic FROM quest_character WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_BATTLE", format!("Load stats failed: {e}"))
            })?;

        // Add equipment bonuses
        let eq_atk: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(attack), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let eq_def: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(defense), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let eq_mag: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(magic), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let total_atk = (atk as i32 + eq_atk).max(1) as u32;
        let total_def = (def as i32 + eq_def).max(0) as u32;
        let total_mag = (mag as i32 + eq_mag).max(0) as u32;
        let effective_power = total_atk + total_mag / 2;

        // Simple turn-based combat
        let mut char_hp = hp as i32;
        let mut mon_hp = monster.hp as i32;
        let mut rounds: u32 = 0;
        let mut total_damage_dealt: u32 = 0;
        let mut total_damage_taken: u32 = 0;

        while char_hp > 0 && mon_hp > 0 && rounds < 50 {
            rounds += 1;

            // Character attacks monster
            let char_dmg = (effective_power as i32 - monster.defense as i32 / 2).max(1);
            mon_hp -= char_dmg;
            total_damage_dealt += char_dmg as u32;

            if mon_hp <= 0 {
                break;
            }

            // Monster attacks character
            let mon_dmg = (monster.attack as i32 - total_def as i32 / 2).max(1);
            char_hp -= mon_dmg;
            total_damage_taken += mon_dmg as u32;
        }

        let victory = mon_hp <= 0;
        let xp_earned = if victory { monster.xp_reward } else { monster.xp_reward / 4 };
        let gold_earned = if victory { monster.gold_reward } else { 0 };

        // Update character HP and stats
        let new_hp = char_hp.max(1) as u32;
        conn.execute(
            "UPDATE quest_character SET hp = ?1 WHERE id = 1",
            params![new_hp],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_HP", format!("HP update failed: {e}"))
        })?;

        if victory {
            conn.execute(
                "UPDATE quest_character SET
                    xp = xp + ?1,
                    gold = gold + ?2,
                    monsters_slain = monsters_slain + 1
                 WHERE id = 1",
                params![xp_earned, gold_earned],
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_VICTORY", format!("Victory update failed: {e}"))
            })?;
        }

        // Log the battle
        conn.execute(
            "INSERT INTO quest_battle_log (monster_name, monster_level, victory, xp_earned, gold_earned)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![monster.name, monster.level, victory as i32, xp_earned, gold_earned],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_BLOG", format!("Battle log failed: {e}"))
        })?;

        // Generate loot on victory
        let loot = if victory {
            self.generate_loot(conn, &monster, char_level)?
        } else {
            Vec::new()
        };

        Ok(BattleResult {
            victory,
            monster_name: monster.name,
            monster_level: monster.level,
            damage_dealt: total_damage_dealt,
            damage_taken: total_damage_taken,
            xp_earned,
            gold_earned,
            loot,
            rounds,
        })
    }

    // -- Crafting --------------------------------------------------------------

    pub fn craft_item(&self, recipe_id: &str) -> Result<Item, ImpForgeError> {
        let recipes = all_recipes();
        let recipe = recipes
            .iter()
            .find(|r| r.id == recipe_id)
            .ok_or_else(|| {
                ImpForgeError::validation("QUEST_NO_RECIPE", format!("Unknown recipe: {recipe_id}"))
            })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check level requirement
        let level: u32 = conn
            .query_row("SELECT level FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        if level < recipe.required_level {
            return Err(ImpForgeError::validation(
                "QUEST_LOW_LEVEL",
                format!("Requires level {}. You are level {}.", recipe.required_level, level),
            ));
        }

        // Check materials
        for (mat_id, needed) in &recipe.materials {
            let count: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM quest_inventory WHERE id LIKE ?1 AND item_type = 'material'",
                    params![format!("{mat_id}%")],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            if count < *needed {
                return Err(ImpForgeError::validation(
                    "QUEST_NO_MATERIALS",
                    format!("Need {needed}x {mat_id}, have {count}."),
                ));
            }
        }

        // Consume materials
        for (mat_id, needed) in &recipe.materials {
            let ids: Vec<String> = {
                let mut stmt = conn
                    .prepare("SELECT id FROM quest_inventory WHERE id LIKE ?1 AND item_type = 'material' LIMIT ?2")
                    .map_err(|e| ImpForgeError::internal("QUEST_CRAFT", format!("{e}")))?;
                let rows = stmt
                    .query_map(params![format!("{mat_id}%"), needed], |r| r.get(0))
                    .map_err(|e| ImpForgeError::internal("QUEST_CRAFT", format!("{e}")))?;
                rows.filter_map(|r| r.ok()).collect()
            };
            for id in ids {
                conn.execute("DELETE FROM quest_inventory WHERE id = ?1", params![id])
                    .map_err(|e| {
                        ImpForgeError::internal("QUEST_CRAFT_DEL", format!("{e}"))
                    })?;
            }
        }

        // Create the crafted item
        let item = generate_item_from_recipe(recipe, level);
        conn.execute(
            "INSERT INTO quest_inventory (id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item.id,
                item.name,
                item.item_type,
                item.rarity.as_str(),
                item.stats.attack,
                item.stats.defense,
                item.stats.magic,
                item.stats.hp_bonus,
                item.level_req,
                item.description,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_CRAFT_INSERT", format!("{e}"))
        })?;

        Ok(item)
    }

    // -- Equipment management -------------------------------------------------

    pub fn equip_item(&self, item_id: &str, slot: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Unequip current item in that slot
        conn.execute(
            "UPDATE quest_inventory SET equipped_slot = NULL WHERE equipped_slot = ?1",
            params![slot],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_UNEQUIP", format!("{e}")))?;

        // Equip the new item
        let rows = conn
            .execute(
                "UPDATE quest_inventory SET equipped_slot = ?1 WHERE id = ?2 AND item_type != 'material'",
                params![slot, item_id],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_EQUIP", format!("{e}")))?;

        if rows == 0 {
            return Err(ImpForgeError::validation(
                "QUEST_ITEM_NOT_FOUND",
                format!("Item '{item_id}' not found or is a material."),
            ));
        }

        Ok(())
    }

    pub fn unequip(&self, slot: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE quest_inventory SET equipped_slot = NULL WHERE equipped_slot = ?1",
            params![slot],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_UNEQUIP", format!("{e}")))?;

        Ok(())
    }

    // -- Skills ---------------------------------------------------------------

    pub fn invest_skill(&self, skill_id: &str) -> Result<Skill, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check skill points
        let sp: u32 = conn
            .query_row("SELECT skill_points FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        if sp == 0 {
            return Err(ImpForgeError::validation(
                "QUEST_NO_SP",
                "No skill points available.",
            ));
        }

        // Check the skill exists and is not maxed
        let (pts, max_pts): (u32, u32) = conn
            .query_row(
                "SELECT points_invested, max_points FROM quest_skills WHERE id = ?1",
                params![skill_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "QUEST_SKILL_NOT_FOUND",
                    format!("Skill '{skill_id}' not found."),
                )
            })?;

        if pts >= max_pts {
            return Err(ImpForgeError::validation(
                "QUEST_SKILL_MAX",
                format!("Skill '{skill_id}' is already maxed ({max_pts}/{max_pts})."),
            ));
        }

        // Invest
        conn.execute(
            "UPDATE quest_skills SET points_invested = points_invested + 1 WHERE id = ?1",
            params![skill_id],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL", format!("{e}")))?;

        conn.execute(
            "UPDATE quest_character SET skill_points = skill_points - 1 WHERE id = 1",
            [],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL_SP", format!("{e}")))?;

        // Return updated skill
        conn.query_row(
            "SELECT id, name, description, tier, points_invested, max_points, prerequisite, effect, branch
             FROM quest_skills WHERE id = ?1",
            params![skill_id],
            |r| {
                Ok(Skill {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    tier: r.get(3)?,
                    points_invested: r.get(4)?,
                    max_points: r.get(5)?,
                    prerequisite: r.get(6)?,
                    effect: r.get(7)?,
                    branch: SkillBranch::from_str(&r.get::<_, String>(8)?),
                })
            },
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL_READ", format!("{e}")))
    }

    // -- Data loaders ---------------------------------------------------------

    fn load_inventory(&self, conn: &Connection) -> Result<Vec<Item>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description
                 FROM quest_inventory ORDER BY rarity DESC, name",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_INV", format!("{e}")))?;

        let items = stmt
            .query_map([], |r| {
                Ok(Item {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    rarity: ItemRarity::from_str(&r.get::<_, String>(3)?),
                    stats: ItemStats {
                        attack: r.get(4)?,
                        defense: r.get(5)?,
                        magic: r.get(6)?,
                        hp_bonus: r.get(7)?,
                    },
                    level_req: r.get(8)?,
                    description: r.get(9)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_INV_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    fn build_equipment(&self, conn: &Connection) -> Result<Equipment, ImpForgeError> {
        let mut eq = Equipment::default();

        let mut stmt = conn
            .prepare(
                "SELECT id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description, equipped_slot
                 FROM quest_inventory WHERE equipped_slot IS NOT NULL",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_EQ", format!("{e}")))?;

        let items: Vec<(Item, String)> = stmt
            .query_map([], |r| {
                Ok((
                    Item {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        item_type: r.get(2)?,
                        rarity: ItemRarity::from_str(&r.get::<_, String>(3)?),
                        stats: ItemStats {
                            attack: r.get(4)?,
                            defense: r.get(5)?,
                            magic: r.get(6)?,
                            hp_bonus: r.get(7)?,
                        },
                        level_req: r.get(8)?,
                        description: r.get(9)?,
                    },
                    r.get::<_, String>(10)?,
                ))
            })
            .map_err(|e| ImpForgeError::internal("QUEST_EQ_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (item, slot) in items {
            match slot.as_str() {
                "weapon" => eq.weapon = Some(item),
                "head" => eq.head = Some(item),
                "chest" => eq.chest = Some(item),
                "legs" => eq.legs = Some(item),
                "boots" => eq.boots = Some(item),
                "gloves" => eq.gloves = Some(item),
                "accessory1" => eq.accessory1 = Some(item),
                "accessory2" => eq.accessory2 = Some(item),
                _ => {}
            }
        }

        Ok(eq)
    }

    fn load_skills(&self, conn: &Connection) -> Result<Vec<Skill>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, tier, points_invested, max_points, prerequisite, effect, branch
                 FROM quest_skills ORDER BY branch, tier, name",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SKILLS", format!("{e}")))?;

        let skills = stmt
            .query_map([], |r| {
                Ok(Skill {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    tier: r.get(3)?,
                    points_invested: r.get(4)?,
                    max_points: r.get(5)?,
                    prerequisite: r.get(6)?,
                    effect: r.get(7)?,
                    branch: SkillBranch::from_str(&r.get::<_, String>(8)?),
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_SKILLS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    pub fn get_quests(&self) -> Result<Vec<Quest>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, objective, objective_target, objective_progress,
                        reward_xp, reward_gold, reward_items, completed
                 FROM quest_quests ORDER BY completed ASC, reward_xp DESC",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_QUESTS", format!("{e}")))?;

        let quests = stmt
            .query_map([], |r| {
                let items_json: String = r.get(8)?;
                let items: Vec<String> =
                    serde_json::from_str(&items_json).unwrap_or_default();
                Ok(Quest {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    objective: r.get(3)?,
                    objective_target: r.get(4)?,
                    objective_progress: r.get(5)?,
                    reward_xp: r.get(6)?,
                    reward_gold: r.get(7)?,
                    reward_items: items,
                    completed: r.get::<_, i32>(9)? != 0,
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_QUESTS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(quests)
    }

    pub fn get_leaderboard(&self) -> Result<Vec<LeaderboardEntry>, ImpForgeError> {
        // Single-player leaderboard: show the player's character as a single entry.
        // In a future version this could sync with a server for multiplayer rankings.
        let char = self.get_character()?;
        Ok(vec![LeaderboardEntry {
            name: char.name,
            class: char.class,
            level: char.level,
            xp: char.xp,
            monsters_slain: char.monsters_slain,
            quests_completed: char.quests_completed,
        }])
    }

    // -- Internal helpers -----------------------------------------------------

    fn grant_material(&self, conn: &Connection, mat_name: &str) -> Result<(), ImpForgeError> {
        let id = format!("mat_{}_{}", mat_name.to_lowercase().replace(' ', "_"), Utc::now().timestamp_millis());
        conn.execute(
            "INSERT INTO quest_inventory (id, name, item_type, rarity, description)
             VALUES (?1, ?2, 'material', 'common', ?3)",
            params![id, mat_name, format!("A {mat_name} gathered from your labors.")],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_MAT", format!("{e}")))?;
        Ok(())
    }

    fn generate_loot(
        &self,
        conn: &Connection,
        monster: &Monster,
        _char_level: u32,
    ) -> Result<Vec<Item>, ImpForgeError> {
        let now = Utc::now().timestamp_millis();
        let mut loot = Vec::new();

        for (item_name, drop_chance) in &monster.loot_table {
            // Deterministic pseudo-random: hash the timestamp with the item name
            let hash = now.wrapping_mul(item_name.len() as i64 + 31) % 100;
            if (hash as f32) < (*drop_chance * 100.0) {
                let rarity = if *drop_chance < 0.05 {
                    ItemRarity::Epic
                } else if *drop_chance < 0.15 {
                    ItemRarity::Rare
                } else if *drop_chance < 0.30 {
                    ItemRarity::Uncommon
                } else {
                    ItemRarity::Common
                };

                let item = Item {
                    id: format!("loot_{}_{}", item_name.to_lowercase().replace(' ', "_"), now),
                    name: item_name.clone(),
                    item_type: "material".to_string(),
                    rarity: rarity.clone(),
                    stats: ItemStats { attack: 0, defense: 0, magic: 0, hp_bonus: 0 },
                    level_req: 1,
                    description: format!("Dropped by {}", monster.name),
                };

                conn.execute(
                    "INSERT INTO quest_inventory (id, name, item_type, rarity, description)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![item.id, item.name, item.item_type, rarity.as_str(), item.description],
                )
                .map_err(|e| ImpForgeError::internal("QUEST_LOOT", format!("{e}")))?;

                loot.push(item);
            }
        }

        Ok(loot)
    }

    fn update_quest_progress(
        &self,
        conn: &Connection,
        action: &str,
    ) -> Result<Option<String>, ImpForgeError> {
        let objective_type = match action {
            "create_document" | "create_note" => "create_documents",
            "run_workflow" => "run_workflows",
            "ai_query" => "ai_queries",
            "create_spreadsheet" => "craft_items",
            _ => return Ok(None),
        };

        // Increment matching active quests
        conn.execute(
            "UPDATE quest_quests SET objective_progress = objective_progress + 1
             WHERE objective = ?1 AND completed = 0",
            params![objective_type],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_PROGRESS", format!("{e}")))?;

        // Check if any quest just completed
        let completed: Option<(String, u64, u64)> = conn
            .query_row(
                "SELECT id, reward_xp, reward_gold FROM quest_quests
                 WHERE completed = 0 AND objective_progress >= objective_target
                 LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();

        if let Some((quest_id, rxp, rgold)) = completed {
            conn.execute(
                "UPDATE quest_quests SET completed = 1 WHERE id = ?1",
                params![quest_id],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_COMPLETE", format!("{e}")))?;

            conn.execute(
                "UPDATE quest_character SET xp = xp + ?1, gold = gold + ?2, quests_completed = quests_completed + 1 WHERE id = 1",
                params![rxp, rgold],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_REWARD", format!("{e}")))?;

            return Ok(Some(quest_id));
        }

        Ok(None)
    }

    fn seed_quests(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let quests = vec![
            ("q_first_doc", "The Scribe's Trial", "Write your first document to earn your quill.", "create_documents", 1u32, 50u64, 25u64),
            ("q_docs_5", "Manuscript Master", "Create 5 documents to prove your scholarly might.", "create_documents", 5, 150, 75),
            ("q_first_workflow", "The Automaton's Apprentice", "Execute your first workflow.", "run_workflows", 1, 75, 40),
            ("q_workflows_3", "Clockwork Commander", "Run 3 workflows to master automation.", "run_workflows", 3, 200, 100),
            ("q_ai_10", "The Oracle's Student", "Make 10 AI queries to learn the arcane arts.", "ai_queries", 10, 100, 50),
            ("q_ai_50", "Spellweaver", "Cast 50 AI spells (queries) to ascend.", "ai_queries", 50, 300, 150),
            ("q_craft_3", "Apprentice Forgemaster", "Craft 3 items at the forge.", "craft_items", 3, 120, 60),
            ("q_slay_10", "Monster Hunter", "Slay 10 monsters in battle.", "slay_monsters", 10, 200, 100),
            ("q_slay_50", "Legendary Slayer", "Defeat 50 monsters across the realm.", "slay_monsters", 50, 500, 250),
            ("q_modules_5", "Jack of All Trades", "Use 5 different ImpForge modules.", "use_modules", 5, 150, 75),
        ];

        for (id, name, desc, obj, target, rxp, rgold) in quests {
            conn.execute(
                "INSERT OR IGNORE INTO quest_quests (id, name, description, objective, objective_target, reward_xp, reward_gold)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, name, desc, obj, target, rxp, rgold],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SEED", format!("{e}")))?;
        }

        Ok(())
    }

    fn seed_skills(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let skills = vec![
            // Combat branch
            ("sk_power_strike", "Power Strike", "Increases base attack damage.", 1u32, 5u32, None::<&str>, "+10% attack per point", "combat"),
            ("sk_crit_chance", "Critical Eye", "Chance to deal double damage.", 2, 5, Some("sk_power_strike"), "+4% crit chance per point", "combat"),
            ("sk_berserker", "Berserker Rage", "More damage when HP is low.", 3, 3, Some("sk_crit_chance"), "+20% damage below 30% HP per point", "combat"),
            // Defense branch
            ("sk_iron_skin", "Iron Skin", "Reduces incoming damage.", 1, 5, None, "+5% damage reduction per point", "defense"),
            ("sk_hp_boost", "Vitality", "Increases maximum HP.", 2, 5, Some("sk_iron_skin"), "+15 max HP per point", "defense"),
            ("sk_regen", "Regeneration", "Recover HP after each battle.", 3, 3, Some("sk_hp_boost"), "+5 HP regen per point", "defense"),
            // Magic branch
            ("sk_arcane_power", "Arcane Power", "Boosts magic damage.", 1, 5, None, "+10% magic per point", "magic"),
            ("sk_mana_flow", "Mana Flow", "AI queries grant bonus XP.", 2, 5, Some("sk_arcane_power"), "+5% AI XP bonus per point", "magic"),
            ("sk_spell_mastery", "Spell Mastery", "Chance for double rewards from AI actions.", 3, 3, Some("sk_mana_flow"), "+8% double reward chance per point", "magic"),
            // Crafting branch
            ("sk_efficient", "Efficient Crafting", "Chance to save materials.", 1, 5, None, "+5% material save chance per point", "crafting"),
            ("sk_quality", "Master Quality", "Crafted items have higher stats.", 2, 5, Some("sk_efficient"), "+10% crafted item stats per point", "crafting"),
            ("sk_rare_craft", "Rare Discovery", "Chance to craft at higher rarity.", 3, 3, Some("sk_quality"), "+5% rarity upgrade chance per point", "crafting"),
            // Leadership branch
            ("sk_gold_find", "Gold Finder", "Monsters drop more gold.", 1, 5, None, "+10% gold from battles per point", "leadership"),
            ("sk_team_spirit", "Team Spirit", "Team contributions grant bonus XP.", 2, 5, Some("sk_gold_find"), "+10% team XP bonus per point", "leadership"),
            ("sk_commander", "Commander", "All stat bonuses increased.", 3, 3, Some("sk_team_spirit"), "+3% all stats per point", "leadership"),
            // Wisdom branch
            ("sk_quick_study", "Quick Study", "All actions grant bonus XP.", 1, 5, None, "+5% XP from all actions per point", "wisdom"),
            ("sk_treasure_sense", "Treasure Sense", "Better loot drop rates.", 2, 5, Some("sk_quick_study"), "+5% loot chance per point", "wisdom"),
            ("sk_enlightenment", "Enlightenment", "Massive XP bonus for diverse module usage.", 3, 3, Some("sk_treasure_sense"), "+15% XP when switching modules per point", "wisdom"),
        ];

        for (id, name, desc, tier, max_pts, prereq, effect, branch) in skills {
            conn.execute(
                "INSERT OR IGNORE INTO quest_skills (id, name, description, tier, max_points, prerequisite, effect, branch)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, name, desc, tier, max_pts, prereq, effect, branch],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SKILL_SEED", format!("{e}")))?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Static data: Zones, Recipes, Action mapping
// ---------------------------------------------------------------------------

/// XP needed to reach a given level: 150 * level^1.6
fn xp_for_level(level: u32) -> u64 {
    (150.0 * (level as f64).powf(1.6)) as u64
}

fn map_action_to_rpg(action: &str) -> RpgReward {
    match action {
        "create_document" => RpgReward { xp: 25, gold: 10, material: Some("Parchment".to_string()), monster_fight: false },
        "run_workflow" => RpgReward { xp: 50, gold: 25, material: Some("Gear".to_string()), monster_fight: true },
        "ai_query" => RpgReward { xp: 15, gold: 5, material: Some("Crystal".to_string()), monster_fight: false },
        "send_email" => RpgReward { xp: 10, gold: 8, material: None, monster_fight: false },
        "create_spreadsheet" => RpgReward { xp: 30, gold: 15, material: Some("Iron Ore".to_string()), monster_fight: false },
        "social_post" => RpgReward { xp: 20, gold: 12, material: Some("Song Scroll".to_string()), monster_fight: false },
        "team_contribution" => RpgReward { xp: 35, gold: 20, material: Some("Banner".to_string()), monster_fight: false },
        "complete_quest" => RpgReward { xp: 100, gold: 50, material: Some("Quest Token".to_string()), monster_fight: true },
        "create_note" => RpgReward { xp: 20, gold: 8, material: Some("Parchment".to_string()), monster_fight: false },
        "create_slide" => RpgReward { xp: 20, gold: 10, material: Some("Canvas".to_string()), monster_fight: false },
        "import_file" => RpgReward { xp: 10, gold: 5, material: None, monster_fight: false },
        _ => RpgReward { xp: 5, gold: 2, material: None, monster_fight: false },
    }
}

fn all_zones() -> Vec<Zone> {
    vec![
        Zone {
            id: "beginners_meadow".into(), name: "Beginner's Meadow".into(),
            description: "A peaceful field where novice adventurers hone their skills.".into(),
            level_min: 1, level_max: 5,
            monsters: vec![
                monster("Slime", 1, 20, 3, 1, 8, 5, vec![("Slime Gel", 0.5)]),
                monster("Rat", 2, 25, 5, 2, 12, 6, vec![("Rat Tail", 0.4)]),
                monster("Goblin", 4, 40, 8, 3, 20, 10, vec![("Goblin Ear", 0.35), ("Rusty Dagger", 0.1)]),
            ],
            boss: Some(monster("Goblin Chief", 5, 80, 14, 6, 50, 30, vec![("Chief's Crown", 0.5), ("Goblin Blade", 0.25)])),
            unlock_condition: "Start here".into(),
        },
        Zone {
            id: "dark_forest".into(), name: "Dark Forest".into(),
            description: "Ancient trees block the sunlight. Beware the creatures within.".into(),
            level_min: 5, level_max: 10,
            monsters: vec![
                monster("Wolf", 5, 50, 12, 5, 25, 12, vec![("Wolf Pelt", 0.45)]),
                monster("Spider", 7, 45, 14, 4, 30, 15, vec![("Spider Silk", 0.5), ("Venom Sac", 0.15)]),
                monster("Bandit", 9, 65, 16, 8, 40, 20, vec![("Stolen Coin", 0.6), ("Bandit Mask", 0.1)]),
            ],
            boss: Some(monster("Forest Wraith", 10, 120, 22, 10, 80, 50, vec![("Wraith Essence", 0.5), ("Shadow Cloak", 0.2)])),
            unlock_condition: "Reach level 5".into(),
        },
        Zone {
            id: "crystal_cave".into(), name: "Crystal Cave".into(),
            description: "Glittering caves filled with magical crystals and their guardians.".into(),
            level_min: 10, level_max: 15,
            monsters: vec![
                monster("Cave Bat", 10, 55, 15, 6, 35, 18, vec![("Bat Wing", 0.5)]),
                monster("Stone Golem", 12, 100, 20, 18, 50, 25, vec![("Golem Core", 0.3), ("Stone Shard", 0.5)]),
                monster("Crystal Elemental", 14, 80, 22, 12, 55, 30, vec![("Pure Crystal", 0.35), ("Elemental Spark", 0.15)]),
            ],
            boss: Some(monster("Crystal Dragon", 15, 180, 30, 20, 120, 80, vec![("Dragon Scale", 0.5), ("Crystal Heart", 0.15)])),
            unlock_condition: "Reach level 10".into(),
        },
        Zone {
            id: "dragons_peak".into(), name: "Dragon's Peak".into(),
            description: "The mountain summit where drakes and wyverns nest.".into(),
            level_min: 15, level_max: 20,
            monsters: vec![
                monster("Drake", 15, 90, 25, 14, 60, 35, vec![("Drake Fang", 0.4)]),
                monster("Wyvern", 17, 110, 28, 16, 70, 40, vec![("Wyvern Wing", 0.35), ("Sky Gem", 0.1)]),
                monster("Fire Dragon", 19, 140, 32, 20, 85, 50, vec![("Dragon Flame", 0.3), ("Fire Ruby", 0.08)]),
            ],
            boss: Some(monster("Elder Dragon", 20, 250, 40, 28, 200, 120, vec![("Elder Scale", 0.5), ("Dragon Heart", 0.1)])),
            unlock_condition: "Reach level 15".into(),
        },
        Zone {
            id: "shadow_realm".into(), name: "Shadow Realm".into(),
            description: "A dimension of darkness where fallen warriors dwell.".into(),
            level_min: 20, level_max: 30,
            monsters: vec![
                monster("Shadow Knight", 22, 130, 35, 22, 90, 55, vec![("Shadow Steel", 0.35)]),
                monster("Lich", 25, 100, 40, 15, 110, 65, vec![("Phylactery Shard", 0.2), ("Death Rune", 0.3)]),
                monster("Demon", 28, 160, 45, 25, 130, 80, vec![("Demon Horn", 0.25), ("Infernal Gem", 0.08)]),
            ],
            boss: Some(monster("Shadow Lord", 30, 350, 55, 35, 300, 200, vec![("Shadow Crown", 0.4), ("Void Fragment", 0.1)])),
            unlock_condition: "Reach level 20".into(),
        },
        Zone {
            id: "forge_of_legends".into(), name: "Forge of Legends".into(),
            description: "An ancient workshop where legendary weapons were born.".into(),
            level_min: 30, level_max: 40,
            monsters: vec![
                monster("Forge Golem", 32, 200, 48, 35, 150, 90, vec![("Legendary Ingot", 0.25)]),
                monster("Flame Spirit", 35, 150, 55, 20, 170, 100, vec![("Eternal Flame", 0.2)]),
                monster("Iron Colossus", 38, 280, 52, 45, 190, 120, vec![("Colossus Plate", 0.15)]),
            ],
            boss: Some(monster("Ancient Forge Guardian", 40, 500, 65, 50, 400, 300, vec![("Guardian Hammer", 0.3), ("Forge Heart", 0.08)])),
            unlock_condition: "Reach level 30".into(),
        },
        Zone {
            id: "celestial_tower".into(), name: "Celestial Tower".into(),
            description: "A tower that pierces the heavens, guarded by celestial beings.".into(),
            level_min: 40, level_max: 50,
            monsters: vec![
                monster("Cloud Serpent", 42, 220, 58, 30, 200, 130, vec![("Cloud Pearl", 0.2)]),
                monster("Thunder Titan", 45, 300, 65, 40, 250, 160, vec![("Thunder Core", 0.15)]),
                monster("Star Warden", 48, 260, 70, 35, 280, 180, vec![("Star Fragment", 0.12)]),
            ],
            boss: Some(monster("Sky Emperor", 50, 700, 80, 55, 500, 400, vec![("Emperor's Crown", 0.25), ("Celestial Gem", 0.05)])),
            unlock_condition: "Reach level 40".into(),
        },
        Zone {
            id: "void_abyss".into(), name: "Void Abyss".into(),
            description: "The edge of existence. Reality itself frays here.".into(),
            level_min: 50, level_max: 75,
            monsters: vec![
                monster("Void Walker", 55, 350, 75, 45, 350, 220, vec![("Void Shard", 0.2)]),
                monster("Reality Breaker", 65, 450, 90, 55, 450, 300, vec![("Reality Tear", 0.1)]),
                monster("Entropy Beast", 72, 550, 100, 60, 550, 380, vec![("Entropy Core", 0.08)]),
            ],
            boss: Some(monster("Void Devourer", 75, 1200, 120, 75, 800, 600, vec![("Devourer Fang", 0.2), ("Void Orb", 0.03)])),
            unlock_condition: "Reach level 50".into(),
        },
        Zone {
            id: "eternal_citadel".into(), name: "Eternal Citadel".into(),
            description: "The fortress of the immortal king. Only the worthy may enter.".into(),
            level_min: 75, level_max: 99,
            monsters: vec![
                monster("Eternal Sentinel", 78, 600, 110, 70, 600, 400, vec![("Sentinel Core", 0.15)]),
                monster("Time Weaver", 85, 500, 130, 50, 700, 500, vec![("Time Crystal", 0.08)]),
                monster("Immortal Knight", 92, 800, 140, 90, 850, 600, vec![("Immortal Steel", 0.05)]),
            ],
            boss: Some(monster("Eternal King", 99, 2000, 180, 100, 1500, 1000, vec![("Eternal Crown", 0.15), ("King's Soul", 0.02)])),
            unlock_condition: "Reach level 75".into(),
        },
        Zone {
            id: "the_final_forge".into(), name: "The Final Forge".into(),
            description: "The heart of ImpForge itself. Here, productivity becomes legend.".into(),
            level_min: 99, level_max: 100,
            monsters: vec![
                monster("Compile Error", 99, 999, 150, 80, 1000, 500, vec![("Debug Token", 0.5)]),
                monster("Merge Conflict", 99, 888, 160, 70, 1000, 500, vec![("Resolution Gem", 0.4)]),
            ],
            boss: Some(monster("ImpForge Itself", 100, 5000, 200, 120, 5000, 3000, vec![("Mythic Forge Hammer", 0.1), ("ImpForge Core", 0.01)])),
            unlock_condition: "Reach level 99".into(),
        },
    ]
}

fn monster(
    name: &str, level: u32, hp: u32, attack: u32, defense: u32,
    xp: u64, gold: u64, loot: Vec<(&str, f32)>,
) -> Monster {
    Monster {
        name: name.to_string(),
        level, hp, attack, defense,
        xp_reward: xp, gold_reward: gold,
        loot_table: loot.into_iter().map(|(n, c)| (n.to_string(), c)).collect(),
    }
}

fn all_recipes() -> Vec<CraftingRecipe> {
    vec![
        CraftingRecipe {
            id: "recipe_iron_sword".into(), name: "Iron Sword".into(),
            result_item_id: "iron_sword".into(),
            materials: vec![("Iron Ore".into(), 3), ("Gear".into(), 1)],
            required_level: 3,
        },
        CraftingRecipe {
            id: "recipe_crystal_staff".into(), name: "Crystal Staff".into(),
            result_item_id: "crystal_staff".into(),
            materials: vec![("Crystal".into(), 5), ("Parchment".into(), 2)],
            required_level: 5,
        },
        CraftingRecipe {
            id: "recipe_leather_armor".into(), name: "Leather Armor".into(),
            result_item_id: "leather_armor".into(),
            materials: vec![("Wolf Pelt".into(), 3), ("Banner".into(), 1)],
            required_level: 6,
        },
        CraftingRecipe {
            id: "recipe_spider_bow".into(), name: "Spider Silk Bow".into(),
            result_item_id: "spider_bow".into(),
            materials: vec![("Spider Silk".into(), 4), ("Gear".into(), 2)],
            required_level: 8,
        },
        CraftingRecipe {
            id: "recipe_golem_shield".into(), name: "Golem Shield".into(),
            result_item_id: "golem_shield".into(),
            materials: vec![("Stone Shard".into(), 3), ("Golem Core".into(), 1)],
            required_level: 12,
        },
        CraftingRecipe {
            id: "recipe_dragon_blade".into(), name: "Dragon Blade".into(),
            result_item_id: "dragon_blade".into(),
            materials: vec![("Dragon Scale".into(), 2), ("Iron Ore".into(), 5), ("Gear".into(), 3)],
            required_level: 16,
        },
        CraftingRecipe {
            id: "recipe_shadow_cloak".into(), name: "Shadow Cloak".into(),
            result_item_id: "shadow_cloak".into(),
            materials: vec![("Shadow Steel".into(), 2), ("Wraith Essence".into(), 1)],
            required_level: 22,
        },
        CraftingRecipe {
            id: "recipe_healing_potion".into(), name: "Healing Potion".into(),
            result_item_id: "healing_potion".into(),
            materials: vec![("Slime Gel".into(), 2), ("Crystal".into(), 1)],
            required_level: 1,
        },
    ]
}

fn generate_item_from_recipe(recipe: &CraftingRecipe, crafter_level: u32) -> Item {
    let bonus = (crafter_level as i32 - recipe.required_level as i32).max(0);
    let (atk, def, mag, hp, itype) = match recipe.result_item_id.as_str() {
        "iron_sword" => (12 + bonus, 0, 0, 0, "weapon"),
        "crystal_staff" => (3, 0, 15 + bonus, 0, "weapon"),
        "leather_armor" => (0, 10 + bonus, 0, 20, "armor"),
        "spider_bow" => (14 + bonus, 0, 2, 0, "weapon"),
        "golem_shield" => (0, 18 + bonus, 0, 30, "armor"),
        "dragon_blade" => (25 + bonus, 0, 5, 0, "weapon"),
        "shadow_cloak" => (0, 12, 10 + bonus, 15, "accessory"),
        "healing_potion" => (0, 0, 0, 50, "potion"),
        _ => (5, 5, 5, 10, "weapon"),
    };

    let rarity = if bonus > 15 {
        ItemRarity::Epic
    } else if bonus > 8 {
        ItemRarity::Rare
    } else if bonus > 3 {
        ItemRarity::Uncommon
    } else {
        ItemRarity::Common
    };

    Item {
        id: format!("crafted_{}_{}", recipe.result_item_id, Utc::now().timestamp_millis()),
        name: recipe.name.clone(),
        item_type: itype.to_string(),
        rarity,
        stats: ItemStats { attack: atk, defense: def, magic: mag, hp_bonus: hp },
        level_req: recipe.required_level,
        description: format!("Crafted at the forge (level {crafter_level})."),
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn quest_create_character(
    name: String,
    class: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<QuestCharacter, ImpForgeError> {
    engine.create_character(&name, &class)
}

#[tauri::command]
pub async fn quest_get_character(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<QuestCharacter, ImpForgeError> {
    engine.get_character()
}

#[tauri::command]
pub async fn quest_track_action(
    action: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<ActionResult, ImpForgeError> {
    engine.track_action(&action)
}

#[tauri::command]
pub async fn quest_auto_battle(
    zone_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<BattleResult, ImpForgeError> {
    engine.auto_battle(&zone_id)
}

#[tauri::command]
pub async fn quest_craft_item(
    recipe_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Item, ImpForgeError> {
    engine.craft_item(&recipe_id)
}

#[tauri::command]
pub async fn quest_equip_item(
    item_id: String,
    slot: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<(), ImpForgeError> {
    engine.equip_item(&item_id, &slot)
}

#[tauri::command]
pub async fn quest_unequip(
    slot: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<(), ImpForgeError> {
    engine.unequip(&slot)
}

#[tauri::command]
pub async fn quest_invest_skill(
    skill_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Skill, ImpForgeError> {
    engine.invest_skill(&skill_id)
}

#[tauri::command]
pub async fn quest_get_zones() -> Result<Vec<Zone>, ImpForgeError> {
    Ok(all_zones())
}

#[tauri::command]
pub async fn quest_get_quests(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Quest>, ImpForgeError> {
    engine.get_quests()
}

#[tauri::command]
pub async fn quest_get_recipes() -> Result<Vec<CraftingRecipe>, ImpForgeError> {
    Ok(all_recipes())
}

#[tauri::command]
pub async fn quest_get_leaderboard(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<LeaderboardEntry>, ImpForgeError> {
    engine.get_leaderboard()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_engine() -> (ForgeQuestEngine, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let engine = ForgeQuestEngine::new(dir.path()).expect("engine");
        (engine, dir)
    }

    #[test]
    fn test_xp_for_level() {
        assert_eq!(xp_for_level(1), 150);
        assert!(xp_for_level(10) > xp_for_level(9));
        assert!(xp_for_level(100) > xp_for_level(50));
    }

    #[test]
    fn test_create_character() {
        let (engine, _dir) = test_engine();
        let char = engine.create_character("TestHero", "warrior").expect("create");
        assert_eq!(char.name, "TestHero");
        assert_eq!(char.class, CharacterClass::Warrior);
        assert_eq!(char.level, 1);
        assert_eq!(char.hp, 120);
        assert!(char.attack > char.magic); // Warriors have higher attack
    }

    #[test]
    fn test_duplicate_character_rejected() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "mage").expect("first");
        let result = engine.create_character("Hero2", "warrior");
        assert!(result.is_err());
    }

    #[test]
    fn test_character_classes_stats() {
        let (engine, _dir) = test_engine();
        let mage = engine.create_character("Mage", "mage").expect("create mage");
        assert!(mage.magic > mage.attack, "Mage should have higher magic than attack");
        assert_eq!(mage.hp, 80, "Mage starts with 80 HP");
    }

    #[test]
    fn test_track_action_grants_xp() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "warrior").expect("create");

        let result = engine.track_action("create_document").expect("track");
        // Warrior gets 1.5x bonus for documents: 25 * 1.5 = 37
        assert_eq!(result.xp_earned, 37);
        assert_eq!(result.gold_earned, 15); // 10 * 1.5
        assert_eq!(result.material_gained, Some("Parchment".to_string()));
    }

    #[test]
    fn test_class_bonus_multiplier() {
        assert_eq!(CharacterClass::Warrior.bonus_multiplier("create_document"), 1.5);
        assert_eq!(CharacterClass::Mage.bonus_multiplier("ai_query"), 1.5);
        assert_eq!(CharacterClass::Warrior.bonus_multiplier("ai_query"), 1.0);
    }

    #[test]
    fn test_map_action_to_rpg() {
        let r = map_action_to_rpg("run_workflow");
        assert_eq!(r.xp, 50);
        assert!(r.monster_fight);
        assert_eq!(r.material, Some("Gear".to_string()));

        let r2 = map_action_to_rpg("unknown_action");
        assert_eq!(r2.xp, 5);
        assert!(!r2.monster_fight);
    }

    #[test]
    fn test_zones_are_valid() {
        let zones = all_zones();
        assert_eq!(zones.len(), 10);
        assert_eq!(zones[0].id, "beginners_meadow");
        assert_eq!(zones[9].id, "the_final_forge");
        // Each zone should have at least one monster
        for zone in &zones {
            assert!(!zone.monsters.is_empty(), "Zone {} has no monsters", zone.name);
        }
    }

    #[test]
    fn test_recipes_are_valid() {
        let recipes = all_recipes();
        assert!(recipes.len() >= 8);
        // Each recipe needs at least one material
        for r in &recipes {
            assert!(!r.materials.is_empty(), "Recipe {} has no materials", r.name);
        }
    }

    #[test]
    fn test_quests_seeded() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "ranger").expect("create");
        let quests = engine.get_quests().expect("quests");
        assert!(quests.len() >= 10, "Should have at least 10 starter quests");
        assert!(quests.iter().all(|q| !q.completed));
    }

    #[test]
    fn test_skills_seeded() {
        let (engine, _dir) = test_engine();
        let char = engine.create_character("Hero", "scholar").expect("create");
        assert!(char.skills.len() >= 18, "Should have at least 18 skills");
        // All skills start at 0 points
        assert!(char.skills.iter().all(|s| s.points_invested == 0));
    }

    #[test]
    fn test_auto_battle() {
        let (engine, _dir) = test_engine();
        engine.create_character("Fighter", "warrior").expect("create");
        let result = engine.auto_battle("beginners_meadow").expect("battle");
        assert!(result.rounds > 0);
        assert!(result.xp_earned > 0);
    }

    #[test]
    fn test_leaderboard() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "bard").expect("create");
        let board = engine.get_leaderboard().expect("leaderboard");
        assert_eq!(board.len(), 1);
        assert_eq!(board[0].name, "Hero");
    }

    #[test]
    fn test_item_rarity_roundtrip() {
        for r in &[ItemRarity::Common, ItemRarity::Uncommon, ItemRarity::Rare, ItemRarity::Epic, ItemRarity::Legendary, ItemRarity::Mythic] {
            assert_eq!(&ItemRarity::from_str(r.as_str()), r);
        }
    }

    #[test]
    fn test_character_class_roundtrip() {
        for c in &[CharacterClass::Warrior, CharacterClass::Mage, CharacterClass::Ranger, CharacterClass::Blacksmith, CharacterClass::Bard, CharacterClass::Scholar] {
            assert_eq!(&CharacterClass::from_str(c.as_str()), c);
        }
    }

    #[test]
    fn test_skill_branch_roundtrip() {
        for b in &[SkillBranch::Combat, SkillBranch::Defense, SkillBranch::Magic, SkillBranch::Crafting, SkillBranch::Leadership, SkillBranch::Wisdom] {
            assert_eq!(&SkillBranch::from_str(b.as_str()), b);
        }
    }
}
