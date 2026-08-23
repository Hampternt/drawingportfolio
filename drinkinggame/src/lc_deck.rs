//! The cards on the table — real draw piles and real discard piles.
//!
//! What this replaces: `LastCallState::deck_counts` used to be a
//! `Vec<(Deck, u16)>` — a *number* per deck, not a deck. Card identity was
//! decided in `lc_routes` by sampling `lc_cards::shoe(deck)` **with
//! replacement** on every request, and the count was decremented
//! independently. Three things followed, none of them intended:
//!
//! * A "40-card shoe" could deal forty copies of the same card. Copy counts
//!   set a card's *probability*, never its scarcity, so the catalog's
//!   `copies: 1` rares were as common as its `copies: 8` staples over any
//!   short window.
//! * The count and the cards had no relationship. Drawing five debited five
//!   from a number; which five cards you got was a separate dice roll.
//! * The reshuffle "reclaimed" discards into that number (`entry.1 =
//!   reclaimed`) — but since sampling ignored the pile entirely, a reshuffle
//!   changed how many draws remained and nothing else.
//!
//! Now a `Shoe` owns two `Vec<Card>`. Drawing pops the back of `draw_pile`;
//! discards land in `discard_pile`; an exhausted pile reshuffles its
//! discards back with the state's own seeded RNG. Cards are conserved — the
//! `test_cards_are_conserved_*` tests pin that the total across piles and
//! hands never changes — so "how many Beer cards are left" is now a fact
//! about a pile rather than a counter that happened to be decremented.
//!
//! `counts()` projects a `Vec<(Deck, u16)>` over the OPEN shoes — the
//! engine's own view of what is on the table. It is deliberately NOT what
//! `PublicView::deck_counts` carries: that one is padded to all five
//! `Deck::ALL` entries, because `lc_render` zips it positionally against
//! `discard_counts` and reads `deck_counts.first()` as the pile's deck. Using
//! the open-only shape there paired each deck's row with another deck's
//! discard count. See `public_view()`'s comment for the full story.
//!
//! Confidentiality: a `Shoe`'s ORDER is the secret, not its size. Counts are
//! public (DDv2 beat 6 — discards are open information), and
//! `LastCallState::public_view` projects `counts()`/`discard_counts()` and
//! never the piles themselves. Nothing here is a secrecy boundary on its
//! own; see the `§3.4.1` comments in `last_call.rs`.

use crate::last_call::{Card, Deck};
use crate::lc_rng::LcRng;
use serde::{Deserialize, Serialize};

/// One deck's two piles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Shoe {
    pub deck: Deck,
    /// Face-down, shuffled. **Drawn from the BACK** — `pop()` is O(1) where
    /// `remove(0)` is O(n), and which end is "the top" is arbitrary for a
    /// shuffled pile.
    pub draw_pile: Vec<Card>,
    /// Face-up, in the order cards arrived. Recycled by `reshuffle`.
    pub discard_pile: Vec<Card>,
}

impl Shoe {
    /// A fresh, shuffled shoe: the copy-expanded catalog for `deck`
    /// (`lc_cards::shoe`), permuted by `rng`. Copies now mean scarcity —
    /// a `copies: 2` card is genuinely two cards in this pile.
    pub fn opened(deck: Deck, rng: &mut LcRng) -> Self {
        let mut draw_pile = crate::lc_cards::shoe(deck);
        rng.shuffle(&mut draw_pile);
        Shoe {
            deck,
            draw_pile,
            discard_pile: Vec::new(),
        }
    }

    /// Cards left to draw before a reshuffle is needed.
    pub fn remaining(&self) -> usize {
        self.draw_pile.len()
    }

    /// Cards sitting in the discard pile.
    pub fn discarded(&self) -> usize {
        self.discard_pile.len()
    }

    /// Everything this shoe still owns — the figure the conservation tests
    /// watch, and the true ceiling on what it can ever deal again.
    pub fn total(&self) -> usize {
        self.draw_pile.len() + self.discard_pile.len()
    }

