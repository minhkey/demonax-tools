#!/bin/bash
set -e

# Configuration
REPO_ROOT="/home/cmd/repos/demonax-tools"
GAME_DIR="$HOME/game"
TEST_DB="$REPO_ROOT/testing/demonax-test.sqlite"
TEST_LOG="$REPO_ROOT/testing/demonax-test.log"
CLI="$REPO_ROOT/target/release/demonax"
DATA_DIR="$HOME/repos/demonax-data/items"
OUTPUT_DIR="$REPO_ROOT/testing/asset/eq"
TEMPLATE="$REPO_ROOT/testing/asset/template/eq.png"
BLANK="$REPO_ROOT/testing/asset/template/blank.png"
PRESENT_CONFIG="$REPO_ROOT/testing/asset/template/present.toml"

# Set environment variables for all commands
export DEMONAX_DATABASE="$TEST_DB"
export DEMONAX_LOG_FILE="$TEST_LOG"
export DEMONAX_GAME_DIR="$GAME_DIR"

# Clean previous test database and log
rm -f "$TEST_DB" "$TEST_LOG"
mkdir -p "$(dirname "$TEST_DB")"

echo "=========================================="
echo "Demonax CLI Comprehensive Test Suite"
echo "=========================================="
echo ""

# Test 1: UpdateCreatures
echo "[1/8] Testing UpdateCreatures..."
time "$CLI" update-creatures --quiet 0
echo ""

# Test 2: UpdateItemsCore
echo "[2/8] Testing UpdateItemsCore..."
time "$CLI" update-items-core --quiet 0
echo ""

# Test 3: UpdateQuestOverview
echo "[3/8] Testing UpdateQuestOverview..."
time "$CLI" update-quest-overview \
  --quest-csv "$REPO_ROOT/testing/asset/csv/quest.csv" \
  --quiet 0
echo ""

# Test 4: UpdateItemsQuests
echo "[4/8] Testing UpdateItemsQuests..."
time "$CLI" update-items-quests --quiet 0
echo ""

# Test 5: UpdateRaids
echo "[5/8] Testing UpdateRaids..."
time "$CLI" update-raids --quiet 0
echo ""

# Test 6: UpdateHarvesting (with custom CSV path)
echo "[6/8] Testing UpdateHarvesting..."
time "$CLI" update-harvesting \
  --harvesting-csv ~/repos/demonax-data/csv/harvesting.csv \
  --quiet 0
echo ""

# Test 7: UpdateSpells (with custom magic.cc path)
echo "[7/8] Testing UpdateSpells..."
time "$CLI" update-spells \
  --magic-cc ~/repos/tibia-game/src/magic.cc \
  --quiet 0
echo ""

# Test 8: ProcessUsr
echo "[8/9] Testing ProcessUsr..."
time "$CLI" process-usr \
  --input-dir "$GAME_DIR/usr" \
  --snapshot-date 2026-01-27 \
  --quiet 0
echo ""

# Test 9: RenderEquipment
echo "[9/10] Testing RenderEquipment..."
mkdir -p "$OUTPUT_DIR"
find "$OUTPUT_DIR" -name "[0-9]*.png" -delete
time "$CLI" render-equipment \
  --data-dir "$DATA_DIR" \
  --output-dir "$OUTPUT_DIR" \
  --template "$TEMPLATE" \
  --blank "$BLANK"
echo ""

# Test 10: GivePresent (only on Fridays)
if [ "$(date +%u)" -eq 5 ]; then
  echo "[10/10] Testing GivePresent..."
  time "$CLI" give-present \
    --usr-path "$GAME_DIR/usr" \
    --present-config "$PRESENT_CONFIG"
  echo ""
else
  echo "[10/10] Skipping GivePresent (not Friday)"
  echo ""
fi

echo "=========================================="
echo "All tests completed!"
echo "Database: $TEST_DB"
echo "=========================================="
echo ""

# Database summary
echo "Database Summary:"
sqlite3 "$TEST_DB" <<EOF
.mode column
SELECT 'players' as table_name, COUNT(*) as row_count FROM players
UNION ALL SELECT 'daily_snapshots', COUNT(*) FROM daily_snapshots
UNION ALL SELECT 'creatures', COUNT(*) FROM creatures
UNION ALL SELECT 'creature_loot', COUNT(*) FROM creature_loot
UNION ALL SELECT 'item_loot_sources', COUNT(*) FROM item_loot_sources
UNION ALL SELECT 'items', COUNT(*) FROM items
UNION ALL SELECT 'item_prices', COUNT(*) FROM item_prices
UNION ALL SELECT 'quests', COUNT(*) FROM quests
UNION ALL SELECT 'raids', COUNT(*) FROM raids
UNION ALL SELECT 'harvesting_data', COUNT(*) FROM harvesting_data
UNION ALL SELECT 'spells', COUNT(*) FROM spells
UNION ALL SELECT 'spell_teachers', COUNT(*) FROM spell_teachers
UNION ALL SELECT 'rune_sellers', COUNT(*) FROM rune_sellers;
EOF
