//! **The re-theme seam.** Every player-facing string the Backroom game can
//! emit lives here, as `pub const` or a `pub fn` over engine enums. Nothing
//! in this file has logic; nothing outside it holds a readable literal.
//!
//! Re-theming the game is editing this one file. That works because theme
//! text is *const data resolved at render time and never serialized* — the
//! same rule `lc_cards.rs` follows — so a rename reaches games already in
//! flight. The one string that is NOT themeable is
//! [`crate::secret_hitler::KIND`], which is persisted in `games.kind`:
//! renaming it orphans every live row.
//!
//! ## The current theme: Backroom
//!
//! A smoke-filled-room power struggle, sized for a bar. The **Regulars**
//! want the room cleaned up; the **Syndicate** wants it theirs; and one
//! Syndicate member — **the Boss** — wins outright by getting handed the
//! chair once the room has gone bad enough. Mechanically this is Secret
//! Hitler (Goat Wolf & Cabbage, 2016); see `docs/design.md` for the
//! attribution note.
//!
//! ## Drinks: the constraint that is not negotiable
//!
//! v1 pours nothing — [`drinks`] returns zero for every event and the plumbing that drains
//! it is inert. The drinking variant ("Secret Sippler") turns these numbers
//! up, and when it does it must obey one rule:
//!
//! > **A pour may key only on a fact already public, or be uniform across
//! > all living seats.**
//!
//! Drinks are written to `events`, which feeds `db::leaderboard`, which is
//! broadcast as `RoomMessage::Leaderboard` over the room's SSE stream —
//! and `routes::sse_stream` takes **no session extractor**, so that stream
//! is also what paints the cookie-less spectator TV. A pour keyed on a
//! hidden fact ("Regulars drink when a Racket passes", "the Boss drinks
//! when investigated") publishes the per-player deltas that partition the
//! table by faction, in public, on a television. The leaderboard is a role
//! oracle; [`DrinkEvent`] is deliberately restricted to public facts so it
//! cannot become one.

use crate::secret_hitler::{Outcome, Party, Policy, Power, Role};

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// The game's name, as players see it.
pub const GAME_NAME: &str = "Backroom";

/// One line under the name on the START card.
pub const TAGLINE: &str = "Five to ten. Someone at this table is lying.";

/// Shown when the room does not have a legal number of players.
pub const NEEDS_5_TO_10: &str = "Backroom needs between 5 and 10 players at the table";

// ---------------------------------------------------------------------------
// Factions and roles
// ---------------------------------------------------------------------------

pub const PARTY_LIBERAL: &str = "Regulars";
pub const PARTY_FASCIST: &str = "Syndicate";

pub const ROLE_LIBERAL: &str = "Regular";
pub const ROLE_FASCIST: &str = "Syndicate";
pub const ROLE_HITLER: &str = "the Boss";

/// The membership card a player is *shown* — the Boss's reads Syndicate,
/// which is the whole point of investigating being weaker than it looks.
pub fn party_name(p: Party) -> &'static str {
    match p {
        Party::Liberal => PARTY_LIBERAL,
        Party::Fascist => PARTY_FASCIST,
    }
}

pub fn role_name(r: Role) -> &'static str {
    match r {
        Role::Liberal => ROLE_LIBERAL,
        Role::Fascist => ROLE_FASCIST,
        Role::Hitler => ROLE_HITLER,
    }
}

/// The one-line brief a player reads on their own private pane.
pub fn role_brief(r: Role) -> &'static str {
    match r {
        Role::Liberal => "Clean up the room. You know nobody — work it out.",
        Role::Fascist => "Take the room. Get the Boss into the second chair.",
        Role::Hitler => "You are the Boss. Stay quiet, get handed the second chair.",
    }
}

// ---------------------------------------------------------------------------
// Offices
// ---------------------------------------------------------------------------

