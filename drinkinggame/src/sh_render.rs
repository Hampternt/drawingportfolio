//! Backroom's **broadcast-safe** renderers.
//!
//! Every function in this file takes `&ShPublicView`, `&ShPublicSeat`,
//! `&HashMap<i64, String>` or scalars — **never `&SecretHitlerState`**. That
//! signature is the secrecy proof: a reviewer verifies this file without
//! reading a single body, and `test_sh_render_takes_no_secret_state` fails
//! the build if a signature drifts. Everything here is published to a room's
//! shared SSE channel, which `routes::sse_stream` serves with no session
//! extractor, so it also paints the cookie-less spectator screen.
//!
//! # These panels are display-only
//!
//! No control, no form and no button lives in a broadcast fragment. The
//! viewer's own controls — vote buttons, the nomination picker, the policy
//! tiles they are holding, a power's target list — are rendered by
//! `sh_routes`'s private builders and fetched over a session-authenticated
//! route.
//!
//! That is stricter than the `data-show-player` contract needs to be: a vote
//! button is not a secret, and `personalize()` could gate one perfectly well.
//! It is worth the strictness anyway. `personalize()` only sets `el.hidden`,
//! the markup reaches every subscriber regardless, and `screen.html` runs no
//! `personalize()` at all by design — so "is this fragment safe?" would
//! become a per-element judgement call on every future edit. With controls
//! out entirely, the rule is one line: **nothing in this file is
//! interactive, and nothing in it is per-viewer.**
//!
//! `test_sh_render_emits_no_personalize_attributes` pins that.

use std::collections::HashMap;

use crate::render::html_escape;
use crate::secret_hitler::{
    BoardSize, LogEntry, Outcome, Phase, Policy, Power, Role, ShPublicSeat, ShPublicView,
    VoteRecord, CHAOS_AT, FASCIST_TRACK, LIBERAL_TRACK,
};
use crate::sh_theme as theme;

/// How many log lines the panel shows. The engine keeps more; a phone does
/// not want them.
const LOG_SHOWN: usize = 6;
/// How many past elections the panel shows in full. The rest stay in the
/// blob — vote history is permanent (§5.3 E21), just not all on screen.
const HISTORY_SHOWN: usize = 4;

fn name_of(names: &HashMap<i64, String>, player_id: i64) -> String {
    html_escape(names.get(&player_id).map(|s| s.as_str()).unwrap_or("?"))
}

fn seat_name(view: &ShPublicView, names: &HashMap<i64, String>, seat: usize) -> String {
    match view.seats.get(seat) {
        Some(s) => name_of(names, s.player_id),
        None => "?".to_string(),
    }
}

/// The banner describing what the table is waiting for.
fn phase_line(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let pres = seat_name(view, names, view.president_seat);
    match view.phase {
        Phase::Nomination => format!(
            "{} &middot; {}",
            theme::PHASE_NOMINATION,
            format_args!("{pres} is picking")
        ),
        Phase::Voting => {
            let cast = view.seats.iter().filter(|s| s.alive && s.voted).count();
            format!(
                "{} &middot; {cast}/{} in",
                theme::PHASE_VOTING,
                view.alive_count
            )
        }
        Phase::PresidentDraft => theme::PHASE_PRESIDENT_DRAFT.to_string(),
        Phase::ChancellorDraft => theme::PHASE_CHANCELLOR_DRAFT.to_string(),
        Phase::VetoPending => theme::PHASE_VETO_PENDING.to_string(),
        Phase::Power(p) => format!("{} &middot; {}", theme::PHASE_POWER, theme::power_name(p)),
        Phase::Over(o) => theme::outcome_banner(o).to_string(),
    }
}

/// One policy track: filled slots, then empty ones. The Racket track marks
/// the slots that print a power, which is public information printed on the
/// board itself.
fn track_html(view: &ShPublicView, fascist: bool) -> String {
    let (filled, total, label, class) = if fascist {
        (
            view.fascist_track,
            FASCIST_TRACK,
            theme::TRACK_FASCIST,
            "sh-track-fascist",
        )
    } else {
        (
            view.liberal_track,
            LIBERAL_TRACK,
            theme::TRACK_LIBERAL,
            "sh-track-liberal",
        )
    };
    let mut slots = String::new();
    for i in 1..=total {
        let on = if i <= filled { " sh-slot-on" } else { "" };
        let power = if fascist {
            match view.board.power_at(i) {
                Some(p) => format!(
                    r#"<span class="sh-slot-power">{}</span>"#,
                    html_escape(theme::power_name(p))
                ),
                None => String::new(),
            }
        } else {
            String::new()
        };
        slots.push_str(&format!(
            r#"<li class="sh-slot{on}">{power}</li>"#,
            on = on,
            power = power
        ));
    }
    format!(
        r#"<div class="sh-track {class}"><span class="sh-track-label">{label} {filled}/{total}</span><ul class="sh-slots">{slots}</ul></div>"#,
        class = class,
        label = html_escape(label),
        filled = filled,
        total = total,
        slots = slots
    )
}

