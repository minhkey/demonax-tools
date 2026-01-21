//! Harvesting rule generation for moveuse.dat files.

use crate::models::HarvestingData;
use crate::error::DemonaxError;

/// Generate a pair of MultiUse rules (success + failure) for one harvesting entry.
///
/// Success rule: Random passes - create reward, change corpse, green shimmer, increment harvesting
/// Failure rule: Random fails - just change corpse (no reward, no effect)
pub fn generate_harvesting_rule(entry: &HarvestingData) -> String {
    let success_rule = format!(
        "MultiUse, IsType(Obj1, {}), IsType(Obj2, {}), Random({}) -> Create(Obj2, {}, 0), Change(Obj2, {}, 0), Effect(User, 13), IncrementHarvestingValue(User, {}, 1)",
        entry.tool_id,
        entry.corpse_id,
        entry.percent_chance,
        entry.reward_id,
        entry.next_corpse_id,
        entry.race_id
    );

    let failure_rule = format!(
        "MultiUse, IsType(Obj1, {}), IsType(Obj2, {}) -> Change(Obj2, {}, 0)",
        entry.tool_id,
        entry.corpse_id,
        entry.next_corpse_id
    );

    format!("{}\n{}", success_rule, failure_rule)
}

/// Generate all harvesting rules from a vector of harvesting data entries.
pub fn generate_all_harvesting_rules(entries: &[HarvestingData]) -> String {
    entries
        .iter()
        .map(generate_harvesting_rule)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert harvesting rules into moveuse.dat content.
/// Replaces content between BEGIN "MultiUse" and BEGIN "Baking".
pub fn insert_harvesting_rules(moveuse_content: &str, rules: &str) -> Result<String, DemonaxError> {
    let lines: Vec<&str> = moveuse_content.lines().collect();

    // Find the line containing BEGIN "MultiUse"
    let multiuse_idx = lines
        .iter()
        .position(|line| line.contains(r#"BEGIN "MultiUse""#))
        .ok_or_else(|| DemonaxError::Parse(
            r#"Could not find BEGIN "MultiUse" in moveuse.dat"#.to_string(),
        ))?;

    // Find the line containing BEGIN "Baking"
    let baking_idx = lines
        .iter()
        .position(|line| line.contains(r#"BEGIN "Baking""#))
        .ok_or_else(|| DemonaxError::Parse(
            r#"Could not find BEGIN "Baking" in moveuse.dat"#.to_string(),
        ))?;

    if baking_idx <= multiuse_idx {
        return Err(DemonaxError::Parse(
            r#"BEGIN "Baking" appears before BEGIN "MultiUse""#.to_string(),
        ));
    }

    // Build the new content:
    // 1. Everything up to and including BEGIN "MultiUse"
    // 2. BEGIN "Harvesting"
    // 3. The new harvesting rules
    // 4. END
    // 5. Everything from BEGIN "Baking" onwards
    let mut result = Vec::new();

    // Include lines up to and including BEGIN "MultiUse"
    for line in &lines[..=multiuse_idx] {
        result.push(*line);
    }

    // Add BEGIN "Harvesting" marker
    result.push(r#"BEGIN "Harvesting""#);

    // Add the new harvesting rules
    for rule_line in rules.lines() {
        result.push(rule_line);
    }

    // Add END marker
    result.push("END");

    // Add lines from BEGIN "Baking" onwards
    for line in &lines[baking_idx..] {
        result.push(*line);
    }

    Ok(result.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_harvesting_rule() {
        let entry = HarvestingData {
            tool_id: 5544,
            corpse_id: 5317,
            next_corpse_id: 5518,
            percent_chance: 9,
            reward_id: 5366,
            race_id: 403,
        };

        let rule = generate_harvesting_rule(&entry);
        let lines: Vec<&str> = rule.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Random(9)"));
        assert!(lines[0].contains("Create(Obj2, 5366, 0)"));
        assert!(lines[0].contains("Effect(User, 13)"));
        assert!(lines[0].contains("IncrementHarvestingValue(User, 403, 1)"));
        assert!(!lines[1].contains("Random"));
        assert!(lines[1].contains("Change(Obj2, 5518, 0)"));
    }

    #[test]
    fn test_insert_harvesting_rules() {
        let moveuse_content = r#"# Header
BEGIN "MultiUse"
old rule 1
old rule 2
BEGIN "Baking"
baking rule 1
"#;

        let rules = "new rule 1\nnew rule 2";
        let result = insert_harvesting_rules(moveuse_content, rules).unwrap();

        assert!(result.contains(r#"BEGIN "MultiUse""#));
        assert!(result.contains(r#"BEGIN "Harvesting""#));
        assert!(result.contains("new rule 1"));
        assert!(result.contains("new rule 2"));
        assert!(result.contains("END"));
        assert!(result.contains(r#"BEGIN "Baking""#));
        assert!(!result.contains("old rule 1"));
        assert!(!result.contains("old rule 2"));

        // Verify order: BEGIN "Harvesting" comes after BEGIN "MultiUse"
        let multiuse_pos = result.find(r#"BEGIN "MultiUse""#).unwrap();
        let harvesting_pos = result.find(r#"BEGIN "Harvesting""#).unwrap();
        let end_pos = result.find("END").unwrap();
        let baking_pos = result.find(r#"BEGIN "Baking""#).unwrap();
        assert!(multiuse_pos < harvesting_pos);
        assert!(harvesting_pos < end_pos);
        assert!(end_pos < baking_pos);
    }

    #[test]
    fn test_insert_harvesting_rules_missing_multiuse() {
        let moveuse_content = r#"BEGIN "Baking"
rule
"#;

        let result = insert_harvesting_rules(moveuse_content, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_harvesting_rules_missing_baking() {
        let moveuse_content = r#"BEGIN "MultiUse"
rule
"#;

        let result = insert_harvesting_rules(moveuse_content, "test");
        assert!(result.is_err());
    }
}
