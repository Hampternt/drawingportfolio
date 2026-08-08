//! Last Call's placeholder card catalog — 20 cards, deliberately adversarial
//! (spec §9), not tidy. Exercises every title-length band the §7.5 ramp
//! branches on, an overflowing body, and both a zero- and six-keyword card,
//! so the rendering rules in later tasks are proven against real content
//! rather than synthetic test fixtures.

use crate::last_call::{Card, CardKind, Deck};

pub struct CardDef {
    pub id: &'static str,
    pub deck: Deck,
    pub kind: CardKind,
    pub cost: u8,
    pub targets: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub keywords: &'static [&'static str],
}

const KW6: &[&str] = &["burst", "loud", "public", "delayed", "stacking", "reactive"];
const KW3: &[&str] = &["slow", "control", "single"];
const NONE: &[&str] = &[];

/// A 149-character body — over BODY_CLAMP_CHARS (108), so it is the one card
/// that proves the 3-line clamp and the `data-expandable` marking against
/// rendered content rather than a test fixture.
const LONG_BODY: &str = "Placeholder. A slow, expensive problem that takes several \
lines to explain properly, which is exactly the point of it existing in a \
catalog that is otherwise far too tidy.";

pub const CATALOG: [CardDef; 20] = [
    // Beer — attrition, costs 1-2
    CardDef {
        id: "beer-01",
        deck: Deck::Beer,
        kind: CardKind::Atk,
        cost: 1,
        targets: "one",
        keywords: NONE,
        title: "Nudge", // 5
        text: "Placeholder. A small, boring hit.",
    },
    CardDef {
        id: "beer-02",
        deck: Deck::Beer,
        kind: CardKind::Atk,
        cost: 2,
        targets: "one",
        keywords: KW3,
        title: "Grind", // 5
        text: "Placeholder. A slightly less small hit.",
    },
    CardDef {
        id: "beer-03",
        deck: Deck::Beer,
        kind: CardKind::Buff,
        cost: 1,
        targets: "self",
        keywords: NONE,
        title: "Second Wind", // 11
        text: "Placeholder. You feel marginally better.",
    },
    CardDef {
        id: "beer-04",
        deck: Deck::Beer,
        kind: CardKind::Util,
        cost: 2,
        targets: "self",
        keywords: NONE,
        title: "Top Up, Then Top Up Again", // 25
        text: "Placeholder. Something happens to your vessel.",
    },
    // Cider — trickster, costs 1-3
    CardDef {
        id: "cider-01",
        deck: Deck::Cider,
        kind: CardKind::Curse,
        cost: 1,
        targets: "one",
        keywords: NONE,
        title: "Sticky", // 6
        text: "Placeholder. Something inconvenient, later.",
    },
    CardDef {
        id: "cider-02",
        deck: Deck::Cider,
        kind: CardKind::Util,
        cost: 2,
        targets: "all",
        keywords: NONE,
        title: "Shuffle", // 7
        text: "Placeholder. Everything moves one to the left.",
    },
    CardDef {
        id: "cider-03",
        deck: Deck::Cider,
        kind: CardKind::Reaction,
        cost: 2,
        targets: "one",
        keywords: KW3,
        title: "Not So Fast, Friend", // 19
        text: "Placeholder. A reaction, once reactions exist.",
    },
    CardDef {
        id: "cider-04",
        deck: Deck::Cider,
        kind: CardKind::Atk,
        cost: 3,
        targets: "one",
        keywords: KW6,
        title: "Windfall", // 8
        text: "Placeholder. A real hit, for a real price.",
    },
    // Wine — control, costs 2-3
    CardDef {
        id: "wine-01",
        deck: Deck::Wine,
        kind: CardKind::Curse,
        cost: 2,
        targets: "one",
        keywords: NONE,
        title: "Decant", // 6
        text: LONG_BODY,
    },
    CardDef {
        id: "wine-02",
        deck: Deck::Wine,
        kind: CardKind::Util,
        cost: 2,
        targets: "all",
        keywords: NONE,
        title: "House Rules Amendment", // 21
        text: "Placeholder. The table agrees to something.",
    },
    CardDef {
        id: "wine-03",
        deck: Deck::Wine,
        kind: CardKind::Buff,
        cost: 3,
        targets: "self",
        keywords: NONE,
        title: "Vintage", // 7
        text: "Placeholder. You are briefly untouchable.",
    },
    CardDef {
        id: "wine-04",
        deck: Deck::Wine,
        kind: CardKind::Atk,
        cost: 3,
        targets: "one",
        keywords: NONE,
        title: "Corked", // 6
        text: "Placeholder. Control, delivered as damage.",
    },
    // Liquor — burst, costs 2-3
    CardDef {
        id: "liquor-01",
        deck: Deck::Liquor,
        kind: CardKind::Atk,
        cost: 2,
        targets: "one",
        keywords: NONE,
        title: "Shot Called", // 11
        text: "Placeholder. Loud and immediate.",
    },
    CardDef {
        id: "liquor-02",
        deck: Deck::Liquor,
        kind: CardKind::Atk,
        cost: 3,
        targets: "one",
        keywords: NONE,
        title: "Double", // 6
        text: "Placeholder. Louder and more immediate.",
    },
    CardDef {
        id: "liquor-03",
        deck: Deck::Liquor,
        kind: CardKind::Curse,
        cost: 2,
        targets: "one",
        keywords: NONE,
        title: "Hangover", // 8
        text: "Placeholder. Payable next round.",
    },
    CardDef {
        id: "liquor-04",
        deck: Deck::Liquor,
        kind: CardKind::Util,
        cost: 3,
        targets: "self",
        keywords: NONE,
        title: "Neat, No Ice, No Mercy", // 22
        text: "Placeholder. Fewer pulls, more effect.",
    },
    // Soft — support, costs 1-2
    CardDef {
        id: "soft-01",
        deck: Deck::Soft,
        kind: CardKind::Buff,
        cost: 1,
        targets: "one",
        keywords: NONE,
        title: "Water Round", // 11
        text: "Placeholder. Someone feels better.",
    },
    CardDef {
        id: "soft-02",
        deck: Deck::Soft,
        kind: CardKind::Util,
        cost: 1,
        targets: "one",
        keywords: NONE,
        title: "Designated", // 10
        text: "Placeholder. You take it for them.",
    },
    CardDef {
        id: "soft-03",
        deck: Deck::Soft,
        kind: CardKind::Buff,
        cost: 2,
        targets: "all",
        keywords: NONE,
        title: "Snack Table", // 11
        text: "Placeholder. Everyone feels better.",
    },
    CardDef {
        id: "soft-04",
        deck: Deck::Soft,
        kind: CardKind::Reaction,
        cost: 2,
        targets: "self",
        keywords: NONE,
        title: "The Long Sober Look Across The Table", // 36
        text: "Placeholder. A reaction, once reactions exist.",
    },
];

