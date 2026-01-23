//! Inventory parsing and serialization for .usr files.
//!
//! Handles the inventory format used in player files:
//! ```text
//! Inventory   = {1 Content={3354},
//!                3 Content={2854 Content={2853, 3031 Amount=40}},
//!                10 Content={2854 Content={3449 Amount=100, 3155 Charges=35}}}
//! ```

use crate::error::{DemonaxError, Result};
use regex::Regex;

/// Represents an item in the inventory, potentially with nested contents (containers).
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryItem {
    pub type_id: i32,
    pub amount: Option<i32>,
    pub charges: Option<i32>,
    pub contents: Vec<InventoryItem>,
}

impl InventoryItem {
    /// Create a new simple item with just a type ID.
    pub fn new(type_id: i32) -> Self {
        Self {
            type_id,
            amount: None,
            charges: None,
            contents: Vec::new(),
        }
    }

    /// Create a new item with amount.
    pub fn with_amount(type_id: i32, amount: i32) -> Self {
        Self {
            type_id,
            amount: Some(amount),
            charges: None,
            contents: Vec::new(),
        }
    }

    /// Create a new item with charges.
    pub fn with_charges(type_id: i32, charges: i32) -> Self {
        Self {
            type_id,
            amount: None,
            charges: Some(charges),
            contents: Vec::new(),
        }
    }

    /// Create a container item with contents.
    pub fn container(type_id: i32, contents: Vec<InventoryItem>) -> Self {
        Self {
            type_id,
            amount: None,
            charges: None,
            contents,
        }
    }

    /// Serialize item to the inventory format string.
    pub fn serialize(&self) -> String {
        let mut result = self.type_id.to_string();

        if let Some(amount) = self.amount {
            result.push_str(&format!(" Amount={}", amount));
        }

        if let Some(charges) = self.charges {
            result.push_str(&format!(" Charges={}", charges));
        }

        if !self.contents.is_empty() {
            let contents_str: Vec<String> = self.contents.iter().map(|i| i.serialize()).collect();
            result.push_str(&format!(" Content={{{}}}", contents_str.join(", ")));
        }

        result
    }
}

/// Represents an inventory slot with its item.
#[derive(Debug, Clone)]
pub struct InventorySlot {
    pub slot_number: i32,
    pub item: InventoryItem,
}

impl InventorySlot {
    pub fn new(slot_number: i32, item: InventoryItem) -> Self {
        Self { slot_number, item }
    }

    /// Serialize slot to the inventory format string.
    pub fn serialize(&self) -> String {
        format!("{} Content={{{}}}", self.slot_number, self.item.serialize())
    }
}

/// Represents the inventory section of a .usr file.
#[derive(Debug, Clone)]
pub struct InventorySection {
    pub slots: Vec<InventorySlot>,
}

impl InventorySection {
    /// Create an empty inventory section.
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Parse inventory section from the raw text between `Inventory   = {` and the closing `}`.
    pub fn parse(inventory_content: &str) -> Result<Self> {
        let content = inventory_content.trim();

        // Empty inventory case
        if content.is_empty() {
            return Ok(Self::new());
        }

        let mut slots = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = content.chars().collect();

        while pos < chars.len() {
            // Skip whitespace and commas
            while pos < chars.len() && (chars[pos].is_whitespace() || chars[pos] == ',') {
                pos += 1;
            }
            if pos >= chars.len() {
                break;
            }

            // Parse slot number
            let slot_start = pos;
            while pos < chars.len() && chars[pos].is_ascii_digit() {
                pos += 1;
            }
            if pos == slot_start {
                // No digit found, might be end of content
                break;
            }
            let slot_str: String = chars[slot_start..pos].iter().collect();
            let slot_number: i32 = slot_str.parse().map_err(|_| {
                DemonaxError::Parse(format!("Invalid slot number: {}", slot_str))
            })?;

            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }

            // Expect "Content="
            let content_marker = "Content=";
            let remaining: String = chars[pos..].iter().collect();
            if !remaining.starts_with(content_marker) {
                return Err(DemonaxError::Parse(format!(
                    "Expected 'Content=' at position {}, found: {}",
                    pos,
                    &remaining[..remaining.len().min(20)]
                )));
            }
            pos += content_marker.len();

            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }

            // Expect '{'
            if pos >= chars.len() || chars[pos] != '{' {
                return Err(DemonaxError::Parse("Expected '{' after Content=".to_string()));
            }
            pos += 1;

