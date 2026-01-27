use crate::error::{DemonaxError, Result};
use crate::file_utils;
use crate::models::{
    Creature, CreatureLoot, CreatureSpell, ParsedUsrFile, PlayerSnapshot,
};
use crate::parsers;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, params, OptionalExtension};
use serde_json;
use std::collections::HashMap;

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path)
            .with_init(|conn| {
                conn.pragma_update(None, "foreign_keys", "ON")?;
                Ok(())
            });
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)
            .map_err(|e| DemonaxError::Pool(e))?;

        let db = Self { pool };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| DemonaxError::Pool(e))
    }

    fn run_migrations(&self) -> Result<()> {
        let mut conn = self.connection()?;

        conn.pragma_update(None, "foreign_keys", "ON")?;

        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            -- Player data schema
            CREATE TABLE IF NOT EXISTS players (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                first_seen DATE NOT NULL,
                last_seen DATE NOT NULL
            );

            CREATE TABLE IF NOT EXISTS daily_snapshots (
                id INTEGER PRIMARY KEY,
                player_id INTEGER NOT NULL,
                snapshot_date DATE NOT NULL,
                level INTEGER NOT NULL,
                experience BIGINT NOT NULL,
                magic_level INTEGER NOT NULL,
                fist_fighting INTEGER NOT NULL,
                club_fighting INTEGER NOT NULL,
                sword_fighting INTEGER NOT NULL,
                axe_fighting INTEGER NOT NULL,
                distance_fighting INTEGER NOT NULL,
                shielding INTEGER NOT NULL,
                fishing INTEGER NOT NULL,
                equipment_json TEXT NOT NULL,
                source_file TEXT NOT NULL,
                processed_timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (player_id) REFERENCES players(id) ON DELETE CASCADE,
                UNIQUE(player_id, snapshot_date)
            );

            CREATE TABLE IF NOT EXISTS daily_quests (
                id INTEGER PRIMARY KEY,
                snapshot_id INTEGER NOT NULL,
                quest_id INTEGER NOT NULL,
                completion_count INTEGER NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE,
                UNIQUE(snapshot_id, quest_id)
            );

            CREATE TABLE IF NOT EXISTS daily_bestiary (
                id INTEGER PRIMARY KEY,
                snapshot_id INTEGER NOT NULL,
                monster_id INTEGER NOT NULL,
                kill_count INTEGER NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE,
                UNIQUE(snapshot_id, monster_id)
            );

            CREATE TABLE IF NOT EXISTS daily_harvesting (
                id INTEGER PRIMARY KEY,
                snapshot_id INTEGER NOT NULL,
                race_id INTEGER NOT NULL,
                harvest_count INTEGER NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE,
                UNIQUE(snapshot_id, race_id)
            );

            -- Creature and loot schema
            CREATE TABLE IF NOT EXISTS creatures (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                short_name TEXT NOT NULL UNIQUE,
                race INTEGER NOT NULL,
                hp INTEGER NOT NULL,
                experience INTEGER NOT NULL,
                type TEXT NOT NULL,
                image_name TEXT NOT NULL,
                has_loot BOOLEAN NOT NULL DEFAULT FALSE,
                article TEXT,
                html_name TEXT,
                mon_link TEXT
            );

            CREATE TABLE IF NOT EXISTS creature_loot (
                id INTEGER PRIMARY KEY,
                creature_id INTEGER NOT NULL,
                item_id INTEGER NOT NULL,
                min_amount INTEGER NOT NULL DEFAULT 1,
                max_amount INTEGER NOT NULL DEFAULT 1,
                chance_raw INTEGER NOT NULL,
                chance_percent REAL NOT NULL,
                FOREIGN KEY (creature_id) REFERENCES creatures(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_creature_loot_creature_id ON creature_loot(creature_id);
            CREATE INDEX IF NOT EXISTS idx_creature_loot_item_id ON creature_loot(item_id);

            -- Creature flags table
            CREATE TABLE IF NOT EXISTS creature_flags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                creature_id INTEGER NOT NULL,
                flag_name TEXT NOT NULL,
                FOREIGN KEY (creature_id) REFERENCES creatures(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_creature_flags_creature_id ON creature_flags(creature_id);

            -- Creature skills table
            CREATE TABLE IF NOT EXISTS creature_skills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                creature_id INTEGER NOT NULL,
                skill_name TEXT NOT NULL,
                skill_value INTEGER NOT NULL,
                FOREIGN KEY (creature_id) REFERENCES creatures(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_creature_skills_creature_id ON creature_skills(creature_id);

            -- Creature spells table
            CREATE TABLE IF NOT EXISTS creature_spells (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                creature_id INTEGER NOT NULL,
                spell_order INTEGER NOT NULL,

                -- Human-readable fields
                spell_name TEXT NOT NULL,
                spell_category TEXT NOT NULL,

                -- Shape details
                shape_type INTEGER NOT NULL,
                shape_name TEXT NOT NULL,
                range INTEGER,
                area_size TEXT,
                angle INTEGER,

                -- Impact details
                impact_type INTEGER NOT NULL,
                impact_name TEXT NOT NULL,

                -- Damage/Healing specific
                damage_type TEXT,
                base_value INTEGER,
                variation INTEGER,
                min_value INTEGER,
                max_value INTEGER,

                -- Speed specific
                speed_modifier INTEGER,
                duration INTEGER,

                -- Summon specific
                summon_race_id INTEGER,
                summon_count INTEGER,

                -- Misc
                priority INTEGER NOT NULL,
                effect_id INTEGER,
                missile_effect_id INTEGER,

                -- Raw data (for debugging)
                raw_shape_params TEXT NOT NULL,
                raw_impact_params TEXT NOT NULL,

                FOREIGN KEY (creature_id) REFERENCES creatures(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_creature_spells_creature_id ON creature_spells(creature_id);
            CREATE INDEX IF NOT EXISTS idx_creature_spells_category ON creature_spells(spell_category);
            CREATE INDEX IF NOT EXISTS idx_creature_spells_damage_type ON creature_spells(damage_type);
            CREATE INDEX IF NOT EXISTS idx_creature_spells_missile_effect ON creature_spells(missile_effect_id);

            -- Item data schema
            CREATE TABLE IF NOT EXISTS items (
                id INTEGER PRIMARY KEY,
                type_id INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL,
                description TEXT,
                weight INTEGER,
                worth INTEGER,
                flags TEXT,
                attributes TEXT,
                image_link TEXT
            );

            CREATE TABLE IF NOT EXISTS item_loot_sources (
                id INTEGER PRIMARY KEY,
                item_id INTEGER NOT NULL,
                creature_id INTEGER NOT NULL,
                drop_chance REAL NOT NULL,
                UNIQUE(item_id, creature_id)
            );

            CREATE TABLE IF NOT EXISTS item_prices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                npc_name TEXT NOT NULL,
                price INTEGER NOT NULL,
                mode TEXT NOT NULL CHECK(mode IN ('buy', 'sell'))
            );

            CREATE INDEX IF NOT EXISTS idx_item_loot_sources_item_id ON item_loot_sources(item_id);
            CREATE INDEX IF NOT EXISTS idx_item_loot_sources_creature_id ON item_loot_sources(creature_id);
            CREATE INDEX IF NOT EXISTS idx_item_prices_item_id ON item_prices(item_id);
            CREATE INDEX IF NOT EXISTS idx_item_prices_npc_name ON item_prices(npc_name);

            -- Quest, raid, spell, harvesting data schema
            CREATE TABLE IF NOT EXISTS quests (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                chest_location TEXT,
                reward_items_json TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS raids (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                type TEXT NOT NULL,
                waves TEXT NOT NULL,
                interval_seconds REAL,
                interval_days REAL,
                message TEXT NOT NULL DEFAULT '',
                creatures TEXT NOT NULL DEFAULT '',
                spawn_composition_json TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS spells (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                words TEXT NOT NULL,
                level INTEGER NOT NULL,
                magic_level INTEGER,
                mana INTEGER NOT NULL,
                soul_points INTEGER NOT NULL DEFAULT 0,
                flags INTEGER NOT NULL DEFAULT 0,
                is_rune INTEGER NOT NULL DEFAULT 0,
                rune_type_id INTEGER,
                charges INTEGER,
                spell_type TEXT NOT NULL DEFAULT 'unknown',
                premium INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS spell_teachers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                npc_name TEXT NOT NULL,
                spell_name TEXT NOT NULL,
                spell_id INTEGER NOT NULL,
                vocation TEXT NOT NULL,
                price INTEGER NOT NULL,
                level_required INTEGER,
                UNIQUE(npc_name, spell_id, vocation)
            );

            CREATE TABLE IF NOT EXISTS harvesting_data (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_id INTEGER NOT NULL,
                corpse_id INTEGER NOT NULL,
                next_corpse_id INTEGER NOT NULL,
                percent_chance INTEGER NOT NULL,
                reward_id INTEGER NOT NULL,
                race_id INTEGER NOT NULL,
                UNIQUE(tool_id, corpse_id)
            );

            CREATE INDEX IF NOT EXISTS idx_harvesting_data_tool_id ON harvesting_data(tool_id);
            CREATE INDEX IF NOT EXISTS idx_harvesting_data_corpse_id ON harvesting_data(corpse_id);
            CREATE INDEX IF NOT EXISTS idx_spell_teachers_spell_id ON spell_teachers(spell_id);
            CREATE INDEX IF NOT EXISTS idx_spell_teachers_npc_name ON spell_teachers(npc_name);

            CREATE TABLE IF NOT EXISTS rune_sellers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                npc_name TEXT NOT NULL,
                item_id INTEGER NOT NULL,
                spell_id INTEGER,
                vocation TEXT,
                price INTEGER NOT NULL,
                charges INTEGER,
                account_type TEXT,
                item_category TEXT NOT NULL CHECK(item_category IN ('rune', 'wand', 'rod')),
                UNIQUE(npc_name, item_id, vocation)
            );

            CREATE INDEX IF NOT EXISTS idx_rune_sellers_item_id ON rune_sellers(item_id);
            CREATE INDEX IF NOT EXISTS idx_rune_sellers_spell_id ON rune_sellers(spell_id);
            CREATE INDEX IF NOT EXISTS idx_rune_sellers_npc_name ON rune_sellers(npc_name);
            "#,
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Insert or update a player record. Returns player ID.
    fn insert_or_update_player(
        &self,
        conn: &Connection,
        player_id: i32,
        player_name: &str,
        snapshot_date: &str,
    ) -> Result<i32> {
        // Check if player exists by ID
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT first_seen, last_seen FROM players WHERE id = ?",
                params![player_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match existing {
            Some((first_seen, last_seen)) => {
                // Update last_seen if snapshot_date > last_seen
                let new_last_seen = if snapshot_date > last_seen.as_str() {
                    snapshot_date
                } else {
                    last_seen.as_str()
                };

                conn.execute(
                    "INSERT OR REPLACE INTO players (id, name, first_seen, last_seen) VALUES (?, ?, ?, ?)",
                    params![player_id, player_name, first_seen, new_last_seen],
                )?;
            }
            None => {
                // Insert new player with explicit ID
                conn.execute(
                    "INSERT INTO players (id, name, first_seen, last_seen) VALUES (?, ?, ?, ?)",
                    params![player_id, player_name, snapshot_date, snapshot_date],
                )?;
            }
        }

        Ok(player_id)
    }

    /// Check if snapshot already exists for player on given date.
    fn snapshot_exists(&self, conn: &Connection, player_id: i32, snapshot_date: &str) -> Result<bool> {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM daily_snapshots WHERE player_id = ? AND snapshot_date = ?",
            params![player_id, snapshot_date],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Insert a daily snapshot and return its ID.
    fn insert_daily_snapshot(
        &self,
        conn: &Connection,
        player_id: i32,
        snapshot_date: &str,
        parsed: &ParsedUsrFile,
    ) -> Result<i32> {
        let skills = &parsed.skills;
        // Convert equipment to JSON
        let equipment_json = serde_json::to_string(&parsed.equipment)?;

        conn.execute(
            "INSERT INTO daily_snapshots (
                player_id, snapshot_date, level, experience, magic_level,
                fist_fighting, club_fighting, sword_fighting, axe_fighting,
                distance_fighting, shielding, fishing, equipment_json, source_file
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                player_id,
                snapshot_date,
                skills.level,
                skills.experience,
                skills.magic_level,
                skills.fist_fighting,
                skills.club_fighting,
                skills.sword_fighting,
                skills.axe_fighting,
                skills.distance_fighting,
                skills.shielding,
                skills.fishing,
                equipment_json,
                parsed.source_file,
            ],
        )?;
        Ok(conn.last_insert_rowid() as i32)
    }

    /// Insert daily quest completions.
    fn insert_daily_quests(&self, conn: &Connection, snapshot_id: i32, parsed: &ParsedUsrFile) -> Result<()> {
        for quest in &parsed.quest_values {
            conn.execute(
                "INSERT INTO daily_quests (snapshot_id, quest_id, completion_count) VALUES (?, ?, ?)",
                params![snapshot_id, quest.quest_id, quest.completion_count],
            )?;
        }
        Ok(())
    }

    /// Insert bestiary entries.
    fn insert_bestiary(&self, conn: &Connection, snapshot_id: i32, parsed: &ParsedUsrFile) -> Result<()> {
        for entry in &parsed.bestiary {
            conn.execute(
                "INSERT INTO daily_bestiary (snapshot_id, monster_id, kill_count) VALUES (?, ?, ?)",
                params![snapshot_id, entry.monster_id, entry.kill_count],
            )?;
        }
        Ok(())
    }

    /// Insert harvesting entries.
    fn insert_harvesting(&self, conn: &Connection, snapshot_id: i32, parsed: &ParsedUsrFile) -> Result<()> {
        for entry in &parsed.harvesting {
            conn.execute(
                "INSERT INTO daily_harvesting (snapshot_id, race_id, harvest_count) VALUES (?, ?, ?)",
                params![snapshot_id, entry.race_id, entry.harvest_count],
            )?;
        }
        Ok(())
    }

    /// Insert a player snapshot (main entry point).
    /// Returns true if inserted, false if snapshot already existed.
    pub fn insert_player_snapshot(
        &self,
        parsed: &ParsedUsrFile,
        snapshot_date: &str,
    ) -> Result<bool> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        let player_id = self.insert_or_update_player(
            &tx,
            parsed.player_id,
            &parsed.skills.name,
            snapshot_date
        )?;

        if self.snapshot_exists(&tx, player_id, snapshot_date)? {
            // Snapshot already exists, skip inserting snapshot but keep player update
            tx.commit()?;
            return Ok(false);
        }

        let snapshot_id = self.insert_daily_snapshot(&tx, player_id, snapshot_date, parsed)?;
        self.insert_daily_quests(&tx, snapshot_id, parsed)?;
        self.insert_bestiary(&tx, snapshot_id, parsed)?;
        self.insert_harvesting(&tx, snapshot_id, parsed)?;

        tx.commit()?;
        Ok(true)
    }

    /// Process .usr files from a directory.
    /// Returns number of successfully processed files.
    pub fn process_usr_files(
        &self,
        input_dir: &std::path::Path,
        snapshot_date: &str,
        quiet: u8,
    ) -> Result<u32> {
        let files = file_utils::find_files_with_extension(input_dir, "usr")?;
        if files.is_empty() {
            if quiet == 0 {
                tracing::info!("No .usr files found in {}", input_dir.display());
            }
            return Ok(0);
        }

        if quiet == 0 {
            tracing::info!("Found {} .usr files to process", files.len());
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for file_path in files {
            match parsers::parse_usr_file(&file_path) {
                Ok(parsed) => {
                    match self.insert_player_snapshot(&parsed, snapshot_date) {
                        Ok(true) => {
                            success_count += 1;
                            if quiet == 0 {
                                tracing::info!("Processed {} successfully", parsed.skills.name);
                            }
                        }
                        Ok(false) => {
                            if quiet == 0 {
                                tracing::debug!("Skipped {} - snapshot already exists", parsed.skills.name);
                            }
                        }
                        Err(e) => {
                            error_count += 1;
                            if quiet < 2 {
                                tracing::warn!("Failed to insert snapshot for {}: {}", parsed.skills.name, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    if quiet < 2 {
                        tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        if quiet == 0 {
            tracing::info!("Processed {} files successfully, {} errors", success_count, error_count);
        }

        Ok(success_count)
    }

    /// Insert or update a creature record. Returns creature ID.
    fn insert_or_update_creature(&self, conn: &Connection, creature: &Creature) -> Result<i32> {
        // Check if creature exists by short_name (unique)
        let existing: Option<i32> = conn
            .query_row(
                "SELECT id FROM creatures WHERE short_name = ?",
                params![creature.short_name],
                |row| row.get(0),
            )
            .optional()?;

        match existing {
            Some(id) => {
                // Update creature stats
                conn.execute(
                    "UPDATE creatures SET name = ?, race = ?, hp = ?, experience = ?, type = ?, image_name = ?, has_loot = ?, article = ? WHERE id = ?",
                    params![
                        creature.name,
                        creature.race,
                        creature.hp,
                        creature.experience,
                        creature.creature_type,
                        creature.image_name,
                        creature.has_loot,
                        creature.article,
                        id,
                    ],
                )?;
                Ok(id)
            }
            None => {
                // Insert new creature
                conn.execute(
                    "INSERT INTO creatures (name, short_name, race, hp, experience, type, image_name, has_loot, article) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        creature.name,
                        creature.short_name,
                        creature.race,
                        creature.hp,
                        creature.experience,
                        creature.creature_type,
                        creature.image_name,
                        creature.has_loot,
                        creature.article,
                    ],
                )?;
                Ok(conn.last_insert_rowid() as i32)
            }
        }
    }

    /// Insert creature loot entries, replacing any existing loot for this creature.
    fn insert_creature_loot(&self, conn: &Connection, creature_id: i32, loot: &[CreatureLoot]) -> Result<()> {
        // Delete existing loot for this creature
        conn.execute(
            "DELETE FROM creature_loot WHERE creature_id = ?",
            params![creature_id],
        )?;

        // Insert new loot
        for entry in loot {
            conn.execute(
                "INSERT INTO creature_loot (creature_id, item_id, min_amount, max_amount, chance_raw, chance_percent) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    creature_id,
                    entry.item_id,
                    entry.min_amount,
                    entry.max_amount,
                    entry.chance_raw,
                    entry.chance_percent,
                ],
            )?;
        }
        Ok(())
    }

    /// Insert creature flags, replacing any existing flags for this creature.
    fn insert_creature_flags(&self, conn: &Connection, creature_id: i32, flags: &[String]) -> Result<()> {
        // Delete existing flags
        conn.execute(
            "DELETE FROM creature_flags WHERE creature_id = ?",
            params![creature_id],
        )?;

        // Insert new flags
        for flag in flags {
            conn.execute(
                "INSERT INTO creature_flags (creature_id, flag_name) VALUES (?, ?)",
                params![creature_id, flag],
            )?;
        }
        Ok(())
    }

    /// Insert creature skills, replacing any existing skills for this creature.
    fn insert_creature_skills(&self, conn: &Connection, creature_id: i32, skills: &[(String, i32)]) -> Result<()> {
        // Delete existing skills
        conn.execute(
            "DELETE FROM creature_skills WHERE creature_id = ?",
            params![creature_id],
        )?;

        // Insert new skills
        for (skill_name, skill_value) in skills {
            conn.execute(
                "INSERT INTO creature_skills (creature_id, skill_name, skill_value) VALUES (?, ?, ?)",
                params![creature_id, skill_name, skill_value],
            )?;
        }
        Ok(())
    }

    /// Insert creature spells, replacing any existing spells for this creature.
    fn insert_creature_spells(&self, conn: &Connection, creature_id: i32, spells: &[CreatureSpell]) -> Result<()> {
        // Delete existing spells
        conn.execute(
            "DELETE FROM creature_spells WHERE creature_id = ?",
            params![creature_id],
        )?;

        // Insert new spells
        for spell in spells {
            conn.execute(
                "INSERT INTO creature_spells (
                    creature_id, spell_order, spell_name, spell_category,
                    shape_type, shape_name, range, area_size, angle,
                    impact_type, impact_name,
                    damage_type, base_value, variation, min_value, max_value,
                    speed_modifier, duration,
                    summon_race_id, summon_count,
                    priority, effect_id, missile_effect_id,
                    raw_shape_params, raw_impact_params
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    creature_id,
                    spell.spell_order,
                    &spell.spell_name,
                    &spell.spell_category,
                    spell.shape_type as i32,
                    &spell.shape_name,
                    spell.range,
                    spell.area_size.as_ref(),
                    spell.angle,
                    spell.impact_type as i32,
                    &spell.impact_name,
                    spell.damage_type.as_ref(),
                    spell.base_value,
                    spell.variation,
                    spell.min_value,
                    spell.max_value,
                    spell.speed_modifier,
                    spell.duration,
                    spell.summon_race_id,
                    spell.summon_count,
                    spell.priority,
                    spell.effect_id,
                    spell.missile_effect_id,
                    &spell.raw_shape_params,
                    &spell.raw_impact_params,
                ],
            )?;
        }
        Ok(())
    }

    /// Rebuild the item_loot_sources table from creature_loot data.
    /// This creates a reverse lookup to find which creatures drop each item.
    /// When a creature has multiple loot entries for the same item, we take the maximum
    /// drop chance and average value to represent the best drop rate.
    fn rebuild_item_loot_sources(&self, conn: &Connection) -> Result<usize> {
        // Clear the table
        conn.execute("DELETE FROM item_loot_sources", [])?;

        // Aggregate from creature_loot and insert into item_loot_sources
        // Use MAX to handle cases where the same creature drops the same item multiple times
        let rows_affected = conn.execute(
            "INSERT INTO item_loot_sources (item_id, creature_id, drop_chance)
             SELECT item_id, creature_id, MAX(chance_percent) as drop_chance
             FROM creature_loot
             GROUP BY item_id, creature_id
             ORDER BY item_id, creature_id",
            [],
        )?;

        Ok(rows_affected)
    }

    /// Process .mon files from a directory.
    /// Returns number of successfully processed files.
    pub fn process_mon_files(
        &self,
        game_path: &std::path::Path,
        quiet: u8,
    ) -> Result<u32> {
        let mon_dir = game_path.join("mon");
        let files = file_utils::find_files_with_extension(&mon_dir, "mon")?;

        // Exclude list from R code
        let exclude = vec![
            "deathslicer.mon",
            "slime2.mon",
            "illusion.mon",
            "butterflyblue.mon",
            "butterflyyellow.mon",
            "butterflyred.mon",
            "butterflypurple.mon",
            "mimic.mon",
            "halloweenhare.mon",
            "flamethrower.mon",
            "magicthrower.mon",
            "plaguethrower.mon",
            "shredderthrower.mon",
            "gamemaster.mon",
            "human.mon",
        ];

        let files: Vec<_> = files
            .into_iter()
            .filter(|path| {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                !exclude.contains(&filename.as_ref())
            })
            .collect();

        if files.is_empty() {
            if quiet == 0 {
                tracing::info!("No .mon files found in {}", mon_dir.display());
            }
            return Ok(0);
        }

        if quiet == 0 {
            tracing::info!("Found {} .mon files to process", files.len());
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for file_path in files {
            match parsers::parse_mon_file(&file_path) {
                Ok(creature) => {
                    let mut conn = self.connection()?;
                    let tx = conn.transaction()?;

                    match self.insert_or_update_creature(&tx, &creature) {
                        Ok(creature_id) => {
                            // Read file text for parsing flags
                            let text = match file_utils::read_latin1_file(&file_path) {
                                Ok(t) => t,
                                Err(e) => {
                                    error_count += 1;
                                    if quiet < 2 {
                                        tracing::warn!("Failed to read file {}: {}", creature.name, e);
                                    }
                                    tx.rollback()?;
                                    continue;
                                }
                            };

                            // Parse and insert flags
                            match parsers::parse_creature_flags(&text) {
                                Ok(flags) => {
                                    if !flags.is_empty() {
                                        if let Err(e) = self.insert_creature_flags(&tx, creature_id, &flags) {
                                            error_count += 1;
                                            if quiet < 2 {
                                                tracing::warn!("Failed to insert flags for {}: {}", creature.name, e);
                                            }
                                            tx.rollback()?;
                                            continue;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_count += 1;
                                    if quiet < 2 {
                                        tracing::warn!("Failed to parse flags for {}: {}", creature.name, e);
                                    }
                                    tx.rollback()?;
                                    continue;
                                }
                            }

                            // Parse and insert skills
                            match parsers::parse_creature_skills(&text) {
                                Ok(skills) => {
                                    if !skills.is_empty() {
                                        if let Err(e) = self.insert_creature_skills(&tx, creature_id, &skills) {
                                            error_count += 1;
                                            if quiet < 2 {
                                                tracing::warn!("Failed to insert skills for {}: {}", creature.name, e);
                                            }
                                            tx.rollback()?;
                                            continue;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_count += 1;
                                    if quiet < 2 {
                                        tracing::warn!("Failed to parse skills for {}: {}", creature.name, e);
                                    }
                                    tx.rollback()?;
                                    continue;
                                }
                            }

                            // Parse and insert spells
                            match parsers::parse_creature_spells(&text) {
                                Ok(mut spells) => {
                                    if !spells.is_empty() {
                                        // Set creature_id for each spell
                                        for spell in &mut spells {
                                            spell.creature_id = creature_id;
                                        }

                                        if let Err(e) = self.insert_creature_spells(&tx, creature_id, &spells) {
                                            error_count += 1;
                                            if quiet < 2 {
                                                tracing::warn!("Failed to insert spells for {}: {}", creature.name, e);
                                            }
                                            tx.rollback()?;
                                            continue;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_count += 1;
                                    if quiet < 2 {
                                        tracing::warn!("Failed to parse spells for {}: {}", creature.name, e);
                                    }
                                    tx.rollback()?;
                                    continue;
                                }
                            }

                            // Parse loot
                            match parsers::parse_creature_loot(&file_path) {
                                Ok(loot) => {
                                    if !loot.is_empty() {
                                        if let Err(e) = self.insert_creature_loot(&tx, creature_id, &loot) {
                                            error_count += 1;
                                            if quiet < 2 {
                                                tracing::warn!("Failed to insert loot for {}: {}", creature.name, e);
                                            }
                                            tx.rollback()?;
                                            continue;
                                        }
                                    }
                                    if let Err(e) = tx.commit() {
                                        error_count += 1;
                                        if quiet < 2 {
                                            tracing::warn!("Failed to commit transaction for {}: {}", creature.name, e);
                                        }
                                    } else {
                                        success_count += 1;
                                        if quiet == 0 {
                                            tracing::info!("Processed {} successfully", creature.name);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_count += 1;
                                    if quiet < 2 {
                                        tracing::warn!("Failed to parse loot for {}: {}", creature.name, e);
                                    }
                                    tx.rollback()?;
                                }
                            }
                        }
                        Err(e) => {
                            error_count += 1;
                            if quiet < 2 {
                                tracing::warn!("Failed to insert creature {}: {}", creature.name, e);
                            }
                            tx.rollback()?;
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    if quiet < 2 {
                        tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        if quiet == 0 {
            tracing::info!("Processed {} creatures successfully, {} errors", success_count, error_count);
        }

        // Rebuild item_loot_sources table from creature_loot data
        if quiet == 0 {
            tracing::info!("Rebuilding item_loot_sources table from creature_loot data...");
        }
        let conn = self.connection()?;
        match self.rebuild_item_loot_sources(&conn) {
            Ok(count) => {
                if quiet == 0 {
                    tracing::info!("Successfully populated item_loot_sources with {} entries", count);
                }
            }
            Err(e) => {
                if quiet < 2 {
                    tracing::warn!("Failed to rebuild item_loot_sources: {}", e);
                }
            }
        }

        Ok(success_count)
    }

    // Item-related methods

    /// Insert or update items from objects.srv
    pub fn insert_or_update_items(&self, items: &[crate::models::Item]) -> Result<usize> {
        let conn = self.connection()?;

        let mut inserted_count = 0;

        for item in items {
            conn.execute(
                "INSERT INTO items (type_id, name, description, flags, attributes, image_link)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(type_id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    flags = excluded.flags,
                    attributes = excluded.attributes,
                    image_link = excluded.image_link",
                (
                    item.type_id,
                    &item.name,
                    &item.description,
                    &item.flags,
                    &item.attributes,
                    "", // image_link will be updated later or during export
                ),
            )?;
            inserted_count += 1;
        }

        Ok(inserted_count)
    }

    /// Clear and insert item prices from .npc files
    pub fn clear_and_insert_item_prices(&self, prices: &[crate::models::ItemPrice]) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        // Clear existing prices
        tx.execute("DELETE FROM item_prices", ())?;

        let mut inserted_count = 0;

        for price in prices {
            tx.execute(
                "INSERT INTO item_prices (item_id, npc_name, price, mode)
                 VALUES (?1, ?2, ?3, ?4)",
                (
                    price.item_id,
                    &price.npc_name,
                    price.price,
                    &price.mode,
                ),
            )?;
            inserted_count += 1;
        }

        tx.commit()?;
        Ok(inserted_count)
    }

    /// Load quest names from CSV file
    ///
    /// Returns a HashMap mapping quest_value to quest_name
    pub fn load_quest_names_from_csv(csv_path: &std::path::Path) -> Result<HashMap<i32, String>> {
        let mut quest_map = HashMap::new();

        let file = std::fs::File::open(csv_path)
            .map_err(|e| DemonaxError::Parse(format!("Failed to open quest CSV: {}", e)))?;

        let mut rdr = csv::Reader::from_reader(file);

        for result in rdr.records() {
            let record = result.map_err(|e| DemonaxError::Parse(format!("Failed to parse CSV record: {}", e)))?;

            // CSV format: quest_value,quest_name,quest_legend,link,level_rec
            if record.len() < 2 {
                continue;
            }

            let quest_value: i32 = record.get(0)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let quest_name = record.get(1)
                .unwrap_or("")
                .to_string();

            if quest_value > 0 && !quest_name.is_empty() {
                quest_map.insert(quest_value, quest_name);
            }
        }

        Ok(quest_map)
    }

    /// Process quest chest data from map files
    ///
    /// Note: This clears existing quest chest data and replaces it with new data.
    /// Quest metadata (names, descriptions) can be provided via quest_names map.
    pub fn process_quest_chests(
        &self,
        chests: &[crate::models::QuestChest],
        quest_names: Option<&HashMap<i32, String>>,
        quiet: u8
    ) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        // Note: We store quest chest locations and rewards in the database
        // Quest metadata (names, descriptions, etc.) can be managed via a separate table
        // For now, we'll aggregate chest data by quest_value

        let mut processed = 0;

        for chest in chests {
            // Filter out Rook-only quests (values 17-35, 58, 59, 223, 224) and 255
            let rook_only = matches!(chest.quest_value, 17..=35 | 58 | 59 | 223 | 224 | 255);
            if rook_only {
                continue;
            }

            // Store quest chest location and contents
            // We can either:
            // 1. Store in quests table with JSON
            // 2. Store in a separate quest_chests table
            // For now, let's store as JSON in the quests table

            let reward_items_json = serde_json::to_string(&chest.item_ids)?;
            let chest_location = format!("{} ({})", chest.ingame_coords, chest.sector_name);

            // Get quest name from map or use default
            let quest_name = quest_names
                .and_then(|map| map.get(&chest.quest_value))
                .map(|s| s.clone())
                .unwrap_or_else(|| format!("Quest {}", chest.quest_value));

            // Insert or update quest
            tx.execute(
                "INSERT INTO quests (id, chest_location, reward_items_json, name, description)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    chest_location = COALESCE(
                        CASE WHEN chest_location IS NULL OR chest_location = ''
                        THEN excluded.chest_location
                        ELSE chest_location || '; ' || excluded.chest_location
                        END,
                        excluded.chest_location
                    ),
                    reward_items_json = excluded.reward_items_json",
                (
                    chest.quest_value,
                    &chest_location,
                    &reward_items_json,
                    &quest_name,
                    "", // Empty description for now
                ),
            )?;

            processed += 1;
        }

        tx.commit()?;

        if quiet == 0 {
            tracing::info!("Processed {} quest chests", processed);
        }

        Ok(processed)
    }

    /// Update items table with quest reward information
    ///
    /// Reads quest data from database and updates items with which quests reward them.
    /// Adds a 'rewarded_from' column to items table if it doesn't exist.
    pub fn update_items_with_quest_rewards(&self, quiet: u8) -> Result<usize> {
        let mut conn = self.connection()?;

        // Add rewarded_from column if it doesn't exist
        conn.execute(
            "ALTER TABLE items ADD COLUMN rewarded_from TEXT",
            (),
        ).ok(); // Ignore error if column already exists

        if quiet == 0 {
            tracing::info!("Querying quests from database");
        }

        // Query all quests with their rewards
        let quests: Vec<(i32, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, name, reward_items_json FROM quests WHERE reward_items_json IS NOT NULL AND reward_items_json != '[]'"
            )?;

            stmt.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        }; // stmt is dropped here

        if quiet == 0 {
            tracing::info!("Found {} quests with rewards", quests.len());
        }

        // Build a map of item_id -> list of quest names
        let mut item_to_quests: HashMap<i32, Vec<String>> = HashMap::new();

        for (_quest_id, quest_name, reward_items_json) in quests {
            // Parse reward_items_json to get item IDs
            let item_ids: Vec<i32> = serde_json::from_str(&reward_items_json)
                .unwrap_or_default();

            for item_id in item_ids {
                item_to_quests
                    .entry(item_id)
                    .or_insert_with(Vec::new)
                    .push(quest_name.clone());
            }
        }

        if quiet == 0 {
            tracing::info!("Mapped {} items to quest rewards", item_to_quests.len());
        }

        // Update items table (stmt is already dropped, so we can now borrow mutably)
        let tx = conn.transaction()?;
        let mut updated_count = 0;

        for (item_id, quest_names) in item_to_quests {
            let rewarded_from = quest_names.join(", ");

            tx.execute(
                "UPDATE items SET rewarded_from = ?1 WHERE type_id = ?2",
                (&rewarded_from, item_id),
            )?;

            updated_count += 1;
        }

        tx.commit()?;

        if quiet == 0 {
            tracing::info!("Updated {} items with quest reward information", updated_count);
        }

        Ok(updated_count)
    }

    /// Insert or update spells from magic.cc
    pub fn insert_or_update_spells(&self, spells: &[crate::models::Spell]) -> Result<usize> {
        let conn = self.connection()?;
        let mut inserted_count = 0;

        for spell in spells {
            conn.execute(
                "INSERT INTO spells (id, name, words, level, magic_level, mana, soul_points,
                                    flags, is_rune, rune_type_id, charges, spell_type, premium)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    words = excluded.words,
                    level = excluded.level,
                    magic_level = excluded.magic_level,
                    mana = excluded.mana,
                    soul_points = excluded.soul_points,
                    flags = excluded.flags,
                    is_rune = excluded.is_rune,
                    rune_type_id = excluded.rune_type_id,
                    charges = excluded.charges,
                    spell_type = excluded.spell_type,
                    premium = excluded.premium",
                (
                    spell.spell_id,
                    &spell.name,
                    &spell.words,
                    spell.level,
                    spell.magic_level,
                    spell.mana,
                    spell.soul_points,
                    spell.flags,
                    spell.is_rune,
                    spell.rune_type_id,
                    spell.charges,
                    &spell.spell_type,
                    spell.premium,
                ),
            )?;
            inserted_count += 1;
        }

        Ok(inserted_count)
    }

    /// Clear and insert spell teaching data
    pub fn clear_and_insert_spell_teachers(&self, teachers: &[crate::models::SpellTeacher]) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        // Clear existing spell teachers
        tx.execute("DELETE FROM spell_teachers", ())?;

        let mut inserted_count = 0;
        for teacher in teachers {
            // Look up spell level from spells table
            let level_required: Option<i32> = tx
                .query_row(
                    "SELECT level FROM spells WHERE id = ?1",
                    [teacher.spell_id],
                    |row| row.get(0)
                )
                .ok();

            tx.execute(
                "INSERT INTO spell_teachers (npc_name, spell_name, spell_id, vocation, price, level_required)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(npc_name, spell_id, vocation) DO UPDATE SET
                    spell_name = excluded.spell_name,
                    price = excluded.price,
                    level_required = excluded.level_required",
                (
                    &teacher.npc_name,
                    format!("Spell {}", teacher.spell_id), // Default spell name, can be joined with spells table
                    teacher.spell_id,
                    &teacher.vocation,
                    teacher.teaching_price,
                    level_required,
                ),
            )?;
            inserted_count += 1;
        }

        tx.commit()?;
        Ok(inserted_count)
    }

    /// Clear and insert rune seller data, linking to spells where applicable
    ///
    /// For runes, looks up spell_id by matching item_id to rune_type_id in spells table
    pub fn clear_and_insert_rune_sellers(
        &self,
        sellers: &[crate::models::RuneSeller]
    ) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        // Clear existing rune sellers
        tx.execute("DELETE FROM rune_sellers", ())?;

        let mut inserted_count = 0;
        for seller in sellers {
            // For runes, lookup spell_id by matching item_id to rune_type_id
            let spell_id = if seller.item_category == "rune" {
                tx.query_row(
                    "SELECT id FROM spells WHERE rune_type_id = ?1",
                    (seller.item_id,),
                    |row| row.get::<_, i32>(0),
                ).ok()
            } else {
                None
            };

            tx.execute(
                "INSERT INTO rune_sellers (npc_name, item_id, spell_id, vocation, price, charges, account_type, item_category)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(npc_name, item_id, vocation) DO UPDATE SET
                    spell_id = excluded.spell_id,
                    price = excluded.price,
                    charges = excluded.charges,
                    account_type = excluded.account_type,
                    item_category = excluded.item_category",
                (
                    &seller.npc_name,
                    seller.item_id,
                    spell_id,
                    &seller.vocation,
                    seller.price,
                    seller.charges,
                    &seller.account_type,
                    &seller.item_category,
                ),
            )?;
            inserted_count += 1;
        }

        tx.commit()?;
        Ok(inserted_count)
    }

    /// Get rune spells that have no sellers in rune_sellers table
    pub fn get_unsold_runes(&self) -> Result<Vec<crate::models::Spell>> {
        let conn = self.connection()?;

        let mut stmt = conn.prepare(
            "SELECT s.id, s.name, s.words, s.level, s.magic_level, s.mana,
                    s.soul_points, s.flags, s.is_rune, s.rune_type_id,
                    s.charges, s.spell_type, s.premium
             FROM spells s
             LEFT JOIN rune_sellers rs ON s.id = rs.spell_id
             WHERE s.is_rune = 1 AND rs.spell_id IS NULL
             ORDER BY s.level, s.name"
        )?;

        let spells = stmt.query_map([], |row| {
            Ok(crate::models::Spell {
                spell_id: row.get(0)?,
                name: row.get(1)?,
                words: row.get(2)?,
                level: row.get(3)?,
                magic_level: row.get(4)?,
                mana: row.get(5)?,
                soul_points: row.get(6)?,
                flags: row.get(7)?,
                is_rune: row.get(8)?,
                rune_type_id: row.get(9)?,
                charges: row.get(10)?,
                spell_type: row.get(11)?,
                premium: row.get(12)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(spells)
    }

    /// Get spells that have no teachers
    pub fn get_untaught_spells(&self) -> Result<Vec<crate::models::Spell>> {
        let conn = self.connection()?;

        let mut stmt = conn.prepare(
            "SELECT s.id, s.name, s.words, s.level, s.magic_level, s.mana,
                    s.soul_points, s.flags, s.is_rune, s.rune_type_id,
                    s.charges, s.spell_type, s.premium
             FROM spells s
             LEFT JOIN spell_teachers st ON s.id = st.spell_id
             WHERE st.spell_id IS NULL
             ORDER BY s.level, s.name"
        )?;

        let spells = stmt.query_map([], |row| {
            Ok(crate::models::Spell {
                spell_id: row.get(0)?,
                name: row.get(1)?,
                words: row.get(2)?,
                level: row.get(3)?,
                magic_level: row.get(4)?,
                mana: row.get(5)?,
                soul_points: row.get(6)?,
                flags: row.get(7)?,
                is_rune: row.get(8)?,
                rune_type_id: row.get(9)?,
                charges: row.get(10)?,
                spell_type: row.get(11)?,
                premium: row.get(12)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(spells)
    }

    /// Insert harvesting data into database
    pub fn insert_harvesting_data(&self, harvesting: &[crate::models::HarvestingData]) -> Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;

        // Clear existing harvesting data
        tx.execute("DELETE FROM harvesting_data", ())?;

        let mut inserted_count = 0;

        for entry in harvesting {
            tx.execute(
                "INSERT INTO harvesting_data (tool_id, corpse_id, next_corpse_id,
                                               percent_chance, reward_id, race_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    entry.tool_id,
                    entry.corpse_id,
                    entry.next_corpse_id,
                    entry.percent_chance,
                    entry.reward_id,
                    entry.race_id,
                ),
            )?;
            inserted_count += 1;
        }

        tx.commit()?;
        Ok(inserted_count)
    }

    /// Insert or update raids from .evt files
    pub fn insert_or_update_raids(&self, raids: &[crate::models::Raid]) -> Result<usize> {
        let conn = self.connection()?;
        let mut inserted_count = 0;

        for raid in raids {
            conn.execute(
                "INSERT INTO raids (name, type, waves, interval_seconds, interval_days,
                                   message, creatures, spawn_composition_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(name) DO UPDATE SET
                    type = excluded.type,
                    waves = excluded.waves,
                    interval_seconds = excluded.interval_seconds,
                    interval_days = excluded.interval_days,
                    message = excluded.message,
                    creatures = excluded.creatures,
                    spawn_composition_json = excluded.spawn_composition_json",
                (
                    &raid.name,
                    &raid.raid_type,
                    &raid.waves,
                    raid.interval_seconds,
                    raid.interval_days,
                    &raid.message,
                    &raid.creatures,
                    &raid.spawn_composition_json,
                ),
            )?;
            inserted_count += 1;
        }

        Ok(inserted_count)
    }

    /// Get the latest snapshot date from the database
    pub fn get_latest_snapshot_date(&self) -> Result<String> {
        let conn = self.connection()?;
        let date: String = conn.query_row(
            "SELECT MAX(snapshot_date) FROM daily_snapshots",
            [],
            |row| row.get(0),
        )?;
        Ok(date)
    }

    /// Get latest snapshots for all players (or a specific player if player_id is provided)
    pub fn get_latest_snapshots(&self, player_id: Option<i32>) -> Result<Vec<PlayerSnapshot>> {
        let conn = self.connection()?;

        let query = if player_id.is_some() {
            "SELECT ds.player_id, p.name, ds.snapshot_date, ds.equipment_json
             FROM daily_snapshots ds
             INNER JOIN players p ON ds.player_id = p.id
             WHERE ds.snapshot_date = (SELECT MAX(snapshot_date) FROM daily_snapshots)
             AND ds.player_id = ?"
        } else {
            "SELECT ds.player_id, p.name, ds.snapshot_date, ds.equipment_json
             FROM daily_snapshots ds
             INNER JOIN players p ON ds.player_id = p.id
             WHERE ds.snapshot_date = (SELECT MAX(snapshot_date) FROM daily_snapshots)"
        };

        let mut stmt = conn.prepare(query)?;

        let snapshots = if let Some(pid) = player_id {
            stmt.query_map([pid], |row| {
                let equipment_json: String = row.get(3)?;
                let equipment: Vec<i32> = serde_json::from_str(&equipment_json)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                Ok(PlayerSnapshot {
                    player_id: row.get(0)?,
                    player_name: row.get(1)?,
                    snapshot_date: row.get(2)?,
                    equipment,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], |row| {
                let equipment_json: String = row.get(3)?;
                let equipment: Vec<i32> = serde_json::from_str(&equipment_json)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                Ok(PlayerSnapshot {
                    player_id: row.get(0)?,
                    player_name: row.get(1)?,
                    snapshot_date: row.get(2)?,
                    equipment,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(snapshots)
    }

    // Additional helper methods will be added as needed
    // Rendering functions will query items, prices, and loot directly as needed
}