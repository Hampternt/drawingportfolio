# Blueprint planner — design and open issues

**Status:** planning. A living design doc, not a build plan. When a slice below
is picked up, it gets its own `docs/manifests/` plan.

## Goal

A new page on the site for planning code and projects, laid out like a factory
game. A plan is a canvas of **machines**: functions, systems or data stages,
each with named **input and output ports**. **Wires** carry typed items from an
output to an input. A group of machines can become one **blueprint**: a single
node with its own ports that opens into the machines inside it.

It does not read or visualise an existing codebase. It is for thinking ahead.

The plan is also a **source of truth for AI coding**. Claude Code can fetch a
plan as context for a feature, and edit it so the plan stays current with what
was actually built.

Prior art: Unreal Engine Blueprints is the node-and-pin editor the idea is
modelled on (typed pins, wires, functions you collapse into one node). It is
built to *run* code. People have asked Epic for a planning-only "pseudo-node",
and the answers were workarounds. Blender's node groups, ComfyUI subgraphs and
Factorio's ghosts and blueprint books each contribute a pattern below.

## Decisions (2026-11-05)

| Question | Chosen | Not chosen |
|---|---|---|
| Where the issues live | This doc, plus a 💭 entry in `docs/INVENTORY.md` | GitHub issues (the repo is public), Kepler tasks |
| Where the plan is stored, and how AI reaches it | Site SQLite, the document stored whole (the `sorting_sessions.payload` pattern), with an AI export endpoint and a token-gated write endpoint | A plan file committed in each code repo, an MCP server first |
| Who sees what | Only the owner edits and sees private plans (`RequireOwner`). Signed-out visitors see **one demo plan**, read-only (portfolio piece). | `RequireAdmin` (any granted admin would see every plan), per-plan visibility flags, everything public |
| Document shape | As drafted below: name-keyed refs, sub-graphs in a flat `graphs` map, `layout` separate from meaning. A rename rewrites its wires; the write response reports dangling wires. | Stable ids with display names |
| Plans and projects | A project (e.g. `drawingportfolio`) holds many plans, one per feature or system. The AI asks for a project and gets its plans. | One plan per project, loose plans |
| Target device | Desktop first: mouse, keyboard, wheel zoom. Touch works but is polished later. | Tablet first-class from the first spike |
| Port typing | Loose: a port has a name and a free-text type label. Mismatched wires are allowed but highlighted, like a belt carrying the wrong item. | Strict matching, untyped sockets |
| Nesting | The stored format supports a node that opens into a sub-graph from day one. v1 UI shows flat graphs only. | Nesting UI in v1, flat-only format |
| Rendering | Hand-rolled SVG with our own pan, zoom, drag and wiring, consistent with the site's manual-over-magic approach | A vendored graph library (Rete, LiteGraph) |
| Prior project | Borrow `~/projects/codeplans`' decisions: refer to things by name rather than id, keep dangling refs and count them, keep the server a dumb validating store. No code reuse. | Starting fresh |

## Shape

### Document

```json
{
  "v": 1,
  "root": "main",
  "graphs": {
    "main": {
      "nodes": {
        "ingest":  { "kind": "machine", "status": "planned", "note": "reads CSV",
                     "in": {}, "out": { "rows": "Vec<Row>" } },
        "clean":   { "kind": "machine", "status": "idea",
                     "in": { "rows": "Vec<Row>" }, "out": { "ok": "Vec<Clean>" } },
        "storage": { "kind": "blueprint", "graph": "storage",
                     "in": { "data": "Vec<Clean>" }, "out": {} }
      },
      "wires": [["ingest.rows", "clean.rows"], ["clean.ok", "storage.data"]]
    },
    "storage": { "nodes": {}, "wires": [] }
  },
  "layout": { "main/ingest": [4, 6], "main/clean": [17, 6] }
}
```

- **Everything is referenced by name.** Nodes and ports are maps keyed by
  name, so duplicate names are impossible. Wires are `node.port` pairs.
  Comfy, Node-RED and Factorio all store slot indexes instead. That suits a
  program, but it is hostile to an AI editing JSON: inserting a port silently
  re-points every wire after it.
- **Sub-graphs live in one flat `graphs` map.** A `blueprint` node names the
  graph it opens into. There is no recursive nesting in the JSON.