/// The failed-government counter.
fn heat_html(view: &ShPublicView) -> String {
    let mut pips = String::new();
    for i in 1..=CHAOS_AT {
        let on = if i <= view.election_tracker {
            " sh-pip-on"
        } else {
            ""
        };
        pips.push_str(&format!(r#"<li class="sh-pip{on}"></li>"#));
    }
    format!(
        r#"<div class="sh-heat"><span class="sh-heat-label">{} {}/{}</span><ul class="sh-pips">{pips}</ul></div>"#,
        html_escape(theme::TRACKER_NAME),
        view.election_tracker,
        CHAOS_AT,
        pips = pips
    )
}

/// One row of the seat roster. Carries only facts visible from a chair:
/// who holds which placard, who is out, who has been dug into, whether a
/// ballot is down, and — once the game is over — the revealed role.
fn seat_row(
    seat: &ShPublicSeat,
    view: &ShPublicView,
    names: &HashMap<i64, String>,
    revealed: Option<Role>,
) -> String {
    let mut badges = String::new();
    if seat.is_president {
        badges.push_str(&format!(
            r#"<span class="sh-badge sh-badge-chair">{}</span>"#,
            html_escape(theme::OFFICE_PRESIDENT_SHORT)
        ));
    }
    if seat.is_nominee {
        let label = if matches!(view.phase, Phase::Voting) {
            "UP"
        } else {
            theme::OFFICE_CHANCELLOR_SHORT
        };
        badges.push_str(&format!(
            r#"<span class="sh-badge sh-badge-second">{}</span>"#,
            html_escape(label)
        ));
    }
    if seat.investigated {
        badges.push_str(r#"<span class="sh-badge sh-badge-dug">DUG</span>"#);
    }
    if !seat.alive {
        badges.push_str(r#"<span class="sh-badge sh-badge-out">OUT</span>"#);
    }
    if let Some(role) = revealed {
        badges.push_str(&format!(
            r#"<span class="sh-badge sh-badge-role">{}</span>"#,
            html_escape(theme::role_name(role))
        ));
    }

    // During a vote, THAT a ballot is down — never which way.
    let ballot = if matches!(view.phase, Phase::Voting) && seat.alive {
        let cls = if seat.voted {
            "sh-ballot sh-ballot-in"
        } else {
            "sh-ballot"
        };
        format!(r#"<span class="{cls}"></span>"#)
    } else {
        String::new()
    };

    let dead = if seat.alive { "" } else { " sh-seat-out" };
    format!(
        r#"<li class="sh-seat{dead}"><span class="sh-seat-name">{name}</span>{badges}{ballot}</li>"#,
        dead = dead,
        name = name_of(names, seat.player_id),
        badges = badges,
        ballot = ballot
    )
}

fn roster_html(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let rows: String = view
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let revealed = view.final_roles.as_ref().and_then(|r| r.get(i).copied());
            seat_row(s, view, names, revealed)
        })
        .collect();
    format!(r#"<ul class="sh-seats">{rows}</ul>"#)
}

/// One public log line. Every variant here is a fact the table watched
/// happen; none carries a faction, a role, or a tile that was not played
/// face up.
fn log_line(e: &LogEntry, view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let n = |seat: usize| seat_name(view, names, seat);
    match e {
        LogEntry::Nominated { president, nominee } => {
            format!("{} {} {}", n(*president), theme::LOG_NOMINATED, n(*nominee))
        }
        LogEntry::Elected {
            president,
            chancellor,
        } => format!(
            "{} &amp; {} {}",
            n(*president),
            n(*chancellor),
            theme::LOG_ELECTED
        ),
        LogEntry::Rejected {
            president,
            nominee,
            ja,
            nay,
        } => format!(
            "{} &amp; {} {} {ja}&ndash;{nay}",
            n(*president),
            n(*nominee),
            theme::LOG_REJECTED
        ),
        LogEntry::Enacted { policy, chaos } => {
            if *chaos {
                format!(
                    "{} &middot; {}",
                    theme::LOG_CHAOS,
                    html_escape(theme::policy_name(*policy))
                )
            } else {
                format!(
                    "{} {}",
                    html_escape(theme::policy_name(*policy)),
                    theme::LOG_ENACTED
                )
            }
        }
        LogEntry::Vetoed {
            president,
            chancellor,
        } => format!(
            "{} &amp; {} {}",
            n(*president),
            n(*chancellor),
            theme::LOG_VETOED
        ),
        LogEntry::VetoRefused { president, .. } => {
            format!("{} {}", n(*president), theme::LOG_VETO_REFUSED)
        }
        LogEntry::Investigated { president, target } => format!(
            "{} {} {}",
            n(*president),
            theme::LOG_INVESTIGATED,
            n(*target)
        ),
        LogEntry::SpecialElection { president, target } => format!(
            "{} {} {}",
            n(*president),
            theme::LOG_SPECIAL_ELECTION,
            n(*target)
        ),
        LogEntry::Peeked { president } => format!("{} {}", n(*president), theme::LOG_PEEKED),
        LogEntry::Executed { president, target } => {
            format!("{} {} {}", n(*president), theme::LOG_EXECUTED, n(*target))
        }
        LogEntry::VetoUnlocked => theme::LOG_VETO_UNLOCKED.to_string(),
    }
}

fn log_html(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    if view.log.is_empty() {
        return String::new();
    }
    let rows: String = view
        .log
        .iter()
        .rev()
        .take(LOG_SHOWN)
        .map(|e| format!(r#"<li>{}</li>"#, log_line(e, view, names)))
        .collect();
    format!(r#"<ul class="sh-log">{rows}</ul>"#)
}

/// One closed election, ballots and all. Public and permanent (§5.3 E21) —
/// the game's primary deduction surface.
fn vote_record_html(r: &VoteRecord, view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let cells: String = r
        .ballots
        .iter()
        .enumerate()
        .map(|(i, b)| match b {
            Some(true) => format!(
                r#"<li class="sh-ball sh-ball-yes">{}</li>"#,
                seat_name(view, names, i)
            ),
            Some(false) => format!(
                r#"<li class="sh-ball sh-ball-no">{}</li>"#,
                seat_name(view, names, i)
            ),
            None => String::new(),
        })
        .collect();
    let verdict = if r.passed { "IN" } else { "OUT" };
    format!(
        r#"<div class="sh-vote-record"><span class="sh-vote-head">{p} &amp; {c} &middot; {verdict}</span><ul class="sh-balls">{cells}</ul></div>"#,
        p = seat_name(view, names, r.president),
        c = seat_name(view, names, r.nominee),
        verdict = verdict,
        cells = cells
    )
}

fn history_html(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    if view.vote_history.is_empty() {
        return String::new();
    }
    let recent: String = view
        .vote_history
        .iter()
        .rev()
        .take(HISTORY_SHOWN)
        .map(|r| vote_record_html(r, view, names))
        .collect();
    let more = view.vote_history.len().saturating_sub(HISTORY_SHOWN);
    let note = if more > 0 {
        format!(r#"<p class="sh-vote-more">and {more} earlier</p>"#)
    } else {
        String::new()
    };
    format!(r#"<details class="sh-history"><summary>Votes</summary>{recent}{note}</details>"#)
}

fn board_label(b: BoardSize) -> &'static str {
    match b {
        BoardSize::Small => "5–6",
        BoardSize::Mid => "7–8",
        BoardSize::Large => "9–10",
    }
}

/// The phone GAME tab, published on every `RoomMessage::Game`.
///
/// Root carries `data-sh-panel` so the client can tell a Backroom frame from
/// any other game's, and `data-sh-seq` so a late or out-of-order private
/// fetch can be dropped. It carries **no** `#sh-private` slot: `swapPanel`
/// replaces this element's innerHTML wholesale, which would destroy the
/// fetched pane on every frame, so the private pane is a sibling section
/// owned by `room.html`.
pub fn sh_public_panel(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let hitler_warning = if view.hitler_check_live && !matches!(view.phase, Phase::Over(_)) {
        format!(
            r#"<p class="sh-warning">{} in the second chair now wins it for the {}.</p>"#,
            html_escape(theme::ROLE_HITLER),
            html_escape(theme::PARTY_FASCIST)
        )
    } else {
        String::new()
    };
    let veto = if view.veto_unlocked && !matches!(view.phase, Phase::Over(_)) {
        format!(
            r#"<p class="sh-note">{}</p>"#,
            html_escape(theme::LOG_VETO_UNLOCKED)
        )
    } else {
        String::new()
    };

    format!(
        r#"<div class="game-active sh-board" data-sh-panel data-sh-seq="{seq}" data-anim-key="{seq}">
<div class="sh-head"><span class="sh-kicker">{game} &middot; {board}</span><p class="sh-phase" data-anim="pop">{phase}</p></div>
{tracks_l}{tracks_f}{heat}{warning}{veto}
{roster}
{last}
{log}
{history}
<p class="sh-deck">{draw} down, {discard} binned</p>
</div>"#,
        seq = view.seq,
        game = html_escape(theme::GAME_NAME),
        board = board_label(view.board),
        phase = phase_line(view, names),
        tracks_l = track_html(view, false),
        tracks_f = track_html(view, true),
        heat = heat_html(view),
        warning = hitler_warning,
        veto = veto,
        roster = roster_html(view, names),
        last = last_vote_html(view, names),
        log = log_html(view, names),
        history = history_html(view, names),
        draw = view.draw_count,
        discard = view.discard_count,
    )
}

/// The ballots of the most recent closed election, revealed all at once.
fn last_vote_html(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    match view.vote_history.last() {
        Some(r) if !view.last_vote.is_empty() => format!(
            r#"<div class="sh-last-vote">{}</div>"#,
            vote_record_html(r, view, names)
        ),
        _ => String::new(),
    }
}

/// The spectator big screen. Same facts, laid out for a television, and
/// still display-only — `screen.html` runs no `personalize()`, so anything
/// per-viewer would render for everyone.
pub fn sh_screen_panel(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    format!(
        r#"<div class="screen-panel sh-screen" data-sh-screen>
<div class="screen-top"><span class="live-dot"></span><span class="screen-kicker">{game}</span><span class="screen-remaining">{draw} down</span></div>
<div class="sh-screen-body"><p class="sh-screen-phase">{phase}</p>{tracks_l}{tracks_f}{heat}{roster}</div>
<div class="screen-footer"><span class="screen-footer-rules">{log}</span></div>
</div>"#,
        game = html_escape(theme::GAME_NAME),
        draw = view.draw_count,
        phase = phase_line(view, names),
        tracks_l = track_html(view, false),
        tracks_f = track_html(view, true),
        heat = heat_html(view),
        roster = roster_html(view, names),
        log = view
            .log
            .last()
            .map(|e| log_line(e, view, names))
            .unwrap_or_default(),
    )
}

/// The ROOM-tab seating fragment, slotted between WHO'S HERE and HOUSE
/// RULES exactly as 3 Man's is.
pub fn sh_seating_html(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    format!(
        r#"<div class="sh-seating"><h3 class="room-h3">{game} &middot; {alive} still in</h3>{roster}</div>"#,
        game = html_escape(theme::GAME_NAME),
        alive = view.alive_count,
        roster = roster_html(view, names),
    )
}

fn over_body(view: &ShPublicView, names: &HashMap<i64, String>, outcome: Outcome) -> String {
    format!(
        r#"<span class="sh-over-kicker">{winner} take it</span><h2 class="sh-over-title">{banner}</h2>{roster}"#,
        winner = html_escape(theme::outcome_winner(outcome)),
        banner = html_escape(theme::outcome_banner(outcome)),
        roster = roster_html(view, names),
    )
}

/// The phone's game-over panel. Roles are revealed here and only here —
/// `final_roles` is `Some` only once the engine is in `Phase::Over`.
pub fn sh_over_panel(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let Some(outcome) = view.outcome else {
        return String::new();
    };
    format!(
        r#"<div class="game-over sh-over">{}</div>"#,
        over_body(view, names, outcome)
    )
}

/// The big screen's game-over panel.
pub fn sh_screen_over(view: &ShPublicView, names: &HashMap<i64, String>) -> String {
    let Some(outcome) = view.outcome else {
        return String::new();
    };
    format!(
        r#"<div class="screen-panel sh-screen-over">{}</div>"#,
        over_body(view, names, outcome)
    )
}

/// The START card on the idle panel.
pub fn sh_start_card(base_path: &str, code: &str) -> String {
    format!(
        r#"<div class="start-card start-card-sh">
<h2 class="start-title">{name}</h2>
<p class="start-sub">{tagline}</p>
<form hx-post="{base_path}/room/{code}/sh/start" hx-swap="none">
<button type="submit" class="btn-primary">START</button>
</form>
</div>"#,
        name = html_escape(theme::GAME_NAME),
        tagline = html_escape(theme::TAGLINE),
    )
}

/// A policy tile's display class, shared by the public tracks and the
/// private hand so a Racket looks the same wherever it appears.
pub fn policy_class(p: Policy) -> &'static str {
    match p {
        Policy::Liberal => "sh-tile sh-tile-liberal",
        Policy::Fascist => "sh-tile sh-tile-fascist",
    }
}

/// A power's display class.
pub fn power_class(p: Power) -> &'static str {
    match p {
        Power::Investigate => "sh-power sh-power-dig",
        Power::SpecialElection => "sh-power sh-power-snap",
        Power::Peek => "sh-power sh-power-peek",
        Power::Execution => "sh-power sh-power-86",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_hitler::SecretHitlerState;

    fn view_and_names(n: usize) -> (ShPublicView, HashMap<i64, String>) {
        let st = SecretHitlerState::new((1..=n as i64).collect(), 42).unwrap();
        let names = (1..=n as i64)
            .map(|i| (i, format!("player{i}")))
            .collect::<HashMap<_, _>>();
        (st.public_view(), names)
    }

    /// This file's own source, minus the test module and minus every
    /// comment — so a sweep matches real code and not the prose explaining
    /// the rule it enforces.
    fn render_code() -> String {
        let src = include_str!("sh_render.rs");
        src.split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The signature sweep. This is the file's whole contract.
    #[test]
    fn test_sh_render_takes_no_secret_state() {
        assert!(
            !render_code().contains("SecretHitlerState"),
            "a builder in sh_render.rs named the full state type — every \
             function here must take &ShPublicView or a scalar, because \
             everything it produces is broadcast to a room's unauthenticated \
             SSE stream"
        );
    }

    #[test]
    fn test_sh_render_emits_no_personalize_attributes() {
        // `personalize()` only sets el.hidden, and screen.html never runs it
        // at all. A per-viewer attribute in a broadcast fragment is either a
        // leak or a lie; this file has neither.
        let code = render_code();
        for attr in ["data-show-player", "data-hide-player", "data-me-text"] {
            assert!(!code.contains(attr), "sh_render.rs emits {attr}");
        }
    }

    #[test]
    fn test_broadcast_panels_carry_no_controls() {
        let (view, names) = view_and_names(7);
        for html in [
            sh_public_panel(&view, &names),
            sh_screen_panel(&view, &names),
            sh_seating_html(&view, &names),
        ] {
            for control in ["<form", "<button", "<input", "hx-post", "hx-get"] {
                assert!(
                    !html.contains(control),
                    "a broadcast fragment contains {control}"
                );
            }
        }
    }

    #[test]
    fn test_the_panel_marks_itself_and_carries_its_seq() {
        let (mut view, names) = view_and_names(5);
        view.seq = 17;
        let html = sh_public_panel(&view, &names);
        assert!(html.contains("data-sh-panel"));
        assert!(html.contains(r#"data-sh-seq="17""#));
        assert!(!html.contains("id=\"sh-private\""), "the pane is a sibling");
    }

    #[test]
    fn test_no_role_reaches_a_panel_while_the_game_runs() {
        let (view, names) = view_and_names(9);
        let html = format!(
            "{}{}{}",
            sh_public_panel(&view, &names),
            sh_screen_panel(&view, &names),
            sh_seating_html(&view, &names)
        );
        assert!(view.final_roles.is_none(), "fixture is mid-game");
        for needle in [
            theme::ROLE_HITLER,
            theme::ROLE_LIBERAL,
            theme::PARTY_LIBERAL,
        ] {
            assert!(!html.contains(needle), "a live panel names {needle:?}");
        }
    }

    #[test]
    fn test_roles_appear_only_on_the_over_panel() {
        let mut st = SecretHitlerState::new((1..=5).collect(), 42).unwrap();
        let names: HashMap<i64, String> = (1..=5i64).map(|i| (i, format!("player{i}"))).collect();
        st.phase = Phase::Over(Outcome::HitlerExecuted);
        let view = st.public_view();
        let html = sh_over_panel(&view, &names);
        assert!(html.contains(theme::ROLE_HITLER));
        assert!(html.contains(&html_escape(theme::outcome_banner(Outcome::HitlerExecuted))));
        assert!(html.contains(theme::PARTY_LIBERAL), "and who took it");
    }

    #[test]
    fn test_the_over_panel_is_empty_while_the_game_runs() {
        let (view, names) = view_and_names(5);
        assert!(sh_over_panel(&view, &names).is_empty());
        assert!(sh_screen_over(&view, &names).is_empty());
    }

    #[test]
    fn test_a_ballot_shows_as_cast_never_as_a_value() {
        let mut st = SecretHitlerState::new((1..=5).collect(), 42).unwrap();
        let names: HashMap<i64, String> = (1..=5i64).map(|i| (i, format!("player{i}"))).collect();
        st.phase = Phase::Voting;
        st.nominee_seat = Some(1);
        st.votes = vec![Some(true), Some(false), None, None, None];
        let html = sh_public_panel(&st.public_view(), &names);
        assert_eq!(
            html.matches("sh-ballot-in").count(),
            2,
            "two ballots are down"
        );
        assert!(!html.contains(theme::VOTE_YES), "and neither is readable");
        assert!(!html.contains(theme::VOTE_NO));
    }

    #[test]
    fn test_names_are_escaped() {
        let (view, _) = view_and_names(5);
        let mut names = HashMap::new();
        names.insert(1i64, "<script>alert(1)</script>".to_string());
        for i in 2..=5i64 {
            names.insert(i, format!("p{i}"));
        }
        let html = sh_public_panel(&view, &names);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_a_missing_name_does_not_panic() {
        let (view, _) = view_and_names(6);
        let html = sh_public_panel(&view, &HashMap::new());
        assert!(html.contains("data-sh-panel"));
    }

    #[test]
    fn test_ten_seats_render() {
        let (view, names) = view_and_names(10);
        let html = sh_public_panel(&view, &names);
        assert_eq!(html.matches("sh-seat-name").count(), 10);
        assert!(html.contains("9–10"), "the Large board is labelled");
    }

    #[test]
    fn test_the_racket_track_prints_its_powers() {
        let (view, names) = view_and_names(9); // Large board
        let html = sh_public_panel(&view, &names);
        assert!(html.contains(theme::POWER_INVESTIGATE));
        assert!(html.contains(theme::POWER_SPECIAL_ELECTION));
        assert!(html.contains(theme::POWER_EXECUTION));
        assert!(
            !html.contains(theme::POWER_PEEK),
            "Read the Room is not on the 9–10 board"
        );
    }

    #[test]
    fn test_the_hitler_warning_tracks_the_public_track() {
        let mut st = SecretHitlerState::new((1..=7).collect(), 42).unwrap();
        let names: HashMap<i64, String> = (1..=7i64).map(|i| (i, format!("p{i}"))).collect();
        assert!(!sh_public_panel(&st.public_view(), &names).contains("sh-warning"));
        st.fascist_track = 3;
        assert!(sh_public_panel(&st.public_view(), &names).contains("sh-warning"));
    }

    #[test]
    fn test_the_start_card_posts_to_the_start_route() {
        let html = sh_start_card("/drinks", "ABCD");
        assert!(html.contains(r#"hx-post="/drinks/room/ABCD/sh/start""#));
        assert!(html.contains(">START<"));
        assert!(html.contains(theme::GAME_NAME));
    }

    #[test]
    fn test_log_lines_name_no_faction_or_tile_that_was_not_played() {
        let mut st = SecretHitlerState::new((1..=7).collect(), 42).unwrap();
        let names: HashMap<i64, String> = (1..=7i64).map(|i| (i, format!("p{i}"))).collect();
        st.log = vec![
            LogEntry::Investigated {
                president: 0,
                target: 3,
            },
            LogEntry::Peeked { president: 0 },
            LogEntry::Executed {
                president: 0,
                target: 4,
            },
        ];
        let html = log_html(&st.public_view(), &names);
        for needle in [
            theme::PARTY_LIBERAL,
            theme::PARTY_FASCIST,
            theme::POLICY_LIBERAL,
            theme::POLICY_FASCIST,
        ] {
            assert!(
                !html.contains(needle),
                "a log line about a private power named {needle:?}"
            );
        }
        assert!(html.contains(theme::LOG_INVESTIGATED));
    }
}
