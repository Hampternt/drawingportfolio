//! Last Call fragments as formatted strings, matching `render.rs`. Public
//! builders take `&PublicView`/`&PublicSeat` — never `&LastCallState` — so an
//! unrevealed card cannot reach a broadcast fragment by construction (spec
//! §3.4). Every root and attribute here is the §7.8 contract; changing one is
//! a breaking change for Plan A2 and Plan B.

use crate::last_call::{Card, Deck, PublicSeat, PublicView, Status, DECK_LOW_THRESHOLD};
use crate::render::html_escape;

/// The four `CardBack` sizes and their `data-size` slugs (§7.8): 16x24 for
/// the HandStrip, 44x62 for flight animation, 46x62 for a pile, 68x92 for a
/// DeckStack/DiscardSlot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackSize {
    Strip,
    Flight,
    Pile,
    Stack,
}

impl BackSize {
    /// The `data-size` value.
    pub fn slug(self) -> &'static str {
        match self {
            BackSize::Strip => "strip",
            BackSize::Flight => "flight",
            BackSize::Pile => "pile",
            BackSize::Stack => "stack",
        }
    }
}

// ---------------------------------------------------------------------
// §7.5 text handling — decided SERVER-SIDE, from the string.
// ---------------------------------------------------------------------

/// Titles at or under this many `chars()` get the largest ramp size
/// (`lc-title-lg`, 30px).
pub const TITLE_RAMP_MD_CHARS: usize = 14;
/// Titles at or under this many `chars()` (and over `TITLE_RAMP_MD_CHARS`)
/// get the mid ramp size (`lc-title-md`, 24px); longer titles get
/// `lc-title-sm` (20px).
pub const TITLE_RAMP_SM_CHARS: usize = 24;
/// Above this many title `chars()`, `is_truncated` marks the card
/// expandable.
pub const TITLE_CLAMP_CHARS: usize = 44;
/// Above this many body `chars()`, `is_truncated` marks the card expandable.
pub const BODY_CLAMP_CHARS: usize = 108;
/// `card_face` renders at most this many keyword chips before folding the
/// rest into a `+n` chip.
pub const MAX_KEYWORD_CHIPS: usize = 3;

/// The `.lc-face-title` size class for `title`, chosen from its character
/// count (not byte length — `chars()` throughout, so multi-byte titles like
/// "Königsschlucküberraschung" ramp on the same table as ASCII ones).
pub fn title_ramp_class(title: &str) -> &'static str {
    match title.chars().count() {
        0..=TITLE_RAMP_MD_CHARS => "lc-title-lg",
        n if n <= TITLE_RAMP_SM_CHARS => "lc-title-md",
        _ => "lc-title-sm",
    }
}

/// The server's conservative estimate that `CardFace`'s CSS clamp (Task 2)
/// will bite: title over `TITLE_CLAMP_CHARS`, body over `BODY_CLAMP_CHARS`,
/// or more than `MAX_KEYWORD_CHIPS` keywords. Deliberately marks early rather
/// than late — a card marked expandable that happened to fit costs nothing;
/// a clipped card that was not marked silently loses rules text.
pub fn is_truncated(card: &Card) -> bool {
    card.title.chars().count() > TITLE_CLAMP_CHARS
        || card.text.chars().count() > BODY_CLAMP_CHARS
        || card.keywords.len() > MAX_KEYWORD_CHIPS
}

// ---------------------------------------------------------------------
// Card primitives (§7.8)
// ---------------------------------------------------------------------

/// D.4/§7.8 `CardPip` — the single cost pip, nested by `card_face` so there
/// is one implementation of it.
pub fn card_pip(card: &Card) -> String {
    let slug = card.deck.slug();
    format!(
        r#"<span class="lc-pip lc-deck-{slug}" data-deck="{slug}" data-cost="{cost}">{cost}</span>"#,
        cost = card.cost,
    )
}

