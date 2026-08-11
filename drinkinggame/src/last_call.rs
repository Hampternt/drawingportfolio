//! Last Call card-game engine — a pure state machine, no I/O, no SQL, no RNG.
//!
//! Slice 1 (this module) defines the object model, the `PublicView`
//! confidentiality projection and a placeholder-dealing `set_vessel`. The
//! beat transitions (`arm`, `disarm`, `lock_in`, `advance_beat`, `resolve`)
//! are stubbed here with their final signatures; slice 3 (the loop) fills in
//! the bodies. `LastCallState` round-trips losslessly through
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
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectOp {
    Damage,
    Heal,
    Shield,
    Dot,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Effect {
    pub source_play: u32,
    pub subject: usize,
    pub op: EffectOp,
    pub magnitude: i32,
    pub expires_round: u32,
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
}

/// DDv2 9.3 — the two ways a game ends.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LcOutcome {
    /// The winning seat.
    Winner(usize),
    /// All remaining players are ghosts (DDv2 9.3).
    Draw,
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
    /// Dies in Task 5 with the last stub.
    NotImplemented,
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
pub const LC_DECK_SIZE: u16 = 40; // placeholder shoe size (D6) — Plan F resets
pub const DMG_PER_COST: i32 = 2; // placeholder mapping (D8)
pub const HEAL_PER_COST: i32 = 2; // placeholder mapping (D8)
pub const DOT_PER_COST: i32 = 1; // placeholder mapping (D8)
pub const CURSE_ROUNDS: u32 = 2; // placeholder mapping (D8)

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
        let mut alive = self.players.iter().filter(|p| p.status == Status::Alive);
        match (alive.next(), alive.next()) {
            (Some(p), None) => Some(LcOutcome::Winner(p.seat)),
            (None, _) => Some(LcOutcome::Draw),
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
    /// Bumps `seq` exactly when a player is actually newly seated — not on
    /// the idempotent replay, not on the full-table `None` — because `seq`
    /// is the freshness floor every SSE-driven repaint compares against
    /// (`lcApply`/`lcApplyTable`'s `if (seq < lcSeq) return`), and two
    /// distinct seatings sharing one `seq` would let the client's
    /// equal-seq-is-a-harmless-duplicate allowance silently accept a stale
    /// repaint as current.
    pub fn add_player(&mut self, player_id: i64, name: &str) -> Option<usize> {
        if let Some(seat) = self.seat_of(player_id) {
            return Some(seat);
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
    /// Placeholder deal (still slice-1 mechanics, kept verbatim by D15): the
    /// vessel also seeds the player's hand with that deck's placeholder
    /// cards. New here: if no player currently holds a vessel of `deck`, its
    /// shoe activates at `LC_DECK_SIZE` (D6) before the deal, and the cards
    /// actually pushed to the hand (the same-deck-replace dedupe can push
    /// zero) are debited from the shoe.
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
        for card in crate::lc_cards::deck_cards(deck) {
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

    pub fn set_handicap(&mut self, target_id: i64, handicap_pct: u16) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(target_id) else {
            return Err(LcError::NotSeated);
        };
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
                    hand_len: p.hand.len(),
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
        }
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
        payment_plan(&trial)?;

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
        payment_plan(p)?;

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
            }
            Beat::Lock => self.reveal(),
            // Deal→Diplomacy, Diplomacy→Lock, Reveal→Resolve: events, tabs,
            // the swap and the reaction window are hollow systems (D14, D9).
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
        // the same cards and accepted them; vessels cannot change between
        // lock and reveal (arming and drawing both live in other beats), so
        // this cannot fail — it is re-run rather than cached only because
        // `handicap_pct` (which `pull_cost` depends on) is settable at any
        // beat via `set_handicap`, and re-simulating against the player's
        // *current* vessels/handicap is what keeps this charge and
        // lock_in's/arm's earlier checks agreeing on what "affordable"
        // meant, per D3's single shared `payment_plan` helper.
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
            let plan = payment_plan(&trial).expect(
                "vessels cannot change between lock and reveal; lock_in already validated this payment plan",
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
            totals[play.source_seat] += pull_cost(play.card.cost, handicap) as u32;
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

    pub fn resolve(&mut self) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }
}

/// Private helper, shared with Task 4's reveal charge: the deterministic
/// greedy payment simulation (D3). Simulates the player's armed cards in
/// arming order against a local copy of each vessel's `pulls_left`: for each
/// card, picks the vessel of `card.deck` with the greatest remaining
/// simulated `pulls_left` (a tie keeps the lowest index — the first-seen
/// candidate is never displaced by an equal one), deducts
/// `pull_cost(card.cost, handicap_pct)`. Returns, per armed card in order,
/// the `(vessel index, pulls)` it pays — or `CantAfford` naming the first
/// card for which no vessel of its deck can cover the cost.
fn payment_plan(player: &LcPlayer) -> Result<Vec<(usize, u8)>, LcError> {
    let mut sim: Vec<u8> = player.vessels.iter().map(|v| v.pulls_left).collect();
    let mut plan = Vec::with_capacity(player.armed.len());
    for a in &player.armed {
        let cost = pull_cost(a.card.cost, player.handicap_pct);
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

    // 12 cards: four distinct Cider ids repeated three times. Deliberate —
    // it bypasses set_vessel's dedupe so the n > 8 hand-strip split has a
    // hand to split, and the strip only ever reads a COUNT. The wheel now
    // indexes by DOM position, not card id (decision 10) — but three
    // visually identical card triples would still make the preview's
    // oversized wheel look broken, so the second and third rep's ids are
    // suffixed to stay distinct.
    st.players[1].hand = (0..3)
        .flat_map(|rep| {
            crate::lc_cards::deck_cards(Deck::Cider)
                .into_iter()
                .map(move |mut c| {
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
    st.discards = crate::lc_cards::deck_cards(Deck::Beer); // discard count 4
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

        assert_eq!(LastCallState::from_json(&st.to_json()), st);
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
        assert_eq!(view.seats[0].hand_len, 4);
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
        assert_eq!(seat0.hand_len, 8);
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
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Soft, "glass").unwrap();
        st.beat = Beat::Lock;
        st
    }

    #[test]
    fn test_arm_moves_hand_to_armed() {
        let mut st = at_lock();
        let before = st.seq;
        st.arm(1, "beer-01").unwrap();
        assert_eq!(st.players[0].hand.len(), 3);
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
        let mut st = at_lock();
        st.players[0].vessels[0].pulls_left = 2;
        st.set_handicap(1, 150).unwrap(); // pull_cost(2,150) = 3
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
        assert_eq!(st.players[0].hand.len(), 4);
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
        assert_eq!(st.players[2].hand.len(), 4);
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
        let mut st = at_lock();
        st.set_handicap(1, 150).unwrap();
        st.arm(1, "beer-01").unwrap(); // pull_cost(1,150) = 2
        st.set_target(1, "beer-01", Some(1)).unwrap();
        st.lock_in(1).unwrap();
        st.advance_beat().unwrap();
        assert_eq!(st.players[0].vessels[0].pulls_left, 6); // 8 - 2
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
        assert_eq!(st.players.iter().filter(|p| p.hand.len() > 8).count(), 1);
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
        // 40 in, 4 catalog cards dealt out.
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 4); // 36
        st.set_vessel(2, Deck::Beer, "can").unwrap();
        // No reactivation — same shoe, four more cards dealt.
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 8); // 32
                                                                   // Same-deck re-registration replaces the vessel; the dedupe deals 0
                                                                   // new cards, so the shoe is untouched.
        st.set_vessel(1, Deck::Beer, "bigger can").unwrap();
        assert_eq!(deck_count(&st, Deck::Beer), LC_DECK_SIZE - 8);
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
        st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 36, hand 4, 8/8
        st.players[0].vessels[0].pulls_left = 2; // most of the can is gone
        let before_seq = st.seq;
        let mut drawn = crate::lc_cards::deck_cards(Deck::Beer); // 4
        drawn.push(crate::lc_cards::card_by_id("beer-01").unwrap()); // 5 — dups fine
        st.finish_and_draw(1, 0, drawn).unwrap();
        let p = &st.players[0];
        assert_eq!(p.vessels[0].pulls_left, 8); // fresh can
        assert_eq!(p.hand.len(), 9);
        assert_eq!(p.draws_this_round, 5);
        assert!(p.drawing);
        assert_eq!(deck_count(&st, Deck::Beer), 31);
        assert_eq!(st.seq, before_seq + 1);
    }

    #[test]
    fn test_one_finish_and_draw_per_round() {
        // TBD-5
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        let mut drawn = crate::lc_cards::deck_cards(Deck::Beer);
        drawn.push(crate::lc_cards::card_by_id("beer-01").unwrap());
        st.finish_and_draw(1, 0, drawn.clone()).unwrap();
        assert_eq!(st.finish_and_draw(1, 0, drawn), Err(LcError::BadDraw));
    }

    #[test]
    fn test_finish_and_draw_validates_the_batch() {
        let mut st = seated();
        st.set_vessel(1, Deck::Beer, "can").unwrap(); // shoe 36 → expects 5
                                                      // Too few:
        assert_eq!(
            st.finish_and_draw(1, 0, crate::lc_cards::deck_cards(Deck::Beer)),
            Err(LcError::BadDraw)
        );
        // Right count, wrong deck in the batch:
        let mut bad = crate::lc_cards::deck_cards(Deck::Beer);
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
        let mut five = crate::lc_cards::deck_cards(Deck::Beer);
        five.push(crate::lc_cards::card_by_id("beer-01").unwrap());
        assert_eq!(st.finish_and_draw(1, 0, five), Err(LcError::BadDraw));
        let three = crate::lc_cards::deck_cards(Deck::Beer)[..3].to_vec();
        st.finish_and_draw(1, 0, three).unwrap();
        assert_eq!(deck_count(&st, Deck::Beer), 0);
        assert_eq!(st.players[0].hand.len(), 7);
    }
}
