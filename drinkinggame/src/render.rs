//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.
//!
//! Every broadcast fragment is identical for every viewer — per-viewer
//! differences are expressed only through the data-attribute contract
//! (`data-show-player` / `data-hide-player` / `data-me-text`, see
//! `fragment-contract.md`), never by branching on a viewer id here.

use crate::cards::Card;
use crate::models::{DrawCount, HouseRule, LeaderboardRow, RoomMember, RulePreset};
use crate::three_man::{GiveMode, Phase};

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
    leaderboard_items_tm(rows, None)
}

/// Same as `leaderboard_items`, but when a 3 Man game is active the current
/// 3 Man's row gets a "3 MAN" badge span (`leaderboard_items` delegates here
/// with `None`, which renders identically to before).
pub fn leaderboard_items_tm(rows: &[LeaderboardRow], three_man: Option<i64>) -> String {
    if rows.is_empty() {
        return r#"<li class="lb-empty">Nobody here yet</li>"#.to_string();
    }
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let rank = i + 1;
            let badge = if three_man == Some(r.id) {
                r#"<span class="tm-chip">3 MAN</span>"#
            } else {
                ""
            };
            format!(
                r#"<li class="lb-row" data-player-id="{id}" data-drinks="{drinks}" data-shots="{shots}" data-rank="{rank}"><span class="lb-rank">{rank}</span><span class="lb-name">{name}</span>{badge}<span class="lb-counts">{drinks} D &middot; {shots} S</span></li>"#,
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
    /// Pre-rendered by `tm_seating_html` (Task 11) when a 3 Man game is
    /// active; `None` for idle/Ring of Fire. Slots between WHO'S HERE and
    /// HOUSE RULES — the seating block replaces nothing.
    pub seating: Option<String>,
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
    let tm_chip = if view.mode == "three_man" {
        r#"<span class="tm-chip">3 MAN</span>"#
    } else {
        ""
    };
    let topbar = format!(
        r#"<template data-topbar><div class="topbar-chips">{topbar_chips}</div>{tm_chip}<span class="topbar-count">{topbar_count_label}</span></template>"#
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

    // Deliberate TABLE-tab deviation from the prototype: seating slots
    // between WHO'S HERE and HOUSE RULES rather than replacing either, so
    // the room-code card and share/big-screen actions stay reachable
    // mid-game.
    let seating_block = view.seating.as_deref().unwrap_or_default();

    format!(
        r#"{topbar}<div class="room-panel" data-mode="{}">{room_code_card}{member_section}{seating_block}{rules_section}{kings_section}{end_form}</div>"#,
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
///
/// `url` embeds the request's `Host` header (see `request_origin` in
/// routes.rs) — attacker-controlled, unbounded length. `QrCode::new` errors
/// once the payload exceeds byte-mode capacity (~2.3KB at the default
/// EcLevel::M), so this degrades to an empty string instead of unwrapping:
/// a request task panicking on an oversized Host header would drop the
/// connection instead of just rendering a screen page with no QR code.
pub fn qr_svg(url: &str) -> String {
    use qrcode::render::svg;
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return String::new();
    };
    let raw = code
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

// ---------------------------------------------------------------------
// 3 Man (Task 11) — phone GAME/TABLE tab, big-screen panel. Renders
// `crate::three_man::ThreeManState`; never mutates it. Per-viewer
// differences go through the same data-attribute contract as Ring of
// Fire: `data-show-player` (server pre-hides with the `hidden` attribute),
// `data-hide-player` (default visible), `data-me-text`+`data-player-id`.
// ---------------------------------------------------------------------

pub struct TmView<'a> {
    pub base_path: &'a str,
    pub code: &'a str,
    pub st: &'a crate::three_man::ThreeManState,
    pub names: &'a std::collections::HashMap<i64, String>,
}

fn tm_name(v: &TmView, id: i64) -> String {
    html_escape(v.names.get(&id).map(|s| s.as_str()).unwrap_or("?"))
}

fn tm_initial(v: &TmView, id: i64) -> String {
    let n = v.names.get(&id).map(|s| s.as_str()).unwrap_or("?");
    html_escape(&n.chars().next().unwrap_or('?').to_uppercase().to_string())
}

/// Standard 3x3 pip layout per die face (row-major positions 0..9).
fn die_pip_positions(v: u8) -> &'static [usize] {
    match v {
        1 => &[4],
        2 => &[0, 8],
        3 => &[0, 4, 8],
        4 => &[0, 2, 6, 8],
        5 => &[0, 2, 4, 6, 8],
        6 => &[0, 2, 3, 5, 6, 8],
        _ => &[],
    }
}

/// Nine cells per die, in grid order — filled positions get `.die-pip`,
/// empty ones get an unstyled placeholder so the 3x3 grid alignment holds.
fn die_face_html(value: u8) -> String {
    let filled = die_pip_positions(value);
    (0..9)
        .map(|i| {
            if filled.contains(&i) {
                r#"<span class="die-pip"></span>"#.to_string()
            } else {
                r#"<span class="die-cell"></span>"#.to_string()
            }
        })
        .collect()
}

/// Two `.die` pip grids, each individually `data-anim="tumble"` so the
/// client's anim-key gate (see `swapPanel` in room.html) replays the
/// tumble on every new roll.
pub fn dice_html(d1: u8, d2: u8) -> String {
    format!(
        r#"<div class="die" data-anim="tumble">{}</div><div class="die" data-anim="tumble">{}</div>"#,
        die_face_html(d1),
        die_face_html(d2),
    )
}

/// One `.seat` per seat in `st.order`, tag precedence ROLLING > ←7 > 9→ >
/// 3 MAN. Shared by the phone seat strip and the screen bottom strip —
/// `extra_cls`/`label`/`caption` are the only things that differ between
/// them.
fn tm_seat_strip_html(v: &TmView, extra_cls: &str, label: &str, caption: &str) -> String {
    let st = v.st;
    let len = st.order.len();
    let order_csv = st
        .order
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let seats: String = st
        .order
        .iter()
        .enumerate()
        .map(|(idx, &id)| {
            let (cls, tag) = if idx == st.roller_idx {
                (" seat-rolling", "ROLLING")
            } else if idx == (st.roller_idx + 1) % len {
                (" seat-left7", "\u{2190}7")
            } else if idx == (st.roller_idx + len - 1) % len {
                (" seat-right9", "9\u{2192}")
            } else if id == st.three_man {
                (" seat-3man", "3 MAN")
            } else {
                ("", "")
            };
            let initial = tm_initial(v, id);
            let name = tm_name(v, id);
            format!(
                r#"<div class="seat{cls}"><span class="seat-tag">{tag}</span><span class="seat-avatar">{initial}</span><span class="seat-name">{name}</span></div>"#
            )
        })
        .collect();
    let roller = st.roller();
    let three_man = st.three_man;
    format!(
        r#"<div class="seat-strip{extra_cls}" data-order="{order_csv}" data-roller="{roller}" data-three-man="{three_man}"><div class="seat-strip-head"><span class="seat-strip-label">{label}</span><span class="seat-strip-caption">{caption}</span></div><div class="seat-strip-row">{seats}</div></div>"#
    )
}

fn tm_turn_banner(v: &TmView) -> String {
    let roller = v.st.roller();
    let name = tm_name(v, roller);
    format!(
        r#"<div class="turn-banner" data-anim="pop"><span class="turn-banner-dot"></span><span class="turn-banner-text" data-me-text="YOUR TURN" data-player-id="{roller}">{name} IS UP</span></div>"#
    )
}

fn tm_roll_button(v: &TmView) -> String {
    format!(
        r#"<button class="btn-draw" hx-post="{}/room/{}/tm/roll" hx-swap="none" data-sound="dice-roll"><span class="btn-draw-label">ROLL THE DICE</span><span class="btn-draw-sub">ANY PHONE CAN TAP</span></button>"#,
        v.base_path, v.code,
    )
}

fn tm_pass_button(v: &TmView) -> String {
    let next = tm_name(v, v.st.left_of(v.st.roller_idx));
    format!(
        r#"<button class="btn-pass" hx-post="{}/room/{}/tm/pass" hx-swap="none">PASS TO {next}</button>"#,
        v.base_path, v.code,
    )
}

/// Dice pips + sum + call rows (or the dashed nobody-drinks box). Rendered
/// whenever `st.dice` is `Some` — Ready-with-`stale` shows the previous
/// roll dimmed, every other phase shows the current one at full opacity.
fn tm_verdict_card(v: &TmView) -> Option<String> {
    let st = v.st;
    let (d1, d2) = st.dice?;
    let sum = d1 + d2;
    let stale_cls = if st.stale { " stale" } else { "" };
    let caption = if st.stale {
        let last = tm_name(v, st.last_roller.unwrap_or_else(|| st.roller()));
        format!("LAST ROLL &middot; {last}")
    } else {
        "ROLLED".to_string()
    };
    let calls_html = if st.calls.is_empty() {
        r#"<div class="nobody-box">Nothing. Nobody drinks. Pass it on.</div>"#.to_string()
    } else {
        st.calls
            .iter()
            .map(|c| {
                let initial = tm_initial(v, c.player_id);
                let name = tm_name(v, c.player_id);
                let amt = c.amount;
                let pid = c.player_id;
                let reason = html_escape(&c.reason);
                format!(
                    r#"<div class="call-row"><span class="call-avatar">{initial}</span><div class="call-body"><span class="call-headline" data-me-text="You drink {amt}" data-player-id="{pid}">{name} drinks {amt}</span><span class="call-reason">{reason}</span></div><span class="call-amount">{amt}</span></div>"#
                )
            })
            .collect()
    };
    let dice = dice_html(d1, d2);
    Some(format!(
        r#"<div class="verdict-card{stale_cls}" data-anim="pop"><div class="dice-row">{dice}<div class="dice-sum"><span class="dice-sum-value">{sum}</span><span class="dice-sum-caption">{caption}</span></div></div><div class="call-list">{calls_html}</div></div>"#
    ))
}

/// HandOff-only: roller-only picker (grid of members minus the current 3
/// Man) + a spectator banner for everyone else.
fn tm_handoff_block(v: &TmView) -> String {
    let st = v.st;
    if st.phase != Phase::HandOff {
        return String::new();
    }
    let roller = st.roller();
    let roller_name = tm_name(v, roller);
    let targets: String = st
        .order
        .iter()
        .filter(|&&id| id != st.three_man)
        .map(|&id| {
            let initial = tm_initial(v, id);
            let name = tm_name(v, id);
            format!(
                r#"<button class="handoff-btn" hx-post="{}/room/{}/tm/three-man" hx-vals='{{"target":{id}}}' hx-swap="none"><span class="handoff-btn-initial">{initial}</span><span class="handoff-btn-name">{name}</span></button>"#,
                v.base_path, v.code,
            )
        })
        .collect();
    format!(
        r#"<div class="handoff-panel" data-show-player="{roller}" hidden><span class="handoff-kicker">YOU ROLLED A THREE &middot; PASS IT ON</span><h3 class="handoff-title">Who's 3 Man now?</h3><p class="handoff-sub">You don't drink for this one &mdash; you hand the title over. Every 3 from here is theirs.</p><div class="handoff-grid">{targets}</div></div><div class="handoff-spectator" data-hide-player="{roller}"><span class="handoff-spectator-tag">3 MAN</span><span class="handoff-spectator-text">{roller_name} is picking the next 3 Man&hellip;</span></div>"#
    )
}

/// Assign-only: owner-only mode choice → slot/target grid → SEND, plus a
/// spectator banner for everyone else.
fn tm_assign_block(v: &TmView) -> String {
    let st = v.st;
    if st.phase != Phase::Assign {
        return String::new();
    }
    let Some(double) = &st.double else {
        return String::new();
    };
    let owner = double.owner;
    let owner_name = tm_name(v, owner);
    let value = double.value;
    let base = v.base_path;
    let code = v.code;

    let inner = match double.mode {
        None => {
            let split_btn = if st.order.len() >= 3 {
                format!(
                    r#"<button class="mode-btn" hx-post="{base}/room/{code}/tm/mode" hx-vals='{{"mode":"split"}}' hx-swap="none"><span class="mode-btn-title">One die each to two people</span><span class="mode-btn-sub">Two different people, one die apiece.</span></button>"#
                )
            } else {
                String::new()
            };
            format!(
                r#"<div class="mode-row"><button class="mode-btn" hx-post="{base}/room/{code}/tm/mode" hx-vals='{{"mode":"both"}}' hx-swap="none"><span class="mode-btn-title">Both dice to one person</span><span class="mode-btn-sub">They roll two and drink the total.</span></button>{split_btn}</div>"#
            )
        }
        Some(_) => {
            let next_slot = double.slots.iter().position(|s| s.is_none());
            let slots_html: String = double
                .slots
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let n = i + 1;
                    match s {
                        Some(pid) => {
                            let name = tm_name(v, *pid);
                            format!(
                                r#"<button class="slot-cell slot-filled" hx-post="{base}/room/{code}/tm/clear-slot" hx-vals='{{"slot":{i}}}' hx-swap="none"><span class="slot-label">SLOT {n}</span><span class="slot-value">{name} &times;</span></button>"#
                            )
                        }
                        None => format!(
                            r#"<div class="slot-cell"><span class="slot-label">SLOT {n}</span><span class="slot-value">&mdash;</span></div>"#
                        ),
                    }
                })
                .collect();
            let targets_html = match next_slot {
                Some(slot) => st
                    .order
                    .iter()
                    .filter(|&&id| id != owner && !double.slots.contains(&Some(id)))
                    .map(|&id| {
                        let initial = tm_initial(v, id);
                        let name = tm_name(v, id);
                        format!(
                            r#"<button class="target-btn" hx-post="{base}/room/{code}/tm/target" hx-vals='{{"slot":{slot},"target":{id}}}' hx-swap="none"><span class="target-btn-initial">{initial}</span><span class="target-btn-name">{name}</span></button>"#
                        )
                    })
                    .collect::<String>(),
                None => String::new(),
            };
            let disabled = if next_slot.is_some() { " disabled" } else { "" };
            format!(
                r#"<div class="slot-grid">{slots_html}</div><div class="target-grid">{targets_html}</div><button class="btn-primary send-btn" hx-post="{base}/room/{code}/tm/send" hx-swap="none" data-sound="dice-give"{disabled}>SEND THE DICE</button>"#
            )
        }
    };

    format!(
        r#"<div class="assign-panel" data-show-player="{owner}" hidden><span class="assign-kicker">DOUBLE {value} &middot; GIVE THE DICE AWAY</span><p class="assign-sub">They drink whatever they roll. If a {value} comes back, you drink the combined total.</p>{inner}</div><div class="assign-spectator" data-hide-player="{owner}"><span class="assign-spectator-tag">DOUBLE {value}</span><span class="assign-spectator-text">{owner_name} is handing out the dice&hellip;</span></div>"#
    )
}

