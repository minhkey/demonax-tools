//! Monster file writer for boosting creature stats.
//!
//! This module handles line-by-line modification of .mon files while preserving
//! the exact formatting and Latin1 encoding.

use crate::error::{DemonaxError, Result};
use crate::file_utils::read_latin1_file;
use encoding_rs::WINDOWS_1252;
use regex::Regex;
use std::fs;
use std::path::Path;

/// Configuration for creature boost operations
#[derive(Debug, Clone)]
pub struct BoostConfig {
    pub exp_multiplier: f64,
    pub loot_multiplier: f64,
    pub max_loot_chance: i32,
}

impl Default for BoostConfig {
    fn default() -> Self {
        Self {
            exp_multiplier: 4.0,
            loot_multiplier: 2.0,
            max_loot_chance: 999,
        }
    }
}

/// Apply boost modifications to a .mon file
pub fn apply_boost_to_file(file_path: &Path, config: &BoostConfig) -> Result<()> {
    // Read the file with Latin1 encoding
    let text = read_latin1_file(file_path)?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    // Boost experience
    boost_experience(&mut lines, config.exp_multiplier)?;

    // Boost loot inventory
    boost_loot_inventory(&mut lines, config.loot_multiplier, config.max_loot_chance)?;

    // Write back with Latin1 encoding
    write_latin1_file(file_path, &lines)?;

    Ok(())
}

/// Boost the Experience line
fn boost_experience(lines: &mut Vec<String>, multiplier: f64) -> Result<()> {
    let exp_regex = Regex::new(r"^Experience\s*=\s*(\d+)").unwrap();

    for line in lines.iter_mut() {
        if let Some(captures) = exp_regex.captures(line) {
            let current_exp: i32 = captures
                .get(1)
                .unwrap()
                .as_str()
                .parse()
                .map_err(|_| DemonaxError::Parse("Invalid experience value".to_string()))?;

            let new_exp = (current_exp as f64 * multiplier) as i32;
            *line = format!("Experience = {}", new_exp);
            break;
        }
    }

    Ok(())
}

/// Boost the loot inventory chances
fn boost_loot_inventory(
    lines: &mut Vec<String>,
    multiplier: f64,
    max_chance: i32,
) -> Result<()> {
    // Find the Inventory section
    let mut inventory_start: Option<usize> = None;
    let mut inventory_end: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("Inventory = {") {
            inventory_start = Some(idx);
            break;
        }
    }

    let start = match inventory_start {
        Some(idx) => idx,
        None => return Ok(()), // No inventory, nothing to boost
    };

    // Check if it's an empty inventory
    if lines[start].trim().ends_with("{}") {
        return Ok(()); // Empty inventory, nothing to boost
    }

    // Find the end of the inventory section
    // Look for either a blank line or the closing brace on its own line
    for idx in (start + 1)..lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.is_empty() || trimmed == "}" {
            inventory_end = Some(idx - 1);
            break;
        }
    }

    // If we didn't find an end, assume it goes to the last line
    let end = inventory_end.unwrap_or(lines.len() - 1);

    // Boost each loot entry
    for idx in start..=end {
        lines[idx] = boost_loot_line(&lines[idx], multiplier, max_chance)?;
    }

    Ok(())
}

/// Boost a single loot line, preserving its exact format
fn boost_loot_line(line: &str, multiplier: f64, max_chance: i32) -> Result<String> {
    // Pattern: (item_id, max_amount, chance_raw),
    // or:      (item_id, max_amount, chance_raw)}
    let tuple_regex = Regex::new(r"\((\d+),\s*(\d+),\s*(\d+)\)([,}])").unwrap();

    let result = tuple_regex.replace(line, |caps: &regex::Captures| {
        let item_id = &caps[1];
        let max_amount = &caps[2];
        let chance: i32 = caps[3].parse().unwrap_or(0);
        let ending = &caps[4];

        let new_chance = ((chance as f64 * multiplier) as i32).min(max_chance);

        format!("({}, {}, {}){}", item_id, max_amount, new_chance, ending)
    });

    Ok(result.to_string())
}

/// Write lines to a file with Latin1 encoding
fn write_latin1_file(path: &Path, lines: &[String]) -> Result<()> {
    let text = lines.join("\n");
    let (encoded, _, had_errors) = WINDOWS_1252.encode(&text);

    if had_errors {
        return Err(DemonaxError::Parse(
            "Failed to encode text as Latin1".to_string(),
        ));
    }

    fs::write(path, &*encoded)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boost_experience_line() {
        let mut lines = vec![
            "Name = Dragon".to_string(),
            "Experience = 120".to_string(),
            "HP = 1000".to_string(),
        ];

        boost_experience(&mut lines, 4.0).unwrap();

        assert_eq!(lines[1], "Experience = 480");
    }

    #[test]
    fn test_boost_loot_single_line() {
        let line = "(3031, 52, 400),";
        let result = boost_loot_line(line, 2.0, 999).unwrap();
        assert_eq!(result, "(3031, 52, 800),");
    }

    #[test]
    fn test_boost_loot_capping() {
        let line = "(3031, 52, 600),";
        let result = boost_loot_line(line, 2.0, 999).unwrap();
        assert_eq!(result, "(3031, 52, 999),");
    }

    #[test]
    fn test_boost_loot_last_line() {
        let line = "(3031, 52, 400)}";
        let result = boost_loot_line(line, 2.0, 999).unwrap();
        assert_eq!(result, "(3031, 52, 800)}");
    }

    #[test]
    fn test_boost_loot_with_different_spacing() {
        let line = "(3031,52,400),";
        let result = boost_loot_line(line, 2.0, 999).unwrap();
        assert_eq!(result, "(3031, 52, 800),");
    }
}
