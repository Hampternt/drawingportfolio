# Inventory

The live map of what this repo **is**, at feature altitude: what exists and
what it does — never how it's implemented (that lives in `CLAUDE.md` and
`docs/design.md`). Work in flight appears as a 🚧 pointer to its manifest in
the section where it will land; converting that placeholder into a real entry
is part of the definition of merged. Manifests live in `docs/manifests/`.

---

## Hub — `/`

The landing page: entry point linking to every public area of the site.

## Art portfolio — `/artportfolio`

The public showcase of drawings.

- Feed of posts grouped by month, with infinite "load more" paging.
- Filter rail: free-text search, tags, and curated collections — combinable,
  with live counts.
- Per-post permalinks and a JSON API of public posts.
- Visibility per post: public (listed), unlisted (permalink only), hidden.
- Admin dashboard (`/admin`, passkey-gated): upload with automatic image
  variants, edit captions/tags, manage collections and visibility.

## Drawing tasks — `/tasks`

A LeetCode-style practice board for drawing: reference images carrying any
number of task prompts, filterable by subject, difficulty, and task type.
Public to browse; admins manage images and tasks in place.

## Fitness — `/fitness` (private)

A personal nutrition and weight tracker, session-gated, dark-themed.

- Today view: calorie/macro targets ring, week strip, meals by slot
  (breakfast/lunch/dinner/snack/other).
- Week view: calorie bars, protein stats, logging streak, weight trend,
  most-logged foods.
- Food database with favourites, custom portions, and recipes; quick-log,
  copy-day, and barcode scanning that matches known products or prefills
  new ones from OpenFoodFacts.

## Drinks — `/drinks`

A party platform for phone-based drinking games in shared rooms.

- Rooms joined by name + PIN; three-tab phone shell (game / standings /
  room) plus a spectator "big screen" view with live standings.
- Three games per room: Ring of Fire, 3 Man, and Last Call. Last Call's
  beats advance when every player taps READY (no clock — the table sets
  its own pace); card swaps are free in the lobby, once a round after;
  staging and locking happen during the open Diplomacy talk beat.
- Test play mode (`DRINKS_TEST_MODE=1`): spawn fake players and hop
  between identities to drive every seat from one browser. Off — routes
  404 — unless the server opts in, so production can never expose it.
- Installable as a home-screen app (standalone, no browser bar) with an
  in-game FULL SCREEN toggle on Android browsers.
- Player accounts with self-service rename; per-viewer personalized UI;
  optional sound effects (server drop-in).

## Infrastructure

Single Rust/Axum binary, server-side rendered, SQLite storage, S3-compatible
object storage for images, passkey (WebAuthn) authentication for the single
admin. Deployed to a Hetzner server via GitHub Actions on push to master,
behind nginx. Quality gates: `scripts/check.sh` (item) and
`scripts/verify.sh` (pack).

---

*Nothing in transit right now.*
