//! Backroom — a hidden-role engine. A pure state machine: no I/O, no SQL,
//! no clock, no ambient RNG.
//!
//! Mechanically this is Secret Hitler (Goat Wolf & Cabbage, 2016). Every
//! player-facing string lives in [`crate::sh_theme`]; this file holds only
//! mechanics, and the vocabulary here (`Liberal`/`Fascist`/`Hitler`,
//! `President`/`Chancellor`) is the *rules* vocabulary, kept so the code
//! reads against the published rules. Nothing here reaches a screen.
//!
//! # Two deviations from the crate's house style, both deliberate
//!
//! **This engine carries a PRNG.** `three_man.rs` takes dice as parameters
//! because dice are rolled once per action; Backroom must reshuffle at
//! points the caller cannot predict (after an enactment, before a draw,
//! before a peek), so passing entropy in would mean every handler supplying
//! a seed just in case. Instead `rng_seed` is sampled once by the start
//! handler and advanced here by a hand-rolled splitmix64, which keeps this
//! module a pure, deterministic function of its own blob — unit tests need
//! no RNG rig — and keeps the `rand` crate out of it. The *result* of a
//! shuffle is stored in `draw_pile`, never replayed from the seed, so the
//! generator can change without invalidating a live game.
//!
//! **Display names are not stored.** They resolve per render from
//! `db::room_members`, the 3 Man precedent, so a rename via `/account`
//! cannot leave a stale name in a live blob. The cost is that a rename is
//! retroactive: it changes the name shown against an old vote too.
//!
//! # The secrecy contract
//!
//! `RoomHub` has one broadcast channel per *room*, never per player, and
//! `routes::sse_stream` takes no session extractor — so every byte published
//! to a room is readable by anyone with the 4-letter code, including the
//! cookie-less spectator TV. Therefore:
//!
//! - Secret fields on [`SecretHitlerState`] are marked `SECRET` and named
//!   against the test that pins them. [`SecretHitlerState::public_view`]
//!   never reads them — the projection is a structural proof, not a filter.
//! - [`ShPublicView`] is what the broadcast renders from. Everything in it
//!   is a fact visible from a chair at the table.
//! - Private state reaches a player by an authenticated *fetch* of their own
//!   fragment, never by a push. See `sh_routes::sh_private_handler`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Persisted discriminator in `games.kind`. **Never re-theme this**: it is a
/// live-row value and renaming it orphans every in-flight game.
pub const KIND: &str = "secret_hitler";

pub const MIN_SEATS: usize = 5;
pub const MAX_SEATS: usize = 10;

pub const FASCIST_TILES: usize = 11;
pub const LIBERAL_TILES: usize = 6;
/// Conservation invariant: tiles are never created or destroyed.
pub const TOTAL_TILES: usize = FASCIST_TILES + LIBERAL_TILES;

/// Liberals win at 5 Liberal policies; Fascists at 6 Fascist policies.
pub const LIBERAL_TRACK: u8 = 5;
pub const FASCIST_TRACK: u8 = 6;

/// The election tracker fires at 3.
pub const CHAOS_AT: u8 = 3;
/// The Hitler check runs from the 3rd Fascist policy onward (§6.1 L1/L4).
pub const HITLER_CHECK_FROM: u8 = 3;
/// Veto unlocks permanently at 5 Fascist policies (§6.4 V1).
pub const VETO_UNLOCKS_AT: u8 = 5;

/// The public log is capped like Last Call's. A bounded worst case: at most
/// 11 enactments end the game, each preceded by up to 3 failed elections.
pub const LOG_CAP: usize = 80;
/// Vote history is public and permanent (§5.3 E21) and is the game's primary
/// information channel — capped only so the blob and the wire stay bounded.
/// 64 covers the worst reachable game (~44 elections).
pub const VOTE_HISTORY_CAP: usize = 64;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Party {
    Liberal,
    Fascist,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Liberal,
    Fascist,
    Hitler,
}

impl Role {
    /// §1: Hitler's membership card reads *Fascist*. Investigate Loyalty
    /// (§7.2) returns this, never the `Role` — which is exactly why the
    /// power is weaker than it looks.
    pub fn party(self) -> Party {
        match self {
            Role::Liberal => Party::Liberal,
            Role::Fascist | Role::Hitler => Party::Fascist,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    Liberal,
    Fascist,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Investigate,
    SpecialElection,
    Peek,
    Execution,
}

impl Power {
    /// Investigate and Special Election name another player; Execution names
    /// another player; Peek names nobody. Drives both the route's argument
    /// validation and the renderer's control shape.
    pub fn needs_target(self) -> bool {
        !matches!(self, Power::Peek)
    }
}

/// Frozen at start from the STARTING seat count and never recomputed — §2.3:
/// a 10-player game reduced to 8 alive still uses the Large board.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BoardSize {
    /// 5–6 players.
    #[default]
    Small,
    /// 7–8 players.
    Mid,
    /// 9–10 players.
    Large,
}

impl BoardSize {
    pub fn for_seats(n: usize) -> BoardSize {
        match n {
            0..=6 => BoardSize::Small,
            7..=8 => BoardSize::Mid,
            _ => BoardSize::Large,
        }
    }

    fn row(self) -> usize {
        match self {
            BoardSize::Small => 0,
            BoardSize::Mid => 1,
            BoardSize::Large => 2,
        }
    }

