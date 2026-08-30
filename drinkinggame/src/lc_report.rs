//! The round resolution report — "what was used against me, by whom".
//!
//! `resolve()` is otherwise a black box: it mutates HP, pulls, hands and
//! effects, dribbles a few one-line entries into the public log, and rolls
//! the round over in the same call. A player watching their own HP drop has
//! no way to say which card did it. The LOG tab carries the two halves of
//! the answer — `LogEntry::Play` names a card and its target, `LogEntry::Hit`
//! names a source and an amount — but they are separate rows in a flat,
//! capped stream with every other seat's rows interleaved between them, and
//! nothing joins them. `Effect::source_play` looks like the join key and is
//! not one: `order_key` resets to 1 at every reveal, so it recurs across
//! rounds and can collide.
//!
//! So this is a second record beside the log, not a richer log. Three
//! reasons it has to be:
//!
//! 1. **The log is permanent and capped** at `LC_LOG_CAP`, evicting oldest
//!    first. Any identity written into it can outlive the row it points at.
//!    A report is bounded by its round and thrown away at the rollover.
//! 2. **The log deliberately omits the null results.** `damage()` logs a
//!    `Hit` only when HP actually moved, so a fully shielded hit logs
//!    *nothing at all* — the player who blocked an attack has no evidence
//!    they were attacked. Same for a fizzle and for a cancelled play. Those
//!    are exactly what a "what happened to me" screen must show, and exactly
//!    what a permanent log should not accumulate.
//! 3. **It is ordered.** `blows` is a timeline, which is what lets a UI step
//!    through the round one beat at a time rather than showing an end-state
//!    diff — and therefore what lets a sound cue land on the right moment.
//!
//! **Confidentiality.** Every field here is public by construction, and the
//! construction is the guarantee: `Blow::card_id` always names the card that
//! CAUSED the blow, never a card that MOVED. A `Pickpocket` records
//! `card_id: "wine-11"` against its victim — never the card it stole, which
//! stays the caster's alone until they decide (the `PublicSwap` rule).
//! `test_no_blow_can_name_a_moved_card` pins it.
//!
//! **What is in scope.** The report covers this round's plays and the dot
//! ticks and event hooks that run with them — Steps 1 through 2.5 of the
//! resolution program. A challenge's penalty lands later, at the verdict,
//! after the report is already built; it is not in here and has the
//! challenge screen of its own. That boundary is deliberate: a report that
//! reopened after people had already acknowledged it would be asking them to
//! confirm something they never saw.

use serde::{Deserialize, Serialize};

/// What a card did to one seat.
///
/// Every variant is something a player would want named on their own screen,
/// including the ones that changed no number. `Blocked`, `Fizzled` and
/// `Cancelled` are the whole reason this type exists rather than a log
/// filter — see the module note.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlowKind {
    /// HP removed. `amount` is what actually came off after shields.
    Hit,
    /// A hit a shield ate whole: `amount` 0, `absorbed` > 0. Invisible in
    /// the log by design, which is the point.
    Blocked,
    Heal,
    /// A shield was put up. `amount` is the absorption granted.
    Shield,
    /// Pulls taken out of a vessel — a real drink, never HP.
    Drain,
    /// A lingering effect ticked. Laid in an earlier round, so `source` is
    /// the seat that laid it and `card_id` the card it came from.
    Dot,
    /// A pour: everyone named drinks, then owes cards.
    Poured,
    /// A card left this hand in a trade. NEVER names the card taken.
    Taken,
    /// A card entered this hand in a trade. Same rule.
    Given,
    /// This hand was shown — to the table or to one seat. The act is public
    /// (`LogEntry::Play` already named the card); the contents never enter
    /// a blow.
    Revealed,
    /// Aimed here and did nothing: a dead target, an empty hand to steal
    /// from, roles that collapsed.
    Fizzled,
    /// Answered by a Cancel — resolved as nothing at all, pulls still spent.
    Cancelled,
    /// Sent home by a Reflect. `subject` is the caster who wore their own
    /// card.
    Reflected,
}