fn to_card(def: &CardDef) -> Card {
    Card {
        id: def.id.to_string(),
        deck: def.deck,
        kind: def.kind,
        cost: def.cost,
        targets: def.targets.to_string(),
        title: def.title.to_string(),
        text: def.text.to_string(),
        keywords: def.keywords.iter().map(|s| s.to_string()).collect(),
        duration: None,
    }
}

/// The four `Card`s for `deck`, in catalog order.
pub fn deck_cards(deck: Deck) -> Vec<Card> {
    CATALOG
        .iter()
        .filter(|def| def.deck == deck)
        .map(to_card)
        .collect()
}

pub fn card_by_id(id: &str) -> Option<Card> {
    CATALOG.iter().find(|def| def.id == id).map(to_card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_catalog_costs_match_deck_spread() {
        let spreads: [(Deck, std::ops::RangeInclusive<u8>); 5] = [
            (Deck::Beer, 1..=2),
            (Deck::Cider, 1..=3),
            (Deck::Wine, 2..=3),
            (Deck::Liquor, 2..=3),
            (Deck::Soft, 1..=2),
        ];
        for (deck, range) in spreads {
            for def in CATALOG.iter().filter(|d| d.deck == deck) {
                assert!(
                    range.contains(&def.cost),
                    "{} cost {} outside {:?} spread",
                    def.id,
                    def.cost,
                    range
                );
            }
            assert_eq!(deck_cards(deck).len(), 4);
        }

        let ids: HashSet<&str> = CATALOG.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), CATALOG.len());
    }

    #[test]
    fn test_catalog_covers_every_title_band() {
        let short = CATALOG
            .iter()
            .filter(|d| d.title.chars().count() <= 14)
            .count();
        let mid = CATALOG
            .iter()
            .filter(|d| (15..=24).contains(&d.title.chars().count()))
            .count();
        let long = CATALOG
            .iter()
            .filter(|d| d.title.chars().count() > 24)
            .count();

        assert_eq!(short, 15);
        assert_eq!(mid, 3);
        assert_eq!(long, 2);
    }

    #[test]
    fn test_catalog_has_an_overflowing_body() {
        let overflowing: Vec<&str> = CATALOG
            .iter()
            .filter(|d| d.text.chars().count() > 108)
            .map(|d| d.id)
            .collect();
        assert_eq!(overflowing, vec!["wine-01"]);
    }

    #[test]
    fn test_catalog_has_zero_three_and_six_keyword_cards() {
        assert!(CATALOG.iter().any(|d| d.keywords.is_empty()));
        assert!(CATALOG.iter().any(|d| d.keywords.len() == 3));
        let six: Vec<&str> = CATALOG
            .iter()
            .filter(|d| d.keywords.len() == 6)
            .map(|d| d.id)
            .collect();
        assert_eq!(six, vec!["cider-04"]);
    }

    #[test]
    fn test_catalog_titles_use_char_counts_not_bytes() {
        for def in CATALOG.iter() {
            assert_eq!(
                def.title.len(),
                def.title.chars().count(),
                "{} title is not pure ASCII",
                def.id
            );
        }
    }
}
