# Last Call — Plan C: the hand group

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Replace the HAND tab's deliberately-throwaway plain list with the
Module Spec C.1–C.3 hand group — HandWheel, ArmedColumn and CostRail, *"one
widget in three parts, and they always ship together."*

**Architecture:** Three new builders in `lc_render.rs` render the group
server-side from the viewer's own private data (`hand`, `armed`, `locked`,
`handicap_pct`), composed by `hand_group()` inside the existing private hand
fragment — same route, same SSE contract, same stale-drop rule; nothing new is
broadcast. A new self-contained vanilla-JS module, `lc_wheel.js`, owns the only
thing HTMX cannot do: the 3D cylinder's per-frame transforms, pointer drag,
momentum snap and rail sync. CSS ships in `lastcall.css` under the existing
token conventions. The arm/disarm *actions* do not exist yet — the JS dispatches
structural `lc:arm`/`lc:disarm` CustomEvents that nothing listens to, which is
the exact hook Plan D/E consumes.

**Slice:** When this plan is done, a player's HAND tab is the designed
interaction surface: an endless draggable wheel of CardFaces with one card in
focus, their armed cards stacked in a live column on the left, and a cost rail
on the right showing the true pull price of every card under their handicap —
browsable one-handed, drunk, without aiming at anything small. The preview page
demonstrates every hard case. Plan D/E (the beat loop: arm / lock / reveal /
resolve, damage) picks up the dispatched events and makes the numbers move.

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-c-hand-group/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

The bundle is silent on each of these; the plan proposes rather than stalls.
Each is recorded where it is implemented.

1. **CostRail bars show the handicapped pull cost (`pull_cost(cost,
   handicap_pct)`), while the CardFace pip keeps the printed cost.** The rail
   answers "what will this actually cost me in sips" (it is a *cost histogram*
   per the Module Spec); the pip is the card's printed value, and showing two
   different numbers in two places is the honest rendering of a handicap.
2. **Rail tap-to-jump treats the whole rail as one pointer surface** — a tap
   maps its y-position to the nearest bar group. Per-group 44px tap targets are
   arithmetically impossible with 10+ cards in a 480px rail, and the Module
   Spec's real rule is "nothing may depend on precise aim."
3. **Renderers treat `hand` and `armed` as disjoint** — the wheel renders
   `hand`, the column renders `armed`, no filtering. This binds Plan D/E:
   `arm()` must MOVE the card out of `hand` into `armed` (and `disarm()` back),
   never copy it. The Module Spec's own words: "the wheel's index list MUST be
   genuinely recomputed, not visually filtered."
4. **A new `armed` flight anchor on the ArmedColumn root** (added to the §7.8.1
   name set and the preview anchor board). Arming visually "moves a card into
   the column"; if slice 3+ wants that flight, the destination must be
   resolvable now or every template gets rewritten later — the exact failure
   §7.8.1 exists to prevent.
5. **Empty-wheel copy:** hand empty and armed empty → the existing "Register
   your drink to be dealt a hand." message; hand empty but armed non-empty →
   "Every card you hold is armed." The first state is pre-deal, the second is a
   real mid-Lock state, and one message for both would lie in one of them.
6. **`lc:arm` / `lc:disarm` bubbling CustomEvents are the Plan D/E hook**
   (detail `{ cardId }`), dispatched on tap of the focused wheel card / an
   armed CardMini. Nothing listens in this plan. Events rather than `hx-post`
   because spec §7.8 forbids behaviour in this slice's markup — the contract
   states structure, and an event name is structure.
7. **Arm/disarm dispatch is suppressed while locked** (`.lc-armed[data-locked]`
   present). Browsing your hand while locked is harmless; appearing to change
   it is not, and suppressing at the dispatch site is one local check.
8. **The wheel's camera (its angle) survives SSE repaints as a raw angle**,
   re-wrapped into the new hand size. When the hand changes the focused card
   may shift one position — accepted: the README already declares `wheelAngle`
   client-local, "a camera, not game state."
