//! Card-triggered table events — the "salute the leader" hook.
//!
//! Distinct from `lc_events.rs`, and the two are easy to confuse. A **round
//! event** (`lc_events`) is scheduled: one per round, chosen from the seed,
//! announced at the Draw→Deal edge, applied by `resolve()`. A **trigger**
//! (here) is reactive: it fires the moment a specific card is drawn, played
//! or discarded, and it interrupts — the table has to look up and do
//! something before the game moves on.
//!
//! The machinery is the deliverable; the catalog is deliberately thin.
//! `TRIGGERS` carries one worked example (`salute`) wired to a card that
//! does not exist yet, so the whole path — definition, fire, queue,
//! acknowledge, expire — is exercised by tests without touching card
//! balance. Adding a real trigger card is then two lines here plus the card
//! itself, no engine change.
//!
//! **Fail-soft, like every other catalog lookup in this crate**
//! (`lc_cards::card_fx`, `lc_events::event_def`): a card id with no trigger
//! resolves to `None` and nothing happens. A trigger id in an old blob whose
//! definition has since been deleted renders as its stored title and
//! acknowledges normally — `TriggerEvent` carries enough to display itself,
//! so a catalog edit can never strand a live game on an event nobody can
//! dismiss.
//!
//! Confidentiality: a fired trigger is PUBLIC the instant it exists — the
//! same call as `ReactionPlay` (I9) and `Haunt` (I10), and for the same
//! reason: the point of "salute the leader" is that everybody sees it.
//! `OnDraw` is the one shape that needs care, since draws are otherwise
//! private: firing on draw tells the table you drew *that card*. That is
//! intentional and is the entire mechanic — but it means a trigger must
//! never be attached to a card whose secrecy matters, so
//! `TriggerWhen::OnDraw` should stay rare and loud.

use serde::{Deserialize, Serialize};

/// When a trigger fires.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TriggerWhen {
    /// The instant the card enters a hand. Public — see the module note.
    OnDraw,
    /// When the card is revealed as a play (beat 4).
    OnPlay,
    /// When the card hits a discard pile, however it got there.
    OnDiscard,
}

impl TriggerWhen {
    pub fn slug(self) -> &'static str {
        match self {
            TriggerWhen::OnDraw => "draw",
            TriggerWhen::OnPlay => "play",
            TriggerWhen::OnDiscard => "discard",
        }
    }
}

/// What the table is asked to do. Every variant is a real-life instruction
/// with at most a bounded state change — the `lc_events` §10.1 guardrail
/// applies here too: **no trigger may end the game.** There is deliberately
/// no `Eliminate`, no `SetHp`, and no variant carrying a win condition.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerAction {
    /// Everyone performs a gesture toward a seat; last to do it drinks.
    /// Resolved in real life — the engine only announces and records.
    Salute { target: SaluteTarget },
    /// Everybody drinks `pulls`, drawer included. Applied to vessels.
    TableDrink { pulls: u8 },
    /// The drawer alone drinks `pulls`.
    DrawerDrinks { pulls: u8 },
    /// Pure announcement — read the card aloud, no state change.
    Announce,
}

/// Who a `Salute` points at, resolved at fire time against the live table.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaluteTarget {
    /// Highest HP among the Alive; ties break to the lowest seat.
    Leader,
    /// Lowest HP among the Alive; ties break to the lowest seat.
    Loser,
    /// The player who drew or played the card.
    Drawer,
}

/// A trigger's static definition. Catalog-side, exactly like `FxDef` — never
/// stored in the blob, so a reword reaches games already in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerDef {
    /// Stable identity, stored on the fired event.
    pub id: &'static str,
    /// The card that fires it.
    pub card_id: &'static str,
    pub when: TriggerWhen,
    pub action: TriggerAction,
    /// UPPERCASE display name.
    pub title: &'static str,
    /// One sentence the table reads aloud.
    pub text: &'static str,
}

