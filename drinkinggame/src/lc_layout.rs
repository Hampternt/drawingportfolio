//! Last Call — seat-ring geometry, clamped to Last Call's table size.
//!
//! Pure functions from a seat count to a table of box-relative percentage
//! constants. No I/O, no async, no globals: this module is geometry in,
//! geometry out, so the same layout drives both the 1920x1080 big screen
//! and the 466px phone pane.

use crate::last_call::MAX_SEATS;

// The geometry itself — the authored rows and the viewer rotation — moved
// to `seat_ring` when Ring of Fire and 3 Man got tables of their own
// (2026-10-18). Last Call keeps this clamped view of it: its tables stop at
// `MAX_SEATS`, and a stale oversized state must still render short rather
// than switch to the generated rows the other games use past 8.
pub use crate::seat_ring::{view_index, SeatPos};

/// Seat placements for a table of `n`, seat 0 first, clockwise.
/// Returns `&[]` for n == 0. Clamps n > MAX_SEATS to the MAX_SEATS row.
pub fn seat_positions(n: usize) -> &'static [SeatPos] {
    crate::seat_ring::authored(n.min(MAX_SEATS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seat_zero_is_bottom_centre_for_every_count() {
        // Every table puts seat 0 at the bottom of the ring: that is the
        // anchor the phone's rotation depends on.
        for n in 2..=MAX_SEATS {
            let p = seat_positions(n)[0];
            assert_eq!(p.0, 50.0, "n={n} seat 0 must be horizontally centred");
            assert!(p.1 > 80.0, "n={n} seat 0 must sit low, got {}", p.1);
        }
    }

    #[test]
    fn test_row_lengths_match_seat_count() {
        for n in 2..=MAX_SEATS {
            assert_eq!(seat_positions(n).len(), n);
        }
    }

    #[test]
    fn test_seven_row_is_the_authored_bundle_geometry() {
        // Transcribed from Game UI.dc.html's big-screen v2. If this test is
        // ever "fixed" to match a formula, the ring stops matching the design.
        assert_eq!(
            seat_positions(7),
            &[
                (50.0, 84.9),
                (20.3, 80.4),
                (8.9, 47.6),
                (26.4, 15.3),
                (74.2, 15.1),
                (91.1, 47.6),
                (79.7, 80.4),
            ]
        );
    }

    #[test]
    fn test_every_seat_is_inside_the_box() {
        for n in 2..=MAX_SEATS {
            for (i, (l, t)) in seat_positions(n).iter().enumerate() {
                assert!((0.0..=100.0).contains(l), "n={n} seat {i} left {l}");
                assert!((0.0..=100.0).contains(t), "n={n} seat {i} top {t}");
            }
        }
    }

    #[test]
    fn test_over_max_seats_clamps_rather_than_panicking() {
        // add_player's ceiling is Task 6's; a renderer handed a stale
        // oversized state must still render rather than index out of bounds.
        assert_eq!(seat_positions(99).len(), MAX_SEATS);
    }

    #[test]
    fn test_view_index_puts_the_viewer_at_the_bottom() {
        // 5 seats, viewer in seat 3 -> viewer occupies slot 0.
        assert_eq!(view_index(3, Some(3), 5), 0);
        assert_eq!(view_index(4, Some(3), 5), 1);
        assert_eq!(view_index(0, Some(3), 5), 2);
        assert_eq!(view_index(1, Some(3), 5), 3);
        assert_eq!(view_index(2, Some(3), 5), 4);
    }

    #[test]
    fn test_view_index_is_identity_for_a_spectator() {
        // The big screen and an unseated member both pass None.
        for seat in 0..6 {
            assert_eq!(view_index(seat, None, 6), seat);
        }
    }

    #[test]
    fn test_view_index_is_identity_for_an_out_of_range_seat() {
        // A stale `me` outliving a table that shrank (Task 6's seat
        // ceiling enforcement) must not underflow/panic; treat it as
        // identity, same as None.
        for seat in [0, 1, 3] {
            assert_eq!(view_index(seat, Some(4), 4), seat);
            assert_eq!(view_index(seat, Some(9), 4), seat);
        }
    }

    #[test]
    fn test_view_index_is_a_permutation() {
        // No two seats may collide on one ring slot — a collision would stack
        // two plaques at identical coordinates and silently hide one.
        for n in 2..=MAX_SEATS {
            for me in 0..n {
                let mut slots: Vec<usize> = (0..n).map(|s| view_index(s, Some(me), n)).collect();
                slots.sort_unstable();
                assert_eq!(slots, (0..n).collect::<Vec<_>>(), "n={n} me={me}");
            }
        }
    }
}
