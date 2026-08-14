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

use crate::last_call::{self, Beat, Card, CardKind, Deck, LastCallState, PublicView};
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
        &format!(
            "Catalog — all {} distinct cards (x copies make the 40-card shoe), spec §9 \
             coverage now on real content",
            CATALOG.len()
        ),
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
        note: "The catalog (spec §9, real content, coverage test-pinned by Task 3) proves \
               the lg/md/sm ramp, the 3-line body clamp and the keyword +n fold against \
               rendered content, not just test fixtures. boundary_cards() sits exactly on \
               the thresholds the catalog can't: each pair shows the clamped card beside \
               its expanded twin, so what the clamp lost is visible, not just asserted."
            .to_string(),
        body,
    }
}

/// Injects `is-hit` onto an already-rendered plaque. `player_plaque` (Plan
/// A) deliberately never emits it — it is a transient event, not a
/// projected state (spec §3.4: a broadcast snapshot cannot say "was hit just
/// now" without leaking timing into state) — so this preview is the only
/// place it can be demonstrated, added by hand rather than authored as a new
/// component.
fn plaque_with_is_hit(seat: &crate::last_call::PublicSeat) -> String {
    lc_render::player_plaque(seat).replacen(
        r#"class="lc-plaque "#,
        r#"class="lc-plaque is-hit "#,
        1,
    )
}

/// Injects `is-urgent` onto an already-rendered beat timer, the same way
/// `plaque_with_is_hit` adds `is-hit`: `beat_timer` has no notion of
/// "urgent," only a duration and an elapsed count, so the preview adds the
/// class by hand to show the rose treatment under 5s remaining.
fn timer_with_is_urgent(duration_ms: u32, elapsed_ms: u32) -> String {
    lc_render::beat_timer(duration_ms, elapsed_ms).replacen(
        r#"class="lc-timer""#,
        r#"class="lc-timer is-urgent""#,
        1,
    )
}

