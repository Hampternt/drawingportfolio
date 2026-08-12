//! Last Call's round-event catalog (DDv2 H3/H4). Seven table-wide effects
//! that land once per round, selected deterministically from the room seed —
//! never randomly re-rolled, so a replayed seed always deals the same
//! sequence.
//!
//! `EVENT_COUNT` is 7, a prime, on purpose (H1/H14): `event_for_round` walks
//! `EVENTS` with a `step` drawn from `1..=6` — every integer in that range is
//! coprime with 7, so no matter which step a seed produces, the walk visits
//! all seven events before repeating (a full cycle) and, because `step != 0`
//! and `step != 7`, never deals the same event in two adjacent rounds. Changing
//! `EVENT_COUNT` away from a prime breaks that guarantee and needs the H1
//! argument re-derived, not just a number bumped.
//!
//! `event_def` is fail-soft (H3): an unrecognised id resolves to `None`
//! rather than panicking, the same contract as `lc_cards::card_fx` — a
//! stale/typo'd event id degrades to "nothing happens" instead of crashing
//! the room.
//!
//! Per §10.1's guardrail (H4): every event pays out in HP, pulls, or a
//! one-round rule change — never a win condition. `EventHook` is a closed
//! enum the engine matches on; there is no hook variant that can end the
//! game.

pub const EVENT_COUNT: usize = 7; // prime — H1/H14; a test names the argument

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventHook {
    CostHalf,                        // happy-hour
    NoPlayPenalty { dmg: i32 },      // last-orders
    Toast { drain: i32, heal: i32 }, // toast — applied once at the Deal reveal
    HostileRedirect,                 // double-vision
    TopSpenderHit { dmg: i32 },      // big-shot
    DotBoost { mult: i32 },          // house-pour
    TableHeal { heal: i32 },         // on-the-house
}

pub struct EventDef {
    pub id: &'static str,
    pub title: &'static str, // UPPERCASE display name
    pub text: &'static str,  // one sentence of rules the table reads aloud
    pub hook: EventHook,
}

pub const EVENTS: [EventDef; EVENT_COUNT] = [
    EventDef {
        id: "happy-hour",
        title: "HAPPY HOUR",
        hook: EventHook::CostHalf,
        text: "Every card costs half its pulls this round, rounded up.",
    },
    EventDef {
        id: "last-orders",
        title: "LAST ORDERS",
        hook: EventHook::NoPlayPenalty { dmg: 2 },
        text: "Play at least one card this round or take 2 damage. No sitting this one out.",
    },
    EventDef {
        id: "toast",
        title: "TOAST",
        hook: EventHook::Toast { drain: 1, heal: 2 },
        text: "Everyone drinks 1 pull together and heals 2. The only moment nobody's competing.",
    },
    EventDef {
        id: "double-vision",
        title: "DOUBLE VISION",
        hook: EventHook::HostileRedirect,
        text: "Every attack hits the player left of its target this round. Aim wrong on purpose.",
    },
    EventDef {
        id: "big-shot",
        title: "BIG SHOT",
        hook: EventHook::TopSpenderHit { dmg: 2 },
        text: "The round's biggest spender takes 2 damage. Nobody likes a show-off.",
    },
    EventDef {
        id: "house-pour",
        title: "HOUSE POUR",
        hook: EventHook::DotBoost { mult: 2 },
        text: "Every curse ticks double this round. Old grudges, freshly poured.",
    },
    EventDef {
        id: "on-the-house",
        title: "ON THE HOUSE",
        hook: EventHook::TableHeal { heal: 2 },
        text: "Everyone heals 2 at the end of the round. The house is feeling generous.",
    },
];

/// An event's definition by id — `None` for an unrecognised id (fail-soft,
/// H3): a stale id resolves inert rather than panicking.
pub fn event_def(id: &str) -> Option<&'static EventDef> {
    EVENTS.iter().find(|e| e.id == id)
}

/// Deterministic per-round event selection (H1). `start` and `step` are both
/// derived from `seed`; `step` is always in `1..=6`, all coprime with the
/// prime `EVENT_COUNT`, so the resulting stepped cycle hits every event once
/// per 7 rounds and never repeats an event in adjacent rounds.
pub fn event_for_round(seed: u64, round: u32) -> &'static EventDef {
    let start = (seed % EVENT_COUNT as u64) as usize;
    // 1..=6, all coprime with 7: the cycle hits all seven events before any
    // repeat and never deals the same event in adjacent rounds (H1).
    let step = (seed % (EVENT_COUNT as u64 - 1)) as usize + 1;
    &EVENTS[(start + step * round as usize) % EVENT_COUNT]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_selection_is_pinned_and_cycles() {
        // seed 42: start 0, step 1 — rounds 1..=7 walk the table in order.
        let ids: Vec<&str> = (1..=7).map(|r| event_for_round(42, r).id).collect();
        assert_eq!(
            ids,
            vec![
                "last-orders",
                "toast",
                "double-vision",
                "big-shot",
                "house-pour",
                "on-the-house",
                "happy-hour"
            ]
        );
        // seed 0xC0FFEE: start 4, step 5 — a second, non-trivial pin.
        assert_eq!(event_for_round(0xC0FFEE, 1).id, "toast"); // (4+5)%7  = 2
        assert_eq!(event_for_round(0xC0FFEE, 2).id, "happy-hour"); // (4+10)%7 = 0
        assert_eq!(event_for_round(0xC0FFEE, 3).id, "house-pour"); // (4+15)%7 = 5
    }

    #[test]
    fn test_no_event_repeats_back_to_back() {
        // H1
        for seed in 0..50u64 {
            for round in 1..30u32 {
                assert_ne!(
                    event_for_round(seed, round).id,
                    event_for_round(seed, round + 1).id,
                    "seed={seed} round={round}"
                );
            }
        }
    }

    #[test]
    fn test_event_catalog_shape() {
        assert_eq!(EVENTS.len(), 7); // prime — H14: changing this count breaks
                                     // the coprimality argument; revisit H1 first.
        let ids: std::collections::HashSet<&str> = EVENTS.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), EVENTS.len());
        assert!(EVENTS
            .iter()
            .all(|e| !e.title.is_empty() && !e.text.is_empty()));
        assert_eq!(event_def("happy-hour").unwrap().hook, EventHook::CostHalf);
        assert!(event_def("nope").is_none()); // the fail-soft arm exists (H3)
    }
}
