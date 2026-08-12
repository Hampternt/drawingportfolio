//! Last Call card-game engine — a pure state machine, no I/O, no SQL, no RNG.
//!
//! Slice 1 defines the object model, the `PublicView` confidentiality
//! projection and a placeholder-dealing `set_vessel`. Slice 3 (the loop)
//! fills in the beat transitions: `arm`/`disarm`/`lock_in` stage secret
//! plays (§3.4.1), `advance_beat` walks the six beats and charges pulls at
//! Lock→Reveal (`reveal`), and `resolve()` runs the beat-6 program —
//! ordered damage/heal/curse resolution, elimination (D11), effect ticking
//! and expiry (D10), the hand soft cap (D12) and the round rollover (D5,
//! D13) — or freezes the table at the final tableau when `outcome()` is
//! `Some` (D16). `LastCallState` round-trips losslessly through
//! `to_json`/`from_json` because later tasks snapshot it into a DB column
//! between requests.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Deck {
    Beer,
    Cider,
    Wine,
    Liquor,
    Soft,
}

impl Deck {
    pub const ALL: [Deck; 5] = [
        Deck::Beer,
        Deck::Cider,
        Deck::Wine,
        Deck::Liquor,
        Deck::Soft,
    ];

    /// A deck constant, not a volume — DDv2 §3.2. Beer 8, Cider 10, Wine 6,
    /// Liquor 4, Soft 6.
    pub fn pulls(self) -> u8 {
        match self {
            Deck::Beer => 8,
            Deck::Cider => 10,
            Deck::Wine => 6,
            Deck::Liquor => 4,
            Deck::Soft => 6,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Deck::Beer => "beer",
            Deck::Cider => "cider",
            Deck::Wine => "wine",
            Deck::Liquor => "liquor",
            Deck::Soft => "soft",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Deck::Beer => "BEER",
            Deck::Cider => "CIDER",
            Deck::Wine => "WINE",
            Deck::Liquor => "LIQUOR",
            Deck::Soft => "SOFT",
        }
    }

