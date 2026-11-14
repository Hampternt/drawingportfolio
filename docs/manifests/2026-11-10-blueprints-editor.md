# Blueprints — S1–S3: model, section and editor v1

**Status:** drafted, awaiting go. Tracking issue: #29.
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

**Midpoint (after S2):** load the seed, then:

1. Open `/blueprints` signed out: the demo is read-only.
2. Sign in as the owner: the seeded projects and plans are listed, and the
   canvas opens them read-only.
3. Sign in as a non-owner user: `/blueprints/<any id>` is a 404.

**End (after S3):** the steps above, plus the editor walk-through. Its exact
steps are written with the build.

Test data is made up and goes into the local dev DB only. Load it with
`scripts/seed-blueprints.sql`, using python3's `sqlite3` module, since there is
no `sqlite3` CLI on this machine.

## Built

_Written at handover._
