# Blueprint planner — S0 canvas spike

**Status:** merged into `dev` 2026-11-05 after the owner's go (PR #28).
Branch `kepler/feat/blueprint-planning-tool`.

## Goal

Prove the hand-rolled canvas before anything is saved. This is slice S0 of
`docs/design/blueprint-planner/README.md`: a throwaway page where the owner can
pan, zoom, drag machines and wire ports together, with mouse and keyboard,
inside a boosted page, on the demo plan from the design canvas. Nothing is
stored. What the spike proves (or breaks) feeds S1 (the model) and S3 (the
editor).

## Decisions

- **Built directly, without a separate go.** The ask was to implement the
  task's work and make the changes rather than describe them. S0 is the only
  slice with no open issue in front of it.
- **Route: `/blueprint-spike`, behind `RequireOwner`.** Temporary on purpose.
  The real name and URL are open issue 1, so there is no hub tile, nav link,
  palette command, migration or `/docs` row. Those belong to S2. Signed-out
  visitors get the login redirect and other signed-in users get a 404, like
  `/admin/users`.
- **HTML nodes over an SVG wire layer, in one transformed stage.** The README
  says "hand-rolled SVG". The reviewed canvas (`Plan.dc.html`) draws nodes as
  `<button>`s in a scaled layer with SVG wires under them. The spike follows
  the canvas: buttons give focus, `aria-label` and the site's focus ring for
  free, and the label sizing stays monospace arithmetic. Not chosen: nodes as
  SVG `<g>` + `<text>`, which would need a hand-made focus model.
