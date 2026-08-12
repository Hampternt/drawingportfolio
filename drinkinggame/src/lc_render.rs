//! Last Call fragments as formatted strings, matching `render.rs`. Public
//! builders take `&PublicView`/`&PublicSeat` — never `&LastCallState` — so an
//! unrevealed card cannot reach a broadcast fragment by construction (spec
//! §3.4). Every root and attribute here is the §7.8 contract; changing one is
//! a breaking change for Plan A2 and Plan B.

use crate::last_call::{
    pull_cost, Beat, Card, Deck, LcOutcome, Play, PublicSeat, PublicView, Status,
    DECK_LOW_THRESHOLD,
};
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
// Plan C additions — Task 1. Private-side builders for the phone's HAND
// tab: the armed-cards column and the drink-cost rail. Both take
// `&[Card]`/scalars, never `&LastCallState`, and are never called from
// anything broadcast.
// ---------------------------------------------------------------------

/// §7.8 `ArmedColumn` — private, the viewer's own armed cards plus one
/// affordance slot. Rendered even when `armed` is empty (`ARMED 0` plus
/// the slot, not an empty state). `locked` swaps the header to
/// `LOCKED {n}` and drops the slot. `data-locked` is a bare presence
/// attribute, never `data-locked="false"` — same rule and reason as
/// `deck_stack`'s `data-low` (lines 314-318): the CSS selects on
/// `[data-locked]`, so a `"false"` value would style every column.
/// Exactly one slot regardless of `n` — it is an affordance, not a
/// capacity meter.
pub fn armed_column(armed: &[Card], locked: bool) -> String {
    let n = armed.len();
    let locked_attr = if locked { " data-locked" } else { "" };
    let head = if locked {
        format!("LOCKED {n}")
    } else {
        format!("ARMED {n}")
    };
    let cards: String = armed.iter().map(card_mini).collect();
    let slot = if locked {
        String::new()
    } else {
        r#"<span class="lc-armed-slot">slot</span>"#.to_string()
    };
    format!(
        r#"<div class="lc-armed" data-count="{n}"{locked_attr} data-flight-anchor="armed"><span class="lc-armed-head">{head}</span>{cards}{slot}</div>"#
    )
}

/// §7.8 `HandWheel` — private, wraps each `card_face` (unchanged) in a
/// positioned `.lc-wheel-card` so the JS can drag/spin the wrapper without
/// reaching into `CardFace` internals (spec §2: "slice 2 replaces the
/// container, not the card"). `data-idx` is the DOM-position index
/// (decision 10); `data-card-id` repeats the face's id at wrapper level.
/// Never called with an empty `hand` — `hand_group` branches to the
/// decision-5 empty copy before reaching here, because the empty-state
/// message depends on `armed` too, which this single-argument builder
/// doesn't see.
pub fn hand_wheel(hand: &[Card]) -> String {
    let n = hand.len();
    let cards: String = hand
        .iter()
        .enumerate()
        .map(|(i, card)| {
            format!(
                r#"<div class="lc-wheel-card" data-idx="{i}" data-card-id="{id}">{face}</div>"#,
                id = html_escape(&card.id),
                face = card_face(card),
            )
        })
        .collect();
    format!(
        r#"<div class="lc-wheel" data-count="{n}"><div class="lc-wheel-stage" data-lc-wheel><div class="lc-wheel-track">{cards}</div><span class="lc-wheel-hint">DRAG TO SPIN</span></div></div>"#
    )
}

/// The viewer's own private hand-group data. All refs — Copy on purpose.
#[derive(Clone, Copy, Debug)]
pub struct HandGroupView<'a> {
    pub hand: &'a [Card],
    pub armed: &'a [Card],
    pub locked: bool,
    pub handicap_pct: u16,
}

