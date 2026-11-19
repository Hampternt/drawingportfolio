# Blueprints — S1–S3: model, section and editor v1

**Status:** building — S1 and S2 built, waiting on the owner's midpoint test before S3. Tracking issue: #29.
Branch `kepler/feat/blueprints-editor`, cut from `kepler/feat/blueprint-planning-tool` (S0).

## Goal

Turn the S0 canvas spike into a real section of the site. **Blueprints** lives at
`/blueprints`. The owner keeps projects, each project holds plans, and the owner
builds plans in the browser: machines with typed ports, wires, notes and status,
with undo and autosave. Signed-out visitors see one read-only demo plan as a
portfolio piece.

These are slices S1, S2 and S3 of `docs/design/blueprint-planner/README.md`.
The AI read and write API and revisions (S4–S6) get a second plan.

## Decisions

Made 2026-11-10 (the design doc's open issues):

- **Name and URL:** Blueprints, at `/blueprints`. This also names the hub tile,
  the nav link and the palette command. Not chosen: `/planner`, `/plans`.
- **Statuses:** `idea` / `planned` / `building` / `done`. Not chosen: `blocked`
  and `dropped`.
- **Node kinds:** `machine` and `blueprint` only. A source is a machine with no
  inputs. Not chosen: separate source and sink kinds.
- **Mismatch rule:** type labels are compared ignoring case and whitespace.
  Not chosen: an exact string compare.
- **Demo plan:** the site's own request flow, the S0
  `docs/design/blueprint-planner/demo-plan.json`, unchanged. Its one mismatch
  and one dangling wire stay in, because they show both states.
- **Port layout:** one port per row, outputs first (as S0 built it). Not
  chosen: Unreal-style side-by-side rows.
- **Comment boxes (5) and links out (7):** later, as the design doc recommends.
- **Tracking:** GitHub issue #29. This replaces the design doc's "no GitHub
  issues" row, since the doc is already public through PR #28.
- **Scope:** S1–S3 now. S4–S6 come after, in their own plan with a security
  review. The token model (open issue 4) is decided then.

Taken as recommended, not asked:

- **Storage:** migration `025`. It adds `blueprint_projects` and
  `blueprint_plans`, and each plan's document is stored whole in a `payload`
  column with an integer `rev`, like `sorting_sessions.payload`. The revisions
  table belongs to S6 and is not created here.
- **Concurrency:** every save sends `If-Match: <rev>`. A stale save gets `412`,
  and the page shows the "changed elsewhere" banner, whose only action is
  Reload. This is the design doc's rule, built now so S5's second writer
  inherits it.
- **The spike's route is replaced, not kept.** `/blueprint-spike` goes away,
  and the canvas scripts move behind `/blueprints`.
- **The outline beside the canvas** is rendered by Askama. That makes it the
  no-JS and screen-reader view on every page load, and after each save the
  editor refreshes it as an htmx fragment. Not chosen: a second client-side
  outline renderer.
- **Blueprint nodes render but cannot be entered.** The stored format supports
  sub-graphs (design decision). The nesting UI is on the Later list.

Made while building (2026-11-14), recommended options taken:

- **Built on this session's go.** The ask was to implement the task's work
  and make the changes rather than describe them, which is the go for
  S1–S3. S4–S6 stay out: the token model (open issue 4) is still the
  owner's call. The build stops at the S2 midpoint for the owner's test.
- **A signed-out visitor asking for any id gets a 404**, like an admin or a
  member. The plan said both "every handler is `RequireOwner`" (which sends
  a visitor to the login page) and "any other id gets a 404 for a visitor".
  Pages that take an id use `OptionalOwner` and answer 404 to anyone but
  the owner. Writes stay `RequireOwner`, so a visitor's write is sent to log
  in and never reaches a handler.
- **URLs carry numeric ids:** `/blueprints/{project}` and
  `/blueprints/{project}/{plan}`, so a rename never breaks a link. A plan
  id under the wrong project is a 404.
- **Names.** Nodes, ports and graphs start with a letter or `_`, then
  letters, digits, `_` or `-`, at most 64 characters. A `.` would split a
  wire, a `/` would break a layout key, and a key that looks like a number
  would be reordered by JavaScript, and port order is row order. Projects
  and plans are lower case, digits and dashes, as the Empty-project board
  says.
- **`status` may be left out** of a node; when present it is one of the
  four. Both validators agree on that.
- **The header counts the whole plan** (every graph), as the design
  canvas's summary does, so it matches what the server returns.
- **Kind is shown, not edited,** in the v1 inspector. A new node is a
  machine. A blueprint needs a sub-graph to open into, and that is the
  nesting UI on the Later list.
- **The payload is stored exactly as the browser sent it.** The server parses
  it only to validate and to render the outline, keeping key order, so
  ports never swap rows after a reload.
- **A save with no `If-Match` is refused with `428`.** Treating it as
  unconditional would bring last-write-wins back.

## Guidelines

- **Read first:**
  - `docs/design/blueprint-planner/README.md` (Decisions, Shape, Canvas, Avoid)
  - `docs/design/blueprint-planner/canvas/README.md`, plus the boards `Main`,
    `Anatomy`, `Input-model`, `Empty-project`, `Empty-plan`,
    `Changed-elsewhere` and `Visitor-demo`
  - `docs/manifests/2026-11-05-blueprint-canvas-spike.md` (what S0 proved, and
    its geometry and input rules)
- **Bring master in first.** The S0 branch predates the Last Call overhaul
  merge, and master is not an ancestor of it. Merge `origin/master` before
  building. Expect conflicts in the `CLAUDE.md` test count, in `/docs`'
  `WORKSPACE_TESTS`, and in `docs/INVENTORY.md`.
- **Gates:** every handler is `RequireOwner`, with one exception. `GET
  /blueprints` asks whether the viewer is the owner: the owner gets the
  projects list, everyone else gets the read-only demo. The demo needs no
  session at all.
  - No existing extractor asks that without rejecting the request.
    `OptionalAdmin` must **not** be used: it is true for any granted admin, so
    an admin would get the owner's projects.
  - Add a non-rejecting `OptionalOwner` in `src/middleware.rs`, true only for
    `is_owner`, with its own tests.
  - Add a route test: a signed-in admin, and a plain member, at `/blueprints`
    each get the demo, not the list.
  - Any other plan or project id gets a 404 for a visitor, an admin or a
    member. Never a 403.
  - Ids come from the path, but every query is owner-scoped in `db.rs`.
  - This is an auth path, so it gets a scoped review before the full gate.
- **Validation lives on both sides:**
  - `blueprint-model.js` is the editor's model.
  - The server rejects a document that breaks the shape with `400`:
    - an unknown `kind` or `status`
    - a bad port name
    - a wire that is not `node.port`
    - a `blueprint` node naming a missing graph
  - Dangling wires and mismatches are never rejected. They are counted and
    returned.
- **"Adding a new section" in the repo's `CLAUDE.md` applies in full:**
  - route module and `main.rs`
  - migration
  - the `base.html` **and** `admin.html` nav
  - the `palette.js` command
  - a `style.css` section
  - the hub tile, and `/docs`' sections table, whose "six sections" wording
    and test become seven
- **Match:**
  - S0's `window.BP` IIFE lifecycle under hx-boost
  - rem-only CSS scoped under `body.site-dark`
  - `?v=` on every asset link
  - `.sqlx` regenerated after the migration
- **Avoid:**
  - the design doc's Avoid list
  - `localStorage` as the store
  - seeding the demo into the production DB (it stays `include_str!`)

## Done when

- A signed-out visitor at `/blueprints` sees the request-flow demo, read-only,
  with its outline and no edit controls.
- The owner creates, renames and deletes projects, and the plans inside them.
  The empty project and empty plan states match the canvas boards.
- In a plan, the owner can do all of these with mouse and keyboard:
  - add machines, including by dragging a wire into empty space and picking
    from the search box
  - edit name, ports, type labels, status and note in the inspector
  - wire ports, delete, undo and redo
- Renaming a machine or port rewrites its wires.
- Edits autosave. A reload shows them.
- Dangling and mismatch counts show in the header.
- A save against a stale `rev` shows the "changed elsewhere" banner, and
  Reload recovers.
- Blueprints appears on the hub, in the nav, in the palette and on `/docs`.
- `/blueprint-spike` is gone.
- The model's checks for validate, rename-rewrites-wires, dangling, mismatch
  and nearest-port run in `./scripts/verify.sh`.

## Test it yourself

**Midpoint (after S2): ready.** In the worktree
`~/kepler/drawingportfolio-master-bd71b17d`, which has its own `.env`
(port 3017) and its own fresh `portfolio.db`:

1. Load the seed:
   `python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('scripts/seed-blueprints.sql').read())"`
2. Start the server: `cargo run`, then open `http://localhost:3017`.
3. **Signed out**, open `/blueprints` (the hub's Blueprints tile, the nav
   link, or Ctrl + K → "Go to Blueprints"):
   - you see "How this site answers a request." over the request-flow plan
   - the header counts read "11 machines · 14 wires · 1 dangling · 1
     mismatch"
   - the outline on the right reads the same with JavaScript off
   - dragging a machine, double-clicking the grid, `A`, `Delete`, and
     dragging from port to port all change nothing
   - pan, zoom, `Tab`, `]` and `>` still work
4. **Sign in as the owner.** The fresh database's owner has no PIN, so
   register a passkey first at `http://localhost:3017/admin/register`
   (allowed from localhost), then sign in at `/admin/login`.
   - `/blueprints` lists `drawingportfolio` (4 plans) and `garden-sensor` (0)
   - `garden-sensor` shows the empty-project state
   - each plan opens on the canvas read-only. `blueprint-planner` shows one
     amber dangling stub (`tokens.scope`); `scratch` is empty
   - create, rename and delete a project and a plan. A bad name (capitals,
     `_`) or a taken one says so under the form
   - "Preview the visitor demo" (bottom of the rail) shows step 3's page
     with a link back
5. **Sign in as a non-owner** (create one at `/admin/users`, then use a
   private window). `/blueprints` is the demo, and `/blueprints/1` or
   `/blueprints/1/1` is a 404. Granting that user admin changes neither.

**End (after S3): ready.** The same setup as the midpoint (seed, `cargo
run`, `http://localhost:3017`, signed in as the owner), then the editor:

1. Open `drawingportfolio / scratch`, the empty plan. The "An empty plan."
   card shows. Press **Add machine** (or `A`, or double-click the grid). A
   machine appears and the card goes.
2. Within a second the toolbar reads "saved · rev 2". Reload: the machine is
   still there.
3. Click the machine. The **Inspect** tab opens.
   - Rename it to `ingest` and press Enter.
   - Change the `out` type to `Vec<Row>`.
   - **+ Add output**, then remove it with ×.
   - Set the status to **building**: the node turns amber.
   - Type a note; it appears on the node.
   - A bad name (`2x`, `a.b`, `constructor`) or a taken one is refused in
     red, and nothing changes.
4. Drag from `ingest`'s output dot into empty grid. The add-node box opens
   "wired to ingest.out". Type `clean` and press Enter: a machine appears
   with an input `in: Vec<Row>`, already wired. `Esc` instead cancels.
5. Rename `ingest` to `reader`. The wire follows: the header still reads
   "0 dangling".
6. `Ctrl + Z` undoes the rename and `Ctrl + Shift + Z` redoes it. Both
   save. The **Outline** tab shows the saved plan.
7. Drag a port onto another machine's port to wire them. Click a wire and
   press `Delete`. Delete a machine with `Delete` or the inspector's button.
8. **Changed elsewhere.** Open the same plan in a second tab, and make and
   save an edit there. Back in the first tab, make any edit:
   - the amber banner says the plan changed elsewhere, and "editing
     paused" shows
   - `A`, dragging and the inspector do nothing
   - **Reload the plan** shows the other tab's version
9. Make an edit and click "How it works" straight away, then come back. The
   edit was saved.
10. `request-flow` shows the demo's dashed red mismatch and amber dangling
    stub. Editing them works the same way.

Test data is made up and goes into the local dev DB only. Load it with
`scripts/seed-blueprints.sql`, using python3's `sqlite3` module, since there is
no `sqlite3` CLI on this machine.

## Built

### Midpoint: S1 and S2 (2026-11-14)

S3 (the editor) is not started; it waits on the midpoint test.

**What changed**

- **Merge:** `origin/master` merged in first (`c85dbf3`). The only
  conflict was the test-count paragraph in `CLAUDE.md`.
- **S1, the model** (`static/blueprint-model.js`):
  - `validate()`
  - node and port renames that rewrite every wire naming them; a port
    rename touches its own side only, and key order is kept
  - port add, remove and type; status; note
  - whole-plan counts, plus a reserved-name list checked against the engine
  - `tests/js/blueprint-model.test.js` grows from 279 to 397 checks
- **The shared fixture,** `tests/fixtures/blueprint-validation.json`: 49
  cases that both validators run, so the editor and the server cannot
  disagree on what a plan is.
- **S2, the section:**
  - migration `025` (`blueprint_projects`, `blueprint_plans`)
  - `.sqlx` regenerated
  - `OptionalOwner`, and `RequireOwner` now carrying the owner's id
  - `src/blueprint_doc.rs`: the validator, counts and outline
  - the routes in `src/routes/blueprint.rs`
  - the templates in `templates/blueprint/`
  - the hub tile, nav link, palette command, `/docs` row ("seven
    sections") and a `layers` icon
  - `scripts/seed-blueprints.sql`
  - `/blueprint-spike` is gone
- **The canvas** boots from `#bp-app` in `demo`, `read` or `edit` mode. The
  owner's plans open in `read` until S3.

**Done differently from the plan**

- Visitors get a 404 on id pages, as recorded under Decisions; the plan
  contradicted itself there.
- `save_blueprint_plan` (the compare-and-set) is built and tested now. It
  has no route until S3. It and four other items S3 will read carry an
  `#[allow(dead_code)]` with a note, so clippy stays at the 18-warning
  baseline. S3 removes the allows.

**Review:** a workflow with three reviewers (gates, saved data and SQL,
validator parity and untrusted rendering), each finding put to an
adversarial verifier.

- **Gates:** nothing found.
- **Saved data:** nothing found.
- **Parity:** one real defect, fixed in `833bb42`. The JS type fold used
  `toLowerCase()` and `\s`, so Greek final sigma, U+0085 and U+FEFF folded
  differently from Rust's, and the header's mismatch count could differ
  from the server's outline. Four Unicode cases joined the fixture.
- **Refuted:** two parser-limit claims (duplicate keys, lone surrogates),
  because no text reaches the parser that could trigger them in this
  slice. S3 sends `JSON.stringify` output, which can do neither.

**Browser smoke:** headless Chrome against the worktree's server, seeded,
with the owner signed in through a dev-only session row (since deleted).
24 checks passed:

- the demo draws 8 machines and 12 wires, with the whole-plan counts
- in the demo, a drag, a double-click, `A`, `Delete` and a port-to-port drag
  all change nothing
- zoom works
- the demo has no form
- the hub tile and nav link both reach `/blueprints`, and `site-dark`
  survives the boosted navigation
- the owner sees the rail, the empty-project state, four plans and a plan
  canvas, all read-only
- Back from `/docs` gives one canvas, and `+` acts once
- creating, then deleting, a project and a plan works, through
  `hx-confirm`
- the owner pages log no console errors
- at phone width (390px) there is no horizontal scroll

The one console error on the demo is `/favicon.ico`, the known site-wide
404.

**Full gate** (`./scripts/verify.sh`, after the last code change):

```
=== cargo fmt --check
=== cargo clippy
=== cargo test
test result: ok. 397 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.12s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 709 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.73s
=== node --check (static JS)
=== node (the board suites)
ok   docs/design/sorting-live/src/model.test.js — passed 265/265 checks
ok   docs/design/sorting-live/src/board.test.js — passed 193/193 checks
ok   tests/js/blueprint-model.test.js — 397 checks, all passed
VERIFY OK — fmt, clippy, tests, JS syntax, board suites all clean.
```

The workspace has 1369 tests. That is 1339 on master, plus S0's 5, plus 25
here. Clippy, measured clean, has 18 distinct warnings, unchanged.

### End: S3

_Written at handover._
