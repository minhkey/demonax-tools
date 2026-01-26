//! Data models for Demonax game data.

use serde::{Deserialize, Serialize};

/// Player skills parsed from .usr file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSkills {
    pub id: i32,
    pub name: String,
    pub level: i32,
    pub experience: i64,
    pub magic_level: i32,
    pub fist_fighting: i32,
    pub club_fighting: i32,
    pub sword_fighting: i32,
    pub axe_fighting: i32,
    pub distance_fighting: i32,
    pub shielding: i32,
    pub fishing: i32,
}

/// Quest completion entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestCompletion {
    pub quest_id: i32,
    pub completion_count: i32,
}

/// Bestiary kill entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestiaryEntry {
    pub monster_id: i32,
    pub kill_count: i32,
}

/// Harvesting entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestingEntry {
    pub race_id: i32,
    pub harvest_count: i32,
}

/// Parsed data from a .usr file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUsrFile {
    pub player_id: i32,
    pub skills: PlayerSkills,
    pub quest_values: Vec<QuestCompletion>,
    pub bestiary: Vec<BestiaryEntry>,
    pub harvesting: Vec<HarvestingEntry>,
    pub equipment: Vec<i32>, // 10 slots, NA represented as -1?
    pub source_file: String,
}

/// Database model for players table
#[derive(Debug, Clone)]
pub struct Player {
    pub id: i32,
    pub name: String,
    pub first_seen: String, // DATE format YYYY-MM-DD
    pub last_seen: String,
}

/// Database model for daily_snapshots table
#[derive(Debug, Clone)]
pub struct DailySnapshot {
    pub id: i32,
    pub player_id: i32,
    pub snapshot_date: String,
    pub level: i32,
    pub experience: i64,
    pub magic_level: i32,
    pub fist_fighting: i32,
    pub club_fighting: i32,
    pub sword_fighting: i32,
    pub axe_fighting: i32,
    pub distance_fighting: i32,
    pub shielding: i32,
    pub fishing: i32,
    pub equipment_json: String,
    pub source_file: String,
    pub processed_timestamp: String,
}

/// Database model for daily_quests table
#[derive(Debug, Clone)]
pub struct DailyQuest {
    pub id: i32,
    pub snapshot_id: i32,
    pub quest_id: i32,
    pub completion_count: i32,
}

/// Database model for daily_bestiary table
#[derive(Debug, Clone)]
pub struct Bestiary {
    pub id: i32,
    pub snapshot_id: i32,
    pub monster_id: i32,
    pub kill_count: i32,
}

/// Database model for daily_harvesting table
#[derive(Debug, Clone)]
pub struct Harvesting {
    pub id: i32,
    pub snapshot_id: i32,
    pub race_id: i32,
    pub harvest_count: i32,
}

/// Creature data parsed from .mon file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creature {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub race: i32,
    pub hp: i32,
    pub experience: i32,
    pub creature_type: String, // 'Regular' or 'Boss'
    pub image_name: String,
    pub has_loot: bool,
    pub article: String,
    pub html_name: String,
}

/// Creature loot entry parsed from .mon file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureLoot {
    pub creature_id: i32,
    pub item_id: i32,
    pub min_amount: i32,
    pub max_amount: i32,
    pub chance_raw: i32,
    pub chance_percent: f64,
    pub average_value: f64,
}

/// Database model for creatures table (matches migration)
#[derive(Debug, Clone)]
pub struct CreatureDb {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub race: i32,
    pub hp: i32,
    pub experience: i32,
    pub creature_type: String,
    pub image_name: String,
    pub has_loot: bool,
    pub article: String,
}

/// Database model for creature_loot table
#[derive(Debug, Clone)]
pub struct CreatureLootDb {
    pub id: i32,
    pub creature_id: i32,
    pub item_id: i32,
    pub min_amount: i32,
    pub max_amount: i32,
    pub chance_raw: i32,
    pub chance_percent: f64,
    pub average_value: f64,
}

/// Creature flags parsed from .mon file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureFlags {
    pub creature_id: i32,
    pub flags: Vec<String>,
}

/// Creature skill entry parsed from .mon file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureSkill {
    pub creature_id: i32,
    pub skill_name: String,
    pub skill_value: i32,
}

/// Spell Shape Types (matches tibia-game SpellShapeType enum)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SpellShapeType {
    Actor = 0,        // Affects caster only
    Victim = 1,       // Single-target with range
    Origin = 2,       // Area around caster
    Destination = 3,  // Area at target location
    Angle = 4,        // Cone-shaped
}

/// Spell Impact Types (matches tibia-game SpellImpactType enum)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SpellImpactType {
    Damage = 0,
    Field = 1,
    Healing = 2,
    Speed = 3,
    Drunken = 4,
    Strength = 5,
    Outfit = 6,
    Summon = 7,
}

/// Damage type constants (bitmask values)
pub mod damage_types {
    pub const PHYSICAL: i32 = 1;
    pub const POISON: i32 = 2;
    pub const FIRE: i32 = 4;
    pub const ENERGY: i32 = 8;
    pub const POISON_PERIODIC: i32 = 32;
    pub const FIRE_PERIODIC: i32 = 64;
    pub const ENERGY_PERIODIC: i32 = 128;
    pub const LIFE_DRAIN: i32 = 256;
    pub const MANA_DRAIN: i32 = 512;
}

