# Last Call — Plan J: the finish (slice 5)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Ship the three surfaces the bundle never designed — the LOG tab, the
end-of-game screen and the lobby — and close every carried cosmetic, so the
game is complete.

**Architecture:** The log is engine data: an append-only, capped
`log: Vec<LogEntry>` on `LastCallState`, entries appended by the transitions
themselves (pure, unit-testable, replay-consistent), projected whole into
`PublicView` and broadcast as a third `<template>` in the existing `lcpublic`
frame — no new SSE event, no new publish, and by vocabulary construction no
hidden information can enter it. Per-player stats are four counters on
`LcPlayer`, incremented at the reveal charge and inside `resolve()`, projected
into `PublicSeat` — the end screen renders only what the engine recorded. The
end screen and lobby are `lc_render.rs` builders over `PublicView`, riding the
transport Plans A2–E already run.

**Slice:** When this plan is done: the LOG tab shows a live round log on every
phone; a finished game shows a designed end screen (winner, standings,
per-player stats, REMATCH and END NIGHT) on phone and big screen; the pre-game
lobby says who the table is waiting on, on phone and big screen; the plaque
matches the v2 mockup at 196px; the big-screen right rail shows the v2 deck
list rows; `lc-setup-decks` has its rule; the redirects use the canonical room
code. **This is the last plan — after it the game is complete.** Card art is
out of scope in one line: it is content/art work, not code, and no code here
blocks it.

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-j-finish/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

The Module Spec names these four screens as "still to design" (join/lobby, log
tab, end-of-game, card art); nothing in the bundle draws any of them. Everything
below is proposed here, implemented as proposed, and design-reviewed by the user
after execution.

**The log**

