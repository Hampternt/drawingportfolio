//! Ring of Fire's card ring: the face-down deck spread in a loose ring in
//! the middle of the felt (table view, 2026-10-18).
//!
//! The layout is seeded from the game id, so the phone, the big screen and
//! every reload draw the same scatter. A draw takes one card off the ring:
//! the one the player tapped (`game_draws.ring_slot`). That slot is visual
//! only — the card drawn is still the next in `deck_order`, so drawing,
//! `card_index` and the double-draw conflict retry are untouched.

use crate::seat_ring::{toward_centre, SeatPos};

/// One card per card in the deck.
pub const SLOTS: usize = 52;

/// The ring's distance out from the centre, as a fraction of the seat rim,
/// and how far a card may stray either side of it.
const RADIUS: f64 = 0.56;
const RADIUS_JITTER: f64 = 0.06;
/// Degrees a card may stray along the ring, and twist off the tangent.
const ANGLE_JITTER: f64 = 2.5;
const TURN_JITTER: f64 = 22.0;

/// A face-down card's place on the felt: centre in the table box's
/// percentages, plus how far it is turned (degrees, clockwise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingCard {
    pub slot: usize,
    pub pos: SeatPos,
    pub turn: f32,
}

/// SplitMix64: a tiny, fixed PRNG. Fixed on purpose — the scatter must
/// never change under a rand upgrade, or every running table reshuffles.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [-1, 1].
    fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// The game's scatter: all 52 slots, evenly round the ring with a little
/// jitter in angle, radius and turn.
pub fn layout(game_id: i64) -> Vec<RingCard> {
    let mut rng = SplitMix(game_id as u64 ^ 0x5249_4E47_4F46_4952); // "RINGOFIR"
    (0..SLOTS)
        .map(|slot| {
            let deg = slot as f64 * 360.0 / SLOTS as f64 + rng.signed() * ANGLE_JITTER;
            let frac = RADIUS + rng.signed() * RADIUS_JITTER;
            let turn = deg + rng.signed() * TURN_JITTER;
            RingCard {
                slot,
                pos: toward_centre(deg, frac),
                turn: ((turn * 10.0).round() / 10.0) as f32,
            }
        })
        .collect()
}

/// The slots still on the table, given each draw's tapped slot in draw
/// order. A draw with no slot (the DRAW button), an out-of-range slot or a
/// slot already gone (two phones tapping the same card at once) takes the
/// lowest slot still there, so the ring always holds exactly
/// `52 - draws` cards.
pub fn remaining(draw_slots: &[Option<i64>]) -> Vec<usize> {
    let mut on_table = [true; SLOTS];
    let mut untapped = 0;
    for slot in draw_slots {
        match slot.and_then(|s| usize::try_from(s).ok()) {
            Some(s) if s < SLOTS && on_table[s] => on_table[s] = false,
            _ => untapped += 1,
        }
    }
    for _ in 0..untapped {
        if let Some(s) = on_table.iter().position(|&t| t) {
            on_table[s] = false;
        }
    }
    (0..SLOTS).filter(|&s| on_table[s]).collect()
}

/// The face-down cards still on the table. `draw_url` makes them tappable
/// (the phone; `assets/card_ring.js` posts the nearest card's slot there);
/// the big screen has no login and gets none.
pub fn ring_html(game_id: i64, draw_slots: &[Option<i64>], draw_url: Option<&str>) -> String {
    let left = remaining(draw_slots);
    let cards: String = layout(game_id)
        .into_iter()
        .filter(|c| left.contains(&c.slot))
        .map(|c| {
            let (l, t) = c.pos;
            let sound = if draw_url.is_some() {
                r#" data-sound="card-draw""#
            } else {
                ""
            };
            format!(
                r#"<div class="ring-card" style="left:{l}%;top:{t}%;transform:translate(-50%,-50%) rotate({turn}deg)" data-slot="{slot}"{sound}></div>"#,
                turn = c.turn,
                slot = c.slot,
            )
        })
        .collect();
    let url = draw_url
        .map(|u| format!(r#" data-draw-url="{}""#, crate::render::html_escape(u)))
        .unwrap_or_default();
    format!(
        r#"<div class="card-ring" data-card-ring data-left="{n}"{url}>{cards}</div>"#,
        n = left.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_is_the_same_for_the_same_game_and_differs_between_games() {
        assert_eq!(layout(7), layout(7));
        assert_ne!(layout(7), layout(8));
        assert_eq!(layout(7).len(), SLOTS);
    }

    #[test]
    fn test_layout_stays_inside_the_felt_and_clear_of_the_seats() {
        for game in 1..40 {
            for c in layout(game) {
                let (l, t) = c.pos;
                // Seats sit on the rim (7–93 / 8.9–91.1); the ring keeps
                // well inside it, and well clear of the centre card.
                let dx = (l - 50.0) / 43.0;
                let dy = (t - 50.0) / 41.1;
                let r = (dx * dx + dy * dy).sqrt();
                assert!(
                    (0.45..=0.67).contains(&r),
                    "game {game} slot {} r {r}",
                    c.slot
                );
            }
        }
    }

    #[test]
    fn test_remaining_takes_the_tapped_slots() {
        let left = remaining(&[Some(5), Some(0)]);
        assert_eq!(left.len(), 50);
        assert!(!left.contains(&5) && !left.contains(&0));
        assert!(left.contains(&1));
    }

    #[test]
    fn test_remaining_always_matches_the_draw_count() {
        // Untapped, duplicate and out-of-range draws each still take one.
        let left = remaining(&[Some(3), None, Some(3), Some(99), Some(-1)]);
        assert_eq!(left.len(), 47);
        assert!(!left.contains(&3));
        // The four others took the lowest slots left: 0, 1, 2 and 4.
        assert_eq!(&left[..2], &[5, 6]);
        let all: Vec<Option<i64>> = (0..52).map(|_| None).collect();
        assert!(remaining(&all).is_empty());
        assert_eq!(remaining(&[]).len(), SLOTS);
    }

    #[test]
    fn test_ring_html_is_tappable_only_with_a_url() {
        let phone = ring_html(1, &[Some(0)], Some("/room/QKAM/game/draw"));
        assert_eq!(phone.matches(r#"class="ring-card""#).count(), 51);
        assert!(!phone.contains(r#"data-slot="0""#));
        assert!(phone.contains(r#"data-draw-url="/room/QKAM/game/draw""#));
        assert!(phone.contains(r#"data-left="51""#));
        let screen = ring_html(1, &[Some(0)], None);
        assert!(!screen.contains("data-draw-url") && !screen.contains("data-sound"));
    }
}
