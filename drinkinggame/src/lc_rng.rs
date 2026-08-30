//! Last Call's deterministic PRNG — SplitMix64, seeded from and stored in
//! the game state.
//!
//! Before the engine owned its decks, card identity was decided in
//! `lc_routes` with `rand::thread_rng()`. That made a game unreproducible:
//! the same blob replayed twice dealt different cards, so nothing downstream
//! could ever check itself against the server, and `LastCallState::rng_seed`
//! was decorative. Every draw, deal and shuffle now runs through here, off a
//! `u64` that lives in the blob — so a state at seq N always produces the
//! same next card, and a desync is a bug you can reproduce rather than a
//! shrug.
//!
//! SplitMix64 on purpose: it is eleven lines, has no dependency, and its
//! output is a pure function of the counter — which means the whole
//! "engine is a pure state machine, no I/O, no RNG" contract in
//! `last_call.rs` survives intact. The engine still has no *ambient*
//! randomness; it has a number it advances. The one `thread_rng()` call
//! left in the crate is the room-creation seed in `lc_routes`, which is
//! exactly where entropy should enter.
//!
//! Deliberately minimal: `seeded`, `next_u64`, `below` and `shuffle` are what
//! the engine actually calls, and nothing else lives here. A `fork`/`range`/
//! `pick` trio was written speculatively, went unused, and shipped two latent
//! bugs nothing could catch because nothing called them (`fork(0)` returned a
//! stream identical to its parent, since `state ^ 0` is `state`; `range`
//! overflowed `hi - lo + 1` on a full-width span). Add a helper here when a
//! caller needs it, with the caller.
//!
//! Not cryptographic, and it must not become load-bearing for secrecy: a
//! player who knows the seed can predict the shoe. Secrecy in Last Call is
//! enforced by `PublicView`'s projection, never by unguessability — see the
//! `§3.4.1` comments in `last_call.rs`.

use serde::{Deserialize, Serialize};

/// A seeded SplitMix64 stream. `Copy` because it is small and callers
/// routinely hold it beside a `&mut` borrow of the table.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LcRng {
    /// The raw counter. Serialized as a plain integer so the stream resumes
    /// exactly where it left off across a snapshot round-trip.
    pub state: u64,
}

impl LcRng {
    /// A stream seeded from the room's `rng_seed`.
    ///
    /// The seed is mixed with an odd constant rather than used raw: two rooms
    /// created in the same millisecond can land on neighbouring seeds, and
    /// mixing once up front separates the whole family for free.
    pub fn seeded(seed: u64) -> Self {
        LcRng {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// The SplitMix64 step. Advances the counter and returns the mixed word.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform value in `0..n`, or `0` when `n == 0` (callers guard the
    /// empty case themselves; returning `0` keeps this total rather than
    /// panicking inside a deal).
    ///
    /// Lemire's multiply-shift with the rejection loop that makes it
    /// *exactly* uniform. The modulo shortcut would bias low indices, which
    /// over a 40-card shoe reshuffled all game is a real thumb on the scale,
    /// not a rounding curiosity. The loop's expected iteration count is
    /// under 1.000000001 for any `n` a card game will ever pass.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let n = n as u64;
        let threshold = n.wrapping_neg() % n;
        loop {
            let r = self.next_u64();
            let m = (r as u128).wrapping_mul(n as u128);
            if (m as u64) >= threshold {
                return (m >> 64) as usize;
            }
        }
    }

    /// In-place Fisher-Yates. Walks back-to-front so index `i` draws from
    /// `0..=i`, the form that is uniform over all permutations — the
    /// forward-walking variant that draws from the whole slice every step
    /// is the classic off-by-one that isn't.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_same_seed_same_stream() {
        let mut a = LcRng::seeded(1234);
        let mut b = LcRng::seeded(1234);
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn test_different_seeds_diverge_immediately() {
        // Neighbouring seeds must not produce neighbouring first draws —
        // rooms created back to back would otherwise deal alike.
        let mut a = LcRng::seeded(1000);
        let mut b = LcRng::seeded(1001);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn test_serde_resumes_the_stream_exactly() {
        // The whole point of storing the counter in the blob: a snapshot
        // taken mid-game deals the same next card after a reload.
        let mut live = LcRng::seeded(77);
        for _ in 0..10 {
            live.next_u64();
        }
        let json = serde_json::to_string(&live).unwrap();
        let mut restored: LcRng = serde_json::from_str(&json).unwrap();
        assert_eq!(live.next_u64(), restored.next_u64());
    }

    #[test]
    fn test_below_stays_in_range_and_is_total_at_zero() {
        let mut r = LcRng::seeded(5);
        for n in 1..40usize {
            for _ in 0..50 {
                assert!(r.below(n) < n, "n={n}");
            }
        }
        assert_eq!(r.below(0), 0); // total, not a panic
    }

    #[test]
    fn test_below_is_not_visibly_biased() {
        // Not a statistical proof — a smoke test that the rejection loop is
        // wired up. 60k draws over 6 buckets: each bucket should sit near
        // 10k, and a modulo-biased generator over this range would skew the
        // low buckets well outside the window.
        let mut r = LcRng::seeded(0xABCD);
        let mut counts = [0usize; 6];
        for _ in 0..60_000 {
            counts[r.below(6)] += 1;
        }
        for (i, &c) in counts.iter().enumerate() {
            assert!((9_000..11_000).contains(&c), "bucket {i} = {c}");
        }
    }

    #[test]
    fn test_shuffle_is_a_permutation() {
        let mut r = LcRng::seeded(9);
        let mut v: Vec<u32> = (0..40).collect();
        r.shuffle(&mut v);
        assert_ne!(v, (0..40).collect::<Vec<u32>>()); // actually moved
        v.sort_unstable();
        assert_eq!(v, (0..40).collect::<Vec<u32>>()); // lost nothing
    }

    #[test]
    fn test_shuffle_handles_degenerate_slices() {
        let mut r = LcRng::seeded(3);
        let mut empty: Vec<u8> = vec![];
        r.shuffle(&mut empty);
        assert!(empty.is_empty());
        let mut one = vec![42];
        r.shuffle(&mut one);
        assert_eq!(one, vec![42]);
    }

    #[test]
    fn test_shuffle_reaches_every_permutation_of_three() {
        // Fisher-Yates walked the wrong way still *looks* random but cannot
        // reach all 3! = 6 orderings uniformly. Pin that it does.
        let mut r = LcRng::seeded(2024);
        let mut seen: HashMap<Vec<u8>, usize> = HashMap::new();
        for _ in 0..6_000 {
            let mut v = vec![1u8, 2, 3];
            r.shuffle(&mut v);
            *seen.entry(v).or_default() += 1;
        }
        assert_eq!(seen.len(), 6, "not every permutation is reachable");
        for (perm, &n) in &seen {
            assert!((700..1_300).contains(&n), "{perm:?} = {n}");
        }
    }
}