pub const OFFICE_PRESIDENT: &str = "Chair";
pub const OFFICE_CHANCELLOR: &str = "Second";
pub const OFFICE_PRESIDENT_SHORT: &str = "CHAIR";
pub const OFFICE_CHANCELLOR_SHORT: &str = "SECOND";

// ---------------------------------------------------------------------------
// Policy tiles and tracks
// ---------------------------------------------------------------------------

pub const POLICY_LIBERAL: &str = "Reform";
pub const POLICY_FASCIST: &str = "Racket";

pub const TRACK_LIBERAL: &str = "Reform";
pub const TRACK_FASCIST: &str = "Racket";

pub fn policy_name(p: Policy) -> &'static str {
    match p {
        Policy::Liberal => POLICY_LIBERAL,
        Policy::Fascist => POLICY_FASCIST,
    }
}

// ---------------------------------------------------------------------------
// The vote
// ---------------------------------------------------------------------------

pub const VOTE_YES: &str = "AYE";
pub const VOTE_NO: &str = "NAY";
pub const VOTE_PENDING: &str = "…";

pub fn vote_name(ja: bool) -> &'static str {
    if ja {
        VOTE_YES
    } else {
        VOTE_NO
    }
}

/// The failed-government counter. Three and the room boils over.
pub const TRACKER_NAME: &str = "Heat";

// ---------------------------------------------------------------------------
// Powers
// ---------------------------------------------------------------------------

pub const POWER_INVESTIGATE: &str = "Dig Up Dirt";
pub const POWER_SPECIAL_ELECTION: &str = "Snap Vote";
pub const POWER_PEEK: &str = "Read the Room";
pub const POWER_EXECUTION: &str = "86 Them";

pub fn power_name(p: Power) -> &'static str {
    match p {
        Power::Investigate => POWER_INVESTIGATE,
        Power::SpecialElection => POWER_SPECIAL_ELECTION,
        Power::Peek => POWER_PEEK,
        Power::Execution => POWER_EXECUTION,
    }
}

/// The instruction the holder of the power reads.
pub fn power_prompt(p: Power) -> &'static str {
    match p {
        Power::Investigate => "Pick someone. You alone see which side they drink on.",
        Power::SpecialElection => "Pick who takes the chair next. Anyone but you.",
        Power::Peek => "The next three off the pile, for your eyes only.",
        Power::Execution => "Pick someone to throw out. They are done for the night.",
    }
}

pub const VETO_NAME: &str = "Kill the round";
pub const VETO_PROPOSE: &str = "Kill this round";
pub const VETO_AGREE: &str = "Agreed — bin them both";
pub const VETO_REFUSE: &str = "No. Play one.";

// ---------------------------------------------------------------------------
// Phase banners
// ---------------------------------------------------------------------------

pub const PHASE_NOMINATION: &str = "The Chair picks a Second";
pub const PHASE_VOTING: &str = "Everyone votes";
pub const PHASE_PRESIDENT_DRAFT: &str = "The Chair is looking at three";
pub const PHASE_CHANCELLOR_DRAFT: &str = "The Second is looking at two";
pub const PHASE_VETO_PENDING: &str = "The Second wants this round killed";
pub const PHASE_POWER: &str = "The Chair has something to do";

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

pub fn outcome_banner(o: Outcome) -> &'static str {
    match o {
        Outcome::LiberalPolicies => "The Regulars cleaned up the room.",
        Outcome::HitlerExecuted => "The Boss got 86'd. The Regulars take it.",
        Outcome::FascistPolicies => "The Syndicate owns the room.",
        Outcome::HitlerChancellor => {
            "The Boss got handed the second chair. The Syndicate takes it."
        }
    }
}

pub fn outcome_winner(o: Outcome) -> &'static str {
    match o {
        Outcome::LiberalPolicies | Outcome::HitlerExecuted => PARTY_LIBERAL,
        Outcome::FascistPolicies | Outcome::HitlerChancellor => PARTY_FASCIST,
    }
}

