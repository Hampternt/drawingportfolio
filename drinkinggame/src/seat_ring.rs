//! The seat-ring table: players seated around the edge of a felt, with a
//! centre slot each game fills. Generalised from Last Call's ring
//! (2026-10-18) so Ring of Fire, 3 Man and the idle room share one table,
//! at both sizes — the 1920x1080 big screen and the phone pane.
//!
//! Geometry is pure: a seat count in, box-relative percentages out. Rows
//! for 2–8 seats are the ones authored for Last Call; past 8 (Ring of Fire
//! and 3 Man rooms have no cap) they are generated on the same ellipse.
//!
//! Last Call renders its own felt (`lc_render`, `#lc-felt`) on these same
//! rows through `lc_layout`, which clamps at its `MAX_SEATS`. Its markup is
//! left alone: its flight anchors, ids and tests key on it.

use crate::render::html_escape;

/// A seat's centre, as percentages of the ring box: (left%, top%).
pub type SeatPos = (f32, f32);

/// The largest table with authored rows. Equal to Last Call's `MAX_SEATS`
/// (pinned by a test), which is the table they were authored for.
pub const AUTHORED_MAX: usize = 8;

/// Indexed by `n - 2`. Seat 0 is bottom-centre; the list runs clockwise.
///
/// The n == 7 row is transcribed from the design bundle rather than
/// generated: its angles are authored so that top-centre stays empty, and
/// the bottom seat is pulled inward to r ~= 0.85 (D.2, "the local player is
/// always nearest the viewer"). Six of its seven seats sit within 4% of the
/// felt's inner hairline ellipse, which is what makes the generated rows
/// consistent with it.
const RING: [&[SeatPos]; AUTHORED_MAX - 1] = [
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

/// The authored row for `n` (0..=AUTHORED_MAX). Past that, `&[]` — callers
/// that need more seats use `positions`.
pub fn authored(n: usize) -> &'static [SeatPos] {
    match n {
        0 => &[],
        1 => &RING[0][..1],
        n if n <= AUTHORED_MAX => RING[n - 2],
        _ => &[],
    }
}

/// Seat placements for a table of any size, seat 0 first, clockwise. The
/// authored rows up to 8; past that, evenly spaced on the inner hairline
/// ellipse the authored rows sit on (half-axes 43% and 41.1%, which is
/// exactly what the n = 4 row reads), seat 0 at the bottom.
pub fn positions(n: usize) -> Vec<SeatPos> {
    if n <= AUTHORED_MAX {
        return authored(n).to_vec();
    }
    (0..n)
        .map(|i| {
            let theta = std::f32::consts::TAU * i as f32 / n as f32;
            let round = |v: f32| (v * 10.0).round() / 10.0;
            (
                round(50.0 - 43.0 * theta.sin()),
                round(50.0 + 41.1 * theta.cos()),
            )
        })
        .collect()
}

/// Which ring slot a seat occupies for a given viewer. `me` is the
/// viewer's own seat; `None` (a spectator, or a member who is not seated)
/// is identity. An out-of-range `me` (`me >= n`, e.g. a stale seat index
/// left over after the table shrank) is also treated as identity, same
/// as `None`. Rotates so the viewer always lands on slot 0 —
/// bottom-centre.
pub fn view_index(seat: usize, me: Option<usize>, n: usize) -> usize {
    match me {
        Some(me) if n > 0 && me < n => (seat + n - me) % n,
        _ => seat,
    }
}

/// One player on the ring.
pub struct RingSeat<'a> {
    pub player_id: i64,
    pub name: &'a str,
    /// A small badge under the name, e.g. "3 MAN".
    pub badge: Option<&'a str>,
    /// The highlighted seat, e.g. whoever is rolling.
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scale {
    Phone,
    Screen,
}

/// Above this many seats the chips shrink so a ring of 12 or more still
/// reads.
const CROWDED: usize = 8;

