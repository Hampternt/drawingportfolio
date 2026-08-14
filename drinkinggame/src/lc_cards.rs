//! Last Call's real card catalog (DDv2 §9 / Plan F). Forty cards across five
//! decks (F2: `id`, `deck`, `kind`, `cost`, `targets`, `title`, `text`,
//! `keywords`, `copies`, `fx`), eight distinct cards per deck at deck-specific
//! copy weights that each sum to `LC_DECK_SIZE` (F3: Beer is a low-cost
//! attrition deck, Cider trades hits for pull-drains, Wine leans on damage
//! over time, Liquor is the small, expensive burst deck, and Soft is the
//! support deck — shields and heals at a premium). `MECHANICAL_KW` (F7) is
//! the small vocabulary of keywords a card's rendering can key rules-relevant
//! behavior off of; `TONE_KW` is flavor vocabulary, whitelisted for the
//! renderer but backing no mechanic. `shoe()` expands the catalog into a
//! 40-card, copy-weighted list — sampling from it is a draw *with*
//! replacement across draws, since the returned `Vec` is not itself mutated
//! or shuffled by anything here (F11); Plan E owns the sampling.
//!
//! F5 originally shipped the five `Reaction` cards inert (`fx: None`, text
//! disclaiming the response window did not exist yet). Plan I arms them:
//! the response window is `Beat::Reveal`, and `rfx: Option<ReactionFx>` is
//! their rules column — a catalog-side analog to `fx` (I4/I6), resolved by
//! id at play time and applied by `resolve()` as play-queue modifiers.
//!
//! The challenge-cards container (2026-08-14) adds a third rules column on
//! the same pattern: `chfx: Option<ChallengeDef>` for `CardKind::Challenge`
//! cards, whose contest resolves at the table (vote) rather than in the
//! engine. Challenge prototypes may carry `copies: 0` — catalog-present,
//! shoe-absent — until Pack 3 balances the real wave into the decks.

use crate::last_call::{Card, CardKind, Deck, EffectOp};

/// A card's rules, resolved by id at play time — never stored in the blob
/// (see the version-skew note on LastCallState; a retune must reach
/// in-flight games).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FxDef {
    pub op: EffectOp,
    pub magnitude: i32,
    /// 0 = immediate. n >= 1 = persists; expires_round = round + n.
    pub rounds: u32,
}

/// A reaction's rules, resolved by id at play time — catalog-side like FxDef
/// (F1): never stored in the blob, so a retune reaches in-flight games.
/// Applied by resolve() as play-queue modifiers (decision I4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionFx {
    /// The answered play resolves as nothing (7.5 parity: pulls stay spent).
    Cancel,
    /// The answered play deals this much less damage to the reactor.
    Reduce(i32),
    /// The answered play resolves against its own source instead.
    Reflect,
}

/// The shape of a challenge card's real-life contest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contest {
    /// Instigator vs the first Alive seat to their right (table order);
    /// the table votes a winner.
    Duel,
    /// The player performs the card's task themselves; the table votes
    /// pass/fail.
    Solo,
}

/// What the challenge's loser suffers. `Drink` and `Rule` resolve in real
/// life — the engine only records and displays them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Penalty {
    Damage(i32),
    /// Pulls drained from the loser's fullest vessel.
    Drain(i32),
    /// Real-life sips; no game-state change beyond the log.
    Drink(u8),
    /// A personal rule (text) the loser carries for this many rounds. The
    /// text lives HERE — the blob stores only `(card_id, expires_round)`,
    /// so a reworded rule reaches in-flight games (the FxDef principle).
    Rule(&'static str, u32),
}

/// A challenge card's rules, resolved by id at play time — catalog-side
/// like FxDef/ReactionFx: never stored in the blob; unknown ids resolve
/// inert (fail-soft).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeDef {
    pub contest: Contest,
    pub penalty: Penalty,
}

pub struct CardDef {
    pub id: &'static str,
    pub deck: Deck,
    pub kind: CardKind,
    pub cost: u8,
    pub targets: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub keywords: &'static [&'static str],
    pub copies: u8,                 // shoe frequency; per-deck sum == LC_DECK_SIZE
    pub fx: Option<FxDef>,          // None ⇔ kind == Reaction or Challenge (F5)
    pub rfx: Option<ReactionFx>,    // Some ⇔ kind == Reaction — the reaction's rules (I4/I6)
    pub chfx: Option<ChallengeDef>, // Some ⇔ kind == Challenge — the contest's rules
}