    pub fn from_slug(s: &str) -> Option<Deck> {
        match s {
            "beer" => Some(Deck::Beer),
            "cider" => Some(Deck::Cider),
            "wine" => Some(Deck::Wine),
            "liquor" => Some(Deck::Liquor),
            "soft" => Some(Deck::Soft),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Beat {
    #[default]
    Draw,
    Deal,
    Diplomacy,
    Lock,
    Reveal,
    Resolve,
}

impl Beat {
    pub const ORDER: [Beat; 6] = [
        Beat::Draw,
        Beat::Deal,
        Beat::Diplomacy,
        Beat::Lock,
        Beat::Reveal,
        Beat::Resolve,
    ];

    pub fn index(self) -> u8 {
        match self {
            Beat::Draw => 1,
            Beat::Deal => 2,
            Beat::Diplomacy => 3,
            Beat::Lock => 4,
            Beat::Reveal => 5,
            Beat::Resolve => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Beat::Draw => "DRAW",
            Beat::Deal => "DEAL",
            Beat::Diplomacy => "DIPLOMACY",
            Beat::Lock => "LOCK",
            Beat::Reveal => "REVEAL",
            Beat::Resolve => "RESOLVE",
        }
    }

    /// The `data-beat` value.
    pub fn slug(self) -> &'static str {
        match self {
            Beat::Draw => "draw",
            Beat::Deal => "deal",
            Beat::Diplomacy => "diplomacy",
            Beat::Lock => "lock",
            Beat::Reveal => "reveal",
            Beat::Resolve => "resolve",
        }
    }

    pub fn hue(self) -> &'static str {
        match self {
            Beat::Draw => "amber",
            Beat::Deal => "amber",
            Beat::Diplomacy => "mint",
            Beat::Lock => "violet",
            Beat::Reveal => "azure",
            Beat::Resolve => "rose",
        }
    }

    /// Wraps Resolve -> Draw.
    pub fn next(self) -> Beat {
        match self {
            Beat::Draw => Beat::Deal,
            Beat::Deal => Beat::Diplomacy,
            Beat::Diplomacy => Beat::Lock,
            Beat::Lock => Beat::Reveal,
            Beat::Reveal => Beat::Resolve,
            Beat::Resolve => Beat::Draw,
        }
    }

    /// None for the auto beats (Deal, Resolve) — Plan E's ticker consumes
    /// this and does not display a countdown for either. DDv2 §5.
    pub fn duration_secs(self) -> Option<u16> {
        match self {
            Beat::Draw => Some(DRAW_SECS),
            Beat::Deal => None,
            Beat::Diplomacy => Some(DIPLOMACY_SECS),
            Beat::Lock => Some(LOCK_SECS),
            Beat::Reveal => Some(REVEAL_SECS),
            Beat::Resolve => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Atk,
    Buff,
    Curse,
    Util,
    Reaction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Alive,
    Eliminated,
}

impl Status {
    /// The `data-status` value.
    pub fn slug(self) -> &'static str {
        match self {
            Status::Alive => "alive",
            Status::Eliminated => "eliminated",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vessel {
    pub deck: Deck,
    pub pulls_max: u8,
    pub pulls_left: u8,
    pub container: String,
}

/// `title` is NOT in DDv2 §1's object model — card titles are shown on
/// CardFace and CardMini and have to live somewhere, so they are folded into
/// `Card` rather than kept in a parallel lookup.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Card {
    pub id: String,
    pub deck: Deck,
    pub kind: CardKind,
    pub cost: u8,
    pub targets: String,
    pub title: String,
    pub text: String,
    pub keywords: Vec<String>,
    pub duration: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LcPlayer {
    pub seat: usize,
    pub player_id: i64,
    pub name: String,
    pub hp: i32,
    pub handicap_pct: u16,
    pub vessels: Vec<Vessel>,
    pub hand: Vec<Card>,
    pub armed: Vec<ArmedCard>,
    pub locked: bool,
    pub drawing: bool,
    pub draws_this_round: u16,
    pub tabs: Vec<String>,
    pub status: Status,
}

/// A staged card: identity plus its declared target. Never projected —
/// `public_view()` does not read `LcPlayer::armed` at all (only `locked`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArmedCard {
    pub card: Card,
    pub target: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Play {
    pub card: Card,
    pub source_seat: usize,
    pub target: Option<usize>,
    pub paid_from: Deck,
    pub order_key: u32,
}

/// The op vocabulary (Plan F authors real cards against this). "Persists"
/// means it is stored as an `Effect` on the room; immediate ops apply during
/// resolution and are never stored.
///
/// | op | persists | semantics |
/// | --- | --- | --- |
/// | `Damage` | no | subtract `magnitude` from subject HP, shields first (in effect-creation order), clamp HP at 0, elimination check immediately (7.6) |
/// | `Heal` | no | add `magnitude` to subject HP; no ceiling (TBD-3) |
/// | `Shield` | yes | absorbs damage up to `magnitude` until `expires_round`; `magnitude` is consumed as it absorbs; removed at 0 |
/// | `Dot` | yes | `magnitude` damage to subject at each `resolve()` after its creation round, through `expires_round` |
/// | `PullDrain` | no | `magnitude` times, decrement the subject's fullest vessel's `pulls_left` (tie: lowest index), floor at 0 (F4) — never touches HP |
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectOp {
    Damage,
    Heal,
    Shield,
    Dot,
    PullDrain,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Effect {
    /// The `Play.order_key` that created this effect — NOT a unique
    /// identity. `order_key` resets to 1 at every reveal (DDv2 §1), so the
    /// same `source_play` value recurs across rounds and could even collide
    /// with a different play's `order_key` in a later round (M5, review
    /// finding). Harmless today — nothing dereferences it — but if a future
    /// LOG tab or "what caused this" lookup ever treats it as an identity,
    /// it needs pairing with `round` (or a real global play id) first.
    pub source_play: u32,
    pub subject: usize,
    pub op: EffectOp,
    pub magnitude: i32,
    pub expires_round: u32,
}

/// A formed pact between two seats — mutual and symmetric; the invariant
/// `a < b` makes the value order-independent of which seat's offer closed it
/// (Step 2/3 of `offer_pact`/`accept_pact`). NEVER projected by
/// `public_view()` (G13; the §3.4.1 pattern applied to pacts) — pacts stay
/// secret until a betrayal or the win exposes them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pact {
    pub a: usize,
    pub b: usize,
    pub formed_round: u32,
}

/// A pending, one-directional pact proposal made during `Beat::Diplomacy`.
/// Beat-scoped (G8): `advance_beat`'s Diplomacy→Lock edge clears every
/// pending offer unconditionally, so none can ever dangle into another beat.
/// NEVER projected by `public_view()`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PactOffer {
    pub from: usize,
    pub to: usize,
}

/// A record of a pact broken by betrayal (Task 2). Unlike `Pact` and
/// `PactOffer`, this one IS read by `public_view()` (G5) — the single pact
/// field the room is allowed to see.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PactBreak {
    pub betrayer: usize,
    pub betrayed: usize,
    pub round: u32,
}

/// `#[serde(default)]` at the container level: a slice-3 field addition to
/// `LastCallState` (never mind `LcPlayer`/`Card`/`Play` themselves, which
/// stay strict — see `from_json`) makes an in-flight room's stored blob,
/// written by the previous binary, best-effort load with the new field at
/// its `Default` value instead of a permanent panic. Nested structs
/// (`LcPlayer`, `Card`, `Play`, `Vessel`, `Effect`) deliberately do NOT get
/// the same treatment: `Deck`/`CardKind`/`Status` have no sensible default
/// variant to fall back to, so a field missing *inside* one of those is
/// correctly still a hard error — that is a corrupt blob, not version skew.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct LastCallState {
    pub players: Vec<LcPlayer>,
    pub round: u32,
    pub beat: Beat,
    pub first_seat: usize,
    pub rng_seed: u64,
    pub plays: Vec<Play>,
    /// §3.4.1: locked-but-unrevealed plays. `public_view()` NEVER reads this
    /// field — only `Task 4`'s reveal step moves entries from here into
    /// `plays`, at which point they become eligible for `public_view`'s
    /// `revealed` projection. See
    /// `test_a_locked_play_is_absent_from_public_view_before_reveal`.
    pub locked_plays: Vec<Play>,
    pub effects: Vec<Effect>,
    pub discards: Vec<Card>,
    pub deck_counts: Vec<(Deck, u16)>,
    pub seq: u64,
    /// Unix ms when the current timed beat expires. `None` = untimed — the
    /// round-1 Draw registration lobby (E1), an auto beat (Deal/Resolve,
    /// which collapse in the same `lc_advance_chain` pass), or the frozen
    /// final tableau after game-over (D16). DATA ONLY: written and read by
    /// `lc_routes` (the ticker and the action routes) — the engine itself
    /// never calls a clock function or reads this field.
    pub beat_deadline_ms: Option<i64>,
    /// Secret, mutual pacts (G13). NEVER projected by `public_view()` — the
    /// same structural move as `locked_plays`: the method simply never reads
    /// this field. See `test_pacts_and_offers_never_reach_the_public_view`.
    pub pacts: Vec<Pact>,
    /// Pending, one-directional offers made during `Beat::Diplomacy`.
    /// Beat-scoped (G8): cleared unconditionally at the Diplomacy→Lock edge
    /// in `advance_beat`. NEVER projected by `public_view()`.
    pub pact_offers: Vec<PactOffer>,
    /// Seats barred from the pact market by a betrayal (Task 2). NEVER
    /// projected as a field in its own right — it is fully derivable from
    /// `pact_breaks`, which IS public (G5), so exposing this one too would
    /// be redundant, not merely secret.
    pub pact_barred: Vec<usize>,
    /// Betrayal records. The ONE pact field `public_view()` may read (G5,
    /// Task 2).
    pub pact_breaks: Vec<PactBreak>,
    /// The round's revealed event id, or `None` during Draw / before round
    /// 1's Deal / after a rollover. At most one event is representable —
    /// DDv2 §10.1's "never two at once" is this type (H2). Set at the
    /// Draw→Deal reveal (`advance_beat`) and cleared at the rollover
    /// (`resolve`) — see those functions for the exact edges.
    pub event: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicVessel {
    pub deck: Deck,
    pub pulls_left: u8,
    pub pulls_max: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicSeat {
    pub seat: usize,
    pub player_id: i64,
    pub name: String,
    pub hp: i32,
    pub status: Status,
    pub vessels: Vec<PublicVessel>,
    pub hand_len: usize,
    pub locked: bool,
    pub drawing: bool,
    /// Cards drawn this round — the plaque's deck-tinted badge (D.1 row 2).
    /// Projected from `LcPlayer::draws_this_round`, which the Draw beat sets
    /// in slice 3 and nothing sets here.
    pub draws: u16,
}

impl PublicSeat {
    /// One deck per vessel, in order.
    pub fn decks(&self) -> Vec<Deck> {
        self.vessels.iter().map(|v| v.deck).collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicView {
    pub seats: Vec<PublicSeat>,
    pub round: u32,
    pub beat: Beat,
    pub first_seat: usize,
    pub deck_counts: Vec<(Deck, u16)>,
    pub discard_count: usize,
    pub revealed: Vec<Play>,
    pub seq: u64,
    pub outcome: Option<LcOutcome>,
    /// Projected verbatim from `LastCallState::beat_deadline_ms` — see that
    /// field's doc comment.
    pub beat_deadline_ms: Option<i64>,
    /// G5, Task 2: the one pact field the room may see. Projected verbatim
    /// from `LastCallState::pact_breaks` — every other pact field
    /// (`pacts`/`pact_offers`/`pact_barred`) stays off this struct entirely
    /// (G10/G13).
    pub pact_breaks: Vec<PactBreak>,
    /// Projected verbatim from `LastCallState::event` — events are public
    /// (H2); only the CURRENT event is ever exposed, never the next one.
    pub event: Option<String>,
}

/// DDv2 9.3 — the two solo ways a game ends — plus the pact win (G2, Task 2).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LcOutcome {
    /// The winning seat.
    Winner(usize),
    /// All remaining players are ghosts (DDv2 9.3).
    Draw,
    /// G2 — DDv1 6.3: the last two standing, still pacted, share the win.
    /// The two seats, `a < b` (seat-ordered, per `outcome()`'s match).
    Pact(usize, usize),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LcError {
    NotSeated,
    BadHandicap,
    /// Action not legal in the current beat.
    WrongBeat,
    /// Eliminated players act on nothing.
    NotAlive,
    /// arm/disarm/set_target after lock_in.
    AlreadyLocked,
    /// card_id not in the expected zone.
    UnknownCard,
    /// A Reaction card at arm time (D9).
    NotPlayable,
    /// lock/arm validation, naming the card (DDv2 6.3).
    CantAfford(String),
    /// A targets=="one" card with no target at lock.
    NeedsTarget(String),
    /// set_target: bad seat / dead seat / class mismatch.
    BadTarget,
    /// finish_and_draw: bad vessel, second draw, bad batch.
    BadDraw,
    /// advance_beat at Resolve — call resolve() instead.
    MustResolve,
    /// offer_pact/accept_pact/decline_pact: the actor is pacted or barred,
    /// or (offer_pact only) fewer than `PACT_MIN_ALIVE` players are Alive.
    PactBlocked,
    /// accept_pact/decline_pact: no pending offer from that seat to this one.
    NoOffer,
}

pub const STARTING_HP: i32 = 15; // DDv2 §2.4, TBD-1
pub const MAX_SEATS: usize = 8; // DDv2 §2.1 (2-8)
pub const HANDICAP_MIN_PCT: u16 = 25;
pub const HANDICAP_MAX_PCT: u16 = 300;
/// Under this many cards a DeckStack count turns amber (`data-low`).
pub const DECK_LOW_THRESHOLD: u16 = 5;

pub const DRAW_SECS: u16 = 30; // DDv2 §5 beat 1
pub const DIPLOMACY_SECS: u16 = 60; // DDv2 §5 beat 3, TBD-6
pub const LOCK_SECS: u16 = 45; // DDv2 §5 beat 4
pub const REVEAL_SECS: u16 = 20; // DDv2 §5 beat 5
pub const DRAW_PER_VESSEL: usize = 5; // DDv2 §4.3, TBD-4
pub const HAND_SOFT_CAP: usize = 12; // DDv2 §8.2, TBD-2
pub const LC_DECK_SIZE: u16 = 40; // 40-card shoe size, test-pinned per Plan F
/// Fewer than this many Alive players and the pact market is closed (G7) —
/// `offer_pact` refuses even a valid target once this floor is breached.
pub const PACT_MIN_ALIVE: usize = 4;

/// Handicap is a percentage (100 = no handicap). Rounds UP, per DDv2 §11.
/// Integer maths on purpose: a float handicap would let a form field carry
/// NaN/inf into the state blob and break both serde equality and `ceil()`.
/// `u32::div_ceil` computes the same value as the brief's literal
/// `(cost * pct + 99) / 100` without tripping clippy's `manual_div_ceil`.
pub fn pull_cost(cost: u8, handicap_pct: u16) -> u8 {
    (cost as u32 * handicap_pct as u32).div_ceil(100) as u8
}

impl LastCallState {
    /// Seats `members` in the order given (`seat` = index), everyone starting
    /// at `STARTING_HP` with no handicap, empty vessels/hand/armed/tabs,
    /// unlocked and not drawing, `Status::Alive`. Round 1, `Beat::Draw`,
    /// `first_seat = 0`, `seq = 0`. `deck_counts` is initialized from
    /// `Deck::ALL` at `0` — settable but never set by this slice (spec §4.1).
    ///
    /// Caps at `MAX_SEATS`: a room with more members than that (everyone
    /// pressed join before anyone pressed START) seats only the first
    /// `MAX_SEATS` and leaves the rest unseated — the same "in the room, not
    /// at the table" outcome `add_player` reaches for a mid-game join. This
    /// is the seating path `add_player`'s own ceiling never sees, because
    /// nothing here calls it (Plan B Task 6).
    pub fn new(members: Vec<(i64, String)>, rng_seed: u64) -> Self {
        let players = members
            .into_iter()
            .take(MAX_SEATS)
            .enumerate()
            .map(|(seat, (player_id, name))| LcPlayer {
                seat,
                player_id,
                name,
                hp: STARTING_HP,
                handicap_pct: 100,
                vessels: Vec::new(),
                hand: Vec::new(),
                armed: Vec::new(),
                locked: false,
                drawing: false,
                draws_this_round: 0,
                tabs: Vec::new(),
                status: Status::Alive,
            })
            .collect();
        LastCallState {
            players,
            round: 1,
            beat: Beat::Draw,
            first_seat: 0,
            rng_seed,
            plays: Vec::new(),
            locked_plays: Vec::new(),
            effects: Vec::new(),
            discards: Vec::new(),
            deck_counts: Deck::ALL.iter().map(|&d| (d, 0)).collect(),
            seq: 0,
            beat_deadline_ms: None, // E1: round 1's Draw is the untimed lobby
            pacts: Vec::new(),
            pact_offers: Vec::new(),
            pact_barred: Vec::new(),
            pact_breaks: Vec::new(),
            event: None,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("LastCallState is always serializable")
    }

    /// Deserializes a snapshot produced by `to_json`. Only ever called on
    /// this engine's own output, so a malformed-JSON parse failure (`""`,
    /// truncated, wrong shape entirely) is still a programming error and
    /// still panics. What no longer panics: a *missing field* at the
    /// `LastCallState` container level, e.g. a slice-3 addition to this
    /// struct read against a blob an older binary wrote — `#[serde(default)]`
    /// on the struct backfills it from `Default` instead. Fields missing
    /// inside a nested `LcPlayer`/`Card`/`Play` still panic; see the comment
    /// on the struct's `#[serde(default)]`.
    ///
    /// INVARIANT this creates: every writer must pass `Some(&st.to_json())`
    /// to `db::start_game` (Plan A2), because every reader does
    /// `from_json(game.state_json.as_deref().unwrap_or_default())` and `""`
    /// is not valid JSON.
    pub fn from_json(s: &str) -> Self {
        let mut st: LastCallState = serde_json::from_str(s).expect("valid LastCallState JSON");
        // The third path into `players` — LastCallState::new and add_player
        // both cap at MAX_SEATS; a blob persisted by a pre-ceiling binary must
        // not deserialize past the ring (seat_pos renders short and a real
        // player's plaque silently vanishes).
        st.players.truncate(MAX_SEATS);
        st
    }

    /// DDv2 9.3. None while the game is undecided — or while fewer than two
    /// players are seated, because a table of one has no game to win (D16).
    /// Plan E queries this after resolve() to decide end-of-game handling.
    pub fn outcome(&self) -> Option<LcOutcome> {
        if self.players.len() < 2 {
            return None;
        }
        let mut alive = self
            .players
            .iter()
            .filter(|p| p.status == Status::Alive)
            .map(|p| p.seat);
        match (alive.next(), alive.next(), alive.next()) {
            (None, _, _) => Some(LcOutcome::Draw),
            (Some(w), None, _) => Some(LcOutcome::Winner(w)),
            // G2 — DDv1 6.3: "If you and your partner are the last two
            // standing, you both win." `players` is seat-ordered, so a < b
            // holds whenever this arm matches.
            (Some(a), Some(b), None) if self.pacts.iter().any(|p| p.a == a && p.b == b) => {
                Some(LcOutcome::Pact(a, b))
            }
            _ => None,
        }
    }

    pub fn seat_of(&self, player_id: i64) -> Option<usize> {
        self.players
            .iter()
            .find(|p| p.player_id == player_id)
            .map(|p| p.seat)
    }

    /// Mid-game join. Mirrors `ThreeManState::add_player`: no-op (returns the
    /// existing seat) if already seated, otherwise pushes at
    /// `seat = players.len()` — unless the table is already at `MAX_SEATS`,
    /// in which case the newcomer is not seated at all and `None` comes
    /// back. Callers must not fail the join on `None`: an unseated member
    /// still belongs to the room, just not to this game (Plan B Task 6).
    ///
    /// Also refuses (M4, review finding) once `outcome()` is `Some` — a
    /// D16 freeze at `Beat::Resolve` is the final tableau, and seating a
    /// third player after a 1v1 ends would make `outcome()` re-evaluate to
    /// `None` (two alive again), retroactively blanking
    /// `public_view().outcome` while `beat` stayed frozen at `Resolve`. An
    /// already-seated member's idempotent replay is unaffected — that check
    /// runs first.
    ///
    /// Bumps `seq` exactly when a player is actually newly seated — not on
    /// the idempotent replay, not on the full-table or game-over `None` —
    /// because `seq` is the freshness floor every SSE-driven repaint
    /// compares against (`lcApply`/`lcApplyTable`'s `if (seq < lcSeq)
    /// return`), and two distinct seatings sharing one `seq` would let the
    /// client's equal-seq-is-a-harmless-duplicate allowance silently accept
    /// a stale repaint as current.
    pub fn add_player(&mut self, player_id: i64, name: &str) -> Option<usize> {
        if let Some(seat) = self.seat_of(player_id) {
            return Some(seat);
        }
        if self.outcome().is_some() {
            return None;
        }
        if self.players.len() >= MAX_SEATS {
            return None;
        }
        let seat = self.players.len();
        self.players.push(LcPlayer {
            seat,
            player_id,
            name: name.to_string(),
            hp: STARTING_HP,
            handicap_pct: 100,
            vessels: Vec::new(),
            hand: Vec::new(),
            armed: Vec::new(),
            locked: false,
            drawing: false,
            draws_this_round: 0,
            tabs: Vec::new(),
            status: Status::Alive,
        });
        self.seq += 1;
        Some(seat)
    }

    /// Registers the player's drink. `pulls_max` is a DECK constant
    /// (DDv2 §3.2); `container` is a free-text label and never affects it.
    /// Draw-beat-gated (D15) — game setup happens at round 1 Draw, so the
    /// route flow that calls this at table setup is unaffected.
    ///
    /// The vessel also seeds the player's hand with that deck's curated
    /// opening hand (F6) — no longer a slice-1 all-cards-in-the-deck stub.
    /// If no player currently holds a vessel of `deck`, its shoe activates
    /// at `LC_DECK_SIZE` (D6) before the deal, and the cards actually pushed
    /// to the hand (the same-deck-replace dedupe can push zero) are debited
    /// from the shoe.
    pub fn set_vessel(
        &mut self,
        player_id: i64,
        deck: Deck,
        container: &str,
    ) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Draw {
            return Err(LcError::WrongBeat);
        }
        if !self
            .players
            .iter()
            .any(|p| p.vessels.iter().any(|v| v.deck == deck))
        {
            if let Some(entry) = self.deck_counts.iter_mut().find(|(d, _)| *d == deck) {
                entry.1 = LC_DECK_SIZE;
            } else {
                self.deck_counts.push((deck, LC_DECK_SIZE));
            }
        }
        let p = &mut self.players[seat];
        p.vessels.retain(|v| v.deck != deck);
        p.vessels.push(Vessel {
            deck,
            pulls_max: deck.pulls(),
            pulls_left: deck.pulls(),
            container: container.to_string(),
        });
        let mut dealt: u16 = 0;
        for card in crate::lc_cards::opening_hand(deck) {
            if !p.hand.iter().any(|c| c.id == card.id) {
                p.hand.push(card);
                dealt += 1;
            }
        }
        if let Some(entry) = self.deck_counts.iter_mut().find(|(d, _)| *d == deck) {
            entry.1 = entry.1.saturating_sub(dealt);
        }
        self.seq += 1;
        Ok(())
    }

    /// DDv2 4.3 finish-&-draw, beat 1 only, once per player per round
    /// (TBD-5). `drawn` is decided by the caller (no RNG here): its length
    /// MUST equal `min(DRAW_PER_VESSEL, shoe count for the vessel's deck)`
    /// and every card MUST belong to that deck (D7: the min is what makes a
    /// short shoe legal). Empties-and-refills the vessel to `pulls_max`,
    /// debits the shoe, extends the hand, sets `drawing` and
    /// `draws_this_round`.
    pub fn finish_and_draw(
        &mut self,
        player_id: i64,
        vessel_idx: usize,
        drawn: Vec<Card>,
    ) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Draw {
            return Err(LcError::WrongBeat);
        }
        let p = &self.players[seat];
        if vessel_idx >= p.vessels.len() {
            return Err(LcError::BadDraw);
        }
        if p.drawing {
            return Err(LcError::BadDraw);
        }
        let deck = p.vessels[vessel_idx].deck;
        let shoe = self
            .deck_counts
            .iter()
            .find(|(d, _)| *d == deck)
            .map(|&(_, c)| c)
            .unwrap_or(0);
        let expected = (DRAW_PER_VESSEL as u16).min(shoe) as usize;
        if drawn.len() != expected || drawn.iter().any(|c| c.deck != deck) {
            return Err(LcError::BadDraw);
        }

        let p = &mut self.players[seat];
        p.vessels[vessel_idx].pulls_left = p.vessels[vessel_idx].pulls_max;
        let drawn_count = drawn.len() as u16;
        p.hand.extend(drawn);
        p.draws_this_round += drawn_count;
        p.drawing = true;
        if let Some(entry) = self.deck_counts.iter_mut().find(|(d, _)| *d == deck) {
            entry.1 = entry.1.saturating_sub(drawn_count);
        }
        self.seq += 1;
        Ok(())
    }

    /// Draw-beat-gated (D19, added post-review — mirrors D15's `set_vessel`
    /// gate): handicap is a "round boundary" setting exactly like vessels.
    /// Before this gate, a handicap raised between `lock_in` and `reveal`
    /// could inflate 7.1's ordering total (computed from the *current*
    /// `handicap_pct`) above what the reveal charge actually paid, buying
    /// initiative at a discount (review finding I1). Gating closes the
    /// window: vessels and handicap are now both frozen for the whole
    /// Lock/Reveal pair, so `payment_plan` re-derived at reveal always
    /// reproduces exactly what `lock_in` validated. Guard order: `NotSeated`
    /// -> `WrongBeat` -> `BadHandicap`.
    pub fn set_handicap(&mut self, target_id: i64, handicap_pct: u16) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(target_id) else {
            return Err(LcError::NotSeated);
        };
        if self.beat != Beat::Draw {
            return Err(LcError::WrongBeat);
        }
        if !(HANDICAP_MIN_PCT..=HANDICAP_MAX_PCT).contains(&handicap_pct) {
            return Err(LcError::BadHandicap);
        }
        self.players[seat].handicap_pct = handicap_pct;
        self.seq += 1;
        Ok(())
    }

    /// Projects to exactly what D.1/D.3 and F.2 legitimately display. Card
    /// identity survives only for plays already revealed — before beat
    /// Reveal, `revealed` is empty, so a broadcast fragment cannot contain an
    /// unrevealed card by construction. `armed` is never projected as a list
    /// or a count: DDv2 §6.3 is "show only a lock tick per seat", which is
    /// `locked`.
    ///
    /// INVARIANT this method depends on and does NOT itself enforce: nothing
    /// may enter `plays` before it is publicly revealable. This gate is a
    /// beat check only — `revealed` clones the whole of `self.plays` the
    /// instant `beat` becomes `Reveal`, with no per-play revealed flag. This
    /// is safe because `lock_in()` (§3.4.1) stages locked cards into
    /// `LastCallState::locked_plays`, which this method never reads at all —
    /// only Task 4's reveal step moves entries from `locked_plays` into
    /// `plays`, at which point they become eligible for `revealed`. See
    /// `test_public_view_never_reveals_before_the_reveal_beat` and
    /// `test_a_locked_play_is_absent_from_public_view_before_reveal` below,
    /// which pin both the general projection gate and the secrecy of
    /// locked-but-unrevealed plays specifically.
    pub fn public_view(&self) -> PublicView {
        PublicView {
            seats: self
                .players
                .iter()
                .map(|p| PublicSeat {
                    seat: p.seat,
                    player_id: p.player_id,
                    name: p.name.clone(),
                    hp: p.hp,
                    status: p.status,
                    vessels: p
                        .vessels
                        .iter()
                        .map(|v| PublicVessel {
                            deck: v.deck,
                            pulls_left: v.pulls_left,
                            pulls_max: v.pulls_max,
                        })
                        .collect(),
                    // DDv2 6.3: before the reveal, only the lock tick is
                    // public. hand_len must therefore not move while a
                    // player stages cards — armed and staged-locked cards
                    // still count as "in hand" to the room. After the
                    // Lock->Reveal flip both extra terms are structurally
                    // zero (armed cleared, locked_plays drained into plays),
                    // so the count drops exactly when the plays go public.
                    hand_len: p.hand.len()
                        + p.armed.len()
                        + self
                            .locked_plays
                            .iter()
                            .filter(|pl| pl.source_seat == p.seat)
                            .count(),
                    locked: p.locked,
                    drawing: p.drawing,
                    draws: p.draws_this_round,
                })
                .collect(),
            round: self.round,
            beat: self.beat,
            first_seat: self.first_seat,
            deck_counts: self.deck_counts.clone(),
            discard_count: self.discards.len(),
            revealed: match self.beat {
                Beat::Reveal | Beat::Resolve => self.plays.clone(),
                _ => Vec::new(),
            },
            seq: self.seq,
            outcome: self.outcome(),
            beat_deadline_ms: self.beat_deadline_ms,
            // G5: the sole pact field this projection reads.
            pact_breaks: self.pact_breaks.clone(),
            event: self.event.clone(),
        }
    }

    /// `pull_cost` with the active event applied (H12) — the ONLY charging
    /// entry point from here on. `happy-hour` halves the charged pulls,
    /// rounded up.
    pub fn effective_pull_cost(&self, cost: u8, handicap_pct: u16) -> u8 {
        let base = pull_cost(cost, handicap_pct);
        match self.event.as_deref().and_then(crate::lc_events::event_def) {
            Some(e) if e.hook == crate::lc_events::EventHook::CostHalf => base.div_ceil(2),
            _ => base,
        }
    }

    /// The seat's total charged pulls over its plays in `self.plays` —
    /// event-aware. Plan E's DRINK chip and Task 3's `SpentAtLeast` read
    /// this.
    pub fn charged_pulls(&self, seat: usize) -> u8 {
        let h = self.players.get(seat).map_or(100, |p| p.handicap_pct);
        self.plays
            .iter()
            .filter(|p| p.source_seat == seat)
            .map(|p| self.effective_pull_cost(p.card.cost, h))
            .sum()
    }

    // Slice 1 defines the shape; slice 3 (the loop) fills these in. The
    // object model is expensive to change later; transitions are not.

    /// DDv2 6.1/6.2. Guard order (pinned by `test_arm_guard_order`):
    /// `NotSeated` -> `NotAlive` -> `WrongBeat` (arming lives in `Beat::Lock`,
    /// DDv2 §5 beat 4) -> `AlreadyLocked` -> `UnknownCard` (not in hand) ->
    /// `NotPlayable` (a Reaction card, D9 — reactions never arm) ->
    /// `CantAfford` if `payment_plan` over the player's current armed cards
    /// plus this one fails (4.2 — checked early for UX; 6.3's lock-time check
    /// via the same helper remains authoritative).
    ///
    /// INVARIANT: an armed card is staged identity, not revealed identity —
    /// it moves onto `LcPlayer::armed`, never onto `self.plays`. See the
    /// doc comment on `public_view` and `locked_plays` (§3.4.1).
    pub fn arm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Lock {
            return Err(LcError::WrongBeat);
        }
        if self.players[seat].locked {
            return Err(LcError::AlreadyLocked);
        }
        let Some(idx) = self.players[seat].hand.iter().position(|c| c.id == card_id) else {
            return Err(LcError::UnknownCard);
        };
        let card = self.players[seat].hand[idx].clone();
        if card.kind == CardKind::Reaction {
            return Err(LcError::NotPlayable);
        }
        // Simulate: current armed cards plus this one, in arming order.
        let mut trial = self.players[seat].clone();
        trial.armed.push(ArmedCard {
            card: card.clone(),
            target: None,
        });
        let halved = matches!(
            self.event
                .as_deref()
                .and_then(crate::lc_events::event_def)
                .map(|e| e.hook),
            Some(crate::lc_events::EventHook::CostHalf)
        );
        payment_plan(&trial, halved)?;

        let p = &mut self.players[seat];
        p.hand.remove(idx);
        p.armed.push(ArmedCard { card, target: None });
        self.seq += 1;
        Ok(())
    }

    /// Guard order: `NotSeated` -> `NotAlive` -> `WrongBeat` ->
    /// `AlreadyLocked` -> `UnknownCard` (not armed). Success returns the card
    /// to `hand`; its target is dropped along with the `ArmedCard`.
    pub fn disarm(&mut self, player_id: i64, card_id: &str) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Lock {
            return Err(LcError::WrongBeat);
        }
        if self.players[seat].locked {
            return Err(LcError::AlreadyLocked);
        }
        let p = &mut self.players[seat];
        let Some(idx) = p.armed.iter().position(|a| a.card.id == card_id) else {
            return Err(LcError::UnknownCard);
        };
        let armed_card = p.armed.remove(idx);
        p.hand.push(armed_card.card);
        self.seq += 1;
        Ok(())
    }

    /// Guard order: `NotSeated` -> `NotAlive` -> `WrongBeat` ->
    /// `AlreadyLocked` -> `UnknownCard` (not armed) -> `BadTarget` (D2):
    /// `targets == "one"` requires `Some(seat)` naming a seat that exists and
    /// is `Alive` (self-targeting allowed); every other target class
    /// (`self`/`all`/`table`) requires `None`.
    pub fn set_target(
        &mut self,
        player_id: i64,
        card_id: &str,
        target: Option<usize>,
    ) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Lock {
            return Err(LcError::WrongBeat);
        }
        if self.players[seat].locked {
            return Err(LcError::AlreadyLocked);
        }
        let Some(idx) = self.players[seat]
            .armed
            .iter()
            .position(|a| a.card.id == card_id)
        else {
            return Err(LcError::UnknownCard);
        };
        let targets_class = self.players[seat].armed[idx].card.targets.clone();
        match targets_class.as_str() {
            "one" => match target {
                Some(t) => match self.players.get(t) {
                    Some(tp) if tp.status == Status::Alive => {}
                    _ => return Err(LcError::BadTarget),
                },
                None => return Err(LcError::BadTarget),
            },
            _ => {
                if target.is_some() {
                    return Err(LcError::BadTarget);
                }
            }
        }
        self.players[seat].armed[idx].target = target;
        self.seq += 1;
        Ok(())
    }

    /// Guard order: `NotSeated` -> `NotAlive` -> `WrongBeat` -> already
    /// locked (`Ok(())`, no seq bump — idempotent replay, the `add_player`
    /// precedent). Then, in arming order: `NeedsTarget(card_id)` for the
    /// first `targets == "one"` card missing a target, then `payment_plan` ->
    /// `CantAfford(card_id)` (DDv2 6.3: "rejects the lock ... naming the
    /// card"). On success each `ArmedCard` becomes a `Play` pushed onto
    /// `locked_plays` (§3.4.1 — NEVER `plays`), `armed` is cleared, `locked`
    /// is set, `seq` bumps. Pulls are not charged here; payment happens at
    /// reveal (DDv2 6.4). Locking zero cards is legal.
    pub fn lock_in(&mut self, player_id: i64) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Lock {
            return Err(LcError::WrongBeat);
        }
        if self.players[seat].locked {
            return Ok(());
        }
        let p = &self.players[seat];
        if let Some(a) = p
            .armed
            .iter()
            .find(|a| a.card.targets == "one" && a.target.is_none())
        {
            return Err(LcError::NeedsTarget(a.card.id.clone()));
        }
        let halved = matches!(
            self.event
                .as_deref()
                .and_then(crate::lc_events::event_def)
                .map(|e| e.hook),
            Some(crate::lc_events::EventHook::CostHalf)
        );
        payment_plan(p, halved)?;

        let plays: Vec<Play> = p
            .armed
            .iter()
            .map(|a| Play {
                card: a.card.clone(),
                source_seat: seat,
                target: match a.card.targets.as_str() {
                    "self" => Some(seat), // D2
                    "one" => a.target,    // validated Some
                    _ => None,
                },
                paid_from: a.card.deck,
                order_key: 0, // set at reveal (DDv2 §1), not here
            })
            .collect();

        let p = &mut self.players[seat];
        p.armed.clear();
        p.locked = true;
        self.locked_plays.extend(plays);
        self.seq += 1;
        Ok(())
    }

