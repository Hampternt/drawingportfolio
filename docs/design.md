# Portfolio Site — Design Guide

A short reference for how this project is structured and where new code belongs.
Intended to keep scope clean as the project grows.

---

## Layers and what lives in each

```
nginx  →  Rust/Axum  →  SQLite / S3
             ↓
         templates/
             ↓
         static/
```

| Layer | Files | Responsibility |
|---|---|---|
| **Routing** | `src/routes/*.rs` | Accept HTTP requests, call db, render template |
| **Data** | `src/db.rs` | All SQL queries — nothing else |
| **Models** | `src/models.rs` | Shared data structs used across routes |
| **Storage** | `src/storage.rs` | S3 upload/delete only |
| **Templates** | `templates/` | HTML structure and layout — no business logic |
| **Styles** | `static/style.css` | Visual appearance only |
| **Behaviour** | `static/*.js` | Client-side interactivity |
| **Reverse proxy** | `deploy/nginx.conf` | TLS, rate limiting, caching headers, gzip |

**Rule:** code belongs in the layer that owns its concern. A route handler should not
build raw SQL strings. A template should not make routing decisions. A JS file should
not know about database IDs beyond what the server gave it.

---

## Page templates

Every user-facing page must extend `base.html`:

```html
{% extends "base.html" %}
{% block title %}Page Title{% endblock %}
{% block content %}
  ...
{% endblock %}
```

`base.html` is the single source of truth for everything that must appear on every page:
- Global stylesheet (`style.css`)
- Self-hosted HTMX (`htmx.min.js`)
- `IS_ADMIN` JS flag
- Command palette (`palette.js`)
- Site header with nav and Ctrl+K hint
- `hx-boost="true"` on `<body>` for fast navigation

**Exception:** `admin.html` is standalone (no base.html inheritance) because it has a
different header. It must manually include all of the above. When adding a new global
feature, update both `base.html` **and** `admin.html`.

---

## Global JavaScript features

Any JS that injects DOM nodes (like the command palette overlay) must handle two cases:

1. **Initial load** — listen on `DOMContentLoaded`
2. **`hx-boost` navigation** — HTMX replaces `<body>` content without a full page reload,
   so `DOMContentLoaded` does not fire again. Listen on `htmx:afterSwap` as well.
3. **Guard against duplicates** — check if the element already exists before injecting.

```js
function myFeatureInit() {
  if (document.getElementById('my-element')) return; // already present
  // ... inject
}
document.addEventListener('DOMContentLoaded', myFeatureInit);
document.addEventListener('htmx:afterSwap', myFeatureInit);
```

---

## Adding a new section (e.g. a new tracker or tool)

Checklist:

- [ ] Create `templates/<section>/feed.html` extending `base.html`
- [ ] Add route module `src/routes/<section>.rs`
- [ ] Register routes in `src/main.rs`
- [ ] Add DB functions to `src/db.rs`, migration to `run_migrations()`
- [ ] Add nav link to `base.html` header
- [ ] Add palette command to `static/palette.js` COMMANDS array
- [ ] Add CSS under a named section comment in `style.css`

---

## CSS scope

All styles live in `static/style.css`. Sections are marked with comments:

```css
/* ── Section Name ──────────────────────────── */
```

Page-specific styles go under their section heading. Avoid inline `<style>` blocks in
templates except for admin.html (which has its own layout). When a style would be useful
across multiple pages, move it to the global file.

---

## Fitness tracker (Nocturne)

The `/fitness` section is a session-gated, phone-first tracker with its own dark visual
layer — the "Nocturne" design system, scoped under `body.fitness-dark` in `style.css`
(`--noc-*` tokens, `.noc-*` primitives). The rest of the site keeps the light default.

Information architecture (mockups and decisions in `docs/design/fitness-redesign/`):

- **Today** (`/fitness?date=`) — calorie ring + macro rails against editable daily
  targets, a Sunday-first week strip (tap a day to load it), entries grouped into
  breakfast/lunch/dinner/snack slots (clock-inferred default when logging), inline entry
  editing, copy-yesterday, and a sticky Scan · Search · Copy action bar.
- **Add sheet** — full-screen overlay; scanner is the default tab (known barcodes become
  a one-tap log card with portion buttons; unknown ones prefill the add-food form from
  OpenFoodFacts), plus Recent, Favourites, saved Meals and Search tabs.
