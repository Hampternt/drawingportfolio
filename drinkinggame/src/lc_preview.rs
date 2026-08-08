//! `GET /lastcall/preview` — a permanent visual style guide rendering Plan
//! A's component builders against Plan A's `preview_state()` fixture and
//! `lc_cards::CATALOG`. Public, unguarded (no `PlayerSession`): it displays
//! fixture constants, touches no room, no player and no database — a style
//! guide you have to log in to read is a style guide nobody reads.
//!
//! CLAUDE.md: templates receive pre-computed values. `build_groups()`
//! assembles every group's markup as a `String` by calling `lc_render`
//! builders; `lc_preview.html` only iterates and emits `{{ g.body|safe }}`.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::last_call::{Card, CardKind, Deck};
use crate::lc_cards::{self, CATALOG};
use crate::lc_render::{self, BackSize};
use crate::GameState;

#[derive(Template)]
#[template(path = "lc_preview.html")]
pub struct LcPreviewTemplate {
    pub base_path: String,
    pub groups: Vec<PreviewGroup>,
}

pub struct PreviewGroup {
    pub id: String,
    pub title: String,
    pub note: String,
    pub body: String,
}

/// A labelled swatch cell: the rendered sample plus its caption.
fn swatch(label: &str, html: &str) -> String {
    format!(
        r#"<div class="lc-preview-swatch"><div class="lc-preview-sample">{html}</div><span class="lc-preview-caption">{label}</span></div>"#
    )
}

/// A row of swatches under a sub-heading.
fn row(heading: &str, cells: &[String]) -> String {
    let cells = cells.concat();
    format!(
        r#"<div class="lc-preview-row"><h3 class="lc-preview-subhead">{heading}</h3><div class="lc-preview-grid">{cells}</div></div>"#
    )
}

/// Deterministic `n`-character filler, built by cycling a filler phrase
/// rather than repeating a single character, so the boundary body cards read
/// as prose rather than as an obviously synthetic wall of one letter.
fn body_of_len(n: usize) -> String {
    const FILLER: &str =
        "Boundary fixture body text, sized to sit exactly on the clamp threshold. ";
    FILLER.chars().cycle().take(n).collect()
}

/// Threshold-boundary cards the catalog cannot cover by design (spec §7.5):
/// titles of exactly 14/15/24/25 `chars()` (the `title_ramp_class`
/// boundaries — the 14/15/24/25-char strings are the exact fixtures already
/// pinned by `lc_render`'s own `test_title_ramp_thresholds`), bodies of
/// exactly 108 and 109 `chars()` (the `is_truncated` body boundary), and
/// keyword counts of 0, 3 and 6 (the chip-fold boundary). Every field other
/// than the one under test is held short/inert, so each pair's caption
/// ("marked expandable" / "not marked") says something true about the single
/// dimension being varied — note `TITLE_CLAMP_CHARS` (44) is a different,
/// much larger threshold than the title *ramp*, so none of the title-length
/// boundary cards below trip `is_truncated` on title alone.
pub fn boundary_cards() -> Vec<(&'static str, Card)> {
    fn card(id: &str, title: &str, text: &str, keywords: &[&str]) -> Card {
        Card {
            id: id.to_string(),
            deck: Deck::Wine,
            kind: CardKind::Util,
            cost: 2,
            targets: "one".to_string(),
            title: title.to_string(),
            text: text.to_string(),
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            duration: None,
        }
    }
    let short_body = "Placeholder body, well under the clamp threshold.";
    vec![
        (
            "Title — 14 chars",
            card("b-title-14", "Fourteen chars", short_body, &[]),
        ),
        (
            "Title — 15 chars",
            card("b-title-15", "Fifteen chars!!", short_body, &[]),
        ),
        (
            "Title — 24 chars",
            card("b-title-24", "Twenty-four characters!!", short_body, &[]),
        ),
        (
            "Title — 25 chars",
            card("b-title-25", "Twenty-five characters!!!", short_body, &[]),
        ),
        (
            "Body — 108 chars",
            card("b-body-108", "Body 108", &body_of_len(108), &[]),
        ),
        (
            "Body — 109 chars",
            card("b-body-109", "Body 109", &body_of_len(109), &[]),
        ),
        (
            "Keywords — 0",
            card("b-kw-0", "No keywords", short_body, &[]),
        ),
        (
            "Keywords — 3",
            card(
                "b-kw-3",
                "Three keywords",
                short_body,
                &["alpha", "bravo", "charlie"],
            ),
        ),
        (
            "Keywords — 6",
            card(
                "b-kw-6",
                "Six keywords",
                short_body,
                &["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"],
            ),
        ),
    ]
}

