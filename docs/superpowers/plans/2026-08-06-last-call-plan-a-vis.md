# Last Call — Plan A-vis: motion and the style guide

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Make Plan A's component library visible and animate it — the §7.7
motion library as plug-and-play classes with a one-shot flight helper, and a
permanent `GET /lastcall/preview` gallery that renders the whole visual
vocabulary from fixtures with no game running.

**Architecture:** The motion contract is **one class plus CSS custom
properties** — `<div class="lc-flight lc-deck-beer" data-flight="draw"
style="--dx:120px; --dy:-80px">` — and a helper that computes the deltas from
two bounding rects, so no animation logic ever lives in feature code. The
preview is an Askama template fed pre-rendered strings by `lc_preview.rs`, which
calls Plan A's builders and nothing else. Public components are rendered
**through `PublicView`** (spec §3.4), which is the only thing in the series that
proves the projection carries *enough* to draw a plaque rather than merely
dropping what it should — a missing field becomes a compile error here, now,
instead of a discovery in Plan A2 after every renderer is written.

**Slice:** Plan A-vis of four for slice 1 (spec §10), in the order **A → A-vis →
A2 → B**.

**It consumes Plan A and authors no components.** Every card, plaque, hand
strip, deck stack and discard slot is a Plan A builder called as-is, and every
fixture is Plan A's `preview_state()`. Plan A already *styled* every root, including
`#lc-felt` and `#lc-flights`; this plan is the first to *render* those two, and
adds the motion library and the gallery page around them.