    /// Shuffle the discard pile back under the draw pile.
    ///
    /// Under, not over: the remaining draw-pile cards stay on top, so a
    /// reshuffle triggered by a *partially* drained pile (which `draw` never
    /// does today, but `LastCallState`'s rollover reshuffle can) doesn't
    /// re-deal a just-discarded card ahead of one that has been waiting.
    /// Returns how many cards were reclaimed — `0` means nothing happened,
    /// which is what the caller uses to decide whether the event is worth a
    /// log line.
    pub fn reshuffle(&mut self, rng: &mut LcRng) -> usize {
        if self.discard_pile.is_empty() {
            return 0;
        }
        let mut reclaimed = std::mem::take(&mut self.discard_pile);
        // copies:0 prototypes (challenge cards outside the copy-weighted
        // shoe) are kept OUT of the recycle, matching the pre-existing
        // `card_in_shoe` rule: reclaiming one would put a card into the
        // draw pile that the shoe was never built to contain.
        let (recyclable, keep): (Vec<Card>, Vec<Card>) = reclaimed
            .drain(..)
            .partition(|c| crate::lc_cards::card_in_shoe(&c.id));
        self.discard_pile = keep;
        let n = recyclable.len();
        if n == 0 {
            return 0;
        }
        let mut merged = recyclable;
        rng.shuffle(&mut merged);
        // `merged` goes to the BOTTOM. The draw pile is drawn from the back,
        // so the bottom is the front of the vec.
        merged.extend(std::mem::take(&mut self.draw_pile));
        self.draw_pile = merged;
        n
    }

    /// Draw up to `n` cards, reshuffling once if the pile runs dry mid-draw.
    ///
    /// Returns fewer than `n` only when the shoe is genuinely exhausted —
    /// draw pile empty AND nothing recyclable in the discard pile. Callers
    /// must treat a short result as legal (`DRAW_PER_VESSEL.min(available)`
    /// is the existing rule, D7); this never pads the result or loops.
    pub fn draw(&mut self, n: usize, rng: &mut LcRng) -> Vec<Card> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if self.draw_pile.is_empty() && self.reshuffle(rng) == 0 {
                break; // genuinely out of cards — not an error, just short
            }
            match self.draw_pile.pop() {
                Some(c) => out.push(c),
                None => break,
            }
        }
        out
    }

    /// Put cards face-up on the discard pile.
    pub fn discard(&mut self, cards: impl IntoIterator<Item = Card>) {
        self.discard_pile.extend(cards);
    }
}

/// Every open shoe on the table.
///
/// A deck is *opened* the first time a player picks it as a vessel — an
/// unopened deck has no pile at all, which is why this is a `Vec<Shoe>` and
/// not a fixed array over `Deck::ALL`. Kept in `Deck::ALL` order by
/// `open()` so `counts()` projects deterministically (the old `deck_counts`
/// was insertion-ordered, and `lc_render`'s pile picker reads entry 0).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct LcTable {
    pub shoes: Vec<Shoe>,
}

impl LcTable {
    pub fn new() -> Self {
        LcTable::default()
    }

    pub fn is_open(&self, deck: Deck) -> bool {
        self.shoes.iter().any(|s| s.deck == deck)
    }

    pub fn shoe(&self, deck: Deck) -> Option<&Shoe> {
        self.shoes.iter().find(|s| s.deck == deck)
    }

    pub fn shoe_mut(&mut self, deck: Deck) -> Option<&mut Shoe> {
        self.shoes.iter_mut().find(|s| s.deck == deck)
    }

    /// Open `deck` if it isn't already, keeping `Deck::ALL` order.
    /// Idempotent — re-opening an in-play deck would reshuffle drawn cards
    /// back into circulation and duplicate every card already in a hand.
    pub fn open(&mut self, deck: Deck, rng: &mut LcRng) {
        if self.is_open(deck) {
            return;
        }
        let shoe = Shoe::opened(deck, rng);
        let rank = |d: Deck| Deck::ALL.iter().position(|&x| x == d).unwrap_or(usize::MAX);
        let at = self
            .shoes
            .iter()
            .position(|s| rank(s.deck) > rank(deck))
            .unwrap_or(self.shoes.len());
        self.shoes.insert(at, shoe);
    }

