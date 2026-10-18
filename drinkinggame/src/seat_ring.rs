//! The seat-ring table: players seated around the edge of a felt, with a
//! centre slot each game fills. Generalised from Last Call's ring
//! (2026-10-18) so Ring of Fire, 3 Man and the idle room share one table,
//! at both sizes — the 1920x1080 big screen and the phone pane.
//!
//! Two kinds of geometry, both pure:
//! - Last Call's authored rows for 2–8 seats (`authored`, read through
//!   `lc_layout`, which clamps at its `MAX_SEATS`). Last Call renders its
//!   own felt (`lc_render`, `#lc-felt`) on them; its markup is left alone,
//!   since its flight anchors, ids and tests key on it.
//! - Free-form seats for every other table (decided after midpoint 1): each
//!   token sits on the rim at a stored angle, wherever it was dropped, and
//!   seat order is the clockwise order from the room's start marker.
//!
//! Angles are degrees in [0, 360), clockwise on screen from the bottom of
//! the table (6 o'clock), so angle 0 is where Last Call puts seat 0.

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
/// felt's inner hairline ellipse, which is what makes the other rows
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

/// Half-axes of the rim ellipse, in percent of the table box: the inner
/// hairline the authored rows sit on (what the n = 4 row reads exactly).
const RIM: (f64, f64) = (43.0, 41.1);
/// The start marker sits just outside the token ring, on the felt's edge.
const MARKER_RIM: (f64, f64) = (47.0, 45.5);
/// Closest two tokens may land; a drop nearer than this is nudged off.
pub const MIN_SEPARATION: f64 = 4.0;

pub fn normalize(deg: f64) -> f64 {
    let d = deg.rem_euclid(360.0);
    if d >= 360.0 {
        0.0
    } else {
        d
    }
}

/// `angle` measured clockwise from the start marker, in [0, 360).
pub fn rel_angle(angle: f64, start: f64) -> f64 {
    normalize(angle - start)
}

/// A point on an ellipse of half-axes `axes` at `deg`: (left%, top%).
fn on_ellipse(deg: f64, axes: (f64, f64)) -> SeatPos {
    let t = deg.to_radians();
    let round = |v: f64| ((v * 10.0).round() / 10.0) as f32;
    (
        round(50.0 - axes.0 * t.sin()),
        round(50.0 + axes.1 * t.cos()),
    )
}

/// Where a token at `deg` sits on the rim.
pub fn rim_point(deg: f64) -> SeatPos {
    on_ellipse(deg, RIM)
}

/// `n` angles evenly round the rim, half a gap clockwise of the marker at
/// 0 — the migration's backfill from a plain seat order.
pub fn even_angles(n: usize) -> Vec<f64> {
    let step = 360.0 / n.max(1) as f64;
    (0..n).map(|i| (i as f64 + 0.5) * step).collect()
}

/// The marker-relative angle for slot `pos` among `others` (sorted
/// marker-relative angles of everyone else seated): the middle of the gap
/// between whoever would sit either side. Slot 0 is the gap after the
/// marker, the last slot the gap before it; `pos` past the end clamps.
pub fn slot_angle(others: &[f64], pos: usize) -> f64 {
    let pos = pos.min(others.len());
    let before = if pos == 0 { 0.0 } else { others[pos - 1] };
    let after = others.get(pos).copied().unwrap_or(360.0);
    (before + after) / 2.0
}

/// The marker-relative angle in the middle of the widest gap between
/// seated tokens (`others`: sorted marker-relative angles), going round
/// the whole table — the gap across the marker is one gap. Alone at the
/// table: opposite the marker. Ties go to the gap met first clockwise.
pub fn widest_gap_angle(others: &[f64]) -> f64 {
    let Some(&first) = others.first() else {
        return 180.0;
    };
    let mut best = (0.0_f64, 0.0_f64); // (width, mid)
    for (i, &a) in others.iter().enumerate() {
        let b = others.get(i + 1).copied().unwrap_or(first + 360.0);
        if b - a > best.0 {
            best = (b - a, (a + b) / 2.0);
        }
    }
    normalize(best.1)
}