/// The table: felt, centre slot (pre-rendered by the game), and the seats
/// at `positions(n)` in ring order. `reorder_url` makes the seats
/// draggable (phone only; `assets/seat_drag.js` posts the drop there) —
/// the big screen has no login, so it never gets one.
pub fn table_html(
    seats: &[RingSeat],
    centre: &str,
    scale: Scale,
    reorder_url: Option<&str>,
) -> String {
    let n = seats.len();
    let spots = positions(n);
    let draggable = reorder_url.is_some();
    let seats_html: String = seats
        .iter()
        .zip(spots.iter())
        .enumerate()
        .map(|(i, (seat, &(left, top)))| {
            let initial = seat
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let active = if seat.active { " is-active" } else { "" };
            let drag = if draggable { " data-drag" } else { "" };
            let badge = seat
                .badge
                .map(|b| format!(r#"<span class="felt-badge">{}</span>"#, html_escape(b)))
                .unwrap_or_default();
            format!(
                r#"<div class="felt-seat{active}" style="left:{left}%;top:{top}%" data-player-id="{id}" data-position="{pos}"{drag}><span class="felt-avatar">{initial}</span><span class="felt-name">{name}</span>{badge}</div>"#,
                id = seat.player_id,
                pos = i + 1,
                initial = html_escape(&initial),
                name = html_escape(seat.name),
            )
        })
        .collect();
    let scale_class = match scale {
        Scale::Phone => "felt-phone",
        Scale::Screen => "felt-screen",
    };
    let crowded = if n > CROWDED { " felt-crowded" } else { "" };
    let reorder_attr = reorder_url
        .map(|u| format!(r#" data-reorder-url="{}""#, html_escape(u)))
        .unwrap_or_default();
    format!(
        r#"<div class="felt-table {scale_class}{crowded}" data-seat-table data-count="{n}"{reorder_attr}><div class="felt"></div><div class="felt-centre">{centre}</div><div class="felt-ring">{seats_html}</div></div>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authored_max_is_last_calls_table() {
        assert_eq!(AUTHORED_MAX, crate::last_call::MAX_SEATS);
    }

    #[test]
    fn test_positions_reuse_the_authored_rows_up_to_eight() {
        for n in 0..=AUTHORED_MAX {
            assert_eq!(positions(n), authored(n).to_vec(), "n={n}");
        }
    }

    #[test]
    fn test_generated_formula_reproduces_the_four_seat_row() {
        // The ellipse the generated rows use is the one the authored n=4
        // row sits on; checking it here keeps the two families consistent.
        let theta = |i: f32| std::f32::consts::TAU * i / 4.0;
        for (i, &(l, t)) in authored(4).iter().enumerate() {
            let gl = 50.0 - 43.0 * theta(i as f32).sin();
            let gt = 50.0 + 41.1 * theta(i as f32).cos();
            assert!((gl - l).abs() < 0.05 && (gt - t).abs() < 0.05, "seat {i}");
        }
    }

    #[test]
    fn test_big_tables_are_generated_inside_the_box_bottom_first() {
        for n in [9, 12, 16, 24] {
            let p = positions(n);
            assert_eq!(p.len(), n);
            assert_eq!(p[0], (50.0, 91.1), "n={n} seat 0 bottom-centre");
            // Clockwise from the bottom: seat 1 is left of centre, lower half.
            assert!(p[1].0 < 50.0 && p[1].1 > 50.0, "n={n} seat 1 {:?}", p[1]);
            for (i, (l, t)) in p.iter().enumerate() {
                assert!((0.0..=100.0).contains(l), "n={n} seat {i} left {l}");
                assert!((0.0..=100.0).contains(t), "n={n} seat {i} top {t}");
            }
            // No two seats share a spot.
            for i in 0..n {
                for j in i + 1..n {
                    assert_ne!(p[i], p[j], "n={n} seats {i} and {j} collide");
                }
            }
        }
    }

    fn seat(id: i64, name: &str) -> RingSeat<'_> {
        RingSeat {
            player_id: id,
            name,
            badge: None,
            active: false,
        }
    }

    #[test]
    fn test_table_html_places_every_seat_in_order() {
        let seats = [seat(7, "ann"), seat(3, "<b>ob")];
        let html = table_html(&seats, "<i>c</i>", Scale::Screen, None);
        assert_eq!(html.matches(r#"class="felt-seat""#).count(), 2);
        assert!(html.contains(r#"style="left:50%;top:91.1%" data-player-id="7" data-position="1""#));
        assert!(html.contains(r#"data-player-id="3" data-position="2""#));
        assert!(html.contains("&lt;b&gt;ob"), "names are escaped");
        assert!(html.contains(r#"<div class="felt-centre"><i>c</i></div>"#));
        assert!(html.contains("felt-screen"));
        // The big screen never gets a drag handle or a reorder target.
        assert!(!html.contains("data-drag") && !html.contains("data-reorder-url"));
        // Nor the Last Call live marker: that handoff is Last Call's alone.
        assert!(!html.contains("data-lc-live"));
    }

    #[test]
    fn test_phone_table_with_a_reorder_url_is_draggable() {
        let seats = [seat(1, "ann"), seat(2, "bob")];
        let html = table_html(&seats, "", Scale::Phone, Some("/room/QKAM/seats/move"));
        assert!(html.contains(r#"data-reorder-url="/room/QKAM/seats/move""#));
        assert_eq!(html.matches(" data-drag>").count(), 2);
    }

    #[test]
    fn test_badge_active_and_crowded() {
        let mut seats: Vec<RingSeat> = (1..=12).map(|i| seat(i, "p")).collect();
        seats[2].active = true;
        seats[4].badge = Some("3 MAN");
        let html = table_html(&seats, "", Scale::Phone, None);
        assert!(html.contains("felt-crowded"));
        assert_eq!(html.matches("is-active").count(), 1);
        assert!(html.contains(r#"<span class="felt-badge">3 MAN</span>"#));
        let small = table_html(&seats[..8], "", Scale::Phone, None);
        assert!(!small.contains("felt-crowded"));
    }
}