            // Find matching '}'
            let content_start = pos;
            let mut brace_count = 1;
            while pos < chars.len() && brace_count > 0 {
                match chars[pos] {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
                if brace_count > 0 {
                    pos += 1;
                }
            }
            if brace_count != 0 {
                return Err(DemonaxError::Parse("Unmatched braces in inventory".to_string()));
            }

            let item_content: String = chars[content_start..pos].iter().collect();
            pos += 1; // Skip closing '}'

            let item = parse_item(&item_content)?;
            slots.push(InventorySlot::new(slot_number, item));
        }

        Ok(Self { slots })
    }

    /// Check if a specific slot is empty (not present in inventory).
    pub fn is_slot_empty(&self, slot: i32) -> bool {
        !self.slots.iter().any(|s| s.slot_number == slot)
    }

    /// Set an item in a specific slot (replaces if exists).
    pub fn set_slot(&mut self, slot: i32, item: InventoryItem) {
        // Remove existing slot if present
        self.slots.retain(|s| s.slot_number != slot);
        // Add new slot
        self.slots.push(InventorySlot::new(slot, item));
        // Sort by slot number
        self.slots.sort_by_key(|s| s.slot_number);
    }

    /// Serialize the inventory section back to the file format.
    pub fn serialize(&self) -> String {
        if self.slots.is_empty() {
            return "Inventory   = {}".to_string();
        }

        let mut result = String::from("Inventory   = {");
        for (i, slot) in self.slots.iter().enumerate() {
            if i == 0 {
                result.push_str(&slot.serialize());
            } else {
                result.push_str(",\n               ");
                result.push_str(&slot.serialize());
            }
        }
        result.push('}');
        result
    }
}

