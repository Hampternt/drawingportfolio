# Last Call — Plan B: the felt surfaces

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Put Plan A's table components onto a felt — the F.2 spectator big
screen and the F.3 phone mini table — both driven by one seat-ring layout module
and the SSE pair Plan A2 already publishes.

**Architecture:** One pure geometry module (`lc_layout.rs`) owns seat placement
as box-relative percentages, so the same table serves a 1920×1080 TV and a 466px
phone pane without `transform: scale()` and without a media query. Two
assemblers in `lc_render.rs` consume it: `lc_screen_panel(&PublicView)` for the
big screen and `lc_mini_table(&PublicView, me_seat)` for the phone. The big
screen is a new page, `lc_screen.html`, reached by a kind branch in the existing
`/room/{code}/screen` handler and repainted from an extended `LcPublic` frame —
no new SSE event, no new publish, no change to frame counts. The phone's table is
**rotated so the viewer sits at bottom-centre**, which is per-viewer data, so it
is fetched over `GET /room/{code}/lastcall/table` exactly as the private hand is,
not broadcast.

**Slice:** When this plan is done, a Last Call game has a table you can look at:
a spectator opens `/room/{CODE}/screen` and sees seats on the felt with live HP,
hand sizes, deck counts and the phase banner; every player's TABLE tab shows the
same state at phone scale with themselves at the bottom. The room's seat ceiling
is enforced, and a Last Call game can be ended without ending the night. Slice 2
(the hand group — HandWheel, ArmedColumn, CostRail) and slice 3 (the beat loop,
which is what makes any of these numbers change) pick up from here.

---

## Global Constraints

Every task's requirements implicitly include this section.

### The component boundary — the single most important rule in this plan

**Plan B assembles Plan A's components. It authors none.** Spec §7.6: *"a
component renders from its own data (Plan A); its placement depends on table
state (Plan B)."* The plaque, hand strip, deck stack, discard slot, card
primitives, banner and beat timer all exist. If a task finds itself writing a new
`.lc-*` component builder, it has crossed the line and must stop and report
rather than build.

Three consequences, all already adjudicated — **do not re-litigate any of them:**

- **The plaque is 204px, as Plan A shipped it.** `Game UI.dc.html`'s v2 big-screen
  mockup draws it at 196px, and Game UI outranks the Module Spec on pixels. It is
  still not Plan B's to change: re-rendering a shipped component is authoring.
  Recorded as a gap for whoever next revises `player_plaque`.
- **The right rail uses `deck_stack()` and `discard_slot()`,** Plan A's D.4
  components. The v2 mockup draws a compact deck *list row* instead (9px dot,
  name, `disc n`, big count). That is a sixth rendering of deck state that Plan A
  did not ship, and building it here would author a component. Recorded as a gap.
- **Beat hue on the big-screen banner comes from `lc_banner()`'s existing
  markup**, resized by CSS. `lc_banner` is authored at phone scale; the big screen
  scales it in a `body.lc-screen` block. Scoping a shipped component is
  assembling; forking it is authoring.

### What is explicitly NOT in this plan

The v2 mockup shows more than this slice owns. Building any of it is scope
failure, not initiative:

| In the mockup | Whose it is |
| --- | --- |
| Left-rail **Event · this round** / **Next round** cards | content systems (later slice 3, "events") |
| **Quests in play** block | same |
| Card art on played cards | never designed — Module Spec G, "four screens still to design" |
| Played cards lying on the felt in front of each seat | slice 3, the loop. Spec §3.4.1 **binds** this: nothing may enter `plays` before it is revealable |
| A resolution plate in the centre of the felt | slice 3 |
| The `−3` damage badge on a plaque | slice 3 |
| LIST / FELT toggle | a later roster concept, not F.2 |

The centre of the felt is **empty** in this plan. That is correct, not unfinished.

### Geometry — the seat ring, measured from the bundle

Derived from `Game UI.dc.html`'s "Big screen v2" (line 176), whose centre column
is 1320×992. Its felt is inset 40 horizontal / 36 vertical; the second hairline
ellipse is inset a further 52 — `left:92px; top:88px; right:92px; bottom:88px` —
giving semi-axes **a = 568, b = 408** about centre **(660, 496)**.

The seven authored seat centres fit that inner ellipse:

| Seat centre | `(dx/a)² + (dy/b)²` | parametric angle |
| --- | --- | --- |
| (660, 842) bottom | **0.719** | 90.0° |
| (268, 798) | 1.024 | 133.0° |
| (118, 472) | 0.914 | 183.5° |
| (348, 152) | 1.013 | 236.9° |
| (980, 150) | 1.037 | 303.6° |
| (1202, 472) | 0.914 | 356.5° |
| (1052, 798) | 1.024 | 47.0° |

Six of seven land within 4% of the ring. **The bottom seat is deliberately pulled
inward** to r ≈ 0.85 — D.2's *"the local player is always nearest the viewer."*
The angles are authored, not evenly stepped: the widest gap straddles 270°,
leaving **top-centre empty**, which is what keeps a 7-seat table from putting a
plaque directly behind the centre of the felt.

**So: the n = 7 row is transcribed from the bundle. Every other row is generated**
by even parametric spacing on the same ellipse — `t_k = 90° + k·(360/n)`, seat 0
at bottom-centre, increasing `t` moving clockwise on screen. All values are
percentages of the ring box, so the ellipse follows its container's aspect ratio;
that is what lets one table serve a landscape TV and a portrait phone pane. The
exact table is in Task 1.

### Motion, scene roots and ids

- **`lc_screen.html` is a third scene root and needs `position: relative`.** So
  does the phone TABLE pane if it hosts flights. `#lc-flights` is
  `position: absolute; inset: 0; overflow: hidden`; without a positioned
  ancestor it forms its containing block against the viewport and clips every
  flight past the first screenful — nodes created with correct deltas and never
  rendered. `body.lc-preview` (Plan A-vis) and `body.lc` (Plan A2) each carry it
  under a test; the new roots get the same, each pinned by its own test.
- **`window.lcAnchor(name)` returns the first match.** Plan A-vis's preview page
  has duplicate anchors by design and the STATUS card says **Plan B must not
  inherit the pattern.** Every seat anchor is seat-indexed:
  `data-flight-anchor="seat-{seat}"`, and deck anchors are
  `data-flight-anchor="deck-{slug}"` / `"discard"`. A test asserts no duplicate
  `id` or `data-flight-anchor` value on any page that carries both a felt and a
  hand.
- No new keyframes. `lc-shake`, `lc-hp-flash`, `lc-pulse`, `lc-fly`, `lc-dot`,
  `lc-banner`, `lc-timer` all exist. Plan B fires none of them either — the beat
  loop that would is slice 3.