9. **The hand group renders as a fixed 480px-tall row inside the scrolling
   HAND pane.** The bundle's wheel fills the whole view, but the pane still
   hosts the setup/handicap section above it until the loop slims that away
   (slice 3's call); 480px = the wheel's visible band (176px focus + two
   fading neighbours) without dead space.
10. **The wheel indexes cards by DOM position (`data-idx`), not card id.** A
    real dealt hand may hold two copies of one catalog card; ids are not
    unique in a hand. `data-card-id` stays on the node as the arm-dispatch
    payload only.
11. **Armed CardMinis take the focused ground** (`--lc-focused`) via a scoped
    rule, matching `Game UI.dc.html`'s armed stack, which draws them on
    `#2e2742` — scoping a shipped component's context is assembling, not
    authoring (the Plan B adjudication, reused).

---

## Global Constraints

Every task's requirements implicitly include this section.

**Spec bindings carried from slice 1 — all still in force:**

- Public renderers take `&PublicView`, never `&LastCallState`. The three new
  builders here are **private** (they render the viewer's own cards), so they
  take `&[Card]`/`HandGroupView` — the private-side twin of the same rule: they
  are called only from the private hand fragment, never from anything
  `broadcast_lc` publishes.
- Private state is fetched per viewer, never broadcast.
  `GET /room/{code}/lastcall/hand` keeps taking **no player identifier** —
  this plan does not touch its signature.
- Renderers emit deck class names, never hex. Every new builder gets a
  `no_hex()` assertion; the hex values below are for CSS only.
- Publish order `room` → `lcpublic` → `lctick` and the await-free
  `broadcast_lc` are untouched — **this plan adds no SSE event, no publish,
  and changes no frame.**
- The §7.8 contract style: roots + `data-*` structure, no behaviour. No
  `hx-post`, no `onclick`, no `action` in any new builder output —
  `test_no_builder_emits_behaviour` is extended to cover them.
- **Do not touch `player_plaque`** (the 204px/196px gap belongs to whoever
  next revises that component — not this plan) and do not touch the felt
  surfaces, `lc_screen.html`, or `lc_layout.rs`.
- **Do not inherit the preview page's duplicate-anchor pattern onto the phone.**
  The phone surface gains exactly one `armed` anchor (inside the single
  `#lc-hand`); the preview page may show several armed columns, which is the
  accepted gallery exception (`lcAnchor` returns the first match).
- One `@media (prefers-reduced-motion: reduce)` block in `lastcall.css` —
  additions go **inside the existing block** (its closing comment says so).

**New-surface rules:**

- The only new route is the JS asset route `/assets/lc_wheel.js`
  (`include_str!`, public, unguarded — the `game.css`/`lc_motion.js` pattern).
  No other route, guard or SSE change.
- `lc_wheel.js` must pass `node --check` (verify.sh covers
  `drinkinggame/assets/*.js`), bind on both `DOMContentLoaded` and
  `htmx:afterSwap` with double-injection guards per CLAUDE.md, and hold no
  live resources (no streams, no EventSource — the shell owns those).
- The wheel's geometry constants, verbatim from Module Spec C.1 / the Game UI
  prototype (`Last Call - Game UI.dc.html:1865ff`):

  ```
  STEP        21        degrees per card
  RADIUS      470       px, track pushed back by translateZ(-470px)
  CARD_H      176       px (CardFace authored height)
  PERSPECTIVE 1400      px, origin 50% 50%
  SENS        0.28      degrees of rotation per px of pointer travel
  SNAP        220 ms    release snap, cubic ease-out
  NOTCH       200 ms    mouse-wheel/trackpad step, one card per notch
  opacity     max(0, 1 − 0.48·|d|)
  visibility  hidden past |d| > 2.05
  z-index     100 − round(10·|d|)
  transitions off while dragging AND for |d| > 1.6 (wrap-around never
              animates across the screen); otherwise transform/opacity
              280ms var(--lc-ease), face background/border 190ms
  focus       |d| < 0.5 → --lc-focused ground, 2px deck-ink border, --lc-lift-lg
  ```

  where `d` is the signed distance in cards from focus, wrapped into
  `[−N/2, N/2]`.
- CostRail values, verbatim from Module Spec C.3: 26px column, inner bar rail
  14px wide right-aligned; bars 3px tall, r2; 2px gap within a card, 7px
  between cards; active card's bars 14px wide at full opacity, all others 9px
  at 40%; width/opacity transition 190ms; card number above in mono 9px
  `--lc-faint`, hand size below.
- ArmedColumn values, verbatim from Module Spec C.2 / the prototype: 62px fixed
  column, 6px gap; header micro-cap `ARMED n` in violet (Space Grotesk 700
  8.5px, .1em tracking, centered); one dashed 46px empty slot (1px dashed
  `rgba(242,238,248,.16)`, r6, mono 9px `slot` label in `--lc-faint`); locked
  → column at 60% opacity, header `LOCKED n`, slot removed.
- **Do not ship the prototype's ↑ / SPIN / ↓ demo row.** The README forbids it.

**Baseline:** `./scripts/verify.sh` green — **371 tests**, **17 distinct**
clippy warnings, all in `drawingportfolio`; `drinkinggame` is clean and must
stay clean (watch `clippy::too_many_arguments` — that is why `HandGroupView`
is a struct, not six loose parameters).

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report.

**Browser checkpoints:** after Task 5 (the whole visual+interaction layer, on
the preview page and the live phone) and before the final whole-plan review.
Not per task.

---

### Task 1: ArmedColumn and CostRail builders

**Class:** B (logic, tests specified below)

**Why this class:** Pure string builders with exact expected values —
`./scripts/verify.sh` runs tests that ARE the spec (bar counts under handicap,
state variants, contract attributes).

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (new builders after `card_dot`,
  ~line 191; tests in the existing `mod tests`)

**Interfaces:**
- Consumes: `Card` (`last_call.rs:195`), `pull_cost(cost: u8, handicap_pct:
  u16) -> u8` (`last_call.rs:328`), `card_mini(&Card) -> String`
  (`lc_render.rs:167`), `html_escape`, the `no_hex` test helper
  (`lc_render.rs:651`).
- Produces:
  - `pub fn armed_column(armed: &[Card], locked: bool) -> String`
  - `pub fn cost_rail(hand: &[Card], handicap_pct: u16) -> String`
  - DOM contract (the §7.8 table gains these rows):

    | Component | Root | Requires | Exposes | Motion anchor |
    | --- | --- | --- | --- | --- |
    | ArmedColumn | `.lc-armed` | `data-count` | `data-locked` (bare presence, locked only) | `armed` |
    | CostRail | `.lc-rail` | `data-count` | `.lc-rail-group[data-idx][data-card-id][data-cost][data-pull-cost]` | — |

- [ ] **Step 1: `armed_column`**

Markup shape (exact attributes; whitespace free like every other builder):

```html
<div class="lc-armed" data-count="{n}"[ data-locked] data-flight-anchor="armed">
  <span class="lc-armed-head">ARMED {n}</span>   <!-- LOCKED {n} when locked -->
  {card_mini(card) for card in armed}
  <span class="lc-armed-slot">slot</span>        <!-- absent when locked -->
</div>
```

`data-locked` is a bare presence attribute, never `data-locked="false"` — same
rule and same reason as `deck_stack`'s `data-low` (`lc_render.rs:314-318`).
The header text is `ARMED {n}` unlocked, `LOCKED {n}` locked. Exactly one slot
regardless of `n` (it is an affordance, not a capacity meter). The column is
rendered even when `armed` is empty — header `ARMED 0` plus the slot.

- [ ] **Step 2: `cost_rail`**

```html
<div class="lc-rail" data-count="{n}">
  <span class="lc-rail-above">01</span>          <!-- 00 when n == 0; JS updates live -->
  <div class="lc-rail-bars">
    <!-- one group per card, in hand order: -->
    <div class="lc-rail-group lc-deck-{slug}[ is-active]" data-idx="{i}" data-card-id="{id}" data-cost="{cost}" data-pull-cost="{pc}">
      <i class="lc-rail-bar"></i>                <!-- × pc -->
    </div>
  </div>
  <span class="lc-rail-below">{n}</span>
</div>
```

`pc = pull_cost(card.cost, handicap_pct)` — decision 1: the rail shows the
true pull price, rounded up. `is-active` is emitted server-side on `data-idx`
`0` only (the initial focus); the JS moves it thereafter. `lc-deck-{slug}` on
the group supplies `--lc-ink` to its bars. `data-card-id` is HTML-escaped via
`html_escape`, as `card_mini` does. `n == 0` renders the root with
`data-count="0"`, label `00` above, `0` below and an empty `.lc-rail-bars` —
stable layout, no special casing downstream.

- [ ] **Step 3: tests** (in `lc_render.rs`'s `mod tests`, using a local
  fixture builder like the existing tests' `Card` constructors)

```rust
#[test]
fn test_cost_rail_applies_handicap_and_rounds_up() {
    // hand: costs 1, 2, 3 (decks beer, wine, liquor — wine exercises the
    // ink-differs ramp). Bar count == data-pull-cost == pull_cost(cost, pct):
    //   pct=100 -> 1,2,3  (6 bars total)
    //   pct=150 -> 2,3,5  (10 bars)   ceil(1.5)=2, ceil(3)=3, ceil(4.5)=5
    //   pct=25  -> 1,1,1  (3 bars)    ceil rounds every fraction up
    //   pct=300 -> 3,6,9  (18 bars)
    // assert bar totals via html.matches(r#"class="lc-rail-bar""#).count()
    // and each group's data-pull-cost attribute; no_hex(&html) on all four.
}

#[test]
fn test_cost_rail_marks_first_group_active_and_survives_empty() {
    // 3-card rail: exactly one `is-active`, on data-idx="0".
    // 0-card rail: data-count="0", contains ">00<" and ">0<", zero
    // lc-rail-group occurrences, no panic.
}

#[test]
fn test_armed_column_states() {
    // empty:    ARMED 0, one lc-armed-slot, no data-locked, anchor present.
    // two:      ARMED 2, two lc-mini roots carrying the two card ids, slot
    //           still present.
    // locked 3: LOCKED 3, ` data-locked` present as a bare attribute (assert
    //           the string `data-locked>` or `data-locked ` — never ="),
    //           zero lc-armed-slot, no "ARMED" substring.
    // no_hex on all three.
}

#[test]
fn test_armed_column_carries_its_motion_anchor() {
    // data-flight-anchor="armed" on the root — mirrors
    // test_plaque_carries_its_motion_anchor.
}
```

Also extend `test_no_builder_emits_behaviour`'s output list with
`armed_column(&cards, false)` and `cost_rail(&cards, 150)` so the no-`hx-`/
no-`onclick`/no-hex sweep covers them permanently.

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_render.rs
git commit -m "feat(lastcall): ArmedColumn and CostRail builders — handicap priced into the rail"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: HandWheel, the hand-group assembly, and the private-fragment rewiring

**Class:** B (logic, tests specified below)

**Why this class:** Markup builders plus data threading, all pinned by unit
tests and two http tests with named assertions (the armed-privacy property is
exactly encodable: A's fragment contains A's armed ids and not B's). The
session gate itself (`PlayerSession`, no player identifier) is not touched.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`hand_wheel`, `HandGroupView`,
  `hand_group`; rewrite `lc_hand_pane` ~line 418; update its four existing
  tests at lines ~1283–1368)
- Modify: `drinkinggame/src/lc_routes.rs` (shared `hand_pane_html` helper;
  `lc_page` ~line 338 and `lc_hand_handler` ~line 420 both call it)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's `armed_column`/`cost_rail`; `card_face(&Card) -> String`;
  `setup_rows(&LastCallState) -> Vec<SetupRow>` (lc_routes.rs);
  `LcPlayer { hand, armed, locked, handicap_pct }`;
  `db::set_game_state` seeding pattern (`tests/http.rs:2213`).
- Produces (Plan D/E builds against these — exact):

  ```rust
  /// The viewer's own private hand-group data. All refs — Copy on purpose.
  #[derive(Clone, Copy, Debug)]
  pub struct HandGroupView<'a> {
      pub hand: &'a [Card],
      pub armed: &'a [Card],
      pub locked: bool,
      pub handicap_pct: u16,
  }

  pub fn hand_wheel(hand: &[Card]) -> String
  pub fn hand_group(hg: &HandGroupView) -> String
  pub fn lc_hand_pane(
      base_path: &str,
      code: &str,
      me: i64,
      hg: &HandGroupView,
      rows: &[SetupRow],
      seq: u64,
  ) -> String
  ```

  - In `lc_routes.rs`: `fn hand_pane_html(base_path: &str, code: &str, st:
    &LastCallState, player_id: i64) -> String` — the single builder of the
    `#lc-hand` fragment, shared by `lc_page` and `lc_hand_handler` (closes the
    STATUS-carried "rows-and-hand lookup duplicated verbatim" minor).
  - DOM contract rows:

    | Component | Root | Requires | Exposes | Motion anchor |
    | --- | --- | --- | --- | --- |
    | HandWheel | `.lc-wheel` | `data-count` | `.lc-wheel-card[data-idx][data-card-id]`; `is-focused` set by JS | — |
    | Hand group | `.lc-handgroup` | — | children `.lc-armed`, `.lc-wheel`, `.lc-rail` in that order | — |

  - The `#lc-hand` root keeps its exact Plan A2 contract: `data-seq`,
    `data-count` (= hand length), `data-flight-anchor="hand"`.

- [ ] **Step 1: `hand_wheel`**

```html
<div class="lc-wheel" data-count="{n}">
  <div class="lc-wheel-stage" data-lc-wheel>
    <div class="lc-wheel-track">
      <div class="lc-wheel-card" data-idx="{i}" data-card-id="{id}">{card_face(card)}</div>
      <!-- one per card, in hand order -->
    </div>
    <span class="lc-wheel-hint">DRAG TO SPIN</span>
  </div>
</div>
```

The wrapper `div` around each `card_face` exists because the JS positions the
wrapper and the face keeps its own authored box — the container is replaced,
**the CardFace rendering is not touched** (spec §2: "slice 2 replaces the
container, not the card"). `data-idx` is the DOM-position index (decision 10);
`data-card-id` repeats the face's id at wrapper level so the JS never reaches
into `card_face` internals. Empty hand: the stage contains no track — instead
`<p class="lc-empty">…</p>` with decision 5's copy (choose the message from
whether `armed` is also empty, so `hand_wheel` grows a second parameter or —
cleaner — the empty-state branch lives in `hand_group`, which sees both
slices; pick the latter and keep `hand_wheel(hand)` single-argument, never
called with an empty hand).

- [ ] **Step 2: `hand_group`**

```rust
pub fn hand_group(hg: &HandGroupView) -> String
```

Emits `<div class="lc-handgroup">{armed_column}{wheel-or-empty}{cost_rail}</div>`
with `armed_column(hg.armed, hg.locked)` first, then `hand_wheel(hg.hand)` (or
the decision-5 `.lc-empty` message in its place when `hg.hand` is empty —
`"Register your drink to be dealt a hand."` when `hg.armed` is empty too,
`"Every card you hold is armed."` otherwise), then
`cost_rail(hg.hand, hg.handicap_pct)`. The rail renders in both empty cases
(its own `n == 0` state, Task 1).

- [ ] **Step 3: rewrite `lc_hand_pane`** to the new signature. The root
  element, the setup `<section>` (deck select, container input, handicap
  rows — all byte-identical to today's) and the seq/count attributes stay; the
  trailing `hand.iter().map(card_face)` list and its empty-message branch are
  replaced by `hand_group(hg)`. `data-count` remains `hg.hand.len()`.

- [ ] **Step 4: `hand_pane_html` in `lc_routes.rs`** — mirrors
  `table_pane_html`'s role (one shared builder so two call sites can never
  disagree about fragment shape):

```rust
fn hand_pane_html(base_path: &str, code: &str, st: &LastCallState, player_id: i64) -> String {
    let rows = setup_rows(st);
    let (hand, armed, locked, handicap_pct) = match st.seat_of(player_id) {
        Some(seat) => {
            let p = &st.players[seat];
            (p.hand.as_slice(), p.armed.as_slice(), p.locked, p.handicap_pct)
        }
        None => (&[] as &[_], &[] as &[_], false, 100),
    };
    let hg = lc_render::HandGroupView { hand, armed, locked, handicap_pct };
    lc_render::lc_hand_pane(base_path, code, player_id, &hg, &rows, st.seq)
}
```

`lc_page` and `lc_hand_handler` both collapse to a call of this. Neither
handler's extractor set, guard chain or response type changes.

- [ ] **Step 5: tests.** In `lc_render.rs`:

```rust
#[test]
fn test_hand_wheel_wraps_each_card_face_unchanged() {
    // 3 distinct cards: root data-count="3"; data-idx 0,1,2 in order; each
    // wrapper's inner HTML contains the byte-exact card_face(card) string
    // (assert html.contains(&card_face(&c)) per card); data-card-id at the
    // wrapper level matches; no_hex.
}

#[test]
fn test_hand_group_orders_armed_wheel_rail_and_picks_the_empty_copy() {
    // populated: find() positions of "lc-armed" < "lc-wheel" < "lc-rail".
    // hand empty + armed empty  -> contains "Register your drink", no lc-wheel.
    // hand empty + armed 1      -> contains "Every card you hold is armed.",
    //                              no lc-wheel, rail data-count="0".
}
```

Update the four existing `lc_hand_pane` tests (contract, prefixed URLs,
handicap-rows-not-self-gated, empty-hand) to the new signature; the empty-hand
test's expected copy moves to the two-message rule above. Add to the
`test_lc_hand_pane_satisfies_the_contract` body: the pane now also contains
exactly one `.lc-handgroup`, one `.lc-armed`, one `data-flight-anchor="armed"`
(the phone surface must not inherit the preview's duplicate-anchor pattern —
one match is what `lcAnchor` returns, so one is what may exist).

In `tests/http.rs`, seeding via the `set_game_state` pattern (http.rs:2213):

```rust
#[tokio::test]
async fn test_hand_fragment_carries_only_the_viewers_armed_cards() {
    // Two sessions A and B in one room, LC started. Build LastCallState via
    // its constructor; give A armed = [card "beer-01"], locked=false; give B
    // armed = [card "cider-01"], locked=true; persist with set_game_state.
    // GET /room/{code}/lastcall/hand as A: body contains "beer-01" inside an
    //   lc-armed block, contains "ARMED 1", does NOT contain "cider-01".
    // GET as B: contains "cider-01" and "LOCKED 1" and ` data-locked`, does
    //   NOT contain "beer-01".
}

#[tokio::test]
async fn test_hand_fragment_prices_the_rail_by_the_viewers_own_handicap() {
    // A handicap_pct=300 with one cost-2 card in hand -> A's fragment has a
    // rail group with data-pull-cost="6"; B at 100 with the same card ->
    // data-pull-cost="2". Same card, two prices — the per-viewer render is
    // load-bearing, not cosmetic.
}
```

Check the existing hand-route tests around http.rs:3867–4033 (privacy,
`?player_id=` byte-identity): they assert on card ids and the absence of the
other player's ids, which the wheel markup preserves via `data-card-id`; update
any assertion that greps for markup this plan replaced (the plain-list
`#lc-hand` had no `.lc-wheel`).

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the hand group replaces the throwaway HAND list — wheel, armed column, cost rail"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: the hand-group CSS

**Class:** A (compiler/lint-gated)

**Why this class:** CSS only; `tests/static_assets.rs` guards nested comment
markers, `verify.sh` is the gate, the browser checkpoint eyeballs the result.

**Files:**
- Modify: `drinkinggame/assets/lastcall.css` (replace the "hand region —
  deliberately throwaway" section at ~line 386; one token added at ~line 29;
  reduced-motion additions inside the existing block at ~line 470)

**Interfaces:**
- Consumes: the class names Task 1/2 emit (`.lc-handgroup`, `.lc-armed`,
  `.lc-armed-head`, `.lc-armed-slot`, `.lc-wheel`, `.lc-wheel-stage`,
  `.lc-wheel-track`, `.lc-wheel-card`, `.lc-wheel-hint`, `.lc-rail`,
  `.lc-rail-above`, `.lc-rail-below`, `.lc-rail-bars`, `.lc-rail-group`,
  `.lc-rail-bar`, `is-active`) and the state classes Task 4's JS toggles
  (`is-focused`, `is-dragging`, `is-far` on `.lc-wheel-card`).
- Produces: the styled surface; one new token `--lc-hair-slot`.

- [ ] **Step 1: token.** In `:root`, after `--lc-hair-max`:

```css
--lc-hair-slot: rgba(242, 238, 248, .16);   /* the armed column's dashed slot (README) */
```

- [ ] **Step 2: replace the throwaway section.** Delete the two-line "hand
  region — deliberately throwaway" comment + rule and write the new section.
  Verbatim:

```css
/* slice 2 — the hand group (Module Spec C.1–C.3): ArmedColumn | HandWheel |
   CostRail. "One widget in three parts and always ship together." This
   replaces Plan A2's throwaway plain-list container; the CardFace inside the
   wheel is Plan A's, untouched. Fixed 480px row inside the scrolling pane
   (decision 9): the setup section still sits above it until the loop slims
   that away. */
#lc-hand { display: flex; flex-direction: column; gap: 12px; padding-bottom: 14px; }
.lc-handgroup { display: flex; gap: 10px; height: 480px; min-height: 0; }

/* C.2 ArmedColumn — 62px fixed. data-locked: 60% opacity, slot removed by
   the renderer (not display:none — the markup is the state). */
.lc-armed { flex: 0 0 62px; display: flex; flex-direction: column; gap: 6px; }
.lc-armed[data-locked] { opacity: .6; }
.lc-armed-head { font-family: var(--font-ui); font-weight: 700; font-size: 8.5px;
                 letter-spacing: .1em; text-transform: uppercase;
                 text-align: center; color: var(--lc-violet); }
.lc-armed-slot { height: 46px; border: 1px dashed var(--lc-hair-slot);
                 border-radius: 6px; display: flex; align-items: center;
                 justify-content: center; font-family: var(--font-mono);
                 font-size: 9px; color: var(--lc-faint); }
/* armed minis sit on the focused ground, per the Game UI armed stack
   (decision 11) — context styling of Plan A's component, not a fork */
.lc-armed .lc-mini { background: var(--lc-focused); }

/* C.1 HandWheel — a 3D cylinder of CardFaces on the X axis. The track is
   pushed back by the radius so the focused card keeps its authored size;
   lc_wheel.js writes each card's rotateX/translateZ, opacity, visibility and
   z-index inline from the constants it owns (STEP 21, RADIUS 470, sens .28). */
.lc-wheel { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.lc-wheel-stage { flex: 1; position: relative; perspective: 1400px;
                  perspective-origin: 50% 50%; overflow: hidden;
                  touch-action: none; cursor: grab; }
.lc-wheel-stage:active { cursor: grabbing; }
.lc-wheel-track { position: absolute; inset: 0; transform-style: preserve-3d;
                  transform: translateZ(-470px); }
.lc-wheel-card { position: absolute; left: 0; right: 0; top: 50%;
                 height: 176px; margin-top: -88px; will-change: transform;
                 transition: transform 280ms var(--lc-ease),
                             opacity 280ms var(--lc-ease); }
/* off while dragging and for wrap-around cards — a card jumping from one end
   of the cylinder to the other must never animate across the screen */
.lc-wheel-card.is-dragging, .lc-wheel-card.is-far { transition: none; }
.lc-wheel-card .lc-cardface { height: 100%;
                              transition: background 190ms var(--lc-ease),
                                          border-color 190ms var(--lc-ease); }
/* focus styling only at |d| < 0.5 — everything else keeps .lc-cardface's own
   1px hairline on the raised ground */
.lc-wheel-card.is-focused .lc-cardface { background: var(--lc-focused);
    border: 2px solid var(--lc-ink, var(--lc-hair-strong));
    box-shadow: var(--lc-lift-lg); }
.lc-wheel-hint { position: absolute; left: 0; right: 0; bottom: 6px;
                 text-align: center; font-family: var(--font-ui);
                 font-weight: 700; font-size: 9px; letter-spacing: .14em;
                 text-transform: uppercase; color: var(--lc-faint);
                 pointer-events: none; }

/* C.3 CostRail — 26px column, a cost histogram you can scrub. Bars are
   right-aligned inside a 14px inner rail; the whole column is one pointer
   surface (decision 2) — lc_wheel.js maps a tap's y to the nearest group. */
.lc-rail { flex: 0 0 26px; display: flex; flex-direction: column;
           align-items: center; gap: 6px; padding: 14px 0;
           touch-action: none; }
.lc-rail-above, .lc-rail-below { font-family: var(--font-mono); font-size: 9px;
                                 color: var(--lc-faint); }
.lc-rail-bars { flex: 1; width: 14px; display: flex; flex-direction: column;
                justify-content: center; align-items: flex-end; }
.lc-rail-group { display: flex; flex-direction: column; align-items: flex-end;
                 gap: 2px; }
.lc-rail-group + .lc-rail-group { margin-top: 7px; }
.lc-rail-bar { display: block; height: 3px; width: 9px; border-radius: 2px;
               background: var(--lc-ink, var(--lc-label)); opacity: .4;
               transition: width 190ms var(--lc-ease),
                           opacity 190ms var(--lc-ease); }
.lc-rail-group.is-active .lc-rail-bar { width: 14px; opacity: 1; }
```

- [ ] **Step 3: reduced motion.** Inside the **existing**
  `@media (prefers-reduced-motion: reduce)` block (never a second block — the
  sheet's closing comment forbids it), add:

```css
.lc-wheel-card, .lc-rail-bar { transition: none; }
```

(The JS half — instant snap instead of glide — is Task 4's.)

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/assets/lastcall.css
git commit -m "feat(lastcall): hand-group CSS — wheel stage, armed column, cost rail"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: `lc_wheel.js` — the wheel interaction module

**Class:** A (compiler/lint-gated)

**Why this class:** Static JS gated by `node --check` in verify.sh (the class
table names static JS as A); behaviour is eyeballed at the browser checkpoint,
same as Plan A-vis's motion tasks.

**Files:**
- Create: `drinkinggame/assets/lc_wheel.js`
- Modify: `drinkinggame/src/routes.rs` (asset handler next to `lc_motion_js`
  ~line 743; route registration ~line 896)
- Modify: `drinkinggame/templates/lc_room.html` (script tag; one call in
  `lcApply`)
- Modify: `drinkinggame/templates/lc_preview.html` (script tag)
- Test: `drinkinggame/tests/http.rs` (asset served — mirror the existing
  `lc_motion.js` asset test)

**Interfaces:**
- Consumes: Task 2's DOM contract (`[data-lc-wheel]` stage, `.lc-wheel-track`,
  `.lc-wheel-card[data-idx][data-card-id]`, `.lc-rail-group[data-idx]`,
  `.lc-rail-above`, `.lc-armed .lc-mini[data-card-id]`,
  `.lc-armed[data-locked]`), Task 3's state classes.
- Produces (Plan D/E builds against these — exact):
  - `window.lcWheelInit(root?)` — idempotent; initializes every
    `[data-lc-wheel]` stage under `root` (default `document`) not already
    bound, and (re)binds armed-column and rail listeners with their own
    guards.
  - CustomEvent `"lc:arm"`, `{ bubbles: true, detail: { cardId: <string> } }`,
    dispatched from the tapped `.lc-wheel-card` when it is the focused card
    and no `.lc-armed[data-locked]` exists in the same `#lc-hand`.
  - CustomEvent `"lc:disarm"`, same shape, dispatched from a tapped
    `.lc-armed .lc-mini`, same locked suppression.
  - Nothing in this plan listens to either event. **Plan D/E's contract: attach
    ONE delegated listener for each on `body.lc` and POST the intent; do not
    rebind per repaint.**

- [ ] **Step 1: the module.** IIFE + `"use strict"`, the `lc_motion.js` shape.
  Core, with the constants verbatim (the rest of the wiring is boilerplate
  following `lc_motion.js` / `palette.js` patterns — pointer capture, guards,
  listeners):

```js
var STEP = 21, RADIUS = 470, SENS = 0.28, SNAP_MS = 220, NOTCH_MS = 200;

function reduced() { /* as lc_motion.js */ }

// One persisted camera angle for the phone's single live wheel (decision 8):
// saved on every angle change by the wheel whose stage sits inside #lc-hand,
// restored (re-wrapped to the new N) when lcWheelInit rebuilds after a
// repaint. Preview wheels are gallery demos and do not persist.
var savedAngle = 0;

function layout(cards, angle, dragging) {
  var N = cards.length;
  for (var i = 0; i < N; i++) {
    var d = i - angle / STEP;
    while (d > N / 2)  d -= N;
    while (d < -N / 2) d += N;
    var ad = Math.abs(d);
    var el = cards[i];
    el.style.transform = "rotateX(" + (-d * STEP) + "deg) translateZ(" + RADIUS + "px)";
    el.style.opacity = String(Math.max(0, 1 - 0.48 * ad));
    el.style.visibility = ad > 2.05 ? "hidden" : "visible";
    el.style.zIndex = String(100 - Math.round(ad * 10));
    el.classList.toggle("is-focused", ad < 0.5);
    el.classList.toggle("is-dragging", !!dragging);
    el.classList.toggle("is-far", ad > 1.6);
  }
  return ((Math.round(angle / STEP) % N) + N) % N;   // focused index
}
```

Per stage (closure state `angle`, `dragging`, `raf`):
  - `snap(a) = Math.round(a / STEP) * STEP`; `glide(to, ms)` = rAF loop with
    ease `1 - Math.pow(1 - p, 3)`, cancelled by any new gesture; under
    `reduced()`, glide sets the target angle in one frame.
  - Pointer: `pointerdown` captures the pointer, records `y0`/`a0`/`t0`;
    `pointermove` sets `angle = a0 - (e.clientY - y0) * SENS` (downward drag →
    lower index) and relayouts; `pointerup`/`pointercancel` releases — if
    total travel < 6px and elapsed < 250ms it is a **tap**: when the tap
    landed on the focused card, dispatch `lc:arm` (locked-suppressed,
    decision 7); otherwise `glide(snap(angle), SNAP_MS)`.
  - `wheel` event (`passive: false`, `preventDefault`): one card per notch —
    `glide(snap(angle + Math.sign(e.deltaY) * STEP), NOTCH_MS)`.
  - After every layout, sync the rail: within the same `.lc-handgroup`, set
    `is-active` on the `.lc-rail-group[data-idx]` matching the focused index
    and write `String(idx + 1).padStart(2, "0")` into `.lc-rail-above`.
  - Rail scrubbing (decision 2): `pointerdown` on `.lc-rail` maps
    `e.clientY` to the nearest `.lc-rail-group` midpoint and glides the
    sibling wheel to `idx * STEP`.
  - Armed column: delegated `click` on `.lc-armed` — a `.lc-mini[data-card-id]`
    target dispatches `lc:disarm` (locked-suppressed).
  - Guards: `stage.dataset.lcWheelBound`, and separate `lcRailBound` /
    `lcArmedBound` flags on their roots (the preview's two-flag lesson,
    `lc_preview.html:56-63`). Angle persistence: only when
    `stage.closest("#lc-hand")` is non-null, mirror every angle change into
    `savedAngle` and start from `savedAngle` on init.
  - Bind `lcWheelInit` on `DOMContentLoaded` and `htmx:afterSwap`
    (`e.target`), per CLAUDE.md. The module holds no live resources — nothing
    to release on `htmx:beforeSwap`.

- [ ] **Step 2: serve it.** In `routes.rs`, next to `lc_motion_js`:

```rust
async fn lc_wheel_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../assets/lc_wheel.js"))
}
```

(match `lc_motion_js`'s exact header construction) and register
`.route("/assets/lc_wheel.js", get(lc_wheel_js))` beside the `lc_motion.js`
line. Add the http test mirroring the existing `lc_motion.js` asset test
(status 200, content type, non-empty body).

- [ ] **Step 3: link it.** `lc_room.html` and `lc_preview.html` each gain
  `<script src="{{ base_path }}/assets/lc_wheel.js" defer></script>` under the
  existing `lc_motion.js` line. In `lc_room.html`'s `lcApply`, after the
  focus-restore block (immediately before the function's end), add:

```js
if (window.lcWheelInit) window.lcWheelInit(pane);
```

— the pane's `innerHTML` swap destroyed the bound stage, so the repaint path
re-initializes explicitly; the camera survives via `savedAngle` (decision 8).
`lcApplyTable` needs nothing (no wheel in the TABLE pane).

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/assets/lc_wheel.js drinkinggame/src/routes.rs drinkinggame/templates/lc_room.html drinkinggame/templates/lc_preview.html drinkinggame/tests/http.rs
git commit -m "feat(lastcall): lc_wheel.js — drag, snap, rail scrub, arm/disarm dispatch"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: preview group 8 — the hand group's hard cases

**Class:** A (compiler/lint-gated)

**Why this class:** Preview markup assembled from existing builders plus a
fixture tweak pinned by existing tests; verify.sh gates it, checkpoint 1
eyeballs it.

**Files:**
- Modify: `drinkinggame/src/lc_preview.rs` (`hand_group_group()` + one entry in
  `build_groups()` between `shell_group` and `flights_group`; `armed` row in
  `flights_group`'s anchor list)
- Modify: `drinkinggame/src/last_call.rs` (`preview_state()` — vary the
  oversized hand's card ids)
- Modify: `drinkinggame/assets/lastcall.css` (preview-section layout rules
  only)

**Interfaces:**
- Consumes: `hand_group`/`HandGroupView`/`armed_column`/`cost_rail` (Tasks
  1–2), `lc_cards::CATALOG`/`deck_cards`, `boundary_cards()`, the
  `swatch`/`row` helpers, `lcWheelInit` auto-init via `DOMContentLoaded`
  (Task 4 — the preview's wheels come alive with zero preview-side JS).
- Produces: the permanent visual proof of every hard case; the `armed` anchor
  on the preview anchor board.

- [ ] **Step 1: `preview_state()` id fix.** The 12-card oversized hand repeats
  four Cider ids three times; its own comment says *"Slice 2's HandWheel
  indexes by card id, so vary the ids before building it."* The wheel now
  indexes by position (decision 10), but three visually identical card triples
  would make the preview's oversized wheel look broken. Clone-and-suffix:

```rust
st.players[1].hand = (0..3)
    .flat_map(|rep| {
        crate::lc_cards::deck_cards(Deck::Cider).into_iter().map(move |mut c| {
            if rep > 0 { c.id = format!("{}-r{rep}", c.id); }
            c
        })
    })
    .collect();
```

Update the fixture comment (count still 12; the hand-strip split tests read
only the count and stay green).

- [ ] **Step 2: `hand_group_group()`.** Fixture data only — `HandGroupView`s
  built inline from CATALOG cards, `boundary_cards()` and synthesized cards
  (the `boundary_cards` constructor pattern); costs the catalog lacks are
  synthesized, since the preview exists to show what a plain game cannot
  reach. Rows:

  1. **"HandWheel — live, drag it"**: one full assembled `.lc-handgroup`
     (armed 2, 8-card mixed-deck hand, handicap 100) inside a
     `.lc-preview-scroll` > `.lc-preview-handframe` sized box — the wheel is
     real and Task 4's JS drives it on this page.
  2. **"HandWheel — degenerate hand sizes"**: oversized (`preview_state()`'s
     14 — seat 1's 12 plus two boundary cards — exercising `|d| > 2.05`
     culling), one-card (snap always returns to it), and the two empty states
     with their distinct copy (decision 5).
  3. **"ArmedColumn — 0 / 1 / many / locked"**: `armed_column(&[], false)`,
     one, four, and `armed_column(&three, true)` — LOCKED 3, dimmed, no slot.
  4. **"CostRail — every cost 1–3 in every deck"**: one rail over 15
     synthesized cards (5 decks × costs 1, 2, 3) at handicap 100 — 30 bars,
     every ink ramp visible, Wine's ink-not-fill included.
  5. **"CostRail — handicap prices the same hand differently"**: the same
     3-card hand (costs 1, 2, 3) at 100 / 150 / 300 side by side; captions
     carry the expected bar counts (1,2,3 · 2,3,5 · 3,6,9) so a regression is
     visible against its label.

  Group note: name decision 1 (the rail shows pull price, the pip shows
  printed cost) so a viewer is told why the numbers differ. **No `#lc-hand`
  id anywhere in this group** — the preview already carries duplicates from
  `shell_group` and must not gain more; the group's wheels live in plain
  `.lc-handgroup` roots, which also keeps their camera out of decision 8's
  persistence (gallery wheels are demos). `shell_group`'s static F.1 frame is
  deliberately left as the plain list — it demonstrates the F.1 chrome order,
  not the hand view; group 8 owns the hand.

- [ ] **Step 3: anchor board.** Add `"armed"` to the `anchors` array in
  `flights_group` (lc_preview.rs:626-643). It resolves against row 1's
  assembled hand group — the first `.lc-armed` on the page, which is exactly
  what `lcAnchor` returns.

- [ ] **Step 4: preview CSS.** In `lastcall.css`'s preview section (below the
  Plan A-vis rules, following its "page layout only, no new colours"
  convention):

```css
/* Plan C group 8 — a sized stage for live preview wheels: the hand group is
   height: 480px by its own rule, but needs a bounded width to read as a
   phone column rather than a full-bleed strip. */
.lc-preview-handframe { width: 366px; height: 480px; }
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_preview.rs drinkinggame/src/last_call.rs drinkinggame/assets/lastcall.css
git commit -m "feat(lastcall): preview group 8 — the hand group's hard cases"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 1 — after Task 5

A human, a real focused browser (automation tabs stay backgrounded and freeze
animation), `cargo run -p drinkinggame`, `http://localhost:3001/lastcall/preview`:

1. Drag the live wheel: 0.28°/px feel, release snaps ~220ms, focused card gets
   the deep lift + 2px deck border, mouse wheel steps exactly one card.
2. Spin past the ends: wrap-around cards never animate across the screen.
3. Rail: active group widens to 14px/full opacity as focus moves; the number
   above tracks; tapping the rail jumps the wheel; a tap anywhere on the 26px
   column lands on the nearest card.
4. Tap the focused card and an armed CardMini — devtools console with
   `document.body.addEventListener("lc:arm", e => console.log(e.detail))`
   shows the cardId; repeat on the locked column sample: nothing fires.
5. Oversized wheel stays smooth (hidden past |d| > 2.05); one-card and both
   empty states render their copy; armed 0/1/many/locked all match the spec.
6. Both CostRail rows match their captions bar-for-bar (30 bars; 1,2,3 /
   2,3,5 / 3,6,9).
7. Devtools "Emulate prefers-reduced-motion: reduce": wheel snaps instantly,
   no glide, rail still syncs.
8. Then the live phone: two browser profiles, start a Last Call game, open the
   HAND tab — wheel over the real (empty-until-slice-3) state renders the
   correct empty copy; register a vessel; the SSE repaint does not steal the
   camera or wipe form state (type into the container field while the other
   session sets a handicap).

## Before this plan is called done

- Every task's `./scripts/verify.sh` output quoted; tests grow from the
  371 baseline; clippy still 17 distinct warnings, `drinkinggame` still clean.
- Browser checkpoint 1 run by a human (above).
- The whole-plan review (plan-economics §4): one review of the full branch
  diff on the most capable model — the only review Tasks 1–5 receive, since
  all five are Class A/B. Reviewer brief must name: the private-fragment
  boundary (no armed data in any broadcast builder), the disjoint hand/armed
  assumption (decision 3) as a recorded bind on Plan D/E, the no-hex sweep
  over the new builders, the single-`armed`-anchor rule on the phone surface,
  and the `lcApply` → `lcWheelInit` repaint path (inline-script territory no
  harness reaches — the same standing property STATUS records for
  `.game-idle`).
- Browser checkpoint 2 before merging the review's fixes: re-run checkpoint 1
  items 1, 4 and 8 against the fixed tree.
- STATUS file updated: slice 2 shipped, the decision-3 bind and the
  `lc:arm`/`lc:disarm` + `lcWheelInit` contract recorded for Plan D/E, and the
  carried pre-deploy item (`from_json` is an uncapped third path into
  `players`) re-stated — it is NOT closed by this plan and expires the moment
  the branch reaches master.

## Self-review (performed while writing)

- **Spec coverage:** C.1 geometry/behaviour/depth/focus ✔ (Task 3 CSS + Task
  4 JS, constants verbatim); C.2 states empty/partial/locked ✔ (Task 1);
  C.3 bars/active/scrubber ✔ (Tasks 1, 3, 4); "one widget, ship together" ✔
  (one plan); 44px/no-precise-aim ✔ (decisions 2, whole-stage drag);
  demo controls not shipped ✔; §7.8.1 anchors ✔ (decision 4); hx-boost
  lifecycle rules ✔ (Task 4 bindings + guards); no new SSE events ✔; private
  route signature untouched ✔; plaque untouched ✔.
- **Placeholder scan:** no TBD/TODO/"handle edge cases"/"similar to Task N";
  every magic number carries its value; both empty-state copies are spelled
  out; test expected values are computed in-line (pull_cost tables).
- **Type consistency:** `HandGroupView<'a>` defined in Task 2, consumed by
  Tasks 2/5; `armed_column(&[Card], bool)` and `cost_rail(&[Card], u16)`
  defined in Task 1, consumed by Tasks 2/5; `lcWheelInit`/`lc:arm`/`lc:disarm`
  defined in Task 4, consumed by Task 5's checkpoint and named for Plan D/E;
  `hand_pane_html` replaces both duplicated lookups; `pull_cost(u8, u16) -> u8`
  matches `last_call.rs:328`.
