//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.
//!
//! Every broadcast fragment is identical for every viewer — per-viewer
//! differences are expressed only through the data-attribute contract
//! (`data-show-player` / `data-hide-player` / `data-me-text`, see
//! `fragment-contract.md`), never by branching on a viewer id here.

use crate::cards::Card;
use crate::models::{DrawCount, HouseRule, LeaderboardRow, RoomMember, RulePreset};

/// Same escape set as the portfolio's html_escape.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Returns the <li> rows for the leaderboard. The surrounding
/// <ol id="standings-list"> lives in the templates so SSE can replace
/// innerHTML wholesale. Rows carry the full data-attr contract so
/// `personalize()` can mark the viewer's row and feed the idle stat card /
/// thumb-bar labels without a second render pass.
pub fn leaderboard_items(rows: &[LeaderboardRow]) -> String {
    if rows.is_empty() {
        return r#"<li class="lb-empty">Nobody here yet</li>"#.to_string();
    }
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let rank = i + 1;
            format!(
                r#"<li class="lb-row" data-player-id="{id}" data-drinks="{drinks}" data-shots="{shots}" data-rank="{rank}"><span class="lb-rank">{rank}</span><span class="lb-name">{name}</span><span class="lb-counts">{drinks} D &middot; {shots} S</span></li>"#,
                id = r.id,
                drinks = r.drinks,
                shots = r.shots,
                name = html_escape(&r.name),
            )
        })
        .collect()
}

pub struct CurrentCard {
    pub card: Card,
    pub title: String,
    pub text: String,
    pub drawer: String,
    pub drawer_id: i64,
    pub draw_id: i64,
    /// True only for an unresolved Jack: rank 11 with no house_rules row for
    /// this draw_id yet. Gates the rule-input form, shown only to the drawer.
    pub pending_rule: bool,
}

pub struct HeldCardView {
    pub draw_id: i64,
    pub holder_id: i64,
    pub holder_name: String,
    pub card: Card,
    pub title: String,
}

pub struct GameView<'a> {
    pub base_path: &'a str,
    pub code: &'a str,
    pub current: Option<CurrentCard>,
    pub remaining: i64,
    pub held: Vec<HeldCardView>,
    pub counts: &'a [DrawCount],
    pub announcement: Option<String>,
    /// Ring of Fire: "{draw_count}-{spend_count}". Client compares this
    /// against the last-seen key per SSE event and only replays the flip
    /// animation on [data-anim] elements when it changes.
    pub anim_key: String,
}

/// Post-game superlatives for the over panel (phone) and screen-over panel.
pub struct GameSummary {
    /// (name, draws) of whoever drew the most cards.
    pub hardest: Option<(String, i64)>,
    /// (name, shots) of whoever logged the most shots.
    pub most_shots: Option<(String, i64)>,
    /// Room-wide drinks + shots logged this session (all members, summed).
    pub room_total: i64,
    /// Name of whoever drew the most recent King, if any King was drawn.
    pub kings_cup: Option<String>,
    pub counts: Vec<DrawCount>,
    pub house_rules: Vec<HouseRule>,
}

pub struct RoomView<'a> {
    pub base_path: &'a str,
    pub code: &'a str,
    pub members: &'a [RoomMember],
    pub house_rules: &'a [HouseRule],
    pub kings: i64,
    /// Active game's `kind` ("ring_of_fire" | "three_man" | ...) or "idle".
    pub mode: &'a str,
}

/// A card face in pure HTML/CSS — rank top-left, suit glyph under it, a
/// large center pip, and a rotated rank bottom-right. Red suits get
/// `.card-red`; used at phone scale (`.hero-card`) and screen scale
/// (`.screen-hero`) — the CSS, not this markup, decides the size.
pub fn card_face_html(card: Card) -> String {
    let red = if card.suit.is_red() { " card-red" } else { "" };
    let rank = card.rank_label();
    let glyph = card.suit.glyph();
    format!(
        r#"<div class="card-big{red}"><span class="card-rank-tl">{rank}</span><span class="card-suit-tl">{glyph}</span><span class="card-pip">{glyph}</span><span class="card-rank-br">{rank}</span></div>"#
    )
}