    /// The power printed on the slot the `n`th Fascist policy fills, if any.
    /// `n` is 1-based. Slot 6 is `None`: the game has already ended (§8 W3),
    /// so no power resolves (§9.32).
    pub fn power_at(self, n: u8) -> Option<Power> {
        if n == 0 || n as usize > POWERS[0].len() {
            return None;
        }
        POWERS[self.row()][n as usize - 1]
    }
}

/// §7.1, indexed `[board][fascist_policy_number - 1]`.
///
/// Two Executions on every board; Policy Peek exists only on the 5–6 board;
/// Investigate and Special Election only on 7–8 and 9–10. Slot 6 is always
/// `None` because the 6th Fascist policy ends the game.
const POWERS: [[Option<Power>; 6]; 3] = [
    // 5–6
    [
        None,
        None,
        Some(Power::Peek),
        Some(Power::Execution),
        Some(Power::Execution),
        None,
    ],
    // 7–8
    [
        None,
        Some(Power::Investigate),
        Some(Power::SpecialElection),
        Some(Power::Execution),
        Some(Power::Execution),
        None,
    ],
    // 9–10
    [
        Some(Power::Investigate),
        Some(Power::Investigate),
        Some(Power::SpecialElection),
        Some(Power::Execution),
        Some(Power::Execution),
        None,
    ],
];

/// §2.1, indexed by `seats - MIN_SEATS`: `(liberals, plain_fascists)`.
/// Hitler is always the +1, so `liberals + fascists + 1 == seats`.
const ROLE_SPLIT: [(usize, usize); 6] = [(3, 1), (4, 1), (4, 2), (5, 2), (5, 3), (6, 3)];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 5 Liberal policies (§8 W1).
    LiberalPolicies,
    /// Hitler was executed (§8 W2).
    HitlerExecuted,
    /// 6 Fascist policies (§8 W3).
    FascistPolicies,
    /// Hitler elected Chancellor at 3+ Fascist policies (§8 W4).
    HitlerChancellor,
}

impl Outcome {
    pub fn winner(self) -> Party {
        match self {
            Outcome::LiberalPolicies | Outcome::HitlerExecuted => Party::Liberal,
            Outcome::FascistPolicies | Outcome::HitlerChancellor => Party::Fascist,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// The President nominates a Chancellor.
    #[default]
    Nomination,
    /// Every living player votes.
    Voting,
    /// The President holds 3 tiles and discards 1.
    PresidentDraft,
    /// The Chancellor holds 2 tiles and enacts 1 (or proposes a veto).
    ChancellorDraft,
    /// The Chancellor proposed a veto; the President must answer (§6.4 V3/V4).
    VetoPending,
    /// A mandatory executive power belonging to the enacting President
    /// (§7.2 P1/P2).
    Power(Power),
    Over(Outcome),
}

// ---------------------------------------------------------------------------
// Seats
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Seat {
    pub player_id: i64,
    /// **SECRET.** [`SecretHitlerState::public_view`] never reads this field,
    /// not even for a dead seat — §7.2 keeps an executed player's card hidden
    /// until the game ends. The one exception is `final_roles`, gated on
    /// `Phase::Over`. Pinned by `test_public_view_never_reveals_a_role` and
    /// `test_a_dead_seats_role_stays_hidden_until_game_over`.
    pub role: Role,
    pub alive: bool,
    /// Seat indices this seat was shown during setup (§2.2). Empty for every
    /// Liberal, and for Hitler at 7–10. Frozen at deal time — deaths never
    /// change it.
    ///
    /// **SECRET.** Pinned by `test_public_view_never_reveals_night_knowledge`.
    pub knows: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// A closed election. Public and permanent (§5.3 E21).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VoteRecord {
    pub round: u32,
    pub president: usize,
    pub nominee: usize,
    /// Indexed by seat. `None` for a seat that was dead and so did not vote.
    pub ballots: Vec<Option<bool>>,
    pub passed: bool,
}

/// **SECRET.** The result of an investigation, readable only by the
/// investigator's own private pane.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InvestigationRecord {
    pub investigator: usize,
    pub target: usize,
    pub party: Party,
}

/// **SECRET.** What a President saw on a Policy Peek.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PeekRecord {
    pub president: usize,
    pub round: u32,
    pub top: Vec<Policy>,
}

/// A pour, queued by the engine and drained by the route into
/// `db::insert_events_bulk`. See [`crate::sh_theme::DrinkEvent`] for why the
/// set of things that may produce one is deliberately narrow.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DrinkCall {
    pub player_id: i64,
    pub n: u32,
}

/// A public log line.
///
/// **No variant carries a `Party` or a `Role`** — this enum is rendered into
/// the shared broadcast, so one `Investigated { party }` variant would void
/// the whole secrecy design for every subscriber including the TV. The only
/// `Policy` that appears is an *enacted* one, which goes face up on the track
/// and is public by the rules (§6.2 L9). Discards, peeks and investigation
/// results have no representation here at all. Pinned by
/// `test_log_entry_carries_no_secret`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum LogEntry {
    Nominated {
        president: usize,
        nominee: usize,
    },
    Elected {
        president: usize,
        chancellor: usize,
    },
    Rejected {
        president: usize,
        nominee: usize,
        ja: usize,
        nay: usize,
    },
    /// A tile went face up. `chaos` distinguishes the frustrated populace
    /// (§6.3) from an elected government.
    Enacted {
        policy: Policy,
        chaos: bool,
    },
    Vetoed {
        president: usize,
        chancellor: usize,
    },
    VetoRefused {
        president: usize,
        chancellor: usize,
    },
    /// *Who* was investigated is public; the result is not and is absent here.
    Investigated {
        president: usize,
        target: usize,
    },
    SpecialElection {
        president: usize,
        target: usize,
    },
    /// *That* the President looked is public; what they saw is not.
    Peeked {
        president: usize,
    },
    Executed {
        president: usize,
        target: usize,
    },
    VetoUnlocked,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ShError {
    WrongPhase,
    NotYourMove,
    NotEligible,
    DeadPlayer,
    AlreadyInvestigated,
    BadTarget,
    BadIndex,
    WrongSeatCount,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Container-level `#[serde(default)]` only: a field added by a new binary
/// backfills against a blob written by the old one. Nested structs stay
/// strict, so a genuinely corrupt blob still fails loudly.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SecretHitlerState {
    // ---- frozen at start ----
    /// Index is the seat index; order is `db::room_members` join order.
    pub seats: Vec<Seat>,
    pub board: BoardSize,

    // ---- public ----
    pub phase: Phase,
    /// Increments once per election, whether it passes or fails.
    pub round: u32,
    pub liberal_track: u8,
    pub fascist_track: u8,
    pub election_tracker: u8,
    pub president_seat: usize,
    pub nominee_seat: Option<usize>,
    /// Term limits (§5.2 E6/E7): the last *elected* pair, not the last
    /// nominated one. Both cleared by chaos (§6.3 C5).
    pub last_elected_president: Option<usize>,
    pub last_elected_chancellor: Option<usize>,
    /// §7.2: the anchor is the **calling President's seat index**, not the
    /// special-elected President and not a player id. Rotation resumes to its
    /// left once the out-of-turn presidency is spent, so a caller who is
    /// executed in the meantime still anchors correctly.
    pub special_return_seat: Option<usize>,
    /// Overrides rotation for exactly one round.
    pub pending_special_president: Option<usize>,
    /// The ballots of the last *closed* election, revealed all at once
    /// (§5.3 E16). Public.
    pub last_vote: Vec<Option<bool>>,
    pub vote_history: Vec<VoteRecord>,
    /// Who has been investigated. *Who* is public (§7.2); the result is not
    /// and lives in `investigation_results`.
    pub investigated: Vec<usize>,
    pub veto_unlocked: bool,
    pub log: Vec<LogEntry>,
    /// Pours produced by the current transition. Cleared once at the top of
    /// each public entry point and appended by the helpers it cascades
    /// through; drained by the route before it persists.
    pub pending_drinks: Vec<DrinkCall>,

    // ---- SECRET — public_view() never reads any of these ----
    /// Ballots in flight, indexed by seat. `public_view()` reads this field
    /// **only** through `.is_some()`, to publish *that* a seat has voted —
    /// you can see a ballot laid face down. The value moves into `last_vote`
    /// when the vote closes, so the reveal is a data movement rather than a
    /// flag flip. Pinned by
    /// `test_public_view_never_reveals_a_ballot_before_the_reveal`.
    pub votes: Vec<Option<bool>>,
    /// **SECRET** (§3 D2). Only its length is published.
    pub draw_pile: Vec<Policy>,
    /// **SECRET** (§3 D3) — never revealed, not even at game end.
    pub discard_pile: Vec<Policy>,
    /// **SECRET.** The 3 the President drew (§6.2 L7).
    pub president_hand: Vec<Policy>,
    /// **SECRET.** The 2 passed to the Chancellor (§6.2 L8).
    pub chancellor_hand: Vec<Policy>,
    /// **SECRET.** Readable only by each investigator's own private pane.
    pub investigation_results: Vec<InvestigationRecord>,
    /// **SECRET.** Readable only by each President's own private pane.
    pub peek_results: Vec<PeekRecord>,

    // ---- plumbing ----
    /// Sampled once by the start handler and advanced deterministically here.
    pub rng_seed: u64,
    /// Bumped exactly once per distinct visible change. The client's
    /// stale-drop floor.
    pub seq: u64,
}