    /// The viewer's own staged plays, hidden everywhere else — feeds Plan
    /// E's private-fetch hand pane so a locked player's own `LOCKED {n}`
    /// header still shows their card minis after `lock_in` empties `armed`
    /// into `locked_plays`. Seat-scoped and pure: callers must gate this to
    /// the requesting player's own seat and never feed it to a broadcast
    /// path — the same secrecy boundary `public_view()` enforces for
    /// everyone else.
    pub fn staged_for(&self, seat: usize) -> Vec<&Card> {
        self.locked_plays
            .iter()
            .filter(|play| play.source_seat == seat)
            .map(|play| &play.card)
            .collect()
    }

    /// The seat's current partner, if any. Reads `pacts` — callable only
    /// from engine internals (`resolve()`, Task 2) and per-viewer code (the
    /// private section renderer, Task 3), never from a public renderer.
    pub fn pact_partner(&self, seat: usize) -> Option<usize> {
        self.pacts.iter().find_map(|p| {
            if p.a == seat {
                Some(p.b)
            } else if p.b == seat {
                Some(p.a)
            } else {
                None
            }
        })
    }

    /// DDv2 pacts (G-series). Guard order: `NotSeated` -> `NotAlive` ->
    /// `WrongBeat` (must be `Beat::Diplomacy`) -> `BadTarget` (target is
    /// self, out of range, or not `Alive`) -> `PactBlocked` (the *offeror*
    /// is pacted or barred, or fewer than `PACT_MIN_ALIVE` players are
    /// Alive — all three are facts the offeror already knows, so refusing
    /// leaks nothing; target-side unavailability deliberately does NOT
    /// refuse, per G11).
    ///
    /// Mutual offer (G8): if `pact_offers` already contains the reverse
    /// offer, the pact forms immediately — every remaining OUTGOING offer
    /// from either newly-pacted seat is removed (not just the pairwise
    /// offer between them: a seat that closes a pact must exit the market
    /// entirely, per "one pact per player", or a stale outgoing offer to a
    /// third party could mutual-close into a second pact for the same
    /// seat). Third-party offers still INCOMING to either seat stay pending
    /// (per G11 — they are the no-ops that expire at Diplomacy's end, not a
    /// pact detector). `seq` bumps once. An identical pending offer already
    /// present is an idempotent no-op (`Ok(())`, no bump — the `add_player`
    /// precedent). Otherwise any other outgoing offer from this seat is
    /// replaced (retarget, G8) and the new one recorded, `seq` bumps. Offers
    /// to a secretly-pacted or barred target are recorded too — they are
    /// no-ops that expire at Diplomacy's end (G11).
    pub fn offer_pact(&mut self, player_id: i64, target_seat: usize) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Diplomacy {
            return Err(LcError::WrongBeat);
        }
        if target_seat == seat {
            return Err(LcError::BadTarget);
        }
        match self.players.get(target_seat) {
            Some(tp) if tp.status == Status::Alive => {}
            _ => return Err(LcError::BadTarget),
        }
        let alive = self
            .players
            .iter()
            .filter(|p| p.status == Status::Alive)
            .count();
        if self.pact_partner(seat).is_some()
            || self.pact_barred.contains(&seat)
            || alive < PACT_MIN_ALIVE
        {
            return Err(LcError::PactBlocked);
        }

        if self
            .pact_offers
            .iter()
            .any(|o| o.from == target_seat && o.to == seat)
        {
            let (a, b) = if seat < target_seat {
                (seat, target_seat)
            } else {
                (target_seat, seat)
            };
            self.pacts.push(Pact {
                a,
                b,
                formed_round: self.round,
            });
            self.pact_offers
                .retain(|o| o.from != seat && o.from != target_seat);
            self.seq += 1;
            return Ok(());
        }

        if self
            .pact_offers
            .iter()
            .any(|o| o.from == seat && o.to == target_seat)
        {
            return Ok(());
        }