- **One `@media (prefers-reduced-motion: reduce)` block in `lastcall.css`, ever.**
  A second silently overrides half the first. If a rule belongs under reduced
  motion, it goes *inside* the existing block.

### Broadcast rules, carried from Plan A2's reviews

- **`broadcast_lc` has no await points**, so both publishes are synchronous under
  the room guard — a stronger property than "the lock is held," because there is
  no suspension point for another task's broadcast to interleave at. **Preserve
  it.** Any new work inside `broadcast_lc` must be synchronous string building.
- **Publish order is `room` → `lcpublic` → `lctick`.** Tests that assert on
  frames must **filter by event name, never index positionally.**
- **A named SSE event with an empty data buffer is silently dropped** by
  EventSource. Never publish an empty payload.
- **Ask "who is subscribed to this, and what are they currently looking at?"**
  before choosing which frames to publish. A shell-only broadcast reaches nobody
  who is not yet on that shell — the bug that made A2's START a visual no-op.

### Style and asset rules

- All CSS goes in `drinkinggame/assets/lastcall.css` under a named section
  comment. No `<style>` blocks in templates. No nested `/* */` — `verify.sh`
  guards it, and a nested marker once silently dropped `.card-big`.
- Renderers emit **deck class names, never hex.** A test rejects `#` in renderer
  output; `lc_render.rs`'s fourteen-builder no-hex loop must be extended to cover
  every builder this plan adds.
- Deck-tinted borders use `--lc-ink-59 / -66 / -80 / -99`. Never `<fill>22`.
- Nothing under 18px on the big screen (F.2, read from 3–4 metres).
- `lc_screen.html` does **not** extend anything — the crate's templates are a
  recorded exception to the repo's base.html rule.

### Verification

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted
in the report. Never a bare `cargo test`: the root `Cargo.toml` is both a package
and the workspace root, so `cargo test` runs 52 of 327 and silently skips the
whole `drinkinggame` crate.

**Baseline before Task 1:** green, **327 tests**, **17 distinct** clippy warnings.
`cargo clippy --workspace --all-targets` prints 19 `warning:` lines — two are
per-target rollup summaries. Compare against 17.

**No `cargo sqlx prepare`.** `drinkinggame` uses runtime-checked sqlx queries and
has no `.sqlx` cache entries; the offline-cache ritual is portfolio-only. There is
no migration in this plan — `games.kind` already carries `last_call` and
`state_json` already exists.

**Browser checkpoints:** after Task 4 (the big screen is the visual layer) and
before the final review. Not per task.

---

### Task 1: Seat-ring geometry

**Class:** B (logic, tests specified below)

**Why this class:** it is a pure function from a seat count to a table of
constants. The acceptance is a unit test with every expected value written out
below — the tests are the spec.

**Files:**
- Create: `drinkinggame/src/lc_layout.rs`
- Modify: `drinkinggame/src/lib.rs` (add `pub mod lc_layout;` beside `lc_render`)
- Test: `drinkinggame/src/lc_layout.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::last_call::MAX_SEATS` (`pub const MAX_SEATS: usize = 8`)
- Produces:
  ```rust
  /// A seat's centre, as percentages of the ring box: (left%, top%).
  pub type SeatPos = (f32, f32);

  /// Seat placements for a table of `n`, seat 0 first, clockwise.
  /// Returns `&[]` for n == 0. Clamps n > MAX_SEATS to the MAX_SEATS row.
  pub fn seat_positions(n: usize) -> &'static [SeatPos];

  /// Which ring slot a seat occupies for a given viewer. `me` is the
  /// viewer's own seat; `None` (a spectator, or a member who is not seated)
  /// is identity. Rotates so the viewer always lands on slot 0 —
  /// bottom-centre.
  pub fn view_index(seat: usize, me: Option<usize>, n: usize) -> usize;
  ```

- [ ] **Step 1: Write the table**

Percentages of the ring box, `(left, top)`. The `n = 7` row is transcribed from
`Game UI.dc.html`'s big-screen v2 (see Global Constraints); every other row is
`t_k = 90° + k·(360/n)` on the same ellipse, `a = 43.0%`, `b = 41.1%`, centre
`(50, 50)`.

```rust
/// Indexed by `n - 2`. Seat 0 is bottom-centre; the list runs clockwise.
///
/// The n == 7 row is transcribed from the design bundle rather than
/// generated: its angles are authored so that top-centre stays empty, and
/// the bottom seat is pulled inward to r ~= 0.85 (D.2, "the local player is
/// always nearest the viewer"). Six of its seven seats sit within 4% of the
/// felt's inner hairline ellipse, which is what makes the generated rows
/// consistent with it.
const RING: [&[SeatPos]; 7] = [
    // n = 2
    &[(50.0, 91.1), (50.0, 8.9)],
    // n = 3
    &[(50.0, 91.1), (12.8, 29.4), (87.2, 29.4)],
    // n = 4
    &[(50.0, 91.1), (7.0, 50.0), (50.0, 8.9), (93.0, 50.0)],
    // n = 5
    &[(50.0, 91.1), (9.1, 62.7), (24.7, 16.7), (75.3, 16.7), (90.9, 62.7)],
    // n = 6
    &[
        (50.0, 91.1), (12.8, 70.5), (12.8, 29.4),
        (50.0, 8.9), (87.2, 29.4), (87.2, 70.5),
    ],
    // n = 7 — authored, from the bundle
    &[
        (50.0, 84.9), (20.3, 80.4), (8.9, 47.6),
        (26.4, 15.3), (74.2, 15.1), (91.1, 47.6), (79.7, 80.4),
    ],
    // n = 8 — D.2's "8 compresses the two bottom positions"
    &[
        (50.0, 91.1), (19.6, 79.1), (7.0, 50.0), (19.6, 20.9),
        (50.0, 8.9), (80.4, 20.9), (93.0, 50.0), (80.4, 79.1),
    ],
];
```

- [ ] **Step 2: Write the two functions**

`seat_positions(n)`: `n == 0` or `n == 1` → the `n.max(2)` row is wrong; return
`&[]` for 0 and the `n = 2` row's first element only for 1 — simplest correct
form is `match n { 0 => &[], 1 => &RING[0][..1], _ => RING[n.min(MAX_SEATS) - 2] }`.
A one-player table is not a legal game (start requires ≥ 2) but the renderer must
not panic on one.

`view_index(seat, me, n)`: `(seat + n - me) % n` when `me` is `Some` and `n > 0`,
otherwise `seat`. Guard `n == 0` before the modulo.

- [ ] **Step 3: Write the tests**

