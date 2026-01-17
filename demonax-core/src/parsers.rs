//! File parsers for Demonax game files.

use crate::error::{DemonaxError, Result};
use crate::file_utils::{read_latin1_file, read_utf8_file};
use crate::models::{
    BestiaryEntry, Creature, CreatureLoot, CreatureSpell, HarvestingEntry, Item, ItemPrice, ParsedUsrFile, PlayerSkills,
    QuestChest, QuestCompletion, Raid, RuneSeller, Spell, SpellImpactType, SpellShapeType, SpellTeacher,
    damage_type_name,
};
use regex::{Regex, escape};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

/// Parse a .usr file and extract player data.
pub fn parse_usr_file(file_path: &Path) -> Result<ParsedUsrFile> {
    let text = read_latin1_file(file_path)?;

    // Helper function to extract value for a key
    fn extract_value(text: &str, key: &str) -> Option<String> {
        let pattern = format!(r"{}\s*=\s*([^\n]*)", regex::escape(key));
        let re = Regex::new(&pattern).ok()?;
        let captures = re.captures(text)?;
        let value = captures.get(1)?.as_str().trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn get_int(text: &str, key: &str) -> Option<i32> {
        extract_value(text, key).and_then(|v| v.parse().ok())
    }

    fn get_string(text: &str, key: &str) -> Option<String> {
        extract_value(text, key).map(|v| v.trim_matches('"').to_string())
    }

    let player_id = get_int(&text, "ID").ok_or_else(|| {
        DemonaxError::Parse(format!("Missing ID field in {:?}", file_path))
    })?;
    let player_name = get_string(&text, "Name").ok_or_else(|| {
        DemonaxError::Parse(format!("Missing Name field in {:?}", file_path))
    })?;

    // Initialize skills with defaults (use -1 for unknown?)
    let mut skills = PlayerSkills {
        id: player_id,
        name: player_name,
        level: -1,
        experience: -1,
        magic_level: -1,
        fist_fighting: -1,
        club_fighting: -1,
        sword_fighting: -1,
        axe_fighting: -1,
        distance_fighting: -1,
        shielding: -1,
        fishing: -1,
    };

    // Parse skill lines
    let skill_re = Regex::new(r"Skill\s*=\s*\([^)]+\)").unwrap();
    for skill_line in skill_re.find_iter(&text) {
        let line = skill_line.as_str();
        // Extract content inside parentheses
        let content_re = Regex::new(r"\((.*?)\)").unwrap();
        if let Some(caps) = content_re.captures(line) {
            let content = caps.get(1).unwrap().as_str();
            let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();
            if parts.len() < 15 {
                continue;
            }
            let skill_id: i32 = parts[0].parse().unwrap_or(-1);
            let skill_value: i32 = parts[1].parse().unwrap_or(-1);
            match skill_id {
                0 => {
                    skills.level = skill_value;
                    skills.experience = parts[11].parse().unwrap_or(-1); // index 12 in R is 11 zero-based
                }
                1 => skills.magic_level = skill_value,
                6 => skills.fist_fighting = skill_value,
                7 => skills.club_fighting = skill_value,
                8 => skills.sword_fighting = skill_value,
                9 => skills.axe_fighting = skill_value,
                10 => skills.distance_fighting = skill_value,
                11 => skills.shielding = skill_value,
                14 => skills.fishing = skill_value,
                _ => {}
            }
        }
    }

    // Parse pair list (QuestValues, Bestiary, Harvesting)
    fn parse_pair_list(text: &str, key: &str) -> Vec<(i32, i32)> {
        let mut result = Vec::new();
        let val = extract_value(text, key);
        if let Some(val) = val {
            // Extract content between braces
            let brace_re = Regex::new(r"\{(.*?)\}").unwrap();
            if let Some(caps) = brace_re.captures(&val) {
                let content = caps.get(1).unwrap().as_str().trim();
                if content.is_empty() {
                    return result;
                }
                // Split by "),("
                let pairs: Vec<&str> = content.split("),(").collect();
                for pair in pairs {
                    let pair = pair.trim_matches(|c| c == '(' || c == ')');
                    let parts: Vec<&str> = pair.split(',').map(|s| s.trim()).collect();
                    if parts.len() == 2 {
                        if let (Ok(id), Ok(count)) = (parts[0].parse(), parts[1].parse()) {
                            result.push((id, count));
                        }
                    }
                }
            }
        }
        result
    }

    let quest_values: Vec<QuestCompletion> = parse_pair_list(&text, "QuestValues")
        .into_iter()
        .map(|(quest_id, completion_count)| QuestCompletion {
            quest_id,
            completion_count,
        })
        .collect();

    let bestiary: Vec<BestiaryEntry> = parse_pair_list(&text, "Bestiary")
        .into_iter()
        .map(|(monster_id, kill_count)| BestiaryEntry {
            monster_id,
            kill_count,
        })
        .collect();

    let harvesting: Vec<HarvestingEntry> = parse_pair_list(&text, "Harvesting")
        .into_iter()
        .map(|(race_id, harvest_count)| HarvestingEntry {
            race_id,
            harvest_count,
        })
        .collect();

    // Parse equipment
    let equipment = parse_equipment(&text);

    Ok(ParsedUsrFile {
        skills,
        quest_values,
        bestiary,
        harvesting,
        equipment,
        source_file: file_path.to_string_lossy().to_string(),
    })
}

/// Parse equipment from Inventory section.
fn parse_equipment(text: &str) -> Vec<i32> {
    let mut equipment = vec![-1; 10]; // 10 slots, -1 for NA

    // Find Inventory = {
    let inv_re = Regex::new(r"Inventory\s*=\s*\{").unwrap();
    let Some(inv_match) = inv_re.find(text) else {
        return equipment;
    };

    // Find matching closing brace
    let start_pos = inv_match.end();
    let mut brace_count = 1;
    let chars: Vec<char> = text[start_pos..].chars().collect();
    let mut i = 0;
    while brace_count > 0 && i < chars.len() {
        match chars[i] {
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            _ => {}
        }
        i += 1;
    }
    if brace_count != 0 {
        return equipment;
    }
    let end_pos = start_pos + i;
    let inv_content = &text[inv_match.start()..end_pos];

    // For each slot 1..=10
    for slot in 1..=10 {
        let pattern = format!(r"(?<=[\s{{,]){}\s+Content\s*=\s*{{\s*(\d+)", slot);
        let re = Regex::new(&pattern).ok();
        if let Some(re) = re {
            if let Some(caps) = re.captures(inv_content) {
                if let Some(value) = caps.get(1) {
                    equipment[slot - 1] = value.as_str().parse().unwrap_or(-1);
                }
            }
        }
    }

    equipment
}

/// Calculate loot drop percentage from raw chance value.
/// Formula: (chance + 1) / 999 * 100
/// If percentage < 1, round to 1 decimal place, else round to integer.
pub fn get_loot_percent(chance_raw: i32) -> f64 {
    let pct = (chance_raw as f64 + 1.0) / 999.0 * 100.0;
    if pct < 1.0 {
        (pct * 10.0).round() / 10.0
    } else {
        pct.round()
    }
}

/// Parse a .mon file and extract creature overview data.
/// Returns a Creature struct with basic stats.
pub fn parse_mon_file(file_path: &Path) -> Result<Creature> {
    let text = read_latin1_file(file_path)?;

    // Helper functions similar to .usr parsing
    fn extract_value(text: &str, key: &str) -> Option<String> {
        let pattern = format!(r"{}\s*=\s*([^\n]*)", escape(key));
        let re = Regex::new(&pattern).ok()?;
        let captures = re.captures(text)?;
        let value = captures.get(1)?.as_str().trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn get_int(text: &str, key: &str) -> Option<i32> {
        extract_value(text, key).and_then(|v| v.parse().ok())
    }

    fn get_string(text: &str, key: &str) -> Option<String> {
        extract_value(text, key).map(|v| v.trim_matches('"').to_string())
    }

    // Special handling for HitPoints which is inside Skills = {(<something>, <hp>)
    fn get_hitpoints(text: &str) -> Option<i32> {
        let re = Regex::new(r"Skills\s*=\s*\{\s*\([^,]+,\s*([0-9]+)").ok()?;
        let caps = re.captures(text)?;
        caps.get(1)?.as_str().parse().ok()
    }

    let name = get_string(&text, "Name").ok_or_else(|| {
        DemonaxError::Parse(format!("Missing Name field in {:?}", file_path))
    })?;
    let article = get_string(&text, "Article").unwrap_or_default();
    let race = get_int(&text, "RaceNumber").unwrap_or(0);
    let hp = get_hitpoints(&text).unwrap_or(0);
    let experience = get_int(&text, "Experience").unwrap_or(0);

    // Determine creature type based on article (as per R code)
    let creature_type = if article == "A" || article == "An" {
        "Regular".to_string()
    } else {
        "Boss".to_string()
    };

    let short_name = name.replace(" ", "").to_lowercase();
    let file_stem = file_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Image name mapping (from R code)
    let img_map = vec![
        ("demodras", "dragonlord"),
        ("dharalion", "elfarcanist"),
        ("loraith", "yalaharipriest"),
        ("leon", "heroguardian"),
        ("grorlam", "stonegolem"),
        ("necropharus", "necromancer"),
        ("beholder", "bonelord"),
        ("theoldwidow", "giantspider"),
    ];
    let mut image_name = short_name.clone();
    for (from, to) in img_map {
        if short_name == from {
            image_name = to.to_string();
            break;
        }
    }

    let has_loot = text.contains("Inventory");

    // TODO: Need to generate ID - perhaps hash of short_name or something.
    // For now use a placeholder.
    let id = 0; // Will be assigned when inserted into database

    Ok(Creature {
        id,
        name,
        short_name,
        race,
        hp,
        experience,
        creature_type,
        image_name,
        has_loot,
        article,
        html_name: file_stem,
    })
}

/// Parse creature loot from .mon file.
/// Returns a vector of CreatureLoot entries.
/// Note: This function requires item data to compute average values.
pub fn parse_creature_loot(file_path: &Path) -> Result<Vec<CreatureLoot>> {
    let text = read_latin1_file(file_path)?;

    // First, get creature ID (placeholder)
    let creature_id = 0;

    // Find Inventory section
    let inv_re = Regex::new(r"Inventory\s*=\s*\{").unwrap();
    let Some(inv_match) = inv_re.find(&text) else {
        return Ok(Vec::new()); // No loot
    };

    // Extract content between braces (simplified)
    let start = inv_match.end();
    let mut brace_count = 1;
    let chars: Vec<char> = text[start..].chars().collect();
    let mut i = 0;
    while brace_count > 0 && i < chars.len() {
        match chars[i] {
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            _ => {}
        }
        i += 1;
    }
    if brace_count != 0 {
        return Ok(Vec::new());
    }
    let end = start + i;
    let inv_content = &text[inv_match.start()..end];

    // Remove "Inventory = {" prefix and trailing "}"
    let content = inv_content
        .strip_prefix("Inventory = {")
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("")
        .trim();

    // Split by "),(" or ")," ?
    // In R code they do: gsub("\\(", "\n", inv) then parse as CSV.
    // Let's replicate: replace '(' with newline, remove ')', split lines.
    let mut processed = content.to_string();
    processed = processed.replace('(', "\n");
    processed = processed.replace(')', "");
    processed = processed.replace(", ", ","); // normalize spaces
    let lines: Vec<&str> = processed.split('\n').filter(|l| !l.trim().is_empty()).collect();

    let mut loot = Vec::new();
    for line in lines {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 3 {
            continue;
        }
        let item_id: i32 = parts[0].parse().unwrap_or(0);
        let amount: i32 = parts[1].parse().unwrap_or(0);
        let chance_raw: i32 = parts[2].parse().unwrap_or(0);

        // In .mon files, amount is max amount, min is 1? Actually R code treats it as max_amount.
        let min_amount = 1;
        let max_amount = amount;
        let chance_percent = get_loot_percent(chance_raw);

        loot.push(CreatureLoot {
            creature_id,
            item_id,
            min_amount,
            max_amount,
            chance_raw,
            chance_percent,
            average_value: 0.0, // Will be calculated later with item data
        });
    }

    Ok(loot)
}

/// Parse creature flags from .mon file.
/// Returns a vector of flag names.
pub fn parse_creature_flags(text: &str) -> Result<Vec<String>> {
    let flags_re = Regex::new(r"Flags\s*=\s*\{([^}]+)\}").unwrap();

    if let Some(caps) = flags_re.captures(text) {
        let content = caps.get(1).unwrap().as_str();
        let flags: Vec<String> = content
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(flags)
    } else {
        Ok(Vec::new())
    }
}

/// Parse creature skills from .mon file.
/// Returns a vector of (skill_name, skill_value) tuples.
pub fn parse_creature_skills(text: &str) -> Result<Vec<(String, i32)>> {
    let skills_section_re = Regex::new(r"Skills\s*=\s*\{([^}]+)\}").unwrap();

    if let Some(caps) = skills_section_re.captures(text) {
        let content = caps.get(1).unwrap().as_str();
        let mut skills = Vec::new();

        // Match pattern: (SkillName, value, ...)
        let skill_re = Regex::new(r"\(([^,]+),\s*([0-9]+)").unwrap();

        for skill_match in skill_re.captures_iter(content) {
            let skill_name = skill_match.get(1).unwrap().as_str().trim().to_string();
            if let Ok(skill_value) = skill_match.get(2).unwrap().as_str().parse::<i32>() {
                skills.push((skill_name, skill_value));
            }
        }

        Ok(skills)
    } else {
        Ok(Vec::new())
    }
}

/// Parse spell shape type from string
fn parse_spell_shape_type(shape_str: &str) -> Option<SpellShapeType> {
    match shape_str.trim() {
        "Actor" => Some(SpellShapeType::Actor),
        "Victim" => Some(SpellShapeType::Victim),
        "Origin" => Some(SpellShapeType::Origin),
        "Destination" => Some(SpellShapeType::Destination),
        "Angle" => Some(SpellShapeType::Angle),
        _ => None,
    }
}

/// Parse spell impact type from string
fn parse_spell_impact_type(impact_str: &str) -> Option<SpellImpactType> {
    match impact_str.trim() {
        "Damage" => Some(SpellImpactType::Damage),
        "Field" => Some(SpellImpactType::Field),
        "Healing" => Some(SpellImpactType::Healing),
        "Speed" => Some(SpellImpactType::Speed),
        "Drunken" => Some(SpellImpactType::Drunken),
        "Strength" => Some(SpellImpactType::Strength),
        "Outfit" => Some(SpellImpactType::Outfit),
        "Summon" => Some(SpellImpactType::Summon),
        _ => None,
    }
}

/// Interpret spell and generate human-readable fields
fn interpret_spell(
    shape_type: SpellShapeType,
    _shape_params: &[i32],
    impact_type: SpellImpactType,
    impact_params: &[i32],
) -> (String, String, String, String) {

    let shape_name = match shape_type {
        SpellShapeType::Actor => "Self",
        SpellShapeType::Victim => "Single Target",
        SpellShapeType::Origin => "Area (Self)",
        SpellShapeType::Destination => "Area (Target)",
        SpellShapeType::Angle => "Cone",
    }.to_string();

    let (impact_name, spell_category) = match impact_type {
        SpellImpactType::Damage => {
            let dmg_type = impact_params.get(0).copied().unwrap_or(1);
            let dmg_name = damage_type_name(dmg_type);
            (format!("{} Damage", dmg_name), "Attack".to_string())
        },
        SpellImpactType::Healing => ("Healing".to_string(), "Heal".to_string()),
        SpellImpactType::Speed => {
            let modifier = impact_params.get(0).copied().unwrap_or(0);
            if modifier > 0 {
                ("Speed Boost".to_string(), "Buff".to_string())
            } else {
                ("Paralyze".to_string(), "Debuff".to_string())
            }
        },
        SpellImpactType::Summon => ("Summon".to_string(), "Summon".to_string()),
        SpellImpactType::Field => ("Field".to_string(), "Attack".to_string()),
        SpellImpactType::Drunken => ("Drunken".to_string(), "Debuff".to_string()),
        SpellImpactType::Strength => ("Strength Boost".to_string(), "Buff".to_string()),
        SpellImpactType::Outfit => ("Transform".to_string(), "Buff".to_string()),
    };

    // Generate spell name based on shape + impact combination
    let spell_name = match (shape_type, impact_type) {
        // Damage spells
        (SpellShapeType::Victim, SpellImpactType::Damage) => {
            let dmg_type = impact_params.get(0).copied().unwrap_or(1);
            match dmg_type {
                1 => "Strike",
                2 => "Poison Strike",
                4 => "Fire Strike",
                8 => "Energy Strike",
                256 => "Life Drain",
                512 => "Mana Drain",
                _ => "Magic Strike",
            }.to_string()
        },
        (SpellShapeType::Destination, SpellImpactType::Damage) => {
            let dmg_type = impact_params.get(0).copied().unwrap_or(1);
            match dmg_type {
                4 => "Fireball",
                8 => "Energy Ball",
                2 => "Poison Bomb",
                _ => "Explosion",
            }.to_string()
        },
        (SpellShapeType::Angle, SpellImpactType::Damage) => {
            let dmg_type = impact_params.get(0).copied().unwrap_or(1);
            match dmg_type {
                4 => "Fire Wave",
                8 => "Energy Wave",
                2 => "Poison Wave",
                1 => "Physical Wave",
                _ => "Magic Wave",
            }.to_string()
        },
        (SpellShapeType::Origin, SpellImpactType::Damage) => {
            let dmg_type = impact_params.get(0).copied().unwrap_or(1);
            match dmg_type {
                4 => "Fire Burst",
                8 => "Energy Burst",
                2 => "Poison Burst",
                32 => "Poison Field (DoT)",
                64 => "Fire Field (DoT)",
                128 => "Energy Field (DoT)",
                _ => "Explosion",
            }.to_string()
        },

        // Healing spells
        (SpellShapeType::Actor, SpellImpactType::Healing) => "Self Heal".to_string(),
        (SpellShapeType::Victim, SpellImpactType::Healing) => "Heal Other".to_string(),
        (SpellShapeType::Origin, SpellImpactType::Healing) => "Area Heal".to_string(),

        // Speed spells
        (SpellShapeType::Actor, SpellImpactType::Speed) => {
            let modifier = impact_params.get(0).copied().unwrap_or(0);
            if modifier > 0 { "Haste" } else { "Self Slow" }.to_string()
        },
        (SpellShapeType::Victim, SpellImpactType::Speed) => {
            let modifier = impact_params.get(0).copied().unwrap_or(0);
            if modifier > 0 { "Speed Boost" } else { "Paralyze" }.to_string()
        },
        (SpellShapeType::Origin, SpellImpactType::Speed) => {
            let modifier = impact_params.get(0).copied().unwrap_or(0);
            if modifier > 0 { "Mass Haste" } else { "Mass Paralyze" }.to_string()
        },

        // Summon spells
        (_, SpellImpactType::Summon) => "Summon Creature".to_string(),

        // Field spells
        (SpellShapeType::Destination, SpellImpactType::Field) => "Magic Field".to_string(),

        // Other combinations
        _ => format!("{} {}", shape_name, impact_name),
    };

    (spell_name, spell_category, shape_name, impact_name)
}

/// Extract detailed spell parameters based on shape and impact types
#[allow(clippy::type_complexity)]
fn extract_spell_details(
    shape_type: SpellShapeType,
    shape_params: &[i32],
    impact_type: SpellImpactType,
    impact_params: &[i32],
) -> (Option<i32>, Option<String>, Option<i32>, Option<String>, Option<i32>, Option<i32>,
      Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>) {

    // Extract range (varies by shape type)
    let range = match shape_type {
        SpellShapeType::Victim => shape_params.get(0).copied(),
        SpellShapeType::Destination => shape_params.get(0).copied(),
        SpellShapeType::Angle => shape_params.get(1).copied(),
        _ => None,
    };

    // Extract area size
    let area_size = match shape_type {
        SpellShapeType::Origin => {
            let radius = shape_params.get(0).copied().unwrap_or(0);
            Some(format!("Radius {}", radius))
        },
        SpellShapeType::Destination => {
            let param1 = shape_params.get(1).copied().unwrap_or(0);
            let param2 = shape_params.get(2).copied().unwrap_or(0);
            Some(format!("{}x{}", param1, param2))
        },
        _ => None,
    };

    // Extract angle (for cone spells)
    let angle = match shape_type {
        SpellShapeType::Angle => shape_params.get(0).copied(),
        _ => None,
    };

    // Extract visual effect IDs
    let (effect_id, missile_effect_id) = match shape_type {
        SpellShapeType::Actor => (shape_params.get(0).copied(), None),
        SpellShapeType::Victim => (
            shape_params.get(2).copied(),  // Hit effect
            shape_params.get(1).copied()   // Missile sprite ID
        ),
        SpellShapeType::Origin => (shape_params.get(1).copied(), None),
        SpellShapeType::Destination => (shape_params.get(3).copied(), None),
        SpellShapeType::Angle => (shape_params.get(2).copied(), None),
    };

    // Extract impact-specific details
    let (damage_type, base_value, variation, min_value, max_value, speed_modifier,
         duration, summon_race_id, summon_count) = match impact_type {

        SpellImpactType::Damage => {
            let dmg_type_id = impact_params.get(0).copied().unwrap_or(1);
            let dmg_type = Some(damage_type_name(dmg_type_id));
            let base = impact_params.get(1).copied();
            let var = impact_params.get(2).copied();
            let min = base.and_then(|b| var.map(|v| b - v));
            let max = base.and_then(|b| var.map(|v| b + v));
            (dmg_type, base, var, min, max, None, None, None, None)
        },

        SpellImpactType::Healing => {
            let base = impact_params.get(0).copied();
            let var = impact_params.get(1).copied();
            let min = base.and_then(|b| var.map(|v| b - v));
            let max = base.and_then(|b| var.map(|v| b + v));
            (None, base, var, min, max, None, None, None, None)
        },

        SpellImpactType::Speed => {
            let modifier = impact_params.get(0).copied();
            let dur = impact_params.get(1).copied();
            (None, None, None, None, None, modifier, dur, None, None)
        },

        SpellImpactType::Summon => {
            let race_id = impact_params.get(0).copied();
            let count = impact_params.get(1).copied();
            (None, None, None, None, None, None, None, race_id, count)
        },

        _ => (None, None, None, None, None, None, None, None, None),
    };

    (range, area_size, angle, damage_type, base_value, variation, min_value, max_value,
     speed_modifier, duration, summon_race_id, summon_count, effect_id, missile_effect_id)
}

/// Parse creature spells from .mon file.
/// Returns a vector of CreatureSpell with human-readable interpretation.
pub fn parse_creature_spells(text: &str) -> Result<Vec<CreatureSpell>> {
    let spells_section_re = Regex::new(r"Spells\s*=\s*\{([^}]+)\}").unwrap();

    if let Some(caps) = spells_section_re.captures(text) {
        let content = caps.get(1).unwrap().as_str();
        let spell_re = Regex::new(r"(\w+)\s*\(([^)]*)\)\s*->\s*(\w+)\s*\(([^)]*)\)\s*:\s*(\d+)").unwrap();
        let mut spells = Vec::new();
        let mut spell_order = 0;

        for spell_match in spell_re.captures_iter(content) {
            let shape_name = spell_match.get(1).unwrap().as_str();
            let shape_params_str = spell_match.get(2).unwrap().as_str();
            let impact_name = spell_match.get(3).unwrap().as_str();
            let impact_params_str = spell_match.get(4).unwrap().as_str();

            let priority: i32 = if let Ok(p) = spell_match.get(5).unwrap().as_str().parse::<i32>() {
                p
            } else {
                continue;
            };

            let shape_type = match parse_spell_shape_type(shape_name) {
                Some(st) => st,
                None => continue,
            };

            let shape_params: Vec<i32> = shape_params_str
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();

            let impact_type = match parse_spell_impact_type(impact_name) {
                Some(it) => it,
                None => continue,
            };

            let impact_params: Vec<i32> = impact_params_str
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();

            // Interpret spell to get human-readable names
            let (spell_name, spell_category, shape_name_readable, impact_name_readable) =
                interpret_spell(shape_type, &shape_params, impact_type, &impact_params);

            // Extract detailed parameters
            let (range, area_size, angle, damage_type, base_value, variation, min_value, max_value,
                 speed_modifier, duration, summon_race_id, summon_count, effect_id, missile_effect_id) =
                extract_spell_details(shape_type, &shape_params, impact_type, &impact_params);

            let raw_shape_params = serde_json::to_string(&shape_params).unwrap_or_else(|_| "[]".to_string());
            let raw_impact_params = serde_json::to_string(&impact_params).unwrap_or_else(|_| "[]".to_string());

            spells.push(CreatureSpell {
                creature_id: 0,
                spell_order,
                spell_name,
                spell_category,
                shape_type,
                shape_name: shape_name_readable,
                range,
                area_size,
                angle,
                impact_type,
                impact_name: impact_name_readable,
                damage_type,
                base_value,
                variation,
                min_value,
                max_value,
                speed_modifier,
                duration,
                summon_race_id,
                summon_count,
                priority,
                effect_id,
                missile_effect_id,
                raw_shape_params,
                raw_impact_params,
            });
            spell_order += 1;
        }

        Ok(spells)
    } else {
        Ok(Vec::new())
    }
}

/// Parse objects.srv file and extract item metadata.
/// Only includes items with "Take" flag, excludes type IDs 1-10.
pub fn parse_objects_srv(file_path: &Path) -> Result<Vec<Item>> {
    let text = read_utf8_file(file_path)?;

    let mut items = Vec::new();

    // Split by double newlines to get individual object records
    let records: Vec<&str> = text.split("\n\n").filter(|s| !s.trim().is_empty()).collect();

    for record in records {
        // Skip comment lines and empty lines
        if record.trim().starts_with('#') {
            continue;
        }

        let lines: Vec<&str> = record.lines().collect();
        if lines.is_empty() {
            continue;
        }

        // Extract fields
        let mut type_id: Option<i32> = None;
        let mut name: Option<String> = None;
        let mut flags: Vec<String> = Vec::new();
        let mut attributes: HashMap<String, String> = HashMap::new();
        let mut description: Option<String> = None;

        for line in lines {
            let line = line.trim();

            // Skip comments
            if line.starts_with('#') {
                continue;
            }

            // Parse key-value pairs
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "TypeID" => {
                        type_id = value.parse().ok();
                    }
                    "Name" => {
                        // Remove quotes and title-case
                        let cleaned = value.trim_matches('"').trim();
                        name = Some(title_case_item_name(cleaned));
                    }
                    "Flags" => {
                        // Split by comma
                        flags = value.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    "Attributes" => {
                        // Parse attributes like "MinimumLevel=50, Weight=10"
                        for attr in value.split(',') {
                            if let Some((attr_key, attr_val)) = attr.split_once('=') {
                                attributes.insert(
                                    attr_key.trim().to_string(),
                                    attr_val.trim().to_string()
                                );
                            }
                        }
                    }
                    "Description" => {
                        description = Some(value.trim_matches('"').to_string());
                    }
                    _ => {
                        // Check if this is an attribute (key=value without "Attributes =" prefix)
                        // In some formats, attributes might be listed individually
                        if let Some(attr_val) = value.strip_prefix("=").or(Some(value)) {
                            attributes.insert(key.to_string(), attr_val.to_string());
                        }
                    }
                }
            }
        }

        // Only include items with:
        // 1. Valid type_id
        // 2. "Take" flag present
        // 3. type_id not in 1-10 (reserved)
        if let (Some(tid), Some(nm)) = (type_id, name) {
            // Filter out reserved IDs
            if tid <= 10 {
                continue;
            }

            // Check for "Take" flag
            if !flags.iter().any(|f| f.eq_ignore_ascii_case("Take")) {
                continue;
            }

            // Convert attributes to JSON string
            let attributes_json = serde_json::to_string(&attributes)
                .unwrap_or_else(|_| "{}".to_string());

            items.push(Item {
                type_id: tid,
                name: nm,
                flags: flags.join(", "),
                attributes: attributes_json,
                description,
            });
        }
    }

    Ok(items)
}

/// Title-case item name and strip leading articles ("a", "an")
fn title_case_item_name(name: &str) -> String {
    let mut words: Vec<String> = name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect();

    // Remove leading "A" or "An" if present
    if !words.is_empty() {
        if words[0].eq_ignore_ascii_case("a") || words[0].eq_ignore_ascii_case("an") {
            words.remove(0);
        }
    }

    words.join(" ")
}

/// Parse .npc file and extract item prices
pub fn parse_npc_file(file_path: &Path) -> Result<Vec<ItemPrice>> {
    let text = read_latin1_file(file_path)?;

    let mut prices = Vec::new();

    // Extract NPC name
    let name_re = Regex::new(r#"Name\s*=\s*"([^"]+)""#)
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    let npc_name = name_re.captures(&text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| {
            file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    // Find all lines with Type= and Price=
    let type_price_re = Regex::new(r"Type\s*=\s*(\d+).*?Price\s*=\s*(\d+)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    for line in text.lines() {
        if let Some(caps) = type_price_re.captures(line) {
            let type_id: i32 = caps.get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let price: i32 = caps.get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);

            // Determine mode: "sell" means NPC is selling (player buying)
            // No "sell" means NPC is buying (player selling)
            let mode = if line.to_lowercase().contains("sell") {
                "sell".to_string()
            } else {
                "buy".to_string()
            };

            prices.push(ItemPrice {
                item_id: type_id,
                npc_name: npc_name.clone(),
                price,
                mode,
            });
        }
    }

    Ok(prices)
}

/// Parse map sector file and extract quest chest data
///
/// Map files are .sec files with coordinates in filename (e.g., "100-200-7.sec")
/// Lines containing "ChestQuestNumber" define quest chests with their contents
pub fn parse_map_sector_file(file_path: &Path) -> Result<Vec<QuestChest>> {
    let text = read_latin1_file(file_path)?;

    // Extract sector info from filename (e.g., "100-200-7.sec")
    let sector_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| DemonaxError::Parse("Invalid sector filename".to_string()))?;

    let parts: Vec<&str> = sector_name.split('-').collect();
    if parts.len() != 3 {
        return Ok(Vec::new()); // Not a valid sector file
    }

    let sector_x: i32 = parts[0].parse().unwrap_or(0) * 32;
    let sector_y: i32 = parts[1].parse().unwrap_or(0) * 32;
    let sector_level: i32 = parts[2].parse().unwrap_or(0);

    // Find all lines with ChestQuestNumber
    let chest_lines: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("ChestQuestNumber"))
        .collect();

    if chest_lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut chests = Vec::new();

    // Regex patterns
    let coord_re = Regex::new(r"^(\d+)-(\d+):")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let quest_value_re = Regex::new(r"ChestQuestNumber\s*=\s*(\d+)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let key_number_re = Regex::new(r"KeyNumber\s*=\s*(\d+)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let content_re = Regex::new(r"Content\s*=\s*\{([^}]+)\}")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    for line in chest_lines {
        // Extract offset coordinates
        let (offset_x, offset_y) = if let Some(caps) = coord_re.captures(line) {
            let x = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let y = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            (x, y)
        } else {
            continue; // Skip malformed lines
        };

        // Extract quest value
        let quest_value = if let Some(caps) = quest_value_re.captures(line) {
            caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0)
        } else {
            continue;
        };

        // Extract key number (optional)
        let key_number = key_number_re
            .captures(line)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse().ok());

        // Extract item IDs from Content={...}
        let item_ids = if let Some(caps) = content_re.captures(line) {
            caps.get(1)
                .map(|m| {
                    m.as_str()
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Calculate in-game coordinates
        let ingame_x = sector_x + offset_x;
        let ingame_y = sector_y + offset_y;
        let ingame_coords = format!("{},{},{}", ingame_x, ingame_y, sector_level);

        chests.push(QuestChest {
            quest_value,
            key_number,
            item_ids,
            sector_name: sector_name.to_string(),
            sector_x,
            sector_y,
            sector_level,
            offset_x,
            offset_y,
            ingame_x,
            ingame_y,
            ingame_coords,
        });
    }

    Ok(chests)
}

/// Parse magic.cc C++ source file to extract spell definitions
///
/// Looks for CreateSpell() calls and extracts spell metadata including
/// all properties (Mana, Level, RuneGr, RuneNr, Flags, etc.)
pub fn parse_magic_cc(file_path: &Path) -> Result<Vec<Spell>> {
    let text = read_utf8_file(file_path)?;

    let mut spells = Vec::new();

    let init_spells_start = text.find("static void InitSpells")
        .ok_or_else(|| DemonaxError::Parse("InitSpells function not found".to_string()))?;

    let init_spells_text = &text[init_spells_start..];
    let lines: Vec<&str> = init_spells_text.lines().collect();

    let create_spell_re = Regex::new(r"Spell\s*=\s*CreateSpell\((\d+),\s*(.+?)\);")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let property_re = Regex::new(r"^\s*Spell->(\w+)\s*=\s*(.+?);")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let words_re = Regex::new(r#""([^"]+)""#)
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        if let Some(caps) = create_spell_re.captures(line) {
            let spell_id: i32 = caps.get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);

            let params = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let words: Vec<String> = words_re
                .captures_iter(params)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            let spell_words = words.join(" ");

            let mut spell = Spell {
                spell_id,
                name: format!("Spell {}", spell_id),
                words: spell_words.clone(),
                level: 0,
                magic_level: None,
                mana: 0,
                soul_points: 0,
                flags: 0,
                is_rune: false,
                rune_type_id: None,
                charges: None,
                spell_type: String::new(),
                premium: false,
            };

            let mut rune_gr: Option<i32> = None;
            let mut rune_nr: Option<i32> = None;

            i += 1;
            while i < lines.len() {
                let prop_line = lines[i].trim();

                if prop_line.contains("CreateSpell") ||
                   (prop_line.is_empty() && i + 1 < lines.len() && lines[i + 1].trim().is_empty()) {
                    break;
                }

                if let Some(prop_caps) = property_re.captures(prop_line) {
                    let property_name = prop_caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let value_str = prop_caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");

                    match property_name {
                        "Mana" => {
                            spell.mana = value_str.parse().unwrap_or(0);
                        }
                        "Level" => {
                            spell.level = value_str.parse().unwrap_or(0);
                        }
                        "RuneGr" => {
                            rune_gr = value_str.parse().ok();
                        }
                        "RuneNr" => {
                            rune_nr = value_str.parse().ok();
                        }
                        "Flags" => {
                            spell.flags = value_str.parse().unwrap_or(0);
                        }
                        "Amount" => {
                            spell.charges = value_str.parse().ok();
                        }
                        "RuneLevel" => {
                            spell.magic_level = value_str.parse().ok();
                        }
                        "SoulPoints" => {
                            spell.soul_points = value_str.parse().unwrap_or(0);
                        }
                        "Comment" => {
                            if let Some(comment_caps) = words_re.captures(value_str) {
                                spell.name = comment_caps.get(1)
                                    .map(|m| m.as_str().to_string())
                                    .unwrap_or_else(|| format!("Spell {}", spell_id));
                            }
                        }
                        _ => {}
                    }
                }

                i += 1;
            }

            if let (Some(gr), Some(nr)) = (rune_gr, rune_nr) {
                if gr != 0 {
                    spell.is_rune = true;
                    spell.rune_type_id = Some(calculate_rune_type_id(gr, nr));
                }
            }

            spell.premium = is_premium(spell.flags);
            spell.spell_type = classify_spell_by_flags(spell.flags, &spell.words, &spell.name);

            spells.push(spell);
        } else {
            i += 1;
        }
    }

    Ok(spells)
}

/// Calculate rune type ID from RuneGr and RuneNr
/// Formula: 3147 + RuneNr (based on magic.cc comments)
fn calculate_rune_type_id(_rune_gr: i32, rune_nr: i32) -> i32 {
    3147 + rune_nr
}

/// Check if spell requires premium account based on flags
fn is_premium(flags: i32) -> bool {
    (flags & 0x02) != 0
}

/// Classify spell type based on flags and spell words
fn classify_spell_by_flags(flags: i32, words: &str, _name: &str) -> String {
    // Flag 0x01: Aggressive spells (check first, takes priority)
    if flags & 0x01 != 0 {
        if words.contains("mas") || words.contains("grav") {
            return "area".to_string();
        }
        return "attack".to_string();
    }

    // Flag 0x08: Healing spells (minimum 100% multiplier)
    // Only classify as healing if not aggressive
    if flags & 0x08 != 0 || words.contains("ura") {
        return "healing".to_string();
    }

    // Summon spells
    if words.contains("evo res") {
        return "summon".to_string();
    }

    // Support spells (haste, etc.)
    if words.contains("hur") && !words.contains("mort") {
        return "support".to_string();
    }

    // Utility spells
    if words.contains("lux") || words.contains("evo") {
        return "utility".to_string();
    }

    "other".to_string()
}

/// Parse .npc files to extract spell teaching data
///
/// Looks for lines containing "buy the spell" or "learn the spell"
/// with Type=, Price=, and vocation information
pub fn parse_npc_spell_teaching(file_path: &Path) -> Result<Vec<SpellTeacher>> {
    let text = read_latin1_file(file_path)?;

    let mut teachers = Vec::new();

    // Extract NPC name
    let name_re = Regex::new(r#"Name\s*=\s*"([^"]+)""#)
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    let npc_name = name_re.captures(&text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Find spell teaching lines
    let spell_lines: Vec<&str> = text
        .lines()
        .filter(|line| {
            (line.contains("buy the spell") || line.contains("learn the spell")) &&
            line.contains("Type=") &&
            line.contains("Price=")
        })
        .collect();

    for line in spell_lines {
        // Extract Type and Price
        let type_re = Regex::new(r"Type\s*=\s*(\d+)")
            .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
        let price_re = Regex::new(r"Price\s*=\s*(\d+)")
            .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

        let spell_id = type_re.captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        let teaching_price = price_re.captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        // Determine vocation(s)
        let vocations = extract_vocations_from_line(line);

        for vocation in vocations {
            teachers.push(SpellTeacher {
                npc_name: npc_name.clone(),
                spell_id,
                vocation,
                teaching_price,
            });
        }
    }

    Ok(teachers)
}

/// Extract vocations from a spell teaching line
fn extract_vocations_from_line(line: &str) -> Vec<String> {
    let mut vocations = Vec::new();

    // Check for explicit vocation patterns
    if line.contains("Knight,") || line.to_lowercase().contains("knight,") {
        vocations.push("Knight".to_string());
    }
    if line.contains("Paladin,") || line.to_lowercase().contains("paladin,") {
        vocations.push("Paladin".to_string());
    }
    if line.contains("Druid,") || line.to_lowercase().contains("druid,") {
        vocations.push("Druid".to_string());
    }
    if line.contains("Sorcerer,") || line.to_lowercase().contains("sorcerer,") {
        vocations.push("Sorcerer".to_string());
    }

    // If no specific vocation found, assume all vocations
    if vocations.is_empty() {
        vocations = vec![
            "Knight".to_string(),
            "Paladin".to_string(),
            "Druid".to_string(),
            "Sorcerer".to_string(),
        ];
    }

    vocations
}

/// Parse .npc files to extract rune/wand/rod seller data
///
/// Looks for lines containing Type=, Price= with "rune", "wand", or "rod"
/// keywords, extracting vocation restrictions from line prefixes or descriptions
pub fn parse_npc_rune_selling(file_path: &Path) -> Result<Vec<RuneSeller>> {
    let text = read_latin1_file(file_path)?;
    let mut sellers = Vec::new();

    // Extract NPC name (same pattern as spell teaching)
    let name_re = Regex::new(r#"Name\s*=\s*"([^"]+)""#)
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    let npc_name = name_re.captures(&text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| {
            file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    // Account type from filename
    let account_type = if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
        if name.contains("-free-") { Some("Free".to_string()) }
        else if name.contains("-prem-") || name.contains("-max-") { Some("Premium".to_string()) }
        else { None }
    } else { None };

    // Find selling lines: contains (rune OR wand OR rod) AND Type= AND Price=
    // Skip bulk purchase lines (contain %1)
    let selling_lines: Vec<&str> = text.lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            (lower.contains("rune") || lower.contains("wand") || lower.contains("rod")) &&
            line.contains("Type=") && line.contains("Price=") && !line.contains("%1")
        })
        .collect();

    for line in selling_lines {
        // Extract Type, Price, Data using regex
        let type_re = Regex::new(r"Type\s*=\s*(\d+)")
            .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
        let price_re = Regex::new(r"Price\s*=\s*(\d+)")
            .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
        let data_re = Regex::new(r"Data\s*=\s*(\d+)")
            .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

        let item_id = match type_re.captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok()) {
                Some(id) => id,
                None => continue,
            };

        let price = match price_re.captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok()) {
                Some(p) => p,
                None => continue,
            };

        let charges = data_re.captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok());

        // Item category
        let lower = line.to_lowercase();
        let item_category = if lower.contains("wand") { "wand" }
            else if lower.contains("rod") { "rod" }
            else { "rune" }.to_string();

        // Extract vocations (same pattern as spell teaching)
        let vocations = extract_rune_vocations(line);

        for vocation in vocations {
            sellers.push(RuneSeller {
                npc_name: npc_name.clone(),
                item_id,
                spell_id: None,  // Set during DB insertion
                vocation,
                price,
                charges,
                account_type: account_type.clone(),
                item_category: item_category.clone(),
            });
        }
    }

    Ok(sellers)
}

/// Extract vocation restrictions for rune/wand/rod purchases
///
/// Returns Vec<Option<String>> where:
/// - Some("Vocation") = restricted to that vocation
/// - None = available to all vocations
fn extract_rune_vocations(line: &str) -> Vec<Option<String>> {
    let lower = line.to_lowercase();

    // Check line prefix (e.g., 'sorcerer,"wand"...')
    if lower.starts_with("sorcerer,") { return vec![Some("Sorcerer".to_string())]; }
    if lower.starts_with("druid,") { return vec![Some("Druid".to_string())]; }
    if lower.starts_with("knight,") { return vec![Some("Knight".to_string())]; }
    if lower.starts_with("paladin,") { return vec![Some("Paladin".to_string())]; }

    // Check description text
    if lower.contains("only for sorcerer") { return vec![Some("Sorcerer".to_string())]; }
    if lower.contains("only for druid") { return vec![Some("Druid".to_string())]; }
    if lower.contains("only for knight") { return vec![Some("Knight".to_string())]; }
    if lower.contains("only for paladin") { return vec![Some("Paladin".to_string())]; }

    // No restriction = available to all vocations
    vec![None]
}

/// Parse .evt raid file
///
/// Extracts raid information including type, interval, messages, and creature spawns
pub fn parse_evt_file(file_path: &Path) -> Result<Raid> {
    let text = read_latin1_file(file_path)?;

    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Extract Type
    let type_re = Regex::new(r"(?m)^Type\s*=\s*(.+)$")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let raid_type = type_re
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Extract Interval
    let interval_re = Regex::new(r"(?m)^Interval\s*=\s*(\d+)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let interval_seconds = interval_re
        .captures(&text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok());

    let interval_days = interval_seconds.map(|sec| sec / 60.0 / 60.0 / 24.0);

    // Extract waves from "# Process:" comment
    let process_re = Regex::new(r"(?mi)^#\s*Process:.*")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let waves = if let Some(process_match) = process_re.find(&text) {
        let process_text = process_match.as_str().to_lowercase();

        if process_text.contains("one") {
            "one"
        } else if process_text.contains("two") {
            "two"
        } else if process_text.contains("three") {
            "three"
        } else if process_text.contains("four") {
            "four"
        } else if process_text.contains("five") {
            "five"
        } else if process_text.contains("six") {
            "six"
        } else if process_text.contains("seven") {
            "seven"
        } else if process_text.contains("eight") {
            "eight"
        } else if process_text.contains("nine") {
            "nine"
        } else if process_text.contains("ten") {
            "ten"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    }.to_string();

    // Extract Messages
    let message_re = Regex::new(r#"(?m)^Message\s*=\s*"?([^"\n]+)"?"#)
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let messages: Vec<String> = message_re
        .captures_iter(&text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .collect();
    let message = messages.join("; ");

    // Extract Race and Count for spawns
    let race_re = Regex::new(r"(?m)^Race\s*=\s*(\d+)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;
    let count_re = Regex::new(r"(?m)^Count\s*=\s*\((\d+),\s*(\d+)\)")
        .map_err(|e| DemonaxError::Parse(format!("Regex error: {}", e)))?;

    let races: Vec<i32> = race_re
        .captures_iter(&text)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse().ok()))
        .collect();

    let counts: Vec<(i32, i32)> = count_re
        .captures_iter(&text)
        .filter_map(|c| {
            let min = c.get(1).and_then(|m| m.as_str().parse().ok())?;
            let max = c.get(2).and_then(|m| m.as_str().parse().ok())?;
            Some((min, max))
        })
        .collect();

    // Build spawn composition
    let mut spawn_map: HashMap<i32, (i32, i32)> = HashMap::new();
    for (race, (min, max)) in races.iter().zip(counts.iter()) {
        let entry = spawn_map.entry(*race).or_insert((0, 0));
        entry.0 += min;
        entry.1 += max;
    }

    // Convert to JSON
    let spawn_vec: Vec<serde_json::Value> = spawn_map
        .iter()
        .map(|(race, (min, max))| {
            json!({
                "race": race,
                "min": min,
                "max": max
            })
        })
        .collect();

    let spawn_composition_json = serde_json::to_string(&spawn_vec)?;

    // Create creatures string (simplified - will be enriched with creature names from DB)
    let creatures = if spawn_map.is_empty() {
        "Unknown".to_string()
    } else {
        spawn_map
            .iter()
            .map(|(race, (min, max))| {
                if min == max {
                    format!("{} Race {}", min, race)
                } else {
                    format!("{} to {} Race {}", min, max, race)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    Ok(Raid {
        name,
        raid_type,
        waves,
        interval_seconds,
        interval_days,
        message,
        creatures,
        spawn_composition_json,
    })
}