- **J1 — The log is public-only, one shared log for the whole table.** Entries
  carry only information that is already public at the moment of appending
  (DDv2 6.3's lock-tick rule, spec §3.3/§3.4). No per-viewer lines: simpler,
  and it means the log can ride the broadcast frame with zero privacy
  machinery. **Flagged consequence:** pact/tab/reaction events (Plans G/H/I,
  unwritten) may not append private content here — if those systems ever want
  per-viewer lines ("your tab completed"), they must ride the private hand
  fetch, never this log. Binding note for those plans.
- **J2 — Vocabulary:** the 15-entry `LogEntry` table in Task 1. The only
  `String` params are card titles, and a title may enter the log **only at or
  after its play's reveal** — the appending sites are the Lock→Reveal edge and
  `resolve()`, nowhere earlier. Arm/disarm/set_target append nothing (secret
  staging); `lock_in` appends the identity-free `Lock { seat }` (the tick).
- **J3 — Cap:** `LC_LOG_CAP = 80` entries, oldest dropped on push. ~80 short
  rows keeps the broadcast frame small and is 3+ rounds of history at a full
  table — a party log, not an audit trail. Rendered newest-first.
- **J4 — Transport:** a third `<template data-lc-log>` inside the existing
  `lcpublic` frame (the exact move Plan B used for `data-lc-screen`): same
  event, same frame count, `broadcast_lc` stays await-free. The phone applies
  it into the LOG pane; the big screen ignores it (F.2 has no log region —
  DDv2's "the big screen announces it" is a later flourish, recorded not
  built).

**The end screen**

- **J5 — Stats are four `#[serde(default)]` counters on `LcPlayer`:**
  `damage_dealt` (HP actually removed from others, attributed to the source
  seat — dot ticks included, via a new `#[serde(default)] source_seat` on
  `Effect` set at creation), `pulls_spent` (play charges only, aligned with
  D4's ordering total — finish-&-draw pulls are drinking, not spending),
  `cards_played` (plays revealed), and `elim_order: Option<u32>` (a monotonic
  1-based counter assigned at elimination — round number can tie, this
  cannot). Only stats the engine records are shown; these are the four worth
  recording. Field-level `serde(default)` keeps the strict nested structs
  skew-safe (`0`/`None` backfill on an older blob; the game is undeployed, so
  no real blob mis-attributes).
- **J6 — Standings order:** alive seats by HP descending, then eliminated
  seats by `elim_order` descending (last out places highest); ties broken by
  lower seat. `LcOutcome::Draw` sorts everyone by `elim_order` descending.
- **J7 — Layout:** phone — the hand pane is *replaced* by an end card (GAME
  OVER, the victory line, the ordered standings with per-player stats) when
  `outcome.is_some()`; big screen — the standings board renders inside Plan
  E's `.lc-centre-victory` takeover on the felt, above the frozen tableau.
  The action bar's outcome row becomes **REMATCH** (amber — it means more
  drinking) + **END NIGHT** (secondary; posts the existing `/lastcall/end`,
  which keeps the room open — the name describes the player's intent, not the
  route's mechanics).
- **J8 — REMATCH is a new route, any member, game-over-gated:** under the room
  lock, end the finished game and run the existing start flow (same
  `room_members` seating as `lc_start_handler` — "same seats" via the same
  flow, mid-night joiners included). 409 while a game is still live.

**The lobby**

- **J9 — The lobby is round-1 `Beat::Draw` with no outcome** — exactly Plan
  E's E1 gate, no new state. Polish only: the phone setup section gains a
  waiting line (`WAITING ON {n}: {NAMES}` / `ALL SET — PRESS START`) and a
  `data-ready` mark per registered row; the big-screen felt centre shows
  `{k} / {n} DRINKS IN` plus who is missing. Copy table in Task 4. No ready
  button, no new routes — "ready" *is* "has a vessel", which E1's begin gate
  already counts.

**Carried cosmetics — every verdict**

- **J10 — Plaque 204px → 196px.** The v2 big-screen mockup draws every plaque
  at `width:196px` and Game UI outranks the Module Spec on pixels; Plan B's
  "re-rendering a shipped component is authoring" excuse expires with this
  plan, which authors components. One CSS value. **Flagged** (visual change to
  a signed-off surface; checkpoint 1 re-eyeballs the ring).
- **J11 — The compact deck list row is built** (v2 mockup: 9px deck dot, name,
  `disc n`, big count) and replaces `deck_stack()` in the big-screen right
  rail, keeping the `deck-{slug}` flight anchors and the `data-low`/
  `data-empty` states; `discard_slot` stays beneath it (its anchor is live).
  `deck_stack` itself survives for the preview page. The "sixth rendering of
  deck state" objection is accepted: this plan is the authoring plan, and the
  mockup is the pixel authority.
- **J12 — `lc-setup-decks` gets its missing rule** (inline-flex + 4px gap).
- **J13 — The `lc_routes.rs` redirects interpolate `ctx.room.code`**, not the
  raw path `code` — a lowercase-code POST currently redirects to a lowercase
  URL. Fixed in the Task 3 route work, pinned by a Location-header test.
- **J14 — `.lc-face-kws` stays unpinned — final.** A string-match test on a
  CSS gap value is brittler than the evidence we have (the preview page shows
  the chips; checkpoints eyeball it). Rejection recorded; parked entry closed.
- **J15 — The felt centre carries nothing further.** Plan E fills it (E15
  revealed plays, E13 victory line) and Task 4 adds the lobby line; verified
  against Plan E's Produces — nothing remains of Plan B's "felt centre is
  empty" gap.
- **J16 — Card art is out of scope:** content/art work, not code (spec §9);
  nothing in this plan blocks it.

---

## Global Constraints

Every task's requirements implicitly include this section.

**Plans C–I status — read first.** Plans C (hand group), D (loop engine), E
(loop wiring) and F (catalog) are **written but unexecuted** at this plan's
writing; Plans G/H/I (pacts, events/tabs, reactions/ghosts) were **not yet
written** (checked twice, 2026-08-11). This plan runs **last**, after all of
them, and is written against their Produces blocks, quoted below where
consumed. If an executed plan's landed code differs from its Produces block,
the landed code wins — adjust the call site, not the interface you produce. If
G/H/I landed transitions of their own, extend Task 1's emission sweep to them
under the J1/J2 rules (public-only, no pre-reveal identity, no pact/tab
content) — their entries join the vocabulary as new variants only if their
events are public by their own plan's rules.

**Interfaces consumed from earlier plans (their Produces blocks, verbatim):**

- Plan D: `pub fn outcome(&self) -> Option<LcOutcome>`;
  `enum LcOutcome { Winner(usize), Draw }`; `PublicView.outcome:
  Option<LcOutcome>`; `advance_beat()` (Lock→Reveal edge charges pulls and
  moves `locked_plays` into `plays`); `resolve()` (beat-6 program + rollover,
  freezes at `Beat::Resolve` on game over — D16); `finish_and_draw(player_id,
  vessel_idx, drawn)`; `ArmedCard { card, target }`; `locked_plays`;
  `Effect.op: EffectOp`.
- Plan E: `ActionBarView` (incl. `outcome`, `vessels_registered`) and
  `lc_action_bar(&ActionBarView)`; the E7 exact-string copy table;
  `data-lc-post` as `lc_loop.js`'s delegated click contract;
  `hand_pane_html` returning
  `{lc_hand_pane(...)}{targets_section}{<template data-lc-actions>}`;
  `persist_and_tick_lc`; `.lc-centre-victory` and the `lc_banner` GAME OVER
  branch (E13/E15); `LcRoomTemplate { …, actions: String }`.
- Plan F: `lc_cards::card_fx(id) -> Option<FxDef>`; `resolve()` resolves
  per-card fx; `EffectOp::PullDrain`.
- Plan C: `hand_group(&HandGroupView)` inside `lc_hand_pane(base_path, code,
  me, hg: &HandGroupView, rows)`.

**Spec bindings still in force:** no private route takes a player identifier
(§6.1); public renderers take `&PublicView` only (§3.4); nothing enters
`plays` before it is revealable (§3.4.1); renderers emit no hex (the `no_hex`
guard covers every new builder); `broadcast_lc` stays awaitless; publish order
`room` → `lcpublic` → `lctick` unchanged; SSE tests filter, never index.

**Baseline:** `./scripts/verify.sh` green. The exact test count depends on how
many of Plans C–I have landed (371 at Plan B sign-off; every plan since adds
tests) — the invariant is **all green, 17 distinct clippy warnings, all in
`drawingportfolio`, `drinkinggame` clippy-clean**. Keep it clean.

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report.

**Browser checkpoints:** after Task 5 (the whole visual strand) and before the
final review. Not per task. No `cargo sqlx prepare` (no migration;
`drinkinggame` is runtime-checked).

---

### Task 1: Engine — the log vocabulary, the cap, and the stat counters

**Class:** B (logic, tests specified below)

**Why this class:** Pure state-machine data with exact expected values — the
tests below are the spec for every append site and every counter.

**Files:**
- Modify: `drinkinggame/src/last_call.rs` (types near `Effect` ~line 233;
  fields on `LcPlayer`/`LastCallState`; append/increment calls inside the
  transitions Plans D/F shipped; tests in the existing `mod tests`)

**Interfaces:**
- Consumes: Plan D's `advance_beat` reveal edge, `resolve()` (as landed —
  find the charge site, the effect-creation site, the elimination site, the
  rollover and the D16 freeze inside them), `finish_and_draw`, `lock_in`;
  Plan F's fx-based resolution; existing `set_vessel`, `set_handicap`,
  `add_player`, `LastCallState::new`.
- Produces (Tasks 2–4 build against these — exact):

```rust
pub const LC_LOG_CAP: usize = 80; // J3

/// One public round-log entry. J2 invariant: the only String params are card
/// titles, and a variant carrying one may be appended only at or after its
/// play's reveal. Seats are indices; names are resolved at render time so a
/// rename never rewrites history.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum LogEntry {
    Round { round: u32 },
    Joined { seat: usize },
    Vessel { seat: usize, deck: Deck },
    Handicap { seat: usize, pct: u16 },
    Draw { seat: usize, deck: Deck, n: u8 },
    Lock { seat: usize },
    Play { seat: usize, title: String, target: Option<usize> },
    Hit { source: usize, target: usize, amount: i32 },
    Heal { seat: usize, amount: i32 },
    Shield { seat: usize, amount: i32 },
    Drain { source: usize, target: usize, amount: i32 },
    Fizzle { seat: usize, title: String },
    Eliminated { seat: usize },
    Reshuffle { deck: Deck },
    GameOver { winner: Option<usize> },
}

// LastCallState gains (container #[serde(default)] makes it skew-safe):
pub log: Vec<LogEntry>,

// LcPlayer gains (field-level defaults — LcPlayer stays strict otherwise; J5):
#[serde(default)] pub damage_dealt: u32,
#[serde(default)] pub pulls_spent: u32,
#[serde(default)] pub cards_played: u32,
#[serde(default)] pub elim_order: Option<u32>,

// Effect gains (attribution for dot-tick damage; J5):
#[serde(default)] pub source_seat: usize,

impl LastCallState {
    /// Appends, dropping the oldest past LC_LOG_CAP. Every append site calls
    /// this — never push directly onto `log`.
    fn push_log(&mut self, entry: LogEntry);
}
```

- [ ] **Step 1: types, fields, `push_log`.** Add the enum, the fields (every
  struct-literal construction site in `last_call.rs` gains the new fields —
  `new`, `add_player`, and any fixture Plans C–F added), and `push_log`:
  `self.log.push(entry); if self.log.len() > LC_LOG_CAP { self.log.remove(0); }`
  (a `Vec`, not `VecDeque` — the blob is JSON either way and 80 is small).
  `Effect` construction sites in `resolve()` set `source_seat` from the
  resolving play's `source_seat`.

- [ ] **Step 2: the emission sweep.** One `push_log` call per site, matched to
  the transitions as landed (prose because Plans D/F's exact bodies are not in
  this file's history yet — the sites are named by their Produces blocks):

  | Site | Entry |
  | --- | --- |
  | `LastCallState::new` | `Round { round: 1 }` |
  | `add_player` (newly seated only) | `Joined { seat }` |
  | `set_vessel` (success) | `Vessel { seat, deck }` |
  | `set_handicap` (success) | `Handicap { seat, pct }` |
  | `finish_and_draw` (success) | `Draw { seat, deck, n: drawn.len() as u8 }` |
  | `lock_in` (success) | `Lock { seat }` |
  | Lock→Reveal edge, per play moved into `plays` | `Play { seat, title, target }` — **and** `pulls_spent +=` that play's charge, `cards_played += 1` on the source |
  | `resolve()`, per damage actually applied (immediate fx and dot ticks) | `Hit { source, target, amount }` with `amount` = HP actually removed (post-clamp), **and** `damage_dealt += amount as u32` on the source (self-damage counts — it is damage dealt) |
  | `resolve()`, heal / shield / drain applied | `Heal` / `Shield` / `Drain` (amounts as applied) |
  | `resolve()`, a targeted play fizzling (7.5) | `Fizzle { seat: source, title }` |
  | `resolve()`, elimination | `Eliminated { seat }`, and `elim_order = Some(next)` where `next` = count of already-eliminated players + 1 |
  | `resolve()` rollover, per reshuffled deck | `Reshuffle { deck }` |
  | `resolve()` rollover (no game over) | `Round { round }` (the new round) |
  | `resolve()` D16 freeze | `GameOver { winner: outcome → Winner(s) ⇒ Some(s), Draw ⇒ None }` |

  Arm, disarm, set_target and the timer append **nothing** (J2).

  > **ERRATUM:** this table omitted the game's four social events — pact
  > breaks, tab settles, reactions, haunts; added post-review per Task 2's
  > adjudication (`PactBreak`/`TabSettle`(`seat` only)/`ReactionPlay`/`Haunt`
  > variants, `push_log` at the betrayal check, the Step 5.5 settle loop,
  > `play_reaction`, and `haunt`).

- [ ] **Step 3: tests.** In `last_call.rs`'s `mod tests` (fixtures from the
  landed Plan D test module — `at_lock()` or equivalent; adjust names to what
  landed):

```rust
#[test]
fn test_log_cap_drops_oldest() {
    let mut st = seated();
    for i in 0..(LC_LOG_CAP as u32 + 20) {
        st.push_log(LogEntry::Round { round: i });
    }
    assert_eq!(st.log.len(), LC_LOG_CAP);
    assert_eq!(st.log[0], LogEntry::Round { round: 20 }); // oldest dropped
}

#[test]
fn test_log_never_carries_identity_before_reveal() {
    // Drive a real staging: arm + target + lock at Beat::Lock (Plan D's
    // transitions), so locked_plays holds a play whose card title is known.
    // Serialize the log alone: no card title appears — the only entries the
    // staging produced are identity-free (Lock { seat }).
    let st = /* fixture: alice armed+targeted+locked "beer-01" (title "Nudge"), still at Beat::Lock */;
    let json = serde_json::to_string(&st.log).unwrap();
    assert!(!json.contains("Nudge"), "{json}");
    assert!(json.contains(r#""t":"lock""#));
}

#[test]
fn test_reveal_and_resolve_write_the_log_and_the_stats() {
    // Fixture: alice (seat 0) locks one targeted Atk play on bob (seat 1),
    // charge C pulls, damage D (use the landed catalog's real numbers —
    // name them as literals here at execution time). advance_beat() through
    // the reveal, then resolve().
    // After the reveal edge:
    //   assert!(st.log.iter().any(|e| matches!(e, LogEntry::Play { seat: 0, .. })));
    //   assert_eq!(st.players[0].pulls_spent, C);
    //   assert_eq!(st.players[0].cards_played, 1);
    // After resolve():
    //   assert!(st.log.iter().any(|e| matches!(e, LogEntry::Hit { source: 0, target: 1, amount } if *amount == D)));
    //   assert_eq!(st.players[0].damage_dealt, D as u32);
}

#[test]
fn test_elimination_order_is_monotonic_and_survives_serde() {
    // Eliminate two players across resolves; the first gets Some(1), the
    // second Some(2); a to_json/from_json round trip preserves both, and
    // from_json("{}")-style skew (an LcPlayer JSON object missing the four
    // stat fields) backfills 0/None — assert by deserializing a hand-written
    // player JSON without them.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): the round log — capped public LogEntry vocabulary, per-player stats, elimination order"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The LOG tab — projection, broadcast template, pane

**Class:** C (logic tests cannot fully encode — reviewer required)

**Why this class:** The log is a new broadcast surface for a hidden-information
game — the public-only property is a cross-task invariant binding every future
append site (including unwritten Plans G/H/I), and the SSE frame grows a third
consumer. Transport-level tests below pin today's leaks; a reviewer judges the
boundary.

**Files:**
- Modify: `drinkinggame/src/last_call.rs` (`PublicView` gains `log`;
  `public_view()` clones it; one projection test)
- Modify: `drinkinggame/src/lc_render.rs` (`lc_log` builder; `lc_public_panel`
  gains the template; tests)
- Modify: `drinkinggame/src/lc_routes.rs` (`LcRoomTemplate` gains
  `log_pane: String`; `lc_page` fills it)
- Modify: `drinkinggame/templates/lc_room.html` (LOG pane renders the field;
  `lcpublic` listener applies the template)
- Modify: `drinkinggame/assets/lastcall.css` (log section)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's `LogEntry`/`log`; `html_escape`, `no_hex`,
  `lc_public_panel` (Plan B shape), the `lcpublic` listener in `lc_room.html`.
- Produces (exact):

```rust
// PublicView gains:
pub log: Vec<LogEntry>,   // public by J1/J2 construction; capped at source

// lc_render.rs — newest-first; names resolved from view.seats, falling back
// to "SEAT {n+1}" for an index the view doesn't know:
pub fn lc_log(view: &PublicView) -> String;
```

DOM contract (the §7.8 table gains this row):

| Component | Root | Requires | Exposes | Filled by |
| --- | --- | --- | --- | --- |
| Log pane | `#lc-log` | `data-count` | `.lc-log-row[data-t]` | `<template data-lc-log>` in `lcpublic` |

- [ ] **Step 1: projection.** `public_view()` gains `log: self.log.clone()`.
  Every `PublicView` literal in fixtures/tests across `lc_render.rs` gains
  `log: Vec::new(),` (the `ring_fixture` pattern from Plan D's Task 1).
  Projection test in `last_call.rs`: the Task 1 lock-beat fixture's
  `serde_json::to_string(&st.public_view())` still contains no staged card
  title — the log rides the same JSON the existing
  `test_public_view_drops_unrevealed_identity` scans, extend that test's
  fixture to have a non-empty log rather than writing a twin.

- [ ] **Step 2: `lc_log`.** Markup (whitespace-free like every builder;
  newest-first — iterate `view.log.iter().rev()`):

```html
<div id="lc-log" data-count="{len}"><ol class="lc-log">{rows}</ol></div>
<!-- empty log: -->
<div id="lc-log" data-count="0"><p class="lc-empty">Nothing logged yet.</p></div>
```

Row: `<li class="lc-log-row" data-t="{tag}">{line}</li>` where `tag` is the
serde tag (`round`, `vessel`, …) and `line` is the copy table — names
uppercased and `html_escape`d, titles `html_escape`d:

| Entry | Line |
| --- | --- |
| Round | `— ROUND {round} —` |
| Joined | `{NAME} TAKES A SEAT` |
| Vessel | `{NAME} REGISTERS {DECK}` |
| Handicap | `{NAME} HANDICAP {pct}%` |
| Draw | `{NAME} FINISHES A {DECK} · +{n}` |
| Lock | `{NAME} LOCKS IN` |
| Play (target Some) | `{NAME} PLAYS {TITLE} → {TARGET}` |
| Play (target None) | `{NAME} PLAYS {TITLE}` |
| Hit | `{SRC} HITS {TGT} −{amount}` |
| Heal | `{NAME} +{amount} HP` |
| Shield | `{NAME} SHIELDS {amount}` |
| Drain | `{SRC} DRAINS {TGT} −{amount} PULLS` |
| Fizzle | `{TITLE} FIZZLES` |
| Eliminated | `{NAME} IS OUT` |
| Reshuffle | `{DECK} RESHUFFLES` |
| GameOver (Some) | `GAME OVER — {NAME} OUTLASTS THE TABLE` |
| GameOver (None) | `GAME OVER — EVERYBODY'S OUT` |

(`−` is U+2212 in the damage lines, matching the HP chip convention;
`{DECK}` is `Deck::label()`.)

- [ ] **Step 3: transport + pane.** `lc_public_panel` gains
  `<template data-lc-log>{lc_log(view)}</template>` after the screen template
  (same frame, no new event — the Plan B `data-lc-screen` move; note it in the
  builder comment and confirm `lc_log` has no `.await`, keeping
  `broadcast_lc` awaitless). `LcRoomTemplate` gains `log_pane: String`
  (`lc_render::lc_log(&view)`); `lc_room.html`'s LOG section becomes
  `<section class="lc-pane" data-lc-pane="log" hidden>{{ log_pane|safe }}</section>`.
  The `lcpublic` listener gains, after the banner swap (same unconditional
  treatment — SSE frames arrive in order; the seq gate exists for fetch
  races, which this surface has none of):

```js
const lg = tpl.querySelector("template[data-lc-log]");
if (lg) { const cur = document.getElementById("lc-log"); if (cur) cur.outerHTML = lg.innerHTML; }
```

- [ ] **Step 4: CSS**, new named section after the setup section:

```css
/* Plan J — the LOG tab (J1–J4): a public shared round log, newest first. */
.lc-log { list-style: none; margin: 0; padding: 4px 2px; display: flex;
          flex-direction: column; gap: 7px; }
.lc-log-row { font-family: var(--font-ui); font-size: 13px; letter-spacing: .04em;
              color: var(--lc-muted); }
.lc-log-row[data-t="round"] { color: var(--lc-faint); font-size: 11px;
                              letter-spacing: .14em; margin-top: 6px; }
.lc-log-row[data-t="hit"], .lc-log-row[data-t="eliminated"],
.lc-log-row[data-t="game_over"] { color: var(--lc-text); }
```

(Token names per `lastcall.css`'s existing ramp — use the sheet's actual
muted/faint text tokens; no new colours.)

- [ ] **Step 5: tests.** In `lc_render.rs`: `test_lc_log_renders_the_copy`
  (a hand-built `PublicView` with one entry of each variant, two seats named
  "alice"/"bob": assert the exact lines above appear, newest first — the last
  pushed entry's line index is smallest via `find()`; empty log → `lc-empty`;
  `no_hex`; a title of `<b>x</b>` renders escaped). In `http.rs`:

```rust
#[tokio::test]
async fn test_the_lcpublic_frame_carries_the_log_but_not_staged_identity() {
    // Plan E's Task 1 rig at Beat::Lock with a staged, locked play.
    // Trigger a broadcast (POST lock as the second player, or set_game_state
    // + a handicap POST). read_sse_until "event: lcpublic": the frame
    // contains "data-lc-log" and "LOCKS IN", and does NOT contain the staged
    // card's title — transport-level J2, the same property
    // test_lc_lock_publishes_the_tick_not_the_cards pins for the seat markers.
}
```

- [ ] **Step 6: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/templates/lc_room.html drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the LOG tab — public round log over the lcpublic frame"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The end-of-game screen, REMATCH, and the redirect fix

**Class:** C (logic tests cannot fully encode — reviewer required)

**Why this class:** `lc_rematch_handler` ends one game and starts another under
one lock (an end/start race a test can only sample), and the task rewires which
surface a finished game shows — route + broadcast work throughout.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (`final_standings`, `lc_end_card`,
  the `.lc-centre-victory` standings board; the `lc_action_bar` outcome row;
  tests)
- Modify: `drinkinggame/src/lc_routes.rs` (`lc_rematch_handler`;
  `hand_pane_html`'s outcome branch; the two `Redirect::to` sites use
  `ctx.room.code`)
- Modify: `drinkinggame/src/routes.rs` (register `rematch` beside the
  `/lastcall/end` line)
- Modify: `drinkinggame/assets/lastcall.css` (end-card + standings rules)
- Test: `drinkinggame/tests/http.rs`

**Interfaces:**
- Consumes: Task 1's stats on `PublicSeat` (projected in Step 1), `LcOutcome`,
  `PublicView.outcome`; Plan E's `lc_action_bar` outcome row, `data-lc-post`
  contract, `hand_pane_html`, `.lc-centre-victory`; `lc_start_handler`'s body
  (the start flow), `lc_end_handler` (END NIGHT posts the existing `end`).
- Produces (exact):

```rust
// PublicSeat gains (public by J5 — derived from public events):
pub damage_dealt: u32,
pub pulls_spent: u32,
pub cards_played: u32,
pub elim_order: Option<u32>,

// lc_render.rs
/// J6 order: alive by hp desc, then eliminated by elim_order desc; ties by
/// lower seat. Pure — the end card and the screen board share it.
pub fn final_standings(view: &PublicView) -> Vec<&PublicSeat>;
/// The phone's game-over pane body (everything inside #lc-hand).
pub fn lc_end_card(view: &PublicView, me: Option<usize>) -> String;

// lc_routes.rs
pub async fn lc_rematch_handler(State, PlayerSession, Path<String>) -> Response;
```

Route: `POST /room/{code}/lastcall/rematch`.

- [ ] **Step 1: projection + `final_standings`.** `PublicSeat` gains the four
  fields (copied in `public_view()`; every `PublicSeat`/fixture literal gains
  them). `final_standings` sorts per J6 —
  `sort_by_key` on
  `(status != Alive, if alive { -hp } else { -(elim_order as i64) }, seat)`
  expressed with a two-arm comparator; exact expected orders in Step 4's test.

- [ ] **Step 2: the two surfaces.** `lc_end_card` (phone; root **is** the
  pane body — `hand_pane_html` wraps it, Step 3):

```html
<section class="lc-endcard">
  <span class="lc-endcard-kicker">GAME OVER</span>
  <h2 class="lc-endcard-victory">{NAME} OUTLASTS THE TABLE</h2>  <!-- Draw: EVERYBODY'S OUT -->
  <ol class="lc-standings">
    <li class="lc-standing" data-seat="{seat}"[ data-me][ data-winner]>
      <span class="lc-standing-place">{place}</span>
      <span class="lc-standing-name">{NAME}</span>
      <span class="lc-standing-fate">{HP {hp}|OUT #{elim_order}}</span>
      <span class="lc-standing-stats">DMG {damage_dealt} · PULLS {pulls_spent} · CARDS {cards_played}</span>
    </li><!-- one per final_standings entry, place = 1-based index -->
  </ol>
</section>
```

`data-winner` on the `Winner(seat)` row only; `data-me` per the viewer.
Big screen: inside the existing `.lc-centre-victory` branch of
`lc_screen_panel` (Plan E's E13/E15 markup), the victory line becomes the
first child and the same `<ol class="lc-standings">` (no `data-me`, no
per-viewer anything — it is broadcast) renders beneath it, capped to the
first **4** rows plus `<span class="lc-standings-more">+{n} MORE</span>` when
longer (the felt centre is not a spreadsheet; phones carry the full table).
Extend Plan E's `test_game_over_takes_over_banner_and_centre` rather than
replacing it.

`lc_action_bar`'s `outcome.is_some()` row (E7 table, first row) becomes:

```html
<button class="lc-btn lc-btn-drink" data-lc-post="rematch">REMATCH</button><button class="lc-btn lc-btn-secondary" data-lc-post="end">END NIGHT</button>
```

(Plan E's delegated `[data-lc-post]` listener posts both with no JS change.
The TABLE pane's static END GAME form stays — it is the mid-game escape.)

- [ ] **Step 3: route work.** `hand_pane_html`: when
  `ctx.st.outcome().is_some()`, the pane is
  `<div id="lc-hand" data-seq="{seq}" data-count="0">{lc_end_card(&view, me)}</div>`
  plus the usual `<template data-lc-actions>` (targets section is empty at
  game over) — the root id and `data-seq` keep `lcApply`'s gate working
  unchanged. `lc_rematch_handler`, in the Task 1/`lc_start_handler` handler
  shape: member_room → room lock → `load_lc` → if
  `ctx.st.outcome().is_none()` return `GameError::OutOfTurn` (409) →
  `db::end_game(&state.pool, ctx.game.id)` → then the `lc_start_handler` body
  verbatim from the member-count check down (members ≥ 2, fresh
  `LastCallState::new`, `db::start_game`, re-`load_lc`,
  `persist_and_broadcast_lc`, 204) — all under the one guard, so no ticker or
  concurrent action can land between the end and the start. Register the
  route. Fix both `Redirect::to` sites (vessel ~line 253, handicap ~line 293)
  to interpolate `ctx.room.code` instead of `code`.

> **ERRATUM (2026-08-12, Task 3 review, C1).** "The `lc_start_handler` body
> verbatim" above is **wrong on one line**: a bare `LastCallState::new` starts
> `seq` at 0, which is correct for `/start` (nobody is on the shell yet when a
> game starts fresh) but wrong for REMATCH, because J8 keeps every phone and
> the big screen in place. Each already holds the FINISHED game's seq as its
> client-side stale-drop floor (`lcApply`/`lcApplyTable` in `lc_room.html`,
> the `lcpublic` frame's own check in `lc_screen.html`) — a fresh game
> restarting at seq 0 lands below that floor and is silently dropped by every
> already-connected surface, forever (no `.game-idle` fires either, since
> nobody left the room). Read the verbatim instruction as "…except carry the
> seq counter forward: `st.seq = ctx.st.seq + 1` (the pre-`end_game` `ctx`,
> still in scope) between constructing the fresh `LastCallState` and calling
> `db::start_game`." The seq counter is scoped to the room's connected
> clients, not to any one game.

- [ ] **Step 4: tests.** `lc_render.rs`:

```rust
#[test]
fn test_final_standings_order() {
    // 4 seats: a alive hp 9, b alive hp 12, c elim_order Some(1),
    // d elim_order Some(2)  ->  [b, a, d, c].
    // Draw case: all eliminated, orders 1..4 -> reverse elim order.
    // Tie: two alive at hp 9 -> lower seat first.
}

#[test]
fn test_end_card_shows_standings_and_stats() {
    // Winner(1) fixture: contains "GAME OVER", "OUTLASTS THE TABLE",
    // data-winner on b's row, "DMG "/"PULLS "/"CARDS " with the fixture's
    // numbers, "OUT #1" on the eliminated row; me = Some(0) puts data-me on
    // a's row; no_hex.
}
```

`http.rs`:

```rust
#[tokio::test]
async fn test_rematch_is_refused_while_the_game_is_live_and_works_after() {
    // Plan E rig, game live: POST rematch -> 409.
    // Freeze it (set_game_state with an outcome-bearing state, or play to
    // game over via the engine): POST rematch -> 204; read_sse_until
    // "event: lcpublic": ROUND 1, no GAME OVER, every member seated;
    // the games table holds exactly one active game for the room.
}

#[tokio::test]
async fn test_vessel_redirect_uses_the_canonical_room_code() {
    // POST /room/{lowercase code}/lastcall/vessel: the Location header ends
    // "/room/{UPPERCASE}/lastcall" (J13).
}
```

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/src/lc_routes.rs drinkinggame/src/routes.rs drinkinggame/src/last_call.rs drinkinggame/assets/lastcall.css drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the end-of-game screen — standings, stats, REMATCH / END NIGHT; canonical-code redirects"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: Lobby polish

**Class:** B (logic, tests specified below)

**Why this class:** Pure builders over `PublicView` with exact copy strings —
the tests are the spec; no route, lock or broadcast changes.

**Files:**
- Modify: `drinkinggame/src/lc_render.rs` (lobby branch in `lc_screen_panel`'s
  centre; waiting line + `data-ready` in the setup section `lc_hand_pane`
  renders; tests)
- Modify: `drinkinggame/assets/lastcall.css` (lobby rules)

**Interfaces:**
- Consumes: `PublicView` (`round`, `beat`, `outcome`, `seats[].vessels`),
  `SetupRow`, Plan E's E1 gate semantics (round-1 Draw is the lobby; "ready"
  = has a vessel).
- Produces: the lobby markup below; no new public functions.

- [ ] **Step 1: the gate and the phone line.** Lobby ⇔
  `view.round == 1 && view.beat == Beat::Draw && view.outcome.is_none()`.
  In the setup section: each row whose `decks` is non-empty gains a bare
  `data-ready` attribute; above the handicap rows, in the lobby only:

```html
<p class="lc-lobby-wait" data-waiting="{n}">WAITING ON {n}: {NAMES}</p>
<!-- all registered: -->
<p class="lc-lobby-wait" data-waiting="0">ALL SET — PRESS START</p>
```

`{NAMES}` = unregistered players' names, uppercased, `html_escape`d, comma-
joined. (The action bar's START gate is Plan E's and is untouched.)

- [ ] **Step 2: the big-screen centre.** In `lc_screen_panel`, when the lobby
  gate holds (and only then — it is mutually exclusive with E15's revealed
  plays and E13's victory takeover by construction), render on the felt:

```html
<div class="lc-centre-lobby">
  <span class="lc-lobby-kicker">LAST CALL</span>
  <span class="lc-lobby-count">{k} / {n} DRINKS IN</span>
  <span class="lc-lobby-names">WAITING ON {NAMES}</span>  <!-- omitted when k == n -->
</div>
```

`k` = seats with ≥ 1 vessel, `n` = seats. CSS in the screen section, shaped
like `.lc-centre-victory` (absolute inset 0, centred column, display font for
the count at 34px, `--lc-faint` kicker) — no new colours.

- [ ] **Step 3: tests.** `lc_render.rs`:

```rust
#[test]
fn test_the_lobby_says_who_it_waits_on() {
    // ring_fixture-style view, round 1 Beat::Draw, 3 seats, one with a
    // vessel: screen panel contains "1 / 3 DRINKS IN" and "WAITING ON " with
    // both missing names; all vesselled -> "3 / 3" and no lc-lobby-names;
    // round 2 Draw -> no lc-centre-lobby; outcome Some -> no
    // lc-centre-lobby; no_hex.
    // Hand pane at the same states: data-ready on the registered row only;
    // "WAITING ON 2: " line, then data-waiting="0" + "ALL SET — PRESS START";
    // round 2 -> no lc-lobby-wait at all.
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/lc_render.rs drinkinggame/assets/lastcall.css
git commit -m "feat(lastcall): lobby polish — waiting-on indicators on phone and felt"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: The deck list row, the 196px plaque, and the CSS closes

**Class:** B (logic, tests specified below)

**Why this class:** One string builder with pinned output plus CSS values; the
rail swap is covered by extending the existing screen-panel tests.

**Files:**
- Modify: `drinkinggame/src/last_call.rs` (`PublicView` gains
  `discard_counts`; projection + one test)
- Modify: `drinkinggame/src/lc_render.rs` (`deck_list_row`; the right rail in
  `lc_screen_panel` swaps to it; tests)
- Modify: `drinkinggame/assets/lastcall.css` (`.lc-deckrow` rules; plaque
  `width: 204px` → `196px` with the comment updated to cite the v2 mockup;
  the missing `lc-setup-decks` rule)

**Interfaces:**
- Consumes: `Deck`, `DECK_LOW_THRESHOLD`, `deck_stack`'s `data-low`/
  `data-empty` contract, `discard_slot`, the ramp blocks' per-deck binding
  pattern (`card_dot`'s).
- Produces (exact):

```rust
// PublicView gains (public — discards are open information, DDv2 beat 6):
pub discard_counts: Vec<(Deck, usize)>,  // per-deck count over `discards`

// lc_render.rs — the v2 mockup's compact rail row (J11):
pub fn deck_list_row(deck: Deck, count: u16, discarded: usize) -> String;
```

DOM contract (the §7.8 table gains this row):

| Component | Root | Requires | Exposes | Motion anchor |
| --- | --- | --- | --- | --- |
| DeckListRow | `.lc-deckrow[data-deck]` | `data-count` | `data-low`, `data-empty` | `deck-{deck}` |

- [ ] **Step 1: projection.** `public_view()` computes `discard_counts` as
  `Deck::ALL` mapped over `discards.iter().filter(|c| c.deck == d).count()`.
  Fixture literals gain the field. Unit test: three discards, two Beer one
  Cider → `[(Beer, 2), (Cider, 1), (Wine, 0), …]`.

- [ ] **Step 2: `deck_list_row`.** Markup (whitespace-free; states mirror
  `deck_stack`'s — `data-low` under `DECK_LOW_THRESHOLD`, `data-empty` at 0,
  both bare-presence):

```html
<div class="lc-deckrow" data-deck="{slug}" data-count="{count}"[ data-low][ data-empty] data-flight-anchor="deck-{slug}"><span class="lc-deckrow-dot"></span><span class="lc-deckrow-name">{LABEL}</span><span class="lc-deckrow-disc">disc {discarded}</span><span class="lc-deckrow-count">{count}</span></div>
```

`lc_screen_panel`'s right rail: `deck_stack` calls become
`deck_list_row(deck, count, discarded)` (zipping `deck_counts` with
`discard_counts` by deck); `discard_slot` stays beneath, unchanged — its
`discard` anchor is a live flight target. `deck_stack` itself is untouched
(the preview page still renders it; the rail's `deck-{slug}` anchors move to
the rows, so the screen page still resolves every anchor exactly once).

- [ ] **Step 3: CSS.** New rules in the table-components section, values from
  the v2 mockup rows; per-deck dot colour bound in the existing ramp blocks
  (the `card_dot` pattern), no hex outside them:

```css
/* DeckListRow (J11) — the v2 mockup's right-rail row: dot, name, disc n,
   big count. Replaces the deck stacks on the big screen only. */
.lc-deckrow { display: flex; align-items: center; gap: 10px;
              background: var(--lc-panel-alt);
              border: 1px solid var(--lc-hair-12); border-radius: 9px;
              padding: 9px 12px; }
.lc-deckrow-dot { width: 9px; height: 9px; border-radius: 50%; }
.lc-deckrow-name { font-family: var(--font-ui); font-weight: 700;
                   font-size: 14px; letter-spacing: .06em;
                   color: var(--lc-text); }
.lc-deckrow-disc { font-family: var(--font-ui); font-size: 11px;
                   letter-spacing: .1em; color: var(--lc-faint);
                   margin-left: auto; }
.lc-deckrow-count { font-family: var(--font-display); font-weight: 900;
                    font-size: 22px; color: var(--lc-text); }
.lc-deckrow[data-low] .lc-deckrow-count { color: var(--lc-amber, #FFB570); }
.lc-deckrow[data-empty] { opacity: .55; }
```

(Adjust token names to the sheet's actual hairline/text/amber tokens — if the
sheet has no amber text token, bind the low state in the ramp/beat blocks the
way `data-low` is coloured for `deck_stack` today; **no new raw hex outside
the token blocks**.) Then the two one-liners: `.lc-plaque` `width: 204px` →
`width: 196px`, with the section comment updated to
`/* PlayerPlaque (D.1) — 196px per the v2 big-screen mockup (Game UI outranks
Module Spec on pixels; Plan J J10). */` — and the missing rule:

```css
.lc-setup-decks { display: inline-flex; gap: 4px; }
```

- [ ] **Step 4: tests.** `lc_render.rs`:

```rust
#[test]
fn test_deck_list_row_states_and_screen_rail_swap() {
    // deck_list_row(Deck::Wine, 4, 16): contains data-deck="wine",
    //   data-count="4", " data-low", no data-empty, "disc 16",
    //   data-flight-anchor="deck-wine"; count 0 -> data-empty; count 21 ->
    //   neither; no_hex.
    // lc_screen_panel(ring_fixture(...)): contains "lc-deckrow" for all five
    //   decks and "lc-discard", and no "lc-deckstack" — the rail swapped.
    //   Every deck-{slug} anchor still present exactly once.
}
```

Extend any existing `lc_screen_panel` test that pinned `deck_stack` in the
rail rather than deleting it.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/last_call.rs drinkinggame/src/lc_render.rs drinkinggame/assets/lastcall.css
git commit -m "feat(lastcall): v2 deck list rows on the rail, 196px plaques, setup-deck gap"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

## Browser checkpoint 1 — after Task 5 (the visual strand)

Two logged-in sessions plus the spectator screen, `cargo run -p drinkinggame`,
a real focused browser:

1. Lobby: register one drink — phone shows `WAITING ON 1: {NAME}`, the felt
   centre shows `1 / 2 DRINKS IN`; register the second — `ALL SET`, felt
   `2 / 2`.
2. Play a round; open LOG on both phones — entries appear live, newest first,
   in the Step-2 copy; nothing about the opponent's unrevealed cards ever
   appears.
3. The big-screen right rail shows the deck list rows (dot, name, `disc n`,
   count), amber count on a low deck; draw flights still land on the rows.
4. Plaques at 196px — the ring still sits on the felt's inner hairline
   ellipse, no overlap at 8 seats (re-eyeball after J10).
5. Finish a game: phones show the end card (standings, stats, REMATCH / END
   NIGHT); the felt shows the victory line + top-4 board. REMATCH lands
   everyone back in a fresh round-1 lobby, same room, same seats. END NIGHT
   lands everyone on the room's idle panel.
6. `lc-setup-decks`: a two-deck player's dots have a visible gap.

## Before the plan is done

- Every task has a class; Tasks 2 and 3 (Class C) each get a task reviewer on
  a capable model; Tasks 1, 4, 5 (B) are covered by the whole-plan review of
  the branch diff at the end, on the most capable model (`plan-economics` §4).
- Browser checkpoint 2 = re-run checkpoint 1's items 2, 4 and 5 against the
  final tree, before the final review.
- No `cargo sqlx prepare` (no migration).
- Interfaces line up: Task 1's `LogEntry`/stat fields are what Task 2 projects
  and Task 3 renders; Task 2's `#lc-log` id is what the `lcpublic` listener
  swaps; Task 3's `data-lc-post="rematch"` rides Plan E's delegated listener
  unchanged; Task 5's `discard_counts` feeds `deck_list_row` only.
- Every assigned item maps: LOG tab → Tasks 1–2; end-of-game + REMATCH/END
  NIGHT → Task 3; lobby → Task 4; plaque 196 / deck list row /
  `lc-setup-decks` → Task 5; redirects → Task 3; `.lc-face-kws` → J14
  (final parking); felt centre → J15 (verified closed); card art → J16 (out
  of scope, one line).
- The J2 secrecy invariant is pinned twice (engine serde test, transport
  frame test) and stated as binding on unwritten Plans G/H/I (J1).
- `drinkinggame` stays clippy-clean; the distinct-warning count stays 17.
