// SPDX-License-Identifier: Apache-2.0
//! SwarmForge -- Idle RPG + OGame-style Colony Builder
//!
//! A medieval/sci-fi hybrid idle game where the user's REAL productivity powers
//! their character AND their planet colony. Writing documents = crafting weapons
//! AND producing biomass. Running workflows = fighting monsters AND building
//! fleet ships. AI queries = casting spells AND researching technology.
//!
//! ## Two Game Layers
//! 1. **RPG Layer** (ForgeQuest legacy): Character, equipment, skills, zones, quests
//! 2. **Colony Layer** (OGame-inspired): Planet resources, buildings, research, fleet,
//!    creep mechanic, dark matter shop -- all with exponential cost curves
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
//!
//! ## OGame-style Production Formula
//! `production_rate(level) = base_rate * level * 1.1^level`
//! Level 1: 30/h, Level 5: ~80/h, Level 10: ~260/h, Level 20: ~1,750/h
//!
//! ## Upgrade Cost Formula
//! `cost(level) = base_cost * factor^level`
//! Exponential growth keeps progression meaningful at every stage.

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
// Forge Swarm — Colony-building meta-game
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UnitType {
    // Tier 1 (from Larva)
    ForgeDrone,    // Resource gatherer -- earns Essence from user actions
    ImpScout,      // Fast task runner -- quick AI queries, small tasks

    // Tier 2 (evolved from Tier 1)
    Viper,         // Multi-purpose -- complex tasks, analysis
    ShadowWeaver,  // Security/stealth -- self-healing, credential guard
    Skyweaver,     // Browser/web -- web scraping, research
    Overseer,      // Monitoring -- health checks, performance watch

    // Tier 3 (evolved from Tier 2)
    Titan,         // Heavy-duty -- MoA ensemble, complex reasoning
    SwarmMother,   // Spawner -- creates new Larva automatically
    Ravager,       // Elite fighter -- boss battles, hard quests

    // Tier 4 (unique, max 1)
    Matriarch,     // Queen -- controls entire swarm, +20% all stats
}