/// Moves a marker-relative drop off any token it would land on (closer
/// than `MIN_SEPARATION`), on the side it was dropped, without crossing
/// the marker. A drop that can't clear every token in its gap stays at
/// the best spot found; order is still well defined (ties go by id).
pub fn nudge(rel: f64, others: &[f64]) -> f64 {
    let mut rel = rel;
    for _ in 0..=others.len() {
        let Some(&hit) = others.iter().find(|&&o| (o - rel).abs() < MIN_SEPARATION) else {
            break;
        };
        let moved = if rel >= hit {
            hit + MIN_SEPARATION
        } else {
            hit - MIN_SEPARATION
        };
        rel = moved.clamp(0.5, 359.5);
    }
    rel
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
    /// Rim angle (see the module doc).
    pub angle: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scale {
    Phone,
    Screen,
}

/// Above this many seats the chips shrink so a ring of 12 or more still
/// reads.
const CROWDED: usize = 8;

/// The phone's drag targets: where a dropped token's angle is posted, and
/// where the start marker's is.
pub struct DragUrls<'a> {
    pub seat: &'a str,
    pub start: &'a str,
}

/// The table: felt, centre slot (pre-rendered by the game), the start
/// marker at `start`, and the seats at their own angles. `seats` comes in
/// seat order, and each token shows its number in it. `drag` makes the
/// tokens and the marker draggable (phone only; `assets/seat_drag.js`
/// posts the drop there) — the big screen has no login, so it never gets
/// one.
pub fn table_html(
    seats: &[RingSeat],
    start: f64,
    centre: &str,
    scale: Scale,
    drag: Option<&DragUrls>,
) -> String {
    let n = seats.len();
    let drag_attr = if drag.is_some() { " data-drag" } else { "" };
    let seats_html: String = seats
        .iter()
        .enumerate()
        .map(|(i, seat)| {
            let (left, top) = rim_point(seat.angle);
            let initial = seat
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let active = if seat.active { " is-active" } else { "" };
            let badge = seat
                .badge
                .map(|b| format!(r#"<span class="felt-badge">{}</span>"#, html_escape(b)))
                .unwrap_or_default();
            format!(
                r#"<div class="felt-seat{active}" style="left:{left}%;top:{top}%" data-player-id="{id}" data-position="{pos}"{drag_attr}><span class="felt-avatar">{initial}<span class="felt-num">{pos}</span></span><span class="felt-name">{name}</span>{badge}</div>"#,
                id = seat.player_id,
                pos = i + 1,
                initial = html_escape(&initial),
                name = html_escape(seat.name),
            )
        })
        .collect();
    let (ml, mt) = on_ellipse(start, MARKER_RIM);
    let marker = format!(
        r#"<div class="felt-marker" style="left:{ml}%;top:{mt}%" data-marker{drag_attr} title="The count starts here">SEAT 1</div>"#
    );
    let scale_class = match scale {
        Scale::Phone => "felt-phone",
        Scale::Screen => "felt-screen",
    };
    let crowded = if n > CROWDED { " felt-crowded" } else { "" };
    let urls = drag
        .map(|d| {
            format!(
                r#" data-seat-url="{}" data-start-url="{}""#,
                html_escape(d.seat),
                html_escape(d.start)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<div class="felt-table {scale_class}{crowded}" data-seat-table data-count="{n}"{urls}><div class="felt"></div><div class="felt-centre">{centre}</div><div class="felt-ring">{marker}{seats_html}</div></div>"#
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
    fn test_rim_point_matches_the_authored_four_seat_row() {
        // The free-form rim is the ellipse Last Call's rows sit on.
        for (i, &p) in authored(4).iter().enumerate() {
            assert_eq!(rim_point(90.0 * i as f64), p, "seat {i}");
        }
    }

    #[test]
    fn test_angles_normalize_and_measure_from_the_marker() {
        assert_eq!(normalize(-90.0), 270.0);
        assert_eq!(normalize(360.0), 0.0);
        assert_eq!(normalize(725.0), 5.0);
        assert_eq!(rel_angle(10.0, 350.0), 20.0);
        assert_eq!(rel_angle(350.0, 10.0), 340.0);
    }

    #[test]
    fn test_even_angles_sit_half_a_gap_past_the_marker() {
        assert_eq!(even_angles(4), vec![45.0, 135.0, 225.0, 315.0]);
        assert!(even_angles(0).is_empty());
    }

    #[test]
    fn test_slot_angle_is_mid_gap() {
        let others = [60.0, 180.0, 300.0];
        assert_eq!(slot_angle(&others, 0), 30.0); // after the marker
        assert_eq!(slot_angle(&others, 1), 120.0);
        assert_eq!(slot_angle(&others, 3), 330.0); // before the marker
        assert_eq!(slot_angle(&others, 99), 330.0);
        assert_eq!(slot_angle(&[], 0), 180.0); // alone: opposite the marker
    }

    #[test]
    fn test_widest_gap_angle_goes_round_the_whole_table() {
        assert_eq!(widest_gap_angle(&[]), 180.0);
        assert_eq!(widest_gap_angle(&[90.0]), 270.0);
        assert_eq!(widest_gap_angle(&[10.0, 40.0, 250.0]), 145.0);
        // The widest gap spans the marker: 260 -> 360+20, mid 320.
        assert_eq!(widest_gap_angle(&[20.0, 100.0, 180.0, 260.0]), 320.0);
        // ... and its middle can fall past the marker: 300 -> 360+100, mid 20.
        assert_eq!(widest_gap_angle(&[100.0, 150.0, 200.0, 300.0]), 20.0);
    }

    #[test]
    fn test_nudge_moves_a_drop_off_a_token_on_its_side() {
        let others = [100.0, 200.0];
        assert_eq!(nudge(150.0, &others), 150.0, "clear drops stay put");
        assert_eq!(nudge(101.0, &others), 104.0);
        assert_eq!(nudge(99.0, &others), 96.0);
        assert_eq!(nudge(100.0, &others), 104.0);
        // Chained: off one token onto the next, and on past it.
        assert_eq!(nudge(3.0, &[2.0, 6.0]), 10.0);
        // Never across the marker.
        assert_eq!(nudge(359.0, &[358.0]), 359.5);
    }

    fn seat(id: i64, name: &str) -> RingSeat<'_> {
        RingSeat {
            player_id: id,
            name,
            badge: None,
            active: false,
            angle: 0.0,
        }
    }

    #[test]
    fn test_table_html_places_every_seat_at_its_angle_and_numbers_it() {
        let mut seats = [seat(7, "ann"), seat(3, "<b>ob")];
        seats[1].angle = 90.0;
        let html = table_html(&seats, 0.0, "<i>c</i>", Scale::Screen, None);
        assert_eq!(html.matches(r#"class="felt-seat""#).count(), 2);
        assert!(html.contains(r#"style="left:50%;top:91.1%" data-player-id="7" data-position="1""#));
        assert!(html.contains(r#"style="left:7%;top:50%" data-player-id="3" data-position="2""#));
        assert!(html.contains(r#"<span class="felt-num">2</span>"#));
        assert!(html.contains("&lt;b&gt;ob"), "names are escaped");
        assert!(html.contains(r#"<div class="felt-centre"><i>c</i></div>"#));
        assert!(html.contains("felt-screen"));
        assert_eq!(html.matches("data-marker").count(), 1);
        // The big screen never gets a drag handle or a drop target.
        assert!(!html.contains("data-drag") && !html.contains("data-seat-url"));
        // Nor the Last Call live marker: that handoff is Last Call's alone.
        assert!(!html.contains("data-lc-live"));
    }

    #[test]
    fn test_phone_table_with_drag_urls_is_draggable_marker_too() {
        let seats = [seat(1, "ann"), seat(2, "bob")];
        let urls = DragUrls {
            seat: "/room/QKAM/seats/angle",
            start: "/room/QKAM/seats/start",
        };
        let html = table_html(&seats, 180.0, "", Scale::Phone, Some(&urls));
        assert!(html.contains(r#"data-seat-url="/room/QKAM/seats/angle""#));
        assert!(html.contains(r#"data-start-url="/room/QKAM/seats/start""#));
        assert_eq!(html.matches(" data-drag>").count(), 2);
        assert!(html.contains(r#"data-marker data-drag"#));
        // The marker sits outside the token rim, at the top for 180.
        assert!(html.contains(r#"class="felt-marker" style="left:50%;top:4.5%""#));
    }

    #[test]
    fn test_badge_active_and_crowded() {
        let mut seats: Vec<RingSeat> = (1..=12).map(|i| seat(i, "p")).collect();
        seats[2].active = true;
        seats[4].badge = Some("3 MAN");
        let html = table_html(&seats, 0.0, "", Scale::Phone, None);
        assert!(html.contains("felt-crowded"));
        assert_eq!(html.matches("is-active").count(), 1);
        assert!(html.contains(r#"<span class="felt-badge">3 MAN</span>"#));
        let small = table_html(&seats[..8], 0.0, "", Scale::Phone, None);
        assert!(!small.contains("felt-crowded"));
    }
}
