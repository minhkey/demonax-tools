//! Equipment rendering module for generating player equipment images.

use crate::error::{DemonaxError, Result};
use crate::models::PlayerSnapshot;
use image::{RgbaImage, imageops, open};
use std::path::{Path, PathBuf};

/// Equipment slot positions (x, y) on the template image
/// Based on coordinates from render_equipment.sh
const EQUIPMENT_POSITIONS: [(i32, i32); 10] = [
    (40, 2),    // Slot 0: helmet
    (3, 17),    // Slot 1: neck
    (77, 17),   // Slot 2: bp (backpack)
    (40, 40),   // Slot 3: armor
    (77, 53),   // Slot 4: right_hand
    (3, 54),    // Slot 5: left_hand
    (40, 77),   // Slot 6: legs
    (40, 114),  // Slot 7: boots
    (3, 91),    // Slot 8: ring
    (77, 90),   // Slot 9: arrows
];

/// Configuration for equipment rendering
pub struct RenderConfig {
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
    pub template_path: PathBuf,
    pub blank_path: PathBuf,
}

/// Load an item image from the data directory
fn load_item_image(data_dir: &Path, item_id: i32) -> Result<RgbaImage> {
    let item_path = data_dir.join(format!("{}.png", item_id));

    if !item_path.exists() {
        return Err(DemonaxError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Item image not found: {}", item_path.display()),
        )));
    }

    let img = open(&item_path)
        .map_err(|e| DemonaxError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to load item image {}: {}", item_path.display(), e),
        )))?
        .to_rgba8();

    Ok(img)
}

/// Render equipment for a single player
pub fn render_player_equipment(
    snapshot: &PlayerSnapshot,
    config: &RenderConfig,
    template: &RgbaImage,
    blank: &RgbaImage,
    quiet: u8,
) -> Result<PathBuf> {
    // Clone the template as the base
    let mut base = template.clone();

    // Overlay each equipment slot
    for (idx, &item_id) in snapshot.equipment.iter().enumerate() {
        if idx >= EQUIPMENT_POSITIONS.len() {
            break; // Safety check
        }

        let item_img = if item_id == -1 {
            // Empty slot, use blank image
            blank.clone()
        } else {
            // Try to load item image, fall back to blank if not found
            match load_item_image(&config.data_dir, item_id) {
                Ok(img) => img,
                Err(e) => {
                    if quiet < 2 {
                        tracing::warn!(
                            "Item {} not found for player {} (slot {}): {}. Using blank.",
                            item_id, snapshot.player_name, idx, e
                        );
                    }
                    blank.clone()
                }
            }
        };

        let (x, y) = EQUIPMENT_POSITIONS[idx];
        imageops::overlay(&mut base, &item_img, x as i64, y as i64);
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&config.output_dir)?;

    // Save the rendered equipment image
    let output_path = config.output_dir.join(format!("{}.png", snapshot.player_id));
    base.save(&output_path)
        .map_err(|e| DemonaxError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to save equipment image: {}", e),
        )))?;

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipment_positions_count() {
        assert_eq!(EQUIPMENT_POSITIONS.len(), 10, "Should have exactly 10 equipment slots");
    }
}