- **Port layout: one port per row, outputs first** (the canvas's proposal).
  **Still open for the owner.** All of it is confined to `measure()` and
  `portAt()` in `blueprint-model.js`, so switching to Unreal's side-by-side
  rows touches those two functions.
- **1u follows the root font size.** The model works only in grid units. The
  page reads 1rem in px at boot, so a larger browser font makes the whole
  canvas larger. This closes the gap `canvas/README.md` lists (the renderer
  drew 1u as a fixed 16px).
- **Zoom range 0.2 to 3.** The S0 criteria test 0.3 and 3. A 200-node plan
  needs to fit below 0.3.
- **The port hit radius is in screen space** (1.375u on screen, a 2.75u
  target), converted to world units by the zoom. That makes a drop land on a
  port at both 0.3 and 3. When neighbouring targets overlap at low zoom, the
  nearest port wins.
- **A wire dropped on empty grid is cancelled.** The add-node search box is
  S3.
- **Three scripts, not four.** `blueprint-model.js` is pure and tested on
  plain node. `blueprint-view.js` draws. `blueprint-app.js` holds input,
  undo and boot. The README's separate `blueprint-input.js` splits out when S3
  grows the input model (menus, the search box).
- **The demo plan is a committed JSON file**,
  `docs/design/blueprint-planner/demo-plan.json`, loaded with `include_str!`.
  It is the design canvas's `DEMO` document, so S2 can reuse it as the
  visitor demo.

## Guidelines

- Read first: `docs/design/blueprint-planner/README.md` (Decisions, Shape,
  Canvas) and `canvas/README.md` (what the review settled).
- Geometry and colour rules come from the reviewed canvas
  (`canvas/Plan.dc.html`, `canvas/Guide.dc.html`). Match these:
  - grid unit 1u = 1rem
  - header 2u, port row 1.5u, body padding 0.5u, note line 1u, minimum
    width 9u, sizes rounded up to whole units
  - wires: forward `c = max(3u, dx/2)`, backward `c = max(6u, |dx|/2 + 3u)`
  - port hue `30 + fnv1a32(label) mod 300`
  - Factorio-ghost status looks
- Input rules come from `canvas/Input-model.dc.html` and the guide's key
  table:
  - right-drag pans past a 0.25u threshold; middle-drag and Space + drag pan
    too
  - left-drag on empty grid box-selects
  - Space never toggles the selection
  - single-letter keys work only while the canvas has focus, never in a
    field
  - Esc steps back one level at a time
- The hx-boost rules from `CLAUDE.md`:
  - the IIFE files expose `window.BP`
  - a body-targeted `htmx:beforeSwap` tears the instance down
  - document-level listeners are bound once and look the instance up at call
    time, so they never pile up across navigations
- CSS: a named `style.css` section, every length in rem, scoped under
  `body.site-dark` so a `.bp-…` rule beats the design system's `button`
  reset.
- Avoid:
  - persistence of any kind: no migration, no API, no `localStorage`
  - the README's "Avoid" list (exec pins, reroute knots, one-wire-per-input)

## Done when

- Pan, wheel zoom, drag and wiring work with mouse and keyboard on
  `/blueprint-spike`, reached by a boosted link as well as by a full load.
- A wire dropped near a port connects at zoom 0.3 and at zoom 3.
- "200 / 400" loads 200 machines and 400 wires. Dragging a box-selected
  group stays smooth; the readout shows the frame time.
- Leaving the page by a boosted link and coming back leaves one working
  canvas: no doubled key handling, no console errors.
- The model's geometry, hit-testing and validation checks run in
  `./scripts/verify.sh`.

## Test it yourself

No test data to load: the demo plan ships in the page. Sign in as the owner
(`/admin/login`), then:

1. Open `http://localhost:3000/blueprint-spike`. The site's request-flow plan
   is drawn on a blueprint grid and fitted to the view. It has one dashed red
   mismatch wire (`router.ai`, a `Request`, into `token_gate.bearer`, a
   `Header`) and one amber dangling stub
   (`exporter.outline → claude.context`). The counts read
   "8 machines · 12 wires · 1 dangling · 1 mismatch".
2. **Pan:** right-drag, middle-drag, or hold Space and drag. A right-click
   that barely moves does nothing (menus are S3).
3. **Zoom:** wheel over a node. It stays under the pointer. Press `+`, `−`,
   `0` (100%) and `F` (fit). The readout shows the zoom.
4. **Drag:** drag a machine. It snaps to whole grid units and its wires
   follow. Shift + click adds to the selection. Left-drag on empty grid draws
   a selection box.
5. **Wire:** drag from an output dot to an input dot on another machine. Do
   it once zoomed far out (the wheel, or `−` until the readout is near 30%)
   and once zoomed in to 300%. Dropping near the dot is enough. Dropping on
   empty grid cancels. Click a wire to select it; `Delete` removes it.
6. **Keyboard:** click the canvas, then:
   - `Tab` walks the machines
   - arrows move the selection by 1u (Shift: 4u)
   - `]` and `[` walk the focused machine's ports
   - `W` starts a wire from the focused port; Tab to another machine, `]` to
     its input (`[` lands on the last input in one press; `]` walks the
     outputs first), `Enter` to connect; `Esc` cancels
   - `A` adds a machine at the centre of the view
   - `Delete` removes the selection; `Ctrl + Z` and `Ctrl + Shift + Z` undo
     and redo
7. **Load:** click "Load 200 / 400". Press `Ctrl + A`, then drag the whole
   lot. The readout under the canvas shows two numbers. Work per frame
   should stay well under 16 ms. The time between frames should sit at
   your display's refresh interval (about 16.7 ms at 60 Hz); much more than
   that means frames are being dropped.
8. **Boost:** click "How it works" in the nav, then press Back (or open the
   page from the address bar again and navigate away and back). One canvas,
   keys act once.
9. Signed in as a non-owner user, `/blueprint-spike` is a 404.

## Built

Commits `43d4946` (the spike), `c084e1c` (review fixes), a formatting
commit, and a last fix: toolbar buttons now hand focus back to the canvas,
so Ctrl + A straight after "Load 200 / 400" reaches it. All are on
`kepler/feat/blueprint-planning-tool`.

**What changed**

- `src/routes/blueprint.rs`: `GET /blueprint-spike` behind `RequireOwner`,
  with five tests. They cover the demo plan's shape, the script-safe escaping
  round trip, the template's marker and scripts, the owner getting the page,
  and admin and plain members getting a 404 while a visitor is redirected.
  The route is registered in `main.rs` and `routes/mod.rs`.
- `templates/blueprint/spike.html`: on `site-dark`, with `hx-history="false"`.
- `static/blueprint-model.js`: pure, in grid units.
  `tests/js/blueprint-model.test.js` has 279 checks and is now in
  `./scripts/verify.sh`. It pins every node size against the design canvas's
  px formula, and pins drops at zoom 0.3, 1 and 3.
- `static/blueprint-view.js` and `static/blueprint-app.js`.
- A `style.css` section, all rem. It sits after the sorting section, so the
  sorting rem guard in `tests/static_assets.rs` covers it too.
- `docs/design/blueprint-planner/demo-plan.json`.

**Different from the plan or the canvas**

- **Demo layout:** `render` and `exporter` sit 4u further right than in the
  canvas's `DEMO`. The review's width fix made `db` 16u wide, which put it on
  top of `exporter`. A model check now fails if demo machines overlap.
- **The demo's mismatch** is `router.ai` (`Request`) into `token_gate.bearer`
  (`Header`), as in the canvas. The first draft of the test steps named the
  wrong wire; they have been corrected.
- **The load-test plan** wires `x→a` and `y→b` with matching types. Random
  types made 351 of the 400 wires mismatches, so the drag test was a screen
  of red dashes.
- **Partial features:** `>` and `<` follow the first wire only, with no
  chooser for several.

**Review:** one workflow, with three reviewers (input and lifecycle, model
and CSS, server and template) and an adversarial verifier per finding. It
found 8 defects, all confirmed and all fixed in `c084e1c`:

- the keyboard ghost wire was left behind after a delete or reload
- a zoom during a drag fought the drag
- a wire selection survived undo by list position
- an arrow-key run swallowed other edits into its undo step
- pointer positions were off by 1px (the canvas border)
- the port press target was 2.125u, not 2.75u
- Fit clipped the dangling tag
- `connect` and `moveNodes` accepted a deleted machine

The server and template reviewer found nothing.

**Browser smoke:** headless Chrome against a local server, signed in as the
owner through a dev-only session row (since deleted). 23 checks, all
passing:

- counts and node and wire totals
- wire ends land on dot centres, worst 0.01px
- a wire is hit-testable
- mouse drops connect at 30% and 300%
- undo
- right-drag pans
- a drop on empty grid makes no wire
- a drag snaps to whole units
- Tab, `]`, `W` and Enter wire from the keyboard
- Esc leaves the canvas
- the 200 / 400 load test, with Ctrl + A straight after the toolbar click
- two boosted arrivals each give one canvas whose keys act once
- Back from `/docs` reloads to one working canvas
- no console errors

A 20px root font scales the canvas, with wire ends within 0.4px. A
follow-up run confirmed the ghost and Fit fixes.

The headless drag measured about 1.5–2.3 ms of script work per frame with all
200 machines moving. The time between frames, 19–31 ms, is the test
harness's own input rate, so smoothness still needs your eye in a real
browser (step 7).

**Not this slice's:** `/favicon.ico` 404s site-wide (it shows on `/cv` too).
It is the 💭 public-polish item.

**Full gate** (`./scripts/verify.sh`, after the last code change):

```
=== cargo fmt --check
=== cargo clippy
=== cargo test
test result: ok. 371 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.73s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 559 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
test result: ok. 252 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.05s
=== node --check (static JS)
=== node (the board suites)
ok   docs/design/sorting-live/src/model.test.js — passed 265/265 checks
ok   docs/design/sorting-live/src/board.test.js — passed 193/193 checks
ok   tests/js/blueprint-model.test.js — 279 checks, all passed
VERIFY OK — fmt, clippy, tests, JS syntax, board suites all clean.
```

The workspace now has 1190 tests, 5 of them new. The figure is updated in
`CLAUDE.md` and in `/docs`, whose `WORKSPACE_TESTS` is now dated
2026-11-05. `CLAUDE.md` also gains the temporary route and the new test
files. The 458 board checks are unchanged: they count the van board only.