- **Library** — grouped by category with filter chips; detail form edits per-100g
  values, package/portions, category, favourite and default portion, and shows a 14-day
  usage strip.
- **Week** (`/fitness/week`) — read-only trends: calorie bars vs target, protein
  average/hit-rate, days-logged streak, weight trend with one-tap log, most-logged foods.
- **Desktop** (≥900px) — same column wider, plus a keyboard quick-add (type-ahead;
  Enter logs the food's default portion into the clock-inferred slot).

---

## Artportfolio feed & filter rail

The mechanics behind `src/routes/feed.rs` — CLAUDE.md carries the summary;
this is the reference.

- **One filter, four routes.** `GET /artportfolio`, `GET /artportfolio/htmx/posts`,
  `GET /artportfolio/api/posts` and `GET /artportfolio/{id}` all share
  `PageQuery::filter()`, which builds one `PostFilter` from `q` (caption),
  `tags` (comma-separated), `collection` (slug) and `vis` (comma-separated
  subset of `public,unlisted,hidden`, admin-only — silently dropped for a
  non-admin or previewing viewer). `db::get_posts_page()` and
  `db::count_posts()` both take that same `PostFilter`, so the page head's
  total and the grid always agree.
- **Month grouping happens in the handler.** Posts are grouped into
  `MonthGroup`s keyed on `created_at[..7]`, each month rendering its own
  `columns` block — `last_month` suppresses the duplicate divider when a
  month spans a page boundary.
- **Search does not use `hx-push-url`** — that would push the fragment URL.
  Instead `htmx_posts` returns an `HX-Push-Url: /artportfolio?q=…` header on
  page 0 only.
- **Rail links carry the full next filter state.** The filter rail
  (`templates/artportfolio/partials/filter_rail.html`, `rail_filters.html`,
  `rail_collections.html`) renders collections and tags as toggle links built
  by `collection_rail_links`/`tag_rail_links` — each link's URL already
  carries the *entire* next filter state (not just its own toggle), built
  from the current `PostFilter` via `filter_url`/`page_url`, so composing
  filters (a collection click then a tag click) needs no client-side merging.
  Admin-only, the rail adds the visibility trio and a `+` new-collection
  input.
- **The rail re-renders out-of-band.** A rail click only swaps `#feed`, so
  `htmx_posts` re-renders `rail_filters.html` (shared by `{% include %}` with
  the full-page path) as an `#art-rail-filters` OOB block on every page-0
  htmx response, rebuilt from the just-applied filter — this is what keeps
  `is-active`/`is-checked` states and counts from freezing after the first
  click.
- **`#art-rail-state`** (hidden inputs: `active_collection`/`active_tags`/
  `active_vis`/`preview`) exists for one consumer only: the search input's
  own `hx-include`, since typing a search is the one rail control whose own
  request does not carry the rest of the filter.

---

## Drinking game (`/drinks`)

The `drinkinggame` crate — own DB, own name+PIN sessions, SSE leaderboards —
nested via `nest_service` in `main.rs`. Its templates do NOT extend
`base.html` (recorded exception).

- **Shell:** a three-tab phone layout (GAME / STANDINGS / ROOM — the ROOM tab
  relabels to TABLE while 3 Man is running) plus a public spectator "big
  screen" view (`/drinks/room/{code}/screen`, joinable via an in-page QR
  code) that mirrors the live game over the same SSE stream.
