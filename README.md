# Demonax Tools

A Rust-based CLI tool for managing Demonax game server metadata. Parse game files, extract data, and store it in a SQLite database for web interface rendering and analysis.

## Overview

Demonax Tools processes various game data files including:
- Player character data (.usr files)
- Creature/monster definitions (.mon files)
- Item metadata (objects.srv binary format)
- NPC data and prices (.npc files)
- Map sectors and quest locations (.sec files)
- Raid events (.evt files)
- Spell definitions (magic.cc source code)
- Skinning mechanics (CSV data)

All data is stored in a SQLite database with a well-structured schema for efficient querying and web rendering.

## Technology Stack

- **Language**: Rust (edition 2024)
- **Database**: SQLite with rusqlite, r2d2 connection pooling
- **CLI Framework**: clap (derive syntax)
- **Error Handling**: anyhow (CLI), eyre (library), thiserror
- **Async/Parallel**: tokio, rayon, futures
- **Logging**: tracing framework with file appender
- **Progress Reporting**: indicatif

## Installation & Building

```bash
# Clone the repository (if not already cloned)
cd /home/cmd/repos/demonax-tools

# Build the release binary
cargo build --release

# Binary location after build
./target/release/demonax
```

## Quick Start

```bash
# Build all game data into a database
./target/release/demonax --database ./game.sqlite update-creatures --game-path /path/to/game
./target/release/demonax --database ./game.sqlite update-items-core --game-path /path/to/game
./target/release/demonax --database ./game.sqlite update-quest-overview --game-path /path/to/game
./target/release/demonax --database ./game.sqlite update-items-quests --game-path /path/to/game
./target/release/demonax --database ./game.sqlite update-raids --game-path /path/to/game
./target/release/demonax --database ./game.sqlite update-skinning --skinning-csv /path/to/skinning.csv
./target/release/demonax --database ./game.sqlite update-spells --magic-cc /path/to/magic.cc --game-path /path/to/game
./target/release/demonax --database ./game.sqlite process-usr --input-dir /path/to/game/usr --snapshot-date 2026-01-07

# Or use the test suite
./test-all-commands.sh
```

## Command Reference

### Global Options

All commands support these global options:

- `--database <PATH>`: SQLite database file path (default: `./demonax.sqlite`)
- `--log-file <PATH>`: Log file path for tracing output
- `-v`, `-vv`, `-vvv`, `-vvvv`: Verbosity levels (0-4 for increasingly detailed logging)
- `--quiet <0-4>`: Reduce output verbosity (0=normal, 4=silent)

### 1. process-usr - Process Player Character Data

Parse .usr player files and store character snapshots in the database.

**Syntax:**
```bash
demonax [--database <DB>] process-usr --input-dir <DIR> --snapshot-date <DATE> [--quiet <0-4>]
```

**Purpose:** Extract player statistics, skills, equipment, quest progress, bestiary kills, and skinning data from .usr files.

**Inputs:**
- `--input-dir`: Directory containing .usr files (typically `game/usr` with subdirectories 00-99)
- `--snapshot-date`: Date for this snapshot in YYYY-MM-DD format
- Optional: `--quiet <0-4>` to control output verbosity

**Outputs:**
- Database tables populated:
  - `players`: Player names and first/last seen dates
  - `daily_snapshots`: Stats snapshot (level, experience, magic level, skills, equipment)
  - `daily_quests`: Quest completion flags
  - `bestiary`: Monster kill counts
  - `skinning`: Skinning progress per race

**Performance:** < 5 seconds for 18 player files

**Example:**
```bash
demonax --database ./demonax.sqlite process-usr \
  --input-dir /home/cmd/tibia_local/game/usr \
  --snapshot-date 2026-01-07
```

**Test Output:** 18 players, 18 snapshots

---

### 2. update-creatures - Process Creature/Monster Data

Parse .mon files for creature statistics and loot tables.

**Syntax:**
```bash
demonax update-creatures --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Extract creature definitions including stats (HP, attack, armor, experience) and loot drop tables.

**Inputs:**
- `--game-path`: Game directory containing `mon/` subdirectory
- `.mon` files define: RaceNumber, Name, Experience, HitPoints, Attack, Armor, Inventory (loot)

**Outputs:**
- Database tables:
  - `creatures`: Creature stats and metadata
  - `creature_loot`: Loot drop tables with item IDs, counts, and drop chances

**Performance:** < 2 seconds for 202 .mon files

**Example:**
```bash
demonax --database ./demonax.sqlite update-creatures \
  --game-path /home/cmd/tibia_local/game
