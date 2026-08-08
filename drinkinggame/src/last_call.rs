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
    pub armed: Vec<Card>,
    pub locked: bool,
    pub drawing: bool,
    pub draws_this_round: u16,
    pub tabs: Vec<String>,
    pub status: Status,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Play {
    pub card: Card,
    pub source_seat: usize,
    pub target: Option<usize>,
    pub paid_from: Deck,
    pub order_key: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Effect {
    pub source_play: u32,
    pub subject: usize,
    pub op: String,
    pub magnitude: i32,
    pub expires_round: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct LastCallState {
    pub players: Vec<LcPlayer>,
    pub round: u32,
    pub beat: Beat,
    pub first_seat: usize,
    pub rng_seed: u64,
    pub plays: Vec<Play>,
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
}

#[derive(Debug, PartialEq, Eq)]
pub enum LcError {
    NotSeated,
    BadHandicap,
    NotImplemented,
}

pub const STARTING_HP: i32 = 15; // DDv2 §2.4, TBD-1
pub const MAX_SEATS: usize = 8; // DDv2 §2.1 (2-8)
pub const HANDICAP_MIN_PCT: u16 = 25;
pub const HANDICAP_MAX_PCT: u16 = 300;
/// Under this many cards a DeckStack count turns amber (`data-low`).
pub const DECK_LOW_THRESHOLD: u16 = 5;

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
    pub fn new(members: Vec<(i64, String)>, rng_seed: u64) -> Self {
        let players = members
            .into_iter()
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
    /// this engine's own output, so a parse failure is a programming error.
    ///
    /// INVARIANT this creates: every writer must pass `Some(&st.to_json())`
    /// to `db::start_game` (Plan A2), because every reader does
    /// `from_json(game.state_json.as_deref().unwrap_or_default())` and `""`
    /// is not valid JSON.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).expect("valid LastCallState JSON")
    }

    pub fn seat_of(&self, player_id: i64) -> Option<usize> {
        self.players
            .iter()
            .find(|p| p.player_id == player_id)
            .map(|p| p.seat)
    }

    /// Mid-game join. Mirrors `ThreeManState::add_player`: no-op if already
    /// seated, otherwise push at `seat = players.len()`. Does not enforce
    /// `MAX_SEATS` — that is a Plan B seat-ring layout concern; the constant
    /// is exported for it.
    pub fn add_player(&mut self, player_id: i64, name: &str) {
        if self.seat_of(player_id).is_some() {
            return;
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
    }

    /// Registers the player's drink. `pulls_max` is a DECK constant
    /// (DDv2 §3.2); `container` is a free-text label and never affects it.
    ///
    /// Slice-1 stub deal: the vessel also seeds the player's hand with that
    /// deck's placeholder cards, because no Draw beat exists yet to do it.
    /// The Draw beat replaces this in slice 3.
    pub fn set_vessel(
        &mut self,
        player_id: i64,
        deck: Deck,
        container: &str,
    ) -> Result<(), LcError> {
        let Some(seat) = self.seat_of(player_id) else {
            return Err(LcError::NotSeated);
        };
        let p = &mut self.players[seat];
        p.vessels.retain(|v| v.deck != deck);
        p.vessels.push(Vessel {
            deck,
            pulls_max: deck.pulls(),
            pulls_left: deck.pulls(),
            container: container.to_string(),
        });
        for card in crate::lc_cards::deck_cards(deck) {
            if !p.hand.iter().any(|c| c.id == card.id) {
                p.hand.push(card);
            }
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
        }
    }

    // Slice 1 defines the shape; slice 3 (the loop) fills these in. The
    // object model is expensive to change later; transitions are not.

    pub fn arm(&mut self, _player_id: i64, _card_id: &str) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }

    pub fn disarm(&mut self, _player_id: i64, _card_id: &str) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }

    pub fn lock_in(&mut self, _player_id: i64) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }

    pub fn advance_beat(&mut self) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }

    pub fn resolve(&mut self) -> Result<(), LcError> {
        Err(LcError::NotImplemented)
    }
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
    st.beat = Beat::Lock;
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
    // hand to split, and the strip only ever reads a COUNT. Slice 2's
    // HandWheel indexes by card id, so vary the ids before building it.
    //
    // `Card` isn't `Copy` (it carries owned `String`s), so `[T]::repeat`
    // (as the brief's literal snippet used) doesn't compile here; build the
    // repeat via `iter::repeat_n` instead.
    st.players[1].hand = std::iter::repeat_n(crate::lc_cards::deck_cards(Deck::Cider), 3)
        .flatten()
        .collect();
    st.players[0].draws_this_round = 3; // the plaque's draw badge
    st.set_vessel(8, Deck::Soft, "any").unwrap(); // 8th seat: MAX_SEATS ceiling
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
            op: "damage".to_string(),
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
        st.add_player(2, "bob");
        assert_eq!(st.players.len(), 3);

        st.add_player(9, "dan");
        assert_eq!(st.players.len(), 4);
        assert_eq!(st.players[3].seat, 3);
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

    #[test]
    fn test_stubs_are_not_implemented() {
        let mut st = seated();
        let before = st.to_json();
        assert_eq!(st.arm(1, "beer-01"), Err(LcError::NotImplemented));
        assert_eq!(st.lock_in(1), Err(LcError::NotImplemented));
        assert_eq!(st.advance_beat(), Err(LcError::NotImplemented));
        assert_eq!(st.resolve(), Err(LcError::NotImplemented));
        assert_eq!(st.to_json(), before);
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
}
