//! Present configuration and application logic for giving items to players.
//!
//! Supports TOML configuration files like:
//! ```toml
//! [container]
//! type_id = 2854  # Present box
//!
//! [[items]]
//! type_id = 3726
//! amount = 99
//!
//! [[items]]
//! type_id = 3155
//! charges = 35
//! ```

use crate::error::{DemonaxError, Result};
use crate::file_utils::read_latin1_file;
use crate::inventory::{
    extract_inventory_section, replace_inventory_section, InventoryItem, InventorySection,
};
use encoding_rs::WINDOWS_1252;
use serde::Deserialize;
use std::path::Path;

/// Configuration for the present container.
#[derive(Debug, Deserialize, Clone)]
pub struct ContainerConfig {
    pub type_id: i32,
}

/// Configuration for an item inside the present.
#[derive(Debug, Deserialize, Clone)]
pub struct PresentItemConfig {
    pub type_id: i32,
    pub amount: Option<i32>,
    pub charges: Option<i32>,
}

/// Complete present configuration loaded from TOML.
#[derive(Debug, Deserialize, Clone)]
pub struct PresentConfig {
    pub container: ContainerConfig,
    pub items: Vec<PresentItemConfig>,
}

impl PresentConfig {
    /// Load present configuration from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            DemonaxError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to read present config from {:?}: {}", path, e),
            ))
        })?;

        Self::from_str(&content)
    }

    /// Parse present configuration from a TOML string.
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| {
            DemonaxError::Parse(format!("Failed to parse present config TOML: {}", e))
        })
    }

    /// Convert this configuration to an InventoryItem (container with contents).
    pub fn to_inventory_item(&self) -> InventoryItem {
        let contents: Vec<InventoryItem> = self
            .items
            .iter()
            .map(|item| {
                let mut inv_item = InventoryItem::new(item.type_id);
                inv_item.amount = item.amount;
                inv_item.charges = item.charges;
                inv_item
            })
            .collect();

        InventoryItem::container(self.container.type_id, contents)
    }
}

/// Result of applying a present to a player file.
#[derive(Debug, Clone)]
pub enum GiftResult {
    /// Present was successfully given.
    Gifted { player_name: String },
    /// Player already has something in the target slot.
    SlotOccupied { player_name: String },
    /// Error occurred while processing the file.
    Error { player_name: String, error: String },
}

impl GiftResult {
    pub fn is_gifted(&self) -> bool {
        matches!(self, GiftResult::Gifted { .. })
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self, GiftResult::SlotOccupied { .. })
    }

    pub fn player_name(&self) -> &str {
        match self {
            GiftResult::Gifted { player_name } => player_name,
            GiftResult::SlotOccupied { player_name } => player_name,
            GiftResult::Error { player_name, .. } => player_name,
        }
    }
}