/// Helper function to decode damage type to human-readable name
pub fn damage_type_name(damage_type: i32) -> String {
    match damage_type {
        1 => "Physical".to_string(),
        2 => "Poison".to_string(),
        4 => "Fire".to_string(),
        8 => "Energy".to_string(),
        32 => "Poison (DoT)".to_string(),
        64 => "Fire (DoT)".to_string(),
        128 => "Energy (DoT)".to_string(),
        256 => "Life Drain".to_string(),
        512 => "Mana Drain".to_string(),
        _ => format!("Unknown({})", damage_type),
    }
}

/// Creature spell with human-readable interpretation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureSpell {
    pub creature_id: i32,
    pub spell_order: i32,

    // Human-readable fields
    pub spell_name: String,
    pub spell_category: String,

    // Shape details
    pub shape_type: SpellShapeType,
    pub shape_name: String,
    pub range: Option<i32>,
    pub area_size: Option<String>,
    pub angle: Option<i32>,

    // Impact details
    pub impact_type: SpellImpactType,
    pub impact_name: String,

    // Damage/Healing specific
    pub damage_type: Option<String>,
    pub base_value: Option<i32>,
    pub variation: Option<i32>,
    pub min_value: Option<i32>,
    pub max_value: Option<i32>,

    // Speed specific
    pub speed_modifier: Option<i32>,
    pub duration: Option<i32>,

    // Summon specific
    pub summon_race_id: Option<i32>,
    pub summon_count: Option<i32>,

    // Misc
    pub priority: i32,
    pub effect_id: Option<i32>,
    pub missile_effect_id: Option<i32>,

    // Raw data (for debugging/verification)
    pub raw_shape_params: String,
    pub raw_impact_params: String,
}

/// Item data parsed from objects.srv
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub type_id: i32,
    pub name: String,
    pub flags: String,          // Comma-separated flags
    pub attributes: String,     // JSON with MinimumLevel, Weight, etc.
    pub description: Option<String>,
}

/// Item loot source (aggregated from creature_loot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLootSource {
    pub item_id: i32,
    pub creature_name: String,
    pub drop_chance: f64,       // Already calculated percentage
}

/// Item price from .npc files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPrice {
    pub item_id: i32,
    pub npc_name: String,
    pub price: i32,
    pub mode: String,           // "buy" or "sell"
}

/// Quest chest data parsed from map files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestChest {
    pub quest_value: i32,
    pub key_number: Option<i32>,
    pub item_ids: Vec<i32>,     // Items in the chest
    pub sector_name: String,    // e.g., "100-200-7"
    pub sector_x: i32,
    pub sector_y: i32,
    pub sector_level: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub ingame_x: i32,
    pub ingame_y: i32,
    pub ingame_coords: String,  // "X,Y,Z" format
}

/// Spell data parsed from magic.cc
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spell {
    pub spell_id: i32,
    pub name: String,
    pub words: String,          // Spell words (e.g., "exura vita")
    pub level: i32,
    pub magic_level: Option<i32>, // For rune spells
    pub mana: i32,
    pub soul_points: i32,
    pub flags: i32,
    pub is_rune: bool,
    pub rune_type_id: Option<i32>,
    pub charges: Option<i32>,   // Rune charges
    pub spell_type: String,     // "healing", "attack", "support", etc.
    pub premium: bool,          // Derived from flags
}

/// Spell teaching data from .npc files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellTeacher {
    pub npc_name: String,
    pub spell_id: i32,
    pub vocation: String,       // "Knight", "Paladin", "Druid", "Sorcerer"
    pub teaching_price: i32,
    pub level_required: Option<i32>,  // Populated during database insertion from spells table
}

/// Rune/wand/rod seller data from .npc files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneSeller {
    pub npc_name: String,
    pub item_id: i32,                  // Type= field from NPC file
    pub spell_id: Option<i32>,         // Linked spell for runes, NULL for wands/rods
    pub vocation: Option<String>,      // None = all vocations
    pub price: i32,
    pub charges: Option<i32>,          // Data= field (for runes)
    pub account_type: Option<String>,  // "Free", "Premium", or None
    pub item_category: String,         // "rune", "wand", "rod"
}

/// Harvesting data entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestingData {
    pub tool_id: i32,
    pub corpse_id: i32,
    pub next_corpse_id: i32,
    pub percent_chance: i32,
    pub reward_id: i32,
    pub race_id: i32,
}

/// Raid data parsed from .evt files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Raid {
    pub name: String,
    pub raid_type: String,      // e.g., "single", "cyclic"
    pub waves: String,           // "one", "two", "three", etc.
    pub interval_seconds: Option<f64>,
    pub interval_days: Option<f64>,
    pub message: String,         // Aggregated messages
    pub creatures: String,       // "5 to 10 Dragon, 2 Demon, ..."
    pub spawn_composition_json: String, // JSON with detailed spawn data
}

/// Player snapshot data for equipment rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub player_id: i32,
    pub player_name: String,
    pub snapshot_date: String,
    pub equipment: Vec<i32>,  // Deserialized from equipment_json
}