/// The trigger catalog.
///
/// Three entries, one per `TriggerWhen`, each pointing at a card id that is
/// **not** in `lc_cards::CATALOG` yet. The machinery is live and tested; the
/// content is a separate call. One per moment on purpose: a variant with no
/// entry is a variant nothing exercises, and `OnPlay`/`OnDiscard` would have
/// been dead code the first time somebody trusted them.
///
/// Wiring a real card up is: add the card to `lc_cards::CATALOG`, then point
/// a `TriggerDef` at its id. Nothing in the engine changes.
pub const TRIGGERS: [TriggerDef; 3] = [
    TriggerDef {
        id: "salute-the-leader",
        card_id: "beer-salute", // placeholder — no such card in CATALOG yet
        when: TriggerWhen::OnDraw,
        action: TriggerAction::Salute {
            target: SaluteTarget::Leader,
        },
        title: "SALUTE THE LEADER",
        text: "Everyone salute whoever's ahead. Last hand up drinks.",
    },
    TriggerDef {
        id: "round-on-the-house",
        card_id: "cider-round", // placeholder
        when: TriggerWhen::OnPlay,
        action: TriggerAction::TableDrink { pulls: 1 },
        title: "A ROUND ON THE HOUSE",
        text: "Everyone drinks one. You're paying, so make it count.",
    },
    TriggerDef {
        id: "one-for-the-road",
        card_id: "liquor-road", // placeholder
        when: TriggerWhen::OnDiscard,
        action: TriggerAction::DrawerDrinks { pulls: 1 },
        title: "ONE FOR THE ROAD",
        text: "Throwing this one away? Finish it first.",
    },
];

/// The trigger a card fires at `when`, if any. Fail-soft: an unknown card id
/// is `None`, never a panic.
pub fn trigger_for(card_id: &str, when: TriggerWhen) -> Option<&'static TriggerDef> {
    TRIGGERS
        .iter()
        .find(|t| t.card_id == card_id && t.when == when)
}

/// A trigger definition by its own id — for rendering a queued event whose
/// card is no longer to hand.
pub fn trigger_def(id: &str) -> Option<&'static TriggerDef> {
    TRIGGERS.iter().find(|t| t.id == id)
}

/// A trigger that has fired and is waiting on the table.
///
/// Carries its own `title`/`text` rather than resolving them from `id` at
/// render time. That is the deliberate exception to this crate's
/// "catalog-side text reaches in-flight games" rule (`FxDef`, `Penalty::Rule`)
/// and it buys the opposite property, which matters more here: a queued
/// event can always be displayed and dismissed even if its definition was
/// deleted by the deploy that happened while the table was mid-round.
/// A pending interrupt that nobody can read or clear would wedge the game.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TriggerEvent {
    /// Identity from `LastCallState::trigger_seq`, never reused. The ack
    /// route echoes it back, so an ack posted from a stale screen cannot
    /// land on the next queued trigger (the `ChallengeState::key` precedent).
    pub key: u64,
    /// The `TriggerDef::id` that fired.
    pub id: String,
    /// The card that fired it.
    pub card_id: String,
    pub when: TriggerWhen,
    pub action: TriggerAction,
    /// The seat that drew/played/discarded the card.
    pub source: usize,
    /// The seat the action points at — resolved at fire time, so a later
    /// HP swing doesn't retarget an announced salute.
    pub target: Option<usize>,
    pub round: u32,
    /// Snapshotted display text — see the struct note.
    pub title: String,
    pub text: String,
    /// Seats that have acknowledged. A trigger clears when every Alive seat
    /// has acked (`LastCallState::ack_trigger`); the presence of a seat here
    /// IS the once-per-seat rule, the `haunts`/`votes` pattern — no separate
    /// counter to keep in sync.
    pub acked: Vec<usize>,
}