```

**Test Output:** 187 creatures, 1911 loot entries

**Data Notes:**
- Only processes .mon files (excludes .evt raid files)
- Loot chances are raw values (1-999) with calculated percentages: (chance + 1) / 999 * 100
  - Minimum: 1 (0.2% drop rate, ultra-rare items from bosses)
  - Maximum: 999 (100% drop rate, guaranteed drops)
- Creatures can have multiple entries for the same item with different amounts/chances

---

### 3. update-items-core - Process Item Metadata and Prices

Parse objects.srv for item metadata and .npc files for buy/sell prices.

**Syntax:**
```bash
demonax update-items-core --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Extract item definitions and NPC trading prices.

**Inputs:**
- `--game-path`: Game directory with `dat/objects.srv` and `npc/` subdirectory
- `objects.srv`: Binary file containing item TypeID, Name, Flags, Attributes
- `.npc` files: NPC dialogue including buy/sell price definitions

**Outputs:**
- Database tables:
  - `items`: Item metadata (type_id, name, flags, attributes)
  - `item_prices`: NPC buy/sell prices

**Performance:** < 6 seconds (2s for objects.srv, ~4s for 352 .npc files in parallel)

**Example:**
```bash
demonax --database ./demonax.sqlite update-items-core \
  --game-path /home/cmd/tibia_local/game
```

**Test Output:** 706 items, 4709 price entries

**Data Notes:**
- Only stores items with "Take" flag (excludes non-portable objects)
- NPC prices include both buy (player purchasing) and sell (player selling) modes
- Parallel processing used for .npc file parsing

---

### 4. update-quest-overview - Extract Quest Chest Locations

Parse .sec map files to find quest chests and their rewards.

**Syntax:**
```bash
demonax update-quest-overview --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Scan map sector files for quest chests and extract coordinates, container contents, and rewards.

**Inputs:**
- `--game-path`: Game directory containing `map/` subdirectory
- `.sec` files: Map sector files with container/chest definitions

**Outputs:**
- Database table:
  - `quests`: Quest name, description, coordinates (x, y, z), rewards (JSON)

**Performance:** < 1 second for 10,538 .sec files (parallel processing)

**Example:**
```bash
demonax --database ./demonax.sqlite update-quest-overview \
  --game-path /home/cmd/tibia_local/game
```

**Test Output:** 259 quests

**Data Notes:**
- Processes map files in parallel using rayon
- Rewards stored as JSON array of item IDs
- Coordinates represent chest locations on the map

---

### 5. update-items-quests - Link Quest Rewards to Items

Enrich items table with quest reward information.

**Syntax:**
```bash
demonax update-items-quests --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Cross-reference quest rewards with items to populate the `rewarded_from` column.

**Inputs:**
- Existing database with `quests` and `items` tables already populated

**Outputs:**
- Updates `items.rewarded_from` column with quest names where the item appears as a reward

**Performance:** < 1 second (database operation)

**Dependencies:** Must run after both `update-quest-overview` and `update-items-core`

**Example:**
```bash
demonax --database ./demonax.sqlite update-items-quests \
  --game-path /home/cmd/tibia_local/game
```

---

### 6. update-raids - Process Raid Event Definitions

Parse .evt files for raid configurations.

**Syntax:**
```bash
demonax update-raids --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Extract raid event definitions including wave configurations, spawn compositions, and timing.

**Inputs:**
- `--game-path`: Game directory containing `mon/` subdirectory
- `.evt` files: Raid event definitions (found in `mon/` alongside .mon files)

**Outputs:**
- Database table:
  - `raids`: Raid name, type, waves, interval (seconds/days), creatures, spawn composition (JSON)

**Performance:** < 1 second for 35 raid files

**Example:**
```bash
demonax --database ./demonax.sqlite update-raids \
  --game-path /home/cmd/tibia_local/game
