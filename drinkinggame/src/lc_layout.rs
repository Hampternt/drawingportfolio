//! Last Call — seat-ring geometry.
//!
//! Pure functions from a seat count to a table of box-relative percentage
//! constants. No I/O, no async, no globals: this module is geometry in,
//! geometry out, so the same layout drives both the 1920x1080 big screen
//! and the 466px phone pane.

use crate::last_call::MAX_SEATS;

/// A seat's centre, as percentages of the ring box: (left%, top%).
pub type SeatPos = (f32, f32);

/// Indexed by `n - 2`. Seat 0 is bottom-centre; the list runs clockwise.
///
/// The n == 7 row is transcribed from the design bundle rather than
/// generated: its angles are authored so that top-centre stays empty, and
/// the bottom seat is pulled inward to r ~= 0.85 (D.2, "the local player is
/// always nearest the viewer"). Six of its seven seats sit within 4% of the
/// felt's inner hairline ellipse, which is what makes the generated rows
/// consistent with it.
const RING: [&[SeatPos]; 7] = [
    // n = 2
    &[(50.0, 91.1), (50.0, 8.9)],
    // n = 3
    &[(50.0, 91.1), (12.8, 29.4), (87.2, 29.4)],
    // n = 4
    &[(50.0, 91.1), (7.0, 50.0), (50.0, 8.9), (93.0, 50.0)],
    // n = 5
    &[
        (50.0, 91.1),
        (9.1, 62.7),
        (24.7, 16.7),
        (75.3, 16.7),
        (90.9, 62.7),
    ],
    // n = 6
    &[
        (50.0, 91.1),
        (12.8, 70.5),
        (12.8, 29.4),
        (50.0, 8.9),
        (87.2, 29.4),
        (87.2, 70.5),
    ],
    // n = 7 — authored, from the bundle
    &[
        (50.0, 84.9),
        (20.3, 80.4),
        (8.9, 47.6),
        (26.4, 15.3),
        (74.2, 15.1),
        (91.1, 47.6),
        (79.7, 80.4),
    ],
    // n = 8 — D.2's "8 compresses the two bottom positions"
    &[
        (50.0, 91.1),
        (19.6, 79.1),
        (7.0, 50.0),
        (19.6, 20.9),
        (50.0, 8.9),
        (80.4, 20.9),
        (93.0, 50.0),
        (80.4, 79.1),
    ],
];

/// Seat placements for a table of `n`, seat 0 first, clockwise.
/// Returns `&[]` for n == 0. Clamps n > MAX_SEATS to the MAX_SEATS row.
pub fn seat_positions(n: usize) -> &'static [SeatPos] {
    match n {
        0 => &[],
        1 => &RING[0][..1],
        _ => RING[n.min(MAX_SEATS) - 2],
    }
}

/// Which ring slot a seat occupies for a given viewer. `me` is the
/// viewer's own seat; `None` (a spectator, or a member who is not seated)
/// is identity. Rotates so the viewer always lands on slot 0 —
/// bottom-centre.
pub fn view_index(seat: usize, me: Option<usize>, n: usize) -> usize {
    match me {
        Some(me) if n > 0 => (seat + n - me) % n,
        _ => seat,
    }
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