/// Extract player name from .usr file content.
fn extract_player_name(content: &str) -> String {
    let name_re = regex::Regex::new(r#"Name\s*=\s*"([^"]+)""#).ok();
    name_re
        .and_then(|re| re.captures(content))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Apply a present to a single .usr file.
///
/// # Arguments
/// * `file_path` - Path to the .usr file
/// * `config` - Present configuration
/// * `target_slot` - Inventory slot to place the present (default: 10)
/// * `dry_run` - If true, don't modify the file
///
/// # Returns
/// A `GiftResult` indicating what happened.
pub fn apply_present_to_file(
    file_path: &Path,
    config: &PresentConfig,
    target_slot: i32,
    dry_run: bool,
) -> GiftResult {
    // Read the file
    let content = match read_latin1_file(file_path) {
        Ok(c) => c,
        Err(e) => {
            return GiftResult::Error {
                player_name: file_path.display().to_string(),
                error: format!("Failed to read file: {}", e),
            };
        }
    };

    let player_name = extract_player_name(&content);

    // Extract inventory section
    let (inv_content, _, _) = match extract_inventory_section(&content) {
        Ok(r) => r,
        Err(e) => {
            return GiftResult::Error {
                player_name,
                error: format!("Failed to extract inventory: {}", e),
            };
        }
    };

    // Parse inventory
    let mut inventory = match InventorySection::parse(&inv_content) {
        Ok(inv) => inv,
        Err(e) => {
            return GiftResult::Error {
                player_name,
                error: format!("Failed to parse inventory: {}", e),
            };
        }
    };

    // Check if slot is empty
    if !inventory.is_slot_empty(target_slot) {
        return GiftResult::SlotOccupied { player_name };
    }

    // Add present to inventory
    let present_item = config.to_inventory_item();
    inventory.set_slot(target_slot, present_item);

    // Serialize new inventory
    let new_inventory = inventory.serialize();

    // Replace inventory section in file content
    let new_content = match replace_inventory_section(&content, &new_inventory) {
        Ok(c) => c,
        Err(e) => {
            return GiftResult::Error {
                player_name,
                error: format!("Failed to replace inventory: {}", e),
            };
        }
    };

    // Write file (unless dry run)
    if !dry_run {
        // Encode back to Windows-1252 (Latin-1)
        let (encoded, _, had_errors) = WINDOWS_1252.encode(&new_content);
        if had_errors {
            return GiftResult::Error {
                player_name,
                error: "Failed to encode file content to Windows-1252".to_string(),
            };
        }

        if let Err(e) = std::fs::write(file_path, &*encoded) {
            return GiftResult::Error {
                player_name,
                error: format!("Failed to write file: {}", e),
            };
        }
    }

    GiftResult::Gifted { player_name }
}

/// Summary of gift distribution results.
#[derive(Debug, Default)]
pub struct GiftSummary {
    pub total_processed: usize,
    pub gifted: usize,
    pub skipped: usize,
    pub errors: usize,
}

impl GiftSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_result(&mut self, result: &GiftResult) {
        self.total_processed += 1;
        match result {
            GiftResult::Gifted { .. } => self.gifted += 1,
            GiftResult::SlotOccupied { .. } => self.skipped += 1,
            GiftResult::Error { .. } => self.errors += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_present_config() {
        let toml = r#"
[container]
type_id = 2854

[[items]]
type_id = 3726
amount = 99

[[items]]
type_id = 3155
charges = 35
"#;

        let config = PresentConfig::from_str(toml).unwrap();
        assert_eq!(config.container.type_id, 2854);
        assert_eq!(config.items.len(), 2);
        assert_eq!(config.items[0].type_id, 3726);
        assert_eq!(config.items[0].amount, Some(99));
        assert_eq!(config.items[1].type_id, 3155);
        assert_eq!(config.items[1].charges, Some(35));
    }

    #[test]
    fn test_config_to_inventory_item() {
        let toml = r#"
[container]
type_id = 2854

[[items]]
type_id = 3726
amount = 99

[[items]]
type_id = 3155
charges = 35
"#;

        let config = PresentConfig::from_str(toml).unwrap();
        let item = config.to_inventory_item();

        assert_eq!(item.type_id, 2854);
        assert_eq!(item.contents.len(), 2);
        assert_eq!(item.contents[0].type_id, 3726);
        assert_eq!(item.contents[0].amount, Some(99));
        assert_eq!(item.contents[1].type_id, 3155);
        assert_eq!(item.contents[1].charges, Some(35));
    }

    #[test]
    fn test_inventory_item_serialization() {
        let toml = r#"
[container]
type_id = 2854

[[items]]
type_id = 3726
amount = 99

[[items]]
type_id = 3155
charges = 35
"#;

        let config = PresentConfig::from_str(toml).unwrap();
        let item = config.to_inventory_item();
        let serialized = item.serialize();

        assert_eq!(
            serialized,
            "2854 Content={3726 Amount=99, 3155 Charges=35}"
        );
    }
}
