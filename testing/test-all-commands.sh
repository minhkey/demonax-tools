#!/bin/bash
set -e

# Configuration
REPO_ROOT="/home/cmd/repos/demonax-tools"
GAME_DIR="$REPO_ROOT/DEV/game"
TEST_DB="$REPO_ROOT/test-output/demonax-test.sqlite"
TEST_LOG="$REPO_ROOT/test-output/demonax-test.log"
CLI="$REPO_ROOT/target/release/demonax"

# Clean previous test database and log
rm -f "$TEST_DB" "$TEST_LOG"
mkdir -p "$(dirname "$TEST_DB")"

echo "=========================================="
echo "Demonax CLI Comprehensive Test Suite"
echo "=========================================="
echo ""

# Test 1: UpdateCreatures
echo "[1/8] Testing UpdateCreatures..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-creatures \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 2: UpdateItemsCore
echo "[2/8] Testing UpdateItemsCore..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-items-core \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 3: UpdateQuestOverview
echo "[3/8] Testing UpdateQuestOverview..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-quest-overview \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 4: UpdateItemsQuests
echo "[4/8] Testing UpdateItemsQuests..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-items-quests \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 5: UpdateRaids
echo "[5/8] Testing UpdateRaids..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-raids \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 6: UpdateHarvesting (with custom CSV path)
echo "[6/8] Testing UpdateHarvesting..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-harvesting \
  --harvesting-csv ~/repos/demonax-data/csv/harvesting.csv \
  --quiet 0
echo ""

# Test 7: UpdateSpells (with custom magic.cc path)
echo "[7/8] Testing UpdateSpells..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" update-spells \
  --magic-cc ~/repos/tibia-game/src/magic.cc \
  --game-path "$GAME_DIR" \
  --quiet 0
echo ""

# Test 8: ProcessUsr
echo "[8/8] Testing ProcessUsr..."
time "$CLI" --database "$TEST_DB" --log-file "$TEST_LOG" process-usr \
  --input-dir "$GAME_DIR/usr" \
  --snapshot-date 2026-01-07 \
  --quiet 0
echo ""

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
UNION ALL SELECT 'items', COUNT(*) FROM items
UNION ALL SELECT 'item_prices', COUNT(*) FROM item_prices
UNION ALL SELECT 'quests', COUNT(*) FROM quests
UNION ALL SELECT 'raids', COUNT(*) FROM raids
UNION ALL SELECT 'harvesting_data', COUNT(*) FROM harvesting_data
UNION ALL SELECT 'spells', COUNT(*) FROM spells
UNION ALL SELECT 'spell_teachers', COUNT(*) FROM spell_teachers
UNION ALL SELECT 'rune_sellers', COUNT(*) FROM rune_sellers;
EOF
