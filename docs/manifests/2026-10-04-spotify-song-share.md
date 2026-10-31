# Pack — Spotify song sharing

**Status:** PARKED — idea manifest only, nothing scheduled. Raised 2026-10-04
and set aside by the owner. Open the decisions below before scoping items.
**Scope:** pack (likely; could split into two if the inbox grows)
**Goal:** friends share songs with each other on the site, and anyone can tap
**Queue** to drop a shared song into the Spotify they are already listening to
— no web player, no interrupted song, no 30-second embed.

The problem it solves: today a shared Spotify link means opening a web page,
which often interrupts the current song, then queueing it from the web UI.
The site should do the queueing for you.

---

## The idea in one picture

1. A friend pastes a Spotify track link on the site.
2. The server fetches title + cover art from Spotify's public oEmbed endpoint
   (no auth) and stores the track id.
3. You see it in a list of shared songs and tap **Queue**.
4. The server calls Spotify's add-to-queue endpoint *as you*; your phone keeps
   playing and the song is up next.

<details>
<summary>What Spotify offers, and what it costs (checked 2026-10-04)</summary>

- **oEmbed** — `https://open.spotify.com/oembed?url=<track link>`, no app, no
  auth. Gives title, thumbnail and an embed iframe. Enough for sharing.
- **Add to queue** — `POST /me/player/queue?uri=spotify:track:<id>`, scope
  `user-modify-playback-state`. Survived the February 2026 API changes.
- **User OAuth** — Authorization Code flow, one connect per person, refresh
  token stored server-side.

Hard limits:

- **Premium only.** Every player endpoint, queue included, needs the person
  pressing Queue to have Spotify Premium. Non-Premium friends can share, not
  queue.
- **5 users per app** in Development Mode since March 2026 (was 25); the app
  owner also needs Premium. Extended quota requires a registered business with
  250k MAU, so 5 is the real ceiling — plan on it including the owner.
- **An active device is required.** With Spotify closed everywhere the call
  fails with `NO_ACTIVE_DEVICE`; the site should say "open Spotify first".
- Spotify keeps removing endpoints (recommendations, audio features, related
  artists, bulk track lookup are gone) — build on oEmbed + queue only.

Zero-build alternative worth remembering: Spotify's own **Jam** is a shared
live queue when everyone is listening together. This feature earns its keep
for *asynchronous* sharing.

Sources: [Feb 2026 changelog](https://developer.spotify.com/documentation/web-api/references/changes/february-2026),
[add to queue](https://developer.spotify.com/console/post-queue),
[scopes](https://developer.spotify.com/documentation/web-api/concepts/scopes),
[TechCrunch on the 5-user cap](https://techcrunch.com/2026/02/06/spotify-changes-developer-mode-api-to-require-premium-accounts-limits-test-users/).

</details>

## Open decisions (answer before scoping)

1. **Who uses it** — how many friends, and do they all have Premium? Decides
   whether the 5-user cap bites.
2. **Shape** — one shared feed everyone posts to, or songs sent to specific
   people (an inbox)?
3. **Extras** — "already queued" marks, reactions/comments, a now-playing
   display? Or share + queue only for v1?
4. **Where it lives** — its own section (e.g. `/music`) or part of an existing
   one?

## Rough shape (not contracts — written when this is picked up)

<details>
<summary>Likely pieces</summary>

- **Spotify connect** — OAuth connect/disconnect on the account page, token +
  refresh token stored per user, refresh on expiry. Risky territory: stored
  third-party credentials → flag for an auth-lens review.
- **Shared songs** — migration for a songs table keyed by `user_id`, paste
  form, oEmbed fetch, list view. Session-gated like `/fitness`
  (`AuthSession`), data scoped from `session.user()`.
- **Queue action** — HTMX button → server calls add-to-queue → inline
  success / "open Spotify first" / "needs Premium" states.
- **New-section checklist** from `CLAUDE.md`: route module + `main.rs`,
  migration, nav link, `palette.js` command, `style.css` section, hub tile.
- **Config** — `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET`,
  `SPOTIFY_REDIRECT_URI` in `.env.example`; redirect URI registered in the
  Spotify dashboard for both localhost and the live domain.

</details>

Agent brief: repo `CLAUDE.md`, this manifest, `src/routes/nutrition.rs` for
per-user scoping, `src/middleware.rs` for extractor choice. No dependency on
other packs.
Agents: not estimated — parked.

## Ledger

- 2026-10-04 — idea manifest written; parked by the owner.
