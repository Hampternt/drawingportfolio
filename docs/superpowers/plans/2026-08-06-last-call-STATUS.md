# Last Call — where we are

Resume card. Point a fresh session at this file.

**What it is:** a third game mode for the `drinkinggame` crate, alongside Ring of
Fire and 3 Man. Players join a room on their phones, register what they are
drinking (which picks their deck), and spend *pulls* — sips — to play cards.
Six beats to a round, out at 0 HP.

**Branch:** `feat/last-call`, based on `origin/master` @ `8931ed0`. Renamed from
`master-3` on 2026-08-09 along with its worktree directory; commit messages and
plan-end notes written before that date still say `master-3`.

## Read these, in this order

| Path | What it is |
| --- | --- |
| `docs/superpowers/specs/2026-08-06-last-call-templates-design.md` | **The spec.** Every decision and why. Start here. |
| `docs/superpowers/plans/2026-08-06-last-call-plan-b.md` | Executed. The felt surfaces — what slice 2 builds on. |
| `.superpowers/sdd/2026-08-06-last-call-plan-b/progress.md` | **Plan B's ledger** (gitignored). Every task's verdict, every deferred minor. |
| `docs/superpowers/plans/2026-08-06-last-call-plan-a2.md` | Done. The wiring Plan B builds on — SSE contract, private-fetch pattern. |
| `docs/superpowers/plans/2026-08-06-last-call-plan-a-vis.md` | Done. Read only if you need what the preview page proves. |
| `docs/design/last-call/README.md` | Design handoff — exact token values, plain markdown. |
| `docs/design/last-call/*.dc.html` | The prototypes. Open in a browser; big HTML, strip tags to read as text. |

Doc precedence, stated by the bundle itself: `Game UI.dc.html` > `Module Spec` >
`README` for pixel values · `Design Doc v2` for rules · **the walkthrough
(`A Round, Step by Step`) is explicitly non-normative** — its damage numbers are
invented.

## Status

Slice 1 is four plans: **A → A-vis → A2 → B**. Everything after slice 1 was
planned in one pass on 2026-08-11 — see "The rest of the game" below.

## The rest of the game — eight plans, written 2026-08-11; only J remains