When it is done there is a URL you can open — `/drinks/lastcall/preview` —
showing every card primitive at all five sizes in all five deck colours (Module
Spec G step 1's done-when, verbatim), the §7.5 text cases including the ones the
catalog cannot sit exactly on, the felt, every plaque state, the F.1 shell
chrome at its real type scale, and each flight direction replayable on demand.

**This plan touches no game.** No `games.kind` value, no setup form, no session
gating, no SSE, no route under `/room/…`, and no database access of any kind. It
is therefore **entirely Class A/B** — if a task here reads as Class C, something
belonging to Plan A2 has been dragged in.

**Why it comes before the wiring.** Spec §10 states the order and the reason:
A-vis is where the design is first seen, and it comes before any wiring so a
token or a text rule can be corrected while the only consumer is a fixture page.
Correcting a ramp threshold here costs one constant; correcting it after Plan A2
and Plan B have built against it costs a refactor across both.

**The four-plan chain, end to end:**

1. **Plan A — the component library** *(done)*. `last_call.rs` types,
   `PublicView`, the adversarial catalog, `preview_state()`, `lastcall.css`
   tokens and §7.6 scene primitives, and `lc_render.rs`'s components to the
   §7.8 contract. Nothing in it was viewable; this plan is where it becomes so.
2. **Plan A-vis — motion and the style guide** *(this plan)*.
3. **Plan A2 — the game wiring.** `last_call` as a third `games.kind`, the setup
   form, the entry redirect, the F.1 phone shell, the private hand route and the
   SSE contract. Every Class C task in the slice lives there.
4. **Plan B — the felt surfaces.** `lc_screen.html`, the D.2 seat-ring angle
   layout that positions Plan A's plaques around **this plan's** felt, the
   `/room/{code}/screen` kind branch, `GET …/lastcall/table`, and the F.3 mini
   table. **Plan B assembles components and authors none.**

**The preview page is permanent.** It is not scaffolding to delete once the game
runs. It is the living style guide and the only way to see a variant — locked,
eliminated, reshuffle, an oversized hand — without engineering the game
situation that produces it, and the only thing that catches a design regression
no test asserts and no player reports (spec §10). Later slices extend it;
nothing removes it.

---

## Global Constraints

Every task's requirements implicitly include this section. It repeats Plan A's
contract tables in full rather than referring to them, because this plan runs in
its own session with a fresh context.

### What Plan A already produced — call these, do not re-derive

```rust
// last_call.rs
Deck { Beer, Cider, Wine, Liquor, Soft }   // ::ALL, .pulls(), .slug(), .label(), ::from_slug()
Beat { Draw, Deal, Diplomacy, Lock, Reveal, Resolve }  // ::ORDER, .index(), .label(), .slug(), .hue(), .next()
Status { Alive, Eliminated }               // .slug()
Card { id, deck, kind, cost, targets, title, text, keywords, duration }
PublicVessel { deck, pulls_left, pulls_max }
PublicSeat { seat, player_id, name, hp, status, vessels, hand_len, locked, drawing, draws }
PublicSeat::decks() -> Vec<Deck>
PublicView { seats, round, beat, first_seat, deck_counts, discard_count, revealed, seq }
LastCallState::public_view(&self) -> PublicView
DECK_LOW_THRESHOLD: u16 = 5;   MAX_SEATS: usize = 8;   STARTING_HP: i32 = 15;

/// Shared runtime fixture builder (spec §8) — NOT `#[cfg(test)]`. Eight seats,
/// two-deck plaques, an oversized hand, every title band, a low deck and an
/// empty one. This plan CONSUMES it and defines no fixtures of its own.
pub fn preview_state() -> LastCallState;

// lc_render.rs — every builder, all emitting deck CLASSES and never hex
pub enum BackSize { Strip, Flight, Pile, Stack }   // .slug() -> the data-size value
pub fn card_face(card: &Card) -> String;
pub fn card_face_expanded(card: &Card) -> String;
pub fn card_pip(card: &Card) -> String;
pub fn card_mini(card: &Card) -> String;
pub fn card_back(deck: Deck, size: BackSize) -> String;
pub fn card_dot(deck: Deck) -> String;
pub fn player_plaque(seat: &PublicSeat) -> String;
pub fn hand_strip(decks: &[Deck], n: usize) -> String;
pub fn deck_rule(decks: &[Deck]) -> String;
pub fn deck_stack(deck: Deck, count: u16) -> String;
pub fn discard_slot(count: usize) -> String;
pub fn lc_banner(view: &PublicView) -> String;
pub fn beat_timer(duration_ms: u32, elapsed_ms: u32) -> String;
pub fn lc_public_panel(view: &PublicView) -> String;
pub fn title_ramp_class(title: &str) -> &'static str;
pub fn is_truncated(card: &Card) -> bool;

// lc_cards.rs — the deliberately adversarial catalog (spec §9)
pub const CATALOG: [CardDef; 20];   // titles in all three ramp bands, one
                                    // 149-char body, one 6-keyword card
pub fn deck_cards(deck: Deck) -> Vec<Card>;

// served by Plan A
GET /assets/lastcall.css
```

**`is-hit` is not projectable, and that is deliberate.** It is a transient
event, not a state — a broadcast snapshot has no way to say "was hit just now"
without leaking timing into state — so `player_plaque` never emits it. The
preview adds it by hand, which is a large part of why this page exists.


### The §7.8 component contracts — verbatim, and binding on every task

Building templates before wiring only pays off if the wiring knows what it is
wiring *to*. Each component declares a DOM contract — the same role the
Interfaces block plays for a plan task. **Plan A already shipped markup matching
this table; this plan renders it into a gallery and adds the last two roots.**

| Component | Root | Requires | Exposes | Motion anchor | Filled by |
| --- | --- | --- | --- | --- | --- |
| Hand region | `#lc-hand` | `data-seq` | `data-count` | `hand` | `GET …/lastcall/hand` (A2) |
| CardFace | `.lc-cardface[data-card-id]` | `data-deck`, `data-cost` | `data-expandable` | — | within Hand region |
| CardPip | `.lc-pip` | `data-deck`, `data-cost` | — | — | within CardFace |
| CardMini | `.lc-mini[data-card-id]` | `data-deck`, `data-cost` | — | — | armed column (slice 2) |
| CardBack | `.lc-back` | `data-deck`, `data-size` | — | — | hand strips, piles, flights |
| CardDot | `.lc-dot` | `data-deck` | — | — | mini-table flights |
| PlayerPlaque | `.lc-plaque[data-seat]` | `data-decks`, `data-hp`, `data-status` | `data-hand-size` | `plaque-seat-{n}` | `LcPublic` SSE (A2) |
| HandStrip | `.lc-handstrip` | `data-hand-size`, `data-decks` | — | — | within PlayerPlaque |
| DeckStack | `.lc-deckstack[data-deck]` | `data-count` | `data-low`, `data-empty` | `deck-{deck}` | `LcPublic` SSE (A2) |
| DiscardSlot | `.lc-discard` | `data-count` | — | `discard` | `LcPublic` SSE (A2) |
| PhaseBanner | `#lc-banner` | `data-beat`, `data-round` | — | — | `LcPublic` SSE (A2) |
| BeatTimer | `#lc-beat-timer` | `data-duration-ms`, `data-elapsed-ms` | — | — | `LcPublic` SSE (A2) |
| Felt scene | `#lc-felt` | — | — | `felt` | static |
| Flight layer | `#lc-flights` | — | — | — | motion helper (§7.7) |

**The contract is structure, never behaviour.** It says `[data-card-id]` exists
and is the click target; it does **not** say that tapping arms the card. **If an
`hx-post` or `hx-get` path appears anywhere in Plan A, the line has been
crossed** — that is slice 2 and 3 work. If a task starts describing what an
interaction *does*, it has drifted out of scope.

### Motion anchors (§7.8.1) — this plan proves every one of them

The §7.7 helper computes `--dx`/`--dy` from two bounding rects, so **every
flight source and destination needs a stable, resolvable name in Plan A's
markup**, even though nothing fires a flight until slice 3. The attribute is
`data-flight-anchor="<name>"` and the complete name set is:

```
deck-beer  deck-cider  deck-wine  deck-liquor  deck-soft
discard
plaque-seat-0 … plaque-seat-7
hand
felt
```

Plan A's plaque, deck stack and discard slot already carry theirs. This plan
adds `felt` and `hand`, and **Task 3 carries the test that resolves all
fourteen on the preview page** — the only place in the series where the whole
set is provable at once. Markup without anchors means slice 3 rewrites every
template, which is the single most expensive thing this staging exists to
prevent.

### Repo rules that bind here

- **No SQL, and no new db function.** This plan reads and writes nothing. If a
  task believes it needs `db.rs`, it has misread the plan.
- **No new component and no new fixture.** Every card, plaque, hand strip, deck
  stack and discard slot is a Plan A builder called as-is, and every fixture
  comes from Plan A's `preview_state()`. A selector or a variant this plan wants
  and cannot find is a bug report against Plan A, not a licence to author markup
  here. The two exceptions are named in the contract table above: the felt
  scene and the flight layer.
- **No migration**, and `cargo sqlx prepare` is not needed — the `drinkinggame`
  crate uses runtime-checked sqlx queries and has no `.sqlx` cache entries
  (CLAUDE.md), and this plan adds no query.
- **The crate's templates do not extend `base.html`** — a recorded exception.
  `lc_preview.html` is standalone, exactly like `room.html` and `screen.html`.
- **No `<style>` blocks in templates.** Every rule lives in
  `assets/lastcall.css` under a named section comment, and the preview page uses
  only classes defined there. A style guide that styles itself inline is not a
  style guide. Never nest `/*` inside a CSS comment — the guard test in Task 2
  exists because that bug silently dropped `.card-big` once.
- **Templates receive pre-computed values.** The preview template gets
  pre-rendered fragment strings from `lc_preview.rs`; it contains no loops over
  game data and no conditionals beyond simple ones.
- **JS that injects DOM binds both `DOMContentLoaded` and `htmx:afterSwap` with
  a double-injection guard** (CLAUDE.md). This plan ships
  `drinkinggame/assets/lc_motion.js` and the preview's replay wiring; both
  follow it. Task 1 also extends `scripts/verify.sh` so `node --check` actually
  covers the crate's assets — today it globs `static/*.js` only.
- **`palette.js` / `base.html` nav are not touched.** Those apply to new
  *portfolio* sections; Last Call lives inside the already-registered `/drinks`
  mount.

### Deck constants — DDv2 §3.1–3.2, spec §4

Pulls are a **deck constant, not a volume**. `pulls_max = deck.pulls()`. The
`container` field is a free-text label and **never affects `pulls_max`** — a
Beer vessel is 8 pulls whether the tin is 50cl or 25cl.

| Deck | slug | pulls | cost spread | role |
| --- | --- | --- | --- | --- |
| Beer | `beer` | 8 | 1–2 | Attrition |
| Cider | `cider` | 10 | 1–3 | Trickster |
| Wine | `wine` | 6 | 2–3 | Control |
| Liquor | `liquor` | 4 | 2–3 | Burst |
| Soft | `soft` | 6 | 1–2 | Support |

Starting HP is **15** for everyone (DDv2 §2.4). Handicap multiplies card cost in
pulls, **rounds up**, and touches nothing else (DDv2 §11).

### Deck colour ramps — spec §7.2, design README

The only taxonomy. No card-type palette, no per-player colour. *Fill* for solid
areas, *ink* for anything on the dark ground; they differ for Wine only.

| Deck | Fill | Ink | Text on fill |
| --- | --- | --- | --- |
| Beer | `#FFB570` | `#FFB570` | `#14101D` |
| Cider | `#B48EF7` | `#B48EF7` | `#14101D` |
| Wine | `#8B2F4A` | `#D4657F` | `#F2EEF8` |
| Liquor | `#F7768E` | `#F7768E` | `#14101D` |
| Soft | `#6FB6FF` | `#6FB6FF` | `#0D1620` |

**Renderers emit deck classes, never hex.** Task 2 owns colour; Task 3 owns
markup. `lc_render.rs` emits `lc-deck-wine`; `lastcall.css` binds `--lc-fill` /
`--lc-ink` / `--lc-on-fill` / `--lc-grid` / the four `--lc-ink-NN` alphas on that
class. Task 3's tests assert this by rejecting any `#` in renderer output.

Beat hues: Draw amber, Diplomacy mint, Lock violet, Reveal azure, Resolve rose.
**Deal has no hue in the bundle** — it inherits Draw's amber (judgment call,
recorded here so it is not re-litigated).

### Surfaces and text — design README

| Token | Value | Use |
| --- | --- | --- |
| page | `#0B0910` | Document ground |
| device | `#0E0C14` | Phone body |
| card / panel | `#16121F` | Plaques, deck rows |
| card alt | `#17141F` | List rows, secondary buttons |
| raised card | `#251F35` | Unfocused wheel cards |
| focused card | `#2E2742` | The focused wheel card |
| card back | `#1B1628` | All card backs and deck stacks |