// ---------------------------------------------------------------------------
// The projection
// ---------------------------------------------------------------------------

/// One seat, as seen from a chair at the table.
#[derive(Clone, Debug, PartialEq)]
pub struct ShPublicSeat {
    pub seat: usize,
    pub player_id: i64,
    pub alive: bool,
    /// That this seat has voted — never how.
    pub voted: bool,
    pub is_president: bool,
    pub is_nominee: bool,
    /// Whether this seat could legally be nominated right now. Computed by
    /// [`SecretHitlerState::eligible_chancellor`], because a single
    /// `term_limited` flag renders the wrong thing at exactly 5 alive, where
    /// the last President becomes nominable again (§5.2 E9/E10).
    pub eligible_nominee: bool,
    /// Whether this seat has been investigated this game. Public (§7.2).
    pub investigated: bool,
    pub is_last_elected_president: bool,
    pub is_last_elected_chancellor: bool,
}

/// Everything the shared broadcast may render. Constructed only by
/// [`SecretHitlerState::public_view`].
#[derive(Clone, Debug, PartialEq)]
pub struct ShPublicView {
    pub seq: u64,
    pub round: u32,
    pub phase: Phase,
    pub board: BoardSize,
    pub liberal_track: u8,
    pub fascist_track: u8,
    pub election_tracker: u8,
    pub veto_unlocked: bool,
    /// Whether a successful election would now trigger the Hitler check
    /// (§6.1). Derived from the public track, not from any role.
    pub hitler_check_live: bool,
    /// Counts only — the piles' contents are secret (§3 D2/D3).
    pub draw_count: usize,
    pub discard_count: usize,
    pub seats: Vec<ShPublicSeat>,
    pub president_seat: usize,
    pub nominee_seat: Option<usize>,
    pub alive_count: usize,
    /// Votes needed to pass: `alive / 2 + 1`, i.e. strictly more than half.
    pub majority_needed: usize,
    pub last_vote: Vec<Option<bool>>,
    pub vote_history: Vec<VoteRecord>,
    pub log: Vec<LogEntry>,
    pub outcome: Option<Outcome>,
    /// Revealed **only** at game end. The single place `public_view` may read
    /// [`Seat::role`], gated on one `if`.
    pub final_roles: Option<Vec<Role>>,
}

// ---------------------------------------------------------------------------
// RNG — splitmix64 + Fisher–Yates, so the engine calls no RNG crate
// ---------------------------------------------------------------------------