pub fn preset_options(presets: &[RulePreset]) -> String {
    presets
        .iter()
        .map(|p| {
            format!(
                r#"<option value="{}">{}</option>"#,
                p.id,
                html_escape(&p.name)
            )
        })
        .collect()
}

/// <li> rows for the preset list page: edit link + delete form.
pub fn preset_rows(base_path: &str, presets: &[RulePreset]) -> String {
    presets
        .iter()
        .map(|p| {
            format!(
                r#"<li><a href="{base_path}/presets/{}">{}</a><form method="post" action="{base_path}/presets/{}/delete" onsubmit="return confirm('Delete this preset?')"><button class="btn-delete">Delete</button></form></li>"#,
                p.id,
                html_escape(&p.name),
                p.id,
            )
        })
        .collect()
}

/// One <fieldset> per rank for the edit form. Field names are rank-suffixed
/// (title_1..title_13 etc.) — the save handler reassembles them by rank.
pub fn preset_edit_rows(rules: &[crate::rules::RuleEntry]) -> String {
    rules
        .iter()
        .map(|r| {
            let label = Card { rank: r.rank, suit: crate::cards::Suit::Spades }.rank_label();
            let checked = if r.holdable { " checked" } else { "" };
            format!(
                r#"<fieldset class="rank-row"><legend>{label}</legend><input name="title_{rank}" value="{title}" maxlength="40" required><textarea name="text_{rank}" rows="2" maxlength="300" required>{text}</textarea><label class="hold-label"><input type="checkbox" name="holdable_{rank}"{checked}> Holdable</label></fieldset>"#,
                rank = r.rank,
                title = html_escape(&r.title),
                text = html_escape(&r.text),
            )
        })
        .collect()
}

/// Idle state: "your night so far" stat card (drinks/shots placeholders
/// filled client-side from the leaderboard's [data-my-drinks]/[data-my-shots]
/// contract) + the Ring of Fire start card. `.start-card-amber` is an
/// unused-in-Phase-1 modifier reserved for the 3 Man start card (Task 11).
pub fn game_idle_panel(base_path: &str, code: &str, presets: &[RulePreset]) -> String {
    let options = preset_options(presets);
    format!(
        r#"<div class="game-idle">
<div class="stat-card">
<span class="stat-label">YOUR NIGHT SO FAR</span>
<div class="stat-row">
<div class="stat-item"><span class="stat-value" data-my-drinks>0</span><span class="stat-unit">DRINKS</span></div>
<div class="stat-item"><span class="stat-value" data-my-shots>0</span><span class="stat-unit">SHOTS</span></div>
</div>
</div>
<div class="start-card">
<h2 class="start-title">Ring of Fire</h2>
<p class="start-sub">52 cards, 13 rules, one King's Cup nobody wants. Everyone in the room sees every card.</p>
<form hx-post="{base_path}/room/{code}/game/start" hx-swap="none">
<select name="preset_id">{options}</select>
<button type="submit" class="btn-primary">START</button>
</form>
<a class="presets-link" href="{base_path}/presets">Edit rule presets &rarr;</a>
</div>
</div>"#
    )
}

/// Shared 2x2 superlatives grid for the phone over panel and the screen
/// over panel — MOST DRAWS / MOST SHOTS / ROOM TOTAL / KING'S CUP.
fn superla_grid(s: &GameSummary) -> String {
    let (draws_name, draws_val) = s
        .hardest
        .clone()
        .unwrap_or_else(|| ("Nobody".to_string(), 0));
    let (shots_name, shots_val) = s
        .most_shots
        .clone()
        .unwrap_or_else(|| ("Nobody".to_string(), 0));
    let kings_name = s.kings_cup.clone().unwrap_or_else(|| "Nobody".to_string());
    format!(
        r#"<div class="superla-grid">
<div class="superla-cell"><span class="superla-label">MOST DRAWS</span><span class="superla-name">{}</span><span class="superla-value">{} draws</span></div>
<div class="superla-cell"><span class="superla-label">MOST SHOTS</span><span class="superla-name">{}</span><span class="superla-value">{} shots</span></div>
<div class="superla-cell"><span class="superla-label">ROOM TOTAL</span><span class="superla-name">{}</span><span class="superla-value">drinks logged</span></div>
<div class="superla-cell"><span class="superla-label">KING'S CUP</span><span class="superla-name">{}</span><span class="superla-value">poured</span></div>
</div>"#,
        html_escape(&draws_name),
        draws_val,
        html_escape(&shots_name),
        shots_val,
        s.room_total,
        html_escape(&kings_name),
    )
}

