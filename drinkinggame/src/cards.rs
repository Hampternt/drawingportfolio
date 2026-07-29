//! Playing-card domain: 52-card deck, shuffle, and the compact
//! "AS,2H,10D" text encoding persisted in games.deck_order.

use rand::seq::SliceRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    pub fn glyph(self) -> &'static str {
        match self {
            Suit::Spades => "\u{2660}",
            Suit::Hearts => "\u{2665}",
            Suit::Diamonds => "\u{2666}",
            Suit::Clubs => "\u{2663}",
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }

    pub fn code(self) -> char {
        match self {
            Suit::Spades => 'S',
            Suit::Hearts => 'H',
            Suit::Diamonds => 'D',
            Suit::Clubs => 'C',
        }
    }

    pub fn from_code(c: char) -> Option<Suit> {
        match c {
            'S' => Some(Suit::Spades),
            'H' => Some(Suit::Hearts),
            'D' => Some(Suit::Diamonds),
            'C' => Some(Suit::Clubs),
            _ => None,
        }
    }
}

/// rank is 1 (Ace) through 13 (King).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub rank: u8,
    pub suit: Suit,
}

const RANK_LABELS: [&str; 13] = [
    "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K",
];

impl Card {
    pub fn rank_label(self) -> &'static str {
        RANK_LABELS[(self.rank - 1) as usize]
    }

    pub fn code(self) -> String {
        format!("{}{}", self.rank_label(), self.suit.code())
    }

    pub fn from_code(s: &str) -> Option<Card> {
        if s.len() < 2 {
            return None;
        }
        let (rank_part, suit_part) = s.split_at(s.len() - 1);
        let suit = Suit::from_code(suit_part.chars().next()?)?;
        let rank = RANK_LABELS.iter().position(|&l| l == rank_part)? as u8 + 1;
        Some(Card { rank, suit })
    }
}

pub fn shuffled_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs]
        .into_iter()
        .flat_map(|suit| (1..=13).map(move |rank| Card { rank, suit }))
        .collect();
    deck.shuffle(&mut rand::thread_rng());
    deck
}

pub fn deck_to_string(deck: &[Card]) -> String {
    deck.iter().map(|c| c.code()).collect::<Vec<_>>().join(",")
}

/// Panics on malformed input — deck strings only ever come from
/// deck_to_string via the games table, so corruption is a bug, not input.
pub fn parse_deck(s: &str) -> Vec<Card> {
    s.split(',')
        .map(|code| Card::from_code(code).expect("corrupt deck_order in db"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_shuffled_deck_is_52_unique_cards() {
        let deck = shuffled_deck();
        assert_eq!(deck.len(), 52);
        let codes: HashSet<String> = deck.iter().map(|c| c.code()).collect();
        assert_eq!(codes.len(), 52);
    }

    #[test]
    fn test_deck_string_roundtrip() {
        let deck = shuffled_deck();
        let s = deck_to_string(&deck);
        assert_eq!(parse_deck(&s), deck);
        // ~150 bytes: 52 codes of 2-3 chars + 51 commas.
        assert!(s.len() < 200);
    }

    #[test]
    fn test_card_codes_and_labels() {
        let ace = Card {
            rank: 1,
            suit: Suit::Spades,
        };
        assert_eq!(ace.code(), "AS");
        assert_eq!(ace.rank_label(), "A");
        let ten = Card {
            rank: 10,
            suit: Suit::Hearts,
        };
        assert_eq!(ten.code(), "10H");
        assert_eq!(Card::from_code("10H"), Some(ten));
        assert_eq!(
            Card::from_code("QD"),
            Some(Card {
                rank: 12,
                suit: Suit::Diamonds
            })
        );
        assert_eq!(Card::from_code(""), None);
        assert_eq!(Card::from_code("XX"), None);
        assert_eq!(Card::from_code("14S"), None);
    }

    #[test]
    fn test_suit_properties() {
        assert!(Suit::Hearts.is_red());
        assert!(Suit::Diamonds.is_red());
        assert!(!Suit::Spades.is_red());
        assert!(!Suit::Clubs.is_red());
        assert_eq!(Suit::Spades.glyph(), "\u{2660}");
        assert_eq!(Suit::from_code('H'), Some(Suit::Hearts));
        assert_eq!(Suit::from_code('x'), None);
    }
}
