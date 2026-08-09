# Last Call — where we are

Resume card. Point a fresh session at this file.

**What it is:** a third game mode for the `drinkinggame` crate, alongside Ring of
Fire and 3 Man. Players join a room on their phones, register what they are
drinking (which picks their deck), and spend *pulls* — sips — to play cards.
Six beats to a round, out at 0 HP.

**Branch:** `master-3`, based on `origin/master` @ `8931ed0`.

## Read these, in this order

| Path | What it is |
| --- | --- |
| `docs/superpowers/specs/2026-08-06-last-call-templates-design.md` | **The spec.** Every decision and why. Start here. |
| `docs/superpowers/plans/2026-08-06-last-call-plan-a2.md` | **Next plan to execute.** |
| `docs/superpowers/plans/2026-08-06-last-call-plan-a-vis.md` | Done. Read only if you need what the preview page proves. |
| `docs/design/last-call/README.md` | Design handoff — exact token values, plain markdown. |
| `docs/design/last-call/*.dc.html` | The prototypes. Open in a browser; big HTML, strip tags to read as text. |

Doc precedence, stated by the bundle itself: `Game UI.dc.html` > `Module Spec` >
`README` for pixel values · `Design Doc v2` for rules · **the walkthrough
(`A Round, Step by Step`) is explicitly non-normative** — its damage numbers are
invented.

## Status

Slice 1 is four plans: **A → A-vis → A2 → B**.

- **Plan A — component library. DONE.** `85f2552..f80615a`, 7 commits,
  `verify.sh` green, 275 tests. Object model + `PublicView`, `lastcall.css`,
  `lc_render.rs` components to the §7.8 DOM contract.
- **Plan A-vis — motion + preview. DONE.** `b5f4472..42e24c8`, 4 commits,
  `verify.sh` green, 296 tests. Seven keyframes + `lc_motion.js`'s
  `lcFlight`/`lcAnchor`, and `GET /drinks/lastcall/preview` — seven groups,
  permanent, public, no session. **There is now a URL you can open.**
  `verify.sh` also now runs `node --check` over `drinkinggame/assets/*.js`.
- **Plan A2 — game wiring. NEXT.** 3 tasks, all Class C, so every one gets a task
  reviewer. Game kind, setup form, entry redirect, phone shell, private hand
  route, SSE contract.
- **Plan B — felt surfaces.** Not written yet; write it fresh from the spec
  after A2, per `plan-economics`.

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

## Open / parked

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

- `./scripts/verify.sh` is the only gate. Baseline: green, 19 clippy warnings.
- Invoke `plan-economics` before writing or executing a plan. Class A/B tasks
  get **no** per-task reviewer — one whole-plan review at the end. Class C
  always gets one.
- Delegate implementation; the controller holds no plan text.
- SDD ledger per plan: `.superpowers/sdd/<plan-basename>/progress.md` (gitignored).
  A task with a `complete` line is done — do not re-run it.

## Resume

```
cd /home/hampter/projects/drawingportfolio.worktrees/master-3
git log --oneline -8
./scripts/verify.sh
cargo run -p drinkinggame   # then open http://localhost:3001/lastcall/preview
```

Then: execute `docs/superpowers/plans/2026-08-06-last-call-plan-a2.md`.

**One thing still owed on Plan A-vis:** browser checkpoint 2 items 6–7 — press
every REPLAY and watch a flight actually travel, then turn on devtools'
"Emulate CSS prefers-reduced-motion: reduce" and press them all again. Both were
verified structurally (nodes created with correct deltas and stagger, layer
drains to empty on `animationend`, zero nodes and `onArrive` still firing under a
stubbed `matchMedia`) but never watched by a human eye — the automation tab stays
backgrounded, which freezes animations. Five minutes in a real browser.
