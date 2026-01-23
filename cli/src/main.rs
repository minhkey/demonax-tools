use anyhow::Result;
use clap::{Parser, Subcommand};
use demonax_core::database::Database;
use demonax_core::file_utils::find_files_with_extension;
use demonax_core::parsers::{parse_evt_file, parse_magic_cc, parse_map_sector_file, parse_npc_file, parse_npc_rune_selling, parse_npc_spell_teaching, parse_objects_srv};
use demonax_core::models::HarvestingData;
use demonax_core::{generate_all_harvesting_rules, insert_harvesting_rules};
use demonax_core::present::{apply_present_to_file, GiftResult, GiftSummary, PresentConfig};
use rayon::prelude::*;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(
    name = "demonax",
    version = "0.1.0",
    about = "CLI tool for Demonax game server metadata management",
    long_about = None
)]
struct Cli {
    /// Path to SQLite database file
    #[arg(long, global = true)]
    database: Option<std::path::PathBuf>,

    /// Path to log file
    #[arg(long, global = true, default_value = "/tmp/demonax-tools.log")]
    log_file: std::path::PathBuf,

    /// Verbosity level (repeat for more verbose output)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process .usr files into database
    ProcessUsr {
        /// Directory containing .usr files
        #[arg(long)]
        input_dir: std::path::PathBuf,
        /// Date for snapshot (YYYY-MM-DD format)
        #[arg(long)]
        snapshot_date: String,
        /// Quiet mode (0=show messages/warnings, 1=suppress messages, 2=suppress both)
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update creature data
    UpdateCreatures {
        /// Game directory with mon/ subdirectory
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update core item data
    UpdateItemsCore {
        /// Game directory with dat/, mon/, npc/ subdirectories
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Add quest reward information to items
    UpdateItemsQuests {
        /// Game directory with map files
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Process map files for quest chest locations
    UpdateQuestOverview {
        /// Game directory with map files
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update raid data
    UpdateRaids {
        /// Game directory with raid files
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update harvesting data
    UpdateHarvesting {
        /// Game directory with harvesting files
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Custom path to harvesting.csv (optional)
        #[arg(long)]
        harvesting_csv: Option<std::path::PathBuf>,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update spell data
    UpdateSpells {
        /// Game directory with spell files
        #[arg(long, default_value = "/home/cmd/tibia_local/game")]
        game_path: std::path::PathBuf,
        /// Custom path to magic.cc (optional)
        #[arg(long)]
        magic_cc: Option<std::path::PathBuf>,
        /// Web directory for output files
        #[arg(long, default_value = "/home/cmd/Documents/demonax/demonax-web")]
        web_path: std::path::PathBuf,
        /// Quiet mode
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },

    /// Update moveuse.dat with harvesting rules from CSV
    UpdateMoveUseHarvesting {
        /// Path to harvesting.csv
        #[arg(long)]
        csv_path: std::path::PathBuf,
        /// Path to moveuse.dat to update
        #[arg(long)]
        moveuse_path: std::path::PathBuf,
    },

    /// Give presents to players by modifying .usr files
    GivePresent {
        /// Path to usr/ directory containing player files
        #[arg(long)]
        usr_path: std::path::PathBuf,

        /// Path to TOML file defining present contents
        #[arg(long)]
        present_config: std::path::PathBuf,

        /// Inventory slot to place present (default: 10)
        #[arg(long, default_value_t = 10)]
        target_slot: i32,

        /// Show what would be done without modifying files
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Quiet mode (0=show messages/warnings, 1=suppress messages, 2=suppress both)
        #[arg(long, default_value_t = 0)]
        quiet: u8,
    },
}

fn setup_logging(verbose: u8, log_file: &std::path::Path) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let filter_level = match verbose {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    let filter = EnvFilter::from_default_env().add_directive(filter_level.into());

    let file_appender = tracing_appender::rolling::never(
        log_file.parent().unwrap_or(std::path::Path::new(".")),
        log_file.file_name().unwrap_or(std::ffi::OsStr::new("demonax.log")),
    );
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::Layer::new().with_writer(std::io::stderr).with_ansi(true))
        .with(fmt::Layer::new().with_writer(non_blocking).with_ansi(false));

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let _guard = setup_logging(cli.verbose, &cli.log_file)?;

    info!("Starting demonax CLI");

    // TODO: Implement command dispatch
    match cli.command {
        Commands::ProcessUsr { input_dir, snapshot_date, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;
            let processed = db.process_usr_files(&input_dir, &snapshot_date, quiet)?;
            info!("Successfully processed {} .usr files", processed);
        }
        Commands::UpdateCreatures { game_path, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;
            let processed = db.process_mon_files(&game_path, quiet)?;
            info!("Successfully processed {} .mon files", processed);
            // TODO: Generate CSV exports for backward compatibility
        }
        Commands::UpdateItemsCore { game_path, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Processing item data from {:?}", game_path);
            }

            // Parse objects.srv for item metadata
            let objects_srv_path = game_path.join("dat").join("objects.srv");
            if !objects_srv_path.exists() {
                anyhow::bail!("objects.srv not found at {:?}", objects_srv_path);
            }

            if quiet == 0 {
                info!("Parsing objects.srv");
            }
            let items = parse_objects_srv(&objects_srv_path)?;
            if quiet == 0 {
                info!("Found {} items with 'Take' flag", items.len());
            }

            // Insert items into database
            let inserted_count = db.insert_or_update_items(&items)?;
            if quiet == 0 {
                info!("Inserted/updated {} items in database", inserted_count);
            }

            // Parse .npc files for prices (in parallel)
            let npc_dir = game_path.join("npc");
            if !npc_dir.exists() {
                if quiet < 2 {
                    tracing::warn!("NPC directory not found at {:?}, skipping price processing", npc_dir);
                }
            } else {
                if quiet == 0 {
                    info!("Finding .npc files in {:?}", npc_dir);
                }
                let npc_files = find_files_with_extension(&npc_dir, "npc")?;
                if quiet == 0 {
                    info!("Found {} .npc files", npc_files.len());
                }

                // Parse all .npc files in parallel
                let all_prices: Vec<_> = npc_files
                    .par_iter()
                    .filter_map(|path| {
                        match parse_npc_file(path) {
                            Ok(prices) => Some(prices),
                            Err(e) => {
                                if quiet < 2 {
                                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                                }
                                None
                            }
                        }
                    })
                    .flatten()
                    .collect();

                if quiet == 0 {
                    info!("Parsed {} price entries from .npc files", all_prices.len());
                }

                // Insert prices into database
                let price_count = db.clear_and_insert_item_prices(&all_prices)?;
                if quiet == 0 {
                    info!("Inserted {} price entries in database", price_count);
                }
            }

            if quiet == 0 {
                info!("Item processing complete. Data stored in database: {:?}", db_path);
            }
        }
        Commands::UpdateItemsQuests { game_path: _, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Updating items with quest reward information");
            }

            // Update items table with quest rewards from database
            let updated_count = db.update_items_with_quest_rewards(quiet)?;

            if quiet == 0 {
                info!("Successfully updated {} items with quest rewards", updated_count);
                info!("Items table now includes 'rewarded_from' column with quest names");
            }
        }
        Commands::UpdateQuestOverview { game_path, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Processing quest overview from map files in {:?}", game_path);
            }

            // Find all map sector files
            let map_dir = game_path.join("map");
            if !map_dir.exists() {
                anyhow::bail!("Map directory not found at {:?}", map_dir);
            }

            if quiet == 0 {
                info!("Scanning map directory: {:?}", map_dir);
            }

            let map_files = find_files_with_extension(&map_dir, "sec")?;
            if quiet == 0 {
                info!("Found {} map sector files", map_files.len());
            }

            // Parse all map files in parallel to extract quest chests
            let all_chests: Vec<_> = map_files
                .par_iter()
                .filter_map(|path| {
                    match parse_map_sector_file(path) {
                        Ok(chests) if !chests.is_empty() => Some(chests),
                        Ok(_) => None, // Empty chests
                        Err(e) => {
                            if quiet < 2 {
                                tracing::warn!("Failed to parse {:?}: {}", path, e);
                            }
                            None
                        }
                    }
                })
                .flatten()
                .collect();

            if quiet == 0 {
                info!("Extracted {} quest chests from map files", all_chests.len());
            }

            // Process quest chests into database
            let processed = db.process_quest_chests(&all_chests, quiet)?;

            if quiet == 0 {
                info!("Successfully processed {} quests into database: {:?}", processed, db_path);
                info!("Quest metadata (names, descriptions) can be added via database updates");
            }
        }
        Commands::UpdateRaids { game_path, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Processing raid data from .evt files");
            }

            // Find all .evt files in mon directory
            let mon_dir = game_path.join("mon");
            if !mon_dir.exists() {
                anyhow::bail!("Mon directory not found at {:?}", mon_dir);
            }

            let evt_files = find_files_with_extension(&mon_dir, "evt")?;

            // Exclude halloweenhare.evt
            let evt_files: Vec<_> = evt_files
                .into_iter()
                .filter(|p| {
                    !p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n == "halloweenhare.evt")
                        .unwrap_or(false)
                })
                .collect();

            if quiet == 0 {
                info!("Found {} raid files", evt_files.len());
            }

            // Parse all .evt files in parallel
            let raids: Vec<_> = evt_files
                .par_iter()
                .filter_map(|path| {
                    match parse_evt_file(path) {
                        Ok(raid) => Some(raid),
                        Err(e) => {
                            if quiet < 2 {
                                tracing::warn!("Failed to parse {:?}: {}", path, e);
                            }
                            None
                        }
                    }
                })
                .collect();

            if quiet == 0 {
                info!("Parsed {} raids successfully", raids.len());
            }

            // Insert into database
            let inserted = db.insert_or_update_raids(&raids)?;

            if quiet == 0 {
                info!("Inserted/updated {} raids in database: {:?}", inserted, db_path);
                info!("Note: Creature names can be enriched by querying creatures table");
            }
        }
        Commands::UpdateHarvesting { game_path, harvesting_csv, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Processing harvesting data");
            }

            // Use custom path if provided, otherwise search common locations
            let harvesting_csv_path = if let Some(custom_path) = harvesting_csv {
                if custom_path.exists() {
                    Some(custom_path)
                } else {
                    anyhow::bail!("Custom harvesting CSV path does not exist: {:?}", custom_path);
                }
            } else {
                let possible_paths = vec![
                    game_path.join("harvesting.csv"),
                    game_path.join("dat/harvesting.csv"),
                    std::path::PathBuf::from("./harvesting.csv"),
                ];
                possible_paths.iter().find(|p| p.exists()).cloned()
            };

            if let Some(csv_path) = harvesting_csv_path {
                if quiet == 0 {
                    info!("Reading harvesting data from {:?}", csv_path);
                }

                // Read harvesting CSV
                let mut reader = csv::Reader::from_path(&csv_path)?;
                let mut harvesting_data = Vec::new();

                for result in reader.deserialize() {
                    let record: HarvestingData = result?;
                    harvesting_data.push(record);
                }

                if quiet == 0 {
                    info!("Parsed {} harvesting entries from CSV", harvesting_data.len());
                }

                // Insert into database
                let inserted = db.insert_harvesting_data(&harvesting_data)?;

                if quiet == 0 {
                    info!("Inserted {} harvesting entries into database", inserted);
                }

                // Note: The R implementation also writes to moveuse.dat file
                // For now, we store in database only. moveuse.dat generation
                // can be added as a separate command if needed.
                if quiet == 0 {
                    info!("Harvesting data stored in database: {:?}", db_path);
                    info!("Note: moveuse.dat file generation not implemented (data in DB only)");
                }
            } else {
                anyhow::bail!("harvesting.csv not found in any standard location");
            }
        }
        Commands::UpdateSpells { game_path, magic_cc, web_path: _, quiet } => {
            let db_path = cli.database.unwrap_or_else(|| std::path::PathBuf::from("./demonax.sqlite"));
            let db = Database::new(&db_path)?;

            if quiet == 0 {
                info!("Processing spell data");
            }

            // Use custom path if provided, otherwise search common locations
            let magic_cc_path = if let Some(custom_path) = magic_cc {
                if custom_path.exists() {
                    Some(custom_path)
                } else {
                    if quiet < 2 {
                        tracing::warn!("Custom magic.cc path does not exist: {:?}", custom_path);
                    }
                    None
                }
            } else {
                let possible_paths = vec![
                    game_path.join("src/magic.cc"),
                    game_path.join("magic.cc"),
                    game_path.parent().and_then(|p| Some(p.join("src/magic.cc"))).unwrap_or_default(),
                ];
                possible_paths.iter().find(|p| p.exists()).cloned()
            };

            let spells = if let Some(magic_path) = magic_cc_path {
                if quiet == 0 {
                    info!("Parsing magic.cc from {:?}", magic_path);
                }
                parse_magic_cc(&magic_path)?
            } else {
                if quiet < 2 {
                    tracing::warn!("magic.cc not found, skipping spell parsing");
                    tracing::warn!("Note: Spell data requires access to game source code");
                }
                vec![]
            };

            if !spells.is_empty() {
                let inserted = db.insert_or_update_spells(&spells)?;
                if quiet == 0 {
                    info!("Inserted/updated {} spells", inserted);
                }
            }

            // Parse .npc files for spell teaching
            let npc_dir = game_path.join("npc");
            if npc_dir.exists() {
                if quiet == 0 {
                    info!("Parsing .npc files for spell teaching data");
                }

                let npc_files = find_files_with_extension(&npc_dir, "npc")?;
                let all_teachers: Vec<_> = npc_files
                    .par_iter()
                    .filter_map(|path| {
                        match parse_npc_spell_teaching(path) {
                            Ok(teachers) if !teachers.is_empty() => Some(teachers),
                            Ok(_) => None,
                            Err(e) => {
                                if quiet < 2 {
                                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                                }
                                None
                            }
                        }
                    })
                    .flatten()
                    .collect();

                if quiet == 0 {
                    info!("Found {} spell teaching entries", all_teachers.len());
                }

                let teacher_count = db.clear_and_insert_spell_teachers(&all_teachers)?;
                if quiet == 0 {
                    info!("Processed {} spell teachers", teacher_count);
                }

                // Parse rune/wand/rod sellers
                if quiet == 0 {
                    info!("Parsing .npc files for rune/wand/rod seller data");
                }

                let all_sellers: Vec<_> = npc_files
                    .par_iter()
                    .filter_map(|path| {
                        match parse_npc_rune_selling(path) {
                            Ok(sellers) if !sellers.is_empty() => Some(sellers),
                            Ok(_) => None,
                            Err(e) => {
                                if quiet < 2 {
                                    tracing::warn!("Failed to parse rune sellers from {:?}: {}", path, e);
                                }
                                None
                            }
                        }
                    })
                    .flatten()
                    .collect();

                if quiet == 0 {
                    info!("Found {} rune/wand/rod seller entries", all_sellers.len());
                }

                let seller_count = db.clear_and_insert_rune_sellers(&all_sellers)?;
                if quiet == 0 {
                    info!("Processed {} rune/wand/rod sellers", seller_count);
                }
            }

            // Log untaught spells
            if !spells.is_empty() && quiet == 0 {
                let untaught_spells = db.get_untaught_spells()?;

                if !untaught_spells.is_empty() {
                    info!("Found {} spells without teachers:", untaught_spells.len());
                    for spell in untaught_spells {
                        info!(
                            "  - {} (ID: {}): {} - Level {}, {} mana",
                            spell.name,
                            spell.spell_id,
                            spell.words,
                            spell.level,
                            spell.mana
                        );
                    }
                }

                // Log unsold runes
                let unsold_runes = db.get_unsold_runes()?;

                if !unsold_runes.is_empty() {
                    info!("Found {} runes without sellers:", unsold_runes.len());
                    for rune in unsold_runes {
                        info!(
                            "  - {} (ID: {}): {} - Rune Type ID: {:?}",
                            rune.name,
                            rune.spell_id,
                            rune.words,
                            rune.rune_type_id
                        );
                    }
                }
            }

            if quiet == 0 {
                info!("Spell processing complete. Data stored in database: {:?}", db_path);
            }
        }
        Commands::UpdateMoveUseHarvesting { csv_path, moveuse_path } => {
            info!("Updating moveuse.dat with harvesting rules");

            // Check paths exist
            if !csv_path.exists() {
                anyhow::bail!("CSV file not found: {:?}", csv_path);
            }
            if !moveuse_path.exists() {
                anyhow::bail!("moveuse.dat not found: {:?}", moveuse_path);
            }

            // Read and parse CSV
            info!("Reading harvesting data from {:?}", csv_path);
            let mut reader = csv::Reader::from_path(&csv_path)?;
            let mut harvesting_data = Vec::new();

            for result in reader.deserialize() {
                let record: HarvestingData = result?;
                harvesting_data.push(record);
            }

            info!("Parsed {} harvesting entries from CSV", harvesting_data.len());

            // Generate rules
            let rules = generate_all_harvesting_rules(&harvesting_data);
            info!("Generated {} rule pairs ({} lines)", harvesting_data.len(), rules.lines().count());

            // Read moveuse.dat
            let moveuse_content = std::fs::read_to_string(&moveuse_path)?;

            // Insert rules
            let updated_content = insert_harvesting_rules(&moveuse_content, &rules)?;

            // Write back
            std::fs::write(&moveuse_path, updated_content)?;

            info!("Successfully updated {:?} with harvesting rules", moveuse_path);
        }
        Commands::GivePresent { usr_path, present_config, target_slot, dry_run, quiet } => {
            if quiet == 0 {
                if dry_run {
                    info!("Giving presents (DRY RUN) from {:?}", present_config);
                } else {
                    info!("Giving presents from {:?}", present_config);
                }
            }

            // Validate paths
            if !usr_path.exists() {
                anyhow::bail!("usr path not found: {:?}", usr_path);
            }
            if !present_config.exists() {
                anyhow::bail!("Present config not found: {:?}", present_config);
            }

            // Load present configuration
            let config = PresentConfig::from_file(&present_config)
                .map_err(|e| anyhow::anyhow!("Failed to load present config: {}", e))?;

            if quiet == 0 {
                info!(
                    "Present: container {} with {} items, target slot {}",
                    config.container.type_id,
                    config.items.len(),
                    target_slot
                );
            }

            // Find all .usr files (recursively in XX/ subdirectories)
            let usr_files = find_files_with_extension(&usr_path, "usr")?;

            if quiet == 0 {
                info!("Found {} .usr files", usr_files.len());
            }

            // Process files and collect results
            let results: Vec<GiftResult> = usr_files
                .par_iter()
                .map(|path| apply_present_to_file(path, &config, target_slot, dry_run))
                .collect();

            // Aggregate summary
            let mut summary = GiftSummary::new();
            for result in &results {
                summary.add_result(result);

                // Log individual results based on quiet level
                match result {
                    GiftResult::Gifted { player_name } => {
                        if quiet == 0 {
                            info!("Gifted: {}", player_name);
                        }
                    }
                    GiftResult::SlotOccupied { player_name } => {
                        if quiet == 0 {
                            info!("Skipped (slot occupied): {}", player_name);
                        }
                    }
                    GiftResult::Error { player_name, error } => {
                        if quiet < 2 {
                            tracing::warn!("Error for {}: {}", player_name, error);
                        }
                    }
                }
            }

            // Print summary
            if quiet == 0 {
                info!("--- Summary ---");
                info!("Total processed: {}", summary.total_processed);
                info!("Gifted: {}", summary.gifted);
                info!("Skipped (slot occupied): {}", summary.skipped);
                info!("Errors: {}", summary.errors);
                if dry_run {
                    info!("(DRY RUN - no files were modified)");
                }
            }
        }
    }

    info!("Demonax CLI finished");
    Ok(())
}