- **Layout is kept apart from meaning.** Positions are in grid units, which
  gives the factory snap. The AI export omits them, so an AI edit cannot shuffle
  the boxes.
- **Dangling wires stay in the file and are counted**, never dropped.
- **A type mismatch is a highlight, not an error.** Fan-in is allowed: planning
  has no runtime that would need to pick one value.

### Storage and API

- One table of plans. The payload is stored whole and carries a `rev`. A
  revisions table keeps each accepted write, so a bad AI edit is a one-row
  revert.
- **Two writers, the browser and the AI**, so last-write-wins is out. Every
  write sends `If-Match: <rev>`. A stale write gets refused, and the browser
  reloads rather than retrying.
- **AI read:** `outline.md`, a compact text rendering: one line per node with
  its ports, wires, status and note, and a header with dangling-wire and
  mismatch counts. Raw JSON (layout omitted) is fetched only when about to
  edit. Not Mermaid: it has no ports.
- **AI write:** a whole-document PUT. The response carries a validation
  summary (dangling count, mismatches), which is the AI's feedback loop.
- **Auth for the AI path is new security surface.** The site has no
  bearer-token auth today, and the endpoint faces the internet. The proposal:
  - tokens minted on the page and shown once, with a `bp_` prefix
  - stored as a SHA-256 hash, scoped read or write, optionally to one plan, and
    revocable
  - a dedicated extractor that a cookie can never satisfy
  - an nginx rate limit on the endpoint
  - a review before merge

### Page

- A new section that follows the `.site-page` / `site-dark` house style.
- **Visitors** get the demo plan, read-only. Asking for any other plan id
  returns 404, never 403.
- **The demo is a committed JSON file** loaded with `include_str!`, so it never
  touches the production DB. Seeds are dev-only.
- **An Askama-rendered outline** sits beside the canvas. It is the
  screen-reader view, the no-JS view and a preview of the AI export.

### Canvas (hand-rolled SVG)

- **Rendering:** one `<g>` transform for pan and zoom; pointer events with
  capture (touch works, pinch comes later); bezier wires port to port.
- **Port colour:** the type label is hashed to a hue. Unwired ports are drawn
  hollow, wired ones solid. A mismatched wire is dashed red.
- **Status is drawn Factorio-ghost style:** an idea or planned node is
  translucent and dashed; a building node is accented; a done node is solid.
- **Making a node:** drag a wire into empty space and a search box opens. It
  creates a node with the matching port already wired.
- **Undo:** a snapshot per action, as the sorting board does. A drag counts as
  one entry.
- **Monospace node labels**, so node sizes are pure arithmetic and the model is
  testable on plain node.
- **Scripts:** `static/blueprint-{model,input,view,app}.js`. Model and input
  run under node tests, which `./scripts/verify.sh` picks up, like the sorting
  board's checks. Each file is an IIFE that exposes `window.BP`, so it survives
  hx-boost re-execution.

## Open issues

Each needs an answer from the owner before the slice that depends on it is
planned. **Rec** is the recommended answer.

1. **Name and URL.** "Blueprints" at `/blueprints`? This also names the hub
   tile, the nav entry and the palette command.
2. **Status vocabulary.** Is `idea / planned / building / done` right, or do
   you want `blocked` or `dropped`?
3. **Node kinds.** Is `machine` + `blueprint` enough, or do you want factory
   **sources and sinks** (external input, storage, user) as their own kinds?
4. **Token model.** Tokens minted and revoked on the page (as proposed), or a
   single env-var token for v1 that only rotates on redeploy? **Rec:** minted
   tokens. Rotation without a deploy matters once a token sits in a local
   `.env`.
5. **Comment boxes in v1?** Unreal-style boxes that group and label a region of
   the canvas. **Rec:** later. Node notes cover v1.
6. **Mismatch rule.** Should "highlight a mismatch" mean an exact string
   compare, or should it ignore case and whitespace (`Vec<Row>` vs
   `vec<row>`)? **Rec:** ignore case and whitespace.
7. **Links out.** Should a node be able to point at real things: a file path,
   a manifest, a branch, a commit? **Rec:** later, as a free `links` list on
   the node.
8. **The demo plan's content.** What should the visitor demo plan *plan*?
   The site itself is a natural choice: the portfolio's request flow as
   machines.