- **Games sharing a room:** Ring of Fire (card draws, server-side rule
  presets at `/drinks/presets`, Jack "make a rule" flow, King's Cup); 3 Man
  (`three_man.rs` — dice, 3-hits-the-3-Man hand-off, doubles that hand out
  gift dice in "both" or "split" mode with payback, per-room async locking
  around each action route); Last Call (the third mode, v1 merged
  2026-08-12); Backroom (the fourth, a hidden-role game — see below).
- **Account self-service:** `/drinks/account` (rename, change PIN — current
  PIN required — and `POST /drinks/logout`, which deletes the session row as
  well as clearing the cookie), linked from the landing page and the ROOM
  tab. A rename re-broadcasts the leaderboard and room panels for every open
  room the player is in, because names are baked into already-rendered SSE
  fragments — plus the game panel, but only while a game is active (with none
  running, `broadcast_game` publishes the name-free idle panel and would wipe
  a game-over summary still on screen).
- **Per-viewer personalization:** UI fragments personalize via a
  `data-show-player`/`data-hide-player`/`data-me-text` attribute contract
  that client-side `personalize()` JS resolves against the viewer's own
  player id — e.g. a hand-off picker is `data-show-player`-gated to the
  roller while everyone else sees a `data-hide-player`-gated spectator banner
  for the same moment.

  **That contract cannot carry a secret.** `personalize()` only sets
  `el.hidden`; the markup still reaches every subscriber, and `screen.html`
  runs no `personalize()` at all by design. It answers "whose button is
  this", never "who may know this".

### Backroom — the hidden-role game

`secret_hitler.rs` (engine) · `sh_theme.rs` (the re-theme seam) ·
`sh_routes.rs` (routes + the private pane) · `sh_render.rs` (broadcast
renderers) · `assets/sh_room.js`. Mechanically Secret Hitler, 5–10 players,
re-themed; the landing page carries the credit.

**Re-theming is one file.** Every player-facing string lives in
`sh_theme.rs` as a const or a fn over the engine's enums, and theme text is
resolved at render time and never serialized — so a re-theme reaches games
already in flight. The one string that is *not* themeable is
`secret_hitler::KIND`, which is persisted in `games.kind`: renaming it
orphans every live row.

**The secrecy contract.** `RoomHub` has one broadcast channel per *room*,
never per player, and `routes::sse_stream` takes **no session extractor** —
so every byte published to a room is readable by anyone with the 4-letter
code, the cookie-less spectator screen included. Therefore:

- `SecretHitlerState::public_view()` is a projection that structurally never
  reads a secret field. `votes` is touched only through `.is_some()`, so a
  ballot laid face-down is public and its value is not; `role` only inside a
  `Phase::Over` gate. Each secret field is doc-commented with the test that
  pins it.
- Every builder in `sh_render.rs` takes `&ShPublicView` and never
  `&SecretHitlerState`. The signature is the proof; a source sweep fails the
  build if one drifts. `sh_view_data` lives in `sh_routes.rs` and returns the
  projection, so `game.rs` never names the full state type either.
- **Broadcast fragments are display-only** — no form, no button, no
  per-viewer attribute. Every control lives in the private pane instead.
  Stricter than needed (a vote button is not a secret), and worth it: it
  turns "is this fragment safe?" from a per-element judgement call into one
  line.
- Private state is **fetched, not pushed**: `GET /room/{code}/sh/private`
  takes `State`, `PlayerSession` and `Path(code)` and nothing else, so no
  request shape can name another player.
- There is no tick message and `hub.rs` is untouched. Every legal action
  changes something public, so the `Game` frame is always a real update and
  doubles as the re-fetch signal — which keeps the SSE snapshot at four
  frames.
- `#sh-private` is a **sibling** of `#game-panel` in `room.html`, because
  `swapPanel()` replaces that element's innerHTML wholesale.

**Drinks are a role oracle.** Pours reach `events` → `db::leaderboard` →
`RoomMessage::Leaderboard`, over that same unauthenticated stream. A pour
keyed on a hidden fact ("Regulars drink when a Racket passes") publishes the
per-player deltas that partition the table by faction, in public, on a
television. `sh_theme::DrinkEvent` is restricted by construction to facts
that are already public or uniform across living seats. **v1 pours nothing**
— the drinking variant turns those numbers up, subject to that rule.

**Test mode is incompatible with this game.** `DRINKS_TEST_MODE=1` exposes
`/test/act-as`, which re-issues the caller's cookie as any room member — for
the other three games that is convenience, here it is a one-POST role dump.
The flag is off by default and off on the live server.

---

## What not to do

- **Don't duplicate global features** — if something should appear everywhere, it belongs
  in `base.html`, not copy-pasted into each template.
- **Don't put SQL in route handlers** — db.rs owns all queries; routes call db functions.
- **Don't put business logic in templates** — templates receive pre-computed values from
  the route handler; they only format and display.
- **Don't use `DOMContentLoaded` alone** for DOM injection — it won't fire on `hx-boost`
  navigations (see Global JavaScript section above).
- **Don't hardcode image size limits in one place** — the 35 MB upload limit is enforced
  at three layers (nginx, Axum, app). Change all three together or the most restrictive
  one silently wins.
