# Handoff: /artportfolio — Drawing Portfolio redesign

## Overview

A redesign of the drawing feed at `/artportfolio` in `Hampternt/drawingportfolio`
(Rust + Axum + Askama + HTMX, `master`). The page today is a single chronological
column of post cards with an admin composer for one file at a time. This design adds:

- a three-state visibility model (public / unlisted / hidden) with inline admin control,
- collections and tags, with a search + filter rail,
- month grouping in the feed,
- a multi-file upload tray (drag-drop, reorder, per-file visibility, one shared caption),
- select mode with batch visibility / collection / delete actions,
- the site's dark design system applied to this section.

The rest of the site keeps its current styling until it is migrated section by section.

## About the design files

The files in this bundle are **design references created in HTML** — prototypes of the
intended look and behaviour, not production code to copy. They are Design Components
(a streaming HTML format) that render the screens in a browser. The task is to
**recreate these designs in this repo's existing environment**: Askama templates in
`templates/`, HTMX for dynamic swaps, and plain CSS in `static/style.css` under a named
section comment. No build step, no JS framework — that constraint is from
`docs/design.md` and `CLAUDE.md` and still holds.

## Fidelity

**High fidelity.** Colours, type, spacing and states below are final and come from the
Hampter Design System. Recreate them exactly; do not approximate. Where this document
and the HTML disagree, this document wins.

## Screens

Design file: `ArtPortfolioFeed.dc.html` (S1–S5). `PortfolioPage.dc.html` holds the
earlier exploration: `0a` the current page recreated, `1a`/`1b` the two directions,
`1c` an alternate upload tray. 1a is the direction being implemented.

### S1 — Visitor view (`is_admin = false`)

Purpose: browse public drawings.

Layout, 1180px content column, 24px gutters (page padding 32px at ≥1280):

```
header                       56px sticky, translucent + 10px blur
page head                    48px top / 32px bottom padding, 32px blueprint grid, hairline bottom
body      flex, gap 32px
  aside   236px fixed        search, collections, tags, keyboard legend; sticky top 80px
  main    flex 1             month divider, 3-column masonry (column-gap 16px, card margin-bottom 16px), Load more
```

Components:

- **Header** — height 56px, `rgba(14,12,20,.82)` + `backdrop-filter: blur(10px)`,
  bottom hairline `rgba(242,238,248,.07)`. Wordmark `hampter.` Archivo 800 17px,
  tracking −0.035em, `#F2EEF8`, violet full stop `#B48EF7`. Nav links Space Grotesk
  500 14px; active `#F2EEF8`, rest `#8D87A0`. Right: `Ctrl` `K` keycap group + the word
  "commands" in 11px mono. This is `base.html` — restyle in place, keep markup.