    /// Draw up to `n` from `deck`. An unopened deck yields nothing.
    pub fn draw(&mut self, deck: Deck, n: usize, rng: &mut LcRng) -> Vec<Card> {
        match self.shoe_mut(deck) {
            Some(s) => s.draw(n, rng),
            None => Vec::new(),
        }
    }

    /// Discard into the card's OWN deck's pile — never the pile of whoever
    /// discarded it. A Beer card played by a Wine drinker goes back to Beer.
    /// A card whose deck was somehow never opened is dropped rather than
    /// silently opening a shoe mid-game (which would deal 40 new cards into
    /// existence); in practice a card cannot exist without its deck being
    /// open, since the only sources are `open` + `draw`.
    pub fn discard(&mut self, cards: impl IntoIterator<Item = Card>) {
        for card in cards {
            if let Some(shoe) = self.shoe_mut(card.deck) {
                shoe.discard_pile.push(card);
            }
        }
    }

    /// Force a reshuffle of one deck. Returns cards reclaimed.
    pub fn reshuffle(&mut self, deck: Deck, rng: &mut LcRng) -> usize {
        match self.shoe_mut(deck) {
            Some(s) => s.reshuffle(rng),
            None => 0,
        }
    }

    /// Cards left to draw in `deck` — `0` for an unopened deck.
    pub fn remaining(&self, deck: Deck) -> usize {
        self.shoe(deck).map(|s| s.remaining()).unwrap_or(0)
    }

    /// The legacy `deck_counts` projection: `(deck, cards left)` per open
    /// shoe, `Deck::ALL` order. `PublicView::deck_counts` carries this
    /// verbatim, so `lc_render` needs no change.
    pub fn counts(&self) -> Vec<(Deck, u16)> {
        self.shoes
            .iter()
            .map(|s| (s.deck, s.remaining().min(u16::MAX as usize) as u16))
            .collect()
    }

    /// Per-deck discard counts, `Deck::ALL` order over OPEN shoes — feeds
    /// `PublicView::discard_counts`.
    pub fn discard_counts(&self) -> Vec<(Deck, usize)> {
        self.shoes.iter().map(|s| (s.deck, s.discarded())).collect()
    }

    /// Every discarded card across all shoes — the flat view the old
    /// `LastCallState::discards` field had.
    pub fn all_discards(&self) -> Vec<&Card> {
        self.shoes
            .iter()
            .flat_map(|s| s.discard_pile.iter())
            .collect()
    }

    /// Total discarded across the table — `PublicView::discard_count`.
    pub fn discard_total(&self) -> usize {
        self.shoes.iter().map(|s| s.discarded()).sum()
    }