```

**Test Output:** 34 raids

**Data Notes:**
- Raid types: cyclic (recurring) or one-time events
- Spawn compositions stored as JSON for flexible querying
- Intervals can be in seconds (for short events) or days (for cyclic raids)

---

### 7. update-skinning - Process Skinning Mechanics

Load skinning recipe data (which tools skin which corpses for which rewards).

**Syntax:**
```bash
demonax update-skinning [--skinning-csv <PATH>] [--game-path <DIR>] [--quiet <0-4>]
```

**Purpose:** Load skinning mechanics from CSV file defining tool/corpse/reward relationships.

**Inputs:**
- `--skinning-csv`: Custom path to skinning.csv (optional)
- `--game-path`: Game directory to search for skinning.csv if custom path not provided
- Searches standard locations if neither provided:
  - `game-path/skinning.csv`
  - `game-path/dat/skinning.csv`
  - `./skinning.csv`
- CSV format: `tool_id,corpse_id,next_corpse_id,percent_chance,reward_id,race_id`

**Outputs:**
- Database table:
  - `skinning_data`: Tool ID, corpse ID, next corpse ID, percent chance, reward ID, race ID

**Performance:** < 1 second

**Example:**
```bash
demonax --database ./demonax.sqlite update-skinning \
  --skinning-csv ~/repos/demonax-data/csv/skinning.csv
```

**Test Output:** 26 skinning recipes

**Data Notes:**
- Custom path argument allows skinning.csv to be stored outside game directory
- Tool IDs are item type IDs (e.g., 3007 for obsidian knife)
- Corpse IDs are item type IDs for dead creature corpses
- Reward IDs are item type IDs obtained from skinning

---

### 8. update-spells - Process Spell Definitions and Teaching

Parse magic.cc for spell definitions and .npc files for spell teaching information.

**Syntax:**
```bash
demonax update-spells [--magic-cc <PATH>] --game-path <DIR> [--quiet <0-4>]
```

**Purpose:** Extract spell definitions (words, mana cost, level requirements) and NPC spell teaching (which NPCs teach which spells).

**Inputs:**
- `--magic-cc`: Custom path to magic.cc C++ source file (optional)
- `--game-path`: Game directory containing `npc/` subdirectory for spell teaching data
- Searches standard locations for magic.cc if custom path not provided:
  - `game-path/src/magic.cc`
  - `game-path/magic.cc`
  - `game-path/../src/magic.cc`
- `magic.cc`: C++ source with spell definitions (words, mana, level, soul points, effects)
- `.npc` files: NPCs that teach spells (spell ID, price, vocation, level requirements)

**Outputs:**
- Database tables:
  - `spells`: Spell ID, name, magic words, level, mana, spell type, premium flag
  - `spell_teachers`: NPC name, spell name, spell ID, vocation, price, level required

**Performance:** < 1 second for magic.cc, ~1 second for .npc parsing

**Example:**
```bash
demonax --database ./demonax.sqlite update-spells \
  --magic-cc ~/repos/tibia-game/src/magic.cc \
  --game-path /home/cmd/tibia_local/game
```

**Test Output:** 108 spells, 637 spell teachers

**Data Notes:**
- Custom magic.cc path useful when source code is in separate repository
- Works without magic.cc (spell teaching from .npc files only)
- Spell types: healing, damage, support, rune, etc.
- Vocation filtering: knight, paladin, sorcerer, druid

---

## Command Execution Order

Commands should be executed in this order due to dependencies:

```bash
# Stage 1: Independent commands (can run in any order)
demonax update-creatures --game-path /path/to/game
demonax update-items-core --game-path /path/to/game
demonax update-quest-overview --game-path /path/to/game

# Stage 2: Dependent on items + quests
demonax update-items-quests --game-path /path/to/game

# Stage 3: Independent game data (can run in any order)
demonax update-raids --game-path /path/to/game
demonax update-skinning --skinning-csv /path/to/skinning.csv
demonax update-spells --magic-cc /path/to/magic.cc --game-path /path/to/game

# Stage 4: Player data (can reference creatures/items)
demonax process-usr --input-dir /path/to/game/usr --snapshot-date 2026-01-07
```

**Dependency Summary:**
- `update-items-quests` requires: `update-items-core` and `update-quest-overview`
- `process-usr` benefits from (but doesn't require): `update-creatures` and `update-items-core` for referencing loot/equipment

---

## Database Structure

### Schema Overview

```sql
-- Player Data
players (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  first_seen TEXT NOT NULL,
  last_seen TEXT NOT NULL
)