/// Group 1 — the card primitive matrix. Module Spec G step 1's done-when,
/// verbatim: one card renders at all five sizes, in all five deck colours,
/// from one object. Card-level builders take `&Card`, never a projection —
/// the hand is private and never projected; `card_face` is only ever called
/// on the viewer's own cards.
fn card_matrix_group() -> PreviewGroup {
    let mut body = String::new();

    for deck in Deck::ALL {
        let card = &lc_cards::deck_cards(deck)[0];
        let cells = vec![
            swatch("CardFace", &lc_render::card_face(card)),
            swatch("CardPip", &lc_render::card_pip(card)),
            swatch("CardMini", &lc_render::card_mini(card)),
            swatch("CardBack", &lc_render::card_back(deck, BackSize::Pile)),
            swatch("CardDot", &lc_render::card_dot(deck)),
        ];
        body.push_str(&row(deck.label(), &cells));
    }

    // The grid IS the card back — the 9px/10px background-size step between
    // sizes is exactly what only an eye catches, so all four sizes appear
    // together here, not just the Pile size used above.
    let back_cells: Vec<String> = [
        BackSize::Strip,
        BackSize::Flight,
        BackSize::Pile,
        BackSize::Stack,
    ]
    .into_iter()
    .map(|size| swatch(size.slug(), &lc_render::card_back(Deck::Beer, size)))
    .collect();
    body.push_str(&row("CardBack — all four sizes", &back_cells));

    // Every cost 1-3 in every deck (spec §10): costs outside a deck's own
    // spread are not playable, but the pip primitive must still render — this
    // is the primitive's matrix, not the catalog's, so the sample card is
    // cloned from the catalog with only its cost overridden, not invented.
    let mut pip_cells = Vec::new();
    for deck in Deck::ALL {
        let sample = &lc_cards::deck_cards(deck)[0];
        for cost in 1..=3u8 {
            let card = Card {
                cost,
                ..sample.clone()
            };
            pip_cells.push(swatch(
                &format!("{} {cost}", deck.label()),
                &lc_render::card_pip(&card),
            ));
        }
    }
    body.push_str(&row("CardPip — every cost 1-3, every deck", &pip_cells));

    PreviewGroup {
        id: "matrix".to_string(),
        title: "Card primitive matrix".to_string(),
        note: "One Card object per row, five renderings from it — CardFace, CardPip, \
               CardMini, CardBack and CardDot — never five different data shapes. \
               Card-level builders take &Card directly: the hand is private and never \
               projected through PublicView, so card_face is only ever called on the \
               viewer's own cards."
            .to_string(),
        body,
    }
}

/// Group 2 — the §7.5 text-handling cases. Two sources, because they prove
/// different things: the catalog proves the ramp against rendered content,
/// `boundary_cards()` proves the exact thresholds the catalog can't sit on.
fn text_cases_group() -> PreviewGroup {
    let mut body = String::new();

    let catalog_cells: Vec<String> = CATALOG
        .iter()
        .map(|def| {
            let card = lc_cards::card_by_id(def.id).unwrap();
            let ramp = lc_render::title_ramp_class(&card.title);
            let label = format!("{} — {ramp}", card.title);
            swatch(&label, &lc_render::card_face(&card))
        })
        .collect();
    body.push_str(&row(
        "Catalog — all 20 cards, deliberately adversarial (spec §9)",
        &catalog_cells,
    ));

    let pair_cells: Vec<String> = boundary_cards()
        .into_iter()
        .map(|(label, card)| {
            let mark = if lc_render::is_truncated(&card) {
                "is_truncated: marked expandable"
            } else {
                "is_truncated: not marked"
            };
            let pair_html = format!(
                r#"<div class="lc-preview-pair"><div>{}</div><div>{}</div></div>"#,
                lc_render::card_face(&card),
                lc_render::card_face_expanded(&card),
            );
            swatch(&format!("{label} — {mark}"), &pair_html)
        })
        .collect();
    body.push_str(&row(
        "Boundary cards — clamped card_face beside its card_face_expanded twin",
        &pair_cells,
    ));

    PreviewGroup {
        id: "text".to_string(),
        title: "Text handling — the §7.5 ramp and clamp".to_string(),
        note: "The catalog (spec §9's deliberately adversarial 20) proves the lg/md/sm \
               ramp, the 3-line body clamp and the keyword +n fold against rendered \
               content, not just test fixtures. boundary_cards() sits exactly on the \
               thresholds the catalog can't: each pair shows the clamped card beside its \
               expanded twin, so what the clamp lost is visible, not just asserted."
            .to_string(),
        body,
    }
}

/// Every group rendered on the preview page, in template order. Groups are
/// pre-rendered HTML strings (CLAUDE.md: templates receive pre-computed
/// values) — `lc_preview.html` only iterates and emits `{{ g.body|safe }}`.
///
/// Deliberately builds no `PublicView`/`PublicSeat` projection: Groups 1-2
/// render only Card-level primitives and the text-handling ramp, and neither
/// touches a public component (§7.8's PlayerPlaque, HandStrip, DeckStack,
/// DiscardSlot, PhaseBanner, BeatTimer, the felt scene or the flight layer —
/// all Task 3's plaque/felt group). The first group that DOES render a
/// public component must build `last_call::preview_state().public_view()`
/// once here and thread `&PublicSeat`/`&PublicView` into every public
/// builder it calls — never raw `LastCallState` (spec §3.4). A public
/// builder fed straight from `LastCallState` is the regression to reject; a
/// missing `PublicSeat`/`PublicView` field needed by a builder becomes a
/// compile error at that call site, which is the whole point of routing
/// through the projection here rather than discovering the gap in Plan A2.
fn build_groups() -> Vec<PreviewGroup> {
    vec![card_matrix_group(), text_cases_group()]
}

pub async fn preview_page(State(state): State<GameState>) -> impl IntoResponse {
    let tpl = LcPreviewTemplate {
        base_path: state.base_path.to_string(),
        groups: build_groups(),
    };
    Html(tpl.render().unwrap())
}
