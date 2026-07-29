//! HTML fragment builders (format! strings, matching the portfolio's
//! post_card_html convention) plus escaping.

use crate::cards::Card;
use crate::models::{DrawCount, LeaderboardRow, RulePreset};

/// Same escape set as the portfolio's html_escape.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Returns the <li> rows for the leaderboard. The surrounding
/// <ol id="leaderboard"> lives in the templates so SSE can replace
/// innerHTML wholesale.
pub fn leaderboard_items(rows: &[LeaderboardRow]) -> String {
    if rows.is_empty() {
        return r#"<li class="lb-empty">Nobody here yet</li>"#.to_string();
    }
    rows.iter()
        .map(|r| {
            format!(
                r#"<li><span class="lb-name">{}</span><span class="lb-counts">{} drinks &middot; {} shots</span></li>"#,
                html_escape(&r.name),
                r.drinks,
                r.shots
            )
        })
        .collect()
}

pub struct CurrentCard {
    pub card: Card,
    pub title: String,
    pub text: String,
    pub drawer: String,
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
}

/// A card face in pure HTML/CSS — rank + suit glyph, red/black via class.
pub fn card_face_html(card: Card) -> String {
    let red = if card.suit.is_red() { " card-red" } else { "" };
    format!(
        r#"<div class="card-face{red}"><span class="card-rank">{}</span><span class="card-suit">{}</span></div>"#,
        card.rank_label(),
        card.suit.glyph(),
    )
}

/// Idle state: preset picker + start button. First preset (Standard, lowest
/// id) is the <select> default by position.
pub fn game_idle_panel(base_path: &str, code: &str, presets: &[RulePreset]) -> String {
    let options: String = presets
        .iter()
        .map(|p| {
            format!(
                r#"<option value="{}">{}</option>"#,
                p.id,
                html_escape(&p.name)
            )
        })
        .collect();
    format!(
        r#"<div class="game-idle">
<form hx-post="{base_path}/room/{code}/game/start" hx-swap="none">
<select name="preset_id">{options}</select>
<button type="submit" class="btn-start">Start Ring of Fire</button>
</form>
<a class="presets-link" href="{base_path}/presets">Edit rule presets</a>
</div>"#
    )
}

fn draw_counts_html(counts: &[DrawCount]) -> String {
    counts
        .iter()
        .map(|c| {
            format!(
                r#"<li><span class="dc-name">{}</span><span class="dc-count">{}</span></li>"#,
                html_escape(&c.name),
                c.draws
            )
        })
        .collect()
}

