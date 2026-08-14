//! Last Call's tab catalog (DDv2 H7/H8/H9). A tab is a per-seat side quest —
//! a small condition to meet within a round, dealt one at a time, paying out
//! HP or pulls when settled.
//!
//! `tab_for` deals seat `seat`'s `nth` tab (0-indexed: the seat's opening
//! tab, then each replacement after a settle). `start` folds in `seat * 3`
//! — 3 is coprime with the prime `TAB_COUNT` (7), so neighbouring seats'
//! opening tabs land on different entries of `TABS` even when they share a
//! room seed, instead of every seat opening on the same tab. `step` is drawn
//! from `1..=6`, all coprime with 7, so a seat's own sequence of tabs is a
//! full 7-cycle that never re-deals the tab it just settled as its immediate
//! replacement (H7).
//!
//! H8: `LcPlayer.tabs` holds only the seat's *current* tabs (ids into
//! `TABS`) — not a history of every tab ever dealt or settled. Settling a
//! tab removes its id and deals the next one; nothing here accumulates state
//! across rounds beyond what `tabs` currently holds.
//!
//! H9's deck-fairness rule: no `TabCheck` variant keys off a card's `deck`
//! or `kind` directly (no "play a Soft card" or "play a Curse"). Soft has no
//! Curse-kind card and Beer has neither a Util-kind card nor a cost-3 card,
//! so any predicate written against deck/kind pairs would be uncompletable
//! from some decks. Every `TabCheck` here reads round-level facts instead
//! (play count, HP, hand size, pulls spent, hostility) that every deck can
//! satisfy regardless of its card mix.
//!
//! Hostility (for `NoHostilePlays`) is read from `lc_cards::card_fx` — the
//! catalog's own `EffectOp`, not any flag baked into the blob — so a retuned
//! card's hostility follows the catalog automatically.

use crate::last_call::{EffectOp, LcPlayer, Play};

pub const TAB_COUNT: usize = 7; // prime — same argument as EVENT_COUNT

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabCheck {
    NoPlays,            // lie-low
    PlaysAtLeast(u8),   // showboat (3)
    FinishedVessel,     // bottoms-up
    SpentAtLeast(u8),   // high-roller (4)
    HandAtLeast(usize), // deep-pockets (8)
    HpAtMost(i32),      // cliffhanger (5)
    NoHostilePlays,     // peacemaker: 1+ plays, none Damage/Dot/PullDrain
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabReward {
    Hp(i32),
    Pulls(u8),
}

pub struct TabDef {
    pub id: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub check: TabCheck,
    pub reward: TabReward,
}

pub const TABS: [TabDef; TAB_COUNT] = [
    TabDef {
        id: "lie-low",
        title: "LIE LOW",
        check: TabCheck::NoPlays,
        reward: TabReward::Hp(2),
        text: "Play no card for a whole round. Look innocent.",
    },
    TabDef {
        id: "showboat",
        title: "SHOWBOAT",
        check: TabCheck::PlaysAtLeast(3),
        reward: TabReward::Pulls(2),
        text: "Play three or more cards in one round. Make it look easy.",
    },
    TabDef {
        id: "bottoms-up",
        title: "BOTTOMS UP",
        check: TabCheck::FinishedVessel,
        reward: TabReward::Hp(2),
        text: "Finish a vessel and draw. The tab covers the next one.",
    },
    TabDef {
        id: "high-roller",
        title: "HIGH ROLLER",
        check: TabCheck::SpentAtLeast(4),
        reward: TabReward::Hp(2),
        text: "Spend four or more pulls on plays in a single round.",
    },
    TabDef {
        id: "deep-pockets",
        title: "DEEP POCKETS",
        check: TabCheck::HandAtLeast(8),
        reward: TabReward::Pulls(2),
        text: "End a round holding eight or more cards. Hoard politely.",
    },
    TabDef {
        id: "cliffhanger",
        title: "CLIFFHANGER",
        check: TabCheck::HpAtMost(5),
        reward: TabReward::Hp(3),
        text: "End a round alive on 5 HP or less. Live dangerously.",
    },
    TabDef {
        id: "peacemaker",
        title: "PEACEMAKER",
        check: TabCheck::NoHostilePlays,
        reward: TabReward::Hp(2),
        text: "Play at least one card in a round without hurting anyone.",
    },
];

/// A tab's definition by id — `None` for an unrecognised id (fail-soft,
/// same H3 contract as `event_def`/`card_fx`).
pub fn tab_def(id: &str) -> Option<&'static TabDef> {
    TABS.iter().find(|t| t.id == id)
}