// ---------------------------------------------------------------------------
// Error messages
//
// `map_sh` may surface ONLY these strings. Error bodies are assigned straight
// into the DOM by `room.html`'s htmx:responseError handler, so a message that
// named a role, a party, a tile or a peeked card would leak it to the one
// player most motivated to read it. Every string below is a public fact.
// ---------------------------------------------------------------------------

pub const ERR_WRONG_PHASE: &str = "not right now";
pub const ERR_NOT_YOUR_MOVE: &str = "that call isn't yours to make";
pub const ERR_NOT_ELIGIBLE: &str = "they can't take the second chair this round";
pub const ERR_DEAD_PLAYER: &str = "you're out for the night";
pub const ERR_ALREADY_INVESTIGATED: &str = "someone already dug into them this game";
pub const ERR_BAD_TARGET: &str = "you can't pick them";
pub const ERR_BAD_INDEX: &str = "you don't hold that one";
pub const ERR_WRONG_SEAT_COUNT: &str = NEEDS_5_TO_10;

// ---------------------------------------------------------------------------
// Log lines
//
// Templates over PUBLIC facts only. There is deliberately no template here
// taking a `Party`, `Role` or `Policy`-you-were-shown — see
// `secret_hitler::LogEntry`, whose variants carry no secret payload by
// construction.
// ---------------------------------------------------------------------------

pub const LOG_NOMINATED: &str = "put forward";
pub const LOG_ELECTED: &str = "took the chairs";
pub const LOG_REJECTED: &str = "was voted down";
pub const LOG_ENACTED: &str = "went up";
pub const LOG_CHAOS: &str = "The room boiled over — top of the pile, face up";
pub const LOG_VETOED: &str = "killed the round";
pub const LOG_VETO_REFUSED: &str = "refused the kill";
pub const LOG_INVESTIGATED: &str = "dug into";
pub const LOG_SPECIAL_ELECTION: &str = "called a snap vote on";
pub const LOG_PEEKED: &str = "read the room";
pub const LOG_EXECUTED: &str = "86'd";
pub const LOG_VETO_UNLOCKED: &str = "Rounds can be killed from here on";

// ---------------------------------------------------------------------------
// Drinks
// ---------------------------------------------------------------------------

/// The public facts a pour may key on. Adding a variant here is a deliberate
/// act: read the module doc's constraint first. There is no variant for
/// "enacted a Racket" keyed to a faction, no variant for an investigation
/// *result*, and no variant keyed on a role — those are the leaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrinkEvent {
    /// The government was voted down. Uniform across living seats.
    ElectionFailed,
    /// The Heat hit three and the top tile flipped. Uniform.
    Chaos,
    /// A Racket went up. Uniform — NOT keyed to the enacting pair, which
    /// would correlate with faction over a game.
    FascistPolicy,
    /// A Reform went up. Uniform.
    LiberalPolicy,
    /// This seat was thrown out. Public: everyone watched it happen.
    Executed,
    /// This seat was dug into. Public: who was investigated is public, the
    /// result is not, and only the former is keyed here.
    Investigated,
    /// The round was killed by agreement. Uniform.
    Vetoed,
}

/// Sips per event. **All zero in v1** — the game ships drink-free and the
/// drain in `sh_routes` is a no-op. Turning these up is the whole of the
/// "Secret Sippler" mode, subject to the module doc's constraint.
pub fn drinks(e: DrinkEvent) -> u32 {
    match e {
        DrinkEvent::ElectionFailed => 0,
        DrinkEvent::Chaos => 0,
        DrinkEvent::FascistPolicy => 0,
        DrinkEvent::LiberalPolicy => 0,
        DrinkEvent::Executed => 0,
        DrinkEvent::Investigated => 0,
        DrinkEvent::Vetoed => 0,
    }
}