/// §7.8 `Hand group` — private, `.lc-handgroup` with children `.lc-armed`,
/// `.lc-wheel` (or the decision-5 empty message), `.lc-costrail` in that
/// order. The empty-hand copy depends on whether `armed` is also empty — the
/// choice lives here rather than in `hand_wheel` because this is the one
/// builder that sees both slices. `cost_rail` always renders, even in both
/// empty cases (its own `n == 0` state, Task 1).
pub fn hand_group(hg: &HandGroupView) -> String {
    let armed = armed_column(hg.armed, hg.locked);
    let wheel = if hg.hand.is_empty() {
        let msg = if hg.armed.is_empty() {
            "Register your drink to be dealt a hand."
        } else {
            "Every card you hold is armed."
        };
        format!(r#"<p class="lc-empty">{msg}</p>"#)
    } else {
        hand_wheel(hg.hand)
    };
    let rail = cost_rail(hg.hand, hg.handicap_pct);
    format!(r#"<div class="lc-handgroup">{armed}{wheel}{rail}</div>"#)
}

/// §7.8 `CostRail` — private, the viewer's hand priced through their own
/// handicap. `pc = pull_cost(card.cost, handicap_pct)` — decision 1: the
/// rail shows the true pull price, rounded up — and each group renders
/// `pc` bars. `is-active` is emitted server-side on `data-idx="0"` only
/// (the initial focus); the JS moves it thereafter. `lc-deck-{slug}` on
/// the group supplies `--lc-ink` to its bars, same convention as every
/// other `lc-deck-*` root in this file. The above-label is the *focused
/// card's ordinal*, not the hand size: `01` whenever `n > 0` (matching the
/// server-emitted `is-active` on `data-idx="0"`), `00` only when `n == 0`.
/// `lc_wheel.js`'s `syncRail` owns updating it live thereafter — this
/// builder only ever paints the initial focus. `n == 0` renders the root
/// with `data-count="0"`, label `00` above, `0` below and an empty
/// `.lc-costrail-bars` — stable layout, no special casing downstream.
///
/// Named `.lc-costrail*`, not `.lc-rail*`: `.lc-rail` already names the
/// big screen's side-rail class (lines 511, 537; `lastcall.css:700`) and
/// `--lc-rail` is a colour token, so this unrelated component gets its
/// own prefix instead of colliding with that surface.
pub fn cost_rail(hand: &[Card], handicap_pct: u16) -> String {
    let n = hand.len();
    let above = if n == 0 {
        "00".to_string()
    } else {
        "01".to_string()
    };
    let groups: String = hand
        .iter()
        .enumerate()
        .map(|(i, card)| {
            let slug = card.deck.slug();
            let pc = pull_cost(card.cost, handicap_pct);
            let active = if i == 0 { " is-active" } else { "" };
            let bars: String = (0..pc)
                .map(|_| r#"<i class="lc-costrail-bar"></i>"#)
                .collect();
            format!(
                r#"<div class="lc-costrail-group lc-deck-{slug}{active}" data-idx="{i}" data-card-id="{id}" data-cost="{cost}" data-pull-cost="{pc}">{bars}</div>"#,
                id = html_escape(&card.id),
                cost = card.cost,
            )
        })
        .collect();
    format!(
        r#"<div class="lc-costrail" data-count="{n}"><span class="lc-costrail-above">{above}</span><div class="lc-costrail-bars">{groups}</div><span class="lc-costrail-below">{n}</span></div>"#
    )
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
        r#"<div class="lc-plaque lc-deck-{first_slug}{state_classes}" data-seat="{seat_n}" data-decks="{decks_attr}" data-hp="{hp}" data-draws="{draws}" data-status="{status}" data-hand-size="{hand_len}" data-flight-anchor="plaque-seat-{seat_n}">{rule}<div class="lc-identity"><span class="lc-name">{name}{lock_tick}</span><span class="lc-hp">{hp_display}</span></div><div class="lc-drinks">{dots}<span class="lc-decknames">{deck_names}</span>{draws_badge}</div>{hand_strip}</div>"#,
        seat_n = seat.seat,
        hp = seat.hp,
        draws = seat.draws,
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
    // Plan E, decision E10: the live timer rides inside #lc-banner as its
    // last child so the lcpublic banner swap (outerHTML on #lc-banner)
    // replaces banner and timer atomically — no orphaned timer element, no
    // inline-script bookkeeping to keep them in sync. Only rendered when the
    // beat both has a deadline (untimed beats — round 1's Draw lobby, the
    // auto beats, the frozen game-over tableau — carry `None`) and a
    // duration (defensive: the two should never disagree, but the timer has
    // nothing to size itself against without a duration either way).
    let timer = match (view.beat_deadline_ms, beat.duration_secs()) {
        (Some(deadline), Some(secs)) => beat_timer_live(u32::from(secs) * 1000, deadline),
        _ => String::new(),
    };
    // Plan E, decision E13: game over is the frozen Resolve tableau (D16),
    // but the banner switches to a dedicated GAME OVER state rather than
    // going on saying RESOLVE — the beat name stops meaning anything once
    // the round stopped mid-cycle. `view.outcome` alone gates this, not
    // `beat == Resolve` (which the freeze always is anyway, but that is not
    // the reason): no timer is emitted either way, since `beat_deadline_ms`
    // is `None` at the freeze regardless of which branch runs here.
    if view.outcome.is_some() {
        return format!(
            r#"<div class="lc-banner lc-beat-rose" id="lc-banner" data-beat="{slug}" data-round="{round}"><span class="lc-banner-beat">GAME OVER</span><span class="lc-banner-meta">ROUND {round} &middot; LAST CALL</span></div>"#,
            slug = beat.slug(),
            round = view.round,
        );
    }
    format!(
        r#"<div class="lc-banner lc-beat-{hue}" id="lc-banner" data-beat="{slug}" data-round="{round}"><span class="lc-banner-beat">{label}</span><span class="lc-banner-meta">ROUND {round} &middot; BEAT {index} OF 6</span>{timer}</div>"#,
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

/// Live twin of `beat_timer` (which the preview keeps): same root id/class,
/// but deadline-driven — lc_loop.js (Task 4) computes remaining client-side
/// from `data-deadline-ms` and sets `--lc-beat-ms`, rather than this being
/// computed server-side once at render time. No inline style, so the
/// no-hex/no-style sweeps still hold.
pub fn beat_timer_live(duration_ms: u32, deadline_ms: i64) -> String {
    format!(
        r#"<div id="lc-beat-timer" class="lc-timer" data-duration-ms="{duration_ms}" data-deadline-ms="{deadline_ms}"></div>"#
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
///
/// The trailing card list (Plan A2) is now `hand_group(hg)` — the armed
/// column, HandWheel (or its empty message) and cost rail (Plan C Task 2).
pub fn lc_hand_pane(
    base_path: &str,
    code: &str,
    me: i64,
    hg: &HandGroupView,
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

    format!(
        r#"<div id="lc-hand" data-seq="{seq}" data-count="{count}" data-flight-anchor="hand"><section class="lc-setup"><h2>Your drink</h2><form method="post" action="{base_path}/room/{code}/lastcall/vessel"><select name="deck">{deck_options}</select><input name="container" maxlength="24" placeholder="50cl can"><button type="submit">REGISTER</button></form><h2>Handicaps</h2>{handicap_rows}</section>{group}</div>"#,
        count = hg.hand.len(),
        group = hand_group(hg),
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
/// `.get()`, never `[]`: an oversized state can still hand this more seats
/// than `seat_positions` has rows for at that count. Both *live* paths into
/// `players` now cap at `MAX_SEATS` (`LastCallState::new`'s `.take`, and
/// `add_player`'s guard), but `from_json` does not — a state blob persisted
/// before that ceiling existed deserializes with every seat it had. Render
/// short rather than panic; this is the one formula both `lc_screen_panel`
/// and `lc_mini_table` share, one argument different.
fn seat_pos(n: usize, seat: usize, me: Option<usize>) -> Option<SeatPos> {
    seat_positions(n).get(view_index(seat, me, n)).copied()
}

/// A seat name, uppercased and escaped, or `""` if `view.seats` no longer
/// carries that seat (defensive: `Play::source_seat`/`target` are validated
/// at lock time, but nothing re-validates them against a `view` built from a
/// different, later state).
fn seat_name_upper(view: &PublicView, seat: usize) -> String {
    view.seats
        .iter()
        .find(|s| s.seat == seat)
        .map(|s| html_escape(&s.name.to_uppercase()))
        .unwrap_or_default()
}

/// Plan E, decision E15: one revealed play's caption plus its `card_mini`,
/// the felt centre's one row. `SRC` is always a seat name; `TGT` is the
/// target seat's name when the play has one (`self` plays name the caster
/// twice, which is correct — spec D2), or the card's own `targets` value
/// uppercased for a `None` target ("all"/"table" cards, whatever the
/// catalog calls them).
fn centre_play(view: &PublicView, play: &Play) -> String {
    let src = seat_name_upper(view, play.source_seat);
    let tgt = match play.target {
        Some(t) => seat_name_upper(view, t),
        None => html_escape(&play.card.targets.to_uppercase()),
    };
    format!(
        r#"<div class="lc-centre-play" data-seat="{seat}"><span class="lc-centre-cap">{src} &rarr; {tgt}</span>{mini}</div>"#,
        seat = play.source_seat,
        mini = card_mini(&play.card),
    )
}

/// Plan E, decision E13: the frozen Resolve tableau's one line, in place of
/// the felt-centre plays once the game has an `outcome`.
fn victory_line(view: &PublicView, outcome: LcOutcome) -> String {
    let line = match outcome {
        LcOutcome::Winner(seat) => {
            format!("{} OUTLASTS THE TABLE", seat_name_upper(view, seat))
        }
        LcOutcome::Draw => "EVERYBODY'S OUT".to_string(),
        // G2/Task 2: the pact win. Names come from `view.seats`, already
        // uppercased and html_escape'd by `seat_name_upper`.
        LcOutcome::Pact(a, b) => format!(
            "{} & {} — THE PACT HOLDS",
            seat_name_upper(view, a),
            seat_name_upper(view, b)
        ),
    };
    format!(r#"<div class="lc-centre-victory">{line}</div>"#)
}

/// F.2 big-screen body — the three-column grid (seat-order rail, felt
/// ring, deck rail). The flight layer is no longer built here: Plan E
/// Task 3 moved `#lc-flights` out to the static shell (`lc_screen.html`),
/// a sibling of this panel rather than something this repaint can destroy
/// — see that template. Absolute seat order throughout — a spectator has
/// no seat, so nothing here rotates.
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
    // Plan E, decision E15: the felt centre. Priority — a decided game shows
    // the frozen tableau's victory line; otherwise `revealed` (populated
    // only at Reveal/Resolve, per `public_view`'s own gate — Plan D's
    // projection tests are the authority on secrecy, not this renderer)
    // shows the ordered plays; empty either way renders nothing. The two
    // are mutually exclusive in practice (an outcome only turns `Some`
    // inside `resolve()`, which drains `plays` before returning), but
    // `outcome` is checked first regardless — the frozen tableau is never
    // allowed to show stale plays.
    let centre = if let Some(outcome) = view.outcome {
        victory_line(view, outcome)
    } else if !view.revealed.is_empty() {
        let plays: String = view.revealed.iter().map(|p| centre_play(view, p)).collect();
        format!(r#"<div class="lc-centre-plays">{plays}</div>"#)
    } else {
        String::new()
    };
    // Plan G, Task 3 (G5): the break strip — the one public trace a pact
    // leaves before the shared win, and only for the round the betrayal
    // happened in. `view.pact_breaks` outlives its round (Plan J's recap
    // reads the full history), so this filters to `round == view.round`
    // rather than rendering every record ever accumulated; the betrayed
    // player additionally gets the private, round-scoped Step-1 notice in
    // `lc_routes::pacts_section_html`, and the mini table stays untouched.
    let pact_breaks: String = view
        .pact_breaks
        .iter()
        .filter(|b| b.round == view.round)
        .map(|b| {
            format!(
                r#"<div class="lc-pact-break">{betrayer} BROKE THEIR PACT WITH {betrayed}</div>"#,
                betrayer = seat_name_upper(view, b.betrayer),
                betrayed = seat_name_upper(view, b.betrayed),
            )
        })
        .collect();
    let pact_breaks_html = if pact_breaks.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="lc-pact-strip">{}</div>"#, pact_breaks)
    };
    let stage = format!(
        r#"<div class="lc-stage"><div id="lc-felt" data-flight-anchor="felt"></div>{centre}{pact_breaks_html}<div class="lc-ring">{seats_html}</div></div>"#,
        centre = centre,
        pact_breaks_html = pact_breaks_html,
        seats_html = seats_html,
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

    // No `#lc-flights` here (Plan E Task 3): the layer is now a static
    // sibling of this grid in `lc_screen.html`, outside every repainted
    // pane, so an `lcpublic` repaint can no longer destroy a mid-flight
    // node and drop its `onArrive`. See that template for the containing
    // block (`body.lc-screen { position: relative }`, `lastcall.css:632`).
    format!(r#"<div class="lc-screen-grid">{left_rail}{stage}{right_rail}</div>"#)
}

/// F.3 phone mini table, rotated so `me` sits at bottom-centre — the whole
/// point of the route in Task 5. `me` is the viewer's own seat, or `None`
/// for a member who is not seated.
///
/// The centre column's event/quest/discard rows (Module Spec F.3) are
/// deliberately not filled here: no Plan A builder renders a
/// dot+label+count row, so doing so would be authoring a new component,
/// which this plan does not do. See the task report for the full
/// adjudication. The `.lc-minitable-row*` CSS that anticipated them has
/// since been deleted as dead — style them next to whatever renders them.
///
/// No flight layer is built here (Plan E Task 3): `#lc-flights` is now a
/// static sibling of `.lc-view` in `lc_room.html`, outside every
/// `[data-lc-pane]` this table (or the hand pane) repaints, so a table
/// (or `lcpublic`) repaint can no longer destroy a mid-flight node and
/// drop its `onArrive`. `ensureLayer`'s existing-layer guard finds that
/// static node regardless of which pane is active, so a flight now
/// survives a tab switch as well as a repaint.
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

    // The pile's deck: simply the first entry in `deck_counts`, defaulting
    // to Beer — same convention as player_plaque's first_slug. Note it does
    // NOT skip exhausted decks: `deck_counts` is ordered, not filtered, so
    // this is "the first deck in the list", not "the first deck still in the
    // shoe". Fine while the pile is decorative; revisit when the draw pile
    // has to show what would actually be drawn.
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
        r#"<div class="lc-minitable"><div id="lc-felt" data-flight-anchor="felt"></div><div class="lc-minitable-ring">{chips}</div>{centre}</div>"#
    )
}

// ---------------------------------------------------------------------
// Plan E additions — Task 4. The F.1 action bar: private, per-viewer, never
// broadcast (the private-side twin of `PublicView`) — rendered only into the
// shell page (`lc_page`) and the private hand fetch (`hand_pane_html`).
// ---------------------------------------------------------------------

/// The viewer's own action-bar data, assembled by `lc_routes::action_bar_view`
/// from `&LastCallState` (never passed here directly — same secrecy
/// discipline `HandGroupView` follows).
#[derive(Clone, Debug)]
pub struct ActionBarView {
    pub beat: Beat,
    pub round: u32,
    pub seated: bool,
    pub alive: bool,
    pub locked: bool,
    pub drawing: bool,
    pub vessels: Vec<(usize, Deck)>, // (vessel index, deck)
    pub charged: u8,                 // viewer's pulls at the reveal (E9)
    pub vessels_registered: usize,   // players with >= 1 vessel (E1 gate)
    pub outcome: Option<LcOutcome>,
}

/// F.1 — the thumb zone's one decision. Precedence: `outcome` (any player,
/// from any beat, can end a decided game) -> `!seated` (a spectator has
/// nothing to decide) -> `!alive` (a ghost has nothing to decide either,
/// even mid-Lock) -> the beat itself. F.1 holds by construction throughout:
/// the drinking-adjacent primary is always `lc-btn-drink` (amber,
/// `lastcall.css:388`), and the beat's one decision is the only thing ever
/// in the thumb zone. `data-lc-post` is a data-contract attribute (the same
/// argument as Plan C's `CustomEvent`s) — `test_no_builder_emits_behaviour`
/// does not forbid it, only `hx-*`/`onclick`/`href`/`action="`.
pub fn lc_action_bar(ab: &ActionBarView) -> String {
    if ab.outcome.is_some() {
        return r#"<button class="lc-btn lc-btn-drink" data-lc-post="end">END GAME</button>"#
            .to_string();
    }
    if !ab.seated {
        return r#"<p class="lc-actions-hint">SPECTATING</p>"#.to_string();
    }
    if !ab.alive {
        return r#"<p class="lc-actions-hint">YOU'RE OUT — HAUNT THE TABLE</p>"#.to_string();
    }
    match ab.beat {
        Beat::Draw => {
            if ab.round == 1 {
                if ab.vessels_registered >= 2 {
                    r#"<button class="lc-btn lc-btn-drink" data-lc-post="begin">START ROUND 1</button>"#.to_string()
                } else {
                    r#"<button class="lc-btn lc-btn-drink" data-lc-post="begin" disabled>START ROUND 1</button><p class="lc-actions-hint">NEEDS 2 DRINKS REGISTERED</p>"#.to_string()
                }
            } else if ab.drawing {
                r#"<p class="lc-actions-hint">FRESH VESSEL — DEALT</p>"#.to_string()
            } else {
                let buttons: String = ab
                    .vessels
                    .iter()
                    .map(|(idx, deck)| {
                        format!(
                            r#"<button class="lc-btn lc-btn-drink" data-lc-post="draw" data-vessel="{idx}">FINISH {label} · DRAW</button>"#,
                            label = deck.label(),
                        )
                    })
                    .collect();
                format!(r#"{buttons}<p class="lc-actions-hint">OR SIT TIGHT</p>"#)
            }
        }
        Beat::Deal => r#"<p class="lc-actions-hint">DEALING…</p>"#.to_string(),
        Beat::Diplomacy => {
            r#"<p class="lc-actions-hint">TALK IT OUT — DEALS AREN'T BINDING</p>"#.to_string()
        }
        Beat::Lock => {
            if ab.locked {
                r#"<p class="lc-actions-hint">LOCKED — WAITING FOR THE TABLE</p>"#.to_string()
            } else {
                r#"<button class="lc-btn lc-btn-drink" data-lc-post="lock">LOCK IN</button>"#
                    .to_string()
            }
        }
        Beat::Reveal | Beat::Resolve => {
            if ab.charged > 0 {
                format!(
                    r#"<div class="lc-btn lc-btn-drink lc-drink-now">DRINK {}</div>"#,
                    ab.charged
                )
            } else {
                r#"<p class="lc-actions-hint">NOTHING TO PAY</p>"#.to_string()
            }
        }
    }
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
        assert!(plaque.contains("data-draws"));
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
        let card = &lc_cards::deck_cards(Deck::Cider)[3]; // cider-04, 5 keywords
        let cards = lc_cards::deck_cards(Deck::Cider);

        // Plan E Task 5: `preview_state()` sits at Beat::Lock with no
        // outcome, so it never exercises the felt centre's revealed-plays
        // or game-over branches — cover those here too, or the loop below
        // would only ever scan `lc_screen_panel`/`lc_banner` output that
        // never took either new path.
        let mut revealed_view = ring_fixture(4);
        revealed_view.revealed = vec![Play {
            card: card.clone(),
            source_seat: 0,
            target: Some(1),
            paid_from: card.deck,
            order_key: 1,
        }];
        let mut outcome_view = ring_fixture(4);
        outcome_view.beat = Beat::Resolve;
        outcome_view.beat_deadline_ms = None;
        outcome_view.outcome = Some(LcOutcome::Winner(0));

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
            lc_screen_panel(&revealed_view),
            lc_banner(&outcome_view),
            lc_screen_panel(&outcome_view),
            armed_column(&cards, false),
            cost_rail(&cards, 150),
            hand_wheel(&cards),
            hand_group(&HandGroupView {
                hand: &cards,
                armed: &cards[..1],
                locked: false,
                handicap_pct: 150,
            }),
            // Plan E Task 4: a populated `lc_action_bar` call, deliberately
            // in the per-vessel Draw state so both `data-lc-post` and
            // `data-vessel` are present in the output — this is the state
            // the loop below proves `data-lc-post` through, not just an
            // absence of the six banned strings.
            lc_action_bar(&ActionBarView {
                beat: Beat::Draw,
                round: 2,
                seated: true,
                alive: true,
                locked: false,
                drawing: false,
                vessels: vec![(0, Deck::Beer), (1, Deck::Soft)],
                charged: 0,
                vessels_registered: 2,
                outcome: None,
            }),
        ];
        for out in &outputs {
            // `action="` (not bare `action`) — a placeholder card body
            // contains the prose "A reaction, once reactions exist." and a
            // bare substring match panics on that, not on markup.
            //
            // `data-lc-post` is NOT in this list — Plan E decision: it is a
            // data-contract attribute for `lc_loop.js`'s one delegated click
            // listener, the same status Plan C's `lc:arm`/`lc:disarm`
            // CustomEvents have. It names WHAT to do, not HOW (no inline
            // handler, no hx-* wiring baked into the string), so it does not
            // trip this test.
            for banned in [
                "hx-post",
                "hx-get",
                "hx-swap",
                "onclick",
                "href",
                "action=\"",
            ] {
                assert!(
                    !out.contains(banned),
                    "found forbidden `{banned}` in: {out}"
                );
            }
            // finding 8: the mechanical no-hex guard used to cover only 6 of
            // 14 builders (the ones a bare `#` check happened to work on);
            // run it over the same array this test already assembles — now
            // eighteen builders, extended by Task 3 to add lc_screen_panel
            // and lc_mini_table (each emits an inline `left/top%` style),
            // by Plan C Task 1 to add armed_column and cost_rail, and by
            // Plan C Task 2 to add hand_wheel and hand_group — so
            // beat_timer's and these builders' inline styles/attrs are
            // covered.
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
        assert_eq!(cider04.keywords.len(), 5);
        let html = card_face(&cider04);
        assert_eq!(html.matches(r#"class="lc-kw""#).count(), 3);
        assert!(html.contains(r#"class="lc-kw lc-kw-more">+2<"#));
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
        // Plan E Task 5: `data-draws` is the motion pass's own read of
        // `PublicSeat::draws` — the badge span is decorative and Task 5's
        // JS reads the attribute, not the span's text.
        assert!(html.contains(r#"data-draws="3""#));

        let seat0 = PublicSeat { draws: 0, ..seat };
        let html0 = player_plaque(&seat0);
        assert!(!html0.contains("lc-draws"));
        assert!(html0.contains(r#"data-draws="0""#)); // the attribute stays even with no badge
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

    /// Fills in the fields `HandGroupView` needs beyond `hand`, so each call
    /// site only spells out what it's varying.
    fn hg<'a>(hand: &'a [Card], armed: &'a [Card]) -> HandGroupView<'a> {
        HandGroupView {
            hand,
            armed,
            locked: false,
            handicap_pct: 100,
        }
    }

    #[test]
    fn test_lc_hand_pane_satisfies_the_contract() {
        let hand = lc_cards::deck_cards(Deck::Beer);
        let armed = [hand[0].clone()];
        let rows = [setup_row(1, "alice", 100, &[Deck::Beer])];
        let view = hg(&hand, &armed);
        let html = lc_hand_pane("", "QK4M", 1, &view, &rows, 7);
        assert!(html.contains(r#"id="lc-hand""#));
        assert!(html.contains(r#"data-seq="7""#));
        assert!(html.contains(&format!(r#"data-count="{}""#, hand.len())));
        assert!(html.contains(r#"data-flight-anchor="hand""#));
        // The phone surface carries exactly one `.lc-handgroup`, one
        // `.lc-armed`, and one `data-flight-anchor="armed"` — one match is
        // what `lcAnchor` returns, so the pane must not inherit the
        // preview's duplicate-anchor pattern.
        assert_eq!(html.matches("lc-handgroup").count(), 1);
        assert_eq!(html.matches(r#"class="lc-armed""#).count(), 1);
        assert_eq!(html.matches(r#"data-flight-anchor="armed""#).count(), 1);
    }

    #[test]
    fn test_lc_hand_pane_posts_to_prefixed_urls() {
        let hand = lc_cards::deck_cards(Deck::Beer);
        let rows = [
            setup_row(1, "alice", 100, &[Deck::Beer]),
            setup_row(2, "bob", 150, &[Deck::Wine]),
        ];
        let view = hg(&hand, &[]);
        let html = lc_hand_pane("/drinks", "QK4M", 1, &view, &rows, 1);
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
        let view = hg(&hand, &[]);
        let html = lc_hand_pane("", "QK4M", 2, &view, &rows, 1);
        assert_eq!(html.matches(">SET<").count(), 3);
        assert_eq!(html.matches("(you)").count(), 1);
        assert!(html.contains("bob (you)"));
    }

    #[test]
    fn test_lc_hand_pane_empty_hand() {
        let rows = [setup_row(1, "alice", 100, &[])];
        let view = hg(&[], &[]);
        let html = lc_hand_pane("", "QK4M", 1, &view, &rows, 0);
        assert!(html.contains("lc-empty"));
        assert!(html.contains("Register your drink to be dealt a hand."));
        assert!(!html.contains("lc-cardface"));
        assert!(!html.contains("lc-wheel"));
        assert!(html.contains(r#"data-count="0""#));
    }

    #[test]
    fn test_hand_wheel_wraps_each_card_face_unchanged() {
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
            rail_card("liquor-fx", Deck::Liquor, 3),
        ];
        let html = hand_wheel(&hand);
        assert!(html.contains(r#"data-count="3""#));
        for (i, card) in hand.iter().enumerate() {
            assert!(html.contains(&format!(
                r#"<div class="lc-wheel-card" data-idx="{i}" data-card-id="{}">"#,
                card.id
            )));
            // CardFace rendering is not touched — the wrapper's inner HTML
            // contains the byte-exact card_face(card) string.
            assert!(html.contains(&card_face(card)));
        }
        no_hex(&html);
    }

    #[test]
    fn test_hand_group_orders_armed_wheel_rail_and_picks_the_empty_copy() {
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
        ];

        let populated = hg(&hand, &hand[..1]);
        let html = hand_group(&populated);
        let armed_at = html.find("lc-armed").unwrap();
        let wheel_at = html.find("lc-wheel").unwrap();
        let rail_at = html.find("lc-costrail").unwrap();
        assert!(armed_at < wheel_at, "armed must precede wheel");
        assert!(wheel_at < rail_at, "wheel must precede rail");
        no_hex(&html);

        let no_hand_no_armed = hg(&[], &[]);
        let html_empty = hand_group(&no_hand_no_armed);
        assert!(html_empty.contains("Register your drink to be dealt a hand."));
        assert!(!html_empty.contains("lc-wheel"));
        no_hex(&html_empty);

        let no_hand_but_armed = hg(&[], &hand[..1]);
        let html_armed = hand_group(&no_hand_but_armed);
        assert!(html_armed.contains("Every card you hold is armed."));
        assert!(!html_armed.contains("lc-wheel"));
        // The rail's own empty state, bound to its root (not the armed
        // column's `data-count="1"`, which would also satisfy a bare
        // `contains(r#"data-count="0""#)` check by coincidence).
        assert!(html_armed.contains(r#"lc-costrail" data-count="0""#));
        no_hex(&html_armed);
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
            outcome: None,
            beat_deadline_ms: None,
            pact_breaks: vec![],
            event: None,
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

    /// G5, Plan G Task 3: the big screen's betrayal line renders only for
    /// the round it happened in — history stays in `pact_breaks` for Plan
    /// J's recap, but doesn't repaint on the felt once the round moves on.
    #[test]
    fn test_the_break_strip_names_the_knife_for_one_round() {
        let mut view = ring_fixture(4);
        view.round = 2;
        view.pact_breaks = vec![crate::last_call::PactBreak {
            betrayer: 0,
            betrayed: 1,
            round: 2,
        }];

        let html = lc_screen_panel(&view);
        assert_eq!(html.matches("lc-pact-break").count(), 1);
        assert!(html.contains("PLAYER1 BROKE THEIR PACT WITH PLAYER2"));
        no_hex(&html);

        view.round = 3;
        let html = lc_screen_panel(&view);
        assert_eq!(html.matches("lc-pact-break").count(), 0);

        view.round = 2;
        view.pact_breaks = vec![];
        let html = lc_screen_panel(&view);
        assert_eq!(html.matches("lc-pact-break").count(), 0);
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
    fn test_screen_panel_no_longer_builds_the_flight_layer() {
        // Plan E Task 3: `#lc-flights` moved out of this builder's output
        // into the static shell (`lc_screen.html`), a sibling of the
        // rendered grid, specifically so an `lcpublic` repaint (which
        // replaces this panel's markup wholesale) can never destroy a
        // mid-flight node and drop its `onArrive`.
        let html = lc_screen_panel(&ring_fixture(4));
        assert!(!html.contains("lc-flights"));
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
    fn test_both_surfaces_pin_the_felt_anchor() {
        // Plan E review adjudication 2: the plan's Task 5 Consumes list named
        // the `felt` anchor as already shipped when it wasn't — without it
        // `lcAnchor("felt")` returns null and every reveal flight silently
        // no-ops. Both builders' `#lc-felt` div carries the attribute; only
        // incidental coverage existed before (the dedup loop below), so pin
        // it explicitly here to protect the contract from a silent drop.
        assert!(lc_screen_panel(&ring_fixture(4)).contains(r#"data-flight-anchor="felt""#));
        assert!(lc_mini_table(&ring_fixture(4), Some(0)).contains(r#"data-flight-anchor="felt""#));
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
            assert_eq!(
                html.matches("id=\"lc-felt\"").count(),
                1,
                "duplicate id=\"lc-felt\""
            );
            // Plan E Task 3: the flight layer is no longer built by either
            // panel — it is static in the shell templates now, so a
            // repaint of this fragment can't destroy a mid-flight node.
            assert!(!html.contains("lc-flights"));
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

    /// Bare-bones fixture `Card` for the felt-centre tests — only `id`,
    /// `deck`, `targets` and `title` vary per call site.
    fn centre_card(id: &str, deck: Deck, targets: &str, title: &str) -> Card {
        Card {
            id: id.to_string(),
            deck,
            kind: crate::last_call::CardKind::Atk,
            cost: 2,
            targets: targets.to_string(),
            title: title.to_string(),
            text: "text".to_string(),
            keywords: Vec::new(),
            duration: None,
        }
    }

    #[test]
    fn test_the_felt_centre_shows_revealed_plays_in_order() {
        // REPLACES test_the_felt_centre_holds_no_plays — that test pinned the
        // slice-1 boundary ("someone has begun slice 3 inside Plan B"); slice
        // 3 is now deliberately here. Secrecy no longer rests on the
        // renderer: `public_view()` only populates `revealed` at
        // Reveal/Resolve, pinned by `last_call.rs`'s own projection tests
        // (Plan D's mandatory §3.4.1 test) — this test only pins what the
        // renderer does with a `revealed` vec once handed one.
        let mut view = ring_fixture(4);
        let targeted = centre_card("cider-target", Deck::Cider, "one", "Targeted Play");
        let untargeted = centre_card("wine-all", Deck::Wine, "all", "Table Play");
        view.revealed = vec![
            Play {
                card: targeted.clone(),
                source_seat: 1,
                target: Some(2),
                paid_from: Deck::Cider,
                order_key: 1,
            },
            Play {
                card: untargeted.clone(),
                source_seat: 0,
                target: None,
                paid_from: Deck::Wine,
                order_key: 2,
            },
        ];
        let html = lc_screen_panel(&view);
        assert!(html.contains("lc-centre-plays"));
        assert!(html.contains(&targeted.title));
        assert!(html.contains(&untargeted.title));
        // vec order (order_key 1, 2) is render order.
        assert!(html.find(&targeted.title).unwrap() < html.find(&untargeted.title).unwrap());
        // ring_fixture names seats "player{n+1}" — seat 1 -> PLAYER2, seat 2
        // -> PLAYER3, seat 0 -> PLAYER1.
        assert!(html.contains("PLAYER2 &rarr; PLAYER3"));
        // The None-target play's caption ends "&rarr; ALL" — the card's own
        // `targets`, uppercased, standing in for a seat name.
        assert!(html.contains("PLAYER1 &rarr; ALL"));
        no_hex(&html);

        // revealed empty -> no "lc-centre-plays" at all.
        assert!(!lc_screen_panel(&ring_fixture(4)).contains("lc-centre-plays"));
    }

    #[test]
    fn test_game_over_takes_over_banner_and_centre() {
        let mut view = ring_fixture(4);
        view.beat = Beat::Resolve;
        view.beat_deadline_ms = None;
        view.outcome = Some(LcOutcome::Winner(1));

        let banner = lc_banner(&view);
        assert!(banner.contains("GAME OVER"));
        assert!(banner.contains("lc-beat-rose"));
        assert!(!banner.contains("data-deadline-ms"));

        let screen = lc_screen_panel(&view);
        assert!(screen.contains("PLAYER2 OUTLASTS THE TABLE"));
        assert!(!screen.contains("lc-centre-plays"));
        no_hex(&banner);
        no_hex(&screen);

        view.outcome = Some(LcOutcome::Draw);
        let screen_draw = lc_screen_panel(&view);
        assert!(screen_draw.contains("EVERYBODY'S OUT"));
        assert!(!screen_draw.contains("lc-centre-plays"));

        // G2/Task 2: the pact win, banner unchanged, names uppercased.
        view.outcome = Some(LcOutcome::Pact(0, 2));
        let screen_pact = lc_screen_panel(&view);
        assert!(screen_pact.contains("PLAYER1 & PLAYER3 — THE PACT HOLDS"));
        assert!(!screen_pact.contains("lc-centre-plays"));
        let banner_pact = lc_banner(&view);
        assert!(banner_pact.contains("GAME OVER"));
        no_hex(&screen_pact);
        no_hex(&banner_pact);
    }

    // -------------------------------------------------------------
    // Plan C Task 1 — ArmedColumn and CostRail.
    // -------------------------------------------------------------

    /// A minimal fixture `Card` for the ArmedColumn/CostRail tests — only
    /// `id`, `deck` and `cost` vary per call site; the rest is filler.
    fn rail_card(id: &str, deck: Deck, cost: u8) -> Card {
        Card {
            id: id.to_string(),
            deck,
            kind: crate::last_call::CardKind::Atk,
            cost,
            targets: "one".to_string(),
            title: "Fixture".to_string(),
            text: "Fixture text.".to_string(),
            keywords: Vec::new(),
            duration: None,
        }
    }

    #[test]
    fn test_cost_rail_applies_handicap_and_rounds_up() {
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
            rail_card("liquor-fx", Deck::Liquor, 3),
        ];
        // (handicap_pct, expected per-card pull costs, expected bar total)
        let cases: [(u16, [u8; 3], usize); 4] = [
            (100, [1, 2, 3], 6),
            (150, [2, 3, 5], 10),
            (25, [1, 1, 1], 3),
            (300, [3, 6, 9], 18),
        ];
        for (pct, expected_costs, expected_bars) in cases {
            let html = cost_rail(&hand, pct);
            assert_eq!(
                html.matches(r#"class="lc-costrail-bar""#).count(),
                expected_bars,
                "pct={pct}"
            );
            for (i, pc) in expected_costs.iter().enumerate() {
                assert!(
                    html.contains(&format!(
                        r#"data-idx="{i}" data-card-id="{}" data-cost="{}" data-pull-cost="{pc}""#,
                        hand[i].id, hand[i].cost
                    )),
                    "pct={pct} idx={i} missing data-pull-cost={pc}: {html}"
                );
            }
            no_hex(&html);
        }
    }

    #[test]
    fn test_cost_rail_marks_first_group_active_and_survives_empty() {
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
            rail_card("liquor-fx", Deck::Liquor, 3),
        ];
        let html = cost_rail(&hand, 100);
        assert_eq!(html.matches("is-active").count(), 1);
        assert!(html.contains(r#"lc-costrail-group lc-deck-beer is-active" data-idx="0""#));
        // Negative check, not just a count: idx 1 and 2 close their class
        // attribute right after the deck slug — no trailing ` is-active`.
        assert!(html.contains(r#"lc-costrail-group lc-deck-wine" data-idx="1""#));
        assert!(html.contains(r#"lc-costrail-group lc-deck-liquor" data-idx="2""#));
        // Important 2 (fix wave): the above-label is the focused card's
        // ordinal (01), never the hand size (3) — `syncRail` owns updating
        // it thereafter, this builder only ever paints the initial focus.
        assert!(html.contains(r#"<span class="lc-costrail-above">01</span>"#));
        no_hex(&html);

        let empty: [Card; 0] = [];
        let html0 = cost_rail(&empty, 100);
        assert!(html0.contains(r#"data-count="0""#));
        assert!(html0.contains(">00<"));
        assert!(html0.contains(">0<"));
        assert_eq!(html0.matches("lc-costrail-group").count(), 0);
        no_hex(&html0);
    }

    #[test]
    fn test_cost_rail_above_label_is_the_focus_ordinal_not_the_hand_size() {
        // Regression pin for Important 2: a 15-card hand's above-label must
        // read "01" (the focused card's ordinal), never "15" (the hand
        // size) — the bug this test would have caught rendered `{n:02}` for
        // both the above and below labels.
        let hand: Vec<Card> = (0..15)
            .map(|i| rail_card(&format!("rail-{i}"), Deck::Beer, 1))
            .collect();
        let html = cost_rail(&hand, 100);
        assert!(html.contains(r#"<span class="lc-costrail-above">01</span>"#));
        assert!(!html.contains(r#"<span class="lc-costrail-above">15</span>"#));
        assert!(html.contains(r#"<span class="lc-costrail-below">15</span>"#));
        no_hex(&html);
    }

    #[test]
    fn test_armed_column_states() {
        let empty: [Card; 0] = [];
        let html_empty = armed_column(&empty, false);
        assert!(html_empty.contains("ARMED 0"));
        assert!(html_empty.contains(r#"data-count="0""#));
        assert_eq!(html_empty.matches("lc-armed-slot").count(), 1);
        assert!(!html_empty.contains("data-locked"));
        assert!(html_empty.contains(r#"data-flight-anchor="armed""#));
        no_hex(&html_empty);

        let two = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
        ];
        let html_two = armed_column(&two, false);
        assert!(html_two.contains("ARMED 2"));
        assert!(html_two.contains(r#"data-count="2""#));
        assert_eq!(
            html_two.matches(r#"<div class="lc-mini lc-deck-"#).count(),
            2
        );
        assert!(html_two.contains(r#"data-card-id="beer-fx""#));
        assert!(html_two.contains(r#"data-card-id="wine-fx""#));
        assert_eq!(html_two.matches("lc-armed-slot").count(), 1);
        no_hex(&html_two);

        let three = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
            rail_card("liquor-fx", Deck::Liquor, 3),
        ];
        let html_locked = armed_column(&three, true);
        assert!(html_locked.contains("LOCKED 3"));
        assert!(html_locked.contains(r#"data-count="3""#));
        assert!(html_locked.contains("data-locked "));
        // Never a value form — the brief's rule: bare presence only.
        assert!(!html_locked.contains("data-locked="));
        assert_eq!(html_locked.matches("lc-armed-slot").count(), 0);
        assert!(!html_locked.contains("ARMED"));
        no_hex(&html_locked);
    }

    #[test]
    fn test_armed_column_carries_its_motion_anchor() {
        let html = armed_column(&[], false);
        assert!(html.contains(r#"data-flight-anchor="armed""#));
    }

    /// Plan E Task 4 / F.1 — one assertion pair per row of the state table,
    /// the exact copy string present and the states it must NOT show absent.
    #[test]
    fn test_action_bar_states() {
        fn base() -> ActionBarView {
            ActionBarView {
                beat: Beat::Draw,
                round: 1,
                seated: true,
                alive: true,
                locked: false,
                drawing: false,
                vessels: Vec::new(),
                charged: 0,
                vessels_registered: 0,
                outcome: None,
            }
        }

        // outcome wins over beat, from any beat.
        let mut ab = base();
        ab.beat = Beat::Lock;
        ab.outcome = Some(LcOutcome::Winner(0));
        let html = lc_action_bar(&ab);
        assert!(html.contains("END GAME"));
        assert!(html.contains(r#"data-lc-post="end""#));
        assert!(!html.contains("LOCK IN"));
        no_hex(&html);

        // unseated: spectating, even mid-Lock.
        let mut ab = base();
        ab.beat = Beat::Lock;
        ab.seated = false;
        let html = lc_action_bar(&ab);
        assert!(html.contains("SPECTATING"));
        no_hex(&html);

        // eliminated: out, even mid-Lock.
        let mut ab = base();
        ab.beat = Beat::Lock;
        ab.alive = false;
        let html = lc_action_bar(&ab);
        assert!(html.contains("YOU'RE OUT — HAUNT THE TABLE"));
        no_hex(&html);

        // lobby (round 1), fewer than two drinks registered: disabled START.
        let mut ab = base();
        ab.vessels_registered = 1;
        let html = lc_action_bar(&ab);
        assert!(html.contains("START ROUND 1"));
        assert!(html.contains(" disabled"));
        assert!(html.contains("NEEDS 2 DRINKS REGISTERED"));
        no_hex(&html);

        // lobby (round 1), two registered: enabled START, no hint.
        let mut ab = base();
        ab.vessels_registered = 2;
        let html = lc_action_bar(&ab);
        assert!(html.contains("START ROUND 1"));
        assert!(!html.contains(" disabled"));
        assert!(!html.contains("NEEDS 2 DRINKS REGISTERED"));
        no_hex(&html);

        // draw, round >= 2, not drawing: per-vessel buttons + sit tight.
        let mut ab = base();
        ab.round = 2;
        ab.vessels = vec![(0, Deck::Beer), (1, Deck::Soft)];
        let html = lc_action_bar(&ab);
        assert!(html.contains(r#"data-vessel="0">FINISH BEER · DRAW"#));
        assert!(html.contains(r#"data-vessel="1">FINISH SOFT · DRAW"#));
        assert!(html.contains("OR SIT TIGHT"));
        no_hex(&html);

        // draw, round >= 2, drawing: the dealt hint, no buttons.
        let mut ab = base();
        ab.round = 2;
        ab.drawing = true;
        let html = lc_action_bar(&ab);
        assert!(html.contains("FRESH VESSEL — DEALT"));
        assert!(!html.contains("data-lc-post=\"draw\""));
        no_hex(&html);

        // deal: the auto-beat hint.
        let mut ab = base();
        ab.beat = Beat::Deal;
        let html = lc_action_bar(&ab);
        assert!(html.contains("DEALING…"));
        no_hex(&html);

        // diplomacy: the talk-it-out hint.
        let mut ab = base();
        ab.beat = Beat::Diplomacy;
        let html = lc_action_bar(&ab);
        assert!(html.contains("TALK IT OUT — DEALS AREN'T BINDING"));
        no_hex(&html);

        // lock, unlocked: LOCK IN.
        let mut ab = base();
        ab.beat = Beat::Lock;
        let html = lc_action_bar(&ab);
        assert!(html.contains("LOCK IN"));
        assert!(!html.contains("LOCKED — WAITING"));
        no_hex(&html);

        // lock, locked: waiting on the table.
        let mut ab = base();
        ab.beat = Beat::Lock;
        ab.locked = true;
        let html = lc_action_bar(&ab);
        assert!(html.contains("LOCKED — WAITING FOR THE TABLE"));
        assert!(!html.contains("LOCK IN"));
        no_hex(&html);

        // reveal, charged: DRINK n, amber (F.1: the drinking-adjacent
        // primary is always lc-btn-drink).
        let mut ab = base();
        ab.beat = Beat::Reveal;
        ab.charged = 3;
        let html = lc_action_bar(&ab);
        assert!(html.contains("DRINK 3"));
        assert!(html.contains("lc-btn-drink"));
        assert!(!html.contains("NOTHING TO PAY"));
        no_hex(&html);

        // reveal, nothing charged: NOTHING TO PAY.
        let mut ab = base();
        ab.beat = Beat::Reveal;
        let html = lc_action_bar(&ab);
        assert!(html.contains("NOTHING TO PAY"));
        no_hex(&html);

        // resolve mirrors reveal.
        let mut ab = base();
        ab.beat = Beat::Resolve;
        ab.charged = 1;
        let html = lc_action_bar(&ab);
        assert!(html.contains("DRINK 1"));
        no_hex(&html);
    }
}