/// Deterministic per-seat, per-slot tab dealing (H7). `seat * 3` spreads
/// neighbouring seats' opening tabs across the catalog (3 is coprime with
/// the prime `TAB_COUNT`); `step`, drawn from `1..=6` and therefore also
/// coprime with 7, walks a full cycle per seat so a replacement is never the
/// tab just settled.
pub fn tab_for(seed: u64, seat: usize, nth: usize) -> &'static TabDef {
    // 3 is coprime with 7: neighbouring seats start on different tabs.
    let start = (seed % TAB_COUNT as u64) as usize + seat * 3;
    // 1..=6, coprime with 7: a replacement is never the tab just settled.
    let step = (seed % (TAB_COUNT as u64 - 1)) as usize + 1;
    &TABS[(start + step * nth) % TAB_COUNT]
}

/// Pure predicate: did `seat` meet `check` this round? `round_plays` is the
/// round's play list captured before resolution drains it; `spent` is the
/// seat's charged pulls (event-aware); `player` is the post-resolution
/// player. Hostility is read from `card_fx` — catalog truth, not blob truth.
pub fn tab_met(
    check: &TabCheck,
    seat: usize,
    round_plays: &[Play],
    player: &LcPlayer,
    spent: u8,
) -> bool {
    let mine = || round_plays.iter().filter(|p| p.source_seat == seat);
    let hostile = |p: &&Play| {
        crate::lc_cards::card_fx(&p.card.id)
            .is_some_and(|f| matches!(f.op, EffectOp::Damage | EffectOp::Dot | EffectOp::PullDrain))
    };
    match check {
        TabCheck::NoPlays => mine().count() == 0,
        TabCheck::PlaysAtLeast(n) => mine().count() >= *n as usize,
        TabCheck::FinishedVessel => player.draws_this_round > 0,
        TabCheck::SpentAtLeast(n) => spent >= *n,
        TabCheck::HandAtLeast(n) => player.hand.len() >= *n,
        TabCheck::HpAtMost(n) => player.hp <= *n,
        TabCheck::NoHostilePlays => mine().count() >= 1 && !mine().any(|p| hostile(&p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::Status;

    #[test]
    fn test_tab_deal_is_pinned_per_seat_and_nth() {
        // H7, seed 42: start 3·seat, step 1
        assert_eq!(tab_for(42, 0, 0).id, "lie-low"); // 0
        assert_eq!(tab_for(42, 1, 0).id, "high-roller"); // 3
        assert_eq!(tab_for(42, 2, 0).id, "peacemaker"); // 6
        assert_eq!(tab_for(42, 3, 0).id, "bottoms-up"); // 9 % 7 = 2
        assert_eq!(tab_for(42, 0, 1).id, "showboat"); // alice's replacement ≠ lie-low
        for seed in 0..50u64 {
            for nth in 0..10 {
                assert_ne!(tab_for(seed, 2, nth).id, tab_for(seed, 2, nth + 1).id);
            }
        }
    }

    #[test]
    fn test_tab_catalog_shape() {
        assert_eq!(TABS.len(), 7); // prime — same H14 note as EVENTS
        let ids: std::collections::HashSet<&str> = TABS.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), TABS.len());
        for t in TABS.iter() {
            match t.reward {
                TabReward::Hp(n) => assert!((1..=5).contains(&n), "{}", t.id),
                TabReward::Pulls(n) => assert!((1..=4).contains(&n), "{}", t.id),
            }
        }
        assert!(tab_def("nope").is_none());
    }

    fn bare_player() -> LcPlayer {
        let filler = crate::lc_cards::card_by_id("beer-01").unwrap();
        LcPlayer {
            seat: 0,
            player_id: 1,
            name: "Alice".to_string(),
            hp: 15,
            handicap_pct: 100,
            vessels: Vec::new(),
            hand: vec![filler.clone(), filler.clone(), filler],
            armed: Vec::new(),
            locked: false,
            ready: false,
            mulliganed: false,
            drawing: false,
            draws_this_round: 0,
            tabs: Vec::new(),
            status: Status::Alive,
            damage_dealt: 0,
            pulls_spent: 0,
            cards_played: 0,
            elim_order: None,
            rules: Vec::new(),
        }
    }

    fn play_for(seat: usize, card_id: &str) -> Play {
        Play {
            card: crate::lc_cards::card_by_id(card_id).unwrap(),
            source_seat: seat,
            target: Some(1),
            paid_from: crate::last_call::Deck::Beer,
            order_key: 1,
        }
    }

    #[test]
    fn test_tab_met_predicates() {
        let player = bare_player();
        let round_plays = vec![play_for(0, "beer-01"), play_for(0, "soft-01")];

        // NoPlays: false for seat 0 (2 plays), true for seat 1 (no plays).
        assert!(!tab_met(&TabCheck::NoPlays, 0, &round_plays, &player, 0));
        assert!(tab_met(&TabCheck::NoPlays, 1, &round_plays, &player, 0));

        // PlaysAtLeast: false at 2 plays for 3, true for 2.
        assert!(!tab_met(
            &TabCheck::PlaysAtLeast(3),
            0,
            &round_plays,
            &player,
            0
        ));
        assert!(tab_met(
            &TabCheck::PlaysAtLeast(2),
            0,
            &round_plays,
            &player,
            0
        ));

        // FinishedVessel: false at draws_this_round 0; true once it's > 0.
        assert!(!tab_met(
            &TabCheck::FinishedVessel,
            0,
            &round_plays,
            &player,
            0
        ));
        let mut drawn = bare_player();
        drawn.draws_this_round = 5;
        assert!(tab_met(
            &TabCheck::FinishedVessel,
            0,
            &round_plays,
            &drawn,
            0
        ));

        // SpentAtLeast(4): spent 3 -> false, spent 4 -> true.
        assert!(!tab_met(
            &TabCheck::SpentAtLeast(4),
            0,
            &round_plays,
            &player,
            3
        ));
        assert!(tab_met(
            &TabCheck::SpentAtLeast(4),
            0,
            &round_plays,
            &player,
            4
        ));

        // HandAtLeast(8): false at hand 3; true once hand grows to 8.
        assert!(!tab_met(
            &TabCheck::HandAtLeast(8),
            0,
            &round_plays,
            &player,
            0
        ));
        let mut big_hand = bare_player();
        let filler = crate::lc_cards::card_by_id("beer-01").unwrap();
        big_hand.hand = std::iter::repeat_n(filler, 8).collect();
        assert!(tab_met(
            &TabCheck::HandAtLeast(8),
            0,
            &round_plays,
            &big_hand,
            0
        ));

        // HpAtMost(5): false at 15; true at 5 and at 3.
        assert!(!tab_met(
            &TabCheck::HpAtMost(5),
            0,
            &round_plays,
            &player,
            0
        ));
        let mut low_hp = bare_player();
        low_hp.hp = 5;
        assert!(tab_met(&TabCheck::HpAtMost(5), 0, &round_plays, &low_hp, 0));
        low_hp.hp = 3;
        assert!(tab_met(&TabCheck::HpAtMost(5), 0, &round_plays, &low_hp, 0));

        // NoHostilePlays: false for seat 0 (beer-01 is Damage); true for a
        // round_plays holding only soft-01; false for a seat with no plays.
        assert!(!tab_met(
            &TabCheck::NoHostilePlays,
            0,
            &round_plays,
            &player,
            0
        ));
        let soft_only = vec![play_for(0, "soft-01")];
        assert!(tab_met(
            &TabCheck::NoHostilePlays,
            0,
            &soft_only,
            &player,
            0
        ));
        assert!(!tab_met(
            &TabCheck::NoHostilePlays,
            1,
            &round_plays,
            &player,
            0
        ));
    }
}