const fn fx(op: EffectOp, magnitude: i32, rounds: u32) -> Option<FxDef> {
    Some(FxDef {
        op,
        magnitude,
        rounds,
    })
}
const fn dmg(m: i32) -> Option<FxDef> {
    fx(EffectOp::Damage, m, 0)
}
const fn heal(m: i32) -> Option<FxDef> {
    fx(EffectOp::Heal, m, 0)
}
const fn shield(m: i32, r: u32) -> Option<FxDef> {
    fx(EffectOp::Shield, m, r)
}
const fn dot(m: i32, r: u32) -> Option<FxDef> {
    fx(EffectOp::Dot, m, r)
}
const fn drain(m: i32) -> Option<FxDef> {
    fx(EffectOp::PullDrain, m, 0)
}

/// F7 — mechanical keywords, each a tested predicate over card data.
pub const MECHANICAL_KW: [&str; 8] = [
    "aoe",
    "burst",
    "dot",
    "shield",
    "heal",
    "drain",
    "reaction",
    "challenge",
];
/// F7 — tone keywords: cosmetic vocabulary, whitelisted, NO rules attach.
pub const TONE_KW: [&str; 6] = ["loud", "public", "petty", "slow", "showy", "quiet"];
/// Single-target immediate damage at or above this carries `burst`.
pub const BURST_KW_MIN_DAMAGE: i32 = 5;

const OPENERS: [(Deck, [&str; 5]); 5] = [
    (
        Deck::Beer,
        ["beer-01", "beer-02", "beer-03", "beer-04", "beer-06"],
    ),
    (
        Deck::Cider,
        ["cider-01", "cider-02", "cider-03", "cider-05", "cider-04"],
    ),
    (
        Deck::Wine,
        ["wine-01", "wine-02", "wine-03", "wine-06", "wine-04"],
    ),
    (
        Deck::Liquor,
        [
            "liquor-01",
            "liquor-02",
            "liquor-03",
            "liquor-04",
            "liquor-06",
        ],
    ),
    (
        Deck::Soft,
        ["soft-01", "soft-02", "soft-03", "soft-05", "soft-06"],
    ),
];