/// Renders up to `MAX_KEYWORD_CHIPS` keyword chips, folding any remainder
/// into a trailing `+n` chip. `cap: None` (the expanded variant) renders
/// every keyword and never folds.
fn keyword_chips(keywords: &[String], cap: Option<usize>) -> String {
    if keywords.is_empty() {
        return String::new();
    }
    let limit = cap.unwrap_or(keywords.len());
    let shown = &keywords[..keywords.len().min(limit)];
    let chips: String = shown
        .iter()
        .map(|k| format!(r#"<span class="lc-kw">{}</span>"#, html_escape(k)))
        .collect();
    let more = if keywords.len() > limit {
        format!(
            r#"<span class="lc-kw lc-kw-more">+{}</span>"#,
            keywords.len() - limit
        )
    } else {
        String::new()
    };
    format!(r#"<div class="lc-face-kws">{chips}{more}</div>"#)
}

/// The shared `card_face`/`card_face_expanded` body — kept as one function
/// so the two builders cannot drift out of sync.
fn face(card: &Card, expanded: bool) -> String {
    let slug = card.deck.slug();
    let label = card.deck.label();
    let ramp = title_ramp_class(&card.title);
    let pip = card_pip(card);
    let expandable = if !expanded && is_truncated(card) {
        " data-expandable"
    } else {
        ""
    };
    let expanded_cls = if expanded {
        " lc-cardface-expanded"
    } else {
        ""
    };
    let cap = if expanded {
        None
    } else {
        Some(MAX_KEYWORD_CHIPS)
    };
    let chips = keyword_chips(&card.keywords, cap);
    format!(
        r#"<article class="lc-cardface lc-deck-{slug}{expanded_cls}" data-card-id="{id}" data-deck="{slug}" data-cost="{cost}"{expandable}><div class="lc-face-top"><span class="lc-face-deck">{label}</span>{pip}</div><h3 class="lc-face-title {ramp}">{title}</h3><p class="lc-face-body">{text}</p>{chips}</article>"#,
        id = html_escape(&card.id),
        cost = card.cost,
        title = html_escape(&card.title),
        text = html_escape(&card.text),
    )
}

/// D.4/§7.8 `CardFace` — private, takes the viewer's own card. Fixed
/// 176px-tall face: title ramp, at most `MAX_KEYWORD_CHIPS` keyword chips
/// (`+n` beyond that), and `data-expandable` when `is_truncated(card)`.
pub fn card_face(card: &Card) -> String {
    face(card, false)
}

/// The expanded detail variant of `card_face`: height auto, no clamps, no
/// chip cap.
pub fn card_face_expanded(card: &Card) -> String {
    face(card, true)
}

/// D.4/§7.8 `CardMini`.
pub fn card_mini(card: &Card) -> String {
    let slug = card.deck.slug();
    format!(
        r#"<div class="lc-mini lc-deck-{slug}" data-card-id="{id}" data-deck="{slug}" data-cost="{cost}"><span class="lc-mini-cost">{cost}</span><span class="lc-mini-title">{title}</span></div>"#,
        id = html_escape(&card.id),
        cost = card.cost,
        title = html_escape(&card.title),
    )
}

/// D.4/§7.8 `CardBack` — public by construction: takes a `Deck`, never a
/// `Card`, so a card a viewer does not own can never be shown as a face.
pub fn card_back(deck: Deck, size: BackSize) -> String {
    let slug = deck.slug();
    format!(
        r#"<div class="lc-back lc-deck-{slug}" data-deck="{slug}" data-size="{size}"></div>"#,
        size = size.slug(),
    )
}

/// D.4/§7.8 `CardDot` — one 8px dot per vessel on the plaque.
pub fn card_dot(deck: Deck) -> String {
    let slug = deck.slug();
    format!(r#"<span class="lc-dot lc-deck-{slug}" data-deck="{slug}"></span>"#)
}

// ---------------------------------------------------------------------
// Table components (§7.6's component half)
// ---------------------------------------------------------------------

/// D.3 `HandStrip`. `n <= 8` renders `n` backs; `n > 8` renders 7 backs plus
/// a `+{n-7}` chip. Backs cycle through `decks` so a two-deck hand reads as
/// two-deck; an empty `decks` slice falls back to the Beer ramp rather than
/// panicking on `% 0` — reachable in Plan A2 between joining and registering
/// a drink.
pub fn hand_strip(decks: &[Deck], n: usize) -> String {
    let fallback = [Deck::Beer];
    let cycle: &[Deck] = if decks.is_empty() { &fallback } else { decks };
    let shown = if n <= 8 { n } else { 7 };
    let backs: String = (0..shown)
        .map(|i| card_back(cycle[i % cycle.len()], BackSize::Strip))
        .collect();
    let more = if n > 8 {
        format!(r#"<span class="lc-handstrip-more">+{}</span>"#, n - 7)
    } else {
        String::new()
    };
    let decks_csv = decks.iter().map(|d| d.slug()).collect::<Vec<_>>().join(",");
    let decks_attr = if decks.is_empty() {
        Deck::Beer.slug().to_string()
    } else {
        decks_csv
    };
    format!(
        r#"<div class="lc-handstrip" data-hand-size="{n}" data-decks="{decks_attr}">{backs}{more}<span class="lc-handstrip-count">{n}</span></div>"#
    )
}

/// The 3px plaque top rule. One deck fills it solid; two or more split it
/// into equal parts via `<i>` halves, because Task 2's CSS owns colour and a
/// gradient would need hex in the renderer. An empty slice renders a neutral
/// rule rather than panicking.
pub fn deck_rule(decks: &[Deck]) -> String {
    match decks {
        [] => r#"<div class="lc-rule lc-rule-1"></div>"#.to_string(),
        [only] => format!(
            r#"<div class="lc-rule lc-rule-1 lc-deck-{}"></div>"#,
            only.slug()
        ),
        many => {
            let parts: String = many
                .iter()
                .map(|d| format!(r#"<i class="lc-deck-{}"></i>"#, d.slug()))
                .collect();
            format!(r#"<div class="lc-rule lc-rule-2">{parts}</div>"#)
        }
    }
}

/// D.1 `PlayerPlaque`, all five states from a projected seat — never an
/// `LcPlayer`, the plaque is a public surface (spec §3.4).
///
/// `locked` and `drawing` come straight from `PublicSeat`; `eliminated` from
/// `status == Status::Eliminated`. `is-hit` is deliberately never emitted
/// here: it is a transient event (the shake + HP flash on a just-landed hit),
/// not a projected state, so a broadcast snapshot has no way to say "was hit
/// just now" without leaking timing into state. Slice 3 adds and removes the
/// class from the client; the preview adds it by hand to demonstrate the
/// animation.
pub fn player_plaque(seat: &PublicSeat) -> String {
    let decks = seat.decks();
    let first_slug = decks.first().map(|d| d.slug()).unwrap_or("beer");
    let eliminated = seat.status == Status::Eliminated;

    let mut state_classes = String::new();
    if seat.locked {
        state_classes.push_str(" is-locked");
    }
    if seat.drawing {
        state_classes.push_str(" is-drawing");
    }
    if eliminated {
        state_classes.push_str(" is-eliminated");
    }

    let lock_tick = if seat.locked {
        r#"<span class="lc-lock-tick">&#9679;</span>"#
    } else {
        ""
    };
    let hp_display = if eliminated {
        "GHOST".to_string()
    } else {
        seat.hp.to_string()
    };

    let dots: String = decks.iter().map(|&d| card_dot(d)).collect();
    let deck_names = decks
        .iter()
        .map(|d| d.label())
        .collect::<Vec<_>>()
        .join(" + ");
    let draws_badge = if seat.draws == 0 {
        String::new()
    } else {
        format!(r#"<span class="lc-draws">{}</span>"#, seat.draws)
    };
    let decks_attr = decks.iter().map(|d| d.slug()).collect::<Vec<_>>().join(",");

    format!(
        r#"<div class="lc-plaque lc-deck-{first_slug}{state_classes}" data-seat="{seat_n}" data-decks="{decks_attr}" data-hp="{hp}" data-status="{status}" data-hand-size="{hand_len}" data-flight-anchor="plaque-seat-{seat_n}">{rule}<div class="lc-identity"><span class="lc-name">{name}{lock_tick}</span><span class="lc-hp">{hp_display}</span></div><div class="lc-drinks">{dots}<span class="lc-decknames">{deck_names}</span>{draws_badge}</div>{hand_strip}</div>"#,
        seat_n = seat.seat,
        hp = seat.hp,
        status = seat.status.slug(),
        hand_len = seat.hand_len,
        rule = deck_rule(&decks),
        name = html_escape(&seat.name),
        hand_strip = hand_strip(&decks, seat.hand_len),
    )
}

/// D.4 `DeckStack`. `data-low` under `DECK_LOW_THRESHOLD` (and above zero);
/// `data-empty` at 0, where the count reads `RESHUFFLE` instead of `0`.
/// Attributes are emitted as bare presence attributes, never
/// `data-low="false"` — the CSS selects on `[data-low]`, so a `"false"` value
/// would style every stack.
pub fn deck_stack(deck: Deck, count: u16) -> String {
    let slug = deck.slug();
    let label = deck.label();
    let low = if count > 0 && count < DECK_LOW_THRESHOLD {
        " data-low"
    } else {
        ""
    };
    let empty = if count == 0 { " data-empty" } else { "" };
    let count_text = if count == 0 {
        "RESHUFFLE".to_string()
    } else {
        count.to_string()
    };
    format!(
        r#"<div class="lc-deckstack lc-deck-{slug}" data-deck="{slug}" data-count="{count}"{low}{empty} data-flight-anchor="deck-{slug}">{back}<span class="lc-deckstack-count">{count_text}</span><span class="lc-deckstack-name">{label}</span></div>"#,
        back = card_back(deck, BackSize::Stack),
    )
}

/// D.4 `DiscardSlot` — a destination, not a deck: same footprint as a
/// `DeckStack` but a dashed hairline, no grid and a neutral count. Carries no
/// `data-deck`.
pub fn discard_slot(count: usize) -> String {
    format!(
        r#"<div class="lc-discard" data-count="{count}" data-flight-anchor="discard"><div class="lc-back" data-size="stack"></div><span class="lc-deckstack-count">{count}</span><span class="lc-deckstack-name">DISCARD</span></div>"#
    )
}

// ---------------------------------------------------------------------
// Shell components, from the projection
// ---------------------------------------------------------------------

/// The banner: beat label + hue class (one decision, returned as the whole
/// element so it can never be split across the renderer and a template) plus
/// round/beat-index meta.
pub fn lc_banner(view: &PublicView) -> String {
    let beat = view.beat;
    format!(
        r#"<div class="lc-banner lc-beat-{hue}" id="lc-banner" data-beat="{slug}" data-round="{round}"><span class="lc-banner-beat">{label}</span><span class="lc-banner-meta">ROUND {round} &middot; BEAT {index} OF 6</span></div>"#,
        hue = beat.hue(),
        slug = beat.slug(),
        round = view.round,
        label = beat.label(),
        index = beat.index(),
    )
}

/// The beat timer. Its inline `style` sets only a duration custom property —
/// no colour, so the no-hex rule holds.
pub fn beat_timer(duration_ms: u32, elapsed_ms: u32) -> String {
    let remaining = duration_ms.saturating_sub(elapsed_ms);
    format!(
        r#"<div id="lc-beat-timer" class="lc-timer" data-duration-ms="{duration_ms}" data-elapsed-ms="{elapsed_ms}" style="--lc-beat-ms:{remaining}ms"></div>"#
    )
}

/// The payload of the `LcPublic` SSE message (Plan A2). Plan A's body is the
/// banner plus the seq marker; Plan A2 and Plan B extend the body, never the
/// signature. The `<template data-lc-banner>` wrapper mirrors the existing
/// `room` event's `<template data-topbar>` convention in `room.html` — one
/// SSE message carrying several destinations.
pub fn lc_public_panel(view: &PublicView) -> String {
    format!(
        r#"<div data-lc-public data-seq="{seq}"><template data-lc-banner>{banner}</template></div>"#,
        seq = view.seq,
        banner = lc_banner(view),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::{preview_state, Beat, LastCallState};
    use crate::lc_cards::{self, CATALOG};

    fn no_hex(s: &str) {
        assert!(!s.contains('#'), "unexpected hex colour in output: {s}");
    }

    #[test]
    fn test_one_card_renders_at_five_sizes_in_five_deck_colours() {
        for deck in Deck::ALL {
            let slug = deck.slug();
            let card = &lc_cards::deck_cards(deck)[0];

            let face_html = card_face(card);
            assert!(face_html.contains("lc-cardface"));
            assert!(face_html.contains(&format!("lc-deck-{slug}")));
            assert!(face_html.contains(&card.title));
            assert!(face_html.contains(&format!(r#"class="lc-pip lc-deck-{slug}""#)));
            no_hex(&face_html);

            let pip = card_pip(card);
            assert!(pip.contains("lc-pip"));
            assert!(pip.contains(&format!("lc-deck-{slug}")));
            assert!(pip.contains(&format!(">{}<", card.cost)));
            no_hex(&pip);

            let mini = card_mini(card);
            assert!(mini.contains("lc-mini"));
            assert!(mini.contains(&format!("lc-deck-{slug}")));
            no_hex(&mini);

            for size in [
                BackSize::Strip,
                BackSize::Flight,
                BackSize::Pile,
                BackSize::Stack,
            ] {
                let back = card_back(deck, size);
                assert!(back.contains("lc-back"));
                assert!(back.contains(&format!("lc-deck-{slug}")));
                assert!(back.contains(&format!(r#"data-size="{}""#, size.slug())));
                no_hex(&back);
            }

            let dot = card_dot(deck);
            assert!(dot.contains("lc-dot"));
            assert!(dot.contains(&format!("lc-deck-{slug}")));
            no_hex(&dot);
        }
    }

    #[test]
    fn test_contract_attributes_are_present() {
        let card = &lc_cards::deck_cards(Deck::Beer)[0];
        let face_html = card_face(card);
        assert!(face_html.contains("data-card-id"));
        assert!(face_html.contains("data-deck"));
        assert!(face_html.contains("data-cost"));

        let pip = card_pip(card);
        assert!(pip.contains("data-deck"));
        assert!(pip.contains("data-cost"));

        let mini = card_mini(card);
        assert!(mini.contains("data-card-id"));
        assert!(mini.contains("data-deck"));
        assert!(mini.contains("data-cost"));

        let back = card_back(Deck::Beer, BackSize::Stack);
        assert!(back.contains("data-deck"));
        assert!(back.contains("data-size"));

        let dot = card_dot(Deck::Beer);
        assert!(dot.contains("data-deck"));

        let view = preview_state().public_view();
        let plaque = player_plaque(&view.seats[0]);
        assert!(plaque.contains("data-seat"));
        assert!(plaque.contains("data-decks"));
        assert!(plaque.contains("data-hp"));
        assert!(plaque.contains("data-status"));
        assert!(plaque.contains("data-hand-size"));

        let strip = hand_strip(&[Deck::Beer], 3);
        assert!(strip.contains("data-hand-size"));
        assert!(strip.contains("data-decks"));

        let stack = deck_stack(Deck::Wine, 10);
        assert!(stack.contains("data-deck"));
        assert!(stack.contains("data-count"));

        let discard = discard_slot(2);
        assert!(discard.contains("data-count"));

        let banner = lc_banner(&view);
        assert!(banner.contains("data-beat"));
        assert!(banner.contains("data-round"));

        let timer = beat_timer(30_000, 5_000);
        assert!(timer.contains("data-duration-ms"));
        assert!(timer.contains("data-elapsed-ms"));
    }

    #[test]
    fn test_no_builder_emits_behaviour() {
        let view = preview_state().public_view();
        let card = &lc_cards::deck_cards(Deck::Cider)[3]; // cider-04, 6 keywords
        let outputs = [
            card_face(card),
            card_face_expanded(card),
            card_pip(card),
            card_mini(card),
            card_back(Deck::Beer, BackSize::Stack),
            card_dot(Deck::Beer),
            player_plaque(&view.seats[0]),
            hand_strip(&[Deck::Beer, Deck::Wine], 9),
            deck_rule(&[Deck::Beer, Deck::Wine]),
            deck_stack(Deck::Wine, 4),
            discard_slot(3),
            lc_banner(&view),
            beat_timer(30_000, 5_000),
            lc_public_panel(&view),
        ];
        for out in outputs {
            for banned in ["hx-post", "hx-get", "hx-swap", "onclick", "href"] {
                assert!(
                    !out.contains(banned),
                    "found forbidden `{banned}` in: {out}"
                );
            }
        }
    }

    #[test]
    fn test_backs_and_dots_carry_no_card_identity() {
        for def in CATALOG.iter() {
            let card = lc_cards::card_by_id(def.id).unwrap();
            for size in [
                BackSize::Strip,
                BackSize::Flight,
                BackSize::Pile,
                BackSize::Stack,
            ] {
                let back = card_back(card.deck, size);
                assert!(!back.contains(&card.id));
                assert!(!back.contains(&card.title));
            }
            let dot = card_dot(card.deck);
            assert!(!dot.contains(&card.id));
            assert!(!dot.contains(&card.title));
        }
    }

    #[test]
    fn test_card_face_escapes_text() {
        let card = Card {
            id: "x-01".to_string(),
            deck: Deck::Beer,
            kind: crate::last_call::CardKind::Atk,
            cost: 1,
            targets: "one".to_string(),
            title: "<script>x</script>".to_string(),
            text: "harmless".to_string(),
            keywords: Vec::new(),
            duration: None,
        };
        let html = card_face(&card);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x</script>"));
    }

    #[test]
    fn test_title_ramp_thresholds() {
        let cases: [(&str, &str); 7] = [
            ("Neat", "lc-title-lg"),
            ("Second Wind", "lc-title-lg"),
            ("Fourteen chars", "lc-title-lg"),
            ("Fifteen chars!!", "lc-title-md"),
            ("Twenty-four characters!!", "lc-title-md"),
            ("Twenty-five characters!!!", "lc-title-sm"),
            ("The Long Sober Look Across The Table", "lc-title-sm"),
            // NOTE: array size below covers this + the extra case
        ];
        for (title, expected) in cases {
            assert_eq!(title_ramp_class(title), expected, "title={title}");
        }

        let unicode = "Königsschlucküberraschung";
        assert_eq!(unicode.chars().count(), 25);
        assert_eq!(title_ramp_class(unicode), "lc-title-sm");

        let card = Card {
            id: "u-01".to_string(),
            deck: Deck::Wine,
            kind: crate::last_call::CardKind::Util,
            cost: 1,
            targets: "one".to_string(),
            title: unicode.to_string(),
            text: "text".to_string(),
            keywords: Vec::new(),
            duration: None,
        };
        let html = card_face(&card);
        assert!(html.contains(r#"class="lc-face-title lc-title-sm""#));
    }

    #[test]
    fn test_is_truncated_marks_expandable() {
        fn card_with(title_len: usize, body_len: usize, kw_count: usize) -> Card {
            Card {
                id: "t-01".to_string(),
                deck: Deck::Beer,
                kind: crate::last_call::CardKind::Atk,
                cost: 1,
                targets: "one".to_string(),
                title: "a".repeat(title_len),
                text: "b".repeat(body_len),
                keywords: (0..kw_count).map(|i| format!("k{i}")).collect(),
                duration: None,
            }
        }

        assert!(!is_truncated(&card_with(44, 10, 0)));
        assert!(is_truncated(&card_with(45, 10, 0)));

        assert!(!is_truncated(&card_with(5, 108, 0)));
        assert!(is_truncated(&card_with(5, 109, 0)));

        assert!(!is_truncated(&card_with(5, 10, 3)));
        assert!(is_truncated(&card_with(5, 10, 4)));

        let clean = card_with(5, 10, 0);
        assert!(!card_face(&clean).contains("data-expandable"));

        let dirty = card_with(45, 10, 0);
        assert!(card_face(&dirty).contains("data-expandable"));

        let wine01 = lc_cards::card_by_id("wine-01").unwrap();
        assert!(is_truncated(&wine01));
        assert!(card_face(&wine01).contains("data-expandable"));
    }

    #[test]
    fn test_keyword_chips_cap_at_three() {
        let none = Card {
            id: "k-00".to_string(),
            deck: Deck::Beer,
            kind: crate::last_call::CardKind::Atk,
            cost: 1,
            targets: "one".to_string(),
            title: "None".to_string(),
            text: "text".to_string(),
            keywords: Vec::new(),
            duration: None,
        };
        let html = card_face(&none);
        assert!(!html.contains("lc-kw\""));
        assert!(!html.contains("lc-kw-more"));

        let three = Card {
            keywords: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            ..none.clone()
        };
        let html = card_face(&three);
        assert_eq!(html.matches(r#"class="lc-kw""#).count(), 3);
        assert!(!html.contains("lc-kw-more"));

        let cider04 = lc_cards::card_by_id("cider-04").unwrap();
        assert_eq!(cider04.keywords.len(), 6);
        let html = card_face(&cider04);
        assert_eq!(html.matches(r#"class="lc-kw""#).count(), 3);
        assert!(html.contains(r#"class="lc-kw lc-kw-more">+3<"#));
        for kw in &cider04.keywords[..3] {
            assert!(html.contains(kw));
        }
    }

    #[test]
    fn test_card_face_expanded_drops_clamps_and_caps() {
        let cider04 = lc_cards::card_by_id("cider-04").unwrap();
        let expanded = card_face_expanded(&cider04);
        assert!(expanded.contains("lc-cardface-expanded"));
        assert!(expanded.contains(&cider04.text));
        for kw in &cider04.keywords {
            assert!(expanded.contains(kw));
        }
        assert!(!expanded.contains("lc-kw-more"));

        let normal = card_face(&cider04);
        assert!(normal.contains("data-expandable"));
        assert!(normal.contains("lc-kw-more"));

        let ramp = title_ramp_class(&cider04.title);
        assert!(expanded.contains(&format!("lc-face-title {ramp}")));
        assert!(normal.contains(&format!("lc-face-title {ramp}")));
    }

    #[test]
    fn test_hand_strip_split() {
        let cases: [(usize, usize, bool); 6] = [
            (0, 0, false),
            (1, 1, false),
            (8, 8, false),
            (9, 7, true),
            (12, 7, true),
            (30, 7, true),
        ];
        for (n, backs, has_more) in cases {
            let html = hand_strip(&[Deck::Beer], n);
            assert_eq!(
                html.matches("lc-back").count(),
                backs,
                "n={n} backs mismatch"
            );
            assert_eq!(
                html.contains("lc-handstrip-more"),
                has_more,
                "n={n} more-chip mismatch"
            );
            if has_more {
                assert!(html.contains(&format!("+{}", n - 7)));
            }
            assert!(html.contains(&format!(r#"data-hand-size="{n}""#)));
            assert!(html.contains(&format!(">{n}<")));
        }
    }

    #[test]
    fn test_hand_strip_cycles_deck_colours() {
        let html = hand_strip(&[Deck::Beer, Deck::Wine], 4);
        let order: Vec<&str> = html
            .match_indices("lc-deck-")
            .map(|(i, _)| &html[i..i + 12])
            .collect();
        assert!(order[0].starts_with("lc-deck-beer"));
        assert!(order[1].starts_with("lc-deck-wine"));
        assert!(order[2].starts_with("lc-deck-beer"));
        assert!(order[3].starts_with("lc-deck-wine"));
        assert!(html.contains(r#"data-decks="beer,wine""#));

        let empty = hand_strip(&[], 3);
        assert_eq!(empty.matches("lc-deck-beer").count(), 3);
    }

    #[test]
    fn test_deck_rule_splits_for_multi_deck() {
        let one = deck_rule(&[Deck::Wine]);
        assert!(one.contains("lc-rule-1"));
        assert!(one.contains("lc-deck-wine"));
        assert!(!one.contains("<i"));
        no_hex(&one);

        let two = deck_rule(&[Deck::Beer, Deck::Wine]);
        assert!(two.contains("lc-rule-2"));
        assert_eq!(two.matches("<i").count(), 2);
        no_hex(&two);

        let none = deck_rule(&[]);
        no_hex(&none);
    }

    #[test]
    fn test_deck_stack_states() {
        let cases: [(u16, bool, bool, &str); 5] = [
            (21, false, false, "21"),
            (5, false, false, "5"),
            (4, true, false, "4"),
            (1, true, false, "1"),
            (0, false, true, "RESHUFFLE"),
        ];
        for (count, low, empty, text) in cases {
            let html = deck_stack(Deck::Beer, count);
            assert_eq!(html.contains("data-low"), low, "count={count}");
            assert_eq!(html.contains("data-empty"), empty, "count={count}");
            assert!(html.contains(&format!(">{text}<")), "count={count}");
            assert!(!html.contains(r#"data-low="false""#));
            assert!(!html.contains(r#"data-empty="false""#));
        }
    }

    fn seat_fixture() -> PublicSeat {
        preview_state().public_view().seats[0].clone()
    }

    #[test]
    fn test_plaque_five_states() {
        let mut seat = seat_fixture();
        seat.locked = false;
        seat.drawing = false;
        seat.status = Status::Alive;

        let idle = player_plaque(&seat);
        for cls in ["is-locked", "is-drawing", "is-hit", "is-eliminated"] {
            assert!(!idle.contains(cls), "idle should not contain {cls}");
        }

        seat.locked = true;
        let locked = player_plaque(&seat);
        assert!(locked.contains("is-locked"));
        assert!(locked.contains("lc-lock-tick"));
        for def in CATALOG.iter() {
            assert!(
                !locked.contains(def.id),
                "locked plaque leaked card id {}",
                def.id
            );
        }
        seat.locked = false;

        seat.drawing = true;
        let drawing = player_plaque(&seat);
        assert!(drawing.contains("is-drawing"));
        seat.drawing = false;

        seat.status = Status::Eliminated;
        let eliminated = player_plaque(&seat);
        assert!(eliminated.contains("is-eliminated"));
        assert!(eliminated.contains(r#"data-status="eliminated""#));
        assert!(eliminated.contains("GHOST"));

        // No fixture ever produces is-hit — player_plaque never emits it.
        let view = preview_state().public_view();
        for s in &view.seats {
            assert!(!player_plaque(s).contains("is-hit"));
        }
    }

    #[test]
    fn test_plaque_carries_its_motion_anchor() {
        let view = preview_state().public_view();
        let mut seat0 = view.seats[0].clone();
        seat0.seat = 0;
        assert!(player_plaque(&seat0).contains(r#"data-flight-anchor="plaque-seat-0""#));

        let mut seat7 = view.seats[0].clone();
        seat7.seat = 7;
        assert!(player_plaque(&seat7).contains(r#"data-flight-anchor="plaque-seat-7""#));

        assert!(deck_stack(Deck::Wine, 4).contains(r#"data-flight-anchor="deck-wine""#));
        assert!(discard_slot(3).contains(r#"data-flight-anchor="discard""#));
    }

    #[test]
    fn test_plaque_multi_deck() {
        let mut st = LastCallState::new(vec![(1, "alice".into())], 1);
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(1, Deck::Liquor, "shot").unwrap();
        let seat = &st.public_view().seats[0];

        let html = player_plaque(seat);
        assert_eq!(html.matches("lc-dot").count(), 2);
        assert!(html.contains("BEER + LIQUOR"));
        assert!(html.contains(r#"data-decks="beer,liquor""#));
        assert!(html.contains("lc-rule-2"));
    }

    #[test]
    fn test_plaque_draw_badge() {
        let seat = PublicSeat {
            seat: 0,
            player_id: 1,
            name: "alice".to_string(),
            hp: 15,
            status: Status::Alive,
            vessels: Vec::new(),
            hand_len: 0,
            locked: false,
            drawing: false,
            draws: 3,
        };
        let html = player_plaque(&seat);
        assert!(html.contains(r#"<span class="lc-draws">3</span>"#));

        let seat0 = PublicSeat { draws: 0, ..seat };
        let html0 = player_plaque(&seat0);
        assert!(!html0.contains("lc-draws"));
    }

    #[test]
    fn test_lc_banner_beat_hue_and_meta() {
        let mut st = LastCallState::new(vec![(1, "a".into())], 1);
        st.round = 6;
        st.beat = Beat::Lock;
        let view = st.public_view();
        let html = lc_banner(&view);
        assert!(html.contains("lc-beat-violet"));
        assert!(html.contains("LOCK"));
        assert!(html.contains("ROUND 6"));
        assert!(html.contains("BEAT 4 OF 6"));
        assert!(html.contains(r#"data-beat="lock""#));
        assert!(html.contains(r#"data-round="6""#));

        st.beat = Beat::Draw;
        let html = lc_banner(&st.public_view());
        assert!(html.contains("lc-beat-amber"));
        assert!(html.contains("BEAT 1 OF 6"));

        st.beat = Beat::Deal;
        let html = lc_banner(&st.public_view());
        assert!(html.contains("lc-beat-amber"));
        assert!(html.contains("BEAT 2 OF 6"));
    }

    #[test]
    fn test_lc_public_panel_carries_seq_and_no_hands() {
        let mut st = LastCallState::new(
            vec![
                (1, "alice".into()),
                (2, "bob".into()),
                (3, "cara".into()),
                (4, "dev".into()),
                (5, "erin".into()),
            ],
            1,
        );
        st.set_vessel(1, Deck::Beer, "can").unwrap();
        st.set_vessel(2, Deck::Cider, "bottle").unwrap();
        st.set_vessel(3, Deck::Wine, "glass").unwrap();
        st.set_vessel(4, Deck::Liquor, "shot").unwrap();
        st.set_vessel(5, Deck::Soft, "any").unwrap();
        st.beat = Beat::Lock;

        let view = st.public_view();
        let html = lc_public_panel(&view);
        assert!(html.contains("data-seq"));
        assert!(html.contains("data-lc-banner"));
        for banned in [
            "beer-01",
            "cider-01",
            "wine-01",
            "liquor-01",
            "soft-01",
            "Nudge",
            "Sticky",
        ] {
            assert!(!html.contains(banned), "leaked {banned}");
        }
    }
}
