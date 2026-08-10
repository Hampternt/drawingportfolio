//! Last Call fragments as formatted strings, matching `render.rs`. Public
//! builders take `&PublicView`/`&PublicSeat` — never `&LastCallState` — so an
//! unrevealed card cannot reach a broadcast fragment by construction (spec
//! §3.4). Every root and attribute here is the §7.8 contract; changing one is
//! a breaking change for Plan A2 and Plan B.

use crate::last_call::{Card, Deck, PublicSeat, PublicView, Status, DECK_LOW_THRESHOLD};
use crate::lc_layout::{seat_positions, view_index, SeatPos};
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
///
/// Deliberately drops `data-expandable` (the brief's prose reads as "keep
/// it," but the shipped behaviour is judged better and is recorded here so
/// Plan A2 does not "restore" it from the brief): an already-expanded card
/// has nothing left to expand, and if Plan A2 binds the expand gesture to
/// `[data-expandable]`, leaving the attribute on would make the detail view
/// re-trigger itself.
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
    // data-decks is the comma-joined slugs, empty string for an empty slice —
    // matching player_plaque's data-decks exactly, so a vessel-less seat's
    // plaque and its nested hand strip never disagree about the seat's deck
    // set. The Beer-ramp fallback above is visual only; it has no attribute
    // counterpart.
    let decks_csv = decks.iter().map(|d| d.slug()).collect::<Vec<_>>().join(",");
    format!(
        r#"<div class="lc-handstrip" data-hand-size="{n}" data-decks="{decks_csv}">{backs}{more}<span class="lc-handstrip-count">{n}</span></div>"#
    )
}

/// The 3px plaque top rule. One deck fills it solid; two or more split it
/// into equal parts via `<i>` halves, because Task 2's CSS owns colour and a
/// gradient would need hex in the renderer. An empty slice renders without
/// panicking, via `.lc-rule-1`'s own hairline fallback (`var(--lc-fill,
/// var(--lc-hair))`) — but that fallback only actually shows through when
/// `deck_rule` is rendered standalone (e.g. Plan A-vis's preview page).
/// Inside `player_plaque`, the root always carries `lc-deck-{first_slug}`
/// (defaulting to Beer), so an empty-decks rule resolves through inheritance
/// to that deck's fill, not the hairline — this case is documented, not
/// "neutral" in a plaque context.
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

/// The payload of the `LcPublic` SSE message (Plan A2/Plan B). Plan A's body
/// is the banner plus the seq marker; Plan A2 added the banner template,
/// Plan B (Task 4) adds a second `<template data-lc-screen>` carrying the
/// big-screen felt so `lc_screen.html` can repaint from the same message —
/// no new SSE event, no new publish, the frame count is unchanged. Both
/// `<template>` wrappers mirror the existing `room` event's
/// `<template data-topbar>` convention in `room.html` — one SSE message
/// carrying several destinations. `lc_screen_panel` is a pure string build
/// (no I/O, no `.await`), so this stays safe to call from `broadcast_lc`,
/// which must remain awaitless (`1e742d4`).
pub fn lc_public_panel(view: &PublicView) -> String {
    format!(
        r#"<div data-lc-public data-seq="{seq}"><template data-lc-banner>{banner}</template><template data-lc-screen>{screen}</template></div>"#,
        seq = view.seq,
        banner = lc_banner(view),
        screen = lc_screen_panel(view),
    )
}

// ---------------------------------------------------------------------
// Plan A2 additions — Task 2. Plan A shipped the contract; this plan owns
// the builder because it posts to routes only this plan owns.
// ---------------------------------------------------------------------

/// One row of the plain setup form: who, their handicap, their registered
/// decks.
#[derive(Clone, Debug)]
pub struct SetupRow {
    pub player_id: i64,
    pub name: String,
    pub handicap_pct: u16,
    pub decks: Vec<Deck>,
}

