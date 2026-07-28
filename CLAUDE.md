# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build            # debug build
cargo build --release  # release build
cargo run              # run dev server on :3000
cargo test             # run all tests
cargo test <name>      # run a single test, e.g. cargo test test_insert_and_get_post
cargo clippy           # lint
cargo fmt              # format
cargo fmt --check      # check formatting without modifying
```

When building without a live database (e.g. on the server): `SQLX_OFFLINE=true cargo build --release`. The `drinkinggame` crate uses runtime-checked sqlx queries — it has no `.sqlx` cache entries, and `cargo sqlx prepare` remains portfolio-only.

The repo is a cargo workspace — `cargo build` / `cargo test` at the root cover both the `drawingportfolio` binary and the `drinkinggame` crate. `cargo run -p drinkinggame` serves the drinking game standalone on `:3001` (no portfolio, no nginx).

Tests live in `src/db.rs`, `src/routes/feed.rs`, and `src/routes/admin.rs`.

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
- `src/routes/feed.rs` — `GET /artportfolio` (art feed), `GET /artportfolio/htmx/posts?page=N` (HTMX paginated cards), `GET /artportfolio/api/posts?page=N` (JSON API)
- `src/routes/admin.rs` — `GET /admin` (auth-gated), `POST /api/admin/posts` (multipart upload), `DELETE /api/admin/posts/{id}`
- `src/routes/auth.rs` — WebAuthn registration ceremony (localhost-only) and login ceremony; creates session cookie on success
- `src/routes/nutrition.rs` — `GET /fitness` (tracker page), `GET /fitness/htmx/day?date=` (HTMX day loader), `POST /api/nutrition/food-items` (add food item with optional image), `DELETE /api/nutrition/food-items/{id}`, `POST /api/nutrition/entries` (log meal), `DELETE /api/nutrition/entries/{id}`

**Models (`src/models.rs`):** Shared structs — `Post`, `FoodItem`, `MealEntry`, `MealEntryWithFood` (computed macros scaled by portion grams).

**Data layer (`src/db.rs`):** All SQLx queries — posts CRUD, session management (30-day expiry), passkey credential storage, ephemeral auth challenge state (5-min expiry). Nutrition functions: `get_food_items()`, `search_food_items(q)`, `insert_food_item()`, `delete_food_item()` (returns image URL for S3 cleanup), `get_meal_entries_for_date(date)`, `insert_meal_entry()`, `delete_meal_entry()`. Migrations run via `include_str!()` in `run_migrations()` — four migrations exist (001 initial schema, 002 adds `format`/`file_size_bytes` to posts, 003 adds `food_items`/`meal_entries`, 004 adds `webp_url`/`avif_url` to posts). Add new migrations as additional `sqlx::query(...).execute(pool)` calls; use `IF NOT EXISTS` / `PRAGMA table_info` guards for idempotence.

**Storage (`src/storage.rs`):** `ObjectStorage` wraps aws-sdk-s3 with `force_path_style(true)` (required for non-AWS endpoints). Upload returns a public URL constructed from `STORAGE_PUBLIC_URL`.

`/drinks` is the `drinkinggame` crate (own DB, own name+PIN sessions, SSE leaderboards) nested via `nest_service` in `main.rs`; its templates do NOT extend `base.html` (recorded exception).

## Key implementation details

- **Post cards** are built as formatted strings in `post_card_html()` / `admin_post_card_html()`, not Askama templates
- **Image uploads:** 35 MB max, JPEG/PNG/WebP only, validated by magic bytes (not just MIME type). Three-layer limit: nginx `client_max_body_size` → Axum `DefaultBodyLimit` (set in `main.rs`) → app-level `MAX_IMAGE_BYTES` in `admin.rs` — all three must match.
- **Image variants:** On upload, WebP and AVIF variants are generated concurrently via `tokio::join!`. Uses the `image` crate (WebP) and `ravif` crate (AVIF, quality=80 speed=6). Failures are non-fatal — if either variant fails, its URL is stored as empty and `post_card_html()` / `admin_post_card_html()` render a `<picture>` element that falls back to the original JPEG.
- **Pagination:** fetches N+1 rows to detect `has_more` without a COUNT query
- **Timestamps:** stored as ISO8601 `TEXT` in SQLite (not UNIX integers) — avoids sqlx nullable inference issues with `DATETIME`
- **chrono** is pinned to `0.4.34` — `0.4.35+` renames `Duration` to `TimeDelta` (breaking change)
- **Nutrition tracker:** Barcode scanning via native `BarcodeDetector` API with OpenFoodFacts fallback. `OptionalAuth` extractor used (not `AuthSession`) — page is public, add/delete food items requires admin.
- **Tests:** `src/db.rs` includes 8 nutrition tests (food item CRUD, search, macro scaling) in addition to the original post/auth tests.

## Architecture Rules

See `docs/design.md` for the human-readable design guide. Rules Claude must follow:

**Templates**
- Every user-facing page must extend `base.html`. Exception: `admin.html` is standalone — when adding any new global feature (script, style, header element), update **both** `base.html` and `admin.html`.
- Never add `<style>` blocks to templates that extend `base.html` — put styles in `style.css` under a named section comment.

**JavaScript lifecycle with hx-boost**
- `hx-boost="true"` on `<body>` means HTMX replaces body content on navigation without a full page reload — `DOMContentLoaded` only fires once (initial load).
- Any JS that injects DOM nodes must listen on **both** `DOMContentLoaded` and `htmx:afterSwap`, and guard with an existence check to prevent duplicates.

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
- `nginx.conf` — reverse proxy with rate limiting on `/api/auth/` (10 req/min, burst 5). **Not deployed by CI/CD** — must be manually copied to `/etc/nginx/sites-available/portfolio` and nginx reloaded. Use `127.0.0.1:3000` not `localhost:3000` (nginx resolves localhost to IPv6 `[::1]` but Axum only binds IPv4). Certbot manages SSL lines — always include them or HTTPS breaks. Also has two manual locations for the drinking game: `/drinks/room/*/sse` disables proxy buffering (SSE would never arrive otherwise), and `/drinks/login` has its own `zone=drinks_login` rate limit (30 req/min, burst 10) so a party's worth of guests behind one NAT IP registering at once don't hit raw 503s.

Only `static/` (served from disk) and `.env` must be present alongside the binary — Askama templates are compiled in. `/opt/portfolio/src/` on the server is a stale old checkout unused by the deploy process.

On first deploy of the drinking game, add `DRINKS_DATABASE_URL=sqlite:///opt/portfolio/drinkinggame.db` to the server's `.env` — the relative-path fallback only works locally because `portfolio.service` sets `WorkingDirectory`.

Server update command:
```bash
cd /opt/portfolio/src && git pull && SQLX_OFFLINE=true cargo build --release && cp target/release/drawingportfolio /opt/portfolio/ && systemctl restart portfolio
```