```rust
#[test]
fn test_seat_zero_is_bottom_centre_for_every_count() {
    // Every table puts seat 0 at the bottom of the ring: that is the
    // anchor the phone's rotation depends on.
    for n in 2..=MAX_SEATS {
        let p = seat_positions(n)[0];
        assert_eq!(p.0, 50.0, "n={n} seat 0 must be horizontally centred");
        assert!(p.1 > 80.0, "n={n} seat 0 must sit low, got {}", p.1);
    }
}

#[test]
fn test_row_lengths_match_seat_count() {
    for n in 2..=MAX_SEATS {
        assert_eq!(seat_positions(n).len(), n);
    }
}

#[test]
fn test_seven_row_is_the_authored_bundle_geometry() {
    // Transcribed from Game UI.dc.html's big-screen v2. If this test is
    // ever "fixed" to match a formula, the ring stops matching the design.
    assert_eq!(
        seat_positions(7),
        &[
            (50.0, 84.9), (20.3, 80.4), (8.9, 47.6),
            (26.4, 15.3), (74.2, 15.1), (91.1, 47.6), (79.7, 80.4),
        ]
    );
}

#[test]
fn test_every_seat_is_inside_the_box() {
    for n in 2..=MAX_SEATS {
        for (i, (l, t)) in seat_positions(n).iter().enumerate() {
            assert!((0.0..=100.0).contains(l), "n={n} seat {i} left {l}");
            assert!((0.0..=100.0).contains(t), "n={n} seat {i} top {t}");
        }
    }
}

#[test]
fn test_over_max_seats_clamps_rather_than_panicking() {
    // add_player's ceiling is Task 6's; a renderer handed a stale
    // oversized state must still render rather than index out of bounds.
    assert_eq!(seat_positions(99).len(), MAX_SEATS);
}

#[test]
fn test_view_index_puts_the_viewer_at_the_bottom() {
    // 5 seats, viewer in seat 3 -> viewer occupies slot 0.
    assert_eq!(view_index(3, Some(3), 5), 0);
    assert_eq!(view_index(4, Some(3), 5), 1);
    assert_eq!(view_index(0, Some(3), 5), 2);
    assert_eq!(view_index(1, Some(3), 5), 3);
    assert_eq!(view_index(2, Some(3), 5), 4);
}

#[test]
fn test_view_index_is_identity_for_a_spectator() {
    // The big screen and an unseated member both pass None.
    for seat in 0..6 {
        assert_eq!(view_index(seat, None, 6), seat);
    }
}

#[test]
fn test_view_index_is_a_permutation() {
    // No two seats may collide on one ring slot — a collision would stack
    // two plaques at identical coordinates and silently hide one.
    for n in 2..=MAX_SEATS {
        for me in 0..n {
            let mut slots: Vec<usize> =
                (0..n).map(|s| view_index(s, Some(me), n)).collect();
            slots.sort_unstable();
            assert_eq!(slots, (0..n).collect::<Vec<_>>(), "n={n} me={me}");
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_layout.rs drinkinggame/src/lib.rs
git commit -m "feat(lastcall): seat-ring geometry as box-relative percentages"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Felt, ring and shell CSS

**Class:** A (compiler/lint-gated)

**Why this class:** CSS plus one `node --check`-covered asset. `verify.sh` runs
`cargo fmt --check`, clippy, the workspace suite, the nested-comment guard over
`assets/*.css` and `node --check` over `assets/*.js`. Nothing here encodes a
decision a test could not see, and the visual result is checked by a human at the
Task 4 browser checkpoint.

**Files:**
- Modify: `drinkinggame/assets/lastcall.css` (append new sections; the existing
  `#lc-felt` block at ~line 124 stays as-is)
- Test: `drinkinggame/tests/http.rs` (asset-shape assertions)

**Interfaces:**
- Consumes: existing tokens `--lc-page --lc-device --lc-panel --lc-panel-alt
  --lc-raised --lc-focused --lc-hair --lc-hair-strong --lc-rail`, the deck ramps,
  the beat hues, and the shipped component classes `.lc-plaque .lc-handstrip
  .lc-deckstack .lc-discard .lc-banner .lc-timer`.
- Produces: class contract consumed by Tasks 3 and 4 —
  `body.lc-screen`, `.lc-screen-head`, `.lc-screen-grid`, `.lc-rail`,
  `.lc-rail-left`, `.lc-rail-right`, `.lc-rail-kicker`, `.lc-seatorder`,
  `.lc-stage`, `.lc-ring`, `.lc-seat`, `.lc-mini`, `.lc-mini-ring`,
  `.lc-mini-chip`, `.lc-mini-centre`.

- [ ] **Step 1: Section comment and the big-screen shell**

Append under `/* Plan B — felt surfaces: F.2 big screen, D.2 seat ring, F.3 mini
table */`.

```css
/* F.2 BigScreenShell — 1920x1080, a display and never an input. Fixed
   88px header, then the felt filling everything below (Module Spec F.2).
   No hover states, no focus rings, no controls: every affordance belongs
   to a phone. */