        self.pact_offers.retain(|o| o.from != seat);
        self.pact_offers.push(PactOffer {
            from: seat,
            to: target_seat,
        });
        self.seq += 1;
        Ok(())
    }

    /// Guard order: `NotSeated` -> `NotAlive` -> `WrongBeat` -> `PactBlocked`
    /// (the accepter is pacted or barred — such players are never *shown*
    /// offers, so this is route-level defence) -> `NoOffer` (no pending
    /// `PactOffer { from: from_seat, to: my_seat }`). Success: forms the
    /// pact, removes every remaining OUTGOING offer from either
    /// newly-pacted seat (the same "one pact per player" reasoning as
    /// `offer_pact`'s mutual-offer branch — see its doc comment), `seq`
    /// bumps.
    pub fn accept_pact(&mut self, player_id: i64, from_seat: usize) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Diplomacy {
            return Err(LcError::WrongBeat);
        }
        if self.pact_partner(seat).is_some() || self.pact_barred.contains(&seat) {
            return Err(LcError::PactBlocked);
        }
        if !self
            .pact_offers
            .iter()
            .any(|o| o.from == from_seat && o.to == seat)
        {
            return Err(LcError::NoOffer);
        }
        let (a, b) = if seat < from_seat {
            (seat, from_seat)
        } else {
            (from_seat, seat)
        };
        self.pacts.push(Pact {
            a,
            b,
            formed_round: self.round,
        });
        self.pact_offers
            .retain(|o| o.from != seat && o.from != from_seat);
        self.seq += 1;
        Ok(())
    }

    /// Guard order: same chain as `accept_pact` including `PactBlocked`,
    /// then `NoOffer`. Success: removes that one offer, `seq` bumps. (The
    /// offeror's WAITING line reverting to a propose button is the answer
    /// they are owed — a decline is a signal from someone who saw the
    /// offer, which only an available player can be.)
    pub fn decline_pact(&mut self, player_id: i64, from_seat: usize) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        if self.players[seat].status != Status::Alive {
            return Err(LcError::NotAlive);
        }
        if self.beat != Beat::Diplomacy {
            return Err(LcError::WrongBeat);
        }
        if self.pact_partner(seat).is_some() || self.pact_barred.contains(&seat) {
            return Err(LcError::PactBlocked);
        }
        let Some(idx) = self
            .pact_offers
            .iter()
            .position(|o| o.from == from_seat && o.to == seat)
        else {
            return Err(LcError::NoOffer);
        };
        self.pact_offers.remove(idx);
        self.seq += 1;
        Ok(())
    }

    /// One beat forward. Draw→Deal clears `drawing`; Lock→Reveal is the
    /// reveal: unlocked players' armed cards return to hand (DDv2 §12,
    /// disconnect at lock), locked plays are charged (6.4) and moved into
    /// `plays` with order_key computed (7.1/7.2). At Resolve returns
    /// Err(MustResolve) — resolve() owns the rollover (D5). Bumps seq on
    /// success.
    pub fn advance_beat(&mut self) -> Result<(), LcError> {
        if self.beat == Beat::Resolve {
            return Err(LcError::MustResolve);
        }
        let from = self.beat;
        self.beat = from.next();
        match from {
            Beat::Draw => {
                for p in &mut self.players {
                    p.drawing = false;
                }
                // DDv2 §5 beat 2: reveal this round's event, replacing the
                // last; §2.6: the first is dealt here at round 1, never at
                // setup. Deterministic from the stored seed (H1) — no RNG
                // enters the engine.
                let def = crate::lc_events::event_for_round(self.rng_seed, self.round);
                self.event = Some(def.id.to_string());
                if let crate::lc_events::EventHook::Toast { drain, heal } = def.hook {
                    for p in self
                        .players
                        .iter_mut()
                        .filter(|p| p.status == Status::Alive)
                    {
                        drain_pulls(p, drain); // fullest vessel, F4's helper
                        p.hp += heal; // no ceiling (TBD-3)
                    }
                }
            }
            Beat::Diplomacy => {
                // G8: offers are beat-scoped. Clearing here is the decline
                // nobody had to press — and it is why no offer can ever
                // dangle across an elimination (eliminations happen at
                // Resolve, offers never survive past Diplomacy).
                self.pact_offers.clear();
            }
            Beat::Lock => self.reveal(),
            // Deal→Diplomacy, Reveal→Resolve: events, tabs and the reaction
            // window are hollow systems (D14, D9).
            _ => {}
        }
        self.seq += 1;
        Ok(())
    }

    /// Lock→Reveal edge — the reveal. See `advance_beat`'s doc comment for
    /// the transition it's called from.
    fn reveal(&mut self) {
        // 1. Unlocked players play nothing (§12): their armed cards go
        // home, uncharged. A disconnect (or simply never locking) forfeits
        // the round's plays but not the cards.
        for p in &mut self.players {
            if p.status == Status::Alive && !p.locked {
                for a in p.armed.drain(..) {
                    p.hand.push(a.card);
                }
            }
        }

        // 2. Charge pulls (6.4). `lock_in` already ran `payment_plan` over
        // the same cards and accepted them; neither vessels nor handicap can
        // change between lock and reveal (vessels are Draw-gated, D15;
        // handicap is now Draw-gated too, D19 — closing the review's I1
        // finding), so re-deriving the plan here is *guaranteed*, not just
        // normally expected, to reproduce the exact numbers lock_in
        // validated. `expect` rather than propagating an error: a
        // `CantAfford` here would mean state diverged from what lock_in
        // accepted, which the D15/D19 gates make unreachable. The same
        // holds for `halved`: `event` only ever changes at the Draw→Deal
        // reveal and is cleared only by the rollover in `resolve()` (H2),
        // both of which sit outside the Lock→Reveal edge this function
        // runs on — so the event `lock_in` saw and the event charged here
        // are structurally the same one.
        let halved = matches!(
            self.event
                .as_deref()
                .and_then(crate::lc_events::event_def)
                .map(|e| e.hook),
            Some(crate::lc_events::EventHook::CostHalf)
        );
        let locked_seats: Vec<usize> = self
            .players
            .iter()
            .filter(|p| p.locked)
            .map(|p| p.seat)
            .collect();
        for seat in locked_seats {
            let armed: Vec<ArmedCard> = self
                .locked_plays
                .iter()
                .filter(|play| play.source_seat == seat)
                .map(|play| ArmedCard {
                    card: play.card.clone(),
                    target: play.target,
                })
                .collect();
            let mut trial = self.players[seat].clone();
            trial.armed = armed;
            let plan = payment_plan(&trial, halved).expect(
                "lock_in already validated affordability against these vessels \
                 and this handicap_pct, and neither can change before reveal \
                 (vessels are Draw-gated, D15; handicap is Draw-gated, D19) — \
                 a CantAfford here means state diverged from what lock_in accepted",
            );
            let p = &mut self.players[seat];
            for (vessel_idx, cost) in plan {
                p.vessels[vessel_idx].pulls_left -= cost;
            }
        }

        // 3. Order (7.1/7.2): bigger spender acts first; `first_seat`
        // breaks ties by table position; the stable sort preserves each
        // player's own arming order among their own plays for free (7.2's
        // within-player rule).
        let n = self.players.len();
        let first_seat = self.first_seat;
        let mut totals = vec![0u32; n];
        for play in &self.locked_plays {
            let handicap = self.players[play.source_seat].handicap_pct;
            totals[play.source_seat] += self.effective_pull_cost(play.card.cost, handicap) as u32;
        }
        self.locked_plays.sort_by_key(|play| {
            let priority = (play.source_seat + n - first_seat) % n;
            (std::cmp::Reverse(totals[play.source_seat]), priority)
        });
        for (i, play) in self.locked_plays.iter_mut().enumerate() {
            play.order_key = i as u32 + 1;
        }

        // 4. Flip everything at once: this is the single point where plays
        // become revealable — `beat` is already `Reveal` (set by
        // `advance_beat` before calling this) when `public_view()` next
        // runs.
        self.plays = std::mem::take(&mut self.locked_plays);
    }

    /// The beat-6 program (DDv2 §7-9) plus the round rollover (D5). Requires
    /// `beat == Beat::Resolve`. In order:
    ///
    /// 1. Resolve `plays` in `order_key` order: a play whose source is now
    ///    `Eliminated` (7.6) is skipped; a `targets == "one"` play whose
    ///    target is `Eliminated` fizzles (7.5) — either way the card still
    ///    ends in `discards` (8.4). Live plays look up their fx by card id in
    ///    the Plan F catalog (`lc_cards::card_fx`, never the card's own kind)
    ///    and apply it per subject: `Damage` (via `apply_damage`, elimination
    ///    checked immediately — D11), `Heal` with no ceiling (TBD-3),
    ///    `PullDrain` (`drain_pulls`, F4), `Shield` upserts into `effects`
    ///    immediately — replace-not-stack by (op, subject), D10 — so it can
    ///    absorb a later play in the same round's order (F8), and `Dot`
    ///    queues (appended in step 3, so it cannot tick this round). An id
    ///    the catalog doesn't recognize (a Reaction, or version skew, F1)
    ///    resolves inert.
    /// 2. Tick every existing `Dot` effect on an `Alive` subject, in
    ///    creation order, through the same `apply_damage` path.
    /// 3. Append the queued curse effects with the no-stack replace rule
    ///    (TBD-8/D10): a queued effect replaces any existing effect sharing
    ///    its `(op, subject)`.
    /// 4. Expire every effect with `expires_round <= self.round`.
    /// 5. Soft-cap (D12): any `Alive` player over `HAND_SOFT_CAP` discards
    ///    from the end of the hand (newest first) down to the cap.
    /// 6. Bump `seq`. If `outcome()` is now `Some`, stop here — `beat` stays
    ///    `Resolve`, the frozen final tableau (D16).
    /// 7. Otherwise roll over: `first_seat` rotates (D13), `round`
    ///    advances, `beat` resets to `Draw`, every player's `locked`/
    ///    `drawing`/`draws_this_round` resets, and any deck whose count sits
    ///    at 0 reclaims its cards from `discards` (8.4/§12) — the shoe is a
    ///    count, not a deck of identities (D6), so "reshuffle" here is a
    ///    fold: every discarded card of that deck moves back into the count
    ///    and out of `discards`, with no ordering to restore.
    pub fn resolve(&mut self) -> Result<(), LcError> {
        if self.beat != Beat::Resolve {
            return Err(LcError::WrongBeat);
        }

        // M3 hardening: no engine transition can produce an empty `players`
        // (a fresh room always seats via `add_player`/`new`), but a
        // hand-corrupted or pre-ceiling blob loaded via `from_json` could.
        // Without seats there is nothing to resolve, tick or roll over — and
        // `% self.players.len()` below would panic on zero — so stop here
        // instead of poisoning the room. `seq` does NOT bump: nothing is
        // mutated (no plays, no players, no rollover), so per D18/the
        // `add_player` precedent this is a no-op return, not a successful
        // mutating transition — bumping here would let a corrupt-blob room
        // manufacture phantom `seq` advances with no state behind them.
        if self.players.is_empty() {
            return Ok(());
        }

        // The active event's hook, resolved once (H3's fail-soft: an
        // unrecognised id resolves inert), plus a per-seat spend snapshot
        // for `TopSpenderHit` (and Task 3) — captured here, before Step 1
        // drains anything, since `charged_pulls` reads `self.plays`, which
        // Step 1 is about to `mem::take`.
        let hook = self
            .event
            .as_deref()
            .and_then(crate::lc_events::event_def)
            .map(|e| e.hook);
        let spent: Vec<u8> = (0..self.players.len())
            .map(|s| self.charged_pulls(s))
            .collect();

        // Step 1: resolve plays in order_key order. `plays` is drained up
        // front (§14: the queue empties every round) and iterated as owned
        // data so mutating `self.players`/`self.effects`/`self.discards`
        // per play needs no fighting the borrow checker.
        //
        // G5 erratum (Plan G whole-plan review, found by Task 3): a
        // `PactBreak` pushed below is stamped with `self.round` — the round
        // the knife was thrown IN, not the round anyone can first see it.
        // `lc_advance_chain` never persists between this call's Step 1 and
        // its Step 8 rollover, so for a non-terminal betrayal (the common
        // case) the very first frame any client fetches already has
        // `st.round` one past the stamp, and both round-scoped surfaces
        // (`pacts_section_html`'s betrayed notice, `lc_screen_panel`'s break
        // strip — both filter `round == st.round`/`round == view.round`)
        // are unreachable. Fix: track which breaks this call pushes
        // (`pact_breaks_start`) and, at the bottom of Step 7/top of Step 8,
        // re-stamp just those with the round players actually land on —
        // `self.round` unchanged (frozen, D16) if this break also ends the
        // game, `self.round` post-increment otherwise. A terminal betrayal's
        // stamp is untouched (still the round the game froze on); a
        // non-terminal one becomes visible for exactly the following round,
        // then ages out on the round after that — loud, not permanent (G5).
        let pact_breaks_start = self.pact_breaks.len();
        let plays = std::mem::take(&mut self.plays);
        // `NoPlayPenalty` (H4): which seats played at all this round, from
        // the round's plays as revealed — not who has a play left by the
        // time Step 1 finishes eliminating sources (M3: bounds-checked, a
        // corrupt blob's `source_seat` could name a seat past the ceiling).
        let mut played_seats = vec![false; self.players.len()];
        for play in &plays {
            if let Some(slot) = played_seats.get_mut(play.source_seat) {
                *slot = true;
            }
        }
        let mut queued_effects: Vec<Effect> = Vec::new();
        for play in plays {
            // M3: `source_seat` is validated at `lock_in` time, but a
            // corrupt/truncated blob (e.g. `from_json`'s MAX_SEATS cap
            // shrinking `players` under a play staged by a pre-ceiling
            // binary) could reference a seat that no longer exists. Treat
            // that the same as an eliminated source — no effect, the card
            // still leaves play — rather than panic on `self.players[..]`.
            let Some(source) = self.players.get(play.source_seat) else {
                self.discards.push(play.card);
                continue;
            };
            if source.status == Status::Eliminated {
                // 7.6: a source eliminated earlier this resolve plays
                // nothing, but the card still leaves play (8.4).
                self.discards.push(play.card);
                continue;
            }

            // D2 subject resolution. "table" has no card in the current
            // catalog and falls back to no subjects — a Reaction's id maps
            // to `fx: None` regardless (F5), so an empty subject list here
            // is otherwise moot until Plan F adds a "table" card. The "one"
            // arm bounds-checks the target the same way (M3) — an
            // out-of-range seat fizzles exactly like a dead one, instead of
            // panicking.
            let subjects: Vec<usize> = match play.card.targets.as_str() {
                "one" => match play.target.and_then(|t| self.players.get(t)) {
                    Some(p) if p.status == Status::Alive => {
                        let target_seat = p.seat;
                        // `double-vision`'s `HostileRedirect` (H4): only
                        // hostile ops redirect — heals/shields land where
                        // aimed. The subject becomes the next Alive seat
                        // clockwise from the target, `(target + k) % n` for
                        // the smallest `k >= 1`; `k` runs up to `n`
                        // inclusive, so the walk always terminates back on
                        // the target itself (still Alive, matched above) if
                        // no other seat is Alive.
                        let redirect = hook == Some(crate::lc_events::EventHook::HostileRedirect)
                            && crate::lc_cards::card_fx(&play.card.id).is_some_and(|f| {
                                matches!(
                                    f.op,
                                    EffectOp::Damage | EffectOp::Dot | EffectOp::PullDrain
                                )
                            });
                        if redirect {
                            let n = self.players.len();
                            let mut subject = target_seat;
                            for k in 1..=n {
                                let candidate = (target_seat + k) % n;
                                if self
                                    .players
                                    .get(candidate)
                                    .is_some_and(|q| q.status == Status::Alive)
                                {
                                    subject = candidate;
                                    break;
                                }
                            }
                            vec![subject]
                        } else {
                            vec![target_seat]
                        }
                    }
                    _ => {
                        // 7.5: fizzle — no effect, pulls stay spent, the
                        // card still occupied its slot.
                        self.discards.push(play.card);
                        continue;
                    }
                },
                "self" => vec![play.source_seat],
                "all" => self
                    .players
                    .iter()
                    .filter(|p| p.status == Status::Alive)
                    .map(|p| p.seat)
                    .collect(),
                _ => Vec::new(),
            };

            // G4/G5: a resolved single-target hostile play on your partner
            // is betrayal. After the fizzle gate on purpose — a play
            // fizzling on an already-dead partner breaks nothing; the
            // end-of-resolve sweep dissolves that pact silently instead
            // (G9). AOE plays (`targets != "one"`) never reach this branch
            // (G4) — the `subjects` match above already gives non-"one"
            // cards `play.target == None`, but the explicit
            // `card.targets == "one"` guard below keeps that invariant from
            // being load-bearing.
            if play.card.targets == "one" {
                if let (Some(target), Some(partner)) =
                    (play.target, self.pact_partner(play.source_seat))
                {
                    let hostile = crate::lc_cards::card_fx(&play.card.id).is_some_and(|f| {
                        matches!(f.op, EffectOp::Damage | EffectOp::Dot | EffectOp::PullDrain)
                    });
                    if target == partner && hostile {
                        // Pact invariant (Task 1): a < b, so this is a
                        // single sorted-pair comparison.
                        let (lo, hi) = (play.source_seat.min(target), play.source_seat.max(target));
                        self.pacts.retain(|p| !(p.a == lo && p.b == hi));
                        self.pact_breaks.push(PactBreak {
                            betrayer: play.source_seat,
                            betrayed: target,
                            round: self.round,
                        });
                        // M3-style defensive dedupe (Task 1's report,
                        // carried concern): a corrupt/replayed blob could in
                        // principle reach this arm twice for the same seat;
                        // under normal play it can't (the pact this reads is
                        // gone after the first hit), but the market ban is
                        // "for the rest of the game" either way (G5), so a
                        // repeat push must stay a no-op rather than pad the
                        // list.
                        if !self.pact_barred.contains(&play.source_seat) {
                            self.pact_barred.push(play.source_seat);
                        }
                    }
                }
            }

            // Plan F: effects come from the binary's catalog, keyed by card
            // id — never from the card's own (possibly blob-carried) kind.
            // A reaction's id maps to `fx: None` in the catalog (D9/F5); an
            // id the catalog no longer recognizes (version skew, F1) maps
            // to `None` the same way — deliberate fail-soft, not a panic.
            match crate::lc_cards::card_fx(&play.card.id) {
                None => {}
                Some(f) => {
                    for subject in subjects {
                        match f.op {
                            EffectOp::Damage => self.apply_damage(subject, f.magnitude),
                            EffectOp::Heal => self.players[subject].hp += f.magnitude, // TBD-3: no ceiling
                            EffectOp::PullDrain => {
                                drain_pulls(&mut self.players[subject], f.magnitude)
                            }
                            // F8: shields register NOW (not queued), so they
                            // absorb later plays in this round's order.
                            // Replace-not-stack by (op, subject) — D10.
                            EffectOp::Shield => {
                                self.effects.retain(|e| {
                                    !(e.op == EffectOp::Shield && e.subject == subject)
                                });
                                self.effects.push(Effect {
                                    source_play: play.order_key,
                                    subject,
                                    op: EffectOp::Shield,
                                    magnitude: f.magnitude,
                                    expires_round: self.round + f.rounds,
                                });
                            }
                            // Dots still queue (appended at step 3): never
                            // tick in their own creation round.
                            EffectOp::Dot => {
                                queued_effects.push(Effect {
                                    source_play: play.order_key,
                                    subject,
                                    op: EffectOp::Dot,
                                    magnitude: f.magnitude,
                                    expires_round: self.round + f.rounds,
                                });
                            }
                        }
                    }
                }
            }
            self.discards.push(play.card);
        }

        // Step 2: tick dots (10.4, creation order). Snapshotted before
        // applying: the no-stack rule (D10) guarantees at most one Dot
        // effect per subject, so a tick can eliminate its own subject
        // (removing that same effect, per D11) without disturbing any other
        // snapshotted entry. Bounds-checked (M3): a corrupt blob's effect
        // could name a subject seat that no longer exists — skip it rather
        // than panic.
        let dot_ticks: Vec<(usize, i32)> = self
            .effects
            .iter()
            .filter(|e| {
                e.op == EffectOp::Dot
                    && self
                        .players
                        .get(e.subject)
                        .is_some_and(|p| p.status == Status::Alive)
            })
            .map(|e| (e.subject, e.magnitude))
            .collect();
        // `house-pour`'s `DotBoost` (H4): every tick this round hits harder
        // by `mult`. Stored `effect.magnitude` and `expires_round` are
        // untouched — only the applied amount is scaled.
        let dot_mult = match hook {
            Some(crate::lc_events::EventHook::DotBoost { mult }) => mult,
            _ => 1,
        };
        for (subject, magnitude) in dot_ticks {
            self.apply_damage(subject, magnitude * dot_mult);
        }

        // Step 2.5: the end-of-round event program (H4) — after dots,
        // before queued effects are appended, so it never touches the
        // effects a play just queued this round.
        match hook {
            Some(crate::lc_events::EventHook::NoPlayPenalty { dmg }) => {
                // `last-orders`: every Alive player with no play this round
                // takes `dmg`. A seat that played is exempt even if its play
                // fizzled or its source was eliminated earlier in Step 1.
                let victims: Vec<usize> = (0..self.players.len())
                    .filter(|&s| self.players[s].status == Status::Alive && !played_seats[s])
                    .collect();
                for s in victims {
                    self.apply_damage(s, dmg);
                }
            }
            Some(crate::lc_events::EventHook::TopSpenderHit { dmg }) => {
                // `big-shot`: every Alive seat at the snapshot maximum (ties
                // share the tax, H4) — but never a seat that spent nothing.
                let max = spent.iter().copied().max().unwrap_or(0);
                if max > 0 {
                    let victims: Vec<usize> = (0..self.players.len())
                        .filter(|&s| self.players[s].status == Status::Alive && spent[s] == max)
                        .collect();
                    for s in victims {
                        self.apply_damage(s, dmg);
                    }
                }
            }
            Some(crate::lc_events::EventHook::TableHeal { heal }) => {
                // `on-the-house`: every Alive player heals, no ceiling
                // (TBD-3).
                for p in self
                    .players
                    .iter_mut()
                    .filter(|p| p.status == Status::Alive)
                {
                    p.hp += heal;
                }
            }
            _ => {}
        }

        // Step 3: append queued curse effects, replacing any existing
        // effect with the same (op, subject) — TBD-8/D10.
        for queued in queued_effects {
            self.effects
                .retain(|e| !(e.op == queued.op && e.subject == queued.subject));
            self.effects.push(queued);
        }

        // Step 4: expire.
        self.effects.retain(|e| e.expires_round > self.round);

        // Step 5: soft cap (8.2, D12) — discard from the end (newest) down
        // to the cap.
        for seat in 0..self.players.len() {
            let p = &mut self.players[seat];
            if p.status == Status::Alive && p.hand.len() > HAND_SOFT_CAP {
                let overflow = p.hand.split_off(HAND_SOFT_CAP);
                self.discards.extend(overflow);
            }
        }

        // Step 6 (G9): a pact whose partner is gone has no win to share.
        // Silent — no break record, nobody barred; the survivor may pact
        // again in a later Diplomacy. A sweep rather than a hook inside the
        // elimination helper, so it cannot depend on where in this
        // resolution order the death happened. Offers need no equivalent
        // sweep: they never survive past Diplomacy (see `advance_beat`'s
        // Diplomacy→Lock edge).
        //
        // M3: bounds-checked like every other seat lookup in this function
        // (the dead-source skip, the "one"-target fizzle, the dot-tick
        // filter) — `from_json` truncates `players` to MAX_SEATS but does
        // NOT sanitize `pacts`, so a pre-ceiling blob could still name a
        // seat past the new ceiling. `.get(..).is_some_and(..)` treats a
        // vanished seat the same as a dead one: the pact silently
        // dissolves, which is the correct G9 outcome either way, instead of
        // panicking on `self.players[p.a]`.
        self.pacts.retain(|p| {
            self.players
                .get(p.a)
                .is_some_and(|q| q.status == Status::Alive)
                && self
                    .players
                    .get(p.b)
                    .is_some_and(|q| q.status == Status::Alive)
        });

        // Step 7: bump seq; stop here if the game just ended (D16).
        self.seq += 1;
        if self.outcome().is_some() {
            return Ok(());
        }

        // Step 8: rollover (D5).
        self.first_seat = (self.first_seat + 1) % self.players.len(); // D13
        self.round += 1;
        // H2: the event lives Deal→Resolve of exactly its round — this is
        // the only place it's cleared. The Draw→Deal reveal (`advance_beat`)
        // sets the next one; "never two at once" holds because that reveal
        // cannot run again until this rollover has already run once.
        self.event = None;
        // G5 erratum: re-stamp this call's breaks (if any) with the round
        // that just became current — the round players actually see them
        // in — rather than the round they were thrown in. See the Step 1
        // comment above `pact_breaks_start`.
        for brk in &mut self.pact_breaks[pact_breaks_start..] {
            brk.round = self.round;
        }
        self.beat = Beat::Draw;
        for p in &mut self.players {
            p.locked = false;
            p.drawing = false;
            p.draws_this_round = 0;
        }
        // Reshuffle (8.4/§12): the shoe is a count, so a deck sitting at 0
        // reclaims every discarded card of that deck straight back into the
        // count, and those cards leave `discards`.
        let empty_decks: Vec<Deck> = self
            .deck_counts
            .iter()
            .filter(|&&(_, count)| count == 0)
            .map(|&(deck, _)| deck)
            .collect();
        for deck in empty_decks {
            let mut reclaimed: u16 = 0;
            self.discards.retain(|c| {
                if c.deck == deck {
                    reclaimed += 1;
                    false
                } else {
                    true
                }
            });
            if let Some(entry) = self.deck_counts.iter_mut().find(|(d, _)| *d == deck) {
                entry.1 = reclaimed;
            }
        }
        Ok(())
    }

    /// Shared by both damage call sites (a live Atk play and a ticking Dot).
    /// Shields on `subject` absorb first, in effect-creation order (`Vec`
    /// insertion order — nothing in this engine reorders `effects`):
    /// `magnitude` is consumed by the hit and the shield is dropped once it
    /// reaches 0. Any remainder comes off HP, clamped at 0 (D11). Hitting 0
    /// eliminates the subject immediately: `status` flips, `hand` and any
    /// stray `armed` cards move to `discards` (ghosts hold no cards, 9.2),
    /// and every effect whose `subject` is this seat is dropped (D10) —
    /// including, when this call is itself a Dot tick, the very effect
    /// being ticked.
    fn apply_damage(&mut self, subject: usize, amount: i32) {
        let mut remaining = amount;
        for effect in self.effects.iter_mut() {
            if remaining <= 0 {
                break;
            }
            if effect.op == EffectOp::Shield && effect.subject == subject {
                let consumed = remaining.min(effect.magnitude);
                effect.magnitude -= consumed;
                remaining -= consumed;
            }
        }
        self.effects
            .retain(|e| !(e.op == EffectOp::Shield && e.magnitude <= 0));

        let p = &mut self.players[subject];
        p.hp = (p.hp - remaining).max(0);
        if p.hp == 0 {
            p.status = Status::Eliminated;
            let mut discarded: Vec<Card> = std::mem::take(&mut p.hand);
            discarded.extend(std::mem::take(&mut p.armed).into_iter().map(|a| a.card));
            self.discards.extend(discarded);
            self.effects.retain(|e| e.subject != subject);
        }
    }
}

/// F4: `PullDrain`'s engine semantics. `n` times, pick the vessel with the
/// greatest `pulls_left` (a tie keeps the lowest index), decrement it by 1;
/// stop early once every vessel sits at 0 (or the player has none). Never
/// touches HP — drains only ever move pulls, D-invariant apply_damage owns
/// HP.
fn drain_pulls(player: &mut LcPlayer, n: i32) {
    for _ in 0..n.max(0) {
        let best = player
            .vessels
            .iter()
            .enumerate()
            .max_by_key(|(idx, v)| (v.pulls_left, std::cmp::Reverse(*idx)))
            .map(|(idx, v)| (idx, v.pulls_left));
        match best {
            Some((idx, pulls_left)) if pulls_left > 0 => player.vessels[idx].pulls_left -= 1,
            _ => break,
        }
    }
}

/// Private helper, shared with Task 4's reveal charge: the deterministic
/// greedy payment simulation (D3). Simulates the player's armed cards in
/// arming order against a local copy of each vessel's `pulls_left`: for each
/// card, picks the vessel of `card.deck` with the greatest remaining
/// simulated `pulls_left` (a tie keeps the lowest index — the first-seen
/// candidate is never displaced by an equal one), deducts
/// `pull_cost(card.cost, handicap_pct)`, halved (rounded up) when `halved`
/// is set — the event-aware charge (H12), computed once by the caller from
/// `LastCallState::event` since this free function has no `self` to read it
/// from. Returns, per armed card in order, the `(vessel index, pulls)` it
/// pays — or `CantAfford` naming the first card for which no vessel of its
/// deck can cover the cost.
fn payment_plan(player: &LcPlayer, halved: bool) -> Result<Vec<(usize, u8)>, LcError> {
    let mut sim: Vec<u8> = player.vessels.iter().map(|v| v.pulls_left).collect();
    let mut plan = Vec::with_capacity(player.armed.len());
    for a in &player.armed {
        let base = pull_cost(a.card.cost, player.handicap_pct);
        let cost = if halved { base.div_ceil(2) } else { base };
        let mut best: Option<usize> = None;
        for (i, v) in player.vessels.iter().enumerate() {
            if v.deck != a.card.deck {
                continue;
            }
            match best {
                Some(bi) if sim[i] <= sim[bi] => {}
                _ => best = Some(i),
            }
        }
        let Some(bi) = best else {
            return Err(LcError::CantAfford(a.card.id.clone()));
        };
        if sim[bi] < cost {
            return Err(LcError::CantAfford(a.card.id.clone()));
        }
        sim[bi] -= cost;
        plan.push((bi, cost));
    }
    Ok(plan)
}