impl TriggerEvent {
    /// Whether `seat` still owes an acknowledgement.
    pub fn awaiting(&self, seat: usize) -> bool {
        !self.acked.contains(&seat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_is_fail_soft() {
        assert!(trigger_for("no-such-card", TriggerWhen::OnDraw).is_none());
        assert!(trigger_def("no-such-trigger").is_none());
    }

    #[test]
    fn test_the_worked_example_resolves() {
        let t = trigger_for("beer-salute", TriggerWhen::OnDraw).expect("example trigger");
        assert_eq!(t.id, "salute-the-leader");
        assert_eq!(
            t.action,
            TriggerAction::Salute {
                target: SaluteTarget::Leader
            }
        );
        assert_eq!(trigger_def("salute-the-leader"), Some(t));
    }

    #[test]
    fn test_when_discriminates() {
        // The same card at a different moment is a different trigger — and
        // absent here, since the salute example only fires on draw.
        assert!(trigger_for("beer-salute", TriggerWhen::OnPlay).is_none());
        assert!(trigger_for("beer-salute", TriggerWhen::OnDiscard).is_none());
    }

    /// Every `TriggerWhen` needs at least one entry, or the engine's firing
    /// site for that moment is never exercised by anything.
    #[test]
    fn test_every_moment_has_a_worked_example() {
        for when in [
            TriggerWhen::OnDraw,
            TriggerWhen::OnPlay,
            TriggerWhen::OnDiscard,
        ] {
            assert!(
                TRIGGERS.iter().any(|t| t.when == when),
                "{when:?} has no example — its fire site is untested"
            );
        }
    }

    /// The examples must not collide with real cards: firing one in a live
    /// game would be a balance change nobody asked for.
    #[test]
    fn test_examples_point_at_no_real_card() {
        for t in TRIGGERS.iter() {
            assert!(
                crate::lc_cards::card_by_id(t.card_id).is_none(),
                "{} points at a real card",
                t.id
            );
        }
    }

    #[test]
    fn test_catalog_shape() {
        let ids: std::collections::HashSet<&str> = TRIGGERS.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), TRIGGERS.len(), "trigger ids must be unique");
        // One trigger per (card, when) pair — two would make `trigger_for`
        // silently pick the first.
        let pairs: std::collections::HashSet<(&str, TriggerWhen)> =
            TRIGGERS.iter().map(|t| (t.card_id, t.when)).collect();
        assert_eq!(pairs.len(), TRIGGERS.len());
        assert!(TRIGGERS
            .iter()
            .all(|t| !t.title.is_empty() && !t.text.is_empty()));
    }

    #[test]
    fn test_no_trigger_can_end_the_game() {
        // The §10.1 guardrail, enforced structurally: TriggerAction has no
        // variant that eliminates a player or sets a win. If a variant is
        // ever added that could, this match stops compiling and the
        // guardrail gets re-argued rather than quietly dropped.
        for t in TRIGGERS.iter() {
            match t.action {
                TriggerAction::Salute { .. }
                | TriggerAction::TableDrink { .. }
                | TriggerAction::DrawerDrinks { .. }
                | TriggerAction::Announce => {}
            }
        }
    }

    #[test]
    fn test_awaiting_tracks_acks() {
        let mut e = TriggerEvent {
            key: 1,
            id: "salute-the-leader".into(),
            card_id: "beer-salute".into(),
            when: TriggerWhen::OnDraw,
            action: TriggerAction::Announce,
            source: 0,
            target: None,
            round: 2,
            title: "T".into(),
            text: "x".into(),
            acked: vec![],
        };
        assert!(e.awaiting(0));
        e.acked.push(0);
        assert!(!e.awaiting(0));
        assert!(e.awaiting(1));
    }

    #[test]
    fn test_event_round_trips_through_serde() {
        let e = TriggerEvent {
            key: 9,
            id: "salute-the-leader".into(),
            card_id: "beer-salute".into(),
            when: TriggerWhen::OnDraw,
            action: TriggerAction::Salute {
                target: SaluteTarget::Leader,
            },
            source: 2,
            target: Some(1),
            round: 4,
            title: "SALUTE THE LEADER".into(),
            text: "...".into(),
            acked: vec![0, 2],
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(e, serde_json::from_str::<TriggerEvent>(&json).unwrap());
    }
}