/// Gifts-only: one ROLL button per pending gift (visible on any phone —
/// gift rolls aren't gated to a single actor), rolled gifts show their
/// values, payback banner once `double.payback` is set.
fn tm_gifts_block(v: &TmView) -> String {
    let st = v.st;
    if st.phase != Phase::Gifts {
        return String::new();
    }
    let Some(double) = &st.double else {
        return String::new();
    };
    let base = v.base_path;
    let code = v.code;
    let rows: String = double
        .gifts
        .iter()
        .enumerate()
        .map(|(slot, g)| {
            let initial = tm_initial(v, g.player_id);
            let name = tm_name(v, g.player_id);
            let body = match &g.values {
                Some(values) => {
                    let total: u32 = values.iter().map(|&x| x as u32).sum();
                    let vals = values
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(r#"<span class="gift-status">rolled {vals} &middot; drinks {total}</span>"#)
                }
                None => {
                    let dice_count = g.dice_count;
                    format!(
                        r#"<span class="gift-status">waiting to roll</span><button class="gift-roll-btn" hx-post="{base}/room/{code}/tm/gift-roll" hx-vals='{{"slot":{slot}}}' hx-swap="none" data-sound="dice-roll">ROLL {dice_count} DICE</button>"#
                    )
                }
            };
            format!(
                r#"<div class="gift-row"><span class="gift-avatar">{initial}</span><div class="gift-body"><span class="gift-name">{name}</span>{body}</div></div>"#
            )
        })
        .collect();
    let payback = match double.payback {
        Some(total) => {
            let owner_name = tm_name(v, double.owner);
            format!(
                r#"<div class="payback-banner">PAYBACK &mdash; {owner_name} drinks {total}</div>"#
            )
        }
        None => String::new(),
    };
    let value = double.value;
    let owner_name = tm_name(v, double.owner);
    format!(
        r#"<div class="gifts-panel"><span class="gifts-kicker">DOUBLE {value} FROM {owner_name}</span>{rows}{payback}</div>"#
    )
}

/// Phone GAME tab while a 3 Man game is active. Root carries
/// `data-anim-key="{seq}"`. Always: turn banner + exposure-line hook +
/// seat strip. Phase-specific: Ready → ROLL (+ stale verdict below);
/// Rolled/HandOff/Assign/Gifts → verdict card + the matching phase block;
/// PASS shows whenever `pass()` would currently succeed.
pub fn tm_phone_panel(v: &TmView) -> String {
    let st = v.st;
    let turn_banner = tm_turn_banner(v);
    let seat_strip = tm_seat_strip_html(v, "", "SEATING &middot; CLOCKWISE", "LEFT = NEXT TO ROLL");
    let verdict = tm_verdict_card(v).unwrap_or_default();
    let handoff = tm_handoff_block(v);
    let assign = tm_assign_block(v);
    let gifts = tm_gifts_block(v);
    let roll_btn = if st.phase == Phase::Ready {
        tm_roll_button(v)
    } else {
        String::new()
    };
    let pass_allowed =
        st.phase == Phase::Rolled || (st.phase == Phase::Gifts && st.gifts_complete());
    let pass_btn = if pass_allowed {
        tm_pass_button(v)
    } else {
        String::new()
    };
    let end_btn = format!(
        r#"<button class="btn-ghost" hx-post="{}/room/{}/tm/end" hx-swap="none" hx-confirm="End the game for everyone?">End the game</button>"#,
        v.base_path, v.code,
    );
    let seq = st.seq;
    format!(
        r#"<div class="game-active" data-anim-key="{seq}">{turn_banner}<p id="exposure-line"></p>{seat_strip}{verdict}{handoff}{assign}{gifts}{roll_btn}{pass_btn}{end_btn}</div>"#
    )
}

/// Big-screen left pane + full-width bottom seat strip. The "WAITING ON"
/// full-pane state only appears before the first roll of the game
/// (`st.dice.is_none()`) — between rolls the dimmed stale verdict stays.
pub fn tm_screen_panel(v: &TmView) -> String {
    let st = v.st;
    let three_man_name = tm_name(v, st.three_man);
    let header = format!(
        r#"<div class="screen-top"><span class="live-dot"></span><span class="screen-kicker">3 MAN</span><span class="tm-chip">{three_man_name}</span></div>"#
    );

    let body = match st.dice {
        None => {
            let roller_name = tm_name(v, st.roller());
            format!(
                r#"<div class="screen-waiting"><span class="screen-kicker">WAITING ON</span><h2 class="screen-hero-title">{roller_name}</h2><p class="screen-hero-sub">Two dice. Three, seven, nine, eleven &mdash; or a double.</p></div>"#
            )
        }
        Some((d1, d2)) => {
            let sum = d1 + d2;
            let dimmed = if st.stale { " stale" } else { "" };
            let caption = if st.stale {
                let last = tm_name(v, st.last_roller.unwrap_or_else(|| st.roller()));
                format!("LAST ROLL &middot; {last}")
            } else {
                "ON THE TABLE".to_string()
            };
            let headline = if st.calls.is_empty() {
                "Nobody drinks.".to_string()
            } else {
                st.calls
                    .iter()
                    .map(|c| html_escape(&c.reason))
                    .collect::<Vec<_>>()
                    .join(" &middot; ")
            };
            let calls_html = if st.calls.is_empty() {
                r#"<div class="nobody-box">Nobody drinks. Pass it on.</div>"#.to_string()
            } else {
                st.calls
                    .iter()
                    .map(|c| {
                        let initial = tm_initial(v, c.player_id);
                        let name = tm_name(v, c.player_id);
                        let amt = c.amount;
                        let reason = html_escape(&c.reason);
                        format!(
                            r#"<div class="call-row"><span class="call-avatar">{initial}</span><div class="call-body"><span class="call-headline">{name} drinks {amt}</span><span class="call-reason">{reason}</span></div><span class="call-amount">{amt}</span></div>"#
                        )
                    })
                    .collect()
            };
            let phase_banner = match st.phase {
                Phase::HandOff => {
                    let roller_name = tm_name(v, st.roller());
                    format!(
                        r#"<div class="handoff-panel"><span class="handoff-kicker">HANDING OVER</span><span class="handoff-title">{roller_name} is picking a new 3 Man</span></div>"#
                    )
                }
                Phase::Assign => match &st.double {
                    Some(double) => {
                        let mode_label = match double.mode {
                            None => "choosing how".to_string(),
                            Some(GiveMode::Both) => "both dice to one person".to_string(),
                            Some(GiveMode::Split) => "one die each to two people".to_string(),
                        };
                        let value = double.value;
                        let owner_name = tm_name(v, double.owner);
                        format!(
                            r#"<div class="assign-panel"><span class="assign-kicker">DOUBLE {value}</span><span class="assign-title">{owner_name} is handing the dice out</span><span class="assign-mode-label">{mode_label}</span></div>"#
                        )
                    }
                    None => String::new(),
                },
                Phase::Gifts => match &st.double {
                    Some(double) => {
                        let rows: String = double
                            .gifts
                            .iter()
                            .map(|g| {
                                let initial = tm_initial(v, g.player_id);
                                let name = tm_name(v, g.player_id);
                                let (status, value) = match &g.values {
                                    Some(values) => {
                                        let total: u32 = values.iter().map(|&x| x as u32).sum();
                                        (format!("drinks {total}"), total.to_string())
                                    }
                                    None => ("rolling&hellip;".to_string(), "&mdash;".to_string()),
                                };
                                format!(
                                    r#"<div class="gift-row"><span class="gift-avatar">{initial}</span><div class="gift-body"><span class="gift-name">{name}</span><span class="gift-status">{status}</span></div><span class="gift-value">{value}</span></div>"#
                                )
                            })
                            .collect();
                        let payback = match double.payback {
                            Some(total) => {
                                let owner_name = tm_name(v, double.owner);
                                format!(
                                    r#"<div class="payback-banner">PAYBACK &mdash; {owner_name} drinks {total}</div>"#
                                )
                            }
                            None => String::new(),
                        };
                        let value = double.value;
                        let owner_name = tm_name(v, double.owner);
                        format!(
                            r#"<div class="gifts-panel"><span class="gifts-kicker">DOUBLE {value} from {owner_name}</span>{rows}{payback}</div>"#
                        )
                    }
                    None => String::new(),
                },
                Phase::Ready | Phase::Rolled => String::new(),
            };
            let dice = dice_html(d1, d2);
            format!(
                r#"<div class="verdict-card{dimmed}"><div class="dice-row">{dice}<div class="dice-sum"><span class="dice-sum-value">{sum}</span><span class="dice-sum-caption">{caption}</span><span class="dice-sum-headline">{headline}</span></div></div>{calls_html}</div>{phase_banner}"#
            )
        }
    };

    let seat_strip = tm_seat_strip_html(
        v,
        " seat-strip-screen",
        "THE TABLE &middot; CLOCKWISE",
        "7 hits the left &middot; 9 hits the right &middot; 11 hits the roller",
    );
    let seq = st.seq;
    format!(
        r#"<div class="screen-panel screen-tm" data-anim-key="{seq}">{header}{body}</div>{seat_strip}"#
    )
}

/// TABLE-tab seating list (↑/↓ move, per-row 3 MAN assign) + a static
/// rules-reference card set. `room_panel` embeds the result verbatim.
pub fn tm_seating_html(v: &TmView) -> String {
    let st = v.st;
    let base = v.base_path;
    let code = v.code;
    let roller = st.roller();
    let rows: String = st
        .order
        .iter()
        .enumerate()
        .map(|(idx, &id)| {
            let pos = idx + 1;
            let tag = if id == roller {
                "ROLLING"
            } else if id == st.three_man {
                "3 MAN"
            } else {
                ""
            };
            let initial = tm_initial(v, id);
            let name = tm_name(v, id);
            format!(
                r#"<div class="seat-move"><span class="seat-pos">{pos}</span><span class="seat-avatar">{initial}</span><div class="seat-info"><span class="seat-name-line">{name}</span><span class="seat-tag-line">{tag}</span></div><div class="seat-actions"><button class="seat-btn" hx-post="{base}/room/{code}/tm/seat" hx-vals='{{"target":{id},"dir":-1}}' hx-swap="none">&uarr;</button><button class="seat-btn" hx-post="{base}/room/{code}/tm/seat" hx-vals='{{"target":{id},"dir":1}}' hx-swap="none">&darr;</button><button class="seat-btn seat-btn-three" hx-post="{base}/room/{code}/tm/three-man" hx-vals='{{"target":{id}}}' hx-swap="none">3 MAN</button></div></div>"#
            )
        })
        .collect();

    let rule_cards = [
        ("3", "Three", "Every 3 (and a total of 3) hits the 3 Man &mdash; unless they rolled it, then the crown moves instead."),
        ("7", "Seven", "Left of the roller drinks."),
        ("9", "Nine", "Right of the roller drinks."),
        ("11", "Eleven", "The roller drinks."),
        ("=", "Doubles", "Give the dice away &mdash; both to one person, or split between two. A matching number coming back means you drink the total."),
    ];
    let cards: String = rule_cards
        .iter()
        .map(|(key, title, text)| {
            format!(
                r#"<div class="rules-ref-card"><span class="rules-ref-key">{key}</span><div class="rules-ref-body"><span class="rules-ref-title">{title}</span><span class="rules-ref-text">{text}</span></div></div>"#
            )
        })
        .collect();

    format!(
        r#"<div class="seating-list"><span class="seating-caption">SEATING ORDER</span><p class="seating-sub">7 and 9 hit the roller's neighbours, so this list has to match the actual room.</p>{rows}</div><div class="rules-ref"><span class="rules-ref-caption">THE RULES</span>{cards}</div>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Card, Suit};
    use crate::models::{DrawCount, RulePreset};
    use crate::three_man::ThreeManState;
    use std::collections::HashMap;

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
            seating: None,
        };
        let html = room_panel(&view);
        assert!(html.starts_with("<template data-topbar>"));
        assert!(html.contains(r#"data-mode="idle""#));
        assert!(!html.contains("tm-chip")); // no 3 MAN badge outside three_man mode
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

    /// `url` embeds an attacker-controlled `Host` header (via
    /// `request_origin` in routes.rs, unbounded length) — a payload past QR
    /// byte-mode capacity must degrade to an empty string, not panic and
    /// take the request task down with it.
    #[test]
    fn test_qr_svg_degrades_on_oversized_input() {
        let svg = qr_svg(&"x".repeat(5000));
        assert_eq!(svg, "");
    }

    // -------------------------------------------------------------
    // 3 Man (Task 11)
    // -------------------------------------------------------------

    /// 3 players alice(1)/bob(2)/cara(3), starter alice, three_man
    /// reassigned to bob — order [1,2,3], roller_idx 0 (alice), Ready.
    fn tm_view_fixture() -> (ThreeManState, HashMap<i64, String>) {
        let mut st = ThreeManState::new(vec![1, 2, 3], 1);
        st.set_three_man(2).unwrap();
        let names = HashMap::from([
            (1, "alice".to_string()),
            (2, "bob".to_string()),
            (3, "cara".to_string()),
        ]);
        (st, names)
    }

    #[test]
    fn test_tm_phone_ready_state() {
        let (st, names) = tm_view_fixture();
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains(r#"data-order="1,2,3""#));
        assert!(html.contains(r#"data-roller="1""#));
        assert!(html.contains(r#"data-three-man="2""#));
        assert!(html.contains(r#"data-anim-key="0""#));
        assert!(html.contains(r#"data-me-text="YOUR TURN" data-player-id="1""#));
        assert!(html.contains("/drinks/room/QK4M/tm/roll"));
        assert!(!html.contains("/tm/pass"));
        assert!(html.contains(r#"id="exposure-line""#));
        assert!(html.contains(r#"<div class="turn-banner" data-anim="pop">"#));
    }

    #[test]
    fn test_tm_phone_rolled_verdict_and_pass() {
        let (mut st, names) = tm_view_fixture();
        st.roll(3, 4).unwrap(); // sum 7 + a lone 3: both hit bob (left-of-roller AND three_man)
        assert_eq!(st.phase, crate::three_man::Phase::Rolled);
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains("die-pip"));
        assert!(html.contains(">7<"));
        assert!(html.contains("bob drinks 1"));
        assert!(html.contains(r#"data-me-text="You drink 1" data-player-id="2""#));
        assert!(html.contains("PASS TO bob"));
        assert!(html.contains("/drinks/room/QK4M/tm/pass"));
        assert!(html.contains(r#"<div class="verdict-card" data-anim="pop">"#));
    }

    #[test]
    fn test_tm_phone_nobody_drinks_box() {
        let (mut st, names) = tm_view_fixture();
        st.roll(2, 4).unwrap(); // sum 6, no 3s — nobody drinks
        assert!(st.calls.is_empty());
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains("nobody-box"));
    }

    #[test]
    fn test_tm_handoff_picker_only_on_roller_phone() {
        let (mut st, names) = tm_view_fixture();
        st.set_three_man(1).unwrap(); // roller (alice) is now also the 3 Man
        st.roll(3, 6).unwrap(); // a lone 3 rolled by the 3 Man themself -> HandOff
        assert_eq!(st.phase, crate::three_man::Phase::HandOff);
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains(r#"data-show-player="1" hidden"#));
        assert!(html.contains(r#"data-hide-player="1""#));
        assert!(html.contains("/drinks/room/QK4M/tm/three-man"));
        // Picker offers exactly bob(2) and cara(3) — the seat strip also
        // renders every name, so assert on the actual pick targets rather
        // than substring-matching "bob"/"cara" (which the strip alone
        // would already satisfy).
        assert_eq!(html.matches(r#"hx-vals='{"target":"#).count(), 2);
        assert!(html.contains(r#"hx-vals='{"target":2}'"#));
        assert!(html.contains(r#"hx-vals='{"target":3}'"#));
        assert!(!html.contains(r#"hx-vals='{"target":1}'"#)); // outgoing 3 Man excluded
    }

    #[test]
    fn test_tm_assign_owner_only_and_split_hidden_at_two_players() {
        let (mut st, names) = tm_view_fixture();
        st.roll(4, 4).unwrap(); // double, no 3s/7/9/11 -> Assign, owner = alice (roller)
        assert_eq!(st.phase, crate::three_man::Phase::Assign);
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains(r#"data-show-player="1" hidden"#));
        assert!(html.contains("Both dice to one person"));
        assert!(html.contains("One die each to two people")); // 3 players -> SPLIT offered

        let mut st2 = ThreeManState::new(vec![1, 2], 1);
        st2.roll(2, 2).unwrap();
        assert_eq!(st2.phase, crate::three_man::Phase::Assign);
        let names2 = HashMap::from([(1, "alice".to_string()), (2, "bob".to_string())]);
        let v2 = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st2,
            names: &names2,
        };
        let html2 = tm_phone_panel(&v2);
        assert!(!html2.contains("One die each to two people"));
    }

    #[test]
    fn test_tm_gifts_rows_and_payback() {
        let (mut st, names) = tm_view_fixture();
        st.roll(4, 4).unwrap();
        st.set_mode(crate::three_man::GiveMode::Both).unwrap();
        {
            // Target grid, pre-pick: /tm/target posts {"slot":N,"target":ID}
            // per the plan's Task 13 route table (not "player").
            let v = TmView {
                base_path: "/drinks",
                code: "QK4M",
                st: &st,
                names: &names,
            };
            let html = tm_phone_panel(&v);
            assert!(html.contains("/drinks/room/QK4M/tm/target"));
            assert!(html.contains(r#"hx-vals='{"slot":0,"target":3}'"#)); // cara offered
            assert!(!html.contains(r#""player":"#));
        }
        st.pick_target(0, 3).unwrap(); // cara
        st.send().unwrap();
        assert_eq!(st.phase, crate::three_man::Phase::Gifts);
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains("ROLL 2 DICE"));
        assert!(html.contains("/drinks/room/QK4M/tm/gift-roll"));
        assert!(html.contains(r#"hx-vals='{"slot":0}'"#));
        assert!(!html.contains("data-show-player")); // gift ROLL is any-phone

        st.gift_roll(0, vec![4, 2]).unwrap(); // a gifted 4 matches the double value -> payback
        assert_eq!(st.double.as_ref().unwrap().payback, Some(6));
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains("payback-banner"));
        assert!(html.contains("PAYBACK"));
        assert!(html.contains("alice drinks 6"));
        assert!(html.contains("PASS TO")); // gifts complete -> pass eligible again
    }

    #[test]
    fn test_tm_stale_verdict_dimmed() {
        let (mut st, names) = tm_view_fixture();
        st.roll(2, 4).unwrap();
        st.pass().unwrap();
        assert!(st.stale);
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_phone_panel(&v);
        assert!(html.contains("verdict-card stale"));
        assert!(html.contains("LAST ROLL &middot; alice"));
    }

    #[test]
    fn test_tm_screen_waiting_only_before_first_roll() {
        let (st, names) = tm_view_fixture();
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let html = tm_screen_panel(&v);
        assert!(html.contains("WAITING ON"));
        // Scoped to the waiting headline — the seat strip also renders
        // "alice", so an unscoped `contains("alice")` would pass regardless.
        assert!(html.contains(r#"<h2 class="screen-hero-title">alice</h2>"#));

        let mut st2 = st;
        st2.roll(2, 4).unwrap();
        st2.pass().unwrap();
        assert!(st2.stale);
        let v2 = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st2,
            names: &names,
        };
        let html2 = tm_screen_panel(&v2);
        assert!(html2.contains("verdict-card stale"));
        assert!(!html2.contains("WAITING ON"));
    }

    #[test]
    fn test_tm_seating_and_topbar_chip() {
        let (st, names) = tm_view_fixture();
        let v = TmView {
            base_path: "/drinks",
            code: "QK4M",
            st: &st,
            names: &names,
        };
        let seating = tm_seating_html(&v);
        assert!(seating.contains("/drinks/room/QK4M/tm/seat"));
        // /tm/seat posts {"target":ID,"dir":±1} per the plan's Task 13
        // route table (not "player"/"delta").
        assert!(seating.contains(r#"hx-vals='{"target":1,"dir":-1}'"#));
        assert!(seating.contains(r#"hx-vals='{"target":1,"dir":1}'"#));
        assert!(!seating.contains(r#""player":"#));
        assert!(!seating.contains(r#""delta":"#));
        assert!(seating.contains("/drinks/room/QK4M/tm/three-man"));
        assert!(seating.contains("rules-ref"));

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
            RoomMember {
                id: 3,
                name: "cara".into(),
                joined_at: "t".into(),
            },
        ];
        let house_rules: Vec<HouseRule> = vec![];
        let room_view = RoomView {
            base_path: "/drinks",
            code: "QK4M",
            members: &members,
            house_rules: &house_rules,
            kings: 0,
            mode: "three_man",
            seating: Some(seating),
        };
        let html = room_panel(&room_view);
        // Scoped to the <template data-topbar> half of the fragment — the
        // seating block's per-row 3 MAN assign buttons also contain the
        // literal text "3 MAN", so an unscoped `html.contains` would pass
        // even if the topbar chip itself were never rendered.
        let topbar = html.split("</template>").next().unwrap();
        assert!(topbar.contains(r#"<span class="tm-chip">3 MAN</span>"#));
        assert!(topbar.contains("at the table"));
    }

    #[test]
    fn test_leaderboard_tm_badge() {
        let rows = vec![
            LeaderboardRow {
                id: 1,
                name: "alice".into(),
                drinks: 1,
                shots: 0,
            },
            LeaderboardRow {
                id: 2,
                name: "bob".into(),
                drinks: 0,
                shots: 0,
            },
        ];
        let html = leaderboard_items_tm(&rows, Some(2));
        assert_eq!(html.matches("3 MAN").count(), 1);
        let bob_idx = html.find("bob").unwrap();
        let badge_idx = html.find("3 MAN").unwrap();
        assert!(badge_idx > bob_idx);

        // plain leaderboard_items (None) never renders the badge.
        assert!(!leaderboard_items(&rows).contains("3 MAN"));
    }

    #[test]
    fn test_dice_pips() {
        assert_eq!(dice_html(5, 2).matches("die-pip").count(), 7);
    }
}
