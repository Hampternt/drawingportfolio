//! Ring of Fire rule sets: 13 entries (Ace..King) serialized as JSON in
//! rule_presets.rules_json and snapshotted into games.rules_json at start.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleEntry {
    pub rank: u8,
    pub title: String,
    pub text: String,
    pub holdable: bool,
}

fn entry(rank: u8, title: &str, text: &str, holdable: bool) -> RuleEntry {
    RuleEntry {
        rank,
        title: title.to_string(),
        text: text.to_string(),
        holdable,
    }
}

/// The group's standard rules, seeded as the "Standard" preset.
pub fn standard_rules() -> Vec<RuleEntry> {
    vec![
        entry(1, "Waterfall", "Everyone drinks; you may only stop when the person before you stops.", false),
        entry(2, "You", "Pick someone to drink.", false),
        entry(3, "Me", "You drink.", false),
        entry(4, "Whores", "Girls drink.", false),
        entry(5, "Thumb Master", "Hold this card. Whenever you put your thumb on the table, last to follow drinks. Spent when used.", true),
        entry(6, "Dicks", "Boys drink.", false),
        entry(7, "Heaven", "Hold this card. Whenever you point up, last to follow drinks. Spent when used.", true),
        entry(8, "Mate", "Pick a mate; they drink whenever you drink.", false),
        entry(9, "Rhyme", "Say a word; go around rhyming with it. First to fail drinks.", false),
        entry(10, "Categories", "Pick a category; go around naming things in it. First to fail drinks.", false),
        entry(11, "Make a Rule", "Invent a rule for the rest of the game. Rule-breakers drink.", false),
        entry(12, "Questions", "Ask anyone a question; they must answer with a question. First to fail drinks.", false),
        entry(13, "King's Cup", "Pour some of your drink into the King's Cup.", false),
    ]
}

pub fn standard_rules_json() -> String {
    serde_json::to_string(&standard_rules()).expect("standard rules serialize")
}

/// Panics on malformed input — rules_json only ever comes from our own
/// serialization, so corruption is a bug, not input.
pub fn parse_rules(json: &str) -> Vec<RuleEntry> {
    serde_json::from_str(json).expect("corrupt rules_json in db")
}

pub fn rule_for_rank(rules: &[RuleEntry], rank: u8) -> &RuleEntry {
    rules
        .iter()
        .find(|r| r.rank == rank)
        .expect("rules_json missing a rank")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_rules_shape() {
        let rules = standard_rules();
        assert_eq!(rules.len(), 13);
        // Ranks 1..=13 in order.
        for (i, r) in rules.iter().enumerate() {
            assert_eq!(r.rank, i as u8 + 1);
            assert!(!r.title.is_empty());
            assert!(!r.text.is_empty());
        }
        // The group's modifications and holdables.
        assert_eq!(rule_for_rank(&rules, 4).title, "Whores");
        assert_eq!(rule_for_rank(&rules, 6).title, "Dicks");
        assert!(rule_for_rank(&rules, 5).holdable); // Thumb Master
        assert!(rule_for_rank(&rules, 7).holdable); // Heaven
        assert_eq!(rules.iter().filter(|r| r.holdable).count(), 2);
    }

    #[test]
    fn test_rules_json_roundtrip() {
        let rules = standard_rules();
        let json = standard_rules_json();
        assert_eq!(parse_rules(&json), rules);
    }
}