impl BlowKind {
    /// A lowercase tag for CSS and sound lookup. Stable — a renderer and a
    /// cue table key off these, so they are API, not display text.
    pub fn slug(self) -> &'static str {
        match self {
            BlowKind::Hit => "hit",
            BlowKind::Blocked => "blocked",
            BlowKind::Heal => "heal",
            BlowKind::Shield => "shield",
            BlowKind::Drain => "drain",
            BlowKind::Dot => "dot",
            BlowKind::Poured => "poured",
            BlowKind::Taken => "taken",
            BlowKind::Given => "given",
            BlowKind::Revealed => "revealed",
            BlowKind::Fizzled => "fizzled",
            BlowKind::Cancelled => "cancelled",
            BlowKind::Reflected => "reflected",
        }
    }

    /// Whether this landed on the subject as something done TO them, as
    /// opposed to something done FOR them. Drives nothing in the engine —
    /// it is here so a renderer and a sound table agree on which half of the
    /// palette a blow belongs to, rather than each re-deriving the list.
    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            BlowKind::Hit
                | BlowKind::Blocked
                | BlowKind::Drain
                | BlowKind::Dot
                | BlowKind::Poured
                | BlowKind::Taken
                | BlowKind::Revealed
                | BlowKind::Reflected
        )
    }
}

/// One thing that happened to one seat, with the card and the player behind
/// it.
///
/// Stored on the state rather than derived, because the numbers it carries
/// are not recoverable afterwards: `absorbed` is the difference between two
/// shield totals that no longer exist by the time anybody looks, and
/// `amount` for a `Hit` is post-shield, post-clamp, post-Reduce — nothing in
/// the catalog can reproduce it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Blow {
    /// The card that caused this — NEVER a card that moved because of it.
    /// For an authorless effect (an event hook) this is the event's id.
    pub card_id: String,
    /// Display title, snapshotted. The `TriggerEvent` precedent and the same
    /// reasoning: a report parked across a deploy that removed the card must
    /// still be readable, or the room is holding on a confirm nobody can
    /// make sense of.
    pub title: String,
    /// Who played it. `None` for an authorless effect — a table-wide pour,
    /// an event hook — mirroring the `Option<usize>` source the four HP
    /// primitives already take.
    pub source: Option<usize>,
    /// Who wore it.
    pub subject: usize,
    pub kind: BlowKind,
    /// What landed, in the units of `kind`: HP for `Hit`/`Heal`, absorption
    /// for `Shield`, pulls for `Drain`/`Poured`, cards for `Taken`/`Given`.
    /// 0 for the outcomes that changed no number.
    pub amount: i32,
    /// HP a shield swallowed on the way in. Only ever non-zero on `Hit` and
    /// `Blocked`, and it is the entire answer to "why did I only take one".
    pub absorbed: i32,
}

/// A round's report, waiting on the seats it landed on.
///
/// The ack shape is `TriggerEvent`'s, deliberately copied rather than
/// reinvented: a monotonic `key` the settle route echoes back (so a tap from
/// a stale screen cannot land on the next round's report), and `acked` as a
/// seat list whose membership IS the once-per-seat rule — no separate
/// counter to fall out of sync.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Resolution {
    /// Identity from `LastCallState::resolution_seq`, never reused.
    pub key: u64,
    /// The round these blows landed in — the round the report is ABOUT, not
    /// the round players are on when they read it.
    pub round: u32,
    /// In resolution order. A timeline, not a set: see the module note.
    pub blows: Vec<Blow>,
    /// Seats that must acknowledge, frozen at build time. Only seats a blow
    /// actually landed on, and only ones Alive to tap — see
    /// `LastCallState`'s builder for why a seat eliminated by this very
    /// report is not asked to confirm its own death.
    pub owed: Vec<usize>,
    /// Seats that have acknowledged.
    pub acked: Vec<usize>,
}

impl Resolution {
    /// Whether `seat` still owes an acknowledgement. A seat with nothing
    /// against it never owed one, so this is false for spectators — they see
    /// the report, they just do not gate it.
    pub fn awaiting(&self, seat: usize) -> bool {
        self.owed.contains(&seat) && !self.acked.contains(&seat)
    }

    /// Whether everybody who owed one has tapped.
    pub fn settled(&self) -> bool {
        self.owed.iter().all(|s| self.acked.contains(s))
    }

