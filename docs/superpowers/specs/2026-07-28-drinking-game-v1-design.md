# Drinking Game v1 — Design

**Date:** 2026-07-28
**Status:** Approved (revised 2026-07-28 after advisory review)

## Summary

A Jackbox-style party companion: players open a website on their phones, join a
room with a code, and log real-life drinks and shots with big buttons. All
screens show a live leaderboard. Light idle-game mechanics come later; v1
ships the skeleton with a clean extension point.

## Decisions made during brainstorming

- **Core loop**: companion tracker — the app tracks real drinking; it does not
  drive it with prompts.
- **Screens**: phones are primary; an unauthenticated read-only big-screen
  (spectator) page is included.
- **V1 scope**: room create/join, drink & shot logging with undo, live
  leaderboard, big-screen view. No idle mechanics yet, but a defined hook.
- **Persistence**: long-term profiles — players keep lifetime stats across
  game nights. Real database from day one.
- **Hosting**: public VPS (Hetzner) behind a domain; room codes gate access.
- **Identity**: name + PIN. A profile is claimable from any device with its
  4-digit PIN. Long-lived session cookie so refreshes never re-prompt.
- **Stack**: Axum + SQLite (sqlx) + Askama templates + HTMX for button
  posts (embedded copy of the portfolio's `htmx.min.js`); live updates via
  native `EventSource` — no htmx SSE extension to vendor. Single
  self-contained binary (assets embedded via `include_str!`, no rust-embed
  dependency).

## Architecture

Single Axum service. Client → server is plain HTML form POSTs; server →
client is Server-Sent Events carrying rendered HTML fragments that HTMX swaps
in. There is no client-side state: a locked or refreshed phone is correct the
moment it reconnects (browsers auto-reconnect SSE).

Each active room owns a `tokio::sync::broadcast` channel. Every connected
screen (player phone or spectator view) subscribes via the room's SSE
endpoint. Any state change re-renders the leaderboard fragment and broadcasts
it to all subscribers.

### Portfolio-server integration

The game lives in this repo and deploys with the portfolio site. The repo
becomes a cargo workspace: the existing `drawingportfolio` crate plus a new
`drinkinggame` crate (**lib + thin bin**):

- Library exposes `pub async fn router(config: Config) -> axum::Router` —
  a **stateless** `Router<()>`: the crate builds and owns its own internal
  state (pool, room hub) so it composes cleanly with `.nest()` and never
  depends on the portfolio's `AppState`. `Config` carries the DB URL, the
  public base path (`/drinks`), and the cookie name. The optional binary
  serves it standalone for local testing.
- The portfolio server mounts it via `.nest("/drinks", ...)` and ships
  through the existing deploy pipeline — no separate service needed.
- Versions track the portfolio's: axum 0.8, askama 0.15 (rendered manually
  via `.render()` + `axum::response::Html`, matching portfolio convention),
  sqlx 0.8, chrono pinned `=0.4.34` (workspace-wide pin).
- **Path-prefix handling**: templates receive `base_path` from `Config` and
  build absolute URLs (`{base_path}/room/ABCD/sse`). Relative URLs are NOT
  used — from `/drinks/room/ABCD` a relative `sse` resolves to
  `/drinks/room/sse` (directory-based resolution), silently dropping the
  room segment.
- **Own SQLite file** (`drinkinggame.db`, env `DRINKS_DATABASE_URL`) — no
  schema entanglement with the portfolio DB; integration is purely at the
  routing layer. `.env.example` gains the new variable.
- **Scoped cookie name** (`dg_session`) to avoid colliding with the
  portfolio's WebAuthn session. Sessions are DB-backed (see data model),
  mirroring the portfolio's session pattern, with expiry cleanup run by the
  crate's own hourly background task.
- **Assets embedded**: the game's small CSS/JS (plus htmx + SSE extension)
  are compiled in via `include_str!` and served from crate routes, keeping
  the single-binary property. The portfolio's disk-served `/static/` is not
  used by the game.
- **sqlx queries are runtime-checked** (`sqlx::query` / `query_as` with
  `FromRow`), NOT compile-time `query!` macros. Rationale: macros resolve
  `DATABASE_URL` at compile time, and a second crate with a second database
  would need its own `.env` and `.sqlx` cache plus
  `cargo sqlx prepare --workspace` coordination — real complexity for zero
  runtime benefit in a schema this small. `SQLX_OFFLINE` builds are
  unaffected (nothing to check at compile time).
- Future single sign-on: `players` gains a nullable link column to the
  portfolio identity; nothing in v1 blocks this. Until then the game's
  name+PIN identity is fully independent of the portfolio's WebAuthn.