/// The live game panel (phone GAME tab): announcement, deck bar, hero card
/// (+ Jack rule form when pending), TAP TO DRAW, IN HAND strip, end-early.
/// Fragment root carries `data-anim-key` for the client's flip-replay gate.
pub fn game_active_panel(view: &GameView) -> String {
    let base_path = view.base_path;
    let code = view.code;

    let announce = view
        .announcement
        .as_deref()
        .map(|a| {
            format!(
                r#"<div class="announce" data-anim="pop">{}</div>"#,
                html_escape(a)
            )
        })
        .unwrap_or_default();

    let deck_pct = if view.remaining <= 0 {
        0.0
    } else {
        (view.remaining as f64 / 52.0) * 100.0
    };
    let deck_row = format!(
        r#"<div class="deck-row"><div class="deck-bar"><div class="deck-fill" style="width:{deck_pct:.1}%"></div></div><span class="deck-left">{} LEFT</span></div>"#,
        view.remaining,
    );

    let (hero, rule_form) = match &view.current {
        Some(c) => {
            let hero = format!(
                r#"<div class="hero-card" data-anim="flip">{}<div class="hero-info"><span class="rule-kicker" data-me-text="YOU DREW" data-player-id="{}">{} DREW</span><h2 class="rule-title">{}</h2><p class="rule-text">{}</p></div></div>"#,
                card_face_html(c.card),
                c.drawer_id,
                html_escape(&c.drawer),
                html_escape(&c.title),
                html_escape(&c.text),
            );
            let rule_form = if c.pending_rule {
                format!(
                    r#"<div class="rule-input-row" data-show-player="{}" hidden><form hx-post="{base_path}/room/{code}/game/rule" hx-swap="none"><input type="hidden" name="draw_id" value="{}"><input type="text" name="text" maxlength="200" required placeholder="Your rule for the rest of the night"><button type="submit" class="btn-primary">SET</button></form></div>"#,
                    c.drawer_id, c.draw_id,
                )
            } else {
                String::new()
            };
            (hero, rule_form)
        }
        None => (String::new(), String::new()),
    };

    let held = if view.held.is_empty() {
        String::new()
    } else {
        let items: String = view
            .held
            .iter()
            .map(|h| {
                let red = if h.card.suit.is_red() { " card-red" } else { "" };
                format!(
                    r#"<li class="held-card"><div class="held-face{red}">{}{}</div><div class="held-info"><span class="held-title">{}</span><span class="held-holder">{}</span></div><button class="use-btn" data-show-player="{}" hidden hx-post="{base_path}/room/{code}/game/spend" hx-vals='{{"draw_id":{}}}' hx-swap="none" data-sound="card-use">USE</button></li>"#,
                    h.card.rank_label(),
                    h.card.suit.glyph(),
                    html_escape(&h.title),
                    html_escape(&h.holder_name),
                    h.holder_id,
                    h.draw_id,
                )
            })
            .collect();
        format!(
            r#"<div class="held-section"><span class="held-label">IN HAND</span><ul class="held-strip">{items}</ul></div>"#
        )
    };

    let draw_btn = format!(
        r#"<button class="btn-draw" hx-post="{base_path}/room/{code}/game/draw" hx-swap="none" data-sound="card-draw"><span class="btn-draw-label">TAP TO DRAW</span><span class="btn-draw-sub">FREE FOR ALL &middot; ANYONE CAN PULL</span></button>"#
    );

    let end_btn = format!(
        r#"<button class="btn-ghost" hx-post="{base_path}/room/{code}/game/end" hx-swap="none" hx-confirm="End the game for everyone?">End game early</button>"#
    );

    format!(
        r#"<div class="game-active" data-anim-key="{}">{announce}{deck_row}{hero}{rule_form}{draw_btn}{held}{end_btn}</div>"#,
        view.anim_key,
    )
}

