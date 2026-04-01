# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build            # debug build
cargo build --release  # release build
cargo run              # run dev server on :3000
cargo test             # run all tests
cargo clippy           # lint
cargo fmt              # format
cargo fmt --check      # check formatting without modifying
```

Tests live in `src/db.rs`, `src/routes/feed.rs`, and `src/routes/admin.rs`.

## Environment

Copy `.env.example` to `.env`. Key variables:

| Variable | Purpose |
|----------|---------|
| `DATABASE_URL` | SQLite path (default: `sqlite:./portfolio.db`) |
| `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` | Cloudflare R2 credentials |
| `R2_ACCOUNT_ID` / `R2_BUCKET` / `R2_PUBLIC_URL` | R2 bucket config |
| `RP_ID` / `RP_ORIGIN` | WebAuthn relying party (domain / full origin URL) |

DB migrations run automatically at startup via `db::run_migrations()`.

## Architecture

Single Rust/Axum binary with server-side rendering via Askama templates + HTMX for dynamic updates.

**Stack:** Rust + Axum 0.8 · SQLite via sqlx 0.8 · Askama 0.15 templates · HTMX · Cloudflare R2 (S3-compatible) · WebAuthn passkeys (webauthn-rs 0.5)

**Request flow:**
1. `src/main.rs` — builds `AppState` (db pool, R2 client, WebAuthn instance), registers routes, starts hourly cleanup task (expired sessions + challenges)
2. `src/middleware.rs` — `AuthSession` extractor: validates session cookie, redirects to `/login` if invalid; `LocalhostOnly` guard blocks passkey registration from external IPs
3. Routes return `Html(template.render())` for full pages or HTML fragments for HTMX swaps

**Route modules:**
- `src/routes/feed.rs` — `GET /` (SSR feed), `GET /htmx/posts?page=N` (HTMX paginated cards), `GET /api/posts?page=N` (JSON API)
- `src/routes/admin.rs` — `GET /admin` (auth-gated), `POST /api/admin/posts` (multipart upload with magic-bytes validation), `DELETE /api/admin/posts/{id}`
- `src/routes/auth.rs` — WebAuthn registration ceremony (localhost-only) and login ceremony; creates session cookie on success

**Data layer (`src/db.rs`):** All SQLx queries — posts CRUD, session management (30-day expiry), passkey credential storage, ephemeral auth challenge state (5-min expiry).

**Storage (`src/storage.rs`):** R2Storage wraps aws-sdk-s3 with ByteStream upload/delete; generates public URLs from `R2_PUBLIC_URL`.

## WebAuthn Notes

- Passkey **registration** is restricted to localhost (`LocalhostOnly` middleware) — nginx blocks `/api/auth/register/` externally
- `RP_ID` must match the domain exactly; `RP_ORIGIN` must be the full origin with scheme and port
- Credentials stored as serialized JSON in `passkey_credentials` table

## Deployment

Deploy config is in `deploy/`:
- `portfolio.service` — systemd unit (runs as `portfolio` user, reads `.env`)
- `nginx.conf` — reverse proxy with rate limiting on `/api/auth/` (10 req/min, burst 15)

Release binary + `templates/` + `static/` must all be present at the working directory when the binary runs (Askama templates are compiled into the binary, but static assets are served from disk).
