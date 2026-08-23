//! Where the game is, and what each seat is expected to be doing — one
//! server-side answer instead of several client-side guesses.
//!
//! `Beat` (in `last_call.rs`) tracks the five-beat round cycle, but the
//! *game's* phase was never a value: "are we in the lobby" was
//! `round == 1 && beat == Beat::Draw`, "is the table parked on a challenge"
//! was `!challenges.is_empty()`, and "is it over" was `outcome().is_some()`.
//! Those three tests were re-derived at each call site — the engine's
//! guards, the routes' gating and the renderer's branches each spelled the
//! rules out again, and the renderer's copy could disagree with the
//! engine's without anything failing. `Phase` makes it one derived value
//! computed in one place.
//!
//! `SeatPhase` does the same job per seat. This is the desync fix the state
//! tracker is really for: the phone used to work out "am I waiting on the
//! table, or is the table waiting on me?" from `locked`, `ready`,
//! `drawing`, `status` and the current beat — five fields, different
//! combinations per beat. Now the server answers it, every viewer reads the
//! same answer off `PublicSeat::phase`, and the *definition* of "blocking"
//! lives next to the engine that enforces it.
//!
//! Both are DERIVED — neither is stored in the blob, so neither can go
//! stale or disagree with the fields it reads. `phase()` and `seat_phase()`
//! on `LastCallState` are the only constructors.

use serde::{Deserialize, Serialize};

/// The game's top-level state. Ordered by progression, so `<`/`>`
/// comparisons read naturally (`phase >= Phase::Playing` = "the game has
/// started").
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Round 1's Draw beat: the registration lobby. Players pick vessels and
    /// handicaps, mulligan freely, and nothing can damage anybody. The
    /// untimed beat (`beat_deadline_ms == None`).
    #[default]
    Lobby,
    /// The normal five-beat loop is running.
    Playing,
    /// Parked at `Beat::Resolve` waiting on the table's votes for a real-life
    /// challenge. Nothing advances until `challenges` empties — the engine
    /// returns `LcError::ChallengePending` for anything that would try.
    Challenge,
    /// A winner, a draw, or a shared pact win. The tableau is frozen (D16):
    /// no beat advances and no action mutates the table again.
    Finished,
}

impl Phase {
    pub fn slug(self) -> &'static str {
        match self {
            Phase::Lobby => "lobby",
            Phase::Playing => "playing",
            Phase::Challenge => "challenge",
            Phase::Finished => "finished",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Lobby => "LOBBY",
            Phase::Playing => "PLAYING",
            Phase::Challenge => "CHALLENGE",
            Phase::Finished => "FINISHED",
        }
    }

    /// Whether ordinary play actions (arm, lock, react, draw) are conceivable
    /// at all. Beat-level legality is still the engine's own guards' job —
    /// this only rules out the three phases where *nothing* is playable.
    pub fn accepts_play(self) -> bool {
        matches!(self, Phase::Lobby | Phase::Playing)
    }

    /// Whether the table is frozen for good.
    pub fn is_over(self) -> bool {
        self == Phase::Finished
    }
}

/// What one seat is doing right now — and, critically, whether the table is
/// waiting on it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SeatPhase {
    /// Alive, the beat wants something from this seat, and it hasn't been
    /// given. This is the ONLY variant that holds a beat up.
    #[default]
    Acting,
    /// Alive and has signalled done for this beat (`ready`) — waiting on
    /// others.
    Ready,
    /// Has committed its plays for the round (`locked`). Nothing more to do
    /// until the reveal.
    Locked,
    /// Alive, but this beat asks nothing of this seat.
    Waiting,
    /// Eliminated. Can still haunt (one ghost vote a round) and vote on
    /// challenges, but holds up nothing.
    Ghost,
    /// The game is over. Nobody is expected to do anything.
    Done,
}

impl SeatPhase {
    pub fn slug(self) -> &'static str {
        match self {
            SeatPhase::Acting => "acting",
            SeatPhase::Ready => "ready",
            SeatPhase::Locked => "locked",
            SeatPhase::Waiting => "waiting",
            SeatPhase::Ghost => "ghost",
            SeatPhase::Done => "done",
        }
    }

    /// Is the table waiting on this seat? The single question the phone's
    /// "your turn" pulse and the big screen's ready-tick both want, now
    /// answered identically because they read the same field.
    pub fn is_blocking(self) -> bool {
        self == SeatPhase::Acting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_orders_by_progression() {
        assert!(Phase::Lobby < Phase::Playing);
        assert!(Phase::Playing < Phase::Challenge);
        assert!(Phase::Challenge < Phase::Finished);
    }

    #[test]
    fn test_only_lobby_and_playing_accept_play() {
        assert!(Phase::Lobby.accepts_play());
        assert!(Phase::Playing.accepts_play());
        assert!(!Phase::Challenge.accepts_play());
        assert!(!Phase::Finished.accepts_play());
        assert!(Phase::Finished.is_over());
        assert!(!Phase::Challenge.is_over());
    }

    #[test]
    fn test_only_acting_blocks_a_beat() {
        assert!(SeatPhase::Acting.is_blocking());
        for p in [
            SeatPhase::Ready,
            SeatPhase::Locked,
            SeatPhase::Waiting,
            SeatPhase::Ghost,
            SeatPhase::Done,
        ] {
            assert!(!p.is_blocking(), "{p:?} must not hold the beat up");
        }
    }

    #[test]
    fn test_slugs_are_stable_and_distinct() {
        // These reach the DOM as data-attributes; a collision would style
        // two states identically.
        let phases = [Phase::Lobby, Phase::Playing, Phase::Challenge, Phase::Finished];
        let slugs: std::collections::HashSet<&str> = phases.iter().map(|p| p.slug()).collect();
        assert_eq!(slugs.len(), phases.len());

        let seats = [
            SeatPhase::Acting,
            SeatPhase::Ready,
            SeatPhase::Locked,
            SeatPhase::Waiting,
            SeatPhase::Ghost,
            SeatPhase::Done,
        ];
        let slugs: std::collections::HashSet<&str> = seats.iter().map(|p| p.slug()).collect();
        assert_eq!(slugs.len(), seats.len());
    }

    #[test]
    fn test_serde_shape() {
        assert_eq!(serde_json::to_string(&Phase::Lobby).unwrap(), "\"lobby\"");
        assert_eq!(
            serde_json::to_string(&SeatPhase::Ghost).unwrap(),
            "\"ghost\""
        );
    }
}