/// Post-game summary: GAME OVER header, HARDEST HIT hero, 2x2 superlatives
/// grid, surviving house rules. The caller appends the idle panel below so
/// STARTing a new deck is one tap away.
pub fn game_over_panel(s: &GameSummary) -> String {
    let (hardest_name, hardest_draws) = s
        .hardest
        .clone()
        .unwrap_or_else(|| ("Nobody".to_string(), 0));
    let initial = hardest_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let hero = format!(
        r#"<div class="summary-hero"><span class="summary-initial">{}</span><div class="summary-body"><span class="summary-label">HARDEST HIT</span><span class="summary-name">{}</span><span class="summary-line">{} draws</span></div></div>"#,
        html_escape(&initial),
        html_escape(&hardest_name),
        hardest_draws,
    );
    let grid = superla_grid(s);
    let rules = if s.house_rules.is_empty() {
        String::new()
    } else {
        let items: String = s
            .house_rules
            .iter()
            .map(|r| {
                format!(
                    r#"<li class="rules-list-item"><span class="rule-jack">J</span><span class="rule-text-line">{}</span></li>"#,
                    html_escape(&r.text),
                )
            })
            .collect();
        format!(
            r#"<div class="rules-survived"><span class="rules-label">RULES THAT SURVIVED THE NIGHT</span><ul class="rules-list">{items}</ul></div>"#
        )
    };
    format!(
        r#"<div class="game-over"><span class="over-kicker">DECK EMPTY &middot; NIGHT LOGGED</span><h2 class="over-title">GAME OVER</h2>{hero}{grid}{rules}</div>"#
    )
}

/// Big-screen left pane, no game running.
pub fn screen_panel_idle(_code: &str) -> String {
    r#"<div class="screen-panel screen-idle"><div class="screen-hero"><span class="screen-kicker">NO GAME RUNNING</span><h2 class="screen-hero-title">Just drinking.</h2><p class="screen-hero-sub">Somebody start Ring of Fire on their phone and this screen becomes the table.</p></div><div class="screen-footer"><div class="kings-fill" style="height:0%"></div><span class="screen-footer-rules">No house rules yet.</span></div></div>"#.to_string()
}

/// Big-screen left pane, game running: display-scale hero card + rule,
/// HELD RIGHT NOW strip, footer (King's Cup fill + house-rules one-liner).
/// `rules`/`kings` are passed separately from `view` because the screen
/// footer needs the *current* full rule set and King count, not anything
/// carried on GameView.
pub fn screen_panel_active(view: &GameView, rules: &[HouseRule], kings: i64) -> String {
    let top = format!(
        r#"<div class="screen-top"><span class="live-dot"></span><span class="screen-kicker">ON THE TABLE</span><span class="screen-remaining">{} of 52 left</span></div>"#,
        view.remaining,
    );
    let hero = match &view.current {
        Some(c) => format!(
            r#"<div class="screen-hero" data-anim="flip">{}<div class="screen-hero-info"><span class="rule-kicker">{} drew</span><h2 class="rule-title">{}</h2><p class="rule-text">{}</p></div></div>"#,
            card_face_html(c.card),
            html_escape(&c.drawer),
            html_escape(&c.title),
            html_escape(&c.text),
        ),
        None => String::new(),
    };
    let held = if view.held.is_empty() {
        String::new()
    } else {
        let items: String = view
            .held
            .iter()
            .map(|h| {
                let red = if h.card.suit.is_red() { " card-red" } else { "" };
                format!(
                    r#"<div class="held-card"><div class="held-face{red}">{}{}</div><div class="held-info"><span class="held-holder">{}</span><span class="held-title">{}</span></div></div>"#,
                    h.card.rank_label(),
                    h.card.suit.glyph(),
                    html_escape(&h.holder_name),
                    html_escape(&h.title),
                )
            })
            .collect();
        format!(
            r#"<div class="screen-held"><span class="screen-held-label">HELD RIGHT NOW</span><div class="screen-held-strip">{items}</div></div>"#
        )
    };
    let kings_pct = (kings.clamp(0, 4) as f64 / 4.0) * 100.0;
    let rules_line = if rules.is_empty() {
        "No house rules yet.".to_string()
    } else {
        rules
            .iter()
            .map(|r| html_escape(&r.text))
            .collect::<Vec<_>>()
            .join(" &middot; ")
    };
    let footer = format!(
        r#"<div class="screen-footer"><div class="kings-fill" style="height:{kings_pct:.1}%"></div><span class="screen-footer-rules">{rules_line}</span></div>"#
    );
    format!(
        r#"<div class="screen-panel screen-active" data-anim-key="{}">{top}{hero}{held}{footer}</div>"#,
        view.anim_key,
    )
}