/// True while every amount is zero — the routes skip the whole drink drain,
/// so v1 writes no `events` rows at all and the leaderboard stays a pure
/// record of the other three games.
pub fn drinks_enabled() -> bool {
    [
        DrinkEvent::ElectionFailed,
        DrinkEvent::Chaos,
        DrinkEvent::FascistPolicy,
        DrinkEvent::LiberalPolicy,
        DrinkEvent::Executed,
        DrinkEvent::Investigated,
        DrinkEvent::Vetoed,
    ]
    .iter()
    .any(|&e| drinks(e) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v1_pours_nothing() {
        assert!(
            !drinks_enabled(),
            "v1 ships drink-free; turning a pour on is the Secret Sippler mode \
             and must first satisfy the module doc's public-facts-only constraint"
        );
    }

    #[test]
    fn test_every_theme_string_is_non_empty() {
        let all = [
            GAME_NAME,
            TAGLINE,
            NEEDS_5_TO_10,
            PARTY_LIBERAL,
            PARTY_FASCIST,
            ROLE_LIBERAL,
            ROLE_FASCIST,
            ROLE_HITLER,
            OFFICE_PRESIDENT,
            OFFICE_CHANCELLOR,
            OFFICE_PRESIDENT_SHORT,
            OFFICE_CHANCELLOR_SHORT,
            POLICY_LIBERAL,
            POLICY_FASCIST,
            TRACK_LIBERAL,
            TRACK_FASCIST,
            VOTE_YES,
            VOTE_NO,
            VOTE_PENDING,
            TRACKER_NAME,
            POWER_INVESTIGATE,
            POWER_SPECIAL_ELECTION,
            POWER_PEEK,
            POWER_EXECUTION,
            VETO_NAME,
            VETO_PROPOSE,
            VETO_AGREE,
            VETO_REFUSE,
            PHASE_NOMINATION,
            PHASE_VOTING,
            PHASE_PRESIDENT_DRAFT,
            PHASE_CHANCELLOR_DRAFT,
            PHASE_VETO_PENDING,
            PHASE_POWER,
            ERR_WRONG_PHASE,
            ERR_NOT_YOUR_MOVE,
            ERR_NOT_ELIGIBLE,
            ERR_DEAD_PLAYER,
            ERR_ALREADY_INVESTIGATED,
            ERR_BAD_TARGET,
            ERR_BAD_INDEX,
        ];
        for s in all {
            assert!(!s.trim().is_empty());
        }
    }

    #[test]
    fn test_the_two_factions_are_distinguishable() {
        // A re-theme that gives both sides the same word makes the whole
        // game unreadable; cheap to assert, easy to trip over.
        assert_ne!(PARTY_LIBERAL, PARTY_FASCIST);
        assert_ne!(POLICY_LIBERAL, POLICY_FASCIST);
        assert_ne!(VOTE_YES, VOTE_NO);
        assert_ne!(OFFICE_PRESIDENT, OFFICE_CHANCELLOR);
    }

    #[test]
    fn test_error_strings_name_no_secret() {
        // `map_sh` puts these straight into the DOM. None may name a role,
        // a faction or a tile.
        let secrets = [
            PARTY_LIBERAL,
            PARTY_FASCIST,
            ROLE_LIBERAL,
            ROLE_HITLER,
            POLICY_LIBERAL,
            POLICY_FASCIST,
        ];
        let errors = [
            ERR_WRONG_PHASE,
            ERR_NOT_YOUR_MOVE,
            ERR_NOT_ELIGIBLE,
            ERR_DEAD_PLAYER,
            ERR_ALREADY_INVESTIGATED,
            ERR_BAD_TARGET,
            ERR_BAD_INDEX,
        ];
        for err in errors {
            for secret in secrets {
                assert!(
                    !err.to_lowercase().contains(&secret.to_lowercase()),
                    "error {err:?} names {secret:?}, which reaches the DOM of the \
                     one player most motivated to read it"
                );
            }
        }
    }
}