    /// Every card this table owns, across both piles of every shoe. The
    /// conservation tests compare this plus all hands against the opening
    /// total.
    pub fn total(&self) -> usize {
        self.shoes.iter().map(|s| s.total()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::LC_DECK_SIZE;

    fn rng() -> LcRng {
        LcRng::seeded(4242)
    }

    #[test]
    fn test_opened_shoe_is_full_and_shuffled() {
        let mut r = rng();
        let shoe = Shoe::opened(Deck::Beer, &mut r);
        assert_eq!(shoe.remaining(), LC_DECK_SIZE as usize);
        assert_eq!(shoe.discarded(), 0);
        // Shuffled, not catalog order.
        let catalog = crate::lc_cards::shoe(Deck::Beer);
        let ids: Vec<&str> = shoe.draw_pile.iter().map(|c| c.id.as_str()).collect();
        let cat_ids: Vec<&str> = catalog.iter().map(|c| c.id.as_str()).collect();
        assert_ne!(ids, cat_ids);
        // ...but the same multiset — copies are preserved exactly.
        let mut a = ids.clone();
        let mut b = cat_ids.clone();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
    }

    #[test]
    fn test_draw_is_without_replacement() {
        // The headline fix. The old route sampled with replacement, so a
        // 40-card shoe could deal 40 identical cards. Drawing the whole
        // pile must now reproduce the catalog multiset exactly.
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Wine, &mut r);
        let drawn = shoe.draw(LC_DECK_SIZE as usize, &mut r);
        assert_eq!(drawn.len(), LC_DECK_SIZE as usize);
        assert_eq!(shoe.remaining(), 0);

        let catalog = crate::lc_cards::shoe(Deck::Wine);
        let mut got: Vec<&str> = drawn.iter().map(|c| c.id.as_str()).collect();
        let mut want: Vec<&str> = catalog.iter().map(|c| c.id.as_str()).collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn test_copies_now_mean_scarcity() {
        // A card with `copies: n` appears exactly n times in the pile — the
        // property with-replacement sampling could not express at all.
        for deck in Deck::ALL {
            let mut r = rng();
            let shoe = Shoe::opened(deck, &mut r);
            for def in crate::lc_cards::CATALOG.iter().filter(|d| d.deck == deck) {
                let n = shoe.draw_pile.iter().filter(|c| c.id == def.id).count();
                assert_eq!(n, def.copies as usize, "{} in {deck:?}", def.id);
            }
        }
    }

    #[test]
    fn test_draw_past_the_end_is_short_not_looping() {
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Soft, &mut r);
        let all = shoe.draw(LC_DECK_SIZE as usize, &mut r);
        assert_eq!(all.len(), LC_DECK_SIZE as usize);
        // Empty draw pile, empty discard pile: a short (empty) result, and
        // crucially it terminates.
        assert!(shoe.draw(5, &mut r).is_empty());
    }

    #[test]
    fn test_draw_auto_reshuffles_when_dry() {
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Cider, &mut r);
        let drawn = shoe.draw(LC_DECK_SIZE as usize, &mut r);
        shoe.discard(drawn);
        assert_eq!(shoe.remaining(), 0);
        assert_eq!(shoe.discarded(), LC_DECK_SIZE as usize);

        let again = shoe.draw(3, &mut r);
        assert_eq!(again.len(), 3, "an exhausted pile should recycle");
        assert_eq!(shoe.remaining(), LC_DECK_SIZE as usize - 3);
        assert_eq!(shoe.discarded(), 0);
    }

    #[test]
    fn test_reshuffle_puts_discards_underneath() {
        // A partially-drawn pile keeps its waiting cards on top, so a
        // just-discarded card can't jump the queue.
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Beer, &mut r);
        let kept: Vec<String> = shoe
            .draw_pile
            .iter()
            .rev()
            .take(3)
            .map(|c| c.id.clone())
            .collect();
        let dropped = shoe
            .draw_pile
            .drain(..LC_DECK_SIZE as usize - 3)
            .collect::<Vec<_>>();
        shoe.discard(dropped);
        assert_eq!(shoe.remaining(), 3);

