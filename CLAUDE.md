# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
./scripts/check.sh     # item gate (~6s): compile + JS syntax — after every item; add targeted tests when logic is touched
./scripts/verify.sh    # pack gate: fmt + clippy + tests + JS syntax — before review/merge and before claiming done
cargo test --workspace # run all tests — the --workspace flag is NOT optional, see below
```

When building without a live database (e.g. on the server): `SQLX_OFFLINE=true cargo build --release`. The `drinkinggame` crate uses runtime-checked sqlx queries — it has no `.sqlx` cache entries, and `cargo sqlx prepare` remains portfolio-only.

The repo is a cargo workspace, but the root `Cargo.toml` is *also* a package — so bare `cargo test` runs the current package only and silently skips the entire `drinkinggame` suite (over two-thirds of the workspace's tests). Always pass `--workspace`, or just run `./scripts/verify.sh`, which does. `cargo run -p drinkinggame` serves the drinking game standalone on `:3001` (no portfolio, no nginx).

**Which worktree/branch am I in, and what else is in flight?** See `docs/WORKTREES.md` — the live index of work streams, worktree layout and branch conventions. Read it before creating a branch or worktree.

`./scripts/verify.sh` is the single acceptance gate — `cargo fmt --check`, `cargo clippy`, the workspace test suite, and `node --check` over `static/*.js` **and `drinkinggame/assets/*.js`** (a nested palette entry broke `palette.js` once; nothing else catches JS syntax). It runs clippy *without* `-D warnings` because the tree carries **21 distinct pre-existing warnings**, all in the `drawingportfolio` crate — `drinkinggame` is clean. Promote it to `-D warnings` once that reaches zero. Beware the count: `cargo clippy --workspace --all-targets` prints 23 `warning:` lines, but two of those are per-target rollup summaries. Compare against **21**, not 23. (Re-measured 2026-08-12 on the slice-3 + Last Call merge; the four added since 17 are dead-code items on collections/tags models awaiting later slices.)

Tests live in `src/db.rs` (db-layer: posts, sessions, nutrition CRUD, slots, targets, ranges, recipes, weights), `src/routes/feed.rs`, `src/routes/admin.rs` and `src/routes/nutrition.rs` (ring/rail math, streak logic), plus `tests/static_assets.rs` — which guards `static/*.css` against nested `/* */` comment markers, which browsers resolve by silently dropping the next rule. The `drinkinggame` crate has its own, larger suite: unit tests across `drinkinggame/src/*.rs` (rooms, db, rules, hub, render, `three_man.rs` state machine, Last Call) plus integration tests in `drinkinggame/tests/http.rs` covering all three games' routes end to end. Workspace total: **746 tests** (measured 2026-08-13 — if you touch this number, get it from `cargo test --workspace`, never from another document).

## Environment

Copy `.env.example` to `.env` — it documents every variable. DB migrations run automatically at startup via `db::run_migrations()`.

## Architecture

Single Rust/Axum binary with server-side rendering via Askama templates + HTMX for dynamic updates.

**Request flow:**
1. `src/main.rs` — builds `AppState` (db pool, ObjectStorage client, WebAuthn instance), registers routes, starts hourly cleanup task (expired sessions + challenges)
2. `src/middleware.rs` — `AuthSession` extractor: validates session cookie, redirects to `/admin/login` if invalid; `LocalhostOnly` guard blocks passkey registration from external IPs
3. Routes return `Html(template.render())` for full pages or HTML fragments for HTMX swaps

**Route modules:**
- `src/routes/hub.rs` — `GET /` (hub/index page)
- `src/routes/feed.rs` — `GET /artportfolio` (art feed), `GET /artportfolio/htmx/posts` (HTMX month sections), `GET /artportfolio/api/posts` (JSON API), `GET /artportfolio/{id}` (single-post permalink). All four share `PageQuery::filter()`, building one `PostFilter` (`q`/`tags`/`collection`/`vis` — `vis` is admin-only, silently dropped otherwise); `get_posts_page()` and `count_posts()` take that same filter, so the page head's total and the grid always agree. Month grouping happens **in the handler**; the filter rail's toggle links each carry the *full* next filter state, and page-0 htmx responses re-render the rail as an `#art-rail-filters` OOB block so its active states and counts never freeze. Full mechanics (rail link building, `HX-Push-Url` contract, `#art-rail-state`) in `docs/design.md` §Artportfolio feed & filter rail.
- `src/routes/admin.rs` — `GET /admin` (auth-gated), `POST /api/admin/posts` (multipart upload, optional `visibility` field), `DELETE /api/admin/posts/{id}`, `PATCH /api/admin/posts/{id}/visibility` (form-encoded, returns the re-rendered card), `PATCH /api/admin/posts/{id}` (caption + comma-separated tags, returns the re-rendered card), `POST /api/admin/collections` / `DELETE /api/admin/collections/{id}` (create/delete a collection, return the rail fragment), `GET /api/admin/posts/{id}/collections` (membership checklist fragment) plus `POST`/`DELETE /api/admin/posts/{id}/collections/{cid}` (toggle membership, re-render the checklist), and `GET /api/admin/posts/{id}/edit` (caption+tags edit-form fragment) — all seven behind `AuthSession`, including the two GET fragment routes
- `src/routes/auth.rs` — WebAuthn registration ceremony (localhost-only) and login ceremony; creates session cookie on success
- `src/routes/tasks.rs` — Drawing Tasks, a LeetCode-inspired practice board: reference images with any number of attached task prompts, filterable by subject/difficulty/task type. `GET /tasks` and `GET /tasks/htmx/board` are public (`OptionalAuth` — admins additionally see management controls); all mutations (`POST /api/tasks`, `DELETE /api/tasks/{id}`, `POST /api/tasks/{id}/toggle`, `POST/DELETE /api/tasks/images…`) require `AuthSession`. Deleting an image cascades to its tasks and returns the URL for S3 cleanup.
- `src/routes/nutrition.rs` — **all routes require `AuthSession`** (decision 2026-08-01). Pages: `GET /fitness?date=` (Today — targets ring, week strip, meal slots), `GET /fitness/week` (week view: calorie bars, protein stats, streak, weight, most-logged). HTMX fragments: `/fitness/htmx/` `day?date=`, `week-strip?date=`, `targets`, `recent`, `favourites`, `meals`, `food-search?q=`, `match-card/{id}`, `barcode-match/{code}`, `entries/{id}/edit`. Actions: `POST /fitness/copy-day`, `POST /fitness/quick-log`. API: `POST/PUT/DELETE /api/nutrition/food-items…` + `POST …/{id}/favourite`, `POST/PUT/DELETE /api/nutrition/entries…` (entries carry a `slot`: breakfast/lunch/dinner/snack/other, clock-inferred client-side), `POST /api/nutrition/targets`, `POST /api/nutrition/weights`, `POST/DELETE /api/nutrition/recipes…` + `POST …/{id}/log`

**Data layer (`src/db.rs`):** All SQLx queries — posts CRUD, session management (30-day expiry), passkey credential storage, ephemeral auth challenge state (5-min expiry). Nutrition functions: `get_food_items()`, `search_food_items(q)`, `insert_food_item()`, `delete_food_item()` (returns image URL for S3 cleanup), `get_meal_entries_for_date(date)`, `insert_meal_entry()`, `delete_meal_entry()`. Migrations run via `include_str!()` in `run_migrations()` — fourteen exist (001 initial schema, 002 post fields, 003 `food_items`/`meal_entries`, 004 image variants, 005 package size, 006 custom portions, 007 drawing tasks, 008 `meal_entries.slot`, 009 `targets` single-row table, 010 food metadata: `category`/`is_favourite`/`default_portion_g`, 011 `weights` + `recipes`/`recipe_items`, 012 `posts.image_width`/`image_height`, 013 `posts.visibility`, 014 collections/tags). Add new migrations as additional `sqlx::query(...).execute(pool)` calls; use `IF NOT EXISTS` / `let _ =` duplicate-column tolerance for idempotence. Schema changes require the sqlx offline-cache ritual: apply the migration to the local dev DB, `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, commit `.sqlx/`.

**Storage (`src/storage.rs`):** `ObjectStorage` wraps aws-sdk-s3 with `force_path_style(true)` (required for non-AWS endpoints). Upload returns a public URL constructed from `STORAGE_PUBLIC_URL`.

`/drinks` is the `drinkinggame` crate (own DB, own name+PIN sessions, SSE leaderboards) nested via `nest_service` in `main.rs`; its templates do NOT extend `base.html` (recorded exception). Three-tab phone shell (GAME / STANDINGS / ROOM) plus a public spectator "big screen" view over the same SSE stream; **three games share a room** — Ring of Fire, 3 Man (`three_man.rs`) and Last Call. UI fragments personalize per viewer via the `data-show-player`/`data-hide-player`/`data-me-text` attribute contract that client-side `personalize()` resolves against the viewer's own player id. Details — account self-service at `/drinks/account`, rename re-broadcast rules, per-game mechanics — in `docs/design.md` §Drinking game.

## Key implementation details

- **Post cards** split by surface. The **feed's** card is the Askama template `templates/partials/post_card.html`, rendered by `PostCardTemplate` from three places — the inlined first page, the HTMX pagination route, and `admin.rs`'s upload response (`source == "gallery"`). Change the card in one place or the three drift. The **admin dashboard's** card is still a format string, `admin_post_card_html()`, and still emits `class="post-card"` — which is why `style.css`'s legacy `.post-card` rules must stay until `/admin` is migrated. Note `feed.rs` no longer has an `html_escape()`; Askama auto-escapes, and `admin.rs`, `tasks.rs` and `nutrition.rs` each carry their own private copy.
- **Image uploads:** 35 MB max, JPEG/PNG/WebP only, validated by magic bytes (not just MIME type). Three-layer limit: nginx `client_max_body_size` → Axum `DefaultBodyLimit` (set in `main.rs`) → app-level `MAX_IMAGE_BYTES` in `admin.rs` — all three must match.
- **Image variants:** The **client** converts to WebP via canvas before upload, so the server stores that as both `image_url` and `webp_url` (`admin.rs`: `let webp_url = image_url.clone()`). Only AVIF is produced server-side, by `encode_as_avif` (`ravif`, quality=80 speed=6) in a **detached `tokio::spawn` that runs after the response is sent** — it backfills `avif_url` via `update_post_avif_url`. Failure is non-fatal: `avif_url` stays empty and the `<picture>` in `templates/partials/post_card.html` (feed) / `admin_post_card_html()` (admin dashboard) falls back. A freshly uploaded post therefore genuinely has an empty `avif_url` for a second or two — the empty-URL branches are load-bearing, not defensive.
- **Image dimensions:** `insert_post` stores `image_width`/`image_height`, read from the image **header** via `image::ImageReader::into_dimensions()` — never a full decode, which on a 35 MB upload would cost seconds for two integers. An unparseable header yields `(0, 0)`, and the card omits both attributes rather than emitting `width="0"`, which would collapse the image.
- **Pagination:** fetches N+1 rows to detect `has_more` without a COUNT query
- **Timestamps:** stored as ISO8601 `TEXT` in SQLite (not UNIX integers) — avoids sqlx nullable inference issues with `DATETIME`
- **chrono** is pinned to `0.4.34` — `0.4.35+` renames `Duration` to `TimeDelta` (breaking change)
- **Nutrition dates/timezones:** server "today" is UTC (`Utc::now()`), meal-slot defaults come from the browser clock, and the day-step arrows format dates from local parts (never `toISOString()`, which is UTC and skips days east of UTC). For a UTC+2 user, logs between 00:00–02:00 local land on the previous UTC date — accepted single-user trade-off.
- **Nutrition tracker:** Session-gated (`AuthSession` on every route — no longer public). UI is the Nocturne dark theme: tokens live under `body.fitness-dark` in `style.css` (`--noc-*` variables, `.noc-*` primitives); design reference and decisions in `docs/design/fitness-redesign/`. The `fitness-dark` body class is re-derived across hx-boost navigations by an inline script in `base.html`/`admin.html` (hx-boost swaps body children, not attributes). Barcode scanning via native `BarcodeDetector` + OpenFoodFacts: a known barcode opens a one-tap log card in the add sheet; unknown ones prefill the add-food form (`openOffLookup` in `barcode.js`).
- **Visibility model:** `public` is listed everywhere; `unlisted` is out of the feed *and* out of the JSON API but served by its permalink; `hidden` 404s for a visitor (never 403 — that confirms the row exists). Enforcement is a required `Viewer` parameter on every post-reading `db.rs` function, so omitting it is a compile error. Each handler derives **one** effective viewer — `if session_is_admin && !preview { Admin } else { Visitor }` — and drives both the query and the template flags from it; deriving them separately renders a visitor's posts with admin badges over them. `?visitor=1` previews a visitor's view and must ride through `load_more_url()`, `page_url()` and `head_label()`, or page 0 and the first *Load more* disagree. `Visibility::from_str` is strict (a 400 for input from outside) while `from_row` fails **closed** to `Hidden`. Two accepted trade-offs: post ids are sequential, so unlisted means "not listed", not "secret"; and `/admin` shows no badge because it still renders through `admin_post_card_html()`, the legacy format string.
## Architecture Rules

See `docs/design.md` for the human-readable design guide. Rules Claude must follow:

**Templates**
- Every user-facing page must extend `base.html`. Exception: `admin.html` is standalone — when adding any new global feature (script, style, header element), update **both** `base.html` and `admin.html`.
- Never add `<style>` blocks to templates that extend `base.html` — put styles in `style.css` under a named section comment.

**JavaScript lifecycle with hx-boost**
- `hx-boost="true"` on `<body>` means HTMX replaces body content on navigation without a full page reload — `DOMContentLoaded` only fires once (initial load).
- Any JS that injects DOM nodes must listen on **both** `DOMContentLoaded` and `htmx:afterSwap`, and guard with an existence check to prevent duplicates.
- JS holding live resources (e.g. the camera stream in `barcode.js`) must release them on a body-targeted `htmx:beforeSwap` — boosted navigation never fires unload — **and** at script top level on re-execution (the script's globals reset while the old resource is still live; `barcode.js` stops any leftover `window.barcodeStream` before reassigning it).

**Adding a new section**
Must update all of: route module + `main.rs` registration, `db.rs` migration, `base.html` nav link, `palette.js` COMMANDS array, `style.css` section.

**Scope rules**
- SQL queries belong in `db.rs` only — route handlers call db functions, never raw SQL.
- Templates receive pre-computed values; they never contain logic beyond simple conditionals.
- Upload size limit is enforced at three layers (nginx `client_max_body_size`, Axum `DefaultBodyLimit`, app `MAX_IMAGE_BYTES`) — change all three together.

## WebAuthn Notes

- Passkey **registration** is restricted to localhost (`LocalhostOnly` middleware) — nginx blocks `/api/auth/register/` externally
- `RP_ID` must match the domain exactly; `RP_ORIGIN` must be the full origin with scheme and port
- `danger-allow-state-serialisation` feature is required to serialize WebAuthn challenge state to SQLite
- Credentials stored as serialized JSON in `passkey_credentials` table
- To register on the production domain: temporarily remove `deny all` from nginx register location, visit `https://<domain>/admin/register`, register, then restore the deny

## Deployment

Deployment mechanics (Hetzner server, nginx manual steps, sounds drop-in, emergency update command) live in the project skill `deploying` — invoke it before touching `deploy/`, `.github/workflows/deploy.yml`, or anything server-side.
