# Drawing Tasks — dark face lift

**Status:** approved 2026-10-22; building on `feat/tasks-facelift`.

## Goal

`/tasks` is the last section still on the old light theme. It should look
like the rest of the site — dark, on the Hampter Design System — and be
nicer to browse:

- **Same dark theme as the art portfolio and the hub**, with no light flash
  or light leftovers when arriving by a boosted link.
- **Laid out like `/artportfolio`**: a head with an eyebrow line (task count,
  how many done), a filter rail on the left, and the tasks as a grid of cards
  on the right.
- **Each card leads with its reference image**, large, with the title,
  difficulty, done mark, prompt and tags beneath it.
- **Filtering is one tap**: subject, difficulty and type become toggle chips
  in the rail, each showing how many tasks it would match, instead of
  dropdowns. Sort stays available.
- **Works on a phone**: the rail collapses above the grid and the grid drops
  to one column.
- **The admin panel matches**: the upload / new-task forms and the image list
  use the design system's fields and buttons.

## Decisions

- **Rail + grid layout** (chosen 2026-10-22), mirroring `/artportfolio`. Not
  chosen: a LeetCode-style dense problem table; re-skinning today's list as is.
- **The markup moves to Askama templates** (chosen 2026-10-22): board, card
  and admin panel become `templates/tasks/partials/*.html`, like the feed's
  `post_card.html`. Auto-escaping replaces `tasks.rs`'s private
  `html_escape`. Not chosen: rewriting the format strings in place.
- **Theme hook: `site-dark`** (recommended, taken). The page wrapper carries
  the `.site-page` marker, so `base.html`'s theme script and every primitive
  scoped to `:is(… body.site-dark)` already apply — no new body class, no
  edit to that `:is()` list. Task-specific CSS is written fresh under the
  existing `/* ── Drawing Tasks */` section, against tokens, replacing the
  light rules there. Not chosen: reusing `art-page`, which would drag the
  art feed's layout rules onto this page.
- **Chip counts are computed in the handler** from the unfiltered task list
  (recommended, taken) — the data is small, so no new SQL.
- **No behaviour change**: same routes, same htmx contract (`#task-board`,
  `#task-filters`, `refresh-board` trigger), same admin gating, no migration.
  "Done" stays the admin's own toggle.

## Guidelines

- Read first: the repo `CLAUDE.md` (Templates, hx-boost lifecycle, the
  specificity trap under *Nutrition tracker* — a bare `.task-x` loses to the
  design system's `button`/`a` resets; prefix with `body.site-dark`), then
  `templates/artportfolio/feed.html` and the `body.art-page .art-rail…` rules
  in `static/style.css` as the pattern to match.
- `.site-page` must stay the outermost element in the content block.
- Filter chips are links or buttons that carry the full next filter state,
  as the art rail's do, and the rail re-renders with the board so active
  states and counts never freeze.
- Tokens only — no hex in the new rules. Difficulty colours come from the
  palette's status tokens.
- No `<style>` block in the template; no nested `/* */` in `style.css`
  (`tests/static_assets.rs` catches it).
- Keep `RequireAdmin` on every mutation and `OptionalAdmin` on the two GETs.

## Done when

- `/tasks` is dark, matches the art portfolio's look, and stays dark after
  boosted navigation in and out.
- The rail filters with one tap, shows counts, and its active chips survive
  a filter change, a "Done" toggle and an admin upload.
- A visitor sees no admin controls; an admin sees them restyled and working.
- On a 375px-wide screen the page has no horizontal scroll.

## Test it yourself

1. Load the made-up test data into the local dev DB (images are inline SVG
   data URIs, so no storage is needed; re-running replaces the seed):
   `python3 -c "import sqlite3; sqlite3.connect('portfolio.db').executescript(open('scripts/seed-tasks.sql').read())"`
   (or `sqlite3 portfolio.db < scripts/seed-tasks.sql` if the CLI is installed).
2. `cargo run`, open `http://localhost:3000/tasks` logged out — dark page,
   grid of cards, no admin controls.
3. Tap a subject chip, then a difficulty chip — the grid narrows, counts
   update, both chips stay active. Tap again to clear.
4. Change sort — order changes, chips keep their state.
5. Navigate to the hub and back via the header — stays dark, no light flash.
6. Log in as admin, toggle "Done" on a card and add a task — board refreshes
   with filters intact; the admin forms look like the rest of the site.
7. Narrow the window to phone width — rail above, one column, no sideways
   scroll.

## Built

_Written at handover._