All eight are in `docs/superpowers/plans/2026-08-11-last-call-plan-*.md`. Each
was written by its own subagent against the spec, DDv2 and the earlier plans'
Produces blocks, and each opens with a **`## Proposed design decisions —
awaiting user review`** section. All 106 proposed decisions were
**user-approved 2026-08-11** ("as far as i can tell they seem good to me.
approved") and the plans committed at `b5aa789`.

- **Plan C — the hand group. EXECUTED 2026-08-12.** `856db68..83cccb9`,
  8 commits, verify green, **381 tests**, `drinkinggame` clippy-clean.
  Whole-plan review (the only review — all five tasks were Class A/B)
  returned CHANGES_REQUIRED (0 critical, 2 important, 7 minor); one fix wave
  fixed all nine; scoped re-review CLEAN. Ledger:
  `.superpowers/sdd/2026-08-11-last-call-plan-c-hand-group/progress.md`.
  **Two naming/contract notes for later plans:** the CostRail shipped as
  `.lc-costrail*` (the plan's `.lc-rail*` collided with the big screen's
  side rail — the STATUS seam's pre-approved rename); and `lc:arm` fires
  *before* the wheel's glide settles — Plan E's listener must not assume
  the wheel is at rest. **Owed to a human: the browser checkpoint** —
  preview group 8 (`/drinks/lastcall/preview`) and the live phone HAND tab:
  drag/snap/notch feel, rail tap-to-jump, armed tap dispatch (devtools:
  `lc:arm` events logged), locked suppression, reduced-motion, and the
  camera-not-card behavior when a hand shrinks mid-view.

- **Plan D — the loop engine. EXECUTED 2026-08-12.** `cba4300..788d8fc`,
  8 commits, verify green, **424 tests**, `drinkinggame` clippy-clean. The
  engine plays a complete round end to end under unit test; every
  `NotImplemented` stub is dead. Whole-plan review APPROVED (1 important,
  9 minor); fix wave closed the important + 6 minors; scoped re-review
  CLEAN. **New decision D19 (post-review, awaiting user eyes):
  `set_handicap` is Draw-beat-gated** — a post-lock handicap raise could
  inflate a locked play's reveal charge *and* buy 7.1 initiative; the gate
  restores lock-time/reveal-time agreement. **Carried into Plan E:** wire
  `staged_for(seat)` into `hand_pane_html` (else the armed column shows
  `LOCKED 0` after lock), the lock_in-replay-returns-WrongBeat handler
  note, and `reveal()`'s direct-index hardening. **Carried into Plan F:**
  fixtures for same-resolve double elimination and the `table` targets
  class (the placeholder catalog cannot drive either).

- **Plan F — the catalog and the damage scale. EXECUTED 2026-08-12.**
  `164d973..b837cb3`, 5 commits, verify green, **442 tests**, `drinkinggame`
  clippy-clean. The game has real content: 40 distinct cards, copy-weighted
  40-card shoes, curated openers, per-card fx via catalog-side `card_fx`
  lookup (blob carries identity only — a balance retune reaches in-flight
  games instantly), `EffectOp::PullDrain` with engine support, D8's
  kind-mapping constants dead, keyword contract bidirectional, §9 floor
  pinned. Both of Plan D's carried fixtures landed (its M8 is closed).
  Whole-plan review APPROVED (0 critical, 0 important, 4 minor — all fixed;
  the reviewer verified all 40 cards against the F12 table field-by-field,
  zero transcription errors); scoped re-review CLEAN. **One F3 prose
  erratum** in the plan file (Liquor dmg/pull arithmetic; code was always
  right). Owed to the same human browser pass as C: the preview page's
  catalog groups now show 40 cards.

- **Plan E — the loop wiring. EXECUTED 2026-08-12.** `e49ab7e..d9dd89c`,
  11 commits, verify green, **470 tests**, `drinkinggame` clippy-clean.
  **The game is playable end to end:** START ROUND out of the registration
  lobby, draw, arm via the wheel, target, LOCK IN, the beat clock advancing
  on the 1 Hz ticker, reveal on the felt with flights, HP moving,
  elimination, frozen game-over tableau, END GAME handoff. Tasks 1–3 were
  Class C with per-task reviews (each closed after ≤1 fix round — the
  reviews caught a real infinite-spin bug in the advance chain and forced a
  deterministic ticker/route contention test); whole-plan review
  CHANGES_REQUIRED narrowly (1 important: error notes rendered raw markup);
  fix wave + scoped re-review CLEAN. Both SSE debts are paid (synthetic
  lag `lctick "0"`; flight layer lives in the static shells).
  **Standing items:** (1) *user decision:* every private arm/disarm bumps
  the public `data-seq` via `LcTick`, costing each phone two fetches per
  private action — brief-mandated, revisit if rooms feel chatty; (2) the
  ticker task is unsupervised (a panic would silently stop all beat
  clocks — hardening candidate for Plan J); (3) **the phone TABLE tab
  cannot show reveal/draw flights** — E15 keeps play data off the phone, so
  this is real scope, owner Plan I/J; checkpoint 2 step 2 is amended
  accordingly; (4) the phone banner swap is not seq-guarded (pre-existing —
  a lag backlog can briefly rewind the banner); (5) the big screen ignores
  `lctick` BY DESIGN (private actions must not repaint it).

- **Plan G — pacts. EXECUTED 2026-08-12.** `b5314f6..1ec3d7a`, 10 commits,
  verify green, **495 tests**, `drinkinggame` clippy-clean. Secret pact
  negotiation during Diplomacy, betrayal loud-by-name for exactly one
  visible round, dissolution incl. elimination pruning, the
  `LcOutcome::Pact` shared win; tick-only routes with a structural secrecy
  proof (Task 4's Class C review: the guarantee is the `PublicView` type
  boundary, verified). **One plan erratum:** the original break-strip
  filter made non-terminal betrayals invisible (stamp/rollover
  interleaving) — found by Task 3's implementer, fixed with a re-stamp at
  rollover, erratum in the plan file. **For the user's G-decision review:**
  (1) the withdrawn-offer channel — when a pair pacts, a third party
  courted by both sees a double-vanish identifying the pair; accepted as
  bounded (every alternative leaks worse); (2) mutual same-resolve
  betrayal is order-asymmetric (only the faster knife is barred);
  (3) double-tap ACCEPT surfaces the blander of the two mandated copy
  bodies. All three recorded in the ledger too.

- **Plan H — events and tabs. EXECUTED 2026-08-12.** `be2ab9e..4631d0a`,
  11 commits, verify green, **532 tests**, `drinkinggame` clippy-clean.
  Seven public round events (seed-stepped, deterministic, never two at
  once) + seven private tab objectives (dealt at seating, detected in
  resolve, settled name-only) — both as 7-entry catalogs whose count is
  test-pinned. The event cost seam (`effective_pull_cost`) now covers
  every price surface incl. the CostRail (a review-caught plan gap — H12
  omitted that site). **One plan erratum** (final-round tab settles were
  invisible — same class as G's; fixed reader-side to keep the settle
  round durable for Plan J's LOG). **Two post-review decisions awaiting
  user eyes:** H15 — betrayal is INTENT-based under redirecting events
  (aiming at your partner breaks the pact even if Double Vision redirects
  the damage; being redirected onto them does not) — controller-ruled,
  test-pinned both directions; and the F.2 ruling that the 88px screen
  header never grows — banner children constrain (one-row settle strip
  with a "+n" chip).

- **Plan I — reactions and ghosts. EXECUTED 2026-08-12.** `7729dc6..1503731`,
  10 commits, verify green, **562 tests**, `drinkinggame` clippy-clean. The
  Reveal beat is the response window (opens unconditionally so holding a
  reaction leaks nothing); `ReactionFx {Cancel, Reduce, Reflect}` arms F's
  five reaction cards with real text; LIFO fizzle, TBD-7 structural;
  `REACT_GRACE_SECS` (10) window extension with a structurally
  deterministic ticker-race harness; ghosts get §9.2 verbatim (one +1
  haunt per round); response surfaces inside existing panes. **Rulings
  applied in the fix wave:** cancelled knives still betray (H15
  intent-based, applied through Cancel); reactions now prompt the DRINK
  chip (I7 already charged them — the UI just never surfaced it).
  **Parked for the user's design review:** reaction spend in
  TabCheck predicates (balance); `NoPlays` ignores reactions; haunt
  vengeance intent (brief-literal today: any Damage play in flight);
  the H15-through-Cancel ruling itself.

| Plan | File suffix | What it ships | Tasks |
| --- | --- | --- | --- |
| **C** | `plan-c-hand-group` | HandWheel + ArmedColumn + CostRail on the HAND tab; `lc_wheel.js`; `lc:arm`/`lc:disarm` CustomEvent hooks for E | 5 (all A/B) |
| **D** | `plan-d-loop-engine` | The stubbed transitions implemented, pure-engine: draw/deal, arm/disarm/`set_target`/lock, `advance_beat`, `resolve()`; `EffectOp {Damage, Heal, Shield, Dot}`; `LcOutcome`; hidden `locked_plays` (§3.4.1); `from_json` seat cap (the pre-deploy item) | 5 (all B) |
| **F** | `plan-f-catalog` | The REAL catalog: 5 decks × 8 distinct cards, copies to `LC_DECK_SIZE` 40; effects as catalog-side `card_fx(id)` lookup (no blob migration); new op `PullDrain` with engine support; balance: par 2 dmg/pull vs HP 15 | 4 (B/A) |
| **E** | `plan-e-loop-wiring` | Playable game: action routes under `RoomLocks`, beat clock on the 1 Hz ticker (`beat_deadline_ms`), F.1 action bar, reveal flights, minimal game-over; closes the flight-layer debt and the `Err(_) => None` lag arm | 5 (3 C, 2 B) |
| **G** | `plan-g-pacts` | Pacts: secretly negotiated during Diplomacy, rewire the endgame to a shared two-player win (`LcOutcome::Pact`); betrayal is public by name | 4 (3 B, 1 C) |
| **H** | `plan-h-events-tabs` | 7 public round events + 7 private tab objectives, both as id+lookup catalogs; deterministic seed-stepped event selection; rewards pay HP/pulls, never winning | 5 (3 B, 2 C) |
| **I** | `plan-i-reactions-ghosts` | The Reveal beat becomes the response window (opens unconditionally — a conditional window would leak who holds reactions); `ReactionFx {Cancel, Reduce, Reflect}` arms F's five inert reaction cards; ghosts get DDv2 §9.2 verbatim: one +1 haunt per round | 5 (4 B, 1 C) |
| **J** | `plan-j-finish` | Public-only game log (capped 80) filling the LOG tab; the designed end-of-game screen + REMATCH; lobby polish; verdict on every carried cosmetic (plaque → 196px, deck list row built, redirects fixed) | 5 (2 C, 3 B) |

**Execution order — binding:** C → D → F → E, then G / H / I in any order
(they commute: G is beat 3, H is beat 2, I is beat 5, and all three add only
container-default fields), then **J last** (its log emission and end-screen
stats hook into whatever G/H/I shipped). F runs before E so E's wiring samples
the real shoe; F deliberately keeps beer/cider/soft magnitudes coincident with
D's placeholder mapping so D's resolve suite survives unedited.

**Cross-plan seams, found during writing — the executor of the *second* plan
in each pair owns the reconciliation:**

- **C ↔ existing CSS:** Plan C's CostRail root class `.lc-rail` collides with
  the big screen's existing `.lc-rail` (`lastcall.css:700`). Owned by Plan C's
  execution/fix wave — rename one.
- **C ↔ D:** D17 proposes `armed: Vec<ArmedCard>`; C's `HandGroupView` takes
  `armed: &[Card]`. Whichever executes second reconciles `hand_pane_html`.
- **J's public-only log binds G/H/I:** no log line may carry secret content —
  G routes pact lines through the private fetch, H announces tab completion
  name-only, I is safe by construction (reactions/haunts are public the moment
  they exist). J may need a small task adding log-emission calls into G/H/I's
  shipped transitions; it says so.
- **H ↔ E:** H's `effective_pull_cost`/`charged_pulls` seam retunes Plan E's
  DRINK chip. H runs after E; H owns it.

- **Plan A — component library. DONE.** `85f2552..f80615a`, 7 commits,
  `verify.sh` green, 275 tests. Object model + `PublicView`, `lastcall.css`,
  `lc_render.rs` components to the §7.8 DOM contract.
- **Plan A-vis — motion + preview. DONE.** `b5f4472..42e24c8`, 4 commits,
  `verify.sh` green, 296 tests. Seven keyframes + `lc_motion.js`'s
  `lcFlight`/`lcAnchor`, and `GET /drinks/lastcall/preview` — seven groups,
  permanent, public, no session. **There is now a URL you can open.**
  `verify.sh` also now runs `node --check` over `drinkinggame/assets/*.js`.
- **Plan A2 — game wiring. DONE.** `5e1de5a..1d8efe8`, 6 commits, `verify.sh`
  green, 327 tests. `last_call` is a third `games.kind`; the room's idle panel
  has a third start card; `GET /room/{code}` redirects members to the F.1 phone
  shell; each viewer fetches **its own** hand over a route that takes no player
  identifier; and an `lcpublic`/`lctick` SSE pair repaints every phone with a
  highest-seq-wins stale-drop rule. **Slice 1 is playable up to the setup form.**
- **Plan B — felt surfaces. ALL SIX TASKS COMPLETE; NOT YET SIGNED OFF.**
  `52c8d62..1f295be`, 12 commits, `verify.sh` green, **371 tests**, 17 distinct
  clippy warnings. Seat-ring geometry (B), felt/ring/mini-table CSS (A), the two
  table assemblers (B), the spectator big screen + kind branch (C), the
  per-viewer TABLE tab (C), and the seat ceiling + end route (C). A Last Call
  game now has a table you can look at: a spectator opens `/room/{CODE}/screen`
  and sees seats on the felt with live HP, hand sizes, deck counts and the phase
  banner; every player's TABLE tab shows the same state at phone scale with
  themselves at the bottom; the seat ceiling is enforced; and a game can be
  ended without ending the night.

  **Why "not signed off" and not "done"** — the plan's own "Before this plan is
  called done" list has two open items:

  1. **Neither browser checkpoint has been run by a human.** Checkpoint 1 never
     happened (the ledger records Task 4's `CANNOT_VERIFY` as "owed to browser
     checkpoint 1, steps 2-3"), so everything lands on checkpoint 2: its six
     items, **plus** the two owed from Plan A-vis (watch a REPLAY flight
     travel; repeat under devtools' "Emulate `prefers-reduced-motion: reduce`"),
     **plus** Task 4's `CANNOT_VERIFY` rendered-layout check under
     `display: contents` at 1920×1080, **plus** Task 5's rotation cross-check
     (two phones side by side: same names, same HP, different rotation).
     Automation cannot substitute — its tab stays backgrounded, which freezes
     animations.
  2. ~~The whole-plan review.~~ **Returned CHANGES_REQUIRED — one major (the
     seat ring rendered at 86%×82% of design) and six minors, all now fixed.**
     See "Whole-plan review outcome" below. Checkpoint 2 should be run against
     the *fixed* ring, which has moved since the review.

  Contrast Plan A2, which says DONE and means it: both of its browser
  checkpoints were run against a live server with two real sessions. (Plan
  A-vis says DONE with two items still owed — they are listed under "Resume".)

  Three findings that shaped it, kept so they are not re-derived:

  - **The phone's mini table is viewer-relative.** D.2's "the local player is
    always nearest the viewer" is a per-viewer *rotation*, which a `RoomHub`
    broadcast structurally cannot carry and `personalize()` cannot fake — it
    hides elements, it does not re-position seven plaques. That is why spec §10
    listed a `…/lastcall/table` route at all. So the big screen rides `lcpublic`
    (one absolute layout, no viewer) and the phone fetches (one layout each).
  - **The big screen needs no new SSE event.** `lc_public_panel` gains a second
    `<template data-lc-screen>` block: same event, same frame count, and
    `broadcast_lc` stays await-free under the room guard.
  - **The bundle's seat ring is parametric after all.** Six of the seven authored
    seats sit within 4% of the felt's inner hairline ellipse (semi-axes 568×408
    about (660, 496) in the 1320×992 centre column); only the bottom seat is
    pulled in to r ≈ 0.85, which is D.2's local-player rule showing up as
    geometry. The angles are authored rather than evenly stepped, leaving
    top-centre empty. The plan transcribes the n = 7 row and generates the rest.

## Decisions not to re-litigate

- **Templates before wiring.** Deliberate. Plan A produces nothing viewable —
  that is why A-vis comes immediately next.
- **Own phone shell**, not the existing `room.html`. DDv2 §14 says reuse it;
  Module Spec F.1 specifies a different fixed vertical order. F.1 wins on the
  bundle's own precedence; §14 is read as reusing the *transport*.
- **Hidden info is fetched per viewer, never broadcast.** `RoomHub` is a
  per-room broadcast and `personalize()` is only a visual hide. The private hand
  route takes **no player identifier at all** — identity comes from the session
  cookie, so "can A fetch B's hand?" is unanswerable rather than guarded.
- **Public renderers take `&PublicView`**, never `&LastCallState`. Same move:
  remove the input rather than check it.
- **Renderers emit deck class names, never hex.** CSS owns colour. A test
  rejects `#` in renderer output.
- **No migration.** `games.kind` has no `CHECK` and `state_json` already exists.
- **Component vs positioning:** a component renders from its own data (Plan A);
  its placement depends on table state (Plan B). Plan B assembles, authors none.
- **Draw badge uses `--lc-ink-59`, not the README's `<fill>22`.** On Wine, 13%
  `#8B2F4A` over `#16121F` is invisible. Adjudicated; comment is in the CSS.
- **Beat "Deal" has no hue** in the bundle; it inherits Draw's amber.

### Carried out of Plan B — read before slice 2

Tasks 4, 5 and 6 were Class C and each got its own reviewer; Tasks 1–3 (Class
A/B) had none by policy and are covered by the whole-plan review below. The full
deferred-minor triage list — roughly two dozen entries, one block per task —
lives in the ledger, `.superpowers/sdd/2026-08-06-last-call-plan-b/progress.md`,
which is gitignored. **These two must not stay only there.**

- **`from_json` is an uncapped THIRD path into `players`** (`last_call.rs:390`).
  Both *live* paths now cap at `MAX_SEATS`; deserialization does not. This is
  minor **today only because no Last Call state blob predates the ceiling — the
  game has never been deployed. That predicate expires the moment this branch
  reaches master.** After that, a state persisted by an older binary
  deserializes with every seat it had, `seat_pos`'s `.get()` renders short, and
  a real player's plaque vanishes from the felt with no error anywhere.
  **Treat as a pre-deploy item, not a slice-2 nicety.**
- **`.game-idle` is a presentational class doing control-flow duty**
  (`lc_room.html:205`). The residual risk is not the name; it is that the
  consumer is inline `<script>`, which neither `node --check` nor any harness in
  this repo reaches. That is a standing property of the repo rather than
  something Task 6 introduced — but this exit path is the first piece of phone
  JS whose failure is silent and total.

**Three gaps Plan B recorded rather than closed.** All three are "that would be
authoring a component", which the plan forbids — Plan B assembles Plan A's
components and authors none. Whoever next revises those components owns them:

- **The plaque renders at 204px**, as Plan A shipped it. `Game UI.dc.html`'s v2
  big-screen mockup draws it at 196px, and Game UI outranks the Module Spec on
  pixels — but re-rendering a shipped component is authoring.
- **The compact deck *list row*** the v2 mockup draws for the right rail (9px
  dot, name, `disc n`, big count) was not built. The rail uses Plan A's
  `deck_stack()` / `discard_slot()` instead; the list row would be a sixth
  rendering of deck state that Plan A never shipped.
- **The felt centre is empty.** Nothing in the bundle specifies what goes there.

### Whole-plan review outcome

One review of the whole branch diff (`52c8d62..1f295be`) on the most capable
model, per `plan-economics` §4 — the only coverage Tasks 1–3 ever receive.
Brief: `.superpowers/sdd/2026-08-06-last-call-plan-b/review-PLAN-brief.md`.
Full report: `…/review-PLAN-report.md`.

**Verdict: CHANGES_REQUIRED.** One major, six minor. **Fix wave applied
2026-08-11** — `verify.sh` green, 371 tests, 17 distinct clippy warnings. Done
inline rather than delegated (the changes were two CSS values, one test body,
one test helper swap, a CSS deletion and three comments — all fully specified by
the reviewer), per `plan-economics` §2 and the precedent set by Task 6's fix
round.

**The major — the seat ring renders at 86% × 82% of design.**
`lastcall.css:746` and `:776` apply `inset: 8.9% 7.0%` to `.lc-ring` /
`.lc-minitable-ring`, but that inset is **already baked into Task 1's
coordinates**. This is a cross-task interface mismatch, not a mistake inside
either task: `lc_layout.rs` says "box-relative", Task 2's CSS comment reads that
as *the ring box*, and the table is in fact **stage-relative**. Two independent
proofs:

- The extreme coordinates are the ellipse's extremes measured against the
  1320×992 stage. `50% ± 568/1320 = 6.97 / 93.03` and `50% ± 408/992 =
  8.87 / 91.13` — which is precisely the n=4 row `(7.0, 50.0)`, `(93.0, 50.0)`
  and the n=2 row `(50.0, 8.9)`, `(50.0, 91.1)`.
- Read as ring-box-relative instead, n=7's seat 2 `(8.9, 47.6)` sits at
  r ≈ 0.82 of the semi-axis — contradicting `lc_layout.rs`'s own doc comment
  that "six of its seven seats sit within 4% of the felt's inner hairline
  ellipse". Only the stage reading makes that sentence true.

Rendered error, recomputed independently: the bottom seat should sit at 84.9% of
stage height and lands at `8.9 + 0.849 × 82.2 = 78.69%` — **61px high** on a 992
stage; the leftmost seat comes in ~75px at 1920×1080.

**Fixed:** `inset: 0` on both rules (`.lc-ring` is a direct child of
`.lc-stage`, `lc_render.rs:528`, so the ring box becomes the stage box and the
numbers are exact), both comments rewritten with the derivation, and an ERRATUM
block added above the plan's CSS snippet — the plan's own wording
("percentages of the ring box") is what caused the defect, so it is corrected in
place rather than silently rewritten.

**This is also why browser checkpoint 1 mattered.** It is a purely visual defect
in generated geometry — every test passes, because the tests check the
*formula*, and the formula is right. Only a human looking at a felt catches it.

All six minors fixed too: the vacuous `test_the_felt_centre_holds_no_plays` now
carries a real `revealed` play and asserts on both the class and the card's
text; the five dead `.lc-minitable-row*` rules are deleted (nothing emitted
them) and the comment that described a discard row no code renders is rewritten;
`for _ in 0..4 { body.next() }` became `read_sse_until` cut at the frame
boundary; the weak `class="game-idle"` substring now goes through
`matches_the_game_idle_selector`; the flight-layer debt has the named line
below; and the "first deck still in the shoe" comment now says what
`deck_counts.first()` actually does (it does **not** skip exhausted decks).

**Both touched tests were mutation-verified**, because a green suite after a fix
is not evidence the fix is load-bearing — that was the finding:

```
MUTATION 1  lc_screen_panel renders card_face per view.revealed
  test_the_felt_centre_holds_no_plays ... FAILED
  "the felt centre rendered a card face"

MUTATION 2  end_room_handler publishes class="screen-panel game-idle" before Ended
  test_ending_the_room_never_races_the_game_idle_redirect ... FAILED
  CONTROL: the same mutation against the OLD substring needle ... ok (green)
```

Mutation 2's control is the point: a **multi-class** root is exactly what
`class="game-idle"` cannot see, so the swap is strictly stronger rather than
cosmetic. Tree restored after both.

**Verified correct** (worth knowing, so it is not re-checked): the component
boundary held — no new `.lc-*` builder; Task 1's generated rows reproduce the
parametric formula exactly; `broadcast_lc` is still await-free; publish order,
frame count and the absence of positional frame indexing all hold; no new SSE
event; the per-viewer route takes no player identifier; all three scene roots
are positioned; the no-hex guard now covers 16 builders; one reduced-motion
block; the `data-lc-live` handoff is structurally checked on both sides.

**Flight-layer debt — one named owner needed.** Tasks 3, 4 and 5 each deferred
the same item onto a future task: every `lcpublic` repaint destroys
`#lc-flights` mid-flight, dropping `onArrive`
(`lc_render.rs:570-592`, `lc_room.html:26-31`). Three deferrals, one unnamed
owner, durable only in a gitignored ledger — which is why it is written here.

### Carried out of Plan A2 — read before Plan B

All three A2 tasks were Class C and each got its own reviewer on the strongest
model. All three returned spec ✅. What they left behind:

**Plan B closed both — do not re-fix.**

- ~~**`MAX_SEATS` is deliberately unenforced.**~~ **Closed by Task 6.**
  `add_player` now returns `None` past the ceiling (`last_call.rs:427-429`) and
  is internally idempotent (an already-seated player gets its existing seat
  back). A ninth visitor reaches the room, is not seated, and the felt still
  renders eight plaques — pinned by
  `test_a_ninth_member_can_still_open_the_room`, which was **mutation-verified**
  (guard → `if false` makes it red).
- ~~**There is no `/lastcall/end` route.**~~ **Closed by Task 6.**
  `lc_end_handler` ends the game while keeping the room open, and publishes a
  `game` frame the phone's `.game-idle` selector acts on — also
  mutation-verified (swap `idle_panel` for a stub and
  `test_ending_publishes_a_game_frame_the_phone_acts_on` goes red).

**Already handled — do not "fix" it again.** `#lc-flights` needs a positioned
ancestor on every scene root, and both roots now have one: `body.lc-preview` from
Plan A-vis and `body.lc` from Plan A2, each pinned by its own test. If Plan B
introduces a *third* scene root it needs the same, because the layer is
`position:absolute; inset:0; overflow:hidden` and without one it clips to the
first viewport — flights created with correct deltas and never rendered.

**Also closed by Plan B** (it was taken early — Task 6 was already inside that
function).

- ~~**`add_player` does not bump `seq`.**~~ **Fixed:** `last_call.rs:446` bumps
  it. Two distinct states can no longer share a seq, so the client's equal-seq
  allowance (which exists so a duplicate repaint is harmless) can no longer
  admit a stale one. Slice 3 does **not** own this any more.

**Smaller carried minors.** `lc-setup-decks` has no CSS rule (no gap between a
multi-deck player's dots). `lc_routes.rs`'s redirects interpolate the raw path
`code` rather than `ctx.room.code`. The rows-and-hand lookup is duplicated
verbatim between `lc_page` and `lc_hand_handler`. `from_json`'s `expect` panic
surface now reaches the unauthenticated SSE route — a pre-existing crate idiom,
not new here. The `Err(_) => None` lag arm still *drops* an `LcTick` without
re-fetching; only its misleading comment was corrected, because changing the
behaviour is slice 3's call.

**Four things worth keeping, learned from the A2 reviews:**

- **A shell-only broadcast reaches nobody who is not yet on the shell.** The
  plan had `persist_and_broadcast_lc` publish only the room panel plus the Last
  Call frames, which made pressing START a complete visual no-op on every phone
  including the starter's — `room.html` has no `lcpublic`/`lctick` listener and
  the room frame is inert for `last_call`. Fixed by publishing the game panel
  too, exactly as `tm_start_handler` does. **Ask "who is subscribed to this, and
  what are they currently looking at?" before choosing which frames to publish.**

- **`broadcast_lc` has no await points**, so both publishes are *synchronous*
  under the room guard. That is a stronger property than "the lock is held" —
  there is no suspension point for another task's broadcast to interleave at.
  It is the structural answer to `1e742d4`, and Plan B should preserve it.
- **The publish order is `room` → `lcpublic` → `lctick`**, because
  `persist_and_broadcast_lc` calls `broadcast_room` first. Tests that assert on
  frames must **filter**, never index positionally.
- **A named SSE event with an empty data buffer is silently dropped** by the
  browser's EventSource parser. `lctick` sends `seq.to_string()` for that reason,
  not because the payload is the information — the event is.

## Open / parked

- **Post-v1 (user-owned): challenge-card mechanics.** After first release the
  user will redesign some card elements around real-life party challenges —
  cards requiring players to say / do / challenge / solve something under a
  time limit. Engineering hooks already in place: catalog-side `card_fx`
  resolves unknown fx **inert by design**, so challenge cards can ship
  text-first; the beat clock's deadline + ticker pattern is the timed-flow
  precedent. Recorded 2026-08-12.

- **Spec §3.4.1 binds slice 3:** `public_view()` decides revelation from the beat
  alone, so **nothing may enter `plays` before it is revealable**. Hold locked
  plays in a field the projection cannot read; that slice owns the test.
- **Parked minor:** `.lc-face-kws` gap is fixed but not pinned by a test. Plan
  A-vis looks at the chips, which is better evidence.

### Carried out of Plan A-vis's review — read before Plan B

Its three tasks were all Class A, so one whole-plan review on the strongest model
was the only review. It returned CHANGES_REQUIRED; everything was fixed in one
wave (`42e24c8`) and a scoped re-review verdicted **all addressed, no new
breakage**. Three findings were deliberately *not* fixed:

- **Duplicate ids / flight anchors on the preview page.** Mostly plan-mandated —
  the page shows the same component in several states, and each carries its
  anchor. `lcAnchor` returns the first match, which is fine for a gallery and
  would not be on a real table. **Plan B must not inherit the pattern.**
- **The at-rest flight swatches go `opacity: 0` under reduced motion.** The fix
  needs a second `@media (prefers-reduced-motion: reduce)` block, which the plan
  forbids for a good reason (the second silently overrides half the first). If
  this ever needs fixing, fix it *inside* the one block.
- **`.lc-preview-grid` went 220px → 200px** after the checkpoint that validated
  the layout, so the clamped/expanded boundary pairs have not been re-eyeballed
  at the new width.

Two lessons worth keeping:

- **`#lc-flights` needs a positioned ancestor.** It is `position:absolute;
  inset:0; overflow:hidden`, so without one it forms its containing block against
  the viewport and clips every flight beyond the first screenful — the nodes are
  created with correct deltas and never rendered. `body.lc-preview` carries
  `position: relative` and a test now guards it. **Plan B's real felt scene needs
  the same thing**, and it is invisible in every synthetic test that only checks
  the flight lifecycle.
- **`lastcall.css` has no width media query at all.** Oversized samples are held
  by `.lc-preview-scroll` wrappers plus a `min-width: 0` chain rather than by
  breakpoints. Fine for a style guide; decide deliberately for real surfaces.
- **No cards exist.** Five decks have bands, pulls, cost spreads and roles, but
  no real card list, text or damage numbers anywhere in the bundle. The current
  catalog is deliberately adversarial placeholder data. This is the true blocker
  for playability, and it is content work, not code.
- **Never designed:** join/lobby, LOG tab content, end-of-game, card art.
- **Hollow systems**, each needing rules before building: events, tabs, pacts,
  ghosts, reactions, effect keywords.

## Working rules

- `./scripts/verify.sh` is the only gate. Baseline after Plan B: green,
  **371 tests** (49 portfolio + 4 `static_assets` + 167 `drinkinggame` unit +
  151 `http`), and **17 distinct** clippy warnings — all in `drawingportfolio`
  (`models.rs` ×10, `nutrition.rs` ×3, `db.rs` ×2, `middleware.rs`, `auth.rs`);
  `drinkinggame` is clean. Counting the warnings is fiddly: grepping the whole
  verify log for `warning` gives 32, because the rustc dead-code warnings are
  emitted once by clippy and again by the test build, and four lines are
  per-target rollup summaries. Two distinct sites both read
  "field `id` is never read". Compare against **17**, per CLAUDE.md.
- Invoke `plan-economics` before writing or executing a plan. Class A/B tasks
  get **no** per-task reviewer — one whole-plan review at the end. Class C
  always gets one.
- Delegate implementation; the controller holds no plan text.
- SDD ledger per plan: `.superpowers/sdd/<plan-basename>/progress.md` (gitignored).
  A task with a `complete` line is done — do not re-run it.

## Resume

```
cd /home/hampter/projects/drawingportfolio.worktrees/last-call
git log --oneline -8
./scripts/verify.sh          # NOT bare `cargo test` — that runs 52 of 327
cargo run -p drinkinggame   # standalone on :3001

# the style guide — no login:
#   http://localhost:3001/lastcall/preview
# the game — two browser profiles, log in as two players, create a room in one,
# open the room URL in the other, press START on the Last Call card:
#   http://localhost:3001/room/{CODE}
```

Then, in this order:

1. ~~Fix wave for the whole-plan review.~~ **Done 2026-08-11**, all seven
   findings, both touched tests mutation-verified. `verify.sh` green.
2. **Sign off Plan B.** Run browser checkpoint 2 — the plan's six items, plus
   everything listed under "Plan B — ALL SIX TASKS COMPLETE" above. A human in a
   real, focused browser; ~15 minutes with four sessions. **The seat ring moved
   in the fix wave**, so this is now also the first human look at the corrected
   geometry: seats should sit ON the felt's inner hairline ellipse, with the
   bottom seat pulled slightly inward (r ≈ 0.85, D.2's local-player rule).
3. ~~Write slice 2's plan.~~ **Done 2026-08-11 — and so is every other plan.**
   All eight remaining plans (C through J, covering every slice to completion)
   are written; see "The rest of the game" above.
4. ~~User design review.~~ **Approved 2026-08-11**, all 106 decisions.
5. **Execute** in the binding order: ~~C~~ → ~~D~~ → ~~F~~ → ~~E~~ → ~~G~~
   → ~~H~~ → ~~I~~ → **J last**. C through I executed and review-clean
   2026-08-12 (see "The rest of the game"). Browser checkpoints owed to a
   human, one combined pass: Plan B checkpoint 2, C's hand group, F's
   40-card preview, E's loop items (step 2 amended — no TABLE-tab
   flights), G's pact flow, H's banner strip + private tab card (incl.
   the tab card's position below the 480px hand group), I's response
   window + ghost bar + centre chips. Each plan names its ledger at
   `.superpowers/sdd/<plan-basename>/progress.md`.

Spec §3.4.1 (nothing enters `plays` before it is revealable) is owned by Plan
D, which holds locked plays in a hidden `locked_plays` field and carries the
mandatory secrecy test.

**Still owed on Plan A-vis:** browser checkpoint 2 items 6–7 — press every
REPLAY and watch a flight actually travel, then turn on devtools' "Emulate CSS
prefers-reduced-motion: reduce" and press them all again. Both were verified
structurally (nodes created with correct deltas and stagger, layer drains to
empty on `animationend`, zero nodes and `onArrive` still firing under a stubbed
`matchMedia`) but never watched by a human eye — the automation tab stays
backgrounded, which freezes animations. Plan B's checkpoint 1 was supposed to
fold these in and never ran, so they roll forward to checkpoint 2. Five minutes in a real browser.

**Nothing is owed on Plan A2.** Both its browser checkpoints were run against a
live server with two real sessions: the redirect, the F.1 shell, hand privacy
(disjoint card sets, neither player's ids in the other's page), the
`?player_id=`-is-byte-identical property, Ring of Fire and 3 Man behaving exactly
as before, the five-frame vs four-frame SSE snapshot, one hand fetch per change,
the stale-drop rule, form state surviving a repaint, and START now landing on a
phone that never reloaded.