### Portfolio-side touchpoints

The game UI is standalone (full-screen party layout) and deliberately does
**not** extend the portfolio's `base.html` — this is an explicit exception
to the design-guide checklist, recorded here. The portfolio still gets:

- A hub-page link to `/drinks`.
- A `palette.js` COMMANDS entry for `/drinks`.
- CLAUDE.md updates: workspace build commands, new env var, nginx notes.

## Data model (SQLite)

All counts — session and lifetime — are derived by querying the append-only
event log; no stored counters.

- `players` — `id`, `name` (unique), `pin_hash` (argon2), `created_at`
- `sessions` — `id` (random token), `player_id`, `expires_at` (90 days),
  `created_at`. Backs the `dg_session` cookie; expired rows swept hourly.
- `rooms` — `id`, `code` (4-letter join code, unique among open rooms),
  `created_at`, `last_activity_at`, `ended_at`
- `room_players` — `room_id`, `player_id`, `joined_at`
- `events` — `id`, `room_id`, `player_id`, `kind` (`drink` | `shot`),
  `created_at`, `undone_at` (nullable). Genuinely append-only: **undo is a
  tombstone** — it sets `undone_at` on the caller's most recent live event
  in that room rather than deleting the row. Counts fold over events where
  `undone_at IS NULL`; future idle mechanics get complete history.

### Room lifecycle

Rooms end two ways (v1 has no admin tools, so this is load-bearing):

1. **Explicit**: an "End night" button on the player view sets `ended_at`.
2. **Inactivity timeout**: the hourly cleanup task ends rooms whose
   `last_activity_at` is older than 12 hours.

`last_activity_at` is bumped on join and on every event. Ended rooms free
their join code (uniqueness is scoped to open rooms), drop their broadcast
channel, and leave the tick loop's active set.

## Pages & routes

- `/` — name + PIN form (registers new names, verifies known ones), then
  create room or join by code. Sets long-lived `dg_session` cookie.
- `/room/:code` — player view: **+1 Drink**, **+1 Shot**, **Undo** buttons
  (large, drunk-proof), own counts, live leaderboard.
- `/room/:code/screen` — spectator view: room code displayed large, live
  leaderboard. Read-only, no auth.
- `POST /room/:code/event` — log a drink or shot (form field selects kind).
- `POST /room/:code/undo` — tombstone caller's most recent live event.
- `POST /room/:code/end` — end the room (any member).
- `GET /room/:code/sse` — SSE stream of rendered leaderboard fragments.

All paths above are relative to the mount prefix (`/drinks` in production).

## Deployment & nginx (manual steps — not CI/CD)

`deploy/nginx.conf` needs two additions, hand-copied to the server per the
existing process:

- **SSE location** (`location /drinks/` or a regex on `/sse$`):
  `proxy_buffering off;` and `proxy_read_timeout 1h;` — without this nginx
  buffers the stream and no events reach clients until the buffer flushes.
  The app also sends `X-Accel-Buffering: no` as belt-and-braces.
- **Rate limiting** on the game login POST, reusing the existing
  `limit_req_zone` pattern used for `/api/auth/` (10 req/min, burst 5):
  a 4-digit PIN is 10,000 combinations — argon2 slows each guess, but the
  proxy must throttle the attempt rate.

Deploy pipeline is otherwise unchanged: the workspace still produces
`target/release/drawingportfolio`; `deploy.yml` needs no path changes.

## Idle-mechanics extension point

A `mechanics` module owns:

1. a server-side tick loop — one global 1 Hz ticker that iterates the
   active-room set (bounded work; no per-room task spawning), and
2. a hook invoked on every logged event.

V1 ships both empty. Future mechanics fold over the event log (which is why
undo tombstones instead of deleting) and broadcast results through the same
SSE channel. Point-like mechanics need no schema change.

## Error handling

- PINs hashed with argon2; wrong PIN → friendly inline HTML error fragment.
- Unknown/ended room code → friendly error page with a link home.
- Domain errors are typed (`thiserror`); handlers map them to HTML fragments
  suitable for HTMX swaps rather than bare status codes.

## Testing

- Unit tests: count folding, undo semantics, room-code generation,
  PIN verify/register logic.
- Integration tests: handlers via `tower::ServiceExt` against in-memory
  SQLite (register/join/log/undo/leaderboard flows).
- Manual: SSE live-update behavior and phone-sized layout verified in a real
  browser before v1 is called done.

## Out of scope for v1

- Any actual idle mechanic (points, multipliers, upgrades, achievements)
- Admin tools (profile merge, room moderation)
- Portfolio single sign-on
- Native/PWA packaging
