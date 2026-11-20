# Blueprint planner — design canvas (reference copy)

A snapshot of the claude.ai Design canvas
<https://claude.ai/artifact/KB2g3BHRHBshZwzqKNv5D6> (private), taken
2026-11-05 after the owner's review comments were applied. The live canvas is
the place to look at it; these files are its source, kept so the design does
not depend on the artifact staying up.

Like the `.dc.html` files in `../../hub-admin-cv-docs/`, these are **design
references, not production code**. They need the Design canvas runtime
(`./support.js`, which is not in this folder) to render, and their fonts point
at `/_blob/…` assets stored on the canvas. Those are the site's own
`static/fonts/` files: Archivo, Space Grotesk and IBM Plex Mono.

## Boards

| File | What |
|---|---|
| `Main.dc.html` | Owner editor: projects rail, canvas, inspector and outline |
| `Changed-elsewhere.dc.html` | The 412 "plan changed elsewhere" banner. Reload is its only action |
| `Visitor-demo.dc.html` | Signed-out visitor: the demo plan, read-only, with the outline |
| `Empty-project.dc.html`, `Empty-plan.dc.html` | Empty states |
| `Tokens.dc.html` | AI tokens: mint (shown once), list, revoke |
| `Anatomy.dc.html` | Node anatomy, wire states, status ladder, blueprint node, add-node search |
| `AI-view.dc.html` | `outline.md` and the JSON export beside the same plan; the write loop |
| `Guide.dc.html` | Grid unit, geometry, type, colour roles, wire curves, port hue, shortcuts |
| `Input-model.dc.html` | Right-click menus, keyboard navigation, focus, touch and screen readers |
| `Issue-status-kinds.dc.html` | Open issues 2 and 3, as options |
| `Issue-name-boxes-rule.dc.html` | Open issues 1, 5 and 6, as options |
| `Plan.dc.html` | The one renderer every board uses: canvas, counts, inspector, outline, `outline.md`, JSON |
| `SiteHeader.dc.html` | `base.html`'s nav, with the planner entry |
| `canvas.json` | Board positions on the canvas |

`Plan.dc.html` holds the demo document in the shape `../README.md` drafts. Every
node size, count, outline line and validation summary on the boards is computed
from it.

## What the review settled

- **Sizes are in grid units, not pixels.** 1u = 1rem (16px at the browser
  default). On screen, px = u · rem · zoom, and Fit sets the zoom from the
  viewport. The guide boards state every size in u. The renderer itself still
  draws with 1u = 16px and scales through zoom, so a change to the browser's
  font size does not reach the canvas yet.
- **Port dots line up with wire ends.** A node is a top-aligned flex column.
  A `<button>` centres its content vertically, which pushed the dots off the
  wire endpoints.
- **Node width counts everything.** That covers the badge's letter-spacing, the
  border, and a blueprint's footer.
- **Input model** (`Input-model.dc.html`): right-drag pans. Tapping Space does
  not toggle the selection. Rebindable keys are a later slice.

Still open: the port layout. This is one port per row, outputs first, so a
node's width is its longest label. It is proposed, not confirmed, along with
the open issues the README lists.