body.lc-screen {
  position: relative;          /* third scene root — #lc-flights needs it */
  margin: 0;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--lc-page);
  color: var(--lc-text);
  font-family: 'Space Grotesk', sans-serif;
  cursor: none;                /* it is a display */
}
.lc-screen-head {
  height: 88px;
  display: flex;
  align-items: center;
  padding: 0 40px;
  gap: 36px;
  border-bottom: 1px solid var(--lc-hair);
  flex: 0 0 88px;
}
.lc-screen-mark {
  font: 900 28px 'Archivo', sans-serif;
  letter-spacing: -.035em;
  color: var(--lc-text);
  text-transform: lowercase;
}
.lc-screen-mark span { color: #B48EF7; }
.lc-screen-meta { display: flex; align-items: baseline; gap: 10px; }
.lc-screen-meta .lc-kicker {
  font: 600 11px 'Space Grotesk', sans-serif;
  letter-spacing: .16em;
  color: var(--lc-label);
  text-transform: uppercase;
}
.lc-screen-code {
  font: 500 22px 'IBM Plex Mono', ui-monospace, monospace;
  color: var(--lc-text);
  letter-spacing: .06em;
}
.lc-screen-round { font: 900 26px 'Archivo', sans-serif; color: var(--lc-text); }

/* The banner is Plan A's component, scaled up for a 3-4 metre read.
   Scoping a shipped component is assembling; forking it would be
   authoring, which this plan may not do. */
body.lc-screen .lc-banner { flex: 1; text-align: center; }
body.lc-screen .lc-banner-beat { font-size: 46px; line-height: 1; }
body.lc-screen .lc-banner-meta { font-size: 12px; letter-spacing: .2em; }
body.lc-screen .lc-timer { height: 3px; }
```

**Note for the implementer:** `.lc-banner-beat` and `.lc-banner-meta` are the
class names `lc_banner()` actually emits (verified against
`lc_render.rs::lc_banner` while this plan was written), and its root carries
`id="lc-banner"` plus `.lc-beat-{hue}`. Scope against those; do not rename the
component's markup.

- [ ] **Step 2: The three-column grid and the rails**

```css
/* 300px | felt | 300px, per the bundle's big-screen v2. The felt keeps the
   authored proportions the seat ring was measured against only if the
   centre column keeps roughly this width. */
.lc-screen-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 300px 1fr 300px;
  overflow: hidden;
}
.lc-rail { padding: 24px; display: flex; flex-direction: column; gap: 20px; }
.lc-rail-left  { border-right: 1px solid var(--lc-hair); }
.lc-rail-right { border-left: 1px solid var(--lc-hair); }
.lc-rail-kicker {
  font: 600 12px 'Space Grotesk', sans-serif;
  letter-spacing: .18em;
  color: var(--lc-label);
  text-transform: uppercase;
  margin-bottom: 11px;
}
.lc-seatorder { display: flex; flex-direction: column; gap: 6px; }
.lc-seatorder-row { display: flex; align-items: center; gap: 9px; }
.lc-seatorder-n {
  font: 500 12px 'IBM Plex Mono', ui-monospace, monospace;
  color: var(--lc-label);
  width: 16px;
}
.lc-seatorder-name {
  font: 800 18px 'Archivo', sans-serif;   /* F.2 floor: nothing under 18px */
  color: var(--lc-body);
  white-space: nowrap;
}
.lc-seatorder-row[data-first] .lc-seatorder-name { color: #4FD6A8; }
.lc-seatorder-row[data-out] { opacity: .4; }

/* Five deck stacks and the discard, two across. Plan A's D.4 components. */
.lc-rail-decks {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 14px;
  justify-items: center;
}
```

- [ ] **Step 3: The stage and the seat ring**

> **ERRATUM (2026-08-11, whole-plan review).** The two `inset: 8.9% 7.0%`
> declarations prescribed below are **wrong and were corrected in the shipped
> CSS to `inset: 0`**. Task 1's seat table is *stage*-relative, not
> ring-box-relative: `a = 43.0%` / `b = 41.1%` are `568/1320` and `408/992`
> measured against the whole 1320×992 stage, so its extremes already **are**
> `7.0 / 93.0` and `8.9 / 91.1`. Insetting the ring box by those same
> percentages applies the ellipse twice and shrinks the ring to 86% × 82% of
> design — the bottom seat 61px high. Wherever this plan says "percentages of
> the ring box", read "of the stage", and make the ring box span the stage.
> Left in place rather than silently rewritten, because the wording here is
> what caused the defect and the next plan should recognise the shape.

```css
/* The stage is the felt's positioning context and the ring box the
   percentages in lc_layout are relative to. Felt inset 40 x 36 from the
   stage, per D.2. */
.lc-stage { position: relative; overflow: hidden; }
.lc-stage > #lc-felt {
  position: absolute;
  left: 40px; top: 36px; right: 40px; bottom: 36px;
}
/* The ring box is the felt's inner hairline ellipse — 92px / 88px in from
   the stage on a 1320x992 centre column. Expressed as a fraction, not those
   pixels: the seat table is percentages OF THIS BOX, so a pixel inset would
   silently change the ring's proportions at any width but 1920, and the
   mini table (which uses the same fractions) would stop matching. */
.lc-ring { position: absolute; inset: 8.9% 7.0%; }
/* A seat is centred on its own midpoint, so a plaque of any width lands
   with its centre on the ellipse (D.2). */