/// Shared runtime fixture builder (spec §8) — NOT `#[cfg(test)]`. Task 3's
/// plaque tests and Plan A-vis's preview route render the same eight-seat
/// state, so a test failure and a visual regression cannot disagree about
/// what the fixture is. One builder, used by both, so a test failure and a
/// visual regression cannot disagree about what the fixture is.
pub fn preview_state() -> LastCallState {
    let mut st = LastCallState::new(
        vec![
            (1, "alice".into()),
            (2, "bob".into()),
            (3, "cara".into()),
            (4, "dev".into()),
            (5, "erin".into()),
            (6, "fin".into()),
            (7, "gus".into()),
            (8, "hal".into()),
        ],
        0xC0FFEE,
    );
    st.round = 6;
    st.set_vessel(1, Deck::Beer, "50cl can").unwrap();
    st.set_vessel(2, Deck::Cider, "50cl bottle").unwrap();
    st.set_vessel(3, Deck::Wine, "15cl glass").unwrap();
    st.set_vessel(4, Deck::Liquor, "4cl shot").unwrap();
    st.set_vessel(5, Deck::Soft, "any").unwrap();
    // two-deck player — README: normal, not an edge case
    st.set_vessel(6, Deck::Beer, "50cl can").unwrap();
    st.set_vessel(6, Deck::Liquor, "4cl shot").unwrap();
    st.players[2].locked = true; // cara: locked
    st.players[4].drawing = true; // erin: drawing
    st.players[6].status = Status::Eliminated; // gus: eliminated
    st.players[6].hp = 0;
    st.players[3].hp = 4; // dev: low HP

    // 12 cards: the first 4 Cider ids repeated three times. Deliberate — it
    // bypasses set_vessel's dedupe so the n > 8 hand-strip split has a hand
    // to split, and the strip only ever reads a COUNT. The wheel now indexes
    // by DOM position, not card id (decision 10) — but three visually
    // identical card triples would still make the preview's oversized wheel
    // look broken, so the second and third rep's ids are suffixed to stay
    // distinct.
    let opener_four = crate::lc_cards::deck_cards(Deck::Cider)
        .into_iter()
        .take(4)
        .collect::<Vec<_>>();
    st.players[1].hand = std::iter::repeat_n(opener_four, 3)
        .enumerate()
        .flat_map(|(rep, cards)| {
            cards.into_iter().map(move |mut c| {
                if rep > 0 {
                    c.id = format!("{}-r{rep}", c.id);
                }
                c
            })
        })
        .collect();
    st.players[0].draws_this_round = 3; // the plaque's draw badge
    st.set_vessel(8, Deck::Soft, "any").unwrap(); // 8th seat: MAX_SEATS ceiling
    st.beat = Beat::Lock;
    st.deck_counts = vec![
        (Deck::Beer, 21),
        (Deck::Cider, 17),
        (Deck::Wine, 4),
        (Deck::Liquor, 0),
        (Deck::Soft, 12),
    ];
    st.discards = crate::lc_cards::deck_cards(Deck::Beer)
        .into_iter()
        .take(4)
        .collect(); // discard count 4
    st.seq = 42;
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seated() -> LastCallState {
        LastCallState::new(
            vec![(1, "alice".into()), (2, "bob".into()), (3, "cara".into())],
            42,
        )
    }

    /// alice(1)/Beer, bob(2)/Cider, cara(3)/Soft, dave(4)/Liquor — seats 0-3.
    /// Vessels registered at Draw, then moved to Diplomacy.
    fn at_diplomacy() -> LastCallState {
        let mut st = LastCallState::new(
            vec![
                (1, "alice".into()),
                (2, "bob".into()),
                (3, "cara".into()),
                (4, "dave".into()),
            ],
            42,
        );
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Soft, "glass").unwrap();
        st.set_vessel(4, Deck::Liquor, "shot").unwrap();
        st.beat = Beat::Diplomacy;
        st
    }

    fn deck_count(st: &LastCallState, deck: Deck) -> u16 {
        st.deck_counts.iter().find(|(d, _)| *d == deck).unwrap().1
    }

    fn set_deck_count(st: &mut LastCallState, deck: Deck, count: u16) {
        st.deck_counts
            .iter_mut()
            .find(|(d, _)| *d == deck)
            .unwrap()
            .1 = count;
    }

    #[test]
    fn test_pull_table() {
        assert_eq!(Deck::Beer.pulls(), 8);
        assert_eq!(Deck::Cider.pulls(), 10);
        assert_eq!(Deck::Wine.pulls(), 6);
        assert_eq!(Deck::Liquor.pulls(), 4);
        assert_eq!(Deck::Soft.pulls(), 6);

        let mut st = seated();
        st.set_vessel(1, Deck::Liquor, "4cl shot glass").unwrap();
        let seat0 = &st.players[0];
        assert_eq!(
            seat0.vessels[0],
            Vessel {
                deck: Deck::Liquor,
                pulls_max: 4,
                pulls_left: 4,
                container: "4cl shot glass".to_string(),
            }
        );
    }

    #[test]
    fn test_pull_cost_rounds_up() {
        let cases: [(u8, u16, u8); 9] = [
            (2, 100, 2),
            (3, 100, 3),
            (1, 150, 2),
            (2, 150, 3),
            (3, 150, 5),
            (3, 50, 2),
            (2, 75, 2),
            (1, 25, 1),
            (4, 300, 12),
        ];
        for (cost, pct, expected) in cases {
            assert_eq!(pull_cost(cost, pct), expected, "cost={cost} pct={pct}");
        }
    }

    #[test]
    fn test_set_handicap_range() {
        let mut st = seated();
        assert_eq!(st.set_handicap(2, 150), Ok(()));
        assert_eq!(st.players[1].handicap_pct, 150);

        assert_eq!(st.set_handicap(2, 24), Err(LcError::BadHandicap));
        assert_eq!(st.players[1].handicap_pct, 150);

        assert_eq!(st.set_handicap(2, 301), Err(LcError::BadHandicap));
        assert_eq!(st.players[1].handicap_pct, 150);

        assert_eq!(st.set_handicap(999, 150), Err(LcError::NotSeated));

        st.beat = Beat::Lock; // D19: handicap is Draw-gated, like set_vessel
        assert_eq!(st.set_handicap(2, 200), Err(LcError::WrongBeat));
        assert_eq!(st.players[1].handicap_pct, 150); // unchanged
    }

    /// finding 9: a missing top-level field (the shape of a slice-3 addition
    /// read against an older blob) backfills from `Default` instead of
    /// panicking.
    #[test]
    fn test_from_json_backfills_missing_top_level_fields() {
        assert_eq!(LastCallState::from_json("{}"), LastCallState::default());
    }

    #[test]
    fn test_serde_round_trip() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Wine, "glass").unwrap();
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("beer-01").unwrap(),
            source_seat: 0,
            target: Some(1),
            paid_from: Deck::Beer,
            order_key: 1,
        });
        st.effects.push(Effect {
            source_play: 1,
            subject: 1,
            op: EffectOp::Dot,
            magnitude: -2,
            expires_round: 3,
        });
        st.discards
            .push(crate::lc_cards::card_by_id("cider-01").unwrap());
        st.beat = Beat::Lock;
        st.seq = 7;
        st.pacts.push(Pact {
            a: 0,
            b: 1,
            formed_round: 1,
        });
        st.pact_offers.push(PactOffer { from: 1, to: 2 });
        st.pact_barred.push(2);
        st.pact_breaks.push(PactBreak {
            betrayer: 0,
            betrayed: 1,
            round: 1,
        });

        assert_eq!(LastCallState::from_json(&st.to_json()), st);
    }

    #[test]
    fn test_offer_and_accept_form_a_pact() {
        let mut st = at_diplomacy();
        let before = st.seq;
        st.offer_pact(1, 1).unwrap(); // alice (seat 0) -> bob (seat 1)
        assert_eq!(st.pact_offers, vec![PactOffer { from: 0, to: 1 }]);
        assert_eq!(st.seq, before + 1);
        st.accept_pact(2, 0).unwrap(); // bob accepts alice's offer
        assert_eq!(
            st.pacts,
            vec![Pact {
                a: 0,
                b: 1,
                formed_round: 1
            }]
        );
        assert!(st.pact_offers.is_empty());
        assert_eq!(st.pact_partner(0), Some(1));
        assert_eq!(st.pact_partner(1), Some(0));
        assert_eq!(st.pact_partner(2), None);
        assert_eq!(st.seq, before + 2);
    }

    /// "One pact per player" (the plan's global constraint) must survive a
    /// mutual close even when one of the closing seats had an unrelated
    /// outgoing offer in flight: if formation only removed the offer
    /// *between* the two seats, alice's stale offer to cara would outlive
    /// her new pact with bob, and cara accepting it would land alice in two
    /// pacts at once. Formation must strip every outgoing offer from both
    /// newly-pacted seats, not just the pairwise one.
    #[test]
    fn test_forming_a_pact_clears_the_new_partners_other_outgoing_offers() {
        let mut st = at_diplomacy();
        st.offer_pact(1, 2).unwrap(); // alice(0) -> cara(2)
        st.offer_pact(2, 0).unwrap(); // bob(1)   -> alice(0)
        st.offer_pact(1, 1).unwrap(); // mutual: pact(0,1) forms
        assert!(st.pact_offers.is_empty()); // alice's stale offer to cara is gone too
        assert_eq!(st.accept_pact(3, 0), Err(LcError::NoOffer)); // nothing left to accept
        assert_eq!(st.pacts.len(), 1);
        assert_eq!(st.pact_partner(0), Some(1));

        // Same guarantee via the accept_pact formation path.
        let mut st2 = at_diplomacy();
        st2.offer_pact(1, 2).unwrap(); // alice(0) -> cara(2)
        st2.offer_pact(2, 0).unwrap(); // bob(1)   -> alice(0)
        st2.accept_pact(1, 1).unwrap(); // alice accepts bob's offer directly
        assert!(st2.pact_offers.is_empty());
        assert_eq!(st2.accept_pact(3, 0), Err(LcError::NoOffer));
        assert_eq!(st2.pacts.len(), 1);
    }

    #[test]
    fn test_offer_guard_order() {
        let mut st = at_diplomacy();
        assert_eq!(st.offer_pact(999, 1), Err(LcError::NotSeated));
        assert_eq!(st.offer_pact(1, 0), Err(LcError::BadTarget)); // self
        assert_eq!(st.offer_pact(1, 9), Err(LcError::BadTarget)); // no such seat
        st.players[2].status = Status::Eliminated;
        assert_eq!(st.offer_pact(1, 2), Err(LcError::BadTarget)); // dead target

        // 3 alive < PACT_MIN_ALIVE — even a valid target is refused (G7):
        assert_eq!(st.offer_pact(1, 1), Err(LcError::PactBlocked));
        st.players[2].status = Status::Alive;
        st.players[0].status = Status::Eliminated;
        assert_eq!(st.offer_pact(1, 1), Err(LcError::NotAlive));
        st.players[0].status = Status::Alive;
        st.beat = Beat::Lock;
        assert_eq!(st.offer_pact(1, 1), Err(LcError::WrongBeat));
    }

    #[test]
    fn test_one_outgoing_offer_retargets_and_repeats_are_free() {
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        let seq = st.seq;
        st.offer_pact(1, 1).unwrap(); // identical repeat: Ok, no bump (G8)
        assert_eq!((st.seq, st.pact_offers.len()), (seq, 1));
        st.offer_pact(1, 2).unwrap(); // retarget replaces (G8)
        assert_eq!(st.pact_offers, vec![PactOffer { from: 0, to: 2 }]);
        assert_eq!(st.seq, seq + 1);
    }

    #[test]
    fn test_mutual_offers_form_the_pact_directly() {
        // G8
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.offer_pact(2, 0).unwrap(); // bob offers alice back
        assert_eq!(
            st.pacts,
            vec![Pact {
                a: 0,
                b: 1,
                formed_round: 1
            }]
        );
        assert!(st.pact_offers.is_empty());
    }

    #[test]
    fn test_offers_to_the_unavailable_go_quietly_nowhere() {
        // G11
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob pacted
                                       // cara offers pacted alice: recorded, not refused — no pact detector.
        st.offer_pact(3, 0).unwrap();
        assert_eq!(st.pact_offers, vec![PactOffer { from: 2, to: 0 }]);
        // alice (pacted) cannot accept it:
        assert_eq!(st.accept_pact(1, 2), Err(LcError::PactBlocked));
        // pacted alice cannot offer either:
        assert_eq!(st.offer_pact(1, 3), Err(LcError::PactBlocked));
        // barred players are out of the market on the offering side too:
        st.pact_barred.push(3); // dave
        assert_eq!(st.offer_pact(4, 2), Err(LcError::PactBlocked));
        // ...but can still be offered to (their bar is public; the offer no-ops):
        st.offer_pact(3, 3).unwrap(); // cara retargets dave
        assert_eq!(st.accept_pact(4, 2), Err(LcError::PactBlocked));
    }

    #[test]
    fn test_accept_and_decline_need_a_real_offer() {
        let mut st = at_diplomacy();
        assert_eq!(st.accept_pact(2, 0), Err(LcError::NoOffer));
        st.offer_pact(1, 1).unwrap();
        let seq = st.seq;
        st.decline_pact(2, 0).unwrap();
        assert!(st.pact_offers.is_empty());
        assert_eq!(st.seq, seq + 1);
        assert_eq!(st.decline_pact(2, 0), Err(LcError::NoOffer)); // already gone
        assert!(st.pacts.is_empty());
    }

    #[test]
    fn test_offers_expire_when_diplomacy_ends() {
        // G8
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.offer_pact(3, 3).unwrap();
        st.advance_beat().unwrap(); // Diplomacy -> Lock
        assert_eq!(st.beat, Beat::Lock);
        assert!(st.pact_offers.is_empty());
    }

    /// The §3.4.1 pattern applied to pacts (Global Constraints): nothing
    /// pact-shaped reaches the projection. Task 2 adds the deliberate
    /// exception (pact_breaks) and NARROWS this assertion to the named
    /// private keys — planned there, not discovered.
    #[test]
    fn test_pacts_and_offers_never_reach_the_public_view() {
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // a formed pact
        st.offer_pact(3, 3).unwrap(); // and a pending offer
        st.pact_barred.push(3);
        for beat in [
            Beat::Draw,
            Beat::Deal,
            Beat::Diplomacy,
            Beat::Lock,
            Beat::Reveal,
            Beat::Resolve,
        ] {
            st.beat = beat;
            let view = st.public_view();
            let json = serde_json::to_string(&view).unwrap();
            // Task 2 narrows this: `PublicView` now HAS a pact field
            // (`pact_breaks`, G5), so a blanket `!json.contains("pact")`
            // would trip on its own presence. Assert every SECRET pact
            // field by name instead.
            assert!(!json.contains("\"pacts\""), "beat={beat:?}");
            assert!(!json.contains("pact_offers"), "beat={beat:?}");
            assert!(!json.contains("pact_barred"), "beat={beat:?}");
            assert!(!json.contains("formed_round"), "beat={beat:?}");
            // The one public pact field stays empty while every pact is
            // intact (G10):
            assert!(view.pact_breaks.is_empty(), "beat={beat:?}");
        }
    }

    #[test]
    fn test_betrayal_breaks_the_pact_publicly_and_bars_the_betrayer() {
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.beat = Beat::Lock;
        st.arm(1, "beer-01").unwrap(); // Damage 2, targets "one"
        st.set_target(1, "beer-01", Some(1)).unwrap(); // ...at her partner
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap(); // Reveal
        st.advance_beat().unwrap(); // Resolve
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 13);
        assert!(st.pacts.is_empty());
        // G5 erratum: the break is not terminal (nobody died), so it rolled
        // over with everything else — the stamp is the round players
        // actually land on (2), not the round the knife was thrown in (1).
        assert_eq!(st.round, 2);
        assert_eq!(
            st.pact_breaks,
            vec![PactBreak {
                betrayer: 0,
                betrayed: 1,
                round: 2
            }]
        );
        assert_eq!(st.pact_barred, vec![0]);
        // The break is the one public trace (G5):
        assert_eq!(st.public_view().pact_breaks, st.pact_breaks);
        // Secrecy holds even now that pact_barred/pact_breaks are both
        // populated (not just in the all-intact case
        // `test_pacts_and_offers_never_reach_the_public_view` covers):
        // pact_breaks is the ONE field allowed to leak.
        let json = serde_json::to_string(&st.public_view()).unwrap();
        assert!(!json.contains("\"pacts\""));
        assert!(!json.contains("pact_offers"));
        assert!(!json.contains("pact_barred"));
        assert!(!json.contains("formed_round"));
        assert!(json.contains("\"pact_breaks\""));
        // Next Diplomacy: the betrayer is out of the market for good.
        st.beat = Beat::Diplomacy;
        assert_eq!(st.offer_pact(1, 2), Err(LcError::PactBlocked));
        assert_eq!(st.accept_pact(1, 2), Err(LcError::PactBlocked));
    }

    #[test]
    fn test_aoe_splash_and_kindness_are_not_betrayal() {
        // G4
        // Splash: alice-bob pacted, alice plays beer-05 (aoe, hits bob too).
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap();
        st.beat = Beat::Lock;
        // beer-05 is not in Beer's opener (F6) — deal it into alice's hand.
        st.players[0]
            .hand
            .push(crate::lc_cards::card_by_id("beer-05").unwrap());
        st.arm(1, "beer-05").unwrap(); // Damage 1 to all, incl. bob and alice
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.hp == 14));
        assert_eq!(st.pacts.len(), 1); // intact
        assert!(st.pact_breaks.is_empty() && st.pact_barred.is_empty());

        // Kindness: cara-bob pacted, cara heals bob ("one"-target, friendly op).
        let mut st = at_diplomacy();
        st.offer_pact(3, 1).unwrap();
        st.accept_pact(2, 2).unwrap();
        st.beat = Beat::Lock;
        st.arm(3, "soft-01").unwrap(); // Heal 2, targets "one"
        st.set_target(3, "soft-01", Some(1)).unwrap();
        st.lock_in(3).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 17);
        assert_eq!(st.pacts.len(), 1);
        assert!(st.pact_breaks.is_empty());
    }

    #[test]
    fn test_elimination_dissolves_the_pact_silently() {
        // G9
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.players[1].hp = 2;
        st.beat = Beat::Lock;
        st.arm(3, "soft-06").unwrap(); // cara (no pact): Damage 2, "one"
        st.set_target(3, "soft-06", Some(1)).unwrap();
        st.lock_in(3).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].status, Status::Eliminated);
        assert!(st.pacts.is_empty()); // dissolved
        assert!(st.pact_breaks.is_empty()); // silently — no break, nobody barred
        assert!(st.pact_barred.is_empty());
    }

    #[test]
    fn test_the_last_two_standing_share_the_win() {
        // G2
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.players[2].hp = 1;
        st.players[3].hp = 1;
        st.beat = Beat::Lock;
        // beer-05 is not in Beer's opener (F6) — deal it into alice's hand.
        st.players[0]
            .hand
            .push(crate::lc_cards::card_by_id("beer-05").unwrap());
        st.arm(1, "beer-05").unwrap(); // aoe 1: kills cara and dave, splashes bob
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        // The splash on bob did not betray (G4), the two deaths dissolved no
        // pact of theirs, and the pair stands:
        assert_eq!(st.outcome(), Some(LcOutcome::Pact(0, 1)));
        assert_eq!(st.public_view().outcome, Some(LcOutcome::Pact(0, 1)));
        assert_eq!(st.beat, Beat::Resolve); // frozen final tableau (D16)
                                            // Without a pact the same tableau plays on:
        let mut st2 = at_diplomacy();
        st2.players[2].status = Status::Eliminated;
        st2.players[3].status = Status::Eliminated;
        assert_eq!(st2.outcome(), None);
        // And the serde name is pinned:
        assert_eq!(
            serde_json::to_string(&LcOutcome::Pact(0, 1)).unwrap(),
            r#"{"pact":[0,1]}"#
        );
    }

    #[test]
    fn test_a_hostile_card_played_on_yourself_is_not_betrayal() {
        // G4: the operative condition is `target == partner`, not merely "a
        // pacted player played a hostile card" — self-targeting must not
        // trip it, even though set_target explicitly allows a "one" card
        // to name its own caster.
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.beat = Beat::Lock;
        st.arm(1, "beer-01").unwrap(); // Damage 2, targets "one"
        st.set_target(1, "beer-01", Some(0)).unwrap(); // ...at herself
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, 13);
        assert_eq!(st.pacts.len(), 1);
        assert!(st.pact_breaks.is_empty() && st.pact_barred.is_empty());
    }

    #[test]
    fn test_the_betrayers_knife_fizzles_on_an_already_dead_partner_same_resolve() {
        // G4/G9: if the partner died earlier in the SAME resolve (here, to
        // a third party), the fizzle gate in the subject-resolution match
        // already `continue`s before the betrayal check is ever reached —
        // per the code comment, "a play fizzling on an already-dead partner
        // breaks nothing; the end-of-resolve sweep dissolves that pact
        // silently instead." This pins that this is NOT indistinguishable
        // from `test_elimination_dissolves_the_pact_silently`: here the
        // knife really was in the surviving partner's hand, and it still
        // produces no PactBreak and bars nobody.
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.players[1].hp = 2;
        st.beat = Beat::Lock;
        // cara out-spends alice (2 pulls vs 1), so her plays resolve first
        // regardless of the table-position tie-break (7.1/7.2) — bob is
        // dead before alice's play is ever reached.
        st.arm(3, "soft-06").unwrap(); // Damage 2 -> bob, kills him outright
        st.set_target(3, "soft-06", Some(1)).unwrap();
        st.arm(3, "soft-01").unwrap(); // Heal 2 -> herself, pure padding spend
        st.set_target(3, "soft-01", Some(2)).unwrap();
        st.lock_in(3).unwrap();
        st.arm(1, "beer-01").unwrap(); // alice's knife: Damage 2 -> her partner
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].status, Status::Eliminated);
        assert!(st.pacts.is_empty()); // dissolved by the Step 6 sweep
        assert!(st.pact_breaks.is_empty()); // NOT a public break
        assert!(st.pact_barred.is_empty()); // alice is not barred
    }

    #[test]
    fn test_a_same_resolve_betrayal_pre_empts_the_pact_win() {
        // G2 vs G4/G5 ordering: betrayal (Step 1) always runs before
        // outcome() (Step 7) within one resolve() call. If the same round
        // that would otherwise leave a pacted pair the last two standing
        // ALSO contains one of them knifing the other, the pact is already
        // gone by the time outcome() looks — no shared win, and the game
        // does not end even though only two players remain.
        let mut st = at_diplomacy();
        st.offer_pact(1, 1).unwrap();
        st.accept_pact(2, 0).unwrap(); // alice-bob
        st.players[2].hp = 1;
        st.players[3].hp = 1;
        st.beat = Beat::Lock;
        // beer-05 is not in Beer's opener (F6) — deal it into alice's hand.
        st.players[0]
            .hand
            .push(crate::lc_cards::card_by_id("beer-05").unwrap());
        st.arm(1, "beer-05").unwrap(); // aoe 1: kills cara and dave, splashes bob
        st.arm(1, "beer-01").unwrap(); // ...and alice also knifes her own partner
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[2].status, Status::Eliminated);
        assert_eq!(st.players[3].status, Status::Eliminated);
        assert_eq!(st.players[1].status, Status::Alive);
        assert_eq!(st.pact_breaks.len(), 1);
        assert!(st.pacts.is_empty());
        assert_eq!(st.outcome(), None); // NOT Pact(0, 1) — betrayal wins the race
    }

    #[test]
    fn test_public_view_drops_unrevealed_identity() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Wine, "glass").unwrap();
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("wine-04").unwrap(), // "Corked"
            source_seat: 2,
            target: Some(0),
            paid_from: Deck::Wine,
            order_key: 1,
        });
        st.beat = Beat::Lock;

        let view = st.public_view();
        assert_eq!(view.seats.len(), 3);
        assert_eq!(view.seats[0].hand_len, 5); // F6 opener, not the old 4-card deal
        assert!(view.revealed.is_empty());
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("Corked"));
        assert!(!json.contains("beer-01"));
        assert!(!json.contains("cider-01"));
        assert!(!json.contains("wine-01"));

        st.beat = Beat::Reveal;
        let view = st.public_view();
        assert_eq!(view.revealed.len(), 1);
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("Corked"));
        assert!(!json.contains("beer-01"));
        assert!(!json.contains("cider-01"));
        assert!(!json.contains("wine-01"));
    }

    /// finding 3: pins the projection side of the `plays`/`revealed`
    /// invariant across every beat, so a future change to the beat gate (or
    /// to what feeds `plays`) trips this rather than a party. Does not test
    /// the invariant itself — "nothing enters `plays` before it is
    /// revealable" is a contract on callers of `plays.push`, documented on
    /// `arm()` and `public_view()`, not something this projection can
    /// enforce from the outside.
    #[test]
    fn test_public_view_never_reveals_before_the_reveal_beat() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("beer-01").unwrap(),
            source_seat: 0,
            target: None,
            paid_from: Deck::Beer,
            order_key: 1,
        });
        for beat in Beat::ORDER {
            st.beat = beat;
            let view = st.public_view();
            match beat {
                Beat::Reveal | Beat::Resolve => {
                    assert_eq!(view.revealed.len(), 1, "beat={beat:?}");
                }
                _ => {
                    assert!(view.revealed.is_empty(), "beat={beat:?}");
                }
            }
        }
    }

    #[test]
    fn test_public_view_multi_deck_vessels() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(1, Deck::Wine, "glass").unwrap();

        let view = st.public_view();
        let seat0 = &view.seats[0];
        assert_eq!(seat0.vessels.len(), 2);
        assert_eq!(
            seat0
                .vessels
                .iter()
                .map(|v| (v.deck, v.pulls_max, v.pulls_left))
                .collect::<Vec<_>>(),
            vec![(Deck::Beer, 8, 8), (Deck::Wine, 6, 6)]
        );
        assert_eq!(seat0.decks(), vec![Deck::Beer, Deck::Wine]);
        assert_eq!(seat0.hand_len, 10); // two 5-card F6 openers, no id overlap
    }

    #[test]
    fn test_public_view_carries_plaque_state() {
        let mut st = seated();
        st.players[1].locked = true;
        st.players[2].drawing = true;

        let view = st.public_view();
        assert!(view.seats[1].locked);
        assert!(view.seats[2].drawing);
    }

    #[test]
    fn test_add_player_is_idempotent() {
        let mut st = seated();
        // bob is already seated at index 1 — re-adding is a no-op that
        // still reports his existing seat, not a fresh one.
        assert_eq!(st.add_player(2, "bob"), Some(1));
        assert_eq!(st.players.len(), 3);

        assert_eq!(st.add_player(9, "dan"), Some(3));
        assert_eq!(st.players.len(), 4);
        assert_eq!(st.players[3].seat, 3);
    }

    #[test]
    fn test_add_player_stops_at_max_seats() {
        // The ninth visitor joins the room and is not seated. The D.2 ring
        // has nowhere to put a seat 8, and a table that silently drops a
        // plaque is worse than one that never seats them.
        let mut st = seated();
        for i in 4..=MAX_SEATS as i64 {
            assert!(st.add_player(i, "filler").is_some());
        }
        assert_eq!(st.players.len(), MAX_SEATS);

        assert_eq!(st.add_player(999, "ninth"), None);
        assert_eq!(st.players.len(), MAX_SEATS);
        assert!(st.seat_of(999).is_none());
    }

    /// M4 (review finding): once the table has frozen at the D16 game-over
    /// tableau, a new join must not un-freeze it. Before this fix, a third
    /// player joining after a 1v1 ended would make `outcome()` re-evaluate
    /// to `None` (two alive again), retroactively blanking
    /// `public_view().outcome` while `beat` stayed stuck at `Resolve`.
    #[test]
    fn test_add_player_refuses_after_the_game_is_over() {
        let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 42);
        st.players[1].status = Status::Eliminated;
        assert_eq!(st.outcome(), Some(LcOutcome::Winner(0)));
        let before = st.seq;

        assert_eq!(st.add_player(3, "cara"), None);
        assert_eq!(st.players.len(), 2); // not seated
        assert_eq!(st.seq, before); // no phantom bump either

        // An already-seated member's idempotent replay is unaffected —
        // that check runs before the outcome refusal.
        assert_eq!(st.add_player(2, "bob"), Some(1));
        assert_eq!(st.seq, before);
    }

    #[test]
    fn test_starting_with_more_than_max_seats_members_seats_only_max() {
        // The path add_player never sees: nine members in the room when
        // somebody presses START. LastCallState::new seats the first
        // MAX_SEATS and leaves the rest unseated.
        let members: Vec<(i64, String)> = (1..=9).map(|i| (i, format!("p{i}"))).collect();
        let st = LastCallState::new(members, 42);
        assert_eq!(st.players.len(), MAX_SEATS);
        assert!(st.seat_of(9).is_none());
    }

    #[test]
    fn test_from_json_caps_players_at_max_seats() {
        let mut st = LastCallState::new((1..=8).map(|i| (i, format!("p{i}"))).collect(), 42);
        // A ninth player, hand-built — the shape only a pre-ceiling blob has.
        let mut ninth = st.players[0].clone();
        ninth.seat = 8;
        ninth.player_id = 9;
        st.players.push(ninth);
        let loaded = LastCallState::from_json(&st.to_json());
        assert_eq!(loaded.players.len(), MAX_SEATS);
        assert!(loaded.seat_of(9).is_none());
    }

    #[test]
    fn test_beat_durations() {
        assert_eq!(
            Beat::ORDER.map(|b| b.duration_secs()),
            [Some(30), None, Some(60), Some(45), Some(20), None]
        );
    }

    #[test]
    fn test_effect_op_serde_names() {
        assert_eq!(serde_json::to_string(&EffectOp::Dot).unwrap(), "\"dot\"");
        assert_eq!(
            serde_json::to_string(&EffectOp::Damage).unwrap(),
            "\"damage\""
        );
        assert_eq!(
            serde_json::to_string(&EffectOp::PullDrain).unwrap(),
            "\"pull_drain\""
        );
    }

    #[test]
    fn test_outcome_detection() {
        let mut st = seated(); // 3 players
        assert_eq!(st.outcome(), None);
        st.players[1].status = Status::Eliminated;
        assert_eq!(st.outcome(), None); // two still alive
        st.players[2].status = Status::Eliminated;
        assert_eq!(st.outcome(), Some(LcOutcome::Winner(0)));
        assert_eq!(st.public_view().outcome, Some(LcOutcome::Winner(0)));
        st.players[0].status = Status::Eliminated;
        assert_eq!(st.outcome(), Some(LcOutcome::Draw));

        let solo = LastCallState::new(vec![(1, "alice".into())], 42);
        assert_eq!(solo.outcome(), None); // no game to win (D16)
    }

    #[test]
    fn test_seating_a_player_bumps_seq() {
        // Two distinct states must never share a seq: the client's
        // equal-seq allowance exists so a duplicate repaint is harmless,
        // and it would otherwise admit a stale one.
        let mut st = seated();
        let before = st.seq;
        assert_eq!(st.add_player(9, "dan"), Some(3));
        assert_eq!(st.seq, before + 1);
    }

    #[test]
    fn test_add_player_does_not_bump_seq_when_not_newly_seated() {
        // The idempotent replay and the full-table rejection both mutate
        // nothing about who's seated, so neither may raise seq — a phantom
        // advance is exactly what the equal-seq allowance in
        // test_seating_a_player_bumps_seq's doc comment can't defend
        // against.
        let mut st = seated();
        let before = st.seq;
        assert_eq!(st.add_player(2, "bob"), Some(1)); // already seated
        assert_eq!(st.seq, before);

        for i in 4..=MAX_SEATS as i64 {
            st.add_player(i, "filler");
        }
        let before_full = st.seq;
        assert_eq!(st.add_player(999, "ninth"), None); // table full
        assert_eq!(st.seq, before_full);
    }

    #[test]
    fn test_beat_order_hues_and_slugs() {
        assert_eq!(
            Beat::ORDER,
            [
                Beat::Draw,
                Beat::Deal,
                Beat::Diplomacy,
                Beat::Lock,
                Beat::Reveal,
                Beat::Resolve
            ]
        );
        assert_eq!(Beat::Draw.index(), 1);
        assert_eq!(Beat::Resolve.index(), 6);
        assert_eq!(Beat::Resolve.next(), Beat::Draw);
        assert_eq!(
            Beat::ORDER.map(|b| b.hue()),
            ["amber", "amber", "mint", "violet", "azure", "rose"]
        );
        assert_eq!(
            Beat::ORDER.map(|b| b.slug()),
            ["draw", "deal", "diplomacy", "lock", "reveal", "resolve"]
        );
    }

    /// alice(1)/Beer, bob(2)/Cider, cara(3)/Soft — vessels registered at Draw,
    /// then moved to the Lock beat.
    fn at_lock() -> LastCallState {
        at_lock_with(|_| {})
    }

    /// Like `at_lock`, but runs `setup` while still at `Beat::Draw` — needed
    /// by tests that call `set_handicap`, which is Draw-gated (D19).
    fn at_lock_with(setup: impl FnOnce(&mut LastCallState)) -> LastCallState {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Soft, "glass").unwrap();
        setup(&mut st);
        st.beat = Beat::Lock;
        st
    }

    #[test]
    fn test_arm_moves_hand_to_armed() {
        let mut st = at_lock();
        let before = st.seq;
        st.arm(1, "beer-01").unwrap();
        assert_eq!(st.players[0].hand.len(), 4);
        assert_eq!(st.players[0].armed.len(), 1);
        assert_eq!(st.players[0].armed[0].card.id, "beer-01");
        assert_eq!(st.players[0].armed[0].target, None);
        assert_eq!(st.seq, before + 1);
    }

    #[test]
    fn test_arm_guard_order() {
        let mut st = at_lock();
        assert_eq!(st.arm(999, "beer-01"), Err(LcError::NotSeated));
        assert_eq!(st.arm(1, "nope"), Err(LcError::UnknownCard));
        // Soft's F6 opener excludes its reaction (soft-04), so it must be
        // added to cara's hand by hand to reach the NotPlayable check —
        // UnknownCard outranks NotPlayable in the guard order.
        st.players[2]
            .hand
            .push(crate::lc_cards::card_by_id("soft-04").unwrap());
        assert_eq!(st.arm(3, "soft-04"), Err(LcError::NotPlayable)); // Reaction, D9
        st.players[0].status = Status::Eliminated;
        assert_eq!(st.arm(1, "beer-01"), Err(LcError::NotAlive));
        st.players[0].status = Status::Alive;
        st.beat = Beat::Draw;
        assert_eq!(st.arm(1, "beer-01"), Err(LcError::WrongBeat));
    }

    #[test]
    fn test_arm_affordability_is_aggregate() {
        let mut st = at_lock();
        st.players[0].vessels[0].pulls_left = 2;
        st.arm(1, "beer-01").unwrap(); // cost 1, plan: 1 of 2
        assert_eq!(
            st.arm(1, "beer-02"), // cost 2, total 3 > 2
            Err(LcError::CantAfford("beer-02".into()))
        );
        // Handicap inflates the check (4.2's cost × handicap):
        let mut st = at_lock_with(|st| st.set_handicap(1, 150).unwrap()); // pull_cost(2,150) = 3
        st.players[0].vessels[0].pulls_left = 2;
        assert_eq!(
            st.arm(1, "beer-02"),
            Err(LcError::CantAfford("beer-02".into()))
        );
    }

    #[test]
    fn test_disarm_returns_the_card() {
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.disarm(1, "beer-01").unwrap();
        assert_eq!(st.players[0].hand.len(), 5);
        assert!(st.players[0].armed.is_empty());
        assert_eq!(st.disarm(1, "beer-01"), Err(LcError::UnknownCard));
    }

    #[test]
    fn test_set_target_classes() {
        // D2
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap(); // targets "one"
        assert_eq!(st.set_target(1, "beer-01", None), Err(LcError::BadTarget));
        assert_eq!(
            st.set_target(1, "beer-01", Some(7)),
            Err(LcError::BadTarget)
        );
        st.players[1].status = Status::Eliminated;
        assert_eq!(
            st.set_target(1, "beer-01", Some(1)),
            Err(LcError::BadTarget)
        );
        st.players[1].status = Status::Alive;
        st.set_target(1, "beer-01", Some(0)).unwrap(); // self-target a "one": legal
        st.set_target(1, "beer-01", Some(1)).unwrap(); // retargeting: legal
        st.arm(1, "beer-03").unwrap(); // targets "self"
        assert_eq!(
            st.set_target(1, "beer-03", Some(1)),
            Err(LcError::BadTarget)
        );
        st.set_target(1, "beer-03", None).unwrap();
    }

    /// D18: seq bumps on every successful mutating transition and on none
    /// that fails. `test_arm_moves_hand_to_armed` already pins arm's bump and
    /// `test_lock_in_stages_plays_and_pays_nothing` pins the idempotent
    /// replay's non-bump; this test covers the remaining combinations Plan E
    /// trusts: disarm and set_target bump on success, and a failed arm/
    /// set_target bumps nothing.
    #[test]
    fn test_seq_bumps_only_on_success_not_on_failure() {
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap();

        let before = st.seq;
        assert_eq!(
            st.arm(1, "beer-02-does-not-exist"),
            Err(LcError::UnknownCard)
        );
        assert_eq!(st.seq, before); // failed arm: no bump

        let before = st.seq;
        st.set_target(1, "beer-01", Some(1)).unwrap();
        assert_eq!(st.seq, before + 1); // successful set_target: bump

        let before = st.seq;
        assert_eq!(
            st.set_target(1, "beer-01", Some(7)),
            Err(LcError::BadTarget)
        );
        assert_eq!(st.seq, before); // failed set_target: no bump

        let before = st.seq;
        st.disarm(1, "beer-01").unwrap();
        assert_eq!(st.seq, before + 1); // successful disarm: bump
    }

    #[test]
    fn test_lock_in_stages_plays_and_pays_nothing() {
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.arm(1, "beer-03").unwrap(); // "self"
        st.lock_in(1).unwrap();
        let p = &st.players[0];
        assert!(p.locked);
        assert!(p.armed.is_empty());
        assert_eq!(p.vessels[0].pulls_left, 8); // payment at reveal (6.4)
        assert!(st.plays.is_empty()); // §3.4.1
        assert_eq!(st.locked_plays.len(), 2);
        assert_eq!(st.locked_plays[0].card.id, "beer-01");
        assert_eq!(st.locked_plays[0].target, Some(1));
        assert_eq!(st.locked_plays[1].target, Some(0)); // self → own seat (D2)
        assert_eq!(st.locked_plays[1].order_key, 0); // set at reveal, not here
                                                     // Idempotent replay: Ok, no bump, nothing re-staged.
        let seq = st.seq;
        st.lock_in(1).unwrap();
        assert_eq!((st.seq, st.locked_plays.len()), (seq, 2));
    }

    #[test]
    fn test_lock_in_names_the_failing_card() {
        // DDv2 6.3
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap(); // "one", no target yet
        assert_eq!(st.lock_in(1), Err(LcError::NeedsTarget("beer-01".into())));
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.arm(1, "beer-02").unwrap();
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.players[0].vessels[0].pulls_left = 2; // 1+2=3 > 2 now
        assert_eq!(st.lock_in(1), Err(LcError::CantAfford("beer-02".into())));
        assert!(!st.players[0].locked);
        assert_eq!(st.players[0].armed.len(), 2); // rejection stages nothing
    }

    /// MANDATORY (spec §3.4.1): the function that stages plays owns this test.
    /// A locked play is invisible to the projection during beats 1–4; only the
    /// lock tick is public.
    #[test]
    fn test_a_locked_play_is_absent_from_public_view_before_reveal() {
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        assert!(st.plays.is_empty());
        assert_eq!(st.locked_plays.len(), 1);
        for beat in [Beat::Draw, Beat::Deal, Beat::Diplomacy, Beat::Lock] {
            st.beat = beat;
            let view = st.public_view();
            assert!(view.revealed.is_empty(), "beat={beat:?}");
            let json = serde_json::to_string(&view).unwrap();
            assert!(!json.contains("beer-01"), "beat={beat:?}");
            assert!(!json.contains("Nudge"), "beat={beat:?}");
            assert!(
                view.seats[0].locked,
                "the lock tick IS public, beat={beat:?}"
            );
        }
    }

    /// Plan E, decision E6: the public hand-size projection must not move
    /// while a player stages cards, or an observer could infer an arm/lock
    /// from the plaque alone (the same secrecy boundary
    /// `test_a_locked_play_is_absent_from_public_view_before_reveal` pins
    /// for card identity). alice's Beer opener (F6) is 5 cards.
    #[test]
    fn test_public_hand_size_holds_still_while_staging() {
        let mut st = at_lock(); // alice holds a 5-card Beer opener
        assert_eq!(st.public_view().seats[0].hand_len, 5);
        st.arm(1, "beer-01").unwrap();
        assert_eq!(st.public_view().seats[0].hand_len, 5); // armed still counts
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        assert_eq!(st.public_view().seats[0].hand_len, 5); // staged still counts
        st.advance_beat().unwrap(); // the reveal
        assert_eq!(st.public_view().seats[0].hand_len, 4); // now it is public
    }

    #[test]
    fn test_acting_after_lock_is_rejected() {
        let mut st = at_lock();
        st.lock_in(1).unwrap(); // locking nothing is legal
        assert_eq!(st.arm(1, "beer-01"), Err(LcError::AlreadyLocked));
        assert_eq!(st.disarm(1, "beer-01"), Err(LcError::AlreadyLocked));
        assert_eq!(
            st.set_target(1, "beer-01", None),
            Err(LcError::AlreadyLocked)
        );
    }

    /// alice locks beer-01(→bob) then beer-02(→bob): 3 pulls. bob locks
    /// cider-04(→alice): 3 pulls. cara arms soft-01 but never locks.
    fn locked_table() -> LastCallState {
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.arm(1, "beer-02").unwrap();
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.arm(2, "cider-04").unwrap();
        st.set_target(2, "cider-04", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.arm(3, "soft-01").unwrap();
        st
    }

    #[test]
    fn test_advance_walks_the_beats_and_refuses_resolve() {
        let mut st = seated();
        for expected in [
            Beat::Deal,
            Beat::Diplomacy,
            Beat::Lock,
            Beat::Reveal,
            Beat::Resolve,
        ] {
            let seq = st.seq;
            st.advance_beat().unwrap();
            assert_eq!(st.beat, expected);
            assert_eq!(st.seq, seq + 1);
        }
        assert_eq!(st.advance_beat(), Err(LcError::MustResolve)); // D5
    }

    #[test]
    fn test_draw_to_deal_clears_drawing() {
        let mut st = seated();
        st.players[0].drawing = true;
        st.advance_beat().unwrap();
        assert!(!st.players[0].drawing);
    }

    #[test]
    fn test_reveal_charges_orders_and_flips() {
        let mut st = locked_table();
        st.advance_beat().unwrap(); // Lock → Reveal
        assert_eq!(st.beat, Beat::Reveal);
        assert!(st.locked_plays.is_empty());
        assert_eq!(st.players[0].vessels[0].pulls_left, 5); // Beer 8-3
        assert_eq!(st.players[1].vessels[0].pulls_left, 7); // Cider 10-3
        assert_eq!(st.players[2].vessels[0].pulls_left, 6); // cara never locked
                                                            // cara's armed card went home, uncharged (§12):
        assert_eq!(st.players[2].hand.len(), 5);
        assert!(st.players[2].armed.is_empty());
        // 3 = 3 tie → seat order from first_seat 0 → alice first, arming order:
        assert_eq!(
            st.plays
                .iter()
                .map(|p| (p.card.id.as_str(), p.order_key))
                .collect::<Vec<_>>(),
            vec![("beer-01", 1), ("beer-02", 2), ("cider-04", 3)]
        );
        // And the projection now — and only now — carries identity:
        let json = serde_json::to_string(&st.public_view()).unwrap();
        assert!(json.contains("Nudge"));
    }

    #[test]
    fn test_bigger_spender_acts_first() {
        // 7.1
        let mut st = at_lock();
        st.arm(1, "beer-01").unwrap(); // alice spends 1
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.arm(2, "cider-04").unwrap(); // bob spends 3
        st.set_target(2, "cider-04", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        assert_eq!(st.plays[0].card.id, "cider-04");
        assert_eq!(st.plays[0].order_key, 1);
    }

    #[test]
    fn test_first_seat_breaks_the_tie() {
        // 7.2
        let mut st = locked_table();
        st.first_seat = 1;
        st.advance_beat().unwrap();
        assert_eq!(st.plays[0].card.id, "cider-04"); // bob's seat leads now
    }

    #[test]
    fn test_handicap_inflates_the_charge() {
        // §11: cost only, rounded up
        let mut st = at_lock_with(|st| st.set_handicap(1, 150).unwrap());
        st.arm(1, "beer-01").unwrap(); // pull_cost(1,150) = 2
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        assert_eq!(st.players[0].vessels[0].pulls_left, 6); // 8 - 2
    }

    /// I1 (review finding, closed by D19): `set_handicap` is now Draw-gated,
    /// so a handicap raised after `lock_in` simply errs — the side channel
    /// that let 7.1's ordering total and the reveal charge disagree is
    /// closed at the source, not papered over with saturation. The reveal
    /// charge and the 7.1 ordering totals must match exactly what `lock_in`
    /// validated, because nothing was able to change in between.
    #[test]
    fn test_set_handicap_after_lock_is_rejected_and_reveal_matches_lock_time() {
        // alice locks beer-01(→bob)+beer-02(→bob) = 3 pulls; bob locks
        // cider-04(→alice) = 3 pulls (see `locked_table`).
        let mut st = locked_table();
        assert_eq!(st.set_handicap(1, 300), Err(LcError::WrongBeat));
        assert_eq!(st.players[0].handicap_pct, 100); // unchanged — no side channel
        st.advance_beat().unwrap(); // Lock -> Reveal
                                    // Charges match exactly the lock-time simulation (handicap 100% throughout):
        assert_eq!(st.players[0].vessels[0].pulls_left, 5); // Beer 8-3
        assert_eq!(st.players[1].vessels[0].pulls_left, 7); // Cider 10-3
                                                            // 7.1 ordering: 3 == 3 tie, alice's seat (first_seat 0) leads —
                                                            // exactly the lock-time totals, not the (rejected) inflated ones:
        assert_eq!(
            st.plays
                .iter()
                .map(|p| (p.card.id.as_str(), p.order_key))
                .collect::<Vec<_>>(),
            vec![("beer-01", 1), ("beer-02", 2), ("cider-04", 3)]
        );
    }

    #[test]
    fn test_staged_for_returns_only_the_named_seat_and_survives_the_flip() {
        let mut st = locked_table();
        // Pre-reveal: alice's two locked cards are visible only to her seat.
        assert_eq!(
            st.staged_for(0)
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec!["beer-01", "beer-02"]
        );
        assert_eq!(
            st.staged_for(1)
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cider-04"]
        );
        assert!(st.staged_for(2).is_empty()); // cara never locked
        st.advance_beat().unwrap(); // Lock → Reveal flips locked_plays into plays
        assert!(st.staged_for(0).is_empty()); // nothing left in locked_plays
        assert!(st.staged_for(1).is_empty());
    }

    #[test]
    fn test_preview_state_covers_every_variant() {
        let st = preview_state();
        assert_eq!(st.players.len(), 8);

        let mut decks_seen: Vec<Deck> = Vec::new();
        for p in &st.players {
            for v in &p.vessels {
                if !decks_seen.contains(&v.deck) {
                    decks_seen.push(v.deck);
                }
            }
        }
        for d in Deck::ALL {
            assert!(decks_seen.contains(&d), "missing deck {d:?}");
        }

        assert_eq!(
            st.players.iter().filter(|p| p.vessels.len() == 2).count(),
            1
        );
        assert_eq!(
            st.players
                .iter()
                .filter(|p| p.status == Status::Eliminated)
                .count(),
            1
        );
        assert_eq!(st.players.iter().filter(|p| p.locked).count(), 1);
        assert_eq!(st.players.iter().filter(|p| p.drawing).count(), 1);
        // Two-deck players (bob's non-oversized 10-card double-opener) are
        // legitimate now, not oversized — only the fixture's deliberate
        // 12-card hand should trip this.
        assert_eq!(st.players.iter().filter(|p| p.hand.len() > 10).count(), 1);
        assert_eq!(
            st.players.iter().filter(|p| p.draws_this_round > 0).count(),
            1
        );

        assert!(st.deck_counts.iter().any(|&(_, c)| c == 0));
        assert!(st.deck_counts.iter().any(|&(_, c)| (1..5).contains(&c)));
    }

    #[test]
    fn test_set_vessel_activates_and_debits_the_shoe() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        // 40 in, 5 opener cards dealt out.
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 5); // 35
        st.set_vessel(2, Deck::Beer, "can").unwrap();
        // No reactivation — same shoe, five more cards dealt.
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 10); // 30
                                                                    // Same-deck re-registration replaces the vessel; the dedupe deals 0
                                                                    // new cards, so the shoe is untouched.
        st.set_vessel(1, Deck::Beer, "bigger can").unwrap();
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 10);
        assert_eq!(st.players[0].vessels.len(), 1);
    }

    #[test]
    fn test_set_vessel_outside_draw_is_rejected() {
        let mut st = seated();
        st.beat = Beat::Lock;
        assert_eq!(st.set_vessel(1, Deck::Beer, "can"), Err(LcError::WrongBeat));
    }

    #[test]
    fn test_finish_and_draw_refills_and_draws() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 35, hand 5, 8/8
        st.players[0].vessels[0].pulls_left = 2; // most of the can is gone
        let before_seq = st.seq;
        let drawn = crate::lc_cards::deck_cards(Deck::Beer)[..5].to_vec();
        st.finish_and_draw(1, 0, drawn).unwrap();
        let p = &st.players[0];
        assert_eq!(p.vessels[0].pulls_left, 8); // fresh can
        assert_eq!(p.hand.len(), 10);
        assert_eq!(p.draws_this_round, 5);
        assert!(p.drawing);
        assert_eq!(deck_count(&st, Deck::Beer), 30);
        assert_eq!(st.seq, before_seq + 1);
    }

    #[test]
    fn test_one_finish_and_draw_per_round() {
        // TBD-5
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        let drawn = crate::lc_cards::deck_cards(Deck::Beer)[..5].to_vec();
        st.finish_and_draw(1, 0, drawn.clone()).unwrap();
        assert_eq!(st.finish_and_draw(1, 0, drawn), Err(LcError::BadDraw));
    }

    #[test]
    fn test_finish_and_draw_validates_the_batch() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 35 → expects 5
                                                      // Too few:
        assert_eq!(
            st.finish_and_draw(1, 0, crate::lc_cards::deck_cards(Deck::Beer)[..4].to_vec()),
            Err(LcError::BadDraw)
        );
        // Right count, wrong deck in the batch:
        let mut bad = crate::lc_cards::deck_cards(Deck::Beer)[..4].to_vec();
        bad.push(crate::lc_cards::card_by_id("cider-01").unwrap());
        assert_eq!(st.finish_and_draw(1, 0, bad), Err(LcError::BadDraw));
        // Bad vessel index:
        assert_eq!(st.finish_and_draw(1, 5, vec![]), Err(LcError::BadDraw));
        // Wrong beat:
        st.beat = Beat::Deal;
        assert_eq!(st.finish_and_draw(1, 0, vec![]), Err(LcError::WrongBeat));
    }

    #[test]
    fn test_short_shoe_draws_partial() {
        // D7
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        set_deck_count(&mut st, Deck::Beer, 3); // shoe nearly out → expects 3
        let five = crate::lc_cards::deck_cards(Deck::Beer)[..5].to_vec();
        assert_eq!(st.finish_and_draw(1, 0, five), Err(LcError::BadDraw));
        let three = crate::lc_cards::deck_cards(Deck::Beer)[..3].to_vec();
        st.finish_and_draw(1, 0, three).unwrap();
        assert_eq!(deck_count(&st, Deck::Beer), 0);
        assert_eq!(st.players[0].hand.len(), 8);
    }

    /// M7 (D7 taken to its edge): an empty shoe makes `expected = 0`, so an
    /// empty `drawn` batch is legal — the vessel still refills (a free
    /// top-up) and `drawing`/`draws_this_round` still register the beat-1
    /// action, but the hand gains nothing because there is nothing left to
    /// deal.
    #[test]
    fn test_finish_and_draw_at_an_empty_shoe_draws_nothing() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        set_deck_count(&mut st, Deck::Beer, 0);
        st.players[0].vessels[0].pulls_left = 2; // most of the can is gone
        let hand_before = st.players[0].hand.len();
        st.finish_and_draw(1, 0, vec![]).unwrap();
        assert_eq!(st.players[0].vessels[0].pulls_left, 8); // still refills
        assert_eq!(st.players[0].hand.len(), hand_before); // nothing to draw
        assert_eq!(st.players[0].draws_this_round, 0);
        assert!(st.players[0].drawing);
        assert_eq!(deck_count(&st, Deck::Beer), 0);
    }

    #[test]
    fn test_resolve_applies_damage_and_rolls_over() {
        let mut st = locked_table(); // alice 3 pulls → bob; bob 3 → alice
        st.advance_beat().unwrap(); // Reveal
        st.advance_beat().unwrap(); // Resolve
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 9); // 15 - 2 (beer-01) - 4 (beer-02)
        assert_eq!(st.players[0].hp, 9); // 15 - 6 (cider-04)
        assert!(st.plays.is_empty()); // the queue empties every round (§14)
        assert_eq!(st.discards.len(), 3);
        assert_eq!(st.round, 2);
        assert_eq!(st.beat, Beat::Draw);
        assert_eq!(st.first_seat, 1); // rotated (D13)
        assert!(st
            .players
            .iter()
            .all(|p| !p.locked && p.draws_this_round == 0));
        assert_eq!(st.outcome(), None);
    }

    #[test]
    fn test_resolve_wrong_beat() {
        let mut st = seated();
        assert_eq!(st.resolve(), Err(LcError::WrongBeat));
    }

    /// M3: no engine transition can empty `players` (only a hand-corrupted
    /// or `Default`/`{}` blob can), but `resolve` must degrade instead of
    /// panicking on `% self.players.len()`.
    #[test]
    fn test_resolve_on_empty_players_does_not_panic() {
        let mut st = LastCallState {
            beat: Beat::Resolve,
            ..Default::default()
        };
        let seq = st.seq;
        assert_eq!(st.resolve(), Ok(()));
        assert_eq!(st.beat, Beat::Resolve); // frozen — no rollover to do
        assert_eq!(st.seq, seq); // no mutation occurred — no bump (D18)
    }

    /// M3: a play whose `source_seat` no longer exists (e.g. a
    /// `from_json`-truncated `players` vec under a play staged by a
    /// pre-ceiling binary) fizzles like an eliminated source instead of
    /// panicking on `self.players[..]`.
    #[test]
    fn test_resolve_skips_a_play_from_a_truncated_source_seat() {
        let mut st = seated(); // 3 seats: 0, 1, 2
        st.beat = Beat::Resolve;
        set_deck_count(&mut st, Deck::Beer, 5); // keep the discard out of the reshuffle
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("beer-01").unwrap(),
            source_seat: 9, // no such seat
            target: Some(1),
            paid_from: Deck::Beer,
            order_key: 1,
        });
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, STARTING_HP); // untouched
        assert_eq!(st.discards.len(), 1); // the card still leaves play (8.4)
    }

    /// M3: a `targets == "one"` play whose target seat no longer exists
    /// fizzles the same way a dead target does, instead of panicking.
    #[test]
    fn test_resolve_fizzles_a_play_targeting_a_truncated_seat() {
        let mut st = seated();
        st.beat = Beat::Resolve;
        set_deck_count(&mut st, Deck::Beer, 5); // keep the discard out of the reshuffle
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("beer-01").unwrap(),
            source_seat: 0,
            target: Some(9), // no such seat
            paid_from: Deck::Beer,
            order_key: 1,
        });
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.hp == STARTING_HP));
        assert_eq!(st.discards.len(), 1);
    }

    /// M3: a stored `Effect` naming a subject seat that no longer exists is
    /// skipped at tick time rather than panicking.
    #[test]
    fn test_resolve_skips_a_dot_on_a_truncated_subject() {
        let mut st = seated();
        st.beat = Beat::Resolve;
        st.effects.push(Effect {
            source_play: 0,
            subject: 9, // no such seat
            op: EffectOp::Dot,
            magnitude: 5,
            expires_round: 99,
        });
        st.resolve().unwrap(); // must not panic
    }

    #[test]
    fn test_heal_has_no_ceiling() {
        // TBD-3
        let mut st = at_lock();
        st.arm(3, "soft-01").unwrap(); // Buff, cost 1, targets "one"
        st.set_target(3, "soft-01", Some(2)).unwrap(); // cara heals herself
        st.lock_in(3).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[2].hp, 17); // 15 + soft-01's Heal 2, past start
    }

    #[test]
    fn test_elimination_is_immediate_and_removes_unresolved_plays() {
        // 7.6, 7.5
        let mut st = at_lock();
        st.players[1].hp = 4;
        // alice arms beer-02 FIRST (4 dmg) then beer-01 (2 dmg), both → bob.
        st.arm(1, "beer-02").unwrap();
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap(); // 3 pulls
        st.arm(2, "cider-04").unwrap(); // bob answers, 3 pulls
        st.set_target(2, "cider-04", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        let _bob_hand = st.players[1].hand.len(); // 4 after arming cider-04
        st.resolve().unwrap();
        // Tie at 3 pulls, alice's seat leads: beer-02 lands, bob hits 0 —
        assert_eq!(st.players[1].hp, 0); // clamped, not negative
        assert_eq!(st.players[1].status, Status::Eliminated);
        // — bob's cider-04 never resolves (alice untouched), his hand discards,
        // and alice's second play fizzles on a dead target with pulls kept:
        assert_eq!(st.players[0].hp, 15);
        assert!(st.players[1].hand.is_empty()); // ghosts hold no cards (9.2)
        assert_eq!(st.players[0].vessels[0].pulls_left, 5); // 7.5: no refund
                                                            // beer-01 + beer-02 + cider-04 + bob's 4 hand cards:
        assert_eq!(st.discards.len(), 7);
        assert_eq!(st.outcome(), None); // cara still stands
    }

    #[test]
    fn test_last_player_standing_freezes_the_table() {
        // 9.3, D16
        let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 42);
        st.set_vessel(1, Deck::Liquor, "shot").unwrap();
        st.players[1].hp = 4;
        st.beat = Beat::Lock;
        st.arm(1, "liquor-02").unwrap(); // Atk cost 3 → 6 dmg
        st.set_target(1, "liquor-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.outcome(), Some(LcOutcome::Winner(0)));
        assert_eq!(st.public_view().outcome, Some(LcOutcome::Winner(0)));
        assert_eq!(st.beat, Beat::Resolve); // frozen final tableau, no rollover
        assert_eq!(st.round, 1);
    }

    #[test]
    fn test_curse_ticks_after_its_round_then_expires() {
        // D8, D10
        let mut st = at_lock();
        st.arm(2, "cider-01").unwrap(); // Curse cost 1 → Dot mag 1, 2 rounds
        st.set_target(2, "cider-01", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap(); // round 1: created, no tick
        assert_eq!(st.players[0].hp, 15);
        assert_eq!(st.effects.len(), 1);
        assert_eq!(st.effects[0].expires_round, 3); // 1 + cider-01's 2 rounds
        for expected_hp in [14, 13] {
            // ticks in rounds 2 and 3 — step straight to Resolve (as
            // `test_table_targets_class_resolves_no_subjects` does above)
            // rather than walking Draw->Deal: that edge now reveals this
            // seed's per-round event (H2, Plan H Task 2), which would
            // change hp for reasons unrelated to curse-duration semantics.
            st.beat = Beat::Resolve;
            st.resolve().unwrap();
            assert_eq!(st.players[0].hp, expected_hp);
        }
        assert!(st.effects.is_empty()); // expired after round 3
    }

    #[test]
    fn test_effects_replace_not_stack() {
        // TBD-8, D10
        let mut st = at_lock();
        st.effects.push(Effect {
            source_play: 0,
            subject: 0,
            op: EffectOp::Dot,
            magnitude: 2,
            expires_round: 9,
        });
        st.arm(2, "cider-01").unwrap();
        st.set_target(2, "cider-01", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        // The old dot ticked once (15-2), then the new curse replaced it:
        assert_eq!(st.players[0].hp, 13);
        assert_eq!(st.effects.len(), 1);
        assert_eq!(
            (st.effects[0].magnitude, st.effects[0].expires_round),
            (1, 3)
        );
    }

    #[test]
    fn test_shields_absorb_before_hp() {
        let mut st = at_lock();
        st.effects.push(Effect {
            source_play: 0,
            subject: 1,
            op: EffectOp::Shield,
            magnitude: 3,
            expires_round: 9,
        });
        st.arm(1, "beer-02").unwrap(); // 4 dmg → bob
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 14); // 3 absorbed, 1 through
        assert!(st.effects.is_empty()); // shield consumed and removed
    }

    #[test]
    fn test_soft_cap_discards_newest_first() {
        // TBD-2, D12
        let mut st = at_lock();
        st.advance_beat().unwrap(); // Reveal (nobody locked, nothing staged)
        st.advance_beat().unwrap(); // Resolve
                                    // Two full copies of Cider's 8 distinct ids, concatenated (16 total)
                                    // — a mutant that discards from the FRONT instead of the end would
                                    // discard copy one's first four ids (cider-01..04) instead of copy
                                    // two's last four (cider-05..08), and those two id sets are disjoint
                                    // enough to catch it without any suffixing trick.
        st.players[1].hand = std::iter::repeat_n(crate::lc_cards::deck_cards(Deck::Cider), 2)
            .flatten()
            .collect(); // 16
        st.resolve().unwrap();
        assert_eq!(st.players[1].hand.len(), HAND_SOFT_CAP);
        assert_eq!(st.discards.len(), 4);
        // Survivors: all 8 of copy one plus copy two's first four
        // (cider-01..04 again); discards: copy two's last four
        // (cider-05..08) — the newest cards, dropped from the end of the
        // hand (D12's "newest first"). Pinning both sides (not just the
        // discard) catches a mutant that discards from the end AND mangles
        // what it leaves behind:
        let hand_ids: Vec<&str> = st.players[1].hand.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            hand_ids,
            vec![
                "cider-01", "cider-02", "cider-03", "cider-04", "cider-05", "cider-06", "cider-07",
                "cider-08", "cider-01", "cider-02", "cider-03", "cider-04",
            ]
        );
        let discard_ids: Vec<&str> = st.discards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            discard_ids,
            vec!["cider-05", "cider-06", "cider-07", "cider-08"]
        );
    }

    #[test]
    fn test_rollover_reshuffles_an_empty_shoe() {
        // 8.4, §12
        let mut st = at_lock();
        set_deck_count(&mut st, Deck::Beer, 0);
        st.discards = crate::lc_cards::deck_cards(Deck::Beer)[..3].to_vec();
        st.discards
            .push(crate::lc_cards::card_by_id("cider-01").unwrap());
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(deck_count(&st, Deck::Beer), 3);
        assert_eq!(st.discards.len(), 1); // the cider card stays put
    }

    #[test]
    fn test_liquor_hits_above_par() {
        // F3's burst premium is engine-real
        let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 42);
        st.set_vessel(1, Deck::Liquor, "shot").unwrap();
        st.beat = Beat::Lock;
        st.arm(1, "liquor-01").unwrap(); // cost 2, Damage 5 — not 2 x cost
        st.set_target(1, "liquor-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 10); // 15 - 5
        assert_eq!(st.players[0].vessels[0].pulls_left, 2); // 4 - 2: cost, not fx
    }

    #[test]
    fn test_shield_card_protects_in_its_own_round_when_it_outspends() {
        // F8
        let mut st = at_lock();
        // soft-07 is not in Soft's opener (F6) — deal it into cara's hand.
        st.players[2]
            .hand
            .push(crate::lc_cards::card_by_id("soft-07").unwrap());
        // cara (Soft) spends 3 pulls (soft-07 + soft-01), alice (Beer) spends 2:
        // cara resolves first, Glass Wall lands on bob before Grind does.
        st.arm(3, "soft-07").unwrap();
        st.set_target(3, "soft-07", Some(1)).unwrap();
        st.arm(3, "soft-01").unwrap();
        st.set_target(3, "soft-01", Some(2)).unwrap();
        st.lock_in(3).unwrap();
        st.arm(1, "beer-02").unwrap(); // Damage 4 -> bob
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 15); // fully absorbed
        assert_eq!(st.effects.len(), 1); // shield survives, worn
        assert_eq!(st.effects[0].magnitude, 1); // 5 - 4
        assert_eq!(st.players[2].hp, 17); // soft-01 healed cara
    }

    #[test]
    fn test_a_cheap_shield_resolves_after_the_big_hit() {
        // F8's tension
        let mut st = at_lock();
        st.arm(3, "soft-02").unwrap(); // 1 pull, Shield 3 -> bob
        st.set_target(3, "soft-02", Some(1)).unwrap();
        st.lock_in(3).unwrap();
        st.arm(1, "beer-02").unwrap(); // 2 pulls, Damage 4 -> bob
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 11); // alice outspent: hit lands first
        assert_eq!(st.effects[0].magnitude, 3); // the late shield arrives intact
    }

    #[test]
    fn test_drain_hits_the_fullest_vessel_and_floors_at_zero() {
        // F4
        let mut st = at_lock();
        // Alice's second vessel is built by hand: set_vessel is Draw-gated and
        // the fixture is already at Lock.
        st.players[0].vessels.push(Vessel {
            deck: Deck::Soft,
            pulls_max: 6,
            pulls_left: 3,
            container: "cup".into(),
        });
        st.arm(2, "cider-03").unwrap(); // Drain 3 -> alice
        st.set_target(2, "cider-03", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        // Beer vessel was fullest (8 vs 3): drained to 5, Soft cup untouched.
        assert_eq!(st.players[0].vessels[0].pulls_left, 5);
        assert_eq!(st.players[0].vessels[1].pulls_left, 3);
        assert_eq!(st.players[0].hp, 15); // drains never touch HP

        // Floor: drain more than remains.
        let mut st = at_lock();
        st.players[0].vessels[0].pulls_left = 2;
        st.arm(2, "cider-03").unwrap();
        st.set_target(2, "cider-03", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[0].vessels[0].pulls_left, 0);
    }

    #[test]
    fn test_aoe_includes_the_source() {
        // F9 / D2
        let mut st = at_lock();
        // beer-05 is not in Beer's opener (F6) — deal it into alice's hand.
        st.players[0]
            .hand
            .push(crate::lc_cards::card_by_id("beer-05").unwrap());
        st.arm(1, "beer-05").unwrap(); // Damage 1 to all, no target needed
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        for p in &st.players {
            assert_eq!(p.hp, 14, "seat {}", p.seat);
        }
    }

    #[test]
    fn test_a_reaction_play_resolves_inert() {
        // F5 — even if one sneaks in
        let mut st = at_lock();
        st.plays.push(Play {
            card: crate::lc_cards::card_by_id("beer-08").unwrap(),
            source_seat: 0,
            target: Some(1),
            paid_from: Deck::Beer,
            order_key: 1,
        });
        st.beat = Beat::Resolve;
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.hp == 15));
        assert_eq!(st.discards.len(), 1); // still discarded (8.4)
    }

    #[test]
    fn test_dot_duration_is_per_card() {
        // F10: no CURSE_ROUNDS
        // cider-07 (Dot 2 x 2) beside cider-01 (Dot 1 x 2): the magnitude comes
        // from the card, not from cost x DOT_PER_COST. cider-07 is not in
        // Cider's opener (F6) — deal it into bob's hand.
        let mut st = at_lock();
        st.players[1]
            .hand
            .push(crate::lc_cards::card_by_id("cider-07").unwrap());
        st.arm(2, "cider-07").unwrap();
        st.set_target(2, "cider-07", Some(0)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.effects[0].magnitude, 2); // per-card, not 1 x cost
        assert_eq!(st.effects[0].expires_round, 3); // round 1 + rounds 2
    }

    /// M8 (Plan D review, carried to Plan F): a same-resolve() double
    /// elimination. Blocked under the placeholder catalog for lack of an AoE
    /// Atk card — beer-05 ("One For The Table", Damage 1 to all) is real
    /// now. All three seats are set to lethal HP before the AoE lands, so
    /// every subject in the `"all"` list hits 0 within the same play's
    /// subject loop (the list is snapshotted before any of them apply,
    /// D2/M3) and the table collapses to a Draw in the same `resolve()` call
    /// that killed them.
    #[test]
    fn test_same_resolve_double_elimination_collapses_to_draw() {
        let mut st = at_lock();
        st.players[0].hp = 1;
        st.players[1].hp = 1;
        st.players[2].hp = 1;
        // beer-05 is not in Beer's opener (F6) — deal it into alice's hand.
        st.players[0]
            .hand
            .push(crate::lc_cards::card_by_id("beer-05").unwrap());
        st.arm(1, "beer-05").unwrap(); // Damage 1 to all, including the source
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.status == Status::Eliminated));
        assert!(st.players.iter().all(|p| p.hp == 0));
        assert_eq!(st.outcome(), Some(LcOutcome::Draw));
        assert_eq!(st.beat, Beat::Resolve); // frozen final tableau (D16)
    }

    /// M8 (Plan D review, carried to Plan F): the `"table"` targets-class
    /// subject-resolution fallback. The catalog still has no `targets ==
    /// "table"` card (`test_targets_are_a_known_class` enforces only
    /// self/one/all), so this drives the `_ => Vec::new()` arm directly with
    /// a constructed `Card` — id "beer-02" so `card_fx` returns a real,
    /// non-`None` `Damage 4` effect, isolating the fallback (empty subject
    /// list) from the separate fail-soft-unknown-id path (`None` effect)
    /// that a made-up id would otherwise conflate it with.
    #[test]
    fn test_table_targets_class_resolves_no_subjects() {
        let mut st = at_lock();
        let mut card = crate::lc_cards::card_by_id("beer-02").unwrap(); // real Damage 4 fx
        card.targets = "table".into(); // no card in the current catalog uses this class
        st.plays.push(Play {
            card,
            source_seat: 0,
            target: None,
            paid_from: Deck::Beer,
            order_key: 1,
        });
        st.beat = Beat::Resolve;
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.hp == 15)); // no subject resolved, nothing hit
        assert_eq!(st.discards.len(), 1); // card still leaves play (8.4)
    }

    // Plan H Task 2: events in the engine.

    #[test]
    fn test_the_event_lives_deal_to_resolve() {
        // H2 — and "never two at once"
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.round = 2; // round 1's Draw is the lobby; use a plain round
        assert_eq!(st.event, None); // Draw: no event
        st.advance_beat().unwrap(); // Draw -> Deal: the reveal
        assert_eq!(st.event.as_deref(), Some("toast")); // seed 42, round 2 (Task 1 pin)
        assert_eq!(st.public_view().event.as_deref(), Some("toast"));
        for _ in 0..4 {
            st.advance_beat().unwrap();
        } // ... -> Resolve
        assert_eq!(st.event.as_deref(), Some("toast")); // still the same ONE event
        st.resolve().unwrap();
        assert_eq!(st.event, None); // rollover cleared it
        st.advance_beat().unwrap(); // round 3's Deal
        assert_eq!(st.event.as_deref(), Some("double-vision")); // replaced, never two
    }

    #[test]
    fn test_toast_pours_one_and_heals_two() {
        // H4, at the Deal reveal
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap(); // 8 pulls
        st.set_vessel(2, Deck::Cider, "bottle").unwrap(); // 10
        st.set_vessel(3, Deck::Soft, "glass").unwrap(); // 6
        st.round = 2;
        st.advance_beat().unwrap(); // reveals toast (seed 42 round 2)
        assert_eq!(st.players[0].vessels[0].pulls_left, 7);
        assert_eq!(st.players[1].vessels[0].pulls_left, 9);
        assert_eq!(st.players[2].vessels[0].pulls_left, 5);
        assert!(st.players.iter().all(|p| p.hp == 17));
    }

    #[test]
    fn test_happy_hour_halves_the_charge_and_the_chip_agrees() {
        // H4, H12
        let mut st = at_lock();
        st.event = Some("happy-hour".into());
        st.arm(1, "beer-02").unwrap(); // cost 2 -> pull_cost 2 -> halved 1
        st.set_target(1, "beer-02", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap(); // the reveal charges
        assert_eq!(st.players[0].vessels[0].pulls_left, 7); // 8 - 1, not 8 - 2
        assert_eq!(st.charged_pulls(0), 1); // what the DRINK chip will show
        assert_eq!(st.effective_pull_cost(3, 150), 3); // ceil(ceil(3*1.5)/2) = ceil(5/2)
    }

    #[test]
    fn test_last_orders_charges_the_silent() {
        // H4
        let mut st = at_lock();
        st.event = Some("last-orders".into());
        st.arm(1, "beer-01").unwrap();
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, 15); // played: exempt
        assert_eq!(st.players[1].hp, 11); // 15 - 2 (beer-01) - 2 (penalty)
        assert_eq!(st.players[2].hp, 13); // 15 - 2 (penalty)
    }

    #[test]
    fn test_double_vision_redirects_attacks_not_heals() {
        // H4
        let mut st = at_lock();
        st.event = Some("double-vision".into());
        st.arm(1, "beer-01").unwrap(); // Damage 2, aimed at bob (seat 1)
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.arm(3, "soft-01").unwrap(); // Heal 2, aimed at alice (seat 0)
        st.set_target(3, "soft-01", Some(0)).unwrap();
        st.lock_in(3).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[1].hp, 15); // aimed at, missed
        assert_eq!(st.players[2].hp, 13); // seat left of the target took it
        assert_eq!(st.players[0].hp, 17); // the heal landed where aimed
    }

    #[test]
    fn test_big_shot_taxes_the_top_spender() {
        // H4
        let mut st = at_lock();
        st.event = Some("big-shot".into());
        st.arm(1, "beer-02").unwrap(); // 2 pulls — the big spender
        st.set_target(1, "beer-02", Some(2)).unwrap();
        st.lock_in(1).unwrap();
        st.arm(2, "cider-02").unwrap(); // 1 pull (drain -> cara)
        st.set_target(2, "cider-02", Some(2)).unwrap();
        st.lock_in(2).unwrap();
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, 13); // top spender taxed
        assert_eq!(st.players[1].hp, 15); // underbidder untouched
        assert_eq!(st.players[2].hp, 11); // beer-02's 4, plus nothing else
    }

    #[test]
    fn test_house_pour_doubles_the_tick_this_round_only() {
        // H4
        let mut st = at_lock();
        st.effects.push(Effect {
            source_play: 0,
            subject: 0,
            op: EffectOp::Dot,
            magnitude: 1,
            expires_round: 9,
        });
        st.event = Some("house-pour".into());
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, 13); // 15 - 1*2 — and event now cleared
                                          // The rollover already cleared `event` (H2). Step straight to
                                          // Resolve rather than walking Draw->Deal again: that edge would
                                          // cross this seed's round-2 reveal (`toast`, +2 HP to the table)
                                          // — an unrelated event's Deal-time hook firing inside a
                                          // house-pour-only assertion. Setting `event` back to `None`
                                          // afterward can't undo a hook that already fired; skipping the
                                          // edge is the isolation this test wants.
        assert_eq!(st.event, None);
        st.beat = Beat::Resolve;
        st.resolve().unwrap();
        assert_eq!(st.players[0].hp, 12); // back to 1 per tick
    }

    #[test]
    fn test_on_the_house_heals_the_table() {
        // H4
        let mut st = at_lock();
        st.event = Some("on-the-house".into());
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap();
        assert!(st.players.iter().all(|p| p.hp == 17));
    }

    #[test]
    fn test_an_unknown_event_id_is_inert() {
        // H3's fail-soft
        let mut st = at_lock();
        st.event = Some("closing-time".into()); // an id this binary never knew
        st.advance_beat().unwrap();
        st.advance_beat().unwrap();
        st.resolve().unwrap(); // no panic, no hook fired
        assert!(st.players.iter().all(|p| p.hp == 15));
    }
}