pub const CATALOG: [CardDef; 43] = [
    // ---- Beer — Attrition, costs 1-2, 8 pulls. Par 2 dmg/pull, no hit > 4.
    CardDef { id: "beer-01", deck: Deck::Beer, kind: CardKind::Atk, cost: 1,
        targets: "one", title: "Nudge", copies: 6, keywords: &[], fx: dmg(2), rfx: None, chfx: None,
        text: "Deal 2 damage. Small, boring, and there is always another one." },
    CardDef { id: "beer-02", deck: Deck::Beer, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Grind", copies: 6, keywords: &["slow"], fx: dmg(4), rfx: None, chfx: None,
        text: "Deal 4 damage. Nothing flashy, the tab just keeps running." },
    CardDef { id: "beer-03", deck: Deck::Beer, kind: CardKind::Buff, cost: 1,
        targets: "self", title: "Second Wind", copies: 6, keywords: &["heal"], fx: heal(2), rfx: None, chfx: None,
        text: "Heal 2. Shake it off, it was only a nudge." },
    CardDef { id: "beer-04", deck: Deck::Beer, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Head of Foam", copies: 6, keywords: &["shield"],
        fx: shield(4, 2), rfx: None, chfx: None,
        text: "Gain a shield that absorbs 4 damage this round and the next 2." },
    CardDef { id: "beer-05", deck: Deck::Beer, kind: CardKind::Atk, cost: 2,
        targets: "all", title: "One For The Table", copies: 5, keywords: &["aoe"],
        fx: dmg(1), rfx: None, chfx: None,
        text: "Deal 1 damage to every player at the table. Yes, including you." },
    CardDef { id: "beer-06", deck: Deck::Beer, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Steady Pour", copies: 5, keywords: &["heal"], fx: heal(4), rfx: None, chfx: None,
        text: "Heal 4. Slow beer, long night." },
    CardDef { id: "beer-07", deck: Deck::Beer, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Tab Runs Long", copies: 4, keywords: &["dot", "slow"],
        fx: dot(1, 5), rfx: None, chfx: None,
        text: "Deal 1 damage a round for 5 rounds. It did not seem like much at the time." },
    CardDef { id: "beer-08", deck: Deck::Beer, kind: CardKind::Reaction, cost: 1,
        targets: "self", title: "Coaster", copies: 2, keywords: &["reaction"], fx: None,
        rfx: Some(ReactionFx::Reduce(3)), chfx: None,
        text: "Reaction: a revealed play deals 3 less damage to you. Slide it over your glass." },
    // ---- Cider — Trickster, costs 1-3, 10 pulls. Par hits + pull drains.
    CardDef { id: "cider-01", deck: Deck::Cider, kind: CardKind::Curse, cost: 1,
        targets: "one", title: "Sticky Pour", copies: 6, keywords: &["dot"], fx: dot(1, 2), rfx: None, chfx: None,
        text: "Deal 1 damage a round for 2 rounds. Something inconvenient, later." },
    CardDef { id: "cider-02", deck: Deck::Cider, kind: CardKind::Util, cost: 1,
        targets: "one", title: "Spilled", copies: 6, keywords: &["drain", "petty"],
        fx: drain(2), rfx: None, chfx: None,
        text: "Drain 2 pulls from a player's fullest vessel. Whoops." },
    CardDef { id: "cider-03", deck: Deck::Cider, kind: CardKind::Util, cost: 2,
        targets: "one", title: "Watered Down", copies: 6, keywords: &["drain"],
        fx: drain(3), rfx: None, chfx: None,
        text: "Drain 3 pulls from a player's fullest vessel. They will taste it eventually." },
    CardDef { id: "cider-04", deck: Deck::Cider, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Windfall", copies: 4,
        keywords: &["burst", "loud", "public", "petty", "showy"], fx: dmg(6), rfx: None, chfx: None,
        text: "Deal 6 damage. The whole orchard at once, and everyone hears it land." },
    CardDef { id: "cider-05", deck: Deck::Cider, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Sour Turn", copies: 6, keywords: &[], fx: dmg(4), rfx: None, chfx: None,
        text: "Deal 4 damage. Sweet up front, then it bites." },
    CardDef { id: "cider-06", deck: Deck::Cider, kind: CardKind::Util, cost: 3,
        targets: "all", title: "Happy Hour Panic", copies: 5, keywords: &["aoe", "drain"],
        fx: drain(1), rfx: None, chfx: None,
        text: "Drain 1 pull from every player at the table, you included. Last orders moved up." },
    CardDef { id: "cider-07", deck: Deck::Cider, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Two Straws", copies: 5, keywords: &["dot"], fx: dot(2, 2), rfx: None, chfx: None,
        text: "Deal 2 damage a round for 2 rounds. Double the trouble, half the dignity." },
    CardDef { id: "cider-08", deck: Deck::Cider, kind: CardKind::Reaction, cost: 2,
        targets: "one", title: "Not So Fast, Friend", copies: 2, keywords: &["reaction"],
        fx: None,
        rfx: Some(ReactionFx::Cancel), chfx: None,
        text: "Reaction: cancel any revealed play, whoever it was aimed at. Keep it where they can see it." },
    // ---- Wine — Control, costs 2-3, 6 pulls. Dots at 2.0-3.0 total/pull.
    CardDef { id: "wine-01", deck: Deck::Wine, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Decant", copies: 6, keywords: &["dot"], fx: dot(2, 2), rfx: None, chfx: None,
        text: "Deal 2 damage a round for 2 rounds. Poured slowly, from a great height, \
               while keeping eye contact the whole time, a patient problem that keeps \
               arriving well after the glass is set down." },
    CardDef { id: "wine-02", deck: Deck::Wine, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Let It Breathe", copies: 6, keywords: &["dot", "slow"],
        fx: dot(1, 4), rfx: None, chfx: None,
        text: "Deal 1 damage a round for 4 rounds. It only improves with time." },
    CardDef { id: "wine-03", deck: Deck::Wine, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Tannin Bite", copies: 6, keywords: &[], fx: dmg(4), rfx: None, chfx: None,
        text: "Deal 4 damage. Dry, sharp, structured." },
    CardDef { id: "wine-04", deck: Deck::Wine, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Corked", copies: 5, keywords: &["burst"], fx: dmg(6), rfx: None, chfx: None,
        text: "Deal 6 damage. Control, delivered as damage." },
    CardDef { id: "wine-05", deck: Deck::Wine, kind: CardKind::Util, cost: 3,
        targets: "all", title: "House Rules Amendment", copies: 5,
        keywords: &["aoe", "drain", "public"], fx: drain(1), rfx: None, chfx: None,
        text: "Drain 1 pull from every player at the table, you included. Motion carried." },
    CardDef { id: "wine-06", deck: Deck::Wine, kind: CardKind::Curse, cost: 3,
        targets: "one", title: "Cellar Chill", copies: 6, keywords: &["dot"], fx: dot(2, 3), rfx: None, chfx: None,
        text: "Deal 2 damage a round for 3 rounds. Best served cold and repeatedly." },
    CardDef { id: "wine-07", deck: Deck::Wine, kind: CardKind::Curse, cost: 3,
        targets: "one", title: "The Long Decant of Winter", copies: 4,
        keywords: &["dot", "slow", "showy"], fx: dot(3, 3), rfx: None, chfx: None,
        text: "Deal 3 damage a round for 3 rounds. A vintage grudge, opened at last." },
    CardDef { id: "wine-08", deck: Deck::Wine, kind: CardKind::Reaction, cost: 2,
        targets: "one", title: "Send It Back", copies: 2, keywords: &["reaction"], fx: None,
        rfx: Some(ReactionFx::Reflect), chfx: None,
        text: "Reaction: a revealed play aimed at one player resolves against its owner instead. Summon the sommelier." },
    // ---- Liquor — Burst, costs 2-3, 4 pulls. Premium 2.5-2.67 dmg/pull.
    CardDef { id: "liquor-01", deck: Deck::Liquor, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Shot Called", copies: 6, keywords: &["burst"], fx: dmg(5), rfx: None, chfx: None,
        text: "Deal 5 damage. Loud and immediate." },
    CardDef { id: "liquor-02", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Double", copies: 5, keywords: &["burst", "loud"],
        fx: dmg(7), rfx: None, chfx: None,
        text: "Deal 7 damage. Louder and more immediate." },
    CardDef { id: "liquor-03", deck: Deck::Liquor, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Hangover", copies: 6, keywords: &["dot"], fx: dot(2, 2), rfx: None, chfx: None,
        text: "Deal 2 damage a round for 2 rounds. Payable next round, with interest." },
    CardDef { id: "liquor-04", deck: Deck::Liquor, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Chaser", copies: 6, keywords: &["heal"], fx: heal(4), rfx: None, chfx: None,
        text: "Heal 4. Something soft to land on." },
    CardDef { id: "liquor-05", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "all", title: "Neat, No Ice, No Mercy", copies: 5,
        keywords: &["aoe", "loud"], fx: dmg(2), rfx: None, chfx: None,
        text: "Deal 2 damage to every player at the table, you included. A round of shots is still a round." },
    CardDef { id: "liquor-06", deck: Deck::Liquor, kind: CardKind::Buff, cost: 3,
        targets: "self", title: "Dutch Courage", copies: 6, keywords: &["shield"],
        fx: shield(7, 2), rfx: None, chfx: None,
        text: "Gain a shield that absorbs 7 damage this round and the next 2. Liquid confidence, briefly real." },
    CardDef { id: "liquor-07", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Last Call", copies: 4,
        keywords: &["burst", "loud", "showy", "public"], fx: dmg(8), rfx: None, chfx: None,
        text: "Deal 8 damage. The biggest hit in the game, named after the night's last mistake." },
    CardDef { id: "liquor-08", deck: Deck::Liquor, kind: CardKind::Reaction, cost: 2,
        targets: "self", title: "Spit It Out", copies: 2, keywords: &["reaction"], fx: None,
        rfx: Some(ReactionFx::Cancel), chfx: None,
        text: "Reaction: cancel a revealed play aimed at you. Undignified but effective." },
    // ---- Soft — Support, costs 1-2, 6 pulls. Shields at premium, one chip.
    CardDef { id: "soft-01", deck: Deck::Soft, kind: CardKind::Buff, cost: 1,
        targets: "one", title: "Water Round", copies: 6, keywords: &["heal"], fx: heal(2), rfx: None, chfx: None,
        text: "Heal 2 on any player. Someone feels better." },
    CardDef { id: "soft-02", deck: Deck::Soft, kind: CardKind::Buff, cost: 1,
        targets: "one", title: "Designated", copies: 6, keywords: &["shield"],
        fx: shield(3, 2), rfx: None, chfx: None,
        text: "Shield any player for 3 damage this round and the next 2. You take it for them." },
    CardDef { id: "soft-03", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "all", title: "Snack Table", copies: 6, keywords: &["aoe", "heal"],
        fx: heal(1), rfx: None, chfx: None,
        text: "Heal 1 on every player at the table, you included. Crisps solve most things." },
    CardDef { id: "soft-04", deck: Deck::Soft, kind: CardKind::Reaction, cost: 1,
        targets: "self", title: "The Long Sober Look Across The Table", copies: 2,
        keywords: &["reaction"], fx: None,
        rfx: Some(ReactionFx::Reduce(4)), chfx: None,
        text: "Reaction: a revealed play deals 4 less damage to you. You know what you did." },
    CardDef { id: "soft-05", deck: Deck::Soft, kind: CardKind::Util, cost: 2,
        targets: "one", title: "Cut Them Off", copies: 6, keywords: &["drain", "petty"],
        fx: drain(3), rfx: None, chfx: None,
        text: "Drain 3 pulls from a player's fullest vessel. It is for their own good." },
    CardDef { id: "soft-06", deck: Deck::Soft, kind: CardKind::Atk, cost: 1,
        targets: "one", title: "Splash of Cold Water", copies: 5, keywords: &[],
        fx: dmg(2), rfx: None, chfx: None,
        text: "Deal 2 damage. Rude, refreshing, effective." },
    CardDef { id: "soft-07", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "one", title: "Glass Wall", copies: 5, keywords: &["shield"],
        fx: shield(5, 2), rfx: None, chfx: None,
        text: "Shield any player for 5 damage this round and the next 2. Politely impenetrable." },
    CardDef { id: "soft-08", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "all", title: "Mother Hen", copies: 4,
        keywords: &["aoe", "shield", "quiet"], fx: shield(2, 2), rfx: None, chfx: None,
        text: "Shield every player for 2 damage this round and the next 2, you included. Everyone gets a coaster." },
    // ---- Challenge prototype (challenge-cards container, Pack 1). Ships at
    // copies: 0 — catalog-present so the engine and test mode can exercise
    // the phase, shoe-absent until Pack 3 balances the real wave in.
    CardDef { id: "liquor-09", deck: Deck::Liquor, kind: CardKind::Challenge, cost: 2,
        targets: "right", title: "Bar Court", copies: 0,
        keywords: &["challenge", "loud", "public"], fx: None, rfx: None,
        chfx: Some(ChallengeDef { contest: Contest::Duel, penalty: Penalty::Damage(4) }),
        text: "Challenge: state your case against the player on your right — the table votes a winner, and the loser takes 4 damage. Court is now in session." },
    CardDef { id: "soft-09", deck: Deck::Soft, kind: CardKind::Challenge, cost: 1,
        targets: "self", title: "Floor Show", copies: 0,
        keywords: &["challenge", "showy"], fx: None, rfx: None,
        chfx: Some(ChallengeDef { contest: Contest::Solo, penalty: Penalty::Drink(2) }),
        text: "Challenge: put on a thirty-second performance of the table's choosing. If they are not impressed, drink 2. No refunds." },
    CardDef { id: "beer-09", deck: Deck::Beer, kind: CardKind::Challenge, cost: 1,
        targets: "right", title: "House Etiquette", copies: 0,
        keywords: &["challenge", "petty"], fx: None, rfx: None,
        chfx: Some(ChallengeDef { contest: Contest::Duel, penalty: Penalty::Rule("Speak only in questions.", 2) }),
        text: "Challenge: out-argue the player on your right — the table votes, and the loser speaks only in questions for the next 2 rounds. House rules are house rules." },
];