/// The private hand fragment's body — the §7.8 "Hand region" component.
/// Not broadcast: served only to its own viewer by
/// `GET /room/{code}/lastcall/hand`.
///
/// Handicap rows are **not** gated on `player_id == me` — spec §2, item 2:
/// any room member may set any player's handicap, because the table sets
/// handicaps rather than each player declaring themselves a lightweight.
/// `me` is used only to append " (you)" to the viewer's own row.
pub fn lc_hand_pane(
    base_path: &str,
    code: &str,
    me: i64,
    hand: &[Card],
    rows: &[SetupRow],
    seq: u64,
) -> String {
    let deck_options: String = Deck::ALL
        .iter()
        .map(|d| format!(r#"<option value="{}">{}</option>"#, d.slug(), d.label()))
        .collect();

    let handicap_rows: String = rows
        .iter()
        .map(|row| {
            let you = if row.player_id == me { " (you)" } else { "" };
            let dots: String = row.decks.iter().map(|&d| card_dot(d)).collect();
            format!(
                r#"<form class="lc-setup-row" method="post" action="{base_path}/room/{code}/lastcall/handicap"><input type="hidden" name="target" value="{player_id}"><span>{name}{you}</span><span class="lc-setup-decks">{dots}</span><input type="number" name="handicap_pct" min="25" max="300" step="5" value="{handicap_pct}"><button type="submit">SET</button></form>"#,
                player_id = row.player_id,
                name = html_escape(&row.name),
                handicap_pct = row.handicap_pct,
            )
        })
        .collect();

    let cards: String = if hand.is_empty() {
        r#"<p class="lc-empty">Register your drink to be dealt a hand.</p>"#.to_string()
    } else {
        hand.iter().map(card_face).collect()
    };

    format!(
        r#"<div id="lc-hand" data-seq="{seq}" data-count="{count}" data-flight-anchor="hand"><section class="lc-setup"><h2>Your drink</h2><form method="post" action="{base_path}/room/{code}/lastcall/vessel"><select name="deck">{deck_options}</select><input name="container" maxlength="24" placeholder="50cl can"><button type="submit">REGISTER</button></form><h2>Handicaps</h2>{handicap_rows}</section>{cards}</div>"#,
        count = hand.len(),
    )
}

// ---------------------------------------------------------------------
// Plan B additions — Task 3. Two assemblers over Task 1's ring geometry
// and Plan A's component builders. Neither authors a new `.lc-*`
// component; both only lay Plan A's plaque/deck-stack/discard-slot/
// card-back out on Task 1's ring.
// ---------------------------------------------------------------------

/// Where seat `seat` sits on an `n`-seat ring, from `me`'s point of view.
/// `me: None` is identity (the big screen; a mini-table spectator).
///
/// `.get()`, never `[]`: until Task 6 lands the `MAX_SEATS` ceiling, a
/// stale oversized state can hand this more seats than `seat_positions`
/// has rows for at that count. Render short rather than panic — this is
/// the one formula both `lc_screen_panel` and `lc_mini_table` share, one
/// argument different.
fn seat_pos(n: usize, seat: usize, me: Option<usize>) -> Option<SeatPos> {
    seat_positions(n).get(view_index(seat, me, n)).copied()
}

/// F.2 big-screen body — the three-column grid plus the flight layer.
/// Header meta (mark, code, round, banner) is the template's (Task 4);
/// this is `.lc-screen-grid` (seat-order rail, felt ring, deck rail)
/// followed by `#lc-flights` as a sibling, not nested inside it — see the
/// comment at its call site below. Absolute seat order throughout — a
/// spectator has no seat, so nothing here rotates.
pub fn lc_screen_panel(view: &PublicView) -> String {
    let n = view.seats.len();

    let seatorder_rows: String = view
        .seats
        .iter()
        .map(|seat| {
            let first_attr = if seat.seat == view.first_seat {
                " data-first"
            } else {
                ""
            };
            // "the live one" is Status::Alive; compared as enums, never as
            // a hardcoded "alive" string.
            let out_attr = if seat.status != Status::Alive {
                " data-out"
            } else {
                ""
            };
            format!(
                r#"<div class="lc-seatorder-row"{first_attr}{out_attr}><span class="lc-seatorder-n">{n}</span><span class="lc-seatorder-name">{name}</span></div>"#,
                n = seat.seat + 1,
                name = html_escape(&seat.name.to_uppercase()),
            )
        })
        .collect();
    let left_rail = format!(
        r#"<div class="lc-rail lc-rail-left"><div class="lc-rail-kicker">SEAT ORDER</div><div class="lc-seatorder">{seatorder_rows}</div></div>"#
    );

    let seats_html: String = view
        .seats
        .iter()
        .filter_map(|seat| {
            seat_pos(n, seat.seat, None).map(|(l, t)| {
                format!(
                    r#"<div class="lc-seat" style="left:{l}%;top:{t}%" data-seat="{s}" data-flight-anchor="seat-{s}">{plaque}</div>"#,
                    s = seat.seat,
                    plaque = player_plaque(seat),
                )
            })
        })
        .collect();
    let stage = format!(
        r#"<div class="lc-stage"><div id="lc-felt"></div><div class="lc-ring">{seats_html}</div></div>"#
    );

    let deck_stacks: String = view
        .deck_counts
        .iter()
        .map(|&(deck, count)| deck_stack(deck, count))
        .collect();
    let right_rail = format!(
        r#"<div class="lc-rail lc-rail-right"><div class="lc-rail-kicker">DECKS LEFT</div><div class="lc-rail-decks">{deck_stacks}{discard}</div></div>"#,
        discard = discard_slot(view.discard_count),
    );

    // #lc-flights is a SIBLING of .lc-screen-grid, not nested inside it and
    // not inside .lc-stage: a flight travels from a deck-stack anchor in
    // the RIGHT RAIL to a seat anchor in the STAGE, so it must not be
    // confined to .lc-stage's box (overflow:hidden, only the grid's middle
    // column) — and .lc-screen-grid itself also carries overflow:hidden,
    // so nesting it there would only escape the clip by relying on
    // clip-escape-via-containing-block rather than avoiding the clipping
    // ancestor outright. Task 2's CSS gives body.lc-screen (the template's
    // root, wrapping this grid — Task 4's territory) `position: relative`
    // expressly "so #lc-flights needs it": that is its containing block,
    // and returning #lc-flights as the last sibling here makes it the last
    // child of that root once Task 4 wraps it. The brief's Step 1.2 places
    // it inside `.lc-stage`; this deviates from the literal text to match
    // the CSS Task 2 actually shipped. See the task report.
    format!(
        r#"<div class="lc-screen-grid">{left_rail}{stage}{right_rail}</div><div id="lc-flights"></div>"#
    )
}

/// F.3 phone mini table, rotated so `me` sits at bottom-centre — the whole
/// point of the route in Task 5. `me` is the viewer's own seat, or `None`
/// for a member who is not seated.
///
/// The centre column's event/quest/discard rows (`.lc-minitable-rows`
/// etc.) are deliberately not filled here: no Plan A builder renders a
/// dot+label+count row, so doing so would be authoring a new component,
/// which this plan does not do. See the task report for the full
/// adjudication.
///
/// # The phone's only flight layer lives in here — read before firing one
///
/// The `#lc-flights` emitted below is the F.1 shell's *sole* layer:
/// `lc_hand_pane` emits none, and `lc_room.html` marks no
/// `[data-lc-scene]`, so `ensureLayer` falls back to `document.body` and
/// then finds this one by descendant query. It therefore sits inside
/// `<section data-lc-pane="table">`, which is `hidden` unless the viewer
/// has the TABLE tab open.
///
/// Harmless today: nothing on the phone calls `lcFlight` — the beat state
/// machine's transitions are stubbed. **The task that first fires a phone
/// flight must move the layer out of this fragment before doing so.** A
/// flight appended into a `display: none` host never animates, so
/// `animationend` never fires, the node is never removed, and `onArrive`
/// never runs. Marking `body.lc` with `data-lc-scene` is *not* the fix —
/// `ensureLayer` searches the host's descendants, so it would still reach
/// this layer.
///
/// Where it should move to is that task's call, not this one's: a
/// deck-to-seat flight belongs to the table, a draw-to-hand flight to the
/// hand, and only the beat loop knows which it fires. Doing it here would
/// also re-open Task 3's builder, its `lc_render` tests, and the
/// containing-block reasoning recorded at `lastcall.css:551-563`.
pub fn lc_mini_table(view: &PublicView, me: Option<usize>) -> String {
    let n = view.seats.len();

    let chips: String = view
        .seats
        .iter()
        .filter_map(|seat| {
            seat_pos(n, seat.seat, me).map(|(l, t)| {
                let locked_attr = if seat.locked { " data-locked" } else { "" };
                let out_attr = if seat.status != Status::Alive {
                    " data-out"
                } else {
                    ""
                };
                let me_attr = if Some(seat.seat) == me { " data-me" } else { "" };
                format!(
                    r#"<div class="lc-minitable-chip" style="left:{l}%;top:{t}%" data-seat="{s}" data-flight-anchor="seat-{s}"{locked_attr}{out_attr}{me_attr}><span class="lc-minitable-name">{name}</span><span class="lc-minitable-hp">{hp}</span></div>"#,
                    s = seat.seat,
                    name = html_escape(&seat.name.to_uppercase()),
                    hp = seat.hp,
                )
            })
        })
        .collect();

    // The pile's deck: the first deck still in the shoe, defaulting to
    // Beer — same convention as player_plaque's first_slug.
    let pile_deck = view
        .deck_counts
        .first()
        .map(|&(deck, _)| deck)
        .unwrap_or(Deck::Beer);
    let centre = format!(
        r#"<div class="lc-minitable-centre">{pile}</div>"#,
        pile = card_back(pile_deck, BackSize::Pile),
    );

    format!(
        r#"<div class="lc-minitable"><div id="lc-felt"></div><div class="lc-minitable-ring">{chips}</div>{centre}<div id="lc-flights"></div></div>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::{preview_state, Beat, LastCallState};
    use crate::lc_cards::{self, CATALOG};

    /// Flags a hex-colour shape (`#` followed by 3/4/6/8 hex digits) rather
    /// than a bare `#`, so it can run over every builder including
    /// `beat_timer` (the one builder that emits an inline `style`) without
    /// tripping on the lock tick's `&#9679;` numeric character reference —
    /// distinguished by the `&` immediately before the `#`.
    fn no_hex(s: &str) {
        let bytes = s.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            if b != b'#' {
                continue;
            }
            if i > 0 && bytes[i - 1] == b'&' {
                continue; // HTML numeric character reference, e.g. &#9679;
            }
            let hex_len = bytes[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            assert!(
                !matches!(hex_len, 3 | 4 | 6 | 8),
                "unexpected hex colour in output: {s}"
            );
        }
    }

    /// finding 8's guard is a nontrivial byte scanner; pin that it actually
    /// fires, so a future edit that breaks `matches!(hex_len, 3|4|6|8)` (or
    /// the entity-skip branch) can't make all fourteen `no_hex` call sites
    /// pass vacuously — the exact defect class finding 4 was filed for,
    /// reintroduced here if left untested.
    #[test]
    fn test_no_hex_fires_on_hex_colours_but_not_numeric_entities() {
        no_hex(r#"<span class="lc-lock-tick">&#9679;</span>"#); // must not panic
    }

    #[test]
    #[should_panic(expected = "unexpected hex colour")]
    fn test_no_hex_panics_on_six_digit_hex() {
        no_hex(r#"<div style="color:#F2EEF8">"#);
    }

    #[test]
    #[should_panic(expected = "unexpected hex colour")]
    fn test_no_hex_panics_on_three_digit_hex() {
        no_hex(r#"<div style="color:#abc">"#);
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
        // finding 1: DiscardSlot's back is deliberately deckless (a
        // destination, not a deck) — pins the Rust-side half of the fix:
        // the CSS side (assets/lastcall.css's fallback) is pinned by
        // tests/http.rs::test_lastcall_css_pins_deckless_back_and_deckstack_shadow_fixes.
        assert!(!discard.contains("lc-deck-"));

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
            lc_screen_panel(&view),
            lc_mini_table(&view, Some(0)),
        ];
        for out in &outputs {
            for banned in ["hx-post", "hx-get", "hx-swap", "onclick", "href"] {
                assert!(
                    !out.contains(banned),
                    "found forbidden `{banned}` in: {out}"
                );
            }
            // finding 8: the mechanical no-hex guard used to cover only 6 of
            // 14 builders (the ones a bare `#` check happened to work on);
            // run it over the same array this test already assembles — now
            // sixteen builders, extended by Task 3 to add lc_screen_panel
            // and lc_mini_table (each emits an inline `left/top%` style) —
            // so beat_timer's and these two's inline `style` are covered.
            no_hex(out);
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
        // finding 10: data-expandable is deliberately dropped on the
        // expanded variant — an already-expanded card has nothing left to
        // expand.
        assert!(!expanded.contains("data-expandable"));

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
        // finding 5: the ramp fallback (Beer-coloured backs) is visual only —
        // the attribute must still read the true, empty deck set.
        assert!(empty.contains(r#"data-decks="""#));
    }

    /// finding 5: a vessel-less seat (reachable in Plan A2 between joining
    /// and registering a drink) must not have its plaque and its nested hand
    /// strip disagree about the seat's deck set — both read `data-decks=""`,
    /// even though the hand strip still ramps its backs to Beer visually.
    #[test]
    fn test_vessel_less_seat_plaque_and_strip_agree_on_decks() {
        let seat = PublicSeat {
            seat: 0,
            player_id: 1,
            name: "alice".to_string(),
            hp: 15,
            status: Status::Alive,
            vessels: Vec::new(),
            hand_len: 2,
            locked: false,
            drawing: false,
            draws: 0,
        };
        let plaque = player_plaque(&seat);
        // both the plaque's own data-decks and the nested strip's must read
        // "" — two occurrences of the same empty attribute, not one.
        assert_eq!(plaque.matches(r#"data-decks="""#).count(), 2);
        let strip = hand_strip(&seat.decks(), seat.hand_len);
        assert!(strip.contains(r#"data-decks="""#));
        assert_eq!(strip.matches("lc-deck-beer").count(), 2);
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

    fn setup_row(player_id: i64, name: &str, handicap_pct: u16, decks: &[Deck]) -> SetupRow {
        SetupRow {
            player_id,
            name: name.to_string(),
            handicap_pct,
            decks: decks.to_vec(),
        }
    }

    #[test]
    fn test_lc_hand_pane_satisfies_the_contract() {
        let hand = lc_cards::deck_cards(Deck::Beer);
        let rows = [setup_row(1, "alice", 100, &[Deck::Beer])];
        let html = lc_hand_pane("", "QK4M", 1, &hand, &rows, 7);
        assert!(html.contains(r#"id="lc-hand""#));
        assert!(html.contains(r#"data-seq="7""#));
        assert!(html.contains(&format!(r#"data-count="{}""#, hand.len())));
        assert!(html.contains(r#"data-flight-anchor="hand""#));
    }

    #[test]
    fn test_lc_hand_pane_posts_to_prefixed_urls() {
        let hand = lc_cards::deck_cards(Deck::Beer);
        let rows = [
            setup_row(1, "alice", 100, &[Deck::Beer]),
            setup_row(2, "bob", 150, &[Deck::Wine]),
        ];
        let html = lc_hand_pane("/drinks", "QK4M", 1, &hand, &rows, 1);
        assert!(html.contains(r#"action="/drinks/room/QK4M/lastcall/vessel""#));
        assert!(html.contains(r#"action="/drinks/room/QK4M/lastcall/handicap""#));
        assert_eq!(
            html.matches(r#"<input type="hidden" name="target" value=""#)
                .count(),
            rows.len()
        );
    }

    /// The regression this guards is gating the control on ownership, which
    /// spec §2 explicitly rejects: any room member may set any player's
    /// handicap, not just their own.
    #[test]
    fn test_lc_hand_pane_handicap_rows_are_not_self_gated() {
        let hand: Vec<Card> = Vec::new();
        let rows = [
            setup_row(1, "alice", 100, &[Deck::Beer]),
            setup_row(2, "bob", 100, &[Deck::Wine]),
            setup_row(3, "cara", 100, &[Deck::Soft]),
        ];
        let html = lc_hand_pane("", "QK4M", 2, &hand, &rows, 1);
        assert_eq!(html.matches(">SET<").count(), 3);
        assert_eq!(html.matches("(you)").count(), 1);
        assert!(html.contains("bob (you)"));
    }

    #[test]
    fn test_lc_hand_pane_empty_hand() {
        let rows = [setup_row(1, "alice", 100, &[])];
        let html = lc_hand_pane("", "QK4M", 1, &[], &rows, 0);
        assert!(html.contains("lc-empty"));
        assert!(!html.contains("lc-cardface"));
        assert!(html.contains(r#"data-count="0""#));
    }

    // -------------------------------------------------------------------
    // Task 3 — the two table assemblers.
    // -------------------------------------------------------------------

    /// `n` seats, `player_id` 1..=n, everyone alive at 15 HP, no locks, no
    /// draws — the fixture Task 3's tests are written against.
    fn ring_fixture(n: usize) -> PublicView {
        let seats = (0..n)
            .map(|seat| PublicSeat {
                seat,
                player_id: seat as i64 + 1,
                name: format!("player{}", seat + 1),
                hp: 15,
                status: Status::Alive,
                vessels: Vec::new(),
                hand_len: 0,
                locked: false,
                drawing: false,
                draws: 0,
            })
            .collect();
        PublicView {
            seats,
            round: 1,
            beat: Beat::Draw,
            first_seat: 0,
            deck_counts: Deck::ALL.iter().map(|&d| (d, 0)).collect(),
            discard_count: 0,
            revealed: Vec::new(),
            seq: 0,
        }
    }

    #[test]
    fn test_screen_panel_places_every_seat_once() {
        for n in 2..=crate::last_call::MAX_SEATS {
            let html = lc_screen_panel(&ring_fixture(n));
            assert_eq!(html.matches("class=\"lc-seat\"").count(), n);
            for seat in 0..n {
                assert!(html.contains(&format!(r#"data-flight-anchor="seat-{seat}""#)));
            }
        }
    }

    #[test]
    fn test_screen_panel_uses_absolute_seat_order() {
        // A spectator has no seat, so the big screen never rotates: seat 0 is
        // at the bottom of the ring for everyone watching.
        let html = lc_screen_panel(&ring_fixture(5));
        let bottom = crate::lc_layout::seat_positions(5)[0];
        assert!(html.contains(&format!(
            r#"style="left:{}%;top:{}%" data-seat="0""#,
            bottom.0, bottom.1
        )));
    }

    #[test]
    fn test_screen_panel_flights_are_a_sibling_of_the_grid_not_the_stage() {
        // Deviation from the brief's Step 1.2, pinned: the flight layer is
        // a sibling of .lc-screen-grid, never inside .lc-stage — the stage
        // is overflow:hidden and only the grid's middle column, so a
        // deck-to-seat flight (deck anchors live in .lc-rail-right) would
        // be clipped and mis-anchored if nested there. Its containing
        // block is body.lc-screen (Task 2's CSS says so in as many words).
        // See the task report.
        let html = lc_screen_panel(&ring_fixture(4));
        let flights = html.find(r#"id="lc-flights""#).unwrap();
        assert!(flights > html.find("lc-rail-right").unwrap());
        assert!(flights > html.find("class=\"lc-stage\"").unwrap());
    }

    #[test]
    fn test_mini_table_puts_the_viewer_at_the_bottom() {
        // D.2: "the local player is always nearest the viewer". This is the
        // property that makes the table per-viewer data and therefore a fetch
        // rather than a broadcast.
        let view = ring_fixture(5);
        let bottom = crate::lc_layout::seat_positions(5)[0];
        for me in 0..5 {
            let html = lc_mini_table(&view, Some(me));
            assert!(
                html.contains(&format!(
                    r#"style="left:{}%;top:{}%" data-seat="{me}""#,
                    bottom.0, bottom.1
                )),
                "seat {me} should hold the bottom slot in its own view"
            );
        }
    }

    #[test]
    fn test_mini_table_for_a_spectator_is_unrotated() {
        // Deviation from the brief, pinned: the brief's literal
        // `assert_eq!(lc_mini_table(&view, None), lc_mini_table(&view, Some(0)))`
        // cannot hold — Some(0) legitimately marks seat 0 `data-me` (test
        // below requires exactly that rule) while a spectator (None) marks
        // nobody, so the two strings differ by that one attribute. The
        // property actually worth pinning is geometry: a spectator's ring
        // is identical to seat 0's own view once `data-me` is discounted,
        // and a spectator never carries `data-me` at all.
        let view = ring_fixture(4);
        let spectator = lc_mini_table(&view, None);
        assert_eq!(
            spectator,
            lc_mini_table(&view, Some(0)).replace(" data-me", "")
        );
        assert!(!spectator.contains("data-me"));
    }

    #[test]
    fn test_only_the_viewer_is_marked_me() {
        let html = lc_mini_table(&ring_fixture(6), Some(2));
        assert_eq!(html.matches("data-me").count(), 1);
        assert!(html.contains(r#"data-seat="2""#));
    }

    #[test]
    fn test_no_duplicate_anchors_or_ids_on_either_surface() {
        // lcAnchor returns the FIRST match. Plan A-vis's preview page has
        // duplicate anchors by design (a gallery shows one component in many
        // states); a real table must not inherit that — a flight would land on
        // the wrong seat and nobody would see why.
        for html in [
            lc_screen_panel(&ring_fixture(7)),
            lc_mini_table(&ring_fixture(7), Some(3)),
        ] {
            for needle in ["id=\"lc-felt\"", "id=\"lc-flights\""] {
                assert_eq!(html.matches(needle).count(), 1, "duplicate {needle}");
            }
            let mut anchors: Vec<&str> = html
                .match_indices("data-flight-anchor=\"")
                .map(|(i, m)| {
                    let rest = &html[i + m.len()..];
                    &rest[..rest.find('"').unwrap()]
                })
                .collect();
            let before = anchors.len();
            anchors.sort_unstable();
            anchors.dedup();
            assert_eq!(anchors.len(), before, "duplicate flight anchor");
        }
    }

    #[test]
    fn test_the_big_screen_is_a_display_never_an_input() {
        // F.2: no hover states, no focus rings, no controls — every affordance
        // belongs to a phone.
        let html = lc_screen_panel(&ring_fixture(6));
        for banned in [
            "hx-post", "hx-get", "hx-swap", "onclick", "href", "<button", "<form", "<input",
        ] {
            assert!(!html.contains(banned), "found `{banned}` on the big screen");
        }
    }

    #[test]
    fn test_the_felt_centre_holds_no_plays() {
        // Spec 3.4.1 binds slice 3, not this one: nothing may enter `plays`
        // before it is revealable, and this plan renders no plays at all.
        // If this test starts failing, someone has begun slice 3 inside Plan B.
        let mut view = ring_fixture(4);
        view.revealed.clear();
        assert!(!lc_screen_panel(&view).contains("lc-cardface"));
    }
}