daily_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  player_id INTEGER NOT NULL,
  snapshot_date TEXT NOT NULL,
  level INTEGER NOT NULL,
  experience INTEGER NOT NULL,
  magic_level INTEGER NOT NULL,
  skills_json TEXT NOT NULL,
  equipment_json TEXT NOT NULL,
  FOREIGN KEY (player_id) REFERENCES players(id) ON DELETE CASCADE,
  UNIQUE(player_id, snapshot_date)
)

daily_quests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  quest_id INTEGER NOT NULL,
  completed INTEGER NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE
)

bestiary (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  monster_id INTEGER NOT NULL,
  kill_count INTEGER NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE
)

skinning (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  race_id INTEGER NOT NULL,
  skin_count INTEGER NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES daily_snapshots(id) ON DELETE CASCADE
)

-- Creature Data
creatures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  race INTEGER NOT NULL UNIQUE,
  name TEXT NOT NULL,
  experience INTEGER NOT NULL,
  hit_points INTEGER NOT NULL,
  attack INTEGER NOT NULL,
  armor INTEGER NOT NULL,
  ...
)

creature_loot (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  creature_id INTEGER NOT NULL,
  item_id INTEGER NOT NULL,
  item_count INTEGER NOT NULL,
  chance_raw INTEGER NOT NULL,
  chance_percent REAL NOT NULL,
  average_value REAL DEFAULT 0.0,
  FOREIGN KEY (creature_id) REFERENCES creatures(id) ON DELETE CASCADE
)

-- Item Data
items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type_id INTEGER NOT NULL UNIQUE,
  name TEXT NOT NULL,
  flags INTEGER NOT NULL,
  attributes TEXT NOT NULL,
  rewarded_from TEXT
)

item_prices (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id INTEGER NOT NULL,
  npc_name TEXT NOT NULL,
  price INTEGER NOT NULL,
  mode TEXT NOT NULL CHECK(mode IN ('buy', 'sell'))
)

-- Game Content
quests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT,
  x INTEGER NOT NULL,
  y INTEGER NOT NULL,
  z INTEGER NOT NULL,
  rewards TEXT NOT NULL
)

raids (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  type TEXT NOT NULL,
  waves TEXT NOT NULL,
  interval_seconds REAL,
  interval_days REAL,
  message TEXT NOT NULL DEFAULT '',
  creatures TEXT NOT NULL DEFAULT '',
  spawn_composition_json TEXT NOT NULL DEFAULT '[]'
)

skinning_data (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tool_id INTEGER NOT NULL,
  corpse_id INTEGER NOT NULL,
  next_corpse_id INTEGER NOT NULL,
  percent_chance INTEGER NOT NULL,
  reward_id INTEGER NOT NULL,
  race_id INTEGER NOT NULL,
  UNIQUE(tool_id, corpse_id)
)

spells (
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
)

spell_teachers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  npc_name TEXT NOT NULL,
  spell_name TEXT NOT NULL,
  spell_id INTEGER NOT NULL,
  vocation TEXT NOT NULL,
  price INTEGER NOT NULL,
  level_required INTEGER,
  UNIQUE(npc_name, spell_id, vocation)
)
```

### Key Relationships

- `daily_snapshots.player_id` → `players.id`
- `daily_quests.snapshot_id` → `daily_snapshots.id`
- `bestiary.snapshot_id` → `daily_snapshots.id`
- `skinning.snapshot_id` → `daily_snapshots.id`
- `creature_loot.creature_id` → `creatures.id`
- `spell_teachers.spell_id` → `spells.id`

**Note:** Some relationships use item type IDs directly rather than foreign keys (e.g., `item_prices.item_id` references `items.type_id`, not `items.id`) to match game file formats.

---

## Database Query Examples

### Player Statistics

```sql
-- Get all players with their latest snapshot
SELECT p.name, ds.level, ds.experience, ds.magic_level, ds.snapshot_date
FROM players p
JOIN daily_snapshots ds ON p.id = ds.player_id
WHERE ds.snapshot_date = (
  SELECT MAX(snapshot_date)
  FROM daily_snapshots
  WHERE player_id = p.id
)
ORDER BY ds.level DESC;