fn to_card(def: &CardDef) -> Card {
    Card {
        id: def.id.to_string(),
        deck: def.deck,
        kind: def.kind,
        cost: def.cost,
        targets: def.targets.to_string(),
        title: def.title.to_string(),
        text: def.text.to_string(),
        keywords: def.keywords.iter().map(|s| s.to_string()).collect(),
        duration: def.fx.and_then(|f| {
            if f.rounds >= 1 {
                Some(format!("{} ROUNDS", f.rounds))
            } else {
                None
            }
        }),
    }
}

/// Every `Card` for `deck`, in catalog order — the eight shoe-carrying
/// cards plus any `copies: 0` challenge prototypes.
pub fn deck_cards(deck: Deck) -> Vec<Card> {
    CATALOG
        .iter()
        .filter(|def| def.deck == deck)
        .map(to_card)
        .collect()
}

pub fn card_by_id(id: &str) -> Option<Card> {
    CATALOG.iter().find(|def| def.id == id).map(to_card)
}

/// A card's rules by id — `None` for reactions AND for an unknown id
/// (fail-soft: a stale/typo'd id resolves inert rather than panicking).
pub fn card_fx(id: &str) -> Option<FxDef> {
    CATALOG.iter().find(|def| def.id == id).and_then(|d| d.fx)
}