.lc-seat {
  position: absolute;
  transform: translate(-50%, -50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
```

- [ ] **Step 4: F.3 mini table**

```css
/* F.3 MiniTable — the same felt at 466px tall for a room with no TV.
   Same state, same events, one order of magnitude less detail: seats
   become name/HP chips and flights become CardDots. It is its own scene
   root (it hosts CardDot flights), so it carries position: relative too. */
.lc-mini {
  position: relative;
  height: 466px;
  margin: 0 0 14px;
}
.lc-mini > #lc-felt { position: absolute; inset: 0; }
/* Proportional, not pixel: the seat table is percentages OF THE RING BOX,
   so the two surfaces only place seats at the same proportional distance
   from the felt edge if both ring boxes are inset by the same fraction.
   8.9% / 7.0% is the big screen's 88px / 92px over its 992 x 1320 stage. */
.lc-mini-ring { position: absolute; inset: 8.9% 7.0%; }
.lc-mini-chip {
  position: absolute;
  transform: translate(-50%, -50%);
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 5px 9px;
  border-radius: 7px;
  background: var(--lc-panel);
  border: 1px solid var(--lc-hair);
  white-space: nowrap;
}
.lc-mini-name { font: 800 12px 'Archivo', sans-serif; color: var(--lc-body); }
.lc-mini-hp   { font: 900 15px 'Archivo', sans-serif; color: var(--lc-text); }
.lc-mini-chip[data-locked] { border-color: #B48EF7; }
.lc-mini-chip[data-out] { opacity: .4; }
.lc-mini-chip[data-me] { border-color: var(--lc-hair-strong); }
/* Centre column: a 46x62 draw pile above the event / quest / discard rows
   (Module Spec F.3). Only the discard row carries a real count in this
   slice; event and quest are content-systems work and render as a dash. */
.lc-mini-centre {
  position: absolute;
  left: 50%; top: 50%;
  transform: translate(-50%, -50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 9px;
}
.lc-mini-rows { display: flex; flex-direction: column; gap: 5px; }
.lc-mini-row { display: flex; align-items: center; gap: 7px; }
.lc-mini-row-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--lc-hair-strong); }
.lc-mini-row-label {
  font: 700 9px 'Space Grotesk', sans-serif;
  letter-spacing: .14em;
  color: var(--lc-label);
  text-transform: uppercase;
}
.lc-mini-row-n { font: 900 13px 'Archivo', sans-serif; color: var(--lc-body); }
```

**Do not add a second `@media (prefers-reduced-motion: reduce)` block.** Nothing
in this task animates; if that changes, extend the one that exists.

- [ ] **Step 5: Pin the scene roots**

Add to `drinkinggame/tests/http.rs`:

```rust
#[test]
fn test_new_scene_roots_are_positioned() {
    // #lc-flights is position:absolute; inset:0; overflow:hidden. Without a
    // positioned ancestor it forms its containing block against the viewport
    // and clips every flight past the first screenful — nodes created with
    // correct deltas and never rendered. Invisible to any test that only
    // checks the flight lifecycle, which is why it is asserted here.
    let css = include_str!("../assets/lastcall.css");
    for root in ["body.lc-screen", ".lc-mini"] {
        let block = css
            .split_once(root)
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(b, _)| b)
            .unwrap_or_else(|| panic!("{root} not found in lastcall.css"));
        assert!(
            block.contains("position: relative"),
            "{root} must be positioned — it is a scene root for #lc-flights"
        );
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): felt stage, seat ring and mini-table CSS"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The two table assemblers

**Class:** B (logic, tests specified below)

**Why this class:** two pure `&PublicView -> String` functions over Plan A's
component builders and Task 1's table. Every claim about them — seat count,
rotation, anchor uniqueness, no hex, no interactivity — is a string assertion with
its expected value written below.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs`
- Test: `drinkinggame/src/lc_render.rs` (`#[cfg(test)] mod tests`, extending the
  existing module)

**Interfaces:**
- Consumes:
  ```rust
  crate::lc_layout::{seat_positions, view_index, SeatPos}
  crate::last_call::{PublicView, PublicSeat, Deck, Status}
  // Plan A components, used exactly as shipped:
  player_plaque(seat: &PublicSeat) -> String
  deck_stack(deck: Deck, count: u16) -> String
  discard_slot(count: usize) -> String
  lc_banner(view: &PublicView) -> String
  ```
- Produces:
  ```rust
  /// F.2 big-screen body: header meta is the template's, this is the grid.
  /// Absolute seat order — a spectator has no seat, so no rotation.
  pub fn lc_screen_panel(view: &PublicView) -> String;

  /// F.3 phone mini table, rotated so `me` sits at bottom-centre.
  /// `me` is the viewer's own seat, or None for a member who is not seated.
  pub fn lc_mini_table(view: &PublicView, me: Option<usize>) -> String;
  ```

  **Two mechanical rules both assemblers must follow, because the tests below
  depend on them:**

  - **Percentages are emitted with `{}`, never a precision specifier.** `{:.1}`
    would render `50.0` as `50.0` in the renderer and `50` in a test needle built
    from the same tuple, and the failure would read as a geometry bug rather than
    a formatting one.
  - **Iterate `view.seats.iter().zip(seat_positions(n))`, never index the
    position table by seat number.** Task 1's `seat_positions` clamps a stale
    oversized state to the `MAX_SEATS` row, so indexing by seat would panic or
    stack two plaques at one coordinate; zipping renders short instead. The
    ceiling itself is Task 6's, but the assembler must not be the thing that
    crashes when it is missing.

- [ ] **Step 1: `lc_screen_panel`**

Build, in order: `.lc-screen-grid` wrapping

1. `.lc-rail.lc-rail-left` — kicker `SEAT ORDER`, then one `.lc-seatorder-row`
   per seat in **absolute** seat order: `.lc-seatorder-n` = `seat + 1`,
   `.lc-seatorder-name` = the name uppercased. Mark `data-first` on
   `view.first_seat` and `data-out` on any seat whose `status` is not the live
   one (use `Status::slug()`, do not hardcode strings).
2. `.lc-stage` containing `<div id="lc-felt"></div>`, then `.lc-ring` holding one
   `.lc-seat` per seat, then `<div id="lc-flights"></div>` **last** so the flight
   layer sits above the felt and below nothing.
3. `.lc-rail.lc-rail-right` — kicker `DECKS LEFT`, then `.lc-rail-decks` holding
   `deck_stack(deck, count)` for each `(deck, count)` in `view.deck_counts` and
   `discard_slot(view.discard_count)` last.

Each `.lc-seat` carries its position inline and its anchor:

```rust
format!(
    r#"<div class="lc-seat" style="left:{l}%;top:{t}%" data-seat="{seat}" data-flight-anchor="seat-{seat}">{plaque}</div>"#,
)
```

`(l, t)` comes from `seat_positions(view.seats.len())[view_index(seat, None, n)]`
— identity rotation, spelled out rather than indexing directly, so the screen and
the phone read as one mechanism with one argument different.

The inline `left`/`top` percentages are the one place this plan emits a `style`
attribute. That is deliberate and consistent with `beat_timer()`, which already
does it: a value computed per render cannot live in a stylesheet. **It is a
percentage, never a hex** — the no-hex guard still applies.

- [ ] **Step 2: `lc_mini_table`**

Same ring, phone scale. `.lc-mini` wrapping `#lc-felt`, `.lc-mini-ring` holding
one `.lc-mini-chip` per seat, `.lc-mini-centre`, and `#lc-flights` last.

Chip contents: `.lc-mini-name` (name, uppercased) and `.lc-mini-hp` (`hp`).
Attributes: `data-seat`, `data-flight-anchor="seat-{seat}"`, plus `data-locked`
when locked, `data-out` when eliminated, and `data-me` when `Some(seat) == me`.

Position comes from `seat_positions(n)[view_index(seat, me, n)]` — **this is the
whole point of the route in Task 5.** The viewer is always at slot 0,
bottom-centre.

`.lc-mini-centre`: a `card_back(deck, BackSize::Pile)` draw pile — `Pile` is the
46×62 rendering F.3 asks for — above three `.lc-mini-row`s. `EVENT` and `QUEST`
render `—` (content systems, not this slice); `DISCARD` renders
`view.discard_count`. Do not add a `BackSize` variant.

- [ ] **Step 3: Tests**

```rust
fn ring_fixture(n: usize) -> PublicView { /* n seats, ids 1..=n, hp 15 */ }

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
        r#"style="left:{}%;top:{}%" data-seat="0""#, bottom.0, bottom.1
    )));
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
                r#"style="left:{}%;top:{}%" data-seat="{me}""#, bottom.0, bottom.1
            )),
            "seat {me} should hold the bottom slot in its own view"
        );
    }
}

#[test]
fn test_mini_table_for_a_spectator_is_unrotated() {
    let view = ring_fixture(4);
    assert_eq!(lc_mini_table(&view, None), lc_mini_table(&view, Some(0)));
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
    for html in [lc_screen_panel(&ring_fixture(7)), lc_mini_table(&ring_fixture(7), Some(3))] {
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
    for banned in ["hx-post", "hx-get", "hx-swap", "onclick", "href", "<button", "<form", "<input"] {
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
```

- [ ] **Step 4: Extend the existing guards**

`lc_render.rs` already has a loop that runs a no-hex check and a
forbidden-attribute check over a fourteen-builder array. **Add
`lc_screen_panel(&view)` and `lc_mini_table(&view, Some(0))` to that same array**
rather than writing a parallel check — the previous plan's review found the guard
covering 6 of 14 builders because it had drifted from the list.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): assemble the big-screen and mini-table felts"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The big screen — page, kind branch and the live handoff

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** it branches a route shared by Ring of Fire and 3 Man on
active-game kind, and it edits `screen.html`, the spectator page all three games
serve. The same reasoning made Plan A2's room-entry redirect Class C. It also
changes what an existing SSE frame carries, and "who is subscribed to this and
what are they currently looking at" is not a question any command answers.