impl UnitType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "forge_drone",
            Self::ImpScout => "imp_scout",
            Self::Viper => "viper",
            Self::ShadowWeaver => "shadow_weaver",
            Self::Skyweaver => "skyweaver",
            Self::Overseer => "overseer",
            Self::Titan => "titan",
            Self::SwarmMother => "swarm_mother",
            Self::Ravager => "ravager",
            Self::Matriarch => "matriarch",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "forge_drone" => Self::ForgeDrone,
            "imp_scout" => Self::ImpScout,
            "viper" => Self::Viper,
            "shadow_weaver" => Self::ShadowWeaver,
            "skyweaver" => Self::Skyweaver,
            "overseer" => Self::Overseer,
            "titan" => Self::Titan,
            "swarm_mother" => Self::SwarmMother,
            "ravager" => Self::Ravager,
            "matriarch" => Self::Matriarch,
            _ => Self::ForgeDrone,
        }
    }

    fn tier(&self) -> u32 {
        match self {
            Self::ForgeDrone | Self::ImpScout => 1,
            Self::Viper | Self::ShadowWeaver | Self::Skyweaver | Self::Overseer => 2,
            Self::Titan | Self::SwarmMother | Self::Ravager => 3,
            Self::Matriarch => 4,
        }
    }

    fn emoji(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "drone",
            Self::ImpScout => "scout",
            Self::Viper => "viper",
            Self::ShadowWeaver => "shadow",
            Self::Skyweaver => "sky",
            Self::Overseer => "eye",
            Self::Titan => "titan",
            Self::SwarmMother => "mother",
            Self::Ravager => "ravager",
            Self::Matriarch => "queen",
        }
    }

    fn base_stats(&self) -> (u32, u32, u32) {
        // (hp, attack, defense)
        match self {
            Self::ForgeDrone => (30, 5, 3),
            Self::ImpScout => (25, 8, 2),
            Self::Viper => (60, 18, 10),
            Self::ShadowWeaver => (50, 12, 18),
            Self::Skyweaver => (45, 15, 8),
            Self::Overseer => (55, 10, 15),
            Self::Titan => (120, 35, 25),
            Self::SwarmMother => (80, 15, 20),
            Self::Ravager => (100, 40, 15),
            Self::Matriarch => (200, 50, 40),
        }
    }

    fn special_ability(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "Gather: +10% Essence from productivity actions",
            Self::ImpScout => "Swift: Completes missions 20% faster",
            Self::Viper => "Analyze: +15% XP from complex tasks",
            Self::ShadowWeaver => "Cloak: 25% chance to avoid mission failure",
            Self::Skyweaver => "Soar: Can run web missions solo",
            Self::Overseer => "Watch: Reveals hidden mission bonuses",
            Self::Titan => "Crush: +30% damage in boss missions",
            Self::SwarmMother => "Spawn: Produces 1 free Larva every 60 min",
            Self::Ravager => "Frenzy: Double attack below 30% HP",
            Self::Matriarch => "Reign: +20% all stats for entire swarm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmUnit {
    pub id: String,
    pub unit_type: UnitType,
    pub name: String,
    pub level: u32,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub special_ability: String,
    pub assigned_task: Option<String>,
    pub efficiency: f32, // 0.0-2.0, improves with use
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionPath {
    pub from: String,
    pub to: String,
    pub essence_cost: u64,
    pub level_requirement: u32,
    pub materials: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BuildingType {
    Nest,              // Increases max unit cap (5 per level, starts at 10)
    EvolutionChamber,  // Unlocks higher tier evolutions
    EssencePool,       // Stores more Essence (1000 per level)
    NeuralWeb,         // ForgeMemory boost (+10% search quality per level)
    Armory,            // +5% unit attack per level
    Sanctuary,         // +5% unit defense per level
    Arcanum,           // +10% AI quality per level
    WarCouncil,        // Unlocks swarm analytics + auto-assign
}

impl BuildingType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Nest => "nest",
            Self::EvolutionChamber => "evolution_chamber",
            Self::EssencePool => "essence_pool",
            Self::NeuralWeb => "neural_web",
            Self::Armory => "armory",
            Self::Sanctuary => "sanctuary",
            Self::Arcanum => "arcanum",
            Self::WarCouncil => "war_council",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "nest" => Self::Nest,
            "evolution_chamber" => Self::EvolutionChamber,
            "essence_pool" => Self::EssencePool,
            "neural_web" => Self::NeuralWeb,
            "armory" => Self::Armory,
            "sanctuary" => Self::Sanctuary,
            "arcanum" => Self::Arcanum,
            "war_council" => Self::WarCouncil,
            _ => Self::Nest,
        }
    }

    fn max_level(&self) -> u32 {
        match self {
            Self::Nest => 20,
            Self::EvolutionChamber => 4,
            Self::EssencePool => 10,
            Self::NeuralWeb => 10,
            Self::Armory => 10,
            Self::Sanctuary => 10,
            Self::Arcanum => 10,
            Self::WarCouncil => 5,
        }
    }

    fn base_upgrade_cost(&self) -> u64 {
        match self {
            Self::Nest => 100,
            Self::EvolutionChamber => 300,
            Self::EssencePool => 150,
            Self::NeuralWeb => 200,
            Self::Armory => 200,
            Self::Sanctuary => 200,
            Self::Arcanum => 250,
            Self::WarCouncil => 400,
        }
    }

    fn bonus_description(&self, level: u32) -> String {
        match self {
            Self::Nest => format!("Max units: {}", 10 + level * 5),
            Self::EvolutionChamber => format!("Unlocks Tier {} evolutions", level + 1),
            Self::EssencePool => format!("Essence cap: {}", 1000 + level * 1000),
            Self::NeuralWeb => format!("+{}% ForgeMemory search quality", level * 10),
            Self::Armory => format!("+{}% unit attack", level * 5),
            Self::Sanctuary => format!("+{}% unit defense", level * 5),
            Self::Arcanum => format!("+{}% AI quality", level * 10),
            Self::WarCouncil => format!("Analytics tier {}/5", level),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    pub id: String,
    pub building_type: BuildingType,
    pub level: u32,
    pub max_level: u32,
    pub bonus: String,
    pub upgrade_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwarmResources {
    pub essence: u64,
    pub minerals: u64,
    pub vespene: u64,    // "Arcane Gas"
    pub biomass: u64,
    pub dark_matter: u64,
}

// ---------------------------------------------------------------------------
// OGame-style Colony System — Resources, Buildings, Research, Fleet, Creep
// ---------------------------------------------------------------------------

/// Planet resources with OGame-style production rates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetResources {
    pub biomass: f64,
    pub minerals: f64,
    pub crystal: f64,
    pub spore_gas: f64,
    pub energy: i64,
    pub dark_matter: u64,
    // Production rates per hour
    pub biomass_per_hour: f64,
    pub minerals_per_hour: f64,
    pub crystal_per_hour: f64,
    pub spore_gas_per_hour: f64,
    pub energy_production: i64,
    pub energy_consumption: i64,
}

impl Default for PlanetResources {
    fn default() -> Self {
        Self {
            biomass: 500.0,
            minerals: 500.0,
            crystal: 0.0,
            spore_gas: 0.0,
            energy: 0,
            dark_matter: 0,
            biomass_per_hour: 20.0,
            minerals_per_hour: 10.0,
            crystal_per_hour: 0.0,
            spore_gas_per_hour: 0.0,
            energy_production: 0,
            energy_consumption: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanetBuildingType {
    // Resource production
    BiomassConverter,
    MineralDrill,
    CrystalSynthesizer,
    SporeExtractor,
    EnergyNest,
    // Infrastructure
    CreepGenerator,
    BroodNest,
    EvolutionLab,
    // Military
    Blighthaven,
    SporeDefense,
    // Storage
    BiomassStorage,
    MineralSilo,
}

impl PlanetBuildingType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "biomass_converter",
            Self::MineralDrill => "mineral_drill",
            Self::CrystalSynthesizer => "crystal_synthesizer",
            Self::SporeExtractor => "spore_extractor",
            Self::EnergyNest => "energy_nest",
            Self::CreepGenerator => "creep_generator",
            Self::BroodNest => "brood_nest",
            Self::EvolutionLab => "evolution_lab",
            Self::Blighthaven => "blighthaven",
            Self::SporeDefense => "spore_defense",
            Self::BiomassStorage => "biomass_storage",
            Self::MineralSilo => "mineral_silo",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "biomass_converter" => Self::BiomassConverter,
            "mineral_drill" => Self::MineralDrill,
            "crystal_synthesizer" => Self::CrystalSynthesizer,
            "spore_extractor" => Self::SporeExtractor,
            "energy_nest" => Self::EnergyNest,
            "creep_generator" => Self::CreepGenerator,
            "brood_nest" => Self::BroodNest,
            "evolution_lab" => Self::EvolutionLab,
            "blighthaven" => Self::Blighthaven,
            "spore_defense" => Self::SporeDefense,
            "biomass_storage" => Self::BiomassStorage,
            "mineral_silo" => Self::MineralSilo,
            _ => Self::BiomassConverter,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "Biomass Converter",
            Self::MineralDrill => "Mineral Drill",
            Self::CrystalSynthesizer => "Crystal Synthesizer",
            Self::SporeExtractor => "Spore Extractor",
            Self::EnergyNest => "Energy Nest",
            Self::CreepGenerator => "Creep Generator",
            Self::BroodNest => "Brood Nest",
            Self::EvolutionLab => "Evolution Lab",
            Self::Blighthaven => "Blighthaven",
            Self::SporeDefense => "Spore Defense",
            Self::BiomassStorage => "Biomass Storage",
            Self::MineralSilo => "Mineral Silo",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "Digests flora and fauna into biomass for the swarm.",
            Self::MineralDrill => "Extracts minerals from deep underground deposits.",
            Self::CrystalSynthesizer => "Grows rare crystals through bio-alchemical synthesis.",
            Self::SporeExtractor => "Harvests spore gas from underground vents.",
            Self::EnergyNest => "Bio-luminescent energy generation for the colony.",
            Self::CreepGenerator => "Spreads living creep across the planet surface.",
            Self::BroodNest => "Increases maximum unit capacity for the swarm.",
            Self::EvolutionLab => "Required for researching new technologies.",
            Self::Blighthaven => "Orbital shipyard for constructing the bio-fleet.",
            Self::SporeDefense => "Planetary defense turrets against enemy raids.",
            Self::BiomassStorage => "Increases maximum biomass storage capacity.",
            Self::MineralSilo => "Increases maximum mineral storage capacity.",
        }
    }

    /// Base cost (biomass, minerals, crystal, spore_gas) and cost factor
    fn base_costs(&self) -> (f64, f64, f64, f64, f64) {
        // (biomass, minerals, crystal, spore_gas, factor)
        match self {
            Self::BiomassConverter =>    (60.0, 15.0, 0.0, 0.0, 1.5),
            Self::MineralDrill =>        (48.0, 24.0, 0.0, 0.0, 1.6),
            Self::CrystalSynthesizer =>  (225.0, 75.0, 0.0, 0.0, 1.5),
            Self::SporeExtractor =>      (225.0, 75.0, 0.0, 0.0, 1.5),
            Self::EnergyNest =>          (75.0, 30.0, 0.0, 0.0, 1.5),
            Self::CreepGenerator =>      (200.0, 100.0, 50.0, 0.0, 1.8),
            Self::BroodNest =>           (400.0, 120.0, 200.0, 0.0, 2.0),
            Self::EvolutionLab =>        (200.0, 400.0, 200.0, 0.0, 2.0),
            Self::Blighthaven =>         (400.0, 200.0, 100.0, 0.0, 2.0),
            Self::SporeDefense =>        (300.0, 200.0, 100.0, 50.0, 1.8),
            Self::BiomassStorage =>      (100.0, 0.0, 0.0, 0.0, 2.0),
            Self::MineralSilo =>         (100.0, 50.0, 0.0, 0.0, 2.0),
        }
    }

    /// Energy delta per level: positive = production, negative = consumption
    fn energy_per_level(&self) -> i64 {
        match self {
            Self::EnergyNest => 22,   // Produces energy
            Self::BiomassConverter => -10,
            Self::MineralDrill => -10,
            Self::CrystalSynthesizer => -20,
            Self::SporeExtractor => -30,
            Self::CreepGenerator => -15,
            Self::Blighthaven => -25,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetBuilding {
    pub building_type: PlanetBuildingType,
    pub level: u32,
    pub upgrading: bool,
    pub upgrade_finish: Option<String>,
    pub display_name: String,
    pub description: String,
    pub cost_biomass: f64,
    pub cost_minerals: f64,
    pub cost_crystal: f64,
    pub cost_spore_gas: f64,
    pub build_time_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TechType {
    Genetics,
    ArmorPlating,
    WeaponSystems,
    PropulsionDrive,
    SwarmIntelligence,
    Regeneration,
    MutationTech,
    CreepBiology,
    SpaceFaring,
    DarkMatterResearch,
}

impl TechType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Genetics => "genetics",
            Self::ArmorPlating => "armor_plating",
            Self::WeaponSystems => "weapon_systems",
            Self::PropulsionDrive => "propulsion_drive",
            Self::SwarmIntelligence => "swarm_intelligence",
            Self::Regeneration => "regeneration",
            Self::MutationTech => "mutation_tech",
            Self::CreepBiology => "creep_biology",
            Self::SpaceFaring => "space_faring",
            Self::DarkMatterResearch => "dark_matter_research",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "genetics" => Self::Genetics,
            "armor_plating" => Self::ArmorPlating,
            "weapon_systems" => Self::WeaponSystems,
            "propulsion_drive" => Self::PropulsionDrive,
            "swarm_intelligence" => Self::SwarmIntelligence,
            "regeneration" => Self::Regeneration,
            "mutation_tech" => Self::MutationTech,
            "creep_biology" => Self::CreepBiology,
            "space_faring" => Self::SpaceFaring,
            "dark_matter_research" => Self::DarkMatterResearch,
            _ => Self::Genetics,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::Genetics => "Genetics",
            Self::ArmorPlating => "Armor Plating",
            Self::WeaponSystems => "Weapon Systems",
            Self::PropulsionDrive => "Propulsion Drive",
            Self::SwarmIntelligence => "Swarm Intelligence",
            Self::Regeneration => "Regeneration",
            Self::MutationTech => "Mutation Tech",
            Self::CreepBiology => "Creep Biology",
            Self::SpaceFaring => "Space Faring",
            Self::DarkMatterResearch => "Dark Matter Research",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Genetics => "Unlocks higher tier biological units.",
            Self::ArmorPlating => "+10% unit defense per level.",
            Self::WeaponSystems => "+10% unit attack per level.",
            Self::PropulsionDrive => "Faster fleet travel speed.",
            Self::SwarmIntelligence => "+5% all production per level.",
            Self::Regeneration => "Units heal over time after battle.",
            Self::MutationTech => "Unlocks biological mutations for units.",
            Self::CreepBiology => "Faster creep spread across the planet.",
            Self::SpaceFaring => "Required for Blighthaven construction.",
            Self::DarkMatterResearch => "Unlocks dark matter shop items.",
        }
    }

    fn base_costs(&self) -> (f64, f64, f64, f64, f64) {
        // (biomass, minerals, crystal, spore_gas, factor)
        match self {
            Self::Genetics =>          (200.0, 100.0, 100.0, 0.0, 2.0),
            Self::ArmorPlating =>      (100.0, 200.0, 0.0, 0.0, 2.0),
            Self::WeaponSystems =>     (200.0, 100.0, 0.0, 0.0, 2.0),
            Self::PropulsionDrive =>   (100.0, 200.0, 100.0, 0.0, 2.0),
            Self::SwarmIntelligence => (300.0, 200.0, 100.0, 0.0, 2.0),
            Self::Regeneration =>      (200.0, 100.0, 200.0, 0.0, 2.0),
            Self::MutationTech =>      (400.0, 200.0, 200.0, 100.0, 2.0),
            Self::CreepBiology =>      (150.0, 50.0, 50.0, 0.0, 2.0),
            Self::SpaceFaring =>       (500.0, 400.0, 300.0, 200.0, 2.0),
            Self::DarkMatterResearch =>(800.0, 400.0, 400.0, 200.0, 2.0),
        }
    }

    /// Required Evolution Lab level to start this research
    fn required_lab_level(&self) -> u32 {
        match self {
            Self::Genetics => 1,
            Self::ArmorPlating => 1,
            Self::WeaponSystems => 1,
            Self::PropulsionDrive => 2,
            Self::SwarmIntelligence => 3,
            Self::Regeneration => 3,
            Self::MutationTech => 5,
            Self::CreepBiology => 2,
            Self::SpaceFaring => 4,
            Self::DarkMatterResearch => 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Research {
    pub tech_type: TechType,
    pub level: u32,
    pub researching: bool,
    pub research_finish: Option<String>,
    pub display_name: String,
    pub description: String,
    pub cost_biomass: f64,
    pub cost_minerals: f64,
    pub cost_crystal: f64,
    pub cost_spore_gas: f64,
    pub research_time_seconds: u64,
    pub required_lab_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShipType {
    BioFighter,
    SporeInterceptor,
    KrakenFrigate,
    Leviathan,
    BioTransporter,
    ColonyPod,
    Devourer,
    WorldEater,
}

#[allow(dead_code)]
impl ShipType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::BioFighter => "bio_fighter",
            Self::SporeInterceptor => "spore_interceptor",
            Self::KrakenFrigate => "kraken_frigate",
            Self::Leviathan => "leviathan",
            Self::BioTransporter => "bio_transporter",
            Self::ColonyPod => "colony_pod",
            Self::Devourer => "devourer",
            Self::WorldEater => "world_eater",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "bio_fighter" => Self::BioFighter,
            "spore_interceptor" => Self::SporeInterceptor,
            "kraken_frigate" => Self::KrakenFrigate,
            "leviathan" => Self::Leviathan,
            "bio_transporter" => Self::BioTransporter,
            "colony_pod" => Self::ColonyPod,
            "devourer" => Self::Devourer,
            "world_eater" => Self::WorldEater,
            _ => Self::BioFighter,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::BioFighter => "Bio Fighter",
            Self::SporeInterceptor => "Spore Interceptor",
            Self::KrakenFrigate => "Kraken Frigate",
            Self::Leviathan => "Leviathan",
            Self::BioTransporter => "Bio Transporter",
            Self::ColonyPod => "Colony Pod",
            Self::Devourer => "Devourer",
            Self::WorldEater => "World Eater",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::BioFighter => "Light fighter -- cheap, fast, disposable.",
            Self::SporeInterceptor => "Heavy fighter with spore weapons.",
            Self::KrakenFrigate => "Multi-role cruiser with tentacle arrays.",
            Self::Leviathan => "Massive battleship of the bio-fleet.",
            Self::BioTransporter => "Cargo ship for resource transport.",
            Self::ColonyPod => "Colonizes new planets for expansion.",
            Self::Devourer => "Destroyer-class vessel that eats enemy ships.",
            Self::WorldEater => "Planet-killer. The ultimate weapon. Requires Lv.20 research.",
        }
    }

    /// (biomass, minerals, crystal, spore_gas) per unit
    fn unit_cost(&self) -> (f64, f64, f64, f64) {
        match self {
            Self::BioFighter =>       (3000.0, 1000.0, 0.0, 0.0),
            Self::SporeInterceptor => (6000.0, 4000.0, 0.0, 0.0),
            Self::KrakenFrigate =>    (20000.0, 7000.0, 2000.0, 0.0),
            Self::Leviathan =>        (45000.0, 15000.0, 0.0, 0.0),
            Self::BioTransporter =>   (2000.0, 2000.0, 0.0, 0.0),
            Self::ColonyPod =>        (10000.0, 10000.0, 10000.0, 10000.0),
            Self::Devourer =>         (60000.0, 50000.0, 15000.0, 0.0),
            Self::WorldEater =>       (5000000.0, 4000000.0, 1000000.0, 1000000.0),
        }
    }

    /// (attack, shields, hp)
    fn combat_stats(&self) -> (u32, u32, u32) {
        match self {
            Self::BioFighter =>       (50, 10, 400),
            Self::SporeInterceptor => (150, 25, 1000),
            Self::KrakenFrigate =>    (400, 50, 2700),
            Self::Leviathan =>        (1000, 200, 6000),
            Self::BioTransporter =>   (5, 10, 1200),
            Self::ColonyPod =>        (50, 100, 3000),
            Self::Devourer =>         (2000, 500, 11000),
            Self::WorldEater =>       (200000, 50000, 900000),
        }
    }

    /// Required Blighthaven level to build
    fn required_shipyard_level(&self) -> u32 {
        match self {
            Self::BioFighter => 1,
            Self::SporeInterceptor => 3,
            Self::KrakenFrigate => 5,
            Self::Leviathan => 7,
            Self::BioTransporter => 2,
            Self::ColonyPod => 4,
            Self::Devourer => 9,
            Self::WorldEater => 12,
        }
    }

    /// Build time per unit in seconds
    fn build_time_per_unit(&self) -> u64 {
        match self {
            Self::BioFighter => 30,
            Self::SporeInterceptor => 60,
            Self::KrakenFrigate => 180,
            Self::Leviathan => 600,
            Self::BioTransporter => 45,
            Self::ColonyPod => 300,
            Self::Devourer => 1200,
            Self::WorldEater => 7200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub ship_type: ShipType,
    pub count: u32,
    pub display_name: String,
    pub description: String,
    pub attack: u32,
    pub shields: u32,
    pub hp: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreepStatus {
    pub coverage_percent: f32,
    pub spread_rate_per_hour: f32,
    pub flora_corrupted: f32,
    pub fauna_consumed: f32,
    pub biomass_bonus: f32,
}

impl Default for CreepStatus {
    fn default() -> Self {
        Self {
            coverage_percent: 0.0,
            spread_rate_per_hour: 0.0,
            flora_corrupted: 0.0,
            fauna_consumed: 0.0,
            biomass_bonus: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShopEffect {
    ProductionBoost(f32),
    ResearchSpeed(f32),
    BuildSpeed(f32),
    FleetSpeed(f32),
    ExtraQueue,
    CreepBoost(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost_dark_matter: u64,
    pub effect: ShopEffect,
    pub duration_hours: Option<u32>,
}

/// Combined planet state for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Planet {
    pub name: String,
    pub resources: PlanetResources,
    pub buildings: Vec<PlanetBuilding>,
    pub research: Vec<Research>,
    pub fleet: Vec<Ship>,
    pub creep: CreepStatus,
    pub storage_biomass_cap: f64,
    pub storage_minerals_cap: f64,
    pub storage_crystal_cap: f64,
    pub storage_spore_gas_cap: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetSlot {
    pub position: u32,
    pub occupied: bool,
    pub planet_name: Option<String>,
    pub player_name: Option<String>,
    pub planet_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTimer {
    pub timer_type: String,
    pub item_name: String,
    pub completed_at: String,
}

// ---------------------------------------------------------------------------
// End of OGame types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Available,
    InProgress,
    Completed,
    Failed,
}

impl MissionStatus {
    #[cfg(test)]
    fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "available" => Self::Available,
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Available,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required_unit_types: Vec<String>,
    pub required_unit_count: u32,
    pub assigned_units: Vec<String>,
    pub duration_minutes: u32,
    pub reward: SwarmResources,
    pub reward_items: Vec<String>,
    pub status: MissionStatus,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionReward {
    pub resources: SwarmResources,
    pub items: Vec<String>,
    pub xp_earned: u64,
    pub mission_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    pub units: Vec<SwarmUnit>,
    pub buildings: Vec<Building>,
    pub resources: SwarmResources,
    pub max_units: u32,
    pub max_essence: u64,
    pub evolution_paths: Vec<EvolutionPath>,
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
            );

            -- Forge Swarm tables
            CREATE TABLE IF NOT EXISTS swarm_units (
                id TEXT PRIMARY KEY,
                unit_type TEXT NOT NULL,
                name TEXT NOT NULL,
                level INTEGER NOT NULL DEFAULT 1,
                hp INTEGER NOT NULL DEFAULT 30,
                attack INTEGER NOT NULL DEFAULT 5,
                defense INTEGER NOT NULL DEFAULT 3,
                special_ability TEXT NOT NULL DEFAULT '',
                assigned_task TEXT,
                efficiency REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS swarm_buildings (
                id TEXT PRIMARY KEY,
                building_type TEXT NOT NULL UNIQUE,
                level INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS swarm_resources (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                essence INTEGER NOT NULL DEFAULT 100,
                minerals INTEGER NOT NULL DEFAULT 0,
                vespene INTEGER NOT NULL DEFAULT 0,
                biomass INTEGER NOT NULL DEFAULT 0,
                dark_matter INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS swarm_missions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                required_unit_types TEXT NOT NULL DEFAULT '[]',
                required_unit_count INTEGER NOT NULL DEFAULT 1,
                assigned_units TEXT NOT NULL DEFAULT '[]',
                duration_minutes INTEGER NOT NULL DEFAULT 5,
                reward_essence INTEGER NOT NULL DEFAULT 0,
                reward_minerals INTEGER NOT NULL DEFAULT 0,
                reward_vespene INTEGER NOT NULL DEFAULT 0,
                reward_biomass INTEGER NOT NULL DEFAULT 0,
                reward_dark_matter INTEGER NOT NULL DEFAULT 0,
                reward_items TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'available',
                started_at TEXT
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_DB_INIT", format!("Table creation failed: {e}"))
        })?;

        // OGame-style colony tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS planet_resources (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                biomass REAL NOT NULL DEFAULT 500.0,
                minerals REAL NOT NULL DEFAULT 500.0,
                crystal REAL NOT NULL DEFAULT 0.0,
                spore_gas REAL NOT NULL DEFAULT 0.0,
                dark_matter INTEGER NOT NULL DEFAULT 0,
                last_collected TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS planet_buildings (
                building_type TEXT PRIMARY KEY,
                level INTEGER NOT NULL DEFAULT 0,
                upgrading INTEGER NOT NULL DEFAULT 0,
                upgrade_finish TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_research (
                tech_type TEXT PRIMARY KEY,
                level INTEGER NOT NULL DEFAULT 0,
                researching INTEGER NOT NULL DEFAULT 0,
                research_finish TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_fleet (
                ship_type TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS planet_creep (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                coverage_percent REAL NOT NULL DEFAULT 0.0,
                flora_corrupted REAL NOT NULL DEFAULT 0.0,
                fauna_consumed REAL NOT NULL DEFAULT 0.0,
                last_updated TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS planet_shop_active (
                item_id TEXT PRIMARY KEY,
                activated_at TEXT NOT NULL,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_achievements (
                achievement_id TEXT PRIMARY KEY,
                earned_at TEXT NOT NULL DEFAULT (datetime('now')),
                dark_matter_awarded INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS planet_login_streak (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                current_streak INTEGER NOT NULL DEFAULT 0,
                last_login TEXT NOT NULL DEFAULT (datetime('now')),
                total_logins INTEGER NOT NULL DEFAULT 0
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("PLANET_DB_INIT", format!("Planet table creation failed: {e}"))
        })?;

        // Seed swarm resources row if missing
        conn.execute(
            "INSERT OR IGNORE INTO swarm_resources (id, essence) VALUES (1, 100)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_SEED", format!("Swarm resources seed failed: {e}"))
        })?;

        // Seed planet resources
        conn.execute(
            "INSERT OR IGNORE INTO planet_resources (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("PLANET_SEED", format!("Planet resources seed failed: {e}"))
        })?;

        // Seed planet creep
        conn.execute(
            "INSERT OR IGNORE INTO planet_creep (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("CREEP_SEED", format!("Creep seed failed: {e}"))
        })?;

        // Seed planet login streak
        conn.execute(
            "INSERT OR IGNORE INTO planet_login_streak (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("LOGIN_SEED", format!("Login streak seed failed: {e}"))
        })?;

        // Seed default buildings and missions
        self.seed_swarm_buildings(&conn)?;
        self.seed_swarm_missions(&conn)?;
        self.seed_planet_buildings(&conn)?;
        self.seed_planet_research(&conn)?;
        self.seed_planet_fleet(&conn)?;

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

        // Also earn planet resources from every productivity action
        self.earn_planet_resources_from_action(&conn, action);

        // Also earn swarm resources from every action (drop the lock first)
        drop(conn);
        let _ = self.earn_swarm_resources(action);

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

    // =========================================================================
    // Forge Swarm — Colony-building meta-game
    // =========================================================================

    fn seed_swarm_buildings(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let buildings = [
            "nest", "evolution_chamber", "essence_pool", "neural_web",
            "armory", "sanctuary", "arcanum", "war_council",
        ];
        for bt in &buildings {
            conn.execute(
                "INSERT OR IGNORE INTO swarm_buildings (id, building_type, level) VALUES (?1, ?1, 0)",
                params![bt],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_swarm_missions(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let missions: Vec<(&str, &str, &str, &str, u32, u32, u64, u64, u64, u64, u64, &str)> = vec![
            ("m_gather", "Gather Essence", "Send a Drone to collect raw Essence from the forge.", "forge_drone", 1, 5, 50, 0, 0, 0, 0, "[]"),
            ("m_scout_web", "Scout the Web", "A Skyweaver scouts the internet for useful data.", "skyweaver", 1, 10, 30, 0, 10, 5, 0, "[\"Web Scroll\"]"),
            ("m_defend", "Defend the Hive", "Shadow Weavers patrol the perimeter for threats.", "shadow_weaver", 2, 15, 40, 0, 0, 0, 0, "[\"Security Report\"]"),
            ("m_raid_mine", "Raid the Data Mine", "Vipers infiltrate a rich data deposit.", "viper", 3, 20, 60, 200, 0, 0, 0, "[]"),
            ("m_arcane", "Arcane Research", "A Titan delves into deep reasoning and analysis.", "titan", 1, 30, 100, 0, 100, 0, 0, "[\"Arcane Tome\"]"),
            ("m_breed", "Breed New Larva", "The Swarm Mother produces new offspring for the hive.", "swarm_mother", 1, 60, 20, 0, 0, 20, 0, "[\"Larva Egg\",\"Larva Egg\",\"Larva Egg\"]"),
            ("m_boss", "Boss Challenge", "Assemble an elite squad to face a fearsome foe.", "any", 5, 45, 500, 50, 50, 50, 10, "[\"Legendary Token\"]"),
            ("m_neural", "Neural Expansion", "Overseers map the neural pathways of the hive mind.", "overseer", 2, 30, 80, 0, 0, 30, 0, "[\"Neural Fragment\"]"),
            ("m_dark", "Dark Matter Harvest", "A Ravager ventures into the void to harvest dark matter.", "ravager", 1, 40, 60, 0, 0, 0, 50, "[]"),
            ("m_final", "The Final Evolution", "The ultimate test. Matriarch leads the Titans to ascend.", "matriarch", 6, 120, 2000, 200, 200, 200, 100, "[\"Mythic Core\"]"),
        ];

        for (id, name, desc, req_types, req_count, dur, ess, min, ves, bio, dm, items) in &missions {
            conn.execute(
                "INSERT OR IGNORE INTO swarm_missions
                 (id, name, description, required_unit_types, required_unit_count,
                  duration_minutes, reward_essence, reward_minerals, reward_vespene,
                  reward_biomass, reward_dark_matter, reward_items)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    id, name, desc,
                    format!("[\"{req_types}\"]"),
                    req_count, dur, ess, min, ves, bio, dm, items
                ],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_MISSION_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    // -- Swarm state ----------------------------------------------------------

    pub fn get_swarm(&self) -> Result<SwarmState, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let units = self.load_swarm_units(&conn)?;
        let buildings = self.load_swarm_buildings(&conn)?;
        let resources = self.load_swarm_resources(&conn)?;

        // Calculate max units from Nest level
        let nest_level = buildings.iter()
            .find(|b| b.building_type == BuildingType::Nest)
            .map(|b| b.level)
            .unwrap_or(0);
        let max_units = 10 + nest_level * 5;

        // Calculate max essence from EssencePool level
        let pool_level = buildings.iter()
            .find(|b| b.building_type == BuildingType::EssencePool)
            .map(|b| b.level)
            .unwrap_or(0);
        let max_essence = 1000 + (pool_level as u64) * 1000;

        let evolution_paths = all_evolution_paths();

        Ok(SwarmState {
            units,
            buildings,
            resources,
            max_units,
            max_essence,
            evolution_paths,
        })
    }

    fn load_swarm_units(&self, conn: &Connection) -> Result<Vec<SwarmUnit>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, unit_type, name, level, hp, attack, defense,
                        special_ability, assigned_task, efficiency
                 FROM swarm_units ORDER BY level DESC, name",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNITS", format!("{e}")))?;

        let units = stmt
            .query_map([], |r| {
                let ut = UnitType::from_str(&r.get::<_, String>(1)?);
                Ok(SwarmUnit {
                    id: r.get(0)?,
                    unit_type: ut,
                    name: r.get(2)?,
                    level: r.get(3)?,
                    hp: r.get(4)?,
                    attack: r.get(5)?,
                    defense: r.get(6)?,
                    special_ability: r.get(7)?,
                    assigned_task: r.get(8)?,
                    efficiency: r.get(9)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_UNITS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(units)
    }

    fn load_swarm_buildings(&self, conn: &Connection) -> Result<Vec<Building>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT id, building_type, level FROM swarm_buildings ORDER BY building_type")
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG", format!("{e}")))?;

        let buildings = stmt
            .query_map([], |r| {
                let bt = BuildingType::from_str(&r.get::<_, String>(1)?);
                let level: u32 = r.get(2)?;
                Ok(Building {
                    id: r.get(0)?,
                    building_type: bt.clone(),
                    level,
                    max_level: bt.max_level(),
                    bonus: bt.bonus_description(level),
                    upgrade_cost: bt.base_upgrade_cost() * (level as u64 + 1),
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(buildings)
    }

    fn load_swarm_resources(&self, conn: &Connection) -> Result<SwarmResources, ImpForgeError> {
        conn.query_row(
            "SELECT essence, minerals, vespene, biomass, dark_matter FROM swarm_resources WHERE id = 1",
            [],
            |r| {
                Ok(SwarmResources {
                    essence: r.get(0)?,
                    minerals: r.get(1)?,
                    vespene: r.get(2)?,
                    biomass: r.get(3)?,
                    dark_matter: r.get(4)?,
                })
            },
        )
        .map_err(|e| ImpForgeError::internal("SWARM_RES", format!("{e}")))
    }

    // -- Spawn & Evolve -------------------------------------------------------

    pub fn spawn_larva(&self) -> Result<SwarmUnit, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check unit cap
        let unit_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM swarm_units", [], |r| r.get(0))
            .unwrap_or(0);

        let nest_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'nest'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let max_units = 10 + nest_level * 5;
        if unit_count >= max_units {
            return Err(ImpForgeError::validation(
                "SWARM_CAP",
                format!("Unit cap reached ({unit_count}/{max_units}). Upgrade your Nest."),
            ));
        }

        // Spawning a Larva costs 25 Essence (first one free if essence >= 25)
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        let spawn_cost: u64 = 25;
        if essence < spawn_cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {spawn_cost} Essence to spawn a Larva. Have {essence}."),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![spawn_cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_SPEND", format!("{e}")))?;

        // Create the larva as a ForgeDrone (Tier 1 default)
        let now = Utc::now().timestamp_millis();
        let id = format!("larva_{now}");
        let name = format!("Larva #{}", unit_count + 1);
        let (hp, atk, def) = UnitType::ForgeDrone.base_stats();

        let unit = SwarmUnit {
            id: id.clone(),
            unit_type: UnitType::ForgeDrone,
            name: name.clone(),
            level: 1,
            hp,
            attack: atk,
            defense: def,
            special_ability: UnitType::ForgeDrone.special_ability().to_string(),
            assigned_task: None,
            efficiency: 0.5,
        };

        conn.execute(
            "INSERT INTO swarm_units (id, unit_type, name, level, hp, attack, defense, special_ability, efficiency)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                unit.id, unit.unit_type.as_str(), unit.name,
                unit.level, unit.hp, unit.attack, unit.defense,
                unit.special_ability, unit.efficiency,
            ],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_SPAWN", format!("{e}")))?;

        Ok(unit)
    }

    pub fn evolve_unit(&self, unit_id: &str, target_type: &str) -> Result<SwarmUnit, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load the unit
        let (current_type_str, level, efficiency): (String, u32, f32) = conn
            .query_row(
                "SELECT unit_type, level, efficiency FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation("SWARM_NO_UNIT", format!("Unit '{unit_id}' not found."))
            })?;

        let target = UnitType::from_str(target_type);

        // Find the evolution path
        let paths = all_evolution_paths();
        let path = paths.iter()
            .find(|p| p.from == current_type_str && p.to == target.as_str())
            .ok_or_else(|| {
                ImpForgeError::validation(
                    "SWARM_NO_PATH",
                    format!("No evolution path from '{}' to '{}'.", current_type_str, target.as_str()),
                )
            })?;

        // Check level requirement
        if level < path.level_requirement {
            return Err(ImpForgeError::validation(
                "SWARM_LOW_LEVEL",
                format!("Unit needs level {} (currently {})", path.level_requirement, level),
            ));
        }

        // Check evolution chamber level (must be >= target tier)
        let chamber_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'evolution_chamber'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if chamber_level < target.tier() {
            return Err(ImpForgeError::validation(
                "SWARM_CHAMBER",
                format!(
                    "Evolution Chamber level {} required (have {}). Upgrade it first.",
                    target.tier(), chamber_level
                ),
            ));
        }

        // Matriarch uniqueness check
        if target == UnitType::Matriarch {
            let existing: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'matriarch'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if existing > 0 {
                return Err(ImpForgeError::validation(
                    "SWARM_MATRIARCH_UNIQUE",
                    "Only one Matriarch may exist in the swarm.",
                ));
            }
        }

        // Check and spend Essence
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if essence < path.essence_cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {} Essence (have {}).", path.essence_cost, essence),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![path.essence_cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EVOLVE_PAY", format!("{e}")))?;

        // Evolve the unit
        let (new_hp, new_atk, new_def) = target.base_stats();
        // Carry over efficiency bonus
        let evolved_efficiency = (efficiency + 0.1).min(2.0);

        conn.execute(
            "UPDATE swarm_units SET unit_type = ?1, hp = ?2, attack = ?3, defense = ?4,
             special_ability = ?5, efficiency = ?6 WHERE id = ?7",
            params![
                target.as_str(), new_hp, new_atk, new_def,
                target.special_ability(), evolved_efficiency, unit_id,
            ],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EVOLVE", format!("{e}")))?;

        // Return the evolved unit
        let unit = SwarmUnit {
            id: unit_id.to_string(),
            unit_type: target.clone(),
            name: format!("{} (evolved)", target.emoji()),
            level,
            hp: new_hp,
            attack: new_atk,
            defense: new_def,
            special_ability: target.special_ability().to_string(),
            assigned_task: None,
            efficiency: evolved_efficiency,
        };

        Ok(unit)
    }

    // -- Buildings ------------------------------------------------------------

    pub fn upgrade_building(&self, building_type: &str) -> Result<Building, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let bt = BuildingType::from_str(building_type);

        let current_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = ?1",
                params![bt.as_str()],
                |r| r.get(0),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_BLDG",
                    format!("Building '{}' not found.", building_type),
                )
            })?;

        if current_level >= bt.max_level() {
            return Err(ImpForgeError::validation(
                "SWARM_BLDG_MAX",
                format!("'{}' is already at max level {}.", building_type, bt.max_level()),
            ));
        }

        let cost = bt.base_upgrade_cost() * (current_level as u64 + 1);
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if essence < cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {} Essence to upgrade (have {}).", cost, essence),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_BLDG_PAY", format!("{e}")))?;

        let new_level = current_level + 1;
        conn.execute(
            "UPDATE swarm_buildings SET level = ?1 WHERE building_type = ?2",
            params![new_level, bt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_BLDG_UP", format!("{e}")))?;

        Ok(Building {
            id: bt.as_str().to_string(),
            building_type: bt.clone(),
            level: new_level,
            max_level: bt.max_level(),
            bonus: bt.bonus_description(new_level),
            upgrade_cost: bt.base_upgrade_cost() * (new_level as u64 + 1),
        })
    }

    // -- Missions -------------------------------------------------------------

    pub fn get_missions(&self) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;
        self.load_swarm_missions(&conn)
    }

    fn load_swarm_missions(&self, conn: &Connection) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, required_unit_types, required_unit_count,
                        assigned_units, duration_minutes, reward_essence, reward_minerals,
                        reward_vespene, reward_biomass, reward_dark_matter, reward_items,
                        status, started_at
                 FROM swarm_missions ORDER BY status, duration_minutes",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_MISSIONS", format!("{e}")))?;

        let missions = stmt
            .query_map([], |r| {
                let req_types_json: String = r.get(3)?;
                let assigned_json: String = r.get(5)?;
                let items_json: String = r.get(12)?;
                Ok(SwarmMission {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    required_unit_types: serde_json::from_str(&req_types_json).unwrap_or_default(),
                    required_unit_count: r.get(4)?,
                    assigned_units: serde_json::from_str(&assigned_json).unwrap_or_default(),
                    duration_minutes: r.get(6)?,
                    reward: SwarmResources {
                        essence: r.get(7)?,
                        minerals: r.get(8)?,
                        vespene: r.get(9)?,
                        biomass: r.get(10)?,
                        dark_matter: r.get(11)?,
                    },
                    reward_items: serde_json::from_str(&items_json).unwrap_or_default(),
                    status: MissionStatus::from_str(&r.get::<_, String>(13)?),
                    started_at: r.get(14)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_MISSIONS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(missions)
    }

    pub fn assign_mission(
        &self,
        mission_id: &str,
        unit_ids: Vec<String>,
    ) -> Result<SwarmMission, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check mission exists and is available
        let status_str: String = conn
            .query_row(
                "SELECT status FROM swarm_missions WHERE id = ?1",
                params![mission_id],
                |r| r.get(0),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_MISSION",
                    format!("Mission '{}' not found.", mission_id),
                )
            })?;

        if status_str != "available" {
            return Err(ImpForgeError::validation(
                "SWARM_MISSION_BUSY",
                format!("Mission '{}' is not available (status: {}).", mission_id, status_str),
            ));
        }

        // Verify all units exist and are not already assigned
        for uid in &unit_ids {
            let task: Option<String> = conn
                .query_row(
                    "SELECT assigned_task FROM swarm_units WHERE id = ?1",
                    params![uid],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    ImpForgeError::validation(
                        "SWARM_NO_UNIT",
                        format!("Unit '{}' not found.", uid),
                    )
                })?;

            if task.is_some() {
                return Err(ImpForgeError::validation(
                    "SWARM_UNIT_BUSY",
                    format!("Unit '{}' is already on a task.", uid),
                ));
            }
        }

        // Assign units to the mission
        let assigned_json = serde_json::to_string(&unit_ids).unwrap_or_else(|_| "[]".to_string());
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE swarm_missions SET status = 'in_progress', assigned_units = ?1, started_at = ?2 WHERE id = ?3",
            params![assigned_json, now, mission_id],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_ASSIGN", format!("{e}")))?;

        // Mark units as assigned
        for uid in &unit_ids {
            conn.execute(
                "UPDATE swarm_units SET assigned_task = ?1 WHERE id = ?2",
                params![mission_id, uid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNIT_ASSIGN", format!("{e}")))?;
        }

        // Return updated mission
        let missions = self.load_swarm_missions(&conn)?;
        missions
            .into_iter()
            .find(|m| m.id == mission_id)
            .ok_or_else(|| {
                ImpForgeError::internal("SWARM_MISSION_LOST", "Mission disappeared after assign.")
            })
    }

    pub fn collect_mission(&self, mission_id: &str) -> Result<MissionReward, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load mission
        let (status_str, started_at_opt, duration, name,
             r_ess, r_min, r_ves, r_bio, r_dm, items_json, assigned_json): (
            String, Option<String>, u32, String,
            u64, u64, u64, u64, u64, String, String,
        ) = conn
            .query_row(
                "SELECT status, started_at, duration_minutes, name,
                        reward_essence, reward_minerals, reward_vespene,
                        reward_biomass, reward_dark_matter, reward_items, assigned_units
                 FROM swarm_missions WHERE id = ?1",
                params![mission_id],
                |r| Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?,
                )),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_MISSION",
                    format!("Mission '{}' not found.", mission_id),
                )
            })?;

        if status_str != "in_progress" {
            return Err(ImpForgeError::validation(
                "SWARM_NOT_ACTIVE",
                format!("Mission '{}' is not in progress.", mission_id),
            ));
        }

        // Check if enough time has passed
        if let Some(ref started) = started_at_opt {
            if let Ok(start_time) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = Utc::now().signed_duration_since(start_time.with_timezone(&Utc));
                let needed = chrono::Duration::minutes(duration as i64);
                if elapsed < needed {
                    let remaining = needed - elapsed;
                    return Err(ImpForgeError::validation(
                        "SWARM_NOT_DONE",
                        format!(
                            "Mission not complete. {} minutes remaining.",
                            remaining.num_minutes().max(1)
                        ),
                    ));
                }
            }
        }

        // Grant resources
        conn.execute(
            "UPDATE swarm_resources SET
                essence = essence + ?1,
                minerals = minerals + ?2,
                vespene = vespene + ?3,
                biomass = biomass + ?4,
                dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![r_ess, r_min, r_ves, r_bio, r_dm],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_REWARD", format!("{e}")))?;

        // Free up assigned units and grant XP to them
        let assigned_ids: Vec<String> =
            serde_json::from_str(&assigned_json).unwrap_or_default();
        for uid in &assigned_ids {
            conn.execute(
                "UPDATE swarm_units SET assigned_task = NULL,
                    level = level + 1,
                    efficiency = MIN(2.0, efficiency + 0.05)
                 WHERE id = ?1",
                params![uid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNIT_FREE", format!("{e}")))?;
        }

        // Reset mission to available
        conn.execute(
            "UPDATE swarm_missions SET status = 'available', assigned_units = '[]', started_at = NULL WHERE id = ?1",
            params![mission_id],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_MISSION_RESET", format!("{e}")))?;

        let items: Vec<String> = serde_json::from_str(&items_json).unwrap_or_default();

        Ok(MissionReward {
            resources: SwarmResources {
                essence: r_ess,
                minerals: r_min,
                vespene: r_ves,
                biomass: r_bio,
                dark_matter: r_dm,
            },
            items,
            xp_earned: r_ess / 2, // Bonus RPG XP from missions
            mission_name: name,
        })
    }

    pub fn swarm_auto_assign(&self) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if WarCouncil is built (level >= 1)
        let wc_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'war_council'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if wc_level < 1 {
            return Err(ImpForgeError::validation(
                "SWARM_NO_WC",
                "Build a War Council (level 1+) to unlock auto-assign.",
            ));
        }

        // Get available missions sorted by reward value
        let mut avail_stmt = conn
            .prepare(
                "SELECT id, required_unit_types, required_unit_count
                 FROM swarm_missions WHERE status = 'available'
                 ORDER BY (reward_essence + reward_minerals + reward_vespene + reward_biomass + reward_dark_matter * 5) DESC",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO", format!("{e}")))?;

        let missions: Vec<(String, String, u32)> = avail_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        // Get idle units (no assigned_task)
        let mut idle_stmt = conn
            .prepare(
                "SELECT id, unit_type FROM swarm_units WHERE assigned_task IS NULL ORDER BY level DESC",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_IDLE", format!("{e}")))?;

        let mut idle_units: Vec<(String, String)> = idle_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_Q2", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        let mut assigned_missions = Vec::new();
        let now = Utc::now().to_rfc3339();

        for (mid, req_types_json, req_count) in &missions {
            let req_types: Vec<String> =
                serde_json::from_str(req_types_json).unwrap_or_default();
            let count = *req_count as usize;

            if idle_units.len() < count {
                continue;
            }

            // Try to match required types
            let mut selected = Vec::new();
            let is_any = req_types.iter().any(|t| t == "any");

            for i in (0..idle_units.len()).rev() {
                if selected.len() >= count {
                    break;
                }
                let (ref uid, ref utype) = idle_units[i];
                if is_any || req_types.iter().any(|t| t == utype) {
                    selected.push(uid.clone());
                    idle_units.remove(i);
                }
            }

            if selected.len() < count {
                // Put units back (they were removed speculatively)
                // Actually we only removed matching ones, so just continue
                continue;
            }

            // Assign this mission
            let assigned_json = serde_json::to_string(&selected).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE swarm_missions SET status = 'in_progress', assigned_units = ?1, started_at = ?2 WHERE id = ?3",
                params![assigned_json, now, mid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_ASSIGN", format!("{e}")))?;

            for uid in &selected {
                conn.execute(
                    "UPDATE swarm_units SET assigned_task = ?1 WHERE id = ?2",
                    params![mid, uid],
                )
                .map_err(|e| ImpForgeError::internal("SWARM_AUTO_UNIT", format!("{e}")))?;
            }

            assigned_missions.push(mid.clone());
        }

        // Return updated missions
        let all = self.load_swarm_missions(&conn)?;
        Ok(all
            .into_iter()
            .filter(|m| assigned_missions.contains(&m.id))
            .collect())
    }

    // -- Resource earning from productivity -----------------------------------

    pub fn earn_swarm_resources(&self, action: &str) -> Result<SwarmResources, ImpForgeError> {
        let earned = swarm_resources_for_action(action);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if any ForgeDrone exists for bonus
        let drone_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'forge_drone'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Matriarch bonus
        let matriarch_bonus: f64 = conn
            .query_row(
                "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'matriarch'",
                [],
                |r| r.get::<_, u32>(0),
            )
            .map(|c| if c > 0 { 1.2 } else { 1.0 })
            .unwrap_or(1.0);

        let drone_bonus = 1.0 + (drone_count as f64 * 0.1); // +10% per drone
        let total_bonus = drone_bonus * matriarch_bonus;

        let actual = SwarmResources {
            essence: (earned.essence as f64 * total_bonus) as u64,
            minerals: (earned.minerals as f64 * total_bonus) as u64,
            vespene: (earned.vespene as f64 * total_bonus) as u64,
            biomass: (earned.biomass as f64 * total_bonus) as u64,
            dark_matter: (earned.dark_matter as f64 * total_bonus) as u64,
        };

        conn.execute(
            "UPDATE swarm_resources SET
                essence = essence + ?1,
                minerals = minerals + ?2,
                vespene = vespene + ?3,
                biomass = biomass + ?4,
                dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![actual.essence, actual.minerals, actual.vespene, actual.biomass, actual.dark_matter],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EARN", format!("{e}")))?;

        Ok(actual)
    }

    // =========================================================================
    // OGame-style Colony System
    // =========================================================================

    fn seed_planet_buildings(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "biomass_converter", "mineral_drill", "crystal_synthesizer",
            "spore_extractor", "energy_nest", "creep_generator",
            "brood_nest", "evolution_lab", "blighthaven",
            "spore_defense", "biomass_storage", "mineral_silo",
        ];
        for bt in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_buildings (building_type, level) VALUES (?1, 0)",
                params![bt],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_planet_research(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "genetics", "armor_plating", "weapon_systems", "propulsion_drive",
            "swarm_intelligence", "regeneration", "mutation_tech",
            "creep_biology", "space_faring", "dark_matter_research",
        ];
        for tt in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_research (tech_type, level) VALUES (?1, 0)",
                params![tt],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_planet_fleet(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "bio_fighter", "spore_interceptor", "kraken_frigate", "leviathan",
            "bio_transporter", "colony_pod", "devourer", "world_eater",
        ];
        for st in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_fleet (ship_type, count) VALUES (?1, 0)",
                params![st],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    // -- OGame production formulas --------------------------------------------

    /// OGame-style production: base_rate * level * 1.1^level
    fn ogame_production_rate(level: u32, base_rate: f64) -> f64 {
        if level == 0 { return 0.0; }
        base_rate * level as f64 * 1.1_f64.powi(level as i32)
    }

    /// OGame-style upgrade cost: base * factor^level
    fn ogame_upgrade_cost(base: f64, level: u32, factor: f64) -> f64 {
        base * factor.powi(level as i32)
    }

    /// Build time in seconds: (cost_bio + cost_min) / (2500 * (1 + brood_nest_level)) * 3600
    fn ogame_build_time(cost_bio: f64, cost_min: f64, brood_nest_level: u32) -> u64 {
        let hours = (cost_bio + cost_min) / (2500.0 * (1.0 + brood_nest_level as f64));
        (hours * 3600.0).max(30.0) as u64
    }

    fn storage_capacity(storage_level: u32) -> f64 {
        5000.0 * (2.5 * (1.0 + storage_level as f64)).floor()
    }

    // -- Planet state ---------------------------------------------------------

    pub fn get_planet(&self) -> Result<Planet, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let resources = self.calculate_planet_resources(&conn)?;
        let buildings = self.load_planet_buildings(&conn)?;
        let research = self.load_planet_research(&conn)?;
        let fleet = self.load_planet_fleet(&conn)?;
        let creep = self.load_creep_status(&conn)?;

        // Storage caps from BiomassStorage and MineralSilo
        let bio_storage_level = buildings.iter()
            .find(|b| b.building_type == PlanetBuildingType::BiomassStorage)
            .map(|b| b.level)
            .unwrap_or(0);
        let min_storage_level = buildings.iter()
            .find(|b| b.building_type == PlanetBuildingType::MineralSilo)
            .map(|b| b.level)
            .unwrap_or(0);

        Ok(Planet {
            name: "Hive Prime".to_string(),
            resources,
            buildings,
            research,
            fleet,
            creep,
            storage_biomass_cap: Self::storage_capacity(bio_storage_level),
            storage_minerals_cap: Self::storage_capacity(min_storage_level),
            storage_crystal_cap: Self::storage_capacity(0), // No crystal storage building yet
            storage_spore_gas_cap: Self::storage_capacity(0),
        })
    }

    fn calculate_planet_resources(&self, conn: &Connection) -> Result<PlanetResources, ImpForgeError> {
        let (biomass, minerals, crystal, spore_gas, dm, last_collected): (f64, f64, f64, f64, u64, String) =
            conn.query_row(
                "SELECT biomass, minerals, crystal, spore_gas, dark_matter, last_collected
                 FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES", format!("{e}")))?;

        // Calculate production rates from building levels
        let buildings = self.load_planet_building_levels(conn)?;
        let swarm_intel_level = self.get_research_level(conn, "swarm_intelligence");
        let swarm_bonus = 1.0 + swarm_intel_level as f64 * 0.05;

        // Creep bonus
        let creep_coverage: f64 = conn
            .query_row("SELECT coverage_percent FROM planet_creep WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0.0);
        let creep_bonus = if creep_coverage >= 100.0 { 1.5 }
            else if creep_coverage >= 50.0 { 1.2 }
            else { 1.0 };

        let bio_rate = Self::ogame_production_rate(buildings.get("biomass_converter").copied().unwrap_or(0), 30.0) * swarm_bonus * creep_bonus;
        let min_rate = Self::ogame_production_rate(buildings.get("mineral_drill").copied().unwrap_or(0), 20.0) * swarm_bonus * creep_bonus;
        let cry_rate = Self::ogame_production_rate(buildings.get("crystal_synthesizer").copied().unwrap_or(0), 10.0) * swarm_bonus * creep_bonus;
        let gas_rate = Self::ogame_production_rate(buildings.get("spore_extractor").copied().unwrap_or(0), 10.0) * swarm_bonus * creep_bonus;

        // Energy calculation
        let mut energy_prod: i64 = 0;
        let mut energy_cons: i64 = 0;
        for (bt_str, level) in &buildings {
            let bt = PlanetBuildingType::from_str(bt_str);
            let delta = bt.energy_per_level() * (*level as i64);
            if delta > 0 { energy_prod += delta; }
            else { energy_cons += delta.abs(); }
        }

        // Calculate elapsed time and accumulate resources
        let elapsed_hours = if let Ok(last) = chrono::NaiveDateTime::parse_from_str(&last_collected, "%Y-%m-%d %H:%M:%S") {
            let now = Utc::now().naive_utc();
            let diff = now.signed_duration_since(last);
            (diff.num_seconds() as f64 / 3600.0).max(0.0)
        } else {
            0.0
        };

        // Biomass bonus from creep consuming flora
        let flora_bonus = 1.0 + (creep_coverage as f64 / 100.0) * 0.5;

        let new_biomass = (biomass + bio_rate * elapsed_hours * flora_bonus).min(Self::storage_capacity(
            buildings.get("biomass_storage").copied().unwrap_or(0)));
        let new_minerals = (minerals + min_rate * elapsed_hours).min(Self::storage_capacity(
            buildings.get("mineral_silo").copied().unwrap_or(0)));
        let new_crystal = (crystal + cry_rate * elapsed_hours).min(Self::storage_capacity(0));
        let new_spore_gas = (spore_gas + gas_rate * elapsed_hours).min(Self::storage_capacity(0));

        // Update stored resources with new values
        conn.execute(
            "UPDATE planet_resources SET biomass = ?1, minerals = ?2, crystal = ?3,
             spore_gas = ?4, last_collected = datetime('now') WHERE id = 1",
            params![new_biomass, new_minerals, new_crystal, new_spore_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_RES_UPDATE", format!("{e}")))?;

        Ok(PlanetResources {
            biomass: new_biomass,
            minerals: new_minerals,
            crystal: new_crystal,
            spore_gas: new_spore_gas,
            energy: energy_prod - energy_cons,
            dark_matter: dm,
            biomass_per_hour: bio_rate * flora_bonus,
            minerals_per_hour: min_rate,
            crystal_per_hour: cry_rate,
            spore_gas_per_hour: gas_rate,
            energy_production: energy_prod,
            energy_consumption: energy_cons,
        })
    }

    fn load_planet_building_levels(&self, conn: &Connection) -> Result<std::collections::HashMap<String, u32>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT building_type, level FROM planet_buildings")
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_LEVELS", format!("{e}")))?;

        let map: std::collections::HashMap<String, u32> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_LEVELS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(map)
    }

    fn get_research_level(&self, conn: &Connection, tech: &str) -> u32 {
        conn.query_row(
            "SELECT level FROM planet_research WHERE tech_type = ?1",
            params![tech],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    fn load_planet_buildings(&self, conn: &Connection) -> Result<Vec<PlanetBuilding>, ImpForgeError> {
        let brood_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'brood_nest'", [], |r| r.get(0))
            .unwrap_or(0);

        let mut stmt = conn
            .prepare("SELECT building_type, level, upgrading, upgrade_finish FROM planet_buildings ORDER BY building_type")
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG", format!("{e}")))?;

        let buildings = stmt
            .query_map([], |r| {
                let bt_str: String = r.get(0)?;
                let level: u32 = r.get(1)?;
                let upgrading: bool = r.get::<_, i32>(2)? != 0;
                let finish: Option<String> = r.get(3)?;
                Ok((bt_str, level, upgrading, finish))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(bt_str, level, upgrading, finish)| {
                let bt = PlanetBuildingType::from_str(&bt_str);
                let (base_bio, base_min, base_cry, base_gas, factor) = bt.base_costs();
                let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
                let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
                let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
                let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);
                let build_time = Self::ogame_build_time(cost_bio, cost_min, brood_level);

                PlanetBuilding {
                    display_name: bt.display_name().to_string(),
                    description: bt.description().to_string(),
                    building_type: bt,
                    level,
                    upgrading,
                    upgrade_finish: finish,
                    cost_biomass: cost_bio,
                    cost_minerals: cost_min,
                    cost_crystal: cost_cry,
                    cost_spore_gas: cost_gas,
                    build_time_seconds: build_time,
                }
            })
            .collect();

        Ok(buildings)
    }

    fn load_planet_research(&self, conn: &Connection) -> Result<Vec<Research>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT tech_type, level, researching, research_finish FROM planet_research ORDER BY tech_type")
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH", format!("{e}")))?;

        let research = stmt
            .query_map([], |r| {
                let tt_str: String = r.get(0)?;
                let level: u32 = r.get(1)?;
                let researching: bool = r.get::<_, i32>(2)? != 0;
                let finish: Option<String> = r.get(3)?;
                Ok((tt_str, level, researching, finish))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(tt_str, level, researching, finish)| {
                let tt = TechType::from_str(&tt_str);
                let (base_bio, base_min, base_cry, base_gas, factor) = tt.base_costs();
                let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
                let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
                let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
                let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);
                let research_time = (((cost_bio + cost_min) / 1000.0) * 3600.0).max(60.0) as u64;

                Research {
                    display_name: tt.display_name().to_string(),
                    description: tt.description().to_string(),
                    tech_type: tt.clone(),
                    level,
                    researching,
                    research_finish: finish,
                    cost_biomass: cost_bio,
                    cost_minerals: cost_min,
                    cost_crystal: cost_cry,
                    cost_spore_gas: cost_gas,
                    research_time_seconds: research_time,
                    required_lab_level: tt.required_lab_level(),
                }
            })
            .collect();

        Ok(research)
    }

    fn load_planet_fleet(&self, conn: &Connection) -> Result<Vec<Ship>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT ship_type, count FROM planet_fleet ORDER BY ship_type")
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET", format!("{e}")))?;

        let fleet = stmt
            .query_map([], |r| {
                let st_str: String = r.get(0)?;
                let count: u32 = r.get(1)?;
                Ok((st_str, count))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(st_str, count)| {
                let st = ShipType::from_str(&st_str);
                let (atk, shields, hp) = st.combat_stats();
                Ship {
                    display_name: st.display_name().to_string(),
                    description: st.description().to_string(),
                    ship_type: st,
                    count,
                    attack: atk,
                    shields,
                    hp,
                }
            })
            .collect();

        Ok(fleet)
    }

    fn load_creep_status(&self, conn: &Connection) -> Result<CreepStatus, ImpForgeError> {
        let (coverage, flora, fauna): (f64, f64, f64) = conn
            .query_row(
                "SELECT coverage_percent, flora_corrupted, fauna_consumed FROM planet_creep WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or((0.0, 0.0, 0.0));

        let gen_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'creep_generator'", [], |r| r.get(0))
            .unwrap_or(0);
        let creep_bio_level = self.get_research_level(conn, "creep_biology");

        let spread_rate = (gen_level as f32 * 0.5 + creep_bio_level as f32 * 0.3).max(0.0);
        let biomass_bonus = coverage as f32 / 100.0 * 50.0; // up to +50%

        Ok(CreepStatus {
            coverage_percent: coverage as f32,
            spread_rate_per_hour: spread_rate,
            flora_corrupted: flora as f32,
            fauna_consumed: fauna as f32,
            biomass_bonus,
        })
    }

    // -- Planet upgrades ------------------------------------------------------

    pub fn upgrade_planet_building(&self, building_type_str: &str) -> Result<PlanetBuilding, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let bt = PlanetBuildingType::from_str(building_type_str);

        // Check if already upgrading something
        let upgrading_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM planet_buildings WHERE upgrading = 1", [], |r| r.get(0))
            .unwrap_or(0);
        if upgrading_count > 0 {
            return Err(ImpForgeError::validation(
                "PLANET_BUSY", "Already upgrading a building. Wait for it to finish.",
            ));
        }

        let level: u32 = conn
            .query_row(
                "SELECT level FROM planet_buildings WHERE building_type = ?1",
                params![bt.as_str()],
                |r| r.get(0),
            )
            .map_err(|_| ImpForgeError::validation("PLANET_NO_BLDG", format!("Building '{}' not found.", building_type_str)))?;

        // Blighthaven requires SpaceFaring research
        if bt == PlanetBuildingType::Blighthaven && level == 0 {
            let sf_level = self.get_research_level(&conn, "space_faring");
            if sf_level < 1 {
                return Err(ImpForgeError::validation(
                    "PLANET_NEED_RESEARCH", "Blighthaven requires Space Faring research level 1.",
                ));
            }
        }

        // Calculate costs
        let (base_bio, base_min, base_cry, base_gas, factor) = bt.base_costs();
        let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
        let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
        let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
        let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);

        // Check resources (collect first)
        drop(conn);
        let resources = {
            let conn2 = self.conn.lock().map_err(|e| {
                ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
            })?;
            self.calculate_planet_resources(&conn2)?
        };

        if resources.biomass < cost_bio || resources.minerals < cost_min
            || resources.crystal < cost_cry || resources.spore_gas < cost_gas {
            return Err(ImpForgeError::validation(
                "PLANET_NO_RES",
                format!("Insufficient resources. Need: {:.0}B {:.0}M {:.0}C {:.0}G",
                    cost_bio, cost_min, cost_cry, cost_gas),
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Deduct resources
        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![cost_bio, cost_min, cost_cry, cost_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT", format!("{e}")))?;

        // Calculate build time
        let brood_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'brood_nest'", [], |r| r.get(0))
            .unwrap_or(0);
        let build_secs = Self::ogame_build_time(cost_bio, cost_min, brood_level);
        let finish = Utc::now() + chrono::Duration::seconds(build_secs as i64);

        conn.execute(
            "UPDATE planet_buildings SET upgrading = 1, upgrade_finish = ?1 WHERE building_type = ?2",
            params![finish.to_rfc3339(), bt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_UPGRADE", format!("{e}")))?;

        let new_cost_bio = Self::ogame_upgrade_cost(base_bio, level + 1, factor);
        let new_cost_min = Self::ogame_upgrade_cost(base_min, level + 1, factor);
        let new_cost_cry = Self::ogame_upgrade_cost(base_cry, level + 1, factor);
        let new_cost_gas = Self::ogame_upgrade_cost(base_gas, level + 1, factor);

        Ok(PlanetBuilding {
            display_name: bt.display_name().to_string(),
            description: bt.description().to_string(),
            building_type: bt,
            level,
            upgrading: true,
            upgrade_finish: Some(finish.to_rfc3339()),
            cost_biomass: new_cost_bio,
            cost_minerals: new_cost_min,
            cost_crystal: new_cost_cry,
            cost_spore_gas: new_cost_gas,
            build_time_seconds: build_secs,
        })
    }

    pub fn collect_planet_resources(&self) -> Result<PlanetResources, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;
        self.calculate_planet_resources(&conn)
    }

    pub fn start_research(&self, tech_str: &str) -> Result<Research, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let tt = TechType::from_str(tech_str);

        // Check if already researching
        let researching_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM planet_research WHERE researching = 1", [], |r| r.get(0))
            .unwrap_or(0);
        if researching_count > 0 {
            return Err(ImpForgeError::validation(
                "PLANET_RESEARCH_BUSY", "Already researching. Wait for it to finish.",
            ));
        }

        // Check Evolution Lab level
        let lab_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'evolution_lab'", [], |r| r.get(0))
            .unwrap_or(0);
        if lab_level < tt.required_lab_level() {
            return Err(ImpForgeError::validation(
                "PLANET_LAB_LOW",
                format!("Requires Evolution Lab level {} (have {}).", tt.required_lab_level(), lab_level),
            ));
        }

        let level: u32 = conn
            .query_row(
                "SELECT level FROM planet_research WHERE tech_type = ?1",
                params![tt.as_str()],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let (base_bio, base_min, base_cry, base_gas, factor) = tt.base_costs();
        let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
        let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
        let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
        let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);

        // Check resources
        let (bio, min, cry, gas): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT biomass, minerals, crystal, spore_gas FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES_CHECK", format!("{e}")))?;

        if bio < cost_bio || min < cost_min || cry < cost_cry || gas < cost_gas {
            return Err(ImpForgeError::validation("PLANET_NO_RES", "Insufficient resources for research."));
        }

        // Deduct
        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![cost_bio, cost_min, cost_cry, cost_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT_R", format!("{e}")))?;

        let research_secs = (((cost_bio + cost_min) / 1000.0) * 3600.0).max(60.0) as i64;
        let finish = Utc::now() + chrono::Duration::seconds(research_secs);

        conn.execute(
            "UPDATE planet_research SET researching = 1, research_finish = ?1 WHERE tech_type = ?2",
            params![finish.to_rfc3339(), tt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_START", format!("{e}")))?;

        let new_cost_bio = Self::ogame_upgrade_cost(base_bio, level + 1, factor);
        let new_cost_min = Self::ogame_upgrade_cost(base_min, level + 1, factor);

        Ok(Research {
            display_name: tt.display_name().to_string(),
            description: tt.description().to_string(),
            tech_type: tt.clone(),
            level,
            researching: true,
            research_finish: Some(finish.to_rfc3339()),
            cost_biomass: Self::ogame_upgrade_cost(base_bio, level + 1, factor),
            cost_minerals: new_cost_min,
            cost_crystal: Self::ogame_upgrade_cost(base_cry, level + 1, factor),
            cost_spore_gas: Self::ogame_upgrade_cost(base_gas, level + 1, factor),
            research_time_seconds: (((new_cost_bio + new_cost_min) / 1000.0) * 3600.0).max(60.0) as u64,
            required_lab_level: tt.required_lab_level(),
        })
    }

    pub fn build_ships(&self, ship_type_str: &str, count: u32) -> Result<Ship, ImpForgeError> {
        if count == 0 {
            return Err(ImpForgeError::validation("PLANET_ZERO", "Must build at least 1 ship."));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let st = ShipType::from_str(ship_type_str);

        // Check Blighthaven level
        let shipyard_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'blighthaven'", [], |r| r.get(0))
            .unwrap_or(0);
        if shipyard_level < st.required_shipyard_level() {
            return Err(ImpForgeError::validation(
                "PLANET_SHIPYARD_LOW",
                format!("Requires Blighthaven level {} (have {}).", st.required_shipyard_level(), shipyard_level),
            ));
        }

        // WorldEater requires SpaceFaring level 20 and creep 75%
        if st == ShipType::WorldEater {
            let sf = self.get_research_level(&conn, "space_faring");
            if sf < 20 {
                return Err(ImpForgeError::validation("PLANET_WE_RESEARCH", "World Eater requires Space Faring level 20."));
            }
            let creep: f64 = conn
                .query_row("SELECT coverage_percent FROM planet_creep WHERE id = 1", [], |r| r.get(0))
                .unwrap_or(0.0);
            if creep < 75.0 {
                return Err(ImpForgeError::validation("PLANET_WE_CREEP", "World Eater requires 75% creep coverage."));
            }
        }

        let (cost_bio, cost_min, cost_cry, cost_gas) = st.unit_cost();
        let total_bio = cost_bio * count as f64;
        let total_min = cost_min * count as f64;
        let total_cry = cost_cry * count as f64;
        let total_gas = cost_gas * count as f64;

        let (bio, min, cry, gas): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT biomass, minerals, crystal, spore_gas FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES_SHIP", format!("{e}")))?;

        if bio < total_bio || min < total_min || cry < total_cry || gas < total_gas {
            return Err(ImpForgeError::validation("PLANET_NO_RES", "Insufficient resources to build ships."));
        }

        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![total_bio, total_min, total_cry, total_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT_S", format!("{e}")))?;

        conn.execute(
            "UPDATE planet_fleet SET count = count + ?1 WHERE ship_type = ?2",
            params![count, st.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_BUILD_SHIP", format!("{e}")))?;

        let new_count: u32 = conn
            .query_row("SELECT count FROM planet_fleet WHERE ship_type = ?1", params![st.as_str()], |r| r.get(0))
            .unwrap_or(count);

        let (atk, shields, hp) = st.combat_stats();
        Ok(Ship {
            display_name: st.display_name().to_string(),
            description: st.description().to_string(),
            ship_type: st,
            count: new_count,
            attack: atk,
            shields,
            hp,
        })
    }

    pub fn get_creep(&self) -> Result<CreepStatus, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Update creep spread based on elapsed time
        let (coverage, last_updated): (f64, String) = conn
            .query_row(
                "SELECT coverage_percent, last_updated FROM planet_creep WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0.0, Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()));

        let gen_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'creep_generator'", [], |r| r.get(0))
            .unwrap_or(0);
        let creep_bio_level = self.get_research_level(&conn, "creep_biology");
        let spread_rate = gen_level as f64 * 0.5 + creep_bio_level as f64 * 0.3;

        let elapsed_hours = if let Ok(last) = chrono::NaiveDateTime::parse_from_str(&last_updated, "%Y-%m-%d %H:%M:%S") {
            let now = Utc::now().naive_utc();
            (now.signed_duration_since(last).num_seconds() as f64 / 3600.0).max(0.0)
        } else { 0.0 };

        let new_coverage = (coverage + spread_rate * elapsed_hours).min(100.0);
        let new_flora = (new_coverage * 0.8).min(100.0);
        let new_fauna = (new_coverage * 0.6).min(100.0);

        conn.execute(
            "UPDATE planet_creep SET coverage_percent = ?1, flora_corrupted = ?2,
             fauna_consumed = ?3, last_updated = datetime('now') WHERE id = 1",
            params![new_coverage, new_flora, new_fauna],
        )
        .map_err(|e| ImpForgeError::internal("CREEP_UPDATE", format!("{e}")))?;

        Ok(CreepStatus {
            coverage_percent: new_coverage as f32,
            spread_rate_per_hour: spread_rate as f32,
            flora_corrupted: new_flora as f32,
            fauna_consumed: new_fauna as f32,
            biomass_bonus: new_coverage as f32 / 100.0 * 50.0,
        })
    }

    #[allow(dead_code)]
    pub fn get_shop_items(&self) -> Result<Vec<ShopItem>, ImpForgeError> {
        Ok(all_shop_items())
    }

    pub fn buy_shop_item(&self, item_id: &str) -> Result<ShopItem, ImpForgeError> {
        let items = all_shop_items();
        let item = items.iter().find(|i| i.id == item_id).ok_or_else(|| {
            ImpForgeError::validation("SHOP_NOT_FOUND", format!("Shop item '{}' not found.", item_id))
        })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let dm: u64 = conn
            .query_row("SELECT dark_matter FROM planet_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if dm < item.cost_dark_matter {
            return Err(ImpForgeError::validation(
                "SHOP_NO_DM",
                format!("Need {} Dark Matter (have {}).", item.cost_dark_matter, dm),
            ));
        }

        conn.execute(
            "UPDATE planet_resources SET dark_matter = dark_matter - ?1 WHERE id = 1",
            params![item.cost_dark_matter],
        )
        .map_err(|e| ImpForgeError::internal("SHOP_BUY", format!("{e}")))?;

        let expires = item.duration_hours.map(|h| {
            (Utc::now() + chrono::Duration::hours(h as i64)).to_rfc3339()
        });

        conn.execute(
            "INSERT OR REPLACE INTO planet_shop_active (item_id, activated_at, expires_at) VALUES (?1, ?2, ?3)",
            params![item.id, Utc::now().to_rfc3339(), expires],
        )
        .map_err(|e| ImpForgeError::internal("SHOP_ACTIVATE", format!("{e}")))?;

        Ok(item.clone())
    }

    pub fn get_galaxy(&self, _galaxy: u32, _system: u32) -> Result<Vec<PlanetSlot>, ImpForgeError> {
        // Single-player galaxy view -- show the player's planet plus procedural neighbors
        let mut slots = Vec::with_capacity(15);
        for pos in 1..=15 {
            if pos == 4 {
                slots.push(PlanetSlot {
                    position: pos,
                    occupied: true,
                    planet_name: Some("Hive Prime".to_string()),
                    player_name: Some("You".to_string()),
                    planet_type: Some("terran".to_string()),
                });
            } else {
                // Deterministic procedural occupation
                let occupied = (pos * 7 + _galaxy * 13 + _system * 31) % 5 < 2;
                slots.push(PlanetSlot {
                    position: pos,
                    occupied,
                    planet_name: if occupied { Some(format!("Planet {}-{}-{}", _galaxy, _system, pos)) } else { None },
                    player_name: if occupied { Some(format!("NPC_{}", pos * _system + _galaxy)) } else { None },
                    planet_type: if occupied {
                        Some(match pos % 4 { 0 => "desert", 1 => "ice", 2 => "gas", _ => "terran" }.to_string())
                    } else { None },
                });
            }
        }
        Ok(slots)
    }

    pub fn check_timers(&self) -> Result<Vec<CompletedTimer>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let now = Utc::now();
        let mut completed = Vec::new();

        // Check building timers
        let mut stmt = conn
            .prepare("SELECT building_type, upgrade_finish FROM planet_buildings WHERE upgrading = 1 AND upgrade_finish IS NOT NULL")
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK", format!("{e}")))?;

        let building_timers: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (bt_str, finish_str) in building_timers {
            if let Ok(finish_time) = chrono::DateTime::parse_from_rfc3339(&finish_str) {
                if now >= finish_time.with_timezone(&Utc) {
                    // Complete the upgrade
                    conn.execute(
                        "UPDATE planet_buildings SET level = level + 1, upgrading = 0, upgrade_finish = NULL WHERE building_type = ?1",
                        params![bt_str],
                    )
                    .map_err(|e| ImpForgeError::internal("TIMER_COMPLETE_B", format!("{e}")))?;

                    let bt = PlanetBuildingType::from_str(&bt_str);
                    completed.push(CompletedTimer {
                        timer_type: "building".to_string(),
                        item_name: bt.display_name().to_string(),
                        completed_at: now.to_rfc3339(),
                    });
                }
            }
        }

        // Check research timers
        let mut stmt2 = conn
            .prepare("SELECT tech_type, research_finish FROM planet_research WHERE researching = 1 AND research_finish IS NOT NULL")
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_R", format!("{e}")))?;

        let research_timers: Vec<(String, String)> = stmt2
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_R_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (tt_str, finish_str) in research_timers {
            if let Ok(finish_time) = chrono::DateTime::parse_from_rfc3339(&finish_str) {
                if now >= finish_time.with_timezone(&Utc) {
                    conn.execute(
                        "UPDATE planet_research SET level = level + 1, researching = 0, research_finish = NULL WHERE tech_type = ?1",
                        params![tt_str],
                    )
                    .map_err(|e| ImpForgeError::internal("TIMER_COMPLETE_R", format!("{e}")))?;

                    let tt = TechType::from_str(&tt_str);
                    completed.push(CompletedTimer {
                        timer_type: "research".to_string(),
                        item_name: tt.display_name().to_string(),
                        completed_at: now.to_rfc3339(),
                    });
                }
            }
        }

        Ok(completed)
    }

    /// Award dark matter from achievements, daily login, etc.
    #[allow(dead_code)]
    pub fn award_dark_matter(&self, amount: u64, reason: &str) -> Result<u64, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE planet_resources SET dark_matter = dark_matter + ?1 WHERE id = 1",
            params![amount],
        )
        .map_err(|e| ImpForgeError::internal("DM_AWARD", format!("{e}")))?;

        // Track achievement
        conn.execute(
            "INSERT OR IGNORE INTO planet_achievements (achievement_id, dark_matter_awarded) VALUES (?1, ?2)",
            params![reason, amount],
        )
        .map_err(|e| ImpForgeError::internal("DM_TRACK", format!("{e}")))?;

        let total: u64 = conn
            .query_row("SELECT dark_matter FROM planet_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        Ok(total)
    }

    /// Called from track_action to also give planet resources from productivity
    fn earn_planet_resources_from_action(&self, conn: &Connection, action: &str) {
        let (bio, min, cry, gas, dm) = match action {
            "create_document" => (15.0, 0.0, 0.0, 0.0, 0u64),
            "run_workflow" => (10.0, 5.0, 0.0, 2.0, 0),
            "ai_query" => (5.0, 0.0, 3.0, 0.0, 0),
            "create_spreadsheet" => (8.0, 10.0, 0.0, 0.0, 0),
            "social_post" => (12.0, 0.0, 0.0, 0.0, 0),
            "create_note" => (10.0, 0.0, 0.0, 0.0, 0),
            "create_slide" => (8.0, 5.0, 0.0, 0.0, 0),
            "complete_quest" => (30.0, 10.0, 5.0, 5.0, 5),
            _ => (3.0, 0.0, 0.0, 0.0, 0),
        };

        let _ = conn.execute(
            "UPDATE planet_resources SET biomass = biomass + ?1, minerals = minerals + ?2,
             crystal = crystal + ?3, spore_gas = spore_gas + ?4, dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![bio, min, cry, gas, dm],
        );
    }
}

// ---------------------------------------------------------------------------
// Static data: Zones, Recipes, Action mapping, Swarm evolution paths
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
// Swarm static data: Evolution paths, resource mapping
// ---------------------------------------------------------------------------

fn all_evolution_paths() -> Vec<EvolutionPath> {
    vec![
        // Tier 1 -> Tier 2
        EvolutionPath {
            from: "forge_drone".into(), to: "viper".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Crystal".into(), 3)],
        },
        EvolutionPath {
            from: "forge_drone".into(), to: "shadow_weaver".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Shadow Steel".into(), 2)],
        },
        EvolutionPath {
            from: "imp_scout".into(), to: "skyweaver".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Cloud Pearl".into(), 2)],
        },
        EvolutionPath {
            from: "imp_scout".into(), to: "overseer".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Golem Core".into(), 2)],
        },
        // Tier 2 -> Tier 3
        EvolutionPath {
            from: "viper".into(), to: "titan".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Dragon Scale".into(), 3), ("Legendary Ingot".into(), 2)],
        },
        EvolutionPath {
            from: "viper".into(), to: "swarm_mother".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Wraith Essence".into(), 3)],
        },
        EvolutionPath {
            from: "skyweaver".into(), to: "ravager".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Demon Horn".into(), 2), ("Infernal Gem".into(), 1)],
        },
        EvolutionPath {
            from: "shadow_weaver".into(), to: "titan".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Void Shard".into(), 2)],
        },
        EvolutionPath {
            from: "overseer".into(), to: "swarm_mother".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Neural Fragment".into(), 3)],
        },
        // Tier 3 -> Tier 4 (Matriarch, only 1 allowed)
        EvolutionPath {
            from: "titan".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("Void Orb".into(), 1)],
        },
        EvolutionPath {
            from: "swarm_mother".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("Eternal Crown".into(), 1)],
        },
        EvolutionPath {
            from: "ravager".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("King's Soul".into(), 1)],
        },
    ]
}

fn swarm_resources_for_action(action: &str) -> SwarmResources {
    match action {
        "create_document" => SwarmResources { essence: 10, minerals: 0, vespene: 0, biomass: 5, dark_matter: 0 },
        "run_workflow" => SwarmResources { essence: 20, minerals: 0, vespene: 0, biomass: 0, dark_matter: 3 },
        "ai_query" => SwarmResources { essence: 5, minerals: 0, vespene: 3, biomass: 0, dark_matter: 0 },
        "create_spreadsheet" => SwarmResources { essence: 15, minerals: 8, vespene: 0, biomass: 0, dark_matter: 0 },
        "send_email" => SwarmResources { essence: 5, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
        "social_post" => SwarmResources { essence: 8, minerals: 0, vespene: 0, biomass: 2, dark_matter: 0 },
        "team_contribution" => SwarmResources { essence: 12, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
        "create_note" => SwarmResources { essence: 8, minerals: 0, vespene: 0, biomass: 3, dark_matter: 0 },
        "create_slide" => SwarmResources { essence: 10, minerals: 2, vespene: 0, biomass: 0, dark_matter: 0 },
        "import_file" => SwarmResources { essence: 5, minerals: 3, vespene: 0, biomass: 0, dark_matter: 0 },
        "complete_quest" => SwarmResources { essence: 30, minerals: 5, vespene: 5, biomass: 5, dark_matter: 2 },
        _ => SwarmResources { essence: 2, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
    }
}

// ---------------------------------------------------------------------------
// OGame Shop items (Dark Matter — NO microtransactions, earned in-game only!)
// ---------------------------------------------------------------------------

fn all_shop_items() -> Vec<ShopItem> {
    vec![
        ShopItem {
            id: "boost_production_25".into(),
            name: "+25% Production (24h)".into(),
            description: "All resource production increased by 25% for 24 hours.".into(),
            cost_dark_matter: 50,
            effect: ShopEffect::ProductionBoost(0.25),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_research_30".into(),
            name: "-30% Research Time (24h)".into(),
            description: "All research completes 30% faster for 24 hours.".into(),
            cost_dark_matter: 75,
            effect: ShopEffect::ResearchSpeed(0.30),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_build_30".into(),
            name: "-30% Build Time (24h)".into(),
            description: "All building upgrades complete 30% faster for 24 hours.".into(),
            cost_dark_matter: 75,
            effect: ShopEffect::BuildSpeed(0.30),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_fleet_50".into(),
            name: "+50% Fleet Speed (12h)".into(),
            description: "Fleet travels 50% faster for 12 hours.".into(),
            cost_dark_matter: 40,
            effect: ShopEffect::FleetSpeed(0.50),
            duration_hours: Some(12),
        },
        ShopItem {
            id: "extra_queue_perm".into(),
            name: "Extra Build Queue (Permanent)".into(),
            description: "Build two things at once, forever. A must-have upgrade.".into(),
            cost_dark_matter: 500,
            effect: ShopEffect::ExtraQueue,
            duration_hours: None,
        },
        ShopItem {
            id: "boost_creep_50".into(),
            name: "+50% Creep Spread (48h)".into(),
            description: "Creep spreads 50% faster for 48 hours.".into(),
            cost_dark_matter: 60,
            effect: ShopEffect::CreepBoost(0.50),
            duration_hours: Some(48),
        },
        ShopItem {
            id: "boost_production_50".into(),
            name: "+50% Production (8h)".into(),
            description: "All resource production increased by 50% for 8 hours. Short but powerful.".into(),
            cost_dark_matter: 30,
            effect: ShopEffect::ProductionBoost(0.50),
            duration_hours: Some(8),
        },
    ]
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
// Swarm Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn quest_get_swarm(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmState, ImpForgeError> {
    engine.get_swarm()
}

#[tauri::command]
pub async fn quest_spawn_larva(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmUnit, ImpForgeError> {
    engine.spawn_larva()
}

#[tauri::command]
pub async fn quest_evolve_unit(
    unit_id: String,
    target_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmUnit, ImpForgeError> {
    engine.evolve_unit(&unit_id, &target_type)
}

#[tauri::command]
pub async fn quest_upgrade_building(
    building_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Building, ImpForgeError> {
    engine.upgrade_building(&building_type)
}

#[tauri::command]
pub async fn quest_assign_mission(
    mission_id: String,
    unit_ids: Vec<String>,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmMission, ImpForgeError> {
    engine.assign_mission(&mission_id, unit_ids)
}

#[tauri::command]
pub async fn quest_collect_mission(
    mission_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<MissionReward, ImpForgeError> {
    engine.collect_mission(&mission_id)
}

#[tauri::command]
pub async fn quest_get_missions(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<SwarmMission>, ImpForgeError> {
    engine.get_missions()
}

#[tauri::command]
pub async fn quest_swarm_auto_assign(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<SwarmMission>, ImpForgeError> {
    engine.swarm_auto_assign()
}

// ---------------------------------------------------------------------------
// SwarmForge OGame-style Tauri Commands (12 new)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_get_planet(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Planet, ImpForgeError> {
    engine.get_planet()
}

#[tauri::command]
pub async fn swarm_upgrade_building(
    building_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<PlanetBuilding, ImpForgeError> {
    engine.upgrade_planet_building(&building_type)
}

#[tauri::command]
pub async fn swarm_collect_resources(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<PlanetResources, ImpForgeError> {
    engine.collect_planet_resources()
}

#[tauri::command]
pub async fn swarm_start_research(
    tech: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Research, ImpForgeError> {
    engine.start_research(&tech)
}

#[tauri::command]
pub async fn swarm_build_ships(
    ship_type: String,
    count: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Ship, ImpForgeError> {
    engine.build_ships(&ship_type, count)
}

#[tauri::command]
pub async fn swarm_get_fleet(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Ship>, ImpForgeError> {
    let planet = engine.get_planet()?;
    Ok(planet.fleet)
}

#[tauri::command]
pub async fn swarm_get_research(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Research>, ImpForgeError> {
    let planet = engine.get_planet()?;
    Ok(planet.research)
}

#[tauri::command]
pub async fn swarm_get_creep(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<CreepStatus, ImpForgeError> {
    engine.get_creep()
}

#[tauri::command]
pub async fn swarm_shop_items() -> Result<Vec<ShopItem>, ImpForgeError> {
    Ok(all_shop_items())
}

#[tauri::command]
pub async fn swarm_shop_buy(
    item_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<ShopItem, ImpForgeError> {
    engine.buy_shop_item(&item_id)
}

#[tauri::command]
pub async fn swarm_get_galaxy(
    galaxy: u32,
    system: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<PlanetSlot>, ImpForgeError> {
    engine.get_galaxy(galaxy, system)
}

#[tauri::command]
pub async fn swarm_check_timers(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<CompletedTimer>, ImpForgeError> {
    engine.check_timers()
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
        let c = engine.create_character("TestHero", "warrior").expect("create");
        assert_eq!(c.name, "TestHero");
        assert_eq!(c.class, CharacterClass::Warrior);
        assert_eq!(c.level, 1);
        assert_eq!(c.hp, 120);
        assert!(c.attack > c.magic);
    }

    #[test]
    fn test_duplicate_character_rejected() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "mage").expect("first");
        assert!(engine.create_character("Hero2", "warrior").is_err());
    }

    #[test]
    fn test_track_action_grants_xp() {
        let (engine, _dir) = test_engine();
        engine.create_character("Hero", "warrior").expect("create");
        let result = engine.track_action("create_document").expect("track");
        assert_eq!(result.xp_earned, 37); // 25 * 1.5
        assert_eq!(result.gold_earned, 15); // 10 * 1.5
    }

    #[test]
    fn test_zones_are_valid() {
        let zones = all_zones();
        assert_eq!(zones.len(), 10);
        for zone in &zones {
            assert!(!zone.monsters.is_empty(), "Zone {} has no monsters", zone.name);
        }
    }

    #[test]
    fn test_recipes_are_valid() {
        let recipes = all_recipes();
        assert!(recipes.len() >= 8);
        for r in &recipes {
            assert!(!r.materials.is_empty(), "Recipe {} has no materials", r.name);
        }
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
    fn test_swarm_initial_state() {
        let (engine, _dir) = test_engine();
        let swarm = engine.get_swarm().expect("get_swarm");
        assert_eq!(swarm.units.len(), 0);
        assert_eq!(swarm.buildings.len(), 8);
        assert_eq!(swarm.resources.essence, 100);
    }

    #[test]
    fn test_spawn_larva() {
        let (engine, _dir) = test_engine();
        let unit = engine.spawn_larva().expect("spawn");
        assert_eq!(unit.unit_type, UnitType::ForgeDrone);
        assert_eq!(unit.level, 1);
        let swarm = engine.get_swarm().expect("swarm");
        assert_eq!(swarm.units.len(), 1);
        assert_eq!(swarm.resources.essence, 75);
    }

    #[test]
    fn test_upgrade_swarm_building() {
        let (engine, _dir) = test_engine();
        let bldg = engine.upgrade_building("nest").expect("upgrade");
        assert_eq!(bldg.level, 1);
        assert_eq!(bldg.building_type, BuildingType::Nest);
    }

    #[test]
    fn test_get_missions() {
        let (engine, _dir) = test_engine();
        let missions = engine.get_missions().expect("missions");
        assert_eq!(missions.len(), 10);
        assert!(missions.iter().all(|m| m.status == MissionStatus::Available));
    }

    #[test]
    fn test_evolution_paths_valid() {
        let paths = all_evolution_paths();
        assert!(paths.len() >= 12);
        for p in &paths {
            assert!(p.essence_cost > 0);
            assert!(p.level_requirement > 0);
        }
    }

    #[test]
    fn test_earn_swarm_resources() {
        let (engine, _dir) = test_engine();
        let earned = engine.earn_swarm_resources("create_document").expect("earn");
        assert_eq!(earned.essence, 10);
        assert_eq!(earned.biomass, 5);
        let swarm = engine.get_swarm().expect("swarm");
        assert_eq!(swarm.resources.essence, 110);
    }

    // ── OGame Colony Tests ────────────────────────────────────────────────

    #[test]
    fn test_ogame_production_rate() {
        let r0 = ForgeQuestEngine::ogame_production_rate(0, 30.0);
        assert_eq!(r0, 0.0);
        let r1 = ForgeQuestEngine::ogame_production_rate(1, 30.0);
        assert!((r1 - 33.0).abs() < 0.1);
        let r5 = ForgeQuestEngine::ogame_production_rate(5, 30.0);
        assert!(r5 > 200.0);
    }

    #[test]
    fn test_ogame_upgrade_cost() {
        let c0 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 0, 1.5);
        assert_eq!(c0, 60.0);
        let c1 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 1, 1.5);
        assert_eq!(c1, 90.0);
        let c5 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 5, 1.5);
        assert!(c5 > 400.0);
    }

    #[test]
    fn test_planet_initial_state() {
        let (engine, _dir) = test_engine();
        let planet = engine.get_planet().expect("planet");
        assert_eq!(planet.name, "Hive Prime");
        assert_eq!(planet.buildings.len(), 12);
        assert_eq!(planet.research.len(), 10);
        assert_eq!(planet.fleet.len(), 8);
        assert!(planet.resources.biomass >= 500.0);
        assert!(planet.creep.coverage_percent >= 0.0);
    }

    #[test]
    fn test_planet_building_types() {
        let types = [
            PlanetBuildingType::BiomassConverter,
            PlanetBuildingType::MineralDrill,
            PlanetBuildingType::CrystalSynthesizer,
            PlanetBuildingType::SporeExtractor,
            PlanetBuildingType::EnergyNest,
            PlanetBuildingType::CreepGenerator,
            PlanetBuildingType::BroodNest,
            PlanetBuildingType::EvolutionLab,
            PlanetBuildingType::Blighthaven,
            PlanetBuildingType::SporeDefense,
            PlanetBuildingType::BiomassStorage,
            PlanetBuildingType::MineralSilo,
        ];
        for bt in &types {
            assert_eq!(&PlanetBuildingType::from_str(bt.as_str()), bt);
            assert!(!bt.display_name().is_empty());
            assert!(!bt.description().is_empty());
        }
    }

    #[test]
    fn test_tech_types() {
        let types = [
            TechType::Genetics, TechType::ArmorPlating,
            TechType::WeaponSystems, TechType::PropulsionDrive,
            TechType::SwarmIntelligence, TechType::Regeneration,
            TechType::MutationTech, TechType::CreepBiology,
            TechType::SpaceFaring, TechType::DarkMatterResearch,
        ];
        for tt in &types {
            assert_eq!(&TechType::from_str(tt.as_str()), tt);
            assert!(tt.required_lab_level() >= 1);
        }
    }

    #[test]
    fn test_ship_types() {
        let types = [
            ShipType::BioFighter, ShipType::SporeInterceptor,
            ShipType::KrakenFrigate, ShipType::Leviathan,
            ShipType::BioTransporter, ShipType::ColonyPod,
            ShipType::Devourer, ShipType::WorldEater,
        ];
        for st in &types {
            assert_eq!(&ShipType::from_str(st.as_str()), st);
            let (atk, _, hp) = st.combat_stats();
            assert!(atk > 0);
            assert!(hp > 0);
            assert!(st.required_shipyard_level() >= 1);
        }
    }

    #[test]
    fn test_shop_items() {
        let items = all_shop_items();
        assert!(items.len() >= 7);
        for item in &items {
            assert!(item.cost_dark_matter > 0);
            assert!(!item.name.is_empty());
        }
    }

    #[test]
    fn test_galaxy_view() {
        let (engine, _dir) = test_engine();
        let slots = engine.get_galaxy(1, 1).expect("galaxy");
        assert_eq!(slots.len(), 15);
        let player_slot = slots.iter().find(|s| s.position == 4).expect("player slot");
        assert!(player_slot.occupied);
        assert_eq!(player_slot.planet_name.as_deref(), Some("Hive Prime"));
    }

    #[test]
    fn test_collect_resources() {
        let (engine, _dir) = test_engine();
        let resources = engine.collect_planet_resources().expect("collect");
        assert!(resources.biomass >= 500.0);
    }

    #[test]
    fn test_check_timers_empty() {
        let (engine, _dir) = test_engine();
        let completed = engine.check_timers().expect("timers");
        assert!(completed.is_empty());
    }

    #[test]
    fn test_creep_status() {
        let (engine, _dir) = test_engine();
        let creep = engine.get_creep().expect("creep");
        assert_eq!(creep.coverage_percent, 0.0);
        assert_eq!(creep.spread_rate_per_hour, 0.0); // No generator built
    }

    #[test]
    fn test_storage_capacity() {
        let cap0 = ForgeQuestEngine::storage_capacity(0);
        assert!(cap0 >= 5000.0);
        let cap5 = ForgeQuestEngine::storage_capacity(5);
        assert!(cap5 > cap0);
    }

    #[test]
    fn test_unit_type_roundtrip() {
        let types = [
            UnitType::ForgeDrone, UnitType::ImpScout, UnitType::Viper,
            UnitType::ShadowWeaver, UnitType::Skyweaver, UnitType::Overseer,
            UnitType::Titan, UnitType::SwarmMother, UnitType::Ravager,
            UnitType::Matriarch,
        ];
        for ut in &types {
            assert_eq!(&UnitType::from_str(ut.as_str()), ut);
        }
    }

    #[test]
    fn test_building_type_roundtrip() {
        let types = [
            BuildingType::Nest, BuildingType::EvolutionChamber,
            BuildingType::EssencePool, BuildingType::NeuralWeb,
            BuildingType::Armory, BuildingType::Sanctuary,
            BuildingType::Arcanum, BuildingType::WarCouncil,
        ];
        for bt in &types {
            assert_eq!(&BuildingType::from_str(bt.as_str()), bt);
        }
    }
}