/// A reaction's rules by id — `None` for non-reactions AND for an unknown id
/// (fail-soft, mirrors `card_fx`).
pub fn card_rfx(id: &str) -> Option<ReactionFx> {
    CATALOG.iter().find(|def| def.id == id).and_then(|d| d.rfx)
}

/// A challenge's rules by id — `None` for non-challenges AND for an unknown
/// id (fail-soft, mirrors `card_fx`/`card_rfx`).
pub fn card_chfx(id: &str) -> Option<ChallengeDef> {
    CATALOG.iter().find(|def| def.id == id).and_then(|d| d.chfx)
}

/// The curated 5-card opening hand for `deck` (F6), in `OPENERS` order.
pub fn opening_hand(deck: Deck) -> Vec<Card> {
    let (_, ids) = OPENERS
        .iter()
        .find(|(d, _)| *d == deck)
        .expect("every Deck has an OPENERS row");
    ids.iter()
        .map(|id| card_by_id(id).unwrap_or_else(|| panic!("opener id {id} missing from CATALOG")))
        .collect()
}

/// Sum of `copies` for `deck` — always `LC_DECK_SIZE` (test-pinned).
pub fn deck_copies(deck: Deck) -> u16 {
    CATALOG
        .iter()
        .filter(|def| def.deck == deck)
        .map(|def| def.copies as u16)
        .sum()
}