Text: primary `#F2EEF8`, body `#CDC6DD`, secondary `#A79FBB`, label `#8D87A0`,
faint `#6A6480`. Status accents: mint `#4FD6A8`, amber `#FFB570`, rose
`#F7768E`, azure `#6FB6FF`, violet `#B48EF7`. Hairlines `rgba(242,238,248,.10)`
to `rgba(242,238,248,.28)`.

Radii: 3px badges, 5px pips, 6px small cards/secondary buttons, 8px primary
buttons and panels, 10px plaques, 12–14px card faces.
Elevation: `0 3px 0 rgba(5,3,10,.5), 0 8px 16px rgba(5,3,10,.42)` (small cards),
`0 6px 0 rgba(5,3,10,.6), 0 22px 40px rgba(5,3,10,.55)` (focused card). The hard
offset is the deck-of-cards look — do not replace it with a blur-only shadow.
Motion: 130ms taps, 190ms state changes, 280ms position changes, all on
`cubic-bezier(.2,.8,.3,1)`.

### §7.5 text-handling thresholds

| Title length | Size | Class |
| --- | --- | --- |
| ≤ 14 chars | 30px (the authored size) | `lc-title-lg` |
| 15–24 chars | 24px | `lc-title-md` |
| > 24 chars | 20px | `lc-title-sm` |

Title clamped to 2 lines; body Space Grotesk 400/15px/1.35 clamped to 3 lines;
at most 3 keyword chips then a `+n` chip; CardMini name clamped to 2 lines.
Truncation is decided **server-side from the string**, so it is deterministic
and unit-testable — which is why the ramp is expressed in characters rather than
measured width.

### Routes added by this plan

Three, all public and unguarded. Routes are written unprefixed —
`nest_service` strips the `/drinks` mount, and only *generated URLs*
interpolate `base_path`.

| Method | Path | Task |
| --- | --- | --- |
| GET | `/assets/lc_motion.js` | 1 |
| GET | `/lastcall/preview` | 2 |

`/assets/lastcall.css` was registered by Plan A; this plan appends sections to
that stylesheet without touching its route.

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report.

**Browser checkpoints:** after **Task 2** (the preview first renders) and after
**Task 3**, before the final review. Not per task. This is the first plan in the
series with anything to look at, and looking is most of its value.

---

---
### Task 1: The §7.7 motion library — seven keyframes and a plug-and-play flight helper

**Class:** A (compiler/lint-gated)

**Why this class:** CSS keyframes plus one self-contained JS module. The two
things that go silently wrong here — a nested `/*` in the stylesheet and a
syntax error in the JS — are both decided by `./scripts/verify.sh` once this
task extends `node --check` to cover the crate's assets, which is Step 5. No
game state, no session, no database.

