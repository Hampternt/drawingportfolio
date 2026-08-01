# Fitness tracker redesign — design reference

Imported 2026-08-01 from the Claude Design project
`https://claude.ai/design/p/1704f99f-29aa-493f-83df-0fb9103b03da`
(built on the "Nocturne" design system, project `993d11db`).

## Files

| File | What it is |
|------|-----------|
| `redesign-mockups.html` | The five phone screens (1a–1e), the scan→confirm→logged flow (2a–2d) and the desktop layout (3a). Open in a browser; each `x-import` block's inner div is the screen markup. Colors/classes reference the Nocturne tokens below. |
| `redesign-plan.md` | The staged implementation plan authored in the design project: current-state audit, principles, screen map, backend needs (migrations 008–011), 6-step sequence, open questions. |
| `nocturne-tokens.css` | The Nocturne design-system stylesheet (tokens + component classes). Source of truth for colors, ramps, spacing, radii, shadows. We port a scoped subset into `static/style.css` — never link this file directly. |
| `nocturne-readme.md` | The Nocturne system's usage guide (do/don't, component classes, interaction states). |

The "current state" mockup from the design project is not kept here — the live
`/fitness` page is its own reference.

## Decisions taken (2026-08-01)

- **Scope:** all 6 steps of the sequence.
- **Auth:** `/fitness` moves behind the passkey session (`AuthSession`), like `/admin`. It stops being publicly readable.
- **Targets:** fixed daily targets — one row (calories, protein, carbs, fat).
- **Meal slot on log:** inferred from the client clock, pre-selected, one tap to change. Hour < 11 → breakfast, < 15 → lunch, < 17 → snack, < 22 → dinner, else snack.
- **Extra schema beyond the plan doc:** `food_items.default_portion_g` — the mockups' "usual portion" (screens 2b/2c, quick-add Enter-to-log) needs a stored per-food default; the plan doc's migration list omitted it. It rides in migration 010.