/// The copy-expanded 40-card shoe for `deck`, catalog order, each card
/// repeated `copies` times. Plan E samples this WITH replacement — nothing
/// here removes a card once drawn (F11); the caller owns dealing without
/// duplicates within a single draw.
pub fn shoe(deck: Deck) -> Vec<Card> {
    CATALOG
        .iter()
        .filter(|def| def.deck == deck)
        .flat_map(|def| std::iter::repeat_n(to_card(def), def.copies as usize))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::LC_DECK_SIZE;
    use std::collections::HashSet;

    #[test]
    fn test_catalog_shape_and_copy_sums() {
        assert_eq!(CATALOG.len(), 43);
        for deck in Deck::ALL {
            // Eight shoe-carrying cards per deck; copies: 0 prototypes sit
            // outside the shoe and outside this count.
            let in_shoe = CATALOG
                .iter()
                .filter(|d| d.deck == deck && d.copies > 0)
                .count();
            assert_eq!(in_shoe, 8, "{deck:?}");
            assert_eq!(deck_copies(deck), LC_DECK_SIZE, "{deck:?}");
            assert_eq!(shoe(deck).len(), LC_DECK_SIZE as usize);
        }
        let ids: HashSet<&str> = CATALOG.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), CATALOG.len());
        for def in CATALOG.iter() {
            assert!(def.id.starts_with(def.deck.slug()), "{}", def.id);
            // copies: 0 is reserved for Challenge prototypes until Pack 3
            // balances them into the shoe.
            if def.kind == CardKind::Challenge {
                assert!((0..=6).contains(&def.copies), "{}", def.id);
            } else {
                assert!((1..=6).contains(&def.copies), "{}", def.id);
            }
        }
    }

    #[test]
    fn test_catalog_costs_match_deck_spread() {
        let spreads: [(Deck, std::ops::RangeInclusive<u8>); 5] = [
            (Deck::Beer, 1..=2),
            (Deck::Cider, 1..=3),
            (Deck::Wine, 2..=3),
            (Deck::Liquor, 2..=3),
            (Deck::Soft, 1..=2),
        ];
        for (deck, range) in spreads {
            for def in CATALOG.iter().filter(|d| d.deck == deck) {
                assert!(
                    range.contains(&def.cost),
                    "{} cost {} outside {:?} spread",
                    def.id,
                    def.cost,
                    range
                );
            }
        }
    }

    #[test]
    fn test_fx_matches_kind() {
        for def in CATALOG.iter() {
            match (def.kind, def.fx) {
                (CardKind::Atk, Some(f)) => assert_eq!(f.op, EffectOp::Damage, "{}", def.id),
                (CardKind::Buff, Some(f)) => assert!(
                    matches!(f.op, EffectOp::Heal | EffectOp::Shield),
                    "{}",
                    def.id
                ),
                (CardKind::Curse, Some(f)) => assert_eq!(f.op, EffectOp::Dot, "{}", def.id),
                (CardKind::Util, Some(f)) => {
                    assert_eq!(f.op, EffectOp::PullDrain, "{}", def.id)
                }
                (CardKind::Reaction, None) => {}
                (CardKind::Challenge, None) => {}
                (kind, fx) => panic!("{}: {kind:?} with fx {fx:?}", def.id),
            }
        }
        for def in CATALOG.iter() {
            assert_eq!(
                def.rfx.is_some(),
                def.kind == CardKind::Reaction,
                "{}",
                def.id
            );
            assert_eq!(
                def.chfx.is_some(),
                def.kind == CardKind::Challenge,
                "{}",
                def.id
            );
        }
    }

    #[test]
    fn test_reaction_fx_table() {
        // decision I6 — arming F5's inert cards
        assert_eq!(card_rfx("beer-08"), Some(ReactionFx::Reduce(3)));
        assert_eq!(card_rfx("cider-08"), Some(ReactionFx::Cancel));
        assert_eq!(card_rfx("wine-08"), Some(ReactionFx::Reflect));
        assert_eq!(card_rfx("liquor-08"), Some(ReactionFx::Cancel));
        assert_eq!(card_rfx("soft-04"), Some(ReactionFx::Reduce(4)));
        assert_eq!(card_rfx("beer-01"), None);
        assert_eq!(card_rfx("nope"), None);
        // The inert-era text is gone from every reaction:
        for def in CATALOG.iter().filter(|d| d.kind == CardKind::Reaction) {
            assert!(!def.text.contains("inert"), "{}", def.id);
            assert!(def.text.starts_with("Reaction:"), "{}", def.id);
        }
    }

    #[test]
    fn test_challenge_fx_table() {
        // Challenge-cards container, Pack 1 — the chfx column resolves by
        // id, fail-soft on unknowns, mirroring fx/rfx.
        assert_eq!(
            card_chfx("liquor-09"),
            Some(ChallengeDef {
                contest: Contest::Duel,
                penalty: Penalty::Damage(4),
            })
        );
        assert_eq!(
            card_chfx("soft-09"),
            Some(ChallengeDef {
                contest: Contest::Solo,
                penalty: Penalty::Drink(2),
            })
        );
        assert_eq!(card_chfx("beer-01"), None);
        assert_eq!(card_chfx("wine-08"), None);
        assert_eq!(card_chfx("nope"), None);
        for def in CATALOG.iter().filter(|d| d.kind == CardKind::Challenge) {
            assert!(def.text.starts_with("Challenge:"), "{}", def.id);
        }
    }

    #[test]
    fn test_fx_rounds_match_op() {
        for f in CATALOG.iter().filter_map(|d| d.fx) {
            assert!(f.magnitude >= 1);
            match f.op {
                EffectOp::Damage | EffectOp::Heal | EffectOp::PullDrain => {
                    assert_eq!(f.rounds, 0)
                }
                EffectOp::Shield | EffectOp::Dot => assert!(f.rounds >= 1),
            }
        }
    }

    #[test]
    fn test_targets_are_a_known_class() {
        for def in CATALOG.iter() {
            assert!(
                ["self", "one", "all", "right"].contains(&def.targets),
                "{}",
                def.id
            );
            // "right" (auto-target: the seat to the instigator's right) is
            // a Duel-challenge-only class; nothing else may claim it.
            assert_eq!(
                def.targets == "right",
                def.chfx.is_some_and(|c| c.contest == Contest::Duel),
                "{}",
                def.id
            );
        }
    }

    #[test]
    fn test_opening_hands() {
        for deck in Deck::ALL {
            let hand = opening_hand(deck);
            assert_eq!(hand.len(), 5, "{deck:?}");
            let first = format!("{}-01", deck.slug());
            assert!(hand.iter().any(|c| c.id == first), "{deck:?} lacks {first}");
            for c in &hand {
                assert_eq!(c.deck, deck);
                assert_ne!(c.kind, CardKind::Reaction, "{} in {deck:?} opener", c.id);
            }
        }
    }

    #[test]
    fn test_duration_is_derived_from_fx() {
        assert_eq!(
            card_by_id("wine-02").unwrap().duration.as_deref(),
            Some("4 ROUNDS")
        );
        assert_eq!(
            card_by_id("beer-04").unwrap().duration.as_deref(),
            Some("2 ROUNDS")
        );
        assert_eq!(card_by_id("beer-01").unwrap().duration, None);
        assert_eq!(card_by_id("soft-04").unwrap().duration, None);
    }

    #[test]
    fn test_catalog_titles_use_char_counts_not_bytes() {
        for def in CATALOG.iter() {
            assert_eq!(
                def.title.len(),
                def.title.chars().count(),
                "{} title is not pure ASCII",
                def.id
            );
        }
    }

    #[test]
    fn test_beer_has_no_bomb() {
        // DDv2 §3.1: "Attrition. No bomb in the list."
        for def in CATALOG.iter().filter(|d| d.deck == Deck::Beer) {
            if let Some(f) = def.fx {
                if f.op == EffectOp::Damage && def.targets == "one" {
                    assert!(f.magnitude <= 4, "{}", def.id);
                }
            }
        }
    }

    #[test]
    fn test_keywords_come_from_the_two_vocabularies() {
        // F7
        for def in CATALOG.iter() {
            for kw in def.keywords {
                assert!(
                    MECHANICAL_KW.contains(kw) || TONE_KW.contains(kw),
                    "{}: unknown keyword {kw}",
                    def.id
                );
            }
            assert!(def.keywords.len() <= 5, "{}", def.id);
        }
    }

    #[test]
    fn test_mechanical_keywords_are_bidirectional() {
        // F7 — chips cannot lie
        for def in CATALOG.iter() {
            let has = |kw: &str| def.keywords.contains(&kw);
            assert_eq!(has("aoe"), def.targets == "all", "{}", def.id);
            assert_eq!(
                has("reaction"),
                def.kind == CardKind::Reaction,
                "{}",
                def.id
            );
            assert_eq!(
                has("challenge"),
                def.kind == CardKind::Challenge,
                "{}",
                def.id
            );
            let op = |o: EffectOp| def.fx.is_some_and(|f| f.op == o);
            assert_eq!(has("dot"), op(EffectOp::Dot), "{}", def.id);
            assert_eq!(has("shield"), op(EffectOp::Shield), "{}", def.id);
            assert_eq!(has("heal"), op(EffectOp::Heal), "{}", def.id);
            assert_eq!(has("drain"), op(EffectOp::PullDrain), "{}", def.id);
            let bursty = def.targets == "one"
                && def.fx.is_some_and(|f| {
                    f.op == EffectOp::Damage && f.magnitude >= BURST_KW_MIN_DAMAGE
                });
            assert_eq!(has("burst"), bursty, "{}", def.id);
        }
    }

    #[test]
    fn test_catalog_covers_every_title_band() {
        // §7.5 ramp, forever
        let len = |d: &CardDef| d.title.chars().count();
        assert!(CATALOG.iter().any(|d| len(d) <= 14));
        assert!(CATALOG.iter().any(|d| (15..=24).contains(&len(d))));
        assert!(CATALOG.iter().any(|d| len(d) > 24));
    }

    #[test]
    fn test_catalog_has_an_overflowing_body() {
        // >108 chars = BODY_CLAMP_CHARS: the 3-line clamp and data-expandable
        // must stay proven against rendered content (spec §9).
        let overflowing: Vec<&str> = CATALOG
            .iter()
            .filter(|d| d.text.chars().count() > 108)
            .map(|d| d.id)
            .collect();
        assert!(overflowing.contains(&"wine-01"), "got {overflowing:?}");
    }

    #[test]
    fn test_catalog_keeps_the_keyword_extremes() {
        assert!(CATALOG.iter().any(|d| d.keywords.is_empty()));
        // > 3 keywords is what makes card_face emit the +n fold chip.
        assert!(CATALOG.iter().any(|d| d.keywords.len() > 3));
    }
}