**Files:**
- Create: `drinkinggame/templates/lc_screen.html`
- Modify: `drinkinggame/src/routes.rs` (`screen_page`, ~line 644; the `.route`
  table needs no new entry), `drinkinggame/src/render.rs`
  (`lc_screen_placeholder`, ~line 397), `drinkinggame/src/lc_render.rs`
  (`lc_public_panel`, ~line 380), `drinkinggame/templates/screen.html`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `lc_render::{lc_screen_panel, lc_banner}`,
  `last_call::LastCallState::{from_json, public_view}`,
  `db::get_active_game`, `crate::routes::request_origin` (for the QR/join copy if
  used — optional here)
- Produces:
  - `lc_screen.html` template struct `LcScreenTemplate { base_path, code, round,
    banner, panel }`
  - `lc_public_panel` gains a second `<template data-lc-screen>` block. **The
    frame count does not change** and neither does the event name.
  - `lc_screen_placeholder` output gains the marker attribute `data-lc-live`.

- [ ] **Step 1: Decide the handoff, then build it**

Two populations must move in opposite directions and neither can be reached by
adding a frame:

- A spectator already on `screen.html` when someone presses START on Last Call.
  `broadcast_game` publishes `Screen(lc_screen_placeholder(code))` to them. They
  need to end up on the Last Call screen.
- A spectator on `lc_screen.html` when the Last Call game ends but the room stays
  open. `end_game`-shaped broadcasts publish a `Screen` frame carrying the idle or
  game-over panel. They need to end up back on `screen.html`.

**One marker solves both.** `lc_screen_placeholder` — the `Screen` frame
published while a Last Call game is active — gains `data-lc-live`. Then:

- `screen.html`: on a `screen` event whose payload contains `data-lc-live`,
  `window.location.reload()`. Inert for Ring of Fire and 3 Man, which never
  publish that marker.
- `lc_screen.html`: on a `screen` event whose payload does **not** contain
  `data-lc-live`, `window.location.reload()`.

Verify while implementing, and state it in the report: `screen_panel_idle`,
`screen_panel_over`, `render::tm_screen_over` and the Ring of Fire screen panels
must all lack the marker. If any of them gains one later the handoff inverts, so
add a test that asserts exactly one screen-panel builder emits it.

- [ ] **Step 2: `lc_screen.html`**

Mirror `lc_room.html`'s head: `lastcall.css` and `lc_motion.js`, no `game.css`,
no base template (recorded crate exception). `<body class="lc-screen">`, then
`.lc-screen-head` (wordmark `last call<span>.</span>`, ROOM + code, ROUND +
number, `{{ banner|safe }}`), then `{{ panel|safe }}`.

Script: one `EventSource` on `BP + "/room/" + CODE + "/sse"` with three
listeners.

```js
es.addEventListener("lcpublic", e => {
  const box = document.createElement("div");
  box.innerHTML = e.data;
  const root = box.querySelector("[data-lc-public]");
  // Stale-drop, the same rule the phone uses: a frame older than the
  // newest seq we have seen would repaint an out-of-date table. Equal seq
  // is fine — a duplicate repaint is harmless.
  const seq = Number(root?.dataset.seq || 0);
  if (!root || seq < lcSeq) return;
  lcSeq = seq;
  const banner = box.querySelector("template[data-lc-banner]");
  if (banner) document.getElementById("lc-banner").outerHTML = banner.innerHTML;
  const panel = box.querySelector("template[data-lc-screen]");
  if (panel) document.getElementById("lc-screen-panel").innerHTML = panel.innerHTML;
});
es.addEventListener("screen", e => {
  // The Last Call game ended but the room did not: fall back to the
  // generic spectator screen.
  if (!e.data.includes("data-lc-live")) window.location.reload();
});
es.addEventListener("ended", () => { es.close(); window.location = BP + "/"; });
```

`lcSeq` is seeded from the server-rendered panel's `data-seq`, exactly as
`lc_room.html` seeds from `#lc-hand`.

There is no `lctick` listener: `lctick` exists to make a phone re-fetch its
private hand, and a spectator has none.

- [ ] **Step 3: Extend `lc_public_panel`**

```rust
pub fn lc_public_panel(view: &PublicView) -> String {
    format!(
        r#"<div data-lc-public data-seq="{seq}"><template data-lc-banner>{banner}</template><template data-lc-screen>{screen}</template></div>"#,
        seq = view.seq,
        banner = lc_banner(view),
        screen = lc_screen_panel(view),
    )
}
```

Two properties this must not break, both from Plan A2's reviews:

- **`broadcast_lc` still has no await points.** `lc_screen_panel` is a pure
  string build, so both publishes stay synchronous under the room guard — there
  is no suspension point for another task's broadcast to interleave at. Adding an
  `.await` anywhere in this path regresses `1e742d4`.
- **The frame count is unchanged.** A2's tests assert a five-frame versus
  four-frame SSE snapshot. Those tests must **filter by event name**, never index
  positionally; fix any that do while you are here and say so.

- [ ] **Step 4: The kind branch in `screen_page`**

In `screen_page` (`routes.rs` ~644), after the room lookup, read
`db::get_active_game`. When `game.kind == "last_call"`, parse the state, build
`LcScreenTemplate` from `lc_screen_panel(&view)` + `lc_banner(&view)` and return
it. Otherwise fall through to the existing `screen.html` path completely
unchanged — Ring of Fire and 3 Man must be byte-identical.

Follow `lc_page`'s shape for the parse; do **not** route it through `load_lc`,
which requires `PlayerSession` — the spectator screen is public and has no player.

- [ ] **Step 5: Tests in `tests/http.rs`**

```rust
#[tokio::test]
async fn test_screen_serves_the_last_call_felt_when_last_call_is_active() { /* body.lc-screen, lc-seat x n */ }

#[tokio::test]
async fn test_screen_is_unchanged_for_ring_of_fire_and_three_man() {
    // The regression this task's class exists for: two games that were
    // working before must render exactly what they rendered before.
}

#[tokio::test]
async fn test_the_last_call_screen_needs_no_session() {
    // A TV in the corner has no cookie. Request it with no Cookie header
    // at all and expect 200 plus a seat.
}

#[tokio::test]
async fn test_exactly_one_screen_panel_builder_marks_itself_live() {
    // The handoff inverts if a second one gains the marker.
}

#[tokio::test]
async fn test_lcpublic_carries_both_templates_and_the_frame_count_is_unchanged() {
    // Filter by event name. Never index positionally — publish order is
    // room -> lcpublic -> lctick and a new frame anywhere shifts indices.
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/templates/lc_screen.html drinkinggame/templates/screen.html \
        drinkinggame/src/routes.rs drinkinggame/src/render.rs \
        drinkinggame/src/lc_render.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): spectator big screen and the live-screen handoff"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 1 — after Task 4

The big screen is the visual layer of this plan; this is where a human looks at
it. Two browser profiles, `cargo run -p drinkinggame` on `:3001`.

1. Log in as two players, create a room, join from the second profile, press
   START on the Last Call card.
2. Open `http://localhost:3001/room/{CODE}/screen` — **in a third window with no
   session at all.** The felt fills below an 88px header; seats sit on the
   ellipse; HP and the beat name are the two things readable from across the
   room. Nothing under 18px.