impl Default for InventorySection {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a single item from the content string (without outer braces).
fn parse_item(content: &str) -> Result<InventoryItem> {
    let content = content.trim();
    if content.is_empty() {
        return Err(DemonaxError::Parse("Empty item content".to_string()));
    }

    // Parse type_id (first number)
    let mut pos = 0;
    let chars: Vec<char> = content.chars().collect();

    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos == 0 {
        return Err(DemonaxError::Parse(format!(
            "Expected type ID at start of item: {}",
            content
        )));
    }

    let type_id_str: String = chars[..pos].iter().collect();
    let type_id: i32 = type_id_str.parse().map_err(|_| {
        DemonaxError::Parse(format!("Invalid type ID: {}", type_id_str))
    })?;

    let mut item = InventoryItem::new(type_id);
    let remaining: String = chars[pos..].iter().collect();
    let remaining = remaining.trim();

    if remaining.is_empty() {
        return Ok(item);
    }

    // Parse attributes (Amount=, Charges=, Content=)
    let amount_re = Regex::new(r"Amount\s*=\s*(\d+)").unwrap();
    let charges_re = Regex::new(r"Charges\s*=\s*(\d+)").unwrap();

    if let Some(caps) = amount_re.captures(remaining) {
        item.amount = caps.get(1).and_then(|m| m.as_str().parse().ok());
    }

    if let Some(caps) = charges_re.captures(remaining) {
        item.charges = caps.get(1).and_then(|m| m.as_str().parse().ok());
    }

    // Parse Content={...} for nested items
    if let Some(content_pos) = remaining.find("Content=") {
        let after_content = &remaining[content_pos + 8..];
        let after_content = after_content.trim();

        if after_content.starts_with('{') {
            // Find matching brace
            let chars: Vec<char> = after_content.chars().collect();
            let mut brace_count = 1;
            let mut end_pos = 1;
            while end_pos < chars.len() && brace_count > 0 {
                match chars[end_pos] {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
                if brace_count > 0 {
                    end_pos += 1;
                }
            }

            let nested_content: String = chars[1..end_pos].iter().collect();
            item.contents = parse_item_list(&nested_content)?;
        }
    }

    Ok(item)
}

/// Parse a comma-separated list of items.
fn parse_item_list(content: &str) -> Result<Vec<InventoryItem>> {
    let content = content.trim();
    if content.is_empty() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    let mut current_item = String::new();
    let mut brace_count = 0;

    for ch in content.chars() {
        match ch {
            '{' => {
                brace_count += 1;
                current_item.push(ch);
            }
            '}' => {
                brace_count -= 1;
                current_item.push(ch);
            }
            ',' if brace_count == 0 => {
                let trimmed = current_item.trim();
                if !trimmed.is_empty() {
                    items.push(parse_item(trimmed)?);
                }
                current_item.clear();
            }
            _ => {
                current_item.push(ch);
            }
        }
    }

    // Don't forget the last item
    let trimmed = current_item.trim();
    if !trimmed.is_empty() {
        items.push(parse_item(trimmed)?);
    }

    Ok(items)
}

/// Extract the inventory section from a complete .usr file content.
/// Returns the content between `Inventory   = {` and its matching `}`.
pub fn extract_inventory_section(file_content: &str) -> Result<(String, usize, usize)> {
    let inv_re = Regex::new(r"Inventory\s*=\s*\{").unwrap();

    let inv_match = inv_re.find(file_content).ok_or_else(|| {
        DemonaxError::Parse("Inventory section not found".to_string())
    })?;

    let start_pos = inv_match.start();
    let brace_start = inv_match.end();

    // Find matching closing brace
    let chars: Vec<char> = file_content[brace_start..].chars().collect();
    let mut brace_count = 1;
    let mut i = 0;
    while i < chars.len() && brace_count > 0 {
        match chars[i] {
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            _ => {}
        }
        i += 1;
    }

    if brace_count != 0 {
        return Err(DemonaxError::Parse("Unmatched braces in inventory section".to_string()));
    }

    let end_pos = brace_start + i;
    let content: String = chars[..i - 1].iter().collect();

    Ok((content, start_pos, end_pos))
}

/// Replace the inventory section in a file with new content.
pub fn replace_inventory_section(file_content: &str, new_inventory: &str) -> Result<String> {
    let (_, start_pos, end_pos) = extract_inventory_section(file_content)?;

    let mut result = String::new();
    result.push_str(&file_content[..start_pos]);
    result.push_str(new_inventory);
    result.push_str(&file_content[end_pos..]);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_inventory() {
        let inv = InventorySection::parse("").unwrap();
        assert!(inv.slots.is_empty());
    }

    #[test]
    fn test_parse_simple_inventory() {
        let content = "1 Content={3354}";
        let inv = InventorySection::parse(content).unwrap();
        assert_eq!(inv.slots.len(), 1);
        assert_eq!(inv.slots[0].slot_number, 1);
        assert_eq!(inv.slots[0].item.type_id, 3354);
    }

    #[test]
    fn test_parse_inventory_with_amount() {
        let content = "1 Content={3031 Amount=40}";
        let inv = InventorySection::parse(content).unwrap();
        assert_eq!(inv.slots[0].item.type_id, 3031);
        assert_eq!(inv.slots[0].item.amount, Some(40));
    }

    #[test]
    fn test_parse_inventory_with_charges() {
        let content = "1 Content={3155 Charges=35}";
        let inv = InventorySection::parse(content).unwrap();
        assert_eq!(inv.slots[0].item.type_id, 3155);
        assert_eq!(inv.slots[0].item.charges, Some(35));
    }

    #[test]
    fn test_parse_container_with_contents() {
        let content = "3 Content={2854 Content={2853, 3031 Amount=40}}";
        let inv = InventorySection::parse(content).unwrap();
        assert_eq!(inv.slots[0].slot_number, 3);
        assert_eq!(inv.slots[0].item.type_id, 2854);
        assert_eq!(inv.slots[0].item.contents.len(), 2);
        assert_eq!(inv.slots[0].item.contents[0].type_id, 2853);
        assert_eq!(inv.slots[0].item.contents[1].type_id, 3031);
        assert_eq!(inv.slots[0].item.contents[1].amount, Some(40));
    }

    #[test]
    fn test_serialize_simple_item() {
        let item = InventoryItem::new(3354);
        assert_eq!(item.serialize(), "3354");
    }

    #[test]
    fn test_serialize_item_with_amount() {
        let item = InventoryItem::with_amount(3031, 40);
        assert_eq!(item.serialize(), "3031 Amount=40");
    }

    #[test]
    fn test_serialize_container() {
        let item = InventoryItem::container(2854, vec![
            InventoryItem::new(2853),
            InventoryItem::with_amount(3031, 40),
        ]);
        assert_eq!(item.serialize(), "2854 Content={2853, 3031 Amount=40}");
    }

    #[test]
    fn test_serialize_empty_inventory() {
        let inv = InventorySection::new();
        assert_eq!(inv.serialize(), "Inventory   = {}");
    }

    #[test]
    fn test_is_slot_empty() {
        let mut inv = InventorySection::new();
        assert!(inv.is_slot_empty(10));

        inv.set_slot(10, InventoryItem::new(3354));
        assert!(!inv.is_slot_empty(10));
        assert!(inv.is_slot_empty(5));
    }

    #[test]
    fn test_roundtrip_complex_inventory() {
        let original = "1 Content={3354},\n               3 Content={2854 Content={2853, 3031 Amount=40}},\n               10 Content={2854 Content={3449 Amount=100, 3155 Charges=35}}";
        let inv = InventorySection::parse(original).unwrap();

        assert_eq!(inv.slots.len(), 3);
        assert_eq!(inv.slots[0].slot_number, 1);
        assert_eq!(inv.slots[1].slot_number, 3);
        assert_eq!(inv.slots[2].slot_number, 10);
    }
}
