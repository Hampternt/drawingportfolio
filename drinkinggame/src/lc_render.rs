//! Last Call fragments as formatted strings, matching `render.rs`. Public
//! builders take `&PublicView`/`&PublicSeat` — never `&LastCallState` — so an
//! unrevealed card cannot reach a broadcast fragment by construction (spec
//! §3.4). Every root and attribute here is the §7.8 contract; changing one is
//! a breaking change for Plan A2 and Plan B.

use crate::last_call::{
    effective_pull_cost_raw, Beat, Card, Deck, LcOutcome, LogEntry, Play, PublicSeat, PublicView,
    Status, DECK_LOW_THRESHOLD,
};
use crate::lc_events::event_def;
use crate::lc_layout::{seat_positions, view_index, SeatPos};
use crate::lc_tabs::{TabDef, TabReward};
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
    /// Whether the active event halves pull costs (`LastCallState::cost_halved`,
    /// H12). I1 (Plan H review): threaded through so `cost_rail`'s per-card
    /// prices price through the same seam as `arm`/`lock_in`/the reveal
    /// charge/the DRINK chip — a Happy Hour round can no longer show a rail
    /// price the engine won't actually charge.
    pub halved: bool,
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
    let rail = cost_rail(hg.hand, hg.handicap_pct, hg.halved);
    format!(r#"<div class="lc-handgroup">{armed}{wheel}{rail}</div>"#)
}