3. Resize to 1920×1080. The three columns hold; the felt does not clip.
4. With the screen already open, end and restart the game: it must fall back to
   the generic screen and come back on its own, no manual reload.
5. Open the screen **before** pressing START, then press START. It must move to
   the Last Call felt by itself.
6. Start Ring of Fire and 3 Man and confirm their screens are exactly as before.

**Also owed from Plan A-vis — not Plan B acceptance, but a human is already in a
browser:** open `/lastcall/preview`, press every REPLAY and watch a flight
actually travel; then turn on devtools' *Emulate CSS `prefers-reduced-motion:
reduce`* and press them all again (expect no nodes, arrival still firing). Both
were verified structurally and never watched. Five minutes.

---

### Task 5: The phone TABLE tab

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** a per-viewer fragment route whose correctness claim is
*"which seat sits at the bottom is derived from the session cookie alone."* That
is the same property `lc_hand_handler` was Class C for, and it cannot be settled
by a command's output. The route takes no player identifier of any kind — no path
segment, no query parameter, no form field — so "can A fetch B's rotation?" is
unanswerable rather than guarded (spec §6.1).

**Files:**
- Modify: `drinkinggame/src/lc_routes.rs`, `drinkinggame/src/routes.rs` (route
  table, beside `/room/{code}/lastcall/hand`),
  `drinkinggame/templates/lc_room.html`
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `lc_render::lc_mini_table(&PublicView, Option<usize>)`,
  `lc_routes::load_lc`, `LastCallState::seat_of(player_id) -> Option<usize>`
- Produces:
  ```rust
  /// `GET /room/{code}/lastcall/table` — PER VIEWER.
  pub async fn lc_table_handler(
      State(state): State<GameState>,
      PlayerSession(player): PlayerSession,
      Path(code): Path<String>,
  ) -> axum::response::Response;
  ```
  Response body is `lc_mini_table(...)` wrapped in an element carrying
  `id="lc-table"` and `data-seq="{seq}"`.

- [ ] **Step 1: Why this is a fetch and not a broadcast — read before writing**

The mini table's data is entirely public. It is fetched anyway, because **the
layout is not public: it is viewer-relative.** D.2 requires the local player at
bottom-centre, so no two players see the same HTML. A `RoomHub` broadcast is one
fragment for the whole room, and the `personalize()` attribute contract is a
*visual hide* — it can dim or reveal an element, but it cannot re-position seven
plaques around an ellipse.

So the big screen (one absolute layout, no viewer) rides `lcpublic`, and the
phone (one layout per viewer) fetches. Same state object, same events, two
transports for two different reasons. **Do not "optimize" this into the
broadcast.**

- [ ] **Step 2: The handler**

Copy `lc_hand_handler`'s shape exactly, including the doc comment's reasoning
about identity coming from the cookie. Body:

```rust
let ctx = load_lc(&state, &code, &player).await?;      // shape, not literal
let me = ctx.st.seat_of(player.id);                     // None is legal
let view = ctx.st.public_view();
Html(format!(
    r#"<div id="lc-table" data-seq="{}">{}</div>"#,
    view.seq,
    lc_render::lc_mini_table(&view, me),
))
```

A room member who has not been seated (joined mid-game, no vessel yet) passes
`None` and gets the unrotated table. That is the same branch `lc_page` already
takes for the hand; do not invent a different one.

- [ ] **Step 3: Wire the pane**

In `lc_room.html`, replace the TABLE pane's placeholder paragraph with the
server-rendered fragment (`lc_page` builds it the same way, so `lc_page` gains a
`table_pane` field). Then extend the existing coalesced fetch: `lcFetchHand`
already debounces the `lcpublic`/`lctick` pair into one request at 60ms — make
that one request fetch **both** fragments (`Promise.all` over the two URLs) so a
change still costs one round trip per surface and never two per event.

The same stale-drop rule applies to the table fragment: compare its `data-seq`
against `lcSeq` and drop anything older.

**The TABLE pane is the fourth scene root** if it hosts CardDot flights, and it
already carries `position: relative` from Task 2's `.lc-mini`. Confirm the pane
does not introduce a second `#lc-flights` when both panes are in the DOM at once
— `ensureLayer` looks up `[data-lc-scene]` and returns the first match, so mark
the scene root deliberately rather than by accident, and assert one
`#lc-flights` per page.

- [ ] **Step 4: Tests**

```rust
#[tokio::test]
async fn test_two_players_get_different_rotations_of_the_same_table() {
    // The core claim. Same room, same state, same seq — different HTML,
    // each with the requester at the bottom slot.
}

#[tokio::test]
async fn test_the_table_route_takes_no_player_identifier() {
    // Byte-identical responses with and without ?player_id= appended:
    // the query string cannot influence whose view is rendered.
}

#[tokio::test]
async fn test_the_table_route_requires_a_session() { /* 302/401, never a body */ }

#[tokio::test]
async fn test_an_unseated_member_gets_the_unrotated_table() {}

#[tokio::test]
async fn test_one_flight_layer_per_page() {
    // Both panes are in the DOM at once; only one #lc-flights may exist.
}
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs \
        drinkinggame/templates/lc_room.html drinkinggame/tests/http.rs
git commit -m "feat(lastcall): per-viewer mini table on the TABLE tab"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 6: The seat ceiling and ending a game

**Class:** C (logic tests cannot encode — reviewer required)

**Why this class:** both halves mutate shared room state through paths other
handlers already use. The ceiling changes an existing join path that runs under
the room lock during a live game; the end route decides which frames reach which
of three surfaces. Neither is settled by a command's output.

These are the two items the STATUS card's *"Carried out of Plan A2"* section
assigns to Plan B by name.

**Files:**
- Modify: `drinkinggame/src/last_call.rs` (`add_player`, ~lines 401–404),
  `drinkinggame/src/routes.rs` (the mid-game join hook, ~lines 210–240, and the
  route table), `drinkinggame/src/lc_routes.rs` (the end handler)
- Test: `drinkinggame/src/last_call.rs`, `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: `last_call::MAX_SEATS`, `db::end_game`, `db::touch_room`,
  `game::idle_panel`, `game::broadcast_room`, `hub::RoomMessage::{Game, Screen}`
- Produces:
  ```rust
  /// Returns the new seat, or None when the table is full.
  pub fn add_player(&mut self, player_id: i64, name: &str) -> Option<usize>;

  /// `POST /room/{code}/lastcall/end`
  pub async fn lc_end_handler(
      State(state): State<GameState>,
      PlayerSession(player): PlayerSession,
      Path(code): Path<String>,
  ) -> axum::response::Response;
  ```

