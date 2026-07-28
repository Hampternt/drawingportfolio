# Drinking Game v1 — Design

Date: 2026-07-28
Status: approved pending user review

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
- **Stack**: Axum + SQLite (sqlx) + Askama templates + HTMX with SSE
  extension. Single self-contained binary (assets embedded via rust-embed).

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

- Library exposes `pub fn router(state: AppState) -> axum::Router` plus
  config (DB path, cookie name). The optional binary serves it standalone
  for local testing.
- The portfolio server mounts it via `.nest("/drinks", ...)` and ships
  through the existing deploy pipeline — no separate service needed.
- Versions track the portfolio's: axum 0.8, askama 0.15 (rendered manually
  via `.render()` + `axum::response::Html`, matching portfolio convention),
  sqlx 0.8.
- **Path-prefix safe**: all links, form actions, SSE URLs, and asset
  references are relative — no hardcoded absolute paths.
- **Own SQLite file** (`drinkinggame.db`) — no schema entanglement with the
  portfolio DB; integration is purely at the routing layer.
- **Scoped cookie name** (`dg_session`) to avoid colliding with the
  portfolio's WebAuthn session.
- Future single sign-on: `players` gains a nullable link column to the
  portfolio identity; nothing in v1 blocks this. Until then the game's
  name+PIN identity is fully independent of the portfolio's WebAuthn.

## Data model (SQLite)

All counts — session and lifetime — are derived by querying the append-only
event log; no stored counters.

- `players` — `id`, `name` (unique), `pin_hash` (argon2), `created_at`
- `rooms` — `id`, `code` (4-letter join code, unique among open rooms),
  `created_at`, `ended_at`
- `room_players` — `room_id`, `player_id`, `joined_at`
- `events` — `id`, `room_id`, `player_id`, `kind` (`drink` | `shot`),
  `created_at`. Append-only. Undo deletes the caller's most recent event in
  that room.

## Pages & routes

- `/` — name + PIN form (registers new names, verifies known ones), then
  create room or join by code. Sets long-lived `dg_session` cookie.
- `/room/:code` — player view: **+1 Drink**, **+1 Shot**, **Undo** buttons
  (large, drunk-proof), own counts, live leaderboard.
- `/room/:code/screen` — spectator view: room code displayed large, live
  leaderboard. Read-only, no auth.
- `POST /room/:code/event` — log a drink or shot (form field selects kind).
- `POST /room/:code/undo` — remove caller's most recent event in this room.
- `GET /room/:code/sse` — SSE stream of rendered leaderboard fragments.

## Idle-mechanics extension point

A `mechanics` module owns:

1. a server-side tick loop (~1 Hz per active room), and
2. a hook invoked on every logged event.

V1 ships both empty. Future mechanics fold over the event log (which is why
the log is append-only) and broadcast results through the same SSE channel.
Point-like mechanics need no schema change.

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