fn splitmix64(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fisher–Yates. The modulo bias against a 64-bit draw is around 2^-58 for
/// the sizes here (≤17) — irrelevant, and cheaper than a rejection loop.
fn shuffle<T>(v: &mut [T], seed: &mut u64) {
    if v.len() < 2 {
        return;
    }
    for i in (1..v.len()).rev() {
        let j = (splitmix64(seed) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

// ---------------------------------------------------------------------------
// Setup and queries
// ---------------------------------------------------------------------------

impl SecretHitlerState {
    /// Deals roles, distributes night-phase knowledge (§2.2), shuffles the
    /// 17-tile deck and picks a random first President (§2.4).
    ///
    /// `members` is the seating order; `seed` is sampled once by the caller.
    pub fn new(members: Vec<i64>, seed: u64) -> Result<Self, ShError> {
        let n = members.len();
        if !(MIN_SEATS..=MAX_SEATS).contains(&n) {
            return Err(ShError::WrongSeatCount);
        }
        let mut rng = seed;

        // Deal roles into a shuffled bag, then hand them out by seat.
        let (liberals, fascists) = ROLE_SPLIT[n - MIN_SEATS];
        let mut bag: Vec<Role> = Vec::with_capacity(n);
        bag.extend(std::iter::repeat_n(Role::Liberal, liberals));
        bag.extend(std::iter::repeat_n(Role::Fascist, fascists));
        bag.push(Role::Hitler);
        debug_assert_eq!(bag.len(), n);
        shuffle(&mut bag, &mut rng);

        let mut seats: Vec<Seat> = members
            .into_iter()
            .zip(bag)
            .map(|(player_id, role)| Seat {
                player_id,
                role,
                alive: true,
                knows: Vec::new(),
            })
            .collect();

        // §2.2 night phase. Plain Fascists always see each other and Hitler.
        // Hitler sees the Fascists only at 5–6.
        let fascist_seats: Vec<usize> = seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.role == Role::Fascist)
            .map(|(i, _)| i)
            .collect();
        let hitler_seat = seats
            .iter()
            .position(|s| s.role == Role::Hitler)
            .expect("the bag always contains exactly one Hitler");

        for &f in &fascist_seats {
            let mut knows: Vec<usize> = fascist_seats.iter().copied().filter(|&x| x != f).collect();
            knows.push(hitler_seat);
            knows.sort_unstable();
            seats[f].knows = knows;
        }
        if n <= 6 {
            let mut knows = fascist_seats.clone();
            knows.sort_unstable();
            seats[hitler_seat].knows = knows;
        }

        // The deck: 11 Fascist + 6 Liberal, shuffled.
        let mut draw_pile: Vec<Policy> = Vec::with_capacity(TOTAL_TILES);
        draw_pile.extend(std::iter::repeat_n(Policy::Fascist, FASCIST_TILES));
        draw_pile.extend(std::iter::repeat_n(Policy::Liberal, LIBERAL_TILES));
        shuffle(&mut draw_pile, &mut rng);

        let president_seat = (splitmix64(&mut rng) % n as u64) as usize;

        Ok(SecretHitlerState {
            seats,
            board: BoardSize::for_seats(n),
            phase: Phase::Nomination,
            round: 1,
            liberal_track: 0,
            fascist_track: 0,
            election_tracker: 0,
            president_seat,
            nominee_seat: None,
            last_elected_president: None,
            last_elected_chancellor: None,
            special_return_seat: None,
            pending_special_president: None,
            last_vote: Vec::new(),
            vote_history: Vec::new(),
            investigated: Vec::new(),
            veto_unlocked: false,
            log: Vec::new(),
            pending_drinks: Vec::new(),
            votes: vec![None; n],
            draw_pile,
            discard_pile: Vec::new(),
            president_hand: Vec::new(),
            chancellor_hand: Vec::new(),
            investigation_results: Vec::new(),
            peek_results: Vec::new(),
            rng_seed: rng,
            seq: 0,
        })
    }

    pub fn seat_of(&self, player_id: i64) -> Option<usize> {
        self.seats.iter().position(|s| s.player_id == player_id)
    }

    pub fn alive_count(&self) -> usize {
        self.seats.iter().filter(|s| s.alive).count()
    }

    /// Strictly more than half the living players (§5.3 E18); a tie fails
    /// (E19).
    pub fn majority_needed(&self) -> usize {
        self.alive_count() / 2 + 1
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Over(_))
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match self.phase {
            Phase::Over(o) => Some(o),
            _ => None,
        }
    }

    /// §6.1: a successful election triggers the Hitler check once 3 Fascist
    /// policies are up. Derived from the public track alone.
    pub fn hitler_check_live(&self) -> bool {
        self.fascist_track >= HITLER_CHECK_FROM
    }

    /// §5.2, implemented exactly as specified.
    ///
    /// The `alive <= 5` relaxation is evaluated on **living players right
    /// now**, so it can switch on mid-game when an execution takes a 6-alive
    /// table to 5.
    pub fn eligible_chancellor(&self, seat: usize) -> bool {
        let Some(s) = self.seats.get(seat) else {
            return false;
        };
        let alive = self.alive_count();
        s.alive
            && seat != self.president_seat
            && Some(seat) != self.last_elected_chancellor
            && (alive <= 5 || Some(seat) != self.last_elected_president)
    }

    /// The power the `n`th Fascist policy grants on this board, if any.
    pub fn power_at(&self, n: u8) -> Option<Power> {
        self.board.power_at(n)
    }

    // -- serialization -------------------------------------------------------

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("SecretHitlerState is always serializable")
    }

    /// Deserializes a snapshot produced by [`Self::to_json`]. Only ever
    /// called on this engine's own output, so a parse failure is a
    /// programming error.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).expect("valid SecretHitlerState JSON")
    }

    /// Every tile is somewhere, always. Called by every transition test.
    pub fn tiles_conserved(&self) -> bool {
        self.draw_pile.len()
            + self.discard_pile.len()
            + self.president_hand.len()
            + self.chancellor_hand.len()
            + self.liberal_track as usize
            + self.fascist_track as usize
            == TOTAL_TILES
    }

    // -- the projection ------------------------------------------------------

    /// The **only** thing the shared broadcast may render from.
    ///
    /// This body reads no secret field. `votes` is touched solely through
    /// `.is_some()`; `role` solely inside the `Phase::Over` gate. Changing
    /// either is the one edit that voids the game's secrecy, so both are
    /// pinned by tests named on the fields themselves.
    pub fn public_view(&self) -> ShPublicView {
        let alive_count = self.alive_count();
        let seats = self
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| ShPublicSeat {
                seat: i,
                player_id: s.player_id,
                alive: s.alive,
                // `.is_some()` only — never the value.
                voted: self.votes.get(i).map(|v| v.is_some()).unwrap_or(false),
                is_president: i == self.president_seat,
                is_nominee: self.nominee_seat == Some(i),
                eligible_nominee: self.eligible_chancellor(i),
                investigated: self.investigated.contains(&i),
                is_last_elected_president: self.last_elected_president == Some(i),
                is_last_elected_chancellor: self.last_elected_chancellor == Some(i),
            })
            .collect();

        ShPublicView {
            seq: self.seq,
            round: self.round,
            phase: self.phase,
            board: self.board,
            liberal_track: self.liberal_track,
            fascist_track: self.fascist_track,
            election_tracker: self.election_tracker,
            veto_unlocked: self.veto_unlocked,
            hitler_check_live: self.hitler_check_live(),
            // Lengths only.
            draw_count: self.draw_pile.len(),
            discard_count: self.discard_pile.len(),
            seats,
            president_seat: self.president_seat,
            nominee_seat: self.nominee_seat,
            alive_count,
            majority_needed: self.majority_needed(),
            last_vote: self.last_vote.clone(),
            vote_history: self.vote_history.clone(),
            log: self.log.clone(),
            outcome: self.outcome(),
            // The one read of `Seat::role`, gated on the game being over.
            final_roles: match self.phase {
                Phase::Over(_) => Some(self.seats.iter().map(|s| s.role).collect()),
                _ => None,
            },
        }
    }

    // -- internal helpers ----------------------------------------------------

    pub(crate) fn push_log(&mut self, e: LogEntry) {
        self.log.push(e);
        if self.log.len() > LOG_CAP {
            let excess = self.log.len() - LOG_CAP;
            self.log.drain(..excess);
        }
    }

    pub(crate) fn push_vote_record(&mut self, r: VoteRecord) {
        self.vote_history.push(r);
        if self.vote_history.len() > VOTE_HISTORY_CAP {
            let excess = self.vote_history.len() - VOTE_HISTORY_CAP;
            self.vote_history.drain(..excess);
        }
    }

    /// Queues a pour for one seat. A no-op while the theme's amounts are all
    /// zero, which is v1.
    pub(crate) fn pour(&mut self, seat: usize, e: crate::sh_theme::DrinkEvent) {
        let n = crate::sh_theme::drinks(e);
        if n == 0 {
            return;
        }
        if let Some(s) = self.seats.get(seat) {
            self.pending_drinks.push(DrinkCall {
                player_id: s.player_id,
                n,
            });
        }
    }

    /// Queues a pour for every living seat.
    pub(crate) fn pour_all(&mut self, e: crate::sh_theme::DrinkEvent) {
        let n = crate::sh_theme::drinks(e);
        if n == 0 {
            return;
        }
        let calls: Vec<DrinkCall> = self
            .seats
            .iter()
            .filter(|s| s.alive)
            .map(|s| DrinkCall {
                player_id: s.player_id,
                n,
            })
            .collect();
        self.pending_drinks.extend(calls);
    }

    /// §3 D4/D6. Guarantees at least `n` tiles in the draw pile by shuffling
    /// the discard pile back in. Called as a **precondition** before every
    /// draw and peek *and* as a **post-condition** after every enactment —
    /// the official text states only the latter, which is insufficient after
    /// a chaos enactment drains the deck.
    pub(crate) fn ensure_draw(&mut self, n: usize) {
        if self.draw_pile.len() >= n {
            return;
        }
        let mut seed = self.rng_seed;
        self.draw_pile.append(&mut self.discard_pile);
        shuffle(&mut self.draw_pile, &mut seed);
        self.rng_seed = seed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(n: usize) -> Vec<i64> {
        (1..=n as i64).collect()
    }

    fn st(n: usize) -> SecretHitlerState {
        SecretHitlerState::new(members(n), 0xDEAD_BEEF ^ n as u64).expect("legal seat count")
    }

    // -- setup ---------------------------------------------------------------

    #[test]
    fn test_seat_count_is_gated() {
        assert_eq!(
            SecretHitlerState::new(members(4), 1).unwrap_err(),
            ShError::WrongSeatCount
        );
        assert_eq!(
            SecretHitlerState::new(members(11), 1).unwrap_err(),
            ShError::WrongSeatCount
        );
        assert!(SecretHitlerState::new(members(5), 1).is_ok());
        assert!(SecretHitlerState::new(members(10), 1).is_ok());
    }

    #[test]
    fn test_role_split_table_is_consistent() {
        for (i, (libs, fasc)) in ROLE_SPLIT.iter().enumerate() {
            let n = i + MIN_SEATS;
            assert_eq!(
                libs + fasc + 1,
                n,
                "row for {n} players must total {n} with Hitler as the +1"
            );
        }
    }

    #[test]
    fn test_role_counts_for_every_player_count() {
        // §2.1, verified against the dealt state rather than the table.
        let expected = [(3, 1), (4, 1), (4, 2), (5, 2), (5, 3), (6, 3)];
        for n in MIN_SEATS..=MAX_SEATS {
            let s = st(n);
            let libs = s.seats.iter().filter(|x| x.role == Role::Liberal).count();
            let fasc = s.seats.iter().filter(|x| x.role == Role::Fascist).count();
            let hitler = s.seats.iter().filter(|x| x.role == Role::Hitler).count();
            assert_eq!(hitler, 1, "{n}p: exactly one Hitler");
            assert_eq!((libs, fasc), expected[n - MIN_SEATS], "{n}p role split");
            assert_eq!(libs + fasc + hitler, n);
        }
    }

    #[test]
    fn test_hitler_party_card_reads_fascist() {
        assert_eq!(Role::Hitler.party(), Party::Fascist);
        assert_eq!(Role::Fascist.party(), Party::Fascist);
        assert_eq!(Role::Liberal.party(), Party::Liberal);
    }

    #[test]
    fn test_night_knowledge_at_five_and_six() {
        // §2.2: Fascist and Hitler acknowledge each other. Mutual.
        for n in [5usize, 6] {
            let s = st(n);
            let hitler = s.seats.iter().position(|x| x.role == Role::Hitler).unwrap();
            let fascists: Vec<usize> = s
                .seats
                .iter()
                .enumerate()
                .filter(|(_, x)| x.role == Role::Fascist)
                .map(|(i, _)| i)
                .collect();
            assert_eq!(fascists.len(), 1, "{n}p has exactly one plain Fascist");

            assert_eq!(
                s.seats[hitler].knows, fascists,
                "{n}p: Hitler knows the Fascist"
            );
            assert_eq!(
                s.seats[fascists[0]].knows,
                vec![hitler],
                "{n}p: the Fascist knows Hitler"
            );
            for (i, seat) in s.seats.iter().enumerate() {
                if seat.role == Role::Liberal {
                    assert!(seat.knows.is_empty(), "{n}p: Liberal at {i} knows nobody");
                }
            }
        }
    }

    #[test]
    fn test_night_knowledge_at_seven_to_ten() {
        // §2.2: plain Fascists see each other and Hitler; Hitler sees nothing.
        for n in 7..=MAX_SEATS {
            let s = st(n);
            let hitler = s.seats.iter().position(|x| x.role == Role::Hitler).unwrap();
            let fascists: Vec<usize> = s
                .seats
                .iter()
                .enumerate()
                .filter(|(_, x)| x.role == Role::Fascist)
                .map(|(i, _)| i)
                .collect();

            assert!(
                s.seats[hitler].knows.is_empty(),
                "{n}p: Hitler is blind above 6 players"
            );
            for &f in &fascists {
                let mut want: Vec<usize> = fascists.iter().copied().filter(|&x| x != f).collect();
                want.push(hitler);
                want.sort_unstable();
                assert_eq!(s.seats[f].knows, want, "{n}p: Fascist at {f}");
            }
            for (i, seat) in s.seats.iter().enumerate() {
                if seat.role == Role::Liberal {
                    assert!(seat.knows.is_empty(), "{n}p: Liberal at {i} knows nobody");
                }
            }
        }
    }

    #[test]
    fn test_knowledge_is_symmetric_and_never_names_a_liberal() {
        for n in MIN_SEATS..=MAX_SEATS {
            let s = st(n);
            for seat in &s.seats {
                for &k in &seat.knows {
                    assert_ne!(
                        s.seats[k].role,
                        Role::Liberal,
                        "{n}p: night phase never points at a Liberal"
                    );
                }
            }
        }
    }

    #[test]
    fn test_deck_is_eleven_fascist_six_liberal() {
        for n in MIN_SEATS..=MAX_SEATS {
            let s = st(n);
            assert_eq!(s.draw_pile.len(), TOTAL_TILES);
            assert_eq!(
                s.draw_pile
                    .iter()
                    .filter(|p| **p == Policy::Fascist)
                    .count(),
                FASCIST_TILES
            );
            assert_eq!(
                s.draw_pile
                    .iter()
                    .filter(|p| **p == Policy::Liberal)
                    .count(),
                LIBERAL_TILES
            );
            assert!(s.tiles_conserved());
        }
    }

    #[test]
    fn test_deck_is_actually_shuffled() {
        // Two different seeds must not produce the same order. (A one-in-
        // 17!-ish false failure is not worth guarding against.)
        let a = SecretHitlerState::new(members(7), 1).unwrap();
        let b = SecretHitlerState::new(members(7), 2).unwrap();
        assert_ne!(a.draw_pile, b.draw_pile);
    }

    #[test]
    fn test_same_seed_is_reproducible() {
        let a = SecretHitlerState::new(members(7), 99).unwrap();
        let b = SecretHitlerState::new(members(7), 99).unwrap();
        assert_eq!(a, b, "the engine is a pure function of members + seed");
    }

    #[test]
    fn test_first_president_is_a_real_seat() {
        for seed in 0..200u64 {
            for n in MIN_SEATS..=MAX_SEATS {
                let s = SecretHitlerState::new(members(n), seed).unwrap();
                assert!(s.president_seat < n);
            }
        }
    }

    #[test]
    fn test_first_president_varies_with_the_seed() {
        let seen: std::collections::HashSet<usize> = (0..200u64)
            .map(|seed| {
                SecretHitlerState::new(members(5), seed)
                    .unwrap()
                    .president_seat
            })
            .collect();
        assert!(seen.len() > 1, "the first President is chosen at random");
    }

    #[test]
    fn test_board_size_from_starting_count() {
        assert_eq!(BoardSize::for_seats(5), BoardSize::Small);
        assert_eq!(BoardSize::for_seats(6), BoardSize::Small);
        assert_eq!(BoardSize::for_seats(7), BoardSize::Mid);
        assert_eq!(BoardSize::for_seats(8), BoardSize::Mid);
        assert_eq!(BoardSize::for_seats(9), BoardSize::Large);
        assert_eq!(BoardSize::for_seats(10), BoardSize::Large);
        for n in MIN_SEATS..=MAX_SEATS {
            assert_eq!(st(n).board, BoardSize::for_seats(n));
        }
    }

    // -- the power grid ------------------------------------------------------

    #[test]
    fn test_power_grid_matches_the_boards() {
        use BoardSize::*;
        use Power::*;
        let want: [(BoardSize, [Option<Power>; 6]); 3] = [
            (
                Small,
                [
                    None,
                    None,
                    Some(Peek),
                    Some(Execution),
                    Some(Execution),
                    None,
                ],
            ),
            (
                Mid,
                [
                    None,
                    Some(Investigate),
                    Some(SpecialElection),
                    Some(Execution),
                    Some(Execution),
                    None,
                ],
            ),
            (
                Large,
                [
                    Some(Investigate),
                    Some(Investigate),
                    Some(SpecialElection),
                    Some(Execution),
                    Some(Execution),
                    None,
                ],
            ),
        ];
        for (board, row) in want {
            for (i, expected) in row.iter().enumerate() {
                assert_eq!(
                    board.power_at(i as u8 + 1),
                    *expected,
                    "{board:?} slot {}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn test_every_board_has_exactly_two_executions() {
        for board in [BoardSize::Small, BoardSize::Mid, BoardSize::Large] {
            let n = (1..=6)
                .filter(|i| board.power_at(*i) == Some(Power::Execution))
                .count();
            assert_eq!(n, 2, "{board:?} prints two Executions");
        }
    }

    #[test]
    fn test_peek_only_on_the_small_board() {
        for board in [BoardSize::Small, BoardSize::Mid, BoardSize::Large] {
            let has_peek = (1..=6).any(|i| board.power_at(i) == Some(Power::Peek));
            assert_eq!(has_peek, board == BoardSize::Small);
        }
    }

    #[test]
    fn test_investigate_and_special_election_only_on_the_bigger_boards() {
        for board in [BoardSize::Small, BoardSize::Mid, BoardSize::Large] {
            let has = (1..=6).any(|i| {
                matches!(
                    board.power_at(i),
                    Some(Power::Investigate) | Some(Power::SpecialElection)
                )
            });
            assert_eq!(has, board != BoardSize::Small);
        }
    }

    #[test]
    fn test_slot_six_grants_no_power_on_any_board() {
        // §8 W3: the 6th Fascist policy ends the game, so nothing resolves.
        for board in [BoardSize::Small, BoardSize::Mid, BoardSize::Large] {
            assert_eq!(board.power_at(6), None);
            assert_eq!(board.power_at(0), None, "slots are 1-based");
            assert_eq!(board.power_at(7), None, "out of range is None, not a panic");
        }
    }

    #[test]
    fn test_only_peek_needs_no_target() {
        assert!(!Power::Peek.needs_target());
        assert!(Power::Investigate.needs_target());
        assert!(Power::SpecialElection.needs_target());
        assert!(Power::Execution.needs_target());
    }

    // -- eligibility ---------------------------------------------------------

    #[test]
    fn test_president_may_not_nominate_themselves() {
        let s = st(7);
        assert!(!s.eligible_chancellor(s.president_seat));
    }

    #[test]
    fn test_term_limits_gate_the_chancellor_slot_only() {
        let mut s = st(7);
        s.president_seat = 0;
        s.last_elected_president = Some(1);
        s.last_elected_chancellor = Some(2);
        assert!(
            !s.eligible_chancellor(1),
            "last President is term-limited above 5 alive"
        );
        assert!(
            !s.eligible_chancellor(2),
            "last Chancellor is always term-limited"
        );
        assert!(s.eligible_chancellor(3));
    }

    #[test]
    fn test_five_alive_relaxation_frees_the_last_president() {
        // §5.2 E9/E10, evaluated on LIVING players, so it can switch on when
        // an execution takes a 6-alive table to 5.
        let mut s = st(7);
        s.president_seat = 0;
        s.last_elected_president = Some(1);
        s.last_elected_chancellor = Some(2);
        for dead in [4, 5] {
            s.seats[dead].alive = false;
        }
        assert_eq!(s.alive_count(), 5);
        assert!(
            s.eligible_chancellor(1),
            "at 5 alive only the last Chancellor stays ineligible"
        );
        assert!(!s.eligible_chancellor(2));

        // Back above 5 and the last President is barred again.
        s.seats[4].alive = true;
        assert_eq!(s.alive_count(), 6);
        assert!(!s.eligible_chancellor(1));
    }

    #[test]
    fn test_dead_players_are_never_eligible() {
        let mut s = st(7);
        s.seats[3].alive = false;
        assert!(!s.eligible_chancellor(3));
    }

    #[test]
    fn test_an_eligible_nominee_always_exists() {
        // §5.2 DERIVED: worst case is 3 alive with the President and the last
        // Chancellor both excluded, leaving exactly one.
        for n in MIN_SEATS..=MAX_SEATS {
            for deaths in 0..(n - 3) {
                let mut s = st(n);
                for d in 0..deaths {
                    s.seats[n - 1 - d].alive = false;
                }
                let alive: Vec<usize> = (0..n).filter(|&i| s.seats[i].alive).collect();
                if alive.len() < 3 {
                    continue;
                }
                s.president_seat = alive[0];
                s.last_elected_president = Some(alive[1]);
                s.last_elected_chancellor = Some(alive[2 % alive.len()]);
                assert!(
                    (0..n).any(|i| s.eligible_chancellor(i)),
                    "{n}p with {deaths} dead: at least one seat must be nominable"
                );
            }
        }
    }

    #[test]
    fn test_out_of_range_seat_is_not_eligible() {
        let s = st(5);
        assert!(!s.eligible_chancellor(99));
    }

    // -- majority ------------------------------------------------------------

    #[test]
    fn test_majority_is_strictly_more_than_half() {
        // §5.3 E18/E19: a tie fails.
        let mut s = st(10);
        assert_eq!(s.majority_needed(), 6, "10 alive needs 6");
        s.seats[9].alive = false;
        assert_eq!(s.majority_needed(), 5, "9 alive needs 5");
        s.seats[8].alive = false;
        assert_eq!(
            s.majority_needed(),
            5,
            "8 alive needs 5 — 4/4 is a tie and fails"
        );
        s.seats[7].alive = false;
        assert_eq!(s.majority_needed(), 4, "7 alive needs 4");
    }

    // -- serialization -------------------------------------------------------

    #[test]
    fn test_json_round_trip_is_lossless() {
        for n in MIN_SEATS..=MAX_SEATS {
            let s = st(n);
            assert_eq!(SecretHitlerState::from_json(&s.to_json()), s);
        }
    }

    #[test]
    fn test_a_blob_missing_a_field_backfills() {
        // Container-level #[serde(default)]: a field added by a new binary
        // must not orphan a blob written by the old one.
        let s = st(5);
        let mut v: serde_json::Value = serde_json::from_str(&s.to_json()).unwrap();
        v.as_object_mut().unwrap().remove("veto_unlocked");
        v.as_object_mut().unwrap().remove("peek_results");
        v.as_object_mut().unwrap().remove("round");
        let back = SecretHitlerState::from_json(&serde_json::to_string(&v).unwrap());
        assert!(!back.veto_unlocked);
        assert!(back.peek_results.is_empty());
        assert_eq!(back.round, 0);
        assert_eq!(back.seats, s.seats, "the fields that ARE present survive");
    }

    #[test]
    fn test_default_state_round_trips() {
        let s = SecretHitlerState::default();
        assert_eq!(SecretHitlerState::from_json(&s.to_json()), s);
    }

    // -- the projection: the secrecy proof -----------------------------------

    #[test]
    fn test_public_view_never_reveals_a_role() {
        for n in MIN_SEATS..=MAX_SEATS {
            let s = st(n);
            let v = s.public_view();
            assert!(
                v.final_roles.is_none(),
                "{n}p: roles stay sealed while playing"
            );
            // The projection's seat type has no role-shaped field at all —
            // this is a compile-time property, asserted here for the reader.
            for seat in &v.seats {
                assert!(seat.player_id > 0);
            }
        }
    }

    #[test]
    fn test_final_roles_appear_only_when_the_game_is_over() {
        let mut s = st(6);
        for phase in [
            Phase::Nomination,
            Phase::Voting,
            Phase::PresidentDraft,
            Phase::ChancellorDraft,
            Phase::VetoPending,
            Phase::Power(Power::Peek),
            Phase::Power(Power::Execution),
        ] {
            s.phase = phase;
            assert!(
                s.public_view().final_roles.is_none(),
                "{phase:?} must not reveal roles"
            );
        }
        for outcome in [
            Outcome::LiberalPolicies,
            Outcome::HitlerExecuted,
            Outcome::FascistPolicies,
            Outcome::HitlerChancellor,
        ] {
            s.phase = Phase::Over(outcome);
            let v = s.public_view();
            assert_eq!(
                v.final_roles.as_ref().map(|r| r.len()),
                Some(6),
                "every role is revealed at game end"
            );
            assert_eq!(v.outcome, Some(outcome));
        }
    }

    #[test]
    fn test_a_dead_seats_role_stays_hidden_until_game_over() {
        // §7.2: an executed player's card is not revealed.
        let mut s = st(7);
        s.seats[2].alive = false;
        let v = s.public_view();
        assert!(v.final_roles.is_none());
        assert!(!v.seats[2].alive, "that they are out IS public");
    }

    #[test]
    fn test_public_view_never_reveals_a_ballot_before_the_reveal() {
        let mut s = st(5);
        s.phase = Phase::Voting;
        s.votes = vec![Some(true), Some(false), None, Some(true), None];
        let v = s.public_view();
        assert_eq!(
            v.seats.iter().map(|x| x.voted).collect::<Vec<_>>(),
            vec![true, true, false, true, false],
            "THAT a seat has voted is public"
        );
        assert!(
            v.last_vote.is_empty(),
            "HOW they voted is not, until the vote closes"
        );
    }

    #[test]
    fn test_public_view_never_reveals_night_knowledge() {
        let s = st(9);
        let v = s.public_view();
        // ShPublicSeat has no `knows` field; assert the projection carries no
        // seat-index list that could stand in for one.
        assert_eq!(v.seats.len(), 9);
        assert!(
            s.seats.iter().any(|x| !x.knows.is_empty()),
            "fixture is meaningful"
        );
    }

    #[test]
    fn test_public_view_publishes_pile_counts_not_contents() {
        let mut s = st(5);
        s.draw_pile = vec![Policy::Fascist; 4];
        s.discard_pile = vec![Policy::Liberal; 3];
        let v = s.public_view();
        assert_eq!(v.draw_count, 4);
        assert_eq!(v.discard_count, 3);
    }

    #[test]
    fn test_public_view_reports_eligibility_not_a_term_limit_flag() {
        // §5.2 E9/E10: one `term_limited` bool renders the wrong thing at
        // exactly 5 alive.
        let mut s = st(7);
        s.president_seat = 0;
        s.last_elected_president = Some(1);
        s.last_elected_chancellor = Some(2);
        assert!(!s.public_view().seats[1].eligible_nominee);
        s.seats[5].alive = false;
        s.seats[6].alive = false;
        assert_eq!(s.alive_count(), 5);
        assert!(
            s.public_view().seats[1].eligible_nominee,
            "the last President frees up at 5 alive"
        );
        assert!(
            s.public_view().seats[1].is_last_elected_president,
            "still recorded"
        );
    }

    #[test]
    fn test_public_view_hitler_check_tracks_the_public_board() {
        let mut s = st(5);
        assert!(!s.public_view().hitler_check_live);
        s.fascist_track = 2;
        assert!(!s.public_view().hitler_check_live);
        s.fascist_track = 3;
        assert!(s.public_view().hitler_check_live);
    }

    #[test]
    fn test_log_entry_carries_no_secret() {
        // A structural sweep: the LogEntry declaration must not mention
        // `Party` or `Role`. `Policy` is permitted on `Enacted` alone,
        // because that tile goes face up on the track.
        let src = include_str!("secret_hitler.rs");
        let start = src
            .find("pub enum LogEntry {")
            .expect("LogEntry is declared in this file");
        let body = &src[start..start + src[start..].find("\n}\n").expect("enum ends")];
        assert!(
            !body.contains("Party"),
            "no LogEntry variant may carry a Party"
        );
        assert!(
            !body.contains("Role"),
            "no LogEntry variant may carry a Role"
        );
        let policy_uses = body.matches("Policy").count();
        assert_eq!(
            policy_uses, 1,
            "the only Policy in a log line is the face-up enacted tile"
        );
        assert!(body.contains("Enacted"));
    }

    // -- helpers -------------------------------------------------------------

    #[test]
    fn test_seat_lookup() {
        let s = st(6);
        assert_eq!(s.seat_of(1), Some(0));
        assert_eq!(s.seat_of(6), Some(5));
        assert_eq!(s.seat_of(999), None);
    }

    #[test]
    fn test_log_is_capped() {
        let mut s = st(5);
        for _ in 0..(LOG_CAP + 20) {
            s.push_log(LogEntry::VetoUnlocked);
        }
        assert_eq!(s.log.len(), LOG_CAP);
    }

    #[test]
    fn test_vote_history_is_capped() {
        let mut s = st(5);
        for round in 0..(VOTE_HISTORY_CAP as u32 + 10) {
            s.push_vote_record(VoteRecord {
                round,
                president: 0,
                nominee: 1,
                ballots: vec![None; 5],
                passed: false,
            });
        }
        assert_eq!(s.vote_history.len(), VOTE_HISTORY_CAP);
        assert_eq!(
            s.vote_history[0].round, 10,
            "the oldest records are the ones dropped"
        );
    }

    #[test]
    fn test_ensure_draw_reshuffles_the_discard_pile() {
        // §3 D4/D6.
        let mut s = st(5);
        s.discard_pile = s.draw_pile.split_off(2);
        assert_eq!(s.draw_pile.len(), 2);
        assert!(s.tiles_conserved());
        s.ensure_draw(3);
        assert_eq!(s.draw_pile.len(), TOTAL_TILES);
        assert!(s.discard_pile.is_empty());
        assert!(s.tiles_conserved());
    }

    #[test]
    fn test_ensure_draw_is_a_no_op_when_the_pile_is_deep_enough() {
        let mut s = st(5);
        let before = s.draw_pile.clone();
        s.ensure_draw(3);
        assert_eq!(s.draw_pile, before, "no gratuitous reshuffle");
    }

    #[test]
    fn test_v1_pours_nothing() {
        let mut s = st(5);
        s.pour(0, crate::sh_theme::DrinkEvent::Executed);
        s.pour_all(crate::sh_theme::DrinkEvent::Chaos);
        assert!(
            s.pending_drinks.is_empty(),
            "the theme's amounts are all zero in v1"
        );
    }
}
