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

When building without a live database (e.g. on the server): `SQLX_OFFLINE=true cargo build --release`

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

DB migrations run automatically at startup via `db::run_migrations()`.

## Architecture

Single Rust/Axum binary with server-side rendering via Askama templates + HTMX for dynamic updates.

**Stack:** Rust + Axum 0.8 · SQLite via sqlx 0.8 · Askama 0.15 templates · HTMX · S3-compatible object storage (Hetzner Object Storage) · WebAuthn passkeys (webauthn-rs 0.5)

**Request flow:**
1. `src/main.rs` — builds `AppState` (db pool, ObjectStorage client, WebAuthn instance), registers routes, starts hourly cleanup task (expired sessions + challenges)
2. `src/middleware.rs` — `AuthSession` extractor: validates session cookie, redirects to `/admin/login` if invalid; `LocalhostOnly` guard blocks passkey registration from external IPs
3. Routes return `Html(template.render())` for full pages or HTML fragments for HTMX swaps

**Route modules:**
- `src/routes/feed.rs` — `GET /` (SSR feed), `GET /htmx/posts?page=N` (HTMX paginated cards), `GET /api/posts?page=N` (JSON API)
- `src/routes/admin.rs` — `GET /admin` (auth-gated), `POST /api/admin/posts` (multipart upload), `DELETE /api/admin/posts/{id}`
- `src/routes/auth.rs` — WebAuthn registration ceremony (localhost-only) and login ceremony; creates session cookie on success

**Data layer (`src/db.rs`):** All SQLx queries — posts CRUD, session management (30-day expiry), passkey credential storage, ephemeral auth challenge state (5-min expiry). Migrations run via `include_str!("../migrations/001_initial.sql")` — to add a migration, add a new `sqlx::query(...).execute(pool)` call in `run_migrations()`.

**Storage (`src/storage.rs`):** `ObjectStorage` wraps aws-sdk-s3 with `force_path_style(true)` (required for non-AWS endpoints). Upload returns a public URL constructed from `STORAGE_PUBLIC_URL`.

## Key implementation details

- **Post cards** are built as formatted strings in `post_card_html()` / `admin_post_card_html()`, not Askama templates
- **Image uploads:** 10 MB max, JPEG/PNG/WebP only, validated by magic bytes (not just MIME type)
- **Pagination:** fetches N+1 rows to detect `has_more` without a COUNT query
- **Timestamps:** stored as ISO8601 `TEXT` in SQLite (not UNIX integers) — avoids sqlx nullable inference issues with `DATETIME`
- **chrono** is pinned to `0.4.34` — `0.4.35+` renames `Duration` to `TimeDelta` (breaking change)

## WebAuthn Notes

- Passkey **registration** is restricted to localhost (`LocalhostOnly` middleware) — nginx blocks `/api/auth/register/` externally
- `RP_ID` must match the domain exactly; `RP_ORIGIN` must be the full origin with scheme and port
- `danger-allow-state-serialisation` feature is required to serialize WebAuthn challenge state to SQLite
- Credentials stored as serialized JSON in `passkey_credentials` table
- To register on the production domain: temporarily remove `deny all` from nginx register location, visit `https://<domain>/admin/register`, register, then restore the deny

## Deployment

Deploy config is in `deploy/`:
- `portfolio.service` — systemd unit (runs as `portfolio` user, reads `.env`)
- `nginx.conf` — reverse proxy with rate limiting on `/api/auth/` (10 req/min, burst 5)

Release binary + `templates/` + `static/` must all be present at the working directory when the binary runs (Askama templates are compiled into the binary, but static assets are served from disk).

Server update command:
```bash
cd /opt/portfolio/src && git pull && SQLX_OFFLINE=true cargo build --release && cp target/release/drawingportfolio /opt/portfolio/ && systemctl restart portfolio
```