/// Group 3 — the §7.6 scene primitives at a size you can judge, and the felt
/// itself (`id="lc-felt"`, `data-flight-anchor="felt"`; sizing lives in
/// lastcall.css's preview-page section, since Plan A's `#lc-felt` rule ships
/// no width/height of its own — that is the consuming page's call). Seat
/// positioning is deliberately absent: the felt is a bare background
/// primitive here, and D.2's angle seat-ring layout is Plan B's.
fn scene_group() -> PreviewGroup {
    let mut body = String::new();

    let surfaces: [(&str, &str); 6] = [
        ("Ground", "lc-ground"),
        ("Device", "lc-device"),
        ("Panel", "lc-panel"),
        ("Panel alt", "lc-panel-alt"),
        ("Raised", "lc-raised"),
        ("Focused", "lc-focused"),
    ];
    let surface_cells: Vec<String> = surfaces
        .iter()
        .map(|(label, class)| {
            swatch(
                label,
                &format!(r#"<div class="{class} lc-preview-swatchbox"></div>"#),
            )
        })
        .collect();
    body.push_str(&row("Grounds and panels", &surface_cells));

    let hairline_cells = vec![
        swatch(
            "lc-hairline (.10)",
            r#"<div class="lc-panel lc-hairline lc-preview-swatchbox"></div>"#,
        ),
        swatch(
            "lc-hairline-strong (.22)",
            r#"<div class="lc-panel lc-hairline-strong lc-preview-swatchbox"></div>"#,
        ),
    ];
    body.push_str(&row("Hairline ladder", &hairline_cells));

    let mut alpha_cells = Vec::new();
    for deck in Deck::ALL {
        let slug = deck.slug();
        for (label, class) in [
            ("59", "lc-edge-subtle"),
            ("66", "lc-edge-plaque"),
            ("80", "lc-edge-back"),
            ("99", "lc-edge-strong"),
        ] {
            alpha_cells.push(swatch(
                &format!("{} — {label}", deck.label()),
                &format!(
                    r#"<div class="lc-panel lc-deck-{slug} {class} lc-preview-swatchbox"></div>"#
                ),
            ));
        }
    }
    body.push_str(&row(
        "Deck-tinted border alphas — invisible anywhere else",
        &alpha_cells,
    ));

    // Wrapped in .lc-preview-scroll (not resized) — the felt is a fixed
    // 640x360 reference dimension; on a narrow viewport the wrapper scrolls
    // instead of the whole page (checkpoint 2 item 8 / I4).
    let felt = r#"<div class="lc-preview-scroll"><div id="lc-felt" data-flight-anchor="felt"></div></div>"#;
    body.push_str(&row(
        "The felt — a background primitive",
        &[swatch(
            "640×360, rail + inner hairline ellipse + shadow stack",
            felt,
        )],
    ));

    PreviewGroup {
        id: "scene".to_string(),
        title: "Scene — grounds, panels and the felt".to_string(),
        note: "Seat positioning is not here: the felt ships as a bare background \
               primitive with nothing on it, and D.2's angle seat-ring layout is \
               Plan B's to add. The four-rung alpha ladder (59/66/80/99) is now \
               fully bound in Plan A's scene-primitives section — .lc-edge-strong \
               closed the gap Task 3 correctly reported rather than inventing."
            .to_string(),
        body,
    }
}

/// Group 4 — the table components and the plaque's five states, all built
/// from `&PublicSeat`/`&PublicView` taken from `view` (never an `LcPlayer` —
/// spec §3.4). Every count is read from `view` rather than hard-coded, so
/// Plan A's `test_preview_state_covers_every_variant` keeps guarding this
/// group.
fn table_components_group(view: &PublicView) -> PreviewGroup {
    let mut body = String::new();

    fn caption(seat: &crate::last_call::PublicSeat, state: &str) -> String {
        format!(
            "{state} — {}, HP {}, {} cards",
            seat.name.to_uppercase(),
            seat.hp,
            seat.hand_len
        )
    }

    let idle = swatch(
        &caption(&view.seats[0], "idle"),
        &lc_render::player_plaque(&view.seats[0]),
    );
    let locked = swatch(
        &caption(&view.seats[2], "locked"),
        &lc_render::player_plaque(&view.seats[2]),
    );
    let drawing = swatch(
        &caption(&view.seats[4], "drawing"),
        &lc_render::player_plaque(&view.seats[4]),
    );
    let eliminated = swatch(
        &caption(&view.seats[6], "eliminated"),
        &lc_render::player_plaque(&view.seats[6]),
    );
    let hit_html = plaque_with_is_hit(&view.seats[0]);
    let hit = swatch(
        &caption(&view.seats[0], "hit (replayable)"),
        &format!(
            r##"<div><div id="lc-preview-hit-plaque">{hit_html}</div><button type="button" class="lc-btn-secondary" data-replay-state="is-hit" data-target="#lc-preview-hit-plaque .lc-plaque">REPLAY</button></div>"##
        ),
    );
    let two_deck = swatch(
        &caption(&view.seats[5], "two-deck"),
        &lc_render::player_plaque(&view.seats[5]),
    );
    let oversized = swatch(
        &caption(&view.seats[1], "oversized hand"),
        &lc_render::player_plaque(&view.seats[1]),
    );
    body.push_str(&row(
        "PlayerPlaque — the five states",
        &[idle, locked, drawing, eliminated, hit, two_deck, oversized],
    ));

    // Every seat the fixture carries — the only place all eight
    // plaque-seat-{n} motion anchors (spec §7.8.1) are provable at once.
    let all_seats: Vec<String> = view
        .seats
        .iter()
        .map(|s| {
            swatch(
                &format!("seat {} — {}", s.seat, s.name.to_uppercase()),
                &lc_render::player_plaque(s),
            )
        })
        .collect();
    body.push_str(&row(
        "All eight seats — every plaque-seat-{n} motion anchor",
        &all_seats,
    ));

    let strip_cells: Vec<String> = [0usize, 1, 4, 8, 9, 30]
        .into_iter()
        .map(|n| {
            swatch(
                &format!("n = {n}"),
                &lc_render::hand_strip(&[Deck::Beer], n),
            )
        })
        .collect();
    body.push_str(&row(
        "HandStrip — every size, either side of the n > 8 split",
        &strip_cells,
    ));

    let deck_cells: Vec<String> = view
        .deck_counts
        .iter()
        .map(|&(deck, count)| {
            swatch(
                &format!("{} — {count}", deck.label()),
                &lc_render::deck_stack(deck, count),
            )
        })
        .collect();
    body.push_str(&row(
        "DeckStack — every deck, from view.deck_counts",
        &deck_cells,
    ));

    let discard = swatch(
        &format!("discard — {}", view.discard_count),
        &lc_render::discard_slot(view.discard_count),
    );
    body.push_str(&row("DiscardSlot", &[discard]));

    let timer = swatch("60s duration, 0 elapsed", &lc_render::beat_timer(60_000, 0));
    let urgent = swatch(
        "60s duration, 56s elapsed — under 5s left",
        &timer_with_is_urgent(60_000, 56_000),
    );
    body.push_str(&row("BeatTimer", &[timer, urgent]));

    PreviewGroup {
        id: "components".to_string(),
        title: "Table components and the plaque's five states".to_string(),
        note: "Every plaque here is built from a &PublicSeat taken from the \
               PublicView projection, never from an LcPlayer — the type system \
               enforces it, which is the whole point of routing through the \
               projection: a missing PublicSeat/PublicView field a builder needed \
               would be a compile error at the call site, not a Plan A2 discovery. \
               is-hit is added by the preview, by hand: player_plaque never emits \
               it (a transient event, not a projected state, spec §3.4), so this \
               page is the only place it can be demonstrated — press REPLAY to \
               restart the shake and HP flash."
            .to_string(),
        body,
    }
}

/// Group 5 — the F.1 phone shell chrome at its real type scale, static
/// markup inside a 390×844 frame. Not the live shell route (Plan A2's); the
/// `.lc-setup` form carries no `action` and no `hx-post`.
fn shell_group(view: &PublicView) -> PreviewGroup {
    let mut body = String::new();

    let mut banner_cells = vec![swatch(
        "current — ROUND 6 · BEAT 4 OF 5 (legacy LOCK)",
        &lc_render::lc_banner(view),
    )];
    for beat in Beat::ORDER {
        let mut v = view.clone();
        v.beat = beat;
        banner_cells.push(swatch(beat.label(), &lc_render::lc_banner(&v)));
    }
    body.push_str(&row(
        "PhaseBanner — once per beat, Draw through Resolve",
        &banner_cells,
    ));

    let tabs_hand_active = r#"<div class="lc-tabs"><button type="button" class="lc-tab" data-lc-tab="hand" aria-selected="true">HAND</button><button type="button" class="lc-tab" data-lc-tab="table" aria-selected="false">TABLE</button><button type="button" class="lc-tab" data-lc-tab="log" aria-selected="false">LOG</button></div>"#;
    let tabs_table_active = r#"<div class="lc-tabs"><button type="button" class="lc-tab" data-lc-tab="hand" aria-selected="false">HAND</button><button type="button" class="lc-tab" data-lc-tab="table" aria-selected="true">TABLE</button><button type="button" class="lc-tab" data-lc-tab="log" aria-selected="false">LOG</button></div>"#;
    body.push_str(&row(
        "Tab row — order never changes; only colour and the underline move",
        &[
            swatch("HAND active", tabs_hand_active),
            swatch("TABLE active", tabs_table_active),
        ],
    ));

    let actions_two_primary = r#"<div class="lc-actions"><button type="button" class="lc-btn lc-btn-drink">DRINK</button><button type="button" class="lc-btn lc-deck-wine">PASS</button></div>"#;
    let actions_primary_secondary = r#"<div class="lc-actions"><button type="button" class="lc-btn lc-deck-beer">PLAY</button><button type="button" class="lc-btn lc-btn-secondary">CANCEL</button></div>"#;
    body.push_str(&row(
        "Action bar",
        &[
            swatch(
                "two primaries — the drinking option amber",
                actions_two_primary,
            ),
            swatch("primary + 92px secondary", actions_primary_secondary),
        ],
    ));

    let setup = r#"<div class="lc-setup"><h2>SETUP</h2><form><select><option>BEER</option><option>CIDER</option><option>WINE</option><option>LIQUOR</option><option>SOFT</option></select><input type="text" placeholder="container, e.g. 50cl can"><button type="button">SET VESSEL</button></form><div class="lc-setup-row"><input type="checkbox" id="lc-preview-handicap"><label for="lc-preview-handicap">handicap</label></div></div>"#;
    body.push_str(&row(
        "Setup form chrome — plain, undesigned on purpose",
        &[swatch("posts nowhere yet — Plan A2 wires it", setup)],
    ));

    let sample_cards: Vec<Card> = ["beer-01", "wine-04", "cider-01"]
        .iter()
        .filter_map(|id| lc_cards::card_by_id(id))
        .collect();
    let hand_faces: String = sample_cards.iter().map(lc_render::card_face).collect();
    let hand_region = format!(
        r#"<div id="lc-hand" data-seq="{seq}" data-count="{n}" data-flight-anchor="hand">{hand_faces}</div>"#,
        seq = view.seq,
        n = sample_cards.len(),
    );
    body.push_str(&row(
        "Hand region — #lc-hand",
        &[swatch("a few CardFaces", &hand_region)],
    ));

    let status_row = r#"<div class="lc-status"><span>9:41</span><span>LAST CALL</span></div>"#;
    // Wrapped in .lc-preview-scroll (not resized) — the frame is a fixed
    // 390x844 F.1 reference dimension; on a narrow viewport the wrapper
    // scrolls instead of the whole page (checkpoint 2 item 8 / I4).
    let frame = format!(
        r#"<div class="lc-preview-scroll"><div class="lc-preview-shell">{status_row}{banner}{tabs_hand_active}<div class="lc-view">{setup}{hand_region}</div>{actions_two_primary}</div></div>"#,
        banner = lc_render::lc_banner(view),
    );
    body.push_str(&row(
        "F.1 fixed order — status → banner → tabs → view → actions",
        &[swatch("390×844 frame", &frame)],
    ));

    PreviewGroup {
        id: "shell".to_string(),
        title: "F.1 phone shell chrome".to_string(),
        note: "Static markup — this is not the live phone shell route, which is \
               Plan A2's GET /lastcall wired to an EventSource; nobody should wire \
               an EventSource into a style guide. The .lc-setup form posts nowhere \
               and triggers nothing — the chrome is structure, where it posts is \
               Plan A2's."
            .to_string(),
        body,
    }
}

/// Group 8 (Plan C) — the hand group's hard cases: a live, draggable
/// `HandWheel` at a realistic size, then the sizes and states a plain game
/// can never reach on its own (oversized, one-card, both empty-state
/// copies, every ArmedColumn cardinality including locked, and the
/// handicap-priced CostRail). `st` is the raw fixture (not `PublicView`) —
/// the hand is private data no projection carries, so this is the one
/// group in the file that reads `st.players[..].hand` directly rather than
/// through `view`.
fn hand_group_group(st: &LastCallState) -> PreviewGroup {
    let mut body = String::new();

    // Row 1 — a full, real HandWheel at a size worth dragging: armed 2,
    // an 8-card hand spanning all five decks, handicap 100. Sized via
    // .lc-preview-scroll > .lc-preview-handframe (Task 5 CSS) so it reads
    // as a phone column, not a full-bleed strip — .lc-handgroup itself
    // only fixes its own height (480px), never its width.
    let live_hand: Vec<Card> = [
        "beer-01",
        "beer-02",
        "cider-01",
        "cider-02",
        "wine-01",
        "wine-04",
        "liquor-01",
        "soft-01",
    ]
    .iter()
    .filter_map(|id| lc_cards::card_by_id(id))
    .collect();
    let live_armed: Vec<Card> = ["liquor-02", "soft-02"]
        .iter()
        .filter_map(|id| lc_cards::card_by_id(id))
        .collect();
    let live_view = lc_render::HandGroupView {
        hand: &live_hand,
        armed: &live_armed,
        locked: false,
        handicap_pct: 100,
        halved: false,
        pulls_left: 0,
    };
    let live_frame = format!(
        r#"<div class="lc-preview-scroll"><div class="lc-preview-handframe">{}</div></div>"#,
        lc_render::hand_group(&live_view)
    );
    body.push_str(&row(
        "HandWheel — live, drag it",
        &[swatch(
            "armed 2 · 8-card mixed-deck hand · handicap 100",
            &live_frame,
        )],
    ));

    // Row 2 — degenerate hand sizes: the oversized wheel (seat 1's 12-card
    // fixture hand plus two boundary_cards, 14 total — past |d| > 2.05,
    // the culling threshold the JS applies), the one-card wheel (snap
    // always returns to its single card), and decision 5's two distinct
    // empty-state copies.
    let mut oversized_hand = st.players[1].hand.clone();
    oversized_hand.extend(boundary_cards().into_iter().take(2).map(|(_, c)| c));
    let oversized_view = lc_render::HandGroupView {
        hand: &oversized_hand,
        armed: &[],
        locked: false,
        handicap_pct: 100,
        halved: false,
        pulls_left: 0,
    };
    let one_card: Vec<Card> = lc_cards::card_by_id("wine-03").into_iter().collect();
    let one_view = lc_render::HandGroupView {
        hand: &one_card,
        armed: &[],
        locked: false,
        handicap_pct: 100,
        halved: false,
        pulls_left: 0,
    };
    let empty_no_armed = lc_render::HandGroupView {
        hand: &[],
        armed: &[],
        locked: false,
        handicap_pct: 100,
        halved: false,
        pulls_left: 0,
    };
    let empty_armed_cards: Vec<Card> = lc_cards::card_by_id("cider-01").into_iter().collect();
    let empty_all_armed = lc_render::HandGroupView {
        hand: &[],
        armed: &empty_armed_cards,
        locked: false,
        handicap_pct: 100,
        halved: false,
        pulls_left: 0,
    };
    body.push_str(&row(
        "HandWheel — degenerate hand sizes",
        &[
            swatch(
                &format!(
                    "oversized — {} cards (seat 1's 12 + two boundary cards)",
                    oversized_hand.len()
                ),
                &lc_render::hand_group(&oversized_view),
            ),
            swatch(
                "one card — snap always returns to it",
                &lc_render::hand_group(&one_view),
            ),
            swatch(
                "empty, nothing armed — \"Register your drink to be dealt a hand.\"",
                &lc_render::hand_group(&empty_no_armed),
            ),
            swatch(
                "empty, everything armed — \"Every card you hold is armed.\"",
                &lc_render::hand_group(&empty_all_armed),
            ),
        ],
    ));

    // Row 3 — ArmedColumn's own cardinalities: 0 (still ARMED 0 plus the
    // slot, never an empty state), 1, many (Wine's full eight-card spread,
    // labelled from its own .len() rather than a hard-coded count now that
    // deck_cards returns eight per deck, not four), and locked (3 — LOCKED
    // 3, dimmed via [data-locked], no slot).
    let one_armed: Vec<Card> = lc_cards::card_by_id("beer-01").into_iter().collect();
    let many_armed = lc_cards::deck_cards(Deck::Wine);
    let three_locked: Vec<Card> = lc_cards::deck_cards(Deck::Liquor)
        .into_iter()
        .take(3)
        .collect();
    body.push_str(&row(
        "ArmedColumn — 0 / 1 / many / locked",
        &[
            swatch(
                "0 — ARMED 0, slot still shown",
                &lc_render::armed_column(&[], false),
            ),
            swatch("1", &lc_render::armed_column(&one_armed, false)),
            swatch(
                &format!("{} — many", many_armed.len()),
                &lc_render::armed_column(&many_armed, false),
            ),
            swatch(
                "locked, 3 — LOCKED 3, dimmed, no slot",
                &lc_render::armed_column(&three_locked, true),
            ),
        ],
    ));

    // Row 4 — every deck at every cost 1-3, synthesized (the catalog's own
    // per-deck spread never covers costs outside it) — 5 decks x 3 costs =
    // 15 cards, 1+2+3=6 bars per deck, 30 bars total.
    let mut priced_all: Vec<Card> = Vec::new();
    for deck in Deck::ALL {
        let sample = &lc_cards::deck_cards(deck)[0];
        for cost in 1..=3u8 {
            priced_all.push(Card {
                cost,
                ..sample.clone()
            });
        }
    }
    body.push_str(&row(
        "CostRail — every cost 1-3 in every deck",
        &[swatch(
            "5 decks x 3 costs — 30 bars, every ink ramp, Wine ink-not-fill",
            &lc_render::cost_rail(&priced_all, 100, false),
        )],
    ));

    // Row 5 — the same 3-card hand (costs 1, 2, 3) priced through three
    // handicaps. Bar counts per handicap_pct, pull_cost(cost, pct)
    // rounding up: 100 -> 1,2,3; 150 -> 2,3,5; 300 -> 3,6,9.
    let handicap_hand: Vec<Card> = ["cider-01", "cider-02", "cider-04"]
        .iter()
        .filter_map(|id| lc_cards::card_by_id(id))
        .collect();
    let rail_cells: Vec<String> = [(100u16, "1,2,3"), (150, "2,3,5"), (300, "3,6,9")]
        .into_iter()
        .map(|(pct, bars)| {
            swatch(
                &format!("handicap {pct} — {bars} bars"),
                &lc_render::cost_rail(&handicap_hand, pct, false),
            )
        })
        .collect();
    body.push_str(&row(
        "CostRail — handicap prices the same hand differently",
        &rail_cells,
    ));

    PreviewGroup {
        id: "hand".to_string(),
        title: "Hand group — the hard cases".to_string(),
        note: "Decision 1: the CostRail shows the true pull price — cost run through \
               the viewer's own handicap, rounded up — while CardPip (group 1) still \
               shows the printed cost; the two numbers disagree on purpose whenever a \
               handicap isn't 100, and row 5 pins the arithmetic. No #lc-hand id \
               anywhere in this group — the preview already carries that id from \
               shell_group and must not gain a second — so every wheel here lives in \
               a plain .lc-handgroup root, which also keeps it out of decision 8's \
               #lc-hand-scoped camera persistence: gallery wheels are demos, they \
               don't remember where you left them. shell_group's static F.1 frame is \
               deliberately left as the plain hand-region stub — it demonstrates the \
               F.1 chrome order, not the hand view; this group owns the hand."
            .to_string(),
        body,
    }
}

/// Group 6 — replayable flights and the anchor board. Motion in a static
/// document is invisible unless it can be fired; each REPLAY button below
/// calls `window.lcFlight` via `lc_preview.html`'s inline script, which
/// delegates to Task 1's helper rather than reimplementing it.
fn flights_group() -> PreviewGroup {
    let mut body = String::new();

    fn replay_btn(direction: &str, from: &str, to: &str, deck: &str, scale: &str) -> String {
        format!(
            r#"<button type="button" class="lc-btn-secondary" data-replay="{direction}" data-from="{from}" data-to="{to}" data-deck="{deck}" data-scale="{scale}">REPLAY</button>"#
        )
    }

    let direction_cells = vec![
        swatch(
            "draw — deck-beer → plaque-seat-0",
            &replay_btn("draw", "deck-beer", "plaque-seat-0", "beer", "card"),
        ),
        swatch(
            "play — plaque-seat-0 → felt",
            &replay_btn("play", "plaque-seat-0", "felt", "beer", "card"),
        ),
        swatch(
            "discard — plaque-seat-0 → discard",
            &replay_btn("discard", "plaque-seat-0", "discard", "beer", "card"),
        ),
        swatch(
            "draw (phone) — deck-soft → plaque-seat-1",
            &replay_btn("draw", "deck-soft", "plaque-seat-1", "soft", "dot"),
        ),
    ];
    body.push_str(&row("Replayable flight directions", &direction_cells));

    let burst = r#"<button type="button" class="lc-btn-secondary" data-replay="draw" data-from="deck-beer" data-to="plaque-seat-0" data-deck="beer" data-scale="card" data-count="7">BURST</button>"#;
    body.push_str(&row(
        "Burst — seven draw flights, 250ms stagger",
        &[swatch("reads as a burst, not a blur", burst)],
    ));

    let anchors = [
        "deck-beer",
        "deck-cider",
        "deck-wine",
        "deck-liquor",
        "deck-soft",
        "discard",
        "plaque-seat-0",
        "plaque-seat-1",
        "plaque-seat-2",
        "plaque-seat-3",
        "plaque-seat-4",
        "plaque-seat-5",
        "plaque-seat-6",
        "plaque-seat-7",
        "hand",
        "armed",
        "felt",
    ];
    let items: String = anchors
        .iter()
        .map(|name| {
            format!(r#"<li><code>{name}</code><span data-anchor-check="{name}">…</span></li>"#)
        })
        .collect();
    let board = format!(r#"<ul class="lc-preview-anchors">{items}</ul>"#);
    body.push_str(&row(
        "Anchor board — every data-flight-anchor name, resolved live",
        &[swatch("checked via window.lcAnchor on load", &board)],
    ));

    let at_rest_card = r#"<div class="lc-preview-flight-box"><div class="lc-flight lc-deck-beer" data-scale="card" data-flight="draw" style="top:0;left:0;--dx:0px;--dy:0px;animation:none"></div></div>"#;
    let at_rest_dot = r#"<div class="lc-preview-flight-box lc-preview-flight-box-dot"><div class="lc-flight lc-deck-beer" data-scale="dot" data-flight="draw" style="top:0;left:0;--dx:0px;--dy:0px;animation:none"></div></div>"#;
    body.push_str(&row(
        "Flight node at rest — footprints without chasing a moving target",
        &[
            swatch("card scale, 44×62", at_rest_card),
            swatch("dot scale, 8×8", at_rest_dot),
        ],
    ));

    PreviewGroup {
        id: "flights".to_string(),
        title: "Replayable flights and the anchor board".to_string(),
        note: "The felt, plaque and deck-stack samples on this page are the \
               reference markup Plan B positions — Plan B changes where they sit, \
               never what they are. Any ✗ on the anchor board means slice 3 would \
               have to rewrite a template to fire a flight at that target."
            .to_string(),
        body,
    }
}

/// Group 7 — the deck colour ramp reference. Hex values are DOCUMENTED text
/// content here, not renderer output — `lc_render.rs` still emits deck
/// classes only, never hex (Task 2's `no_hex` tests hold that line over the
/// renderers; this group's hex is prose, describing the tokens, same as the
/// global-constraints table it is transcribed from).
fn deck_ramp_group() -> PreviewGroup {
    let mut body = String::new();

    let rows: [(Deck, &str, &str, &str); 5] = [
        (Deck::Beer, "#FFB570", "#FFB570", "#14101D"),
        (Deck::Cider, "#B48EF7", "#B48EF7", "#14101D"),
        (Deck::Wine, "#8B2F4A", "#D4657F", "#F2EEF8"),
        (Deck::Liquor, "#F7768E", "#F7768E", "#14101D"),
        (Deck::Soft, "#6FB6FF", "#6FB6FF", "#0D1620"),
    ];
    for (deck, fill_hex, ink_hex, on_fill_hex) in rows {
        let slug = deck.slug();
        let fill = swatch(
            &format!("fill — {fill_hex}"),
            &format!(r#"<div class="lc-deck-{slug} lc-preview-ramp-fill"></div>"#),
        );
        let ink = swatch(
            &format!("ink — {ink_hex}"),
            &format!(r#"<div class="lc-panel lc-deck-{slug} lc-preview-ramp-ink">{ink_hex}</div>"#),
        );
        let on_fill = swatch(
            &format!("text on fill — {on_fill_hex}"),
            &format!(r#"<div class="lc-deck-{slug} lc-preview-ramp-onfill">{on_fill_hex}</div>"#),
        );
        let a59 = swatch(
            "alpha 59",
            &format!(
                r#"<div class="lc-panel lc-deck-{slug} lc-edge-subtle lc-preview-swatchbox"></div>"#
            ),
        );
        let a66 = swatch(
            "alpha 66",
            &format!(
                r#"<div class="lc-panel lc-deck-{slug} lc-edge-plaque lc-preview-swatchbox"></div>"#
            ),
        );
        let a80 = swatch(
            "alpha 80",
            &format!(
                r#"<div class="lc-panel lc-deck-{slug} lc-edge-back lc-preview-swatchbox"></div>"#
            ),
        );
        let a99 = swatch(
            "alpha 99",
            &format!(
                r#"<div class="lc-panel lc-deck-{slug} lc-edge-strong lc-preview-swatchbox"></div>"#
            ),
        );
        body.push_str(&row(
            deck.label(),
            &[fill, ink, on_fill, a59, a66, a80, a99],
        ));
    }

    PreviewGroup {
        id: "ramps".to_string(),
        title: "Deck colour ramp reference".to_string(),
        note: "Wine is the row that matters: it is the only deck where fill and \
               ink differ, which is invisible in every other swatch on this page. \
               All four deck-tinted border-alpha rungs (59/66/80/99) are shown \
               side by side per deck, now that .lc-edge-strong binds the fourth."
            .to_string(),
        body,
    }
}

/// Every group rendered on the preview page, in template order. Groups are
/// pre-rendered HTML strings (CLAUDE.md: templates receive pre-computed
/// values) — `lc_preview.html` only iterates and emits `{{ g.body|safe }}`.
///
/// The `PublicView` projection is built once here (Task 2's Step 1 deferred
/// this: neither of its two groups touched a public component) and threaded
/// as `&PublicView`/`&PublicSeat` into every builder that needs one — never
/// raw `LastCallState` (spec §3.4). A public builder fed straight from
/// `LastCallState` is the regression to reject; a missing `PublicSeat`/
/// `PublicView` field a builder needed becomes a compile error at that call
/// site, which is the whole point of routing through the projection here
/// rather than discovering the gap in Plan A2.
fn build_groups() -> Vec<PreviewGroup> {
    let st = last_call::preview_state();
    let view = st.public_view();
    vec![
        card_matrix_group(),
        text_cases_group(),
        scene_group(),
        table_components_group(&view),
        shell_group(&view),
        hand_group_group(&st),
        flights_group(),
        deck_ramp_group(),
    ]
}

pub async fn preview_page(State(state): State<GameState>) -> impl IntoResponse {
    let tpl = LcPreviewTemplate {
        base_path: state.base_path.to_string(),
        groups: build_groups(),
    };
    Html(tpl.render().unwrap())
}