-- Player progression over time
SELECT snapshot_date, level, experience, magic_level
FROM daily_snapshots
WHERE player_id = (SELECT id FROM players WHERE name = 'PlayerName')
ORDER BY snapshot_date;

-- Top monster hunters (bestiary kills)
SELECT p.name, b.monster_id, b.kill_count
FROM bestiary b
JOIN daily_snapshots ds ON b.snapshot_id = ds.id
JOIN players p ON ds.player_id = p.id
ORDER BY b.kill_count DESC
LIMIT 20;

-- Player skills (parse JSON)
SELECT p.name, ds.level, json_extract(ds.skills_json, '$.sword') as sword_skill
FROM players p
JOIN daily_snapshots ds ON p.id = ds.player_id
WHERE ds.snapshot_date = '2026-01-07';

-- Players by quest completion count
SELECT p.name, COUNT(dq.id) as quests_completed
FROM players p
JOIN daily_snapshots ds ON p.id = ds.player_id
JOIN daily_quests dq ON ds.id = dq.snapshot_id
WHERE dq.completed = 1
GROUP BY p.id
ORDER BY quests_completed DESC;
```

### Creature & Loot Analysis

```sql
-- Top creatures by experience
SELECT name, experience, hit_points, attack, armor
FROM creatures
ORDER BY experience DESC
LIMIT 10;

-- Loot table for a specific creature
SELECT c.name, cl.item_id, cl.item_count, cl.chance_percent
FROM creature_loot cl
JOIN creatures c ON cl.creature_id = c.id
WHERE c.name = 'demon'
ORDER BY cl.chance_percent DESC;

-- Items dropped by creatures (with item names via JOIN)
SELECT c.name AS creature, i.name AS item, cl.chance_percent, cl.item_count
FROM creature_loot cl
JOIN creatures c ON cl.creature_id = c.id
JOIN items i ON cl.item_id = i.type_id
WHERE c.name = 'dragon'
ORDER BY cl.chance_percent DESC;

-- Creatures with most loot variety
SELECT c.name, COUNT(*) as loot_items, SUM(cl.chance_percent) as total_drop_chance
FROM creature_loot cl
JOIN creatures c ON cl.creature_id = c.id
GROUP BY c.id
ORDER BY loot_items DESC
LIMIT 10;

-- Average loot value per creature
SELECT c.name, AVG(cl.average_value) as avg_loot_value
FROM creature_loot cl
JOIN creatures c ON cl.creature_id = c.id
GROUP BY c.id
ORDER BY avg_loot_value DESC
LIMIT 20;

-- Find creatures that drop a specific item
SELECT c.name, cl.chance_percent, cl.item_count
FROM creature_loot cl
JOIN creatures c ON cl.creature_id = c.id
WHERE cl.item_id = 3031  -- gold coins
ORDER BY cl.chance_percent DESC;
```

### Item & Economy Data

```sql
-- Most expensive items to buy from NPCs
SELECT i.name, ip.npc_name, ip.price
FROM items i
JOIN item_prices ip ON i.type_id = ip.item_id
WHERE ip.mode = 'buy'
ORDER BY ip.price DESC
LIMIT 20;

-- Best selling prices (items NPCs will buy)
SELECT i.name, ip.npc_name, ip.price
FROM items i
JOIN item_prices ip ON i.type_id = ip.item_id
WHERE ip.mode = 'sell'
ORDER BY ip.price DESC
LIMIT 20;

-- Quest reward items
SELECT type_id, name, rewarded_from
FROM items
WHERE rewarded_from IS NOT NULL
ORDER BY type_id;

-- Price comparison (buy vs sell for same item)
SELECT
  i.name,
  MAX(CASE WHEN ip.mode = 'buy' THEN ip.price END) as buy_price,
  MAX(CASE WHEN ip.mode = 'sell' THEN ip.price END) as sell_price
FROM items i
JOIN item_prices ip ON i.type_id = ip.item_id
GROUP BY i.type_id
HAVING buy_price IS NOT NULL AND sell_price IS NOT NULL
ORDER BY (buy_price - sell_price) DESC
LIMIT 20;

