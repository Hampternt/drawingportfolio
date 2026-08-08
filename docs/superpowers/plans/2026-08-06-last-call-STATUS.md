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
| `docs/superpowers/plans/2026-08-06-last-call-plan-a-vis.md` | **Next plan to execute.** |
| `docs/superpowers/plans/2026-08-06-last-call-plan-a2.md` | The one after. |
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
- **Plan A-vis — motion + preview. NEXT.** 3 tasks, all Class A. Seven keyframes
  and the flight helper, then `GET /lastcall/preview`. **Ends at the browser
  checkpoint — the first time any of this is visible.**
- **Plan A2 — game wiring.** 3 tasks, all Class C, so every one gets a task
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
```

Then: execute `docs/superpowers/plans/2026-08-06-last-call-plan-a-vis.md`.