- [ ] **Step 1: Enforce `MAX_SEATS` — on both seating paths, not just one**

There are **two** ways a player reaches a seat, and only one of them is
`add_player`:

1. `add_player` (`last_call.rs` ~401) mirrors `ThreeManState::add_player` and
   pushes at `seat = players.len()` with no ceiling — a recorded Plan A decision,
   deliberately deferred to whoever built the ring. The ninth visitor to open the
   room link gets seat 8.
2. **`LastCallState::new` (~338) seats every room member at once** —
   `members.into_iter().enumerate()`, no ceiling, never routed through
   `add_player`. A room with nine people who press START gets a nine-seat table
   on the *very first render*, before any join hook runs.

Both must be capped, or Task 3's assembler is handed a state
`seat_positions` has no row for.

- `add_player`: return `Option<usize>` — `None` when
  `self.players.len() >= MAX_SEATS`, `Some(seat)` otherwise. Update the mid-game
  join hook in `routes.rs` (~line 217) to handle `None`: the member still joins
  the *room* (they can watch, use the ROOM tab), they are simply not seated in
  this game. **Do not fail the join.**
- `LastCallState::new`: seat the first `MAX_SEATS` members and leave the rest
  unseated — the same "in the room, not at the table" outcome, reached the same
  way. `lc_start_handler` (`lc_routes.rs:128`) needs no signature change.

**Expect existing fixtures to break, and fix them rather than deleting the
assertion.** `add_player`'s signature change touches `last_call.rs:824-829`
(`test_add_player_is_idempotent`), and `last_call.rs:609`
(`st.set_vessel(8, Deck::Soft, "any")`, commented *"8th seat: MAX_SEATS
ceiling"*) is a fixture built when no ceiling existed — check whether it means
index 8 or the 8th seat, and say which in the report. A test that goes green
because an assertion was removed is a regression wearing a passing suit.

**Also bump `seq` when a player is actually seated.** The STATUS card assigns
`add_player`-does-not-bump-`seq` to slice 3 because it was outside every A2 task's
file list; this task is inside that file and changing that exact function, and
leaving two distinct states sharing a seq while the client's equal-seq allowance
admits either is a live correctness bug on the very fragment this plan adds. One
line. Say in the report that it was taken early and why.

- [ ] **Step 2: The end route**

`POST /room/{code}/lastcall/end`, modelled on `tm_end_handler`
(`tm_routes.rs:136`) line for line: `member_room` → take the room lock → `load_lc`
→ `db::end_game` → `db::touch_room` → publish `Game(idle_panel)` and
`Screen(<no marker>)` → `broadcast_room`.

The `Screen` frame it publishes must **not** carry `data-lc-live`, which is what
sends every open `lc_screen.html` back to the generic screen (Task 4, step 1).
That is the whole handoff — verify it in a test, not by reading.

`get_active_game` returns `None` once the game is ended, so the re-render is
kind-aware for free; `tm_end_handler` has a comment saying exactly this. Follow
it.

Add a way to reach the route from `lc_room.html` — a plain `END GAME` control in
the ROOM/TABLE region, following `lc_room.html`'s existing button markup. It ends
the game and keeps the night going; `POST /room/{code}/end` still ends the room.

- [ ] **Step 3: Tests**

```rust
#[test]
fn test_add_player_stops_at_max_seats() {
    // The ninth visitor joins the room and is not seated. The D.2 ring has
    // nowhere to put a seat 8, and a table that silently drops a plaque is
    // worse than one that never seats them.
}

#[test]
fn test_starting_with_more_than_max_seats_members_seats_only_max() {
    // The path add_player never sees: nine members in the room when
    // somebody presses START. LastCallState::new seats the first
    // MAX_SEATS and leaves the rest unseated.
    let members: Vec<(i64, String)> =
        (1..=9).map(|i| (i, format!("p{i}"))).collect();
    let st = LastCallState::new(members, 42);
    assert_eq!(st.players.len(), MAX_SEATS);
    assert!(st.seat_of(9).is_none());
}

#[test]
fn test_seating_a_player_bumps_seq() {
    // Two distinct states must never share a seq: the client's equal-seq
    // allowance exists so a duplicate repaint is harmless, and it would
    // otherwise admit a stale one.
}

#[tokio::test]
async fn test_ending_last_call_keeps_the_room_open() {
    // The room survives, the game does not, and the start card comes back.
}

#[tokio::test]
async fn test_ending_publishes_an_unmarked_screen_frame() {
    // Filter by event name. This is the frame that returns every open
    // spectator screen to the generic view.
}

#[tokio::test]
async fn test_ending_requires_membership() {}

#[tokio::test]
async fn test_a_ninth_member_can_still_open_the_room() {
    // Unseated, but not locked out: the ROOM tab and the TABLE tab both
    // render, and the table renders unrotated.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_routes.rs \
        drinkinggame/src/routes.rs drinkinggame/templates/lc_room.html \
        drinkinggame/tests/http.rs
git commit -m "feat(lastcall): enforce the seat ceiling and end a game without ending the room"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 2 — before the final review

1. Four players, one room, Last Call started. Every phone's TABLE tab shows
   **itself** at the bottom of the felt and the others clockwise from there.
   Cross-check two phones side by side: same names, same HP, different rotation.
2. The big screen shows the same four seats in absolute order.
3. Change one player's vessel. Every phone's table and the big screen repaint;
   nobody's HAND tab loses focus or a half-typed field.
4. Switch a phone to TABLE, leave it there, and trigger a change from another
   phone. The pane repaints in place and the tab does not jump back to HAND.
5. Press END GAME. Every phone returns to the room's start card and every open
   big screen returns to the generic spectator screen, both without a manual
   reload.
6. With eight seated, open the room link as a ninth member. They reach the room,
   are not seated, and the felt still renders eight plaques.

## Final review

One whole-plan review of the branch diff on the most capable model, per
`plan-economics` §4 — this covers Tasks 1–3 (Class A/B, no per-task reviewer).
Tasks 4, 5 and 6 each get their own task reviewer at the time they are built.

Reviewers write the full report to a file and return the verdict plus one-line
findings. Full reports pasted into the controller stay resident for the rest of
the session.

## Before this plan is called done

- `./scripts/verify.sh` green, with the new test count and the clippy count
  quoted. Baseline was 327 tests and 17 distinct warnings.
- Both browser checkpoints run by a human, including the two items owed from
  Plan A-vis.
- The STATUS card (`docs/superpowers/plans/2026-08-06-last-call-STATUS.md`)
  updated: Plan B done, what it carried forward, and what slice 2 inherits.
- `.superpowers/sdd/2026-08-06-last-call-plan-b/progress.md` has a `complete`
  line per task.
- Recorded gaps, if still open, restated in the STATUS card: the 196px-vs-204px
  plaque, the compact deck-list row, and the empty felt centre.