-- Find NPCs that trade a specific item
SELECT ip.npc_name, ip.mode, ip.price
FROM item_prices ip
JOIN items i ON ip.item_id = i.type_id
WHERE i.name LIKE '%sword%'
ORDER BY ip.price;
```

### Spell & Teaching Data

```sql
-- All spells sorted by level and mana
SELECT name, words, spell_type, level, mana, premium
FROM spells
ORDER BY level, mana;

-- Most expensive spells to learn
SELECT st.npc_name, s.name, s.words, st.price, st.level_required, st.vocation
FROM spell_teachers st
JOIN spells s ON st.spell_id = s.id
ORDER BY st.price DESC
LIMIT 20;

-- Spells taught by a specific NPC
SELECT s.name, s.words, st.vocation, st.price, st.level_required
FROM spell_teachers st
JOIN spells s ON st.spell_id = s.id
WHERE st.npc_name = 'Elane'
ORDER BY st.price;

-- Spells by vocation
SELECT spell_type, COUNT(*) as spell_count
FROM spells
GROUP BY spell_type
ORDER BY spell_count DESC;

-- Premium vs free spells
SELECT premium, COUNT(*) as spell_count
FROM spells
GROUP BY premium;

-- NPCs teaching the most spells
SELECT npc_name, COUNT(*) as spells_taught
FROM spell_teachers
GROUP BY npc_name
ORDER BY spells_taught DESC;
```

### Raid & Quest Information

```sql
-- All raids with their frequency
SELECT name, type, interval_days, creatures
FROM raids
ORDER BY interval_days;

-- Cyclic raids (recurring events)
SELECT name, interval_days, waves, message
FROM raids
WHERE type = 'cyclic'
ORDER BY interval_days;

-- Quests by area (Z-level grouping)
SELECT z, COUNT(*) as quest_count
FROM quests
GROUP BY z
ORDER BY quest_count DESC;

-- Quests with most valuable rewards (parse JSON)
SELECT name, x, y, z, rewards
FROM quests
WHERE rewards LIKE '%3031%'  -- Contains gold coins
LIMIT 20;
```

### Skinning Mechanics

```sql
-- All skinning recipes
SELECT tool_id, corpse_id, next_corpse_id, percent_chance, reward_id, race_id
FROM skinning_data
ORDER BY percent_chance DESC;

-- Skinning recipes by tool
SELECT tool_id, COUNT(*) as recipes
FROM skinning_data
GROUP BY tool_id;

