# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
./scripts/verify.sh    # the gate: fmt + clippy + tests + JS syntax — run this before claiming done
cargo build            # debug build
cargo build --release  # release build
cargo run              # run dev server on :3000
cargo test --workspace # run all tests — the --workspace flag is NOT optional, see below
cargo test <name>      # run a single test, e.g. cargo test test_insert_and_get_post
cargo clippy           # lint
cargo fmt              # format
cargo fmt --check      # check formatting without modifying
```

When building without a live database (e.g. on the server): `SQLX_OFFLINE=true cargo build --release`. The `drinkinggame` crate uses runtime-checked sqlx queries — it has no `.sqlx` cache entries, and `cargo sqlx prepare` remains portfolio-only.

The repo is a cargo workspace, but the root `Cargo.toml` is *also* a package — so bare `cargo test` runs the current package only and silently skips `drinkinggame`'s 177 tests (**80 of 257 run**). Always pass `--workspace`, or just run `./scripts/verify.sh`, which does. `cargo run -p drinkinggame` serves the drinking game standalone on `:3001` (no portfolio, no nginx).

**Which worktree/branch am I in, and what else is in flight?** See `docs/WORKTREES.md` — the live index of work streams, worktree layout and branch conventions. Read it before creating a branch or worktree.

`./scripts/verify.sh` is the single acceptance gate — `cargo fmt --check`, `cargo clippy`, the workspace test suite, and `node --check` over `static/*.js` (a nested palette entry broke `palette.js` once; nothing else catches JS syntax). It runs clippy *without* `-D warnings` because the tree carries 19 pre-existing warnings; promote it once that reaches zero.

Tests live in `src/db.rs`, `src/routes/feed.rs`, `src/routes/admin.rs`, and `src/routes/nutrition.rs` (portfolio, 76 tests) plus `tests/static_assets.rs` (4 tests — guards `static/*.css` against nested `/* */` comment markers, which browsers resolve by silently dropping the next rule). The `drinkinggame` crate has its own, larger suite: unit tests across `drinkinggame/src/*.rs` (rooms, db, rules, hub, render, `three_man.rs` state machine — 100 tests) plus integration tests in `drinkinggame/tests/http.rs` (77 tests) covering both games' routes end to end.

## Environment

Copy `.env.example` to `.env`. Key variables:

| Variable | Purpose |
|----------|---------|
| `DATABASE_URL` | SQLite path (e.g. `sqlite:./portfolio.db` or absolute `sqlite:///opt/portfolio/portfolio.db`) |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | S3-compatible storage credentials |
| `STORAGE_ENDPOINT` | S3 endpoint URL (e.g. `https://hel1.your-objectstorage.com`) |
| `STORAGE_BUCKET` | Bucket name |
| `STORAGE_PUBLIC_URL` | Public base URL for served images |
| `RP_ID` / `RP_ORIGIN` | WebAuthn relying party (domain / full origin URL) |
| `DRINKS_DATABASE_URL` | SQLite path for the drinking game (separate file from the portfolio DB) |
| `DRINKS_SOUNDS_DIR` | Directory the drinking game reads sound-effect mp3s from at request time (default `drinks-sounds`, relative to the working directory) |

DB migrations run automatically at startup via `db::run_migrations()`.

## Architecture

Single Rust/Axum binary with server-side rendering via Askama templates + HTMX for dynamic updates.

**Stack:** Rust + Axum 0.8 · SQLite via sqlx 0.8 · Askama 0.15 templates · HTMX · S3-compatible object storage (Hetzner Object Storage) · WebAuthn passkeys (webauthn-rs 0.5)

**Request flow:**
1. `src/main.rs` — builds `AppState` (db pool, ObjectStorage client, WebAuthn instance), registers routes, starts hourly cleanup task (expired sessions + challenges)
2. `src/middleware.rs` — `AuthSession` extractor: validates session cookie, redirects to `/admin/login` if invalid; `LocalhostOnly` guard blocks passkey registration from external IPs
3. Routes return `Html(template.render())` for full pages or HTML fragments for HTMX swaps

**Route modules:**
- `src/routes/hub.rs` — `GET /` (hub/index page)
- `src/routes/feed.rs` — `GET /artportfolio` (art feed), `GET /artportfolio/htmx/posts?page=N&q=&last_month=` (HTMX month sections), `GET /artportfolio/api/posts?page=N&q=` (JSON API). All three take the same optional `q` caption filter, applied by `db::get_posts_page()`; the page head's total comes from `db::count_posts()`, called only on a full page render. Posts are grouped into `MonthGroup`s **in the handler** (keyed on `created_at[..7]`), each month rendering its own `columns` block — `last_month` suppresses the duplicate divider when a month spans a page boundary. Search does **not** use `hx-push-url`: that pushes the fragment URL, so `htmx_posts` returns an `HX-Push-Url: /artportfolio?q=…` header on page 0 only.
- `src/routes/admin.rs` — `GET /admin` (auth-gated), `POST /api/admin/posts` (multipart upload), `DELETE /api/admin/posts/{id}`
- `src/routes/auth.rs` — WebAuthn registration ceremony (localhost-only) and login ceremony; creates session cookie on success
- `src/routes/tasks.rs` — Drawing Tasks, a LeetCode-inspired practice board: reference images with any number of attached task prompts, filterable by subject/difficulty/task type. `GET /tasks` and `GET /tasks/htmx/board` are public (`OptionalAuth` — admins additionally see management controls); all mutations (`POST /api/tasks`, `DELETE /api/tasks/{id}`, `POST /api/tasks/{id}/toggle`, `POST/DELETE /api/tasks/images…`) require `AuthSession`. Deleting an image cascades to its tasks and returns the URL for S3 cleanup.
- `src/routes/nutrition.rs` — **all routes require `AuthSession`** (decision 2026-08-01). Pages: `GET /fitness?date=` (Today — targets ring, week strip, meal slots), `GET /fitness/week` (week view: calorie bars, protein stats, streak, weight, most-logged). HTMX fragments: `/fitness/htmx/` `day?date=`, `week-strip?date=`, `targets`, `recent`, `favourites`, `meals`, `food-search?q=`, `match-card/{id}`, `barcode-match/{code}`, `entries/{id}/edit`. Actions: `POST /fitness/copy-day`, `POST /fitness/quick-log`. API: `POST/PUT/DELETE /api/nutrition/food-items…` + `POST …/{id}/favourite`, `POST/PUT/DELETE /api/nutrition/entries…` (entries carry a `slot`: breakfast/lunch/dinner/snack/other, clock-inferred client-side), `POST /api/nutrition/targets`, `POST /api/nutrition/weights`, `POST/DELETE /api/nutrition/recipes…` + `POST …/{id}/log`

**Models (`src/models.rs`):** Shared structs — `Post`, `FoodItem` (incl. `category`, `is_favourite`, `default_portion_g`), `MealEntry`/`MealEntryWithFood` (computed macros scaled by portion grams; carries `slot`), `Targets`, `RecentFood`, `RecipeWithTotals`, `TaskImage`/`DrawingTaskWithImage` (drawing tasks board).

**Data layer (`src/db.rs`):** All SQLx queries — posts CRUD, session management (30-day expiry), passkey credential storage, ephemeral auth challenge state (5-min expiry). Nutrition functions: `get_food_items()`, `search_food_items(q)`, `insert_food_item()`, `delete_food_item()` (returns image URL for S3 cleanup), `get_meal_entries_for_date(date)`, `insert_meal_entry()`, `delete_meal_entry()`. Migrations run via `include_str!()` in `run_migrations()` — twelve exist (001 initial schema, 002 post fields, 003 `food_items`/`meal_entries`, 004 image variants, 005 package size, 006 custom portions, 007 drawing tasks, 008 `meal_entries.slot`, 009 `targets` single-row table, 010 food metadata: `category`/`is_favourite`/`default_portion_g`, 011 `weights` + `recipes`/`recipe_items`, 012 `posts.image_width`/`image_height`). Add new migrations as additional `sqlx::query(...).execute(pool)` calls; use `IF NOT EXISTS` / `let _ =` duplicate-column tolerance for idempotence. Schema changes require the sqlx offline-cache ritual: apply the migration to the local dev DB, `DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, commit `.sqlx/`.

**Storage (`src/storage.rs`):** `ObjectStorage` wraps aws-sdk-s3 with `force_path_style(true)` (required for non-AWS endpoints). Upload returns a public URL constructed from `STORAGE_PUBLIC_URL`.

`/drinks` is the `drinkinggame` crate (own DB, own name+PIN sessions, SSE leaderboards) nested via `nest_service` in `main.rs`; players self-serve their identity at `/drinks/account` (rename, change PIN — current PIN required —, and `POST /drinks/logout`, which deletes the session row as well as clearing the cookie), linked from the landing page and the ROOM tab. A rename re-broadcasts the leaderboard and room panels for every open room the player is in, because names are baked into already-rendered SSE fragments — plus the game panel, but only while a game is active (with none running, `broadcast_game` publishes the name-free idle panel and would wipe a game-over summary still on screen). its templates do NOT extend `base.html` (recorded exception). Redesigned as a three-tab phone shell (GAME / STANDINGS / ROOM — the ROOM tab relabels to TABLE while 3 Man is running) with a public spectator "big screen" view (`/drinks/room/{code}/screen`, joinable via an in-page QR code) that mirrors the live game over the same SSE stream. Two games share a room: Ring of Fire (card draws, server-side rule presets at `/drinks/presets`, Jack "make a rule" flow, King's Cup) and 3 Man (`three_man.rs` — dice, 3-hits-the-3-Man hand-off, doubles that hand out gift dice in "both" or "split" mode with payback, per-room async locking around each action route). Both games' UI fragments personalize per viewer via a `data-show-player`/`data-hide-player`/`data-me-text` attribute contract that client-side `personalize()` JS resolves against the viewer's own player id — e.g. a hand-off picker is `data-show-player`-gated to the roller while everyone else sees a `data-hide-player`-gated spectator banner for the same moment.

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
- **Tests:** `src/db.rs` holds the db-layer tests (nutrition CRUD, slots, targets, ranges, recipes, weights) and `src/routes/nutrition.rs` has unit tests for the ring/rail math and streak logic — 257 tests across the workspace.

## Architecture Rules

See `docs/design.md` for the human-readable design guide. Rules Claude must follow:

**Writing or executing a plan**
- Invoke the project skill `plan-economics` **before** writing a spec into a plan or starting subagent-driven-development. It sets plan sizing (one plan = one session = one deployable slice), delegates plan writing and design ingestion to subagents, and defines the per-task risk classes that decide which tasks get an LLM review. It overrides parts of `superpowers:writing-plans` and `superpowers:subagent-driven-development` — the overrides are named in the skill.
- Plan shape: `docs/superpowers/plan-template.md`. Every task carries a class and one acceptance line (`./scripts/verify.sh`).

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

**Server:** Hetzner cx23 (x86_64/amd64), Ubuntu — GitHub Actions runner must be `ubuntu-24.04` (not arm).

Deployment is automated via `.github/workflows/deploy.yml` — push to `master` builds on GitHub's x86_64 runner and deploys to the server. Manual command below is for emergency use only.

Deploy config is in `deploy/`:
- `portfolio.service` — systemd unit (runs as `portfolio` user, reads `.env`)
- `nginx.conf` — reverse proxy with rate limiting on `/api/auth/` (10 req/min, burst 5). **Not deployed by CI/CD** — must be manually copied to `/etc/nginx/sites-available/portfolio` and nginx reloaded. Use `127.0.0.1:3000` not `localhost:3000` (nginx resolves localhost to IPv6 `[::1]` but Axum only binds IPv4). Certbot manages SSL lines — always include them or HTTPS breaks. Also has two manual locations for the drinking game: `/drinks/room/*/sse` disables proxy buffering (SSE would never arrive otherwise), and `/drinks/login` has its own `zone=drinks_login` rate limit (30 req/min, burst 10) so a party's worth of guests behind one NAT IP registering at once don't hit raw 503s. All three `/drinks`-serving locations (the two above plus the catch-all `location /`) also set `proxy_set_header X-Forwarded-Proto $scheme;` — `request_origin()` (`drinkinggame/src/routes.rs`) reads it to build the absolute URL encoded into the room QR code; without it every scan would embed an `http://` link even though the site is HTTPS-only. Since this file is manually deployed, remember to add that header line on the server the next time `nginx.conf` is copied over.

Only `static/` (served from disk) and `.env` must be present alongside the binary — Askama templates are compiled in. `/opt/portfolio/src/` on the server is a stale old checkout unused by the deploy process.

On first deploy of the drinking game, add `DRINKS_DATABASE_URL=sqlite:///opt/portfolio/drinkinggame.db` to the server's `.env` — the relative-path fallback only works locally because `portfolio.service` sets `WorkingDirectory`.

The drinking game's fonts (woff2) are `include_bytes!`-compiled into the binary — nothing to copy to the server for those. Its sound effects are the opposite: no mp3s are committed to the repo (out of scope by design), so the game ships silent until mp3s are dropped in. To enable sound, create the directory named by `DRINKS_SOUNDS_DIR` (default `drinks-sounds`, relative to `portfolio.service`'s `WorkingDirectory`) on the server and drop in `drink.mp3`, `shot.mp3`, `card-draw.mp3`, `card-use.mp3`, `dice-roll.mp3`, `dice-give.mp3` — any other filename 404s. No restart needed; the route reads from disk per request.

Server update command:
```bash
cd /opt/portfolio/src && git pull && SQLX_OFFLINE=true cargo build --release && cp target/release/drawingportfolio /opt/portfolio/ && systemctl restart portfolio
```