/// Big-screen left pane, game over: "{name} lost." + superlatives grid.
/// Prefers the King's Cup drawer as the "loser" headline (the traditional
/// Ring of Fire punishment); falls back to the hardest-hit player when no
/// King was drawn.
pub fn screen_panel_over(s: &GameSummary) -> String {
    let name = s
        .kings_cup
        .clone()
        .or_else(|| s.hardest.clone().map(|(n, _)| n))
        .unwrap_or_else(|| "Nobody".to_string());
    let line = match &s.hardest {
        Some((n, d)) => format!("{} took {d} cards to get there.", html_escape(n)),
        None => "Nobody drew a card.".to_string(),
    };
    let grid = superla_grid(s);
    format!(
        r#"<div class="screen-panel screen-over"><span class="screen-kicker">DECK EMPTY</span><h2 class="screen-hero-title">{} lost.</h2><p class="screen-hero-sub">{line}</p>{grid}</div>"#,
        html_escape(&name),
    )
}

/// Phone ROOM/TABLE tab: starts with a `<template data-topbar>` the client
/// copies into the shell's top bar, then the room-code card, WHO'S HERE
/// grid, HOUSE RULES list, King's Cup fill, and the end-the-night form.
/// Root `<div>` carries `data-mode` so the client can rename tab 3
/// (ROOM ↔ TABLE) per the fragment contract.
pub fn room_panel(view: &RoomView) -> String {
    let base_path = view.base_path;
    let code = view.code;
    let n = view.members.len();

    let topbar_chips: String = view
        .members
        .iter()
        .map(|m| {
            let initial = m
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            format!(
                r#"<span class="member-chip">{}</span>"#,
                html_escape(&initial)
            )
        })
        .collect();
    let topbar_count_label = if view.mode == "three_man" {
        format!("{n} at the table")
    } else {
        format!("{n} here")
    };
    let topbar = format!(
        r#"<template data-topbar><div class="topbar-chips">{topbar_chips}</div><span class="topbar-count">{topbar_count_label}</span></template>"#
    );

    let room_code_card = format!(
        r#"<div class="room-code-card"><span class="room-code-label">ROOM CODE</span><span class="room-code-value">{code}</span><div class="room-code-actions"><button type="button" class="room-action-btn" data-share>SHARE LINK</button><a class="room-action-btn" href="{base_path}/room/{code}/screen" target="_blank">OPEN BIG SCREEN</a></div></div>"#
    );

    let member_grid: String = view
        .members
        .iter()
        .map(|m| {
            let initial = m.name.chars().next().unwrap_or('?').to_uppercase().to_string();
            format!(
                r#"<div class="member-chip-row"><span class="member-chip">{}</span><span class="member-name">{}</span><span class="member-dot"></span></div>"#,
                html_escape(&initial),
                html_escape(&m.name),
            )
        })
        .collect();
    let member_section = format!(
        r#"<div class="member-section"><span class="member-section-label">WHO'S HERE &middot; {n}</span><div class="member-grid">{member_grid}</div></div>"#
    );

    let rules_items: String = view
        .house_rules
        .iter()
        .map(|r| {
            format!(
                r#"<li class="rules-list-item"><span class="rule-jack">J</span><div class="rule-body"><span class="rule-text-line">{}</span><span class="rule-byline" data-me-text="your rule" data-player-id="{}">{}'s rule</span></div></li>"#,
                html_escape(&r.text),
                r.player_id,
                html_escape(&r.player_name),
            )
        })
        .collect();
    let rules_section = format!(
        r#"<div class="rules-section"><span class="rules-section-label">HOUSE RULES</span><ul class="rules-list">{rules_items}</ul></div>"#
    );

    let kings_pct = (view.kings.clamp(0, 4) as f64 / 4.0) * 100.0;
    let kings_pips: String = (1..=4)
        .map(|n| {
            let filled = if n <= view.kings {
                " kings-pip-filled"
            } else {
                ""
            };
            format!(r#"<span class="kings-pip{filled}"></span>"#)
        })
        .collect();
    let kings_section = format!(
        r#"<div class="kings-section"><div class="kings-track"><div class="kings-fill" style="height:{kings_pct:.1}%"></div></div><div class="kings-info"><span class="kings-title">King's Cup</span><span class="kings-count">{} / 4</span><div class="kings-pips">{kings_pips}</div></div></div>"#,
        view.kings,
    );

    let end_form = format!(
        r#"<form class="end-night-form" method="post" action="{base_path}/room/{code}/end" onsubmit="return confirm('End the night for everyone?')"><button type="submit" class="btn-danger">End the night for everyone</button></form>"#
    );

    format!(
        r#"{topbar}<div class="room-panel" data-mode="{}">{room_code_card}{member_section}{rules_section}{kings_section}{end_form}</div>"#,
        html_escape(view.mode),
    )
}

/// A scannable SVG QR code pointing at a room URL, styled to match the app's
/// text color so it drops straight into the screen page without a white box.
///
/// qrcode's svg renderer always prefixes its output with an
/// `<?xml version="1.0" ...?>` declaration, which is invalid to inline
/// directly into an HTML document (and unnecessary — we're embedding this
/// `<svg>` as a fragment, not serving it as a standalone .svg file). Strip
/// it so the returned string is a bare `<svg>...</svg>` fragment.
pub fn qr_svg(url: &str) -> String {
    use qrcode::render::svg;
    let raw = qrcode::QrCode::new(url.as_bytes())
        .expect("qr encode")
        .render::<svg::Color>()
        .quiet_zone(false)
        .min_dimensions(160, 160)
        .dark_color(svg::Color("#f2eef8"))
        .light_color(svg::Color("transparent"))
        .build();
    match raw.find("<svg") {
        Some(idx) => raw[idx..].to_string(),
        None => raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Card, Suit};
    use crate::models::{DrawCount, RulePreset};

    #[test]
    fn test_escape() {
        assert_eq!(html_escape("<b>&\"'"), "&lt;b&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn test_card_face_marks_red_suits() {
        let html = card_face_html(Card {
            rank: 12,
            suit: Suit::Hearts,
        });
        assert!(html.contains("card-big"));
        assert!(html.contains("Q"));
        assert!(html.contains("\u{2665}"));
        assert!(html.contains("card-red"));
        let html = card_face_html(Card {
            rank: 1,
            suit: Suit::Spades,
        });
        assert!(!html.contains("card-red"));
    }

    fn preset(id: i64, name: &str) -> RulePreset {
        RulePreset {
            id,
            name: name.into(),
            rules_json: crate::rules::standard_rules_json(),
            created_at: "2026-07-29".into(),
        }
    }

    #[test]
    fn test_leaderboard_rows_carry_data_attrs() {
        let rows = vec![LeaderboardRow {
            id: 7,
            name: "<x>".into(),
            drinks: 2,
            shots: 1,
        }];
        let html = leaderboard_items(&rows);
        assert!(html.contains(r#"data-player-id="7""#));
        assert!(html.contains(r#"data-drinks="2""#));
        assert!(html.contains(r#"data-rank="1""#));
        assert!(html.contains("&lt;x&gt;"));
    }

    #[test]
    fn test_leaderboard_empty_branch() {
        let html = leaderboard_items(&[]);
        assert!(html.contains("Nobody here yet"));
        assert!(html.contains("lb-empty"));
    }

    #[test]
    fn test_idle_panel_has_stat_card_and_start() {
        let html = game_idle_panel("/drinks", "QK4M", &[preset(1, "Standard")]);
        assert!(html.contains("data-my-drinks"));
        assert!(html.contains("data-my-shots"));
        assert!(html.contains("<select"));
        assert!(html.contains(">START<"));
        assert!(html.contains("/drinks/presets"));
    }

    #[test]
    fn test_idle_panel_escapes_preset_names() {
        let html = game_idle_panel("/drinks", "QK4M", &[preset(1, "<Wild>")]);
        assert!(html.contains("&lt;Wild&gt;"));
        assert!(!html.contains("<Wild>"));
    }

    /// Jack drawn, pending rule, one held Thumb Master. holder_id (2)
    /// deliberately differs from drawer_id (1) so the two data-show-player
    /// assertions can't collapse into the same match.
    fn jack_view<'a>(code: &'a str, counts: &'a [DrawCount]) -> GameView<'a> {
        GameView {
            base_path: "/drinks",
            code,
            current: Some(CurrentCard {
                card: Card {
                    rank: 11,
                    suit: Suit::Spades,
                },
                title: "Make a Rule".into(),
                text: "Invent a rule for the rest of the game.".into(),
                drawer: "alice".into(),
                drawer_id: 1,
                draw_id: 3,
                pending_rule: true,
            }),
            remaining: 49,
            // Held card is a second, still-unspent draw — together with the
            // Jack (draw 3) and one earlier spent draw, that's 3 draws total,
            // 1 spend, matching anim_key "3-1" below.
            held: vec![HeldCardView {
                draw_id: 2,
                holder_id: 2,
                holder_name: "bob".into(),
                card: Card {
                    rank: 7,
                    suit: Suit::Hearts,
                },
                title: "Heaven".into(),
            }],
            counts,
            announcement: None,
            anim_key: "3-1".into(),
        }
    }

    #[test]
    fn test_active_panel_contract() {
        let counts = vec![DrawCount {
            name: "alice".into(),
            draws: 3,
        }];
        let view = jack_view("QK4M", &counts);
        let html = game_active_panel(&view);
        assert!(html.contains(r#"data-anim-key="3-1""#));
        assert!(html.contains(r#"data-anim="flip""#));
        // Jack input revealed only on the drawer's phone:
        assert!(html.contains(&format!(r#"data-show-player="{}" hidden"#, 1)));
        assert!(html.contains("/drinks/room/QK4M/game/rule"));
        // USE button per personalization contract:
        assert!(html.contains(r#"data-show-player="2" hidden"#));
        assert!(html.contains("TAP TO DRAW"));
        assert!(html.contains(r#"data-sound="card-draw""#));
        assert!(html.contains(r#"data-sound="card-use""#));
    }

    #[test]
    fn test_active_panel_non_jack_has_no_rule_input() {
        let counts: Vec<DrawCount> = vec![];
        let view = GameView {
            base_path: "/drinks",
            code: "QK4M",
            current: Some(CurrentCard {
                card: Card {
                    rank: 5,
                    suit: Suit::Clubs,
                },
                title: "Thumb Master".into(),
                text: "Hold this card.".into(),
                drawer: "alice".into(),
                drawer_id: 1,
                draw_id: 1,
                pending_rule: false,
            }),
            remaining: 51,
            held: vec![],
            counts: &counts,
            announcement: None,
            anim_key: "1-0".into(),
        };
        let html = game_active_panel(&view);
        assert!(!html.contains("/drinks/room/QK4M/game/rule"));
        assert!(!html.contains("rule-input-row"));
    }

    /// Game just started, nobody has drawn yet: no hero card, no rule form,
    /// but the deck bar and TAP TO DRAW button still render.
    #[test]
    fn test_active_panel_pre_first_draw_has_no_hero() {
        let counts: Vec<DrawCount> = vec![];
        let view = GameView {
            base_path: "/drinks",
            code: "QK4M",
            current: None,
            remaining: 52,
            held: vec![],
            counts: &counts,
            announcement: None,
            anim_key: "0-0".into(),
        };
        let html = game_active_panel(&view);
        assert!(!html.contains("hero-card"));
        assert!(!html.contains(r#"data-anim="flip""#));
        assert!(html.contains("52 LEFT"));
        assert!(html.contains("TAP TO DRAW"));
    }

    #[test]
    fn test_over_panel_superlatives() {
        let summary = GameSummary {
            hardest: Some(("alice".into(), 14)),
            most_shots: Some(("<bob>".into(), 5)),
            room_total: 22,
            kings_cup: Some("carol".into()),
            counts: vec![
                DrawCount {
                    name: "alice".into(),
                    draws: 14,
                },
                DrawCount {
                    name: "<bob>".into(),
                    draws: 9,
                },
            ],
            house_rules: vec![HouseRule {
                id: 1,
                draw_id: 3,
                player_id: 1,
                player_name: "alice".into(),
                text: "No pointing".into(),
            }],
        };
        let html = game_over_panel(&summary);
        assert!(html.contains("GAME OVER"));
        assert!(html.contains("HARDEST HIT"));
        assert!(html.contains("MOST DRAWS") && html.contains("alice"));
        assert!(html.contains("MOST SHOTS") && html.contains("bob"));
        assert!(html.contains("ROOM TOTAL") && html.contains("22"));
        assert!(html.contains("KING") && html.contains("No pointing"));
        assert!(html.contains("carol"));
        assert!(html.contains("&lt;bob&gt;"));
    }

    #[test]
    fn test_room_panel_topbar_and_mode() {
        let members = vec![
            RoomMember {
                id: 1,
                name: "alice".into(),
                joined_at: "t".into(),
            },
            RoomMember {
                id: 2,
                name: "bob".into(),
                joined_at: "t".into(),
            },
        ];
        let house_rules: Vec<HouseRule> = vec![];
        let view = RoomView {
            base_path: "/drinks",
            code: "QK4M",
            members: &members,
            house_rules: &house_rules,
            kings: 1,
            mode: "idle",
        };
        let html = room_panel(&view);
        assert!(html.starts_with("<template data-topbar>"));
        assert!(html.contains(r#"data-mode="idle""#));
        assert!(html.contains("WHO"));
        assert!(html.contains("OPEN BIG SCREEN"));
        assert!(html.contains("kings-fill"));
        assert!(html.contains("1 / 4"));
        assert_eq!(html.matches("<span class=\"kings-pip\">").count(), 3);
        assert_eq!(
            html.matches("<span class=\"kings-pip kings-pip-filled\">")
                .count(),
            1
        );
    }

    #[test]
    fn test_screen_panels() {
        let idle = screen_panel_idle("QK4M");
        assert!(idle.contains("Just drinking."));

        let counts = vec![DrawCount {
            name: "alice".into(),
            draws: 3,
        }];
        let view = jack_view("QK4M", &counts);
        let rules: Vec<HouseRule> = vec![];
        let active = screen_panel_active(&view, &rules, 0);
        assert!(active.contains("card-big"));
        assert!(active.contains("HELD RIGHT NOW"));
        assert!(active.contains("screen-footer"));

        let summary = GameSummary {
            hardest: Some(("alice".into(), 30)),
            most_shots: None,
            room_total: 40,
            kings_cup: Some("alice".into()),
            counts: vec![],
            house_rules: vec![],
        };
        let over = screen_panel_over(&summary);
        assert!(over.contains("lost"));
    }

    #[test]
    fn test_preset_rows_link_and_delete() {
        let html = preset_rows("/drinks", &[preset(3, "House <1>")]);
        assert!(html.contains(r#"href="/drinks/presets/3""#));
        assert!(html.contains("House &lt;1&gt;"));
        assert!(html.contains(r#"action="/drinks/presets/3/delete""#));
    }

    #[test]
    fn test_preset_edit_rows_cover_all_13_ranks() {
        let html = preset_edit_rows(&crate::rules::standard_rules());
        for n in 1..=13 {
            assert!(html.contains(&format!(r#"name="title_{n}""#)));
            assert!(html.contains(&format!(r#"name="text_{n}""#)));
            assert!(html.contains(&format!(r#"name="holdable_{n}""#)));
        }
        assert!(html.contains("<legend>A</legend>"));
        assert!(html.contains("<legend>K</legend>"));
        // Holdables (5, 7) come back pre-checked.
        assert_eq!(html.matches("checked").count(), 2);
    }

    #[test]
    fn test_qr_svg_renders() {
        let svg = qr_svg("https://example.com/drinks/room/QK4M");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("f2eef8")); // dark modules in text color
    }
}