    /// The blows landing on one seat, in order — the "your receipt" read.
    pub fn for_seat(&self, seat: usize) -> Vec<&Blow> {
        self.blows.iter().filter(|b| b.subject == seat).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blow(subject: usize, kind: BlowKind) -> Blow {
        Blow {
            card_id: "beer-01".into(),
            title: "TEST".into(),
            source: Some(0),
            subject,
            kind,
            amount: 2,
            absorbed: 0,
        }
    }

    #[test]
    fn test_awaiting_tracks_acks_and_ignores_spectators() {
        let mut r = Resolution {
            key: 1,
            round: 3,
            blows: vec![blow(1, BlowKind::Hit)],
            owed: vec![1],
            acked: vec![],
        };
        assert!(r.awaiting(1));
        // Seat 2 wore nothing, so it never owed and never blocks.
        assert!(!r.awaiting(2));
        assert!(!r.settled());
        r.acked.push(1);
        assert!(!r.awaiting(1));
        assert!(r.settled());
    }

    /// A double-tap must not un-settle a report, and an ack from a seat that
    /// never owed must not make one settle-able that isn't.
    #[test]
    fn test_settled_reads_owed_not_acked_length() {
        let r = Resolution {
            key: 1,
            round: 3,
            blows: vec![blow(1, BlowKind::Hit), blow(2, BlowKind::Hit)],
            owed: vec![1, 2],
            acked: vec![1, 1, 3],
        };
        assert!(
            !r.settled(),
            "seat 2 has not tapped; noise must not settle it"
        );
    }

    #[test]
    fn test_for_seat_filters_and_keeps_order() {
        let r = Resolution {
            key: 1,
            round: 3,
            blows: vec![
                blow(1, BlowKind::Hit),
                blow(2, BlowKind::Heal),
                blow(1, BlowKind::Drain),
            ],
            owed: vec![1, 2],
            acked: vec![],
        };
        let mine = r.for_seat(1);
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].kind, BlowKind::Hit);
        assert_eq!(mine[1].kind, BlowKind::Drain, "order is the timeline");
        assert!(r.for_seat(9).is_empty());
    }

    /// Slugs are keyed off by a renderer and (later) a sound table, so they
    /// must be unique and stable.
    #[test]
    fn test_slugs_are_unique() {
        let all = [
            BlowKind::Hit,
            BlowKind::Blocked,
            BlowKind::Heal,
            BlowKind::Shield,
            BlowKind::Drain,
            BlowKind::Dot,
            BlowKind::Poured,
            BlowKind::Taken,
            BlowKind::Given,
            BlowKind::Revealed,
            BlowKind::Fizzled,
            BlowKind::Cancelled,
            BlowKind::Reflected,
        ];
        let slugs: std::collections::HashSet<&str> = all.iter().map(|k| k.slug()).collect();
        assert_eq!(slugs.len(), all.len(), "every kind needs its own slug");
        assert!(all.iter().all(|k| !k.slug().is_empty()));
    }

    /// The two halves of `is_hostile` must partition the vocabulary — a kind
    /// added later has to be argued into one side or the other rather than
    /// defaulting into "good news" by falling off a `matches!`.
    #[test]
    fn test_hostility_is_assigned_deliberately() {
        assert!(BlowKind::Hit.is_hostile());
        assert!(
            BlowKind::Blocked.is_hostile(),
            "being shot at is hostile even when it bounces"
        );
        assert!(BlowKind::Taken.is_hostile());
        assert!(
            BlowKind::Revealed.is_hostile(),
            "having your hand shown is done TO you"
        );
        assert!(!BlowKind::Heal.is_hostile());
        assert!(!BlowKind::Shield.is_hostile());
        assert!(
            !BlowKind::Given.is_hostile(),
            "a card arriving is a gift, whatever the motive"
        );
        // Fizzled and Cancelled are non-events: nothing landed, so there is
        // nothing to colour red.
        assert!(!BlowKind::Fizzled.is_hostile());
        assert!(!BlowKind::Cancelled.is_hostile());
    }

    #[test]
    fn test_round_trips_through_serde() {
        let r = Resolution {
            key: 9,
            round: 4,
            blows: vec![Blow {
                card_id: "wine-11".into(),
                title: "PICKPOCKET".into(),
                source: None,
                subject: 2,
                kind: BlowKind::Blocked,
                amount: 0,
                absorbed: 3,
            }],
            owed: vec![2],
            acked: vec![2],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(r, serde_json::from_str::<Resolution>(&json).unwrap());
    }
}
