//! Creature boosting system for daily random creature selection.
//!
//! This module handles:
//! - Random creature selection with filtering
//! - Backup and restore operations
//! - Image file copying for web display

use crate::error::{DemonaxError, Result};
use crate::parsers::parse_mon_file;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for creature selection
#[derive(Debug, Clone)]
pub struct CreatureSelector {
    pub excluded_names: Vec<&'static str>,
    pub min_exp: i32,
}

impl Default for CreatureSelector {
    fn default() -> Self {
        Self {
            excluded_names: vec![
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
                "human.mon",
                "gamemaster.mon",
            ],
            min_exp: 50,
        }
    }
}

/// Select a random creature that meets the criteria
pub fn select_random_creature(
    mon_dir: &Path,
    selector: &CreatureSelector,
) -> Result<PathBuf> {
    if !mon_dir.exists() {
        return Err(DemonaxError::NotFound(format!(
            "Monster directory not found: {}",
            mon_dir.display()
        )));
    }

    // Get all .mon files
    let mut mon_files: Vec<PathBuf> = fs::read_dir(mon_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "mon" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Filter out excluded creatures
    mon_files.retain(|path| {
        let filename = path.file_name().unwrap().to_str().unwrap();
        !selector.excluded_names.contains(&filename)
    });

    if mon_files.is_empty() {
        return Err(DemonaxError::NotFound(
            "No eligible monster files found".to_string(),
        ));
    }

    // Try to find a valid creature (retry logic)
    let max_attempts = 100;
    for attempt in 0..max_attempts {
        // Use system time as seed for randomness
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let selected_path = mon_files
            .choose(&mut rng)
            .ok_or_else(|| DemonaxError::NotFound("No monster files to choose from".to_string()))?;

        // Parse and check criteria
        match parse_mon_file(selected_path) {
            Ok(creature) => {
                if creature.has_loot
                    && creature.experience > selector.min_exp
                    && creature.creature_type != "Boss"
                {
                    return Ok(selected_path.clone());
                }
            }
            Err(_) => {
                // Skip creatures that fail to parse
            }
        }

        // Sleep briefly before next attempt to ensure different seed
        if attempt < max_attempts - 1 {
            thread::sleep(Duration::from_millis(100));
        }
    }

    Err(DemonaxError::NotFound(format!(
        "Could not find eligible creature after {} attempts",
        max_attempts
    )))
}

/// Restore the previously boosted creature, if any
///
/// Returns the path to the restored creature file, or None if no backup exists
pub fn restore_previous_boost(game_path: &Path) -> Result<Option<PathBuf>> {
    let backup_dir = game_path.join("boosted_original");

    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)?;
        return Ok(None);
    }

    // Find any .mon file in the backup directory
    let backup_files: Vec<PathBuf> = fs::read_dir(&backup_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "mon" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if backup_files.is_empty() {
        return Ok(None);
    }

    // There should only be one backup file, but handle the case of multiple
    for backup_path in backup_files {
        let filename = backup_path
            .file_name()
            .ok_or_else(|| DemonaxError::Parse("Invalid backup filename".to_string()))?;

        let restore_path = game_path.join("mon").join(filename);

        // Move the backup file back to the mon directory (overwriting)
        fs::rename(&backup_path, &restore_path)?;

        return Ok(Some(restore_path));
    }

    Ok(None)
}

/// Backup a creature file before modification
pub fn backup_creature_file(source: &Path, game_path: &Path) -> Result<PathBuf> {
    let backup_dir = game_path.join("boosted_original");

    // Create backup directory if it doesn't exist
    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)?;
    }

    // Check if there's already a backup
    let existing_backups: Vec<PathBuf> = fs::read_dir(&backup_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "mon" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if !existing_backups.is_empty() {
        return Err(DemonaxError::Validation(
            "Cannot create backup: previous boost not restored. Run restore-boosted-creature first.".to_string(),
        ));
    }

    let filename = source
        .file_name()
        .ok_or_else(|| DemonaxError::Parse("Invalid source filename".to_string()))?;

    let backup_path = backup_dir.join(filename);
    fs::copy(source, &backup_path)?;

    Ok(backup_path)
}

/// Copy creature image to website location
pub fn copy_creature_image(
    race_id: i32,
    source_dir: &Path,
    dest_path: &Path,
) -> Result<()> {
    let source_file = source_dir.join(format!("{}.png", race_id));

    if !source_file.exists() {
        // Log warning but don't fail
        tracing::warn!(
            "Source image not found: {}. Skipping image copy.",
            source_file.display()
        );
        return Ok(());
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = dest_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::copy(&source_file, dest_path)?;

    Ok(())
}

/// Information about a boosted creature
#[derive(Debug, Clone)]
pub struct BoostInfo {
    pub creature_name: String,
    pub short_name: String,
    pub race_id: i32,
    pub original_exp: i32,
    pub boosted_exp: i32,
    pub loot_items_count: i32,
}
