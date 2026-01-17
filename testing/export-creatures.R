#!/usr/bin/env Rscript

library(RSQLite)
library(DBI)
library(tidyverse)

# Command line arguments
args <- commandArgs(trailingOnly = TRUE)
db_path <- ifelse(length(args) >= 1, args[1], "./test.db")
output_dir <- ifelse(length(args) >= 2, args[2], ".")

# Connect to database
con <- dbConnect(RSQLite::SQLite(), db_path)

# Query creature spells with JOIN
creature_spells <- dbGetQuery(con, "
  SELECT
    c.name as creature_name,
    c.hp,
    c.experience,
    c.type,
    cs.spell_name,
    cs.spell_category,
    cs.shape_name,
    cs.range,
    cs.area_size,
    cs.angle,
    cs.impact_name,
    cs.damage_type,
    cs.base_value,
    cs.variation,
    cs.speed_modifier,
    cs.duration,
    cs.summon_race_id,
    cs.summon_count,
    cs.priority,
    cs.effect_id,
    cs.missile_effect_id
  FROM creature_spells cs
  JOIN creatures c ON cs.creature_id = c.id
  ORDER BY c.name, cs.spell_order
")

# Calculate enhanced spell metrics
# ================================
# priority: Lower = more frequent (priority 3 = 33% chance, priority 7 = 14% chance per cycle)
# base_value: Damage/healing base (actual = base ± variation from source code)
spells_enhanced <- creature_spells %>%
  mutate(
    # Cast frequency: probability spell will cast each creature "think" cycle
    cast_chance_percent = round(100 / priority, 2),

    # Damage range: base ± variation (confirmed from game source)
    min_damage = base_value - variation,
    max_damage = base_value + variation,

    # Expected damage per cycle: accounts for cast frequency
    # Higher priority (rare) spells contribute less to overall danger
    expected_damage_per_cycle = round(base_value / priority, 2),

    # Spell weight: adjusts expected damage by spell type effectiveness
    # - Direct damage (Attack) = 1.0 (full weight)
    # - DoT (Damage over Time) = 0.6 (sustained but lower immediate threat)
    # - Debuffs (Paralyze, etc) = 0.4 (indirect danger)
    # - Healing = -0.5 (reduces creature danger to player)
    # - Other (buffs, summons) = 0.3 (situational)
    spell_weight = case_when(
      str_detect(damage_type, "DoT|Periodic") ~ 0.6,
      spell_category == "Heal" ~ -0.5,
      spell_category == "Debuff" ~ 0.4,
      spell_category == "Attack" ~ 1.0,
      TRUE ~ 0.3
    )
  )

# Calculate creature danger summary
# ==================================
# DANGER SCORE CALCULATION:
#
# danger_score = total_spell_dps + hp_factor + special_bonus
#
# WHERE:
#   total_spell_dps = sum of (base_damage / priority * spell_weight) for all damage spells
#                     This represents expected spell damage output per creature cycle
#
#   hp_factor = hp / 1000
#               Represents creature survivability/tankiness
#               Higher HP = longer fight = more dangerous
#
#   special_bonus = bonus points for high-impact abilities:
#                   +10 for Paralyze (immobilizes player)
#                   +15 for Life Drain (damages AND heals creature)
#                   +20 for Summon (multiplies threat)
#                   -5 for Healing (makes fight longer but less bursty)
#
# WHAT IS INCLUDED:
#   - Spell damage with cast frequency weighting
#   - Creature HP (survivability)
#   - Special spell abilities (detected from spell data)
#
# WHAT IS NOT INCLUDED:
#   - Physical combat stats (Attack/Defend/Armor) - not currently parsed from .mon files
#   - Creature flags (KickBoxes, SeeInvisible, etc.) - not used in calculation
#   - Creature skills (GoStrength, FistFighting, etc.) - not used in calculation
#   - Strategy percentages from .mon files - not parsed
#
# NOTE: This is a SPELL-CENTRIC danger rating. Does not account for melee-only
# creatures or physical combat effectiveness. Best used for spell-casting creatures.
#
danger_summary <- spells_enhanced %>%
  group_by(creature_name, hp, experience, type) %>%
  summarise(
    spell_count = n(),
    damage_spell_count = sum(!is.na(damage_type)),

    # Total expected spell DPS: sum of weighted spell damage accounting for cast frequency
    total_spell_dps = sum(
      ifelse(!is.na(base_value),
             base_value / priority * spell_weight,
             0),
      na.rm = TRUE
    ),

    # Priority metrics (lower = more dangerous, casts more often)
    avg_priority = mean(priority, na.rm = TRUE),
    min_priority = min(priority, na.rm = TRUE),

    # Special abilities (detected from spell names/types)
    has_paralyze = any(str_detect(spell_name, "Paralyze")),
    has_life_drain = any(str_detect(damage_type, "Life Drain")),
    has_summon = any(spell_category == "Summon"),
    has_healing = any(spell_category == "Heal"),
    .groups = "drop"
  ) %>%
  mutate(
    # HP factor: creature survivability (hp / 1000)
    hp_factor = hp / 1000,

    # Special ability bonuses
    special_bonus = (has_paralyze * 10) + (has_life_drain * 15) +
                    (has_summon * 20) - (has_healing * 5),

    # Final danger score (higher = more dangerous)
    danger_score = total_spell_dps + hp_factor + special_bonus
  ) %>%
  arrange(desc(danger_score))

# Query other tables
creature_flags <- dbGetQuery(con, "
  SELECT c.name as creature_name, cf.flag_name
  FROM creature_flags cf
  JOIN creatures c ON cf.creature_id = c.id
  ORDER BY c.name, cf.flag_name
")

creature_skills <- dbGetQuery(con, "
  SELECT c.name as creature_name, cs.skill_name, cs.skill_value
  FROM creature_skills cs
  JOIN creatures c ON cs.creature_id = c.id
  ORDER BY c.name, cs.skill_name
")

# Close database connection
dbDisconnect(con)

# Export to CSV
write_csv(spells_enhanced, file.path(output_dir, "creature_spells_enhanced.csv"))
write_csv(creature_flags, file.path(output_dir, "creature_flags.csv"))
write_csv(creature_skills, file.path(output_dir, "creature_skills.csv"))
write_csv(danger_summary, file.path(output_dir, "creature_danger_summary.csv"))