/// §7.8 `CostRail` — private, the viewer's hand priced through their own
/// handicap. `pc = effective_pull_cost_raw(card.cost, handicap_pct, halved)`
/// — decision 1: the rail shows the true pull price, rounded up, further
/// halved when `halved` (I1, Plan H review: the same seam
/// `LastCallState::effective_pull_cost` charges through, so the rail and
/// the DRINK chip can never disagree during Happy Hour) — and each group
/// renders `pc` bars. `is-active` is emitted server-side on `data-idx="0"`
/// only (the initial focus); the JS moves it thereafter. `lc-deck-{slug}`
/// on the group supplies `--lc-ink` to its bars, same convention as every
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
pub fn cost_rail(hand: &[Card], handicap_pct: u16, halved: bool) -> String {
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
            let pc = effective_pull_cost_raw(card.cost, handicap_pct, halved);
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
    if seat.ready {
        state_classes.push_str(" is-ready");
    }
    if seat.drawing {
        state_classes.push_str(" is-drawing");
    }
    if eliminated {
        state_classes.push_str(" is-eliminated");
    }

    // The lock tick doubles as the ready tick — same glyph, same slot; the
    // plaque's is-locked/is-ready classes pick which rule shows it.
    let lock_tick = if seat.locked || seat.ready {
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

/// DeckListRow (J11) — the v2 mockup's compact right-rail row for the big
/// screen: a `card_dot`-style tint, the deck name, its discard count and the
/// remaining count, in one line. Replaces `deck_stack` in `lc_screen_panel`'s
/// right rail only; `deck_stack` itself ships unchanged for the preview page
/// (Plan A-vis's gallery still demonstrates it). States mirror
/// `deck_stack`'s: `data-low` under `DECK_LOW_THRESHOLD` (and above zero),
/// `data-empty` at 0 — both bare-presence attributes, never `="false"`, so
/// the CSS can select on `[data-low]`/`[data-empty]` alone.
pub fn deck_list_row(deck: Deck, count: u16, discarded: usize) -> String {
    let slug = deck.slug();
    let label = deck.label();
    let low = if count > 0 && count < DECK_LOW_THRESHOLD {
        " data-low"
    } else {
        ""
    };
    let empty = if count == 0 { " data-empty" } else { "" };
    format!(
        r#"<div class="lc-deckrow" data-deck="{slug}" data-count="{count}"{low}{empty} data-flight-anchor="deck-{slug}"><span class="lc-deckrow-dot lc-deck-{slug}"></span><span class="lc-deckrow-name">{label}</span><span class="lc-deckrow-disc">disc {discarded}</span><span class="lc-deckrow-count">{count}</span></div>"#
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
    // The live timer died with the beat clock (2026-08-13): beats wait for
    // the table's ready/lock taps, so the banner carries no countdown.
    // (`beat_timer` survives for the static preview page only.)
    //
    // Plan E, decision E13: game over is the frozen Resolve tableau (D16),
    // but the banner switches to a dedicated GAME OVER state rather than
    // going on saying RESOLVE — the beat name stops meaning anything once
    // the round stopped mid-cycle. `view.outcome` alone gates this, not
    // `beat == Resolve` (which the freeze always is anyway, but that is not
    // the reason).
    if view.outcome.is_some() {
        // Plan H review fix (⚠️ 1 / erratum a2e66ab): game over owns the
        // EVENT chip outright (H6) — no event strip ever renders here — but
        // a settle from the game-ending round still has nowhere else to
        // surface (`PublicView.settled`'s only consumer is this function),
        // so the erratum's `ended && t.round == self.round` arm would
        // otherwise be permanently dead. Names render on the frozen
        // tableau; the event does not.
        let strip = settled_strip(&view.settled);
        return format!(
            r#"<div class="lc-banner lc-beat-rose" id="lc-banner" data-beat="{slug}" data-round="{round}"><span class="lc-banner-beat">GAME OVER</span><span class="lc-banner-meta">ROUND {round} &middot; LAST CALL</span>{strip}</div>"#,
            slug = beat.slug(),
            round = view.round,
        );
    }
    // Plan H, Task 4: one strip, at most one occupant — first match wins.
    // 1. the round's event (Deal onward, H6) outranks the settlement
    //    announcement outright (H2's lifecycle means they never truly
    //    coexist, but the renderer still has to pick one).
    // 2. name-only settlement announcements, Draw beat only (H11) — never
    //    the tab id/title, which stays off `PublicView` entirely (H8).
    // An event id `event_def` does not recognise renders nothing — H3's
    // fail-soft, applied here at the display layer too.
    let event_strip = if let Some(id) = &view.event {
        match event_def(id) {
            Some(def) => format!(
                r#"<div class="lc-event" data-event="{id}"><span class="lc-event-name">{title}</span><span class="lc-event-text">{text}</span></div>"#,
                id = html_escape(id),
                title = html_escape(def.title),
                text = html_escape(def.text),
            ),
            None => String::new(),
        }
    } else if beat == Beat::Draw && !view.settled.is_empty() {
        settled_strip(&view.settled)
    } else {
        String::new()
    };
    format!(
        r#"<div class="lc-banner lc-beat-{hue}" id="lc-banner" data-beat="{slug}" data-round="{round}"><span class="lc-banner-beat">{label}</span><span class="lc-banner-meta">ROUND {round} &middot; BEAT {index} OF 6</span>{event_strip}</div>"#,
        hue = beat.hue(),
        slug = beat.slug(),
        round = view.round,
        label = beat.label(),
        index = beat.index(),
    )
}

/// Plan H review fix (Important 1): collapses every settled name into a
/// single `.lc-event` row, never more. The spectator screen scales this
/// component up into `.lc-screen-head`, a fixed `height: 88px` /
/// `flex: 0 0 88px` box (F.2, authored design — it does not grow); a
/// second wrapped row measured ~31px of overflow spilling through the
/// header's border into the felt/rail below. The first settling name
/// always renders in full; any more collapse into a `+{n}` count rather
/// than a second line — never a second `.lc-event` div. Empty input (the
/// common case — most rounds settle nothing) renders nothing.
fn settled_strip(settled: &[String]) -> String {
    let Some(first) = settled.first() else {
        return String::new();
    };
    let name = html_escape(&first.to_uppercase());
    let extra = settled.len() - 1;
    if extra == 0 {
        format!(
            r#"<div class="lc-event" data-settled><span class="lc-event-name">{name} SETTLED A TAB</span></div>"#
        )
    } else {
        format!(
            r#"<div class="lc-event" data-settled><span class="lc-event-name">{name} SETTLED A TAB</span><span class="lc-event-text">&middot; +{extra}</span></div>"#
        )
    }
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
/// no new SSE event, no new publish, the frame count is unchanged. Plan J
/// (Task 2) adds a third `<template data-lc-log>`, same move again: the LOG
/// pane repaints off this frame too, no new event, no new publish. Every
/// `<template>` wrapper mirrors the existing `room` event's
/// `<template data-topbar>` convention in `room.html` — one SSE message
/// carrying several destinations. `lc_screen_panel` and `lc_log` are both
/// pure string builds (no I/O, no `.await`), so this stays safe to call from
/// `broadcast_lc`, which must remain awaitless (`1e742d4`).
pub fn lc_public_panel(view: &PublicView) -> String {
    format!(
        r#"<div data-lc-public data-seq="{seq}"><template data-lc-banner>{banner}</template><template data-lc-screen>{screen}</template><template data-lc-log>{log}</template></div>"#,
        seq = view.seq,
        banner = lc_banner(view),
        screen = lc_screen_panel(view),
        log = lc_log(view),
    )
}

// ---------------------------------------------------------------------
// Plan J additions — Task 2. The public round log: `lc_log` renders
// `PublicView::log` (J1's capped, public-only `LogEntry` vocabulary) into
// the `#lc-log` pane, newest first.
// ---------------------------------------------------------------------

/// A seat's name for the log — uppercased and escaped, or `SEAT {n+1}` when
/// `view.seats` doesn't carry that index. Distinct from `seat_name_upper`
/// (which returns `""` in the same case, fine for a caption fragment): a log
/// line always needs SOME token to fill its `{NAME}` slot, per the brief.
fn log_seat_name(view: &PublicView, seat: usize) -> String {
    view.seats
        .iter()
        .find(|s| s.seat == seat)
        .map(|s| html_escape(&s.name.to_uppercase()))
        .unwrap_or_else(|| format!("SEAT {}", seat + 1))
}

/// The serde tag `LogEntry`'s `#[serde(tag = "t", rename_all = "snake_case")]`
/// would produce for `entry` — kept as an explicit match (rather than
/// serializing and re-parsing) so `lc_log` stays a pure, cheap string build.
fn log_tag(entry: &LogEntry) -> &'static str {
    match entry {
        LogEntry::Round { .. } => "round",
        LogEntry::Joined { .. } => "joined",
        LogEntry::Vessel { .. } => "vessel",
        LogEntry::Handicap { .. } => "handicap",
        LogEntry::Draw { .. } => "draw",
        LogEntry::Lock { .. } => "lock",
        LogEntry::Play { .. } => "play",
        LogEntry::Hit { .. } => "hit",
        LogEntry::Heal { .. } => "heal",
        LogEntry::Shield { .. } => "shield",
        LogEntry::Drain { .. } => "drain",
        LogEntry::Fizzle { .. } => "fizzle",
        LogEntry::Eliminated { .. } => "eliminated",
        LogEntry::Reshuffle { .. } => "reshuffle",
        LogEntry::GameOver { .. } => "game_over",
        LogEntry::PactBreak { .. } => "pact_break",
        LogEntry::TabSettle { .. } => "tab_settle",
        LogEntry::ReactionPlay { .. } => "reaction_play",
        LogEntry::Haunt { .. } => "haunt",
    }
}

/// One log entry's copy line, per the brief's table. Names are uppercased
/// and escaped (`log_seat_name`); card titles are escaped only, never
/// uppercased. `−` is U+2212 in the damage lines, matching the HP chip
/// convention.
fn log_line(view: &PublicView, entry: &LogEntry) -> String {
    match entry {
        LogEntry::Round { round } => format!("— ROUND {round} —"),
        LogEntry::Joined { seat } => format!("{} TAKES A SEAT", log_seat_name(view, *seat)),
        LogEntry::Vessel { seat, deck } => {
            format!("{} REGISTERS {}", log_seat_name(view, *seat), deck.label())
        }
        LogEntry::Handicap { seat, pct } => {
            format!("{} HANDICAP {pct}%", log_seat_name(view, *seat))
        }
        LogEntry::Draw { seat, deck, n } => format!(
            "{} FINISHES A {} · +{n}",
            log_seat_name(view, *seat),
            deck.label()
        ),
        LogEntry::Lock { seat } => format!("{} LOCKS IN", log_seat_name(view, *seat)),
        LogEntry::Play {
            seat,
            title,
            target,
        } => match target {
            Some(t) => format!(
                "{} PLAYS {} → {}",
                log_seat_name(view, *seat),
                html_escape(title),
                log_seat_name(view, *t)
            ),
            None => format!(
                "{} PLAYS {}",
                log_seat_name(view, *seat),
                html_escape(title)
            ),
        },
        LogEntry::Hit {
            source,
            target,
            amount,
        } => format!(
            "{} HITS {} −{amount}",
            log_seat_name(view, *source),
            log_seat_name(view, *target)
        ),
        LogEntry::Heal { seat, amount } => format!("{} +{amount} HP", log_seat_name(view, *seat)),
        LogEntry::Shield { seat, amount } => {
            format!("{} SHIELDS {amount}", log_seat_name(view, *seat))
        }
        LogEntry::Drain {
            source,
            target,
            amount,
        } => format!(
            "{} DRAINS {} −{amount} PULLS",
            log_seat_name(view, *source),
            log_seat_name(view, *target)
        ),
        LogEntry::Fizzle { title, .. } => format!("{} FIZZLES", html_escape(title)),
        LogEntry::Eliminated { seat } => format!("{} IS OUT", log_seat_name(view, *seat)),
        LogEntry::Reshuffle { deck } => format!("{} RESHUFFLES", deck.label()),
        // Fix wave (Important 1): a pact win names both winners, matching
        // the end card's/banner's "THE PACT HOLDS" framing on the same
        // screen instead of the solo-outlast copy for one of the two.
        LogEntry::GameOver { winner, winner2 } => match (winner, winner2) {
            (Some(a), Some(b)) => format!(
                "GAME OVER — {} & {} — THE PACT HOLDS",
                log_seat_name(view, *a),
                log_seat_name(view, *b)
            ),
            (Some(w), None) => {
                format!("GAME OVER — {} OUTLASTS THE TABLE", log_seat_name(view, *w))
            }
            (None, _) => "GAME OVER — EVERYBODY'S OUT".to_string(),
        },
        // Task 1 erratum (Task 2 adjudication) — the four social events.
        LogEntry::PactBreak { betrayer, betrayed } => format!(
            "{} BROKE THEIR PACT WITH {}",
            log_seat_name(view, *betrayer),
            log_seat_name(view, *betrayed)
        ),
        // `seat` ONLY — never the tab or its reward (H8/H11).
        LogEntry::TabSettle { seat } => format!("{} SETTLED A TAB", log_seat_name(view, *seat)),
        LogEntry::ReactionPlay { seat, title } => format!(
            "{} ANSWERS WITH {}",
            log_seat_name(view, *seat),
            html_escape(title)
        ),
        LogEntry::Haunt { seat, target } => match target {
            Some(t) => format!(
                "{} HAUNTS {}",
                log_seat_name(view, *seat),
                log_seat_name(view, *t)
            ),
            None => format!("{} HAUNTS THE TABLE", log_seat_name(view, *seat)),
        },
    }
}

/// The `#lc-log` pane body (§7.8 DOM contract) — newest first, since the
/// most recent thing that happened is what a viewer opening the tab wants to
/// see first. `view.log` arrives already capped at `LC_LOG_CAP`
/// (`LastCallState::push_log`); this renders every entry it's given, no
/// further truncation.
pub fn lc_log(view: &PublicView) -> String {
    if view.log.is_empty() {
        return r#"<div id="lc-log" data-count="0"><p class="lc-empty">Nothing logged yet.</p></div>"#
            .to_string();
    }
    let rows: String = view
        .log
        .iter()
        .rev()
        .map(|entry| {
            format!(
                r#"<li class="lc-log-row" data-t="{tag}">{line}</li>"#,
                tag = log_tag(entry),
                line = log_line(view, entry),
            )
        })
        .collect();
    format!(
        r#"<div id="lc-log" data-count="{len}"><ol class="lc-log">{rows}</ol></div>"#,
        len = view.log.len(),
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
    lobby: bool,
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
            // Plan J Task 4: unconditional (not just in the lobby) — the
            // §7.8 setup section marks every player who has registered a
            // vessel, at any round, so a mid-game handicap tweak still shows
            // who's dealt in.
            let ready_attr = if row.decks.is_empty() {
                ""
            } else {
                " data-ready"
            };
            format!(
                r#"<form class="lc-setup-row"{ready_attr} method="post" action="{base_path}/room/{code}/lastcall/handicap"><input type="hidden" name="target" value="{player_id}"><span>{name}{you}</span><span class="lc-setup-decks">{dots}</span><input type="number" name="handicap_pct" min="25" max="300" step="5" value="{handicap_pct}"><button type="submit">SET</button></form>"#,
                player_id = row.player_id,
                name = html_escape(&row.name),
                handicap_pct = row.handicap_pct,
            )
        })
        .collect();

    // Plan J Task 4: the phone's half of the lobby polish — who's still
    // unregistered, or the all-set line once everyone has a vessel. Gated
    // on `lobby` (round-1 Draw, no outcome — E1's gate, computed by the
    // caller from `&LastCallState` so this builder stays `PublicView`-only
    // in spirit without taking the whole view just for two fields) so a
    // mid-game handicap tweak never grows a stray "WAITING ON" line.
    let wait_line = if !lobby {
        String::new()
    } else {
        let waiting: Vec<&SetupRow> = rows.iter().filter(|r| r.decks.is_empty()).collect();
        if waiting.is_empty() {
            r#"<p class="lc-lobby-wait" data-waiting="0">ALL SET — PRESS START</p>"#.to_string()
        } else {
            let names = waiting
                .iter()
                .map(|r| html_escape(&r.name.to_uppercase()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                r#"<p class="lc-lobby-wait" data-waiting="{n}">WAITING ON {n}: {names}</p>"#,
                n = waiting.len(),
            )
        }
    };

    format!(
        r#"<div id="lc-hand" data-seq="{seq}" data-count="{count}" data-flight-anchor="hand"><section class="lc-setup"><h2>Your drink</h2><form method="post" action="{base_path}/room/{code}/lastcall/vessel"><select name="deck">{deck_options}</select><input name="container" maxlength="24" placeholder="50cl can"><button type="submit">REGISTER</button></form>{wait_line}<h2>Handicaps</h2>{handicap_rows}</section>{group}</div>"#,
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
        r#"<div class="lc-centre-play" data-seat="{seat}"><span class="lc-centre-cap">{src} &rarr; {tgt}</span>{mini}{chips}</div>"#,
        seat = play.source_seat,
        mini = card_mini(&play.card),
        chips = centre_chips(view, play),
    )
}

/// Plan I Task 5: the reactions/haunts riding one revealed play, rendered
/// straight off `PublicView` — both `reactions` and `haunts` are public
/// from the moment they're cast (I9/I10), so there is nothing here to hide.
/// Order is played/cast order, i.e. the projected `Vec`'s own order — never
/// re-sorted. Empty string when nothing rides this play, so a play with no
/// riders adds no `.lc-centre-chips` wrapper at all.
fn centre_chips(view: &PublicView, play: &Play) -> String {
    let react_chips: String = view
        .reactions
        .iter()
        .filter(|r| r.answers == play.order_key)
        .map(|r| {
            format!(
                r#"<span class="lc-chip lc-chip-react" data-deck="{deck}">{reactor}: {title}</span>"#,
                deck = r.card.deck.slug(),
                reactor = seat_name_upper(view, r.source_seat),
                title = html_escape(&r.card.title),
            )
        })
        .collect();
    let haunt_chips: String = view
        .haunts
        .iter()
        .filter(|h| h.play == play.order_key)
        .map(|h| {
            format!(
                r#"<span class="lc-chip lc-chip-haunt">{ghost} +1</span>"#,
                ghost = seat_name_upper(view, h.seat),
            )
        })
        .collect();
    if react_chips.is_empty() && haunt_chips.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="lc-centre-chips">{react_chips}{haunt_chips}</div>"#)
    }
}

/// The one-line victory headline, shared by the felt centre (`victory_line`)
/// and the phone end card (`lc_end_card`) so the two surfaces can never
/// disagree on the wording.
fn outcome_headline(view: &PublicView, outcome: LcOutcome) -> String {
    match outcome {
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
    }
}

/// J6 order: alive by hp desc, then eliminated by elim_order desc; ties by
/// lower seat. Pure — the end card and the screen board share it.
pub fn final_standings(view: &PublicView) -> Vec<&PublicSeat> {
    let mut seats: Vec<&PublicSeat> = view.seats.iter().collect();
    seats.sort_by_key(|s| {
        let rank = if s.status == Status::Alive {
            -(s.hp as i64)
        } else {
            -(s.elim_order.unwrap_or(0) as i64)
        };
        (s.status != Status::Alive, rank, s.seat)
    });
    seats
}

/// Plan J Task 4 / E1: the felt centre's lobby state — round-1 Draw before
/// the table has enough vessels registered to start. `k` is the number of
/// seats with at least one vessel (E1's own "ready" test), `n` the seat
/// count; the names line lists the unregistered seats and is omitted
/// entirely once `k == n`, matching the phone's own all-set line.
fn lobby_centre(view: &PublicView) -> String {
    let n = view.seats.len();
    let waiting: Vec<&PublicSeat> = view.seats.iter().filter(|s| s.vessels.is_empty()).collect();
    let k = n - waiting.len();
    let names_html = if waiting.is_empty() {
        String::new()
    } else {
        let names = waiting
            .iter()
            .map(|s| html_escape(&s.name.to_uppercase()))
            .collect::<Vec<_>>()
            .join(", ");
        format!(r#"<span class="lc-lobby-names">WAITING ON {names}</span>"#)
    };
    format!(
        r#"<div class="lc-centre-lobby"><span class="lc-lobby-kicker">LAST CALL</span><span class="lc-lobby-count">{k} / {n} DRINKS IN</span>{names_html}</div>"#
    )
}

/// One `<li class="lc-standing">` row — shared by the phone's full table
/// (`lc_end_card`) and the felt centre's capped-to-4 board (`victory_line`).
/// `me` is `None` on the broadcast surface (no single viewer to mark).
/// `data-winner` marks the `Winner(seat)` row only — a pact names two
/// winners in the headline, but the row attribute is singular by contract.
fn standing_row_html(
    seat: &PublicSeat,
    place: usize,
    outcome: LcOutcome,
    me: Option<usize>,
) -> String {
    let me_attr = if me == Some(seat.seat) {
        " data-me"
    } else {
        ""
    };
    let winner_attr = if matches!(outcome, LcOutcome::Winner(w) if w == seat.seat) {
        " data-winner"
    } else {
        ""
    };
    let fate = if seat.status == Status::Alive {
        format!("HP {}", seat.hp)
    } else {
        // Fix wave (M4): `elim_order` is 1-based (J5) — every seat the
        // engine actually eliminates gets one at the same site its status
        // flips (`resolve()`, last_call.rs). `None` here can only come from
        // a hand-built or corrupt blob; fall back to a bare "OUT" rather
        // than the nonsense ordinal "OUT #0" a naive `unwrap_or(0)` would
        // print.
        match seat.elim_order {
            Some(n) => format!("OUT #{n}"),
            None => "OUT".to_string(),
        }
    };
    format!(
        r#"<li class="lc-standing" data-seat="{s}"{me_attr}{winner_attr}><span class="lc-standing-place">{place}</span><span class="lc-standing-name">{name}</span><span class="lc-standing-fate">{fate}</span><span class="lc-standing-stats">DMG {dmg} · PULLS {pulls} · CARDS {cards}</span></li>"#,
        s = seat.seat,
        name = html_escape(&seat.name.to_uppercase()),
        dmg = seat.damage_dealt,
        pulls = seat.pulls_spent,
        cards = seat.cards_played,
    )
}

/// Plan E, decision E13: the frozen Resolve tableau's one line, in place of
/// the felt-centre plays once the game has an `outcome`. Plan J Task 3: now
/// also the broadcast standings board beneath it — capped to the first 4
/// rows plus a "+N MORE" tail (the felt centre is not a spreadsheet; phones
/// carry the full table via `lc_end_card`).
fn victory_line(view: &PublicView, outcome: LcOutcome) -> String {
    let line = outcome_headline(view, outcome);
    let standings = final_standings(view);
    let total = standings.len();
    const CAP: usize = 4;
    let rows: String = standings
        .iter()
        .copied()
        .take(CAP)
        .enumerate()
        .map(|(i, seat)| standing_row_html(seat, i + 1, outcome, None))
        .collect();
    let more = if total > CAP {
        format!(
            r#"<span class="lc-standings-more">+{} MORE</span>"#,
            total - CAP
        )
    } else {
        String::new()
    };
    format!(
        r#"<div class="lc-centre-victory">{line}<ol class="lc-standings">{rows}</ol>{more}</div>"#
    )
}

/// The phone's game-over pane body — everything inside `#lc-hand` once the
/// game has an outcome. Root is the pane body, not `#lc-hand` itself:
/// `hand_pane_html` (lc_routes.rs) wraps it in that div so the root id and
/// `data-seq` keep `lcApply`'s stale-drop gate working unchanged. Full
/// standings, one row per seat, no cap (the phone is not the felt centre);
/// `me` marks the viewer's own row via `data-me`. `""` if called without an
/// outcome — defensive only, `hand_pane_html` never does.
pub fn lc_end_card(view: &PublicView, me: Option<usize>) -> String {
    let Some(outcome) = view.outcome else {
        return String::new();
    };
    let victory = outcome_headline(view, outcome);
    let rows: String = final_standings(view)
        .into_iter()
        .enumerate()
        .map(|(i, seat)| standing_row_html(seat, i + 1, outcome, me))
        .collect();
    format!(
        r#"<section class="lc-endcard"><span class="lc-endcard-kicker">GAME OVER</span><h2 class="lc-endcard-victory">{victory}</h2><ol class="lc-standings">{rows}</ol></section>"#
    )
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
    // the frozen tableau's victory line; otherwise the lobby gate (Plan J
    // Task 4 / E1: round-1 Draw with no outcome — mutually exclusive with
    // both `outcome` and `revealed` by construction, since `revealed` only
    // populates at Reveal/Resolve); otherwise `revealed` (populated only at
    // Reveal/Resolve, per `public_view`'s own gate — Plan D's projection
    // tests are the authority on secrecy, not this renderer) shows the
    // ordered plays; empty either way renders nothing. `outcome` is checked
    // first regardless — the frozen tableau is never allowed to show stale
    // plays.
    let lobby = view.round == 1 && view.beat == Beat::Draw && view.outcome.is_none();
    let centre = if let Some(outcome) = view.outcome {
        victory_line(view, outcome)
    } else if lobby {
        lobby_centre(view)
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

    // J11: the v2 mockup's compact deck list rows replace the deck stacks on
    // the big screen only — `deck_stack` itself still ships for the preview
    // page. Zipped by position: both `deck_counts` and `discard_counts` are
    // `Deck::ALL`-ordered (see `public_view`'s projection), so pairing by
    // index never mismatches a count to the wrong deck.
    let deck_rows: String = view
        .deck_counts
        .iter()
        .zip(view.discard_counts.iter())
        .map(|(&(deck, count), &(_, discarded))| deck_list_row(deck, count, discarded))
        .collect();
    let right_rail = format!(
        r#"<div class="lc-rail lc-rail-right"><div class="lc-rail-kicker">DECKS LEFT</div><div class="lc-rail-decks">{deck_rows}{discard}</div></div>"#,
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
                // `data-locked` doubles as the ready marker — one border
                // treatment for "this seat is done with the beat".
                let locked_attr = if seat.locked || seat.ready {
                    " data-locked"
                } else {
                    ""
                };
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
    /// The viewer's own ready tick — drives the open beats' READY button
    /// swap the same way `locked` drives Lock's.
    pub ready: bool,
    pub drawing: bool,
    pub vessels: Vec<(usize, Deck)>, // (vessel index, deck)
    pub charged: u8,                 // viewer's pulls at the reveal (E9)
    pub vessels_registered: usize,   // players with >= 1 vessel (E1 gate)
    pub outcome: Option<LcOutcome>,
    // Plan I Task 5: filled route-side from `&LastCallState` (never
    // broadcast — this whole struct is the private, per-viewer projection
    // `lc_action_bar` renders from, same as `charged`/`vessels_registered`
    // above).
    pub haunt_plays: Vec<(u32, String)>, // (order_key, "SRC → TGT"), damage plays only
    pub haunted: bool,                   // this ghost already voted this round
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
        // Plan J Task 3 / J8: REMATCH (any member, gated game-over server-
        // side by `lc_rematch_handler`) replaces the old solitary END GAME
        // button; END NIGHT (still `data-lc-post="end"`, `lc_end_handler`
        // unchanged) is the exit that closes the table for good. Both post
        // through the same delegated `[data-lc-post]` listener — no JS
        // change needed.
        return r#"<button class="lc-btn lc-btn-drink" data-lc-post="rematch">REMATCH</button><button class="lc-btn lc-btn-secondary" data-lc-post="end">END NIGHT</button>"#
            .to_string();
    }
    if !ab.seated {
        return r#"<p class="lc-actions-hint">SPECTATING</p>"#.to_string();
    }
    if !ab.alive {
        // Plan I Task 5 / DDv2 §9.2: a ghost's one decision, gated the same
        // way `!alive` always was (this branch only ever ran mid-Reveal or
        // otherwise — a ghost never had a Draw/Lock decision either), now
        // three rows instead of one. `haunted` wins over `haunt_plays` —
        // once a ghost has cast this round's vote there is nothing left to
        // pick, regardless of what's still in flight.
        return match ab.beat {
            Beat::Reveal if ab.haunted => {
                r#"<p class="lc-actions-hint">YOUR CURSE IS CAST</p>"#.to_string()
            }
            Beat::Reveal if !ab.haunt_plays.is_empty() => ab
                .haunt_plays
                .iter()
                .map(|(order_key, caption)| {
                    format!(
                        r#"<button class="lc-btn lc-haunt-btn" data-lc-post="haunt" data-play="{order_key}">HAUNT {caption} +1</button>"#
                    )
                })
                .collect(),
            _ => r#"<p class="lc-actions-hint">YOU'RE OUT — HAUNT THE TABLE</p>"#.to_string(),
        };
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
                format!(
                    r#"<p class="lc-actions-hint">FRESH VESSEL — DEALT</p>{}"#,
                    ready_control(ab.ready)
                )
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
                format!("{buttons}{}", ready_control(ab.ready))
            }
        }
        Beat::Deal => r#"<p class="lc-actions-hint">DEALING…</p>"#.to_string(),
        Beat::Diplomacy => {
            format!(
                r#"<p class="lc-actions-hint">TALK IT OUT — DEALS AREN'T BINDING</p>{}"#,
                ready_control(ab.ready)
            )
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
            let pay = if ab.charged > 0 {
                format!(
                    r#"<div class="lc-btn lc-btn-drink lc-drink-now">DRINK {}</div>"#,
                    ab.charged
                )
            } else {
                r#"<p class="lc-actions-hint">NOTHING TO PAY</p>"#.to_string()
            };
            // Resolve never renders for a live table (`lc_advance_chain`
            // collapses it in the same pass), so the ready control is
            // Reveal's — gated anyway, for the frozen-tableau edge.
            if ab.beat == Beat::Reveal {
                format!("{pay}{}", ready_control(ab.ready))
            } else {
                pay
            }
        }
    }
}

/// The open beats' READY control (clock removal, 2026-08-13): the tap that
/// replaces the countdown, or Lock's "waiting" hint once it's cast. Not
/// `lc-btn-drink` — READY is never a drinking instruction, so the amber
/// primary stays reserved (F.1) and the button takes the secondary style.
fn ready_control(ready: bool) -> String {
    if ready {
        r#"<p class="lc-actions-hint">READY — WAITING FOR THE TABLE</p>"#.to_string()
    } else {
        r#"<button class="lc-btn lc-btn-secondary" data-lc-post="ready">READY</button>"#.to_string()
    }
}

/// Plan H Task 5 / decision H13: the private tab card — a secret, per-seat
/// side quest rendered ONLY into the viewer's own hand fragment (the
/// `ActionBarView` precedent: a private-side builder that never touches
/// `PublicView`). `hand_pane_html` is the sole caller, mirroring
/// `pacts_section_html`'s contract — the seat is resolved by the caller from
/// `PlayerSession`, never taken here as an id, so this function has no way
/// to answer for anyone but the viewer it was called for.
///
/// Static catalog strings need no escaping today, but title/text still cross
/// `html_escape` — the builder must not rely on the catalog staying tame
/// (the same argument `lc_hand_pane` makes for card titles).
pub fn lc_tab_panel(tab: Option<&TabDef>) -> String {
    match tab {
        Some(def) => {
            let (amount, unit) = match def.reward {
                TabReward::Hp(n) => (n, "HP"),
                TabReward::Pulls(n) => (n as i32, "PULLS"),
            };
            format!(
                r#"<section class="lc-tabcard" data-tab="{id}"><h2>YOUR TAB</h2><span class="lc-tabcard-name">{title}</span><p class="lc-tabcard-text">{text}</p><span class="lc-tabcard-pay">PAYS +{amount} {unit}</span></section>"#,
                id = html_escape(def.id),
                title = html_escape(def.title),
                text = html_escape(def.text),
            )
        }
        None => r#"<section class="lc-tabcard" data-tab-settled><h2>YOUR TAB</h2><p class="lc-tabcard-text">TAB SETTLED — a new one comes at the deal.</p></section>"#.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::last_call::{preview_state, Beat, LastCallState, PublicVessel};
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

        // Plan I Task 5 (H's lesson again): a content-bearing chips fixture
        // — `revealed_view` alone never populates `reactions`/`haunts`, so
        // the sweep below would only ever scan `centre_play` output that
        // never took the chips branch.
        let mut chips_view = revealed_view.clone();
        chips_view.reactions = vec![crate::last_call::ReactionPlay {
            card: card.clone(),
            source_seat: 1,
            answers: 1,
        }];
        chips_view.haunts = vec![crate::last_call::Haunt { seat: 2, play: 1 }];

        // Review fix (Important 2): `preview_state()`/`outcome_view` never
        // set `event`/`settled`, so the sweep below never scanned the new
        // strip markup at all — three more `lc_banner` variants that
        // actually take each of Task 4's three live branches (event chip,
        // collapsed multi-name settlement, and the review fix's game-over
        // settlement) so a future edit to the strip stays guarded here too.
        let mut event_view = ring_fixture(2);
        event_view.beat = Beat::Deal;
        event_view.event = Some("happy-hour".to_string());
        let mut settled_view = ring_fixture(2);
        settled_view.beat = Beat::Draw;
        settled_view.settled = vec!["alice".to_string(), "bob".to_string()];
        let mut outcome_settled_view = ring_fixture(2);
        outcome_settled_view.beat = Beat::Resolve;
        outcome_settled_view.beat_deadline_ms = None;
        outcome_settled_view.outcome = Some(LcOutcome::Winner(0));
        outcome_settled_view.event = Some("happy-hour".to_string());
        outcome_settled_view.settled = vec!["alice".to_string()];

        // Plan J Task 2: `lc_log`'s own content-bearing fixture. `view`
        // (from `preview_state()`) only ever pushes Round/Joined/Vessel
        // entries — nothing here locks a card, resolves a hit, or ends the
        // game — so a Play title's escaping, the Hit line's U+2212 minus,
        // and the GameOver em dash would otherwise never reach this sweep.
        let mut log_view = ring_fixture(2);
        log_view.log = vec![
            LogEntry::Play {
                seat: 0,
                title: "<b>x</b>".to_string(),
                target: Some(1),
            },
            LogEntry::Hit {
                source: 0,
                target: 1,
                amount: 3,
            },
            LogEntry::GameOver {
                winner: None,
                winner2: None,
            },
            // Task 1 erratum (Task 2 adjudication): the four social events —
            // otherwise this sweep never scans their `log_line` arms either.
            LogEntry::PactBreak {
                betrayer: 0,
                betrayed: 1,
            },
            LogEntry::TabSettle { seat: 0 },
            LogEntry::ReactionPlay {
                seat: 1,
                title: "<b>y</b>".to_string(),
            },
            LogEntry::Haunt {
                seat: 0,
                target: Some(1),
            },
            LogEntry::Haunt {
                seat: 1,
                target: None,
            },
        ];

        // Plan J Task 3: a content-bearing end-card fixture — `outcome_view`
        // alone never sets stats or `elim_order`, so the sweep below would
        // only ever scan `lc_end_card` output with every seat at its
        // `ring_fixture` defaults (HP, zero stats, no eliminated row) —
        // never the "OUT #n" fate branch, a populated stats line, or
        // `data-me`.
        let mut end_card_view = ring_fixture(3);
        end_card_view.beat = Beat::Resolve;
        end_card_view.beat_deadline_ms = None;
        end_card_view.outcome = Some(LcOutcome::Winner(0));
        end_card_view.seats[1].status = Status::Eliminated;
        end_card_view.seats[1].elim_order = Some(1);
        end_card_view.seats[0].damage_dealt = 7;
        end_card_view.seats[0].pulls_spent = 3;
        end_card_view.seats[0].cards_played = 2;

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
            lc_screen_panel(&chips_view),
            lc_banner(&outcome_view),
            lc_screen_panel(&outcome_view),
            lc_banner(&event_view),
            lc_banner(&settled_view),
            lc_banner(&outcome_settled_view),
            lc_log(&log_view),
            armed_column(&cards, false),
            cost_rail(&cards, 150, false),
            hand_wheel(&cards),
            hand_group(&HandGroupView {
                hand: &cards,
                armed: &cards[..1],
                locked: false,
                handicap_pct: 150,
                halved: false,
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
                ready: false,
                drawing: false,
                vessels: vec![(0, Deck::Beer), (1, Deck::Soft)],
                charged: 0,
                vessels_registered: 2,
                outcome: None,
                haunt_plays: Vec::new(),
                haunted: false,
            }),
            // Plan I Task 5: a ghost mid-Reveal with a live haunt target —
            // the new content-bearing branch (H's lesson: an empty-state
            // fixture proves nothing about the button markup it renders).
            lc_action_bar(&ActionBarView {
                beat: Beat::Reveal,
                round: 2,
                seated: true,
                alive: false,
                locked: false,
                ready: false,
                drawing: false,
                vessels: Vec::new(),
                charged: 0,
                vessels_registered: 2,
                outcome: None,
                haunt_plays: vec![(1, "PLAYER1 → PLAYER2".to_string())],
                haunted: false,
            }),
            // Plan H Task 5: both of `lc_tab_panel`'s content-bearing states
            // — a live tab and the settled placeholder — so the sweep
            // actually exercises the new builder's markup (Task 4's fix
            // round showed a strip-free fixture covers nothing).
            lc_tab_panel(Some(crate::lc_tabs::tab_def("lie-low").unwrap())),
            lc_tab_panel(None),
            // Plan J Task 3: the outcome branch's REMATCH/END NIGHT row —
            // the two `lc_action_bar` fixtures above are both `outcome:
            // None`, so this is the sweep's only exercise of the game-over
            // row's markup.
            lc_action_bar(&ActionBarView {
                beat: Beat::Resolve,
                round: 3,
                seated: true,
                alive: true,
                locked: false,
                ready: false,
                drawing: false,
                vessels: Vec::new(),
                charged: 0,
                vessels_registered: 2,
                outcome: Some(LcOutcome::Winner(0)),
                haunt_plays: Vec::new(),
                haunted: false,
            }),
            // Plan J Task 3: the end card itself, me = the eliminated seat
            // (data-me AND the "OUT #n" fate branch in the same output).
            lc_end_card(&end_card_view, Some(1)),
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
            ready: false,
            drawing: false,
            draws: 0,
            damage_dealt: 0,
            pulls_spent: 0,
            cards_played: 0,
            elim_order: None,
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

    /// Plan J, Task 5 (J11): `deck_list_row`'s states mirror `deck_stack`'s,
    /// plus the rail swap — the big screen carries rows, not stacks, while
    /// every `deck-{slug}` anchor still resolves exactly once.
    #[test]
    fn test_deck_list_row_states_and_screen_rail_swap() {
        let low = deck_list_row(Deck::Wine, 4, 16);
        assert!(low.contains(r#"data-deck="wine""#));
        assert!(low.contains(r#"data-count="4""#));
        assert!(low.contains(" data-low"));
        assert!(!low.contains("data-empty"));
        assert!(low.contains("disc 16"));
        assert!(low.contains(r#"data-flight-anchor="deck-wine""#));
        no_hex(&low);

        let empty = deck_list_row(Deck::Wine, 0, 16);
        assert!(empty.contains("data-empty"));

        let plenty = deck_list_row(Deck::Wine, 21, 0);
        assert!(!plenty.contains("data-low"));
        assert!(!plenty.contains("data-empty"));
        assert!(!plenty.contains(r#"data-low="false""#));
        assert!(!plenty.contains(r#"data-empty="false""#));

        let html = lc_screen_panel(&ring_fixture(5));
        for deck in Deck::ALL {
            assert!(html.contains(&format!(r#"data-deck="{}""#, deck.slug())));
        }
        assert_eq!(
            html.matches(r#"class="lc-deckrow""#).count(),
            Deck::ALL.len()
        );
        assert!(html.contains("lc-discard"));
        // `discard_slot` legitimately reuses `.lc-deckstack-count`/`-name`
        // for its own spans (shared styling), so the rail-swap check looks
        // for `deck_stack`'s own root class specifically, not the bare
        // substring "lc-deckstack".
        assert!(!html.contains(r#"class="lc-deckstack lc-deck-"#));
        for deck in Deck::ALL {
            assert_eq!(
                html.matches(&format!(r#"data-flight-anchor="deck-{}""#, deck.slug()))
                    .count(),
                1,
                "deck-{} anchor should resolve exactly once",
                deck.slug()
            );
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
        for cls in ["is-locked", "is-ready", "is-drawing", "is-hit", "is-eliminated"] {
            assert!(!idle.contains(cls), "idle should not contain {cls}");
        }
        assert!(!idle.contains("lc-lock-tick"));

        seat.ready = true;
        let ready = player_plaque(&seat);
        assert!(ready.contains("is-ready"));
        assert!(ready.contains("lc-lock-tick")); // shared tick glyph
        seat.ready = false;

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
            ready: false,
            drawing: false,
            draws: 3,
            damage_dealt: 0,
            pulls_spent: 0,
            cards_played: 0,
            elim_order: None,
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
    fn test_the_banner_strip_shows_the_event_or_the_settlements_never_both() {
        // event Some, settled non-empty, beat Lock: the event outranks the
        // settlement announcement outright — one occupant, first match wins.
        let mut view = ring_fixture(2);
        view.beat = Beat::Lock;
        view.event = Some("happy-hour".to_string());
        view.settled = vec!["alice".to_string()];
        let html = lc_banner(&view);
        assert!(html.contains(r#"data-event="happy-hour""#));
        assert!(html.contains("HAPPY HOUR"));
        assert!(html.contains("Every card costs half its pulls this round, rounded up."));
        assert!(!html.contains("SETTLED A TAB"));
        assert_eq!(html.matches(r#"class="lc-event""#).count(), 1);
        no_hex(&html);

        // event None, settled two names, beat Draw: review fix (Important
        // 1) — the screen's fixed-88px header has no room for a second
        // wrapped row, so multiple settles collapse into ONE row: the
        // first name in full, the rest as a `+{n}` count.
        let mut view = ring_fixture(2);
        view.beat = Beat::Draw;
        view.settled = vec!["alice".to_string(), "bob".to_string()];
        let html = lc_banner(&view);
        assert!(html.contains("ALICE SETTLED A TAB"), "{html}");
        assert!(!html.contains("BOB SETTLED A TAB"), "{html}"); // collapsed, not a second name
        assert!(html.contains("+1"), "{html}");
        assert!(!html.contains("data-event"));
        assert_eq!(html.matches(r#"class="lc-event""#).count(), 1, "{html}");
        no_hex(&html);

        // Three settles: still exactly one row, count grows to +2.
        let mut view = ring_fixture(2);
        view.beat = Beat::Draw;
        view.settled = vec!["alice".to_string(), "bob".to_string(), "cara".to_string()];
        let html = lc_banner(&view);
        assert!(html.contains("ALICE SETTLED A TAB"), "{html}");
        assert!(html.contains("+2"), "{html}");
        assert_eq!(html.matches(r#"class="lc-event""#).count(), 1, "{html}");
        no_hex(&html);

        // event None, settled non-empty, beat Lock: announcements are a
        // Draw-beat thing (H11) — no strip at all outside Draw.
        let mut view = ring_fixture(2);
        view.beat = Beat::Lock;
        view.settled = vec!["alice".to_string()];
        let html = lc_banner(&view);
        assert!(!html.contains(r#"class="lc-event""#));
        no_hex(&html);

        // event Some, outcome Some, settled empty: game over owns the
        // banner (H6/E13) — no strip at all when there is nothing to
        // announce.
        let mut view = ring_fixture(2);
        view.event = Some("happy-hour".to_string());
        view.outcome = Some(LcOutcome::Winner(0));
        let html = lc_banner(&view);
        assert!(!html.contains(r#"class="lc-event""#));
        no_hex(&html);

        // Review fix (⚠️ 1 / erratum a2e66ab): outcome Some AND settled
        // non-empty — the event still never renders (game over owns that
        // chip), but a final-round settle is not lost: the name still
        // appears on the frozen tableau, or the erratum's whole reason for
        // existing (a settle in the game-ending round) would be
        // unreachable on every surface.
        let mut view = ring_fixture(2);
        view.event = Some("happy-hour".to_string());
        view.outcome = Some(LcOutcome::Winner(0));
        view.settled = vec!["alice".to_string()];
        let html = lc_banner(&view);
        assert!(html.contains("GAME OVER"), "{html}");
        assert!(html.contains("ALICE SETTLED A TAB"), "{html}");
        assert!(!html.contains(r#"data-event"#), "{html}"); // no event, ever, on game over
        assert!(!html.contains("HAPPY HOUR"), "{html}");
        assert_eq!(html.matches(r#"class="lc-event""#).count(), 1, "{html}");
        no_hex(&html);

        // an id `event_def` does not recognise renders nothing (H3's
        // fail-soft, applied at the display layer too).
        let mut view = ring_fixture(2);
        view.beat = Beat::Lock;
        view.event = Some("closing-time".to_string());
        let html = lc_banner(&view);
        assert!(!html.contains(r#"class="lc-event""#));
        no_hex(&html);
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
            halved: false,
        }
    }

    #[test]
    fn test_lc_hand_pane_satisfies_the_contract() {
        let hand = lc_cards::deck_cards(Deck::Beer);
        let armed = [hand[0].clone()];
        let rows = [setup_row(1, "alice", 100, &[Deck::Beer])];
        let view = hg(&hand, &armed);
        let html = lc_hand_pane("", "QK4M", 1, &view, &rows, 7, false);
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
        let html = lc_hand_pane("/drinks", "QK4M", 1, &view, &rows, 1, false);
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
        let html = lc_hand_pane("", "QK4M", 2, &view, &rows, 1, false);
        assert_eq!(html.matches(">SET<").count(), 3);
        assert_eq!(html.matches("(you)").count(), 1);
        assert!(html.contains("bob (you)"));
    }

    #[test]
    fn test_lc_hand_pane_empty_hand() {
        let rows = [setup_row(1, "alice", 100, &[])];
        let view = hg(&[], &[]);
        let html = lc_hand_pane("", "QK4M", 1, &view, &rows, 0, false);
        assert!(html.contains("lc-empty"));
        assert!(html.contains("Register your drink to be dealt a hand."));
        assert!(!html.contains("lc-cardface"));
        assert!(!html.contains("lc-wheel"));
        assert!(html.contains(r#"data-count="0""#));
    }

    /// Plan J Task 4 / E1: the lobby is round-1 `Beat::Draw` with no
    /// outcome — "ready" is having a vessel, on both the felt (`{k}/{n}`,
    /// names of the unregistered) and the phone (`data-ready` rows, the
    /// same waiting/all-set line). Both surfaces go quiet the moment the
    /// gate no longer holds, whether because the round moved on or the game
    /// ended.
    #[test]
    fn test_the_lobby_says_who_it_waits_on() {
        let mut view = ring_fixture(3);
        view.seats[0].vessels = vec![PublicVessel {
            deck: Deck::Beer,
            pulls_left: 1,
            pulls_max: 1,
        }];

        let one_of_three = lc_screen_panel(&view);
        assert!(one_of_three.contains("lc-centre-lobby"));
        assert!(one_of_three.contains("1 / 3 DRINKS IN"));
        assert!(one_of_three.contains("WAITING ON "));
        assert!(one_of_three.contains("PLAYER2"));
        assert!(one_of_three.contains("PLAYER3"));
        no_hex(&one_of_three);

        let mut all_vesselled = view.clone();
        for seat in &mut all_vesselled.seats {
            seat.vessels = vec![PublicVessel {
                deck: Deck::Beer,
                pulls_left: 1,
                pulls_max: 1,
            }];
        }
        let all_html = lc_screen_panel(&all_vesselled);
        assert!(all_html.contains("3 / 3 DRINKS IN"));
        assert!(!all_html.contains("lc-lobby-names"));
        no_hex(&all_html);

        let mut round_two = view.clone();
        round_two.round = 2;
        assert!(!lc_screen_panel(&round_two).contains("lc-centre-lobby"));

        let mut decided = view.clone();
        decided.outcome = Some(LcOutcome::Winner(0));
        assert!(!lc_screen_panel(&decided).contains("lc-centre-lobby"));

        // The phone half — same states, `setup_rows`-shaped input.
        let rows = [
            setup_row(1, "player1", 100, &[Deck::Beer]),
            setup_row(2, "player2", 100, &[]),
            setup_row(3, "player3", 100, &[]),
        ];
        let hg_view = hg(&[], &[]);
        let waiting_html = lc_hand_pane("", "QK4M", 1, &hg_view, &rows, 1, true);
        assert_eq!(waiting_html.matches("data-ready").count(), 1);
        assert!(waiting_html.contains(r#"<form class="lc-setup-row" data-ready"#));
        assert!(waiting_html.contains(r#"data-waiting="2">WAITING ON 2: "#));
        assert!(waiting_html.contains("PLAYER2"));
        assert!(waiting_html.contains("PLAYER3"));
        no_hex(&waiting_html);

        let all_rows = [
            setup_row(1, "player1", 100, &[Deck::Beer]),
            setup_row(2, "player2", 100, &[Deck::Wine]),
            setup_row(3, "player3", 100, &[Deck::Soft]),
        ];
        let all_ready_html = lc_hand_pane("", "QK4M", 1, &hg_view, &all_rows, 1, true);
        assert_eq!(all_ready_html.matches("data-ready").count(), 3);
        assert!(all_ready_html.contains(r#"data-waiting="0">ALL SET — PRESS START"#));

        // Round 2: no wait line at all, though `data-ready` is unconditional.
        let round_two_html = lc_hand_pane("", "QK4M", 1, &hg_view, &rows, 1, false);
        assert!(!round_two_html.contains("lc-lobby-wait"));
        assert_eq!(round_two_html.matches("data-ready").count(), 1);
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
                ready: false,
                drawing: false,
                draws: 0,
                damage_dealt: 0,
                pulls_spent: 0,
                cards_played: 0,
                elim_order: None,
            })
            .collect();
        PublicView {
            seats,
            round: 1,
            beat: Beat::Draw,
            first_seat: 0,
            deck_counts: Deck::ALL.iter().map(|&d| (d, 0)).collect(),
            discard_count: 0,
            discard_counts: Deck::ALL.iter().map(|&d| (d, 0)).collect(),
            revealed: Vec::new(),
            seq: 0,
            outcome: None,
            beat_deadline_ms: None,
            pact_breaks: vec![],
            event: None,
            settled: Vec::new(),
            reactions: Vec::new(),
            haunts: Vec::new(),
            log: Vec::new(),
        }
    }

    /// Plan J, Task 2: every copy-table row from the brief, newest first,
    /// escaped, with the empty-log fallback and the `SEAT {n+1}` fallback
    /// (M1, fix wave: this test body pushes `Joined { seat: 5 }` — a seat
    /// absent from this two-seat fixture's `view.seats` — and asserts
    /// `log_seat_name`'s `unwrap_or_else` arm renders "SEAT 6 TAKES A SEAT"
    /// directly below, rather than relying on coverage from elsewhere).
    /// Keep the variant list here and
    /// `test_log_tag_matches_the_serde_tag_for_every_variant`'s list in
    /// sync — a variant this test's log doesn't exercise is untested for
    /// copy, one the other test doesn't build is unpinned for its `data-t`
    /// tag; a new variant needs an entry in both.
    #[test]
    fn test_lc_log_renders_the_copy() {
        let mut view = ring_fixture(2);
        view.seats[0].name = "alice".to_string();
        view.seats[1].name = "bob".to_string();

        let empty = lc_log(&view);
        assert!(empty.contains("lc-empty"), "{empty}");
        assert!(empty.contains(r#"data-count="0""#), "{empty}");
        no_hex(&empty);

        view.log = vec![
            LogEntry::Round { round: 2 },
            LogEntry::Joined { seat: 0 },
            LogEntry::Vessel {
                seat: 0,
                deck: Deck::Beer,
            },
            LogEntry::Handicap { seat: 0, pct: 80 },
            LogEntry::Draw {
                seat: 0,
                deck: Deck::Beer,
                n: 2,
            },
            LogEntry::Lock { seat: 0 },
            LogEntry::Play {
                seat: 0,
                title: "<b>x</b>".to_string(),
                target: Some(1),
            },
            LogEntry::Play {
                seat: 1,
                title: "Nudge".to_string(),
                target: None,
            },
            LogEntry::Hit {
                source: 0,
                target: 1,
                amount: 3,
            },
            LogEntry::Heal { seat: 0, amount: 2 },
            LogEntry::Shield { seat: 0, amount: 4 },
            LogEntry::Drain {
                source: 0,
                target: 1,
                amount: 5,
            },
            LogEntry::Fizzle {
                seat: 0,
                title: "Whiff".to_string(),
            },
            LogEntry::Eliminated { seat: 1 },
            LogEntry::Reshuffle { deck: Deck::Cider },
            LogEntry::GameOver {
                winner: Some(0),
                winner2: None,
            },
            // Fix wave (Important 1): the pact-win shape — both winners,
            // "THE PACT HOLDS" copy, matching the end card/banner.
            LogEntry::GameOver {
                winner: Some(0),
                winner2: Some(1),
            },
            LogEntry::GameOver {
                winner: None,
                winner2: None,
            },
            // The `SEAT {n+1}` fallback (brief's Produces block): seat 5
            // doesn't exist in this two-seat fixture's `view.seats`.
            LogEntry::Joined { seat: 5 },
            // Task 1 erratum (Task 2 adjudication): the four social events.
            LogEntry::PactBreak {
                betrayer: 0,
                betrayed: 1,
            },
            LogEntry::TabSettle { seat: 1 },
            LogEntry::ReactionPlay {
                seat: 0,
                title: "<i>y</i>".to_string(),
            },
            LogEntry::Haunt {
                seat: 1,
                target: Some(0),
            },
            LogEntry::Haunt {
                seat: 0,
                target: None,
            },
        ];

        let html = lc_log(&view);
        no_hex(&html);
        assert!(html.contains(r#"data-count="24""#), "{html}");
        assert!(html.contains(r#"data-t="round""#), "{html}");
        // Review Minor 2: the three styled tags (`lastcall.css:560-561`) —
        // asserted individually so a typo in any `log_tag` arm loses the
        // emphasis styling loudly, not silently under a green suite.
        assert!(html.contains(r#"data-t="hit""#), "{html}");
        assert!(html.contains(r#"data-t="eliminated""#), "{html}");
        assert!(html.contains(r#"data-t="game_over""#), "{html}");
        assert!(html.contains(r#"data-t="pact_break""#), "{html}");
        assert!(html.contains(r#"data-t="tab_settle""#), "{html}");
        assert!(html.contains(r#"data-t="reaction_play""#), "{html}");
        assert!(html.contains(r#"data-t="haunt""#), "{html}");

        // oldest -> newest push order; rendering is newest-first, so the
        // find() position of each line must strictly DECREASE down this list.
        let lines = [
            "— ROUND 2 —",
            "ALICE TAKES A SEAT",
            "ALICE REGISTERS BEER",
            "ALICE HANDICAP 80%",
            "ALICE FINISHES A BEER · +2",
            "ALICE LOCKS IN",
            "ALICE PLAYS &lt;b&gt;x&lt;/b&gt; → BOB",
            "BOB PLAYS Nudge",
            "ALICE HITS BOB −3",
            "ALICE +2 HP",
            "ALICE SHIELDS 4",
            "ALICE DRAINS BOB −5 PULLS",
            "Whiff FIZZLES",
            "BOB IS OUT",
            "CIDER RESHUFFLES",
            "GAME OVER — ALICE OUTLASTS THE TABLE",
            "GAME OVER — ALICE & BOB — THE PACT HOLDS",
            "GAME OVER — EVERYBODY'S OUT",
            "SEAT 6 TAKES A SEAT",
            "ALICE BROKE THEIR PACT WITH BOB",
            "BOB SETTLED A TAB",
            "ALICE ANSWERS WITH &lt;i&gt;y&lt;/i&gt;",
            "BOB HAUNTS ALICE",
            "ALICE HAUNTS THE TABLE",
        ];
        let positions: Vec<usize> = lines
            .iter()
            .map(|l| {
                html.find(l)
                    .unwrap_or_else(|| panic!("missing line: {l} in {html}"))
            })
            .collect();
        for w in positions.windows(2) {
            assert!(w[0] > w[1], "not newest-first: {positions:?} for {lines:?}");
        }
    }

    /// Review Minor 3 (fix wave): `log_tag` hand-mirrors
    /// `LogEntry`'s `#[serde(tag = "t", rename_all = "snake_case")]` with
    /// nothing pinning the two together — a variant rename could silently
    /// drift the `data-t` DOM contract out of sync with the wire tag. One
    /// instance of every variant, round-tripped through `serde_json` and
    /// compared against `log_tag`'s own answer. Keep this list in sync with
    /// `test_lc_log_renders_the_copy`'s `view.log` — see its doc comment.
    #[test]
    fn test_log_tag_matches_the_serde_tag_for_every_variant() {
        let entries = vec![
            LogEntry::Round { round: 1 },
            LogEntry::Joined { seat: 0 },
            LogEntry::Vessel {
                seat: 0,
                deck: Deck::Beer,
            },
            LogEntry::Handicap { seat: 0, pct: 50 },
            LogEntry::Draw {
                seat: 0,
                deck: Deck::Beer,
                n: 1,
            },
            LogEntry::Lock { seat: 0 },
            LogEntry::Play {
                seat: 0,
                title: "x".to_string(),
                target: None,
            },
            LogEntry::Hit {
                source: 0,
                target: 1,
                amount: 1,
            },
            LogEntry::Heal { seat: 0, amount: 1 },
            LogEntry::Shield { seat: 0, amount: 1 },
            LogEntry::Drain {
                source: 0,
                target: 1,
                amount: 1,
            },
            LogEntry::Fizzle {
                seat: 0,
                title: "x".to_string(),
            },
            LogEntry::Eliminated { seat: 0 },
            LogEntry::Reshuffle { deck: Deck::Beer },
            LogEntry::GameOver {
                winner: Some(0),
                winner2: None,
            },
            LogEntry::GameOver {
                winner: Some(0),
                winner2: Some(1),
            },
            LogEntry::PactBreak {
                betrayer: 0,
                betrayed: 1,
            },
            LogEntry::TabSettle { seat: 0 },
            LogEntry::ReactionPlay {
                seat: 0,
                title: "x".to_string(),
            },
            LogEntry::Haunt {
                seat: 0,
                target: Some(1),
            },
            LogEntry::Haunt {
                seat: 0,
                target: None,
            },
        ];
        for entry in &entries {
            let value = serde_json::to_value(entry).unwrap();
            let wire_tag = value["t"].as_str().unwrap();
            assert_eq!(
                wire_tag,
                log_tag(entry),
                "log_tag drifted from the serde tag for {entry:?}"
            );
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
        // Plan J Task 4: `revealed` only ever populates at Reveal/Resolve in
        // production (this test's own comment above) — round-1 Draw with no
        // outcome is now the lobby gate, so the fixture has to leave that
        // state or it exercises the lobby centre instead of this one.
        view.beat = Beat::Reveal;
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

    /// Plan I Task 5: reaction/haunt chips ride the play they answer, not
    /// the felt in general — a second, unridden play in the same reveal
    /// renders no chips block of its own.
    #[test]
    fn test_centre_chips_ride_their_play() {
        let mut view = ring_fixture(3);
        // Plan J Task 4: same reasoning as the test above — `revealed` is a
        // Reveal/Resolve-only field in production, and round-1 Draw is now
        // the lobby gate.
        view.beat = Beat::Reveal;
        let atk = centre_card("wine-atk", Deck::Wine, "one", "Table Play");
        view.revealed = vec![
            Play {
                card: atk.clone(),
                source_seat: 0,
                target: Some(1),
                paid_from: Deck::Wine,
                order_key: 1,
            },
            Play {
                card: atk.clone(),
                source_seat: 1,
                target: Some(2),
                paid_from: Deck::Wine,
                order_key: 2,
            },
        ];
        let reaction_card = centre_card("cider-08", Deck::Cider, "one", "Not So Fast, Friend");
        view.reactions = vec![crate::last_call::ReactionPlay {
            card: reaction_card,
            source_seat: 2,
            answers: 1,
        }];
        view.haunts = vec![crate::last_call::Haunt { seat: 2, play: 1 }];

        let html = lc_screen_panel(&view);
        // ring_fixture names seats "player{n+1}" — seat 2 -> PLAYER3.
        assert!(html.contains("lc-chip-react"), "{html}");
        assert!(html.contains("PLAYER3: Not So Fast, Friend"), "{html}");
        assert!(html.contains("lc-chip-haunt"), "{html}");
        assert!(html.contains("PLAYER3 +1"), "{html}");
        // exactly one chips block: play 1 has riders, play 2 has none.
        assert_eq!(html.matches("lc-centre-chips").count(), 1, "{html}");
        no_hex(&html);
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
        // Plan J Task 3: the broadcast standings board rides beneath the
        // victory line — no data-me (no single viewer on this surface), the
        // Winner(seat) row alone carries data-winner, and 4 seats fit under
        // the cap with no "MORE" tail.
        assert!(screen.contains(r#"<ol class="lc-standings">"#));
        assert!(screen.contains(r#"data-seat="1" data-winner"#));
        assert!(!screen.contains("data-me"));
        assert!(!screen.contains("lc-standings-more"));
        no_hex(&banner);
        no_hex(&screen);

        view.outcome = Some(LcOutcome::Draw);
        let screen_draw = lc_screen_panel(&view);
        assert!(screen_draw.contains("EVERYBODY'S OUT"));
        assert!(!screen_draw.contains("lc-centre-plays"));
        assert!(!screen_draw.contains("data-winner")); // nobody to mark

        // G2/Task 2: the pact win, banner unchanged, names uppercased.
        view.outcome = Some(LcOutcome::Pact(0, 2));
        let screen_pact = lc_screen_panel(&view);
        assert!(screen_pact.contains("PLAYER1 & PLAYER3 — THE PACT HOLDS"));
        assert!(!screen_pact.contains("lc-centre-plays"));
        assert!(!screen_pact.contains("data-winner")); // a pact names two winners, marks neither row
        let banner_pact = lc_banner(&view);
        assert!(banner_pact.contains("GAME OVER"));
        no_hex(&screen_pact);
        no_hex(&banner_pact);

        // Plan J Task 3: a fifth seat pushes the broadcast board past its
        // 4-row cap.
        let mut five = ring_fixture(5);
        five.beat = Beat::Resolve;
        five.beat_deadline_ms = None;
        five.outcome = Some(LcOutcome::Winner(0));
        let screen_five = lc_screen_panel(&five);
        assert!(screen_five.contains(r#"<span class="lc-standings-more">+1 MORE</span>"#));
        assert_eq!(screen_five.matches("lc-standing\"").count(), 4);
        no_hex(&screen_five);
    }

    // -------------------------------------------------------------
    // Plan J Task 3 — the end-of-game screen: final_standings and lc_end_card.
    // -------------------------------------------------------------

    /// J6: alive by hp desc, then eliminated by elim_order desc; ties by
    /// lower seat.
    #[test]
    fn test_final_standings_order() {
        let mut view = ring_fixture(4);
        view.seats[0].hp = 9; // a
        view.seats[1].hp = 12; // b
        view.seats[2].status = Status::Eliminated; // c
        view.seats[2].elim_order = Some(1);
        view.seats[3].status = Status::Eliminated; // d
        view.seats[3].elim_order = Some(2);

        let order: Vec<usize> = final_standings(&view).iter().map(|s| s.seat).collect();
        assert_eq!(order, vec![1, 0, 3, 2]); // b, a, d, c

        // Draw case: all eliminated, orders 1..4 -> reverse elim order.
        let mut draw = ring_fixture(4);
        for (i, seat) in draw.seats.iter_mut().enumerate() {
            seat.status = Status::Eliminated;
            seat.elim_order = Some(i as u32 + 1);
        }
        let draw_order: Vec<usize> = final_standings(&draw).iter().map(|s| s.seat).collect();
        assert_eq!(draw_order, vec![3, 2, 1, 0]);

        // Tie: two alive at hp 9 -> lower seat first.
        let mut tie = ring_fixture(4);
        tie.seats[0].hp = 9;
        tie.seats[2].hp = 9;
        let tie_order: Vec<usize> = final_standings(&tie).iter().map(|s| s.seat).collect();
        // seats 0 and 2 (both hp 9) must appear in seat order, ahead of
        // whatever untouched hp-15 seats sort as (1, 3 at hp 15 outrank 9).
        assert_eq!(tie_order, vec![1, 3, 0, 2]);
    }

    #[test]
    fn test_end_card_shows_standings_and_stats() {
        let mut view = ring_fixture(2);
        view.beat = Beat::Resolve;
        view.beat_deadline_ms = None;
        view.outcome = Some(LcOutcome::Winner(1));
        view.seats[0].status = Status::Eliminated;
        view.seats[0].elim_order = Some(1);
        view.seats[1].damage_dealt = 11;
        view.seats[1].pulls_spent = 4;
        view.seats[1].cards_played = 3;

        let html = lc_end_card(&view, Some(0));
        assert!(html.contains("GAME OVER"));
        assert!(html.contains("PLAYER2 OUTLASTS THE TABLE"));
        assert!(html.contains(r#"data-seat="1" data-winner"#)); // b's row
        assert!(html.contains(r#"data-seat="0" data-me"#)); // me = seat 0
        assert!(html.contains("DMG 11"));
        assert!(html.contains("PULLS 4"));
        assert!(html.contains("CARDS 3"));
        assert!(html.contains("OUT #1"));
        no_hex(&html);
    }

    /// Fix wave (M4): an Eliminated seat with no `elim_order` — only
    /// reachable via a hand-built or corrupt blob, never the engine itself
    /// (`elim_order` is always set at the same site status flips) — renders
    /// a bare "OUT", never the nonsense ordinal "OUT #0".
    #[test]
    fn test_end_card_out_fate_falls_back_without_a_number_when_elim_order_is_missing() {
        let mut view = ring_fixture(2);
        view.beat = Beat::Resolve;
        view.beat_deadline_ms = None;
        view.outcome = Some(LcOutcome::Winner(1));
        view.seats[0].status = Status::Eliminated;
        view.seats[0].elim_order = None;

        let html = lc_end_card(&view, None);
        assert!(html.contains("OUT</span>"), "{html}");
        assert!(!html.contains("OUT #"), "{html}");
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
            let html = cost_rail(&hand, pct, false);
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
    fn test_cost_rail_halved_matches_effective_pull_cost() {
        // I1 (Plan H review): under a cost-halving event, the rail's
        // per-card prices must price through the same seam as the engine
        // charge — `pull_cost` further halved (rounded up) — not the raw
        // unhalved `pull_cost` the pre-fix builder used.
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1), // pull_cost 1 -> halved 1
            rail_card("wine-fx", Deck::Wine, 2), // pull_cost 2 -> halved 1
            rail_card("liquor-fx", Deck::Liquor, 3), // pull_cost 3 -> halved 2
        ];
        let html = cost_rail(&hand, 100, true);
        let expected_costs = [1u8, 1, 2];
        for (i, pc) in expected_costs.iter().enumerate() {
            assert!(
                html.contains(&format!(
                    r#"data-idx="{i}" data-card-id="{}" data-cost="{}" data-pull-cost="{pc}""#,
                    hand[i].id, hand[i].cost
                )),
                "idx={i} missing halved data-pull-cost={pc}: {html}"
            );
        }
        assert_eq!(html.matches(r#"class="lc-costrail-bar""#).count(), 4); // 1+1+2
        no_hex(&html);
    }

    #[test]
    fn test_cost_rail_marks_first_group_active_and_survives_empty() {
        let hand = [
            rail_card("beer-fx", Deck::Beer, 1),
            rail_card("wine-fx", Deck::Wine, 2),
            rail_card("liquor-fx", Deck::Liquor, 3),
        ];
        let html = cost_rail(&hand, 100, false);
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
        let html0 = cost_rail(&empty, 100, false);
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
        let html = cost_rail(&hand, 100, false);
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
                ready: false,
                drawing: false,
                vessels: Vec::new(),
                charged: 0,
                vessels_registered: 0,
                outcome: None,
                haunt_plays: Vec::new(),
                haunted: false,
            }
        }

        // outcome wins over beat, from any beat. Plan J Task 3 / J8:
        // REMATCH replaces the old solitary END GAME button; END NIGHT
        // (still `data-lc-post="end"`) is the exit.
        let mut ab = base();
        ab.beat = Beat::Lock;
        ab.outcome = Some(LcOutcome::Winner(0));
        let html = lc_action_bar(&ab);
        assert!(html.contains("REMATCH"));
        assert!(html.contains(r#"data-lc-post="rematch""#));
        assert!(html.contains("END NIGHT"));
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

        // draw, round >= 2, not drawing: per-vessel buttons + the READY
        // tap (the sit-tight action since the clock's removal).
        let mut ab = base();
        ab.round = 2;
        ab.vessels = vec![(0, Deck::Beer), (1, Deck::Soft)];
        let html = lc_action_bar(&ab);
        assert!(html.contains(r#"data-vessel="0">FINISH BEER · DRAW"#));
        assert!(html.contains(r#"data-vessel="1">FINISH SOFT · DRAW"#));
        assert!(html.contains(r#"data-lc-post="ready">READY"#));
        no_hex(&html);

        // draw, round >= 2, drawing: the dealt hint, no draw buttons — but
        // still the READY tap (a fresh hand doesn't advance the table).
        let mut ab = base();
        ab.round = 2;
        ab.drawing = true;
        let html = lc_action_bar(&ab);
        assert!(html.contains("FRESH VESSEL — DEALT"));
        assert!(!html.contains("data-lc-post=\"draw\""));
        assert!(html.contains(r#"data-lc-post="ready">READY"#));
        no_hex(&html);

        // draw, round >= 2, already ready: waiting hint, no second tap.
        let mut ab = base();
        ab.round = 2;
        ab.ready = true;
        let html = lc_action_bar(&ab);
        assert!(html.contains("READY — WAITING FOR THE TABLE"));
        assert!(!html.contains(r#"data-lc-post="ready""#));
        no_hex(&html);

        // deal: the auto-beat hint.
        let mut ab = base();
        ab.beat = Beat::Deal;
        let html = lc_action_bar(&ab);
        assert!(html.contains("DEALING…"));
        no_hex(&html);

        // diplomacy: the talk-it-out hint + the READY tap.
        let mut ab = base();
        ab.beat = Beat::Diplomacy;
        let html = lc_action_bar(&ab);
        assert!(html.contains("TALK IT OUT — DEALS AREN'T BINDING"));
        assert!(html.contains(r#"data-lc-post="ready">READY"#));
        no_hex(&html);

        // diplomacy, ready: the waiting hint replaces the tap.
        let mut ab = base();
        ab.beat = Beat::Diplomacy;
        ab.ready = true;
        let html = lc_action_bar(&ab);
        assert!(html.contains("READY — WAITING FOR THE TABLE"));
        assert!(!html.contains(r#"data-lc-post="ready""#));
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

        // reveal, nothing charged: NOTHING TO PAY, plus the READY tap —
        // Reveal is an open beat now, the response window closes when the
        // whole table taps.
        let mut ab = base();
        ab.beat = Beat::Reveal;
        let html = lc_action_bar(&ab);
        assert!(html.contains("NOTHING TO PAY"));
        assert!(html.contains(r#"data-lc-post="ready">READY"#));
        no_hex(&html);

        // resolve mirrors reveal's pay row but never offers READY (auto
        // beat — only the frozen tableau ever renders it).
        let mut ab = base();
        ab.beat = Beat::Resolve;
        ab.charged = 1;
        let html = lc_action_bar(&ab);
        assert!(html.contains("DRINK 1"));
        assert!(!html.contains(r#"data-lc-post="ready""#));
        no_hex(&html);
    }

    /// Plan I Task 5 / DDv2 §9.2: the ghost's three-row action bar. Plan
    /// E's original `!alive` row (`YOU'RE OUT — HAUNT THE TABLE`) stays
    /// true outside the window (any beat but Reveal), and an *alive*
    /// viewer never sees a haunt button no matter what `haunt_plays` says —
    /// `ActionBarView` is trusted input here (the route never fills
    /// `haunt_plays` for a living player), but the renderer's own branch
    /// order is what this test pins.
    #[test]
    fn test_ghost_bar_haunts_only_in_the_window() {
        fn ab(
            alive: bool,
            beat: Beat,
            haunt_plays: Vec<(u32, String)>,
            haunted: bool,
        ) -> ActionBarView {
            ActionBarView {
                beat,
                round: 1,
                seated: true,
                alive,
                locked: false,
                ready: false,
                drawing: false,
                vessels: Vec::new(),
                charged: 0,
                vessels_registered: 2,
                outcome: None,
                haunt_plays,
                haunted,
            }
        }

        // ghost, mid-Reveal, not yet voted, a Damage play in flight: HAUNT.
        let html = lc_action_bar(&ab(
            false,
            Beat::Reveal,
            vec![(1, "ALICE → BOB".to_string())],
            false,
        ));
        assert!(
            html.contains(r#"data-lc-post="haunt" data-play="1""#),
            "{html}"
        );
        assert!(html.contains("HAUNT ALICE → BOB +1"), "{html}");
        no_hex(&html);

        // ghost, mid-Reveal, already voted: the curse-cast hint, no button.
        let html = lc_action_bar(&ab(
            false,
            Beat::Reveal,
            vec![(1, "ALICE → BOB".to_string())],
            true,
        ));
        assert!(html.contains("YOUR CURSE IS CAST"), "{html}");
        assert!(!html.contains(r#"data-lc-post="haunt""#), "{html}");
        no_hex(&html);

        // ghost, outside the window (Lock): Plan E's original row, unchanged.
        let html = lc_action_bar(&ab(
            false,
            Beat::Lock,
            vec![(1, "ALICE → BOB".to_string())],
            false,
        ));
        assert!(html.contains("YOU'RE OUT — HAUNT THE TABLE"), "{html}");
        assert!(!html.contains(r#"data-lc-post="haunt""#), "{html}");
        no_hex(&html);

        // alive, mid-Reveal: no haunt button ever, regardless of haunt_plays.
        let html = lc_action_bar(&ab(
            true,
            Beat::Reveal,
            vec![(1, "ALICE → BOB".to_string())],
            false,
        ));
        assert!(!html.contains(r#"data-lc-post="haunt""#), "{html}");
        no_hex(&html);
    }

    #[test]
    fn test_the_tab_card_states_its_deal() {
        // Plan H Task 5: the three shapes `lc_tab_panel` can take — a live
        // HP-reward tab, a live pulls-reward tab (both reward units, per the
        // brief), and the settled placeholder.
        let lie_low = crate::lc_tabs::tab_def("lie-low").unwrap();
        let html = lc_tab_panel(Some(lie_low));
        assert!(html.contains(r#"data-tab="lie-low""#));
        assert!(html.contains("YOUR TAB"));
        assert!(html.contains("LIE LOW"));
        assert!(html.contains(lie_low.text));
        assert!(html.contains("PAYS +2 HP"));
        no_hex(&html);

        let showboat = crate::lc_tabs::tab_def("showboat").unwrap();
        let html = lc_tab_panel(Some(showboat));
        assert!(html.contains(r#"data-tab="showboat""#));
        assert!(html.contains("PAYS +2 PULLS"));
        no_hex(&html);

        let html = lc_tab_panel(None);
        assert!(html.contains("data-tab-settled"));
        assert!(html.contains("TAB SETTLED"));
        assert!(!html.contains("data-tab="));
        no_hex(&html);
    }
}