-- Find what rewards come from skinning a specific corpse
SELECT corpse_id, reward_id, percent_chance
FROM skinning_data
WHERE corpse_id = 3101  -- Example corpse ID
ORDER BY percent_chance DESC;
```

---

## Testing

Run the comprehensive test suite:

```bash
./test-all-commands.sh
```

This script:
- Builds the CLI with `cargo build --release`
- Creates a fresh test database at `test-output/demonax-test.sqlite`
- Runs all 8 commands in the correct dependency order
- Uses test data from `DEV/game/`
- Displays timing for each command
- Shows database summary with row counts for all tables

**Expected total runtime:** ~9 seconds

**Test data includes:**
- 202 .mon files (creatures)
- 1 objects.srv + 352 .npc files (items and prices)
- 10,538 .sec files (map sectors for quests)
- 35 .evt files (raids)
- 1 skinning.csv (26 recipes)
- 1 magic.cc + 352 .npc files (spells and teaching)
- 18 .usr files (players)

---

## Performance Benchmarks

Based on test data in `DEV/game/`:

| Command             | Files Processed     | Time  | Database Records           |
|---------------------|---------------------|-------|----------------------------|
| update-creatures    | 202 .mon            | 1.6s  | 187 creatures, 1911 loot   |
| update-items-core   | 1 .srv + 352 .npc   | 5.9s  | 706 items, 4709 prices     |
| update-quest-overview | 10,538 .sec       | 0.06s | 259 quests                 |
| update-items-quests | DB operation        | 0.02s | Updates items.rewarded_from|
| update-raids        | 35 .evt             | 0.3s  | 34 raids                   |
| update-skinning     | 1 CSV               | 0.02s | 26 recipes                 |
| update-spells       | 1 .cc + 352 .npc    | 0.95s | 108 spells, 637 teachers   |
| process-usr         | 18 .usr             | 0.25s | 18 players, 18 snapshots   |

**Total:** ~9 seconds, ~50-100 MB database (depending on loot/quest data volume)

**Performance characteristics:**
- Parallel processing: .npc and .sec files processed using rayon
- Memory efficient: Streaming parsers for large binary files
- Incremental updates: Most commands use UPSERT (INSERT ... ON CONFLICT)
- Connection pooling: r2d2 for efficient database access

---

## Architecture

### Code Structure

```
demonax-tools/
├── cli/                    # Binary crate (627 lines)
│   └── src/
│       └── main.rs         # Command implementations
├── demonax-core/           # Library crate (2,166 lines)
│   ├── migrations/         # SQL schema definitions
│   │   ├── 001_initial_player_schema.up.sql
│   │   ├── 002_creature_loot_schema.up.sql
│   │   ├── 003_item_schema.up.sql
│   │   └── 004_game_data_schema.up.sql
│   └── src/
│       ├── lib.rs          # Public exports
│       ├── models.rs       # Data structures (258 lines)
│       ├── parsers.rs      # File format parsers (1,002 lines)
│       ├── database.rs     # SQLite operations (811 lines)
│       ├── error.rs        # Error types (41 lines)
│       ├── file_utils.rs   # File discovery (41 lines)
│       └── processors.rs   # Processing logic (4 lines)
├── test-all-commands.sh    # Comprehensive test suite
├── test-output/            # Test results and databases
├── DEV/game/               # Test data
├── CONTEXT/                # Migration documentation
└── Cargo.toml              # Workspace configuration
```

### Key Components

**Parsers (`demonax-core/src/parsers.rs`):**
- `.usr` files: Binary player data with Lua-like structured format
- `.mon` files: Lua-like creature definitions
- `.npc` files: Lua dialogue/shop definitions
- `.sec` files: Binary map sector data with container parsing
- `objects.srv`: Binary item database with flags and attributes
- `magic.cc`: C++ source code parsing for spell definitions

**Database (`demonax-core/src/database.rs`):**
- Automatic migrations using rusqlite_migration
- Connection pooling with r2d2
- Transactional inserts with error handling
- UPSERT patterns for incremental updates

**Models (`demonax-core/src/models.rs`):**
- Strongly-typed Rust structs for all game entities
- Serialization support with serde for JSON fields
- Validation and conversion methods

**Error Handling:**
- Library uses `eyre::Result` for detailed error context
- CLI uses `anyhow::Result` for user-friendly error messages
- Errors propagated with `?` operator (no panics)

---

## Migration from R

The previous R package implementation has been moved to the `OLD/` directory. The Rust implementation provides:

✅ **Better Performance**
- Parallel processing with rayon (10,538 map files in 0.06s vs ~30s in R)
- Native code execution (no R interpreter overhead)
- Efficient memory usage with streaming parsers

✅ **Type Safety**
- Compile-time guarantees prevent runtime errors
- Strong typing for all game entities
- No silent type coercion issues

✅ **Better Error Handling**
- Result types with rich context
- No silent failures or `NA` propagation
- Clear error messages with file paths and line numbers

✅ **Single Binary Deployment**
- No R runtime dependencies
- No package installation required
- Cross-platform compilation (Linux, musl)

✅ **Comprehensive Logging**
- Structured logging with tracing framework
- Verbosity levels from -v to -vvvv
- Optional file logging for debugging

✅ **Progress Reporting**
- indicatif progress bars for long operations
- Real-time feedback during processing
- Time estimates for multi-file operations

See `CONTEXT/rust-migration-testing.txt` for detailed migration notes and testing results.

---

## Contributing

When modifying this codebase, please follow the guidelines in `CLAUDE.md`:

- Use Rust edition 2024 and latest dependency versions
- Prioritize correctness and clarity over performance
- Use `?` for error propagation (avoid `unwrap()`)
- No `mod.rs` files (use `src/module.rs` instead)
- Use clap derive syntax for CLI arguments
- Use tracing for logging (not println!)
- Use rayon for parallel processing

---

## License

(To be added)