**Files:**
- Modify: `drinkinggame/assets/lastcall.css` (a `/* motion */` section)
- Create: `drinkinggame/assets/lc_motion.js`
- Modify: `drinkinggame/src/routes.rs` (`lc_motion_js` handler + registration)
- Modify: `scripts/verify.sh` (extend `node --check` to the crate's assets)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: the deck classes and `--lc-ease` from **Plan A**'s stylesheet; the
  `data-flight-anchor` names from the Global Constraints.
- Produces — **this is the contract every later slice fires rather than
  rewrites.** One class plus CSS custom properties; no animation logic in
  feature code:

```html
<div class="lc-flight lc-deck-beer" data-flight="draw" data-scale="card"
     style="--dx:120px; --dy:-80px"></div>
```

```js
// assets/lc_motion.js — one global, deliberately.
/**
 * Fires one flight from `fromEl` to `toEl` and calls onArrive when it lands.
 * @param {Element} fromEl  source (deck stack, plaque)
 * @param {Element} toEl    destination (plaque, discard slot, centre)
 * @param {{direction?: "draw"|"play"|"discard", deck?: string,
 *          scale?: "card"|"dot", delay?: number, onArrive?: () => void}} opts
 */
window.lcFlight = function (fromEl, toEl, opts) { … };

/** Resolves a `data-flight-anchor` name to its element, or null. */
window.lcAnchor = function (name, root) { … };
```

Keyframe names, all seven: `lc-fly`, `lc-dot`, `lc-shake`, `lc-hp-flash`,
`lc-pulse`, `lc-banner`, `lc-timer`.
State classes later slices toggle: `.is-hit` (shake + HP flash), `.is-drawing`
(deck-rule pulse), `.is-urgent` (timer under 5s).
Route: `GET /assets/lc_motion.js` → `application/javascript`.

- [ ] **Step 1: The two bundle keyframes, verbatim**

`lc-fly` and `lc-dot` are the **only** two keyframes the bundle defines. Copy
them exactly — the percentage stops are the design, not an approximation:

```css
/* motion — flights. lc-fly and lc-dot are verbatim from the design bundle. */
@keyframes lc-fly {                     /* big screen, 6s cycle */
  0%   { opacity: 0; transform: translate(0,0) scale(.5); }
  3%   { opacity: 1; transform: translate(0,0) scale(.85); }
  22%  { opacity: 1; transform: translate(var(--dx), var(--dy)) scale(1); }
  27%  { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.92); }
  100% { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.92); }
}
@keyframes lc-dot {                     /* phone mini-table, 4.4s cycle */
  0%   { opacity: 0; transform: translate(0,0) scale(.6); }
  8%   { opacity: 1; transform: translate(0,0) scale(1); }
  46%  { opacity: 1; transform: translate(var(--dx), var(--dy)) scale(1); }
  54%  { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.6); }
  100% { opacity: 0; transform: translate(var(--dx), var(--dy)) scale(.6); }
}
```

**These are one-shot in production.** The prototypes run them on infinite loops
to demonstrate motion in a static document; that is a demo artifact, not the
design (spec §7.7). The 6s and 4.4s figures are the prototype's *cycle* length —
the visible travel finishes at 27% and 54% respectively, so production fires
them once with `animation-fill-mode: forwards` and the helper removes the node
on `animationend`. Do not shorten the keyframes to "fix" the dead tail; the
percentages are what give the flight its ease, and the node is gone before the
tail matters.

```css
.lc-flight {
  position: absolute; top: 0; left: 0; pointer-events: none;
  background: var(--lc-fill, var(--lc-body));
  animation-duration: 6s; animation-timing-function: linear;
  animation-fill-mode: forwards; animation-iteration-count: 1;
}
.lc-flight[data-scale="card"] { width: 44px; height: 62px; border-radius: 3px;
                                animation-name: lc-fly; }
.lc-flight[data-scale="dot"]  { width: 8px;  height: 8px;  border-radius: 50%;
                                animation-duration: 4.4s; animation-name: lc-dot; }
```

The flight node carries `lc-deck-{slug}` for its colour, like every other
renderer output — the spec's illustrative snippet shows an inline
`--deck-fill:#FFB570`, which this plan supports as an **optional override** for
callers with no deck (the discard flight), because `background` already falls
back through `var(--lc-fill, …)`. One colour source, and no hex in renderer
output.

- [ ] **Step 2: The five authored keyframes**

The bundle names these in prose only — no keyframes exist for them anywhere in
`Game UI.dc.html`. They are authored here, on the design's own motion budget:
130ms taps, 190ms state changes, 280ms position changes, all on
`cubic-bezier(.2,.8,.3,1)`.

```css
/* motion — state changes. Named in the bundle as prose only; authored here. */
@keyframes lc-shake {                   /* plaque hit — 4px horizontal, 190ms */
  0%, 100% { transform: translateX(0); }
  20%  { transform: translateX(-4px); }
  45%  { transform: translateX(4px); }
  70%  { transform: translateX(-2px); }
  90%  { transform: translateX(2px); }
}
@keyframes lc-hp-flash {                /* HP to rose and back */
  0%   { color: var(--lc-text); }
  15%  { color: var(--lc-rose); }
  60%  { color: var(--lc-rose); }
  100% { color: var(--lc-text); }
}
@keyframes lc-pulse {                   /* deck rule, while that player draws */
  0%, 100% { opacity: 1; }
  50%      { opacity: .45; }
}
@keyframes lc-banner {                  /* beat-name cross-fade, 280ms */
  0%   { opacity: 0; transform: translateY(4px); }
  100% { opacity: 1; transform: none; }
}
@keyframes lc-timer {                   /* beat rail filling down */
  from { transform: scaleX(1); }
  to   { transform: scaleX(0); }
}

.lc-plaque.is-hit            { animation: lc-shake 190ms var(--lc-ease) 1; }
.lc-plaque.is-hit .lc-hp     { animation: lc-hp-flash 560ms var(--lc-ease) 1; }
.lc-plaque.is-drawing .lc-rule { animation: lc-pulse 900ms var(--lc-ease) infinite; }
.lc-banner-beat              { animation: lc-banner 280ms var(--lc-ease) 1; }
.lc-timer {
  height: 2px; transform-origin: left center;
  background: var(--lc-beat, var(--lc-amber));
  animation: lc-timer var(--lc-beat-ms, 60s) linear 1 forwards;
}
.lc-timer.is-urgent { background: var(--lc-rose); }
```

Two rules the design states and the code must honour: the banner's **hue does
not animate**, only the name cross-fades — which is why `lc-banner` touches
opacity and transform but never colour; and the timer has **no numerals and no
ticking**, because a bar is legible across a room and does not create urgency in
a game meant to be slow. The `.is-urgent` swap to rose is a class the caller
toggles under 5s, not a keyframe colour stop.

`lc-pulse` is the one infinite animation in the library, and correctly so — it
marks a *state* ("this player is drawing"), not an event. Everything else is
one-shot.

- [ ] **Step 3: `prefers-reduced-motion` — one block, not two**

```css
@media (prefers-reduced-motion: reduce) {
  /* Stop the flights and hold them at rest opacity — an attribute selector on
     the animation name, as the bundle does. */
  .lc-flight { animation: none !important; opacity: 0; }
  .lc-plaque.is-hit, .lc-plaque.is-hit .lc-hp,
  .lc-plaque.is-drawing .lc-rule,
  .lc-banner-beat, .lc-timer { animation: none !important; }
  .lc-timer { transform: scaleX(0); }
  .lc-cardface, .lc-btn { transition: none; }
}
```

This is the stylesheet's **only** `prefers-reduced-motion` block — Plan A's
Task 2 was told not to add one, so this is the first and must stay the last. Two blocks means the second silently overrides half the
first, and the failure is invisible to anyone not testing with the emulation on.

- [ ] **Step 4: `assets/lc_motion.js` — the helper**

Self-contained vanilla JS, no framework, no bundler, written to the pattern
`static/palette.js` already uses.

```js
// Last Call motion helper. The contract is one class plus CSS custom
// properties (spec §7.7) — no animation logic lives in feature code.
(function () {
  "use strict";
  var LAYER_ID = "lc-flights";

  function reduced() {
    return window.matchMedia &&
           window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  // Double-injection guard: hx-boost swaps body children without a reload, so
  // this runs again on every navigation and must not stack a second layer.
  function ensureLayer(root) {
    var host = (root || document).querySelector("[data-lc-scene]") || document.body;
    var layer = host.querySelector("#" + LAYER_ID);
    if (layer) return layer;                     // <- the guard
    layer = document.createElement("div");
    layer.id = LAYER_ID;
    host.appendChild(layer);
    return layer;
  }

  window.lcAnchor = function (name, root) {
    return (root || document)
      .querySelector('[data-flight-anchor="' + name + '"]');
  };

  function centre(el, originRect) {
    var r = el.getBoundingClientRect();
    return { x: r.left + r.width / 2 - originRect.left,
             y: r.top + r.height / 2 - originRect.top };
  }

  window.lcFlight = function (fromEl, toEl, opts) {
    opts = opts || {};
    var arrive = opts.onArrive || function () {};
    // Reduced motion: no node at all, but the arrival still fires. The
    // README's rule is that arrival must tick the destination's counter —
    // "the number and the animation are one event, never two" — so skipping
    // the animation must never skip the count.
    if (reduced() || !fromEl || !toEl) { arrive(); return; }

    var layer = ensureLayer();
    var origin = layer.getBoundingClientRect();
    var a = centre(fromEl, origin), b = centre(toEl, origin);

    var node = document.createElement("div");
    node.className = "lc-flight" + (opts.deck ? " lc-deck-" + opts.deck : "");
    node.setAttribute("data-flight", opts.direction || "draw");
    node.setAttribute("data-scale", opts.scale === "dot" ? "dot" : "card");
    node.style.left = a.x + "px";
    node.style.top = a.y + "px";
    node.style.setProperty("--dx", (b.x - a.x) + "px");
    node.style.setProperty("--dy", (b.y - a.y) + "px");
    if (opts.delay) node.style.animationDelay = opts.delay + "ms";

    // Fires once, then removes itself. No timers: animationend is the only
    // signal that stays correct when the tab is backgrounded and rAF throttles.
    node.addEventListener("animationend", function () {
      node.remove();
      arrive();
    }, { once: true });
    layer.appendChild(node);
  };

  function init() { ensureLayer(); }
  document.addEventListener("DOMContentLoaded", init);
  document.addEventListener("htmx:afterSwap", function (e) { ensureLayer(e.target); });
})();
```

Four details that are requirements, not style:

- **Bind on both `DOMContentLoaded` and `htmx:afterSwap`, with an existence
  guard** (CLAUDE.md). `ensureLayer` returning early on a found layer *is* the
  guard; do not replace it with a boolean flag, which survives a body swap and
  then wrongly suppresses re-creation. Listen on `document`, not `document.body`
  — at script-evaluation time in `<head>`-deferred position `document.body` may
  not exist yet, and the event bubbles to `document` anyway.
- **Arrival fires even under reduced motion.** README: *"Arrival must tick the
  destination's counter. The number and the animation are one event, never
  two."* A reduced-motion user who stops seeing counts is a bug, not an
  accommodation.
- **`animationend`, never `setTimeout`.** A backgrounded tab throttles timers;
  `animationend` still fires when the tab returns, and `{ once: true }` means
  the listener cannot leak.
- **`lcAnchor` is why §7.8.1 exists.** Slice 3 calls
  `lcFlight(lcAnchor("deck-beer"), lcAnchor("plaque-seat-3"), …)` and needs both
  ends to resolve against markup Plan A shipped. The helper does the lookup so
  the anchor names, not selectors, are the coupling.

Stagger for a burst of draws is the caller's `opts.delay` — the README's
0.2–0.3s per player, so seven simultaneous draws read as a burst out of the
middle of the table rather than a blur. The helper does not schedule; it fires
what it is told, once.

- [ ] **Step 5: Serve it, and make `node --check` actually cover it**

```rust
// routes.rs, next to htmx_js
async fn lc_motion_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../assets/lc_motion.js"),
    )
}
```

```rust
.route("/assets/lc_motion.js", get(lc_motion_js))
```

Then **extend `scripts/verify.sh`**. Its `node --check` loop currently covers
`static/*.js` only — the crate's own assets are not checked at all, so a syntax
error in `lc_motion.js` would compile fine (it is a string to Rust) and fail
only in a browser. Change the loop to iterate both globs:

```bash
for f in static/*.js drinkinggame/assets/*.js; do
```

and update the header comment's check-list line 4 accordingly. This is the same
class of gap that let the `palette.js` nested-entry bug through (`c72d614`), and
it is two words of shell.

- [ ] **Step 6: Tests in `tests/http.rs`**

1. Add `("/assets/lc_motion.js", "application/javascript")` to the loop in
   `test_assets_are_served`.
2. `test_lastcall_css_has_every_keyframe` — the served stylesheet contains all
   seven: `@keyframes lc-fly`, `lc-dot`, `lc-shake`, `lc-hp-flash`, `lc-pulse`,
   `lc-banner`, `lc-timer`.
3. `test_lastcall_css_reduced_motion_is_one_block` — the sheet contains
   `prefers-reduced-motion: reduce` **exactly once**, and that block contains
   `.lc-flight` and `animation: none`.
4. `test_lc_motion_js_binds_both_lifecycle_events` — the served JS contains
   `DOMContentLoaded` **and** `htmx:afterSwap`, and contains `animationend` and
   `data-flight-anchor`. Crude, and it is the regression that matters: the guard
   rule is the one CLAUDE.md calls out by name, and a rewrite that drops one
   binding passes every other check.
5. Plan A's nested-comment scanner already covers `/assets/lastcall.css` —
   confirm it still runs, now that this task appends a section to that file.

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/assets/lastcall.css drinkinggame/assets/lc_motion.js \
        drinkinggame/src/routes.rs scripts/verify.sh drinkinggame/tests/http.rs
git commit -m "feat(drinks): Last Call motion library — seven keyframes and a one-shot flight helper"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: `GET /lastcall/preview` — the route, the `PublicView` fixtures and the card matrix

**Class:** A (compiler/lint-gated)

**Why this class:** an Askama template and a handler with no extractors beyond
`State`, assembling strings from Plan A's builders. Askama compiles into the
binary, so a broken template is a build error; the route's contract is "returns 200 and contains
these class names", which is an integration test whose output you can eyeball.
Nothing here reads a session, a room or the database.

**Files:**
- Create: `drinkinggame/src/lc_preview.rs`
- Create: `drinkinggame/templates/lc_preview.html`
- Modify: `drinkinggame/src/lib.rs` (`pub mod lc_preview;`)
- Modify: `drinkinggame/src/routes.rs` (`router()` — one registration)
- Modify: `drinkinggame/assets/lastcall.css` (a `/* preview page */` section)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: every `lc_render` builder, `lc_cards::{CATALOG, deck_cards}`,
  `last_call::{Deck, Card, CardKind, LastCallState, PublicView, PublicSeat,
  preview_state}` and the full CSS class contract — **all from Plan A**.
  **`preview_state()` already exists**: Plan A defines it in `last_call.rs`,
  not `#[cfg(test)]`, exactly so this route can render the same eight-seat
  state its unit tests assert against (spec §8). Do **not** write it again, and
  do not touch `last_call.rs` in this task — a second definition will not
  compile, and a second *copy* would be worse: the whole point is that a test
  failure and a visual regression cannot disagree about what the fixture is.
- Produces:

```rust
// lc_preview.rs — everything this task defines
#[derive(Template)]
#[template(path = "lc_preview.html")]
pub struct LcPreviewTemplate { pub base_path: String, pub groups: Vec<PreviewGroup> }

pub struct PreviewGroup { pub id: String, pub title: String, pub note: String, pub body: String }

/// Threshold-boundary cards the catalog cannot cover by design.
pub fn boundary_cards() -> Vec<(&'static str, Card)>;

/// GET /lastcall/preview — public, unguarded, fixture-only.
pub async fn preview_page(State<GameState>) -> impl IntoResponse;
```

- [ ] **Step 1: Render public components through `PublicView` — spec §3.4**

**The preview page renders public components from `PublicView` fixtures, not
from raw `LastCallState`.** Build the projection once at the top of
`build_groups()` and pass `&PublicSeat` / `&PublicView` into every public
builder:

```rust
let st = last_call::preview_state();
let view = st.public_view();          // <- everything public goes through this
```

This is the only thing in Plan A that proves the projection carries **enough**
to draw a plaque, rather than merely dropping what it should. A missing field
becomes a compile error here, now — instead of a discovery in Plan A2 after
every renderer is written. Say that in the group's `note` so a later refactor
that "simplifies" the preview back to raw state is recognised as the regression
it is.

Card-level builders still take `&Card`, because the hand is private and never
projected — `card_face` is only ever called on the viewer's own cards.

- [ ] **Step 2: `lc_preview.rs` builds every group as a pre-rendered string**

CLAUDE.md: templates receive pre-computed values. `lc_preview.rs` assembles each
section's HTML by calling builders and pushes a `PreviewGroup`;
`lc_preview.html` iterates and emits `{{ g.body|safe }}`. Sample markup lives in
Rust where it is greppable, not buried in HTML. Two private helpers:

```rust
/// A labelled swatch cell: the rendered sample plus its caption.
fn swatch(label: &str, html: &str) -> String;
/// A row of swatches under a sub-heading.
fn row(heading: &str, cells: &[String]) -> String;
```

- [ ] **Step 3: Group 1 — the card primitive matrix**

**Module Spec G step 1's done-when, verbatim: one card renders at all five
sizes, in all five deck colours, from one object.** A real matrix — five rows
(one per deck), five columns:

```rust
for deck in Deck::ALL {
    let card = &lc_cards::deck_cards(deck)[0];
    cells.push(swatch("CardFace", &lc_render::card_face(card)));
    cells.push(swatch("CardPip",  &lc_render::card_pip(card)));
    cells.push(swatch("CardMini", &lc_render::card_mini(card)));
    cells.push(swatch("CardBack", &lc_render::card_back(deck, BackSize::Pile)));
    cells.push(swatch("CardDot",  &lc_render::card_dot(deck)));
}
```

One `Card` object per row, five renderings from it — never five different data
shapes. Add a row showing `CardBack` at **all four** sizes (`strip` 16×24,
`flight` 44×62, `pile` 46×62, `stack` 68×92): the grid *is* the card back, and
the 9px/10px `background-size` step between them is exactly what only an eye
catches.

Then **every cost 1–3 in every deck** (spec §10): a 15-cell block of `card_pip`,
five decks × costs 1, 2, 3. Costs outside a deck's spread are not playable but
the *pip* must still render — this is the primitive's matrix, not the catalog's.

- [ ] **Step 4: Group 2 — the §7.5 text cases**

Two sources, because they prove different things.

**From the catalog**, which spec §9 makes deliberately adversarial: render every
one of the 20 `CATALOG` cards as a `card_face`. This is the group that proves
the ramp is exercised by *rendered content* and not only by test fixtures — the
`lg`, `md` and `sm` branches are all reachable from the catalog alone (15, 3 and
2 cards respectively), `wine-01`'s 149-character body overflows, `cider-04`
carries six keywords and most carry none.

**From `boundary_cards()`**, for the thresholds the catalog cannot sit exactly
on: titles of exactly 14, 15, 24 and 25 characters; a body of exactly 108 and
one of 109; keyword counts of 0, 3 and 6. Each rendered **twice, side by side**
— the clamped `card_face` and the `card_face_expanded` variant. That pairing is
the group's whole point: it shows what was lost, which a single clamped card
cannot. Caption each pair with its label and whether `is_truncated` marked it.

Assert the fixtures rather than trusting them:
`test_boundary_cards_hit_their_boundaries` — the 14/15/24/25-char titles really
are 14/15/24/25 `chars()`, and the two bodies are exactly 108 and 109. A
mis-typed fixture that quietly lands on the wrong side of a threshold makes the
whole group meaningless.

- [ ] **Step 5: The template and the route**

`lc_preview.html` is standalone (does not extend `base.html` — recorded crate
exception), links `{{ base_path }}/assets/lastcall.css`, defers
`{{ base_path }}/assets/lc_motion.js`, and contains **no `<style>` block**:

```html
<body class="lc lc-preview" data-lc-scene>
  <header class="lc-preview-head">
    <h1>Last Call — visual vocabulary</h1>
    <p>Fixture data. No game, no session, no live state. This page is permanent:
       it is the only way to see a variant without engineering the situation
       that produces it.</p>
  </header>
  <nav class="lc-preview-nav">…one anchor per group id…</nav>
  {% for g in groups %}
  <section class="lc-preview-group" id="{{ g.id }}">
    <h2>{{ g.title }}</h2>
    <p class="lc-preview-note">{{ g.note }}</p>
    <div class="lc-preview-body">{{ g.body|safe }}</div>
  </section>
  {% endfor %}
</body>
```

`data-lc-scene` on `<body>` is what `lc_motion.js`'s `ensureLayer` looks for, so
`#lc-flights` is created inside the page rather than appended to a random
ancestor.

Add the `.lc-preview*` classes to `lastcall.css` under a `/* preview page */`
section comment in **this** task, so the section stays with the page it serves.
Layout only — page max-width, group spacing, the swatch grid, a caption style,
and a `.lc-preview-pair` two-column cell for Step 4's clamped/expanded pairing.
Introduce no new colours; use the tokens.

```rust
pub async fn preview_page(State(state): State<GameState>) -> impl IntoResponse {
    let tpl = LcPreviewTemplate {
        base_path: state.base_path.to_string(),
        groups: build_groups(),
    };
    Html(tpl.render().unwrap())
}
```

```rust
// routes.rs, in router(), next to the /presets block
.route("/lastcall/preview", get(crate::lc_preview::preview_page))
```

**No `PlayerSession`.** Unlike its neighbours `/presets` and `/presets/{id}`,
which are session-gated, the preview takes no extractor beyond `State` — it
displays fixture constants, touches no room, no player and no database, and a
style guide you have to log in to read is a style guide nobody reads. `State` is
consumed only for `base_path`, so the asset links resolve under both the
`/drinks` mount and the standalone bin.

- [ ] **Step 6: Tests in `tests/http.rs`**

Use the existing `get(&app, path)` helper — no login, which is itself the
assertion that the route is unguarded.

1. `test_preview_page_is_public` — `GET /lastcall/preview` with **no cookie**
   returns `200`. Under `test_app_with_base("/drinks")` the body contains
   `href="/drinks/assets/lastcall.css"` and `src="/drinks/assets/lc_motion.js"`.
2. `test_preview_renders_five_primitives_in_five_decks` — the G step-1
   done-when. For each of the five slugs the body contains `lc-deck-{slug}`;
   and it contains `lc-cardface`, `lc-pip`, `lc-mini`, `lc-back`, `lc-dot`,
   with `lc-cardface` at least 5 times and each
   `data-size="{strip|flight|pile|stack}"` at least once.
3. `test_preview_shows_every_title_ramp_step` — the body contains
   `lc-title-lg`, `lc-title-md` **and** `lc-title-sm`. All three are reachable
   from the catalog alone; if `CATALOG` regresses to tidy short titles this
   fails, which is exactly what spec §9 asks for.
4. `test_preview_shows_truncation_and_expansion` — the body contains
   `data-expandable`, `lc-kw-more`, `+3` (the six-chip overflow) and
   `lc-cardface-expanded`.
5. `test_preview_shows_every_cost_pip` — `data-cost="1"`, `"2"` and `"3"` each
   appear at least five times (once per deck).
6. `test_preview_has_no_style_element_and_no_behaviour` — the body contains no
   `<style` element, and no `hx-post`, `hx-get` or `onclick`. Inline
   `style="--dx:…"` attributes **are** expected on Task 3's at-rest flight
   sample, so assert specifically that no `style=` attribute contains a `#` —
   colour comes from the stylesheet, positions may come from custom properties.

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/src/lc_preview.rs drinkinggame/templates/lc_preview.html \
        drinkinggame/src/lib.rs drinkinggame/src/routes.rs \
        drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(drinks): Last Call preview route, card matrix and the text-handling cases"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

> ### Browser checkpoint 1 — after Task 2
>
> `cargo run -p drinkinggame` (standalone on `:3001`, no portfolio, no nginx)
> and open `http://localhost:3001/lastcall/preview`. No login.
>
> 1. The matrix shows five rows, one per deck, each with CardFace, CardPip,
>    CardMini, CardBack and CardDot. **This is Module Spec G step 1's
>    done-when** — if any cell is missing or grey, the task is not done.
> 2. Wine's ink (`#D4657F`) is legibly lighter than its fill (`#8B2F4A`): the
>    Wine CardFace's deck label and border read clearly on the dark ground while
>    the Wine CardPip is the darker solid with light text. This is the one ramp
>    where fill and ink differ, and the one a copy-paste breaks.
> 3. Card backs show the grid at all four sizes — still visible at 16×24.
> 4. Measure a CardFace in devtools: **176px** tall, not 208px. If it is 208 the
>    `box-sizing: border-box` reset is missing and every size on the page is
>    wrong too.
> 5. **The text group is the one to actually read.** Across the 20 catalog cards
>    you can see all three title sizes without hunting; `Decant`'s long body
>    clips at three lines with no orphan half-line; `Windfall` shows three chips
>    and `+3`. In the boundary pairs, each clamped card sits beside its expanded
>    twin and the twin shows the text the clamped one lost. Nothing overflows
>    its 176px box.
> 6. Fonts load — titles are Archivo 900, not a system fallback. Check the
>    Network tab for the woff2 requests under `/assets/fonts/`.

---

### Task 3: The preview's scene, components, states and replayable flights

**Class:** A (compiler/lint-gated)

**Why this class:** more groups appended to the same template and the same
`Vec<PreviewGroup>`, plus page-local JS that calls the Task 1 helper. Askama
compiles the template, `node --check` (extended in Task 1) covers the JS, and
the tests below assert the contract attributes and anchors are present. Still no
session, no room, no database.

**Files:**
- Modify: `drinkinggame/src/lc_preview.rs`
- Modify: `drinkinggame/templates/lc_preview.html` (the replay wiring)
- Modify: `drinkinggame/assets/lastcall.css` (preview-page layout only)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `PreviewGroup`, `swatch`, `row` (Task 2) and `preview_state()`
  (**Plan A**);
  `lc_render::{player_plaque, hand_strip, deck_rule, deck_stack, discard_slot,
  lc_banner, beat_timer, card_back, card_dot}` and the state classes
  (**Plan A**); `#lc-felt` and `#lc-flights` (Task 1);
  `window.lcFlight` / `window.lcAnchor` (Task 1).
- Produces: no new public signatures — `build_groups()` returns more groups.
  **Plan B's contract:** the felt, plaque and deck-stack samples on this page
  are the reference markup Plan B positions. Plan B changes *where* they sit,
  never *what they are*.

- [ ] **Step 1: Group 3 — the scene, including the felt**

The §7.6 primitives at a size you can judge: grounds and panels as labelled
swatches; the hairline ladder; the four deck-tinted border alphas (`59`, `66`,
`80`, `99`) side by side per deck — that ladder is invisible anywhere else and
is exactly the sort of value that drifts; and **the felt** in a 640×360 box,
with its rail, its inner hairline ellipse and its shadow stack, carrying
`id="lc-felt"` and `data-flight-anchor="felt"`.

Seat positioning is **not** here. The felt ships as a background primitive;
D.2's angle layout is Plan B. Say so in the group's `note`, or the next reader
will assume the seat ring was forgotten.

- [ ] **Step 2: Group 4 — the table components and the plaque's five states**

Every plaque is built from a **`&PublicSeat`** taken from the Task 2, Step 1
projection, never from an `LcPlayer`.

| Swatch | Built from | Shows |
| --- | --- | --- |
| idle plaque | `player_plaque(&view.seats[0])` | the base: name, HP, deck dots, hand strip |
| locked | `view.seats[2]` (cara) | violet tick, and **no cards** |
| drawing | `view.seats[4]` (erin) | the deck rule pulsing |
| eliminated | `view.seats[6]` (gus) | 40%, `GHOST` in place of HP |
| hit | `view.seats[0]` with `is-hit` added by the preview, replayable | 4px shake + HP flash to rose |
| two-deck plaque | `view.seats[5]` (fin) | one dot per vessel, 3px rule split 50/50, backs cycling both decks |
| oversized hand | `view.seats[1]` (bob, 12 cards) | `n > 8` → 7 backs + `+5` |
| hand strip sizes | `hand_strip(&[Deck::Beer], n)` for `n` in 0, 1, 4, 8, 9, 30 | the split, either side of the boundary |
| deck stacks | `deck_stack(d, c)` from `view.deck_counts` | Wine 4 is `data-low` amber; Liquor 0 is `data-empty` / `RESHUFFLE` |
| discard slot | `discard_slot(view.discard_count)` | dashed, no grid, neutral count |
| beat timer | `beat_timer(60_000, 0)` and a copy with `is-urgent` | fills down; rose under 5s |

**`is-hit` is added by the preview, not by `player_plaque`.** It is a transient
event, not a projected state — a broadcast snapshot has no way to say "was hit
just now" without leaking timing into state — see Plan A's `player_plaque`. The
preview is
therefore the only place it can be demonstrated, which is a large part of why
this page exists.

Drive every count from `view` rather than hard-coding, so Plan A's `test_preview_state_covers_every_variant` keeps guarding this group.

- [ ] **Step 3: Group 5 — F.1 shell chrome, static**

The type scale, tab row, phase banner and action bar at their real sizes, inside
a 390×844 frame, in F.1's fixed vertical order: status row → phase banner → tab
row → view → action bar. **Static markup — this is not the live phone shell
route, which is Plan A2's.** Put that in the group's `note` so nobody wires an
`EventSource` into a style guide.

- The banner from the real builder, `lc_render::lc_banner(&view)` → `LOCK` in
  violet with `ROUND 6 · BEAT 4 OF 6`.
- Beside it, the banner **once per beat** — all six, `Draw` through `Resolve` —
  so the hue set is checkable at a glance and Deal's inherited amber sits next
  to Draw's.
- Tabs HAND / TABLE / LOG, with a second copy showing TABLE active. **The order
  never changes and the active tab is never hoisted** — only colour and the 2px
  underline move (F.1). Two copies side by side is what makes a regression
  obvious.
- Action bar: one row with two primaries (the drinking option amber, per F.1),
  one row with a primary plus the 92px secondary.
- The `.lc-setup` form chrome, so the plain setup form Plan A2 renders has a
  known appearance before it exists. **No `action` and no `hx-post`** — the
  chrome is structure; where it posts is Plan A2's.
- The hand region: `#lc-hand` with `data-seq` and `data-count`, carrying
  `data-flight-anchor="hand"`, holding a few `card_face`s.

- [ ] **Step 4: Group 6 — replayable flights and the anchor board**

Motion in a static document is invisible unless you can fire it. Each direction
gets a **REPLAY** button:

| Direction | From anchor | To anchor | Scale |
| --- | --- | --- | --- |
| `draw` | `deck-beer` | `plaque-seat-0` | `card` |
| `play` | `plaque-seat-0` | `felt` | `card` |
| `discard` | `plaque-seat-0` | `discard` | `card` |
| `draw` (phone) | `deck-soft` | `plaque-seat-1` | `dot` |

Plus a **BURST** button firing seven `draw` flights with the README's 0.2–0.3s
stagger, because "seven simultaneous draws read as a burst out of the middle of
the table rather than a blur" is a claim only a burst can check.

And an **anchor board**: a small list rendering every name from the §7.8.1 set,
each resolved live via `window.lcAnchor`, showing ✓ or ✗. It is three lines of
JS and it turns "did Plan A ship the anchors" from a code review question into
something the page answers itself.

The page's script calls the Task 1 helper — it does not reimplement it:

```html
<script src="{{ base_path }}/assets/lc_motion.js" defer></script>
<script>
// Preview-only: wire the REPLAY buttons to the shared helper. No animation
// logic here — that is the point of the library (spec §7.7).
function lcPreviewInit() {
  document.querySelectorAll("[data-replay]").forEach(function (btn) {
    if (btn.dataset.lcBound) return;         // double-injection guard
    btn.dataset.lcBound = "1";
    btn.addEventListener("click", function () {
      var from = window.lcAnchor(btn.dataset.from);
      var to = window.lcAnchor(btn.dataset.to);
      var n = Number(btn.dataset.count || 1);
      for (var i = 0; i < n; i++) {
        window.lcFlight(from, to, {
          direction: btn.dataset.replay,
          deck: btn.dataset.deck,
          scale: btn.dataset.scale,
          delay: i * 250,
        });
      }
    });
  });
  document.querySelectorAll("[data-anchor-check]").forEach(function (el) {
    el.textContent = window.lcAnchor(el.dataset.anchorCheck) ? "✓" : "✗";
  });
}
document.addEventListener("DOMContentLoaded", lcPreviewInit);
document.addEventListener("htmx:afterSwap", lcPreviewInit);
</script>
```

Both bindings and the `data-lcBound` guard are required even though this page is
not `hx-boost`ed — the rule is CLAUDE.md's, the cost is three lines, and a
future move of this markup into a boosted page must not silently double every
listener.

Add one static swatch showing the flight node **at rest** (`.lc-flight` with
`animation: none` via an inline `style`), so its 44×62 and 8×8 footprints are
checkable without chasing a moving target.

- [ ] **Step 5: Group 7 — the deck ramp reference**

A plain table: five rows, each with the deck name, a filled chip
(`--lc-fill`), an ink-on-dark text sample (`--lc-ink`), a text-on-fill sample,
and the four alpha steps. Wine is the row that matters — the only deck where
fill and ink differ, and a regression there is invisible in every other swatch.
Include the hex values as text so the page is self-documenting; they are the
*documented* values, not renderer output, so this does not violate Plan A's
no-hex-in-renderers rule. Note the distinction in the group's `note`.

- [ ] **Step 6: Tests in `tests/http.rs`**

1. `test_preview_resolves_every_motion_anchor` — **the §7.8.1 test.** For every
   name in the set — `deck-beer`, `deck-cider`, `deck-wine`, `deck-liquor`,
   `deck-soft`, `discard`, `plaque-seat-0` … `plaque-seat-7`, `hand`, `felt` —
   the body contains `data-flight-anchor="{name}"`. All fourteen, with no
   exceptions: the fixture is eight seats precisely so `plaque-seat-7` is
   provable here. Markup without anchors means slice 3 rewrites every template,
   which is the single most expensive thing this staging prevents.
2. `test_preview_shows_all_six_beat_hues` — the body contains `lc-beat-amber`,
   `lc-beat-mint`, `lc-beat-violet`, `lc-beat-azure`, `lc-beat-rose`, with
   `lc-beat-amber` at least twice (Draw and Deal).
3. `test_preview_shows_every_plaque_state` — the body contains `is-locked`,
   `is-drawing`, `is-hit`, `is-eliminated`, `GHOST`, and `lc-lock-tick`. And
   the locked plaque's markup contains **no** `data-card-id` — the lock tick is
   all a spectator may see.
4. `test_preview_shows_deck_stack_states` — the body contains `data-low`,
   `data-empty` and `RESHUFFLE`, and a `.lc-discard` with `data-count`.
5. `test_preview_shows_oversized_hand_split` — contains `+5` (bob's 12-card
   `n − 7`) and `+23` (the 30-card sample), and the `n = 8` sample has exactly
   8 `data-size="strip"` occurrences.
6. `test_preview_tab_order_is_fixed` — `HAND` before `TABLE` before `LOG` in the
   source (assert by `find()` index) in **both** tab-row copies. F.1's rule that
   the active tab is never hoisted.
7. `test_preview_has_the_felt_and_all_three_flight_directions` — contains
   `id="lc-felt"`, and `data-replay="draw"`, `data-replay="play"`,
   `data-replay="discard"`.
8. `test_preview_script_delegates_to_the_motion_library` — the page's inline
   script contains `DOMContentLoaded`, `htmx:afterSwap`, `lcFlight` and
   `lcAnchor`, and the page does **not** contain `@keyframes` or
   `getBoundingClientRect`. Animation logic belongs to `lc_motion.js`, not to
   the page that demonstrates it — that is the whole claim of a "plug-and-play"
   library, and this is the only mechanical way to hold it.

- [ ] **Step 7: Commit**

```bash
git add drinkinggame/src/lc_preview.rs drinkinggame/templates/lc_preview.html \
        drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(drinks): preview the scene, table components, plaque states and replayable flights"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

> ### Browser checkpoint 2 — after Task 3, before the final review
>
> Reload `http://localhost:3001/lastcall/preview`.
>
> 1. The felt reads as a table: the radial gradient is visible, the 11px rail
>    surrounds it, the inner hairline ellipse sits 56px in, and the shadow stack
>    lifts it off the page. No seats on it — that is Plan B.
> 2. **The anchor board is all ✓.** Any ✗ means slice 3 would have to rewrite a
>    template to fire a flight at that target.
> 3. Every plaque state is visible together: idle, locked (violet tick, no
>    cards), drawing (the deck rule pulsing), eliminated (40%, `GHOST`), and the
>    two-deck plaque with its rule split 50/50 and its strip alternating beer
>    and liquor backs. Press REPLAY on **hit**: the plaque shakes 4px and the HP
>    flashes rose, once, and settles.
> 4. Deck stacks: Wine's `4` is amber, Liquor reads `RESHUFFLE`, the discard is
>    dashed with a neutral count. Bob's oversized hand reads 7 backs + `+5`.
> 5. Shell chrome renders in F.1's fixed order inside a 390×844 frame. Both
>    tab-row copies keep HAND / TABLE / LOG in order; only colour and the 2px
>    underline move, and the underline colour is per-tab (HAND violet, TABLE
>    azure, LOG grey). All six phase banners are visible together: Draw and Deal
>    amber, Diplomacy mint, Lock violet, Reveal azure, Resolve rose.
> 6. Press REPLAY on each flight direction. Each fires **once** and the node is
>    gone afterwards — check the Elements panel: `#lc-flights` is empty at rest
>    and never accumulates nodes, however many times you press. Press BURST:
>    seven cards leave the deck staggered, reading as a burst, not a blur.
> 7. **Turn on "Emulate CSS prefers-reduced-motion: reduce" in devtools and
>    press every REPLAY again.** Nothing animates, nothing is left behind in
>    `#lc-flights`, and the console stays clean. This is the accessibility
>    requirement and the one nobody re-checks later.
> 8. Resize narrow. Nothing overflows horizontally — a reference you have to
>    scroll sideways is a bad one.
>
> Then run the plan-end whole-diff review on the most capable model — every task
> in this plan is Class A or B and carried no per-task reviewer, so this single
> review covers all three. Anything the gallery reveals about a **Plan A** token
> or text rule gets corrected now, before Plan A2 and Plan B build against it:
> that is the reason this plan sits where it does in the order.

---

## Before this plan is done

- Every task is Class A or B, and every acceptance is a real command. **No task
  in this plan is Class C** — every Class C task in the slice lives in Plan A2.
- **No component and no fixture was authored here.** Every card, plaque, hand
  strip, deck stack and discard slot is a Plan A builder called as-is, and every
  fixture is Plan A's `preview_state()`. The only markup this plan originates is
  `#lc-felt`, `#lc-flights`, the gallery chrome and the static F.1 shell sample.
  A selector or a variant this plan wanted and could not find was a bug report
  against Plan A.
- **No `hx-post`, `hx-get` or `onclick` appears anywhere in this plan's output.**
  The §7.8 contract is structure, never behaviour; interactions are slice 2 and
  3. Task 2's `test_preview_has_no_style_element_and_no_behaviour` holds that
  line mechanically. The static setup-form chrome carries no `action`.
- **All fourteen `data-flight-anchor` names resolve on the preview page**, proven
  by Task 3's `test_preview_resolves_every_motion_anchor`. This is the only place
  in the series where the whole set is provable at once.
- `scripts/verify.sh` now runs `node --check` over `drinkinggame/assets/*.js` as
  well as `static/*.js` — without that change this plan's JavaScript is unchecked
  by the gate.
- The stylesheet has exactly **one** `@media (prefers-reduced-motion: reduce)`
  block, and it stops the flights, the shake, the HP flash, the pulse, the banner
  cross-fade and the timer.
- No migration was written and `cargo sqlx prepare` was not run; neither is
  needed. Nothing in this plan reads or writes the database.
- Spec §2's "In" list maps as: the rest of (7) — Tasks 1–3. §7.7 is entirely
  this plan (Tasks 1+3); §7.6's felt is rendered here (Task 3) from Plan A's
  styling; §7.8.1's anchors are completed and proven here. (3), (4) and the rest
  of (7) were **Plan A**; (1), (2), (5), (6), (8) are **Plan A2**; (9), (10) are
  **Plan B**.
- `/lastcall/preview` is registered, public, and covered by tests that run with
  no session. It is permanent — later slices extend it; nothing removes it.