        shoe.reshuffle(&mut r);
        let next3: Vec<String> = shoe
            .draw_pile
            .iter()
            .rev()
            .take(3)
            .map(|c| c.id.clone())
            .collect();
        assert_eq!(next3, kept, "the top of the pile must survive a reshuffle");
    }

    #[test]
    fn test_reshuffle_of_an_empty_discard_is_a_noop() {
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Wine, &mut r);
        let before = shoe.draw_pile.clone();
        assert_eq!(shoe.reshuffle(&mut r), 0);
        assert_eq!(shoe.draw_pile, before);
    }

    #[test]
    fn test_cards_are_conserved_across_draw_and_discard() {
        let mut r = rng();
        let mut shoe = Shoe::opened(Deck::Liquor, &mut r);
        let start = shoe.total();
        let mut hand = shoe.draw(7, &mut r);
        assert_eq!(shoe.total() + hand.len(), start);
        shoe.discard(hand.drain(..3));
        assert_eq!(shoe.total() + hand.len(), start);
        shoe.reshuffle(&mut r);
        assert_eq!(shoe.total() + hand.len(), start);
    }

    #[test]
    fn test_table_open_is_idempotent_and_ordered() {
        let mut r = rng();
        let mut t = LcTable::new();
        // Open out of order; `counts()` must still come back in Deck::ALL
        // order, which `lc_render`'s pile picker depends on.
        t.open(Deck::Wine, &mut r);
        t.open(Deck::Beer, &mut r);
        t.open(Deck::Soft, &mut r);
        let order: Vec<Deck> = t.counts().iter().map(|&(d, _)| d).collect();
        assert_eq!(order, vec![Deck::Beer, Deck::Wine, Deck::Soft]);

        // Re-opening must not deal 40 fresh cards into a live deck.
        let before = t.total();
        t.open(Deck::Beer, &mut r);
        assert_eq!(t.total(), before);
        assert_eq!(t.shoes.len(), 3);
    }

    #[test]
    fn test_unopened_decks_are_inert() {
        let mut r = rng();
        let mut t = LcTable::new();
        assert!(!t.is_open(Deck::Beer));
        assert_eq!(t.remaining(Deck::Beer), 0);
        assert!(t.draw(Deck::Beer, 5, &mut r).is_empty());
        assert_eq!(t.reshuffle(Deck::Beer, &mut r), 0);
        assert!(t.counts().is_empty());
    }

    #[test]
    fn test_discard_routes_by_the_cards_own_deck() {
        let mut r = rng();
        let mut t = LcTable::new();
        t.open(Deck::Beer, &mut r);
        t.open(Deck::Wine, &mut r);
        let beer = t.draw(Deck::Beer, 2, &mut r);
        // Discard Beer cards through the table — they must land in Beer's
        // pile, not Wine's, regardless of who played them.
        t.discard(beer);
        assert_eq!(t.shoe(Deck::Beer).unwrap().discarded(), 2);
        assert_eq!(t.shoe(Deck::Wine).unwrap().discarded(), 0);
    }

    #[test]
    fn test_table_totals_and_projections() {
        let mut r = rng();
        let mut t = LcTable::new();
        t.open(Deck::Beer, &mut r);
        t.open(Deck::Cider, &mut r);
        assert_eq!(t.total(), LC_DECK_SIZE as usize * 2);

        let hand = t.draw(Deck::Beer, 4, &mut r);
        assert_eq!(
            t.counts(),
            vec![(Deck::Beer, LC_DECK_SIZE - 4), (Deck::Cider, LC_DECK_SIZE)]
        );
        t.discard(hand);
        assert_eq!(t.discard_total(), 4);
        assert_eq!(t.discard_counts(), vec![(Deck::Beer, 4), (Deck::Cider, 0)]);
        assert_eq!(t.all_discards().len(), 4);
    }

    #[test]
    fn test_the_whole_table_is_reproducible_from_a_seed() {
        // The desync fix, in one assertion: same seed, same table, same
        // cards, in the same order.
        let deal = |seed: u64| {
            let mut r = LcRng::seeded(seed);
            let mut t = LcTable::new();
            t.open(Deck::Beer, &mut r);
            t.open(Deck::Wine, &mut r);
            let a = t.draw(Deck::Beer, 5, &mut r);
            let b = t.draw(Deck::Wine, 5, &mut r);
            (a, b, t)
        };
        assert_eq!(deal(999), deal(999));
        assert_ne!(deal(999).0, deal(1000).0);
    }

    #[test]
    fn test_table_round_trips_through_serde() {
        let mut r = rng();
        let mut t = LcTable::new();
        t.open(Deck::Beer, &mut r);
        let d = t.draw(Deck::Beer, 6, &mut r);
        t.discard(d);
        let json = serde_json::to_string(&t).unwrap();
        let back: LcTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