## Primer: the design guide and UI mockup artifact

A separate session makes one artifact that serves as this tool's design guide
and UI mockup. Build it on the **Hampter Design System**, dark, as the site's
own screens (hub, CV, tasks) wear it: `site-dark` tokens in `static/style.css`.
Earlier mockups in the same system: `docs/design/hub-admin-cv-docs/` (its
`README.md` explains what a `.dc.html` reference is and how it gets ported).
Read this doc's Decisions and Shape first; the artifact illustrates them and
must not contradict them.

**It must show:**

- **The canvas:** a factory-blueprint grid, pan and zoom chrome, and a small
  plan of 5–8 machines wired together (use the demo idea from issue 8: the
  site's own request flow).
- **Node anatomy:** title, `in` ports on the left, `out` ports on the right,
  type labels, the note, the status badge. Unwired ports hollow, wired solid,
  port colour hashed from the type label.
- **Wires:** normal, selected, type mismatch (dashed red), and dangling
  (counted in the header, as there is no target to draw to).
- **Status styles:** the Factorio-ghost ladder for each status: translucent
  and dashed through to solid.
- **A blueprint node:** what a collapsed sub-graph looks like on the canvas,
  even though entering it is a later slice.
- **The page around the canvas:**
  - the project → plans list
  - the side outline (the screen-reader and no-JS view)
  - a node inspector for editing name, ports, status and note
  - the drag-a-wire-into-space add-node search box
- **States:**
  - empty project
  - empty plan
  - the visitor's read-only demo, with no edit controls
  - the "plan changed elsewhere, reload" banner shown after an AI edit (412)
  - the token panel: mint (shown once), list, revoke
- **The AI side:** a sample of `outline.md` rendered for the same plan, so the
  canvas and the AI's view can be compared.
- **Design-guide notes:** spacing, the grid unit, the monospace label font,
  wire curve rules, and keyboard shortcuts.

**Open issues the artifact can help decide:** it may show options side by side
for issue 1 (name), 2 (status set), 3 (source and sink kinds), 5 (comment
boxes) and 6 (mismatch rule). Do not mark any of them decided; the owner picks.

**Out of scope:** working code, real persistence, touch polish (desktop first).

## Slices (the feature issues, riskiest first)

| # | Slice | Done when | Depends on |
|---|---|---|---|
| S0 | **Canvas spike**: no persistence, a throwaway page | Pan, wheel zoom, drag and wiring work with mouse and keyboard inside a boosted page; wires drop onto ports at zoom 0.3 and 3; 200 nodes and 400 wires drag smoothly | — |
| S1 | **Document and model**: `blueprint-model.js` plus node tests | Validate, add, move, connect, rename-by-name (rewriting wires), dangling count, mismatch check, nearest-port, all in the gate | 2, 3, 6 |
| S2 | **Section scaffold**: the next free migration, routes, hub tile, nav, palette, `/docs` row and its "six sections" wording, projects and their plans, read-only demo for visitors | A visitor sees the demo; the owner creates projects and plans; any other plan id 404s | 1, 8 |
| S3 | **Editor v1** | The owner builds a plan in the browser: nodes, ports, wires, notes, status, undo, autosave with `If-Match` | S0, S1, S2 |
| S4 | **AI read**: `outline.md` and the JSON export | Claude Code fetches a project's plans as context with a token | S2, 4 |
| S5 | **AI write** | Claude Code edits a plan with `If-Match`, gets a validation summary back, and an open browser tab notices and reloads. Security review and rate limit done; a CLAUDE.md snippet to paste into other repos | S4 |
| S6 | **Revisions** | The page lists the plan's history and can revert to any revision | S5 |

**Later:**
- the nesting UI (enter a blueprint, Ctrl-G to collapse a selection)
- a shared blueprint library, the equivalent of Factorio blueprint books
- a server-side op list (`rename`, `connect`) and then an MCP server over it
- belt-style orthogonal wire routing
- touch polish: pinch zoom, tablet-sized hit targets
- a minimap
- comment boxes

**Avoid:**
- Unreal's split between exec and data pins: control flow is a runtime
  concern
- Unity's one-wire-per-input rule
- reroute nodes: they are layout-only clutter in the AI view
- JSON Patch: its positional array paths go stale
- last-write-wins
- a demo seeded into production