/// The live game panel: announcement, current card + rule, deck button,
/// held-card strip, per-player draw counts, end-early button.
pub fn game_active_panel(view: &GameView) -> String {
    let base_path = view.base_path;
    let code = view.code;
    let announcement = view
        .announcement
        .as_deref()
        .map(|a| format!(r#"<p class="game-announcement">{}</p>"#, html_escape(a)))
        .unwrap_or_default();
    let current = match &view.current {
        Some(c) => format!
            (r#"<div class="game-current">{}<div class="game-rule"><p class="rule-drawer">{} drew</p><h3 class="rule-title">{}</h3><p class="rule-text">{}</p></div></div>"#,
            card_face_html(c.card),
            html_escape(&c.drawer),
            html_escape(&c.title),
            html_escape(&c.text),
        ),
        None => r#"<div class="game-current"><p class="rule-text">Fresh deck. Tap to draw the first card.</p></div>"#.to_string(),
    };
    let held: String = if view.held.is_empty() {
        String::new()
    } else {
        let items: String = view
            .held
            .iter()
            .map(|h| {
                format!(
                    r#"<li class="held-card">{}<span class="held-holder">{} · {}</span><button class="use-btn" hidden data-holder-id="{}" data-draw-id="{}" hx-post="{base_path}/room/{code}/game/spend" hx-vals='{{"draw_id":{}}}' hx-swap="none">Use</button></li>"#,
                    card_face_html(h.card),
                    html_escape(&h.holder_name),
                    html_escape(&h.title),
                    h.holder_id,
                    h.draw_id,
                    h.draw_id,
                )
            })
            .collect();
        format!(r#"<ul class="held-strip">{items}</ul>"#)
    };
    format!(
        r#"<div class="game-active">
{announcement}
{current}
<button class="btn-draw" hx-post="{base_path}/room/{code}/game/draw" hx-swap="none">Tap to draw<span class="deck-count">{} cards left</span></button>
{held}
<ol class="draw-counts">{}</ol>
<button class="btn-game-end" hx-post="{base_path}/room/{code}/game/end" hx-swap="none" hx-confirm="End the game for everyone?">End game early</button>
</div>"#,
        view.remaining,
        draw_counts_html(view.counts),
    )
}

/// Post-game summary. The idle panel (rendered separately) restores Start.
pub fn game_summary_panel(counts: &[DrawCount]) -> String {
    format!(
        r#"<div class="game-summary"><h3>Game over</h3><ol class="draw-counts">{}</ol></div>"#,
        draw_counts_html(counts),
    )
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
    fn test_leaderboard_items_escapes_names() {
        let rows = vec![LeaderboardRow {
            name: "<script>".into(),
            drinks: 2,
            shots: 1,
        }];
        let html = leaderboard_items(&rows);
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("2 drinks"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_leaderboard_items_empty() {
        assert!(leaderboard_items(&[]).contains("Nobody here yet"));
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
    fn test_card_face_marks_red_suits() {
        let html = card_face_html(Card {
            rank: 12,
            suit: Suit::Hearts,
        });
        assert!(html.contains("Q"));
        assert!(html.contains("\u{2665}"));
        assert!(html.contains("card-red"));
        let html = card_face_html(Card {
            rank: 1,
            suit: Suit::Spades,
        });
        assert!(!html.contains("card-red"));
    }

    #[test]
    fn test_idle_panel_lists_presets_and_start() {
        let html = game_idle_panel(
            "/drinks",
            "ABCD",
            &[preset(1, "Standard"), preset(2, "<Wild>")],
        );
        assert!(html.contains("/drinks/room/ABCD/game/start"));
        assert!(html.contains(r#"<option value="1">Standard</option>"#));
        assert!(html.contains("&lt;Wild&gt;")); // names escaped
        assert!(html.contains("Start Ring of Fire"));
    }

    #[test]
    fn test_active_panel_shows_card_held_and_counts() {
        let counts = vec![DrawCount {
            name: "alice".into(),
            draws: 3,
        }];
        let view = GameView {
            base_path: "/drinks",
            code: "ABCD",
            current: Some(CurrentCard {
                card: Card {
                    rank: 5,
                    suit: Suit::Clubs,
                },
                title: "Thumb Master".into(),
                text: "Thumbs!".into(),
                drawer: "alice".into(),
            }),
            remaining: 49,
            held: vec![HeldCardView {
                draw_id: 7,
                holder_id: 2,
                holder_name: "bob".into(),
                card: Card {
                    rank: 7,
                    suit: Suit::Hearts,
                },
                title: "Heaven".into(),
            }],
            counts: &counts,
            announcement: Some("bob used Heaven!".into()),
        };
        let html = game_active_panel(&view);
        assert!(html.contains("Thumb Master"));
        assert!(html.contains("alice drew"));
        assert!(html.contains("49 cards left"));
        assert!(html.contains("/drinks/room/ABCD/game/draw"));
        assert!(html.contains("/drinks/room/ABCD/game/end"));
        // Use button: hidden by default, tagged with holder + draw ids so the
        // page JS reveals it only on the holder's phone.
        assert!(html.contains(r#"data-holder-id="2""#));
        assert!(html.contains(r#"data-draw-id="7""#));
        assert!(html.contains("hidden"));
        assert!(html.contains("bob used Heaven!"));
        assert!(html.contains("alice") && html.contains("3"));
    }

    #[test]
    fn test_active_panel_before_first_draw_has_no_current_card() {
        let counts: Vec<DrawCount> = vec![];
        let view = GameView {
            base_path: "",
            code: "ABCD",
            current: None,
            remaining: 52,
            held: vec![],
            counts: &counts,
            announcement: None,
        };
        let html = game_active_panel(&view);
        assert!(html.contains("52 cards left"));
        assert!(html.contains("Tap to draw"));
    }

    #[test]
    fn test_summary_panel() {
        let counts = vec![
            DrawCount {
                name: "alice".into(),
                draws: 30,
            },
            DrawCount {
                name: "<bob>".into(),
                draws: 22,
            },
        ];
        let html = game_summary_panel(&counts);
        assert!(html.contains("Game over"));
        assert!(html.contains("alice"));
        assert!(html.contains("30"));
        assert!(html.contains("&lt;bob&gt;"));
    }
}