- **Page head** — micro-label 11px IBM Plex Mono, uppercase, tracking .10em, `#5F5876`:
  `117 drawings · newest first`. H1 Archivo 800 40px/1.22, tracking −0.025em, `#F2EEF8`.
  Intro paragraph Space Grotesk 400 15px/1.5, `#8D87A0`, max 68ch:
  *"Mostly practice — studies, drills, and the odd finished piece."* (placeholder copy —
  replace with the owner's line before shipping).
  Background: `#0B0910` + 32px blueprint grid at 4.5% white.
- **Search field** — 34px tall, `#17141F` fill, 1px `rgba(242,238,248,.10)`, radius 5px,
  leading `search` glyph 14px, trailing `/` keycap. Placeholder "Search captions".
- **Collections list** — rows 6px/8px padding, radius 5px, label 14px `#CDC6DD`,
  count 11px mono `#5F5876` right-aligned. Hover: `rgba(242,238,248,.04)`, label `#F2EEF8`.
- **Tags** — pill Tags, 24px tall, wrap with 6px gap, lowercase.
- **Keyboard legend** — 11px mono `#5F5876`, above a hairline: `J / K — next, previous`,
  `? — all shortcuts`.
- **Month divider** — 11px mono uppercase `2026-07 · 5 drawings` + 1px rule filling the row.
- **PostCard** — `#17141F`, 1px `rgba(242,238,248,.10)`, radius 8px, no shadow at rest.
  Media area sits on `#0E0C14` + blueprint grid while loading; the drawing renders
  uncropped at its natural ratio (`width:100%`, no forced aspect). Body padding
  20px 24px 24px, gap 12px: caption 15px/1.5 `#CDC6DD`, meta row 11px mono `#5F5876`
  with the ISO date. Hover: border `rgba(242,238,248,.20)`, `translateY(-2px)`,
  shadow-2, 190ms `cubic-bezier(.2,.8,.3,1)`.
- **Load more** — secondary Button, 34px, `chevron-down` glyph. Same HTMX contract as today.

### S2 — Admin view (`is_admin = true`)

Everything in S1, plus:

- Header gains an `admin` Badge (accent tone, dot).
- Page head counts split: `128 drawings · 117 public · 4 unlisted · 7 hidden`.
- Head actions: secondary **View as visitor** (`eye`, shortcut `V`) and primary
  **Upload** (`upload`, shortcut `U`, violet `#B48EF7` fill, ink text).
- Rail gains a **+** IconButton beside "Collections" (new collection) and a
  **Visibility · admin** group: three checkboxes, all checked by default.
- Feed toolbar right side: **Select** (`check-square`, shortcut `S`) and a sort button
  labelled with the current sort ("Newest").
- Each card carries a visibility **Badge** at top-right, 8px inset:
  `public` success (mint `#4FD6A8`), `unlisted` warning (amber `#FFB570`),
  `hidden` neutral. Hidden cards render at `opacity: .5`.
- Card hover (and keyboard focus within the card) fades in a control cluster at the
  card's top-left over 130ms: `eye-off` hide, `link` unlist, `pencil` edit caption/tags,
  `folder-plus` add to collection, `trash-2` delete. 28px IconButtons, 6px gap.

### S3 — Select mode

Entered with **Select** or `S`. A bar replaces the feed toolbar and sticks under the
header at `top: 56px`: background `rgba(180,142,247,.10)`, 1px `rgba(180,142,247,.45)`,
10px/32px padding. Left: `3 selected` in 11px mono `#D6BDFB`. Then small Buttons —
Make public (`globe`), Unlist (`link`), Hide (`eye-off`), Add to collection
(`folder-plus`), Delete (`trash-2`). Right: `Esc to leave` hint and a ghost **Clear**.
Cards gain a Checkbox at top-left (8px inset); clicking anywhere on the card toggles
selection while the mode is active; shift-click selects a range.

### S4 — Multi-upload tray

Dialog, 720px wide, radius 12px, `#17141F`, shadow-3, scrim 72% black + 3px blur.
Opens from **Upload**, `U`, the palette command, or a file drop anywhere on the feed.

- Header: title Archivo 700 20px + running `4 files · 21.5 MB · 35 MB limit each` in
  11px mono; close IconButton.
- Drop zone: 1px dashed `rgba(242,238,248,.20)`, radius 8px, 24px padding, blueprint
  grid fill. "Drop drawings here" / "jpeg · png · webp — or click to browse".
- File rows: `grip-vertical` drag handle, 44px thumbnail (object-URL preview), name
  14px `#F2EEF8`, size + state in 11px mono, a 3px progress rail (violet fill,
  `rgba(242,238,248,.07)` track), a visibility Select (Public / Unlisted / Hidden,
  default Public), and a remove IconButton. Rows reorder by drag; order = post order.
- **Caption for all N** textarea (2 rows) and a **Tags** input, both applied to every
  file and editable per drawing afterwards.
- Footer: visibility tally in 11px mono, Cancel, and primary **Upload N files**.
- States: queued → uploading (per-file progress) → done (row collapses to a check) or
  error (row turns rose `#F7768E` with the exact reason, e.g.
  *"File is 41 MB — the limit is 35 MB."* and a Retry button). Failures do not abort
  the queue.

### S5 — Filtered, no results

Active filters render as removable Tags in a row above the grid with a ghost
**Clear filters** on the right. Empty state is one mono line behind a `>` prompt —
`> no drawings match perspective + ink + "loomis"` — with a **Reset filters** button.
The all-empty case (no posts at all) uses the same treatment: `> no drawings yet.`

## Interactions & behaviour

- **Filtering** is server-side and HTMX-driven: rail controls issue
  `hx-get="/artportfolio/htmx/posts"` with the current query string,
  `hx-target="#feed"`, `hx-swap="innerHTML"`, `hx-push-url="true"` so filters are
  linkable and survive reload. Debounce the search input 200ms (`hx-trigger="keyup changed delay:200ms"`).
- **Pagination** keeps the existing N+1 `has_more` pattern; the Load more button carries
  the active filters in its URL.
- **Visibility change** — `PATCH /api/admin/posts/{id}/visibility`, HTMX swaps the single
  card (`hx-target="closest .post-card" hx-swap="outerHTML"`), so the badge and opacity
  update without a reload. Optimistic UI is not needed.
- **Batch actions** — `POST /api/admin/posts/batch` with `{ids, action, value}`; response
  re-renders the affected cards. Delete asks for confirmation in a Dialog.
- **Upload** — the tray sends one `POST /api/admin/posts` per file, sequentially, via
  `XMLHttpRequest` for `upload.onprogress`. Each request carries `image`, `caption`,
  `tags`, `visibility`, `collection_id`, `position`. Successful responses are prepended
  to `#feed`.
- **Keyboard** — reserved keys unchanged (`Ctrl+K`, `?`, `/`, `Esc`, `↑ ↓`, `↵`, `K` `J`).
  Section-local single letters: `U` upload, `S` select mode, `V` view as visitor.
  None fire while an input, textarea or select has focus.
- **Palette** — add to `COMMANDS` in `static/palette.js`: "Upload drawings" (adminOnly,
  opens the tray), "Filter drawings by tag", "Toggle visitor view" (adminOnly),
  "Go to Art Portfolio" (exists).
- **Motion** — 130ms controls, 190ms surfaces, 280ms overlays, all
  `cubic-bezier(.2,.8,.3,1)`. Press state is `translateY(1px)`, never a scale.
  `prefers-reduced-motion` zeroes all durations.
- **Focus** — one ring everywhere: 2px page colour then 2px violet.
- **Responsive** — masonry is 3 columns ≥1180px, 2 columns 780–1179px, 1 column below.
  Under 900px the rail collapses into a "Filters" button opening a full-screen sheet;
  card hover controls become always-visible at 44px hit targets.
- **hx-boost caveat** — every JS init (tray, select mode, key handlers) must bind on both
  `DOMContentLoaded` and `htmx:afterSwap` and guard against double-injection, the pattern
  `static/palette.js` already uses.

## What needs to be built

Server:

1. **Migration 012** — `ALTER TABLE posts ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public'`
   (`public` | `unlisted` | `hidden`), plus `position INTEGER NOT NULL DEFAULT 0`.
2. **Migration 013** — `collections(id, name, slug, created_at)`,
   `post_collections(post_id, collection_id)`, `tags(id, name)`, `post_tags(post_id, tag_id)`.
   Index `post_tags(tag_id)` and `posts(visibility, created_at DESC)`.
3. **`src/models.rs`** — `Visibility` enum (`as_str`/`from_str`), `Post.visibility`,
   `Post.tags: Vec<String>`, `Collection`, `PostFilter { q, tags, collection, visibility }`.
4. **`src/db.rs`** — `get_posts_filtered(pool, filter, page, is_admin)` (visitors see
   `visibility = 'public'` only; unlisted is reachable only by direct id),
   `get_post_by_id`, `set_post_visibility`, `batch_set_visibility`, `batch_delete`,
   `list_collections_with_counts`, `list_tags_with_counts`, `add_post_to_collection`,
   `set_post_tags`. All SQL stays in this file.
5. **`src/routes/feed.rs`** — extend `PageQuery` with `q`, `tag`, `collection`,
   `vis`; month grouping computed in the handler, not the template; keep the
   inline-first-page optimisation.
6. **`src/routes/admin.rs`** — `PATCH /api/admin/posts/{id}/visibility`,
   `PATCH /api/admin/posts/{id}` (caption + tags), `POST /api/admin/posts/batch`,
   `POST /api/admin/collections`, `POST /api/admin/posts/{id}/collections`.
   Upload gains `visibility`, `tags`, `collection_id`, `position` fields; the three-layer
   35 MB limit (nginx, Axum `DefaultBodyLimit`, `MAX_IMAGE_BYTES`) is unchanged.
7. **`GET /artportfolio/api/posts`** — add the same filters; never return non-public rows
   without a session.

Templates (`templates/artportfolio/`) — the visual building blocks, one file each:

| File | Renders |
| --- | --- |
| `feed.html` | Page shell: extends `base.html`, page head, rail + main grid |
| `partials/filter_rail.html` | Search, collections, tags, visibility group, key legend |
| `partials/feed_toolbar.html` | Month divider row, Select and sort buttons |
| `partials/select_bar.html` | Batch action bar (admin, select mode) |
| `partials/post_card.html` | One card — media, caption, meta, badge, hover controls |
| `partials/post_grid.html` | Month groups + cards + Load more (the HTMX swap target) |
| `partials/upload_tray.html` | Dialog markup; rows cloned client-side per file |
| `partials/empty_state.html` | `>` prompt line + reset action |

`post_card_html()` in `src/routes/feed.rs` builds cards as format strings today; move it
to `partials/post_card.html` so the card exists in one place, and have both the inline
first page and the HTMX route render that partial.

Static:

- `static/style.css` — new section `/* ── Drawing Portfolio ─────── */` holding the
  tokens and component rules below. Scope with a `.art-page` body-level class so the
  rest of the site is untouched while it migrates.
- `static/upload_tray.js` — drop handling, ordering, per-file XHR queue, progress.
- `static/feed_select.js` — select mode, shift-range, batch submit.
- `static/palette.js` — the new commands.

Tests to add: db-layer filter tests (visitor never sees hidden/unlisted), visibility
transition tests, batch action tests, and a `post_card` render test asserting the badge
is absent for visitors. `./scripts/verify.sh` is the gate.

## Design tokens

Colours:

| Token | Hex | Use |
| --- | --- | --- |
| `--ink-950` | `#0B0910` | page |
| `--ink-900` | `#0E0C14` | raised / media wells |
| `--ink-850` | `#17141F` | cards, dialogs, inputs |
| `--ink-700` | `#262232` | chips |
| `--ink-050` | `#F2EEF8` | strong text |
| body text | `#CDC6DD` | captions, rail labels |
| muted | `#8D87A0` | prose, inactive nav |
| faint | `#5F5876` | 11px mono micro-labels |
| `--violet-400` | `#B48EF7` | primary button, focus ring, links, wordmark stop |
| `--violet-300` | `#D6BDFB` | text on accent surfaces |
| mint | `#4FD6A8` | public badge |
| amber | `#FFB570` | unlisted badge |
| rose | `#F7768E` | upload errors, destructive |
| hairline | `rgba(242,238,248,.07)` | dividers |
| border | `rgba(242,238,248,.10)` | card borders |
| header fill | `rgba(14,12,20,.82)` + `blur(10px)` | sticky header |
| accent surface | `rgba(180,142,247,.10)` / border `rgba(180,142,247,.45)` | select bar |

Type: Archivo 800/900 headings (tracking −0.025em, leading 1.04–1.22); Space Grotesk
400/500 body and controls at 15px/1.5; IBM Plex Mono for dates, counts, keycaps and
11px uppercase micro-labels tracked .10em. Fonts are already in the repo
(`drinkinggame/assets/fonts/*.woff2`) — serve from `/static/fonts/`; only the mono is a
substitution (IBM Plex Mono, Google Fonts).

Spacing scale: 2 4 6 8 12 16 20 24 32 40 48 64 80 112 160. Control heights 28 / 34 / 42.
Radii: 3 keycaps and badges, 5 buttons and inputs, 8 cards, 12 dialogs, pill for tags.
Shadows: none at rest; shadow-2 on card hover; shadow-3 for dialog and tray.

## Assets

No images ship with this design — every media area is a placeholder showing the 32px
blueprint grid; real drawings come from object storage. Icons are Lucide v0.462.0
rendered as CSS masks over `currentColor`
(`https://unpkg.com/lucide-static@0.462.0/icons/<name>.svg`): `search`, `upload`, `eye`,
`eye-off`, `link`, `globe`, `pencil`, `trash-2`, `folder-plus`, `plus`, `x`,
`check-square`, `chevron-down`, `arrow-up-down`, `grip-vertical`, `tag`, `rotate-ccw`.
Lucide is a flagged substitution — the repo ships no icon set. Self-host the 17 SVGs
under `/static/icons/` rather than hitting the CDN in production.

## Files

- `ArtPortfolioFeed.dc.html` — S1–S5, the design being implemented.
- `PortfolioPage.dc.html` — `0a` current page recreated, `1a` / `1b` directions, `1c` alternate tray.
- `_ds/hampter-design-system-.../` — the design system: `tokens/`, `components/components.css`.
  The CSS there is plain and can be lifted into `static/style.css` directly.
- `github.md` — repo association and the screen → source-file map.

Open both `.dc.html` files in a browser to see the designs.
